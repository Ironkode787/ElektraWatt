// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 4a (acceptance PI8): `item.place`, `marker.place` and `box.structure` with every rejection, and
//! the amendments `rep.move` (`items`), `edit.delete` and `fields.set` (the two kinds) and `project.drawing.set`
//! (`marker_mode`).
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    BoxStyleParam, CreateExternalRef, CreateStructureBox, DeleteSelection, FreeTextParam, GraphicParam,
    ImageParam, ItemKindParam, KindName, MarkerModeParam, MoveReps, PlaceItem, PlaceMarker, SetCompany,
    SetDrawing, SetFields, SetProjectSettings, ShapeParam, StrokeParam, TargetRef,
};
use ew_commands::{CmdError, Engine, Origin};
use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_model::Model;
use ew_schema::{
    AnyId, ColourSpec, Connection, Csa, ExternalRef, GraphicShape, Kind, Page, PageItem, PageItemKind,
    Record, Slot, StructureAssignment, StructureBox,
};
use ew_units::{Angle2, Nm, Pt2};
use serde_json::{Value, json};

fn item(e: &Engine, id: Id<PageItem>) -> std::sync::Arc<PageItem> {
    e.model().table::<PageItem>().get(&id).expect("item").clone()
}

fn sbox(e: &Engine, id: Id<StructureBox>) -> std::sync::Arc<StructureBox> {
    e.model().table::<StructureBox>().get(&id).expect("box").clone()
}

fn conn(e: &Engine, id: Id<Connection>) -> std::sync::Arc<Connection> {
    e.model()
        .table::<Connection>()
        .get(&id)
        .expect("connection")
        .clone()
}

/// The JSON Pointer of an `InvalidParams` refusal of `command`.
fn pointer(err: CmdError, command: &str) -> String {
    match err {
        CmdError::InvalidParams { command: c, path, .. } => {
            assert_eq!(c, command);
            path
        }
        other => panic!("expected InvalidParams of {command}, got {other:?}"),
    }
}

fn graphic(shape: ShapeParam) -> PlaceItem {
    PlaceItem {
        page: PAGE_101,
        kind: ItemKindParam::Graphic(GraphicParam {
            shape,
            stroke: None,
            dash: Default::default(),
            fill: false,
        }),
    }
}

fn text(text: &str, bind: Option<&str>, angle: i32) -> PlaceItem {
    PlaceItem {
        page: PAGE_101,
        kind: ItemKindParam::FreeText(FreeTextParam {
            text: MlText::neutral(text),
            bind: bind.map(|b| Ident::from_input(b).expect("ident")),
            at: pt(50_800_000, 30_480_000),
            angle: Angle2(angle),
            style: None,
            align: None,
        }),
    }
}

fn place(e: &mut Engine, cmd: &PlaceItem) -> Id<PageItem> {
    let out = e.execute(cmd).expect("placed");
    out.outcome.primary.expect("primary").1.cast()
}

fn marker(connections: Vec<Id<Connection>>) -> PlaceMarker {
    PlaceMarker {
        page: PAGE_102,
        connections,
        at: pt(7_937_500, 15_875_000),
        angle: Angle2::ZERO,
        csa: None,
        colours: Vec::new(),
    }
}

fn rect(x0: i64, y0: i64, x1: i64, y1: i64) -> Vec<Pt2> {
    vec![pt(x0, y0), pt(x1, y0), pt(x1, y1), pt(x0, y1)]
}

fn structure_box(assigns: StructureAssignment) -> CreateStructureBox {
    CreateStructureBox {
        page: PAGE_101,
        outline: rect(0, 0, 50_000_000, 30_000_000),
        style: BoxStyleParam::Dotted,
        caption: MlText::neutral("External Connection"),
        assigns,
        external: None,
    }
}

fn location(node: Id<ew_schema::StructureNode>) -> StructureAssignment {
    StructureAssignment {
        location: Some(Slot::Node(node)),
        ..StructureAssignment::default()
    }
}

fn mv(reps: Vec<Id<ew_schema::Representation>>, items: Vec<AnyId>, delta: Pt2) -> MoveReps {
    MoveReps {
        items,
        steps: None,
        reps,
        delta,
        page: None,
    }
}

fn set(kind: KindName, target: AnyId, path: &str, value: Value) -> SetFields {
    SetFields {
        kind,
        targets: vec![target],
        path: path.into(),
        value,
    }
}

// ---------------------------------------------------------------------------------------------------------
// item.place

