// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The checks of the acceptance runner (doc 20 §21 AC-01 … AC-13, AC-18, AC-20; §19 rows gated M1).
//!
//! Every entry says what it compared and what is left out. A criterion that cannot be evaluated here is
//! reported "not applicable at M1 headless" with the missing piece, never skipped.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use ew_derive::{DeviceTag, PageOrder, PotentialNameOf, PotentialPartition, RepAnchorCell, XrefTextOf};
use ew_ids::Id;
use ew_scene::page::page_scene;
use ew_schema::{Device, Representation};
use sha2::{Digest, Sha256};

use super::report::{Outcome, Status, fail, na, pass};
use super::runner::{Ctx, Runner};
use crate::golden_common::{self, all_props, repo, rows_csv, rows_of, run_log, tags, texts, xrefs};
use crate::golden_files::{decode_rgba, differing_percent, frame_figure_errors, frame_json, page_png};
use crate::pdf_support::{structure_text, xref_counts};

/// The folios of doc 20 §5 in page order (36 pages): the stored pages, the dividers and the generated report
/// pages with their continuation folios.
const SECTION_5_FOLIOS: [&str; 36] = [
    "1", "2", "2.1", "3", "100", "101", "102", "103", "106", "200", "201", "202", "203", "204", "205", "206",
    "900", "901", "901.1", "902", "903", "904", "905", "906", "907", "907.1", "907.2", "908", "908.1",
    "908.2", "909", "910", "910.1", "910.2", "910.3", "911",
];

/// The folios of AC-02: 1, 3, 100–106, 200–206 and 900 (the stored and divider pages of that range).
const AC_02_FOLIOS: [&str; 15] = [
    "1", "3", "100", "101", "102", "103", "106", "200", "201", "202", "203", "204", "205", "206", "900",
];

/// The title-block cells AC-04 binds on every page (doc 20 §17.2): the exact text each prints.
const TITLE_BLOCK_VALUES: [&str; 6] = [
    "Revision: V2.0",
    "=P01",
    "+J-E-2416",
    "Object: J-E-2416",
    "Creation: 03-03-2025",
    "Saved: 14-04-2025",
];

/// The expected CSV of the report rows (`expected/csv/<name>.csv`) and the report block that derives them.
pub const ROW_PAIRS: [(&str, &str); 6] = [
    ("terminals_1X1", "903"),
    ("terminals_X1031", "904"),
    ("terminals_X1032", "905"),
    ("cables", "906"),
    ("wires", "907"),
    ("potentials", "909"),
];

pub fn all(r: &mut Runner) {
    // AC-01 … AC-13
    r.entry(
        "AC-01",
        "frame, grid and title-block geometry equals expected/geometry/frame.json",
        ac01_frame,
    );
    r.entry("AC-01", "the printed frame equals the figures of §17.1", |_| {
        let errors = frame_figure_errors();
        if errors.is_empty() {
            pass("outer frame, header band, column width, row height, first column edge and drawing area within 0.01 mm")
        } else {
            fail(errors.join("; "))
        }
    });
    r.entry(
        "AC-02",
        "every representation anchors in the cell of representations.csv",
        ac02_anchor_cells,
    );
    r.entry(
        "AC-02",
        "text of folios 1, 3, 100-106, 200-206, 900 equals expected/text",
        ac02_texts,
    );
    r.entry(
        "AC-02",
        "raster of those folios within 0.5 % of expected/png",
        ac02_raster,
    );
    r.entry("AC-03", "page order equals the 36 folios of §5", ac03_pages);
    r.entry(
        "AC-04",
        "every page prints the bound title-block values",
        ac04_title_blocks,
    );
    r.entry(
        "AC-05",
        "the interruption-point texts equal §15 (22 pairs, 44 texts)",
        ac05_ip_texts,
    );
    r.entry(
        "AC-05",
        "63 linked texts with 64 targets, 4 external texts, \"3-B2\"",
        ac05_counts,
    );
    r.entry(
        "AC-06",
        "every device tag equals its table key; no stale token in any page text",
        ac06_tags,
    );
    r.entry(
        "AC-07",
        "folio 901 holds the 27 rows of §14.1 over 901 and 901.1",
        ac07_bom,
    );
    r.entry("AC-07", "bom export --mode assembly equals §14.2", |_| {
        na("the assembly BOM and `ew export` do not exist yet (FR-EXP-005)")
    });
    r.entry(
        "AC-08",
        "folios 903-905 equal expected/csv, designations unique per strip",
        ac08_terminals,
    );
    r.entry(
        "AC-09",
        "folio 907 equals expected/csv/wires.csv: 44 wires, 6 bridges",
        ac09_wire_rows,
    );
    r.entry(
        "AC-09",
        "every wire has an effective size, colour and wire number",
        ac09_properties,
    );
    r.entry(
        "AC-10",
        "the potential partition holds the 19 potentials of §7.3 and bond B1",
        ac10_partition,
    );
    r.entry("AC-10", "folio 909 equals expected/csv/potentials.csv", |c| {
        row_pair(c, "potentials")
    });
    r.entry(
        "AC-11",
        "folio 906 equals expected/csv/cables.csv: 3 cables, 13 cores",
        ac11_cables,
    );
    r.entry("AC-11", "export cables equals §10", |_| {
        na("`ew export cables` does not exist yet (FR-EXP-005)")
    });
    r.entry(
        "AC-12",
        "folio 1 prints the colour and size tables generated from the profiles",
        ac12_tables,
    );
    r.entry(
        "AC-12",
        "changing the 24 V colour updates every affected wire and folio 907",
        ac12_colour,
    );
    r.entry(
        "AC-13",
        "PDF structure equals expected/pdf (outline, 64 links, page labels, fonts, SHA-256)",
        ac13_structure,
    );
    r.entry("AC-13", "PDF/A-2b passes veraPDF", |_| {
        na("veraPDF is not part of the headless harness; PDF/A output is not written yet (FR-OUT-007)")
    });

    // The SQL half of the acceptance pairs (doc 20 §3.1 `acceptance/*.sql`, doc 18 §3.10 step 2).
    for (id, name) in [
        ("AC-08", "terminals_1X1, terminals_X1031, terminals_X1032"),
        ("AC-09", "wires"),
        ("AC-10", "potentials"),
        ("AC-11", "cables"),
    ] {
        r.entry(id, &format!("acceptance SQL over the api_v1 views equals expected/csv: {name}"), |_| {
            na("not applicable: SQL projection pending (WP-S1.10); the CSV half above compares the derived report rows")
        });
    }

    // AC-18, AC-20: Golden B and the D-01 replay.
    r.entry("AC-18", "step B1: the ten tags of the tag map", |_| {
        fail(
            "variants/b-renumber.ewcmd does not exist: numbering.renumber of v0 renumbers no frozen tag and cannot \
             drop the strip rule (8 of the 10 tags are frozen, 2 are strips); see golden/J-E-2416/README.md",
        )
    });
    r.entry(
        "AC-18",
        "step B2: 2 tags and 14 cross-reference texts, same topology, undo restores the records",
        ac18_b2,
    );
    r.entry(
        "AC-20",
        "renumbering folio 3 to 4 updates -3W1, -3X1 and \"3-B2\" everywhere; undo restores",
        ac20_folio_3_to_4,
    );

    // The other criteria of gate G-M1 (doc 20 §22): reported, not run here.
    r.entry("AC-14", "byte-identical PDF, SVG, CSV and JSONL across serial, parallel and shuffled runs", |_| {
        na("not run by this runner: tests/page_output.rs holds the PDF across runs and record shuffles and tests/golden_fixture.rs the JSONL; SVG export and the parallel export do not exist yet")
    });
    r.entry("AC-15", "build paths (a), (b) and (c) agree", |_| {
        na("not run by this runner: (a) against (c) is tests/golden_fixture.rs; path (b), the builder DSL, does not exist yet")
    });
    r.entry("AC-16", "freshness: scripted edits update every output; verify-derived is clean with the oracle on", |_| {
        na("not run by this runner: the scripted edits (rename T55, move -102F2, folio 103 to 104, 24 V colour) need the tag and move commands and `ew verify-derived`; the oracle is on for every query of this run (see the oracle count)")
    });
    r.entry("AC-17", "open <= 0.5 s, check <= 2 s, PDF export <= 3 s", |_| {
        na("not measured (milestone rule): performance claims are made at milestone deliveries only")
    });
    r.entry(
        "AC-39",
        "the anonymisation guard passes on golden/, docs/ and expected/",
        |_| na("not run by this runner: `cargo xtask check-anonymisation` is the guard"),
    );

    // §19 rows gated M1.
    defects(r);
}

