// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `library.embed` (`EmbedLibraryCopies`, FT-167): embeds library items into the project as copies with
//! their lineage (doc 05 §13.2, doc 07 §3.1).

use std::collections::BTreeSet;

use ew_ids::{Id, v5};
use ew_schema::{AnyId, LibDocument, Sha256};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::tx::TxBuilder;

/// `library.embed` (`EmbedLibraryCopies`): embeds library items into the project, each as a copy that
/// carries its lineage `{library_id, item_id, version, sha256}` (doc 05 §13.2), such as the frame form
/// without grid that generated section dividers and report pages need (doc 20 §20.2 step 4).
///
/// `copies` holds library documents in the `lib/` JSON form (doc 05 §3.4: `$k`, `id`, the copy metadata,
/// then the item's members) of the kinds `part`, `symbol`, `fndef`, `form`, `macro` and `manufacturer`; every copy
/// is inserted as it is, in one transaction. At least one. Each copy is checked, at `/copies/i`:
/// - it reads as a library document of one of those kinds (else invalid);
/// - its `id` is the copy identity `v5(NS_LIBCOPY, library_id ‖ item_id)` (ADR-0004 amendment 2; a "keep
///   both" copy belongs to a paste, not to embedding);
/// - its `library_id` is not the project's: a project-local item is created in the project, not embedded (a
///   project-local part copy is `parts.import`'s), but a form of the project's own (a frame or a title block made
///   outside the program, FT-1859) is embedded as any other form;
/// - its `sha256` is the hash of the item's own members (the rule of EW-CORE-014): the copy is consistent
///   with the hash it claims (the hash does not prove where the copy came from).
///
/// Rejected: a copy of an item the project has embedded already, at any version ("update from library" is
/// a previewed command of its own, FR-PRT-014), and the same item twice in `copies`. The closure (a part's
/// FnDefs, a symbol's FnDef) is the caller's to include: a reference to an item that is not embedded is
/// not a link (doc 05 §13.2), and the commands that use an item resolve it. Unused copies stay until
/// purged; an edit never removes a copy that a record names through a `LibRef` (EW-INV-021), but undo
/// removes the embedded copies again, also when records name them, because the state before the embedding
/// is valid (ADR-0009 amendment 7). Outcome: the first copy; the receipt lists every inserted key.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EmbedLibraryCopies {
    /// The library documents to embed (`lib/` JSON form).
    pub copies: Vec<Value>,
}

/// The copy's lineage, kind-erased: copy ID, library, item, sha256 claimed and computed.
pub(crate) struct Lineage {
    pub(crate) id: AnyId,
    pub(crate) library: ew_ids::Uuid,
    pub(crate) item: ew_ids::Uuid,
    pub(crate) claimed: Sha256,
    computed: Option<[u8; 32]>,
}

/// Checks one library document of a command parameter at `at` (the rules of [`EmbedLibraryCopies`]): it reads as
/// a library document of an implemented kind; its `id` is the copy identity; its `library_id` is not
/// the project's (`project`) unless it is a form; its `sha256` is the hash of its item. Returns the document and its
/// lineage.
pub(crate) fn check_copy(
    command: &'static str,
    at: &str,
    value: &Value,
    project: ew_ids::Uuid,
) -> Result<(LibDocument, Lineage), CmdError> {
    let (doc, l) = read_copy(command, at, value)?;
    // A form of the project's own is embedded as it is (FT-1859): no command makes one yet, so a person's frame or
    // title block comes in this way.
    if l.library == project && !matches!(doc, LibDocument::Form(_)) {
        return Err(CmdError::invalid_params(
            command,
            format!("{at}/library_id"),
            text!("refusal-library-embed-local"),
        ));
    }
    Ok((doc, l))
}

/// Reads one library document of a command parameter at `at` and checks it without regard to its library: it reads as
/// a library document of an implemented kind (part, symbol, fndef, form, macro or manufacturer; the caller narrows
/// the kinds it takes), its `id` is the copy identity and its `sha256` the hash of
/// its item ([`check_copy`] without the library rule; `parts.import` requires the opposite one).
pub(crate) fn read_copy(
    command: &'static str,
    at: &str,
    value: &Value,
) -> Result<(LibDocument, Lineage), CmdError> {
    let doc = LibDocument::from_value(value.clone()).map_err(|e| {
        CmdError::invalid_params(
            command,
            at.to_owned(),
            text!("refusal-params-detail", crate::error::schema_error_text(&e)),
        )
    })?;
    let l = match doc.copy_meta() {
        Some(c) => Lineage {
            id: ew_ids::Id::from_uuid(c.id),
            library: c.library_id.as_uuid(),
            item: c.item_id.as_uuid(),
            claimed: c.sha256,
            computed: ew_format::library_copy_hash(&doc),
        },
        None => {
            return Err(CmdError::invalid_params(
                command,
                at.to_owned(),
                text!("refusal-library-embed-kind"),
            ));
        }
    };
    let identity: Id<()> = v5::lib_copy_id(l.library, l.item);
    if l.id != identity.cast() {
        return Err(CmdError::invalid_params(
            command,
            format!("{at}/id"),
            text!("refusal-library-copy-id", l.id.to_string(), identity.to_string()),
        ));
    }
    if l.computed != Some(l.claimed.0) {
        return Err(CmdError::invalid_params(
            command,
            format!("{at}/sha256"),
            text!("refusal-library-copy-hash", l.claimed.to_string()),
        ));
    }
    Ok((doc, l))
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
                text!("refusal-params-no-library-document"),
            ));
        }
        let project = tx.project().id.as_uuid();
        let mut docs = Vec::with_capacity(self.copies.len());
        let mut seen = BTreeSet::new();
        for (i, value) in self.copies.iter().enumerate() {
            let at = format!("/copies/{i}");
            let (doc, l) = check_copy(Self::NAME, &at, value, project)?;
            if !seen.insert(l.id) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    at,
                    text!("refusal-params-item-twice"),
                ));
            }
            // An address profile that does not render never reaches a project.
            // Its frame by key, as `ew_derive::unreadable_profile` words it; the engine's reason as it prints.
            if let LibDocument::PlcProfile(c) = &doc
                && let Err(reason) = ew_derive::address_profile(&c.item)
            {
                let why = text!(
                    "refusal-plc-profile-unreadable",
                    c.item.profile.id.to_string(),
                    c.item.profile.version.to_string(),
                    reason.to_string()
                );
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    at,
                    text!("refusal-params-detail", why),
                ));
            }
            let kind = doc.kind().expect("an implemented kind");
            if tx.row(&(kind, l.id)).is_some() {
                let item = crate::words::document_item(tx, &doc);
                return tx.reject(text!("refusal-library-embed-held", item));
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
                LibDocument::Macro(c) => tx.insert(c)?.cast(),
                LibDocument::Manufacturer(c) => tx.insert(c)?.cast(),
                LibDocument::PlcProfile(c) => tx.insert(c)?.cast(),
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
