// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The checks of the acceptance runner (doc 20 §21 AC-01 … AC-13, AC-18, AC-20; §19 rows gated M1).
//!
//! Every entry says what it compared and what is left out. A criterion whose feature does not exist yet is
//! `not_built` (it blocks the gate like a failure); one that cannot be evaluated headless or at this delivery
//! is `not_applicable` with the reason; none is skipped silently. A part of a criterion that holds while
//! another part is missing is its own entry.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::{Recovery, Session, SessionOptions};
use ew_derive::{
    DeviceTag, PageOrder, PageRef, PotentialNameOf, PotentialPartition, RepAnchorCell, TitleBlockValues,
    WireNumbers, XrefTextOf,
};
use ew_ids::Id;
use ew_scene::page::page_scene;
use ew_schema::{Device, Record, Representation};
use sha2::{Digest, Sha256};

use ew_store::StoreOptions;

use super::report::{Outcome, Status, fail, na, not_built, pass};
use super::runner::{Ctx, Runner};
use crate::golden_common::{self, all_props, repo, rows_csv, rows_of, run_log, tags, texts, xrefs};
use crate::golden_files::{decode_rgba, differing_percent, frame_figure_errors, frame_json, page_png};
use crate::pdf_support::{pdf_options, pdf_pages, structure_text, xref_counts};
use crate::view::golden_session;

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
    // AC-01 ... AC-13
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
        "AC-01",
        "the written PDF's vector content of every page equals the frame geometry within 0.2 mm",
        |_| {
            not_built(
                "no per-page comparison of the written PDF exists: the checks above read the frame forms and AC-04 the title-block texts, not the lines of the PDF (vector extraction with the 0.2 mm tolerance of §3.3)",
            )
        },
    );
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
        "the 9 main and 10 auxiliary texts equal §15 by value",
        ac05_main_aux,
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
        "AC-06",
        "every printed occurrence of a tag is identical, the PDF text layer included",
        |_| {
            not_built(
                "the harness has no PDF text extraction (FR-OUT-008): the check above reads the generated page texts, not the text layer of the written PDF",
            )
        },
    );
    r.entry(
        "AC-07",
        "folio 901 holds the 27 rows of §14.1 over 901 and 901.1",
        ac07_bom,
    );
    r.entry("AC-07", "bom export --mode assembly equals §14.2", |_| {
        not_built("the assembly BOM mode and `ew export bom` do not exist yet (FR-EXP-005, FR-REP-005)")
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
        "every wire has an effective size and colour",
        ac09_properties,
    );
    r.entry("AC-09", "every wire has a wire number", ac09_numbers);
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
        not_built("`ew export cables` does not exist yet (FR-EXP-005)")
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
    r.entry("AC-12", "the colour change is one commit", ac12_one_commit);
    r.entry(
        "AC-13",
        "PDF structure equals expected/pdf (outline, 64 links, page labels, fonts, SHA-256)",
        ac13_structure,
    );
    r.entry("AC-13", "text extraction of the PDF keeps spaces", |_| {
        not_built("the harness has no PDF text extraction (FR-OUT-008); the ToUnicode maps are written, but nothing reads them back")
    });
    r.entry("AC-13", "PDF/A-2b validated in process by krilla", ac13_pdfa);
    r.entry("AC-13", "veraPDF cross-check of PDF/A-2b", |_| {
        na("veraPDF is an external tool that is not installed in the headless harness; the CI job that has it runs the cross-check")
    });

    // The SQL half of the acceptance pairs (doc 20 §3.1 `acceptance/*.sql`, doc 18 §3.10 step 2).
    for (id, name) in [
        ("AC-08", "terminals_1X1, terminals_X1031, terminals_X1032"),
        ("AC-09", "wires"),
        ("AC-10", "potentials"),
        ("AC-11", "cables"),
    ] {
        r.entry(
            id,
            &format!("acceptance SQL over the api_v1 views equals expected/csv: {name}"),
            |_| {
                not_built(
                    "SQL projection pending (WP-S1.10); the CSV half above compares the derived report rows",
                )
            },
        );
    }

    // AC-18, AC-20: Golden B and the D-01 replay.
    r.entry("AC-18", "step B1: the ten tags of the tag map", ac18_b1);
    r.entry(
        "AC-18",
        "step B2: 2 tags and 14 cross-reference texts, same topology, undo restores the records",
        ac18_b2,
    );
    r.entry(
        "AC-18",
        "step B2: the preview lists the folio change as one page record",
        ac18_preview,
    );
    r.entry(
        "AC-18",
        "step B2: the preview lists the 2 tag and 14 cross-reference changes",
        ac18_derived_preview,
    );
    r.entry(
        "AC-18",
        "step B2: no orphan reference and the same interruption-point pairing after the commit",
        ac18_orphans_pairing,
    );
    r.entry(
        "AC-20",
        "renumbering folio 3 to 4 updates -3W1, -3X1 and \"3-B2\" everywhere; undo restores",
        ac20_folio_3_to_4,
    );
    r.entry(
        "AC-20",
        "renumbering folio 3 to 4 writes only that page's file",
        ac20_write_set,
    );

    // The other criteria of gate G-M1 (doc 20 §22).
    r.entry("AC-14", "PDF and JSONL are byte-identical across runs and record shuffles", |_| {
        na("a Linux-only gate criterion, evaluated by tests/page_output.rs (PDF), tests/golden_fixture.rs (JSONL) and the CI Linux job, not by this runner")
    });
    r.entry(
        "AC-14",
        "SVG and the parallel export are byte-identical to the serial export",
        |_| not_built("neither the SVG export nor a parallel export exists yet (FR-OUT-013, FR-OUT-019)"),
    );
    r.entry(
        "AC-15",
        "build path (a), the CSV bootstrap, equals path (c), the checked-in fixture",
        |_| na("evaluated by tests/golden_fixture.rs (byte-identical regeneration), not by this runner"),
    );
    r.entry(
        "AC-15",
        "build path (b), the builder DSL, produces the same expected CSVs",
        |_| not_built("the builder DSL (the golden builders of ew-testkit, WP-S6.01) does not exist yet"),
    );
    r.entry("AC-16", "freshness: scripted edits update every output; verify-derived is clean with the oracle on", |_| {
        not_built("the scripted edits (rename T55, move -102F2, folio 103 to 104, 24 V colour) need the tag, move and profile commands and `ew verify-derived`; for the record, the oracle is on for the sessions `oracle` and `profiled` of this run, and off for the sessions that apply edits or scripts (AC-12, AC-18, AC-20, D-06, D-11, D-12) and for the cross-reference scans")
    });
    if r.perf() {
        r.entry(
            "AC-17",
            "opening the golden project takes at most 0.5 s",
            ac17_open,
        );
        r.entry("AC-17", "a full check takes at most 2 s", |_| {
            not_built("the pre-delivery pack is incomplete (the remaining rules are FT-192, `ew check` offline mode FT-191), so a full-check time would not be the time of doc 20 §18")
        });
        r.entry("AC-17", "exporting the PDF takes at most 3 s", ac17_pdf);
    } else {
        r.entry("AC-17", "open <= 0.5 s, check <= 2 s, PDF export <= 3 s", |_| {
            na("measured at milestone deliveries (cargo xtask acceptance --milestone, release profile)")
        });
    }
    r.entry(
        "AC-39",
        "the anonymisation guard passes on golden/, docs/ and expected/",
        |_| {
            na("the guard is `cargo xtask check-anonymisation` (the xtask crate, outside this test crate); the repository checks run it")
        },
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
        "{} folios derived against 36 in §5; missing {}{} (908.1, 908.2: no PLC channel records, FT-111)",
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
                .join("; "),
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
            "63 linked texts with 64 link targets and 4 external texts on the drawn pages; -201U1 prints 3-B2",
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
            "{checked} devices print their table key; no \"-4W1\", \"-4X1\", \"T51\", \"JE2416\" or core colour \"BR\" in any page text"
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

/// The differences between the printed parts list and the table of doc 20 §14.1 that are accepted, as
/// `(position, column, doc 20 prints, the parts list prints, why)`. Every other cell must be equal.
///
/// - Rows 22 to 26, folio column: §14.1 gives folio 902 (the layout page the enclosure, plate, rail, duct and
///   brackets are placed on). The layout placements are not modelled yet (follow-up task), so rows 22 to 25
///   have no folio and row 26 lists the folios of the strips that carry the brackets.
/// - Row 26, tags: §14.1 lists "1X1, -X1031, -X1032"; the parts list sorts the tags in the order of the ID-9
///   sign order, "-X1031, -X1032, 1X1" (ruling).
const AC07_ACCEPTED: [(&str, &str, &str, &str); 6] = [
    ("22", "Folios", "902", ""),
    ("23", "Folios", "902", ""),
    ("24", "Folios", "902", ""),
    ("25", "Folios", "902", ""),
    ("26", "Folios", "902", "101, 103"),
    ("26", "Tags", "1X1, -X1031, -X1032", "-X1031, -X1032, 1X1"),
];

/// Folio 901 equals the 27 rows of doc 20 §14.1 (`expected/parts_list_14_1.csv`, generated from the doc by
/// `tables_from_doc20.py`) over 901 and 901.1, cell by cell, but for the accepted differences.
fn ac07_bom(c: &Ctx) -> Outcome {
    let q = c.rows_q();
    let set = rows_of(&q, &c.batch, "901");
    let n = set.rows.len();
    let pages = q
        .get::<ew_reports::BlockPages>((golden_common::block(&c.batch, "901"), 0))
        .len();
    if n != 27 || pages != 2 {
        return fail(format!(
            "{n} rows over {pages} page(s), §14.1 has 27 over 2 (901 and 901.1, the 4 TBD rows 24-27 on the second)"
        ));
    }
    let want = match expected("parts_list_14_1.csv") {
        Ok(bytes) => csv_rows(&String::from_utf8_lossy(&bytes)),
        Err(e) => return fail(e),
    };
    let header = &want[0];
    let mut wrong = Vec::new();
    let mut accepted = 0;
    for (i, expect) in want.iter().skip(1).enumerate() {
        let got = golden_common::texts(&q, &set, i);
        for (col, name) in header.iter().enumerate() {
            let (e, g) = (expect[col].as_str(), got.get(col).map_or("", String::as_str));
            if e == g {
                continue;
            }
            if AC07_ACCEPTED.contains(&(expect[0].as_str(), name.as_str(), e, g)) {
                accepted += 1;
            } else {
                wrong.push(format!(
                    "row {} {name}: §14.1 \"{e}\", printed \"{g}\"",
                    expect[0]
                ));
            }
        }
    }
    if wrong.is_empty() {
        pass(format!(
            "27 rows over 901 and 901.1 equal expected/parts_list_14_1.csv (§14.1) in every cell but {accepted} accepted differences (folio 902 of rows 22-26: layout placements are not modelled; row 26 tags in the ID-9 sign order)"
        ))
    } else {
        fail(format!(
            "{} cells differ from §14.1: {}",
            wrong.len(),
            list(wrong, 4)
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
    let n = all_props(&q).len();
    if n == 44 {
        pass("44 wires have an effective size and colour")
    } else {
        fail(format!(
            "{n} wires have an effective size and colour, expected 44"
        ))
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

/// Folio 1's colour and size tables are generated from the profiles of the project settings: every text the
/// profiles give for the cover (the voltage class, function, potential type and colour name of each colour
/// row, the size of each size row, the standards text of the profile and the note of §7.2) is printed outside
/// the frame layer.
fn ac12_tables(c: &Ctx) -> Outcome {
    let runs_outside_the_frame: Vec<String> = runs(&c.text["1"])
        .into_iter()
        .filter(|(layer, _, _)| layer != "frame")
        .map(|(_, text, _)| text)
        .collect();
    let printed: BTreeSet<&str> = runs_outside_the_frame.iter().map(String::as_str).collect();
    // A text with a character the default font lacks (the minus of "signal \u{2212}") is printed in several runs
    // (the symbol fallback font): the runs of the cover joined in paint order must contain it.
    let joined = runs_outside_the_frame.concat();
    let (colour, size) = golden_common::profiles_json();
    let en = |v: &serde_json::Value| v["en"].as_str().unwrap_or_default().to_owned();
    let mut want: Vec<String> = Vec::new();
    let rows = colour["rows"].as_array().map_or(&[][..], Vec::as_slice);
    for row in rows {
        for key in ["group", "function", "potential_type", "colour_name"] {
            want.push(en(&row[key]));
        }
    }
    let sizes = size["rows"].as_array().map_or(&[][..], Vec::as_slice);
    for row in sizes {
        want.push(format!("{} mm²", row["csa"].as_str().unwrap_or_default()));
    }
    want.push(en(&colour["standards"]));
    want.push("If size or sign is not indicated in diagram, standard size or sign is to be used.".to_owned());
    want.sort();
    want.dedup();
    let missing: Vec<String> = want
        .iter()
        .filter(|w| !printed.contains(w.as_str()) && !joined.contains(w.as_str()))
        .cloned()
        .collect();
    if printed.is_empty() {
        fail("folio 1 prints only its frame and title block: the cover tables are not drawn")
    } else if missing.is_empty() {
        pass(format!(
            "folio 1 prints the {} colour rows, the {} size tables, the standards text of the profile and the note",
            rows.len(),
            sizes.len()
        ))
    } else {
        fail(format!("folio 1 does not print {}", list(missing, 4)))
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
            "{changed} wires take the new colour and nothing else changes; folio 907's rows follow (recomputed from the edited profile)"
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
            "{0} pages: outline, 64 links, page labels = folio labels, embedded fonts and SHA-256 equal the committed files; no Tahoma (the folio count is {0} against 36, AC-03)",
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

/// The tag map of doc 20 §2.2 step B1: (Golden A, Golden B) for the ten devices that change.
const B1_TAG_MAP: [(&str, &str); 10] = [
    ("S51", "-101S1"),
    ("F55", "-101F2"),
    ("T55", "-101T1"),
    ("E55", "-101E1"),
    ("S56", "-101S2"),
    ("1X1", "-101X1"),
    ("-XPE", "-101X2"),
    ("-X1031", "-103X1"),
    ("-X1032", "-103X2"),
    ("E56", "-101E2"),
];

/// Replays step B1 (`variants/b1-page-based-tags.ewcmd`) and compares the tags of the non-terminal devices
/// with the ten rows of doc 20 §2.2: exactly those change, to those values; no cross-reference text changes.
fn ac18_b1(c: &Ctx) -> Outcome {
    let mut s = golden_common::session(&c.batch);
    let terminals: BTreeSet<Id<Device>> = {
        let q = s.query();
        q.devices()
            .iter()
            .filter(|(_, d)| d.aspects.terminal.is_some())
            .map(|(id, _)| id)
            .collect()
    };
    let (tags_before, xrefs_before) = (tags(&s), xrefs(&s));
    let text = match std::fs::read_to_string(golden_common::variant("b1-page-based-tags.ewcmd")) {
        Ok(t) => t,
        Err(e) => return fail(format!("variants/b1-page-based-tags.ewcmd: {e}")),
    };
    if let Err(e) = run_log(&mut s, &text, "b1-page-based-tags.ewcmd") {
        return fail(e);
    }
    let tags_after = tags(&s);
    let changed: BTreeMap<&str, &str> = tags_before
        .iter()
        .filter(|(id, old)| !terminals.contains(*id) && tags_after[*id] != **old)
        .map(|(id, old)| (old.as_str(), tags_after[id].as_str()))
        .collect();
    let mut wrong = Vec::new();
    for (a, b) in B1_TAG_MAP {
        match changed.get(a) {
            Some(got) if *got == b => {}
            Some(got) => wrong.push(format!("{a} becomes {got}, expected {b}")),
            None => wrong.push(format!("{a} does not change, expected {b}")),
        }
    }
    for (a, got) in &changed {
        if !B1_TAG_MAP.iter().any(|(x, _)| x == a) {
            wrong.push(format!("{a} becomes {got}, not in the tag map"));
        }
    }
    if xrefs(&s) != xrefs_before {
        wrong.push("cross-reference texts changed".into());
    }
    if wrong.is_empty() {
        pass("the ten tags of the tag map change and nothing else; no cross-reference text changes")
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
                    "-3W1 and -3X1 become -4W1 and -4X1, {xref_changes} cross-reference text(s) follow (\"3-B2\" becomes \"4-B2\"), undo restores the records"
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
    r.entry_from(
        "D-01",
        "renumbering writes only that page's file",
        ("AC-20", "renumbering folio 3 to 4 writes only that page's file"),
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
            pass("no page text contains \"JE2416\"")
        } else {
            fail(format!("\"JE2416\" printed on {}", list(hits, 5)))
        }
    });
    r.entry(
        "D-03",
        "the numbering report lists the frozen tags (EF-15)",
        |_| {
            not_built(
                "rule EW-NUM-001 (EF-15: a frozen tag that deviates from the scheme reports at info, FR-CHK-025) is not in ew-checks (FT-192), and no numbering report lists the frozen tags (FR-NUM-022)",
            )
        },
    );
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
        "Folio 911 (the revision overview, V1.0 and V2.0) is still a placeholder: the report kind does not read the revision records yet.",
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
            pass(format!("all {} pages print \"{want}\"", c.text.len()))
        } else {
            fail(format!("not printed on {}", list(off, 5)))
        }
    });
    r.entry(
        "D-06",
        "an override set on 101 changes the description of 101 only",
        d06_override,
    );
    r.entry(
        "D-06",
        "an override is reported as info and shown in the property grid",
        |_| {
            not_built(
                "rule EW-CORE-023 (title-block override, info, doc 15 §4, FR-REP-036) is not in ew-checks (WP-S1.16), and the property grid (FR-UI-009) does not exist",
            )
        },
    );
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
    r.entry_from(
        "D-09",
        "W1-W34 are numbered, unique and stable; the cable cores are not numbered",
        ("AC-09", "every wire has a wire number"),
        "",
    );
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
    r.entry(
        "D-11",
        "an empty terminal designation is rejected (parameter layer or invariant)",
        |c| set_designation(c, "terminals|-X1031:8", "", true),
    );

    r.entry(
        "D-12",
        "a duplicate designation is rejected by the invariant",
        |c| set_designation(c, "terminals|-X1031:3", "4", false),
    );
    r.entry(
        "D-12",
        "swapping two designations in one transaction is accepted",
        d12_swap,
    );
    r.entry("D-14", "the bus port prints \"3-B2\", never \"3-B0\"", |c| {
        let q = c.memoised.query();
        let text = q
            .get::<XrefTextOf>(id_of(c, "representations|-201U1"))
            .text
            .clone();
        if text.contains("3-B2") && !text.contains("3-B0") {
            pass(format!("-201U1 prints \"{text}\""))
        } else {
            fail(format!("-201U1 prints \"{text}\""))
        }
    });
    r.entry("D-14", "forcing the origin anchor reports \"3-B0: empty cell\"", |_| {
        not_built("no command sets a representation's anchor policy, and rule EW-CORE-053 (cross-reference cell invalid, FR-CHK-035) is not in ew-checks (WP-S1.16)")
    });
    r.entry("D-17", "core colour BN is printed, never BR", |c| {
        let q = c.rows_q();
        let set = rows_of(&q, &c.batch, "906");
        let colour = set.column("colour").expect("the colour column");
        let colours: BTreeSet<String> = (0..set.rows.len())
            .map(|i| texts(&q, &set, i)[colour].clone())
            .collect();
        if colours.contains("BN") && !colours.contains("BR") {
            pass("folio 906 prints core colour BN and no BR")
        } else {
            fail(format!("colours on 906: {colours:?}"))
        }
    });
    r.entry(
        "D-17",
        "strict import rejects \"BR\"; lenient import keeps it as a foreign code with the quick fix \"use BN\"",
        |_| not_built("the CSV importer has no strict or lenient mode (ImportContext carries none) and quick fixes do not exist yet"),
    );
    r.entry(
        "D-18",
        "the typed ratings and manufacturer names print on folios 101 and 102",
        |c| {
            let want = ["C10", "C4", "B16", "15 \u{00b0}C", "Murrelektronik", "Eaton"];
            let printed: BTreeSet<String> = ["101", "102"]
                .iter()
                .flat_map(|f| runs(&c.text[*f]).into_iter().map(|(_, t, _)| t))
                .collect();
            let missing: Vec<&str> = want
                .iter()
                .copied()
                .filter(|w| !printed.iter().any(|p| p.contains(w)))
                .collect();
            if missing.is_empty() {
                pass("all six values print")
            } else {
                fail(format!("not printed on 101/102: {}", missing.join(", ")))
            }
        },
    );
    r.entry("D-20", "W13/W14 are a star at F55:2, W7/W8 a chain on -101F1:1", |_| {
        not_built("the wire list prints neither the junction nor its wiring style (README, \"Wire list (907)\"), so the wire list cannot show the notation")
    });
    r.entry("D-22", "the 0 V to PE connection is the bond B1", |c| {
        let q = c.oracle.query();
        let bonds = q.bonds().len();
        if bonds == 1 {
            pass("the model holds exactly one bond, B1 (the connectivity test bond_b1_links_0v_and_pe checks its ends)")
        } else {
            fail(format!("{bonds} bonds, expected B1 only"))
        }
    });
    r.entry(
        "D-22",
        "replacing B1 by a plain wire is reported (EW-POT-001)",
        d22_plain_wire,
    );
    r.entry(
        "D-22",
        "a PELV potential without exactly one 0 V-PE bond is reported",
        |_| {
            not_built(
                "rule EW-POT-002 (PELV or SELV bonding, FR-CHK-028) is not in ew-checks (WP-S1.16, no FT row yet): the replacement is reported by EW-POT-001 only",
            )
        },
    );
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
            pass(format!("all {} title blocks print \"{want}\"", c.text.len()))
        } else {
            fail(format!("not printed on {}", list(off, 5)))
        }
    });
    r.entry("D-30", "the cover binds the customer", |_| {
        not_built("the cover (folio 1) is not drawn (README, \"Folio 1\")")
    });
    r.entry(
        "D-33",
        "\"SK 3110.000\" keeps its space and physical page 6 is labelled \"101\"",
        |c| {
            let space = runs(&c.text["101"]).iter().any(|(_, t, _)| t == "SK 3110.000");
            let label = c.pdf_pages.get(5).map(|p| p.label.as_str());
            let mut wrong = Vec::new();
            if !space {
                wrong.push("folio 101 prints no \"SK 3110.000\"".to_owned());
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
        |_| {
            not_built(
                "rule EW-CORE-065 (description lint, EF-25, FR-CHK-040) is not in ew-checks (1.0, FT-192); the 27-row parts list is D-04",
            )
        },
    );
    r.entry("D-36", "assembly rows A1 and A2, order code on folio 3", |_| {
        not_built("the assembly BOM mode and `ew export bom` do not exist yet (FR-REP-005)")
    });
    // Doc 20 §19: the profile names its basis ("company convention") and declares DBU a company code (profile
    // metadata); the cover prints the standards text of U-35, not the basis note.
    r.entry("D-37", "profiles name their basis; the cover prints it", |c| {
        let (colour, _) = golden_common::profiles_json();
        let basis = colour["basis"].as_str().unwrap_or_default();
        let note = colour["basis_note"]["en"].as_str().unwrap_or_default();
        let standards = colour["standards"]["en"].as_str().unwrap_or_default();
        let dbu = colour["codes"]
            .as_array()
            .and_then(|codes| codes.iter().find(|c| c["code"] == "DBU"));
        let printed: Vec<String> = runs(&c.text["1"])
            .into_iter()
            .filter(|(layer, _, _)| layer != "frame")
            .map(|(_, t, _)| t)
            .collect();
        let mut wrong = Vec::new();
        if basis != "company_convention" || !note.starts_with("company convention") {
            wrong.push(format!("the profile's basis is `{basis}`, expected the company convention"));
        }
        if dbu.is_none_or(|d| d["base"] != "BU") {
            wrong.push("DBU is not declared a company code on BU".to_owned());
        }
        if standards.is_empty() || !printed.iter().any(|t| t == standards) {
            wrong.push(format!("the cover does not print the standards text \"{standards}\""));
        }
        if printed.iter().any(|t| t.contains(note)) {
            wrong.push("the cover prints the basis note, which is metadata".to_owned());
        }
        if wrong.is_empty() {
            pass(format!(
                "the colour profile's basis is {basis} with DBU declared a company code on BU; the cover prints \"{standards}\" and keeps the basis note as metadata (U-35)"
            ))
        } else {
            fail(wrong.join("; "))
        }
    });
    r.entry("D-41", "one assembly -201U1 with children on both pages, break marks", |_| {
        not_built("the assembly aspect and the generated overview break marks are not implemented (schema v0 keeps the assembly in `extra`)")
    });
    r.entry(
        "D-42",
        "one strip -X1031 with segments at 103-A3 and 103-A10",
        d42,
    );
}

/// The main and auxiliary texts of doc 20 §15 ("Main ↔ auxiliary references"): what each main prints after
/// "∞" and what each auxiliary prints in brackets.
fn section_15_main_aux() -> Vec<String> {
    let doc = std::fs::read_to_string(repo().join("docs/design/20-golden-project.md")).expect("doc 20");
    let start = doc
        .find("**Main \u{2194} auxiliary references")
        .expect("the §15 table");
    let mut out = Vec::new();
    for line in doc[start..].lines().skip(1) {
        if line.starts_with("---") {
            break;
        }
        if !line.starts_with("| -") {
            continue;
        }
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        let main = cells[1];
        let mut aux = cells[2].replace("**", "");
        while let (Some(open), Some(close)) = (aux.find('('), aux.find(')')) {
            aux.replace_range(open..=close, "");
        }
        let aux: Vec<&str> = aux.split(',').map(str::trim).filter(|a| !a.is_empty()).collect();
        out.push(format!("\u{221e} {}", aux.join(", ")));
        out.extend(aux.iter().map(|_| format!("({main})")));
    }
    out.sort();
    out
}

fn ac05_main_aux(c: &Ctx) -> Outcome {
    let want = section_15_main_aux();
    if want.len() != 19 {
        return fail(format!("{} texts parsed from §15, 19 expected", want.len()));
    }
    let mut got: Vec<String> = Vec::new();
    for g in &c.pages {
        for r in &g.view.reps {
            for v in &r.slots {
                if !v.item_links.is_empty() && (v.text.starts_with('\u{221e}') || v.text.starts_with('(')) {
                    got.push(v.text.clone());
                }
            }
        }
    }
    got.sort();
    if got == want {
        pass("the 9 main texts (\"\u{221e} ...\") and 10 auxiliary texts (\"(...)\") equal §15 as a multiset")
    } else {
        let missing: Vec<String> = want.iter().filter(|w| !got.contains(w)).cloned().collect();
        let extra: Vec<String> = got.iter().filter(|g| !want.contains(g)).cloned().collect();
        fail(format!(
            "{} printed, 19 in §15; missing {}; unexpected {}",
            got.len(),
            list(missing, 4),
            list(extra, 4)
        ))
    }
}

/// AC-09, D-09: the wire numbers of doc 20 §8.1 (FT-100): connection-oriented `{page}.{counter}`, counter scope
/// page, W1 to W34 numbered and the ten cable cores W35 to W44 not (FR-NUM-019), numbers not printed on the
/// pages. The page of each number is the folio of doc 20 §8.2; the counters inside a page follow the numbering
/// order of doc 08 §5.2, which the document leaves to the runner (§8.1: the ties and the cells of the CPs), so
/// their order is not compared with the "No." column of §8.2. The same numbers come out of a model with
/// reversed records.
fn ac09_numbers(c: &Ctx) -> Outcome {
    let numbers_of = |s: &Session| -> Vec<(u32, Option<String>)> {
        let q = golden_common::plain(s);
        let n = q.get::<WireNumbers>(());
        (1..=44u32)
            .map(|w| {
                let id: Id<ew_schema::Connection> = id_of(c, &format!("connections|W{w}"));
                (w, n.number(id).map(|x| x.label.clone()))
            })
            .collect()
    };
    let q = golden_common::plain(&c.profiled);
    let got = numbers_of(&c.profiled);
    let mut wrong = Vec::new();
    let mut labels = BTreeSet::new();
    let mut per_page: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for (w, label) in &got {
        match (*w <= 34, label) {
            (true, Some(l)) => {
                let (page, counter) = l.split_once('.').unwrap_or((l, ""));
                let folio = match w {
                    1..=19 => "101",
                    20..=32 => "102",
                    _ => "103",
                };
                if page != folio {
                    wrong.push(format!("W{w} is {l}, doc 20 §8.2 puts it on folio {folio}"));
                }
                if !labels.insert(l.clone()) {
                    wrong.push(format!("{l} is given twice"));
                }
                if let Ok(n) = counter.parse::<u32>() {
                    per_page.entry(page.to_owned()).or_default().push(n);
                }
            }
            (true, None) => wrong.push(format!("W{w} has no number")),
            (false, Some(l)) => wrong.push(format!("the cable core W{w} is numbered {l}")),
            (false, None) => {}
        }
    }
    for (page, mut counters) in per_page {
        counters.sort_unstable();
        if counters != (1..=counters.len() as u32).collect::<Vec<_>>() {
            wrong.push(format!("the counters of folio {page} are {counters:?}, not 1..n"));
        }
    }
    // The 907 rows print the same numbers, wire by wire.
    let rows = rows_of(&q, &c.batch, "907");
    let no = rows.column("no").expect("the No. column");
    for (w, label) in &got {
        let id: Id<ew_schema::Connection> = id_of(c, &format!("connections|W{w}"));
        let printed = rows
            .rows
            .iter()
            .position(|r| r.id == ew_reports::RowId::entity(id))
            .map(|i| texts(&q, &rows, i)[no].clone());
        if printed.as_deref() != Some(label.as_deref().unwrap_or("")) {
            wrong.push(format!(
                "folio 907 prints {printed:?} for W{w}, the query gives {label:?}"
            ));
        }
    }
    // Stable: record order does not matter.
    let reversed = golden_common::session_with(&c.batch, |p, r| {
        golden_common::with_profiles(p);
        r.reverse();
    });
    if numbers_of(&reversed) != got {
        wrong.push("a model with reversed records numbers the wires differently".to_owned());
    }
    if wrong.is_empty() {
        pass(format!(
            "W1-W34 carry {} unique `{{page}}.{{counter}}` numbers (19 on 101, 13 on 102, 2 on 103, counters 1..n per page), W35-W44 (cable cores) none; folio 907 prints them; the same numbers with reversed records",
            labels.len()
        ))
    } else {
        fail(list(wrong, 4))
    }
}

/// D-22: dropping the bond record leaves W27 a plain wire between 0V and PE, so one potential holds two
/// potential types; EW-POT-001 reports it. Golden A itself is clean (checked first).
fn d22_plain_wire(c: &Ctx) -> Outcome {
    let findings = |s: &Session| -> Vec<String> {
        s.check(&PRE_DELIVERY, CheckMode::Release)
            .findings
            .iter()
            .filter(|f| f.rule == "EW-POT-001" && f.is_open())
            .map(|f| f.message.clone())
            .collect()
    };
    let clean = golden_common::session(&c.batch);
    let baseline = findings(&clean);
    if !baseline.is_empty() {
        return fail(format!(
            "Golden A with its bond already reports EW-POT-001: {}",
            list(baseline, 2)
        ));
    }
    let bond = c.ids["bonds|B1"].clone();
    let edited = golden_common::session_with(&c.batch, |p, records| {
        golden_common::with_profiles(p);
        records.retain(|r| !matches!(r, Record::Bond(b) if b.id.to_string() == bond));
    });
    let reported = findings(&edited);
    if reported.is_empty() {
        fail("without B1, W27 joins 0V and PE and EW-POT-001 reports nothing")
    } else {
        pass(format!(
            "Golden A is clean; without B1, W27 joins 0V and PE and EW-POT-001 reports {} type conflict(s)",
            reported.len()
        ))
    }
}

fn ac13_pdfa(c: &Ctx) -> Outcome {
    let options = pdf_options(&c.memoised.query());
    if options.conformance == ew_export::Conformance::PdfA2b && c.pdf.bytes.starts_with(b"%PDF-") {
        pass(format!(
            "the {}-page PDF was written under Conformance::PdfA2b, which makes krilla's A2_B archival validator check the document (it refuses an invalid one)",
            c.pdf_pages.len()
        ))
    } else {
        fail(format!(
            "the golden PDF is written with conformance {:?}",
            options.conformance
        ))
    }
}

/// A unique scratch directory under the system temp directory.
fn scratch_dir(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("ew-acceptance-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).expect("a directory");
    for e in std::fs::read_dir(from).expect("a readable directory") {
        let e = e.expect("an entry");
        let target = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), &target).expect("a copied file");
        }
    }
}

fn ac17_open(_: &Ctx) -> Outcome {
    let copy = scratch_dir("open");
    let state = scratch_dir("open-state");
    copy_dir(&repo().join("golden/J-E-2416.ew"), &copy);
    let mut options = StoreOptions::new("acceptance");
    options.state_dir = Some(state.clone());
    let started = std::time::Instant::now();
    let opened = Session::open(&copy, options, SessionOptions::default(), Recovery::Keep);
    let elapsed = started.elapsed();
    let outcome = match opened {
        Ok((session, _)) => {
            session.close();
            if elapsed.as_secs_f64() <= 0.5 {
                pass(format!(
                    "opened in {:.3} s on this machine (the baseline machine is the reference)",
                    elapsed.as_secs_f64()
                ))
            } else {
                fail(format!(
                    "opened in {:.3} s, the limit is 0.5 s",
                    elapsed.as_secs_f64()
                ))
            }
        }
        Err(e) => fail(format!("the fixture does not open: {e:?}")),
    };
    let _ = std::fs::remove_dir_all(&copy);
    let _ = std::fs::remove_dir_all(&state);
    outcome
}

fn ac17_pdf(c: &Ctx) -> Outcome {
    let view = golden_session(None);
    let q = view.query();
    let started = std::time::Instant::now();
    let pages = pdf_pages(&q);
    let out = ew_export::pdf(&pages, &pdf_options(&q), &c.fonts, None);
    let elapsed = started.elapsed().as_secs_f64();
    match out {
        Ok(_) if elapsed <= 3.0 => pass(format!("{} pages exported in {elapsed:.3} s", pages.len())),
        Ok(_) => fail(format!(
            "{} pages exported in {elapsed:.3} s, the limit is 3 s",
            pages.len()
        )),
        Err(e) => fail(format!("the export failed: {e:?}")),
    }
}

/// The `page.renumber` command of Golden B step B2 with the section resolved against the session.
fn b2_command(s: &Session) -> ew_api::AnyCommand {
    let q = s.query();
    let section = q
        .sections()
        .iter()
        .find(|(_, sec)| sec.number == 1)
        .map(|(id, _)| id)
        .expect("section 1");
    ew_api::AnyCommand {
        name: "page.renumber".into(),
        params: serde_json::json!({
            "scope": {"section": section},
            "start": 101,
            "step": 1,
            "keep_gaps": false
        }),
    }
}

fn ac18_preview(c: &Ctx) -> Outcome {
    let mut s = golden_common::session(&c.batch);
    let cmd = b2_command(&s);
    let preview = match s.preview(&cmd) {
        Ok(p) => p,
        Err(e) => return fail(format!("the preview failed: {e:?}")),
    };
    let diff = preview.diff();
    let kinds: Vec<String> = diff
        .changes
        .iter()
        .map(|ch| format!("{:?} {:?} {:?}", ch.change, ch.key.0, ch.fields))
        .collect();
    let committed = s.world().rev.0;
    if diff.len() == 1 && diff.changes[0].key.0 == ew_schema::Kind::Page && committed == 0 {
        pass(format!(
            "the preview changes one page record ({}) and commits nothing",
            kinds[0]
        ))
    } else {
        fail(format!(
            "the preview lists {} changes: {}",
            diff.len(),
            list(kinds, 4)
        ))
    }
}

/// Step B2's preview with its derived diff (`Session::derived_diff`): exactly the two tags -106W1, -106W2
/// and 14 cross-reference texts "106-…" → "104-…", nothing committed.
fn ac18_derived_preview(c: &Ctx) -> Outcome {
    let mut s = golden_common::session(&c.batch);
    let cmd = b2_command(&s);
    let preview = match s.preview(&cmd) {
        Ok(p) => p,
        Err(e) => return fail(format!("the preview failed: {e:?}")),
    };
    let d = s.derived_diff(&preview);
    let mut wrong = Vec::new();
    let mut tags: Vec<(&str, &str)> = d
        .tags
        .iter()
        .map(|t| (t.before.as_str(), t.after.as_str()))
        .collect();
    tags.sort_unstable();
    if tags != [("-106W1", "-104W1"), ("-106W2", "-104W2")] {
        wrong.push(format!("the tag changes are {tags:?}"));
    }
    if d.xrefs.len() != 14 {
        wrong.push(format!(
            "{} cross-reference texts change, expected 14",
            d.xrefs.len()
        ));
    }
    if let Some(x) = d
        .xrefs
        .iter()
        .find(|x| !x.before.contains("106-") || x.after != x.before.replace("106-", "104-"))
    {
        wrong.push(format!("{:?} becomes {:?}", x.before, x.after));
    }
    if s.world().rev.0 != 0 {
        wrong.push("the preview committed".into());
    }
    if wrong.is_empty() {
        pass(
            "the derived diff of the preview lists -106W1 → -104W1, -106W2 → -104W2 and 14 texts \"106-…\" → \"104-…\"; nothing is committed",
        )
    } else {
        fail(wrong.join("; "))
    }
}

/// The 24 V colour change of AC-12 as one command: `project.settings.set` of the colour profile with row 0
/// (24 V DC positive) set to RD, previewed and committed as one undo step; the wires that take row 0 follow,
/// nothing else changes, folio 907's rows follow, and one undo restores the wire colours.
fn ac12_one_commit(c: &Ctx) -> Outcome {
    let mut s = golden_common::session_with_profiles(&c.batch);
    let before = all_props(&golden_common::plain(&s));
    let rows_before = {
        let q = golden_common::plain(&s);
        rows_csv(&q, &rows_of(&q, &c.batch, "907"))
    };
    let (mut colour, _) = golden_common::profiles_json();
    colour["rows"][0]["colour"] = serde_json::json!(["RD"]);
    let cmd = ew_api::AnyCommand {
        name: "project.settings.set".into(),
        params: serde_json::json!({"member": "colour_profile", "value": colour}),
    };
    let preview = match s.preview(&cmd) {
        Ok(p) => p,
        Err(e) => return fail(format!("the preview failed: {e:?}")),
    };
    let mut wrong = Vec::new();
    if preview.diff().len() != 1 || preview.diff().changes[0].key.0 != ew_schema::Kind::Project {
        wrong.push(format!(
            "the preview changes {} records, expected the project record only",
            preview.diff().len()
        ));
    }
    if let Err(e) = s.commit(preview) {
        return fail(format!("the commit failed: {e:?}"));
    }
    if s.history().entries().len() != 1 {
        wrong.push(format!("{} undo steps, expected 1", s.history().entries().len()));
    }
    let after = all_props(&golden_common::plain(&s));
    let mut changed = 0;
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
    let rows_after = {
        let q = golden_common::plain(&s);
        rows_csv(&q, &rows_of(&q, &c.batch, "907"))
    };
    if rows_after == rows_before {
        wrong.push("folio 907 prints the same rows after the commit".into());
    }
    if s.undo().is_err() || all_props(&golden_common::plain(&s)) != before {
        wrong.push("one undo does not restore the wire colours".into());
    }
    if changed == 0 {
        wrong.push("no wire takes row 0".into());
    }
    if wrong.is_empty() {
        pass(format!(
            "one project.settings.set commit (one undo step, the project record only): {changed} wires take RD, nothing else changes, folio 907 follows, one undo restores"
        ))
    } else {
        fail(list(wrong, 3))
    }
}

/// The link targets (representation, page) of every cross-reference text, by representation.
/// The link targets of one cross-reference text: (representation, page).
type LinkTargets = Vec<(Id<Representation>, Id<ew_schema::Page>)>;

fn xref_links(s: &Session) -> BTreeMap<Id<Representation>, LinkTargets> {
    let q = s.query();
    q.reps()
        .ids()
        .map(|id| {
            let t = q.get::<XrefTextOf>(id);
            (id, t.links.iter().map(|l| (l.rep, l.page)).collect())
        })
        .collect()
}

fn ac18_orphans_pairing(c: &Ctx) -> Outcome {
    let mut s = golden_common::session(&c.batch);
    let before = xref_links(&s);
    let cmd = b2_command(&s);
    let result = s.preview(&cmd).and_then(|p| s.commit(p));
    if let Err(e) = result {
        return fail(format!("step B2 failed: {e:?}"));
    }
    let mut wrong = Vec::new();
    if let Err(e) = s.world().model.integrity_check() {
        wrong.push(format!("integrity check: {e:?}"));
    }
    if xref_links(&s) != before {
        wrong.push("the link targets of the cross-reference texts changed".to_owned());
    }
    if wrong.is_empty() {
        pass(
            "the model's integrity check (every reference resolves) holds, and all interruption-point and main/auxiliary link targets are the same records",
        )
    } else {
        fail(wrong.join("; "))
    }
}

fn ac20_write_set(c: &Ctx) -> Outcome {
    let mut s = golden_common::session(&c.batch);
    let dir = scratch_dir("write-set");
    let state = scratch_dir("write-set-state");
    let mut options = StoreOptions::new("acceptance");
    options.state_dir = Some(state.clone());
    let outcome = (|| -> Outcome {
        if let Err(e) = s.create(&dir, options) {
            return fail(format!("the session cannot be saved as a project: {e:?}"));
        }
        let page = &c.ids["pages|3"];
        let log = format!(
            "{{\"$schema\":\"ew.cmdlog/1\"}}\n{{\"cmd\":\"page.renumber\",\"params\":{{\"scope\":{{\"pages\":[\"{page}\"]}},\"start\":4,\"step\":1}}}}\n"
        );
        if let Err(e) = run_log(&mut s, &log, "d-01.ewcmd") {
            return fail(e);
        }
        match s.save() {
            Ok(report) => {
                let written: Vec<&str> = report.written.iter().map(|p| p.as_str()).collect();
                let want = format!("pages/{page}.jsonl");
                if written == [want.as_str()] && report.removed.is_empty() {
                    pass(format!(
                        "the save writes {want} only (and {} the manifest)",
                        if report.manifest_written {
                            "rewrites"
                        } else {
                            "leaves"
                        }
                    ))
                } else {
                    fail(format!(
                        "the save writes {written:?} and removes {:?}",
                        report.removed
                    ))
                }
            }
            Err(e) => fail(format!("the save failed: {e:?}")),
        }
    })();
    s.close();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&state);
    outcome
}

fn d06_override(c: &Ctx) -> Outcome {
    let title = |s: &Session, folio: &str| {
        let q = s.query();
        let page: Id<ew_schema::Page> = id_of(c, &format!("pages|{folio}"));
        q.get::<TitleBlockValues>(PageRef::Drawn(page))
            .get("page.description")
            .cloned()
    };
    let mut s = golden_common::session(&c.batch);
    let (b101, b102) = (title(&s, "101"), title(&s, "102"));
    let cmd = ew_api::AnyCommand {
        name: "fields.set".into(),
        params: serde_json::json!({
            "kind": "page",
            "targets": [c.ids["pages|101"]],
            "path": "description_override",
            "value": {"en": "Override text"},
        }),
    };
    if let Err(e) = s.execute(&cmd) {
        return fail(format!("the override was rejected: {e:?}"));
    }
    let (a101, a102) = (title(&s, "101"), title(&s, "102"));
    if a101 != b101 && a101.is_some() && a102 == b102 {
        pass("the description of 101 becomes the override; 102 keeps the project description")
    } else {
        fail(format!(
            "description of 101: {b101:?} then {a101:?}; of 102: {b102:?} then {a102:?}"
        ))
    }
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

/// D-12: -X1031:3 and -X1031:4 swap designations in one `terminal.designate` transaction; the prevent rule
/// EW-TERM-002 sees only the final state, so no temporary name is needed (FR-TRM-022).
fn d12_swap(c: &Ctx) -> Outcome {
    let mut s = golden_common::session(&c.batch);
    let (k3, k4) = ("terminals|-X1031:3", "terminals|-X1031:4");
    let cmd = ew_api::AnyCommand {
        name: "terminal.designate".into(),
        params: serde_json::json!({"assignments": [
            {"terminal": c.ids[k3], "designation": "4"},
            {"terminal": c.ids[k4], "designation": "3"},
        ]}),
    };
    let before = s.world().rev;
    if let Err(e) = s.execute(&cmd) {
        return fail(format!("the swap was refused with {} ({e})", error_kind(&e)));
    }
    let after = s.world().rev;
    let q = s.query();
    let designation = |key: &str| {
        q.devices()
            .get(id_of::<Device>(c, key))
            .and_then(|d| d.aspects.terminal.as_ref())
            .map(|t| t.designation.as_str().to_owned())
            .unwrap_or_default()
    };
    let (d3, d4) = (designation(k3), designation(k4));
    let undo_steps = s.history().entries().len();
    if d3 == "4" && d4 == "3" && after > before && undo_steps == 1 {
        pass(
            "one terminal.designate transaction swaps -X1031:3 and -X1031:4 (one undo step); EW-TERM-002 accepts the final state",
        )
    } else {
        fail(format!(
            "after the swap -X1031:3 is `{d3}` and -X1031:4 is `{d4}` ({undo_steps} undo steps)"
        ))
    }
}

/// The name of a command error's kind.
fn error_kind(e: &ew_api::CmdError) -> &'static str {
    use ew_api::CmdError;
    match e {
        CmdError::UnknownCommand { .. } => "UnknownCommand",
        CmdError::InvalidParams { .. } => "InvalidParams",
        CmdError::NotFound { .. } => "NotFound",
        CmdError::Rejected { .. } => "Rejected",
        CmdError::Exposure { .. } => "Exposure",
        CmdError::NeedsPreview { .. } => "NeedsPreview",
        CmdError::Prevented { .. } => "Prevented",
        CmdError::Model(ew_model::ModelError::Invariant { .. }) => "Model(Invariant)",
        CmdError::Model(ew_model::ModelError::Prevented { .. }) => "Model(Prevented)",
        CmdError::Model(_) => "Model(other)",
        _ => "other",
    }
}

/// Sets the designation of a terminal through `fields.set` and expects a refusal: an invariant or a prevent
/// rule, and with `params_layer` also the parameter layer (`InvalidParams`: parameter validation is part of the
/// command discipline, ADR-0008). An acceptance, or any other error, is a failure.
fn set_designation(c: &Ctx, key: &str, value: &str, params_layer: bool) -> Outcome {
    use ew_api::CmdError;
    use ew_model::ModelError;
    let mut s = golden_common::session(&c.batch);
    let cmd = ew_api::AnyCommand {
        name: "fields.set".into(),
        params: serde_json::json!({
            "kind": "device",
            "targets": [c.ids[key]],
            "path": "terminal.designation",
            "value": value,
        }),
    };
    let Err(e) = s.execute(&cmd) else {
        return fail(format!("`{value}` was accepted for {key}, expected a refusal"));
    };
    let layer = match &e {
        CmdError::Model(ModelError::Invariant { id, .. }) => Some(format!("the invariant {id}")),
        CmdError::Model(ModelError::Prevented { rule, .. }) | CmdError::Prevented { rule, .. } => {
            Some(format!("the prevent rule {rule}"))
        }
        CmdError::Rejected { source, .. } => Some(format!("the command's own rule ({source})")),
        CmdError::InvalidParams { path, .. } if params_layer => {
            Some(format!("the parameter layer (InvalidParams at {path})"))
        }
        _ => None,
    };
    match layer {
        Some(layer) => pass(format!("{key}: designation `{value}` is refused by {layer}")),
        None => fail(format!(
            "{key}: designation `{value}` is refused with {}, not by an invariant or a prevent rule ({e:?})",
            error_kind(&e)
        )),
    }
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
