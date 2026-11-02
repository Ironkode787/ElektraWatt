// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-867 by derivation (decision log ID-1137) through the page scene the PDF and SVG writers draw
//! (`ew_api::output::page_scene`): a black box turned by a half turn places its slot texts as on the same box unturned
//! (`ew_derive::text_frame`), so every text keeps its place, angle and alignment relative to the box, a text override
//! stays in the unturned frame, and the cross-reference text keeps its dodge. The box is the starter CPU placed from the
//! picker's command (`box.black` from the part), with an auxiliary coil of its device elsewhere on the page, so it
//! prints a cross-reference.

use ew_api::output::ew_dl::{EntityRef, Item, ItemKind, Layer, Transform};
use ew_api::{AnyCommand, NewProjectKind, ProjectTemplate, Session, SessionOptions};
use ew_derive::{DisplayedTag, PageOrder, PageRef, XrefTextOf};
use ew_ids::{Id, Uuid};
use ew_schema::{Kind, Page, Representation};
use ew_units::{Orient2, Pt2, Rect};
use serde_json::{Value, json};

const BLACKBOX: &str = "ew.gen.blackbox@qet10";
const COIL: &str = "ew.iec.coil@qet10";
/// Three steps of the page's connection grid (7937500/3 nm).
const STEP: i64 = 7_937_500;
/// The slots of the generic black box (`content/symbols/ew.gen.blackbox.json`).
const TAG: u16 = 0;
const FUNCTION_TEXT: u16 = 1;

/// One drawn line of text relative to the box: the text and the four corners of its layout box on the page (top-left,
/// bottom-left, bottom-right, top-right of the run) less the box's top-left corner, which fix its place, its angle
/// and its alignment.
type Drawn = (String, [Pt2; 4]);

struct Fixture {
    s: Session,
    page: Id<Page>,
    rep: Id<Representation>,
}

fn run(s: &mut Session, name: &str, params: Value) -> ew_api::Receipt {
    s.execute(&AnyCommand {
        name: name.into(),
        params,
    })
    .unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

/// A new project with the starter CPU as a black box (no box CPs, so no pin texts), its device's function text set,
/// and an auxiliary coil of the same device to the right of it.
fn fixture(seed: u64) -> Fixture {
    let options = SessionOptions {
        ids: Some(ew_testids::seeded_ids(seed)),
        ..SessionOptions::default()
    };
    let mut s = Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Schematic, options).unwrap();
    let part = s
        .library_parts()
        .iter()
        .find(|c| c.item.part_number.as_str() == "EW-GEN-CPU-1")
        .expect("the starter CPU")
        .id
        .as_uuid();
    let copies = s
        .starter_embed_part(part, BLACKBOX)
        .expect("the closure resolves")
        .unwrap_or_else(|| json!({"copies": []}));
    let symbol = s.starter_symbol_copy(BLACKBOX).expect("a starter symbol");
    let page = {
        let q = s.query();
        let mut pages: Vec<Id<Page>> = q.pages().iter().map(|(id, _)| id).collect();
        pages.sort();
        pages[0]
    };
    let r = run(
        &mut s,
        "box.black",
        json!({
            "page": page.to_string(),
            "symbol": symbol.to_string(),
            "pos": [12 * STEP, 12 * STEP],
            "box": {"outline": [[0, 0], [5 * STEP, 0], [5 * STEP, 8 * STEP], [0, 8 * STEP]]},
            "target": {"from_part": {"part": part.to_string(), "template": 0, "letter": "A"}},
            "copies": copies["copies"],
        }),
    );
    let created = |kind: Kind| -> Uuid {
        r.created
            .iter()
            .find(|k| k.0 == kind)
            .map(|k| k.1.as_uuid())
            .expect("created")
    };
    let (device, rep) = (created(Kind::Device), created(Kind::Representation));
    run(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [device.to_string()], "path": "function_text",
            "value": {"en": "Controller"}}),
    );
    let coil = s.starter_symbol_copy(COIL).expect("a starter coil");
    let coil_copies = s
        .starter_embed(COIL)
        .map_or_else(Vec::new, |c| c["copies"].as_array().cloned().unwrap_or_default());
    run(
        &mut s,
        "device.place",
        json!({
            "page": page.to_string(),
            "symbol": coil.to_string(),
            "pos": [30 * STEP, 12 * STEP],
            "target": {"join_device": {"device": device.to_string(), "role": "aux"}},
            "copies": Value::Array(coil_copies),
        }),
    );
    Fixture {
        s,
        page,
        rep: Id::from_uuid(rep),
    }
}

