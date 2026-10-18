// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed identifiers `Id<T>`: UUIDv7 for new records, UUIDv5 for instantiated, imported and generated ones,
//! plus a per-session `u32` interner.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
