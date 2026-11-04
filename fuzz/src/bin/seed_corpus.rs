// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Writes the seed corpora of the generated targets into `fuzz/corpus/<target>/` (FT-297): small inputs made from
//! the golden project (no private data: the golden project is anonymised), deterministic, so a rerun gives the
//! same files. `cargo xtask settle` runs it (the step `fuzz-seeds`, from the repository root): `cargo run
//! --manifest-path fuzz/Cargo.toml --locked --bin seed_corpus -- <dir>`; an argument names the output directory
//! (without one, `fuzz/corpus`).

use std::path::{Path, PathBuf};

use ew_fuzz::golden;
use ew_fuzz::importers;
use ew_fuzz::loader::{self, MODE_COMPUTED_MANIFEST, MODE_TINY_LIMITS};
use ew_fuzz::open_path::{self, MODE_READ_ONLY, MODE_STALE_MANIFEST, OP_KINDS, Op};

/// The golden file of that name; the seeds name files by path.
fn file(path: &str) -> &'static [u8] {
    golden::files()
        .get(path)
        .unwrap_or_else(|| panic!("the golden project has no file {path}"))
}

/// The smallest golden file whose path starts with `prefix`.
fn smallest(prefix: &str) -> &'static str {
    golden::paths()
        .iter()
        .filter(|p| p.starts_with(prefix))
        .min_by_key(|p| file(p).len())
        .unwrap_or_else(|| panic!("the golden project has no file below {prefix}"))
}

fn write(dir: &Path, name: &str, bytes: &[u8]) {
    std::fs::create_dir_all(dir).expect("the corpus directory is created");
    std::fs::write(dir.join(name), bytes).unwrap_or_else(|e| panic!("writing {name}: {e}"));
}

fn loader_seeds(dir: &Path) {
    let computed = MODE_COMPUTED_MANIFEST;
    let roots = [
        "project.jsonl",
        "structure.jsonl",
        "revisions.jsonl",
        "report-blocks.jsonl",
    ];
    let pick = |paths: &[&str]| -> Vec<(String, &'static [u8])> {
        paths.iter().map(|p| ((*p).to_owned(), file(p))).collect()
    };
    let with = |mode: u8, paths: &[&str], manifest: Option<&[u8]>| -> Vec<u8> {
        let files = pick(paths);
        let refs: Vec<(&str, &[u8])> = files.iter().map(|(p, b)| (p.as_str(), *b)).collect();
        loader::encode(mode, &refs, manifest)
    };
    write(dir, "root-files", &with(computed, &roots, None));
    write(dir, "project-only", &with(computed, &roots[..1], None));
    let page = smallest("pages/");
    let shard = smallest("devices/");
    write(
        dir,
        "shard-and-page",
        &with(computed, &[shard, page, "structure.jsonl"], None),
    );
    write(
        dir,
        "library-documents",
        &with(
            computed,
            &[
                smallest("lib/fndefs/"),
                smallest("lib/forms/"),
                smallest("lib/parts/"),
                smallest("lib/symbols/"),
                "project.jsonl",
            ],
            None,
        ),
    );
    write(dir, "manifest-only", &with(0, &[], Some(file("manifest.json"))));
    write(
        dir,
        "stale-manifest",
        &with(
            0,
            &["revisions.jsonl", "structure.jsonl"],
            Some(file("manifest.json")),
        ),
    );
    write(
        dir,
        "tiny-limits",
        &with(computed | MODE_TINY_LIMITS, &roots[..2], None),
    );
    write(dir, "empty", &[0]);
    // The shape an older build wrote (the typed revisions, FT-165): the revision lines with their members in
    // byte-wise key order, next to the project.
    let old: Vec<u8> = file("revisions.jsonl")
        .split(|b| *b == b'\n')
        .filter(|l| !l.is_empty())
        .flat_map(|l| {
            let value: serde_json::Value = serde_json::from_slice(l).expect("a revision line");
            let mut line = serde_json::to_vec(&value).expect("a line");
            line.push(b'\n');
            line
        })
        .collect();
    write(
        dir,
        "old-shape-revisions",
        &loader::encode(
            computed,
            &[
                ("project.jsonl", file("project.jsonl")),
                ("revisions.jsonl", &old),
            ],
            None,
        ),
    );
    // WP-S2.04 part 4a: the smallest page with page items of every typed variant, a variant kept verbatim and a
    // structure box appended in line order (the page items, rank 43, then the box, rank 44, after the page's own
    // records), next to the project and the structure.
    let page_path = smallest("pages/");
    let page_bytes = file(page_path);
    let page_id = page_path
        .trim_start_matches("pages/")
        .trim_end_matches(".jsonl")
        .to_owned();
    // Line order (doc 05 §3.3): the items and the box go after the records of lower rank and before the page's
    // embedded report blocks (rank 45) and anything ranked after them.
    let later = |line: &[u8]| {
        let k = serde_json::from_slice::<serde_json::Value>(line)
            .ok()
            .and_then(|v| v.get("$k").and_then(|k| k.as_str().map(str::to_owned)));
        matches!(
            k.as_deref(),
            Some("report_block" | "macro_instance" | "placeholder_group")
        )
    };
    let lines: Vec<&[u8]> = page_bytes
        .split(|b| *b == b'\n')
        .filter(|l| !l.is_empty())
        .collect();
    let mut with_items = Vec::new();
    for line in lines.iter().filter(|l| !later(l)) {
        with_items.extend_from_slice(line);
        with_items.push(b'\n');
    }
    for line in page_item_lines(&page_id) {
        with_items.extend_from_slice(line.as_bytes());
        with_items.push(b'\n');
    }
    for line in lines.iter().filter(|l| later(l)) {
        with_items.extend_from_slice(line);
        with_items.push(b'\n');
    }
    write(
        dir,
        "page-items",
        &loader::encode(
            computed,
            &[
                ("project.jsonl", file("project.jsonl")),
                ("structure.jsonl", file("structure.jsonl")),
                (page_path, &with_items),
            ],
            None,
        ),
    );
}

