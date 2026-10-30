// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Command parameters as JSON (C-8): every registered command has an example that validates against its
//! schema, resolves and round-trips; wrong parameters are rejected with the JSON Pointer of the culprit.

mod common;

use std::collections::BTreeSet;

use common::fixture::*;
use common::*;
use ew_commands::{AnyCommand, CmdError, CommandRegistry, validate_schema};
use serde_json::{Value, json};

/// One example parameter object per command, referencing the golden IDs.
fn examples() -> Vec<(&'static str, Value)> {
    let fn_a = json!({"cp": {"function": FN_102F2.to_string(), "idx": 1}});
    let fn_b = json!({"cp": {"function": FN_102G1.to_string(), "idx": 1}});
    vec![
        (
            "page.create",
            json!({"after": PAGE_102.to_string(), "title": "Control"}),
        ),
        (
            "page.delete",
            json!({"pages": [PAGE_101.to_string()], "delete_devices": true}),
        ),
        (
            "page.renumber",
            json!({"scope": {"pages": [PAGE_101.to_string(), PAGE_102.to_string()]},
                "start": 110, "step": 2, "keep_gaps": true}),
        ),
        (
            "page.reorder",
            json!({"pages": [PAGE_102.to_string()], "before": PAGE_101.to_string()}),
        ),
        (
            "structure.create_node",
            json!({"aspect": "function", "code": "P02", "parent": FUNC_P01.to_string(),
                "aliases": ["P2"], "description": "Second"}),
        ),
        (
            "section.create",
            json!({"number": 2, "folio_range": [200, 299], "title": "Wiring", "divider": true}),
        ),
        (
            "report_block.create",
            json!({"source": {"kind": "device_list"}, "form": FORM.to_string(),
                "section": SEC1.to_string(), "first_folio": 150}),
        ),
        (
            "device.create",
            json!({"letter": "K", "counter": 3, "kind": "psu", "function_text": "Relay"}),
        ),
        (
            "device.place",
            json!({"page": PAGE_102.to_string(), "symbol": SYM_TWO.to_string(), "pos": [7_937_500, 0],
                "orient": "r90", "variant": 0,
                "target": {"new_device": {"letter": "K"}},
                "connect": [{"cp": 0, "to": {"junction": {"junction": J5.to_string()}}}]}),
        ),
        (
            "rep.move",
            json!({"reps": [REP_102F2.to_string()], "delta": [7_937_500, -15_875_000],
                "page": PAGE_101.to_string()}),
        ),
        (
            "rep.transform",
            json!({"reps": [REP_102F2.to_string(), REP_102G1.to_string()], "rotate": 1, "mirror": true}),
        ),
        (
            "rep.set_anchor_policy",
            json!({"reps": [REP_102F2.to_string()], "anchor": "graphic_centre"}),
        ),
        (
            "connection.draw",
            json!({"a": fn_a, "b": fn_b, "page": PAGE_102.to_string(), "route": [[0, 0], [7_937_500, 0]]}),
        ),
        (
            "connection.route",
            json!({"connection": CON_W22.to_string(), "route": [[0, 0], [7_937_500, 7_937_500]]}),
        ),
        ("connection.delete", json!({"connections": [CON_W22.to_string()]})),
        (
            "junction.edit",
            json!({"junction": J5.to_string(), "style": "dot", "wiring": "star",
                "sequence": [CON_J5_IN.to_string(), CON_W22.to_string(), CON_W27.to_string()]}),
        ),
        (
            "ip.place",
            json!({"page": PAGE_101.to_string(), "symbol": SYM.to_string(), "pos": [0, 0],
                "name": "L-2", "direction": "source", "star_source": true, "sort": 3}),
        ),
        (
            "ip.continue",
            json!({"ip": FN_IP_101.to_string(), "page": PAGE_102.to_string(), "pos": [7_937_500, 15_875_000]}),
        ),
        (
            "potential.define",
            json!({"at": {"cp": {"function": FN_102G1.to_string(), "idx": 4}}, "name": "24VDC",
                "voltage": "24"}),
        ),
        (
            "terminal.add",
            json!({"strip": DEV_X1031.to_string(), "fndef": FNDEF_TERMINAL.to_string(),
                "terminals": [{"designation": "4", "seq": 4, "rail_bonded": true}]}),
        ),
        (
            "terminal.designate",
            json!({"assignments": [{"terminal": DEV_T1.to_string(), "designation": "7"}]}),
        ),
        (
            "cable.create",
            json!({"letter": "W", "counter": 2, "conductors": [{"core": "1", "shield": false, "pair": 1}]}),
        ),
        (
            "conductor.add",
            json!({"cable": DEV_3W1.to_string(), "conductors": [{"core": "D"}]}),
        ),
        (
            "box.black",
            json!({"page": PAGE_102.to_string(), "symbol": SYM_TWO.to_string(), "pos": [7_937_500, 0],
                "box": {"outline": [[0, 0], [31_750_000, 0], [31_750_000, 23_812_500]],
                    "cps": [{"idx": 0, "pos": [7_937_500, 0], "dir": "down"}]},
                "target": {"new_device": {"letter": "A"}}}),
        ),
        (
            "bond.create",
            json!({"a": fn_a.clone(), "b": fn_b.clone(), "kind": "equipotential",
                "via": CON_W27.to_string()}),
        ),
        (
            "external_ref.create",
            json!({"location": LOC.to_string(), "device": "JE-N1000-03", "connection": "L"}),
        ),
        (
            "assembly.attach",
            json!({"parent": DEV_102G1.to_string(), "children": [
                {"device": DEV_102F2.to_string(), "position": 1},
                {"device": DEV_101F1.to_string()}
            ]}),
        ),
        (
            "valve.add",
            json!({"module": DEV_102F2.to_string(), "position": 2,
                "part": {"part": {"copy": PART_MCB.to_string()}, "qty": "1"},
                "letter": "Q", "fndef": FNDEF_MCB.to_string()}),
        ),
        (
            "device.link",
            json!({"from": DEV_102F2.to_string(), "to": DEV_101F1.to_string(), "kind": "drives"}),
        ),
        (
            "plc.config_project.create",
            json!({"name": "PLC1", "profile": "siemens-tia-intl@2027.1"}),
        ),
        (
            "plc.bind_channel",
            json!({"channel": DEV_T1.to_string(), "endpoint": fn_a.clone()}),
        ),
        ("plc.set_module_part", json!({"module": DEV_102F2.to_string()})),
        (
            "project.supply.set",
            json!({"path": "origin.source", "value": null}),
        ),
        ("library.embed", json!({"copies": [{"$k": "form"}]})),
        (
            "revision.create",
            json!({"label": "V3.0", "date": "2025-06-02", "author": "engineer",
                "description": "Changes on 102", "page_notes": [{"page": PAGE_102.to_string()}]}),
        ),
        (
            "fields.set",
            json!({"kind": "device", "targets": [DEV_102F2.to_string()], "path": "tag.counter", "value": 5}),
        ),
        (
            "edit.delete",
            json!({"targets": [{"kind": "device", "id": DEV_102F2.to_string()}]}),
        ),
        (
            "numbering.renumber",
            json!({"scope": "project", "scheme": "page_based", "include_frozen": true,
                "options": {"only_nonconforming": false, "close_gaps": true}}),
        ),
        (
            "project.settings.set",
            json!({"member": "numbering", "value": {"default": "page-based", "schemes": [
                {"key": "page-based", "format": "{page}{letter}{counter}", "rules": []}]}}),
        ),
        (
            "project.company.set",
            json!({"name": "Example Panels", "notice": "Property of Example Panels."}),
        ),
        (
            "project.frames.set",
            json!({"default": FORM.to_string(),
                "per_page_type": [{"page_type": "title_page", "form": FORM.to_string()}]}),
        ),
        (
            "project.languages.set",
            json!({"source": "en", "date": "dd_mm_yyyy", "decimal_separator": "point"}),
        ),
        ("project.checks.set", json!({"release_scheme": "pre-delivery"})),
        (
            "project.assets.purge",
            json!({"assets": [format!("sha256:{}", "ab".repeat(32))]}),
        ),
        (
            "rep.exchange_symbol",
            json!({"rep": REP_102F2.to_string(), "symbol": SYM_TWO.to_string(),
                "pin_map": [{"symbol_cp": 0, "function_cp": 1}]}),
        ),
        (
            "project.drawing.set",
            json!({"crossings": "hops", "marker_mode": "explicit"}),
        ),
        // WP-S2.05 part 2
        (
            "fields.set_many",
            json!({"edits": [
                {"kind": "device", "targets": [DEV_102F2.to_string()], "path": "function_text", "value": "Spare"},
                {"kind": "device", "targets": [DEV_101F1.to_string()], "path": "function_text", "value": "Reserve"}]}),
        ),
        // WP-S2.05 part 3
        (
            "edit.paste",
            json!({"page": PAGE_101.to_string(), "at": [0, 0], "fragment": {"format": "0.1",
                "project": PAGE_101.to_string(), "grid": [7_937_500, 3], "page_kind": "multi_line", "anchor": [0, 0],
                "records": [], "copies": [], "nodes": []}}),
        ),
        // WP-S2.04 part 3
        (
            "connection.draw_to_ip",
            json!({"a": fn_a, "page": PAGE_102.to_string(), "route": [[0, 0], [7_937_500, 0]],
                "ip": {"symbol": SYM.to_string(), "pos": [7_937_500, 0], "name": "24VDC-2",
                    "direction": "source", "star_source": true, "sort": 2}}),
        ),
        (
            "ip.arrange",
            json!({"ips": [FN_IP_102.to_string(), FN_IP_101.to_string()], "set_sort": true}),
        ),
        (
            "box.reshape",
            json!({"rep": REP_102F2.to_string(),
                "outline": [[0, 0], [31_750_000, 0], [31_750_000, 23_812_500], [0, 23_812_500]],
                "cps": [{"idx": 0, "pos": [7_937_500, 0], "dir": "up"}]}),
        ),
        // WP-S2.04 part 4a
        (
            "box.structure",
            json!({"page": PAGE_101.to_string(),
                "outline": [[0, 0], [50_000_000, 0], [50_000_000, 30_000_000], [0, 30_000_000]],
                "style": "dotted", "caption": "External Connection",
                "assigns": {"location": LOC.to_string()}}),
        ),
        (
            "item.place",
            json!({"page": PAGE_101.to_string(),
                "kind": {"free_text": {"text": "40x60", "at": [50_800_000, 30_480_000], "angle": 90_000,
                    "style": "text.normal", "align": "centre"}}}),
        ),
        (
            "marker.place",
            json!({"page": PAGE_102.to_string(), "connections": [CON_W22.to_string(), CON_W27.to_string()],
                "at": [7_937_500, 15_875_000], "angle": 90_000, "csa": "1.5", "colours": [["BK"], ["BU"]]}),
        ),
        // FT-410
        ("parts.import", json!({"copies": [{"$k": "part"}]})),
        // Cycle 33
        (
            "connection.draw_many",
            json!({"connections": [{"a": fn_a, "b": fn_b}]}),
        ),
        // WP-S2.07 part 1
        (
            "terminal.insert",
            json!({"strip": DEV_X1031.to_string(), "before": DEV_T2.to_string(),
                "fndef": FNDEF_TERMINAL.to_string(), "terminals": [{"designation": "9"}]}),
        ),
        (
            "terminal.move",
            json!({"terminals": [DEV_T3.to_string()], "before": DEV_T1.to_string()}),
        ),
        // WP-S1.19 part 3
        (
            "part.assign",
            json!({"devices": [DEV_T3.to_string()], "part": FNDEF_TERMINAL.to_string(), "mode": "replace",
                "from": FNDEF_TERMINAL.to_string(), "qty": "2"}),
        ),
        (
            "library.update",
            json!({"copies": [{"$k": "part"}], "cp_maps": [{"copy": FNDEF_TERMINAL.to_string(), "map": [[1, 2]]}],
                "embed": [{"$k": "manufacturer"}]}),
        ),
        // WP-S1.19 part 5
        (
            "macro.insert",
            json!({"copies": [{"$k": "macro"}], "macro": FNDEF_TERMINAL.to_string(), "page": DEV_T1.to_string(),
                "at": [0, 0], "value_set": "1", "values": {"pos": 2}, "instance": DEV_T3.to_string()}),
        ),
    ]
}

