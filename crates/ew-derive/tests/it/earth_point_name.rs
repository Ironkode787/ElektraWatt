// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1933: a protective-earth potential that only an earth symbol names (an earth connection, which has no tag: an
//! imported earth point whose pin is `1`) is called PE, not `1` (doc 08 §3.5).

use crate::common;

use common::{Fx, id, uid};
use ew_derive::{PotentialNameOf, PotentialPartition, Q};
use serde_json::json;

const FNDEF_EARTH: u128 = 0x501;
const FNDEF_LAMP: u128 = 0x502;
const SYM_EARTH: u128 = 0x510;
const SYM_LAMP: u128 = 0x511;
const P1: u128 = 0x100;
const P2: u128 = 0x101;
const EARTH: u128 = 0x1000;
const EARTH_FN: u128 = 0x1001;
const LAMP: u128 = 0x1100;
const LAMP_FN: u128 = 0x1101;

/// The name of the potential on the earth point's pin of role `role`, the earth point drawn on page 1 and the lamp
/// whose PE pin is wired to it on page 2.
fn earth_name(role: &str) -> String {
    let mut fx = Fx::new();
    fx.lib_item(
        "fndef",
        FNDEF_EARTH,
        json!({"meta": {}, "category": ["EarthingBar", "Misc", "Point"], "contact_kind": "pole",
            "cps": [{"idx": 0, "designation": "1", "potential_role": role}]}),
    );
    fx.fndef(FNDEF_LAMP, ["Misc", "Misc", "Lamp"], &[(0, "X1"), (1, "PE")]);
    fx.symbol(SYM_EARTH, FNDEF_EARTH, json!("origin"), &[(0, 0, 0, 0)]);
    fx.symbol(
        SYM_LAMP,
        FNDEF_LAMP,
        json!("origin"),
        &[(0, 0, 0, 0), (1, 1, 0, 4_000_000)],
    );
    fx.page(P1, 1, None, "multi_line", &[], "one");
    fx.page(P2, 2, None, "multi_line", &[], "two");
    fx.device(EARTH, "X", Some(1));
    fx.function(EARTH_FN, Some(EARTH), FNDEF_EARTH, true);
    fx.device(LAMP, "E", Some(1))
        .function(LAMP_FN, Some(LAMP), FNDEF_LAMP, true);
    fx.rep(0x2001, P1, SYM_EARTH, &[EARTH_FN], 0, 0);
    fx.rep(0x2101, P2, SYM_LAMP, &[LAMP_FN], 0, 0);
    fx.push(json!({
        "$k": "connection", "id": uid(0x3001), "page": uid(P2),
        "a": {"cp": [uid(LAMP_FN), 1]}, "b": {"cp": [uid(EARTH_FN), 0]},
    }));
    let m = fx.build();
    let q = Q::new(&m);
    let part = q.get::<PotentialPartition>(());
    let p = part
        .potentials()
        .find(|p| p.cps.iter().any(|c| c.function == id(EARTH_FN)))
        .expect("the earth point's potential");
    q.get::<PotentialNameOf>(p.id).name.text.clone()
}

#[test]
fn a_protective_earth_named_only_by_an_earth_point_is_pe() {
    assert_eq!(earth_name("pe"), "PE");
    // A functional earth keeps the pin's name, as every other fallback.
    assert_eq!(earth_name("fe"), "1");
}
