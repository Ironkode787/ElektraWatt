// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Doc 08 §6.1's and §6.7's printing (decision log ID-1151, FT-1026), through the page scene the PDF and SVG writers
//! draw: a terminal drawn alone prints its designation, or its function text under its strip's `label_mode`
//! FunctionText; the strip's tag prints once per page at the strip's first terminal when the page holds no segment of
//! the strip; `tag_print` PerPage prints a strip's tag at its first segment on each page. Golden A's records, edited
//! before the session opens: strip 1X1's terminals stand alone at 101-D2, strip -X1031 is drawn by two segments on
//! page 103.

use std::collections::BTreeMap;

use crate::common;

use ew_api::Session;
use ew_api::output::ew_dl::{Item, ItemKind, Layer, Transform};
use ew_derive::ew_mltext::MlText;
use ew_derive::{PageOrder, PageRef};
use ew_ids::{Id, Uuid};
use ew_schema::{Device, Function, Page, Record, Representation, TagPrint};
use ew_units::{Nm, Pt2};

/// Golden A's records with the edits of a test.
struct Golden {
    project: ew_schema::Project,
    records: Vec<Record>,
    lib: Vec<ew_schema::LibDocument>,
}

impl Golden {
    fn new() -> Golden {
        let batch = common::golden_batch();
        let (project, records) = common::split(&batch);
        Golden {
            project,
            records,
            lib: batch.lib.clone(),
        }
    }

    fn session(self) -> Session {
        common::session_of(self.project, self.records, self.lib)
    }

