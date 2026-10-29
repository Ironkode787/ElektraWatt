// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.16 part 3: the EW-CORE rules of cables, parts and devices (EW-CORE-031, -032, -034, -036, -037, -041 to
//! -045, -048), EW-CORE-008's `form` half (FT-368) and the canonical finding order (FT-594). One positive and one
//! negative fixture per rule (P3-1), prevent rules refusing the candidate and only it, and the ID-permutation tests
//! of WP-S1.16 §10.

mod common;

use common::*;
use ew_checks::{CheckMode, FULL, Finding, PRE_DELIVERY, Severity, prevent, run};
use ew_derive::Q;
use ew_derive::ew_schema::{AnyId, Kind};
use ew_model::Model;
use serde_json::{Value, json};

/// The findings of `rule` in a release run of the `full` scheme.
fn release_findings_of(m: &Model, rule: &str) -> Vec<Finding> {
    run(&Q::new(m), &FULL, CheckMode::Release)
        .findings
        .into_iter()
        .filter(|f| f.rule == rule)
        .collect()
}

/// `(subject, message)` of each finding, in report order.
fn texts(f: &[Finding]) -> Vec<(String, String)> {
    f.iter().map(|f| (f.subject.clone(), f.message.clone())).collect()
}

fn targets(f: &Finding) -> Vec<(Kind, AnyId)> {
    f.targets.iter().map(|t| (t.kind, t.id)).collect()
}

// ---------------------------------------------------------------------------------------------------------
// Cables

/// A cable `-W<counter>` with ID `n`.
fn cable(fx: &mut Fx, n: u128, counter: u32) {
    fx.push(json!({"$k": "device", "id": us(n), "tag": {"letter": "W", "counter": counter}, "cable": {}}));
}

/// A conductor `n` of cable `cable` with core identification `core` and extra members.
fn conductor(fx: &mut Fx, n: u128, cable: u128, core: &str, extra: &Value) {
    let mut r = json!({"$k": "conductor", "id": us(n), "cable": us(cable), "core": core});
    if let Some(m) = extra.as_object() {
        for (k, v) in m {
            r[k] = v.clone();
        }
    }
    fx.push(r);
}

fn colour_profile() -> Value {
    json!({"colour_profile": {"key": "k", "codes": [{"code": "DBU", "base": "BU", "shade": "dark"}],
        "rows": [{"colour": ["DBU"]}]}})
}

fn colours_fx(settings: Value, c2: Value) -> Fx {
    let mut fx = Fx::new();
    fx.settings = settings;
    cable(&mut fx, 0x6000, 1);
    conductor(
        &mut fx,
        0x6001,
        0x6000,
        "1",
        &json!({"colour": [{"foreign": "BR"}]}),
    );
    conductor(&mut fx, 0x6002, 0x6000, "2", &json!({"colour": c2}));
    conductor(&mut fx, 0x6003, 0x6000, "3", &json!({"colour": ["GN", "YE"]}));
    fx
}

fn colours(settings: Value, c2: Value) -> Model {
    colours_fx(settings, c2).build()
}

/// EW-CORE-031: a foreign code and a company code the profile does not declare are errors; IEC codes and a declared
/// company code are not.
#[test]
fn core031_reports_foreign_and_undeclared_company_colours() {
    let f = findings_of(&colours(Value::Null, json!(["DBU"])), "EW-CORE-031");
    assert_eq!(
        texts(&f),
        [
            (
                "-?W1/1".to_owned(),
                "core -?W1/1 has the colour BR: \"BR\" is not an IEC 60757 code or a company code of the colour profile"
                    .to_owned()
            ),
            (
                "-?W1/2".to_owned(),
                "core -?W1/2 has the colour DBU: \"DBU\" is not an IEC 60757 code or a company code of the colour profile"
                    .to_owned()
            ),
        ]
    );
    assert_eq!(f[0].code, "003031");
    assert_eq!(f[0].severity, Severity::Error);
    assert_eq!(f[0].discriminator, "colour");
    assert_eq!(targets(&f[0]), [(Kind::Conductor, uid(0x6001))]);
    // With the profile declaring DBU only the foreign code is left.
    let f = findings_of(&colours(colour_profile(), json!(["DBU"])), "EW-CORE-031");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].subject, "-?W1/1");
}

