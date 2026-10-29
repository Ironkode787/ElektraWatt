// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.19 part 4, the parts import (IM-1 to IM-6 at the importer's level; the saved map, the library write and the
//! CLI are tested in `ew-api` and `ew`). The parts are public catalogue data of the golden project's manufacturers
//! and invented generic parts.

use std::collections::BTreeMap;

use ew_import_catalog::{
    CatalogTarget, ColumnMap, PartsImport, RowStatus, SourceFormat, parse_parts, part_item, pn_norm,
};
use ew_interop_api::ew_ids::Uuid;
use ew_interop_api::ew_schema::{Date, LibCopy, LibDocument, Manufacturer, Part, PartKind};
use ew_interop_api::{Code, ImportError, ImportInput, Limits, Severity};
use serde_json::json;

/// A library of the master list, and the target library.
const MASTER: Uuid = Uuid::from_u128(0x7202fd8b_1024_4bf1_9e99_670772eaa589);
const TARGET: Uuid = Uuid::from_u128(0x0190a000_0000_7000_8000_0000000000a1);

fn manufacturer(key: &str, name: &str, aliases: &[&str]) -> LibCopy<Manufacturer> {
    let item = Uuid::new_v5(&MASTER, key.as_bytes());
    let doc = json!({
        "id": ew_interop_api::ew_ids::v5::lib_copy_id::<()>(MASTER, item).as_uuid().to_string(),
        "library_id": MASTER.to_string(),
        "item_id": item.to_string(),
        "version": "1.0.0",
        "sha256": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "key": key,
        "name": name,
        "aliases": aliases,
    });
    serde_json::from_value(doc).expect("a manufacturer copy")
}

fn master() -> Vec<LibCopy<Manufacturer>> {
    vec![
        manufacturer("SE", "Schneider Electric", &["Schneider"]),
        manufacturer("PXC", "Phoenix Contact", &[]),
    ]
}

/// A supplier list with German headers, `;`-separated, with a byte-order mark: one row per case of the report.
const LIST: &str = "\u{feff}Hersteller;Artikelnummer;Typ;Bezeichnung;Art;Nennstrom;Einheit;Bemerkung\r
Schneider;A9P52610;iC40N 1P+N C10;Leitungsschutzschalter;device;10 A;;Lager\r
Schneider Electric;A9P52616 / A9P52620;iC40N 1P+N C16;Leitungsschutzschalter;;16 A;;\r
Phoenix Contact;3209510;PT 2,5;Durchgangsklemme;terminal;;;\r
;;;;;;;\r
phoenix  contact;3209510;PT 2,5;Durchgangsklemme;Terminal;;;\r
Phoenix Contact;3209510;PT 2,5;Andere Klemme;terminal;;;\r
Generic Panels;GP-100;;Baustein;gizmo;;;\r
Phoenix Contact;;;Ohne Nummer;;;;\r
Phoenix Contact;2904602;QUINT4-PS;Stromversorgung;device;10 Amp;;\r
Example Wire Co;EW-1x1,5;;Aderleitung;wire;;m;\r
";

fn map() -> ColumnMap {
    ColumnMap::from_toml(
        r#"
[columns]
manufacturer = "Hersteller"
part_number = "Artikelnummer"
type_designation = "Typ"
"description.de" = "Bezeichnung"
kind = "Art"
"tech_data.i_n" = "Nennstrom"
unit = "Einheit"
"#,
    )
    .expect("the map")
}

fn date() -> Date {
    "2026-10-29".parse().unwrap()
}

fn import_bytes(name: &str, bytes: Vec<u8>, target: &CatalogTarget<'_>) -> Result<PartsImport, ImportError> {
    let input = ImportInput::new(name, bytes).unwrap();
    parse_parts(&input, &map(), &master(), target)
}

fn target() -> CatalogTarget<'static> {
    CatalogTarget {
        as_of: Some(date()),
        ..CatalogTarget::new(TARGET)
    }
}

fn part(doc: &LibDocument) -> &LibCopy<Part> {
    match doc {
        LibDocument::Part(c) => c,
        other => panic!("not a part: {:?}", other.kind()),
    }
}

