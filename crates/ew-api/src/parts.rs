// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parts, the one place for parts (doc 07 §5): what its list finds ([`Session::parts_find`]), where a part lives
//! ([`Lives`], doc 07 §5.8), what the open sheet lacks ([`Session::sheet_lacks`]) and Keep in my parts
//! ([`Session::keep_in_my_parts`]). The open part is a [`PartSheet`]: the part as a value with the texts as typed and
//! their messages, read, checked and saved by the session's part editing.

use ew_derive::part_fields::{Group as G, Lack};
use ew_ids::{Id, Uuid, v5};
use ew_library::Tier;
use ew_schema::{Kind, LibCopy, LibDocument, Part, Sha256};
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
    /// The set an import has just brought, while it applies (doc 07 §5.9): its rows, not found by a search.
    Import,
    /// The project's workshop stock (doc 07 §5.10): the parts of In this project that stand in it, in its order.
    Stock,
}

impl PartScope {
    /// The scopes, in the list's order.
    pub const ALL: [PartScope; 5] = [
        PartScope::All,
        PartScope::Project,
        PartScope::Mine,
        PartScope::Import,
        PartScope::Stock,
    ];

    /// The scopes' words, in the list's order: the closed set `parts.find`'s `scope` takes.
    pub const WORDS: [&'static str; 5] = {
        let mut out = [""; 5];
        let mut i = 0;
        while i < out.len() {
            out[i] = PartScope::ALL[i].word();
            i += 1;
        }
        out
    };