fn any(name: &str, params: Value) -> AnyCommand {
    AnyCommand {
        name: name.into(),
        params,
    }
}

fn invalid(name: &str, params: Value) -> (String, String, String) {
    match CommandRegistry::builtin().resolve(&any(name, params)) {
        Err(CmdError::InvalidParams {
            command,
            path,
            message,
        }) => (command, path, message),
        Err(other) => panic!("expected InvalidParams, got {other:?}"),
        Ok(_) => panic!("expected the parameters to be rejected"),
    }
}

#[test]
fn every_registered_command_has_an_example() {
    let registry = CommandRegistry::builtin();
    let registered: BTreeSet<&str> = registry.commands().map(|c| c.meta().name).collect();
    let covered: BTreeSet<&str> = examples().iter().map(|(n, _)| *n).collect();
    assert!(!registered.is_empty());
    assert_eq!(registered, covered);
    assert_eq!(examples().len(), covered.len(), "one example per command");
}

#[test]
fn examples_validate_against_their_schema() {
    let registry = CommandRegistry::builtin();
    for (name, params) in examples() {
        let schema = registry.get(name).expect("registered").schema();
        assert_eq!(validate_schema(schema, &params), Ok(()), "{name}");
    }
}

#[test]
fn examples_resolve_and_round_trip() {
    let registry = CommandRegistry::builtin();
    for (name, params) in examples() {
        let schema = registry.get(name).expect("registered").schema();
        let resolved = registry
            .resolve(&any(name, params))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(resolved.name(), name);
        let json = resolved.to_json().expect("serialisable");
        assert_eq!(validate_schema(schema, &json), Ok(()), "{name}: serialised form");
        let again = registry
            .resolve(&any(name, json.clone()))
            .unwrap_or_else(|e| panic!("{name}: round trip: {e}"));
        assert_eq!(again.to_json().expect("serialisable"), json, "{name}");
    }
}

