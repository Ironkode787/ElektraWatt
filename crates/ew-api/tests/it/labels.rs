// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The label engine on the sample (doc 13 §5.3): the labels list holds a label of each set the sample has, in the order
//! the fitter applies them, each with the stock's marker that marks its set; a cable carries a marker at each end, and
//! a device two labels where the shop sets labels on the device and beside it.

use crate::common;

use ew_api::{AnyCommand, Session};
use ew_derive::{LabelRows, LabelSet};
use serde_json::json;

/// The sets of the sample's labels in their order, each once, and the copies of the first label of `set`.
fn sets(s: &Session) -> Vec<LabelSet> {
    let mut out: Vec<LabelSet> = s.query().get::<LabelRows>(()).iter().map(|l| l.set).collect();
    out.dedup();
    out
}

fn copies(s: &Session, set: LabelSet) -> u8 {
    let q = s.query();
    q.get::<LabelRows>(())
        .iter()
        .find(|l| l.set == set)
        .map(|l| l.copies)
        .expect("a label of the set")
}

#[test]
fn the_labels_follow_the_fitters_order_with_their_marker_parts() {
    let mut s = common::sample();
    assert_eq!(
        sets(&s),
        [
            LabelSet::WireMarkers,
            LabelSet::TerminalMarkers,
            LabelSet::StripMarkers,
            LabelSet::CableMarkers,
            LabelSet::DeviceLabels,
            LabelSet::TubeMarkers,
        ]
    );
    let q = s.query();
    let labels = q.get::<LabelRows>(());
    let first = labels
        .iter()
        .find(|l| l.set == LabelSet::WireMarkers)
        .expect("a wire marker");
    assert_eq!(first.lines, ["-101Q1:1"]);
    let part = first
        .marker_part
        .and_then(|p| q.parts().get(p))
        .expect("the stock's wire marker");
    assert_eq!(part.item.part_number.as_str(), "EW-GEN-MRK-1");
    drop(q);
    assert_eq!(copies(&s, LabelSet::CableMarkers), 2);
    assert_eq!(copies(&s, LabelSet::DeviceLabels), 1);
    let cmd = AnyCommand {
        name: "project.settings.set".into(),
        params: json!({"member": "workshop.device_labels", "value": "on_device_and_beside"}),
    };
    match s.execute(&cmd) {
        Err(ew_api::CmdError::NeedsPreview { .. }) => s.preview(&cmd).and_then(|p| s.commit(p)),
        other => other,
    }
    .expect("labels beside the devices");
    assert_eq!(copies(&s, LabelSet::DeviceLabels), 2);
}

/// The device labels of the placed devices come in their mounting order, before the devices no layout places.
#[test]
fn the_placed_devices_are_labelled_before_the_others() {
    let s = common::sample();
    let q = s.query();
    let placed = |id: ew_ids::Id<ew_schema::Device>| q.idx().placements_of_device(id).next().is_some();
    let order: Vec<bool> = q
        .get::<LabelRows>(())
        .iter()
        .filter(|l| l.set == LabelSet::DeviceLabels)
        .filter_map(|l| match l.target {
            ew_derive::LabelTarget::Device(d) => Some(placed(d)),
            _ => None,
        })
        .collect();
    assert!(order.contains(&true) && order.contains(&false), "{order:?}");
    assert!(order.windows(2).all(|w| w[0] || !w[1]), "{order:?}");
}