/// Every shape is placed as given (a rectangle with `min` the smaller corner), with the outcome naming the item;
/// undo removes it.
#[test]
fn item_place_places_every_shape_and_free_texts() {
    let mut e = engine(golden_model());
    let shapes = [
        ShapeParam::Line {
            a: pt(0, 0),
            b: pt(10_000_000, 0),
        },
        ShapeParam::Polyline {
            points: vec![pt(0, 0), pt(1_000_000, 0), pt(1_000_000, 1_000_000)],
        },
        ShapeParam::Rect {
            min: pt(5_000_000, 6_000_000),
            max: pt(1_000_000, 2_000_000),
        },
        ShapeParam::Circle {
            centre: pt(5_000_000, 5_000_000),
            r: Nm(2_500_000),
        },
        ShapeParam::Arc {
            centre: pt(5_000_000, 5_000_000),
            r: Nm(2_500_000),
            start: Angle2(90_000),
            sweep: Angle2(-180_000),
        },
        ShapeParam::Polygon {
            points: vec![pt(0, 0), pt(1_000_000, 0), pt(0, 1_000_000)],
        },
    ];
    for shape in shapes {
        let id = place(&mut e, &graphic(shape));
        let it = item(&e, id);
        assert_eq!(it.page, PAGE_101);
        if let PageItemKind::Graphic(g) = &it.kind {
            if let GraphicShape::Rect(r) = &g.shape {
                assert_eq!(
                    (r.min, r.max),
                    (pt(1_000_000, 2_000_000), pt(5_000_000, 6_000_000))
                );
            }
        } else {
            panic!("a graphic");
        }
    }
    let mut styled = graphic(ShapeParam::Line {
        a: pt(0, 0),
        b: pt(0, 1),
    });
    if let ItemKindParam::Graphic(g) = &mut styled.kind {
        g.stroke = Some(StrokeParam { width: Nm(350_000) });
        g.fill = true;
    }
    place(&mut e, &styled);
    let bound = place(&mut e, &text("", Some("project.description"), 0));
    let PageItemKind::FreeText(t) = &item(&e, bound).kind else {
        panic!("a free text");
    };
    assert_eq!(t.bind.as_ref().map(Ident::as_str), Some("project.description"));
    let plain = place(&mut e, &text("40x60", None, 270_000));
    assert_eq!(e.model().table::<PageItem>().len(), 9);
    e.undo(&Origin::Gui).expect("undo");
    assert!(e.model().table::<PageItem>().get(&plain).is_none());
}

/// Every rejection of `item.place`, at its pointer; a missing page is not found.
#[test]
fn item_place_rejections() {
    let mut e = engine(golden_model());
    let degenerate = [
        ShapeParam::Line {
            a: pt(1, 1),
            b: pt(1, 1),
        },
        ShapeParam::Polyline {
            points: vec![pt(0, 0), pt(0, 0)],
        },
        ShapeParam::Polygon {
            points: vec![pt(0, 0), pt(1, 0), pt(1, 0)],
        },
        ShapeParam::Polygon {
            points: vec![pt(0, 0), pt(1, 0), pt(2, 0)],
        },
        ShapeParam::Rect {
            min: pt(0, 0),
            max: pt(5, 0),
        },
        ShapeParam::Circle {
            centre: pt(0, 0),
            r: Nm(0),
        },
        ShapeParam::Arc {
            centre: pt(0, 0),
            r: Nm(0),
            start: Angle2::ZERO,
            sweep: Angle2(90_000),
        },
        ShapeParam::Arc {
            centre: pt(0, 0),
            r: Nm(5),
            start: Angle2::ZERO,
            sweep: Angle2::ZERO,
        },
    ];
    for shape in degenerate {
        let err = e.execute(&graphic(shape.clone())).unwrap_err();
        assert_eq!(pointer(err, "item.place"), "/kind/graphic", "{shape:?}");
    }
    let mut thin = graphic(ShapeParam::Line {
        a: pt(0, 0),
        b: pt(0, 1),
    });
    if let ItemKindParam::Graphic(g) = &mut thin.kind {
        g.stroke = Some(StrokeParam { width: Nm(0) });
    }
    assert_eq!(
        pointer(e.execute(&thin).unwrap_err(), "item.place"),
        "/kind/graphic"
    );
    for bad in [
        text("x", Some("project.reference"), 0),
        text("", None, 0),
        text("x", None, 45_000),
    ] {
        assert_eq!(
            pointer(e.execute(&bad).unwrap_err(), "item.place"),
            "/kind/free_text"
        );
    }
    let mut missing = text("x", None, 0);
    missing.page = id(0x3f);
    assert!(matches!(e.execute(&missing), Err(CmdError::NotFound { .. })));
    assert_eq!(e.model().table::<PageItem>().len(), 0, "nothing was placed");
}

// ---------------------------------------------------------------------------------------------------------
// item.place, images (WP-S2.04 part 5, §11; FR-SYM-025)

/// A placeholder logo: an SVG of 40 × 10 user units (no company's artwork).
const IMAGE_SVG: &[u8] =
    br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 10"><rect width="40" height="10"/></svg>"#;

