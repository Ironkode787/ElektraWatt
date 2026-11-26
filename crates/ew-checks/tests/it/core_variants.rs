// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-CORE-085: a part use that names a variant its part does not hold (doc 07 §2.5).

use crate::common;

use common::*;
use ew_schema::Kind;
use serde_json::json;

/// A part use naming the variant RD of a part that holds BU only is a warning naming the device, the key and the part;
/// one naming BU is not.
#[test]
fn core085_reports_a_variant_the_part_does_not_hold() {
    let mut fx = Fx::base();
    fx.part(
        0x7100,
        "P-1",
        &json!({"description": "Terminal", "variants": [{"key": "BU", "name": {"en": "Blue"}, "part_number": "P-1-BU"}]}),
    );
    fx.func_tagged(0x1001, 1, P1);
    fx.func_tagged(0x1002, 2, P1);
    for (device, key) in [(0x21001, "RD"), (0x21002, "BU")] {
        fx.with_parts(device, &[0x7100]);
        let r = fx
            .records
            .iter_mut()
            .find(|r| r["id"] == json!(us(device)))
            .expect("the device record");
        r["parts"][0]["part"]["variant"] = json!(key);
    }
    let f = findings_of(&fx.build(), "EW-CORE-085");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!((f[0].code, f[0].discriminator.as_str()), ("105085", "RD"));
    assert!(
        f[0].message()
            .ends_with(" names the variant RD, which P-1 does not hold"),
        "{}",
        f[0].message()
    );
    assert_eq!(
        targets(&f[0]),
        [(Kind::Device, uid(0x21001)), (Kind::Part, uid(0x7100))]
    );
}
