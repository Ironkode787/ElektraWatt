// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The sample project's catalogue parts: the generator of the system library
//! `content/libraries/ew-sample-parts.ewlib` from Golden A's part and manufacturer copies, read as bytes (Golden A is
//! only read, and keeps its project-local copies unchanged).
//!
//! [`parts`] takes Golden A's 29 part copies less the four `TBD` test parts and the two configured order codes (one
//! machine's configurations, not catalogue items), with their manufacturers. Each part keeps Golden A's data (its
//! dimensions, CP pattern, function template, sections) and gains:
//! - a function template and a CP pattern where Golden A's copy has none and the part's first device by tag has
//!   functions on starter function definitions: those functions, the main one first, and the CP designations they
//!   print (the function definition's, or the function's alias); a decoration function (`ew.fn.graphic`, a cable's
//!   drawing) is no function of the part;
//! - the bus port the device's function holds (195740's DP slave port), as a bus section;
//! - a function category where Golden A's copy has none and the part is one a black box places (2904602, MICO
//!   4.10),
//!   so that the box takes the category's letter;
//! - `meta.confidence` per section: `test` where doc 20 gives the value as a test input (the dimensions, ▲U-26; the
//!   solenoid-position map H1 of 533361, ▲U-23; the FE pin of 550208, ▲U-18), `unverified` for the rest. No section
//!   is `verified`: nobody checked a value against the maker's data.
//!
//! `EW_BLESS=1 cargo nextest run -p ew-library --test it -E 'test(/^sample_parts::/)'` rewrites the library (run it
//! twice: the second run changes nothing).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ew_format::ew_schema::LibDocument;
use ew_ids::Uuid;
use ew_library::{LibManifest, LibraryTiers, Registry, Tier, starter};
use serde_json::{Value, json};

const KEY: &str = "ew-sample-parts";
const NAME: &str = "Sample project parts";
const LICENCE: &str = "CC0-1.0";
/// The version of every item that has not changed since the first published content.
const VERSION: &str = "0.1.0";
/// The library's version: 0.1.1 since the supply 2904602 names the starter FnDef that declares its + and - as source
/// outputs (FT-682).
const LIBRARY_VERSION: &str = "0.1.1";

/// The version of part `key`: the next one for a part changed after its first publication.
fn part_version(key: &str) -> &'static str {
    if key == "2904602" { "0.1.1" } else { VERSION }
}
const ZERO: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

/// The ID of the library: `v5(SYSTEM_LIBRARY, "ew-sample-parts")`, as the starter parts'.
fn library() -> Uuid {
    ew_ids::v5::imported_id::<()>(starter::LIBRARY, KEY).as_uuid()
}

/// The item ID of a sample-parts item: `v5(library, "<kind>:<key>")`.
fn item_id(kind: &str, key: &str) -> Uuid {
    ew_ids::v5::imported_id::<()>(library(), &format!("{kind}:{key}")).as_uuid()
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn golden() -> PathBuf {
    repo().join("golden/J-E-2416.ew")
}

fn shipped_dir() -> PathBuf {
    repo().join("content/libraries/ew-sample-parts.ewlib")
}

/// Every `.json` document of a Golden A `lib/` directory, by its `id`.
fn golden_lib(dir: &str) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for e in std::fs::read_dir(golden().join("lib").join(dir)).unwrap() {
        let path = e.unwrap().path();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        out.insert(v["id"].as_str().unwrap().to_owned(), v);
    }
    out
}

/// Golden A's device and function records.
fn golden_devices() -> Vec<Value> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(golden().join("devices")).unwrap() {
        let text = std::fs::read_to_string(e.unwrap().path()).unwrap();
        out.extend(
            text.lines()
                .filter(|l| !l.is_empty())
                .map(|l| serde_json::from_str::<Value>(l).unwrap()),
        );
    }
    out
}

/// The record's `source_ref` key (`devices|-101F1`, `functions|-101F1`).
fn source_key(r: &Value) -> &str {
    r["source_ref"]["key"].as_str().unwrap_or_default()
}

