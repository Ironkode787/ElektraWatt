// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Lowering the shipped frame and title-block forms (content/forms) with sample bindings.

use std::fmt::Write as _;

use ew_dl::{DisplayList, ImageFormat, ItemKind, PathEl};
use ew_scene::forms::{
    AssetRef, BindingFinding, BoundValue, CELL_PADDING, CellOwner, CellRef, FormFinding, OverflowKind,
    PageBindings, SheetMap, lower_form,
};
use ew_schema::{Date, FormDef, LibDocument};
use ew_text::bundled;
use ew_units::{Grid, Nm, Pt2};

fn form(text: &str) -> FormDef {
    match LibDocument::from_json(text).expect("valid form document") {
        LibDocument::Form(copy) => copy.item,
        other => panic!("not a form: {other:?}"),
    }
}

fn grid_frame() -> FormDef {
    form(include_str!(
        "../../../content/forms/ew.frame.qet-a4-landscape-grid.json"
    ))
}

fn nogrid_frame() -> FormDef {
    form(include_str!(
        "../../../content/forms/ew.frame.qet-a4-landscape-nogrid.json"
    ))
}

fn title_block() -> FormDef {
    form(include_str!("../../../content/forms/ew.titleblock.qet-a4.json"))
}

/// Folio 101-like bindings with neutral placeholder data (no project data of the reference).
fn bindings() -> PageBindings {
    let mut b = PageBindings::new();
    for (path, text) in [
        ("page.title", "Power Supply 1 - Main Current"),
        ("page.description", "Expansion Control Cabinets"),
        ("page.function", "=P01"),
        ("page.location", "+J-E-2416"),
        ("page.folio", "101"),
        ("page.prev", "100"),
        ("page.next", "102"),
        ("project.reference", "<project ref>"),
        ("project.drawing_id", "<drawing id>"),
        ("project.object", "J-E-2416"),
        ("revision.current.label", "V3.0"),
        ("revision.current.author.name", "<author>"),
        ("revision.current.checker.name", "<checker>"),
        ("party.end_user.name", "<end user>"),
        (
            "project.notice",
            "Reproduction or disclosure only with written permission.",
        ),
    ] {
        b.text(path, text).unwrap();
    }
    b.set(
        "project.created",
        BoundValue::Date(Date::new(2025, 3, 3).unwrap()),
    )
    .unwrap();
    b.set(
        "project.edited",
        BoundValue::Date(Date::new(2025, 4, 14).unwrap()),
    )
    .unwrap();
    b.set(
        "project.logo",
        BoundValue::Asset(AssetRef::new([7; 32], ImageFormat::Svg)),
    )
    .unwrap();
    b
}

/// One line per item: paths as their points, glyph runs as text, origin and size, images as rectangles.
fn summary(list: &DisplayList) -> String {
    let mut s = String::new();
    for item in &list.items {
        match &item.kind {
            ItemKind::Path(p) => {
                let pts: Vec<String> = p
                    .path
                    .els
                    .iter()
                    .map(|e| match e {
                        PathEl::MoveTo(q) | PathEl::LineTo(q) => format!("{},{}", q.x.0, q.y.0),
                        PathEl::ClosePath => "z".into(),
                        _ => "~".into(),
                    })
                    .collect();
                let w = p.stroke.as_ref().map_or(0, |s| s.width.0);
                writeln!(s, "path w{w} {}", pts.join(" ")).unwrap();
            }
            ItemKind::Glyphs(r) => {
                writeln!(
                    s,
                    "text {:?} at {},{} size {} adv {}",
                    r.text,
                    r.origin.x.0,
                    r.origin.y.0,
                    r.size.0,
                    r.layout_box.max().x.0
                )
                .unwrap();
            }
            ItemKind::Image(i) => writeln!(
                s,
                "image {},{} {},{}",
                i.rect.min().x.0,
                i.rect.min().y.0,
                i.rect.max().x.0,
                i.rect.max().y.0
            )
            .unwrap(),
            other => writeln!(s, "{other:?}").unwrap(),
        }
    }
    s
}

#[test]
fn cell_padding_is_two_qet_px() {
    assert_eq!(Some(CELL_PADDING), Grid::QET_1PX.at(2));
}