/// A terminal's marker carries its function text on a second line where its strip prints function texts.
#[test]
fn a_terminal_marker_says_its_function_where_the_strip_prints_it() {
    let mut s = common::sample();
    let lines = |s: &Session| {
        let id = s.resolve_reference("tag:-X1021:1").expect("the terminal");
        s.query()
            .get::<LabelRows>(())
            .iter()
            .find(|l| {
                l.set == LabelSet::TerminalMarkers
                    && l.target == ew_derive::LabelTarget::Device(ew_ids::Id::from_uuid(id))
            })
            .map(|l| l.lines.clone())
            .expect("the terminal's marker")
    };
    let terminal = s
        .resolve_reference("tag:-X1021:1")
        .expect("the terminal")
        .to_string();
    let strip = s.resolve_reference("tag:-X1021").expect("the strip").to_string();
    common::run_cmd(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [terminal], "path": "function_text", "value": {"en": "24V"}}),
    );
    assert_eq!(lines(&s), ["1"]);
    common::run_cmd(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [strip], "path": "strip.label_mode", "value": "function_text"}),
    );
    assert_eq!(lines(&s), ["1", "24V"]);
}

/// The project's export profile gives the label files their columns and delimiter; a column the labels list does not
/// have, and a profile without columns, are refused in words.
#[test]
fn the_export_profile_gives_the_label_files_their_columns() {
    let mut s = common::sample();
    let set = |s: &mut Session, value: serde_json::Value| {
        let cmd = AnyCommand {
            name: "project.settings.set".into(),
            params: json!({"member": "workshop.label_profile", "value": value}),
        };
        match s.execute(&cmd) {
            Err(ew_api::CmdError::NeedsPreview { .. }) => s.preview(&cmd).and_then(|p| s.commit(p)),
            other => other,
        }
    };
    let wrong =
        set(&mut s, json!({"columns": [{"key": "text_9", "heading": "T"}]})).expect_err("no such column");
    assert!(wrong.to_string().contains("text_9"), "{wrong}");
    assert!(set(&mut s, json!({"columns": []})).is_err());
    set(
        &mut s,
        json!({"columns": [{"key": "text_1", "heading": "Line"}, {"key": "copies", "heading": "N"}], "delimiter": "semicolon"}),
    )
    .expect("a profile of its own");
    let files = s
        .label_files(&ew_api::LabelScope::All, ew_api::LabelFileFormat::Csv, None)
        .expect("the label files");
    let wires = files
        .iter()
        .find(|f| f.set == LabelSet::WireMarkers)
        .expect("the wire markers' file");
    let text = String::from_utf8(wires.bytes.clone()).expect("UTF-8");
    assert!(text.starts_with("\u{feff}Line;N\r\n-101Q1:1;1\r\n"), "{text}");
}

/// A core's marker, where the shop marks cores, says in its For cell which core it marks by its cable and core, as the
/// wire list's No. does (`-101W1 BN at -X1011:L1/b`): a core has no wire number.
#[test]
fn a_cores_marker_is_for_its_cable_and_core() {
    let mut s = common::sample();
    let cmd = AnyCommand {
        name: "project.settings.set".into(),
        params: json!({"member": "workshop.mark_cores", "value": true}),
    };
    match s.execute(&cmd) {
        Err(ew_api::CmdError::NeedsPreview { .. }) => s.preview(&cmd).and_then(|p| s.commit(p)),
        other => other,
    }
    .expect("cores marked");
    let q = s.query();
    let set = ew_reports::project_rows(&q, &ew_schema::ReportKind::Labels, None).expect("the labels list");
    let at = set
        .columns
        .iter()
        .position(|c| c.key == "for")
        .expect("the For column");
    let fors: Vec<String> = (0..set.rows.len())
        .map(|i| ew_reports::csv::row_texts(&q, &set, i)[at].clone())
        .collect();
    assert!(fors.iter().any(|f| f == "-101W1 BN at -X1011:L1/b"), "{fors:?}");
    assert!(fors.iter().all(|f| !f.starts_with(' ')), "{fors:?}");
}

