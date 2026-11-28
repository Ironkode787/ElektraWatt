// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A list's pages print what its block chooses (doc 09 §3.8): the columns with their headings and widths, the
//! caption, the legend and the rows the model does not hold, in the project's language; the list pages on the
//! set's paper; the terminal diagram that names each terminal's strip. Over copies of the sample.

use crate::common;

use ew_api::Session;
use ew_api::output::{PageFrameOf, PageTableOf, RowExport, page_layer, row_export};
use ew_derive::{PageOrder, PageRef, text_in};
use ew_dl::{Item, ItemKind, Layer};
use ew_mltext::LangTag;
use ew_reports::{BlockRows, Registry};
use ew_scene::table::TableView;
use serde_json::{Value, json};

/// The ID of the sample's page-oriented block whose first folio is `folio`.
fn block_at(s: &Session, folio: u32) -> String {
    let q = s.query();
    q.report_blocks()
        .iter()
        .find(|(_, b)| b.first_folio == Some(folio) && b.embedded.is_none())
        .map(|(id, _)| id.as_uuid().to_string())
        .unwrap_or_else(|| panic!("a block at {folio}"))
}

fn set(s: &mut Session, block: &str, path: &str, value: Value) {
    common::run_cmd(
        s,
        "fields.set",
        json!({"kind": "report_block", "targets": [block], "path": path, "value": value}),
    );
}

/// The generated pages of the block at `folio`, in page order.
fn pages_of(s: &Session, folio: u32) -> Vec<PageRef> {
    let block = block_at(s, folio);
    s.query()
        .get::<PageOrder>(())
        .entries
        .iter()
        .filter(|e| matches!(e.page, PageRef::Generated { block: b, .. } if b.as_uuid().to_string() == block))
        .map(|e| e.page)
        .collect()
}

/// The printed table of `page`.
fn table(s: &Session, page: PageRef) -> TableView {
    s.query()
        .get::<PageTableOf>(page)
        .as_ref()
        .clone()
        .expect("a list page")
}