/// Brings `bytes` into the engine's asset store (the company logo command is the one command that does), removes the
/// logo again and returns the asset's hash: the asset stays in the store.
fn hold_asset(e: &mut Engine, bytes: &[u8]) -> ew_schema::Sha256 {
    use sha2::Digest;
    let set = |logo: &str| SetCompany {
        name: None,
        notice: None,
        personal_data: None,
        logo: Some(logo.to_owned()),
    };
    // A bulk command: previewed, then committed.
    for logo in [ew_schema::base64_encode(bytes), String::new()] {
        let p = e.preview(&set(&logo)).expect("logo");
        e.commit(p).expect("committed");
    }
    let hash = ew_schema::Sha256(sha2::Sha256::digest(bytes).into());
    assert!(e.model().asset(&hash).is_some(), "the asset stays in the store");
    hash
}

fn image(asset: ew_schema::Sha256, a: Pt2, b: Pt2) -> PlaceItem {
    PlaceItem {
        page: PAGE_101,
        kind: ItemKindParam::Image(ImageParam { asset, rect: (a, b) }),
    }
}

/// `item.place` places an image of a held asset with the rectangle stored smaller coordinates first; the record
/// round-trips as `{"image": {"asset", "rect"}}` and undo removes it.
#[test]
fn item_place_places_an_image_of_a_held_asset() {
    let mut e = engine(golden_model());
    let hash = hold_asset(&mut e, IMAGE_SVG);
    let id = place(
        &mut e,
        &image(hash, pt(90_000_000, 50_000_000), pt(50_000_000, 40_000_000)),
    );
    let it = item(&e, id);
    assert_eq!(it.page, PAGE_101);
    let PageItemKind::Image(img) = &it.kind else {
        panic!("an image")
    };
    assert_eq!(img.asset, hash);
    assert_eq!(
        (img.rect.0.min(), img.rect.0.max()),
        (pt(50_000_000, 40_000_000), pt(90_000_000, 50_000_000))
    );
    assert_eq!(
        serde_json::to_value(&it.kind).unwrap(),
        json!({"image": {"asset": hash.to_string(),
            "rect": [[50_000_000, 40_000_000], [90_000_000, 50_000_000]]}})
    );
    e.undo(&Origin::Gui).expect("undo");
    assert!(e.model().table::<PageItem>().get(&id).is_none());
    assert!(e.model().asset(&hash).is_some(), "undo keeps the bytes");
}

/// The rejections of an image: an asset the project does not hold (`/kind/image/asset`), a rectangle without area
/// (`/kind/image`), a missing page (not found); nothing is placed.
#[test]
fn item_place_image_rejections() {
    let mut e = engine(golden_model());
    let hash = hold_asset(&mut e, IMAGE_SVG);
    let unknown = ew_schema::Sha256([0xab; 32]);
    let err = e
        .execute(&image(unknown, pt(0, 0), pt(10_000_000, 5_000_000)))
        .unwrap_err();
    let CmdError::InvalidParams { path, message, .. } = &err else {
        panic!("{err:?}")
    };
    assert_eq!(path, "/kind/image/asset");
    assert!(
        message.contains(&unknown.to_string()) && message.contains("holds no asset"),
        "{message}"
    );
    for (a, b) in [
        (pt(0, 0), pt(10_000_000, 0)),
        (pt(0, 0), pt(0, 10_000_000)),
        (pt(5, 5), pt(5, 5)),
    ] {
        assert_eq!(
            pointer(e.execute(&image(hash, a, b)).unwrap_err(), "item.place"),
            "/kind/image"
        );
    }
    let mut missing = image(hash, pt(0, 0), pt(10_000_000, 5_000_000));
    missing.page = id(0x3f);
    assert!(matches!(e.execute(&missing), Err(CmdError::NotFound { .. })));
    assert_eq!(e.model().table::<PageItem>().len(), 0, "nothing was placed");
}

/// `rep.move` moves an image by translating its rectangle.
#[test]
fn rep_move_moves_an_image() {
    let mut e = engine(golden_model());
    let hash = hold_asset(&mut e, IMAGE_SVG);
    let img = place(&mut e, &image(hash, pt(0, 0), pt(40_000_000, 10_000_000)));
    e.execute(&mv(vec![], vec![img.cast()], pt(7_937_500, 15_875_000)))
        .expect("moved");
    let PageItemKind::Image(i) = &item(&e, img).kind else {
        panic!("an image")
    };
    assert_eq!(
        (i.rect.0.min(), i.rect.0.max()),
        (pt(7_937_500, 15_875_000), pt(47_937_500, 25_875_000))
    );
}

// ---------------------------------------------------------------------------------------------------------
// marker.place

