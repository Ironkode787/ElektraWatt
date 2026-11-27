// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Front view and holes through the part editor's actions (doc 07 §5): a part of My parts made with a round front and
//! a 22 mm cut-out opens at the section with its hole as kind and size, takes a fixing and the outline of a DXF
//! drawing, and the update Save previews gives the project's copy both; Remove takes the outline away again.

use crate::common::{Tmp, done};
use ew_viewmodel::ew_api::library::LibraryTiers;
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// A closed 30 × 30 square in a DXF drawing that names no unit.
const SQUARE_DXF: &str = "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLWPOLYLINE\n8\n0\n90\n4\n70\n1\n10\n0\n20\n0\n10\n30\n20\n0\n10\n30\n20\n30\n10\n0\n20\n30\n0\nENDSEC\n0\nEOF\n";

fn fields(s: &mut ShellModel, values: &[(&str, &str)]) {
    for (field, value) in values {
        done(s, "parts.field", json!({"field": field, "value": value}));
    }
}

/// The part editor's holes as (kind, size) and the number of the outline's lines, of LAMP-22 opened again.
fn front_of(s: &mut ShellModel) -> (Vec<(String, String)>, usize) {
    done(s, "parts.open", json!({"part": "LAMP-22"}));
    let Some(editor) = s.parts_place().and_then(|p| p.sheet.as_ref()) else {
        panic!("the part in Parts")
    };
    assert!(editor.sheet.own && editor.sheet.fork, "a part of My parts");
    let t = editor.sheet.table("mech.holes").expect("the holes");
    let at = |m: &str| t.columns.iter().position(|(c, _)| *c == m).expect(m);
    let holes = t
        .rows
        .iter()
        .map(|r| {
            (
                r[at(ew_viewmodel::ew_api::HOLE_KIND)].clone(),
                r[at(ew_viewmodel::ew_api::HOLE_SIZE)].clone(),
            )
        })
        .collect();
    (holes, editor.sheet.outline.len())
}

#[test]
fn a_part_of_my_parts_changes_its_front_view_and_holes() {
    let tmp = Tmp::new("part-front");
    let mine = LibraryTiers::under(&tmp.0.join("data"), &tmp.0.join("cache"), Vec::new(), Vec::new());
    let mut s = ShellModel::new(Mode::Gui, tmp.config().with_libraries(Some(mine)));
    crate::common::make_project(&mut s, &tmp.0.join("p.ew"), &[]);
    done(&mut s, "parts.new", json!({"start": "door"}));
    fields(
        &mut s,
        &[
            ("manufacturer", "Example"),
            ("part_number", "LAMP-22"),
            ("mech.width", "30"),
            ("mech.height", "30"),
            ("mech.mount_kind", "door_cutout"),
            ("mech.front_shape", "round"),
        ],
    );
    done(&mut s, "parts.row", json!({"table": "mech.holes"}));
    fields(
        &mut s,
        &[
            ("mech.holes[0].x", "15"),
            ("mech.holes[0].y", "15"),
            ("mech.holes[0].hole.kind", "cutout"),
            ("mech.holes[0].hole.size", "22"),
        ],
    );
    done(&mut s, "parts.save", json!({}));
    let (holes, lines) = front_of(&mut s);
    assert_eq!(holes, [("cutout".to_owned(), "22 mm".to_owned())]);
    assert_eq!(lines, 0);
    // A fixing and an outline, saved as the part's next version.
    done(&mut s, "parts.row", json!({"table": "mech.holes"}));
    fields(
        &mut s,
        &[
            ("mech.holes[1].x", "5"),
            ("mech.holes[1].y", "15"),
            ("mech.holes[1].hole.kind", "fixing"),
            ("mech.holes[1].hole.size", "M3"),
        ],
    );
    let dxf = tmp.0.join("front.dxf");
    std::fs::write(&dxf, SQUARE_DXF).unwrap();
    done(&mut s, "parts.outline", json!({"path": dxf.to_string_lossy()}));
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "preview.apply", json!({}));
    let (holes, lines) = front_of(&mut s);
    assert_eq!(
        holes,
        [
            ("cutout".to_owned(), "22 mm".to_owned()),
            ("fixing".to_owned(), "M3".to_owned())
        ]
    );
    assert_eq!(lines, 1);
    // Remove: the front is drawn by its shape again.
    done(
        &mut s,
        "parts.field",
        json!({"field": "mech.front_2d", "value": ""}),
    );
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "preview.apply", json!({}));
    let (_, lines) = front_of(&mut s);
    assert_eq!(lines, 0);
    s.shutdown();
}
