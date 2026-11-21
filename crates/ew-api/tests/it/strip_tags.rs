// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Doc 08 §6.1's and §6.7's printing, through the page scene the PDF and SVG writers
//! draw: a terminal drawn alone prints its designation, or its function text under its strip's `label_mode`
//! FunctionText; the strip's tag prints once per page at the strip's first terminal when the page holds no segment of
//! the strip; `tag_print` PerPage prints a strip's tag at its first segment on each page. The sample's records, edited
//! before the session opens: strip -X1011's terminals stand alone on 101; two segments of strip -X1031 are added on
//! page 103 beside its terminals drawn alone. Where a segment draws its terminals' connection points: the page tables
//! and the page's CPs of a strip drawn by its segments alone.

use std::collections::{BTreeMap, BTreeSet};

use crate::common;

use ew_api::Session;
use ew_api::geometry::Dir;
use ew_derive::ew_mltext::MlText;
use ew_derive::{PageOrder, PageRef};
use ew_dl::{Item, ItemKind, Layer, Transform};
use ew_ids::{Id, Uuid};
use ew_schema::{Device, Function, LibDocument, Page, Record, Representation, TagPrint};
use ew_units::Pt2;

/// The sample's records with the edits of a test.
struct Sample {
    project: ew_schema::Project,
    records: Vec<Record>,
    lib: Vec<LibDocument>,
    /// The sample's device IDs by displayed tag.
    tags: Session,
}

impl Sample {
    fn new() -> Sample {
        let (project, records, lib) = common::sample_records();
        Sample {
            project,
            records,
            lib,
            tags: common::sample(),
        }
    }

    fn session(self) -> Session {
        common::session_of(self.project, self.records, self.lib)
    }