/// `marker.place` sets the connections' marker overrides and lists them in order; undo restores them.
#[test]
fn marker_place_sets_overrides_and_lists_the_connections() {
    let mut e = engine(golden_model());
    let before: Vec<_> = [CON_W22, CON_W27].iter().map(|c| conn(&e, *c)).collect();
    let mut m = marker(vec![CON_W27, CON_W22]);
    m.angle = Angle2(90_000);
    m.csa = Some(Csa::Mm2(ew_units::Dec3(1_500)));
    m.colours = vec![
        serde_json::from_value::<ColourSpec>(json!(["BK"])).unwrap(),
        serde_json::from_value::<ColourSpec>(json!(["GN", "YE"])).unwrap(),
    ];
    let out = e.execute(&m).expect("placed");
    let mid: Id<PageItem> = out.outcome.primary.unwrap().1.cast();
    let PageItemKind::WireMarker(w) = &item(&e, mid).kind else {
        panic!("a wire marker");
    };
    assert_eq!(w.connections, vec![CON_W27, CON_W22], "printed order kept");
    assert_eq!(w.angle, Angle2(90_000));
    for c in [CON_W22, CON_W27] {
        let c = conn(&e, c);
        assert!(c.overrides.show_marker);
        assert_eq!(c.overrides.csa, Some(Csa::Mm2(ew_units::Dec3(1_500))));
    }
    assert_eq!(
        serde_json::to_value(&conn(&e, CON_W22).overrides.colour).unwrap(),
        json!(["GN", "YE"])
    );
    e.undo(&Origin::Gui).unwrap();
    for (c, b) in [CON_W22, CON_W27].iter().zip(before) {
        assert_eq!(conn(&e, *c), b);
    }
}

/// Every rejection of `marker.place`, at its pointer.
#[test]
fn marker_place_rejections() {
    let mut e = engine(golden_model());
    let cases: Vec<(PlaceMarker, &str)> = vec![
        (marker(vec![]), "/connections"),
        (marker(vec![CON_W22, CON_W22]), "/connections/1"),
        (marker(vec![CON_W22, CON_W11A]), "/connections/1"),
        (
            PlaceMarker {
                angle: Angle2(45_000),
                ..marker(vec![CON_W22])
            },
            "/angle",
        ),
        (
            PlaceMarker {
                colours: vec![serde_json::from_value(json!(["BK"])).unwrap()],
                ..marker(vec![CON_W22, CON_W27])
            },
            "/colours",
        ),
        (
            PlaceMarker {
                colours: vec![serde_json::from_value(json!(["DBU"])).unwrap()],
                ..marker(vec![CON_W22])
            },
            "/colours/0",
        ),
        (
            PlaceMarker {
                colours: vec![serde_json::from_value(json!([{"foreign": "XX"}])).unwrap()],
                ..marker(vec![CON_W22])
            },
            "/colours/0",
        ),
    ];
    for (cmd, at) in cases {
        assert_eq!(
            pointer(e.execute(&cmd).unwrap_err(), "marker.place"),
            at,
            "{cmd:?}"
        );
    }
    assert!(matches!(
        e.execute(&marker(vec![id(0x4ff)])),
        Err(CmdError::NotFound { .. })
    ));
    let mut no_page = marker(vec![CON_W22]);
    no_page.page = id(0x3f);
    assert!(matches!(e.execute(&no_page), Err(CmdError::NotFound { .. })));
    // A company code the project's colour profile declares is accepted.
    let profile = SetProjectSettings {
        member: "colour_profile".into(),
        value: json!({"key": "house", "codes": [{"code": "DBU", "base": "BU", "shade": "dark"}],
            "rows": [{"colour": ["DBU"]}]}),
    };
    let preview = e.preview(&profile).expect("a colour profile (a bulk command)");
    e.commit(preview).expect("committed");
    e.execute(&PlaceMarker {
        colours: vec![serde_json::from_value(json!(["DBU"])).unwrap()],
        ..marker(vec![CON_W22])
    })
    .expect("a declared company code");
}

