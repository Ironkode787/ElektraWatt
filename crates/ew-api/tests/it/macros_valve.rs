// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.19 part 5, FT-671 and FT-672: the valve typical of doc 07 §9.3 as macros, and the orientation of an inserted
//! window macro.
//!
//! - **MC-4.** Folio 203 saved through `Session::save_macro` as the page macro "valve-manifold page" with the groups
//!   A, B (`$has_b`) and `field` (`$has_field`) and the value sets 1 to 4 of doc 07 §9.3, inserted four times into
//!   Golden A with folios 203 to 206 and every device they hold removed (in memory; the committed fixture is not
//!   opened), reproduces those folios: devices, channels and their derived addresses Q0.1 … Q6.7, device links,
//!   process tags, the "Spare" texts, the representations of the new pages and the report rows.
//! - **MC-5.** The page macro inserted a second time with value set 1 lists the eight doubled addresses Q0.1, Q0.3 …
//!   Q1.7 on the card and commits as one undo step.
//! - **The window macro** "valve module typical" (one module with its valves and the `field` group) and FT-672: it
//!   inserts at 90° with every representation turned about the anchor; the PSU block of 102 at 90° keeps its wires
//!   on their CPs.
//! - **MC-7** for both: twice-run determinism of the saved documents and the composed commands.

use crate::golden_common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ew_api::library::{LibraryTiers, Registry};
use ew_api::output::TableEnd;
use ew_api::{AnyCommand, InsertAt, MacroConflict, MacroScope, MacroSpec, MacroValues, SavedMacro, Session};
use ew_api::{BomMode, Export, SessionOptions};
use ew_derive::{ChannelAddr, ChannelAddresses, DeviceTag, Q};
use ew_ids::{Id, Uuid};
use ew_mltext::MlText;
use ew_schema::{Device, Endpoint, Function, LibDocument, Page, Record, Representation};
use ew_units::{Orient2, Pt2};
use serde_json::{Value, json};

struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tiers(root: &Path) -> LibraryTiers {
    LibraryTiers {
        system: Vec::new(),
        community: root.join("data").join("packs"),
        company: Vec::new(),
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    }
}

/// A fresh, empty user library.
fn user_library(name: &str) -> (Dir, LibraryTiers, Uuid) {
    let root = Dir(ew_testdir::fresh_dir(name));
    let t = tiers(&root.0);
    let mut reg = Registry::open(&t).unwrap();
    let lib = reg
        .create(&t.user.join("mine.ewlib"), "mine", "Mine", "CC0-1.0")
        .unwrap();
    (root, t, lib)
}

/// Golden A in memory from the golden tables with its colour and size profiles (as the acceptance tests build it),
/// with the libraries of `tiers`.
fn golden(tiers: Option<LibraryTiers>) -> Session {
    let batch = golden_common::batch();
    let mut project = None;
    let mut records = Vec::new();
    for r in &batch.records {
        match r {
            Record::Project(p) => project = Some(p.clone()),
            other => records.push(other.clone()),
        }
    }
    let mut project = project.expect("the batch holds the project");
    golden_common::with_profiles(&mut project);
    let options = SessionOptions {
        libraries: tiers,
        ids: Some(ew_commands::testkit::seeded_ids(38)),
        ..SessionOptions::default()
    };
    Session::from_records(project, records, batch.lib.clone(), options).unwrap()
}

fn run(s: &mut Session, name: &str, params: Value) {
    let cmd = AnyCommand {
        name: name.into(),
        params,
    };
    let preview = s.preview(&cmd).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    s.commit(preview).unwrap();
}

fn commit(s: &mut Session, cmd: &AnyCommand) {
    let preview = s.preview(cmd).unwrap_or_else(|e| panic!("{e:?}"));
    s.commit(preview).unwrap();
}

fn page_of(s: &Session, folio: &str) -> Uuid {
    s.page_tables(folio).expect("a drawn page").page
}

fn device(s: &Session, tag: &str) -> Uuid {
    s.resolve_reference(&format!("tag:{tag}")).expect(tag)
}

fn tag_of(q: &Q, d: Id<Device>) -> String {
    q.get::<DeviceTag>(d)
        .as_ref()
        .as_ref()
        .map_or_else(|| "?".to_owned(), |t| t.full.clone())
}

fn r0(page: Uuid) -> InsertAt {
    InsertAt {
        page,
        at: None,
        orient: Orient2::R0,
    }
}

// ---------------------------------------------------------------------------------------------------------
// The typical of folio 203 (doc 20 §6.2, doc 07 §9.3)

/// The devices folio 203 draws, by group.
struct Typical {
    /// Group A: the module on slot 1 (-201U3), its valves and their field devices.
    a: Group,
    /// Group B: the module on slot 2 (-201U4), its valves and their field devices.
    b: Group,
}

struct Group {
    module: Uuid,
    valves: [Uuid; 4],
    field: [Uuid; 4],
}

impl Typical {
    fn of(s: &Session) -> Typical {
        let group = |m: &str, f: [&str; 4]| Group {
            module: device(s, m),
            valves: [1, 2, 3, 4].map(|i| device(s, &format!("{m}-Q{i}"))),
            field: f.map(|t| device(s, t)),
        };
        Typical {
            a: group("-201U3", ["-203Q1", "-203Q2", "-203Q3", "-203Q4"]),
            b: group("-201U4", ["-203Q5", "-203Q6", "-203Q7", "-203Q8"]),
        }
    }
}