// ---------------------------------------------------------------------------------------------------------
// Helpers

/// Reads the committed expected file under `golden/J-E-2416/expected`.
fn expected(path: &str) -> Result<Vec<u8>, String> {
    let p = repo().join("golden/J-E-2416/expected").join(path);
    std::fs::read(&p).map_err(|e| format!("expected/{path}: {e}"))
}

/// A comparison with a committed file that never blesses.
fn same_as_expected(path: &str, bytes: &[u8]) -> Result<(), String> {
    if expected(path)? == bytes {
        Ok(())
    } else {
        Err(format!("expected/{path} differs from the generated output"))
    }
}

/// The rows of a CSV text (the golden dialect: quoted fields, doubled quotes, LF line ends).
fn csv_rows(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => row.push(std::mem::take(&mut field)),
            '\n' if !quoted => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            c => field.push(c),
        }
    }
    rows
}

/// The (layer, text, y in mm) of every glyph run of a text CSV.
fn runs(csv: &str) -> Vec<(String, String, f64)> {
    csv_rows(csv)
        .into_iter()
        .skip(1)
        .map(|r| (r[1].clone(), r[3].clone(), r[5].parse().unwrap_or(0.0)))
        .collect()
}

/// The ID of the record with source key `key`.
fn id_of<T>(c: &Ctx, key: &str) -> Id<T> {
    Id::from_str(c.ids.get(key).unwrap_or_else(|| panic!("no record `{key}`"))).unwrap()
}

fn list(items: impl IntoIterator<Item = String>, limit: usize) -> String {
    let all: Vec<String> = items.into_iter().collect();
    if all.len() <= limit {
        all.join(", ")
    } else {
        format!("{}, and {} more", all[..limit].join(", "), all.len() - limit)
    }
}

// ---------------------------------------------------------------------------------------------------------
// AC-01 … AC-13

fn ac01_frame(_: &Ctx) -> Outcome {
    match same_as_expected("geometry/frame.json", frame_json().as_bytes()) {
        Ok(()) => pass(
            "the two frame forms and the QET title block print the committed figures (exact, stricter than 0.2 mm)",
        ),
        Err(e) => fail(e),
    }
}

fn ac02_anchor_cells(c: &Ctx) -> Outcome {
    let rows = golden_common::table("representations.csv");
    let q = c.oracle.query();
    let mut wrong = Vec::new();
    let mut checked = 0;
    for r in &c.batch.records {
        let ew_schema::Record::Representation(rep) = r else {
            continue;
        };
        let key = golden_common::source_key(r)
            .and_then(|k| k.strip_prefix("representations|"))
            .expect("a source key");
        let want = &rows[key]["cell"];
        let got = q
            .get::<RepAnchorCell>(rep.id)
            .as_ref()
            .as_ref()
            .map(ToString::to_string);
        if got.as_deref() != Some(want.as_str()) {
            wrong.push(format!("{key}: table {want}, derived {got:?}"));
        }
        checked += 1;
    }
    if checked != rows.len() {
        return fail(format!("{checked} representations, {} table rows", rows.len()));
    }
    if wrong.is_empty() {
        pass(format!("all {checked} anchor cells equal their table cells"))
    } else {
        fail(format!("{} of {checked} off: {}", wrong.len(), list(wrong, 3)))
    }
}

fn ac02_texts(c: &Ctx) -> Outcome {
    let mut differing = Vec::new();
    for folio in AC_02_FOLIOS {
        let Some(csv) = c.text.get(folio) else {
            differing.push(format!("{folio} (not generated)"));
            continue;
        };
        if let Err(e) = same_as_expected(&format!("text/{folio}.csv"), csv.as_bytes()) {
            differing.push(format!("{folio}: {e}"));
        }
    }
    if differing.is_empty() {
        pass(format!(
            "{} folios: strings, anchors (3 decimals) and sizes equal the committed text CSVs",
            AC_02_FOLIOS.len()
        ))
    } else {
        fail(list(differing, 4))
    }
}

