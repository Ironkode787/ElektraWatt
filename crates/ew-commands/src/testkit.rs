// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Test support for dependent crates (feature `testkit`, dev-dependency edges only).

use ew_ids::IdSource;
use ew_ids::v7::SeededSource;

/// A reproducible ID source: the same seed yields the same IDs on every platform (doc 18 §2.2).
pub fn seeded_ids(seed: u64) -> Box<dyn IdSource + Send> {
    Box::new(SeededSource::new(seed))
}
