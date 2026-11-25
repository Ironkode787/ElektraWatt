// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The program's own words in a list's cells and its boxes' titles print in the project's language, as its
//! headings do (doc 09 §4.7); a value of the project's data (a tag, a part number, a colour code) stays as
//! it is. Over copies of the sample, whose language is English, once as they are and once made Dutch.

use crate::common;

use ew_api::Session;
use ew_api::output::page_layer;
use ew_derive::{PageOrder, PageRef};
use ew_dl::{ItemKind, Layer};
use ew_ids::{Id, Uuid};
use ew_reports::render::{RenderOptions, render};
use ew_schema::{LengthSource, Record, ReportKind, ReportSource};
use serde_json::json;

/// The sample's records with its first cable's length taken from its part and its second's missing, a PART DATA box
/// beside the sample's legend on 902, and the valve terminal's assembly of one unlabelled configuration; and the valve
/// terminal's ID.
fn records() -> (ew_schema::Project, Vec<Record>, Vec<ew_schema::LibDocument>, Uuid) {
    let (project, mut records, lib) = common::sample_records_with_legend_box();
    let mut cables: Vec<&mut ew_schema::Device> = records
        .iter_mut()
        .filter_map(|r| match r {
            Record::Device(d) if d.aspects.cable.is_some() => Some(d),
            _ => None,
        })
        .collect();
    cables.sort_by_key(|d| d.id);
    let lengths = [LengthSource::FromPart, LengthSource::Missing];
    for (cable, length) in cables.into_iter().zip(lengths) {
        cable.aspects.cable.as_mut().expect("a cable").length = length;
    }
    let mut part_data = records
        .iter()
        .find_map(|r| match r {
            Record::ReportBlock(b) if b.embedded.is_some() => Some(b.clone()),
            _ => None,
        })
        .expect("the sample's legend on 902");
    part_data.id = Id::from_uuid(Uuid::from_u128(0x1955));
    part_data.source = ReportSource::Kind(ReportKind::PartData);
    part_data.columns = Vec::new();
    part_data.title = Default::default();
    records.push(Record::ReportBlock(part_data));
    let mut values: Vec<serde_json::Value> = records
        .iter()
        .map(|r| serde_json::to_value(r).expect("a record"))
        .collect();
    let terminal = values
        .iter_mut()
        .find(|v| v["$k"] == "device" && v["plc_station"]["role"] == "valve_terminal")
        .expect("the sample's valve terminal");
    terminal["assembly"] = json!({"configs": [{"order_code": "VT-1"}]});
    let terminal: Uuid = terminal["id"]
        .as_str()
        .and_then(|t| t.parse().ok())
        .expect("its ID");
    let records = values
        .into_iter()
        .map(|v| Record::from_value(v).expect("a record"))
        .collect();
    (project, records, lib, terminal)
}

/// The edited sample, in English as the sample is, or made Dutch, and its valve terminal.
fn sample(dutch: bool) -> (Session, Uuid) {
    let (project, records, lib, terminal) = records();
    let mut s = common::session_of(project, records, lib);
    // A frozen tag that deviates, for the numbering report's words (the sample freezes none).
    let uc1 = s.resolve_reference("-UC1").expect("-UC1");
    common::run_cmd(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [uc1.to_string()], "path": "tag.frozen", "value": {"text": "UC9"}}),
    );
    if dutch {
        common::run_cmd(
            &mut s,
            "project.settings.set",
            json!({ "member": "languages", "value": {"source": "nl-NL", "date": "dd_mm_yyyy", "decimal_separator": "comma"} }),
        );
    }
    (s, terminal)
}

/// Every cell of the whole-project rows of `kind` (of the device `include`), printed in the project's language as a
/// list page prints them.
fn cells_of(s: &Session, kind: &str, include: Option<Uuid>) -> Vec<String> {
    let q = s.query();
    let opts = RenderOptions {
        lang: ew_derive::print_language(&q),
        ..RenderOptions::default()
    };
    let def = ew_reports::Registry::builtin()
        .by_name(kind)
        .unwrap_or_else(|| panic!("the kind {kind}"));
    let rows =
        ew_reports::project_rows(&q, &def.kind(), include).unwrap_or_else(|| panic!("the kind {kind}"));
    rows.rows
        .iter()
        .flat_map(|r| r.cells.iter().map(|c| render(&q, c, &opts)))
        .collect()
}

fn cells(s: &Session, kind: &str) -> Vec<String> {
    cells_of(s, kind, None)
}

