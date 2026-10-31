// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The integration tests of `ew-render-cpu`, one test binary: each module is a former test file.

#[path = "../common/mod.rs"]
mod common;

mod jpeg;
mod mono_png;
mod region;
mod render;
