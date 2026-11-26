// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The treatment of a wire's ends on the sample (doc 13 §4): 101.4, 101.5 and 101.6 leave -X1011:PE clamp a through one
//! star, 2.5, 0.75 and 2.5 mm²: the two of one size share a twin ferrule, the third takes a ferrule of its own, each
//! with its part from the generic stock; the shop's `two_wires` set to a ferrule each gives three ferrules, which the
//! terminal's two-conductor clamp cannot take (EW-POT-010); `project.settings.set` sets the two members in words.

use crate::common;

use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::output::NavKind;
use ew_api::{AnyCommand, Session};
use ew_ids::Id;
use serde_json::{Value, json};

fn run(s: &mut Session, name: &str, params: Value) -> Result<ew_api::Receipt, ew_api::CmdError> {
    let cmd = AnyCommand {
        name: name.into(),
        params,
    };
    match s.execute(&cmd) {
        Err(ew_api::CmdError::NeedsPreview { .. }) => s.preview(&cmd).and_then(|p| s.commit(p)),
        other => other,
    }
}

/// The cells of the columns `keys` of the wire `no` in the Connections table.
fn cells(s: &Session, no: &str, keys: &[&str]) -> Vec<String> {
    let t = s.navigator_table(NavKind::Connections);
    let row = (t.nodes.iter())
        .find(|n| n.table_row && n.label == no)
        .unwrap_or_else(|| panic!("no wire {no}"));
    keys.iter()
        .map(|k| {
            let c = t.columns.iter().position(|c| c.key == *k).expect("the column");
            row.cells[c].clone()
        })
        .collect()
}

/// The EW-POT-010 findings of a check before delivery, as their messages.
fn over_capacity(s: &Session) -> Vec<String> {
    s.check(&PRE_DELIVERY, CheckMode::Release)
        .findings
        .iter()
        .filter(|f| f.rule == "EW-POT-010")
        .map(|f| f.message())
        .collect()
}

#[test]
fn two_wires_of_one_size_on_one_clamp_share_a_twin_ferrule() {
    let mut s = common::sample();
    let keys = ["from_end", "from_part", "from_strip"];
    assert_eq!(
        cells(&s, "101.4", &keys),
        ["twin 2.5 BU · 101.6", "EW-GEN-FER-2X2.5", "10 mm"]
    );
    assert_eq!(cells(&s, "101.6", &keys[..1]), ["twin 2.5 BU · 101.4"]);
    assert_eq!(
        cells(&s, "101.5", &keys),
        ["ferrule 0.75 GY", "EW-GEN-FER-0.75", "8 mm"]
    );
    assert_eq!(over_capacity(&s), Vec::<String>::new());
    run(
        &mut s,
        "project.settings.set",
        json!({"member": "workshop.two_wires", "value": "ferrule_each"}),
    )
    .expect("a ferrule each");
    assert_eq!(
        cells(&s, "101.4", &keys),
        ["ferrule 2.5 BU", "EW-GEN-FER-2.5", "10 mm"]
    );
    let over = over_capacity(&s);
    assert!(
        over.iter()
            .any(|m| m.starts_with("-X1011:PE") && m.ends_with("takes 2 conductors; 3 land on it")),
        "{over:?}"
    );
}

#[test]
fn the_workshop_members_are_set_in_words() {
    let mut s = common::sample();
    run(
        &mut s,
        "project.settings.set",
        json!({"member": "workshop.ferrules", "value": "every_fine_stranded_end"}),
    )
    .expect("every fine-stranded end");
    assert_eq!(
        s.query().project().settings.workshop.ferrules,
        ew_schema::FerruleRule::EveryFineStrandedEnd
    );
    let refused = run(
        &mut s,
        "project.settings.set",
        json!({"member": "workshop.ferrules", "value": "sideways"}),
    )
    .expect_err("an unknown rule");
    assert!(
        format!("{refused}").contains("not_into_spring_clamps"),
        "{refused}"
    );
    run(
        &mut s,
        "project.settings.set",
        json!({"member": "workshop.ferrules", "value": null}),
    )
    .expect("the default");
    assert_eq!(
        s.query().project().settings.workshop.ferrules,
        ew_schema::FerruleRule::NotIntoSpringClamps
    );
}

