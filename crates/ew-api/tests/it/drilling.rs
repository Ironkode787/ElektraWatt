// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The drawings for drilling (doc 13 §9.1) of the sample's door: the template lies in the door's bottom-left corner,
//! the corner the hole table measures from, and each hole stands whole on one of its printed sheets; the DXF holds the handle's two fixings through the door
//! and the six cut-outs, each on its tool's layer, and two exports are the same bytes. A part made in the test, not a
//! starter part, puts its own holes on the door's drawings. The door's DXF is `tests/expected/door_drilling.dxf`.

use crate::common;

use ew_api::output::{DrillingAsk, Paper};
use ew_api::{AnyCommand, Session};
use ew_ids::{Id, Uuid};
use ew_schema::{Corner, MountingSurface, SurfaceKind};
use serde_json::json;

fn run(s: &mut Session, cmd: &AnyCommand) {
    match s.execute(cmd) {
        Err(ew_api::CmdError::NeedsPreview { .. }) => s.preview(cmd).and_then(|p| s.commit(p)),
        other => other,
    }
    .expect(&cmd.name);
}

/// The door of the sample's cabinet.
fn door(s: &Session) -> Id<MountingSurface> {
    s.query()
        .table::<MountingSurface>()
        .iter()
        .find(|(_, f)| f.kind == SurfaceKind::Door)
        .map(|(id, _)| id)
        .expect("the sample's door")
}

/// Each `CIRCLE` of a DXF text: its layer and its radius as written.
fn circles(text: &str) -> Vec<(String, String)> {
    text.split("\n  0\nCIRCLE\n")
        .skip(1)
        .map(|e| {
            let lines: Vec<&str> = e.lines().collect();
            let value = |code: &str| {
                lines
                    .chunks(2)
                    .find(|p| p[0].trim() == code)
                    .map(|p| p[1].to_owned())
                    .unwrap_or_default()
            };
            (value("8"), value("40"))
        })
        .collect()
}

/// The door's DXF text.
fn door_dxf(s: &Session, door: Id<MountingSurface>) -> String {
    let ask = DrillingAsk {
        surfaces: vec![door.as_uuid()],
        paper: Paper::A4,
        template: false,
        dxf: true,
        date: None,
    };
    let files = s.drilling_files(&ask).expect("the drawings");
    let dxf = files.iter().find(|f| f.name.ends_with(".dxf")).expect("the DXF");
    String::from_utf8(dxf.bytes.clone()).expect("UTF-8")
}

#[test]
fn the_door_s_template_lies_in_its_datum_corner_and_its_dxf_holds_its_holes() {
    let s = common::sample();
    let door = door(&s);
    let q = s.query();
    let t = ew_derive::layout::template_sheets(&q, door, Paper::A4).expect("holes");
    assert_eq!(
        t.corner,
        Corner::BottomLeft,
        "the corner the hole table measures from"
    );
    // A hole on the join of two sheets stands whole on one of them: H5 of the door lies on a join of the template.
    for hole in ew_derive::layout::hole_boxes(&q, door) {
        assert!(
            t.tiles
                .iter()
                .filter(|a| a.number.is_some())
                .any(|a| a.area.min().x <= hole.min().x
                    && a.area.min().y <= hole.min().y
                    && hole.max().x <= a.area.max().x
                    && hole.max().y <= a.area.max().y),
            "{hole:?} is whole on no sheet"
        );
    }
    drop(q);

    let ask = DrillingAsk {
        surfaces: vec![door.as_uuid()],
        paper: Paper::A4,
        template: true,
        dxf: true,
        date: None,
    };
    let files = s.drilling_files(&ask).expect("the drawings");
    assert_eq!(files, s.drilling_files(&ask).expect("again"), "the same bytes");
    let pdf = files
        .iter()
        .find(|f| f.name.ends_with(".pdf"))
        .expect("the template");
    assert!(pdf.bytes.starts_with(b"%PDF"));
    let text = door_dxf(&s, door);
    // The door's DXF as the workshop gets it: `tests/expected/door_drilling.dxf` (`EW_BLESS=1` writes it).
    let expected = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/expected/door_drilling.dxf");
    if ew_testdir::bless() {
        std::fs::write(expected, &text).expect("door_drilling.dxf");
    }
    let want = std::fs::read_to_string(expected).expect("door_drilling.dxf (EW_BLESS=1 writes it)");
    assert_eq!(text, want.replace("\r\n", "\n"));
    let probe =
        ew_import_dxf::probe(text.as_bytes(), &ew_interop_api::Limits::default()).expect("it reads back");
    assert_eq!(probe.unit, Some(ew_import_dxf::Unit::Millimetre));
    let found = circles(&text);
    let of = |layer: &str, r: &str| found.iter().filter(|(l, x)| l == layer && x == r).count();
    assert_eq!(
        of("HOLE_4.5", "2.25"),
        2,
        "the handle's fixings, through the door"
    );
    assert_eq!(
        of("CUT_22.3", "11.15"),
        6,
        "the five fronts' and the handle's cut-outs"
    );
}