#[test]
fn a_missing_required_member_is_reported_at_the_object() {
    let (command, path, message) = invalid("connection.route", json!({"connection": CON_W22.to_string()}));
    assert_eq!(command, "connection.route");
    assert_eq!(path, "");
    assert!(message.contains("`route`"), "{message}");
    let (_, path, _) = invalid("fields.set", json!({}));
    assert_eq!(path, "");
}

#[test]
fn an_unknown_member_is_reported_at_the_member() {
    let (_, path, message) = invalid("page.create", json!({"after": PAGE_102.to_string(), "bogus": 1}));
    assert_eq!(path, "/bogus");
    assert!(message.contains("bogus"), "{message}");
    // Inside a nested variant object as well.
    let (_, path, _) = invalid(
        "connection.draw",
        json!({"a": {"cp": {"function": FN_102F2.to_string(), "idx": 1, "extra": 0}},
            "b": {"junction": {"junction": J5.to_string()}}}),
    );
    assert_eq!(path, "/a/cp/extra");
}

#[test]
fn a_wrong_type_deep_inside_is_reported_with_its_pointer() {
    let a = json!({"cp": {"function": FN_102F2.to_string(), "idx": "one"}});
    let b = json!({"junction": {"junction": J5.to_string()}});
    let (_, path, message) = invalid("connection.draw", json!({"a": a, "b": b}));
    assert_eq!(path, "/a/cp/idx");
    assert!(message.contains("expected integer"), "{message}");

    let a = json!({"cp": {"function": FN_102F2.to_string(), "idx": 1}});
    let (_, path, _) = invalid(
        "connection.draw",
        json!({"a": a, "b": b, "route": [[0, 0], ["x", 1]]}),
    );
    assert_eq!(path, "/route/1/0");

    let (_, path, _) = invalid(
        "connection.route",
        json!({"connection": CON_W22.to_string(), "route": [[0, 0], [1]]}),
    );
    assert_eq!(path, "/route/1", "a point needs two coordinates");
}

