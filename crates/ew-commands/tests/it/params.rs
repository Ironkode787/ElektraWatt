// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Command parameters as JSON (C-8): every example resolves against its schema and
//! round-trips; wrong parameters are rejected with the JSON Pointer of the culprit.

use crate::common;

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
        (
            "connection.route_many",
            json!({"routes": [{"connection": CON_W22.to_string(), "route": [[0, 0], [7_937_500, 7_937_500]]}]}),
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
            "valve.set_occupancy",
            json!({"module": DEV_102F2.to_string(), "position": 2,
                "occupancy": {"valve": {"state": "spare", "part": PART_MCB.to_string()}}}),
        ),
        (
            "fluid.port.close",
            json!({"function": FN_102F2.to_string(), "idx": 1, "closure": {"kind": "plug"}}),
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
            "bus.logical_net.create",
            json!({"bus": "profibus_dp", "name": "DP", "rate": 1_500_000}),
        ),
        (
            "bus.physical_net.create",
            json!({"name": "DP", "medium": "copper"}),
        ),
        (
            "plc.bind_channel",
            json!({"channel": DEV_T1.to_string(), "endpoint": fn_a.clone()}),
        ),
        ("plc.set_module_part", json!({"module": DEV_102F2.to_string()})),
        (
            "plc.set_start",
            json!({"modules": [DEV_102F2.to_string()], "dir": "output", "spec": "Q10"}),
        ),
        (
            "plc.set_profile",
            json!({"config_project": DEV_T1.to_string(), "profile": "iec-61131-3"}),
        ),
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
            "revision.set_author",
            json!({"revision": DEV_T1.to_string(), "role": "engineer"}),
        ),
        (
            "revision.withdraw",
            json!({"revision": DEV_T1.to_string(), "reason": "Issued in error"}),
        ),
        (
            "revision.approve",
            json!({"revision": DEV_T1.to_string(), "checker": "checker", "date": "2025-06-02",
                "scheme": "pre-delivery",
                "packs": [{"id": "EW-CORE", "version": "0.0.0",
                    "sha256": "sha256:0000000000000000000000000000000000000000000000000000000000000000"}],
                "waivers_ack": [], "content_digest":
                    "sha256:1111111111111111111111111111111111111111111111111111111111111111"}),
        ),
        (
            "revision.release",
            json!({"revision": DEV_T1.to_string(), "released_on": "2025-06-09", "generation": 7,
                "content_digest": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
                "derivation_profile": "2027.1",
                "rule_packs": [{"id": "EW-CORE", "version": "0.0.0",
                    "sha256": "sha256:0000000000000000000000000000000000000000000000000000000000000000"}],
                "page_digests": {"0190a5e0-0000-7000-8000-000000000102": "0123456789abcdef0123456789abcdef"},
                "changed_pages": ["0190a5e0-0000-7000-8000-000000000102"],
                "archive": {"name": "V2.0.ewarchive",
                    "sha256": "sha256:2222222222222222222222222222222222222222222222222222222222222222"},
                "git_tag": "ew/rev/V2.0"}),
        ),
        (
            "fields.set",
            json!({"kind": "device", "targets": [DEV_102F2.to_string()], "path": "tag.counter", "value": 5,
                "copies": [{"$k": "part"}]}),
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
            "check.waive",
            json!({"rule": "EW-CORE-041", "fingerprint": "9c2e5a0b7d14f3e8a6c1b05d2e9f41ab",
                   "reason": {"en-GB": "PE stud is part of NSYPMM107"}, "author": "engineer", "date": "2025-04-14"}),
        ),
        (
            "check.unwaive",
            json!({"waiver": "6c1d0a37-92e4-5f8b-a0d6-3e7b9c2f4a18"}),
        ),
        // Its rows are not parameters: the example writes nothing.
        ("external.apply", json!({"generation": 2, "commit": "a1b2c3d"})),
        (
            "merge.resolve",
            json!({"choices": [{"conflict": "5a8f1e3c-0d72-5b49-9e16-c3a7f0b8d254", "take": "theirs"}]}),
        ),
        (
            "project.assets.purge",
            json!({"assets": [format!("sha256:{}", "ab".repeat(32))]}),
        ),
        (
            "rep.exchange_symbol",
            json!({"rep": REP_102F2.to_string(), "symbol": SYM_TWO.to_string(),
                "pin_map": [{"symbol_cp": 0, "function_cp": 1}], "copies": [{"$k": "symbol"}]}),
        ),
        (
            "project.drawing.set",
            json!({"crossings": "hops", "marker_mode": "explicit"}),
        ),
        (
            "fields.set_many",
            json!({"edits": [
                {"kind": "device", "targets": [DEV_102F2.to_string()], "path": "function_text", "value": "Spare"},
                {"kind": "device", "targets": [DEV_101F1.to_string()], "path": "function_text", "value": "Reserve"}],
                "copies": [{"$k": "part"}]}),
        ),
        (
            "edit.paste",
            json!({"page": PAGE_101.to_string(), "at": [0, 0], "fragment": {"format": "0.1",
                "project": PAGE_101.to_string(), "grid": [7_937_500, 3], "page_kind": "multi_line", "anchor": [0, 0],
                "records": [], "copies": [], "nodes": []}}),
        ),
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
        ("parts.import", json!({"copies": [{"$k": "part"}]})),
        (
            "connection.draw_many",
            json!({"connections": [{"a": fn_a, "b": fn_b}]}),
        ),
        (
            "terminal.insert",
            json!({"strip": DEV_X1031.to_string(), "before": DEV_T2.to_string(),
                "fndef": FNDEF_TERMINAL.to_string(), "terminals": [{"designation": "9"}]}),
        ),
        (
            "terminal.move",
            json!({"terminals": [DEV_T3.to_string()], "before": DEV_T1.to_string()}),
        ),
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
        (
            "macro.insert",
            json!({"copies": [{"$k": "macro"}], "macro": FNDEF_TERMINAL.to_string(), "page": DEV_T1.to_string(),
                "at": [0, 0], "orient": "r90", "value_set": "1", "values": {"pos": 2}, "instance": DEV_T3.to_string()}),
        ),
        (
            "rep.join_device",
            json!({"rep": REP_102F2.to_string(), "device": DEV_T3.to_string()}),
        ),
        (
            "part.edit_copy",
            json!({"copy": PART_MCB.to_string(), "part": {"description": "MCB"}, "fork": DEV_T3.to_string(),
                "copies": [{"$k": "manufacturer"}]}),
        ),
        // The layout IDs are placeholders (resolving never reads the model)
        (
            "layout.space.create",
            json!({"location": LOC.to_string(), "name": "Cabinet 1"}),
        ),
        ("layout.space.delete", json!({"space": DEV_T1.to_string()})),
        (
            "layout.enclosure.create",
            json!({"space": DEV_T1.to_string(), "device": DEV_102G1.to_string(),
                "pose": {"t": [0, 0, 0], "q": [1_000_000_000, 0, 0, 0]}, "pe_bond": true}),
        ),
        (
            "layout.enclosure.delete",
            json!({"enclosure": DEV_T1.to_string()}),
        ),
        (
            "layout.carrier.add",
            json!({"surface": DEV_T1.to_string(), "kind": "rail", "part": PART_MCB.to_string(),
                "path": [[20_000_000, 800_000_000], [620_000_000, 800_000_000]],
                "params": {"rail": {"profile": "ts35x7_5"}}, "pe_bond": true}),
        ),
        (
            "layout.carrier.edit",
            json!({"carrier": DEV_T1.to_string(), "part": null,
                "params": {"duct": {"width": 40_000_000, "height": 60_000_000, "cover": true}}}),
        ),
        (
            "layout.carrier.delete",
            json!({"carrier": DEV_T1.to_string(), "then": "rehome_to_surface"}),
        ),
        (
            "layout.place",
            json!({"device": DEV_101F1.to_string(), "part": PART_MCB.to_string(),
                "carrier": {"carrier": DEV_T1.to_string()},
                "pose": {"t": [96_000_000, 0, 0], "q": [1_000_000_000, 0, 0, 0]}}),
        ),
        (
            "layout.move",
            json!({"placement": DEV_T1.to_string(), "carrier": {"surface": DEV_T2.to_string()},
                "pose": {"t": [10_000_000, 20_000_000, 0], "q": [0, 0, 0, 1_000_000_000]}}),
        ),
        ("layout.remove", json!({"placement": DEV_T1.to_string()})),
        (
            "layout.lock",
            json!({"placement": DEV_T1.to_string(), "locked": true}),
        ),
        (
            "page.surface_view.set",
            json!({"page": PAGE_101.to_string(), "surface": DEV_T2.to_string(),
                "datum_at": [25_000_000, 167_000_000], "label_mode": "full"}),
        ),
        (
            "layout.place_assembly",
            json!({"device": DEV_102G1.to_string(), "carrier": {"carrier": DEV_T1.to_string()},
                "s": 20_000_000, "mode": "push"}),
        ),
        (
            "layout.auto_fill",
            json!({"carrier": DEV_T1.to_string(), "devices": [DEV_101F1.to_string()], "order": "schematic"}),
        ),
        (
            "layout.enclosure.create_from_part",
            json!({"location": LOC.to_string(), "part": PART_MCB.to_string(), "function": DEV_T2.to_string(),
                "pose": {"t": [0, 0, 0], "q": [1_000_000_000, 0, 0, 0]}, "pe_bond": true}),
        ),
        (
            "layout.space.edit",
            json!({"space": DEV_T1.to_string(), "name": null}),
        ),
        (
            "layout.enclosure.edit",
            json!({"enclosure": DEV_T1.to_string(), "pose": {"t": [100_000_000, 0, 0], "q": [1_000_000_000, 0, 0, 0]}}),
        ),
        (
            "layout.surface.edit",
            json!({"surface": DEV_T1.to_string(), "pe_bond": true}),
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
            ..
        }) => (command, path, message),
        Err(other) => panic!("expected InvalidParams, got {other:?}"),
        Ok(_) => panic!("expected the parameters to be rejected"),
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