/// EW-CORE-031 prevent: a batch that gives a touched core a foreign colour is refused; a foreign colour a lenient
/// import committed, left alone or touched without a colour change, is not.
#[test]
fn core031_prevent_refuses_a_new_invalid_colour_only() {
    let before = colours(Value::Null, json!(["BK"]));
    let after = colours(Value::Null, json!([{"foreign": "XX"}]));
    let key = (Kind::Conductor, uid(0x6002));
    let v = prevent(&Q::new(&before), &Q::new(&after), &[key]);
    assert_eq!(v.len(), 1, "{v:#?}");
    assert_eq!(v[0].rule, "EW-CORE-031");
    assert_eq!(v[0].subject, "-?W1/2");
    // The held "BR" of core 1 is not refused when core 1 is touched without a change.
    let v = prevent(
        &Q::new(&before),
        &Q::new(&before),
        &[(Kind::Conductor, uid(0x6001))],
    );
    assert!(v.is_empty(), "{v:#?}");
}

fn cable_with_part_fx(c1: &Value, extra_core: bool) -> Fx {
    let mut fx = Fx::new();
    fx.part(
        0x7200,
        "CAB-3G1.5",
        &json!({"kind": "cable", "description": "Cable", "sections": {"cable_cores": [
            {"core": "1", "colour": ["BK"], "csa": "1.5"},
            {"core": "2", "colour": ["BU"], "csa": "1.5"},
            {"core": "SH", "shield": true}
        ]}}),
    );
    cable(&mut fx, 0x6000, 1);
    fx.with_parts(0x6000, &[0x7200]);
    conductor(&mut fx, 0x6001, 0x6000, "1", c1);
    conductor(&mut fx, 0x6002, 0x6000, "2", &json!({"colour": ["BU"]}));
    if extra_core {
        conductor(&mut fx, 0x6003, 0x6000, "3", &json!({}));
    }
    fx
}

fn cable_with_part(c1: &Value, extra_core: bool) -> Model {
    cable_with_part_fx(c1, extra_core).build()
}

/// EW-CORE-032: a core whose colour or cross-section differs from the part's core of its identification, and a core
/// the part does not define, are warnings; matching and unset members are not.
#[test]
fn core032_reports_cores_that_differ_from_the_cable_part() {
    let f = findings_of(
        &cable_with_part(&json!({"colour": ["BN"], "csa": "2.5"}), true),
        "EW-CORE-032",
    );
    assert_eq!(
        texts(&f),
        [
            (
                "-?W1/1".to_owned(),
                "core -?W1/1 differs from the core 1 of the cable part CAB-3G1.5: colour BN ≠ BK, cross-section 2.5 ≠ 1.5"
                    .to_owned()
            ),
            (
                "-?W1/3".to_owned(),
                "core -?W1/3 is not a core of the cable part CAB-3G1.5".to_owned()
            ),
        ]
    );
    assert_eq!(f[0].severity, Severity::Warning);
    assert_eq!(f[0].code, "003032");
    assert_eq!(
        targets(&f[0]),
        [(Kind::Conductor, uid(0x6001)), (Kind::Part, uid(0x7200))]
    );
    assert!(findings_of(&cable_with_part(&json!({"colour": ["BK"]}), false), "EW-CORE-032").is_empty());
}

/// An external interruption point `n` (pairing name `name`) at location `+J-E-206`, wired to CP 0 of the function
/// `dev` by connection `conn`; `core` makes the connection a core of cable `-W9`.
fn external(fx: &mut Fx, n: u128, name: &str, dev: u128, conn: u128, core: Option<u128>) {
    fx.ip(n, P1, name, "target");
    let ext = n + 0x100;
    let f = fx
        .records
        .iter_mut()
        .find(|r| r["id"] == json!(us(n)))
        .expect("the IP function");
    f["kind"]["ip"]["external"] = json!(us(ext));
    fx.push(json!({"$k": "external_ref", "id": us(ext), "location": us(0x900), "device": "N1", "connection": name}));
    let extra = core.map_or(Value::Null, |c| json!({"kind": {"core": us(c)}}));
    fx.conn(conn, P1, cp(n), cp(dev), &extra);
}

