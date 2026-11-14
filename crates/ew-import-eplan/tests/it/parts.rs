// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The EDZ and parts XML reader (`read_parts`, doc 14 §5.2, §5.3): what a part is made of, the findings, the limits.
//! The files are synthetic (CC0): a part of the shape of doc 14, never a vendor's data.

use ew_import_eplan::{FndefMap, read_parts};
use ew_interop_api::{Code, ImportError, ImportInput, Limits, PartDraft};

use crate::reader::{Entry, seven, zip_of};

/// A power supply of the shape of the golden part 2904602: three input pins and four output pins in one function.
const PSU: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<part P_ARTICLE_PARTNR="2904602" P_ARTICLE_MANUFACTURER="Phoenix Contact" P_ARTICLE_ORDERNR="2904602"
      P_ARTICLE_TYPENR="QUINT4-PS/1AC/24DC/10/PT"
      P_ARTICLE_DESCR1="de_DE@Stromversorgung;en_US@Power supply;"
      P_ARTICLE_DESCR2="de_DE@1-phasig;en_US@single phase;" P_ARTICLE_PICTUREFILE="2904602.png"
      P_ARTICLE_WIDTH="70" P_ARTICLE_HEIGHT="130" P_ARTICLE_DEPTH="125.5">
  <variant P_ARTICLE_VARIANT="1">
    <functiontemplate connectionDesignation="L&#10;N&#10;PE&#10;+&#10;+&#10;−&#10;−"
                      connectiondescription="Phase&#10;Neutral&#10;Earth&#10;Output&#10;Output&#10;Return&#10;Return"
                      functiondefcategory="2" functiondefgroup="1" functiondefid="1"/>
    <drillingPosition x="1" y="2" diameter="4"/>
  </variant>
  <freeproperty name="colour" value="grey"/>
  <accessorylist><accessory P_ARTICLE_PARTNR="2904603" P_ARTICLE_MANUFACTURER="Phoenix Contact" quantity="2"/></accessorylist>
</part>"#;

fn input(name: &str, bytes: &[u8]) -> ImportInput {
    ImportInput::new(name, bytes.to_vec()).unwrap()
}

fn read(name: &str, bytes: &[u8]) -> Result<ew_import_eplan::PartsRead, ImportError> {
    read_parts(&input(name, bytes), &FndefMap::shipped(), &Limits::default())
}

fn designations(d: &PartDraft) -> Vec<String> {
    d.body["cp_pattern"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["designation"].as_str().unwrap().to_owned())
        .collect()
}

fn codes(r: &ew_import_eplan::PartsRead) -> Vec<Code> {
    r.entries.iter().map(|e| e.code).collect()
}