/// FT-451 (FR-CON-019): `fields.set` on a connection's `overrides.colour` refuses at `/value`, with
/// `marker.place`'s text, a code that is neither IEC 60757 nor a company code of the colour profile, and a
/// foreign code; IEC codes, a declared company code and `null` pass; a refusal changes nothing.
#[test]
fn fields_set_refuses_a_colour_the_profile_does_not_know() {
    let mut e = engine(golden_model());
    let target: AnyId = CON_W22.cast();
    let before = conn(&e, CON_W22).overrides.colour.clone();
    let refused = |e: &mut Engine, value: Value| match e
        .execute(&set(KindName::Connection, target, "overrides.colour", value))
        .unwrap_err()
    {
        CmdError::InvalidParams {
            command,
            path,
            message,
            ..
        } => {
            assert_eq!((command.as_str(), path.as_str()), ("fields.set", "/value"));
            message
        }
        other => panic!("expected InvalidParams, got {other:?}"),
    };
    assert_eq!(
        refused(&mut e, json!(["DBU"])),
        "`DBU` is neither an IEC 60757 colour code nor a company code of the project's colour profile"
    );
    assert!(
        refused(&mut e, json!(["BK", {"foreign": "XX"}])).contains("is a foreign colour code"),
        "the second code of a two-colour spec is checked too"
    );
    assert_eq!(
        conn(&e, CON_W22).overrides.colour,
        before,
        "a refusal changes nothing"
    );
    e.execute(&set(
        KindName::Connection,
        target,
        "overrides.colour",
        json!(["GN", "YE"]),
    ))
    .expect("an IEC code");
    let profile = SetProjectSettings {
        member: "colour_profile".into(),
        value: json!({"key": "house", "codes": [{"code": "DBU", "base": "BU", "shade": "dark"}],
            "rows": [{"colour": ["DBU"]}]}),
    };
    let preview = e.preview(&profile).expect("a colour profile (a bulk command)");
    e.commit(preview).expect("committed");
    e.execute(&set(
        KindName::Connection,
        target,
        "overrides.colour",
        json!(["DBU"]),
    ))
    .expect("a declared company code");
    e.execute(&set(
        KindName::Connection,
        target,
        "overrides.colour",
        Value::Null,
    ))
    .expect("null removes the colour");
    assert_eq!(conn(&e, CON_W22).overrides.colour, None);
}

// ---------------------------------------------------------------------------------------------------------
// box.structure

/// `box.structure` creates the box; every rejection at its pointer.
#[test]
fn box_structure_creates_a_box_and_refuses_bad_members() {
    let mut e = engine(golden_model());
    let ext = e
        .execute(&CreateExternalRef {
            location: LOC,
            device: Ident::from_input("JE-N1000-03").unwrap(),
            connection: None,
            project: None,
        })
        .unwrap()
        .outcome
        .primary
        .unwrap()
        .1
        .cast::<ExternalRef>();
    let mut cmd = structure_box(location(LOC));
    cmd.external = Some(ext);
    let out = e.execute(&cmd).expect("created");
    let b = sbox(&e, out.outcome.primary.unwrap().1.cast());
    assert_eq!(b.outline.len(), 4);
    assert_eq!(b.caption, Some(MlText::neutral("External Connection")));
    assert_eq!(b.external, Some(ext));

    let mut two = structure_box(location(LOC));
    two.outline = vec![pt(0, 0), pt(1, 1)];
    assert_eq!(pointer(e.execute(&two).unwrap_err(), "box.structure"), "/outline");
    let mut flat = structure_box(location(LOC));
    flat.outline = vec![pt(0, 0), pt(1, 1), pt(2, 2), pt(3, 3)];
    assert_eq!(
        pointer(e.execute(&flat).unwrap_err(), "box.structure"),
        "/outline"
    );
    let wrong_aspect = structure_box(location(FUNC_P01));
    assert_eq!(
        pointer(e.execute(&wrong_aspect).unwrap_err(), "box.structure"),
        "/assigns/location"
    );
    let missing_node = structure_box(location(id(0x1f)));
    assert_eq!(
        pointer(e.execute(&missing_node).unwrap_err(), "box.structure"),
        "/assigns/location"
    );
    let mut missing_ext = structure_box(location(LOC));
    missing_ext.external = Some(id(0x6ff));
    assert_eq!(
        pointer(e.execute(&missing_ext).unwrap_err(), "box.structure"),
        "/external"
    );
    let mut no_page = structure_box(location(LOC));
    no_page.page = id(0x3f);
    assert!(matches!(e.execute(&no_page), Err(CmdError::NotFound { .. })));
    // An empty caption is not stored; `none` is a valid slot.
    let mut plain = structure_box(StructureAssignment {
        function: Some(Slot::None),
        ..StructureAssignment::default()
    });
    plain.caption = MlText::default();
    let out = e.execute(&plain).unwrap();
    assert_eq!(sbox(&e, out.outcome.primary.unwrap().1.cast()).caption, None);
}

// ---------------------------------------------------------------------------------------------------------
// rep.move with items

