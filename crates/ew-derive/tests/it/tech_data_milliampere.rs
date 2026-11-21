// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A current in milliampere in technical data (a coil's or an input's current) prints with its unit.

use crate::common;

use common::{Fx, id, uid};
use ew_derive::{Q, tech_data_text};
use serde_json::json;

/// A relay coil's own data, 20 mA at 24 V, prints "20 mA, 24 V" (keys in order); a unit of a newer format would
/// print the bare number.
#[test]
fn a_current_in_milliampere_prints_with_its_unit() {
    let mut fx = Fx::new();
    fx.push(json!({
        "$k": "device", "id": uid(0x1000), "tag": {"letter": "K", "frozen": {"text": "K1"}},
        "tech_data": [
            {"key": "coil_voltage", "value": {"value": "24", "unit": "V"}},
            {"key": "coil_current", "value": {"value": "20", "unit": "mA"}},
        ],
    }));
    let m = fx.build();
    let q = Q::new(&m);
    assert_eq!(tech_data_text(&q, id(0x1000)), "20 mA, 24 V");
}