impl Fixture {
    fn half_turn(&mut self) {
        let rep = self.rep.to_string();
        run(&mut self.s, "rep.transform", json!({"reps": [rep], "rotate": 2}));
    }

    /// Sets the box's text-slot overrides through the generic field edit.
    fn set_slots(&mut self, value: Value) {
        let rep = self.rep.to_string();
        run(
            &mut self.s,
            "fields.set",
            json!({"kind": "representation", "targets": [rep], "path": "text_slots", "value": value}),
        );
    }

    fn record(&self) -> Representation {
        self.s
            .query()
            .reps()
            .get(self.rep)
            .map(|r| Representation::clone(r))
            .expect("the box")
    }

    /// The top-left corner of the box's outline on the page.
    fn corner(&self) -> Pt2 {
        let r = self.record();
        let points: Vec<Pt2> = r
            .box_
            .as_ref()
            .expect("a black box")
            .outline
            .iter()
            .map(|p| r.pos + r.orient.apply(*p))
            .collect();
        Pt2::new(
            points.iter().map(|p| p.x).min().unwrap(),
            points.iter().map(|p| p.y).min().unwrap(),
        )
    }

    /// The texts the page scene draws for the box in its labels layer, relative to the box, sorted.
    fn texts(&self) -> Vec<Drawn> {
        let q = self.s.query();
        let order = q.get::<PageOrder>(());
        let entry = order
            .entries
            .iter()
            .find(|e| e.page == PageRef::Drawn(self.page))
            .expect("the page in the page order");
        let scene = ew_api::output::page_scene(&q, entry);
        let entity = EntityRef(self.rep.as_uuid());
        let corner = self.corner();
        let mut out = Vec::new();
        for (layer, list) in &scene.layers {
            if *layer == Layer::Labels {
                collect(&list.items, &[], None, entity, corner, &mut out);
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    /// The drawn text `text` of the box.
    fn text(&self, text: &str) -> Drawn {
        self.texts()
            .into_iter()
            .find(|t| t.0 == text)
            .unwrap_or_else(|| panic!("`{text}` drawn on the box"))
    }

    /// The printed cross-reference of the box; not empty.
    fn xref(&self) -> String {
        let text = self.s.query().get::<XrefTextOf>(self.rep).text.clone();
        assert!(!text.is_empty(), "the box prints a cross-reference");
        text
    }

    /// The printed tag of the box's device.
    fn tag(&self) -> String {
        let rep = self.record();
        let q = self.s.query();
        let device = q
            .functions()
            .get(rep.functions[0])
            .and_then(|f| f.device)
            .expect("a device");
        let tag = q.get::<DisplayedTag>((device, rep.page)).as_ref().clone();
        assert!(!tag.is_empty(), "the box prints a tag");
        tag
    }
}

/// The glyph runs of `entity` among `items` (an item without an entity takes its group's), each line's layout box as
/// the page draws it: its four corners through the run's own transform and every enclosing group's (`groups`,
/// outermost first), relative to `corner`.
fn collect(
    items: &[Item],
    groups: &[Transform],
    inherited: Option<EntityRef>,
    entity: EntityRef,
    corner: Pt2,
    out: &mut Vec<Drawn>,
) {
    for item in items {
        let owner = item.entity.or(inherited);
        match &item.kind {
            ItemKind::Glyphs(run) if owner == Some(entity) => {
                let quad = run.layout_quad().map(|q| {
                    let p = groups.iter().rev().fold(q, |p, t| t.apply(p));
                    Pt2::new(p.x - corner.x, p.y - corner.y)
                });
                // A line printed in several runs (a fallback font, a link per item) is one text: a run that starts
                // where the one before it ends continues it.
                match out.last_mut() {
                    Some((text, last)) if last[3] == quad[0] && last[2] == quad[1] => {
                        text.push_str(&run.text);
                        last[2] = quad[2];
                        last[3] = quad[3];
                    }
                    _ => out.push((run.text.clone(), quad)),
                }
            }
            ItemKind::Group(g) => {
                let mut inner = groups.to_vec();
                inner.extend(g.transform);
                collect(&g.items, &inner, owner, entity, corner, out);
            }
            _ => {}
        }
    }
}

/// The rectangle a drawn text's layout box covers (relative to the box).
fn page_box(t: &Drawn) -> Rect {
    let quad = &t.1;
    Rect::from_points(
        Pt2::new(
            quad.iter().map(|p| p.x).min().unwrap(),
            quad.iter().map(|p| p.y).min().unwrap(),
        ),
        Pt2::new(
            quad.iter().map(|p| p.x).max().unwrap(),
            quad.iter().map(|p| p.y).max().unwrap(),
        ),
    )
}

/// Whether two rectangles share an inner point.
fn overlap(a: Rect, b: Rect) -> bool {
    a.min().x < b.max().x && b.min().x < a.max().x && a.min().y < b.max().y && b.min().y < a.max().y
}

/// Every slot text of the box (the tag, part number and manufacturer captions, the function text and the
/// cross-reference) keeps its place, angle and alignment relative to the box through a half turn and back.
#[test]
fn every_slot_text_keeps_its_place_angle_and_alignment_through_a_half_turn() {
    let mut f = fixture(1);
    let xref = f.xref();
    let before = f.texts();
    assert!(before.iter().any(|t| t.0 == xref), "{before:?}");
    assert!(before.iter().any(|t| t.0 == "Controller"), "{before:?}");
    assert!(before.len() >= 4, "{before:?}");
    f.half_turn();
    assert_eq!(f.record().orient, Orient2::R180);
    assert_eq!(f.texts(), before);
    f.half_turn();
    assert_eq!(f.record().orient, Orient2::R0);
    assert_eq!(f.texts(), before);
}

/// A tag the user moved before the turn keeps its place relative to the box.
#[test]
fn a_moved_tag_keeps_its_place_relative_to_the_box() {
    let mut f = fixture(2);
    let tag = f.tag();
    f.set_slots(json!([{"slot": TAG, "pos": [2_000_000, -3_000_000]}]));
    let before = f.text(&tag);
    f.half_turn();
    assert_eq!(f.text(&tag), before);
}

/// An override set by the field edit while the box is half turned is a point of the unturned frame: it draws where
/// the same override draws on the unturned box, keeps that place relative to the box after the turn back, and the
/// record keeps the value given.
#[test]
fn an_override_set_while_half_turned_keeps_its_place_after_the_turn_back() {
    let slots = json!([{"slot": TAG, "pos": [2_000_000, -3_000_000]}]);
    let mut unturned = fixture(3);
    let tag = unturned.tag();
    unturned.set_slots(slots.clone());
    let expected = unturned.text(&tag);

    let mut f = fixture(3);
    f.half_turn();
    f.set_slots(slots);
    let given = f.record().text_slots;
    assert_eq!(f.text(&tag), expected);
    f.half_turn();
    assert_eq!(f.record().orient, Orient2::R0);
    assert_eq!(f.text(&tag), expected);
    assert_eq!(f.record().text_slots, given);
}

/// The cross-reference text keeps its dodge on a half-turned box: with the function text moved onto it, it drops
/// clear of the function text before and after the turn.
#[test]
fn the_cross_reference_keeps_its_dodge_on_a_half_turned_box() {
    let mut f = fixture(4);
    let xref = f.xref();
    let undodged = f.text(&xref);
    f.set_slots(json!([{"slot": FUNCTION_TEXT, "pos": [1_000_000, 1_000_000]}]));
    assert!(
        overlap(page_box(&undodged), page_box(&f.text("Controller"))),
        "the function text is moved onto the cross-reference's own place"
    );
    let clear = |f: &Fixture| !overlap(page_box(&f.text(&xref)), page_box(&f.text("Controller")));
    assert!(clear(&f), "the cross-reference dodges the function text");
    let before = f.text(&xref);
    f.half_turn();
    assert!(
        clear(&f),
        "the cross-reference dodges the function text after a half turn"
    );
    assert_eq!(f.text(&xref), before);
}
