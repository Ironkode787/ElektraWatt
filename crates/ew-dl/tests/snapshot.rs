// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Snapshot of the IR shape: a symbol instance, a cross-reference text run with a link and a dashed
//! linkage, as `Debug` output. A change to any IR type shows up here in review.
//!
//! The snapshot is inline (no `.snap` files to annotate for REUSE). To update it, run the test, check the
//! new value in the failure diff and paste it, or use `cargo insta test --accept` where cargo-insta is
//! installed.

use std::sync::Arc;

use ew_dl::{
    Brush, DashPattern, Destination, DisplayList, EntityRef, FontKey, Glyph, Item, Layer, Link, Path, Pen,
    ShapedRun, Stroke, SymbolGeomId, Transform,
};
use ew_units::{Nm, Orient2, Pt2, Rect};
use uuid::Uuid;

fn mm(x: i64, y: i64) -> Pt2 {
    Pt2::new(Nm::from_mm(x), Nm::from_mm(y))
}

fn composed() -> DisplayList {
    let symbol = SymbolGeomId {
        symbol: Uuid::from_u128(0x0192_0000_0000_7000_8000_0000_0000_0001),
        variant: Orient2::R90,
        extra: None,
    };
    let representation = Uuid::from_u128(0x0192_0000_0000_7000_8000_0000_0000_0002);
    let target_page = Uuid::from_u128(0x0192_0000_0000_5000_8000_0000_0000_0003);
    let xref = ShapedRun {
        font: FontKey {
            sha256: [0xab; 32],
            face_index: 0,
        },
        size: Nm(2_500_000),
        origin: mm(44, 60),
        transform: Transform::IDENTITY,
        glyphs: vec![
            Glyph {
                id: 20,
                x: Nm(0),
                y: Nm(0),
            },
            Glyph {
                id: 19,
                x: Nm(1_390_000),
                y: Nm(0),
            },
        ],
        text: "13".into(),
        brush: Brush::BLACK,
        layout_box: Rect::from_points(
            Pt2::new(Nm(0), Nm(-1_900_000)),
            Pt2::new(Nm(2_780_000), Nm(500_000)),
        ),
    };
    DisplayList {
        items: vec![
            Item::instance(symbol, Transform::from_translation(mm(40, 60)))
                .with_entity(EntityRef(representation))
                .with_link(Link::Target(Destination::Representation(representation))),
            Item::glyphs(Arc::new(xref)).with_link(Link::Source(Destination::Page(target_page))),
            Item::path(
                Path::line(mm(40, 64), mm(40, 70)),
                Some(Stroke::pen(Pen::P026).dashed(DashPattern::Dash42)),
                None,
            ),
        ],
    }
}

#[test]
fn composed_display_list() {
    insta::assert_debug_snapshot!(composed(), @r#"
    DisplayList {
        items: [
            Item {
                kind: Instance(
                    InstanceItem {
                        symbol: SymbolGeomId {
                            symbol: 01920000-0000-7000-8000-000000000001,
                            variant: R90,
                            extra: None,
                        },
                        transform: Transform {
                            translate: Pt2 {
                                x: Nm(
                                    40000000,
                                ),
                                y: Nm(
                                    60000000,
                                ),
                            },
                            orient: R0,
                            angle: Angle2(
                                0,
                            ),
                        },
                    },
                ),
                entity: Some(
                    EntityRef(
                        01920000-0000-7000-8000-000000000002,
                    ),
                ),
                link: Some(
                    Target(
                        Representation(
                            01920000-0000-7000-8000-000000000002,
                        ),
                    ),
                ),
            },
            Item {
                kind: Glyphs(
                    ShapedRun {
                        font: FontKey {
                            sha256: "abababababababababababababababababababababababababababababababab",
                            face_index: 0,
                        },
                        size: Nm(
                            2500000,
                        ),
                        origin: Pt2 {
                            x: Nm(
                                44000000,
                            ),
                            y: Nm(
                                60000000,
                            ),
                        },
                        transform: Transform {
                            translate: Pt2 {
                                x: Nm(
                                    0,
                                ),
                                y: Nm(
                                    0,
                                ),
                            },
                            orient: R0,
                            angle: Angle2(
                                0,
                            ),
                        },
                        glyphs: [
                            Glyph {
                                id: 20,
                                x: Nm(
                                    0,
                                ),
                                y: Nm(
                                    0,
                                ),
                            },
                            Glyph {
                                id: 19,
                                x: Nm(
                                    1390000,
                                ),
                                y: Nm(
                                    0,
                                ),
                            },
                        ],
                        text: "13",
                        brush: Solid(
                            Color {
                                r: 0,
                                g: 0,
                                b: 0,
                                a: 255,
                            },
                        ),
                        layout_box: Rect {
                            min: Pt2 {
                                x: Nm(
                                    0,
                                ),
                                y: Nm(
                                    -1900000,
                                ),
                            },
                            max: Pt2 {
                                x: Nm(
                                    2780000,
                                ),
                                y: Nm(
                                    500000,
                                ),
                            },
                        },
                    },
                ),
                entity: None,
                link: Some(
                    Source(
                        Page(
                            01920000-0000-5000-8000-000000000003,
                        ),
                    ),
                ),
            },
            Item {
                kind: Path(
                    PathItem {
                        path: Path {
                            els: [
                                MoveTo(
                                    Pt2 {
                                        x: Nm(
                                            40000000,
                                        ),
                                        y: Nm(
                                            64000000,
                                        ),
                                    },
                                ),
                                LineTo(
                                    Pt2 {
                                        x: Nm(
                                            40000000,
                                        ),
                                        y: Nm(
                                            70000000,
                                        ),
                                    },
                                ),
                            ],
                        },
                        stroke: Some(
                            Stroke {
                                width: Nm(
                                    260000,
                                ),
                                cap: Butt,
                                join: Miter,
                                dash: Some(
                                    Dash {
                                        segments: [
                                            Nm(
                                                1040000,
                                            ),
                                            Nm(
                                                520000,
                                            ),
                                        ],
                                        offset: Nm(
                                            0,
                                        ),
                                    },
                                ),
                            },
                        ),
                        fill: None,
                    },
                ),
                entity: None,
                link: None,
            },
        ],
    }
    "#);
}

#[test]
fn layer_names() {
    let names: Vec<&str> = Layer::ALL.iter().map(|l| l.name()).collect();
    insta::assert_debug_snapshot!(names, @r#"
    [
        "frame",
        "symbols",
        "wires",
        "labels",
        "tables",
        "revision",
        "highlights",
        "surface",
        "carriers",
        "placements",
        "features",
        "dims",
    ]
    "#);
}
