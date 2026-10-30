// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Connection-point instantiation (`function_cps`, doc 03 §7, FR-DEV-008): FnDef CPs, overrides, black
//! boxes from a part pattern, functions from a part's function template.

mod common;

use common::{Fx, LIBRARY, id, uid};
use ew_derive::{Cp, FunctionCps, Q};
use ew_ids::{Id, Uuid, v5::lib_copy_id};
use ew_schema::{Discipline, FnDef, PotentialType, Side};
use serde_json::{Value, json};

const FNDEF_MCB: u128 = 0x500;
const FNDEF_BOX: u128 = 0x501;
const PART_PLAIN: u128 = 0x600;
const PART_PATTERN: u128 = 0x601;
const FNDEF_TERM: u128 = 0x502;
const FNDEF_OTHER: u128 = 0x503;
const PART_TERM: u128 = 0x602;
const PART_TERM_SUBSET: u128 = 0x603;
const PART_TERM_OTHER: u128 = 0x604;

fn part_json(number: &str, extra: Value) -> Value {
    let mut p = json!({
        "meta": {},
        "manufacturer": {"library": uid(LIBRARY), "item": uid(0x9000), "version_req": "^1"},
        "part_number": number,
    });
    for (k, v) in extra.as_object().expect("object") {
        p[k] = v.clone();
    }
    p
}

fn base() -> Fx {
    let mut fx = Fx::new();
    // MCB 1P+N: duplicate designations "N" at idx 2 and 3 (REF-029).
    fx.fndef(
        FNDEF_MCB,
        ["ProtectionOvercurrent", "MCB", "1P+N"],
        &[(0, "1"), (1, "2"), (2, "N"), (3, "N")],
    );
    fx.fndef(FNDEF_BOX, ["Misc", "Misc", "BlackBox"], &[(0, "X")]);
    fx.lib_item("part", PART_PLAIN, part_json("PLAIN", json!({})));
    fx.lib_item(
        "part",
        PART_PATTERN,
        part_json(
            "PATTERN",
            json!({"cp_pattern": [
                {"idx": 0, "designation": "L", "level": 1, "side": "side1"},
                {"idx": 1, "designation": "+", "potential_role": "l_plus", "side": "side2"},
                {"idx": 2, "designation": "PE", "potential_role": "pe"},
            ]}),
        ),
    );
    fx
}

fn cps(fx: &Fx, f: u128) -> Vec<Cp> {
    let m = fx.build();
    let q = Q::new(&m);
    q.get::<FunctionCps>(id(f)).as_ref().clone()
}

fn designations(cps: &[Cp]) -> Vec<(u16, &str)> {
    cps.iter().map(|c| (c.idx, c.designation.as_str())).collect()
}

#[test]
fn fndef_cps_with_duplicate_designations() {
    let mut fx = base();
    fx.device(0x1000, "F", Some(1))
        .function(0x1001, Some(0x1000), FNDEF_MCB, true);
    let out = cps(&fx, 0x1001);
    assert_eq!(designations(&out), [(0, "1"), (1, "2"), (2, "N"), (3, "N")]);
    for c in &out {
        assert_eq!(c.alias, None);
        assert_eq!(c.description, None);
        assert!(!c.unused);
        assert!(!c.overridden);
        assert!(c.physical);
    }
}

#[test]
fn override_replaces_designation_and_alias() {
    let mut fx = base();
    fx.device(0x1000, "F", Some(1))
        .function(0x1001, Some(0x1000), FNDEF_MCB, true)
        .with(
            "cp_overrides",
            json!([{"idx": 0, "designation": "L1", "alias": "1 / L1", "description": {"en": "Phase"}}]),
        );
    let out = cps(&fx, 0x1001);
    assert_eq!(designations(&out), [(0, "L1"), (1, "2"), (2, "N"), (3, "N")]);
    assert_eq!(out[0].alias.as_deref(), Some("1 / L1"));
    assert!(out[0].description.is_some());
    assert!(out[0].overridden);
    assert!(out[1..].iter().all(|c| !c.overridden));
}