#[test]
fn grid_frame_layer() {
    let tb = title_block();
    let out = lower_form(&grid_frame(), Some(&tb), &bindings(), bundled());
    // Frame: outer, separator, two band lines, 16 column ticks, 7 row ticks, 17 + 8 labels; title block:
    // 11 lines, logo image and 17 texts (the logo cell prints no text).
    let paths = out
        .display_list
        .items
        .iter()
        .filter(|i| matches!(i.kind, ItemKind::Path(_)))
        .count();
    assert_eq!(paths, 1 + 1 + 2 + 16 + 7 + 11);
    let texts: Vec<&str> = out
        .display_list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(r) => Some(r.text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(&texts[..3], ["0", "1", "2"]);
    assert_eq!(texts[16], "16");
    assert_eq!(texts[17], "A");
    assert_eq!(texts[24], "H");
    for want in [
        "Power Supply 1 - Main Current",
        "Project: <project ref>",
        "Description: Expansion Control Cabinets",
        "Drawing: <drawing id>",
        "Revision: V3.0",
        "=P01",
        "+J-E-2416",
        "End user: <end user>",
        "Object: J-E-2416",
        "Checked: <checker>",
        "Author: <author>",
        "Creation: 03-03-2025",
        "Saved: 14-04-2025",
        "Page: 101",
        "Prev: 100",
        "Next: 102",
    ] {
        assert!(texts.contains(&want), "missing {want:?} in {texts:?}");
    }
    // Every glyph run lies inside the frame's outer rectangle.
    let outer = out.display_list.items[0].clone();
    let ItemKind::Path(outer) = outer.kind else {
        panic!("outer first")
    };
    let bounds = outer.path.bounds().unwrap();
    assert_eq!(bounds.max(), Pt2::new(Nm(275_166_667), Nm(192_616_667)));
    assert!(out.display_list.bounds().is_some_and(|b| b == bounds));
    // The notice fits on at most four lines; nothing else is reported.
    assert_eq!(out.findings, vec![], "{:#?}", out.findings);
    insta::assert_snapshot!(summary(&out.display_list), @r#"
    path w264583 0,0 0,192616667 275166667,192616667 275166667,0 z
    path w264583 0,174625000 275166667,174625000
    path w264583 0,5291667 275166667,5291667
    path w264583 5291667,0 5291667,174625000
    path w264583 21166667,0 21166667,5291667
    path w264583 37041667,0 37041667,5291667
    path w264583 52916667,0 52916667,5291667
    path w264583 68791667,0 68791667,5291667
    path w264583 84666667,0 84666667,5291667
    path w264583 100541667,0 100541667,5291667
    path w264583 116416667,0 116416667,5291667
    path w264583 132291667,0 132291667,5291667
    path w264583 148166667,0 148166667,5291667
    path w264583 164041667,0 164041667,5291667
    path w264583 179916667,0 179916667,5291667
    path w264583 195791667,0 195791667,5291667
    path w264583 211666667,0 211666667,5291667
    path w264583 227541667,0 227541667,5291667
    path w264583 243416667,0 243416667,5291667
    path w264583 259291667,0 259291667,5291667
    path w264583 0,26458333 5291667,26458333
    path w264583 0,47625000 5291667,47625000
    path w264583 0,68791667 5291667,68791667
    path w264583 0,89958333 5291667,89958333
    path w264583 0,111125000 5291667,111125000
    path w264583 0,132291667 5291667,132291667
    path w264583 0,153458333 5291667,153458333
    text "0" at 12321117,3779308 size 3175000 adv 1816100
    text "1" at 28196117,3779308 size 3175000 adv 1816100
    text "2" at 44071117,3779308 size 3175000 adv 1816100
    text "3" at 59946117,3779308 size 3175000 adv 1816100
    text "4" at 75821117,3779308 size 3175000 adv 1816100
    text "5" at 91696117,3779308 size 3175000 adv 1816100
    text "6" at 107571117,3779308 size 3175000 adv 1816100
    text "7" at 123446117,3779308 size 3175000 adv 1816100
    text "8" at 139321117,3779308 size 3175000 adv 1816100
    text "9" at 155196117,3779308 size 3175000 adv 1816100
    text "10" at 170163067,3779308 size 3175000 adv 3632200
    text "11" at 186038067,3779308 size 3175000 adv 3632200
    text "12" at 201913067,3779308 size 3175000 adv 3632200
    text "13" at 217788067,3779308 size 3175000 adv 3632200
    text "14" at 233663067,3779308 size 3175000 adv 3632200
    text "15" at 249538067,3779308 size 3175000 adv 3632200
    text "16" at 265413067,3779308 size 3175000 adv 3632200
    text "A" at 1631421,17008475 size 3175000 adv 2028825
    text "B" at 1613958,38175141 size 3175000 adv 2063750
    text "C" at 1642533,59341808 size 3175000 adv 2006600
    text "D" at 1486958,80508475 size 3175000 adv 2317750
    text "E" at 1763183,101675141 size 3175000 adv 1765300
    text "F" at 1821921,122841808 size 3175000 adv 1647825
    text "G" at 1490133,144008475 size 3175000 adv 2311400
    text "H" at 1469496,165175141 size 3175000 adv 2352675
    path w264583 12170833,174625000 12170833,192616667
    path w264583 165100000,174625000 165100000,192616667
    path w264583 192616667,174625000 192616667,192616667
    path w264583 220133333,174625000 220133333,192616667
    path w264583 247650000,174625000 247650000,183620833
    path w264583 123825000,183620833 123825000,192616667
    path w264583 261408333,179122917 261408333,183620833
    path w264583 192616667,179122917 275166667,179122917
    path w264583 12170833,183620833 275166667,183620833
    path w264583 12170833,188118750 165100000,188118750
    path w264583 192616667,188118750 220133333,188118750
    image 529167,175154167 11641666,192087500
    text "Power Supply 1 - Main Current" at 12700000,180256391 size 3175000 adv 45259625
    text "Project: <project ref>" at 12700000,186719897 size 2381250 adv 23276719
    text "Description: Expansion Control Cabinets" at 12700000,191217814 size 2381250 adv 44834175
    text "Drawing: <drawing id>" at 124354167,186719897 size 2381250 adv 25269825
    text "Revision: V3.0" at 124354167,191217814 size 2381250 adv 15316200
    text "=P01" at 175173746,180256391 size 3175000 adv 7369175
    text "+J-E-2416" at 171979696,189252225 size 3175000 adv 13757275
    text "End user: <end user>" at 193145834,177724064 size 2381250 adv 23636287
    text "Object: J-E-2416" at 193145834,182221981 size 2381250 adv 17497425
    text "Checked: <checker>" at 193145834,186719897 size 2381250 adv 22221825
    text "Author: <author>" at 193145834,191217814 size 2381250 adv 19292887
    text "Creation: 03-03-2025" at 223583498,177629608 size 2116667 adv 20616337
    text "Saved: 14-04-2025" at 224848206,182127525 size 2116667 adv 18086920
    text "Page: 101" at 254123296,178007433 size 3175000 adv 14570075
    text "Prev: 100" at 249984682,182127525 size 2116667 adv 9088968
    text "Next: 102" at 263590616,182127525 size 2116667 adv 9393768
    text "Reproduction or disclosure only with written permission." at 223065451,188779943 size 1852083 adv 49169099
    "#);
}

#[test]
fn nogrid_frame_has_no_bands_or_labels() {
    let tb = title_block();
    let out = lower_form(&nogrid_frame(), Some(&tb), &bindings(), bundled());
    let first: Vec<String> = summary(&out.display_list)
        .lines()
        .take(3)
        .map(str::to_owned)
        .collect();
    assert_eq!(
        first[0],
        "path w264583 0,0 0,192616667 275166667,192616667 275166667,0 z"
    );
    assert_eq!(first[1], "path w264583 0,174625000 275166667,174625000");
    // Then directly the title block's first inner line.
    assert_eq!(first[2], "path w264583 12170833,174625000 12170833,192616667");
}

#[test]
fn unresolved_and_overflowing_cells_are_reported() {
    let tb = title_block();
    let mut b = bindings();
    let long = "A page title far too long for the large description cell of the title block. ".repeat(4);
    b.text("page.title", &long).unwrap();
    let mut empty = PageBindings::new();
    empty.text("page.folio", "1").unwrap();
    let out = lower_form(&grid_frame(), Some(&tb), &b, bundled());
    assert!(out.findings.iter().any(|f| matches!(
        f,
        FormFinding::Overflow {
            cell: CellRef {
                owner: CellOwner::TitleBlock,
                index: 1
            },
            kind: OverflowKind::ShrinkLimit,
            ..
        }
    )));
    let out = lower_form(&grid_frame(), Some(&tb), &empty, bundled());
    let unresolved: Vec<&str> = out
        .findings
        .iter()
        .filter_map(|f| match f {
            FormFinding::Binding {
                finding: BindingFinding::Unresolved { placeholder },
                ..
            } => Some(placeholder.as_str()),
            _ => None,
        })
        .collect();
    assert!(unresolved.contains(&"{page.title}"));
    assert!(unresolved.contains(&"{project.created | date}"));
    assert!(!unresolved.contains(&"{page.folio}"));
    // Unresolved placeholders print verbatim.
    assert!(summary(&out.display_list).contains("text \"Project: {project.reference}\""));
}

#[test]
fn missing_title_block_is_reported() {
    let out = lower_form(&grid_frame(), None, &bindings(), bundled());
    assert_eq!(out.findings, vec![FormFinding::TitleBlockMissing]);
}

#[test]
fn deterministic_and_sheet_mapped() {
    let tb = title_block();
    let a = lower_form(&grid_frame(), Some(&tb), &bindings(), bundled());
    let b = lower_form(&grid_frame(), Some(&tb), &bindings(), bundled());
    assert_eq!(a, b);
    let map = SheetMap::of(&grid_frame());
    let sheet = map.apply_display_list(&a.display_list, None);
    let b = sheet.bounds().unwrap();
    // Printed frame 9.79–281.18 × 9.79–199.76 mm (doc 20 §17.1), within 10 µm.
    let near = |got: Nm, mm_hundredths: i64| (got.0 - mm_hundredths * 10_000).abs() <= 10_000;
    assert!(near(b.min().x, 979) && near(b.min().y, 979), "{b:?}");
    assert!(near(b.max().x, 28_118) && near(b.max().y, 19_976), "{b:?}");
}