/// What every template keeps: neighbouring sheets overlap by 10 mm, the sheets reach over what it covers, every hole a
/// sheet can hold stands whole on a printed one, and every other hole is named as drawn across the sheets.
fn keeps_its_rules(s: &Session, door: Id<MountingSurface>, t: &ew_derive::layout::TemplateSheets) {
    let overlap = ew_derive::layout::TEMPLATE_OVERLAP;
    let tile = |row: u32, col: u32| {
        t.tiles
            .iter()
            .find(|a| (a.row, a.col) == (row, col))
            .expect("a tile")
    };
    for a in &t.tiles {
        if a.col + 1 < t.cols {
            let next = tile(a.row, a.col + 1).area;
            let shared = if t.left() {
                a.area.max().x.0 - next.min().x.0
            } else {
                next.max().x.0 - a.area.min().x.0
            };
            assert_eq!(shared, overlap.0, "{a:?} and the sheet beside it");
        }
        if a.row + 1 < t.rows {
            let next = tile(a.row + 1, a.col).area;
            let shared = if t.top() {
                next.max().y.0 - a.area.min().y.0
            } else {
                a.area.max().y.0 - next.min().y.0
            };
            assert_eq!(shared, overlap.0, "{a:?} and the sheet above or below it");
        }
    }
    let reach = t.tiles.iter().fold(t.tiles[0].area, |r, a| r.union(a.area));
    assert!(
        reach.contains(t.cover.min()) && reach.contains(t.cover.max()),
        "{reach:?} {:?}",
        t.cover
    );
    let q = s.query();
    for (f, hole) in q
        .get::<ew_derive::layout::SurfaceFeatures>(door)
        .iter()
        .zip(ew_derive::layout::hole_boxes(&q, door))
    {
        let whole = t.tiles.iter().filter(|a| a.number.is_some()).any(|a| {
            a.area.min().x <= hole.min().x
                && a.area.min().y <= hole.min().y
                && hole.max().x <= a.area.max().x
                && hole.max().y <= a.area.max().y
        });
        assert!(
            whole || t.larger.contains(&f.no),
            "H{} {hole:?} is whole on no sheet",
            f.no
        );
    }
}

/// The sample's door with a rectangular cut-out of one's own, `w` × `h` mm about the point `at` of its sheet, and its
/// template on A4.
fn with_cut_out(
    w: i64,
    h: i64,
    at: (i64, i64),
) -> (Session, Id<MountingSurface>, ew_derive::layout::TemplateSheets) {
    with_cut_outs(&[(w, h, at)])
}

/// The sample's door with rectangular cut-outs of one's own, each `w` × `h` mm about its point `at` of the door's sheet,
/// and its template on A4.
fn with_cut_outs(
    cuts: &[(i64, i64, (i64, i64))],
) -> (Session, Id<MountingSurface>, ew_derive::layout::TemplateSheets) {
    let mut s = common::sample();
    let door = door(&s);
    for (w, h, at) in cuts {
        run(
            &mut s,
            &AnyCommand {
                name: "layout.feature.add".into(),
                params: json!({"surface": door.as_uuid().to_string(), "at": [at.0, at.1], "kind": "rect",
                    "size": format!("{w} \u{d7} {h}")}),
            },
        );
    }
    let t = ew_derive::layout::template_sheets(&s.query(), door, Paper::A4).expect("a template");
    (s, door, t)
}