/// FR-IMP-025: an EDZ of the 2904602 shape yields the pins L, N, PE, +, +, −, − and German and English descriptions;
/// the ZIP container reads the same, and says it is a ZIP (EW-IMP-203).
#[test]
fn an_edz_in_7z_and_in_zip_gives_the_part_with_its_pins_and_descriptions() {
    let seven = read(
        "2904602.edz",
        &seven(&[("2904602.part.xml", PSU.as_bytes())], false),
    )
    .unwrap();
    let zip = read(
        "2904602.edz",
        &zip_of(&[Entry::stored("data/2904602.part.xml", PSU.as_bytes())], None),
    )
    .unwrap();
    assert_eq!((seven.format, zip.format), ("edz-7z", "edz-zip"));
    for read in [&seven, &zip] {
        let [part] = read.drafts.as_slice() else {
            panic!("one part")
        };
        assert_eq!(part.part_number, "2904602");
        assert_eq!(part.manufacturer, "Phoenix Contact");
        assert_eq!(designations(part), ["L", "N", "PE", "+", "+", "−", "−"]);
        assert_eq!(
            part.body["description"],
            serde_json::json!({"de-DE": "Stromversorgung, 1-phasig", "en-US": "Power supply, single phase"})
        );
        // Two pins of one designation stay two CPs.
        let idx: Vec<u64> = part.body["cp_pattern"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["idx"].as_u64().unwrap())
            .collect();
        assert_eq!(idx, [0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(part.body["function_template"][0]["main"], true);
        assert_eq!(
            part.body["function_template"][0]["fndef"]["key"],
            "ew.fn.psu-ac-dc"
        );
    }
    assert!(!codes(&seven).contains(&Code::imp(203)));
    assert!(codes(&zip).contains(&Code::imp(203)));
}

/// FR-IMP-026: the part says where it came from: the file, its SHA-256, the vendor terms.
#[test]
fn a_part_carries_its_provenance_and_the_foreign_terms() {
    let read = read(
        "psu.edz",
        &zip_of(&[Entry::stored("a.part.xml", PSU.as_bytes())], None),
    )
    .unwrap();
    let part = &read.drafts[0];
    assert!(part.sources.iter().any(|s| s.contains("psu.edz")));
    assert!(
        part.sources
            .iter()
            .any(|s| s.starts_with("sha256:") && s.len() == 7 + 64)
    );
    assert_eq!(part.meta["foreign_terms"], true);
    assert_eq!(part.meta["licence"], "LicenseRef-vendor-terms");
}

/// What the part model has no member for is said, not lost silently: the picture, the drilling positions, the other
/// variants; the accessories and free properties are kept.
#[test]
fn extras_are_kept_or_reported() {
    let read = read("psu.xml", PSU.as_bytes()).unwrap();
    let part = &read.drafts[0];
    assert_eq!(read.format, "part-xml");
    assert_eq!(part.body["eplan.accessories"][0]["part_number"], "2904603");
    assert_eq!(part.body["free_props"][0]["key"], "colour");
    let said: Vec<&str> = read.entries.iter().map(|e| e.message.as_str()).collect();
    assert!(said.iter().any(|m| m.contains("drilling positions")), "{said:?}");
    assert!(said.iter().any(|m| m.contains("picture")), "{said:?}");
}

/// EW-IMP-202: a template whose function definition the table does not map gives generic pins; the user's own row maps it.
#[test]
fn an_unmapped_function_definition_gives_generic_pins_until_the_user_maps_it() {
    let xml = PSU.replace("functiondefcategory=\"2\"", "functiondefcategory=\"99\"");
    let generic = read("a.xml", xml.as_bytes()).unwrap();
    assert!(codes(&generic).contains(&Code::imp(202)));
    assert_eq!(
        generic.drafts[0].body["function_template"][0]["fndef"]["key"],
        "ew.fn.blackbox"
    );
    let mine = FndefMap::shipped()
        .with_user("[[map]]\ncategory = \"99\"\ngroup = \"1\"\nid = \"1\"\nfndef = \"ew.fn.psu-ac-dc\"\n")
        .unwrap();
    let mapped = read_parts(&input("a.xml", xml.as_bytes()), &mine, &Limits::default()).unwrap();
    assert!(!codes(&mapped).contains(&Code::imp(202)));
    assert_eq!(
        mapped.drafts[0].body["function_template"][0]["fndef"]["key"],
        "ew.fn.psu-ac-dc"
    );
    assert!(
        FndefMap::shipped()
            .with_user("[[map]]\ncategory = \"1\"")
            .is_err()
    );
}

/// The pins of a terminal are sorted naturally within their function definition ("10" after "3"); a lettered
/// group keeps the template's order.
#[test]
fn numbered_pins_sort_naturally_and_lettered_pins_keep_their_order() {
    let xml = r#"<part P_ARTICLE_PARTNR="T10" P_ARTICLE_MANUFACTURER="Acme">
        <functiontemplate connectionDesignation="1&#10;2&#10;10&#10;3" functiondefinition="1.1.1"/></part>"#;
    let read = read("t.xml", xml.as_bytes()).unwrap();
    let part = &read.drafts[0];
    assert_eq!(designations(part), ["1", "2", "3", "10"]);
    assert_eq!(part.body["kind"], "terminal");
}

/// `;;` inside a text is a `;`; a field without a language is neutral; a language that is none is reported.
#[test]
fn multilingual_fields_are_split_at_single_semicolons() {
    let xml = r#"<part P_ARTICLE_PARTNR="D1" P_ARTICLE_MANUFACTURER="Acme"
        P_ARTICLE_DESCR1="de_DE@Relais 5;;6 V;en_US@Relay 5;;6 V;xx_@@;"><functiontemplate connectionDesignation="1"/></part>"#;
    let part = &read("d.xml", xml.as_bytes()).unwrap().drafts[0];
    assert_eq!(
        part.body["description"],
        serde_json::json!({"de-DE": "Relais 5;6 V", "en-US": "Relay 5;6 V"})
    );
    let plain = r#"<part P_ARTICLE_PARTNR="D2" P_ARTICLE_MANUFACTURER="Acme" P_ARTICLE_DESCR1="Relay"/>"#;
    assert_eq!(
        read("d.xml", plain.as_bytes()).unwrap().drafts[0].body["description"],
        "Relay"
    );
}

/// FR-IMP-028: a parts XML of 50 parts with variants, accessories and free properties; its `length-unit` scales the
/// sizes to nanometres (EW-IMP-205 when it is not mm) and an unknown unit refuses the file.
#[test]
fn a_parts_xml_of_fifty_parts_is_read_with_its_variants_and_its_unit() {
    let mut xml = String::from("<?xml version=\"1.0\"?>\n<partsmanagement length-unit=\"cm\">\n");
    for n in 0..50 {
        xml.push_str(&format!(
            "<part P_ARTICLE_PARTNR=\"P{n:03}\" P_ARTICLE_MANUFACTURER=\"Acme\" P_ARTICLE_WIDTH=\"7,5\">\n\
             <variant P_ARTICLE_VARIANT=\"A\"><functiontemplate connectionDesignation=\"1&#10;2\"/></variant>\n\
             <variant P_ARTICLE_VARIANT=\"B\"><functiontemplate connectionDesignation=\"1&#10;2&#10;3\"/></variant>\n\
             <freeproperty name=\"series\" value=\"S{n}\"/>\n\
             <accessorylist><accessory P_ARTICLE_PARTNR=\"A{n}\" P_ARTICLE_MANUFACTURER=\"Acme\" quantity=\"1\"/></accessorylist>\n\
             </part>\n"
        ));
    }
    xml.push_str("</partsmanagement>");
    let read = read("parts.xml", xml.as_bytes()).unwrap();
    assert_eq!(read.format, "parts-xml");
    assert_eq!(read.drafts.len(), 50);
    let first = &read.drafts[0];
    assert_eq!(first.body["mech"]["width"], 75_000_000);
    assert_eq!(
        first.body["eplan.variants"][0]["pins"],
        serde_json::json!(["1", "2", "3"])
    );
    assert_eq!(first.body["eplan.accessories"].as_array().unwrap().len(), 1);
    assert!(codes(&read).contains(&Code::imp(205)));
    let odd = xml.replace("length-unit=\"cm\"", "length-unit=\"furlong\"");
    assert!(matches!(
        read_parts(
            &input("p.xml", odd.as_bytes()),
            &FndefMap::shipped(),
            &Limits::default()
        ),
        Err(ImportError::Unsupported { .. })
    ));
}

/// EW-IMP-204: a 3D model beyond the member limit that nobody reads does not refuse the EDZ; it is said ignored.
#[test]
fn a_large_model_in_an_edz_is_counted_not_read() {
    let model = vec![7u8; 5 * 1024 * 1024];
    let edz = zip_of(
        &[
            Entry::stored("a.part.xml", PSU.as_bytes()),
            Entry::stored("a.e3d", &model),
            Entry::stored("a.png", b"png"),
        ],
        None,
    );
    let read = read("a.edz", &edz).unwrap();
    assert_eq!(read.drafts.len(), 1);
    assert!(codes(&read).contains(&Code::imp(204)));
}

/// A hostile file is refused in words and never panics: a DOCTYPE, a depth over the limit, too many attributes, an
/// archive without part data, a file of another kind.
#[test]
fn hostile_files_are_refused() {
    let doctype = format!("<!DOCTYPE part [<!ENTITY a \"b\">]>{PSU}");
    assert!(matches!(
        read("a.xml", doctype.as_bytes()),
        Err(ImportError::Malformed { .. })
    ));
    let deep = format!("{}{}", "<a>".repeat(300), "</a>".repeat(300));
    let deep = format!("<partsmanagement>{deep}</partsmanagement>");
    assert!(matches!(
        read("a.xml", deep.as_bytes()),
        Err(ImportError::LimitExceeded { .. })
    ));
    let many: String = (0..100).map(|i| format!(" a{i}=\"x\"")).collect();
    let tight = Limits {
        max_xml_attributes_per_element: 50,
        ..Limits::default()
    };
    assert!(matches!(
        read_parts(
            &input("a.xml", format!("<part{many}/>").as_bytes()),
            &FndefMap::shipped(),
            &tight
        ),
        Err(ImportError::LimitExceeded { .. })
    ));
    let none = zip_of(&[Entry::stored("readme.txt", b"hello")], None);
    assert!(matches!(read("a.edz", &none), Err(ImportError::Malformed { .. })));
    assert!(read("a.xml", b"<project version=\"0.100\"/>").is_err());
    assert!(read("a.edz", b"PK\x03\x04 not a zip").is_err());
    let nested = zip_of(&[Entry::stored("../a.part.xml", PSU.as_bytes())], None);
    assert!(read("a.edz", &nested).is_err());
}

/// A part with thousands of pins is cut to what a part may hold and the cut is said (doc 14 §11: 256 MB), however many
/// such parts the file has.
#[test]
fn a_hostile_pin_list_is_cut_and_said() {
    let pins = (0..10_000)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("&#10;");
    let mut xml = String::from("<partsmanagement>");
    for n in 0..100 {
        xml.push_str(&format!(
            "<part P_ARTICLE_PARTNR=\"P{n}\" P_ARTICLE_MANUFACTURER=\"Acme\"><functiontemplate connectionDesignation=\"{pins}\"/></part>"
        ));
    }
    xml.push_str("</partsmanagement>");
    let read = read("p.xml", xml.as_bytes()).unwrap();
    assert_eq!(read.drafts.len(), 100);
    let total: usize = read
        .drafts
        .iter()
        .map(|d| {
            d.body
                .get("cp_pattern")
                .map_or(0, |c| c.as_array().unwrap().len())
        })
        .sum();
    assert!(total <= 262_144, "{total}");
    assert!(read.drafts.iter().all(|d| {
        d.body
            .get("cp_pattern")
            .is_none_or(|c| c.as_array().unwrap().len() <= 4_096)
    }));
    assert!(codes(&read).contains(&Code::LIMIT));
}

/// A part with no part number or no manufacturer still becomes a draft; the placer rejects it in words.
#[test]
fn a_part_without_number_or_manufacturer_is_left_to_the_placer() {
    let read = read(
        "a.xml",
        br#"<partsmanagement><part P_ARTICLE_MANUFACTURER="Acme"/><part P_ARTICLE_PARTNR="X"/></partsmanagement>"#,
    )
    .unwrap();
    assert_eq!(read.drafts.len(), 2);
    assert_eq!(read.drafts[0].part_number, "");
    assert_eq!(read.drafts[1].manufacturer, "");
}

// Allowed: the program boundary of a test, read once; the value never reaches a verdict.
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

/// The synthetic EDZ the other crates' tests read (`ew-api`, `ew`): one part in a 7-Zip and in a ZIP container,
/// made here from [`PSU`] (`EW_BLESS=1` writes them).
#[test]
fn the_synthetic_edz_fixtures_are_current() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for (name, bytes) in [
        (
            "synthetic-psu-7z.edz",
            seven(&[("2904602.part.xml", PSU.as_bytes())], false),
        ),
        (
            "synthetic-psu-zip.edz",
            zip_of(&[Entry::stored("2904602.part.xml", PSU.as_bytes())], None),
        ),
    ] {
        let path = dir.join(name);
        if bless() {
            std::fs::write(&path, &bytes).unwrap();
        }
        let held = std::fs::read(&path).unwrap_or_else(|e| panic!("{name}: {e} (EW_BLESS=1 writes it)"));
        assert!(held == bytes, "{name} is not this archive (EW_BLESS=1 writes it)");
        assert_eq!(read(name, &held).unwrap().drafts.len(), 1);
    }
}