/// The page items and the structure box of the `page-items` seed, in line order, on page `page`.
fn page_item_lines(page: &str) -> Vec<String> {
    let item = |n: u8, kind: &str| {
        format!(
            r#"{{"$k":"page_item","id":"5a0c1e2d-3b4f-5a61-8000-0000000000{n:02x}","page":"{page}","kind":{kind}}}"#
        )
    };
    vec![
        item(
            1,
            r#"{"graphic":{"shape":{"polyline":{"points":[[0,0],[1000000,0],[1000000,2000000]]}},"dash":"dashed"}}"#,
        ),
        item(
            2,
            r#"{"free_text":{"text":"40x60","at":[50800000,30480000],"angle":90000}}"#,
        ),
        item(
            3,
            r#"{"free_text":{"bind":"page.folio","at":[10000000,10000000],"align":"right"}}"#,
        ),
        item(
            4,
            r#"{"macro_box":{"key":"m1","outline":[[0,0],[1000000,0],[1000000,1000000]]}}"#,
        ),
        format!(
            r#"{{"$k":"structure_box","id":"5a0c1e2d-3b4f-5a61-8000-000000000005","page":"{page}","outline":[[0,0],[60000000,0],[60000000,30000000],[0,30000000]],"style":"dash_dot","caption":"Box","assigns":{{}}}}"#
        ),
    ]
}

fn open_path_seeds(dir: &Path) {
    let op = |file: &str, line: u8, op: u8, arg: u8| Op {
        file: golden::index_of(file),
        line,
        op,
        arg,
    };
    write(dir, "golden-as-it-is", &open_path::encode(0, &[]));
    write(
        dir,
        "golden-stale-manifest",
        &open_path::encode(MODE_STALE_MANIFEST, &[op("revisions.jsonl", 0, 0, 0)]),
    );
    write(dir, "golden-read-only", &open_path::encode(MODE_READ_ONLY, &[]));
    // The older build's shape: the revisions in byte-wise member order.
    write(
        dir,
        "old-shape-revisions",
        &open_path::encode(
            0,
            &[op("revisions.jsonl", 0, 0, 0), op("revisions.jsonl", 1, 0, 0)],
        ),
    );
    // The findings of the first hosted fuzz-smoke run (cycle 22), as libFuzzer wrote them: a BOM before a library
    // copy (a part copy, then the member order of a revision; edit mode), the same before a function definition
    // (stale manifest, read-only), and a swap of two lines inside a part copy (not a shape: it moves `sha256` into
    // `meta`).
    write(
        dir,
        "lib-document-bom",
        &[0x00, 0x75, 0x73, 0x65, 0x72, 0xa9, 0x01, 0x00, 0x00],
    );
    write(dir, "lib-document-bom-read-only", &[0xff, 0xff, 0xff, 0x65, 0x00]);
    write(
        dir,
        "lib-document-line-swap",
        &[0x2f, 0x73, 0x74, 0x61, 0x74, 0x65],
    );
    for kind in 0..OP_KINDS {
        // Revisions are leaf records: most changes to them still open (a change to the structure refuses to).
        let target = "revisions.jsonl";
        write(
            dir,
            &format!("op-{kind:02}"),
            &open_path::encode(0, &[op(target, 1, kind, 0x21)]),
        );
    }
}