/// Holes whose spans overlap across a join, which no join clears together: each is whole on one sheet or drawn across
/// the sheets and named on the first, never cut without a word. Two holes at 0–150 and 100–230 mm from the corner's
/// edge, three chained the same way, and the pair along the other edge.
#[test]
fn holes_overlapping_across_a_join_are_whole_or_drawn_across() {
    let s = common::sample();
    let sheet = ew_derive::layout::template_sheets(&s.query(), door(&s), Paper::A4)
        .expect("a template")
        .sheet;
    drop(s);
    let mm = |n: i64| n * 1_000_000;
    // A cut-out `w` mm long from `a` mm along one edge of the corner (`across`: from the left edge, else from the
    // bottom), 20 mm wide, 400 mm in along the other.
    let cut = |a: i64, w: i64, across: bool| {
        let (along, other) = (mm(a) + mm(w) / 2, mm(400));
        let at = if across {
            (sheet.min().x.0 + along, sheet.min().y.0 + other)
        } else {
            (sheet.min().x.0 + other, sheet.min().y.0 + along)
        };
        if across { (w, 20, at) } else { (20, w, at) }
    };
    for cuts in [
        vec![cut(0, 150, true), cut(100, 130, true)],
        vec![cut(0, 150, true), cut(100, 130, true), cut(200, 130, true)],
        vec![cut(0, 150, false), cut(100, 130, false)],
    ] {
        let (s, door, t) = with_cut_outs(&cuts);
        keeps_its_rules(&s, door, &t);
    }
}

/// Where the join of the door's first two columns of sheets lies, across, and the middle of its first row, up.
fn first_join(t: &ew_derive::layout::TemplateSheets) -> (i64, i64) {
    let (a, b) = (t.tiles[0].area, t.tiles[1].area);
    let x = if t.left() {
        (b.min().x.0 + a.max().x.0) / 2
    } else {
        (a.min().x.0 + b.max().x.0) / 2
    };
    (x, (a.min().y.0 + a.max().y.0) / 2)
}

/// A hole on a join, a large cut-out and one larger than a sheet: the sheets still overlap by the 10 mm that lines them
/// up, the join moves so that a hole one sheet can hold stands whole on one, a cut-out takes at most one sheet more
/// across and up than the door without it, and a cut-out larger than a sheet is drawn across them, which the first
/// sheet says; the door always gets its template.
#[test]
fn a_join_moves_clear_of_a_hole_and_a_hole_larger_than_a_sheet_is_drawn_across() {
    let s = common::sample();
    let plain = ew_derive::layout::template_sheets(&s.query(), door(&s), Paper::A4).expect("a template");
    assert!(plain.cols > 1, "the door takes two columns of sheets");
    let join = first_join(&plain);
    // A 22 mm hole on the join: the join moves clear of it.
    let (s, door, t) = with_cut_out(22, 22, join);
    keeps_its_rules(&s, door, &t);
    // A 150 mm cut-out on it: a sheet more at most, across and up.
    let (s, door, t) = with_cut_out(150, 150, join);
    keeps_its_rules(&s, door, &t);
    assert!(
        t.cols <= plain.cols + 1 && t.rows <= plain.rows + 1,
        "{} × {}",
        t.rows,
        t.cols
    );
    // The 197 × 141 mm cut-out of a touch panel on A4.
    let (s, door, t) = with_cut_out(197, 141, join);
    keeps_its_rules(&s, door, &t);
    // A cut-out larger than any sheet of A4 is drawn across them, and the first sheet names it.
    let (s, door, t) = with_cut_out(300, 290, join);
    keeps_its_rules(&s, door, &t);
    assert_eq!(t.larger.len(), 1, "the one larger than a sheet");
    let ask = DrillingAsk {
        surfaces: vec![door.as_uuid()],
        paper: Paper::A4,
        template: true,
        dxf: false,
        date: None,
    };
    let files = s.drilling_files(&ask).expect("the drawings");
    let pages = crate::pdf_read::PdfFile::parse(&files[0].bytes)
        .and_then(|f| f.pages())
        .expect("the template reads back");
    let says =
        |p: &crate::pdf_read::PageRead| p.runs.iter().any(|r| r.text.contains("Drawn across the sheets"));
    assert!(says(&pages[0]), "the first sheet says so");
    assert!(!pages[1..].iter().any(says), "only the first sheet says so");
}