/// The records of `devices` that a macro saved from `page` holds: the devices, all their functions, the
/// representations on `page`, the modules' channels and the device links among `among`.
fn members(s: &Session, devices: &[Uuid], page: Uuid, among: &BTreeSet<Uuid>) -> Vec<Uuid> {
    let q = s.query();
    let set: BTreeSet<Uuid> = devices.iter().copied().collect();
    let mut out: Vec<Uuid> = devices.to_vec();
    for d in devices {
        for f in q.idx().functions_of_device(Id::from_uuid(*d)) {
            out.push(f.as_uuid());
            for r in q.idx().reps_of_function(f) {
                if q.reps().get(r).is_some_and(|r| r.page.as_uuid() == page) {
                    out.push(r.as_uuid());
                }
            }
        }
    }
    for (id, ch) in q.table::<ew_schema::Channel>().iter() {
        if set.contains(&ch.module.as_uuid()) {
            out.push(id.as_uuid());
        }
    }
    for (id, l) in q.table::<ew_schema::DeviceLink>().iter() {
        let (from, to) = (l.from.as_uuid(), l.to.as_uuid());
        if among.contains(&from) && among.contains(&to) && (set.contains(&from) || set.contains(&to)) {
            out.push(id.as_uuid());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn ids(v: &[Uuid]) -> Vec<String> {
    v.iter().map(Uuid::to_string).collect()
}

fn binding(record: Uuid, field: &str) -> Value {
    json!({"record": record.to_string(), "field": field})
}

/// The variables of one module group (`a` or `b`): its position and tag counter, the valves' states and
/// function texts, the field devices' process tags.
fn group_variables(p: &str, g: &Group) -> Vec<Value> {
    let mut v = vec![
        json!({"name": format!("{p}_pos"), "ty": "number", "bindings": [binding(g.module, "device.position")]}),
        json!({"name": format!("{p}_counter"), "ty": "tag_counter",
            "bindings": [binding(g.module, "device.tag.counter")]}),
    ];
    for i in 0..4 {
        v.push(json!({"name": format!("{p}_state{}", i + 1), "ty": "state",
            "bindings": [binding(g.valves[i], "device.state")]}));
        v.push(json!({"name": format!("{p}_fn{}", i + 1), "ty": "text",
            "bindings": [binding(g.valves[i], "device.function_text")]}));
        v.push(json!({"name": format!("{p}_pt{}", i + 1), "ty": "process_tag",
            "bindings": [binding(g.field[i], "device.process_tag")]}));
    }
    v
}

/// The process tags of the field devices of folio 203 (doc 20 §6.2).
const PT_A: [&str; 4] = ["TA05-V-15", "TA05-V-33", "TA05-V-18", "TA05-V-17"];
const PT_B: [&str; 4] = ["TA05-V-36", "TA05-V-38", "TA05-V-16", "TA05-V-37"];

/// One value set: folio's title, the slots' positions and tag counters under -201U1, and per group either the
/// process tags of its field devices (used valves) or "Spare".
fn value_set(name: &str, title: &str, a: (u32, u32), b: Option<(u32, u32)>, field: bool) -> Value {
    let mut values = serde_json::Map::new();
    values.insert("title".into(), json!(title));
    values.insert("has_field".into(), json!(field));
    values.insert("has_b".into(), json!(b.is_some()));
    let mut group = |p: &str, (pos, counter): (u32, u32), pts: [&str; 4]| {
        values.insert(format!("{p}_pos"), json!(pos));
        values.insert(format!("{p}_counter"), json!(counter));
        for (i, pt) in (1..=4).zip(pts) {
            if field {
                values.insert(format!("{p}_state{i}"), json!("used"));
                values.insert(format!("{p}_pt{i}"), json!(pt));
            } else {
                values.insert(format!("{p}_state{i}"), json!("spare"));
                values.insert(format!("{p}_fn{i}"), json!("Spare"));
            }
        }
    };
    group("a", a, PT_A);
    if let Some(b) = b {
        group("b", b, PT_B);
    }
    json!({"name": name, "values": values})
}

/// The page macro "valve-manifold page" of doc 07 §9.3, saved from folio 203.
fn page_spec(s: &Session) -> MacroSpec {
    let t = Typical::of(s);
    let p203 = page_of(s, "203");
    let all: BTreeSet<Uuid> = [&t.a, &t.b]
        .iter()
        .flat_map(|g| std::iter::once(g.module).chain(g.valves).chain(g.field))
        .collect();
    let group_b: Vec<Uuid> = std::iter::once(t.b.module)
        .chain(t.b.valves)
        .chain(t.b.field)
        .collect();
    let field: Vec<Uuid> = t.a.field.iter().chain(&t.b.field).copied().collect();
    let mut variables = vec![
        json!({"name": "assembly", "ty": "device_ref",
            "bindings": [binding(t.a.module, "device.parent"), binding(t.b.module, "device.parent")]}),
        json!({"name": "title", "ty": "text", "bindings": [binding(p203, "page.title")]}),
        json!({"name": "has_b", "ty": "bool", "default": true}),
        json!({"name": "has_field", "ty": "bool", "default": true}),
    ];
    variables.extend(group_variables("a", &t.a));
    variables.extend(group_variables("b", &t.b));
    let groups = json!([
        {"name": "b", "members": ids(&members(s, &group_b, p203, &all)), "include": "has_b"},
        {"name": "field", "members": ids(&members(s, &field, p203, &all)), "include": "has_field"},
    ]);
    let value_sets = json!([
        value_set("1", "Valve Manifold 1", (2, 3), Some((3, 4)), true),
        value_set("2", "Valve Manifold 2", (4, 5), Some((5, 6)), false),
        value_set("3", "Valve Manifold 3", (6, 7), Some((7, 8)), false),
        value_set("4", "Valve Manifold 4", (10, 2), None, false),
    ]);
    MacroSpec {
        key: "valve-manifold-page".into(),
        name: MlText::neutral("Valve-manifold page"),
        description: MlText::neutral(
            "Two valve modules of the manifold with their field devices (doc 07 §9.3)",
        ),
        scope: MacroScope::Pages(vec![p203]),
        origin: None,
        keep_tags: Vec::new(),
        variables: serde_json::from_value(Value::Array(variables)).unwrap(),
        groups: serde_json::from_value(groups).unwrap(),
        value_sets: serde_json::from_value(value_sets).unwrap(),
    }
}

/// The window macro "valve module typical" of doc 07 §9.3: group A of folio 203 (the module's aux symbol, its four
/// valves and the `field` group), with `$assembly`, `$pos`, `$state1 … 4`, `$fn1 … 4` and `$pt1 … 4`.
fn module_spec(s: &Session) -> MacroSpec {
    let t = Typical::of(s);
    let p203 = page_of(s, "203");
    let all: BTreeSet<Uuid> = std::iter::once(t.a.module)
        .chain(t.a.valves)
        .chain(t.a.field)
        .collect();
    let q = s.query();
    let scope: Vec<Uuid> = all
        .iter()
        .flat_map(|d| q.idx().functions_of_device(Id::from_uuid(*d)).collect::<Vec<_>>())
        .flat_map(|f| q.idx().reps_of_function(f).collect::<Vec<_>>())
        .filter(|r| q.reps().get(*r).is_some_and(|r| r.page.as_uuid() == p203))
        .map(|r| r.as_uuid())
        .collect();
    drop(q);
    let mut variables = vec![
        json!({"name": "assembly", "ty": "device_ref", "bindings": [binding(t.a.module, "device.parent")]}),
        json!({"name": "has_field", "ty": "bool", "default": true}),
    ];
    let mut group = group_variables("m", &t.a);
    group.retain(|v| v["name"] != "m_counter");
    variables.extend(group);
    let groups = json!([
        {"name": "field", "members": ids(&members(s, &t.a.field, p203, &all)), "include": "has_field"},
    ]);
    let mut spare = serde_json::Map::new();
    spare.insert("has_field".into(), json!(false));
    for i in 1..=4 {
        spare.insert(format!("m_state{i}"), json!("spare"));
        spare.insert(format!("m_fn{i}"), json!("Spare"));
    }
    MacroSpec {
        key: "valve-module-typical".into(),
        name: MlText::neutral("Valve module typical"),
        description: MlText::neutral("One valve module with four valves (doc 07 §9.3)"),
        scope: MacroScope::Window(scope),
        origin: None,
        keep_tags: Vec::new(),
        variables: serde_json::from_value(Value::Array(variables)).unwrap(),
        groups: serde_json::from_value(groups).unwrap(),
        value_sets: serde_json::from_value(json!([{"name": "spare", "values": spare}])).unwrap(),
    }
}

// ---------------------------------------------------------------------------------------------------------
// MC-4

/// The devices folios 203 to 206 hold: the modules -201U3 … -201U8 and -202U2, their valves, the field devices.
fn typical_devices(s: &Session) -> Vec<Uuid> {
    let q = s.query();
    let mut out = Vec::new();
    for m in [
        "-201U3", "-201U4", "-201U5", "-201U6", "-201U7", "-201U8", "-202U2",
    ] {
        out.push(device(s, m));
        for i in 1..=4 {
            out.push(device(s, &format!("{m}-Q{i}")));
        }
    }
    for i in 1..=8 {
        out.push(device(s, &format!("-203Q{i}")));
    }
    drop(q);
    out
}

/// The modules each value set recreates: (group A, group B).
const SLOTS: [(&str, Option<&str>); 4] = [
    ("-201U3", Some("-201U4")),
    ("-201U5", Some("-201U6")),
    ("-201U7", Some("-201U8")),
    ("-202U2", None),
];

fn instance(set: usize) -> Uuid {
    Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0203 + set as u128)
}

/// What one device is, by its printed tag: the facts MC-4 compares.
#[derive(Clone, PartialEq, Eq, Debug)]
struct DeviceFacts {
    parts: Value,
    parent: Option<String>,
    position: Option<u32>,
    plc_module: Value,
    state: Value,
    function_text: Value,
    process_tag: Value,
    structure: String,
    functions: Vec<String>,
}

fn facts(q: &Q, d: Id<Device>) -> DeviceFacts {
    let dev = q.devices().get(d).expect("a device");
    let v = serde_json::to_value(dev.as_ref()).unwrap();
    let mut functions: Vec<String> = q
        .idx()
        .functions_of_device(d)
        .filter_map(|f| q.functions().get(f))
        .map(|f| format!("{} {:?} {:?}", f.def, f.role, f.kind))
        .collect();
    functions.sort();
    DeviceFacts {
        parts: v["parts"].clone(),
        parent: dev.parent.map(|p| tag_of(q, p)),
        position: dev.position,
        plc_module: v["plc_module"].clone(),
        state: v["state"].clone(),
        function_text: v["function_text"].clone(),
        process_tag: v["process_tag"].clone(),
        // the resolved nodes; where they come from differs by design (doc 07 §3.5: a structure equal to the page's is
        // saved page-relative, so the inserted device inherits what the original stated)
        structure: q
            .get::<ew_derive::StructureOf>(d)
            .slots
            .iter()
            .map(|(aspect, slot)| format!("{aspect:?} {} {:?}", slot.node, slot.codes))
            .collect::<Vec<_>>()
            .join("; "),
        functions,
    }
}

/// A function as its device's tag and its definition.
fn function_name(q: &Q, f: Id<Function>) -> String {
    match q.functions().get(f) {
        Some(func) => format!(
            "{}:{}",
            func.device.map_or_else(|| "-".to_owned(), |d| tag_of(q, d)),
            func.def
        ),
        None => format!("missing {f}"),
    }
}

fn endpoint_name(q: &Q, e: &Endpoint) -> String {
    match e {
        Endpoint::Cp(f, idx) => format!("{}.{idx}", function_name(q, *f)),
        Endpoint::Junction(j) => format!("junction {j}"),
        Endpoint::Unknown(v) => format!("{v}"),
    }
}

/// A module's channels: number, kind, binding and derived address.
fn channels(q: &Q, module: Id<Device>) -> Vec<String> {
    let table = q.table::<ew_schema::Channel>();
    q.get::<ChannelAddresses>(module)
        .iter()
        .map(|e| {
            let ch = table.get(e.channel).expect("a channel");
            let address = match &e.address {
                ChannelAddr::At { text, .. } => text.clone(),
                other => format!("{other:?}"),
            };
            let bound = ch
                .binding
                .as_ref()
                .map_or_else(|| "-".to_owned(), |b| endpoint_name(q, b));
            format!("{} {:?} {bound} {address}", ch.number, ch.kind)
        })
        .collect()
}

/// The device links that name one of `tags`, as `from > to kind`.
fn links(q: &Q, tags: &BTreeSet<String>) -> BTreeSet<String> {
    q.table::<ew_schema::DeviceLink>()
        .iter()
        .map(|(_, l)| (tag_of(q, l.from), tag_of(q, l.to), format!("{:?}", l.kind)))
        .filter(|(a, b, _)| tags.contains(a) || tags.contains(b))
        .map(|(a, b, k)| format!("{a} > {b} {k}"))
        .collect()
}

/// A page's content by folio: each representation as its functions, symbol and placement, each connection by its
/// end CPs.
fn page_content(q: &Q, folio: u32) -> (Vec<String>, Vec<String>) {
    let page = q
        .pages()
        .iter()
        .find(|(_, p)| p.folio.number == folio)
        .map(|(id, _)| id)
        .expect("the folio");
    let mut reps: Vec<String> = q
        .idx()
        .reps_by_page(page)
        .filter_map(|r| q.reps().get(r))
        .map(|r: &std::sync::Arc<Representation>| {
            let mut v = serde_json::to_value(r.as_ref()).unwrap();
            let fns: Vec<String> = r.functions.iter().map(|f| function_name(q, *f)).collect();
            for k in ["id", "functions", "page", "source_ref", "pin_map"] {
                v.as_object_mut().unwrap().remove(k);
            }
            format!("{fns:?} {v}")
        })
        .collect();
    reps.sort();
    let mut wires: Vec<String> = q
        .idx()
        .connections_by_page(page)
        .filter_map(|c| q.connections().get(c))
        .map(|c| format!("{} — {}", endpoint_name(q, &c.a), endpoint_name(q, &c.b)))
        .collect();
    wires.sort();
    (reps, wires)
}

/// The overview check (EW-CORE-046, not implemented yet): the children of `assembly` without a representation on an
/// assembly-overview page, by tag.
fn missing_overview_children(q: &Q, assembly: Id<Device>) -> BTreeSet<String> {
    q.idx()
        .device_children(assembly)
        .filter(|c| {
            !q.idx().functions_of_device(*c).any(|f| {
                q.idx().reps_of_function(f).any(|r| {
                    q.reps()
                        .get(r)
                        .and_then(|r| q.pages().get(r.page))
                        .is_some_and(|p| p.kind == ew_schema::PageType::AssemblyOverview)
                })
            })
        })
        .map(|c| tag_of(q, c))
        .collect()
}

/// The committed report rows (`expected/rows/<block>.csv`) against the rows derived from `s`.
fn row_differences(s: &Session) -> Vec<String> {
    let batch = golden_common::batch();
    let q = golden_common::plain(s);
    let dir = golden_common::repo().join("golden/J-E-2416/expected/rows");
    let mut out = Vec::new();
    for block in ["901", "901.assembly", "903", "904", "905", "906", "907", "909"] {
        let derived = if block == "901.assembly" {
            ew_reports::export::export_csv(&q, Export::Bom(BomMode::Assembly)).unwrap()
        } else {
            golden_common::rows_csv(&q, &golden_common::rows_of(&q, &batch, block))
        };
        let committed = std::fs::read_to_string(dir.join(format!("{block}.csv"))).unwrap();
        for (i, (a, b)) in committed.lines().zip(derived.lines()).enumerate() {
            if a != b {
                out.push(format!(
                    "{block} line {}:\n  committed {a}\n  derived   {b}",
                    i + 1
                ));
            }
        }
        if committed.lines().count() != derived.lines().count() {
            out.push(format!(
                "{block}: {} committed lines, {} derived",
                committed.lines().count(),
                derived.lines().count()
            ));
        }
    }
    out
}

/// MC-4 (FR-MAC-004, FR-MAC-018): the valve-manifold page macro saved from folio 203, inserted with the value sets 1
/// to 4 into Golden A without folios 203 to 206 and their devices, reproduces those folios.
#[test]
fn mc4_the_valve_manifold_macro_reproduces_folios_203_to_206() {
    let (_root, t, lib) = user_library("ew-api-mc4");
    let original = golden(None);
    let mut s = golden(Some(t));
    let saved = s.save_macro(lib, &page_spec(&s)).unwrap();
    assert!(saved.open_ends.is_empty(), "{:?}", saved.open_ends);

    // Golden A without folios 203 to 206 and every device the typical creates, with all their representations (the
    // modules' overview symbols on 201 and 202 among them); the assembly heads -201U1 and -202U1 stay.
    let gone = typical_devices(&s);
    let targets: Vec<Value> = gone
        .iter()
        .map(|d| json!({"kind": "device", "id": d.to_string()}))
        .collect();
    run(&mut s, "edit.delete", json!({ "targets": targets }));
    let pages: Vec<String> = ["203", "204", "205", "206"]
        .iter()
        .map(|f| page_of(&s, f).to_string())
        .collect();
    run(&mut s, "page.delete", json!({ "pages": pages }));
    assert!(s.page_tables("203").is_none());
    let assembly = device(&s, "-201U1");

    // The four insertions, each after the page the previous one created.
    let mut after = page_of(&s, "202");
    for set in 1..=4 {
        let cmd = s
            .insert_macro(
                &saved.reference,
                &r0(after),
                &MacroValues {
                    value_set: Some(set.to_string()),
                    values: [("assembly".to_owned(), json!(assembly.to_string()))].into(),
                },
                instance(set),
            )
            .unwrap();
        let conflicts = s.macro_conflicts(&cmd).unwrap();
        assert!(
            !conflicts
                .iter()
                .any(|c| matches!(c, MacroConflict::Refused { .. } | MacroConflict::Address(_))),
            "value set {set}: {conflicts:?}"
        );
        commit(&mut s, &cmd);
        after = page_of(&s, &(202 + set).to_string());
    }

    // The overview on 201 and 202 is not regenerated by an insert: the overview check (EW-CORE-046) would list
    // exactly the recreated modules. Its regeneration (D-10) is not built; the user's step of placing their overview
    // symbols as before stands in for it.
    let q = s.query();
    let u1 = Id::from_uuid(assembly);
    let missing = missing_overview_children(&q, u1);
    drop(q);
    let qo = original.query();
    assert!(missing_overview_children(&qo, u1).is_empty());
    let mut modules: Vec<(usize, &str, &str)> = Vec::new();
    for (i, (a, b)) in SLOTS.iter().enumerate() {
        modules.push((i + 1, "-201U3", a));
        if let Some(b) = b {
            modules.push((i + 1, "-201U4", b));
        }
    }
    let local = |tag: &str| device(&original, tag);
    let mut expected_missing = BTreeSet::new();
    let q = s.query();
    for (set, from, to) in &modules {
        let new = Uuid::new_v5(&instance(*set), local(from).hyphenated().to_string().as_bytes());
        expected_missing.insert(tag_of(&q, Id::from_uuid(new)));
        let _ = to;
    }
    drop(q);
    assert_eq!(missing, expected_missing);
    for (set, from, to) in &modules {
        // the module's overview function and its representation in the original
        let old_module = Id::<Device>::from_uuid(device(&original, to));
        let (old_fn, rep) = qo
            .idx()
            .functions_of_device(old_module)
            .find_map(|f| {
                qo.idx()
                    .reps_of_function(f)
                    .filter_map(|r| qo.reps().get(r).cloned())
                    .find(|r| {
                        qo.pages()
                            .get(r.page)
                            .is_some_and(|p| p.kind == ew_schema::PageType::AssemblyOverview)
                    })
                    .map(|r| (f, r))
            })
            .expect("an overview symbol");
        let local_fn = qo
            .idx()
            .functions_of_device(Id::from_uuid(local(from)))
            .find(|f| qo.functions().get(*f).unwrap().def == qo.functions().get(old_fn).unwrap().def)
            .expect("the module's overview function");
        let new_fn = Uuid::new_v5(
            &instance(*set),
            local_fn.as_uuid().hyphenated().to_string().as_bytes(),
        );
        let folio = qo.pages().get(rep.page).unwrap().folio.number.to_string();
        let page = page_of(&s, &folio);
        run(
            &mut s,
            "device.place",
            json!({"page": page.to_string(), "symbol": rep.symbol.to_string(), "pos": rep.pos,
                "orient": rep.orient, "rep_type": rep.rep_type,
                "target": {"existing_function": {"function": new_fn.to_string()}}}),
        );
    }
    // The assembly's configuration covers its children by ID; the user maps the order code onto the new children.
    let u1_old = qo.devices().get(u1).unwrap();
    let mut configs = serde_json::to_value(&u1_old.aspects.assembly.as_ref().unwrap().configs).unwrap();
    let q = s.query();
    let by_tag: BTreeMap<String, Uuid> = q.devices().ids().map(|d| (tag_of(&q, d), d.as_uuid())).collect();
    drop(q);
    for c in configs.as_array_mut().unwrap() {
        let covers: BTreeSet<String> = c["covers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| {
                let d: Uuid = d.as_str().unwrap().parse().unwrap();
                let tag = tag_of(&qo, Id::from_uuid(d));
                by_tag
                    .get(&tag)
                    .unwrap_or_else(|| panic!("{tag} is not recreated: {:?}", by_tag.keys()))
                    .to_string()
            })
            .collect();
        c["covers"] = json!(covers);
    }
    run(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [assembly.to_string()], "path": "assembly.configs", "value": configs}),
    );

    // Every recreated device equals the original by its tag.
    let q = s.query();
    let tags: BTreeSet<String> = gone.iter().map(|d| tag_of(&qo, Id::from_uuid(*d))).collect();
    let mut differ = Vec::new();
    for tag in &tags {
        let Some(new) = by_tag_now(&q, tag) else {
            differ.push(format!("{tag}: not recreated"));
            continue;
        };
        let old = by_tag_now(&qo, tag).unwrap();
        let (a, b) = (facts(&qo, old), facts(&q, new));
        if a != b {
            differ.push(format!("{tag}:\n  original {a:?}\n  inserted {b:?}"));
        }
        let (a, b) = (channels(&qo, old), channels(&q, new));
        if a != b {
            differ.push(format!("{tag} channels:\n  original {a:?}\n  inserted {b:?}"));
        }
    }
    assert!(differ.is_empty(), "{}", differ.join("\n"));
    assert_eq!(links(&q, &tags), links(&qo, &tags));
    assert_eq!(q.devices().iter().count(), qo.devices().iter().count());
    let addresses: BTreeSet<String> = tags
        .iter()
        .filter_map(|t| by_tag_now(&q, t))
        .flat_map(|m| channels(&q, m))
        .filter_map(|c| c.split(' ').next_back().map(str::to_owned))
        .filter(|a| a.starts_with('Q'))
        .collect();
    assert!(
        addresses.contains("Q0.1") && addresses.contains("Q6.7"),
        "{addresses:?}"
    );
    for folio in [201, 202, 203, 204, 205, 206] {
        assert_eq!(page_content(&q, folio), page_content(&qo, folio), "folio {folio}");
    }
    drop(q);
    let rows = row_differences(&s);
    assert!(rows.is_empty(), "{}", rows.join("\n"));
}