#[test]
fn override_without_designation_keeps_it_and_sets_unused() {
    let mut fx = base();
    fx.device(0x1000, "F", Some(1))
        .function(0x1001, Some(0x1000), FNDEF_MCB, true)
        .with("cp_overrides", json!([{"idx": 3, "unused": true}]));
    let out = cps(&fx, 0x1001);
    assert_eq!(out[3].designation, "N");
    assert!(out[3].unused);
    assert!(out[3].overridden);
    assert!(!out[2].unused);
}

#[test]
fn override_of_a_missing_idx_is_ignored() {
    let mut fx = base();
    fx.device(0x1000, "F", Some(1))
        .function(0x1001, Some(0x1000), FNDEF_MCB, true)
        .with("cp_overrides", json!([{"idx": 9, "designation": "Z"}]));
    let out = cps(&fx, 0x1001);
    assert_eq!(out.len(), 4);
    assert!(out.iter().all(|c| !c.overridden));
}

#[test]
fn black_box_takes_the_cps_of_the_first_part_with_a_pattern() {
    let mut fx = base();
    fx.device(0x1000, "U", Some(1))
        .with(
            "parts",
            json!([
                {"part": {"copy": uid(PART_PLAIN)}, "qty": "1"},
                {"part": {"copy": uid(PART_PATTERN)}, "qty": "1"},
            ]),
        )
        .function(0x1001, Some(0x1000), FNDEF_BOX, true)
        .with("kind", json!("black_box"))
        .with("cp_overrides", json!([{"idx": 2, "alias": "earth"}]));
    let out = cps(&fx, 0x1001);
    assert_eq!(designations(&out), [(0, "L"), (1, "+"), (2, "PE")]);
    assert_eq!(out[0].level, Some(1));
    assert!(out[0].side.is_some());
    assert_eq!(out[1].potential_role, Some(PotentialType::LPlus));
    assert!(out.iter().all(|c| c.physical && !c.source_output && !c.optional));
    assert_eq!(out[2].alias.as_deref(), Some("earth"));
    assert!(out[2].overridden);
}

#[test]
fn black_box_without_a_pattern_falls_back_to_the_fndef() {
    let mut fx = base();
    fx.device(0x1000, "U", Some(1))
        .with("parts", json!([{"part": {"copy": uid(PART_PLAIN)}, "qty": "1"}]))
        .function(0x1001, Some(0x1000), FNDEF_BOX, true)
        .with("kind", json!("black_box"));
    let out = cps(&fx, 0x1001);
    assert_eq!(designations(&out), [(0, "X")]);
}

#[test]
fn missing_function_and_missing_def_have_no_cps() {
    // A function naming a missing FnDef copy cannot exist in a valid snapshot (EW-INV-001), so only the
    // missing function is testable here; the missing-def branch is defensive.
    let fx = base();
    assert!(cps(&fx, 0x4242).is_empty());
}

/// The `LibRef` JSON naming the library item `n` (an FnDef whose copy `fndef_copy(n)` is embedded).
fn fndef_ref(n: u128) -> Value {
    json!({"library": uid(LIBRARY), "item": uid(n), "version_req": "^1"})
}

/// The copy ID of library item `n`: `v5(NS_LIBCOPY, library, item)`, the ID that a template reference
/// resolves to (doc 05 §13.2). The function's `def` must be this ID for the template to name it.
fn fndef_copy(n: u128) -> String {
    let copy: Id<FnDef> = lib_copy_id(Uuid::from_u128(LIBRARY), Uuid::from_u128(n));
    copy.as_uuid().hyphenated().to_string()
}

/// An embedded FnDef with item ID `n` and its real copy ID.
fn lib_fndef(fx: &mut Fx, n: u128, category: [&str; 3], cps: Value) {
    fx.lib(json!({
        "$k": "fndef",
        "id": fndef_copy(n),
        "library_id": uid(LIBRARY),
        "item_id": uid(n),
        "version": "1.0.0",
        "sha256": common::sha(0x42),
        "meta": {}, "category": category, "contact_kind": "pole", "cps": cps,
    }));
}

