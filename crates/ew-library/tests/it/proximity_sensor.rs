// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1840: the three-wire PNP proximity sensor among the starter parts, on its own starter symbol.

use ew_format::ew_schema::{LibDocument, LibRef};
use ew_library::{PartQuery, Registry, starter};

/// A search for a proximity sensor finds the generic PNP sensor, whose three pins are its cores brown (L+), black
/// (the switched output) and blue (M), on the function the starter symbol draws, with a pin for each.
#[test]
fn a_search_for_a_proximity_sensor_finds_the_three_wire_sensor() {
    let reg = Registry::starter_only();
    let hits = reg.search(&PartQuery {
        text: "proximity sensor".into(),
        ..PartQuery::default()
    });
    let hit = hits
        .iter()
        .find(|h| ew_derive::text_in(&h.description, None).contains("PNP"))
        .unwrap_or_else(|| {
            panic!(
                "no PNP sensor among {:?}",
                hits.iter().map(|h| &h.description).collect::<Vec<_>>()
            )
        });
    let r = LibRef {
        library: ew_ids::Id::from_uuid(hit.library),
        item: ew_ids::Id::from_uuid(hit.item),
        version_req: "^0.1".into(),
        extra: Default::default(),
    };
    let Ok(LibDocument::Part(part)) = reg.resolve(&r) else {
        panic!("the sensor resolves to a part")
    };
    let designations: Vec<&str> = part
        .item
        .cp_pattern
        .iter()
        .map(|c| c.designation.as_str())
        .collect();
    assert_eq!(designations, ["BN", "BK", "BU"]);
    let main = part
        .item
        .function_template
        .iter()
        .find(|t| t.main)
        .expect("a main function");
    let Ok(LibDocument::Symbol(symbol)) = LibDocument::from_json(
        starter::item("ew.iec.proximity-sensor-pnp")
            .expect("the sensor's symbol")
            .json,
    ) else {
        panic!("a symbol")
    };
    assert_eq!(
        symbol.item.fndef.item, main.fndef.item,
        "the symbol draws the sensor's function"
    );
    assert_eq!(
        symbol.item.cps.len(),
        designations.len(),
        "the symbol draws every pin"
    );
}