fn by_tag_now(q: &Q, tag: &str) -> Option<Id<Device>> {
    q.devices().ids().find(|d| tag_of(q, *d) == tag)
}

// ---------------------------------------------------------------------------------------------------------
// MC-5

/// MC-5 (FR-MAC-005): the folio-203 typical inserted a second time with value set 1 doubles slots 1 and 2: the card
/// lists exactly the eight bound addresses Q0.1, Q0.3 … Q1.7, and the commit is one undo step that undoes exactly.
#[test]
fn mc5_a_second_typical_lists_the_doubled_addresses_and_undoes_in_one_step() {
    let (_root, t, lib) = user_library("ew-api-mc5");
    let mut s = golden(Some(t));
    let saved = s.save_macro(lib, &page_spec(&s)).unwrap();
    let before = ew_commands::rows::model_lines(&s.world().model).unwrap();
    let depth = s.history().entries().len();
    let cmd = s
        .insert_macro(
            &saved.reference,
            &r0(page_of(&s, "206")),
            &MacroValues {
                value_set: Some("1".into()),
                values: [("assembly".to_owned(), json!(device(&s, "-201U1").to_string()))].into(),
            },
            instance(5),
        )
        .unwrap();
    let conflicts = s.macro_conflicts(&cmd).unwrap();
    let addresses: Vec<&str> = conflicts
        .iter()
        .filter_map(|c| match c {
            MacroConflict::Address(a) => Some(a.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        addresses,
        ["Q0.1", "Q0.3", "Q0.5", "Q0.7", "Q1.1", "Q1.3", "Q1.5", "Q1.7"],
        "{conflicts:?}"
    );
    assert!(
        !conflicts
            .iter()
            .any(|c| matches!(c, MacroConflict::Refused { .. })),
        "{conflicts:?}"
    );
    commit(&mut s, &cmd);
    assert_eq!(s.history().entries().len(), depth + 1, "one undo step");
    assert!(s.page_tables("207").is_some());
    s.undo(&ew_api::Origin::Gui).unwrap();
    assert_eq!(
        ew_commands::rows::model_lines(&s.world().model).unwrap(),
        before,
        "one undo restores the records"
    );
}

// ---------------------------------------------------------------------------------------------------------
// Value set 4, the window macro and FT-672

/// FR-MAC-018: value set 4 inserts one module and no field devices, and leaves no dangling connection or link.
#[test]
fn value_set_4_inserts_one_module_without_field_devices() {
    let (_root, t, lib) = user_library("ew-api-mc4-set4");
    let mut s = golden(Some(t));
    let saved = s.save_macro(lib, &page_spec(&s)).unwrap();
    let devices = s.query().devices().iter().count();
    let links = s.query().table::<ew_schema::DeviceLink>().iter().count();
    let cmd = s
        .insert_macro(
            &saved.reference,
            &r0(page_of(&s, "206")),
            &MacroValues {
                value_set: Some("4".into()),
                values: [("assembly".to_owned(), json!(device(&s, "-201U1").to_string()))].into(),
            },
            instance(6),
        )
        .unwrap();
    commit(&mut s, &cmd);
    let q = s.query();
    assert_eq!(
        q.devices().iter().count(),
        devices + 5,
        "one module and its four valves"
    );
    assert_eq!(q.table::<ew_schema::DeviceLink>().iter().count(), links);
    let p207 = page_of(&s, "207");
    let t = s.page_tables("207").unwrap();
    assert_eq!(t.reps.len(), 5, "the module's aux symbol and four valves");
    assert!(t.connections.is_empty());
    for (_, c) in q.connections().iter() {
        for e in [&c.a, &c.b] {
            if let Endpoint::Cp(f, _) = e {
                assert!(q.functions().contains(*f), "a dangling connection end");
            }
        }
    }
    let _ = p207;
}

/// The window macro "valve module typical" saves with its variables and inserts on a new fluid page at R0 and at 90°
/// (FT-672): the representations turn about the anchor, the black box's orientation with them.
#[test]
fn the_valve_module_typical_inserts_turned_about_its_anchor() {
    let (_root, t, lib) = user_library("ew-api-valve-module");
    let mut s = golden(Some(t));
    let saved = s.save_macro(lib, &module_spec(&s)).unwrap();
    let doc = s.libraries().resolve(&saved.reference).unwrap();
    let LibDocument::Macro(m) = &doc else {
        panic!("a macro")
    };
    let anchor: Pt2 = serde_json::from_value(m.item.variants[0].fragment["anchor"].clone()).unwrap();
    let p206 = page_of(&s, "206");
    let section = serde_json::to_value(
        s.query()
            .pages()
            .get(Id::<Page>::from_uuid(p206))
            .unwrap()
            .folio
            .section,
    )
    .unwrap();
    run(
        &mut s,
        "page.create",
        json!({"number": 207, "kind": "fluid", "title": "Valve Manifold 5", "section": section}),
    );
    let p207 = page_of(&s, "207");
    let g = ew_units::Grid::QET_10PX;
    let at = Pt2::new(g.at(60).unwrap(), g.at(40).unwrap());
    let assembly = device(&s, "-201U1");
    let insert = |s: &Session, orient: Orient2, set: Option<&str>, n: u128| {
        s.insert_macro(
            &saved.reference,
            &InsertAt {
                page: p207,
                at: Some(at),
                orient,
            },
            &MacroValues {
                value_set: set.map(str::to_owned),
                values: [
                    ("assembly".to_owned(), json!(assembly.to_string())),
                    ("m_pos".to_owned(), json!(12)),
                ]
                .into(),
            },
            Uuid::from_u128(n),
        )
        .unwrap()
    };
    let module = device(&s, "-201U3");
    // At R0 the module and its field group come in; with the spare set the field group is dropped.
    let cmd = insert(&s, Orient2::R0, None, 1);
    commit(&mut s, &cmd);
    assert_eq!(
        s.page_tables("207").unwrap().reps.len(),
        13,
        "aux, 4 valves, 4 × (valve, actuator)"
    );
    s.undo(&ew_api::Origin::Gui).unwrap();
    let cmd = insert(&s, Orient2::R90, Some("spare"), 2);
    assert_eq!(cmd.params["orient"], json!("r90"));
    commit(&mut s, &cmd);
    let q = s.query();
    let originals: BTreeMap<Uuid, std::sync::Arc<Representation>> =
        q.reps().iter().map(|(id, r)| (id.as_uuid(), r.clone())).collect();
    let t207 = s.page_tables("207").unwrap();
    assert_eq!(t207.reps.len(), 5, "the spare set drops the field group");
    let inst = Uuid::from_u128(2);
    let mut turned = 0;
    for (old_id, old) in &originals {
        if old.page.as_uuid() != page_of(&s, "203") {
            continue;
        }
        let new_id = Uuid::new_v5(&inst, old_id.hyphenated().to_string().as_bytes());
        let Some(new) = q.reps().get(Id::from_uuid(new_id)) else {
            continue;
        };
        turned += 1;
        assert_eq!(new.page.as_uuid(), p207);
        assert_eq!(new.orient, old.orient.then(Orient2::R90));
        // index offsets from the anchor turn: (dx, dy) → (−dy, dx)
        let k = |p: Pt2| [g.snap(p.x), g.snap(p.y)];
        let (o, a, n, t) = (k(old.pos), k(anchor), k(new.pos), k(at));
        assert_eq!([n[0] - t[0], n[1] - t[1]], [-(o[1] - a[1]), o[0] - a[0]]);
        assert_eq!(
            new.box_, old.box_,
            "a black box's outline is relative to its representation"
        );
    }
    assert_eq!(turned, 5);
    let new_module = Uuid::new_v5(&inst, module.hyphenated().to_string().as_bytes());
    let d = q.devices().get(Id::from_uuid(new_module)).unwrap();
    assert_eq!(d.position, Some(12));
    assert_eq!(d.parent.map(|p| p.as_uuid()), Some(assembly));
    let channels_of_new = q
        .table::<ew_schema::Channel>()
        .iter()
        .filter(|(_, c)| c.module.as_uuid() == new_module)
        .count();
    assert_eq!(channels_of_new, 8);
}

/// FT-672: the PSU block of folio 102 inserted at 90° on a new folio 105: every representation and every route vertex
/// turns about the anchor, and each wire still ends at its CPs on the inserted functions.
#[test]
fn a_window_macro_at_90_degrees_keeps_its_wires_on_their_cps() {
    let (_root, t, lib) = user_library("ew-api-orient");
    let mut s = golden(Some(t));
    // the folio draws its wires with the scene's router; store those routes so the block carries bends to turn
    for c in s.page_tables("102").unwrap().connections {
        if !c.stored_route && c.polyline.len() > 2 {
            run(
                &mut s,
                "connection.route",
                json!({"connection": c.id.to_string(), "route": c.polyline}),
            );
        }
    }
    let page = s.page_tables("102").unwrap();
    let scope: Vec<Uuid> = page
        .reps
        .iter()
        .filter(|r| r.device.is_some())
        .map(|r| r.id)
        .chain(page.connections.iter().map(|c| c.id))
        .chain(page.junctions.iter().map(|j| j.id))
        .collect();
    let spec = MacroSpec {
        key: "psu-24vdc".into(),
        name: MlText::neutral("PSU 24 V DC"),
        description: MlText::neutral(""),
        scope: MacroScope::Window(scope),
        origin: None,
        keep_tags: Vec::new(),
        variables: Vec::new(),
        groups: Vec::new(),
        value_sets: Vec::new(),
    };
    let saved: SavedMacro = s.save_macro(lib, &spec).unwrap();
    let doc = s.libraries().resolve(&saved.reference).unwrap();
    let LibDocument::Macro(m) = &doc else {
        panic!("a macro")
    };
    let fragment = &m.item.variants[0].fragment;
    let anchor: Pt2 = serde_json::from_value(fragment["anchor"].clone()).unwrap();
    let p102 = page_of(&s, "102");
    let section = serde_json::to_value(
        s.query()
            .pages()
            .get(Id::<Page>::from_uuid(p102))
            .unwrap()
            .folio
            .section,
    )
    .unwrap();
    run(
        &mut s,
        "page.create",
        json!({"number": 105, "title": "Power Supply 5", "section": section}),
    );
    let p105 = page_of(&s, "105");
    let g = ew_units::Grid::QET_10PX;
    let at = Pt2::new(g.at(80).unwrap(), g.at(20).unwrap());
    let inst = Uuid::from_u128(0x105);
    let cmd = s
        .insert_macro(
            &saved.reference,
            &InsertAt {
                page: p105,
                at: Some(at),
                orient: Orient2::R90,
            },
            &MacroValues::default(),
            inst,
        )
        .unwrap();
    commit(&mut s, &cmd);
    let q = s.query();
    let k = |p: Pt2| [g.snap(p.x), g.snap(p.y)];
    let turn = |p: Pt2| {
        let (o, a, t) = (k(p), k(anchor), k(at));
        [t[0] - (o[1] - a[1]), t[1] + (o[0] - a[0])]
    };
    let new_of = |old: Uuid| Uuid::new_v5(&inst, old.hyphenated().to_string().as_bytes());
    let records: Vec<Value> = fragment["records"].as_array().unwrap().clone();
    // the junction normalisers may merge or split the inserted wires, so the wires are compared by what they
    // connect and where they run, not by ID
    let mut old_links: Vec<(String, String)> = Vec::new();
    let mut functions = BTreeSet::new();
    let mut turned_points = BTreeSet::new();
    for r in &records {
        let id: Uuid = r["id"].as_str().unwrap().parse().unwrap();
        match r["$k"].as_str().unwrap() {
            "representation" => {
                let old: Representation = serde_json::from_value(r.clone()).unwrap();
                let new = q
                    .reps()
                    .get(Id::from_uuid(new_of(id)))
                    .expect("the representation");
                assert_eq!(k(new.pos), turn(old.pos));
                assert_eq!(new.orient, old.orient.then(Orient2::R90));
            }
            "function" => {
                functions.insert(new_of(id));
            }
            "connection" => {
                let old: ew_schema::Connection = serde_json::from_value(r.clone()).unwrap();
                let node = |e: &Endpoint| match e {
                    Endpoint::Cp(f, i) => format!("cp:{}:{i}", new_of(f.as_uuid())),
                    Endpoint::Junction(j) => format!("j:{}", j.as_uuid()),
                    other => panic!("{other:?}"),
                };
                old_links.push((node(&old.a), node(&old.b)));
                for p in &old.route {
                    if ew_commands::geometry::point_on_grid(g, *p) {
                        turned_points.insert(turn(*p));
                    }
                }
            }
            "junction" => {
                let at: Pt2 = serde_json::from_value(r["at"].clone()).unwrap();
                turned_points.insert(turn(at));
            }
            _ => {}
        }
    }
    assert!(!old_links.is_empty());
    let t105 = s.page_tables("105").unwrap();
    let mut new_links = Vec::new();
    for c in &t105.connections {
        let mut ends = Vec::new();
        for (end, at_end) in [(&c.a, c.polyline.first()), (&c.b, c.polyline.last())] {
            let at_end = *at_end.expect("a drawn wire");
            match end {
                TableEnd::Cp { function, idx } => {
                    assert!(
                        functions.contains(function),
                        "{}: an end on an inserted function",
                        c.id
                    );
                    let cp = t105
                        .cps
                        .iter()
                        .find(|p| p.function == *function && p.idx == *idx)
                        .expect("the CP is drawn on 105");
                    assert_eq!(cp.at, at_end, "{}: a route end on its CP", c.id);
                    ends.push(format!("cp:{function}:{idx}"));
                }
                TableEnd::Junction { id } => {
                    let j = t105
                        .junctions
                        .iter()
                        .find(|j| j.id == *id)
                        .expect("the junction is on 105");
                    assert_eq!(j.at, at_end, "{}: a route end on its junction", c.id);
                    ends.push(format!("j:{id}"));
                }
            }
        }
        // every bend of a stored route is a turned bend, CP or junction of the saved block (a wire without a route
        // is drawn by the scene's router)
        for &(x, y) in c.polyline.iter().filter(|_| c.stored_route) {
            let p = Pt2::new(ew_units::Nm(x), ew_units::Nm(y));
            if ew_commands::geometry::point_on_grid(g, p) {
                let cp_or_junction = t105.cps.iter().any(|cp| cp.at == (x, y))
                    || t105.junctions.iter().any(|j| j.at == (x, y));
                assert!(
                    cp_or_junction || turned_points.contains(&k(p)),
                    "{}: a stray bend {:?}",
                    c.id,
                    k(p)
                );
            }
        }
        new_links.push((ends[0].clone(), ends[1].clone()));
    }
    assert!(
        t105.connections
            .iter()
            .any(|c| c.stored_route && c.polyline.len() > 2),
        "a turned route with bends"
    );
    // intact: the CPs fall into the same nets as on the saved block
    assert_eq!(cp_nets(&old_links), cp_nets(&new_links));
}

/// The CP nodes (`cp:…`) of a set of links, grouped by the nets the links join (junctions count as nodes but are
/// left out of the result).
fn cp_nets(links: &[(String, String)]) -> BTreeSet<BTreeSet<String>> {
    let mut parent: BTreeMap<String, String> = BTreeMap::new();
    fn find(parent: &mut BTreeMap<String, String>, n: &str) -> String {
        let p = parent
            .entry(n.to_string())
            .or_insert_with(|| n.to_string())
            .clone();
        if p == n {
            return p;
        }
        let root = find(parent, &p);
        parent.insert(n.to_string(), root.clone());
        root
    }
    for (a, b) in links {
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        if ra != rb {
            parent.insert(ra, rb);
        }
    }
    let nodes: Vec<String> = parent.keys().cloned().collect();
    let mut nets: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for n in nodes {
        if n.starts_with("cp:") {
            let r = find(&mut parent, &n);
            nets.entry(r).or_default().insert(n);
        }
    }
    nets.into_values().collect()
}

// ---------------------------------------------------------------------------------------------------------
// MC-7 and the old logs

fn saved_twice(name: &str, spec: fn(&Session) -> MacroSpec, orient: Orient2) -> (Value, AnyCommand) {
    let (_root, t, lib) = user_library(name);
    let s = golden(Some(t));
    let mut s = s;
    let saved = s.save_macro(lib, &spec(&s)).unwrap();
    let doc = serde_json::to_value(s.libraries().resolve(&saved.reference).unwrap()).unwrap();
    let kind = doc["kind"].as_str().unwrap().to_owned();
    let at = (kind == "window").then(|| {
        let g = ew_units::Grid::QET_10PX;
        Pt2::new(g.at(60).unwrap(), g.at(40).unwrap())
    });
    let cmd = s
        .insert_macro(
            &saved.reference,
            &InsertAt {
                page: page_of(&s, "206"),
                at,
                orient,
            },
            &MacroValues {
                value_set: None,
                values: [("assembly".to_owned(), json!(device(&s, "-201U1").to_string()))].into(),
            },
            instance(9),
        )
        .unwrap();
    (doc, cmd)
}

/// MC-7: two runs save equal documents and compose equal commands, for the page macro and for the window macro at 90°.
#[test]
fn mc7_the_typicals_save_and_insert_deterministically() {
    let strip = |mut v: Value| {
        for k in ["id", "library_id", "item_id", "sha256"] {
            v.as_object_mut().unwrap().remove(k);
        }
        v
    };
    let params = |c: &AnyCommand| {
        let mut p = c.params.clone();
        p.as_object_mut().unwrap().remove("copies");
        p.as_object_mut().unwrap().remove("macro");
        p
    };
    for (spec, orient) in [
        (page_spec as fn(&Session) -> MacroSpec, Orient2::R0),
        (module_spec, Orient2::R90),
    ] {
        let (doc_a, cmd_a) = saved_twice("ew-api-mc7-valve-a", spec, orient);
        let (doc_b, cmd_b) = saved_twice("ew-api-mc7-valve-b", spec, orient);
        assert_eq!(strip(doc_a), strip(doc_b));
        assert_eq!(params(&cmd_a), params(&cmd_b));
    }
}

/// A command written before FT-672 (no `orient`) reads, writes back unchanged and inserts as before; a page macro
/// with an orientation is refused at `/orient`.
#[test]
fn a_log_without_orient_replays_unchanged_and_a_page_macro_takes_no_orient() {
    let (_root, t, lib) = user_library("ew-api-orient-old");
    let mut s = golden(Some(t));
    let saved = s.save_macro(lib, &page_spec(&s)).unwrap();
    let cmd = s
        .insert_macro(
            &saved.reference,
            &r0(page_of(&s, "206")),
            &MacroValues {
                value_set: Some("4".into()),
                values: [("assembly".to_owned(), json!(device(&s, "-201U1").to_string()))].into(),
            },
            instance(7),
        )
        .unwrap();
    assert!(cmd.params.get("orient").is_none(), "r0 is not written");
    let typed: ew_commands::commands::InsertMacro = serde_json::from_value(cmd.params.clone()).unwrap();
    assert_eq!(typed.orient, Orient2::R0);
    assert_eq!(serde_json::to_value(&typed).unwrap(), cmd.params);
    // The records of the replay equal those of the session that composed it.
    let line = json!({"cmd": cmd.name, "params": cmd.params});
    let text = format!("{{\"$schema\":\"ew.cmdlog/1\",\"label\":\"old\"}}\n{line}\n");
    let mut replay = golden(None);
    replay
        .run_script(&ew_api::CmdLog::parse(&text).unwrap(), "old.ewcmd")
        .unwrap();
    commit(&mut s, &cmd);
    assert_eq!(
        ew_commands::rows::model_lines(&replay.world().model).unwrap(),
        ew_commands::rows::model_lines(&s.world().model).unwrap()
    );
    s.undo(&ew_api::Origin::Gui).unwrap();
    let mut turned = cmd.clone();
    turned.params["orient"] = json!("r90");
    match s.preview(&turned).unwrap_err() {
        ew_api::CmdError::InvalidParams { path, .. } => assert_eq!(path, "/orient"),
        other => panic!("{other:?}"),
    }
}