/// A terminal FnDef whose CPs carry a discipline, an alias and flags, and a family of terminal parts whose
/// CP patterns differ from it.
fn with_terminals() -> Fx {
    let mut fx = base();
    lib_fndef(
        &mut fx,
        FNDEF_TERM,
        ["Terminal", "Terminal", "Feed"],
        json!([
            {"idx": 0, "designation": "a", "alias": "A", "discipline": "bus", "potential_role": "l_minus",
             "source_output": true, "optional": true, "level": 9, "side": "side2"},
            {"idx": 1, "designation": "b", "discipline": "pneumatic", "physical": false},
            {"idx": 2, "designation": "c"},
            {"idx": 3, "designation": "d", "alias": "D"},
        ]),
    );
    lib_fndef(
        &mut fx,
        FNDEF_OTHER,
        ["Terminal", "Terminal", "Other"],
        json!([{"idx": 0, "designation": "o"}]),
    );
    // The pattern differs from the FnDef in designation, level, side and (for idx 1) potential role.
    let pattern = json!([
        {"idx": 0, "designation": "1", "level": 1, "side": "side1"},
        {"idx": 1, "designation": "2", "level": 2, "side": "side2", "potential_role": "l_plus"},
        {"idx": 2, "designation": "3", "level": 3},
        {"idx": 3, "designation": "4"},
    ]);
    fx.lib_item(
        "part",
        PART_TERM,
        part_json(
            "TERM",
            json!({"cp_pattern": pattern, "function_template": [
                {"fndef": fndef_ref(FNDEF_TERM), "main": true},
            ]}),
        ),
    );
    // A template naming only idx 3 and 1 (given in that order): indices, not positions.
    fx.lib_item(
        "part",
        PART_TERM_SUBSET,
        part_json(
            "TERM-SUBSET",
            json!({"cp_pattern": pattern, "function_template": [
                {"fndef": fndef_ref(FNDEF_TERM), "main": true, "cps": [3, 1]},
            ]}),
        ),
    );
    // A pattern, but a template that names another FnDef.
    fx.lib_item(
        "part",
        PART_TERM_OTHER,
        part_json(
            "TERM-OTHER",
            json!({"cp_pattern": pattern, "function_template": [
                {"fndef": fndef_ref(FNDEF_OTHER), "main": true},
            ]}),
        ),
    );
    fx
}

fn terminal_function(fx: &mut Fx, parts: &[u128]) {
    let uses: Vec<Value> = parts
        .iter()
        .map(|p| json!({"part": {"copy": uid(*p)}, "qty": "1"}))
        .collect();
    fx.device(0x1000, "X", Some(1))
        .with("parts", Value::Array(uses))
        .push(json!({"$k": "function", "id": uid(0x1001), "device": uid(0x1000), "def": fndef_copy(FNDEF_TERM)}));
}

#[test]
fn terminal_function_from_part_template_uses_the_pattern() {
    let mut fx = with_terminals();
    // The plain part comes first and has no template: the first template match wins.
    terminal_function(&mut fx, &[PART_PLAIN, PART_TERM]);
    fx.record(0x1001)["cp_overrides"] = json!([{"idx": 2, "alias": "three"}]);
    let out = cps(&fx, 0x1001);
    assert_eq!(designations(&out), [(0, "1"), (1, "2"), (2, "3"), (3, "4")]);
    // From the pattern: level, side, clamp; the role from the pattern, else from the FnDef.
    assert_eq!((out[0].level, out[0].side.clone()), (Some(1), Some(Side::Side1)));
    assert_eq!((out[1].level, out[1].side.clone()), (Some(2), Some(Side::Side2)));
    assert_eq!((out[2].level, out[2].side.clone()), (Some(3), None));
    assert_eq!((out[3].level, out[3].side.clone()), (None, None));
    assert_eq!(out[0].potential_role, Some(PotentialType::LMinus));
    assert_eq!(out[1].potential_role, Some(PotentialType::LPlus));
    assert_eq!(out[2].potential_role, None);
    // From the FnDef: discipline, alias, source output, optional, physical.
    assert_eq!(out[0].discipline, Discipline::Bus);
    assert_eq!(out[0].alias.as_deref(), Some("A"));
    assert!(out[0].source_output && out[0].optional && out[0].physical);
    assert_eq!(out[1].discipline, Discipline::Pneumatic);
    assert!(!out[1].physical);
    assert_eq!(out[2].discipline, Discipline::Electrical);
    assert_eq!(out[3].alias.as_deref(), Some("D"));
    // Overrides apply afterwards.
    assert_eq!(out[2].alias.as_deref(), Some("three"));
    assert!(out[2].overridden);
}