/// Items and boxes move with the representations by `delta`, and alone by grid steps (the steps' rounded nm length).
#[test]
fn rep_move_moves_items_and_boxes() {
    let mut e = engine(golden_model());
    let line = place(
        &mut e,
        &graphic(ShapeParam::Line {
            a: pt(0, 0),
            b: pt(10_000_000, 0),
        }),
    );
    let label = place(&mut e, &text("40x60", None, 0));
    let b: Id<StructureBox> = e
        .execute(&structure_box(location(LOC)))
        .unwrap()
        .outcome
        .primary
        .unwrap()
        .1
        .cast();
    e.execute(&mv(
        vec![REP_101F1],
        vec![line.cast(), b.cast(), label.cast()],
        pt(7_937_500, 15_875_000),
    ))
    .expect("moved");
    let PageItemKind::Graphic(g) = &item(&e, line).kind else {
        panic!("a graphic")
    };
    let GraphicShape::Line(l) = &g.shape else {
        panic!("a line")
    };
    assert_eq!(
        (l.a, l.b),
        (pt(7_937_500, 15_875_000), pt(17_937_500, 15_875_000))
    );
    assert_eq!(sbox(&e, b).outline[0], pt(7_937_500, 15_875_000));
    // By grid steps, items alone: k × num / den rounded half away (page 101's grid).
    let grid = e.model().table::<Page>().get(&PAGE_101).unwrap().grid;
    let step = |k: i64| -> i64 {
        let num = i128::from(k) * i128::from(grid.num());
        let den = i128::from(grid.den());
        let (q, r) = (num / den, num % den);
        i64::try_from(if 2 * r.abs() >= den { q + num.signum() } else { q }).unwrap()
    };
    e.execute(&MoveReps {
        items: vec![label.cast()],
        steps: Some([2, -1]),
        reps: vec![],
        delta: Pt2::ORIGIN,
        page: None,
    })
    .expect("a step move of an item");
    let PageItemKind::FreeText(t) = &item(&e, label).kind else {
        panic!("a free text")
    };
    assert_eq!(
        t.at,
        pt(
            50_800_000 + 7_937_500 + step(2),
            30_480_000 + 15_875_000 + step(-1)
        )
    );
}

/// The rejections of `rep.move`'s `items`.
#[test]
fn rep_move_items_rejections() {
    let mut e = engine(golden_model());
    let on_101 = place(&mut e, &text("a", None, 0));
    let mut other = text("b", None, 0);
    other.page = PAGE_102;
    let on_102 = place(&mut e, &other);
    let none = e.execute(&mv(vec![], vec![], pt(1, 0))).unwrap_err();
    assert!(matches!(none, CmdError::Rejected { .. }), "{none:?}");
    assert_eq!(
        pointer(
            e.execute(&mv(vec![], vec![on_101.cast(), on_102.cast()], pt(1, 0)))
                .unwrap_err(),
            "rep.move"
        ),
        "/items/1"
    );
    assert_eq!(
        pointer(
            e.execute(&mv(vec![REP_102G1], vec![on_101.cast()], pt(1, 0)))
                .unwrap_err(),
            "rep.move"
        ),
        "/reps/0"
    );
    assert_eq!(
        pointer(
            e.execute(&mv(vec![], vec![on_101.cast(), on_101.cast()], pt(1, 0)))
                .unwrap_err(),
            "rep.move"
        ),
        "/items/1"
    );
    let mut cross = mv(vec![REP_101F1], vec![on_101.cast()], pt(1, 0));
    cross.page = Some(PAGE_102);
    assert_eq!(pointer(e.execute(&cross).unwrap_err(), "rep.move"), "/items");
    assert!(matches!(
        e.execute(&mv(vec![], vec![id::<()>(0x4ff)], pt(1, 0))),
        Err(CmdError::NotFound { .. })
    ));
}

/// An item of a variant this build keeps verbatim cannot be moved.
#[test]
fn rep_move_refuses_an_unknown_variant() {
    let (project, mut records, _) = mini_golden_parts();
    let lib = library();
    let unknown: PageItem = serde_json::from_value(
        json!({"id": id::<()>(0x4300).to_string(), "page": PAGE_101.to_string(),
        "kind": {"macro_box": {"key": "m"}}}),
    )
    .unwrap();
    records.push(Record::PageItem(unknown));
    let mut e = engine(Model::from_records(project, records, lib).unwrap());
    let err = e
        .execute(&mv(vec![], vec![id::<()>(0x4300)], pt(1, 0)))
        .unwrap_err();
    let CmdError::Rejected { reason, .. } = err else {
        panic!("{err:?}");
    };
    assert_eq!(
        reason,
        format!(
            "page item {} cannot be moved: it is of a kind this build keeps verbatim, or the move takes it \
             outside the drawable range",
            id::<()>(0x4300)
        )
    );
}

/// A cross-page `rep.move` takes the connections that leave the page out of the old page's wire markers; a marker
/// left empty goes, and undo restores it.
#[test]
fn a_cross_page_move_unlinks_the_old_pages_markers() {
    let mut e = engine(golden_model());
    let mut m = marker(vec![CON_W11A]);
    m.page = PAGE_101;
    let marker_id: Id<PageItem> = e.execute(&m).unwrap().outcome.primary.unwrap().1.cast();
    e.execute(&MoveReps {
        items: Vec::new(),
        steps: None,
        reps: vec![REP_101F1, REP_IP_101],
        delta: pt(0, 0),
        page: Some(PAGE_102),
    })
    .expect("moved across pages");
    assert_eq!(conn(&e, CON_W11A).page, Some(PAGE_102));
    assert!(
        e.model().table::<PageItem>().get(&marker_id).is_none(),
        "the marker left empty goes"
    );
    e.undo(&Origin::Gui).expect("undo");
    let PageItemKind::WireMarker(w) = &item(&e, marker_id).kind else {
        panic!("a marker");
    };
    assert_eq!(w.connections, vec![CON_W11A]);
}