fn ac02_raster(c: &Ctx) -> Outcome {
    if !c.raster {
        return na("the raster comparison was switched off for this run (Options::raster)");
    }
    let mut worst: Option<(f64, String)> = None;
    for g in c
        .pages
        .iter()
        .filter(|g| AC_02_FOLIOS.contains(&g.entry.folio.as_str()))
    {
        let scene = page_scene(&g.view, ew_text::bundled());
        let bytes = page_png(&scene, &c.fonts);
        let committed = match expected(&format!("png/{}.png", g.entry.folio)) {
            Ok(b) => b,
            Err(e) => return fail(e),
        };
        let (w0, h0, a) = decode_rgba(&committed);
        let (w1, h1, b) = decode_rgba(&bytes);
        if (w0, h0) != (w1, h1) {
            return fail(format!("folio {}: size differs", g.entry.folio));
        }
        let pct = differing_percent(&a, &b, 16);
        if worst.as_ref().is_none_or(|w| pct > w.0) {
            worst = Some((pct, g.entry.folio.clone()));
        }
    }
    let worst = worst.expect("the AC-02 folios exist");
    if worst.0 <= 0.5 {
        pass(format!(
            "{} folios within the 0.5 % tolerance (worst {:.3} % on folio {}); the PNGs are candidates awaiting the FT-10 approval",
            AC_02_FOLIOS.len(),
            worst.0,
            worst.1
        ))
    } else {
        fail(format!(
            "folio {}: {:.3} % of the pixels differ",
            worst.1, worst.0
        ))
    }
}

fn ac03_pages(c: &Ctx) -> Outcome {
    let q = c.oracle.query();
    let derived: Vec<String> = q
        .get::<PageOrder>(())
        .entries
        .iter()
        .map(|e| e.folio.clone())
        .collect();
    if derived == SECTION_5_FOLIOS {
        return pass("36 folios in the order of §5; Prev/Next follow the order by construction");
    }
    let missing: Vec<String> = SECTION_5_FOLIOS
        .iter()
        .filter(|f| !derived.iter().any(|d| d == *f))
        .map(|f| (*f).to_owned())
        .collect();
    let extra: Vec<String> = derived
        .iter()
        .filter(|d| !SECTION_5_FOLIOS.contains(&d.as_str()))
        .cloned()
        .collect();
    fail(format!(
        "{} folios derived against 36 in §5; missing {}{} (901.1: the BOM rows 24-27 are not in the tables, FT-110; 908.1, 908.2: no PLC channel records, FT-111)",
        derived.len(),
        list(missing, 6),
        if extra.is_empty() {
            String::new()
        } else {
            format!("; unexpected {}", list(extra, 6))
        }
    ))
}

fn ac04_title_blocks(c: &Ctx) -> Outcome {
    let mut missing: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (folio, csv) in &c.text {
        let cells: BTreeSet<String> = runs(csv)
            .into_iter()
            .filter(|(layer, _, y)| layer == "frame" && *y > 180.0)
            .map(|(_, text, _)| text)
            .collect();
        for want in TITLE_BLOCK_VALUES {
            if !cells.contains(want) {
                missing.entry(want).or_default().push(folio);
            }
        }
    }
    if missing.is_empty() {
        pass(format!("all {} pages print the six bound values", c.text.len()))
    } else {
        fail(
            missing
                .iter()
                .map(|(want, folios)| format!("\"{want}\" missing on {} pages", folios.len()))
                .collect::<Vec<_>>()
                .join("; ")
                + " (the revision label is not bound yet; the TOC pages print no structure cells)",
        )
    }
}

/// The interruption-point table of doc 20 §15: (name, source folio, target folio, printed at source, printed at
/// target).
fn section_15_pairs() -> Vec<(String, String, String, String, String)> {
    let doc = std::fs::read_to_string(repo().join("docs/design/20-golden-project.md")).expect("doc 20");
    let start = doc
        .find("**Interruption points (22 pairs):**")
        .expect("the §15 table");
    let end = doc[start..]
        .find("**External connection points")
        .expect("its end")
        + start;
    doc[start..end]
        .lines()
        .filter(|l| l.starts_with("| ") && !l.starts_with("| Name") && !l.starts_with("|---"))
        .map(|l| {
            let cells: Vec<&str> = l.trim_matches('|').split('|').map(str::trim).collect();
            let folio = |cell: &str| cell.split('-').next().unwrap().to_owned();
            (
                cells[0].to_owned(),
                folio(cells[1]),
                folio(cells[2]),
                cells[3].to_owned(),
                cells[4].to_owned(),
            )
        })
        .collect()
}

fn ac05_ip_texts(c: &Ctx) -> Outcome {
    let q = c.memoised.query();
    let pairs = section_15_pairs();
    if pairs.len() != 22 {
        return fail(format!("{} pairs parsed from §15, 22 expected", pairs.len()));
    }
    let mut wrong = Vec::new();
    for (name, source, target, at_source, at_target) in &pairs {
        for (folio, want) in [(source, at_source), (target, at_target)] {
            let key = format!("representations|{name}@{folio}");
            let Some(id) = c.ids.get(&key) else {
                wrong.push(format!("{name}@{folio}: no such interruption point"));
                continue;
            };
            let got = q.get::<XrefTextOf>(Id::<Representation>::from_str(id).unwrap());
            if got.text != *want {
                wrong.push(format!("{name}@{folio}: \"{}\", §15 says \"{want}\"", got.text));
            }
        }
    }
    if wrong.is_empty() {
        pass("44 texts equal the \"Printed at source / target\" columns of §15")
    } else {
        fail(format!("{} of 44 differ: {}", wrong.len(), list(wrong, 3)))
    }
}

fn ac05_counts(c: &Ctx) -> Outcome {
    let (linked, targets, external) = xref_counts(&c.pages);
    let q = c.memoised.query();
    let main = q.get::<XrefTextOf>(id_of(c, "representations|-201U1"));
    let mut wrong = Vec::new();
    if (linked, targets, external) != (63, 64, 4) {
        wrong.push(format!(
            "{linked} linked texts with {targets} targets and {external} external, expected 63, 64, 4"
        ));
    }
    if main.text != "\u{221e} 3-B2, 106-G3" {
        wrong.push(format!(
            "-201U1 prints \"{}\", expected \"\u{221e} 3-B2, 106-G3\"",
            main.text
        ));
    }
    if wrong.is_empty() {
        pass(
            "63 linked texts with 64 link targets and 4 external texts on the drawn pages; -201U1 prints 3-B2 (the 19 main/auxiliary texts are checked by count and by the -201U1 text)",
        )
    } else {
        fail(wrong.join("; "))
    }
}

