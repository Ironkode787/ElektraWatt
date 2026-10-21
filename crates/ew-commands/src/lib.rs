// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Command`, `DynCommand`, `TxBuilder`, normalisers, validators, numbering engine, `History`, previews,
//! macros and `ApplyImport`.
//!
//! `ew-commands` v0 (WP-S1.04) implements the editing layer of ADR-0008 (ARCH §5): every change of a
//! project is a [`Command`] that runs against a [`TxBuilder`], is completed by the normalisers, validated
//! by `ew-model` and committed as one [`Transaction`] of `ew-model` [`Op`](ew_model::Op)s.
//!
//! - **Commands.** [`Command`] (typed, with a stable dotted name, [`CommandMeta`] and a JSON Schema for its
//!   parameters) and its object-safe companion [`DynCommand`]. [`CommandRegistry`] is the single list of
//!   commands; [`AnyCommand`] is the untyped form of scripts, plugins and MCP, validated against the
//!   command's schema. The 25 v0 commands are in [`commands`].
//! - **Transactions.** [`TxBuilder`] is a scratch snapshot with read-your-writes over the current model;
//!   it mints IDs from an injected `IdSource` and records the [`ReadSet`] for the optimistic rebase.
//! - **Pipeline.** [`Engine`] runs a command, the [`normalise`] registry (at most three passes), validates
//!   the candidate with `Model::apply_with` (structural invariants, then the prevent hook), commits it,
//!   records it in the [`History`] and appends a [`Change`] to the change feed. [`Engine::preview`] and
//!   [`Engine::commit`] split the same pipeline around a [`Preview`]. `ew_api::Session` wraps the engine.
//! - **Undo.** One project-wide [`History`] with coalescing, bounds, marker lines and a saved marker; undo
//!   applies inverse ops as a new revision.
//!
//! Not in v0: the full numbering engine (WP-S1.11), persistence and the journal (WP-S1.09), the job pool
//! (WP-S1.05; previews run synchronously), macros, `ApplyImport` and `edit.paste`.
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
//! engine.undo().unwrap(); // the page is gone, as revision 3
//! assert_eq!(engine.world().rev.0, 3);
//! assert!(engine.history().can_redo());
//! ```

#![forbid(unsafe_code)]
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod command;
pub mod commands;
mod derived;
mod diff;
mod engine;
mod error;
pub mod geometry;
mod heap;
mod history;
mod jsonschema;
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
#[cfg(feature = "testkit")]
pub mod testkit;
mod transaction;
mod tx;

pub use command::{Command, DynCommand, Outcome};
pub use derived::{DerivedChange, DerivedDiff, derived_diff};
pub use diff::{ChangeKind, EntityChange, ModelDiff, diff_model};
pub use engine::{
    Change, Engine, EngineOptions, NoPrevent, Preview, Receipt, Restored, World, affected_pages,
};
pub use error::CmdError;
pub use history::{History, HistoryEntry, HistoryLimits};
pub use jsonschema::{
    MAX_PARAM_BYTES, MAX_PARAM_DEPTH, MAX_PARAM_ITEMS, MAX_PARAM_STRING, SchemaViolation, check_limits,
    validate as validate_schema,
};
pub use meta::{CommandMeta, Confirmation, Cost, Exposure, Origin, Stability};
pub use registry::{AnyCommand, CommandRegistry, RegisteredCommand};
pub use transaction::{Submit, Transaction, TxContext, Viewport};
pub use tx::{IndexRead, ReadSet, TxBuilder};

/// The ID source trait of `ew-ids`, re-exported for crates that inject one into an [`Engine`].
pub use ew_ids::IdSource;
