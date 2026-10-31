// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The integration tests of `elektrawatt`, one test binary: each module is a former test file.

#[path = "../support/mod.rs"]
mod support;

mod create;
mod gpp_demo;
mod image;
mod strip_editor;