    /// The scope's word in `parts.find`.
    pub const fn word(self) -> &'static str {
        match self {
            PartScope::All => "all",
            PartScope::Project => "project",
            PartScope::Mine => "mine",
            PartScope::Import => "import",
            PartScope::Stock => "stock",
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
    /// one of them). A copy of a library that is not installed (a colleague's parts, a library removed) is only in this
    /// project: the project carries it, and it is his to change and to keep (doc 07 §5.8).
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
            None => Lives::OnlyHere,
        }
    }

    /// Whether library `library` is installed.
    pub(crate) fn installed(&self, library: Uuid) -> bool {
        self.libraries().libraries().iter().any(|l| l.id == library)
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
        if (sheet.new || sheet.own)
            && let Ok(p) = self.sectioned_item(sheet)
        {
            return p;
        }
        self.draft_part(sheet)
            .map(|(p, _)| p)
            .unwrap_or_else(|_| sheet.original.clone())
    }

    /// What the open sheet's part lacks, as Ready for and the fields show it.
    pub fn sheet_lacks(&self, sheet: &PartSheet) -> Vec<Lack> {
        ew_derive::part_fields::lacks(&self.sheet_part(sheet))
    }

    /// The devices of the project that use the held copy `copy`, each with its tag, in the tags' order.
    pub fn part_users(&self, copy: Uuid) -> Vec<(Uuid, String)> {
        let q = self.query();
        let mut out: Vec<(Uuid, String)> = q
            .devices()
            .iter()
            .filter(|(_, d)| d.parts.iter().any(|u| u.part.copy.as_uuid() == copy))
            .map(|(id, _)| (id.as_uuid(), ew_derive::short_tag(&q, id)))
            .collect();
        out.sort_by(|a, b| ew_mltext::natural_cmp(&a.1, &b.1));
        out
    }

    /// Ready for of the open sheet's part: every feature that reads it, one entry each.
    pub fn sheet_ready(&self, sheet: &PartSheet) -> Vec<ew_derive::part_fields::Ready> {
        ew_derive::part_fields::ready_for(&self.sheet_part(sheet))
    }

    /// The list of Parts: the parts that match `text` (as the choosers match it) in `scope`, of `kind` when given,
    /// only those that lack something with `lacking`, each with where it lives; at most `limit` rows. The scope of an
    /// import's set finds nothing: its rows are the set's; the stock's finds its parts in its order.
    pub fn parts_find(
        &self,
        text: &str,
        scope: PartScope,
        kind: Option<&str>,
        lacking: bool,
        lang: Option<String>,
        limit: usize,
    ) -> Vec<PartFound> {
        if scope == PartScope::Import {
            return Vec::new();
        }
        let offers = self.part_offers(&PartOfferQuery {
            text: text.to_owned(),
            // Every kind, a cable gland too, which a device's choosers leave out.
            kinds: kind.map_or_else(crate::part_kinds, |k| vec![k.to_owned()]),
            view: if matches!(scope, PartScope::Project | PartScope::Stock) {
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
            if out.len() >= limit && scope != PartScope::Stock {
                break;
            }
        }
        // The stock: its parts in its order of preference.
        if scope == PartScope::Stock {
            let stock = self.stock();
            out.retain(|f| stock.contains(&f.copy));
            out.sort_by_key(|f| stock.iter().position(|c| *c == f.copy));
            out.truncate(limit);
        }
        out
    }

    /// Whether a copy of library `library` is only in this project: the library is the project's own, or one this
    /// window does not have (doc 07 §3.1).
    pub(crate) fn only_here(&self, library: Uuid) -> bool {
        library == self.query().project().id.as_uuid() || !self.installed(library)
    }

    /// Keep in my parts (doc 07 §5.8), one part or several: the parts of the project's copies `copies` (each only in this
    /// project) written into My parts as items of their own, with the parts only in this project that they name (their
    /// plate, what they come with, a piece's symbol, their bus symbol), all in one write; and the `part.keep` that
    /// embeds them and moves every use of each project's copy to its kept one with what it chose, in one undo step.
    /// The project's own copies stay, unused, until purged.
    ///
    /// # Errors
    ///
    /// [`PartDraftError::Refused`] (`reason-part-keep`) when a copy is not a part only in this project;
    /// [`PartDraftError::Field`] on `part` when My parts cannot be made or written, or on `part_number` when My parts
    /// holds another part of the maker and number of one of them; nothing is written then.
    pub fn keep_in_my_parts(&mut self, copies: &[Uuid]) -> Result<KeptParts, PartDraftError> {
        let held: Vec<LibCopy<Part>> = {
            let q = self.query();
            copies
                .iter()
                .map(|c| {
                    q.parts()
                        .get(Id::from_uuid(*c))
                        .filter(|c| self.only_here(c.library_id.as_uuid()))
                        .map(|c| (**c).clone())
                })
                .collect::<Option<_>>()
                .ok_or(PartDraftError::Refused(key!("reason-part-keep")))?
        };
        let lib = crate::eplan::user_parts_library(self.libraries_mut())
            .map_err(|_| PartDraftError::Field(message("part", key!("error-part-keep-no-my-parts"), &[])))?;
        // A part kept before is in My parts already: said in words, as a new part of its maker and number is.
        let mine = self.libraries().documents(lib, Kind::Part);
        if let Some(c) = held.iter().find(|c| {
            mine.iter()
                .any(|d| d.copy_meta().is_some_and(|m| m.item_id.as_uuid() == c.item_id.as_uuid()))
        }) {
            let maker = ew_derive::manufacturer_name(&self.query(), &c.item.manufacturer);
            return Err(PartDraftError::Field(message(
                "part_number",
                key!("error-part-exists"),
                &[c.item.part_number.as_str(), &maker],
            )));
        }
        let mut kept = Kept::default();
        let mut tos = Vec::new();
        for c in &held {
            let (item, _) = self.keep_item(c.library_id.as_uuid(), c.item_id.as_uuid(), lib, &mut kept)?;
            tos.push(v5::lib_copy_id::<Part>(lib, item).as_uuid());
        }
        // The parts named beside them are said; the parts asked for are not.
        kept.with.retain(|(item, _)| !held.iter().any(|c| c.item_id.as_uuid() == *item));
        let refs: Vec<_> = held.iter().map(|c| lib_ref(lib, c.item_id.as_uuid())).collect();
        let copies = self.write_kept(lib, &kept, &refs)?;
        let mut moves = kept.moves.clone();
        // The first part asked for is the step's own.
        let first = moves
            .iter()
            .position(|(from, _)| *from == held[0].id.as_uuid())
            .map_or_else(|| (held[0].id.as_uuid(), tos[0]), |i| moves.remove(i));
        Ok(KeptParts {
            command: keep_command(first, &moves, copies),
            with: kept.with.into_iter().map(|(_, w)| w).collect(),
        })
    }

    /// Points every reference of `value` (a part's, a symbol's or a function definition's `lib/` JSON form, its maker
    /// aside) that names a copy only in this project at that item kept in My parts `lib`, keeping it into `kept` with
    /// what it names in turn ([`Session::keep_item`]). A reference to an installed library stays as it is; one that
    /// names nothing the project holds stays too, and the closure then says so.
    pub(crate) fn keep_named(&self, value: &mut Value, lib: Uuid, kept: &mut Kept) -> Result<(), PartDraftError> {
        match value {
            Value::Object(m) => {
                let uuid = |k: &str| m.get(k).and_then(Value::as_str).and_then(|s| s.parse::<Uuid>().ok());
                if let (Some(library), Some(item), Some(_)) =
                    (uuid("library"), uuid("item"), m.get("version_req"))
                {
                    if self.only_here(library) && self.held_doc(library, item).is_some() {
                        let (item, version) = self.keep_item(library, item, lib, kept)?;
                        let minor = version.split('.').take(2).collect::<Vec<_>>().join(".");
                        m.insert("library".into(), Value::String(lib.to_string()));
                        m.insert("item".into(), Value::String(item.to_string()));
                        m.insert("version_req".into(), Value::String(format!("^{minor}")));
                    }
                    return Ok(());
                }
                for (k, v) in m.iter_mut() {
                    if k != "manufacturer" && k != "meta" {
                        self.keep_named(v, lib, kept)?;
                    }
                }
            }
            Value::Array(items) => {
                for v in items {
                    self.keep_named(v, lib, kept)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// The copy the project holds of item `item` of library `library`: a part, a symbol or a function definition.
    fn held_doc(&self, library: Uuid, item: Uuid) -> Option<LibDocument> {
        let q = self.query();
        let of = |l: Uuid, i: Uuid| l == library && i == item;
        (q.parts().iter())
            .find(|(_, c)| of(c.library_id.as_uuid(), c.item_id.as_uuid()))
            .map(|(_, c)| LibDocument::Part((**c).clone()))
            .or_else(|| {
                (q.symbols().iter())
                    .find(|(_, c)| of(c.library_id.as_uuid(), c.item_id.as_uuid()))
                    .map(|(_, c)| LibDocument::Symbol((**c).clone()))
            })
            .or_else(|| {
                (q.fndefs().iter())
                    .find(|(_, c)| of(c.library_id.as_uuid(), c.item_id.as_uuid()))
                    .map(|(_, c)| LibDocument::Fndef((**c).clone()))
            })
    }

    /// Keeps the project's copy of item `item` of library `library` (only in this project) in My parts `lib` with what
    /// it names: the item and version My parts holds it under then. My parts holding it already (kept before, or in
    /// this keep) leaves it there; a part of its maker and number that My parts holds as another item refuses it.
    fn keep_item(
        &self,
        library: Uuid,
        item: Uuid,
        lib: Uuid,
        kept: &mut Kept,
    ) -> Result<(Uuid, String), PartDraftError> {
        if let Some(v) = kept.versions.get(&item) {
            return Ok((item, v.clone()));
        }
        let doc = self
            .held_doc(library, item)
            .ok_or(PartDraftError::Refused(key!("reason-part-keep")))?;
        let kind = doc.kind().unwrap_or(Kind::Part);
        let copy = doc.copy_meta().map(|c| (c.id, c.version.to_owned()));
        let mine = self.libraries().documents(lib, kind);
        if let Some(c) = mine.iter().find_map(|d| d.copy_meta().filter(|c| c.item_id.as_uuid() == item)) {
            if let (LibDocument::Part(_), Some((from, _))) = (&doc, &copy) {
                kept.moves.push((*from, c.id));
            }
            return Ok((item, c.version.to_owned()));
        }
        let version = copy.as_ref().map(|(_, v)| v.clone()).unwrap_or_default();
        kept.versions.insert(item, version.clone());
        let q = self.query();
        match doc {
            LibDocument::Part(c) => {
                let maker = ew_derive::manufacturer_name(&q, &c.item.manufacturer);
                let number = c.item.part_number.as_str().to_owned();
                let parts: Vec<Part> = mine
                    .iter()
                    .filter_map(|d| match d {
                        LibDocument::Part(c) => Some(c.item.clone()),
                        _ => None,
                    })
                    .collect();
                if self.same_part(&parts, &number, &maker) {
                    return Err(PartDraftError::Field(message(
                        "part_number",
                        key!("error-part-exists"),
                        &[&number, &maker],
                    )));
                }
                drop(q);
                let mut new = self.rehome(&c, lib, kept)?;
                new.item.meta.extra.remove("source");
                new.item.manufacturer = self.my_maker(lib, &maker, &mut kept.written)?;
                let new = crate::part_new::finish(new, LibDocument::Part);
                kept.moves.push((c.id.as_uuid(), new.id.as_uuid()));
                kept.with.push((item, format!("{maker} {number}").trim().to_owned()));
                kept.written.push(LibDocument::Part(new));
            }
            LibDocument::Symbol(c) => {
                drop(q);
                let new = crate::part_new::finish(self.rehome(&c, lib, kept)?, LibDocument::Symbol);
                kept.written.push(LibDocument::Symbol(new));
            }
            LibDocument::Fndef(c) => {
                drop(q);
                let new = crate::part_new::finish(self.rehome(&c, lib, kept)?, LibDocument::Fndef);
                kept.written.push(LibDocument::Fndef(new));
            }
            _ => return Err(PartDraftError::Refused(key!("reason-part-keep"))),
        }
        Ok((item, version))
    }

    /// The project's copy `c` as the same item of My parts `lib`, its references to copies only in this project kept
    /// with it; its hash is the caller's to take.
    fn rehome<T>(&self, c: &LibCopy<T>, lib: Uuid, kept: &mut Kept) -> Result<LibCopy<T>, PartDraftError>
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        let bad = |e: serde_json::Error| {
            PartDraftError::Field(message("part", key!("error-part-value"), &[&e.to_string()]))
        };
        let mut value = serde_json::to_value(&c.item).map_err(bad)?;
        self.keep_named(&mut value, lib, kept)?;
        Ok(LibCopy {
            id: v5::lib_copy_id(lib, c.item_id.as_uuid()),
            v: c.v,
            library_id: Id::from_uuid(lib),
            item_id: c.item_id,
            version: c.version.clone(),
            sha256: Sha256([0; 32]),
            item: serde_json::from_value(value).map_err(bad)?,
        })
    }

    /// Writes what `kept` keeps into My parts `lib` in one write and reads the closure of `refs` (the parts asked for)
    /// back from it: the documents a command embeds. Where the closure cannot be read, what was written is taken out
    /// again and the refusal names the part in words.
    pub(crate) fn write_kept(
        &mut self,
        lib: Uuid,
        kept: &Kept,
        refs: &[ew_schema::LibRef],
    ) -> Result<Vec<Value>, PartDraftError> {
        let name = self
            .libraries()
            .libraries()
            .into_iter()
            .find(|l| l.id == lib)
            .map(|l| l.name)
            .unwrap_or_default();
        if !kept.written.is_empty() {
            self.libraries_mut().write(lib, &kept.written).map_err(|e| {
                PartDraftError::Field(message(
                    "part",
                    key!("error-part-library"),
                    &[&name, &e.to_string()],
                ))
            })?;
        }
        // The parts kept beside them come into the project too, whatever names them (their uses move to them).
        let mut refs = refs.to_vec();
        refs.extend(kept.written.iter().filter_map(|d| match d {
            LibDocument::Part(c) => Some(lib_ref(lib, c.item_id.as_uuid())),
            _ => None,
        }));
        match self.closure_docs(&refs) {
            Ok(docs) => Ok(docs.iter().filter_map(|d| serde_json::to_value(d).ok()).collect()),
            Err(e) => {
                let mut by_kind: std::collections::BTreeMap<Kind, Vec<Uuid>> = Default::default();
                for d in &kept.written {
                    if let (Some(k), Some(c)) = (d.kind(), d.copy_meta()) {
                        by_kind.entry(k).or_default().push(c.item_id.as_uuid());
                    }
                }
                for (k, items) in by_kind {
                    let _ = self.libraries_mut().remove(lib, k, &items);
                }
                // In words, never by the reference's identifiers.
                Err(match e {
                    crate::ApiError::Library { .. } => {
                        PartDraftError::Field(message("part", key!("error-part-names-unknown"), &[]))
                    }
                    other => PartDraftError::Field(message(
                        "part",
                        key!("error-part-library"),
                        &[&name, &other.to_string()],
                    )),
                })
            }
        }
    }
}

/// What Keep in my parts does ([`Session::keep_in_my_parts`]): the command that brings the kept parts into the project,
/// and the parts only in this project kept with them, each by its maker and number ("Example EX-EP-1").
#[derive(Clone, Debug)]
pub struct KeptParts {
    /// The `part.keep` of the kept parts.
    pub command: AnyCommand,
    /// The parts kept with them, by maker and number, in the order they were kept.
    pub with: Vec<String>,
}

/// The parts only in this project that keeping a part in My parts brings with it (doc 07 §3.1): what is written into
/// My parts in the one write, the uses that move from each project's copy to its kept copy, the parts kept by item with
/// their maker and number, and the version each kept item has.
#[derive(Default)]
pub(crate) struct Kept {
    /// The documents to write into My parts.
    pub(crate) written: Vec<LibDocument>,
    /// Each project's copy kept, and the copy of My parts that takes its uses.
    pub(crate) moves: Vec<(Uuid, Uuid)>,
    /// Each part kept by its item, with its maker and number.
    pub(crate) with: Vec<(Uuid, String)>,
    /// The version of each item kept in this write.
    versions: std::collections::BTreeMap<Uuid, String>,
}

/// The `part.keep` of the kept copies: `first` the step's own, `also` the others.
pub(crate) fn keep_command(first: (Uuid, Uuid), also: &[(Uuid, Uuid)], copies: Vec<Value>) -> AnyCommand {
    let also: Vec<Value> = also
        .iter()
        .map(|(from, to)| json!({"from": from, "to": to}))
        .collect();
    let mut params = json!({ "from": first.0, "to": first.1, "copies": copies });
    if !also.is_empty() {
        params["also"] = Value::Array(also);
    }
    AnyCommand {
        name: "part.keep".into(),
        params,
    }
}

/// The groups of the first question's cards, in its order (doc 07 §5.5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StartGroup {
    /// Devices.
    Devices,
    /// Terminals, cables and wiring.
    Wiring,
    /// Cabinet.
    Cabinet,
    /// Pneumatics.
    Fluid,
}

impl StartGroup {
    /// Every group, in the question's order.
    pub const ALL: [StartGroup; 4] = [
        StartGroup::Devices,
        StartGroup::Wiring,
        StartGroup::Cabinet,
        StartGroup::Fluid,
    ];

    /// The group's heading.
    pub fn word(self) -> &'static str {
        match self {
            StartGroup::Devices => key!("part-starts-devices"),
            StartGroup::Wiring => key!("part-starts-wiring"),
            StartGroup::Cabinet => key!("part-starts-cabinet"),
            StartGroup::Fluid => key!("part-starts-fluid"),
        }
    }
}

/// The section a start begins a part with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StartSection {
    /// The pneumatic section: a valve's ports and coil, a cylinder's ports.
    Fluid,
    /// The enclosure section: a cabinet's size, plate and doors.
    Enclosure,
}

/// A start's card of the first question: its group, its title, the line of what it will be asked for, its icon.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StartCard {
    /// The group it stands in.
    pub group: StartGroup,
    /// Its title's key.
    pub word: &'static str,
    /// The key of its line.
    pub line: &'static str,
    /// The window's icon name.
    pub icon: &'static str,
}