/// The texts of the glyph runs of `page`'s frame layer, groups included.
fn frame_texts(s: &Session, page: PageRef) -> Vec<String> {
    fn walk(items: &[Item], out: &mut Vec<String>) {
        for item in items {
            match &item.kind {
                ItemKind::Glyphs(run) => out.push(run.text.clone()),
                ItemKind::Group(g) => walk(&g.items, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(&page_layer(&s.query(), page, Layer::Frame).list.items, &mut out);
    out
}

/// A block's own columns print in its order, each with its width and its heading (or the kind's), and its
/// caption and legend print on every page; the CSV keeps every column of the kind.
#[test]
fn a_list_prints_the_columns_headings_widths_caption_and_legend_its_block_chooses() {
    let mut s = common::sample();
    let first = pages_of(&s, 903)[0];
    let before = table(&s, first);
    let at = |key: &str| {
        before
            .columns
            .iter()
            .position(|c| c.header == english("terminal_diagram", key))
    };
    let (side1, terminal) = (at("side1").expect("side 1"), at("terminal").expect("terminal"));
    let block = block_at(&s, 903);
    set(
        &mut s,
        &block,
        "columns",
        json!([
            {"key": "side1", "weight": 30},
            {"key": "terminal", "weight": 10, "heading": "Klem"},
        ]),
    );
    set(&mut s, &block, "caption", json!("Terminals of the sample"));
    set(&mut s, &block, "legend", json!("PE: protective earth"));
    let after = table(&s, first);
    let heads: Vec<(&str, u32)> = after
        .columns
        .iter()
        .map(|c| (c.header.as_str(), c.weight))
        .collect();
    assert_eq!(
        heads,
        [(english("terminal_diagram", "side1").as_str(), 30), ("Klem", 10)]
    );
    assert_eq!(after.caption.as_deref(), Some("Terminals of the sample"));
    assert_eq!(after.legend.as_deref(), Some("PE: protective earth"));
    // The same rows, their cells those of the two columns; the caption and the legend take a row each.
    for (row, was) in after.rows.iter().zip(&before.rows) {
        assert_eq!(row.cells, [was.cells[side1].clone(), was.cells[terminal].clone()]);
    }
    let q = s.query();
    let rows = q.get::<BlockRows>(ew_ids::Id::from_uuid(block.parse().expect("a UUID")));
    let lists = pages_of(&s, 903);
    let listed: usize = lists.iter().map(|p| table(&s, *p).rows.len()).sum();
    assert_eq!(listed, rows.rows.len(), "every row is on a page");
    // The CSV keeps every column of the kind.
    let csv = row_export(&q, &RowExport::Report, Some("903"))
        .expect("903's rows")
        .csv;
    let header = csv.lines().next().expect("a header");
    for c in Registry::builtin()
        .by_name("terminal_diagram")
        .expect("the kind")
        .columns()
    {
        assert!(header.contains(c.english()), "{} in {header}", c.key);
    }
}

/// The English heading of the column `key` of report kind `kind`.
fn english(kind: &str, key: &str) -> String {
    Registry::builtin()
        .by_name(kind)
        .and_then(|k| k.columns().into_iter().find(|c| c.key == key))
        .map(|c| c.english().to_owned())
        .unwrap_or_else(|| panic!("{kind} has {key}"))
}

/// In a project whose language is Dutch a list prints its headings and its title block's labels in Dutch;
/// before, every heading and label printed its English. The wire list's block chooses no columns, so it prints the
/// columns its kind prints (`ReportKindDef::printed`, doc 13 §3.1), not every column of the list.
#[test]
fn a_dutch_project_prints_its_list_heads_and_title_block_labels_in_dutch() {
    let mut s = common::sample();
    common::run_cmd(
        &mut s,
        "project.settings.set",
        json!({ "member": "languages", "value": {"source": "nl-NL", "date": "dd_mm_yyyy", "decimal_separator": "comma"} }),
    );
    let nl: LangTag = "nl-NL".parse().expect("a tag");
    let page = pages_of(&s, 907)[0];
    let heads: Vec<String> = table(&s, page).columns.into_iter().map(|c| c.header).collect();
    // The page prints each end of a wire on a line of its own (doc 13 §3.1): its heads are that form's.
    let def = Registry::builtin().by_name("wire_list").expect("the kind");
    let form = ew_reports::per_end::per_end_of(
        &def.columns(),
        def.printed().expect("the wire list names its printed columns"),
    )
    .expect("the wire list pairs its ends");
    let dutch: Vec<String> = form
        .iter()
        .map(|c| text_in(&c.heading, Some(&nl)).to_owned())
        .collect();
    let english: Vec<String> = form
        .iter()
        .map(|c| text_in(&c.heading, None).to_owned())
        .collect();
    assert_eq!(heads, dutch);
    assert_ne!(heads, english);
    let texts = frame_texts(&s, page);
    assert!(texts.iter().any(|t| t.starts_with("Omschrijving:")), "{texts:?}");
    assert!(!texts.iter().any(|t| t.starts_with("Description:")), "{texts:?}");
}

/// A row of the block's own is listed with the kind's rows, by the block's sorting among them, in the CSV
/// too, and stays when the model changes: the list is made anew from the model and the block.
#[test]
fn rows_of_your_own_are_listed_sorted_and_kept_when_the_model_changes() {
    let mut s = common::sample();
    let block = block_at(&s, 903);
    set(
        &mut s,
        &block,
        "added_rows",
        json!([{"cells": {"terminal": "1a", "label": "spare"}}]),
    );
    let id = ew_ids::Id::from_uuid(block.parse().expect("a UUID"));
    let terminal = |s: &Session| {
        let rows = s.query().get::<BlockRows>(id);
        let at = rows.column("terminal").expect("a terminal column");
        rows.rows
            .iter()
            .position(|r| r.id.to_string().starts_with("n:added"))
            .map(|i| (i, rows.rows.len(), rows.rows[i].cells[at].clone()))
    };
    let (i, n, cell) = terminal(&s).expect("the added row");
    assert_eq!(
        (i, cell),
        (n - 1, ew_reports::Cell::Ident("1a".to_owned())),
        "unsorted: after the kind's rows"
    );
    // Sorted by terminal, it stands among the model's rows ("1a" before "2").
    set(&mut s, &block, "sort", json!([{"key": "terminal"}]));
    let (i, n, _) = terminal(&s).expect("the added row");
    assert!(i < n - 1, "{i} of {n}");
    let csv = row_export(&s.query(), &RowExport::Report, Some("903"))
        .expect("903's rows")
        .csv;
    assert!(
        csv.lines().any(|l| l.contains(",1a,") && l.contains("spare")),
        "{csv}"
    );
    // A change of the model leaves it where it is.
    let page = common::first_page(&s);
    common::run_cmd(
        &mut s,
        "fields.set",
        json!({"kind": "page", "targets": [page.to_string()], "path": "title", "value": "Supply"}),
    );
    assert_eq!(terminal(&s).map(|(i, _, _)| i), Some(i));
}

/// The terminal diagram names each terminal's strip, so two strips' terminals of one designation read apart.
#[test]
fn two_strips_terminals_of_one_designation_read_apart_in_the_terminal_diagram() {
    let s = common::sample();
    let mut pe = Vec::new();
    for page in pages_of(&s, 903) {
        let t = table(&s, page);
        let col = |key: &str| {
            t.columns
                .iter()
                .position(|c| c.header == english("terminal_diagram", key))
                .unwrap_or_else(|| panic!("a {key} column"))
        };
        let (strip, terminal) = (col("strip"), col("terminal"));
        pe.extend(
            t.rows
                .iter()
                .filter(|r| r.cells.get(terminal).is_some_and(|c| c == "PE"))
                .map(|r| r.cells[strip].clone()),
        );
    }
    assert!(pe.len() > 1, "the sample has more than one terminal PE: {pe:?}");
    assert!(pe.iter().all(|s| !s.is_empty()), "{pe:?}");
    let strips: std::collections::BTreeSet<&String> = pe.iter().collect();
    assert_eq!(
        strips.len(),
        pe.len(),
        "every terminal PE names another strip: {pe:?}"
    );
}

/// The starter A4 frame with the grid made a frame of the sample's own (its library ID the project's, no key, as an
/// import makes one): its copy ID and its `lib/` JSON form.
fn own_frame(s: &Session) -> (String, Value) {
    let text = ew_api::output::SHIPPED_FORMS
        .iter()
        .find(|t| t.contains("\"ew.frame.qet-a4-landscape-grid\""))
        .expect("the starter frame with the grid");
    let ew_schema::LibDocument::Form(mut copy) = ew_schema::LibDocument::from_json(text).expect("a form")
    else {
        panic!("not a form");
    };
    let project = s.query().project().id.as_uuid();
    let item = ew_ids::Uuid::from_u128(0x7e01);
    copy.library_id = ew_ids::Id::from_uuid(project);
    copy.item_id = ew_ids::Id::from_uuid(item);
    copy.id = ew_ids::v5::lib_copy_id(project, item);
    copy.item.meta.key = None;
    let doc = ew_schema::LibDocument::Form(copy.clone());
    copy.sha256 = ew_schema::Sha256(ew_format::library_copy_hash(&doc).expect("a hash"));
    let id = copy.id.as_uuid().to_string();
    (
        id,
        serde_json::to_value(ew_schema::LibDocument::Form(copy)).expect("its JSON"),
    )
}

/// A new list starts on the set's own frame (doc 09 §3.8): without frame defaults the frame most drawn pages print
/// on, which a project of its own frames has, and the Form row names that frame by its paper and its pages, never by
/// an ID; frame defaults on the starter forms start it on the starter frame.
#[test]
fn a_new_list_starts_on_the_frame_most_pages_print_on() {
    let mut s = common::sample();
    assert_eq!(s.new_list_form(), None, "the sample is on the starter forms");
    let (own, json) = own_frame(&s);
    common::run_cmd(&mut s, "library.embed", json!({ "copies": [json] }));
    common::run_cmd(
        &mut s,
        "project.settings.set",
        json!({ "member": "frames", "value": {} }),
    );
    let q = s.query();
    let pages: Vec<String> = q
        .pages()
        .iter()
        .filter(|(_, p)| p.kind != ew_schema::PageType::TitlePage)
        .map(|(id, _)| id.as_uuid().to_string())
        .collect();
    drop(q);
    let (most, rest) = pages.split_at(pages.len() / 2 + 1);
    common::run_cmd(
        &mut s,
        "fields.set",
        json!({"kind": "page", "targets": most, "path": "form", "value": own}),
    );
    assert_eq!(s.new_list_form().map(|u| u.to_string()), Some(own.clone()));
    // The Form row offers the project's forms by name: the own frame by its paper and the pages it is on.
    let choices = s.field_choices("page", "form", "");
    let named = choices
        .iter()
        .find(|c| c.value == json!(own))
        .expect("the own frame is offered");
    assert!(
        named.text.starts_with("Frame A4 landscape, on "),
        "{}",
        named.text
    );
    assert!(!choices.iter().any(|c| c.text.contains(&own)), "no ID is a name");
    let texts: std::collections::BTreeSet<&String> = choices.iter().map(|c| &c.text).collect();
    assert_eq!(texts.len(), choices.len(), "every form reads apart");
    // Typed by its name, the row takes it.
    let parsed = s
        .parse_field_input("page", "form", &named.text)
        .expect("its name");
    assert_eq!(parsed.value, json!(own));
    assert!(!rest.is_empty());
    // Frame defaults on a starter frame: a new list starts on the starter frame.
    let starter = ew_api::shipped_frames(ew_api::output::GRID_FRAME).expect("the shipped frames");
    common::run_cmd(
        &mut s,
        "project.settings.set",
        json!({ "member": "frames", "value": starter }),
    );
    assert_eq!(s.new_list_form(), None);
}

/// The journey's first change of the title block makes the starter forms the project's own (doc 09 §3.9): a copy made
/// from a starter stands for it, so a new list still starts on the frame without rulers, the project's copy of it,
/// which prints the changed block, not on the drawn pages' frame with rulers.
#[test]
fn after_the_first_change_of_the_title_block_a_new_list_starts_on_the_frame_without_rulers() {
    let mut s = common::sample();
    let facts = s.frame_facts("101").expect("page 101's frame");
    let block = facts.block.expect("the title block").to_string();
    let mut item = facts.block_item.clone().expect("its item");
    item["cells"][1]["label"] = json!("Drawing no.:");
    common::run_cmd(
        &mut s,
        "form.edit",
        json!({"copy": block, "form": item,
            "forks": facts.block_forks.iter().map(|u| u.to_string()).collect::<Vec<_>>()}),
    );
    let after = s.frame_facts("101").expect("page 101's frame");
    let list = s
        .new_list_form()
        .expect("the project's copy of the frame without rulers");
    let q = s.query();
    let form = q
        .forms()
        .get(ew_ids::Id::from_uuid(list))
        .expect("a form the project holds");
    assert_eq!(
        form.item.meta.key.as_ref().map(|k| k.as_str()),
        Some(ew_api::output::NOGRID_FRAME),
        "the frame without rulers"
    );
    assert_eq!(
        form.library_id.as_uuid(),
        q.project().id.as_uuid(),
        "the project's copy"
    );
    assert_eq!(
        form.item.title_block.map(|b| b.as_uuid()),
        after.block,
        "it prints the changed block"
    );
}

/// A set whose frame is the starter A3 frame prints its lists on it, as many rows to a page as its list area
/// holds, more than the A4 one.
#[test]
fn a_set_on_a3_prints_its_lists_on_a3() {
    let mut s = common::sample();
    let a3 = ew_api::shipped_frames(ew_api::output::A3_GRID_FRAME).expect("the shipped frames");
    common::run_cmd(
        &mut s,
        "project.settings.set",
        json!({ "member": "frames", "value": a3 }),
    );
    let q = s.query();
    let page = pages_of(&s, 907)[0];
    let frame = q.get::<PageFrameOf>(page).as_ref().clone().expect("a frame");
    assert_eq!(
        (frame.form.sheet.width.0, frame.form.sheet.height.0),
        (420_000_000, 297_000_000)
    );
    let a3_rows = frame
        .form
        .list_area(frame.title_block.as_deref(), false, false)
        .rows;
    assert!(a3_rows > ew_schema::HOUSE_ROWS_PER_PAGE, "{a3_rows}");
    // The first page is full: its head and each wire's two lines, short by less than a wire.
    let first = i64::try_from(table(&s, page).rows.len()).expect("rows");
    assert!(first <= a3_rows && a3_rows - first < 2, "{first} of {a3_rows}");
}

/// The wire list's first page says how to read the papers (doc 13 §3.1), from the project's settings: the reserve and
/// the door loop a length holds, the marking, the strips whose field is on side 1; then each wire on two lines, the
/// From end above the To end. Its second page says nothing of it.
#[test]
fn the_wire_lists_first_page_says_how_to_read_it_from_the_settings() {
    let (mut project, mut records, lib) = common::sample_records();
    let ws = &mut project.settings.workshop;
    ws.reserve = Some(ew_units::Nm::from_mm(80));
    ws.door_loop = Some(ew_units::Nm::from_mm(250));
    ws.marking = ew_schema::Marking::WireNumber;
    for r in &mut records {
        if let ew_schema::Record::Device(d) = r
            && let Some(strip) = d.aspects.strip.as_mut()
        {
            strip.field_side = ew_schema::Side::Side1;
        }
    }
    let s = common::session_of(project, records, lib);
    let pages = pages_of(&s, 907);
    let first: Vec<String> = table(&s, pages[0])
        .rows
        .iter()
        .map(|r| r.cells.join("|"))
        .collect();
    assert_eq!(first[0], "How to read these papers", "{first:#?}");
    for words in [
        "50 mm at each end",
        "80 mm at each end and 250 mm across a door's hinge",
        "A wire's marker carries the wire's number.",
        "On -X1011, -X1021, -X1031 it is the other way round: side 1 is the field.",
    ] {
        assert_eq!(
            first.iter().any(|l| l.contains(words)),
            words != "50 mm at each end",
            "{words}: {first:#?}"
        );
    }
    // The first wire: its From end, then its To end under it.
    let at = first
        .iter()
        .position(|l| l.contains("|From|"))
        .expect("a wire");
    assert!(first[at + 1].starts_with("|To|"), "{first:#?}");
    let second = table(&s, pages[1]);
    assert!(
        !second.rows.iter().any(|r| r.cells[0] == "How to read these papers"),
        "the head is the first page's"
    );
}