#[test]
fn template_cps_subset_follows_template_indices() {
    let mut fx = with_terminals();
    terminal_function(&mut fx, &[PART_TERM_SUBSET]);
    let out = cps(&fx, 0x1001);
    // `cps` names CP indices (`CpData.idx`), not positions of the pattern, and the result is in idx order.
    assert_eq!(designations(&out), [(1, "2"), (3, "4")]);
    assert_eq!(out[0].discipline, Discipline::Pneumatic);
    assert_eq!(out[1].alias.as_deref(), Some("D"));
}

#[test]
fn function_not_named_by_a_template_keeps_fndef_cps() {
    let mut fx = with_terminals();
    // The part has a pattern, but its template names another FnDef; the pattern part has no template at all.
    terminal_function(&mut fx, &[PART_TERM_OTHER, PART_PATTERN]);
    let out = cps(&fx, 0x1001);
    assert_eq!(designations(&out), [(0, "a"), (1, "b"), (2, "c"), (3, "d")]);
    assert_eq!(out[0].level, Some(9));
    assert_eq!(out[0].discipline, Discipline::Bus);
}

// ---- FT-655: a function's template entry ----

const FNDEF_COIL: u128 = 0x510;
const FNDEF_CO: u128 = 0x511;
const PART_RELAY: u128 = 0x610;
const PART_RELAY_SPARE: u128 = 0x611;

/// A relay part with a coil and four changeover contacts of one FnDef, each contact a template entry naming three
/// pattern CPs (WP-S2.18's EW-GEN-RLY-1 shape), and a second part whose template has one changeover contact only.
fn with_relay() -> Fx {
    let mut fx = base();
    lib_fndef(
        &mut fx,
        FNDEF_COIL,
        ["Relay", "Coil", "Coil"],
        json!([{"idx": 0, "designation": "A1"}, {"idx": 1, "designation": "A2"}]),
    );
    lib_fndef(
        &mut fx,
        FNDEF_CO,
        ["Relay", "Contact", "CO"],
        json!([
            {"idx": 0, "designation": "11", "alias": "common"},
            {"idx": 1, "designation": "12", "discipline": "bus"},
            {"idx": 2, "designation": "14"},
        ]),
    );
    let designations = [
        "A1", "A2", "11", "12", "14", "21", "22", "24", "31", "32", "34", "41", "42", "44",
    ];
    let pattern: Vec<Value> = designations
        .iter()
        .enumerate()
        .map(|(i, d)| json!({"idx": i, "designation": d}))
        .collect();
    let mut template = vec![json!({"fndef": fndef_ref(FNDEF_COIL), "main": true, "cps": [0, 1]})];
    for k in 0..4u16 {
        let first = 2 + 3 * k;
        template.push(json!({"fndef": fndef_ref(FNDEF_CO), "cps": [first, first + 1, first + 2]}));
    }
    fx.lib_item(
        "part",
        PART_RELAY,
        part_json(
            "RELAY",
            json!({"cp_pattern": pattern, "function_template": template}),
        ),
    );
    fx.lib_item(
        "part",
        PART_RELAY_SPARE,
        part_json(
            "RELAY-SPARE",
            json!({
                "cp_pattern": [
                    {"idx": 0, "designation": "51"}, {"idx": 1, "designation": "52"}, {"idx": 2, "designation": "54"},
                ],
                "function_template": [{"fndef": fndef_ref(FNDEF_CO), "cps": [0, 1, 2]}],
            }),
        ),
    );
    fx
}

/// The relay device 0x1000 with the given part uses and function 0x1001 of FnDef `def` with `template_entry`.
fn relay_function(fx: &mut Fx, parts: &[u128], def: u128, entry: Option<Value>) {
    let uses: Vec<Value> = parts
        .iter()
        .map(|p| json!({"part": {"copy": uid(*p)}, "qty": "1"}))
        .collect();
    let mut f = json!({"$k": "function", "id": uid(0x1001), "device": uid(0x1000), "def": fndef_copy(def)});
    if let Some(e) = entry {
        f["template_entry"] = e;
    }
    fx.device(0x1000, "K", Some(1))
        .with("parts", Value::Array(uses))
        .push(f);
}