fn ac06_tags(c: &Ctx) -> Outcome {
    let q = c.oracle.query();
    let mut wrong = Vec::new();
    let mut checked = 0;
    for r in &c.batch.records {
        let ew_schema::Record::Device(dev) = r else {
            continue;
        };
        let key = golden_common::source_key(r).expect("a source key");
        let key = key
            .strip_prefix("devices|")
            .or_else(|| key.strip_prefix("terminals|"))
            .expect("a device key");
        let tag = q.get::<DeviceTag>(dev.id);
        let short = tag.as_ref().as_ref().map(|t| t.short());
        if short.as_deref() != Some(key) {
            wrong.push(format!("{key}: derived {short:?}"));
        }
        checked += 1;
    }
    let stale = ["-4W1", "-4X1", "T51", "JE2416"];
    let mut found = Vec::new();
    for (folio, csv) in &c.text {
        for (_, text, _) in runs(csv) {
            if stale.iter().any(|s| text.contains(s)) || text == "BR" {
                found.push(format!("{folio}: \"{text}\""));
            }
        }
    }
    if wrong.is_empty() && found.is_empty() {
        pass(format!(
            "{checked} devices print their table key; no \"-4W1\", \"-4X1\", \"T51\", \"JE2416\" or core colour \"BR\" in any page text (PDF text layer and per-occurrence identity not checked)"
        ))
    } else {
        fail(format!(
            "{} tags differ ({}); {} stale tokens ({})",
            wrong.len(),
            list(wrong, 3),
            found.len(),
            list(found, 3)
        ))
    }
}

fn ac07_bom(c: &Ctx) -> Outcome {
    let q = c.rows_q();
    let set = rows_of(&q, &c.batch, "901");
    let n = set.rows.len();
    let pages = q
        .get::<ew_reports::BlockPages>((golden_common::block(&c.batch, "901"), 0))
        .len();
    if n == 27 && pages == 2 {
        pass("27 rows over 901 and 901.1")
    } else {
        fail(format!(
            "{n} rows over {pages} page(s), §14.1 has 27 over 2 (rows 24-27 are TBD parts that the tables do not hold, FT-110)"
        ))
    }
}

fn ac08_terminals(c: &Ctx) -> Outcome {
    let mut bad = Vec::new();
    for (name, _) in &ROW_PAIRS[..3] {
        if let Err(e) = row_pair_text(&c.batch, &c.rows_q(), name)
            .and_then(|t| same_as_expected(&format!("csv/{name}.csv"), t.as_bytes()))
        {
            bad.push(e);
        }
    }
    // Designations are unique per strip.
    let q = c.oracle.query();
    let mut seen: BTreeMap<Id<Device>, BTreeSet<String>> = BTreeMap::new();
    let mut duplicates = Vec::new();
    for (_, dev) in q.devices().iter() {
        let Some(t) = &dev.aspects.terminal else { continue };
        if !seen
            .entry(t.strip)
            .or_default()
            .insert(t.designation.as_str().to_owned())
        {
            duplicates.push(format!("{} on {}", t.designation.as_str(), t.strip));
        }
    }
    if bad.is_empty() && duplicates.is_empty() {
        pass(
            "3, 20 and 5 terminal rows equal the committed CSVs (hand-compared with §9, differences in the README); designations unique per strip",
        )
    } else {
        fail(format!("{}; duplicates: {}", list(bad, 3), list(duplicates, 3)))
    }
}

/// The derived rows of a report block as CSV text, for `expected/csv/<name>.csv`.
pub fn row_pair_text(
    batch: &ew_import_tables::ew_interop_api::ImportBatch,
    q: &ew_derive::Q,
    name: &str,
) -> Result<String, String> {
    let (_, block) = ROW_PAIRS
        .iter()
        .find(|(n, _)| *n == name)
        .ok_or_else(|| format!("no row pair {name}"))?;
    Ok(rows_csv(q, &rows_of(q, batch, block)))
}

fn row_pair(c: &Ctx, name: &str) -> Outcome {
    match row_pair_text(&c.batch, &c.rows_q(), name)
        .and_then(|t| same_as_expected(&format!("csv/{name}.csv"), t.as_bytes()))
    {
        Ok(()) => pass(format!(
            "the derived rows equal expected/csv/{name}.csv (the SQL half is a separate entry)"
        )),
        Err(e) => fail(e),
    }
}

fn ac09_wire_rows(c: &Ctx) -> Outcome {
    let q = c.rows_q();
    let set = rows_of(&q, &c.batch, "907");
    let data = set
        .rows
        .iter()
        .filter(|r| r.kind == ew_reports::RowKind::Data)
        .count();
    if data != 44 + 6 {
        return fail(format!(
            "{data} data rows, §8.2 and §8.3 have 44 wires and 6 bridges"
        ));
    }
    match row_pair(c, "wires") {
        (Status::Pass, _) => pass("44 wire rows and 6 bridge rows equal expected/csv/wires.csv"),
        other => other,
    }
}

fn ac09_properties(c: &Ctx) -> Outcome {
    let q = golden_common::plain(&c.profiled);
    let props = all_props(&q);
    let rows = rows_of(&q, &c.batch, "907");
    let no = rows.column("no").expect("the No. column");
    let numbered = (0..44).filter(|i| !texts(&q, &rows, *i)[no].is_empty()).count();
    let mut wrong = Vec::new();
    if props.len() != 44 {
        wrong.push(format!(
            "{} wires have an effective size and colour, expected 44",
            props.len()
        ));
    }
    if numbered != 44 {
        wrong.push(format!(
            "{numbered} of 44 wires have a wire number (the wire numbering query does not exist yet, FT-100)"
        ));
    }
    if wrong.is_empty() {
        pass("44 wires with size, colour and wire number")
    } else {
        fail(wrong.join("; "))
    }
}

fn ac10_partition(c: &Ctx) -> Outcome {
    let q = c.oracle.query();
    let part = q.get::<PotentialPartition>(());
    let n = part.potential_count();
    let bonds = q.bonds().len();
    let mut wrong = Vec::new();
    if n != 19 {
        wrong.push(format!(
            "{n} potentials, §7.3 has 19 (the PROFIBUS DP potential needs the Bus connections, FT-105)"
        ));
    }
    if bonds != 1 {
        wrong.push(format!("{bonds} bonds, expected B1 only"));
    }
    if wrong.is_empty() {
        pass("19 potentials and bond B1")
    } else {
        fail(wrong.join("; "))
    }
}