/// The texts the boxes on folio `folio` print.
fn box_texts(s: &Session, folio: &str) -> Vec<String> {
    let q = s.query();
    let page = q
        .get::<PageOrder>(())
        .entries
        .iter()
        .find(|e| e.folio == folio && matches!(e.page, PageRef::Drawn(_)))
        .map(|e| e.page)
        .unwrap_or_else(|| panic!("folio {folio}"));
    page_layer(&q, page, Layer::Tables)
        .list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(run) => Some(run.text.clone()),
            _ => None,
        })
        .collect()
}

/// The program's words the edited sample's lists print, by kind: `(kind, English, Dutch)`, each a whole cell.
const WORDS: &[(&str, &str, &str)] = &[
    ("cable_overview_diagrams", "Cable overview", "Kabeloverzicht"),
    ("cable_overview_diagrams", "from part", "uit artikel"),
    ("cable_overview_diagrams", "length missing", "lengte ontbreekt"),
    ("wire_list", "Bridges", "Bruggen"),
    (
        "wire_list",
        "plug-in bridge, terminals 1\u{2013}4",
        "insteekbrug, klemmen 1\u{2013}4",
    ),
    ("wire_list", "core", "ader"),
    ("wire_list", "bond/rule", "potentiaalverbinding/regel"),
    ("potential_overview", "Bond", "Potentiaalverbinding"),
    ("potential_overview", "(cable)", "(kabel)"),
    ("plc_overview", "spare", "reserve"),
    ("plc_overview", "used", "gebruikt"),
    ("io_list", "used", "gebruikt"),
    ("structure_overview", "Structure", "Structuur"),
    ("structure_overview", "Device", "Apparaat"),
    ("numbering_report", "tag", "kenmerk"),
    ("numbering_report", "deviates", "wijkt af"),
    ("network_address_list", "yes", "ja"),
    ("network_address_list", "no", "nee"),
    ("parts_list", "pc", "st."),
];

/// A Dutch project's lists hold the Dutch word in each cell where the English one stood, and none of the English; the
/// English sample's are unchanged.
#[test]
fn a_dutch_projects_lists_print_the_programs_own_words_in_dutch() {
    let ((english, terminal), (dutch, _)) = (sample(false), sample(true));
    for (kind, en, nl) in WORDS {
        let (in_english, in_dutch) = (cells(&english, kind), cells(&dutch, kind));
        assert!(
            in_english.iter().any(|c| c == en),
            "{kind}: {en} in {in_english:?}"
        );
        assert!(
            !in_english.iter().any(|c| c == nl),
            "{kind}: {nl} in {in_english:?}"
        );
        assert!(in_dutch.iter().any(|c| c == nl), "{kind}: {nl} in {in_dutch:?}");
        assert!(!in_dutch.iter().any(|c| c == en), "{kind}: {en} in {in_dutch:?}");
    }
    // The junction a wire passes, by its place, its wiring and its point; the point's name and the colour codes are the
    // project's.
    let joined = |s: &Session, words: &str| {
        cells(s, "wire_list")
            .iter()
            .any(|c| c.starts_with(&format!("101-C9 {words} -X1011:PE/a")))
    };
    assert!(joined(&english, "star at") && !joined(&dutch, "star at"));
    assert!(joined(&dutch, "ster bij"));
    assert!(cells(&dutch, "wire_list").iter().any(|c| c == "GNYE"));
    // A valve terminal's configuration without a label is named by its number.
    let configs = |s: &Session| cells_of(s, "part_data", Some(terminal));
    assert!(configs(&english).iter().any(|c| c == "Order Code 1"));
    assert!(configs(&dutch).iter().any(|c| c == "Bestelcode 1"));
}

/// The boxes on a drawn page print their titles and their headers in the project's language: the legend and the PART
/// DATA box on 902.
#[test]
fn a_dutch_projects_boxes_print_their_titles_in_dutch() {
    let ((english, _), (dutch, _)) = (sample(false), sample(true));
    let (en, nl) = (box_texts(&english, "902"), box_texts(&dutch, "902"));
    for (word, dutch_word) in [("LEGEND", "LEGENDA"), ("PART DATA", "ARTIKELGEGEVENS")] {
        assert!(en.iter().any(|t| t == word), "{word} in {en:?}");
        assert!(!en.iter().any(|t| t == dutch_word), "{dutch_word} in {en:?}");
        assert!(nl.iter().any(|t| t == dutch_word), "{dutch_word} in {nl:?}");
        assert!(!nl.iter().any(|t| t == word), "{word} in {nl:?}");
    }
    // The legend's header row: its columns' headings, in Dutch on a drawn page too.
    let heading = |texts: &[String], word: &str| texts.iter().any(|t| t == word);
    assert!(
        heading(&en, "Description") && !heading(&en, "Omschrijving"),
        "{en:?}"
    );
    assert!(
        heading(&nl, "Omschrijving") && !heading(&nl, "Description"),
        "{nl:?}"
    );
}