/// The copy of the stock part numbered `number`.
fn stock(s: &Session, number: &str) -> String {
    let q = s.query();
    let parts = q.parts();
    q.project()
        .settings
        .workshop
        .stock
        .iter()
        .find(|r| {
            parts
                .get(r.copy)
                .is_some_and(|c| c.item.part_number.as_str() == number)
        })
        .map(|r| r.copy.to_string())
        .unwrap_or_else(|| panic!("no stock part {number}"))
}

/// The connection that lands on the end `end` of wire `no` and the member that holds its end set by hand.
fn end_of(s: &Session, no: &str, end: usize) -> (String, &'static str) {
    let t = s.navigator_table(NavKind::Connections);
    let row = (t.nodes.iter())
        .find(|n| n.table_row && n.label == no)
        .unwrap_or_else(|| panic!("no wire {no}"));
    let (c, at_a) = s.wire_end(&row.records, end).expect("an end of the project");
    (
        c.to_string(),
        if at_a {
            "overrides.end_a"
        } else {
            "overrides.end_b"
        },
    )
}

/// The EW-MFG-009 findings, as their messages and the commands of their quick fixes.
fn misfits(s: &Session) -> Vec<(String, Option<(String, Value)>)> {
    s.check(&PRE_DELIVERY, CheckMode::Release)
        .findings
        .iter()
        .filter(|f| f.rule == "EW-MFG-009")
        .map(|f| {
            (
                f.message(),
                f.fix.as_ref().map(|x| (x.command.clone(), x.params.clone())),
            )
        })
        .collect()
}

/// The sample's 101.4 lands on -101G1:PE with a ring lug set by hand (no ring lug in the stock: no part); a twin ferrule
/// set there instead has no second wire and a ferrule part of 0.75 mm² does not fit the 2.5 mm² wire (EW-MFG-009), whose
/// quick fix gives the rule's ferrule back; a part the project does not hold is refused.
#[test]
fn an_end_set_by_hand_prints_as_set_and_a_misfit_is_reported() {
    let mut s = common::sample();
    let keys = ["to_end", "to_part", "to_strip"];
    assert_eq!(cells(&s, "101.4", &keys), ["ring lug", "", ""]);
    let (holder, member) = end_of(&s, "101.4", 1);
    let set = |s: &mut Session, value: Value| {
        run(
            s,
            "fields.set",
            json!({"kind": "connection", "targets": [holder], "path": member, "value": value}),
        )
    };
    assert_eq!(misfits(&s), Vec::new());
    // Its To end row in Properties: the ring lug, set by hand, with Reset; the stored member is not shown again.
    let sheet = s.property_sheet(&[holder.parse().expect("an ID")]);
    let rows: Vec<&ew_api::output::PropRow> = sheet.sections.iter().flat_map(|x| x.rows.iter()).collect();
    assert!(
        rows.iter().all(|r| !r.path.starts_with("overrides.end_")),
        "{:?}",
        rows.iter().map(|r| &r.path).collect::<Vec<_>>()
    );
    let row = (sheet.sections.iter().flat_map(|x| x.rows.iter()))
        .find(|r| r.key == "connection/derived.to_end")
        .expect("the To end row");
    assert_eq!(
        (&row.value, &row.provenance, row.path.as_str()),
        (
            &ew_api::output::RowValue::One {
                json: json!("ring_lug"),
                text: "ring_lug".into()
            },
            &ew_api::output::Provenance::Overridden {
                note: "set by hand".into()
            },
            "derived.to_end"
        )
    );
    assert!(row.actions.contains(&ew_api::output::RowAction::Reset), "{row:?}");
    set(&mut s, json!({"treatment": "twin_ferrule"})).expect("a twin ferrule");
    assert_eq!(cells(&s, "101.4", &keys[..1]), ["twin 2.5 BU"]);
    let found = misfits(&s);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0]
            .0
            .ends_with("set by hand does not fit: a twin ferrule with no second wire"),
        "{found:?}"
    );
    let (command, params) = found[0].1.clone().expect("Reset to the rule");
    run(&mut s, &command, params).expect("the quick fix");
    assert_eq!(cells(&s, "101.4", &keys[..1]), ["ferrule 2.5 BU"]);
    assert_eq!(misfits(&s), Vec::new());
    let small = stock(&s, "EW-GEN-FER-0.75");
    set(&mut s, json!({"treatment": "ferrule", "part": {"copy": small}})).expect("a ferrule part");
    assert_eq!(cells(&s, "101.4", &keys[1..2]), ["EW-GEN-FER-0.75"]);
    let found = misfits(&s);
    assert!(
        found
            .iter()
            .any(|(m, _)| m.ends_with("a ferrule for 0.75 mm² on a 2.5 mm² wire")),
        "{found:?}"
    );
    let unknown = ew_ids::Uuid::from_u128(0x5e47_0000_0000_4000_8000_0000_0000_0042).to_string();
    set(&mut s, json!({"part": {"copy": unknown}})).expect_err("a part the project does not hold");
}