/// Whether a Golden A part copy is a catalogue part: not a `TBD` test part, not a configured order code.
fn is_catalogue(p: &Value) -> bool {
    p["part_number"] != "TBD" && p["kind"] != "configured"
}

/// The reference to starter FnDef `key` at the starter's version, with its key for the reader.
fn fndef_ref(key: &str) -> Value {
    let doc = LibDocument::from_json(starter::item(key).unwrap_or_else(|| panic!("{key}")).json).unwrap();
    let LibDocument::Fndef(c) = doc else {
        panic!("{key} is not a FnDef")
    };
    let minor = c.version.split('.').take(2).collect::<Vec<_>>().join(".");
    json!({
        "library": starter::LIBRARY.to_string(),
        "item": c.item_id.as_uuid().to_string(),
        "version_req": format!("^{minor}"),
        "key": key,
    })
}

/// The CP designations of starter FnDef `key`, in index order.
fn fndef_cps(key: &str) -> Vec<String> {
    let v: Value = serde_json::from_str(starter::item(key).unwrap().json).unwrap();
    v["cps"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| c["designation"].as_str().unwrap().to_owned())
        .collect()
}

/// What a part copy without its own template gains from its device in Golden A (D3, D4).
struct Derived {
    /// The function template.
    template: Value,
    /// The CP pattern.
    pattern: Value,
    /// The source of each function.
    sources: Vec<String>,
    /// The bus section: the DP port a port function holds, without the project's address and nets.
    bus: Option<Value>,
}

/// The derived function template and CP pattern of a part copy without its own (D3): the functions on starter FnDefs
/// of its first device by tag, less the ones its sibling parts' templates give, the main one first; with the source
/// of each function and the bus port of a port function (D4).
fn derived_template(
    part: &Value,
    copies: &BTreeMap<String, Value>,
    fndefs: &BTreeMap<String, Value>,
    records: &[Value],
) -> Option<Derived> {
    let id = part["id"].as_str().unwrap();
    let mut devices: Vec<&Value> = records
        .iter()
        .filter(|r| r["$k"] == "device")
        .filter(|d| {
            d["parts"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|u| u["part"]["copy"] == id)
        })
        .collect();
    devices.sort_by_key(|d| source_key(d));
    let device = devices.first()?;
    let mut functions: Vec<(&Value, String)> = records
        .iter()
        .filter(|r| r["$k"] == "function" && r["device"] == device["id"])
        .filter_map(|f| {
            let def = fndefs.get(f["def"].as_str()?)?;
            (def["library_id"] == starter::LIBRARY.to_string())
                .then(|| (f, def["meta"]["key"].as_str().unwrap().to_owned()))
        })
        .filter(|(_, key)| key != "ew.fn.graphic")
        .collect();
    // The functions a sibling part use's own template gives stay that part's (195740 beside 550208's black box).
    for u in device["parts"].as_array().unwrap() {
        let Some(sibling) = copies.get(u["part"]["copy"].as_str().unwrap()) else {
            continue;
        };
        if sibling["id"] == id {
            continue;
        }
        for t in sibling["function_template"].as_array().into_iter().flatten() {
            if let Some(i) = functions
                .iter()
                .position(|(_, k)| t["fndef"]["key"] == k.as_str())
            {
                functions.remove(i);
            }
        }
    }
    if functions.is_empty() {
        return None;
    }
    // The main function first (no `role`), then by source.
    functions.sort_by_key(|(f, _)| (f.get("role").is_some(), source_key(f).to_owned()));
    let mut pattern = Vec::new();
    let mut template = Vec::new();
    let mut sources = Vec::new();
    for (n, (f, key)) in functions.iter().enumerate() {
        let mut cps = Vec::new();
        for (i, des) in fndef_cps(key).into_iter().enumerate() {
            let alias = f["cp_overrides"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|o| o["idx"] == i)
                .and_then(|o| o["alias"].as_str());
            let idx = pattern.len();
            pattern.push(json!({"idx": idx, "designation": alias.map_or(des, str::to_owned)}));
            cps.push(idx);
        }
        let mut t = json!({"fndef": fndef_ref(key)});
        if n == 0 {
            t["main"] = json!(true);
        }
        if !cps.is_empty() {
            t["cps"] = json!(cps);
        }
        template.push(t);
        sources.push(format!("golden.table:{}", source_key(f)));
    }
    let ports: Vec<Value> = functions
        .iter()
        .filter(|(_, key)| key == "ew.fn.dp-port")
        .filter_map(|(f, _)| f.get("bus_port"))
        .map(|p| json!({"interface": p["interface"], "bus": p["bus"], "role": p["role"]}))
        .collect();
    Some(Derived {
        template: json!(template),
        pattern: json!(pattern),
        sources,
        bus: (!ports.is_empty()).then(|| json!({ "ports": ports })),
    })
}

