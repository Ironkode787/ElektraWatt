// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `parts.import` (`ImportParts`, FT-410): inserts project-local part copies that a parts table produced
//! (`ew_import_tables::parse_parts_tables`), with the manufacturer copies they name (WP-S1.19 D10), skipping what the
//! project has already.

use std::collections::BTreeSet;

use ew_schema::{Kind, LibCopy, LibDocument, Manufacturer, Part};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::tx::TxBuilder;

use super::library::read_copy;

/// `parts.import` (`ImportParts`, FT-410): creates project-local part copies, the one command that does (a copy of
/// another library is embedded with `library.embed`, which refuses project-local items), with the project-local
/// manufacturer copies the parts name (WP-S1.19 D10).
///
/// `copies` holds part and manufacturer copies in the `lib/` JSON form (doc 05 §3.4), as a parts table import
/// produces them: the shell and `ew import parts` read the file and parse it (`ew_import_tables::parse_parts_tables`,
/// the golden dialect), the command never sees a path. At least one part copy. Each copy is checked, at `/copies/i`:
/// - it reads as a library document of kind `part` or `manufacturer` (else invalid);
/// - its `id` is the copy identity `v5(NS_LIBCOPY, library_id ‖ item_id)` and its `sha256` the hash of its item
///   (the rules of `library.embed`);
/// - its `library_id` is the project's ID: the copy is project-local;
/// - a part copy has a `key` (`meta.key`), which is what a re-import matches on; a key or a copy listed twice is
///   invalid.
///
/// **Duplicates.** A part copy whose key a project-local part copy of the project has already, or whose copy ID
/// exists, is skipped and counted; a manufacturer copy whose copy ID exists is skipped; nothing is updated (updating a
/// part is FR-PRT-014's previewed command). The rest are inserted in the given order, in one transaction (one undo
/// step); when every copy is skipped the transaction is empty.
///
/// Cost `Bulk` (always previewed: the shell shows the card with the created and skipped counts), exposure `Edit` (the
/// GUI, `ew import parts`, and a command log replayed by `ew apply`). Outcome: the first inserted part copy; the note
/// `created <n>, skipped <m>`, which counts the part copies.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportParts {
    /// The project-local part copies to create, and the manufacturer copies they name (`lib/` JSON form).
    pub copies: Vec<Value>,
}

/// The counts of a `parts.import` outcome note (`created <n>, skipped <m>`): `(created, skipped)`, `None` for another
/// note.
pub fn import_counts(note: &str) -> Option<(usize, usize)> {
    let rest = note.strip_prefix("created ")?;
    let (created, skipped) = rest.split_once(", skipped ")?;
    Some((created.parse().ok()?, skipped.parse().ok()?))
}

impl Command for ImportParts {
    const NAME: &'static str = "parts.import";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let project = tx.project().id.as_uuid();
        let mut copies: Vec<LibCopy<Part>> = Vec::with_capacity(self.copies.len());
        let mut makers: Vec<LibCopy<Manufacturer>> = Vec::new();
        let mut keys = BTreeSet::new();
        let mut ids = BTreeSet::new();
        for (i, value) in self.copies.iter().enumerate() {
            let at = format!("/copies/{i}");
            let (doc, l) = read_copy(Self::NAME, &at, value)?;
            if !matches!(doc, LibDocument::Part(_) | LibDocument::Manufacturer(_)) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    at,
                    "not a part or manufacturer copy",
                ));
            }
            if l.library != project {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("{at}/library_id"),
                    "not a project-local copy (library_id must be the project ID); a copy of another library is \
                     embedded with library.embed",
                ));
            }
            if !ids.insert(l.id) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    at,
                    "the copy is listed twice",
                ));
            }
            match doc {
                LibDocument::Part(copy) => {
                    let Some(key) = copy.item.meta.key.as_ref().map(ToString::to_string) else {
                        return Err(CmdError::invalid_params(
                            Self::NAME,
                            format!("{at}/key"),
                            "a part copy without a key (the key is what a re-import matches on)",
                        ));
                    };
                    if !keys.insert(key) {
                        return Err(CmdError::invalid_params(
                            Self::NAME,
                            at,
                            "the part is listed twice",
                        ));
                    }
                    copies.push(copy);
                }
                LibDocument::Manufacturer(copy) => makers.push(copy),
                _ => unreachable!("checked above"),
            }
        }
        if copies.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/copies",
                "give at least one part copy",
            ));
        }
        let existing: BTreeSet<String> = tx
            .iter::<LibCopy<Part>>()
            .iter()
            .filter(|c| c.library_id.as_uuid() == project)
            .filter_map(|c| c.item.meta.key.as_ref().map(ToString::to_string))
            .collect();
        let (mut created, mut skipped) = (0usize, 0usize);
        let mut primary = None;
        for copy in copies {
            let key = copy
                .item
                .meta
                .key
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default();
            if existing.contains(&key) || tx.row(&(Kind::Part, copy.id.cast())).is_some() {
                skipped += 1;
                continue;
            }
            let id = tx.insert(copy)?;
            primary.get_or_insert((Kind::Part, id.cast()));
            created += 1;
        }
        for copy in makers {
            if tx.row(&(Kind::Manufacturer, copy.id.cast())).is_none() {
                tx.insert(copy)?;
            }
        }
        Ok(Outcome {
            primary,
            note: Some(format!("created {created}, skipped {skipped}")),
            ..Outcome::default()
        })
    }
}