/// A label's row is what it labels (doc 13 §5.3): an existing tube-label block prints the sample's tube ends as it did
/// before the labels list took its columns, each tube's two ends in the tube list's order, the from end first, each row
/// the tube with its end and the tube its source, so that the block's exclusion of a tube applies and the row opens the
/// tube; a wire marker's row is the wire's end, a device label's the device.
#[test]
fn a_label_row_is_what_it_labels_with_its_record_as_its_source() {
    use ew_reports::{RowId, RowKey, RowKind};
    let s = common::sample();
    let q = s.query();
    let tubes = q.get::<ew_derive::TubeRows>(());
    let rows = |kind: ew_schema::ReportKind| {
        ew_reports::project_rows(&q, &kind, None)
            .expect("the list")
            .rows
            .into_iter()
            .filter(|r| r.kind == RowKind::Data)
            .map(|r| (r.id, r.sources))
            .collect::<Vec<_>>()
    };
    let want: Vec<(RowId, Vec<ew_schema::AnyId>)> = tubes
        .iter()
        .flat_map(|t| {
            (0..2).map(move |end| (RowId::entity(t.id).with(RowKey::Index(end)), vec![t.id.cast()]))
        })
        .collect();
    assert!(!want.is_empty(), "the sample has tubes");
    assert_eq!(rows(ew_schema::ReportKind::TubeLabels), want);
    let labels = q.get::<LabelRows>(());
    let all = rows(ew_schema::ReportKind::Labels);
    let row_of = |set: LabelSet| {
        let n = labels.iter().position(|l| l.set == set).expect("a label of the set");
        (labels[n].target.clone(), all[n].clone())
    };
    let (target, (id, sources)) = row_of(LabelSet::WireMarkers);
    let ew_derive::LabelTarget::WireEnd(wire, end) = target else { panic!("a wire end") };
    assert_eq!((id, sources), (RowId::entity(wire).with(RowKey::Index(u32::from(end))), vec![wire.cast()]));
    let (target, (id, sources)) = row_of(LabelSet::DeviceLabels);
    let ew_derive::LabelTarget::Device(d) = target else { panic!("a device") };
    assert_eq!((id, sources), (RowId::entity(d), vec![d.cast()]));
    let ids: std::collections::BTreeSet<&RowId> = all.iter().map(|(id, _)| id).collect();
    assert_eq!(ids.len(), all.len(), "no two rows of the labels list are one");
}

/// A terminal of several levels gets a marker per level (doc 13 §5.3), each with the level's designation: the sample's
/// terminal -X1021:6 drawn onto -X1021:5 makes a block of two levels, marked `5` and `6`; a terminal of one level keeps
/// its one marker.
#[test]
fn a_terminal_of_two_levels_gets_a_marker_per_level() {
    let mut s = common::sample();
    let device = |s: &Session, tag: &str| -> ew_ids::Id<ew_schema::Device> {
        ew_ids::Id::from_uuid(s.resolve_reference(&format!("tag:{tag}")).expect("the terminal"))
    };
    let markers = |s: &Session, t: ew_ids::Id<ew_schema::Device>| -> Vec<Vec<String>> {
        s.query()
            .get::<LabelRows>(())
            .iter()
            .filter(|l| l.set == LabelSet::TerminalMarkers && l.target.device() == Some(t))
            .map(|l| l.lines.clone())
            .collect()
    };
    let (five, four) = (device(&s, "-X1021:5"), device(&s, "-X1021:4"));
    assert_eq!(markers(&s, five), [["5"]]);
    let q = s.query();
    let six = device(&s, "-X1021:6");
    let f = q.idx().functions_of_device(six).next().expect("its function");
    let rep = q
        .reps()
        .iter()
        .find(|(_, r)| r.functions.contains(&f))
        .map(|(id, _)| id)
        .expect("its symbol");
    drop(q);
    common::run_cmd(
        &mut s,
        "rep.join_device",
        json!({"rep": rep.to_string(), "device": five.to_string()}),
    );
    assert_eq!(markers(&s, five), [["5"], ["6"]]);
    assert_eq!(markers(&s, four), [["4"]], "a terminal of one level");
}
