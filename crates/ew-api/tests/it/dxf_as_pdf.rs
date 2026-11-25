// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The DXF set as the PDF (doc 09 §4.9), proven by reading the files back, never by their bytes: every text
//! of the PDF's display list of a page comes back through `ew-import-dxf` where the PDF has it, as high and with its
//! characters, on the sample's pages and on a small project of the test's own; there a device's texts come back as
//! the attributes of its symbol's insert under their names, the block named by its symbol in words; a cross-reference
//! links to the file of its target's page; the title block's picture is an `IMAGE` of the picture file the set writes
//! beside the pages; and the import's symbol wizard reads the coil's block with its tag slot.

use std::collections::BTreeSet;

use ew_api::Session;
use ew_api::dxf::Space;
use ew_api::output::{DxfExport, DxfLayers, ExportOptions};
use ew_derive::DeviceTag;
use ew_dl::{Item, ItemKind, ShapedRun, Transform};
use ew_ids::{Id, Uuid};
use ew_schema::{Device, Kind, PageItemKind};
use ew_units::{Nm, Pt2};
use serde_json::{Value, json};

use crate::common;

/// The entities of a DXF file read back as their group-code pairs: each code 0 opens one.
fn entities(text: &str) -> Vec<Vec<(i32, String)>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<Vec<(i32, String)>> = Vec::new();
    for pair in lines.chunks(2) {
        let code: i32 = pair[0].trim().parse().expect("a group code");
        if code == 0 {
            out.push(Vec::new());
        }
        if let Some(e) = out.last_mut() {
            e.push((code, pair.get(1).copied().unwrap_or("").to_owned()));
        }
    }
    out
}

/// The first value of `code` in an entity.
fn get(e: &[(i32, String)], code: i32) -> Option<&str> {
    e.iter().find(|(c, _)| *c == code).map(|(_, v)| v.as_str())
}

/// An insert of the model space as read back: its block and its attributes (name, text, hyperlink).
#[derive(Debug)]
struct Insert {
    block: String,
    attributes: Vec<(String, String, Option<String>)>,
}

/// The inserts of the ENTITIES section with the ATTRIBs that follow each.
fn inserts(ents: &[Vec<(i32, String)>]) -> Vec<Insert> {
    let start = ents
        .iter()
        .position(|e| get(e, 0) == Some("SECTION") && get(e, 2) == Some("ENTITIES"))
        .expect("an ENTITIES section");
    let mut out: Vec<Insert> = Vec::new();
    for e in &ents[start + 1..] {
        match get(e, 0) {
            Some("INSERT") => out.push(Insert {
                block: get(e, 2).unwrap_or_default().to_owned(),
                attributes: Vec::new(),
            }),
            Some("ATTRIB") => out
                .last_mut()
                .expect("an ATTRIB follows its INSERT")
                .attributes
                .push((
                    get(e, 2).unwrap_or_default().to_owned(),
                    get(e, 1).unwrap_or_default().to_owned(),
                    get(e, 1000).map(str::to_owned),
                )),
            Some("ENDSEC") => break,
            _ => {}
        }
    }
    out
}

/// The face's capitals over its em, as the DXF writes a text's height.
fn cap_of(run: &ShapedRun) -> f64 {
    let fonts = ew_text::bundled();
    let face = fonts.faces().find(|f| f.key == run.font).expect("a bundled face");
    f64::from(face.metrics.cap_height) / f64::from(face.metrics.units_per_em)
}

/// Where the DXF import anchors a text that starts at the first glyph of `run` under `t`: half its cap height up from
/// its baseline, square to it.
fn anchor(run: &ShapedRun, t: Transform, cap: f64) -> Pt2 {
    let rt = run
        .transform
        .then(Transform::from_translation(run.origin))
        .then(t);
    let x0 = run.layout_box.min().x;
    let at = rt.apply(Pt2::new(x0, Nm(0)));
    let ahead = rt.apply(Pt2::new(x0 + Nm(1_000_000), Nm(0)));
    let (dx, dy) = (
        (ahead.x.0 - at.x.0) as f64 / 1e6,
        (ahead.y.0 - at.y.0) as f64 / 1e6,
    );
    let half = (i128::from(run.size.0) as f64 * cap).trunc() / 2.0;
    Pt2::new(
        Nm(at.x.0 + (dy * half).round() as i64),
        Nm(at.y.0 + (-dx * half).round() as i64),
    )
}

