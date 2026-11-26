// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a part use prints, in every list (doc 07 §2.5 "Variants"): a device that names a variant of its part prints the
//! variant's own number in the I/O list, the PLC overview, the valve terminal list, the enclosure legend and the SQL
//! view's PLC channels, each read through `ew_derive::printed_part`.

use crate::common;

use ew_api::Session;
use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::output::{RowExport, row_export};
use ew_api::sql::Profile;
use ew_ids::Id;
use ew_schema::Device;
use serde_json::json;

/// The sample with the part of the device `tag` forked as the project's own with a variant `bu` of the number
/// `<number>-BU`, which the device takes; and that number.
fn with_variant(tag: &str) -> (Session, String) {
    let mut s = common::sample();
    let device = s.resolve_reference(&format!("tag:{tag}")).expect("the device");
    let copy = s
        .query()
        .devices()
        .get(Id::<Device>::from_uuid(device))
        .and_then(|d| d.parts.first().map(|u| u.part.copy.as_uuid()))
        .expect("its part");
    let mut d = s.part_draft(copy).expect("its draft");
    let number = format!("{}-BU", d.text("part_number"));
    assert!(d.add_table_row("variants"));
    s.set_draft_field(&mut d, "variants[0].key", "bu").expect("a key");
    s.set_draft_field(&mut d, "variants[0].name", "Blue")
        .expect("a name");
    s.set_draft_field(&mut d, "variants[0].part_number", &number)
        .expect("its number");
    let plan = s.part_edit_plan(&d).expect("the fork with its variant");
    let preview = s.preview(&plan.command).expect("its preview");
    s.commit(preview).expect("the fork");
    let mut uses = serde_json::to_value(
        &s.query()
            .devices()
            .get(Id::<Device>::from_uuid(device))
            .expect("the device")
            .parts,
    )
    .expect("its uses");
    uses[0]["part"]["variant"] = json!("bu");
    common::run_cmd(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [device.to_string()], "path": "parts", "value": uses}),
    );
    (s, number)
}

/// The CSV of the report kind `kind` over the whole project.
fn list(s: &Session, kind: &str) -> String {
    row_export(&s.query(), &RowExport::Kind(kind.to_owned()), None)
        .unwrap_or_else(|e| panic!("{kind}: {e}"))
        .csv
}

#[test]
fn the_io_list_and_the_plc_overview_print_a_cards_variant() {
    let (s, number) = with_variant("-104A1-A1");
    for kind in ["io_list", "plc_overview"] {
        assert!(list(&s, kind).contains(&number), "{kind} prints {number}");
    }
}

#[test]
fn the_valve_terminal_list_prints_a_valve_terminals_variant() {
    let (s, number) = with_variant("-201K1");
    assert!(list(&s, "valve_terminal_config").contains(&number));
}

#[test]
fn the_enclosure_legend_prints_a_placed_devices_variant() {
    let (s, number) = with_variant("-102F1");
    let legend = list(&s, "enclosure_legend");
    assert!(legend.contains(&number), "{legend}");
}

#[test]
fn the_sql_views_plc_channels_print_a_cards_variant() {
    let (s, number) = with_variant("-104A1-A1");
    let report = s.check(&PRE_DELIVERY, CheckMode::Release);
    let p = s.sql_projection(Some(&report)).expect("the projection");
    let set = p
        .reader(Profile::EW_SQL)
        .unwrap()
        .query("SELECT DISTINCT part_number FROM api_v1_plc_channel WHERE module = '-104A1-A1'")
        .unwrap();
    assert_eq!(set.rows.len(), 1);
    assert_eq!(set.rows[0][0].text(), number);
}