/// Wrong parameters are rejected at the JSON Pointer of the culprit: `(command, parameters, pointer, a word the
/// message holds)`.
#[test]
fn a_rejection_points_at_the_culprit() {
    let cp = |idx: Value| json!({"cp": {"function": FN_102F2.to_string(), "idx": idx}});
    let junction = json!({"junction": {"junction": J5.to_string()}});
    let page_ip = |direction: &str| {
        json!({"page": PAGE_101.to_string(), "symbol": SYM.to_string(), "pos": [0, 0],
            "name": "L-2", "direction": direction})
    };
    let cases: Vec<(&str, Value, &str, &str)> = vec![
        // A missing required member is reported at the object.
        (
            "connection.route",
            json!({"connection": CON_W22.to_string()}),
            "",
            "`route`",
        ),
        ("fields.set", json!({}), "", ""),
        // An unknown member is reported at the member, inside a nested variant object as well.
        (
            "page.create",
            json!({"after": PAGE_102.to_string(), "bogus": 1}),
            "/bogus",
            "bogus",
        ),
        (
            "connection.draw",
            json!({"a": {"cp": {"function": FN_102F2.to_string(), "idx": 1, "extra": 0}}, "b": junction}),
            "/a/cp/extra",
            "",
        ),
        // A wrong type deep inside is reported with its pointer.
        (
            "connection.draw",
            json!({"a": cp(json!("one")), "b": junction}),
            "/a/cp/idx",
            "expected integer",
        ),
        (
            "connection.draw",
            json!({"a": cp(json!(1)), "b": junction, "route": [[0, 0], ["x", 1]]}),
            "/route/1/0",
            "",
        ),
        // A point needs two coordinates.
        (
            "connection.route",
            json!({"connection": CON_W22.to_string(), "route": [[0, 0], [1]]}),
            "/route/1",
            "",
        ),
        // A wrong enum variant, and an externally tagged variant that does not exist.
        ("ip.place", page_ip("sideways"), "/direction", ""),
        (
            "page.create",
            json!({"after": PAGE_102.to_string(), "kind": "nonsense"}),
            "/kind",
            "",
        ),
        (
            "page.renumber",
            json!({"scope": {"everything": []}, "start": 1}),
            "/scope",
            "",
        ),
        // The error of a named variant points into it.
        (
            "page.renumber",
            json!({"scope": {"pages": [PAGE_101.to_string(), 7]}, "start": 1}),
            "/scope/pages/1",
            "",
        ),
        // A malformed UUID is reported where it stands.
        (
            "connection.delete",
            json!({"connections": ["not-a-uuid"]}),
            "/connections/0",
            "",
        ),
        // Parameters of the wrong shape are rejected at the root.
        ("page.create", json!([1, 2]), "", "expected object"),
    ];
    for (name, params, pointer, word) in cases {
        let (command, path, message) = invalid(name, params.clone());
        assert_eq!(command, name, "{name} {params}");
        assert_eq!(path, pointer, "{name} {params}: {message}");
        assert!(message.contains(word), "{name} {params}: {message}");
    }
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