fn ac11_cables(c: &Ctx) -> Outcome {
    let q = c.rows_q();
    let set = rows_of(&q, &c.batch, "906");
    let cores = set.column("core").expect("the core column");
    let n_cores = (0..set.rows.len())
        .filter(|i| set.rows[*i].kind == ew_reports::RowKind::Data && !texts(&q, &set, *i)[cores].is_empty())
        .count();
    let cables = set
        .rows
        .iter()
        .filter(|r| r.kind == ew_reports::RowKind::Data)
        .count()
        - n_cores;
    if (cables, n_cores) != (3, 13) {
        return fail(format!("{cables} cables and {n_cores} cores, expected 3 and 13"));
    }
    match row_pair(c, "cables") {
        (Status::Pass, _) => pass("3 cables and 13 cores equal expected/csv/cables.csv"),
        other => other,
    }
}

fn ac12_tables(c: &Ctx) -> Outcome {
    let csv = &c.text["1"];
    let table_texts = runs(csv)
        .into_iter()
        .filter(|(layer, _, _)| layer != "frame")
        .count();
    if table_texts > 0 {
        pass(format!("folio 1 prints {table_texts} texts outside the frame"))
    } else {
        fail(
            "folio 1 prints only its frame and title block: the cover tables are not drawn (README, \"Folio 1\")",
        )
    }
}

fn ac12_colour(c: &Ctx) -> Outcome {
    let before_q = golden_common::plain(&c.profiled);
    let before = all_props(&before_q);
    let rows_before = rows_csv(&before_q, &rows_of(&before_q, &c.batch, "907"));
    let after_session = golden_common::session_with(&c.batch, |p, _| {
        let (mut colour, size) = golden_common::profiles_json();
        colour["rows"][0]["colour"] = serde_json::json!(["RD"]);
        p.settings.extra.insert("colour_profile".into(), colour);
        p.settings.extra.insert("size_profile".into(), size);
    });
    let after_q = golden_common::plain(&after_session);
    let after = all_props(&after_q);
    let rows_after = rows_csv(&after_q, &rows_of(&after_q, &c.batch, "907"));
    let mut changed = 0;
    let mut wrong = Vec::new();
    for (w, b) in &before {
        let a = &after[w];
        if matches!(&b.1, ew_derive::PropSource::ColourRow { row: 0, .. }) {
            if a.0 == "RD" {
                changed += 1;
            } else {
                wrong.push(format!("wire {w} kept {}", a.0));
            }
        } else if a != b {
            wrong.push(format!("wire {w} changed without taking row 0"));
        }
    }
    if rows_before == rows_after {
        wrong.push("folio 907 prints the same rows after the change".into());
    }
    if wrong.is_empty() {
        pass(format!(
            "{changed} wires take the new colour and nothing else changes; folio 907's rows follow (recomputed from the edited profile; the one-commit property needs a profile command, folio 1's tables are not drawn)"
        ))
    } else {
        fail(list(wrong, 3))
    }
}

fn ac13_structure(c: &Ctx) -> Outcome {
    let mut wrong = Vec::new();
    let structure = structure_text(&c.pdf_pages, &c.pdf);
    if let Err(e) = same_as_expected("pdf/structure.json", structure.as_bytes()) {
        wrong.push(e);
    }
    let sha = format!("{:x}\n", Sha256::digest(&c.pdf.bytes));
    if let Err(e) = same_as_expected("pdf/sha256.txt", sha.as_bytes()) {
        wrong.push(e);
    }
    let r = &c.pdf.report;
    if r.links != 64 || r.plain_text_links != 0 {
        wrong.push(format!(
            "{} links and {} plain-text links, expected 64 and 0",
            r.links, r.plain_text_links
        ));
    }
    if !r.missing_fonts.is_empty() || r.dropped_notdef_glyphs != 0 {
        wrong.push(format!(
            "{} missing fonts, {} dropped glyphs",
            r.missing_fonts.len(),
            r.dropped_notdef_glyphs
        ));
    }
    if structure.contains("Tahoma") {
        wrong.push("a Tahoma font is embedded".into());
    }
    let labels: Vec<&str> = c.pdf_pages.iter().map(|p| p.label.as_str()).collect();
    let order: Vec<&str> = c.pages.iter().map(|g| g.entry.folio.as_str()).collect();
    if labels != order {
        wrong.push("the page labels differ from the folio labels of page_order".into());
    }
    if wrong.is_empty() {
        pass(format!(
            "{} pages: outline, 64 links, page labels = folio labels, embedded fonts and SHA-256 equal the committed files; no Tahoma (the folio count is 33 against 36, AC-03; text extraction not checked)",
            c.pdf_pages.len()
        ))
    } else {
        fail(wrong.join("; "))
    }
}

// ---------------------------------------------------------------------------------------------------------
// Golden B and the folio replay

fn ac18_b2(c: &Ctx) -> Outcome {
    let mut s = golden_common::session(&c.batch);
    let before = s.world();
    let records_before = ew_commands::rows::model_lines(&before.model).expect("records");
    let (tags_before, xrefs_before) = (tags(&s), xrefs(&s));
    let (partition, wires) = {
        let q = s.query();
        (
            q.get::<PotentialPartition>(()),
            q.connections()
                .iter()
                .map(|(_, w)| (**w).clone())
                .collect::<Vec<_>>(),
        )
    };
    let text = match std::fs::read_to_string(golden_common::variant("b2-renumber-pages.ewcmd")) {
        Ok(t) => t,
        Err(e) => return fail(format!("variants/b2-renumber-pages.ewcmd: {e}")),
    };
    if let Err(e) = run_log(&mut s, &text, "b2-renumber-pages.ewcmd") {
        return fail(e);
    }
    let tag_changes = tags_before
        .iter()
        .filter(|(id, old)| tags(&s)[*id] != **old)
        .count();
    let xref_changes = {
        let after = xrefs(&s);
        xrefs_before
            .iter()
            .filter(|(id, old)| after[*id] != **old)
            .count()
    };
    let topology = {
        let q = s.query();
        *q.get::<PotentialPartition>(()) == *partition
            && q.connections()
                .iter()
                .map(|(_, w)| (**w).clone())
                .collect::<Vec<_>>()
                == wires
    };
    let steps = s.history().entries().len();
    let undone = s.undo().is_ok()
        && ew_commands::rows::model_lines(&s.world().model).expect("records") == records_before;
    let mut wrong = Vec::new();
    if tag_changes != 2 {
        wrong.push(format!("{tag_changes} tags change, expected 2"));
    }
    if xref_changes != 14 {
        wrong.push(format!(
            "{xref_changes} cross-reference texts change, expected 14"
        ));
    }
    if !topology {
        wrong.push("the topology changed".into());
    }
    if steps != 1 || !undone {
        wrong.push("one undo does not restore the records byte for byte".into());
    }
    if wrong.is_empty() {
        pass(
            "folio 106 becomes 104: 2 tags and 14 texts change, same potentials and connections, one undo restores the records",
        )
    } else {
        fail(wrong.join("; "))
    }
}