/// One line of text of the PDF's display list: as one text (its runs' texts from the first's start, in the face of the
/// run of most characters), and as one text per run.
struct Line {
    whole: (String, Pt2),
    runs: Vec<(String, Pt2)>,
}

/// The lines of text of a sheet display list, groups followed through their transforms.
fn pdf_lines(items: &[Item], t: Transform, out: &mut Vec<Line>) {
    for item in items {
        match &item.kind {
            ItemKind::Glyphs(run) => {
                let a = (run.text.clone(), anchor(run, t, cap_of(run)));
                out.push(Line {
                    whole: a.clone(),
                    runs: vec![a],
                });
            }
            ItemKind::Group(g) => {
                let inner = g.transform.map_or(t, |gt| gt.then(t));
                let runs: Vec<&ShapedRun> = g
                    .items
                    .iter()
                    .filter_map(|i| match &i.kind {
                        ItemKind::Glyphs(r) => Some(&**r),
                        _ => None,
                    })
                    .collect();
                if !runs.is_empty() && runs.len() == g.items.len() {
                    let main = runs
                        .iter()
                        .copied()
                        .rev()
                        .max_by_key(|r| r.text.chars().count())
                        .expect("a run");
                    out.push(Line {
                        whole: (
                            runs.iter().map(|r| r.text.as_str()).collect(),
                            anchor(runs[0], inner, cap_of(main)),
                        ),
                        runs: runs
                            .iter()
                            .map(|r| (r.text.clone(), anchor(r, inner, cap_of(r))))
                            .collect(),
                    });
                } else {
                    pdf_lines(&g.items, inner, out);
                }
            }
            _ => {}
        }
    }
}

/// The distance between two points in nanometres.
fn distance(a: Pt2, b: Pt2) -> f64 {
    let (dx, dy) = ((a.x.0 - b.x.0) as f64, (a.y.0 - b.y.0) as f64);
    (dx * dx + dy * dy).sqrt()
}

/// For every page of the export, every text of the PDF's display list is a text of its DXF read back, where the PDF has
/// it (within 1 µm, the importer's rounding); the largest distance, in nanometres.
fn texts_stand_as_in_the_pdf(s: &Session, options: &ExportOptions, dxf: &DxfExport) -> f64 {
    let pages = s.export_pages(options).expect("the pages");
    assert_eq!(dxf.files.len(), pages.pages.len());
    let mut largest: f64 = 0.0;
    for (file, page) in dxf.files.iter().zip(&pages.pages) {
        let back = ew_import_dxf::convert(
            file.text.as_bytes(),
            &Default::default(),
            None,
            &Space::Model,
            &BTreeSet::new(),
            &file.name,
        )
        .expect("the file reads back");
        let texts: Vec<(String, Pt2)> = back
            .items
            .iter()
            .filter_map(|(_, _, k)| match k {
                PageItemKind::FreeText(t) => Some((t.text.as_neutral()?.to_owned(), t.at)),
                _ => None,
            })
            .collect();
        let nearest = |text: &str, at: Pt2| {
            texts
                .iter()
                .filter(|(t, _)| t == text)
                .map(|(_, p)| distance(*p, at))
                .min_by(f64::total_cmp)
        };
        let mut lines = Vec::new();
        pdf_lines(&page.content.items, Transform::IDENTITY, &mut lines);
        assert!(!lines.is_empty(), "{}: texts", file.name);
        for line in lines.iter().filter(|l| !l.whole.0.trim().is_empty()) {
            let d = nearest(&line.whole.0, line.whole.1).or_else(|| {
                line.runs
                    .iter()
                    .filter(|(t, _)| !t.trim().is_empty())
                    .map(|(t, at)| nearest(t, *at))
                    .try_fold(0.0f64, |m, d| Some(m.max(d?)))
            });
            let d =
                d.unwrap_or_else(|| panic!("{}: the text {:?} is not in the DXF", file.name, line.whole.0));
            assert!(
                d <= 1_000.0,
                "{}: the text {:?} stands {d} nm from where the PDF has it",
                file.name,
                line.whole.0
            );
            largest = largest.max(d);
        }
    }
    largest
}