/// A deterministic tape: an LCG stream.
fn tape(seed: u64, len: usize) -> Vec<u8> {
    let mut state = seed;
    (0..len)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 56) as u8
        })
        .collect()
}

fn command_seeds(dir: &Path) {
    // The seeds of `fuzz_smoke`, as plain seeded runs (a tape of zeros), and runs with a tape.
    write(dir, "seed-1", &ew_fuzz::command::encode(1, 8, &[]));
    write(dir, "seed-2", &ew_fuzz::command::encode(2, 8, &[]));
    write(dir, "seed-7-tape", &ew_fuzz::command::encode(7, 8, &tape(7, 192)));
    write(
        dir,
        "seed-13-tape",
        &ew_fuzz::command::encode(13, 8, &tape(13, 256)),
    );
    write(
        dir,
        "seed-21-short",
        &ew_fuzz::command::encode(21, 5, &tape(21, 48)),
    );
}

/// The helper the synthetic QET builder names (`crate::golden_common::repo`).
mod golden_common {
    pub fn repo() -> std::path::PathBuf {
        ew_fuzz::golden::repo()
    }
}

/// The synthetic QET project of Golden A's folios 101 to 103 (`crates/ew-api/tests/qet_synth`), built from the public
/// golden tables.
#[allow(dead_code, clippy::all, unused_imports)]
#[path = "../../../crates/ew-api/tests/qet_synth/mod.rs"]
mod qet_synth;