fn supply_fx(cored: bool) -> Fx {
    let mut fx = Fx::base();
    fx.push(json!({"$k": "structure_node", "id": us(0x900), "aspect": "location", "code": "J-E-206"}));
    fx.func_tagged(0x1001, 1, P1);
    fx.func_tagged(0x1002, 2, P1);
    let core = if cored {
        cable(&mut fx, 0x6000, 9);
        conductor(&mut fx, 0x6001, 0x6000, "1", &json!({}));
        conductor(&mut fx, 0x6002, 0x6000, "2", &json!({}));
        [Some(0x6001), Some(0x6002)]
    } else {
        [None, None]
    };
    external(&mut fx, 0x3001, "L", 0x1001, 0x2001, core[0]);
    external(&mut fx, 0x3002, "N", 0x1002, 0x2002, core[1]);
    fx
}

fn supply(cored: bool) -> Model {
    supply_fx(cored).build()
}

/// EW-CORE-034 (D-19): the wired external points of one location without a cable are one warning naming the
/// location, the inside devices and the wires; cored wires are not reported.
#[test]
fn core034_reports_an_external_connection_without_cable() {
    let f = findings_of(&supply(false), "EW-CORE-034");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "003034");
    assert_eq!(f[0].severity, Severity::Warning);
    assert_eq!(f[0].subject, "+J-E-206 → -1K1, -1K2");
    assert!(
        f[0].message
            .starts_with("the external connection +J-E-206 → -1K1, -1K2 has no cable: wires "),
        "{}",
        f[0].message
    );
    assert_eq!(f[0].page, Some(uid(P1).cast()));
    assert!(targets(&f[0]).contains(&(Kind::StructureNode, uid(0x900))));
    assert!(targets(&f[0]).contains(&(Kind::Function, uid(0x3001))));
    assert!(findings_of(&supply(true), "EW-CORE-034").is_empty());
}

/// A cable with core `1` used by connections `conns` (each between two fresh functions).
fn cores_fx(conns: &[u128]) -> Fx {
    let mut fx = Fx::base();
    cable(&mut fx, 0x6000, 1);
    conductor(&mut fx, 0x6001, 0x6000, "1", &json!({}));
    for (i, c) in conns.iter().enumerate() {
        let a = 0x1001 + 2 * i as u128;
        fx.func(a, P1);
        fx.func(a + 1, P1);
        fx.conn(*c, P1, cp(a), cp(a + 1), &json!({"kind": {"core": us(0x6001)}}));
    }
    fx
}

fn cores(conns: &[u128]) -> Model {
    cores_fx(conns).build()
}

/// EW-CORE-036 (CI-05): a core assigned to two physical wires is an error; one wire is not.
#[test]
fn core036_reports_a_core_assigned_to_two_wires() {
    let f = findings_of(&cores(&[0x2001, 0x2002]), "EW-CORE-036");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "003036");
    assert_eq!(f[0].subject, "-?W1/1");
    assert!(
        f[0].message.starts_with("core -?W1/1 is assigned to 2 wires ("),
        "{}",
        f[0].message
    );
    assert!(targets(&f[0]).contains(&(Kind::Connection, uid(0x2002))));
    assert!(findings_of(&cores(&[0x2001]), "EW-CORE-036").is_empty());
}

/// EW-CORE-036 prevent: assigning the core to a second wire is refused; a batch that leaves an existing double
/// assignment alone is not.
#[test]
fn core036_prevent_refuses_the_second_assignment_only() {
    let before = cores(&[0x2001]);
    let after = cores(&[0x2001, 0x2002]);
    let v = prevent(
        &Q::new(&before),
        &Q::new(&after),
        &[(Kind::Connection, uid(0x2002))],
    );
    assert_eq!(v.len(), 1, "{v:#?}");
    assert_eq!(v[0].rule, "EW-CORE-036");
    let v = prevent(
        &Q::new(&after),
        &Q::new(&after),
        &[(Kind::Connection, uid(0x2001))],
    );
    assert!(v.is_empty(), "{v:#?}");
}