// ---------------------------------------------------------------------------------------------------------
// edit.delete and the marker list

/// `edit.delete` takes page items and structure boxes; deleting a connection takes it out of a marker's list and a
/// marker left empty goes with it; the junction clean-up that merges two connections puts the kept one in the
/// removed one's place.
#[test]
fn delete_unlinks_markers_and_takes_items_and_boxes() {
    let mut e = engine(golden_model());
    let both: Id<PageItem> = e
        .execute(&marker(vec![CON_W29, CON_W27]))
        .unwrap()
        .outcome
        .primary
        .unwrap()
        .1
        .cast();
    let alone: Id<PageItem> = e
        .execute(&marker(vec![CON_W29]))
        .unwrap()
        .outcome
        .primary
        .unwrap()
        .1
        .cast();
    e.execute(&DeleteSelection {
        targets: vec![TargetRef {
            kind: KindName::Connection,
            id: CON_W29.cast(),
        }],
    })
    .expect("deleted");
    assert!(
        e.model().table::<PageItem>().get(&alone).is_none(),
        "an empty marker goes"
    );
    let PageItemKind::WireMarker(w) = &item(&e, both).kind else {
        panic!("a marker");
    };
    assert_eq!(w.connections, vec![CON_W27]);
    let feed: Id<PageItem> = e
        .execute(&marker(vec![CON_J5_IN]))
        .unwrap()
        .outcome
        .primary
        .unwrap()
        .1
        .cast();
    // Junction J5 joins J5_IN, W22 and W27: deleting W22 leaves two members, which the clean-up merges into the
    // kept one; a marker that listed the removed member lists the kept one instead, so both markers survive.
    e.execute(&DeleteSelection {
        targets: vec![TargetRef {
            kind: KindName::Connection,
            id: CON_W22.cast(),
        }],
    })
    .expect("deleted");
    let survivor: Vec<Id<Connection>> = [CON_J5_IN, CON_W27]
        .into_iter()
        .filter(|c| e.model().table::<Connection>().get(c).is_some())
        .collect();
    assert_eq!(survivor.len(), 1, "the clean-up merged J5's two members");
    for m in [both, feed] {
        let it = e
            .model()
            .table::<PageItem>()
            .get(&m)
            .expect("the marker survives");
        let PageItemKind::WireMarker(w) = &it.kind else {
            panic!("a marker");
        };
        assert_eq!(
            w.connections, survivor,
            "the kept member takes the removed one's place"
        );
    }
    // Items and boxes by `edit.delete`.
    let t = place(&mut e, &text("x", None, 0));
    let b: Id<StructureBox> = e
        .execute(&structure_box(location(LOC)))
        .unwrap()
        .outcome
        .primary
        .unwrap()
        .1
        .cast();
    e.execute(&DeleteSelection {
        targets: vec![
            TargetRef {
                kind: KindName::PageItem,
                id: t.cast(),
            },
            TargetRef {
                kind: KindName::StructureBox,
                id: b.cast(),
            },
        ],
    })
    .expect("deleted");
    assert!(e.model().table::<PageItem>().get(&t).is_none());
    assert!(e.model().table::<StructureBox>().get(&b).is_none());
}

/// Deleting a page deletes its items and boxes (FR-PAG-025).
#[test]
fn deleting_a_page_deletes_its_items_and_boxes() {
    let mut e = engine(golden_model());
    place(&mut e, &text("x", None, 0));
    e.execute(&structure_box(location(LOC))).unwrap();
    e.execute(&DeleteSelection {
        targets: vec![TargetRef {
            kind: KindName::Page,
            id: PAGE_101.cast(),
        }],
    })
    .expect("deleted");
    assert_eq!(e.model().table::<PageItem>().len(), 0);
    assert_eq!(e.model().table::<StructureBox>().len(), 0);
}

// ---------------------------------------------------------------------------------------------------------
// fields.set

