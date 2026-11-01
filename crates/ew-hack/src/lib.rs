// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The workspace-hack crate of cargo-hakari: it has no code. Its manifest depends on the third-party crates that the
//! GPL members share, with the union of their features, so that building one package set after another reuses them
//! (`.config/hakari.toml`, ci/README.md "Workspace hack").