/// What a new part starts as (doc 07 §5.5): a card of the first question, or a start a form names (a station's head,
/// a bus line's plug). The answer decides what the sheet asks for: its kind and its presets, and the groups of a
/// device it asks for beyond those every device has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Start {
    /// The start's name in a command (`parts.new {start}`).
    pub name: &'static str,
    /// Its card; `None` for a start only a form names.
    pub card: Option<StartCard>,
    /// The fields it presets, as the sheet's fields and their texts (`kind` first).
    pub presets: &'static [(&'static str, &'static str)],
    /// The groups of a device it asks for that a device without data in them does not show: what it is in the PLC,
    /// its network ports.
    pub asks: &'static [G],
    /// The section of its kind the part starts with, empty: its group asks for it, and the feature that reads it
    /// takes the part (Enclosure… offers a cabinet before its size is typed).
    pub section: Option<StartSection>,
    /// The role of the one network port it starts with (`X1`, PROFINET), if any.
    pub port: Option<&'static str>,
    /// Whether a device of it is drawn as its pieces, each a symbol (On the schematic asks what each piece is and its
    /// pins); else as a box with its pins.
    pub pieces: bool,
}

const fn card(
    group: StartGroup,
    word: &'static str,
    line: &'static str,
    icon: &'static str,
) -> Option<StartCard> {
    Some(StartCard {
        group,
        word,
        line,
        icon,
    })
}