/// `(line, part number, status)` per row outcome, as text.
fn statuses(i: &PartsImport) -> Vec<(u32, String, String)> {
    i.rows
        .iter()
        .map(|r| {
            let s = match &r.status {
                RowStatus::New => "new".to_owned(),
                RowStatus::Updated { from, version } => format!("updated {from} -> {version}"),
                RowStatus::Unchanged => "unchanged".to_owned(),
                RowStatus::Duplicate { of_line } => format!("duplicate of {of_line}"),
                RowStatus::Rejected(why) => format!("rejected: {why}"),
            };
            (r.line, r.part_number.clone(), s)
        })
        .collect()
}

/// IM-1, IM-3, IM-4, IM-5: a list with other headers imports through a map; every row's outcome.
#[test]
fn a_mapped_list_imports_with_a_report_of_every_row() {
    let i = import_bytes("liste.csv", LIST.as_bytes().to_vec(), &target()).unwrap();
    let rows = statuses(&i);
    let want: Vec<(u32, &str, &str)> = vec![
        (2, "A9P52610", "new"),
        (3, "A9P52616", "new"),
        (3, "A9P52620", "new"),
        (4, "3209510", "new"),
        (6, "3209510", "duplicate of 4"),
        (
            7,
            "3209510",
            "rejected: same manufacturer and part number as line 4, with other content",
        ),
        (8, "GP-100", "rejected: column `Art`: unknown kind `gizmo`"),
        (9, "", "rejected: no part number"),
        (
            10,
            "2904602",
            "rejected: column `Nennstrom`: `10 Amp`: `Amp` is not a unit of the format",
        ),
        (11, "EW-1x1,5", "new"),
    ];
    let got: Vec<(u32, &str, &str)> = rows
        .iter()
        .map(|(l, p, s)| (*l, p.as_str(), s.as_str()))
        .collect();
    assert_eq!(got, want);
    let c = i.counts();
    assert_eq!((c.new, c.duplicate, c.rejected), (5, 1, 4));

    // IM-3: "A / B" is two parts, each flagged, and the report says so; never one number.
    let split: Vec<_> = i.rows.iter().filter(|r| r.candidate).collect();
    assert_eq!(split.len(), 2);
    let flagged: Vec<_> = i
        .report
        .with_code(Code::imp(5))
        .map(|e| e.message.clone())
        .collect();
    assert!(
        flagged
            .iter()
            .any(|m| m.contains("`A9P52616 / A9P52620` names 2 parts")
                && m.contains("`A9P52616` and `A9P52620`")),
        "{flagged:?}"
    );
    assert!(
        !i.parts
            .iter()
            .any(|d| pn_norm(part(d).item.part_number.as_ref()) == "A9P52616A9P52620"),
        "no joined number"
    );

    // IM-4: "Schneider" and "Schneider Electric" are one manufacturer, the alias reported once; "phoenix  contact"
    // folds onto Phoenix Contact; a name the master lacks becomes a new manufacturer of the target.
    let aliases: Vec<_> = i.report.with_code(Code::ALIAS_MAPPED).collect();
    assert_eq!(aliases.len(), 1);
    assert_eq!(
        aliases[0].message,
        "manufacturer `Schneider` mapped to Schneider Electric (SE) through its aliases"
    );
    let se = master()[0].item_id.as_uuid();
    let by_pn: BTreeMap<String, &LibCopy<Part>> = i
        .parts
        .iter()
        .map(|d| (part(d).item.part_number.to_string(), part(d)))
        .collect();
    for pn in ["A9P52610", "A9P52616", "A9P52620"] {
        let p = by_pn[pn];
        assert_eq!(p.item.manufacturer.item.as_uuid(), se, "{pn}");
        assert_eq!(p.item.manufacturer.library.as_uuid(), MASTER);
        assert_eq!(p.item.manufacturer.version_req, "^1");
        assert_eq!(p.item_id.as_uuid(), part_item(TARGET, "SE", &pn_norm(pn)));
    }
    assert_eq!(i.manufacturers.len(), 1);
    let LibDocument::Manufacturer(new) = &i.manufacturers[0] else {
        panic!("a manufacturer");
    };
    assert_eq!(new.item.key.to_string(), "EXAMPLEWIRECO");
    assert_eq!(new.library_id.as_uuid(), TARGET);
    assert_eq!(by_pn["EW-1x1,5"].item.manufacturer.item, new.item_id);
    assert!(
        i.report.with_code(Code::imp(5)).any(|e| e
            .message
            .starts_with("manufacturer `Example Wire Co` is not in the master list")),
        "the new manufacturer is reported"
    );

    // IM-5: each rejection is an error entry with its line and reason.
    let errors: Vec<_> = i
        .report
        .entries
        .iter()
        .filter(|e| e.severity == Severity::Error)
        .map(|e| (e.location.as_ref().and_then(|l| l.line), e.message.clone()))
        .collect();
    assert_eq!(errors.len(), 4);
    assert_eq!(
        errors[1],
        (Some(8), "row rejected: column `Art`: unknown kind `gizmo`".into())
    );

    // The unmapped column is reported, not guessed.
    assert!(
        i.report
            .with_code(Code::DROPPED_BY_DESIGN)
            .any(|e| e.message == "the column `Bemerkung` is not mapped: its values are not imported")
    );

    // The members of one part.
    let mcb = by_pn["A9P52610"];
    assert_eq!(mcb.version, "1.0.0");
    assert_eq!(
        serde_json::to_value(&mcb.item).unwrap(),
        json!({
            "meta": {"key": "se.a9p52610", "sources": ["liste.csv"], "as_of": "2026-10-29"},
            "manufacturer": {"library": MASTER.to_string(), "item": se.to_string(), "version_req": "^1", "key": "SE"},
            "part_number": "A9P52610",
            "type_designation": "iC40N 1P+N C10",
            "description": {"de": "Leitungsschutzschalter"},
            "tech_data": [{"key": "i_n", "value": {"value": "10", "unit": "A"}}],
        })
    );
    assert_eq!(by_pn["3209510"].item.kind, PartKind::Terminal);
    assert_eq!(
        serde_json::to_value(&by_pn["EW-1x1,5"].item.unit).unwrap(),
        json!("m")
    );
    // Each copy is valid and hashes its item.
    for d in &i.parts {
        d.validate().unwrap();
    }
}