/// A door whose sheet a file stretches from the least to the greatest coordinate the format holds gets no drawings,
/// where the template's and the DXF's sums of its corners would leave `i64`: Drilling… leaves it out and asking for it
/// makes no file.
#[test]
fn a_sheet_beyond_the_drawing_range_gets_no_drawings() {
    let (project, mut records, lib) = common::sample_records();
    let far = i64::MAX - 1;
    let at = |x: i64, y: i64| ew_units::Pt2::new(ew_units::Nm(x), ew_units::Nm(y));
    for r in &mut records {
        if let ew_schema::Record::MountingSurface(m) = r
            && m.kind == SurfaceKind::Door
        {
            m.usable = vec![at(-far, -far), at(far, -far), at(far, far), at(-far, far)];
        }
    }
    let s = common::session_of(project, records, lib);
    let door = door(&s);
    assert!(
        !ew_derive::layout::drawable(&s.query(), door),
        "the door reaches past the range"
    );
    let offered = s.drilling_surfaces(Paper::A4);
    assert!(!offered.is_empty(), "the plate and the bottom are offered");
    assert!(offered.iter().all(|d| d.surface != door.as_uuid()));
    let ask = DrillingAsk {
        surfaces: vec![door.as_uuid()],
        paper: Paper::A4,
        template: true,
        dxf: true,
        date: None,
    };
    assert_eq!(s.drilling_files(&ask).expect("no file"), Vec::new());
}

#[test]
fn a_part_of_one_s_own_puts_its_holes_on_the_door_s_drawings() {
    let mut s = common::sample();
    let door = door(&s);
    let before = circles(&door_dxf(&s, door))
        .iter()
        .filter(|(l, _)| l == "HOLE_4.5")
        .count();
    // A lamp of one's own: a 40 mm square front with its cut-out and two M4 fixings.
    let mut d = s.new_part_draft().expect("a new part");
    d.to_library = false;
    for (f, v) in [
        ("manufacturer", "Acme"),
        ("part_number", "OWN-LAMP-40"),
        ("mech.width", "40"),
        ("mech.height", "40"),
        ("mech.mount_kind", "door_cutout"),
    ] {
        s.set_draft_field(&mut d, f, v).expect(f);
    }
    for (i, (x, kind, size)) in [
        ("20", "cutout", "22 mm"),
        ("5", "fixing", "M4"),
        ("35", "fixing", "M4"),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(d.add_table_row("mech.holes"));
        for (f, v) in [("x", x), ("y", "20"), ("hole.kind", kind), ("hole.size", size)] {
            s.set_draft_field(&mut d, &format!("mech.holes[{i}].{f}"), v)
                .expect(f);
        }
    }
    let made = s.new_part_command(&d).expect("the new part's command");
    run(&mut s, &made.command);
    let copy = s
        .query()
        .parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == "OWN-LAMP-40")
        .map(|(id, _)| id.as_uuid())
        .expect("the part is held");
    // -102E1 takes it in place of its starter lamp, in the same place on the door.
    let lamp: Uuid = s.resolve_reference("tag:-102E1").expect("-102E1");
    run(
        &mut s,
        &AnyCommand {
            name: "fields.set".into(),
            params: json!({"kind": "device", "targets": [lamp.to_string()], "path": "parts",
                "value": [{"part": {"copy": copy.to_string()}, "qty": "1"}]}),
        },
    );
    // Its placement on the door goes, and it is placed again with its new part where it stood.
    let placement = s
        .query()
        .table::<ew_schema::Placement>()
        .iter()
        .find(|(_, p)| p.device.as_uuid() == lamp)
        .map(|(id, _)| id.as_uuid())
        .expect("-102E1 is placed");
    run(
        &mut s,
        &AnyCommand {
            name: "edit.delete".into(),
            params: json!({"targets": [{"kind": "placement", "id": placement.to_string()}]}),
        },
    );
    run(
        &mut s,
        &AnyCommand {
            name: "layout.place".into(),
            params: json!({"device": lamp.to_string(), "part": copy.to_string(),
                "carrier": {"surface": door.as_uuid().to_string()},
                "pose": {"t": [240_000_000, 560_000_000, 0], "q": [1_000_000_000, 0, 0, 0]}}),
        },
    );
    let after = circles(&door_dxf(&s, door))
        .iter()
        .filter(|(l, _)| l == "HOLE_4.5")
        .count();
    assert_eq!(after, before + 2, "the own part's two fixings, through the door");
}