/// Replays "renumber folio 3 as 4" (D-01, AC-20).
/// What replaying "renumber folio 3 as 4" changed.
struct Folio3To4 {
    tags: Vec<(String, String)>,
    xref_changes: usize,
    main_text: String,
    undone: bool,
}

fn folio_3_to_4(c: &Ctx) -> Result<Folio3To4, String> {
    let mut s = golden_common::session(&c.batch);
    let records_before = ew_commands::rows::model_lines(&s.world().model).expect("records");
    let (tags_before, xrefs_before) = (tags(&s), xrefs(&s));
    let page = &c.ids["pages|3"];
    let log = format!(
        "{{\"$schema\":\"ew.cmdlog/1\",\"label\":\"D-01\"}}\n{{\"cmd\":\"page.renumber\",\"params\":{{\"scope\":{{\"pages\":[\"{page}\"]}},\"start\":4,\"step\":1}}}}\n"
    );
    run_log(&mut s, &log, "d-01.ewcmd")?;
    let tags_after = tags(&s);
    let mut changed: Vec<(String, String)> = tags_before
        .iter()
        .filter(|(id, old)| tags_after[*id] != **old)
        .map(|(id, old)| (old.clone(), tags_after[id].clone()))
        .collect();
    changed.sort();
    let xrefs_after = xrefs(&s);
    let xref_changes = xrefs_before
        .iter()
        .filter(|(id, old)| xrefs_after[*id] != **old)
        .count();
    let main: Id<Representation> = id_of(c, "representations|-201U1");
    let main_text = xrefs_after.get(&main).cloned().unwrap_or_default();
    let undone = s.undo().is_ok()
        && ew_commands::rows::model_lines(&s.world().model).expect("records") == records_before;
    Ok(Folio3To4 {
        tags: changed,
        xref_changes,
        main_text,
        undone,
    })
}