/// The manufacturer documents of the library, by Golden A's manufacturer item ID.
fn manufacturers(parts: &[&Value]) -> BTreeMap<String, Value> {
    let lib = library();
    let mut out = BTreeMap::new();
    for m in golden_lib("manufacturers").values() {
        let golden_item = m["item_id"].as_str().unwrap();
        if !parts.iter().any(|p| p["manufacturer"]["item"] == golden_item) {
            continue;
        }
        let key = m["key"].as_str().unwrap();
        let item = item_id("manufacturer", key);
        out.insert(
            golden_item.to_owned(),
            json!({
                "$k": "manufacturer", "id": item.to_string(), "library_id": lib.to_string(),
                "item_id": item.to_string(), "version": VERSION, "sha256": ZERO,
                "meta": {"licence": LICENCE, "sources": m["meta"]["sources"]},
                "key": key, "name": m["name"],
            }),
        );
    }
    out
}

/// The library's documents: the manufacturers, then the parts, each in key order.
fn documents() -> Vec<LibDocument> {
    let lib = library();
    let copies = golden_lib("parts");
    let fndefs = golden_lib("fndefs");
    let records = golden_devices();
    let mut catalogue: Vec<&Value> = copies.values().filter(|p| is_catalogue(p)).collect();
    catalogue.sort_by_key(|p| p["meta"]["key"].as_str().unwrap().to_owned());
    let makers = manufacturers(&catalogue);
    let mut docs: Vec<LibDocument> = makers
        .values()
        .map(|m| LibDocument::from_value(m.clone()).unwrap())
        .collect();
    for p in &catalogue {
        let key = p["meta"]["key"].as_str().unwrap();
        let item = item_id("part", key);
        let mut v = (*p).clone();
        // A part that Golden A's copy leaves without a function category gains one, so that a black box
        // placed from it takes the category's letter (the reference drawing's -102G1 and -102F2).
        if v.get("category").is_none()
            && let Some(category) = category_of(key)
        {
            v["category"] = category;
        }
        let o = v.as_object_mut().unwrap();
        o.insert("id".into(), json!(item.to_string()));
        o.insert("library_id".into(), json!(lib.to_string()));
        o.insert("item_id".into(), json!(item.to_string()));
        o.insert("version".into(), json!(part_version(key)));
        o.insert("sha256".into(), json!(ZERO));
        let maker = &makers[p["manufacturer"]["item"].as_str().unwrap()];
        v["manufacturer"] =
            json!({"library": lib.to_string(), "item": maker["item_id"], "version_req": "^0.1"});
        if v["sections"]["enclosure"].get("plate").is_some() {
            let plate = copies
                .values()
                .find(|c| c["item_id"] == v["sections"]["enclosure"]["plate"]["item"])
                .unwrap();
            let plate_key = plate["meta"]["key"].as_str().unwrap();
            v["sections"]["enclosure"]["plate"] = json!({
                "library": lib.to_string(), "item": item_id("part", plate_key).to_string(), "version_req": "^0.1",
            });
        }
        let mut sources: Vec<Value> = p["meta"]["sources"].as_array().unwrap().clone();
        if p.get("function_template").is_none()
            && let Some(d) = derived_template(p, &copies, &fndefs, &records)
        {
            if let Some(bus) = d.bus {
                v["sections"]["bus"] = bus;
            }
            if d.pattern.as_array().is_some_and(|p| !p.is_empty()) {
                v["cp_pattern"] = d.pattern;
            }
            v["function_template"] = d.template;
            sources.extend(d.sources.into_iter().map(Value::from));
        }
        // A device made from a part needs exactly one main entry: Golden A's 550208 shares its device with 195740,
        // whose function is the main one there, so as a part of its own its one entry is its main function.
        if let Some(t) = v.get_mut("function_template").and_then(Value::as_array_mut)
            && !t.iter().any(|t| t["main"] == true)
        {
            t[0]["main"] = json!(true);
        }
        let confidence = confidence(&v);
        v["meta"] = json!({"key": key, "licence": LICENCE, "sources": sources, "confidence": confidence});
        docs.push(LibDocument::from_value(v).unwrap_or_else(|e| panic!("{key}: {e}")));
    }
    docs
}