/// A small QET project: two elements with a wire, a folio-report pair over two folios, a terminal and a project UUID.
const SMALL_QET: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<project version="0.100" title="Seed" uuid="{0190d8a4-6d3e-7c41-8f00-0000000000a1}">
  <properties/>
  <diagram order="1" folio="1" title="A" cols="17" colsize="60" rows="8" rowsize="80" height="660">
    <elements>
      <element type="embed://import/simple.elmt" uuid="{00000000-0000-4000-8000-000000000001}" x="100" y="100" z="10" orientation="0" prefix="">
        <terminals><terminal x="0" y="-10" orientation="n" id="1"/><terminal x="0" y="10" orientation="s" id="2"/></terminals>
        <inputs/><elementInformations><elementInformation name="label" show="1">-1K1</elementInformation></elementInformations>
        <dynamic_texts/><texts_groups/><links_uuids/>
      </element>
      <element type="embed://import/next.elmt" uuid="{00000000-0000-4000-8000-000000000002}" x="100" y="200" z="10" orientation="0" prefix="">
        <terminals><terminal x="0" y="-10" orientation="n" id="3"/></terminals>
        <inputs/><elementInformations/><dynamic_texts/><texts_groups/>
        <links_uuids><link_uuid uuid="{00000000-0000-4000-8000-000000000003}"/></links_uuids>
      </element>
    </elements>
    <conductors>
      <conductor element1="{00000000-0000-4000-8000-000000000001}" terminal1="{00000000-0000-4000-9000-000000000001}" element2="{00000000-0000-4000-8000-000000000002}" terminal2="{00000000-0000-4000-9000-000100000000}" num="" function="" tension_protocol="" conductor_color="" conductor_section="" type="multi" x="0" y="0"/>
    </conductors>
  </diagram>
  <diagram order="2" folio="2" title="B" cols="17" colsize="60" rows="8" rowsize="80" height="660">
    <elements>
      <element type="embed://import/previous.elmt" uuid="{00000000-0000-4000-8000-000000000003}" x="100" y="100" z="10" orientation="0" prefix="">
        <terminals><terminal x="0" y="10" orientation="s" id="1"/></terminals>
        <inputs/><elementInformations/><dynamic_texts/><texts_groups/>
        <links_uuids><link_uuid uuid="{00000000-0000-4000-8000-000000000002}"/></links_uuids>
      </element>
      <element type="embed://import/terminal.elmt" uuid="{00000000-0000-4000-8000-000000000004}" x="200" y="100" z="10" orientation="0" prefix="">
        <terminals><terminal x="0" y="-10" orientation="n" id="2"/><terminal x="0" y="10" orientation="s" id="3"/></terminals>
        <inputs/><elementInformations><elementInformation name="label" show="1">-X1</elementInformation></elementInformations>
        <dynamic_texts/><texts_groups/><links_uuids/>
      </element>
    </elements>
    <conductors>
      <conductor element1="{00000000-0000-4000-8000-000000000003}" terminal1="{00000000-0000-4000-9000-000200000000}" element2="{00000000-0000-4000-8000-000000000004}" terminal2="{00000000-0000-4000-9000-000300000000}" num="" function="" tension_protocol="" conductor_color="" conductor_section="" type="multi" x="0" y="0"/>
    </conductors>
  </diagram>
  <collection><category name="import">
    <element name="simple.elmt"><definition type="element" version="0.100.0" link_type="simple" width="20" height="30" hotspot_x="10" hotspot_y="10"><uuid uuid="{00000000-0000-4000-8000-00000000e000}"/><names><name lang="en">simple</name></names><informations/><description><terminal x="0" y="-10" orientation="n" uuid="{00000000-0000-4000-9000-000000000000}" type="Generic"/><terminal x="0" y="10" orientation="s" uuid="{00000000-0000-4000-9000-000000000001}" type="Generic"/></description></definition></element>
    <element name="next.elmt"><definition type="element" version="0.100.0" link_type="next_report" width="20" height="20" hotspot_x="10" hotspot_y="10"><uuid uuid="{00000000-0000-4000-8000-00000000e001}"/><names><name lang="en">next</name></names><informations/><description><terminal x="0" y="-10" orientation="n" uuid="{00000000-0000-4000-9000-000100000000}" type="Generic"/></description></definition></element>
    <element name="previous.elmt"><definition type="element" version="0.100.0" link_type="previous_report" width="20" height="20" hotspot_x="10" hotspot_y="10"><uuid uuid="{00000000-0000-4000-8000-00000000e002}"/><names><name lang="en">previous</name></names><informations/><description><terminal x="0" y="10" orientation="s" uuid="{00000000-0000-4000-9000-000200000000}" type="Generic"/></description></definition></element>
    <element name="terminal.elmt"><definition type="element" version="0.100.0" link_type="terminal" width="20" height="30" hotspot_x="10" hotspot_y="10"><uuid uuid="{00000000-0000-4000-8000-00000000e003}"/><names><name lang="en">terminal</name></names><kindInformations><kindInformation name="type" show="1">generic</kindInformation></kindInformations><informations/><description><terminal x="0" y="-10" orientation="n" uuid="{00000000-0000-4000-9000-000300000000}" type="Generic"/><terminal x="0" y="10" orientation="s" uuid="{00000000-0000-4000-9000-000300000001}" type="Generic"/></description></definition></element>
  </category></collection>
</project>
"#;

fn qet_project_seeds(dir: &Path) {
    write(dir, "small-project", SMALL_QET.as_bytes());
    write(dir, "synthetic-101-103", qet_synth::build().text.as_bytes());
}

