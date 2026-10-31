// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The integration tests of `ew-store`, one test binary: each module is a former test file.
// The shared helpers below were compiled under each including file's raw-reader opt-out (ADR-0001 amendment 2).
#![allow(clippy::disallowed_methods)]

#[path = "../common/mod.rs"]
mod common;

mod assets;
mod integrity;
mod layout_incremental;
mod lock;
mod migration;
mod part1_migration;
mod recovery;