#[test]
fn a_wrong_enum_variant_is_rejected() {
    let (_, path, _) = invalid(
        "ip.place",
        json!({"page": PAGE_101.to_string(), "symbol": SYM.to_string(), "pos": [0, 0],
            "name": "L-2", "direction": "sideways"}),
    );
    assert_eq!(path, "/direction");
    let (_, path, _) = invalid(
        "page.create",
        json!({"after": PAGE_102.to_string(), "kind": "nonsense"}),
    );
    assert_eq!(path, "/kind");
    // An externally tagged variant that does not exist.
    let (_, path, _) = invalid("page.renumber", json!({"scope": {"everything": []}, "start": 1}));
    assert_eq!(path, "/scope");
}

#[test]
fn the_error_of_a_named_variant_points_into_it() {
    let (_, path, _) = invalid(
        "page.renumber",
        json!({"scope": {"pages": [PAGE_101.to_string(), 7]}, "start": 1}),
    );
    assert_eq!(path, "/scope/pages/1");
}

#[test]
fn a_malformed_uuid_is_reported_where_it_stands() {
    let (_, path, _) = invalid("connection.delete", json!({"connections": ["not-a-uuid"]}));
    assert_eq!(path, "/connections/0");
}

#[test]
fn parameters_of_the_wrong_shape_are_rejected_at_the_root() {
    let (_, path, message) = invalid("page.create", json!([1, 2]));
    assert_eq!(path, "");
    assert!(message.contains("expected object"), "{message}");
}

