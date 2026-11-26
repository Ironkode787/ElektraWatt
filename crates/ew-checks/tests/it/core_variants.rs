// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-CORE-085: a part use that names a variant its part does not hold (doc 07 §2.5).

use crate::common;

use common::*;
use ew_schema::Kind;
use serde_json::json;

/// A part use naming the variant RD of a part that holds BU only is a warning naming the device, the key and the part;
/// one naming BU is not; its quick fix takes the part's own number.
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
    // The quick fix gives the part use the part's own number: the device's parts without the key.
    let fix = f[0].fix.as_ref().expect("a quick fix");
    assert_eq!(
        (fix.command.as_str(), &fix.params["path"]),
        ("fields.set", &json!("parts"))
    );
    assert!(
        fix.params["value"][0]["part"].get("variant").is_none(),
        "{}",
        fix.params
    );
}

/// One device whose three part uses name BU: P-1 holds it, P-2 and P-3 do not. One finding per part and key, each
/// targeting its part, and each quick fix drops the key from the uses of its part alone: P-1's blue stays.
#[test]
fn core085_reports_each_part_and_its_fix_keeps_a_variant_the_part_holds() {
    let mut fx = Fx::base();
    let blue = json!({"variants": [{"key": "BU", "name": {"en": "Blue"}, "part_number": "P-1-BU"}]});
    fx.part(0x7100, "P-1", &blue);
    fx.part(0x7200, "P-2", &json!({}));
    fx.part(0x7300, "P-3", &json!({}));
    fx.func_tagged(0x1001, 1, P1);
    fx.with_parts(0x21001, &[0x7100, 0x7200, 0x7300]);
    let r = fx
        .records
        .iter_mut()
        .find(|r| r["id"] == json!(us(0x21001)))
        .expect("the device record");
    for u in r["parts"].as_array_mut().expect("its part uses") {
        u["part"]["variant"] = json!("BU");
    }
    let mut f = findings_of(&fx.build(), "EW-CORE-085");
    f.sort_by_key(targets);
    let parts: Vec<_> = f.iter().map(|f| targets(f)[1]).collect();
    assert_eq!(
        parts,
        [(Kind::Part, uid(0x7200)), (Kind::Part, uid(0x7300))],
        "{f:#?}"
    );
    for (finding, dropped) in f.iter().zip([1usize, 2]) {
        let value = &finding.fix.as_ref().expect("a quick fix").params["value"];
        let keys: Vec<Option<&str>> = (0..3)
            .map(|i| value[i]["part"].get("variant").and_then(|v| v.as_str()))
            .collect();
        let mut expected = [Some("BU"); 3];
        expected[dropped] = None;
        assert_eq!(keys, expected, "{value}");
    }
}
