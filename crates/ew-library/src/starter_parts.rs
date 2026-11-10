// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The system library `ew-starter-parts` compiled in: the files of `content/libraries/ew-starter-parts.ewlib`, which
//! the build script lists from the folder.

macro_rules! content {
    ($path:literal) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/", $path))
    };
}

/// The library's `library.json`.
pub(crate) const MANIFEST: &str = content!("libraries/ew-starter-parts.ewlib/library.json");

/// The records files of the library, as `(path, text)` in path order.
pub(crate) const FILES: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/starter_parts.rs"));