    /// The device tagged `tag` ("-X1011", a terminal "-X1011:L1").
    fn device_mut(&mut self, tag: &str) -> &mut Device {
        let id = self
            .tags
            .resolve_reference(&format!("tag:{tag}"))
            .unwrap_or_else(|e| panic!("no device {tag}: {e:?}"));
        self.records
            .iter_mut()
            .find_map(|r| match r {
                Record::Device(d) if d.id.as_uuid() == id => Some(d),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no device record {tag}"))
    }

    /// The main function of the device tagged `tag`.
    fn function(&mut self, tag: &str) -> Id<Function> {
        let device = self.device_mut(tag).id;
        self.records
            .iter()
            .find_map(|r| match r {
                Record::Function(f) if f.device == Some(device) && f.role == ew_schema::FnRole::Main => {
                    Some(f.id)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("no function of {tag}"))
    }

    /// The representation of the main function of the device tagged `tag`.
    fn rep_mut(&mut self, tag: &str) -> &mut Representation {
        let function = self.function(tag);
        self.records
            .iter_mut()
            .find_map(|r| match r {
                Record::Representation(x) if x.functions.contains(&function) => Some(x),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no representation of {tag}"))
    }

    fn page(&self, number: u32) -> Id<Page> {
        self.records
            .iter()
            .find_map(|r| match r {
                Record::Page(p) if p.folio.number == number => Some(p.id),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no page {number}"))
    }

    fn set_function_text(&mut self, tag: &str, text: &str) {
        self.device_mut(tag).function_text = MlText::neutral(text);
    }

    fn strip_mut(&mut self, tag: &str) -> &mut ew_schema::Strip {
        self.device_mut(tag).aspects.strip.as_mut().expect("a strip")
    }

    /// Two segments of strip -X1031 on page 103, at `(x, y)` connection-grid steps; their IDs in the order given. The
    /// built-in strip-segment symbol and its function definition are added to the library, and the strip, whose
    /// terminals the sample draws alone, gets the segments' function.
    fn add_x1031_segments(&mut self, at: [(i64, i64); 2]) -> Vec<Uuid> {
        let content = crate::view::repo().join("content");
        for file in [
            "symbols/qet10/ew.gen.strip-segment@qet10.json",
            "fndefs/ew.fn.strip-segment.json",
        ] {
            let text = std::fs::read_to_string(content.join(file)).expect("a built-in library file");
            self.lib
                .push(LibDocument::from_json(&text).expect("a library document"));
        }
        let symbol = symbol_copy(self, "ew.gen.strip-segment@qet10");
        let strip = self.device_mut("-X1031").id;
        let function = Uuid::from_u128(0x5e90_0000);
        self.records.push(
            Record::from_value(serde_json::json!({
                "$k": "function", "id": function.to_string(), "device": strip.to_string(),
                "def": "a0d5b37a-fb2c-52dd-9633-2a47f11c9c22"
            }))
            .expect("the strip's function"),
        );
        let page = self.page(103);
        at.iter()
            .enumerate()
            .map(|(n, &(x, y))| {
                let id = Uuid::from_u128(0x5e90_0001 + n as u128);
                self.records.push(
                    Record::from_value(serde_json::json!({
                        "$k": "representation", "id": id.to_string(), "functions": [function.to_string()],
                        "page": page.to_string(), "symbol": symbol.to_string(), "pos": [x * STEP, y * STEP],
                        "rep_type": "strip_segment"
                    }))
                    .expect("a strip segment"),
                );
                id
            })
            .collect()
    }
}

/// One connection-grid step of the sample's pages (the QET 10 px grid, 7937500/3 nm), times three.
const STEP: i64 = 7_937_500;

/// The texts the labels layer of folio `number` draws, by the representation they belong to, each list sorted.
fn texts(s: &Session, number: u32) -> BTreeMap<Uuid, Vec<String>> {
    drawn(s, number)
        .into_iter()
        .fold(BTreeMap::new(), |mut m, (entity, text, _)| {
            m.entry(entity).or_insert_with(Vec::new).push(text);
            m
        })
        .into_iter()
        .map(|(k, mut v)| {
            v.sort();
            (k, v)
        })
        .collect()
}

/// The texts of the labels layer of folio `number`: the entity, the text and its layout quad on the page.
fn drawn(s: &Session, number: u32) -> Vec<(Uuid, String, [Pt2; 4])> {
    let q = s.query();
    let page = q
        .pages()
        .iter()
        .find(|(_, p)| p.folio.number == number)
        .map(|(id, _)| id)
        .expect("the folio");
    let order = q.get::<PageOrder>(());
    let entry = order
        .entries
        .iter()
        .find(|e| e.page == PageRef::Drawn(page))
        .expect("the page in the page order");
    let scene = ew_api::output::page_scene(&q, entry);
    let mut out = Vec::new();
    for (layer, list) in &scene.layers {
        if *layer == Layer::Labels {
            collect(&list.items, &[], None, &mut out);
        }
    }
    out
}

fn collect(
    items: &[Item],
    groups: &[Transform],
    inherited: Option<Uuid>,
    out: &mut Vec<(Uuid, String, [Pt2; 4])>,
) {
    for item in items {
        let owner = item.entity.map(|e| e.0).or(inherited);
        match &item.kind {
            ItemKind::Glyphs(run) => {
                if let Some(owner) = owner {
                    let quad = run
                        .layout_quad()
                        .map(|q| groups.iter().rev().fold(q, |p, t| t.apply(p)));
                    out.push((owner, run.text.clone(), quad));
                }
            }
            ItemKind::Group(g) => {
                let mut inner = groups.to_vec();
                inner.extend(g.transform);
                collect(&g.items, &inner, owner, out);
            }
            _ => {}
        }
    }
}

/// The representation IDs of the main functions of the devices `tags`.
fn rep_ids(g: &mut Sample, tags: &[&str]) -> Vec<Uuid> {
    tags.iter().map(|t| g.rep_mut(t).id.as_uuid()).collect()
}

/// The texts drawn at `reps`, in that order (a representation with none: empty).
fn at(texts: &BTreeMap<Uuid, Vec<String>>, reps: &[Uuid]) -> Vec<Vec<String>> {
    reps.iter()
        .map(|r| texts.get(r).cloned().unwrap_or_default())
        .collect()
}

/// The sample with the representations of strip -X1011's terminals "L1", "L2" and "PE" (in that order) as IDs.
fn sample_with_terminal_reps() -> (Sample, Vec<Uuid>) {
    let mut g = Sample::new();
    let reps = rep_ids(&mut g, &["-X1011:L1", "-X1011:L2", "-X1011:PE"]);
    (g, reps)
}

fn strings(v: &[&[&str]]) -> Vec<Vec<String>> {
    v.iter()
        .map(|x| x.iter().map(|s| (*s).to_owned()).collect())
        .collect()
}

/// Rule 2: the sample's page 101 prints strip -X1011's tag once, at terminal "L1" (the strip's first terminal in its
/// own order), above its designation where the text placer sets it (left of the terminal or above it,
/// docs/design/06a-drafting.md §5.2); the designations stay.
#[test]
fn the_strip_tag_prints_once_at_the_first_terminal_of_a_page_without_a_segment() {
    let (g, reps) = sample_with_terminal_reps();
    let s = g.session();
    assert_eq!(
        at(&texts(&s, 101), &reps),
        strings(&[&["-X1011", "L1"], &["L2"], &["PE"]])
    );
    let lines = drawn(&s, 101);
    let line = |text: &str| {
        lines
            .iter()
            .find(|(e, t, _)| *e == reps[0] && t == text)
            .map(|(_, _, q)| *q)
            .unwrap()
    };
    let (designation, tag) = (line("L1"), line("-X1011"));
    // The quad runs top-left, bottom-left, bottom-right, top-right: the tag stands above the designation.
    assert!(tag[2].y < designation[2].y, "{tag:?} above {designation:?}");
}

/// Rule 2: with terminal "L2" moved to page 102, each page prints the tag once, at its own first terminal.
#[test]
fn the_strip_tag_prints_once_on_each_page() {
    let (mut g, reps) = sample_with_terminal_reps();
    let p102 = g.page(102);
    g.rep_mut("-X1011:L2").page = p102;
    let s = g.session();
    assert_eq!(
        at(&texts(&s, 101), &reps),
        strings(&[&["-X1011", "L1"], &[], &["PE"]])
    );
    assert_eq!(
        at(&texts(&s, 102), &reps),
        strings(&[&[], &["-X1011", "L2"], &[]])
    );
}

/// Rule 1: under `label_mode` FunctionText a terminal prints its function text in its tag slot and nothing in its
/// function-text slot; an empty function text falls back to the designation. Under Designation it prints its
/// designation and, in its function-text slot, its function text.
#[test]
fn a_terminal_prints_its_designation_or_its_function_text_by_label_mode() {
    let (mut g, reps) = sample_with_terminal_reps();
    g.set_function_text("-X1011:L1", "Feed");
    let mut by_text = Sample::new();
    by_text.set_function_text("-X1011:L1", "Feed");
    by_text.strip_mut("-X1011").label_mode = ew_schema::LabelMode::FunctionText;
    let s = g.session();
    assert_eq!(
        at(&texts(&s, 101), &reps),
        strings(&[&["-X1011", "Feed", "L1"], &["L2"], &["PE"]])
    );
    let s = by_text.session();
    assert_eq!(
        at(&texts(&s, 101), &reps),
        strings(&[&["-X1011", "Feed"], &["L2"], &["PE"]])
    );
}

/// Rule 2: a page that holds a segment of the strip prints no strip tag at a terminal drawn alone; the segments print
/// theirs (PerSegment, the default: at both).
#[test]
fn a_page_with_a_segment_of_the_strip_prints_no_extra_tag() {
    let mut g = Sample::new();
    let segments = g.add_x1031_segments([(10, 10), (20, 10)]);
    let alone = rep_ids(&mut g, &["-X1031:U"]);
    let s = g.session();
    let t = texts(&s, 103);
    assert_eq!(at(&t, &alone), strings(&[&["U"]]));
    let mut tagged: Vec<Uuid> = drawn(&s, 103)
        .into_iter()
        .filter(|(_, text, _)| text == "-X1031")
        .map(|(e, _, _)| e)
        .collect();
    tagged.sort();
    assert_eq!(tagged, segments);
}

/// Rule 3: `tag_print` PerPage prints the strip's tag at its first segment of the page in (row, column, ID) order
/// only, also when the page shows a terminal of the strip drawn alone.
#[test]
fn per_page_prints_the_strip_tag_once_at_the_first_segment() {
    let mut g = Sample::new();
    let segments = g.add_x1031_segments([(10, 10), (20, 10)]);
    g.strip_mut("-X1031").tag_print = TagPrint::PerPage;
    let alone = rep_ids(&mut g, &["-X1031:U"]);
    let s = g.session();
    let tagged: Vec<Uuid> = drawn(&s, 103)
        .into_iter()
        .filter(|(_, text, _)| text == "-X1031")
        .map(|(e, _, _)| e)
        .collect();
    assert_eq!(tagged, vec![segments[0]]);
    assert_eq!(at(&texts(&s, 103), &alone), strings(&[&["U"]]));
}

/// An unnumbered strip's terminals name their strip: its displayed tag, with "?" for the counter it lacks. With
/// -X1011's frozen tag and its counter removed, the sample's strip rule derives the tag with the page token: "-X101?"
/// (a strip without a page token reads "-X??").
#[test]
fn an_unnumbered_strip_prints_its_tag_at_its_terminals() {
    let (mut g, reps) = sample_with_terminal_reps();
    let strip = g.device_mut("-X1011");
    strip.tag.frozen = None;
    strip.tag.counter = None;
    let s = g.session();
    assert_eq!(
        at(&texts(&s, 101), &reps),
        strings(&[&["-X101?", "L1"], &["L2"], &["PE"]])
    );
}

/// The symbol copy of library key `key` in the library of `g`.
fn symbol_copy(g: &Sample, key: &str) -> Id<ew_schema::SymbolDef> {
    g.lib
        .iter()
        .find_map(|d| match d {
            LibDocument::Symbol(c) if c.item.meta.key.as_ref().is_some_and(|k| k.as_str() == key) => {
                Some(c.id)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("no symbol {key}"))
}

/// Hides the tag slot (slot 0 of both terminal symbols) of the representation of `tag`.
fn hide_tag(g: &mut Sample, tag: &str) {
    let hidden: ew_schema::SlotOverride =
        serde_json::from_value(serde_json::json!({"slot": 0, "visible": false})).unwrap();
    g.rep_mut(tag).text_slots.insert(hidden);
}

/// A first terminal that shows no tag slot carries no strip tag; the next terminal in the strip's order does: the
/// designation is hidden, or the symbol has no tag slot (the interruption-point symbol, whose slots are `function_text`
/// and `xref`).
#[test]
fn a_first_terminal_without_a_shown_tag_slot_passes_the_strip_tag_on() {
    fn hidden(g: &mut Sample) {
        hide_tag(g, "-X1011:L1");
    }
    fn ip_symbol(g: &mut Sample) {
        let ip = symbol_copy(g, "ew.gen.ip@qet10");
        g.rep_mut("-X1011:L1").symbol = ip;
    }
    let cases = [
        ("hidden designation", hidden as fn(&mut Sample)),
        ("ip symbol", ip_symbol),
    ];
    for (name, edit) in cases {
        let (mut g, reps) = sample_with_terminal_reps();
        edit(&mut g);
        let s = g.session();
        assert_eq!(
            at(&texts(&s, 101), &reps),
            strings(&[&[], &["-X1011", "L2"], &["PE"]]),
            "{name}"
        );
    }
}

/// With no terminal of the strip showing its tag slot on the page, the page prints no strip tag.
#[test]
fn no_shown_tag_slot_no_strip_tag() {
    let (mut g, reps) = sample_with_terminal_reps();
    for tag in ["-X1011:L1", "-X1011:L2", "-X1011:L3", "-X1011:N", "-X1011:PE"] {
        hide_tag(&mut g, tag);
    }
    let s = g.session();
    assert_eq!(at(&texts(&s, 101), &reps), strings(&[&[], &[], &[]]));
}

/// A strip drawn by its segments alone (FR-TRM-011): -X1031's terminals lose their own representations on 103 and
/// its two segments draw them, the strip's terminals split over the segments in seq order (the first segment the
/// first half, rounded down). Each terminal's side-1 CPs stand on the segment's top edge and leave up, its side-2 CPs
/// on the bottom edge below them and leave down, so a wire attaches on both sides; the page's CPs hold every one of
/// them, which only the segments put on the page.
#[test]
fn a_strip_drawn_by_its_segments_draws_its_terminals_cps_on_both_sides() {
    let mut g = Sample::new();
    let strip = g.device_mut("-X1031").id;
    let mut terminals: Vec<(u32, Id<Device>)> = g
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Device(d) => d
                .aspects
                .terminal
                .as_ref()
                .filter(|t| t.strip == strip)
                .map(|t| (t.seq, d.id)),
            _ => None,
        })
        .collect();
    terminals.sort();
    let main_of = |records: &[Record], device: Id<Device>| -> Id<Function> {
        records
            .iter()
            .find_map(|r| match r {
                Record::Function(f) if f.device == Some(device) && f.role == ew_schema::FnRole::Main => {
                    Some(f.id)
                }
                _ => None,
            })
            .expect("a terminal's main function")
    };
    let mains: Vec<Id<Function>> = terminals.iter().map(|(_, d)| main_of(&g.records, *d)).collect();
    g.records
        .retain(|r| !matches!(r, Record::Representation(x) if x.functions.iter().any(|f| mains.contains(f))));
    let segments = g.add_x1031_segments([(10, 10), (20, 10)]);
    let s = g.session();
    let tables = s.page_tables("103").expect("page 103");
    let page_cps: BTreeSet<Uuid> = s.page_cps("103").iter().map(|c| c.id).collect();
    let first_half = terminals.len() / 2;
    for (k, main) in mains.iter().enumerate() {
        let cps: Vec<_> = tables
            .cps
            .iter()
            .filter(|c| c.function == main.as_uuid())
            .collect();
        let up: Vec<_> = cps.iter().filter(|c| c.dir == Some(Dir::Up)).collect();
        let down: Vec<_> = cps.iter().filter(|c| c.dir == Some(Dir::Down)).collect();
        assert!(
            !up.is_empty() && !down.is_empty() && up.len() + down.len() == cps.len(),
            "terminal {k} draws CPs on both sides and no other: {cps:#?}"
        );
        let segment = if k < first_half { segments[0] } else { segments[1] };
        for c in &cps {
            assert_eq!(c.rep, segment, "terminal {k}'s CP {} is its segment's", c.label);
            assert!(page_cps.contains(&c.id), "the page's CPs hold {}", c.label);
        }
        let (top, bottom) = (up[0].at, down[0].at);
        assert!(
            up.iter().all(|c| c.at == top) && down.iter().all(|c| c.at == bottom),
            "one point per side"
        );
        assert!(
            top.0 == bottom.0 && top.1 < bottom.1,
            "side 2 below side 1: {top:?} {bottom:?}"
        );
    }
}
