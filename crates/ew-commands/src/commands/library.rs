// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `library.embed` (`EmbedLibraryCopies`, FT-167): embeds library items into the project as copies with
//! their lineage (doc 05 §13.2, doc 07 §3.1).

use std::collections::BTreeSet;

use ew_ids::{Id, v5};
use ew_schema::{AnyId, LibCopy, LibDocument, Sha256};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Digest;

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::tx::TxBuilder;

/// `library.embed` (`EmbedLibraryCopies`): embeds library items into the project, each as a copy that
/// carries its lineage `{library_id, item_id, version, sha256}` (doc 05 §13.2), such as the frame form
/// without grid that generated section dividers and report pages need (doc 20 §20.2 step 4).
///
/// `copies` holds library documents in the `lib/` JSON form (doc 05 §3.4: `$k`, `id`, the copy metadata,
/// then the item's members) of the kinds `part`, `symbol`, `fndef` and `form`; every copy is inserted as it
/// is, in one transaction. At least one. Each copy is checked, at `/copies/i`:
/// - it reads as a library document of one of those kinds (else invalid);
/// - its `id` is the copy identity `v5(NS_LIBCOPY, library_id ‖ item_id)` (ADR-0004 amendment 2; a "keep
///   both" copy belongs to a paste, not to embedding);
/// - its `library_id` is not the project's: a project-local item is created in the project, not embedded;
/// - its `sha256` is the hash of the item's own members (the rule of EW-CORE-014), so the lineage is true.
///
/// Rejected: a copy of an item the project has embedded already, at any version ("update from library" is
/// a previewed command of its own, FR-PRT-014), and the same item twice in `copies`. The closure (a part's
/// FnDefs, a symbol's FnDef) is the caller's to include: a reference to an item that is not embedded is
/// not a link (doc 05 §13.2), and the commands that use an item resolve it. Unused copies stay until
/// purged. Outcome: the first copy; the receipt lists every inserted key.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EmbedLibraryCopies {
    /// The library documents to embed (`lib/` JSON form).
    pub copies: Vec<Value>,
}

/// The copy's lineage, kind-erased: copy ID, library, item, sha256 claimed and computed.
struct Lineage {
    id: AnyId,
    library: ew_ids::Uuid,
    item: ew_ids::Uuid,
    claimed: Sha256,
    computed: Option<[u8; 32]>,
}

fn lineage<T: Serialize>(c: &LibCopy<T>) -> Lineage {
    let computed = serde_json::to_string(&c.item)
        .ok()
        .map(|text| sha2::Sha256::digest(text.as_bytes()).into());
    Lineage {
        id: c.id.cast(),
        library: c.library_id.as_uuid(),
        item: c.item_id.as_uuid(),
        claimed: c.sha256,
        computed,
    }
}

impl Command for EmbedLibraryCopies {
    const NAME: &'static str = "library.embed";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.copies.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/copies",
                "give at least one library document",
            ));
        }
        let project = tx.project().id.as_uuid();
        let mut docs = Vec::with_capacity(self.copies.len());
        let mut seen = BTreeSet::new();
        for (i, value) in self.copies.iter().enumerate() {
            let at = format!("/copies/{i}");
            let doc = LibDocument::from_value(value.clone())
                .map_err(|e| CmdError::invalid_params(Self::NAME, at.clone(), e.to_string()))?;
            let l = match &doc {
                LibDocument::Part(c) => lineage(c),
                LibDocument::Symbol(c) => lineage(c),
                LibDocument::Fndef(c) => lineage(c),
                LibDocument::Form(c) => lineage(c),
                LibDocument::Unknown(_) => {
                    return Err(CmdError::invalid_params(
                        Self::NAME,
                        at,
                        "not a library document of kind part, symbol, fndef or form",
                    ));
                }
            };
            let identity: Id<()> = v5::lib_copy_id(l.library, l.item);
            if l.id != identity.cast() {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("{at}/id"),
                    format!(
                        "the copy ID is {}, not the copy identity {identity} = v5(NS_LIBCOPY, library_id ‖ item_id)",
                        l.id
                    ),
                ));
            }
            if l.library == project {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("{at}/library_id"),
                    "a project-local item (library_id = the project ID) is not embedded",
                ));
            }
            if l.computed != Some(l.claimed.0) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("{at}/sha256"),
                    format!(
                        "{} is not the hash of the item's members (EW-CORE-014); the copy's lineage would be \
                         false",
                        l.claimed
                    ),
                ));
            }
            if !seen.insert(l.id) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    at,
                    "the item is listed twice",
                ));
            }
            let kind = doc.kind().expect("an implemented kind");
            if tx.row(&(kind, l.id)).is_some() {
                return tx.reject(format!(
                    "{kind} copy {} (item {}) is embedded already; updating it to another version is \"update \
                     from library\" (FR-PRT-014)",
                    l.id, l.item
                ));
            }
            docs.push((kind, doc));
        }
        let mut primary = None;
        for (kind, doc) in docs {
            let id = match doc {
                LibDocument::Part(c) => tx.insert(c)?.cast(),
                LibDocument::Symbol(c) => tx.insert(c)?.cast(),
                LibDocument::Fndef(c) => tx.insert(c)?.cast(),
                LibDocument::Form(c) => tx.insert(c)?.cast(),
                LibDocument::Unknown(_) => unreachable!("rejected above"),
            };
            primary.get_or_insert((kind, id));
        }
        Ok(Outcome {
            primary,
            ..Outcome::default()
        })
    }
}