fn relay_cps(parts: &[u128], def: u128, entry: Option<Value>) -> Vec<Cp> {
    let mut fx = with_relay();
    relay_function(&mut fx, parts, def, entry);
    cps(&fx, 0x1001)
}

#[test]
fn each_contact_of_one_fndef_takes_its_own_template_entry() {
    let expected = [
        [(0, "11"), (1, "12"), (2, "14")],
        [(0, "21"), (1, "22"), (2, "24")],
        [(0, "31"), (1, "32"), (2, "34")],
        [(0, "41"), (1, "42"), (2, "44")],
    ];
    for (k, want) in expected.iter().enumerate() {
        let out = relay_cps(&[PART_RELAY], FNDEF_CO, Some(json!(k + 1)));
        // The CPs keep the FnDef's indices (a symbol's `fn_cp` and the FnDef's internal links name them) and
        // print the entry's pattern designations.
        assert_eq!(designations(&out), want.to_vec(), "contact {}", k + 1);
        // The other members come from the FnDef CP each stands for.
        assert_eq!(out[0].alias.as_deref(), Some("common"));
        assert_eq!(out[1].discipline, Discipline::Bus);
        assert_eq!(out[2].discipline, Discipline::Electrical);
    }
    let coil = relay_cps(&[PART_RELAY], FNDEF_COIL, Some(json!(0)));
    assert_eq!(designations(&coil), [(0, "A1"), (1, "A2")]);
}

#[test]
fn a_function_without_template_entry_takes_the_first_entry_of_its_fndef() {
    // A file written before FT-655: as before, the first entry naming the FnDef.
    let out = relay_cps(&[PART_RELAY], FNDEF_CO, None);
    assert_eq!(designations(&out), [(0, "11"), (1, "12"), (2, "14")]);
    let coil = relay_cps(&[PART_RELAY], FNDEF_COIL, None);
    assert_eq!(designations(&coil), [(0, "A1"), (1, "A2")]);
}

#[test]
fn a_stale_template_entry_falls_back_to_the_first_entry_of_its_fndef() {
    // Out of range in every part use.
    let out = relay_cps(&[PART_RELAY], FNDEF_CO, Some(json!(99)));
    assert_eq!(designations(&out), [(0, "11"), (1, "12"), (2, "14")]);
    // In range, but the entry names another FnDef (the coil).
    let out = relay_cps(&[PART_RELAY], FNDEF_CO, Some(json!(0)));
    assert_eq!(designations(&out), [(0, "11"), (1, "12"), (2, "14")]);
    // The maximum index does not overflow.
    let out = relay_cps(&[PART_RELAY], FNDEF_CO, Some(json!(u16::MAX)));
    assert_eq!(designations(&out), [(0, "11"), (1, "12"), (2, "14")]);
}

#[test]
fn the_template_entry_is_read_in_the_first_part_use_that_has_it() {
    // The spare part comes first: its entry 0 names the FnDef, so a function with entry 0 takes it, while entry
    // 3 is only in the relay part.
    let out = relay_cps(&[PART_RELAY_SPARE, PART_RELAY], FNDEF_CO, Some(json!(0)));
    assert_eq!(designations(&out), [(0, "51"), (1, "52"), (2, "54")]);
    let out = relay_cps(&[PART_RELAY_SPARE, PART_RELAY], FNDEF_CO, Some(json!(3)));
    assert_eq!(designations(&out), [(0, "31"), (1, "32"), (2, "34")]);
    // Without the member the first part use with an entry of the FnDef wins, as before.
    let out = relay_cps(&[PART_RELAY_SPARE, PART_RELAY], FNDEF_CO, None);
    assert_eq!(designations(&out), [(0, "51"), (1, "52"), (2, "54")]);
}

#[test]
fn overrides_apply_to_the_fndef_index_of_a_positional_entry() {
    let mut fx = with_relay();
    relay_function(&mut fx, &[PART_RELAY], FNDEF_CO, Some(json!(2)));
    fx.record(0x1001)["cp_overrides"] = json!([{"idx": 2, "designation": "24x"}]);
    let out = cps(&fx, 0x1001);
    assert_eq!(designations(&out), [(0, "21"), (1, "22"), (2, "24x")]);
    assert!(out[2].overridden);
}