    /// The device of golden key `key` ("1X1", a terminal "1X1:1").
    fn device_mut(&mut self, key: &str) -> &mut Device {
        let key = if key.contains(':') {
            format!("terminals|{key}")
        } else {
            format!("devices|{key}")
        };
        self.records
            .iter_mut()
            .find_map(|r| match r {
                Record::Device(d) if d.source_ref.as_ref().is_some_and(|s| s.key.as_str() == key) => Some(d),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no device {key}"))
    }

    fn rep_mut(&mut self, key: &str) -> &mut Representation {
        let key = format!("representations|{key}");
        self.records
            .iter_mut()
            .find_map(|r| match r {
                Record::Representation(x) if x.source_ref.as_ref().is_some_and(|s| s.key.as_str() == key) => {
                    Some(x)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("no representation {key}"))
    }

    fn function(&self, key: &str) -> Id<Function> {
        let key = format!("functions|{key}");
        self.records
            .iter()
            .find_map(|r| match r {
                Record::Function(f) if f.source_ref.as_ref().is_some_and(|s| s.key.as_str() == key) => {
                    Some(f.id)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("no function {key}"))
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

    fn set_function_text(&mut self, key: &str, text: &str) {
        self.device_mut(key).function_text = MlText::neutral(text);
    }

    fn strip_mut(&mut self, key: &str) -> &mut ew_schema::Strip {
        self.device_mut(key).aspects.strip.as_mut().expect("a strip")
    }

    /// A representation of terminal `function` of its own, on `page`, a copy of 1X1:1's terminal symbol at
    /// `(x, y)` connection-grid steps.
    fn add_terminal_rep(&mut self, function: &str, page: u32, at: (i64, i64)) -> Id<Representation> {
        let function = self.function(function);
        let page = self.page(page);
        let mut rep = self.rep_mut("1X1:1").clone();
        rep.id = Id::from_uuid(Uuid::from_u128(0x1151_0001));
        rep.functions = vec![function];
        rep.page = page;
        rep.pin_map = Default::default();
        rep.source_ref = None;
        rep.pos = Pt2::new(Nm(at.0 * STEP), Nm(at.1 * STEP));
        let id = rep.id;
        self.records.push(Record::Representation(rep));
        id
    }
}

/// One connection-grid step of the golden pages (the QET 10 px grid, 7937500/3 nm), times three.
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

/// The representation IDs of the golden records `keys`.
fn rep_ids(g: &mut Golden, keys: &[&str]) -> Vec<Uuid> {
    keys.iter().map(|k| g.rep_mut(k).id.as_uuid()).collect()
}

/// The texts drawn at `reps`, in that order (a representation with none: empty).
fn at(texts: &BTreeMap<Uuid, Vec<String>>, reps: &[Uuid]) -> Vec<Vec<String>> {
    reps.iter()
        .map(|r| texts.get(r).cloned().unwrap_or_default())
        .collect()
}

fn strings(v: &[&[&str]]) -> Vec<Vec<String>> {
    v.iter()
        .map(|x| x.iter().map(|s| (*s).to_owned()).collect())
        .collect()
}

/// Rule 2: Golden A's page 101 prints strip 1X1's tag once, at terminal "1" (the strip's first terminal in its
/// own order: "1" before "2" before "PE"), one grid step above its designation; the designations stay.
#[test]
fn the_strip_tag_prints_once_at_the_first_terminal_of_a_page_without_a_segment() {
    let mut g = Golden::new();
    let reps = rep_ids(&mut g, &["1X1:1", "1X1:2", "1X1:PE"]);
    let s = g.session();
    assert_eq!(
        at(&texts(&s, 101), &reps),
        strings(&[&["1", "1X1"], &["2"], &["PE"]])
    );
    let lines = drawn(&s, 101);
    let line = |text: &str| {
        lines
            .iter()
            .find(|(e, t, _)| *e == reps[0] && t == text)
            .map(|(_, _, q)| *q)
            .unwrap()
    };
    let (designation, tag) = (line("1"), line("1X1"));
    // The quad runs top-left, bottom-left, bottom-right, top-right. The tag stands one connection-grid step above the
    // tag slot's anchor (where the designation stands, as no placement rule moves it here), bottom-aligned, so it is
    // above the designation, at the same right edge (the slot's horizontal alignment, middle right).
    assert_eq!(tag[2].x, designation[2].x);
    assert!(tag[2].y < designation[2].y, "{tag:?} above {designation:?}");
}

/// Rule 2: with terminal "2" moved to page 102, each page prints the tag once, at its own first terminal.
#[test]
fn the_strip_tag_prints_once_on_each_page() {
    let mut g = Golden::new();
    let reps = rep_ids(&mut g, &["1X1:1", "1X1:2", "1X1:PE"]);
    let p102 = g.page(102);
    g.rep_mut("1X1:2").page = p102;
    let s = g.session();
    assert_eq!(
        at(&texts(&s, 101), &reps),
        strings(&[&["1", "1X1"], &[], &["PE"]])
    );
    assert_eq!(at(&texts(&s, 102), &reps), strings(&[&[], &["1X1", "2"], &[]]));
}

/// Rule 1: under `label_mode` FunctionText a terminal prints its function text in its tag slot and nothing in its
/// function-text slot; an empty function text falls back to the designation. Under Designation it prints its
/// designation and, in its function-text slot, its function text.
#[test]
fn a_terminal_prints_its_designation_or_its_function_text_by_label_mode() {
    let mut g = Golden::new();
    let reps = rep_ids(&mut g, &["1X1:1", "1X1:2", "1X1:PE"]);
    g.set_function_text("1X1:1", "L");
    let mut by_text = Golden::new();
    by_text.set_function_text("1X1:1", "L");
    by_text.strip_mut("1X1").label_mode = ew_schema::LabelMode::FunctionText;
    let s = g.session();
    assert_eq!(
        at(&texts(&s, 101), &reps),
        strings(&[&["1", "1X1", "L"], &["2"], &["PE"]])
    );
    let s = by_text.session();
    assert_eq!(
        at(&texts(&s, 101), &reps),
        strings(&[&["1X1", "L"], &["2"], &["PE"]])
    );
}

/// Rule 2: a page that holds a segment of the strip prints no strip tag at a terminal drawn alone; the segments print
/// theirs as before (PerSegment, Golden A's: at both).
#[test]
fn a_page_with_a_segment_of_the_strip_prints_no_extra_tag() {
    let mut g = Golden::new();
    let segments = rep_ids(&mut g, &["-X1031", "-X1031.2"]);
    let alone = g.add_terminal_rep("-X1031:1", 103, (30, 60)).as_uuid();
    let s = g.session();
    let t = texts(&s, 103);
    assert_eq!(at(&t, &[alone]), strings(&[&["1+"]]));
    let mut tagged: Vec<Uuid> = drawn(&s, 103)
        .into_iter()
        .filter(|(_, text, _)| text == "-X1031")
        .map(|(e, _, _)| e)
        .collect();
    tagged.sort();
    let mut segments = segments;
    segments.sort();
    assert_eq!(tagged, segments);
}

/// Rule 3: `tag_print` PerPage prints the strip's tag at its first segment of the page in (row, column, ID) order
/// only, also when the page shows a terminal of the strip drawn alone.
#[test]
fn per_page_prints_the_strip_tag_once_at_the_first_segment() {
    let mut g = Golden::new();
    let segments = rep_ids(&mut g, &["-X1031", "-X1031.2"]);
    g.strip_mut("-X1031").tag_print = TagPrint::PerPage;
    let alone = g.add_terminal_rep("-X1031:1", 103, (30, 60)).as_uuid();
    let s = g.session();
    let tagged: Vec<Uuid> = drawn(&s, 103)
        .into_iter()
        .filter(|(_, text, _)| text == "-X1031")
        .map(|(e, _, _)| e)
        .collect();
    assert_eq!(tagged, vec![segments[0]]);
    assert_eq!(at(&texts(&s, 103), &[alone]), strings(&[&["1+"]]));
}

/// FT-1026: an unnumbered strip's terminals name their strip: its displayed tag, with "?" for the counter it lacks.
/// Golden A's 1X1 is frozen as "1X1"; with its frozen tag and its counter removed, the golden strip rule derives the
/// tag with the page token: "-X101?" (a strip without a page token reads "-X??").
#[test]
fn an_unnumbered_strip_prints_its_tag_at_its_terminals() {
    let mut g = Golden::new();
    let reps = rep_ids(&mut g, &["1X1:1", "1X1:2", "1X1:PE"]);
    let strip = g.device_mut("1X1");
    strip.tag.frozen = None;
    strip.tag.counter = None;
    let s = g.session();
    assert_eq!(
        at(&texts(&s, 101), &reps),
        strings(&[&["-X101?", "1"], &["2"], &["PE"]])
    );
}

/// The symbol copy of library key `key` in Golden A's library.
fn symbol_copy(g: &Golden, key: &str) -> Id<ew_schema::SymbolDef> {
    g.lib
        .iter()
        .find_map(|d| match d {
            ew_schema::LibDocument::Symbol(c)
                if c.item.meta.key.as_ref().is_some_and(|k| k.as_str() == key) =>
            {
                Some(c.id)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("no symbol {key}"))
}

/// Hides the tag slot (slot 0 of both golden terminal symbols) of representation `key`.
fn hide_tag(g: &mut Golden, key: &str) {
    let hidden: ew_schema::SlotOverride =
        serde_json::from_value(serde_json::json!({"slot": 0, "visible": false})).unwrap();
    g.rep_mut(key).text_slots.insert(hidden);
}

/// C46-p4#3: a first terminal whose designation is hidden carries no strip tag; the next terminal in the strip's
/// order does.
#[test]
fn a_hidden_first_designation_moves_the_strip_tag_to_the_next_terminal() {
    let mut g = Golden::new();
    let reps = rep_ids(&mut g, &["1X1:1", "1X1:2", "1X1:PE"]);
    hide_tag(&mut g, "1X1:1");
    let s = g.session();
    assert_eq!(
        at(&texts(&s, 101), &reps),
        strings(&[&[], &["1X1", "2"], &["PE"]])
    );
}

/// C46-p4#3: a first terminal whose symbol has no tag slot (drawn with the interruption-point symbol, whose slots are
/// `function_text` and `xref`) carries no strip tag; the next terminal does.
#[test]
fn a_first_terminal_without_a_tag_slot_passes_the_strip_tag_on() {
    let mut g = Golden::new();
    let reps = rep_ids(&mut g, &["1X1:1", "1X1:2", "1X1:PE"]);
    let ip = symbol_copy(&g, "ew.gen.ip@qet10");
    g.rep_mut("1X1:1").symbol = ip;
    let s = g.session();
    assert_eq!(
        at(&texts(&s, 101), &reps),
        strings(&[&[], &["1X1", "2"], &["PE"]])
    );
}

/// C46-p4#3: with no terminal of the strip showing its tag slot on the page, the page prints no strip tag.
#[test]
fn no_shown_tag_slot_no_strip_tag() {
    let mut g = Golden::new();
    let reps = rep_ids(&mut g, &["1X1:1", "1X1:2", "1X1:PE"]);
    for key in ["1X1:1", "1X1:2", "1X1:PE"] {
        hide_tag(&mut g, key);
    }
    let s = g.session();
    assert_eq!(at(&texts(&s, 101), &reps), strings(&[&[], &[], &[]]));
}