#[test]
fn every_text_of_the_samples_pdf_stands_in_its_dxf_where_the_pdf_has_it() {
    let s = common::sample();
    let options = ExportOptions::default();
    let dxf = s
        .export_dxf(&options, &DxfLayers::default(), false)
        .expect("the DXF set");
    let largest = texts_stand_as_in_the_pdf(&s, &options, &dxf);
    eprintln!("the sample: the largest distance of a text from the PDF's is {largest} nm");
    // The symbols' texts are attributes of their inserts.
    let attributes: usize = dxf
        .files
        .iter()
        .map(|f| {
            inserts(&entities(&f.text))
                .iter()
                .map(|i| i.attributes.len())
                .sum::<usize>()
        })
        .sum();
    assert!(attributes > 0, "the sample's devices carry their texts");
}

fn run(s: &mut Session, name: &str, params: Value) -> ew_api::Receipt {
    common::run_cmd(s, name, params)
}

/// The device a placement made.
fn created(r: &ew_api::Receipt, kind: Kind) -> Uuid {
    r.created
        .iter()
        .find(|k| k.0 == kind)
        .map(|k| k.1.as_uuid())
        .expect("created")
}

fn tag(s: &Session, d: Uuid) -> String {
    s.query()
        .get::<DeviceTag>(Id::<Device>::from_uuid(d))
        .as_ref()
        .as_ref()
        .map(|t| t.short())
        .expect("a tag")
}

/// Places the starter symbol `key` on `page` at grid step `(x, y)` for `target`.
fn place(s: &mut Session, page: Uuid, key: &str, (x, y): (i64, i64), target: Value) -> ew_api::Receipt {
    const STEP: i64 = 7_937_500;
    let symbol = s.starter_symbol_copy(key).expect("a starter symbol");
    let copies = s
        .starter_embed(key)
        .map_or_else(Vec::new, |c| c["copies"].as_array().cloned().unwrap_or_default());
    run(
        s,
        "device.place",
        json!({"page": page.to_string(), "symbol": symbol.to_string(), "pos": [x * STEP, y * STEP],
            "target": target, "copies": copies}),
    )
}

