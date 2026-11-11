// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 5 through the built binary (acceptance check IM2): a UI script brings a placeholder SVG into a copy
//! of Golden A, arms the Image tool, picks the asset and places it by two grid-cursor corners; the transcript has
//! no UUID, the ghost member and describe line name the asset, `page.describe` lists
//! the `image` item, the saved project holds it, and the `--record` log replayed on a third copy (the session's
//! command-log replay, what `ew apply` runs) gives the same item. Golden A itself is only copied, never written.

use crate::support::bin_exe;

pub(crate) mod placement;

/// IM2 per gate: one run of the script (the second run and the byte-identical comparison are the deep nightly's,
/// `headless::nightly_every_script_transcript_is_byte_identical_twice`; test clean-up, 2026-10-30).
#[test]
fn an_image_is_placed_by_a_script_twice_identically_and_replays() {
    placement::im2_an_image_is_placed_by_a_script_twice_identically_and_replays_case(false);
}
