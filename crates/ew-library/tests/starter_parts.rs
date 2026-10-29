// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The generic starter parts of WP-S2.18 D5: their definitions and the generator of the system library
//! `content/libraries/ew-starter-parts.ewlib`.
//!
//! [`parts`] defines the parts; `the_shipped_library_is_the_generated_one` writes them with the library writer into a
//! fresh library of the system library's ID and compares every file with the shipped directory and with the compiled-in
//! file list `crates/ew-library/src/starter_parts.rs`. `EW_BLESS=1 cargo nextest run -p ew-library --test
//! starter_parts` rewrites both (run it twice: the second run changes nothing).
//!
//! The parts are CC0-1.0 part facts (doc 19 §2.1 L8): manufacturer "Generic", part numbers `EW-GEN-<kind>-<n>`, a
//! description, no order or catalogue data; each carries the template functions on the starter FnDefs, the CP pattern,
//! the contact data of its contacts and the technical data the starter symbols print in their `tech_data` slot.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ew_format::ew_schema::{Kind, LibDocument, LibRef};
use ew_ids::Uuid;
use ew_library::{
    LibManifest, LibraryTiers, PartQuery, Registry, STARTER_PARTS_KEY, starter, starter_parts_library,
};
use serde_json::{Value, json};

const ZERO: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
const NAME: &str = "ElektraWatt starter parts";
const LICENCE: &str = "CC0-1.0";
const VERSION: &str = "0.1.0";
/// The manufacturer of every generic part.
const GENERIC: &str = "Generic";

/// The item ID of a starter-parts item: `v5(library, "<kind>:<key>")`.
fn item_id(kind: &str, key: &str) -> Uuid {
    ew_ids::v5::imported_id::<()>(starter_parts_library(), &format!("{kind}:{key}")).as_uuid()
}

/// The reference to starter FnDef `key`, with its key for the reader.
fn fndef(key: &str) -> Value {
    let doc = LibDocument::from_json(starter::item(key).unwrap_or_else(|| panic!("{key}")).json).unwrap();
    let LibDocument::Fndef(c) = doc else {
        panic!("{key} is not a FnDef")
    };
    json!({
        "library": starter::LIBRARY.to_string(),
        "item": c.item_id.as_uuid().to_string(),
        "version_req": "^0.1",
        "key": key,
    })
}

/// A quantity.
fn q(value: &str, unit: &str) -> Value {
    json!({"value": value, "unit": unit})
}

