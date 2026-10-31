// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The integration tests of `ew-export`, one test binary: each module is a former test file.

// The JPEG fixtures of the renderer's tests: built from fixed coefficients, the same bytes on every platform.
#[path = "../../../ew-render-cpu/tests/common/jpeg.rs"]
mod jpeg;

mod pdf;
mod svg;