/// In a Dutch project the label files and the drawings for drilling are named in Dutch, in the words the window uses
/// for the same things, as the lists' files are: no English name is left in them; the template's sheets and the DXF
/// write their words in Dutch as well.
#[test]
fn a_dutch_project_names_its_label_files_and_drawings_for_drilling_in_dutch() {
    let mut s = common::sample();
    common::run_cmd(
        &mut s,
        "project.settings.set",
        json!({"member": "languages", "value": {"source": "nl-NL"}}),
    );
    let labels: Vec<String> = s
        .label_files(&ew_api::LabelScope::All, None, None)
        .expect("the label files")
        .into_iter()
        .map(|f| f.name)
        .collect();
    assert!(labels.iter().any(|n| n.contains(" Draadmerkers ")), "{labels:?}");
    assert!(!labels.iter().any(|n| n.contains("markers")), "{labels:?}");
    let ask = DrillingAsk {
        surfaces: vec![door(&s).as_uuid()],
        paper: Paper::A4,
        template: true,
        dxf: true,
        date: None,
    };
    let files = s.drilling_files(&ask).expect("the drawings");
    let drilling: Vec<String> = files.iter().map(|f| f.name.clone()).collect();
    assert!(
        drilling.iter().any(|n| n.ends_with(" Deur boormal.pdf")),
        "{drilling:?}"
    );
    assert!(
        !drilling
            .iter()
            .any(|n| n.contains("Door") || n.contains("template")),
        "{drilling:?}"
    );
    // The template's sheets print their words in Dutch: the sheet's place, the hole table's corner, the 100 mm bar and
    // the edges to lay it on; the DXF's datum too.
    let pdf = files
        .iter()
        .find(|f| f.name.ends_with(".pdf"))
        .expect("the template");
    let runs: Vec<String> = crate::pdf_read::PdfFile::parse(&pdf.bytes)
        .and_then(|f| f.pages())
        .expect("the template reads back")
        .into_iter()
        .flat_map(|p| p.runs.into_iter().map(|r| r.text))
        .collect();
    for dutch in [
        "Blad 1 van ",
        "Gatentabel: X en Y vanaf de hoek",
        "100 mm: print op ware grootte",
        "kant van de deur: leg deze kant ertegen",
    ] {
        assert!(runs.iter().any(|r| r.contains(dutch)), "{dutch}: {runs:#?}");
    }
    for english in ["Sheet ", "Hole table", "actual size", "lay this edge"] {
        assert!(!runs.iter().any(|r| r.contains(english)), "{english}: {runs:#?}");
    }
    let dxf = files.iter().find(|f| f.name.ends_with(".dxf")).expect("the DXF");
    let dxf = String::from_utf8(dxf.bytes.clone()).expect("UTF-8");
    assert!(dxf.contains("0,0: hoek "), "the datum in Dutch");
}
