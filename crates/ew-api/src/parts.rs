// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parts, the one place for parts (doc 07 §5): what its list finds ([`Session::parts_find`]), where a part lives
//! ([`Lives`], doc 07 §5.8), what the open sheet lacks ([`Session::sheet_lacks`]) and Keep in my parts
//! ([`Session::keep_in_my_parts`]). The open part is a [`PartSheet`]: the part as a value with the texts as typed and
//! their messages, read, checked and saved by the session's part editing.

use ew_derive::part_fields::Lack;
use ew_ids::{Id, Uuid, v5};
use ew_library::Tier;
use ew_schema::{Device, Kind, LibCopy, LibDocument, Part, Sha256};
use serde_json::{Value, json};

use crate::part_editor::{PartDraftError, PartSheet, message};
use crate::part_offers::lib_ref;
use crate::{AnyCommand, OfferView, PartOffer, PartOfferQuery, PartPick, Session};

/// Where a part lives, as a person sees it (doc 07 §5.8).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Lives {
    /// A part of My parts: his, changed freely, there in his next project.
    Mine,
    /// A starter part, or a part of a library he cannot write, by the library's name: never changed, made his own.
    NotMine {
        /// The library's name.
        library: String,
        /// Whether it is a part of the program's own starter content.
        starter: bool,
    },
    /// A part the project holds that no library he can read has: changed freely, for this project.
    OnlyHere,
    /// A new part, not saved yet.
    New,
}

/// The list's scopes (doc 07 §5.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PartScope {
    /// Every part: the project's and the libraries'.
    #[default]
    All,
    /// The parts the project holds, used or not.
    Project,
    /// The parts of My parts.
    Mine,
}

impl PartScope {
    /// The scopes, in the list's order.
    pub const ALL: [PartScope; 3] = [PartScope::All, PartScope::Project, PartScope::Mine];

    /// The scope's word in `parts.find`.
    pub fn word(self) -> &'static str {
        match self {
            PartScope::All => "all",
            PartScope::Project => "project",
            PartScope::Mine => "mine",
        }
    }
}

/// A row of the list: the part as the choosers offer it, where it lives and whether it lacks something.
#[derive(Clone, PartialEq, Debug)]
pub struct PartFound {
    /// The part, its numbers and its description.
    pub offer: PartOffer,
    /// Where it lives.
    pub lives: Lives,
    /// The copy ID it has, or would have, in the project.
    pub copy: Uuid,
    /// Whether a feature lacks a datum of it ([`ew_derive::part_fields::lacks`]).
    pub lacks: bool,
}

impl PartFound {
    /// The part as `parts.open` names it: `id:<copy>` for a part the project holds, `lib:<library>/<item>` else.
    pub fn param(&self) -> String {
        match self.offer.pick {
            PartPick::Held(c) => format!("id:{c}"),
            PartPick::Library { library, item } => format!("lib:{library}/{item}"),
        }
    }
}

impl Session {
    /// Where a part of library `library` lives: My parts, the project, or a library he cannot write (the starter's
    /// one of them).
    pub(crate) fn lives_in(&self, library: Uuid) -> Lives {
        if library == self.query().project().id.as_uuid() {
            return Lives::OnlyHere;
        }
        let registry = self.libraries();
        match registry.libraries().into_iter().find(|l| l.id == library) {
            Some(l) if l.key == crate::eplan::USER_PARTS_KEY && l.writable => Lives::Mine,
            Some(l) => Lives::NotMine {
                starter: l.key == ew_library::STARTER_PARTS_KEY || l.tier == Tier::System,
                library: l.name,
            },
            None => Lives::NotMine {
                library: library.to_string(),
                starter: false,
            },
        }
    }

    /// Where the open sheet's part lives.
    pub fn sheet_lives(&self, sheet: &PartSheet) -> Lives {
        match (sheet.new, sheet.source) {
            (false, Some((library, _))) => self.lives_in(library),
            _ => Lives::New,
        }
    }

    /// The part of the open sheet as its texts give it now, else as it was opened.
    pub fn sheet_part(&self, sheet: &PartSheet) -> Part {
        if sheet.new || sheet.own {
            if let Ok(p) = self.sectioned_item(sheet) {
                return p;
            }
        }
        self.draft_part(sheet)
            .map(|(p, _)| p)
            .unwrap_or_else(|_| sheet.original.clone())
    }

    /// What the open sheet's part lacks, as Ready for and the fields show it.
    pub fn sheet_lacks(&self, sheet: &PartSheet) -> Vec<Lack> {
        ew_derive::part_fields::lacks(&self.sheet_part(sheet))
    }

