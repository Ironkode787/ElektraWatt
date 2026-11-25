// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `bus.cable` takes its cable part and its plugs by part number, as every other part parameter takes a part.

use crate::common;

use common::{Tmp, done, failed};
use ew_viewmodel::shell::Mode;
use serde_json::json;

/// The sample's PROFIBUS line from the CPU to the valve terminal takes the PROFIBUS cable and the M12 plug by their
/// part numbers, in any case; a number no plug has is refused with it.
#[test]
fn bus_cable_takes_its_cable_and_plugs_by_part_number() {
    let tmp = Tmp::new("ft1902");
    let mut s = tmp.sample_in(Mode::Headless, true, "105");
    let select = json!({ "refs": ["wire:-104A1:A -> -201K1:A"] });
    done(&mut s, "select.set", select.clone());
    done(
        &mut s,
        "bus.cable",
        json!({ "part": "EW-GEN-CBL-2", "plug_a": "ew-gen-plg-5", "plug_b": "EW-GEN-PLG-5" }),
    );
    done(&mut s, "select.set", select);
    let message = failed(
        &mut s,
        "bus.cable",
        json!({ "part": "EW-GEN-CBL-2", "plug_a": "EW-GEN-PLG-3" }),
    );
    assert!(message.contains("EW-GEN-PLG-3"), "{message}");
}