/// The settable paths of the two kinds, the variant rule and the checks repeated at `/value`.
#[test]
fn fields_set_edits_items_and_boxes_with_the_place_checks() {
    let mut e = engine(golden_model());
    let t = place(&mut e, &text("40x60", None, 0));
    let line = place(
        &mut e,
        &graphic(ShapeParam::Line {
            a: pt(0, 0),
            b: pt(10, 0),
        }),
    );
    let b: Id<StructureBox> = e
        .execute(&structure_box(location(LOC)))
        .unwrap()
        .outcome
        .primary
        .unwrap()
        .1
        .cast();
    e.execute(&set(
        KindName::PageItem,
        t.cast(),
        "kind.free_text.text",
        json!("50x70"),
    ))
    .unwrap();
    e.execute(&set(KindName::PageItem, t.cast(), "layer", json!("notes")))
        .unwrap();
    let it = item(&e, t);
    assert_eq!(it.layer.as_ref().map(Ident::as_str), Some("notes"));
    // Another variant's path, `kind` as a whole and a marker's connections are refused at `/path`.
    for path in ["kind.graphic.shape", "kind", "kind.wire_marker.connections"] {
        let err = e
            .execute(&set(KindName::PageItem, t.cast(), path, json!(null)))
            .unwrap_err();
        assert!(
            matches!(&err, CmdError::InvalidParams { path: p, .. } if p == "/path"),
            "{path}: {err:?}"
        );
    }
    // The place checks at `/value`.
    let refusals = [
        (t, "kind.free_text.bind", json!("project.reference")),
        (t, "kind.free_text.angle", json!(45_000)),
        (t, "kind.free_text.align", json!("middle")),
        (
            line,
            "kind.graphic.shape",
            json!({"line": {"a": [0, 0], "b": [0, 0]}}),
        ),
        (line, "kind.graphic.dash", json!("zigzag")),
    ];
    for (target, path, value) in refusals {
        let err = e
            .execute(&set(KindName::PageItem, target.cast(), path, value.clone()))
            .unwrap_err();
        assert!(
            matches!(&err, CmdError::InvalidParams { path: p, .. } if p == "/value"),
            "{path} = {value}: {err:?}"
        );
    }
    // A box: outline, style, caption, assigns, external; the checks at `/value`.
    e.execute(&set(KindName::StructureBox, b.cast(), "style", json!("dash_dot")))
        .unwrap();
    for (path, value) in [
        ("outline", json!([[0, 0], [1, 1]])),
        ("assigns", json!({"location": FUNC_P01.to_string()})),
        ("external", json!(id::<()>(0x6ff).to_string())),
        ("style", json!("wavy")),
    ] {
        let err = e
            .execute(&set(KindName::StructureBox, b.cast(), path, value.clone()))
            .unwrap_err();
        assert!(
            matches!(&err, CmdError::InvalidParams { path: p, .. } if p == "/value"),
            "{path} = {value}: {err:?}"
        );
    }
}

/// The next edit of a wire marker of an older file writes a clean list.
#[test]
fn an_edit_of_a_stale_marker_writes_a_clean_list() {
    let (project, mut records, _) = mini_golden_parts();
    let lib = library();
    let stale: PageItem = serde_json::from_value(json!({"id": id::<()>(0x4300).to_string(), "page": PAGE_102.to_string(),
        "kind": {"wire_marker": {"connections": [CON_W22.to_string(), id::<()>(0x4ff).to_string(), CON_W11A.to_string()],
            "at": [0, 0], "fields": ["csa", "colour"]}}}))
    .unwrap();
    records.push(Record::PageItem(stale));
    let mut e = engine(Model::from_records(project, records, lib).expect("a stale list loads"));
    e.execute(&set(
        KindName::PageItem,
        id::<()>(0x4300),
        "kind.wire_marker.angle",
        json!(90_000),
    ))
    .expect("an edit of the marker");
    let PageItemKind::WireMarker(w) = &item(&e, id(0x4300)).kind else {
        panic!("a marker");
    };
    assert_eq!(w.connections, vec![CON_W22]);
    e.undo(&Origin::Gui).expect("undo restores the stale list");
}

// ---------------------------------------------------------------------------------------------------------
// project.drawing.set marker_mode

#[test]
fn drawing_set_writes_and_removes_the_marker_mode() {
    let mut e = engine(golden_model());
    e.execute(&SetDrawing {
        crossings: None,
        marker_mode: Some(Some(MarkerModeParam::Deviations)),
    })
    .unwrap();
    let drawing = |e: &Engine| e.model().project().settings.extra.get("drawing").cloned();
    assert_eq!(drawing(&e), Some(json!({"marker_mode": "deviations"})));
    e.execute(&SetDrawing {
        crossings: Some(Some(ew_commands::commands::Crossings::Hops)),
        marker_mode: None,
    })
    .unwrap();
    assert_eq!(
        drawing(&e),
        Some(json!({"crossings": "hops", "marker_mode": "deviations"}))
    );
    e.execute(&SetDrawing {
        crossings: Some(None),
        marker_mode: Some(None),
    })
    .unwrap();
    assert_eq!(drawing(&e), None, "an object left empty is removed");
    let err = e
        .execute(&SetDrawing {
            crossings: None,
            marker_mode: None,
        })
        .unwrap_err();
    assert_eq!(pointer(err, "project.drawing.set"), "");
    let _ = Kind::PageItem;
}