const fn start(
    name: &'static str,
    card: Option<StartCard>,
    presets: &'static [(&'static str, &'static str)],
) -> Start {
    Start {
        name,
        card,
        presets,
        asks: &[],
        section: None,
        port: None,
        pieces: false,
    }
}

/// A PLC part's groups.
const PLC_ASKS: &[G] = &[G::Plc, G::Ports];

/// Every start of the place, the cards in the question's order (doc 07 §5.5).
pub static STARTS: &[Start] = &[
    Start {
        pieces: true,
        ..start(
            "device",
            card(
                StartGroup::Devices,
                key!("part-start-device"),
                key!("part-start-device-line"),
                "circuit-switch-open",
            ),
            &[("kind", "device")],
        )
    },
    Start {
        asks: &[G::Ports],
        ..start(
            "box",
            card(
                StartGroup::Devices,
                key!("part-start-box"),
                key!("part-start-box-line"),
                "ew-black-box",
            ),
            &[("kind", "device")],
        )
    },
    Start {
        asks: PLC_ASKS,
        ..start(
            "module",
            card(
                StartGroup::Devices,
                key!("part-start-module"),
                key!("part-start-module-line"),
                "cpu",
            ),
            &[("kind", "device"), ("plc.role", "module")],
        )
    },
    Start {
        pieces: true,
        ..start(
            "door",
            card(
                StartGroup::Devices,
                key!("part-start-door"),
                key!("part-start-door-line"),
                "circle-dot",
            ),
            &[("kind", "device"), ("mech.mount_kind", "door_cutout")],
        )
    },
    start(
        "terminal",
        card(
            StartGroup::Wiring,
            key!("part-start-terminal"),
            key!("part-start-terminal-line"),
            "ew-terminal-strip",
        ),
        &[("kind", "terminal"), ("levels", "1")],
    ),
    start(
        "cable",
        card(
            StartGroup::Wiring,
            key!("part-start-cable"),
            key!("part-start-cable-line"),
            "ew-cable",
        ),
        &[("kind", "cable"), ("unit", "m")],
    ),
    start(
        "wire",
        card(
            StartGroup::Wiring,
            key!("part-start-wire"),
            key!("part-start-wire-line"),
            "wave-sine",
        ),
        &[("kind", "wire"), ("unit", "m")],
    ),
    start(
        "ferrule",
        card(
            StartGroup::Wiring,
            key!("part-start-ferrule"),
            key!("part-start-ferrule-line"),
            "pin",
        ),
        &[("kind", "ferrule")],
    ),
    start(
        "marker",
        card(
            StartGroup::Wiring,
            key!("part-start-marker"),
            key!("part-start-marker-line"),
            "tag",
        ),
        &[("kind", "marker")],
    ),
    start(
        "gland",
        card(
            StartGroup::Wiring,
            key!("part-start-gland"),
            key!("part-start-gland-line"),
            "plug",
        ),
        &[("kind", "gland")],
    ),
    Start {
        section: Some(StartSection::Enclosure),
        ..start(
            "enclosure",
            card(
                StartGroup::Cabinet,
                key!("part-start-enclosure"),
                key!("part-start-enclosure-line"),
                "door",
            ),
            &[("kind", "enclosure")],
        )
    },
    start(
        "plate",
        card(
            StartGroup::Cabinet,
            key!("part-start-plate"),
            key!("part-start-plate-line"),
            "ew-mounting-plate",
        ),
        &[("kind", "mounting_plate")],
    ),
    start(
        "rail",
        card(
            StartGroup::Cabinet,
            key!("part-start-rail"),
            key!("part-start-rail-line"),
            "ew-din-rail",
        ),
        &[("kind", "rail"), ("unit", "m")],
    ),
    start(
        "duct",
        card(
            StartGroup::Cabinet,
            key!("part-start-duct"),
            key!("part-start-duct-line"),
            "ew-duct",
        ),
        &[("kind", "duct"), ("unit", "m")],
    ),
    start(
        "accessory",
        card(
            StartGroup::Cabinet,
            key!("part-start-accessory"),
            key!("part-start-accessory-line"),
            "assembly",
        ),
        &[("kind", "accessory")],
    ),
    Start {
        section: Some(StartSection::Fluid),
        pieces: true,
        ..start(
            "valve",
            card(
                StartGroup::Fluid,
                key!("part-start-valve"),
                key!("part-start-valve-line"),
                "droplet",
            ),
            &[("kind", "device")],
        )
    },
    start(
        "tube",
        card(
            StartGroup::Fluid,
            key!("part-start-tube"),
            key!("part-start-tube-line"),
            "pipeline",
        ),
        &[("kind", "tube"), ("unit", "m")],
    ),
    Start {
        section: Some(StartSection::Fluid),
        pieces: true,
        ..start(
            "fluid",
            card(
                StartGroup::Fluid,
                key!("part-start-fluid"),
                key!("part-start-fluid-line"),
                "wind",
            ),
            &[("kind", "device")],
        )
    },
    // The heads of a station the rack's view asks for: a CPU leads its bus as a PROFINET controller, the others answer
    // as devices; a valve terminal's eight channels are outputs for its valves, a compact device's inputs.
    Start {
        asks: PLC_ASKS,
        port: Some("controller"),
        ..start("cpu", None, &[("kind", "device"), ("plc.role", "station")])
    },
    Start {
        asks: PLC_ASKS,
        port: Some("device"),
        ..start("remote", None, &[("kind", "device"), ("plc.role", "station")])
    },
    Start {
        asks: PLC_ASKS,
        port: Some("device"),
        ..start(
            "valve_terminal",
            None,
            &[
                ("kind", "device"),
                ("plc.role", "station"),
                ("plc.kind", "do"),
                ("plc.count", "8"),
            ],
        )
    },
    Start {
        asks: PLC_ASKS,
        port: Some("device"),
        ..start(
            "compact",
            None,
            &[
                ("kind", "device"),
                ("plc.role", "station"),
                ("plc.kind", "di"),
                ("plc.count", "8"),
            ],
        )
    },
    // A bus line's New plug…: an accessory with a passive interface of the line's bus.
    Start {
        asks: &[G::Ports],
        port: Some("passive"),
        ..start("plug", None, &[("kind", "accessory")])
    },
];