/// A project of the test's own: a coil on its first page and its contact on a second one, each pointing at the other,
/// an input card drawn as a black box, and a logo in the title block.
#[test]
fn a_small_projects_dxf_carries_its_devices_texts_links_and_logo() {
    let mut s = common::new_project(1877);
    let first = common::first_page(&s);
    let second = run(
        &mut s,
        "page.create",
        json!({"kind": "multi_line", "after": first.to_string()}),
    )
    .outcome
    .primary
    .expect("the new page")
    .1
    .as_uuid();
    let coil = place(
        &mut s,
        first,
        "ew.iec.coil@qet10",
        (30, 12),
        json!({"new_device": {"letter": "K"}}),
    );
    let device = created(&coil, Kind::Device);
    place(
        &mut s,
        second,
        "ew.iec.contact-no@qet10",
        (30, 12),
        json!({"join_device": {"device": device.to_string()}}),
    );
    crate::plc_window::place(&mut s, "EW-GEN-DI-1", 1);
    let strip = created(
        &run(&mut s, "device.create", json!({"letter": "X", "strip": {}})),
        Kind::Device,
    );
    let part = common::library_part(&s, "EW-GEN-TRM-1");
    let terminal = "ew.iec.terminal@qet10";
    let copies = s
        .starter_embed_part(part, terminal)
        .expect("the closure resolves")
        .unwrap_or_else(|| json!({"copies": []}));
    let symbol = s.starter_symbol_copy(terminal).expect("a starter terminal");
    run(
        &mut s,
        "device.place",
        json!({"page": first.to_string(), "symbol": symbol.to_string(), "pos": [40 * 7_937_500, 24 * 7_937_500],
            "target": {"new_terminal": {"strip": strip.to_string(), "designation": "1", "part": part.to_string()}},
            "copies": copies["copies"]}),
    );
    let logo = common::red_png(8, 4);
    run(
        &mut s,
        "project.company.set",
        json!({"logo": ew_api::base64_encode(&logo)}),
    );

    let options = ExportOptions::default();
    let dxf = s
        .export_dxf(&options, &DxfLayers::default(), false)
        .expect("the DXF set");
    let largest = texts_stand_as_in_the_pdf(&s, &options, &dxf);
    eprintln!("the test's project: the largest distance of a text from the PDF's is {largest} nm");
    let pages = s.export_pages(&options).expect("the pages");
    let file_of = |n: usize| ew_export::dxf_file_name(&pages.pages[n].label);

    // The coil's insert: its block named by its symbol, its tag and its cross-reference to the contact's page.
    let k = tag(&s, device);
    let device_insert = |file: &str| -> Insert {
        let text = &dxf
            .files
            .iter()
            .find(|f| f.name == file)
            .expect("the page's file")
            .text;
        inserts(&entities(text))
            .into_iter()
            .find(|i| i.attributes.iter().any(|(n, v, _)| n == "TAG" && *v == k))
            .unwrap_or_else(|| panic!("{file}: an insert tagged {k}"))
    };
    let coil_insert = device_insert(&file_of(0));
    let coil_symbol = s.starter_symbol_copy("ew.iec.coil@qet10").expect("the coil");
    let name = {
        let q = s.query();
        let copy = q
            .symbols()
            .get(Id::from_uuid(coil_symbol))
            .expect("the coil's copy")
            .clone();
        ew_derive::text_in(&copy.item.name, ew_derive::print_language(&q).as_ref()).to_owned()
    };
    // The symbol's name in words, a character a DXF name cannot hold as `_`, and its variant.
    let held: String = name
        .chars()
        .map(|c| if "<>/\\\":;?*|,='`".contains(c) { '_' } else { c })
        .collect();
    assert_eq!(coil_insert.block, format!("{held} R0"));
    let link_to = |i: &Insert| -> Vec<String> {
        i.attributes
            .iter()
            .filter(|(n, _, _)| n.starts_with("CROSS_REFERENCE"))
            .filter_map(|(_, _, l)| l.clone())
            .collect()
    };
    assert_eq!(link_to(&coil_insert), [file_of(1)], "{coil_insert:?}");
    assert_eq!(link_to(&device_insert(&file_of(1))), [file_of(0)]);
    // The card's pin texts are its connection points' designations.
    let card = inserts(&entities(&dxf.files[0].text))
        .into_iter()
        .find(|i| i.attributes.iter().any(|(n, _, _)| n.starts_with("DESIGNATION_")));
    assert!(card.is_some(), "the black box's pin texts");
    // The terminal carries its strip's tag.
    let strip_tag = tag(&s, strip);
    assert!(
        inserts(&entities(&dxf.files[0].text)).iter().any(|i| i
            .attributes
            .iter()
            .any(|(n, v, _)| n == "STRIP" && *v == strip_tag)),
        "the terminal's strip tag {strip_tag}"
    );

    // The title block's logo: an IMAGE on the frame layer that names the picture file written beside the pages.
    let [picture] = dxf.pictures.as_slice() else {
        panic!(
            "one picture: {:?}",
            dxf.pictures.iter().map(|p| &p.name).collect::<Vec<_>>()
        )
    };
    assert_eq!(&*picture.bytes, logo.as_slice());
    for file in &dxf.files {
        let ents = entities(&file.text);
        let image = ents
            .iter()
            .find(|e| get(e, 0) == Some("IMAGE"))
            .unwrap_or_else(|| panic!("{}: the logo", file.name));
        assert_eq!(get(image, 8), Some(DxfLayers::default().frame.as_str()));
        let def = ents
            .iter()
            .find(|e| get(e, 0) == Some("IMAGEDEF") && get(e, 5) == get(image, 340))
            .expect("its definition");
        assert_eq!(get(def, 1), Some(picture.name.as_str()));
    }

    // The import's symbol wizard reads the coil's block as a symbol with its tag slot.
    let symbols = ew_import_dxf::symbols(
        dxf.files[0].text.as_bytes(),
        &Default::default(),
        None,
        ew_units::Grid::MM4,
        &dxf.files[0].name,
    )
    .expect("the blocks read back");
    let coil_block = symbols
        .iter()
        .find(|b| b.name == coil_insert.block)
        .expect("the coil's block");
    assert!(coil_block.tag.is_some(), "the TAG definition is the tag slot");
}