/// EW-CORE-037: a shield no connection assigns, in a cable wired core by core, is a warning (online only); a connected
/// shield, a core and a cable drawn as a whole are not.
#[test]
fn core037_reports_a_shield_connected_at_neither_end() {
    let mut fx = Fx::base();
    cable(&mut fx, 0x6000, 1);
    conductor(&mut fx, 0x6001, 0x6000, "SH", &json!({"shield": true}));
    conductor(&mut fx, 0x6002, 0x6000, "1", &json!({}));
    // A cable none of whose conductors is assigned is drawn as a whole: not judged.
    assert!(
        findings_of(&fx.build(), "EW-CORE-037").is_empty(),
        "single-line cable"
    );
    fx.func(0x1003, P1);
    fx.func(0x1004, P1);
    fx.conn(
        0x2002,
        P1,
        cp(0x1003),
        cp(0x1004),
        &json!({"kind": {"core": us(0x6002)}}),
    );
    let open = fx.clone();
    let f = findings_of(&open.build(), "EW-CORE-037");
    assert_eq!(
        texts(&f),
        [(
            "-?W1/SH".to_owned(),
            "shield -?W1/SH is connected at neither end; connect it at one end or both".to_owned()
        )]
    );
    assert!(
        release_findings_of(&open.build(), "EW-CORE-037").is_empty(),
        "online only"
    );
    fx.func(0x1001, P1);
    fx.func(0x1002, P1);
    fx.conn(
        0x2001,
        P1,
        cp(0x1001),
        cp(0x1002),
        &json!({"kind": {"core": us(0x6001)}}),
    );
    assert!(findings_of(&fx.build(), "EW-CORE-037").is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// Devices and parts

/// EW-CORE-041: a device with functions and no part is a warning; a device with a part, a strip, a cable and a
/// device without functions are not reported.
#[test]
fn core041_reports_a_device_without_part() {
    let mut fx = Fx::base();
    fx.part(0x7100, "P-1", &json!({"description": "Relay"}));
    fx.func_tagged(0x1001, 1, P1);
    fx.func_tagged(0x1002, 2, P1);
    fx.with_parts(0x21002, &[0x7100]);
    fx.device(0x103, "A", Some(1));
    fx.push(json!({"$k": "device", "id": us(0x104), "tag": {"letter": "X", "counter": 1}, "strip": {}}));
    cable(&mut fx, 0x6000, 1);
    let f = findings_of(&fx.build(), "EW-CORE-041");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "102041");
    assert_eq!(f[0].severity, Severity::Warning);
    assert_eq!(f[0].discriminator, "");
    assert_eq!(targets(&f[0]), [(Kind::Device, uid(0x21001))]);
    assert!(
        f[0].message
            .ends_with("has no part; assign one, or mark it as supplied by others")
    );
}

/// EW-CORE-042: a part copy without description or manufacturer is a warning; a complete one is not.
#[test]
fn core042_reports_incomplete_part_data() {
    let mut fx = Fx::base();
    fx.part(0x7100, "P-1", &json!({"description": "Relay"}));
    fx.part(
        0x7101,
        "P-2",
        &json!({"manufacturer": {"library": us(0x7000), "item": us(0x7FFE), "version_req": "*"}}),
    );
    fx.func_tagged(0x1001, 1, P1);
    fx.with_parts(0x21001, &[0x7100, 0x7101]);
    let f = findings_of(&fx.build(), "EW-CORE-042");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "102042");
    assert_eq!(f[0].discriminator, "part-data");
    assert!(
        f[0].message.starts_with("part P-2 of ")
            && f[0].message.ends_with(" has no description, no manufacturer"),
        "{}",
        f[0].message
    );
    assert_eq!(
        targets(&f[0]),
        [(Kind::Device, uid(0x21001)), (Kind::Part, uid(0x7101))]
    );
}