/// The function category a catalogue part gains: 2904602 is a power supply (`PowerSupplyAcDc`, G in
/// EN 81346-2:2009), MICO 4.10 an electronic load distribution that monitors and protects its channels
/// (`ProtectionOvercurrent`, F).
fn category_of(key: &str) -> Option<Value> {
    match key {
        "2904602" => Some(json!(["PowerSupplyAcDc", "PowerSupply", "AcDc"])),
        "MICO 4.10" => Some(json!([
            "ProtectionOvercurrent",
            "ElectronicCircuitBreaker",
            "4Channel"
        ])),
        _ => None,
    }
}

/// The confidence of each section of a part document (D4).
fn confidence(v: &Value) -> Value {
    let mech = &v["mech"];
    let dims = ["width", "height", "depth"].iter().any(|m| mech.get(m).is_some());
    let fe_pin = v["cp_pattern"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|cp| cp["potential_role"] == "fe");
    let mut c = serde_json::Map::new();
    let mark = |test: bool| json!(if test { "test" } else { "unverified" });
    c.insert("part".into(), mark(false));
    if v.get("mech").is_some() {
        c.insert("mech".into(), mark(dims));
    }
    if v.get("cp_pattern").is_some() {
        c.insert("cp_pattern".into(), mark(fe_pin));
    }
    if v.get("function_template").is_some() {
        c.insert("function_template".into(), mark(false));
    }
    for (name, _) in v["sections"].as_object().into_iter().flatten() {
        let test = matches!(name.as_str(), "plc" | "enclosure" | "mounting_plate");
        c.insert(name.clone(), mark(test));
    }
    Value::Object(c)
}

// The test boundary: `EW_BLESS=1` asks the generator to rewrite the shipped files.
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    // The bless switch of a test: it chooses whether the test rewrites the shipped files, never a result.
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

/// Every file of a library directory by its relative path (`/`-separated), with `\r\n` read as `\n`.
fn files_of(dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![(dir.to_path_buf(), String::new())];
    while let Some((at, rel)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let r = if rel.is_empty() {
                name
            } else {
                format!("{rel}/{name}")
            };
            if e.file_type().unwrap().is_dir() {
                stack.push((e.path(), r));
            } else {
                out.insert(
                    r,
                    std::fs::read_to_string(e.path()).unwrap().replace("\r\n", "\n"),
                );
            }
        }
    }
    out
}

fn tiers(root: &Path, system: Vec<PathBuf>, company: Vec<PathBuf>) -> LibraryTiers {
    LibraryTiers {
        system,
        community: root.join("data").join("packs"),
        company,
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    }
}