/// IM-1 at this level: the same list with English headers, comma-separated, through another map, gives the same
/// parts.
#[test]
fn another_header_dialect_through_its_own_map_gives_the_same_parts() {
    let english = "Maker,Article,Type,Text DE,Kind,In,Unit\n\
                   Schneider,A9P52610,iC40N 1P+N C10,Leitungsschutzschalter,device,10 A,\n";
    let map = ColumnMap::new(
        None,
        [
            ("manufacturer", "Maker"),
            ("part_number", "Article"),
            ("type_designation", "Type"),
            ("description.de", "Text DE"),
            ("kind", "Kind"),
            ("tech_data.i_n", "In"),
            ("unit", "Unit"),
        ],
    )
    .unwrap();
    let input = ImportInput::new("liste.csv", english.as_bytes().to_vec()).unwrap();
    let a = parse_parts(&input, &map, &master(), &target()).unwrap();
    let b = import_bytes("liste.csv", LIST.as_bytes().to_vec(), &target()).unwrap();
    assert_eq!(a.parts[0], b.parts[0]);
}

// -------------------------------------------------------------------------------------------------------------------
// A minimal XLSX writer for the tests: a stored (uncompressed) ZIP with the workbook parts calamine reads.

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for b in bytes {
        crc ^= u32::from(*b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn zip(files: &[(&str, String)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, body) in files {
        let offset = out.len() as u32;
        let crc = crc32(body.as_bytes());
        let size = body.len() as u32;
        let header = |sig: u32, central: bool| {
            let mut h = Vec::new();
            h.extend(sig.to_le_bytes());
            if central {
                h.extend(20u16.to_le_bytes());
            }
            h.extend(20u16.to_le_bytes()); // version needed
            h.extend(0u16.to_le_bytes()); // flags
            h.extend(0u16.to_le_bytes()); // stored
            h.extend(0u32.to_le_bytes()); // time, date
            h.extend(crc.to_le_bytes());
            h.extend(size.to_le_bytes());
            h.extend(size.to_le_bytes());
            h.extend((name.len() as u16).to_le_bytes());
            h.extend(0u16.to_le_bytes()); // extra
            if central {
                h.extend(0u16.to_le_bytes()); // comment
                h.extend(0u16.to_le_bytes()); // disk
                h.extend(0u16.to_le_bytes()); // internal attributes
                h.extend(0u32.to_le_bytes()); // external attributes
                h.extend(offset.to_le_bytes());
            }
            h.extend(name.as_bytes());
            h
        };
        out.extend(header(0x0403_4b50, false));
        out.extend(body.as_bytes());
        central.extend(header(0x0201_4b50, true));
    }
    let cd_offset = out.len() as u32;
    let cd_size = central.len() as u32;
    out.extend(central);
    out.extend(0x0605_4b50u32.to_le_bytes());
    out.extend([0u8; 4]);
    out.extend((files.len() as u16).to_le_bytes());
    out.extend((files.len() as u16).to_le_bytes());
    out.extend(cd_size.to_le_bytes());
    out.extend(cd_offset.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
}

fn column(i: usize) -> String {
    let mut n = i + 1;
    let mut s = Vec::new();
    while n > 0 {
        s.push(b'A' + ((n - 1) % 26) as u8);
        n = (n - 1) / 26;
    }
    s.reverse();
    String::from_utf8(s).unwrap()
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// A cell: text, a number, or a formula with its cached text.
enum X<'a> {
    T(&'a str),
    N(f64),
    F(&'a str, &'a str),
}

fn sheet_xml(rows: &[Vec<X<'_>>]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#,
    );
    for (r, row) in rows.iter().enumerate() {
        xml += &format!(r#"<row r="{}">"#, r + 1);
        for (c, cell) in row.iter().enumerate() {
            let at = format!("{}{}", column(c), r + 1);
            xml += &match cell {
                X::T("") => continue,
                X::T(t) => format!(r#"<c r="{at}" t="inlineStr"><is><t>{}</t></is></c>"#, escape(t)),
                X::N(n) => format!(r#"<c r="{at}"><v>{n}</v></c>"#),
                X::F(f, v) => format!(
                    r#"<c r="{at}" t="str"><f>{}</f><v>{}</v></c>"#,
                    escape(f),
                    escape(v)
                ),
            };
        }
        xml += "</row>";
    }
    xml + "</sheetData></worksheet>"
}

fn xlsx(sheets: &[(&str, Vec<Vec<X<'_>>>)]) -> Vec<u8> {
    let mut files = vec![
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>"#.to_owned(),
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.to_owned(),
        ),
    ];
    let mut book = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>"#,
    );
    let mut rels = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    let mut bodies = Vec::new();
    for (i, (name, rows)) in sheets.iter().enumerate() {
        book += &format!(
            r#"<sheet name="{name}" sheetId="{}" r:id="rId{}"/>"#,
            i + 1,
            i + 1
        );
        rels += &format!(
            r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{}.xml"/>"#,
            i + 1,
            i + 1
        );
        bodies.push((format!("xl/worksheets/sheet{}.xml", i + 1), sheet_xml(rows)));
    }
    book += "</sheets></workbook>";
    rels += "</Relationships>";
    files.push(("xl/workbook.xml", book));
    files.push(("xl/_rels/workbook.xml.rels", rels));
    let named: Vec<(&str, String)> = files
        .into_iter()
        .chain(bodies.iter().map(|(n, b)| (n.as_str(), b.clone())))
        .collect();
    zip(&named)
}

/// The list of [`LIST`] as workbook rows: part numbers as numbers where a spreadsheet keeps them so, one type
/// designation as a formula with its cached value.
fn list_rows() -> Vec<Vec<X<'static>>> {
    use X::{F, N, T};
    vec![
        vec![
            T("Hersteller"),
            T("Artikelnummer"),
            T("Typ"),
            T("Bezeichnung"),
            T("Art"),
            T("Nennstrom"),
            T("Einheit"),
            T("Bemerkung"),
        ],
        vec![
            T("Schneider"),
            T("A9P52610"),
            F("\"iC40N 1P+N \"&\"C10\"", "iC40N 1P+N C10"),
            T("Leitungsschutzschalter"),
            T("device"),
            T("10 A"),
            T(""),
            T("Lager"),
        ],
        vec![
            T("Schneider Electric"),
            T("A9P52616 / A9P52620"),
            T("iC40N 1P+N C16"),
            T("Leitungsschutzschalter"),
            T(""),
            T("16 A"),
        ],
        vec![
            T("Phoenix Contact"),
            N(3_209_510.0),
            T("PT 2,5"),
            T("Durchgangsklemme"),
            T("terminal"),
        ],
        vec![],
        vec![
            T("phoenix  contact"),
            N(3_209_510.0),
            T("PT 2,5"),
            T("Durchgangsklemme"),
            T("Terminal"),
        ],
        vec![
            T("Phoenix Contact"),
            N(3_209_510.0),
            T("PT 2,5"),
            T("Andere Klemme"),
            T("terminal"),
        ],
        vec![T("Generic Panels"), T("GP-100"), T(""), T("Baustein"), T("gizmo")],
        vec![T("Phoenix Contact"), T(""), T(""), T("Ohne Nummer")],
        vec![
            T("Phoenix Contact"),
            N(2_904_602.0),
            T("QUINT4-PS"),
            T("Stromversorgung"),
            T("device"),
            T("10 Amp"),
        ],
        vec![
            T("Example Wire Co"),
            T("EW-1x1,5"),
            T(""),
            T("Aderleitung"),
            T("wire"),
            T(""),
            T("m"),
        ],
    ]
}

/// The parts with the source file's name taken out (it names the file), and the report without the file.
/// A report entry without its file: code, severity, message and line.
type Entry = (Code, Severity, String, Option<u32>);

fn comparable(i: &PartsImport) -> (Vec<LibDocument>, Vec<Entry>) {
    let parts = i
        .parts
        .iter()
        .map(|d| {
            let mut c = part(d).clone();
            c.item.meta.sources.clear();
            c.sha256 = ew_interop_api::ew_schema::Sha256([0; 32]);
            LibDocument::Part(c)
        })
        .collect();
    let report = i
        .report
        .entries
        .iter()
        .map(|e| {
            (
                e.code,
                e.severity,
                e.message.clone(),
                e.location.as_ref().and_then(|l| l.line),
            )
        })
        .collect();
    (parts, report)
}

/// IM-2: the same list as XLSX gives the same parts, rows and report; a formula's cached value is read.
#[test]
fn the_same_list_as_xlsx_gives_the_same_parts_and_report() {
    let book = xlsx(&[("Teile", list_rows())]);
    assert_eq!(SourceFormat::detect(&book), SourceFormat::Xlsx);
    let x = import_bytes("liste.xlsx", book, &target()).unwrap();
    let c = import_bytes("liste.csv", LIST.as_bytes().to_vec(), &target()).unwrap();
    assert_eq!(x.report.source.format, "xlsx");
    assert_eq!(c.report.source.format, "csv");
    assert_eq!(statuses(&x), statuses(&c));
    assert_eq!(comparable(&x), comparable(&c));
    assert_eq!(x.manufacturers, c.manufacturers);
}

/// A named sheet is read; a missing one refuses the file.
#[test]
fn a_named_sheet_is_read_and_a_missing_one_refuses_the_file() {
    let book = xlsx(&[("Notes", vec![vec![X::T("nothing")]]), ("Teile", list_rows())]);
    let mut m = map();
    m.sheet = Some("Teile".into());
    let input = ImportInput::new("liste.xlsx", book.clone()).unwrap();
    let i = parse_parts(&input, &m, &master(), &target()).unwrap();
    assert_eq!(i.counts().new, 5);
    // The first sheet has no mapped columns.
    let first = parse_parts(&input, &map(), &master(), &target()).unwrap_err();
    assert!(first.to_string().contains("is not in the table"), "{first}");
    m.sheet = Some("Parts".into());
    let err = parse_parts(&input, &m, &master(), &target()).unwrap_err();
    assert!(err.to_string().contains("no sheet named `Parts`"), "{err}");
}

/// IM-6 at this level: a second import of the changed list against the held parts keeps the item IDs, reports the
/// unchanged and updated rows, keeps the held version of a change (which a library refuses) unless bumped.
#[test]
fn a_second_import_updates_in_place() {
    let first = import_bytes("liste.csv", LIST.as_bytes().to_vec(), &target()).unwrap();
    let held: BTreeMap<Uuid, LibCopy<Part>> = first
        .parts
        .iter()
        .map(|d| (part(d).item_id.as_uuid(), part(d).clone()))
        .collect();
    let changed = LIST.replace(
        "Durchgangsklemme;terminal;;;\r\n;",
        "Durchgangsklemme grau;terminal;;;\r\n;",
    );
    let existing = |item: Uuid| held.get(&item).cloned();
    let later = CatalogTarget {
        as_of: Some("2026-11-02".parse().unwrap()),
        existing: &existing,
        ..CatalogTarget::new(TARGET)
    };
    let second = import_bytes("liste-2.csv", changed.clone().into_bytes(), &later).unwrap();
    let rows = statuses(&second);
    assert_eq!(rows[0].2, "unchanged");
    assert_eq!(rows[3], (4, "3209510".into(), "updated 1.0.0 -> 1.0.0".into()));
    assert_eq!(second.needs_bump().len(), 1);
    // Unchanged parts are the held documents (the old date and source); the changed one has the new date.
    assert_eq!(second.parts[0], first.parts[0]);
    let terminal = part(&second.parts[3]);
    assert_eq!(terminal.item_id, part(&first.parts[3]).item_id, "same item ID");
    assert_eq!(terminal.id, part(&first.parts[3]).id, "same copy ID");
    assert_eq!(terminal.item.meta.as_of, Some("2026-11-02".parse().unwrap()));
    // With the bump the change gets the next patch version.
    let bumped = CatalogTarget { bump: true, ..later };
    let third = import_bytes("liste-2.csv", changed.into_bytes(), &bumped).unwrap();
    assert_eq!(statuses(&third)[3].2, "updated 1.0.0 -> 1.0.1");
    assert!(third.needs_bump().is_empty());
    assert_eq!(part(&third.parts[3]).version, "1.0.1");
}

/// The input limit is checked before parsing; a workbook that declares more unpacked bytes than the limit is
/// refused before it is opened.
#[test]
fn oversized_inputs_are_refused_before_parsing() {
    let small = CatalogTarget {
        limits: Limits {
            max_input_bytes: 64,
            ..Limits::default()
        },
        ..CatalogTarget::new(TARGET)
    };
    let err = import_bytes("liste.csv", LIST.as_bytes().to_vec(), &small).unwrap_err();
    assert_eq!(
        err,
        ImportError::LimitExceeded {
            what: "input size".into(),
            limit: 64
        }
    );
    let book = xlsx(&[("Teile", list_rows())]);
    let packed = book.len() as u64;
    let tight = CatalogTarget {
        limits: Limits {
            max_input_bytes: packed,
            ..Limits::default()
        },
        ..CatalogTarget::new(TARGET)
    };
    // Stored entries unpack to about their packed size minus the headers: under the limit.
    assert!(import_bytes("liste.xlsx", book.clone(), &tight).is_ok());
    // Declare a huge entry: refused from the directory alone.
    let mut lying = book;
    let cd = lying
        .windows(4)
        .position(|w| w == [0x50, 0x4b, 0x01, 0x02])
        .unwrap();
    lying[cd + 24..cd + 28].copy_from_slice(&0x7fff_ffffu32.to_le_bytes());
    let err = import_bytes("liste.xlsx", lying, &tight).unwrap_err();
    assert!(
        matches!(&err, ImportError::LimitExceeded { what, .. } if what == "unpacked workbook size"),
        "{err}"
    );
}

/// A map whose part-number column the table lacks refuses the file; a missing optional column is a warning.
#[test]
fn a_map_that_does_not_fit_the_table() {
    let text = "Maker,Article\nSchneider,A9P52610\n";
    let input = ImportInput::new("x.csv", text.as_bytes().to_vec()).unwrap();
    let err = parse_parts(&input, &map(), &master(), &target()).unwrap_err();
    assert!(
        err.to_string()
            .contains("`Hersteller` (manufacturer) is not in the table"),
        "{err}"
    );
    let m = ColumnMap::new(
        None,
        [
            ("manufacturer", "Maker"),
            ("part_number", "Article"),
            ("gtin", "EAN"),
        ],
    )
    .unwrap();
    let i = parse_parts(&input, &m, &master(), &target()).unwrap();
    assert_eq!(i.counts().new, 1);
    assert!(
        i.report.entries.iter().any(
            |e| e.severity == Severity::Warning && e.message.contains("`EAN` (gtin) is not in the table")
        )
    );
}

/// The same bytes give the same result.
#[test]
fn the_import_is_deterministic() {
    let a = import_bytes("liste.csv", LIST.as_bytes().to_vec(), &target()).unwrap();
    let b = import_bytes("liste.csv", LIST.as_bytes().to_vec(), &target()).unwrap();
    assert_eq!(a, b);
}