#[test]
fn an_alias_resolves_to_the_canonical_command() {
    let registry = CommandRegistry::builtin();
    let (_, params) = examples()
        .into_iter()
        .find(|(n, _)| *n == "device.place")
        .expect("example");
    let resolved = registry.resolve(&any("symbol.place", params.clone())).unwrap();
    assert_eq!(resolved.name(), "device.place");
    let (command, _, _) = invalid("symbol.place", json!({}));
    assert_eq!(command, "device.place", "errors name the canonical command");
    let via_alias = AnyCommand::of(&*resolved).expect("serialisable");
    assert_eq!(via_alias.name, "device.place");
}

#[test]
fn an_unknown_name_is_an_unknown_command() {
    let err = CommandRegistry::builtin()
        .resolve(&any("page.explode", json!({})))
        .map(|_| ())
        .unwrap_err();
    assert_eq!(
        err,
        CmdError::UnknownCommand {
            name: "page.explode".into()
        }
    );
}

#[test]
fn any_command_runs_with_validated_parameters_only() {
    let mut e = engine(golden_model());
    let bad = any("connection.route", json!({"connection": CON_W22.to_string()}));
    let err = e.execute(&bad).unwrap_err();
    assert!(matches!(err, CmdError::InvalidParams { .. }), "{err:?}");
    assert_eq!(e.model().rev().0, 0);
    let good = any(
        "connection.route",
        json!({"connection": CON_W22.to_string(), "route": [[0, 0], [7_937_500, 7_937_500]]}),
    );
    e.execute(&good).unwrap();
    assert_eq!(e.model().rev().0, 1);
}

// ---------------------------------------------------------------------------------------------------------
// Integer limits of the JSON encoding (I-JSON, 2^53): floats and out-of-range integers are rejected.

const MAX_SAFE: i64 = 9_007_199_254_740_991;

fn move_params(delta: Value) -> Value {
    json!({"reps": [REP_102F2.to_string()], "delta": delta})
}

#[test]
fn a_point_rejects_floats_and_integers_beyond_two_to_the_53() {
    let registry = CommandRegistry::builtin();
    for bad in [
        json!([1.5, 0]),
        json!([0, 1.5]),
        json!([9_007_199_254_740_992_i64, 0]),
        json!([0, -9_007_199_254_740_992_i64]),
    ] {
        let (command, path, _) = invalid("rep.move", move_params(bad.clone()));
        assert_eq!(command, "rep.move");
        assert!(path.starts_with("/delta"), "{bad}: {path}");
    }
    for ok in [
        json!([MAX_SAFE, 0]),
        json!([0, -MAX_SAFE]),
        json!([-MAX_SAFE, MAX_SAFE]),
    ] {
        assert!(
            registry
                .resolve(&any("rep.move", move_params(ok.clone())))
                .is_ok(),
            "{ok}"
        );
    }
}

#[test]
fn an_integer_parameter_rejects_floats_and_integers_beyond_two_to_the_53() {
    for (name, member, base) in [
        ("page.create", "number", json!({"title": "Control"})),
        (
            "page.renumber",
            "start",
            json!({"scope": {"pages": [PAGE_101.to_string()]}}),
        ),
    ] {
        for bad in [
            json!(1.5),
            json!(9_007_199_254_740_992_i64),
            json!(-9_007_199_254_740_992_i64),
        ] {
            let mut params = base.clone();
            params[member] = bad.clone();
            let (command, path, message) = invalid(name, params);
            assert_eq!(command, name);
            // a float is reported at the member; a too large integer fails the `u32` conversion (the path
            // of that serde error is not asserted)
            if bad.is_f64() {
                assert_eq!(path, format!("/{member}"), "{name} {bad}: {message}");
            }
        }
    }
}