/// Two devices sharing the placeholder part `0x7300` and a real part; the devices' IDs are `ids` (tags K1, K2).
fn placeholders_fx(ids: [u128; 2]) -> Fx {
    let mut fx = Fx::base();
    fx.part(
        0x7300,
        "TBD",
        &json!({"kind": "duct", "description": "Wiring duct"}),
    );
    fx.part(0x7100, "P-1", &json!({"description": "Relay"}));
    for (counter, n) in [(1, ids[0]), (2, ids[1])] {
        fx.drawn_device(n, "K", Some(counter), P1);
        fx.with_parts(n, &[0x7300, 0x7100]);
    }
    fx
}

fn placeholders(ids: [u128; 2]) -> Model {
    placeholders_fx(ids).build()
}

/// EW-CORE-043 (D-04, EF-17): one finding per placeholder copy (part number TBD), a warning online and an error in
/// release mode, naming every device that uses it; a real part is not reported. Permuting the devices' IDs leaves the
/// finding unchanged.
#[test]
fn core043_reports_each_placeholder_part_once() {
    let a = placeholders([0x301, 0x302]);
    let f = findings_of(&a, "EW-CORE-043");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "102043");
    assert_eq!(f[0].severity, Severity::Warning);
    assert_eq!(f[0].subject, "TBD Wiring duct");
    assert_eq!(
        f[0].message,
        "placeholder part \"Wiring duct\" (part number TBD) is used by -1K1, -1K2; choose the part before release"
    );
    assert_eq!(
        targets(&f[0]),
        [
            (Kind::Device, uid(0x301)),
            (Kind::Device, uid(0x302)),
            (Kind::Part, uid(0x7300))
        ]
    );
    let r = release_findings_of(&a, "EW-CORE-043");
    assert_eq!(r[0].severity, Severity::Error);
    // ID permutation: the tags stay, the IDs swap.
    let b = placeholders([0x302, 0x301]);
    let g = findings_of(&b, "EW-CORE-043");
    assert_eq!(texts(&g), texts(&f));
    assert_eq!(g[0].fingerprint, f[0].fingerprint, "keyed on the part copy");
}

/// A PLC module `-A1` with its input start `input` and channel 0's address `address`.
fn module_fx(input: &str, address: &str) -> Fx {
    let mut fx = Fx::new();
    let module = 0x500u128;
    fx.push(
        json!({"$k": "device", "id": us(module), "tag": {"letter": "A", "counter": 1},
        "plc_module": {"start": {"input": input}}}),
    );
    let channel: AnyId = ew_derive::ew_ids::v5::plc_channel_id(uid(module), 0);
    fx.push(
        json!({"$k": "channel", "id": channel.to_string(), "module": us(module), "number": 0, "kind": "di",
        "address": address}),
    );
    fx
}

fn module(input: &str, address: &str) -> Model {
    module_fx(input, address).build()
}

/// EW-CORE-044 (D-15): a `Tbd` module start and a `Tbd` channel address are infos online and errors in release
/// mode; `auto` values are not reported.
#[test]
fn core044_reports_open_tbd_values() {
    let m = module("tbd", "tbd");
    let f = findings_of(&m, "EW-CORE-044");
    assert_eq!(f.len(), 2, "{f:#?}");
    assert!(
        f.iter()
            .all(|f| f.severity == Severity::Info && f.code == "102044")
    );
    let mut discriminators: Vec<&str> = f.iter().map(|f| f.discriminator.as_str()).collect();
    discriminators.sort_unstable();
    assert_eq!(discriminators, ["address", "start.input"]);
    let r = release_findings_of(&m, "EW-CORE-044");
    assert!(r.iter().all(|f| f.severity == Severity::Error), "{r:#?}");
    assert!(findings_of(&module("auto", "auto"), "EW-CORE-044").is_empty());
}