fn qet_element_seeds(dir: &Path) {
    let fixtures = golden::repo().join("crates/ew-import-qet/tests/fixtures");
    let mut names: Vec<PathBuf> = std::fs::read_dir(&fixtures)
        .expect("the element fixtures")
        .map(|e| e.expect("a directory entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "elmt"))
        .collect();
    names.sort();
    for path in names {
        let name = path
            .file_stem()
            .expect("a file name")
            .to_string_lossy()
            .into_owned();
        write(dir, &name, &std::fs::read(&path).expect("a readable fixture"));
    }
}

fn golden_tables_seeds(dir: &Path) {
    let table = |name: &str| -> Vec<u8> {
        std::fs::read(golden::repo().join("golden/J-E-2416/tables").join(name))
            .unwrap_or_else(|e| panic!("reading {name}: {e}"))
    };
    let seed = |lenient: bool, names: &[&str]| -> Vec<u8> {
        let tables: Vec<(u8, Vec<u8>)> = names
            .iter()
            .map(|n| (importers::table_index(n), table(n)))
            .collect();
        let refs: Vec<(u8, &[u8])> = tables.iter().map(|(i, b)| (*i, b.as_slice())).collect();
        importers::encode_tables(lenient, &refs)
    };
    write(
        dir,
        "project-pages",
        &seed(
            false,
            &["project.csv", "structure.csv", "external_refs.csv", "pages.csv"],
        ),
    );
    write(
        dir,
        "devices-functions-connections",
        &seed(
            true,
            &[
                "project.csv",
                "structure.csv",
                "external_refs.csv",
                "pages.csv",
                "devices.csv",
                "functions.csv",
                "connections.csv",
                "potentials.csv",
            ],
        ),
    );
    write(
        dir,
        "parts-lenient",
        &seed(
            true,
            &[
                "project.csv",
                "structure.csv",
                "external_refs.csv",
                "pages.csv",
                "parts.csv",
                "part_cps.csv",
            ],
        ),
    );
}

/// The supplier table of the parts seeds: the fixed map's headers, a split part number and a row without one.
const SUPPLIER: [[&str; 8]; 4] = [
    [
        "Manufacturer",
        "Part number",
        "Type",
        "Order number",
        "GTIN",
        "Description",
        "Kind",
        "Unit",
    ],
    [
        "Phoenix Contact",
        "3044102",
        "UT 4",
        "3044102",
        "4017918960445",
        "Feed-through terminal",
        "terminal",
        "piece",
    ],
    [
        "Siemens",
        "3RT2015-1BB41 / 3RT2015-1BB42",
        "3RT2015",
        "",
        "",
        "Contactor",
        "contactor",
        "piece",
    ],
    [
        "Weidmüller",
        "",
        "WDU 2.5",
        "",
        "",
        "Terminal",
        "terminal",
        "piece",
    ],
];

/// A ZIP of stored `files` (no compression), the smallest workbook container.
fn stored_zip(files: &[(&str, String)]) -> Vec<u8> {
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
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
    let u16_of = |n: usize| u16::try_from(n).expect("a small seed").to_le_bytes();
    let u32_of = |n: usize| u32::try_from(n).expect("a small seed").to_le_bytes();
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, body) in files {
        let offset = out.len();
        let header = |sig: u32, central: bool| {
            let mut h = sig.to_le_bytes().to_vec();
            if central {
                h.extend(20u16.to_le_bytes());
            }
            h.extend(20u16.to_le_bytes());
            h.extend([0u8; 8]); // flags, method (stored), time, date
            h.extend(crc32(body.as_bytes()).to_le_bytes());
            h.extend(u32_of(body.len()));
            h.extend(u32_of(body.len()));
            h.extend(u16_of(name.len()));
            h.extend([0u8; 2]);
            if central {
                h.extend([0u8; 8]); // comment, disk, internal and external attributes
                h.extend([0u8; 2]);
                h.extend(u32_of(offset));
            }
            h.extend(name.as_bytes());
            h
        };
        out.extend(header(0x0403_4b50, false));
        out.extend(body.as_bytes());
        central.extend(header(0x0201_4b50, true));
    }
    let (cd_offset, cd_size) = (out.len(), central.len());
    out.extend(central);
    out.extend(0x0605_4b50u32.to_le_bytes());
    out.extend([0u8; 4]);
    out.extend(u16_of(files.len()));
    out.extend(u16_of(files.len()));
    out.extend(u32_of(cd_size));
    out.extend(u32_of(cd_offset));
    out.extend([0u8; 2]);
    out
}

/// [`SUPPLIER`] as a one-sheet workbook with inline strings.
fn supplier_xlsx() -> Vec<u8> {
    let mut sheet = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#,
    );
    for (r, row) in SUPPLIER.iter().enumerate() {
        sheet += &format!(r#"<row r="{}">"#, r + 1);
        for (c, cell) in row.iter().enumerate().filter(|(_, c)| !c.is_empty()) {
            let column = char::from(b'A' + u8::try_from(c).expect("eight columns"));
            sheet += &format!(
                r#"<c r="{column}{}" t="inlineStr"><is><t>{cell}</t></is></c>"#,
                r + 1
            );
        }
        sheet += "</row>";
    }
    sheet += "</sheetData></worksheet>";
    stored_zip(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>"#.to_owned(),
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.to_owned(),
        ),
        (
            "xl/workbook.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Parts" sheetId="1" r:id="rId1"/></sheets></workbook>"#.to_owned(),
        ),
        (
            "xl/_rels/workbook.xml.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#.to_owned(),
        ),
        ("xl/worksheets/sheet1.xml", sheet),
    ])
}

