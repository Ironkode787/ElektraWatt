// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Where the pins of a box nobody arranged stand on its edges (doc 07 §2.5, "The device box"): the rule a part's box
//! without a layout, the black box tool and a bus symbol's sockets (doc 10 §2.7) share.

/// The pins of a box without a layout in their order along its edges and how many of them stand on top: with some
/// but not all of them among `outputs` (a source's outputs) the others on top and those below, else the first half
/// on top.
pub fn split(cps: &[u16], outputs: &[u16]) -> (Vec<u16>, usize) {
    let n = cps.len();
    let out = |c: &&u16| outputs.contains(c);
    let below = cps.iter().filter(out).count();
    if below > 0 && below < n {
        let inputs = cps.iter().filter(|c| !out(c));
        (inputs.chain(cps.iter().filter(out)).copied().collect(), n - below)
    } else {
        (cps.to_vec(), n.div_ceil(2))
    }
}

/// The grid offset of pin `k` of `m` on an edge `width` grid steps long: the pins one common pitch apart, the largest
/// whole number of grid steps that fits `m + 1` times into the edge (at least one), and the group centred on the edge
/// (half a step to the left when it cannot be centred exactly). Every pin lies at least one grid step from both
/// corners whenever `width ≥ m + 1`.
pub fn spread(width: i64, m: i64, k: i64) -> i64 {
    let pitch = (width / (m + 1)).max(1);
    let start = (width - (m - 1) * pitch) / 2;
    start + k * pitch
}