    /// The list of Parts: the parts that match `text` (as the choosers match it) in `scope`, of `kind` when given,
    /// only those that lack something with `lacking`, each with where it lives; at most `limit` rows.
    pub fn parts_find(
        &self,
        text: &str,
        scope: PartScope,
        kind: Option<&str>,
        lacking: bool,
        lang: Option<String>,
        limit: usize,
    ) -> Vec<PartFound> {
        let offers = self.part_offers(&PartOfferQuery {
            text: text.to_owned(),
            kinds: kind.map(|k| vec![k.to_owned()]).unwrap_or_default(),
            view: if scope == PartScope::Project {
                OfferView::Project
            } else {
                OfferView::All
            },
            include_inactive: true,
            lang,
            ..PartOfferQuery::default()
        });
        let q = self.query();
        let mut out = Vec::new();
        for offer in offers {
            let (library, part, copy) = match offer.pick {
                PartPick::Held(c) => {
                    let Some(copy) = q.parts().get(Id::from_uuid(c)) else {
                        continue;
                    };
                    (copy.library_id.as_uuid(), copy.item.clone(), c)
                }
                PartPick::Library { library, item } => {
                    let Ok(LibDocument::Part(c)) = self.libraries().resolve(&lib_ref(library, item)) else {
                        continue;
                    };
                    let copy: Id<Part> = v5::lib_copy_id(library, item);
                    (library, c.item, copy.as_uuid())
                }
            };
            let lives = self.lives_in(library);
            if scope == PartScope::Mine && lives != Lives::Mine {
                continue;
            }
            let lacks = !ew_derive::part_fields::lacks(&part).is_empty();
            if lacking && !lacks {
                continue;
            }
            out.push(PartFound {
                offer,
                lives,
                copy,
                lacks,
            });
            if out.len() >= limit {
                break;
            }
        }
        out
    }

    /// Keep in my parts (doc 07 §5.8): the part of the project's own copy `copy` written into My parts as an item of
    /// its own, and the command that gives the project that copy: `part.assign` (replace) for the devices that use the
    /// project's copy, else `library.embed`. The project's own copy stays, unused, until purged.
    ///
    /// # Errors
    ///
    /// [`PartDraftError::Refused`] (`reason-part-keep`) when `copy` is not a part only in this project;
    /// [`PartDraftError::Field`] on `part` when My parts cannot be written or the part is in it already.
    pub fn keep_in_my_parts(&mut self, copy: Uuid) -> Result<AnyCommand, PartDraftError> {
        let held = {
            let q = self.query();
            q.parts()
                .get(Id::from_uuid(copy))
                .filter(|c| c.library_id.as_uuid() == q.project().id.as_uuid())
                .map(|c| (**c).clone())
        };
        let held = held.ok_or(PartDraftError::Refused(key!("reason-part-keep")))?;
        let lib = crate::eplan::user_parts_library(self.libraries_mut()).map_err(|e| {
            PartDraftError::Field(message("part", key!("error-part-library"), &["", &e.to_string()]))
        })?;
        let name = self
            .libraries()
            .libraries()
            .into_iter()
            .find(|l| l.id == lib)
            .map(|l| l.name)
            .unwrap_or_default();
        let failed = |e: String| PartDraftError::Field(message("part", key!("error-part-library"), &[&name, &e]));
        let maker = ew_derive::manufacturer_name(&self.query(), &held.item.manufacturer);
        let mine: Vec<Part> = self
            .libraries()
            .documents(lib, Kind::Part)
            .iter()
            .filter_map(|d| match d {
                LibDocument::Part(c) => Some(c.item.clone()),
                _ => None,
            })
            .collect();
        if self.same_part(&mine, held.item.part_number.as_str(), &maker) {
            return Err(PartDraftError::Field(message(
                "part_number",
                key!("error-part-exists"),
                &[held.item.part_number.as_str(), &maker],
            )));
        }
        let mut item = held.item.clone();
        item.meta.extra.remove("source");
        let mut written = Vec::new();
        item.manufacturer = self.my_maker(lib, &maker, &mut written)?;
        let item_id = held.item_id.as_uuid();
        let kept = crate::part_new::finish(
            LibCopy {
                id: v5::lib_copy_id(lib, item_id),
                v: 1,
                library_id: Id::from_uuid(lib),
                item_id: held.item_id,
                version: held.version.clone(),
                sha256: Sha256([0; 32]),
                item,
            },
            LibDocument::Part,
        );
        written.push(LibDocument::Part(kept.clone()));
        self.libraries_mut()
            .write(lib, &written)
            .map_err(|e| failed(e.to_string()))?;
        let docs = self
            .closure_docs(&[lib_ref(lib, item_id)])
            .map_err(|e| failed(e.to_string()))?;
        let copies: Vec<Value> = docs.iter().filter_map(|d| serde_json::to_value(d).ok()).collect();
        let users: Vec<Id<Device>> = {
            let q = self.query();
            q.devices()
                .iter()
                .filter(|(_, d)| d.parts.iter().any(|u| u.part.copy == held.id))
                .map(|(id, _)| id)
                .collect()
        };
        let command = if users.is_empty() {
            AnyCommand {
                name: "library.embed".into(),
                params: json!({ "copies": copies }),
            }
        } else {
            AnyCommand {
                name: "part.assign".into(),
                params: json!({
                    "devices": users,
                    "part": kept.id,
                    "copies": copies,
                    "mode": "replace",
                    "from": held.id,
                }),
            }
        };
        Ok(command)
    }
}