fn parts_table_seeds(dir: &Path) {
    let csv: String = SUPPLIER
        .iter()
        .map(|row| {
            let cells: Vec<String> = row
                .iter()
                .map(|c| {
                    if c.contains(['/', ' ']) {
                        format!("\"{c}\"")
                    } else {
                        (*c).to_owned()
                    }
                })
                .collect();
            cells.join(",") + "\n"
        })
        .collect();
    write(dir, "supplier-csv", csv.as_bytes());
    write(dir, "supplier-xlsx", &supplier_xlsx());
}

/// The `.ewlib` reader's seeds (WP-S6.04, ID-1354), from the sample parts library of `content/libraries/`: its
/// manifest with the manufacturers and one parts shard, three shards under a computed manifest, the same with another
/// session's lock, and a lock record alone.
fn ewlib_read_seeds(dir: &Path) {
    use ew_fuzz::ewlib_read::{
        LOCK_SELECTOR, MANIFEST_SELECTOR, MODE_COMPUTED_MANIFEST, encode, selector_of,
    };
    let lib = golden::repo().join("content/libraries/ew-sample-parts.ewlib");
    let read = |rel: &str| std::fs::read(lib.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
    let manifest = read("library.json");
    let section = |rel: &str| -> (u8, Vec<u8>) {
        let (selector, shard, generation) =
            selector_of(rel).unwrap_or_else(|| panic!("{rel} is a records file"));
        let mut bytes = vec![shard, generation];
        bytes.extend_from_slice(&read(rel));
        (selector, bytes)
    };
    let shards = ["manufacturers.2.jsonl", "parts/06.2.jsonl", "parts/07.2.jsonl"];
    let records: Vec<(u8, Vec<u8>)> = shards.iter().map(|p| section(p)).collect();
    let lock = b"{\"lock\":1,\"session\":\"0190f5e0-6c1d-7a42-8b00-00000000c0de\",\"user\":\"designer\",\"host\":\"cad-01\",\
\"pid\":4242,\"pid_started\":\"2026-03-02T08:00:00Z\",\"app_version\":\"ElektraWatt 0.1.0\",\
\"started_at\":\"2026-03-02T08:00:05Z\"}\n";
    let refs = |extra: &[(u8, &[u8])]| -> Vec<(u8, Vec<u8>)> {
        records
            .iter()
            .cloned()
            .chain(extra.iter().map(|(s, b)| (*s, b.to_vec())))
            .collect()
    };
    let enc = |mode: u8, sections: &[(u8, Vec<u8>)]| {
        let sections: Vec<(u8, &[u8])> = sections.iter().map(|(s, b)| (*s, b.as_slice())).collect();
        encode(mode, &sections)
    };
    write(
        dir,
        "sample-manifest",
        &enc(
            0,
            &[
                (MANIFEST_SELECTOR, manifest.clone()),
                records[0].clone(),
                records[1].clone(),
            ],
        ),
    );
    write(dir, "sample-computed", &enc(MODE_COMPUTED_MANIFEST, &refs(&[])));
    write(
        dir,
        "sample-computed-locked",
        &enc(MODE_COMPUTED_MANIFEST, &refs(&[(LOCK_SELECTOR, lock)])),
    );
    write(dir, "lock-only", &enc(0, &[(LOCK_SELECTOR, lock.to_vec())]));
}

fn main() {
    let out: PathBuf = std::env::args_os()
        .nth(1)
        .map_or_else(|| golden::repo().join("fuzz").join("corpus"), PathBuf::from);
    loader_seeds(&out.join("jsonl_loader"));
    open_path_seeds(&out.join("migration_chain"));
    command_seeds(&out.join("command_sequence"));
    qet_project_seeds(&out.join("qet_project"));
    qet_element_seeds(&out.join("qet_element"));
    golden_tables_seeds(&out.join("golden_tables"));
    parts_table_seeds(&out.join("parts_table"));
    ewlib_read_seeds(&out.join("ewlib_read"));
    println!("seed corpora written to {}", out.display());
}