fn ac20_folio_3_to_4(c: &Ctx) -> Outcome {
    match folio_3_to_4(c) {
        Err(e) => fail(e),
        Ok(Folio3To4 {
            tags,
            xref_changes,
            main_text: main,
            undone,
        }) => {
            let mut wrong = Vec::new();
            if tags
                != [
                    ("-3W1".to_owned(), "-4W1".to_owned()),
                    ("-3X1".to_owned(), "-4X1".to_owned()),
                ]
            {
                wrong.push(format!("tags changed: {tags:?}"));
            }
            if main != "\u{221e} 4-B2, 106-G3" {
                wrong.push(format!("-201U1 prints \"{main}\""));
            }
            if !undone {
                wrong.push("undo does not restore the records".into());
            }
            if wrong.is_empty() {
                pass(format!(
                    "-3W1 and -3X1 become -4W1 and -4X1, {xref_changes} cross-reference text(s) follow (\"3-B2\" becomes \"4-B2\"), undo restores the records (the page-file write set is not checked: no save in this check)"
                ))
            } else {
                fail(wrong.join("; "))
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// §19: the defect regression rows gated M1

fn defects(r: &mut Runner) {
    r.entry_from(
        "D-01",
        "renumber folio 3 as 4: -4W1, -4X1 and \"4-B2\" everywhere, undo restores",
        (
            "AC-20",
            "renumbering folio 3 to 4 updates -3W1, -3X1 and \"3-B2\" everywhere; undo restores",
        ),
        "",
    );
    r.entry("D-03", "no output prints the structure alias JE2416", |c| {
        let hits: Vec<String> = c
            .text
            .iter()
            .filter(|(_, csv)| runs(csv).iter().any(|(_, t, _)| t.contains("JE2416")))
            .map(|(f, _)| f.clone())
            .collect();
        if hits.is_empty() {
            pass("no page text contains \"JE2416\" (the EF-15 listing of the frozen tags needs the check engine: not applicable)")
        } else {
            fail(format!("\"JE2416\" printed on {}", list(hits, 5)))
        }
    });
    r.entry_from(
        "D-04",
        "the parts list holds the 27 rows of §14.1",
        ("AC-07", "folio 901 holds the 27 rows of §14.1 over 901 and 901.1"),
        "",
    );
    r.entry_from(
        "D-05",
        "V2.0 on every page",
        ("AC-04", "every page prints the bound title-block values"),
        "The revision table (page 911 lists V1.0 and V2.0) is not derived: schema v0 has no revision record.",
    );
    r.entry("D-06", "the project description is one text on every page", |c| {
        let want = "Description: Expansion Control Cabinets";
        let off: Vec<String> = c
            .text
            .iter()
            .filter(|(_, csv)| {
                !runs(csv)
                    .iter()
                    .any(|(layer, t, y)| layer == "frame" && *y > 180.0 && t == want)
            })
            .map(|(f, _)| f.clone())
            .collect();
        if off.is_empty() {
            pass(format!(
                "all {} pages print \"{want}\" (setting an override on 101 needs the page description command: not applicable)",
                c.text.len()
            ))
        } else {
            fail(format!("not printed on {}", list(off, 5)))
        }
    });
    r.entry_from(
        "D-07",
        "outline, 64 cross-reference links and page labels",
        (
            "AC-13",
            "PDF structure equals expected/pdf (outline, 64 links, page labels, fonts, SHA-256)",
        ),
        "",
    );
    r.entry(
        "D-08",
        "W29 takes 0.75 DBU from the profile, W20 1.5 mm2 from its CDP and DBU from the profile",
        d08,
    );
    r.entry("D-09", "W1-W44 are numbered, unique and stable", |_| {
        na("the wire numbering query does not exist yet (FT-100); the generated pages are counted by AC-03")
    });
    r.entry("D-10", "-202U2 carries 4 valves", |c| {
        let q = c.oracle.query();
        let parent: Id<Device> = id_of(c, "devices|-202U2");
        let n = q.idx().device_children(parent).count();
        if n == 4 {
            pass("-202U2 has 4 child valves (the rendering of folio 202 is covered by AC-02)")
        } else {
            fail(format!("-202U2 has {n} children, expected 4"))
        }
    });
    r.entry("D-11", "a terminal designation cannot be set empty", |c| {
        set_designation(c, "terminals|-X1031:8", &[""], false)
    });
    r.entry("D-12", "a duplicate designation is rejected", |c| {
        let (status, reason) = set_designation(c, "terminals|-X1031:3", &["4"], false);
        if status == Status::Fail {
            fail(format!(
                "{reason} (designations are unique per strip by invariant, but v0 has no such invariant and ew-checks holds no prevent rule yet)"
            ))
        } else {
            (status, reason)
        }
    });
    r.entry("D-14", "the bus port prints \"3-B2\", never \"3-B0\"", |c| {
        let q = c.memoised.query();
        let text = q.get::<XrefTextOf>(id_of(c, "representations|-201U1")).text.clone();
        if text.contains("3-B2") && !text.contains("3-B0") {
            pass(format!("-201U1 prints \"{text}\" (forcing the origin anchor needs a command that does not exist yet: not applicable)"))
        } else {
            fail(format!("-201U1 prints \"{text}\""))
        }
    });
    r.entry("D-17", "core colour BN is printed, never BR", |c| {
        let q = c.rows_q();
        let set = rows_of(&q, &c.batch, "906");
        let colour = set.column("colour").expect("the colour column");
        let colours: BTreeSet<String> = (0..set.rows.len())
            .map(|i| texts(&q, &set, i)[colour].clone())
            .collect();
        if colours.contains("BN") && !colours.contains("BR") {
            pass("folio 906 prints core colour BN and no BR (the strict-import rejection of \"BR\" needs the CSV import command: not applicable)")
        } else {
            fail(format!("colours on 906: {colours:?}"))
        }
    });
    r.entry("D-18", "the typed ratings and manufacturer names print on folios 101 and 102", |c| {
        let want = ["C10", "C4", "B16", "15 \u{00b0}C", "Murrelektronik", "Eaton"];
        let printed: BTreeSet<String> = ["101", "102"]
            .iter()
            .flat_map(|f| runs(&c.text[*f]).into_iter().map(|(_, t, _)| t))
            .collect();
        let missing: Vec<&str> = want.iter().copied().filter(|w| !printed.iter().any(|p| p.contains(w))).collect();
        if missing.is_empty() {
            pass("all six values print")
        } else {
            fail(format!("not printed on 101/102: {} (the symbol slots for ratings and part data are not filled yet)", missing.join(", ")))
        }
    });
    r.entry("D-20", "W13/W14 are a star at F55:2, W7/W8 a chain on -101F1:1", |_| {
        na("the wire list prints neither the junction nor its wiring style (README, \"Wire list (907)\"); the junction records are checked by the connectivity tests")
    });
    r.entry("D-22", "the 0 V to PE connection is the bond B1, not a wire", |c| {
        let q = c.oracle.query();
        let bonds = q.bonds().len();
        if bonds == 1 {
            pass("the model holds exactly one bond, B1 (the connectivity test bond_b1_links_0v_and_pe checks its ends; replacing it by a wire needs the check engine: not applicable)")
        } else {
            fail(format!("{bonds} bonds, expected B1 only"))
        }
    });
    r.entry("D-24", "24VDC-1, 0V and PE carry their IP names as aliases", d24);
    r.entry(
        "D-27",
        "28 valve child tags and 8 field devices with process tags in +FIELD",
        d27,
    );
    r.entry(
        "D-29",
        "every device tag prints in the style tag.main (9.62 pt)",
        d29,
    );
    r.entry("D-30", "the title block binds the end user", |c| {
        let want = "End user: <end user>";
        let off: Vec<String> = c
            .text
            .iter()
            .filter(|(_, csv)| !runs(csv).iter().any(|(l, t, _)| l == "frame" && t == want))
            .map(|(f, _)| f.clone())
            .collect();
        if off.is_empty() {
            pass(format!(
                "all {} title blocks print \"{want}\" (the cover is not drawn: not applicable)",
                c.text.len()
            ))
        } else {
            fail(format!("not printed on {}", list(off, 5)))
        }
    });
    r.entry(
        "D-33",
        "\"SK 3110.000\" keeps its space and physical page 6 is labelled \"101\"",
        |c| {
            let space = runs(&c.text["101"]).iter().any(|(_, t, _)| t == "SK 3110.000");
            let label = c.pdf_pages.get(5).map(|p| p.label.as_str());
            let mut wrong = Vec::new();
            if !space {
                wrong.push(
                    "folio 101 prints no \"SK 3110.000\" (the part-number slot is not filled yet)".to_owned(),
                );
            }
            if label != Some("101") {
                wrong.push(format!("physical page 6 is labelled {label:?}"));
            }
            if wrong.is_empty() {
                pass("both hold")
            } else {
                fail(wrong.join("; "))
            }
        },
    );
    r.entry(
        "D-35",
        "BOM descriptions per §14, ratings only at symbols",
        |_| na("needs the 27-row parts list (D-04) and the typed-data lint EF-25 of the check engine"),
    );
    r.entry("D-36", "assembly rows A1 and A2, order code on folio 3", |_| {
        na("the assembly BOM mode and `ew export bom` do not exist yet (FR-REP-005)")
    });
    r.entry("D-37", "profiles name their basis; the cover prints it", |_| {
        na("the bootstrap does not emit the colour and size profiles (decision log ID-90) and the cover is not drawn")
    });
    r.entry("D-41", "one assembly -201U1 with children on both pages, break marks", |_| {
        na("the assembly aspect and the generated overview break marks are not implemented (schema v0 keeps the assembly in `extra`)")
    });
    r.entry(
        "D-42",
        "one strip -X1031 with segments at 103-A3 and 103-A10",
        d42,
    );
}

fn d08(c: &Ctx) -> Outcome {
    let q = golden_common::plain(&c.profiled);
    let props = all_props(&q);
    let wire = |n: u32| &props[&c.ids[&format!("connections|W{n}")]];
    let (w29, w20) = (wire(29), wire(20));
    let mut wrong = Vec::new();
    if w29.0 != "DBU" || w29.2 != "0.75" || !matches!(w29.1, ew_derive::PropSource::ColourRow { .. }) {
        wrong.push(format!("W29 is {} {} ({:?})", w29.2, w29.0, w29.1));
    }
    if w20.2 != "1.5" || !matches!(w20.3, ew_derive::PropSource::Override(_)) || w20.0 != "DBU" {
        wrong.push(format!("W20 is {} {} ({:?}, {:?})", w20.2, w20.0, w20.3, w20.1));
    }
    if wrong.is_empty() {
        pass(
            "W29 0.75 DBU from the profile; W20 1.5 mm2 from its CDP, DBU from the profile (the explain output and the one-commit update of folios 1 and 907 are AC-12)",
        )
    } else {
        fail(wrong.join("; "))
    }
}

/// Sets the designation of a terminal through `fields.set`; the entries of `values` are tried in order.
fn set_designation(c: &Ctx, key: &str, values: &[&str], expect_accepted: bool) -> Outcome {
    let mut s = golden_common::session(&c.batch);
    let id = &c.ids[key];
    for value in values {
        let cmd = ew_api::AnyCommand {
            name: "fields.set".into(),
            params: serde_json::json!({
                "kind": "device",
                "targets": [id],
                "path": "terminal.designation",
                "value": value,
            }),
        };
        let result = s.execute(&cmd);
        match (result, expect_accepted) {
            (Err(_), false) => {}
            (Ok(_), true) => {}
            (Ok(_), false) => return fail(format!("`{value}` was accepted for {key}, expected a rejection")),
            (Err(e), true) => return fail(format!("`{value}` was rejected for {key}: {e:?}")),
        }
    }
    pass(format!(
        "{key}: designation {values:?} is rejected (the swap in one transaction needs a composite command: not applicable)"
    ))
}

fn d24(c: &Ctx) -> Outcome {
    let q = c.oracle.query();
    let want: [(&str, Vec<&str>); 3] = [
        (
            "24VDC-1",
            vec!["24VDC-AIR1", "24VDC-AIR2", "24VDC-AIR3", "24VDC-AIR4"],
        ),
        (
            "0V",
            vec!["0VDC", "0VDC-AIR1", "0VDC-AIR2", "0VDC-AIR3", "0VDC-AIR4"],
        ),
        (
            "PE",
            vec![
                "J-E-206-PE",
                "PE-1.1",
                "PE-1.2",
                "PE-1.3",
                "PE-1.4",
                "PE-AIR1",
                "PE-AIR2",
            ],
        ),
    ];
    let mut wrong = Vec::new();
    for (name, aliases) in want {
        let Some(p) = ew_derive::potential_named(&q, name) else {
            wrong.push(format!("no potential {name}"));
            continue;
        };
        let got: BTreeSet<String> = q.get::<PotentialNameOf>(p).aliases.iter().cloned().collect();
        let want: BTreeSet<String> = aliases.iter().map(|a| (*a).to_owned()).collect();
        if got != want {
            wrong.push(format!("{name}: aliases {got:?}"));
        }
    }
    if wrong.is_empty() {
        pass(
            "24VDC-1 with AIR1-AIR4, 0V with 0VDC and AIR1-AIR4, PE with its 7 aliases (the canonical name and aliases of folio 909 are in AC-10)",
        )
    } else {
        fail(list(wrong, 3))
    }
}

fn d27(c: &Ctx) -> Outcome {
    let q = c.oracle.query();
    let mut children = 0;
    let mut field = 0;
    let mut not_in_field = Vec::new();
    for (id, dev) in q.devices().iter() {
        let tag = q.get::<DeviceTag>(id);
        let Some(tag) = tag.as_ref() else { continue };
        let short = tag.short();
        if short.starts_with("-20") && short.contains("U") && short.contains("-Q") && dev.parent.is_some() {
            children += 1;
        }
        if dev.process_tag.is_some() {
            field += 1;
            if !tag.full.contains("+FIELD") {
                not_in_field.push(short);
            }
        }
    }
    if (children, field) == (28, 8) && not_in_field.is_empty() {
        pass("28 valve child tags (-201U3-Q1 ...) and 8 field devices with process tags in +FIELD")
    } else {
        fail(format!(
            "{children} valve child tags and {field} process-tag devices ({} outside +FIELD)",
            not_in_field.len()
        ))
    }
}

fn d29(c: &Ctx) -> Outcome {
    let q = c.oracle.query();
    let tags: BTreeSet<String> = q
        .devices()
        .ids()
        .filter_map(|id| q.get::<DeviceTag>(id).as_ref().as_ref().map(|t| t.short()))
        .collect();
    let mut sizes: BTreeSet<String> = BTreeSet::new();
    let mut seen = 0;
    for folio in ["101", "202", "203", "204", "205", "206"] {
        for row in csv_rows(&c.text[folio]).into_iter().skip(1) {
            if row[1] == "labels" && tags.contains(&row[3]) {
                seen += 1;
                sizes.insert(row[6].clone());
            }
        }
    }
    if seen > 0 && sizes == BTreeSet::from(["9.62".to_owned()]) {
        pass(format!(
            "{seen} printed device tags on 101, 202 and 203-206 are all 9.62 pt"
        ))
    } else {
        fail(format!("{seen} printed device tags with sizes {sizes:?}"))
    }
}

fn d42(c: &Ctx) -> Outcome {
    let q = c.oracle.query();
    let rep = |key: &str| -> Option<(String, String)> {
        let id = Id::<Representation>::from_str(c.ids.get(key)?).ok()?;
        let r = q.reps().get(id)?;
        let page = q.pages().get(r.page)?;
        let cell = q.get::<RepAnchorCell>(id).as_ref().as_ref()?.to_string();
        Some((page.folio.number.to_string(), cell))
    };
    let strips = q
        .devices()
        .iter()
        .filter(|(_, d)| d.aspects.strip.is_some())
        .count();
    let got = [rep("representations|-X1031"), rep("representations|-X1031.2")];
    let want = Some(("103".to_owned(), "A3".to_owned()));
    let want2 = Some(("103".to_owned(), "A10".to_owned()));
    if got[0] == want && got[1] == want2 {
        pass(format!(
            "one device -X1031 (of {strips} strips) with segments at 103-A3 and 103-A10 (the single BOM row is part of D-04)"
        ))
    } else {
        fail(format!("segments at {got:?}"))
    }
}