/// The cells `keys` of the wires `nos`.
fn ends(s: &Session, nos: &[&str], key: &str) -> Vec<String> {
    nos.iter().map(|n| cells(s, n, &[key]).remove(0)).collect()
}

/// 101.4, 101.5 and 101.6 land on -X1011:PE clamp a through one star's feed arm: each wire's end there is its own. Two
/// of them set by hand to two treatments keep them, and the third keeps the rule's; each is stored on a connection of
/// its own wire, never on the shared arm. The star then loses 101.5: the two ends set by hand stay.
#[test]
fn two_wires_at_one_clamp_keep_their_own_ends() {
    let mut s = common::sample();
    let star = ["101.4", "101.5", "101.6"];
    let shared: Vec<String> = {
        let t = s.navigator_table(NavKind::Connections);
        let records = |no: &str| {
            (t.nodes.iter())
                .find(|n| n.table_row && n.label == no)
                .map(|n| n.records.iter().map(ToString::to_string).collect::<Vec<_>>())
                .unwrap_or_default()
        };
        let (a, b) = (records("101.4"), records("101.6"));
        a.into_iter().filter(|r| b.contains(r)).collect()
    };
    assert!(!shared.is_empty(), "the star's feed arm");
    for (no, treatment) in [("101.4", "ring_lug"), ("101.6", "bare")] {
        let (holder, member) = end_of(&s, no, 0);
        assert!(!shared.contains(&holder), "{no} holds its end on the shared arm");
        run(
            &mut s,
            "fields.set",
            json!({"kind": "connection", "targets": [holder], "path": member, "value": {"treatment": treatment}}),
        )
        .expect("an end set by hand");
    }
    assert_eq!(
        ends(&s, &star, "from_end"),
        ["ring lug", "ferrule 0.75 GY", "bare"]
    );
    // 101.5 is pulled: the connections no other wire stands for go, the star keeps two wires.
    let gone: Vec<String> = {
        let t = s.navigator_table(NavKind::Connections);
        let row = (t.nodes.iter())
            .find(|n| n.table_row && n.label == "101.5")
            .expect("101.5");
        let others: Vec<String> = (t.nodes.iter())
            .filter(|n| n.table_row && n.label != "101.5")
            .flat_map(|n| n.records.iter().map(ToString::to_string))
            .collect();
        row.records
            .iter()
            .map(ToString::to_string)
            .filter(|r| !others.contains(r))
            .collect()
    };
    run(&mut s, "connection.delete", json!({ "connections": gone })).expect("101.5 pulled");
    // The numbers follow the pull; the two wires are known by their To ends, -101G1 and -X1031:PE.
    let t = s.navigator_table(NavKind::Connections);
    let col = |k: &str| t.columns.iter().position(|c| c.key == k).expect("a column");
    let from_end_to = |to: &str| {
        (t.nodes.iter())
            .find(|n| n.table_row && n.cells[col("from")] == "-X1011:PE" && n.cells[col("to")] == to)
            .map(|n| n.cells[col("from_end")].clone())
            .unwrap_or_else(|| panic!("no wire to {to}"))
    };
    assert_eq!(
        [from_end_to("-101G1"), from_end_to("-X1031:PE")],
        ["ring lug", "bare"]
    );
}