/// A device with the part contacts `part` and contact functions of the kinds `functions`.
fn contacts_fx(part: &[&str], functions: &[&str]) -> Fx {
    let mut fx = Fx::base();
    let defs = [("no", 0x520u128), ("nc", 0x521), ("co", 0x522)];
    for (kind, n) in defs {
        fx.fndef_with(
            n,
            n + 0x10,
            &[(0, "1", Value::Null), (1, "2", Value::Null)],
            &json!([]),
            &json!({"contact_kind": kind}),
        );
    }
    let list: Vec<Value> = part
        .iter()
        .map(|k| json!({"kind": k, "cps": ["1", "2"]}))
        .collect();
    fx.part(
        0x7100,
        "K-1",
        &json!({"description": "Contactor", "contacts": list}),
    );
    fx.device(0x301, "K", Some(1));
    fx.with_parts(0x301, &[0x7100]);
    for (i, k) in functions.iter().enumerate() {
        let def = defs.iter().find(|(d, _)| d == k).expect("a contact kind").1;
        fx.push(
            json!({"$k": "function", "id": us(0x1101 + i as u128), "device": us(0x301), "def": us(def),
            "role": if i == 0 { "main" } else { "aux" }}),
        );
    }
    fx
}

fn contacts(part: &[&str], functions: &[&str]) -> Model {
    contacts_fx(part, functions).build()
}

/// EW-CORE-045: contact functions beyond the part's contacts, or of another kind, are an error; a change-over contact
/// serves a normally closed one; a part without contact data is not judged.
#[test]
fn core045_reports_contacts_beyond_the_part() {
    let f = findings_of(&contacts(&["no", "nc"], &["no", "no"]), "EW-CORE-045");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "102045");
    assert_eq!(f[0].severity, Severity::Error);
    assert!(
        f[0].message
            .ends_with("uses 2 NO, 0 NC, 0 CO contacts; its parts have 1 NO, 1 NC, 0 CO"),
        "{}",
        f[0].message
    );
    assert!(findings_of(&contacts(&["no", "co"], &["no", "nc"]), "EW-CORE-045").is_empty());
    assert!(findings_of(&contacts(&[], &["no", "no", "no"]), "EW-CORE-045").is_empty());
}

/// An assembly `-U1` with children `-U1-Q1` and `-U1-Q2` (both with parts), its order codes covering `covers`.
fn assembly_fx(covers: &[&[u128]], basis: &str) -> Fx {
    let mut fx = Fx::new();
    fx.part(0x7100, "VALVE", &json!({"description": "Valve"}));
    fx.part(
        0x7101,
        "CONF",
        &json!({"kind": "configured", "description": "Valve terminal"}),
    );
    let configs: Vec<Value> = covers
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let ids: Vec<String> = c.iter().map(|n| us(*n)).collect();
            json!({"order_code": format!("OC{i}"), "part": {"copy": us(0x7101)}, "covers": ids})
        })
        .collect();
    fx.push(
        json!({"$k": "device", "id": us(0x100), "tag": {"letter": "U", "counter": 1},
        "assembly": {"configs": configs, "bom_basis": basis}}),
    );
    for (n, counter) in [(0x101u128, 1u32), (0x102, 2)] {
        fx.push(json!({"$k": "device", "id": us(n), "parent": us(0x100),
            "tag": {"letter": "Q", "counter": counter, "scheme": "child"}}));
        fx.with_parts(n, &[0x7100]);
    }
    fx
}

fn assembly(covers: &[&[u128]], basis: &str) -> Model {
    assembly_fx(covers, basis).build()
}

/// EW-CORE-048 (D-36): a child with parts that no order code covers, and one two order codes cover, are warnings;
/// covering each child once, or an assembly listing its components, is not reported.
#[test]
fn core048_reports_children_not_covered_exactly_once() {
    let f = findings_of(&assembly(&[&[0x101], &[0x101]], "assembly"), "EW-CORE-048");
    let mut d: Vec<&str> = f.iter().map(|f| f.discriminator.as_str()).collect();
    d.sort_unstable();
    assert_eq!(d, ["covered-twice", "uncovered"], "{f:#?}");
    assert!(
        f.iter()
            .all(|f| f.code == "102048" && f.severity == Severity::Warning)
    );
    assert!(findings_of(&assembly(&[&[0x100, 0x101, 0x102]], "assembly"), "EW-CORE-048").is_empty());
    assert!(findings_of(&assembly(&[&[0x101]], "components"), "EW-CORE-048").is_empty());
}

