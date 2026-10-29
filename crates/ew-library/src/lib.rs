// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parts, symbol, form and macro libraries as canonical `.ewlib` text directories at every tier, a derived
//! SQLite FTS5 index per user, and embedding used items into projects.
//!
//! WP-S1.19 part 2 (decisions D3 to D6):
//!
//! - **Tiers** ([`Tier`], [`LibraryTiers`]): system (the compiled-in [`starter`] as `ew-starter`, and
//!   `content/libraries/*.ewlib`), community (`<data dir>/packs`), company (the settings' paths), user
//!   (`<data dir>/libraries`). System and community libraries refuse writes ([`LibraryError::ReadOnly`]).
//! - **The `.ewlib` directory** (doc 07 §2.2): `library.json` ([`LibManifest`]) written last, records in
//!   `parts/<xx>.<generation>.jsonl`, `symbols/`, `fndefs/`, `forms/`, `macros/` and
//!   `manufacturers.<generation>.jsonl`, one canonical line per item sorted by item ID. Reading follows the project
//!   loader: the limits of doc 05 §3.6, regular files only, no links followed, the manifest's hashes checked; what
//!   fails is skipped with a finding.
//! - **The lock** ([`lock`]): `library.lock` with the project lock's record and stale rule; a stale lock is taken
//!   over only after the user confirmed ([`Registry::take_over_stale`]).
//! - **The index**: one SQLite file per user in the cache directory ([`INDEX_FILE`]), never on a share, never
//!   authoritative, reindexed per library when its generation or a file hash changes.
//! - **Search** ([`PartQuery`], [`ItemQuery`]): the rank tiers of doc 07 §4.3 and the tie-breaks of D6.
//!
//! The index is never read by a command or a derivation (WP-S1.19 §12 item 4): a project's outputs depend on its
//! embedded copies only (NFR-REL-11).

#![forbid(unsafe_code)]

mod error;
mod index;
pub mod lock;
mod manifest;
mod record;
mod registry;
mod search;
pub mod starter;
mod tiers;

pub use error::LibraryError;
pub use index::{INDEX_FILE, IndexState};
pub use lock::LockPolicy;
pub use manifest::{FORMAT, LibManifest, MANIFEST};
pub use registry::{LibraryInfo, Registry, WriteFault, WriteReport, version_matches};
pub use search::{FnCompat, ItemHit, ItemQuery, PartHit, PartQuery, item_kind, pn_norm, quantise};
pub use tiers::{LibraryTiers, Tier, default_cache_dir, default_data_dir};

/// The key of the shipped system library of starter parts.
pub const STARTER_PARTS_KEY: &str = "ew-starter-parts";

/// The ID of the system library `ew-starter-parts` (WP-S1.19 §7.1): `v5(SYSTEM_LIBRARY, "ew-starter-parts")`, the
/// directory `content/libraries/ew-starter-parts.ewlib` that WP-S2.18 part 2 fills.
pub fn starter_parts_library() -> ew_ids::Uuid {
    ew_ids::v5::imported_id::<()>(starter::LIBRARY, STARTER_PARTS_KEY).as_uuid()
}