/// From any connection of a wire, Properties set both of its ends (`Session::layout_edit` of the rows From end and
/// To end); the stored member is the wire's own connection's.
#[test]
fn properties_set_either_end_of_a_wire() {
    let mut s = common::sample();
    let (holder, _) = end_of(&s, "101.1", 0);
    for (row, text, cell) in [
        ("derived.from_end", "bare", "from_end"),
        ("derived.to_end", "ring_lug", "to_end"),
    ] {
        let cmd = s
            .layout_edit(&ew_api::output::LayoutEdit {
                kind: "connection".into(),
                target: holder.parse().expect("an ID"),
                others: Vec::new(),
                row: row.into(),
                text: text.into(),
            })
            .expect("the end's command");
        run(&mut s, &cmd.name, cmd.params).expect("the end set by hand");
        assert_eq!(cells(&s, "101.1", &[cell])[0], text.replace('_', " "));
    }
}

/// A workshop setting that is not set shows the value that applies, as the project's default.
#[test]
fn an_unset_workshop_setting_shows_its_default() {
    let s = common::sample();
    let sheet = s.property_sheet(&[s.query().project().id.as_uuid()]);
    let row = (sheet.sections.iter().flat_map(|x| x.rows.iter()))
        .find(|r| r.key == "project/settings.workshop.two_wires")
        .expect("the Two wires row");
    assert_eq!(
        (&row.value, &row.provenance),
        (
            &ew_api::output::RowValue::One {
                json: json!("twin_ferrule"),
                text: "twin_ferrule".into()
            },
            &ew_api::output::Provenance::Derived {
                via: "project default".into()
            }
        )
    );
}

/// Each end's marker names the far end (doc 13 §5.1); a core and a jumper carry none; a long one is broken to the
/// stock's wire marker, and one that still does not fit is kept whole and EW-MFG-004 names it; wire-number marking
/// prints the number; a marker set by hand wins, `none` leaves the end unmarked.
#[test]
fn each_end_is_marked_with_its_far_end() {
    let mut s = common::sample();
    let keys = ["from_marker", "to_marker"];
    assert_eq!(cells(&s, "101.1", &keys), ["-101Q1:1", "-X1011:L1"]);
    assert_eq!(cells(&s, "-101W1 BN", &keys), ["", ""]);
    assert_eq!(cells(&s, "101.14", &keys), ["", ""]);
    // The stock's wire marker holds two lines of 12: a field wire's marker at the cabinet end is broken after its `-`.
    let long = cells(&s, "104.4", &keys);
    assert!(long.iter().any(|m| m == "+CAB-104A1- / A2:L+"), "{long:?}");
    let fits = |s: &Session| {
        s.check(&PRE_DELIVERY, CheckMode::Online)
            .findings
            .iter()
            .filter(|f| f.rule == "EW-MFG-004")
            .map(|f| f.message())
            .collect::<Vec<_>>()
    };
    assert_eq!(fits(&s), Vec::<String>::new());
    // Both ends on two lines of 12 do not fit a field wire's: kept whole, named by EW-MFG-004.
    run(
        &mut s,
        "project.settings.set",
        json!({"member": "workshop.marking", "value": "both_ends"}),
    )
    .expect("both-ends marking");
    let found = fits(&s);
    assert!(
        found
            .iter()
            .any(|m| m.ends_with("needs 16 characters on a line of 12")),
        "{found:?}"
    );
    run(
        &mut s,
        "project.settings.set",
        json!({"member": "workshop.marking", "value": null}),
    )
    .expect("the default");
    let (holder, member) = end_of(&s, "101.1", 0);
    for (marker, shown) in [(json!({"text": "K1"}), "K1"), (json!("none"), "")] {
        run(
            &mut s,
            "fields.set",
            json!({"kind": "connection", "targets": [holder], "path": member, "value": {"marker": marker}}),
        )
        .expect("a marker set by hand");
        assert_eq!(cells(&s, "101.1", &keys[..1]), [shown]);
    }
    run(
        &mut s,
        "project.settings.set",
        json!({"member": "workshop.marking", "value": "wire_number"}),
    )
    .expect("wire-number marking");
    assert_eq!(cells(&s, "101.2", &keys), ["101.2", "101.2"]);
}