/// FT-368, EW-CORE-008 `form`: a page whose frame defaults name form copies the project does not hold is an error in
/// release mode, one finding per page; frame defaults that name nothing are not reported.
#[test]
fn core008_reports_frame_defaults_naming_a_missing_form() {
    let mut fx = Fx::base();
    fx.settings = json!({"frames": {"default": us(0xBAD0), "per_page_type": [
        {"page_type": "multi_line", "form": us(0xBAD1)}
    ]}});
    let m = fx.build();
    let f = release_findings_of(&m, "EW-CORE-008");
    assert_eq!(f.len(), 2, "{f:#?}");
    assert!(
        f.iter()
            .all(|f| f.discriminator == "form" && f.severity == Severity::Error)
    );
    assert_eq!(f[0].subject, "folio 1");
    assert!(
        f[0].message
            .contains("the frame for page type multi_line names form copy"),
        "{}",
        f[0].message
    );
    assert!(
        f[0].message.contains("the default frame names form copy"),
        "{}",
        f[0].message
    );
    assert!(findings_of(&m, "EW-CORE-008").is_empty(), "release mode only");
    assert!(release_findings_of(&Fx::base().build(), "EW-CORE-008").is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// FT-594: the canonical order

/// Three functions with an unconnected CP 1 each, on devices tagged -K10, -K2 and -K1, whose IDs are `ids` in that
/// tag order.
fn unconnected(ids: [u128; 3]) -> Model {
    let mut fx = Fx::base();
    for (n, counter) in ids.iter().zip([10u32, 2, 1]) {
        fx.func_tagged(*n, counter, P1);
    }
    fx.conn(0x2001, P1, cp(ids[0]), cp(ids[1]), &Value::Null);
    fx.conn(0x2002, P1, cp(ids[1]), cp(ids[2]), &Value::Null);
    fx.build()
}

/// FT-594: the findings of one code print in natural order of their subjects, whatever the record IDs: permuting the
/// IDs of the functions gives the same report order (the self-test recordings' `check.txt` no longer churns).
#[test]
fn findings_of_one_code_order_by_printed_subject_under_id_permutation() {
    let order = |m: &Model| -> Vec<(String, String)> {
        run(&Q::new(m), &PRE_DELIVERY, CheckMode::Release)
            .findings
            .iter()
            .filter(|f| f.rule == "EW-POT-009")
            .map(|f| (f.subject.clone(), f.message.clone()))
            .collect()
    };
    let subjects = |v: &[(String, String)]| v.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>();
    let a = order(&unconnected([0x1001, 0x1002, 0x1003]));
    assert_eq!(subjects(&a), ["-1K1:2", "-1K2:2", "-1K10:2"], "natural order");
    for ids in [
        [0x1003, 0x1002, 0x1001],
        [0x1002, 0x1003, 0x1001],
        [0x1001, 0x1003, 0x1002],
    ] {
        assert_eq!(order(&unconnected(ids)), a, "{ids:x?}");
    }
}

// ---------------------------------------------------------------------------------------------------------
// ID permutation (WP-S1.16 §10)

/// One permutation case: the rule, its fixture and the ID pairs to swap.
type Case = (&'static str, Fx, Vec<(u128, u128)>);

/// The fixture with the record and library IDs of each pair swapped everywhere they occur.
fn permuted(fx: &Fx, pairs: &[(u128, u128)]) -> Model {
    let swap = |v: &Value| -> Value {
        let mut text = v.to_string();
        for (i, (a, b)) in pairs.iter().enumerate() {
            let tmp = format!("@swap{i}@");
            text = text
                .replace(&us(*a), &tmp)
                .replace(&us(*b), &us(*a))
                .replace(&tmp, &us(*b));
        }
        serde_json::from_str(&text).expect("JSON")
    };
    let mut out = fx.clone();
    out.records = fx.records.iter().map(swap).collect();
    out.lib = fx.lib.iter().map(swap).collect();
    out.build()
}

/// Every finding of a release and an online run of the `full` scheme as printed: rule, severity, subject, message,
/// in report order.
fn printed(m: &Model) -> Vec<String> {
    let q = Q::new(m);
    [CheckMode::Online, CheckMode::Release]
        .into_iter()
        .flat_map(|mode| run(&q, &FULL, mode).findings)
        .map(|f| format!("{} {} {}: {}", f.rule, f.severity, f.subject, f.message))
        .collect()
}

/// The ID-permutation test of every part-3 rule: swapping record IDs (conductors, connections, devices, functions,
/// part copies, pages) changes no printed finding and no report order, so a fresh project, whose IDs are random,
/// prints the same findings in the same order (ID-677, WP-S1.16 §10, FT-594).
#[test]
fn part3_findings_do_not_follow_record_ids() {
    let cases: Vec<Case> = vec![
        (
            "EW-CORE-031",
            colours_fx(Value::Null, json!(["DBU"])),
            vec![(0x6001, 0x6002), (0x6000, 0x6003)],
        ),
        (
            "EW-CORE-032",
            cable_with_part_fx(&json!({"colour": ["BN"]}), true),
            vec![(0x6001, 0x6003)],
        ),
        (
            "EW-CORE-034",
            supply_fx(false),
            vec![(0x3001, 0x3002), (0x1001, 0x1002), (0x2001, 0x2002)],
        ),
        (
            "EW-CORE-036",
            cores_fx(&[0x2001, 0x2002, 0x2003]),
            vec![(0x2001, 0x2003), (0x1001, 0x1006)],
        ),
        ("EW-CORE-041", supply_fx(false), vec![(0x21001, 0x21002)]),
        (
            "EW-CORE-043",
            placeholders_fx([0x301, 0x302]),
            vec![(0x301, 0x302), (0x7100, 0x7300)],
        ),
        ("EW-CORE-044", module_fx("tbd", "tbd"), vec![]),
        (
            "EW-CORE-045",
            contacts_fx(&["no", "nc"], &["no", "no"]),
            vec![(0x1101, 0x1102)],
        ),
        (
            "EW-CORE-048",
            assembly_fx(&[&[0x101], &[0x101]], "assembly"),
            vec![(0x101, 0x102)],
        ),
    ];
    let mut shield = Fx::base();
    cable(&mut shield, 0x6000, 1);
    conductor(&mut shield, 0x6001, 0x6000, "SH", &json!({"shield": true}));
    conductor(&mut shield, 0x6002, 0x6000, "SH2", &json!({"shield": true}));
    conductor(&mut shield, 0x6003, 0x6000, "1", &json!({}));
    shield.func(0x1001, P1);
    shield.func(0x1002, P1);
    shield.conn(
        0x2001,
        P1,
        cp(0x1001),
        cp(0x1002),
        &json!({"kind": {"core": us(0x6003)}}),
    );
    let mut parts = Fx::base();
    parts.part(0x7100, "P-1", &json!({}));
    parts.part(0x7101, "P-2", &json!({}));
    parts.func_tagged(0x1001, 1, P1);
    parts.with_parts(0x21001, &[0x7100, 0x7101]);
    let mut frames = Fx::base();
    frames.settings = json!({"frames": {"default": us(0xBAD0)}});
    let mut cases = cases;
    cases.push(("EW-CORE-037", shield, vec![(0x6001, 0x6002)]));
    cases.push(("EW-CORE-042", parts, vec![(0x7100, 0x7101)]));
    cases.push(("EW-CORE-008", frames, vec![(0x100, 0x101)]));
    for (rule, fx, pairs) in cases {
        let base = printed(&fx.build());
        assert!(base.iter().any(|l| l.starts_with(rule)), "{rule}: {base:#?}");
        assert_eq!(printed(&permuted(&fx, &pairs)), base, "{rule}");
    }
}