/// A symbol named in many languages, as a QElectroTech element is (`ar`, `en`, `nl`), names its block in the project's
/// language: an English project's in English, a Dutch one's in Dutch, and a project that names no language in English,
/// not in the first language of the name.
#[test]
fn a_block_is_named_in_the_projects_language_else_in_english() {
    let names = |language: Option<&str>| -> BTreeSet<String> {
        let (mut project, records, mut lib) = common::sample_records();
        project.languages =
            language.map(|l| serde_json::from_value(json!({"source": l})).expect("languages"));
        for doc in &mut lib {
            if let ew_schema::LibDocument::Symbol(s) = doc {
                s.item.name = serde_json::from_value(json!({"ar": "رمز", "en": "Symbol", "nl": "Symbool"}))
                    .expect("a name");
            }
        }
        let s = common::session_of(project, records, lib);
        let options = ExportOptions {
            pages: ew_api::output::PageSelection::parse("101").expect("a folio"),
            ..ExportOptions::default()
        };
        let dxf = s
            .export_dxf(&options, &DxfLayers::default(), false)
            .expect("the DXF set");
        entities(&dxf.files[0].text)
            .iter()
            .filter(|e| get(e, 0) == Some("BLOCK"))
            .filter_map(|e| get(e, 2))
            .filter(|n| !n.starts_with('*'))
            .map(|n| n.split(' ').next().unwrap_or_default().to_owned())
            // The blocks of the renamed symbols (a junction's symbol is no copy of the project's).
            .filter(|w| ["رمز", "Symbol", "Symbool"].contains(&w.as_str()))
            .collect()
    };
    let words = |w: &str| BTreeSet::from([w.to_owned()]);
    assert_eq!(names(Some("en")), words("Symbol"));
    assert_eq!(names(Some("nl")), words("Symbool"));
    assert_eq!(names(None), words("Symbol"));
}

/// A text is named by what printed it, not by the slot box it lies in: a coil's first designation moved onto its held
/// tag keeps the name `DESIGNATION_1`, and the tag stays `TAG`.
#[test]
fn a_moved_text_keeps_the_name_of_its_own_slot() {
    let mut s = common::new_project(1877);
    let page = common::first_page(&s);
    let coil = place(
        &mut s,
        page,
        "ew.iec.coil@qet10",
        (30, 12),
        json!({"new_device": {"letter": "K"}}),
    );
    let (device, rep) = (created(&coil, Kind::Device), created(&coil, Kind::Representation));
    let (k, symbol) = (tag(&s, device), s.starter_symbol_copy("ew.iec.coil@qet10").expect("the coil"));
    let slot_of = |kind: ew_schema::SlotKind| -> (u16, Pt2) {
        let q = s.query();
        let copy = q.symbols().get(Id::from_uuid(symbol)).expect("the coil's copy").clone();
        let (i, slot) = copy
            .item
            .text_slots
            .iter()
            .enumerate()
            .find(|(_, t)| t.kind == kind)
            .expect("the slot");
        (u16::try_from(i).expect("an index"), slot.pos)
    };
    let (tag_slot, tag_at) = slot_of(ew_schema::SlotKind::Tag);
    let (pin_slot, _) = slot_of(ew_schema::SlotKind::CpDesignation(0));
    // The tag held where its slot puts it, so the layout does not move it off the designation.
    run(
        &mut s,
        "rep.text_set",
        json!({"rep": rep.to_string(), "slot": tag_slot, "pos": [tag_at.x.0, tag_at.y.0]}),
    );
    // The designation's anchor where its box's middle is the tag box's (the coil stands upright: its text frame is the
    // page's, shifted to its origin).
    let boxes = s.page_slot_texts("1");
    let printed = |slot: u16| {
        boxes
            .iter()
            .find(|b| b.rep == rep && b.slot == slot)
            .copied()
            .expect("the slot's box")
    };
    let middle = |r: ew_units::Rect| Pt2::new(Nm((r.min().x.0 + r.max().x.0) / 2), Nm((r.min().y.0 + r.max().y.0) / 2));
    let (to, pin) = (middle(printed(tag_slot).rect), printed(pin_slot));
    let from = middle(pin.rect);
    let moved = Pt2::new(
        to.x - (from.x - pin.at.x) - pin.frame.origin.x,
        to.y - (from.y - pin.at.y) - pin.frame.origin.y,
    );
    run(
        &mut s,
        "rep.text_set",
        json!({"rep": rep.to_string(), "slot": pin_slot, "pos": [moved.x.0, moved.y.0]}),
    );
    let dxf = s
        .export_dxf(&ExportOptions::default(), &DxfLayers::default(), false)
        .expect("the DXF set");
    let insert = inserts(&entities(&dxf.files[0].text))
        .into_iter()
        .find(|i| i.attributes.iter().any(|(_, v, _)| *v == k))
        .expect("the coil's insert");
    let names: BTreeSet<&str> = insert.attributes.iter().map(|(n, _, _)| n.as_str()).collect();
    assert!(names.contains("TAG") && names.contains("DESIGNATION_1"), "{insert:?}");
    assert!(!names.contains("TAG_2"), "{insert:?}");
}