/// The part of an end set by hand is a link: a fork of the part takes the end with it, and a part of the project's own
/// that an end names is not deleted: the delete policy refuses it, naming the wire.
#[test]
fn the_part_of_an_end_set_by_hand_is_a_link() {
    let mut s = common::sample();
    let copy = stock(&s, "EW-GEN-FER-2.5");
    let (holder, member) = end_of(&s, "101.4", 0);
    run(
        &mut s,
        "fields.set",
        json!({"kind": "connection", "targets": [holder], "path": member, "value": {"treatment": "ferrule", "part": {"copy": copy}}}),
    )
    .expect("an end set by hand");
    let stored = |s: &Session| {
        let q = s.query();
        let c = q
            .connections()
            .get(Id::from_uuid(holder.parse().expect("an ID")))
            .expect("the end's connection")
            .clone();
        let end = if member.ends_with("end_a") {
            c.overrides.end_a.clone()
        } else {
            c.overrides.end_b.clone()
        };
        end.and_then(|e| e.part).map(|p| p.copy.to_string())
    };
    assert_eq!(stored(&s), Some(copy.clone()));
    let mut item = {
        let q = s.query();
        serde_json::to_value(
            &q.parts()
                .get(Id::from_uuid(copy.parse().expect("an ID")))
                .expect("held")
                .item,
        )
        .expect("JSON")
    };
    item["order_number"] = json!("SHOP-2.5");
    let fork_item = ew_ids::Uuid::from_u128(0x5e47_0000_0000_4000_8000_0000_0000_0f0e).to_string();
    run(
        &mut s,
        "part.edit_copy",
        json!({"copy": copy, "part": item, "fork": fork_item}),
    )
    .expect("the fork");
    let fork = {
        let q = s.query();
        let project = q.project().id.as_uuid();
        q.parts()
            .iter()
            .find(|(_, c)| {
                c.library_id.as_uuid() == project && c.item.part_number.as_str() == "EW-GEN-FER-2.5"
            })
            .map(|(id, _)| id.to_string())
            .expect("the project's own copy")
    };
    assert_eq!(stored(&s), Some(fork.clone()), "the end follows the fork");
    // The stock follows the fork too: given its library copy back, only the end names the fork.
    let stock_now: Vec<Value> = s
        .query()
        .project()
        .settings
        .workshop
        .stock
        .iter()
        .map(|r| {
            let id = r.copy.to_string();
            json!({ "copy": if id == fork { copy.clone() } else { id } })
        })
        .collect();
    run(&mut s, "workshop.stock", json!({ "stock": stock_now })).expect("the stock as it was");
    let refused = match run(&mut s, "part.delete", json!({ "copy": fork })).expect_err("named by an end") {
        ew_api::CmdError::Model(_, Some(words)) => words.to_string(),
        other => panic!("refused in words: {other}"),
    };
    assert_eq!(
        refused,
        "part EW-GEN-FER-2.5 cannot be removed: wire 101.4 still names it"
    );
}