/// The start named `name`.
pub fn start_named(name: &str) -> Option<&'static Start> {
    STARTS.iter().find(|s| s.name == name)
}

impl Session {
    /// A new part's sheet as `start` begins it: its kind and presets typed in, a pneumatic section where it has one,
    /// its network port; the sheet asks for what the start decides ([`PartSheet::sections`]).
    ///
    /// # Errors
    ///
    /// As [`Session::new_part_draft`].
    pub fn start_sheet(&self, start: &'static Start) -> Result<PartSheet, PartDraftError> {
        let mut sheet = self.new_part_draft()?;
        if let Some(section) = start.section {
            match section {
                StartSection::Fluid => sheet.original.sections.fluid = Some(Default::default()),
                StartSection::Enclosure => sheet.original.sections.enclosure = Some(Default::default()),
            }
            (sheet.rows, sheet.tables) = self.part_rows(&self.query(), &sheet.original);
        }
        sheet.start = Some(start);
        for (field, text) in start.presets {
            self.set_draft_field(&mut sheet, field, text)?;
        }
        if let Some(role) = start.port {
            sheet.add_bus_row();
            self.set_draft_field(&mut sheet, "bus.0.role", role)?;
        }
        Ok(sheet)
    }
}

impl PartSheet {
    /// Whether the network ports are the one port the start gave, as it gave it (`X1`, PROFINET, the start's role).
    pub fn port_as_started(&self) -> bool {
        let Some(role) = self.start.and_then(|s| s.port) else {
            return false;
        };
        matches!(self.bus.as_slice(), [r] if *r == crate::part_editor::BusRow {
            interface: "X1".to_owned(),
            bus: "profinet".to_owned(),
            role: role.to_owned(),
            ..Default::default()
        })
    }
}