/// SC-3: the generator, written by the library writer into a fresh library of the library's ID, equals the shipped
/// directory (`EW_BLESS=1` rewrites it).
#[test]
fn the_shipped_library_is_the_generated_one() {
    let root = ew_testdir::fresh_dir("ew-library-sample-parts");
    let dir = root.join("company").join("ew-sample-parts.ewlib");
    std::fs::create_dir_all(&dir).unwrap();
    let mut manifest = LibManifest::new(library(), KEY, NAME, LICENCE);
    manifest.version = LIBRARY_VERSION.to_owned();
    std::fs::write(dir.join("library.json"), manifest.to_text()).unwrap();
    let mut reg = Registry::open(&tiers(&root, Vec::new(), vec![root.join("company")])).unwrap();
    reg.write(library(), &documents()).unwrap();
    drop(reg);
    let generated = files_of(&dir);
    let _ = std::fs::remove_dir_all(&root);
    let shipped = shipped_dir();
    if bless() {
        for (p, _) in files_of(&shipped) {
            std::fs::remove_file(shipped.join(&p)).unwrap();
        }
        for (p, text) in &generated {
            let path = shipped.join(p);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
    }
    assert_eq!(
        files_of(&shipped),
        generated,
        "content/libraries/ew-sample-parts.ewlib is not the generated library (EW_BLESS=1 rewrites it)"
    );
}

/// The records of the shipped library, by part number (parts) and key (manufacturers).
fn shipped_records() -> (BTreeMap<String, Value>, BTreeMap<String, Value>) {
    let (mut parts, mut makers) = (BTreeMap::new(), BTreeMap::new());
    for (path, text) in files_of(&shipped_dir()) {
        for line in text.lines().filter(|l| !l.is_empty() && path != "library.json") {
            let v: Value = serde_json::from_str(line).unwrap();
            if path.starts_with("parts/") {
                parts.insert(v["part_number"].as_str().unwrap().to_owned(), v);
            } else {
                makers.insert(v["key"].as_str().unwrap().to_owned(), v);
            }
        }
    }
    (parts, makers)
}

/// SC-3: Golden A's catalogue parts, none of them a `TBD` test part or an order code, with their manufacturers, and
/// every section's confidence `test` or `unverified`.
#[test]
fn the_library_holds_golden_as_catalogue_parts_and_their_makers() {
    let copies = golden_lib("parts");
    let mut expected: Vec<String> = copies
        .values()
        .filter(|p| is_catalogue(p))
        .map(|p| p["part_number"].as_str().unwrap().to_owned())
        .collect();
    expected.sort();
    let (parts, makers) = shipped_records();
    assert_eq!(parts.keys().cloned().collect::<Vec<_>>(), expected);
    assert!(!expected.is_empty());
    assert!(!parts.contains_key("TBD"));
    assert!(parts.values().all(|p| p["kind"] != "configured"));
    for p in parts.values() {
        let n = p["part_number"].as_str().unwrap();
        let maker = p["manufacturer"]["item"].as_str().unwrap();
        assert!(makers.values().any(|m| m["id"] == maker), "{n}: its manufacturer");
        let marks = p["meta"]["confidence"].as_object().unwrap();
        assert!(!marks.is_empty(), "{n}");
        for (section, c) in marks {
            assert!(c == "test" || c == "unverified", "{n}: {section} is {c}");
        }
        assert_eq!(p["meta"]["licence"], LICENCE, "{n}");
    }
    // Every manufacturer is one a part names.
    for m in makers.values() {
        assert!(
            parts.values().any(|p| p["manufacturer"]["item"] == m["id"]),
            "{}",
            m["key"]
        );
    }
}

/// D3, D4: the derived data, whole, for the parts doc 07 §9.2 and doc 20 name: A9P52610's template CPs 1, 2, N, N;
/// -102F1's printed L1 and T1 on A9F78116; 195740's DP slave port beside 550208's black box; 533361's PLC section
/// with H1 as `test`; 550208's FE pin as `test`.
#[test]
fn the_derived_templates_sections_and_marks_are_golden_as() {
    let (parts, _) = shipped_records();
    let designations = |n: &str| -> Vec<&str> {
        parts[n]["cp_pattern"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["designation"].as_str().unwrap())
            .collect()
    };
    let template = |n: &str| -> Vec<(&str, bool)> {
        parts[n]["function_template"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| (t["fndef"]["key"].as_str().unwrap(), t["main"] == true))
            .collect()
    };
    assert_eq!(designations("A9P52610"), ["1", "2", "N", "N"]);
    assert_eq!(template("A9P52610"), [("ew.fn.mcb-1pn", true)]);
    assert_eq!(
        parts["A9P52610"]["meta"]["sources"],
        json!(["golden.table:parts|A9P52610", "golden.table:functions|-101F1"])
    );
    assert_eq!(designations("A9F78116"), ["L1", "T1"]);
    // -201U1 holds both part uses: 550208 keeps its black box, 195740 takes the overview and the DP port.
    assert_eq!(
        template("195740"),
        [("ew.fn.overview", true), ("ew.fn.dp-port", false)]
    );
    assert_eq!(designations("195740"), ["A", "B", "SH"]);
    assert_eq!(
        parts["195740"]["sections"]["bus"],
        json!({"ports": [{"interface": "DP", "bus": "profibus_dp", "role": "slave"}]})
    );
    assert_eq!(
        template("550208"),
        [("ew.fn.blackbox", true)],
        "Golden A's own template, its entry the main one"
    );
    assert_eq!(parts["550208"]["meta"]["confidence"]["cp_pattern"], "test");
    assert_eq!(
        template("533361"),
        [("ew.fn.overview", true), ("ew.fn.blackbox", false)]
    );
    assert_eq!(parts["533361"]["meta"]["confidence"]["plc"], "test");
    assert_eq!(
        parts["533361"]["sections"]["plc"]["positions"],
        json!({"count": 4, "map": [{"solenoid": "14", "channel": "2p"}, {"solenoid": "12", "channel": "2p+1"}]})
    );
    assert_eq!(parts["SK 3102.000"]["meta"]["confidence"]["mech"], "test");
    assert_eq!(
        parts["571334"]["meta"]["confidence"]["mech"], "unverified",
        "no dimensions"
    );
    // A cable's drawing is no function of the part; a layout part has no function.
    for n in [
        "6XV1830-0EH10",
        "NEBU-G78W5-K-2-N-LE5",
        "NSYPLA1074G",
        "NSYPMM107",
    ] {
        assert!(parts[n].get("function_template").is_none(), "{n}");
    }
    assert_eq!(
        parts["NSYPLA1074G"]["sections"]["enclosure"]["plate"]["item"], parts["NSYPMM107"]["id"],
        "the enclosure's plate is the library's"
    );
}

/// D3: the installation's system tier (`content/libraries`) finds the library, and every part resolves with its
/// closure (its manufacturer, its plate and the starter FnDefs of its template).
#[test]
fn the_system_tier_finds_the_library_and_resolves_its_parts() {
    let root = ew_testdir::fresh_dir("ew-library-sample-tier");
    let reg = Registry::open(&tiers(&root, vec![repo().join("content/libraries")], Vec::new())).unwrap();
    let info = reg
        .libraries()
        .into_iter()
        .find(|l| l.key == KEY)
        .expect("the sample parts library in the system tier");
    assert_eq!(
        (
            info.id,
            info.name.as_str(),
            info.tier,
            info.version.as_str(),
            info.licence.as_str()
        ),
        (library(), NAME, Tier::System, LIBRARY_VERSION, LICENCE)
    );
    assert_eq!(info.findings, Vec::<String>::new());
    for doc in documents() {
        let LibDocument::Part(c) = doc else { continue };
        let resolved = reg
            .resolve(&ew_format::ew_schema::LibRef {
                library: c.library_id,
                item: c.item_id,
                version_req: "^0.1".into(),
                extra: Default::default(),
            })
            .unwrap_or_else(|e| panic!("{}: {e}", c.item.part_number.as_str()));
        assert!(matches!(resolved, LibDocument::Part(_)));
        reg.resolve(&c.item.manufacturer).unwrap();
        for t in &c.item.function_template {
            reg.resolve(&t.fndef).unwrap();
        }
    }
    drop(reg);
    let _ = std::fs::remove_dir_all(&root);
}
