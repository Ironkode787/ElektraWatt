// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Command`, `DynCommand`, `TxBuilder`, normalisers, validators, numbering engine, `History`, previews,
//! macros and `ApplyImport`.
//!
//! `ew-commands` v0 implements the editing layer of ADR-0008 (ARCH §5): every change of a
//! project is a [`Command`] that runs against a [`TxBuilder`], is completed by the normalisers, validated
//! by `ew-model` and committed as one [`Transaction`] of `ew-model` [`Op`](ew_model::Op)s.
//!
//! - **Commands.** [`Command`] (typed, with a stable dotted name, [`CommandMeta`] and a JSON Schema for its
//!   parameters) and its object-safe companion [`DynCommand`]. [`CommandRegistry`] is the single list of
//!   commands; [`AnyCommand`] is the untyped form of scripts, plugins and MCP, read into the command struct after
//!   the size limits of its parameters ([`check_limits`]). The 37 v0 commands are in [`commands`].
//! - **Transactions.** [`TxBuilder`] is a scratch snapshot with read-your-writes over the current model;
//!   it mints IDs from an injected `IdSource` and records the [`ReadSet`] for the optimistic rebase.
//! - **Pipeline.** [`Engine`] runs a command, the [`normalise`] registry (at most three passes), validates
//!   the candidate with `Model::apply_with` (structural invariants, the delete-time rule EW-INV-021, then
//!   the prevent hook), commits it, records it in the [`History`] and appends a [`Change`] to the change
//!   feed. [`Engine::preview`] and
//!   [`Engine::commit`] split the same pipeline around a [`Preview`]. `ew_api::Session` wraps the engine.
//! - **Undo.** One project-wide [`History`] with bounds, marker lines and a saved marker; undo
//!   applies inverse ops as a new revision on the history path (`Model::apply_history`: structural
//!   invariants only, ARCH §5.3).
//!
//! Not in v0: the full numbering engine, persistence and the journal, the job pool
//! (previews run synchronously), macros, `ApplyImport` and `edit.paste`.
//!
//! # Example
//!
//! ```
//! use ew_commands::commands::{CreatePage, CreateSection};
//! use ew_commands::{Engine, EngineOptions};
//! use ew_mltext::MlText;
//!
//! let project = serde_json::from_str(
//!     r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
//! )
//! .unwrap();
//! let model = ew_model::Model::from_records(project, [], []).unwrap();
//! let mut engine = Engine::new(model, EngineOptions::default());
//!
//! let section = engine
//!     .execute(&CreateSection {
//!         number: 1,
//!         folio_range: (100, 199),
//!         title: MlText::neutral("Wiring Definition"),
//!         divider: false,
//!     })
//!     .unwrap();
//! let section_id = section.outcome.primary.unwrap().1.cast();
//! let page = engine.execute(&CreatePage::at(Some(section_id), 101)).unwrap();
//! assert_eq!(page.rev.0, 2);
//!
//! engine.undo(&ew_commands::Origin::Gui).unwrap(); // the page is gone, as revision 3
//! assert_eq!(engine.world().rev.0, 3);
//! assert!(engine.history().can_redo());
//! ```
//!
//! # Texts
//!
//! A refusal, a notice and a record in words are an [`ew_l10n::Text`] of this crate's text files
//! (`texts/<language>/<area>.ftl`, architecture §18): the command line prints its English, the window renders it in
//! its language. Code names a key with `text!`, checked when the code is built:
//!
//! ```
//! assert_eq!(ew_commands::text!("refusal-words-page", "101").to_string(), "page 101");
//! ```
//!
//! A key the files lack fails the build, in `text!` as in `key!`:
//!
//! ```compile_fail
//! let _ = ew_commands::text!("refusal-words-paeg", "101");
//! ```
//!
//! ```compile_fail
//! let _ = ew_commands::key!("refusal-words-paeg");
//! ```

#![forbid(unsafe_code)]
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

// `TEXTS`, `text_english`, `text!` and `key!`, before the modules that name a key.
ew_l10n::texts!(crate);

mod command;
pub mod commands;
mod derived;
mod diff;
mod engine;
mod error;
pub mod geometry;
mod history;
mod limits;
mod meta;
pub mod normalise;
pub mod numbering;
mod proxy;
mod registry;
#[cfg(feature = "testkit")]
pub mod rows;
#[cfg(not(feature = "testkit"))]
#[allow(dead_code)] // the dumps serve the tests (feature `testkit`); the crate uses a part of them
mod rows;
mod transaction;
mod tx;
mod words;

pub use command::{Command, Counts, DynCommand, Outcome};
pub use derived::{DerivedChange, DerivedDiff, derived_diff};
pub use diff::{ChangeKind, EntityChange, ModelDiff, diff_model, holds_record};
pub use engine::{
    Change, Engine, EngineOptions, NoPrevent, Preview, Receipt, Restored, World, affected_pages,
};
pub use error::{
    CmdError, HistoryBlock, asset_error_text, ident_error_text, path_error_text, schema_error_text,
    serde_error_text,
};
pub use history::{History, HistoryEntry, HistoryLimits};
pub use limits::{
    LimitViolation, MAX_PARAM_BYTES, MAX_PARAM_DEPTH, MAX_PARAM_ITEMS, MAX_PARAM_STRING, check_limits,
};
pub use meta::{CommandMeta, Confirmation, Cost, Exposure, Origin};
pub use registry::{AnyCommand, CommandRegistry, RegisteredCommand};
pub use transaction::{Submit, Transaction};
pub use tx::{IndexRead, ReadSet, TxBuilder};
pub use words::{comma_joined, damaged_copy as damaged_copy_words};

/// The ID source trait of `ew-ids`, re-exported for crates that inject one into an [`Engine`].
pub use ew_ids::IdSource;

#[cfg(test)]
mod tests {
    use super::*;

    /// A text writes its arguments in place, a text argument in English; the table is sorted, so each key is found.
    #[test]
    fn a_text_writes_its_arguments_in_english() {
        let t = text!("refusal-words-page", "101");
        assert_eq!(t.to_string(), "page 101");
        let t = text!(
            "refusal-connection-start-on-other-page",
            text!("refusal-words-page", "2"),
            text!("refusal-words-page", "3")
        );
        assert_eq!(
            t.to_string(),
            "the wire's start is drawn on page 2, not on page 3; start the wire on page 3"
        );
        assert!(TEXTS.windows(2).all(|w| w[0].0 < w[1].0), "sorted, no key twice");
        assert!(TEXTS.iter().all(|(k, t)| text_english(k) == Some(*t)));
        // A literal brace of the text file is a brace here.
        assert_eq!(
            text_english("refusal-fields-master-cp"),
            Some("a master is a CP: {\"cp\": [function, index]}")
        );
    }
}