/// One generic part: number, part kind, description, the CP designations (index order, with a potential role),
/// the template (FnDef key, main, CP indices), the contacts (kind, CP designations) and the technical data.
struct Def {
    number: &'static str,
    kind: Option<&'static str>,
    description: &'static str,
    cps: Vec<(&'static str, Option<&'static str>)>,
    template: Vec<(&'static str, bool, Vec<u16>)>,
    contacts: Vec<(&'static str, Vec<&'static str>)>,
    tech: Vec<(&'static str, Value)>,
    plc: Option<Value>,
}

fn cps(designations: &[&'static str]) -> Vec<(&'static str, Option<&'static str>)> {
    designations.iter().map(|d| (*d, None)).collect()
}

fn range(from: u16, to: u16) -> Vec<u16> {
    (from..to).collect()
}

/// The generic starter parts of D5 (the MCB 3P and the valve 3/2 are de-scoped: the starter set has no FnDef or
/// symbol for them).
///
/// The relay and the contactor template their coil only; their contacts are the contact image (`contacts`), and a
/// contact joins the device as an auxiliary function with its FnDef's designations. A template with several entries
/// of one FnDef would give every such function the CPs of the first entry (the CP derivation matches a function to
/// its template entry by FnDef alone), so no part here has one.
fn defs() -> Vec<Def> {
    let relay_cps = cps(&[
        "A1", "A2", "11", "12", "14", "21", "22", "24", "31", "32", "34", "41", "42", "44",
    ]);
    let card = |dir: &str, kind: &str| {
        json!({
            "role": "module",
            "io": [{"dir": dir, "bits": 8}],
            "channels": {"count": 8, "kind": kind, "data_type": "Bool", "designation": "{n}"},
            "bit_of": "n",
            "pin_of": "{n+1}",
        })
    };
    vec![
        Def {
            number: "EW-GEN-RLY-1",
            kind: None,
            description: "Relay with four changeover contacts",
            cps: relay_cps,
            template: vec![("ew.fn.coil", true, vec![0, 1])],
            contacts: vec![
                ("co", vec!["11", "12", "14"]),
                ("co", vec!["21", "22", "24"]),
                ("co", vec!["31", "32", "34"]),
                ("co", vec!["41", "42", "44"]),
            ],
            tech: vec![("coil_voltage", q("24", "V"))],
            plc: None,
        },
        Def {
            number: "EW-GEN-CON-1",
            kind: None,
            description: "Contactor with three main contacts and one auxiliary normally open contact",
            cps: cps(&["A1", "A2", "1", "2", "3", "4", "5", "6", "13", "14"]),
            template: vec![("ew.fn.coil", true, vec![0, 1])],
            contacts: vec![
                ("no", vec!["1", "2"]),
                ("no", vec!["3", "4"]),
                ("no", vec!["5", "6"]),
                ("no", vec!["13", "14"]),
            ],
            tech: vec![("coil_voltage", q("24", "V")), ("rated_current", q("9", "A"))],
            plc: None,
        },
        Def {
            number: "EW-GEN-MCB-1",
            kind: None,
            description: "Miniature circuit breaker, 1 pole",
            cps: cps(&["1", "2"]),
            template: vec![("ew.fn.mcb-1p", true, vec![0, 1])],
            contacts: Vec::new(),
            tech: vec![("characteristic", json!("C")), ("rated_current", q("16", "A"))],
            plc: None,
        },
        Def {
            number: "EW-GEN-MCB-2",
            kind: None,
            description: "Miniature circuit breaker, 1 pole and neutral",
            cps: cps(&["1", "2", "N", "N"]),
            template: vec![("ew.fn.mcb-1pn", true, vec![0, 1, 2, 3])],
            contacts: Vec::new(),
            tech: vec![("characteristic", json!("C")), ("rated_current", q("10", "A"))],
            plc: None,
        },
        Def {
            number: "EW-GEN-FUS-1",
            kind: None,
            description: "Fuse holder with fuse link",
            cps: cps(&["1", "2"]),
            template: vec![("ew.fn.fuse", true, vec![0, 1])],
            contacts: Vec::new(),
            tech: vec![("rated_current", q("4", "A"))],
            plc: None,
        },
        Def {
            number: "EW-GEN-PB-1",
            kind: None,
            description: "Pushbutton with one normally open contact",
            cps: cps(&["13", "14"]),
            template: vec![("ew.fn.pushbutton-no", true, vec![0, 1])],
            contacts: vec![("no", vec!["13", "14"])],
            tech: Vec::new(),
            plc: None,
        },
        Def {
            number: "EW-GEN-SEL-1",
            kind: None,
            description: "Selector switch, two positions, with two normally open contacts",
            cps: cps(&["1", "2", "3", "4"]),
            template: vec![("ew.fn.switch-2p-manual", true, vec![0, 1, 2, 3])],
            contacts: vec![("no", vec!["1", "2"]), ("no", vec!["3", "4"])],
            tech: Vec::new(),
            plc: None,
        },
        Def {
            number: "EW-GEN-POS-1",
            kind: None,
            description: "Position switch with one normally open contact",
            cps: cps(&["13", "14"]),
            template: vec![("ew.fn.position-switch-no", true, vec![0, 1])],
            contacts: vec![("no", vec!["13", "14"])],
            tech: Vec::new(),
            plc: None,
        },
        Def {
            number: "EW-GEN-LMP-1",
            kind: None,
            description: "Pilot lamp",
            cps: cps(&["X1", "X2"]),
            template: vec![("ew.fn.lamp", true, vec![0, 1])],
            contacts: Vec::new(),
            tech: vec![("rated_voltage", q("24", "V"))],
            plc: None,
        },
        Def {
            number: "EW-GEN-MOT-1",
            kind: None,
            description: "Three-phase motor",
            cps: vec![("U1", None), ("V1", None), ("W1", None), ("PE", Some("pe"))],
            template: vec![("ew.fn.motor-3ph", true, vec![0, 1, 2, 3])],
            contacts: Vec::new(),
            tech: vec![("rated_power", q("370", "W")), ("rated_voltage", q("400", "V"))],
            plc: None,
        },
        Def {
            number: "EW-GEN-PSU-1",
            kind: None,
            description: "Power supply unit, AC to DC",
            cps: vec![
                ("L", Some("l")),
                ("N", Some("n")),
                ("PE", Some("pe")),
                ("+", Some("l_plus")),
                ("+", Some("l_plus")),
                ("-", Some("m")),
                ("-", Some("m")),
            ],
            template: vec![("ew.fn.blackbox", true, range(0, 7))],
            contacts: Vec::new(),
            tech: vec![
                ("input_voltage", q("230", "V")),
                ("output_current", q("5", "A")),
                ("output_voltage", q("24", "V")),
            ],
            plc: None,
        },
        Def {
            number: "EW-GEN-VLV-1",
            kind: None,
            description: "Solenoid valve 5/2, spring return",
            cps: cps(&["A1", "A2", "1", "2", "3", "4", "5", "14"]),
            template: vec![
                ("ew.fn.coil", true, vec![0, 1]),
                ("ew.fn.valve-5-2-sol-spring", false, range(2, 8)),
            ],
            contacts: Vec::new(),
            tech: vec![("coil_voltage", q("24", "V"))],
            plc: None,
        },
        Def {
            number: "EW-GEN-TRM-1",
            kind: Some("terminal"),
            description: "Feed-through terminal",
            cps: cps(&["1", "2"]),
            template: vec![("ew.fn.terminal", true, vec![0, 1])],
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
        },
        Def {
            number: "EW-GEN-TRM-2",
            kind: Some("terminal"),
            description: "Protective earth terminal",
            cps: vec![("1", Some("pe")), ("2", Some("pe")), ("RF", Some("pe"))],
            template: vec![("ew.fn.terminal-pe", true, vec![0, 1, 2])],
            contacts: Vec::new(),
            tech: Vec::new(),
            plc: None,
        },
        Def {
            number: "EW-GEN-DI-1",
            kind: None,
            description: "PLC digital input card, 8 channels",
            cps: vec![
                ("1", None),
                ("2", None),
                ("3", None),
                ("4", None),
                ("5", None),
                ("6", None),
                ("7", None),
                ("8", None),
                ("M", Some("m")),
            ],
            template: vec![("ew.fn.blackbox", true, range(0, 9))],
            contacts: Vec::new(),
            tech: vec![("rated_voltage", q("24", "V"))],
            plc: Some(card("in", "di")),
        },
        Def {
            number: "EW-GEN-DO-1",
            kind: None,
            description: "PLC digital output card, 8 channels",
            cps: vec![
                ("1", None),
                ("2", None),
                ("3", None),
                ("4", None),
                ("5", None),
                ("6", None),
                ("7", None),
                ("8", None),
                ("L+", Some("l_plus")),
                ("M", Some("m")),
            ],
            template: vec![("ew.fn.blackbox", true, range(0, 10))],
            contacts: Vec::new(),
            tech: vec![("output_current", q("0.5", "A")), ("rated_voltage", q("24", "V"))],
            plc: Some(card("out", "do")),
        },
    ]
}

/// The manufacturer record "Generic".
fn manufacturer() -> LibDocument {
    let lib = starter_parts_library();
    let item = item_id("manufacturer", GENERIC);
    LibDocument::from_value(json!({
        "$k": "manufacturer", "id": item.to_string(), "library_id": lib.to_string(),
        "item_id": item.to_string(), "version": VERSION, "sha256": ZERO,
        "meta": {"licence": LICENCE, "sources": ["ew.original"]},
        "key": GENERIC, "name": {"en": GENERIC},
    }))
    .unwrap()
}

/// The part documents, in the order of [`defs`].
fn parts() -> Vec<LibDocument> {
    let lib = starter_parts_library();
    let mfr = item_id("manufacturer", GENERIC);
    defs()
        .into_iter()
        .map(|d| {
            let item = item_id("part", d.number);
            let main = d.template.iter().find(|t| t.1).map(|t| t.0).unwrap();
            let category = match LibDocument::from_json(starter::item(main).unwrap().json).unwrap() {
                LibDocument::Fndef(c) => serde_json::to_value(&c.item.category).unwrap(),
                _ => unreachable!(),
            };
            let mut v = json!({
                "$k": "part", "id": item.to_string(), "library_id": lib.to_string(),
                "item_id": item.to_string(), "version": VERSION, "sha256": ZERO,
                "meta": {"key": d.number, "licence": LICENCE, "sources": ["ew.original"]},
                "manufacturer": {"library": lib.to_string(), "item": mfr.to_string(), "version_req": "^0.1"},
                "part_number": d.number,
                "description": {"en": d.description},
                "category": category,
                "cp_pattern": d.cps.iter().enumerate().map(|(i, (des, role))| {
                    let mut cp = json!({"idx": i, "designation": des});
                    if let Some(r) = role {
                        cp["potential_role"] = json!(r);
                    }
                    cp
                }).collect::<Vec<_>>(),
                "function_template": d.template.iter().map(|(key, main, cps)| {
                    let mut t = json!({"fndef": fndef(key), "cps": cps});
                    if *main {
                        t["main"] = json!(true);
                    }
                    t
                }).collect::<Vec<_>>(),
            });
            if let Some(k) = d.kind {
                v["kind"] = json!(k);
            }
            if !d.tech.is_empty() {
                v["tech_data"] = d
                    .tech
                    .iter()
                    .map(|(k, val)| json!({"key": k, "value": val}))
                    .collect();
            }
            if !d.contacts.is_empty() {
                v["contacts"] = d
                    .contacts
                    .iter()
                    .map(|(k, cps)| json!({"kind": k, "cps": cps}))
                    .collect();
            }
            if let Some(plc) = d.plc {
                v["sections"] = json!({ "plc": plc });
            }
            LibDocument::from_value(v).unwrap_or_else(|e| panic!("{}: {e}", d.number))
        })
        .collect()
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn shipped_dir() -> PathBuf {
    repo()
        .join("content")
        .join("libraries")
        .join("ew-starter-parts.ewlib")
}

// The test boundary: `EW_BLESS=1` asks the generator to rewrite the shipped files.
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
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
                name.clone()
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

/// The text of `crates/ew-library/src/starter_parts.rs` for the records files `paths`.
fn module_text(paths: &[&String]) -> String {
    // The header of the generated file, not of this one. REUSE-IgnoreStart
    let mut s = String::from(
        "// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors\n\
         // SPDX-License-Identifier: GPL-3.0-or-later\n\n\
         //! The system library `ew-starter-parts` compiled in (WP-S2.18 D6): the files of\n\
         //! `content/libraries/ew-starter-parts.ewlib`. Generated by the test `starter_parts` of this crate\n\
         //! (`EW_BLESS=1 cargo nextest run -p ew-library --test starter_parts`); do not edit.\n\n\
         /// The library's `library.json`.\n\
         pub(crate) const MANIFEST: &str =\n    include_str!(\"../../../content/libraries/ew-starter-parts.ewlib/library.json\");\n\n\
         /// The records files the manifest names, as `(path, text)`.\n\
         pub(crate) const FILES: &[(&str, &str)] = &[\n",
    );
    // REUSE-IgnoreEnd
    for p in paths {
        s.push_str(&format!(
            "    (\n        \"{p}\",\n        include_str!(\"../../../content/libraries/ew-starter-parts.ewlib/{p}\"),\n    ),\n"
        ));
    }
    s.push_str("];\n");
    s
}

/// The generator: the parts written by the library writer into a fresh library of the system library's ID equal the
/// shipped directory and the compiled-in file list (`EW_BLESS=1` rewrites both).
#[test]
fn the_shipped_library_is_the_generated_one() {
    let root = ew_testdir::fresh_dir("ew-library-starter-parts");
    let dir = root.join("company").join("ew-starter-parts.ewlib");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("library.json"),
        LibManifest::new(starter_parts_library(), STARTER_PARTS_KEY, NAME, LICENCE).to_text(),
    )
    .unwrap();
    let mut reg = Registry::open(&LibraryTiers {
        system: Vec::new(),
        community: root.join("data").join("packs"),
        company: vec![root.join("company")],
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    })
    .unwrap();
    let mut docs = vec![manufacturer()];
    docs.extend(parts());
    reg.write(starter_parts_library(), &docs).unwrap();
    drop(reg);
    let generated = files_of(&dir);
    let _ = std::fs::remove_dir_all(&root);
    let records: Vec<&String> = generated.keys().filter(|p| *p != "library.json").collect();
    let module = module_text(&records);
    let module_path = repo()
        .join("crates")
        .join("ew-library")
        .join("src")
        .join("starter_parts.rs");
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
        std::fs::write(&module_path, &module).unwrap();
    }
    assert_eq!(
        files_of(&shipped),
        generated,
        "content/libraries/ew-starter-parts.ewlib is not the generated library (EW_BLESS=1 rewrites it)"
    );
    let current = std::fs::read_to_string(&module_path)
        .unwrap()
        .replace("\r\n", "\n");
    assert_eq!(
        current, module,
        "src/starter_parts.rs does not list the library's files (EW_BLESS=1 rewrites it)"
    );
}

/// D5: every part carries a description, the manufacturer "Generic", a part number of the fixed form, a function
/// template with one main function on starter FnDefs whose CP indices exist, and contacts whose CPs are in its CP
/// pattern; no order data.
#[test]
fn every_generic_part_is_usable() {
    let numbers: Vec<&str> = defs().iter().map(|d| d.number).collect();
    assert_eq!(numbers.len(), 16);
    for doc in parts() {
        let LibDocument::Part(c) = doc else { panic!() };
        let p = &c.item;
        let n = p.part_number.as_str();
        let mut words = n.split('-');
        assert_eq!((words.next(), words.next()), (Some("EW"), Some("GEN")), "{n}");
        let kind = words.next().unwrap();
        assert!(
            !kind.is_empty() && kind.bytes().all(|b| b.is_ascii_uppercase()),
            "{n}"
        );
        assert!(
            words.next().unwrap().parse::<u32>().is_ok() && words.next().is_none(),
            "{n}"
        );
        assert!(!p.description.is_empty(), "{n}");
        assert!(
            p.order_number.is_none() && p.gtin.is_none() && p.type_designation.is_none(),
            "{n}"
        );
        assert_eq!(
            p.manufacturer.item.as_uuid(),
            item_id("manufacturer", GENERIC),
            "{n}"
        );
        assert_eq!(p.function_template.iter().filter(|t| t.main).count(), 1, "{n}");
        for t in &p.function_template {
            assert_eq!(t.fndef.library.as_uuid(), starter::LIBRARY, "{n}");
            for i in &t.cps {
                assert!(p.cp_pattern.iter().any(|cp| cp.idx == *i), "{n}: CP {i}");
            }
        }
        for contact in &p.contacts {
            for d in &contact.cps {
                assert!(
                    p.cp_pattern.iter().any(|cp| cp.designation == *d),
                    "{n}: contact CP {d}"
                );
            }
        }
    }
}

/// D6: the registry of a session with no library configured holds the starter parts, compiled in; a part resolves
/// with its closure (the part, its template FnDefs from the starter, the manufacturer).
#[test]
fn the_compiled_in_registry_resolves_a_part_with_its_closure() {
    let reg = Registry::starter_only();
    let keys: Vec<String> = reg.libraries().into_iter().map(|l| l.key).collect();
    assert_eq!(keys, ["ew-starter", "ew-starter-parts"]);
    let docs = reg.documents(starter_parts_library(), Kind::Part);
    assert_eq!(docs.len(), defs().len());
    let hits = reg.search(&PartQuery {
        text: "EW-GEN-RLY-1".into(),
        ..PartQuery::default()
    });
    assert_eq!(hits.first().map(|h| h.part_number.as_str()), Some("EW-GEN-RLY-1"));
    let r = LibRef {
        library: ew_ids::Id::from_uuid(starter_parts_library()),
        item: ew_ids::Id::from_uuid(item_id("part", "EW-GEN-RLY-1")),
        version_req: "^0.1".into(),
        extra: Default::default(),
    };
    let closure = reg.closure(&r).unwrap();
    let kinds: Vec<Option<Kind>> = closure.iter().map(LibDocument::kind).collect();
    // The part, its coil's FnDef (the contacts are its contact image) and the manufacturer.
    assert_eq!(
        kinds,
        [Some(Kind::Part), Some(Kind::Fndef), Some(Kind::Manufacturer)]
    );
}
