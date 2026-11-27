// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The drawings for drilling (doc 13 §9.1) of the sample's door: the template lies in the door's top-left corner, the
//! corner nearest its holes, and prints five of six A4 sheets; the DXF holds the handle's two fixings through the door
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
fn the_door_s_template_lies_in_its_top_left_corner_and_its_dxf_holds_its_holes() {
    let s = common::sample();
    let door = door(&s);
    let t = ew_derive::layout::template_sheets(&s.query(), door, Paper::A4).expect("holes");
    assert_eq!(t.corner, Corner::TopLeft);
    assert_eq!((t.rows, t.cols), (2, 3));
    assert_eq!(t.printed(), 5, "the sixth holds no hole and neither edge");

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
