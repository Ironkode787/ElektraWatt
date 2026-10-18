// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Generic memo engine: revisions, dependency recording, early cut-off, durability levels, LRU, statistics,
//! oracle hooks and a reverse-dependency API per query family. Knows nothing about ECAD.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