/// An end stored on any connection of the wire's own is where Properties and Reset find it: an end set on one of a
/// wire's own connections other than its carrier (as a carrier change leaves it) is the one the Properties row resets, and the Connections
/// table's first record of the wire is the connection that keeps its own values.
#[test]
fn an_end_is_reset_where_it_is_stored() {
    use ew_derive::WireRecordSet;
    let mut s = common::sample();
    let (wire, k, holder, at_a) = {
        let q = s.query();
        let records = q.get::<WireRecordSet>(());
        let shared = |c: &Id<ew_schema::Connection>| {
            records.iter().filter(|r| r.wire.constituents.contains(c)).count() > 1
        };
        records
            .iter()
            .filter(|r| r.number.is_some())
            .find_map(|r| {
                let own: Vec<(usize, Id<ew_schema::Connection>)> =
                    (r.wire.constituents.iter().copied().enumerate())
                        .filter(|(_, c)| !shared(c))
                        .collect();
                // Not the carrier, where an edit writes first: a connection of its own the wire takes its values from later.
                let (k, c) = *own
                    .iter()
                    .find(|(_, c)| *c != r.wire.id)
                    .filter(|_| own.len() >= 2)?;
                let a = [true, false]
                    .into_iter()
                    .find(|a| ew_derive::end_through(&q, &r.wire, k, *a) == Some(0))?;
                Some((r.number.clone()?, k, c, a))
            })
            .expect("a wire with two connections of its own")
    };
    let _ = k;
    let member = if at_a {
        "overrides.end_a"
    } else {
        "overrides.end_b"
    };
    run(
        &mut s,
        "fields.set",
        json!({"kind": "connection", "targets": [holder.to_string()], "path": member, "value": {"treatment": "bare"}}),
    )
    .expect("an end stored on the last own connection");
    assert_eq!(cells(&s, &wire, &["from_end"]), ["bare"]);
    assert_eq!(
        end_of(&s, &wire, 0),
        (
            holder.to_string(),
            if at_a {
                "overrides.end_a"
            } else {
                "overrides.end_b"
            }
        )
    );
    let cmd = s
        .layout_edit(&ew_api::output::LayoutEdit {
            kind: "connection".into(),
            target: holder.as_uuid(),
            others: Vec::new(),
            row: "derived.from_end".into(),
            text: String::new(),
        })
        .expect("Reset's command");
    run(&mut s, &cmd.name, cmd.params).expect("the end reset");
    assert_ne!(
        cells(&s, &wire, &["from_end"]),
        ["bare"],
        "Reset gave the rule's back"
    );
}

/// Properties on a stretch several wires share (the star's feed arm of 101.4, 101.5 and 101.6) show no one wire's
/// length or ends as editable: the rows are read-only and say that several wires share it; on a wire's own connection
/// a length typed in Properties is where the Connections table writes it, the wire's first own connection.
#[test]
fn properties_of_a_shared_stretch_are_read_only() {
    use ew_api::output::{Provenance, RowAction};
    use ew_derive::WireRecordSet;
    let mut s = common::sample();
    let (shared, own) = {
        let q = s.query();
        let records = q.get::<WireRecordSet>(());
        let star: Vec<&ew_derive::WireRecord> = records
            .iter()
            .filter(|r| matches!(r.number.as_deref(), Some("101.4" | "101.6")))
            .collect();
        let shared = (star[0].wire.constituents.iter())
            .find(|c| star[1].wire.constituents.contains(c))
            .copied()
            .expect("the star's feed arm");
        let wire = records
            .iter()
            .find(|r| r.own.len() >= 2)
            .expect("a wire with two connections of its own");
        (
            shared,
            (wire.own[0], wire.own[1], wire.number.clone().expect("a number")),
        )
    };
    let sheet = s.property_sheet(&[shared.as_uuid()]);
    for key in [
        "connection/derived.length",
        "connection/derived.from_end",
        "connection/derived.to_part",
    ] {
        let row = (sheet.sections.iter().flat_map(|x| x.rows.iter()))
            .find(|r| r.key == key)
            .unwrap_or_else(|| panic!("no row {key}"));
        assert!(
            !row.actions
                .iter()
                .any(|a| matches!(a, RowAction::Edit | RowAction::Override | RowAction::Reset)),
            "{key}: {:?}",
            row.actions
        );
        assert_eq!(
            row.provenance,
            Provenance::Derived {
                via: "several wires share this stretch: set a wire's own values in the Connections table"
                    .into()
            },
            "{key}"
        );
    }
    // On the wire's second own connection a length typed in Properties lands on its first, as the table's does.
    let (first, second, no) = own;
    let cmd = s
        .layout_edit(&ew_api::output::LayoutEdit {
            kind: "connection".into(),
            target: second.as_uuid(),
            others: Vec::new(),
            row: "derived.length".into(),
            text: "750".into(),
        })
        .expect("the length's command");
    run(&mut s, &cmd.name, cmd.params).expect("a length typed");
    assert_eq!(cells(&s, &no, &["length"]), ["750 mm"]);
    let q = s.query();
    let length = |c: Id<ew_schema::Connection>| q.connections().get(c).and_then(|c| c.overrides.length);
    assert!(length(first).is_some() && length(second).is_none());
}
