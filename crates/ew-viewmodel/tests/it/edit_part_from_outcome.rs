// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Edit part… from an outcome (doc 07 §5.7): a finding or a refusal that comes from a part's missing datum offers Edit
//! part…, which opens Parts on that part at the field, ringed while the part lacks it. Each journey with a part the
//! person made: his own pilot lamp without holes in the door, his own cabinet without a depth.

use crate::common::{Tmp, done, form};

use ew_viewmodel::ew_api::library::LibraryTiers;
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::shell::parts_place::Origin;
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// The open part's number and the field it was opened at.
fn opened_at(s: &ShellModel) -> (String, Option<&'static str>) {
    let open = s
        .parts_place()
        .and_then(|p| p.sheet.as_ref())
        .expect("Parts on a part");
    (open.sheet.text("part_number").to_owned(), open.field)
}

/// Journey 2 of the design: his own pilot lamp, made from the starter's and without its hole, is given to the sample's
/// -102E1 in the door; the check's finding about it offers Edit part…, which opens his lamp at its holes with the strip
/// "Opened from a finding on page 904 …" and Back to Messages. A hole typed and saved, the finding is gone.
#[test]
fn a_finding_about_his_lamp_without_holes_opens_parts_at_the_holes() {
    let tmp = Tmp::new("edit-part-finding");
    let mut s = tmp.sample_in(Mode::Gui, true, "904");
    crate::common::own_of(&mut s, "EW-GEN-LMP-1", "PL-22-GN");
    done(&mut s, "parts.open", json!({"part": "PL-22-GN"}));
    done(&mut s, "parts.row", json!({"table": "mech.holes", "remove": 1}));
    save(&mut s);
    done(&mut s, "parts.back", json!({}));
    done(&mut s, "select.set", json!({"refs": ["tag:-102E1"]}));
    done(&mut s, "part.choose", json!({"use": 0}));
    done(&mut s, "part.pick", json!({"part": "PL-22-GN"}));
    done(&mut s, "messages.show", json!({"on": true}));
    let row = s
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.nodes
                .iter()
                .find(|n| n.role == NavRole::Finding && n.cells[1] == "EW-LAY-017")
                .cloned()
        })
        .expect("EW-LAY-017 about the lamp");
    assert_eq!(row.cells[5], "Edit part\u{2026}", "the row offers Edit part…");
    done(&mut s, "messages.fix", json!({"key": row.key}));
    let (number, field) = opened_at(&s);
    assert_eq!(number, "PL-22-GN");
    assert!(field.is_some_and(|f| f.starts_with("mech.holes[].")), "{field:?}");
    let place = s.parts_place().expect("Parts");
    assert!(matches!(place.origin, Origin::Finding { .. }));
    assert!(
        place
            .strip()
            .is_some_and(|t| t.starts_with("Opened from a finding on page 904")),
        "{:?}",
        place.strip()
    );
    assert_eq!(place.back_label(), "Back to Messages");
    done(&mut s, "parts.row", json!({"table": "mech.holes"}));
    for (f, v) in [
        ("mech.holes[0].x", "15"),
        ("mech.holes[0].y", "15"),
        ("mech.holes[0].hole.kind", "cutout"),
        ("mech.holes[0].hole.size", "22"),
    ] {
        done(&mut s, "parts.field", json!({"field": f, "value": v}));
    }
    save(&mut s);
    done(&mut s, "parts.back", json!({}));
    assert!(s.parts_place().is_none(), "back on the project");
    done(&mut s, "messages.check", json!({}));
    let still = s.nav(NavKind::Messages).and_then(|n| n.table()).is_some_and(|t| {
        t.nodes
            .iter()
            .any(|n| n.role == NavRole::Finding && n.cells[1] == "EW-LAY-017")
    });
    assert!(!still, "the finding is gone");
    s.shutdown();
}

/// His own cabinet made without a depth: Surface… of its bottom is refused, and the refusal's toast offers Edit
/// part…, which opens the cabinet in Parts at Depth, ringed.
#[test]
fn a_refusal_for_his_cabinet_without_a_depth_opens_parts_at_depth() {
    let tmp = Tmp::new("edit-part-refusal");
    let mine = LibraryTiers::under(&tmp.0.join("data"), &tmp.0.join("cache"), Vec::new(), Vec::new());
    let mut s = ShellModel::new(Mode::Gui, tmp.config().with_libraries(Some(mine)));
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("p.ew"), "template": "builtin"}),
    );
    done(&mut s, "parts.new", json!({"start": "enclosure"}));
    for (f, v) in [
        ("manufacturer", "Example"),
        ("part_number", "ENC-8-10"),
        ("sections.enclosure.width", "800"),
        ("sections.enclosure.height", "1000"),
    ] {
        done(&mut s, "parts.field", json!({"field": f, "value": v}));
    }
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "parts.back", json!({}));
    done(
        &mut s,
        "page.new",
        json!({"folio": "301", "type": "panel_layout", "title": "Cabinet layout"}),
    );
    done(
        &mut s,
        "layout.enclosure_new",
        json!({"new_location": "CAB1", "part": "ENC-8-10"}),
    );
    done(&mut s, "layout.surface_page", json!({}));
    let bottom = form(&s).fields[0]
        .options
        .iter()
        .map(|(v, _)| v.clone())
        .find(|v| v.ends_with("/bottom"))
        .expect("the cabinet's bottom");
    done(&mut s, "ui.dismiss", json!({}));
    let out = s.run_said(
        Some("layout.surface_page"),
        |s| s.dispatch("layout.surface_page", &json!({"surface": bottom})),
        |_| false,
    );
    assert_ne!(
        out,
        ew_viewmodel::shell::Outcome::Done,
        "a bottom without a depth"
    );
    let toast = s.toast().cloned().expect("the refusal's toast");
    assert_eq!(toast.actions(), ["edit_part"]);
    assert_eq!(toast.label("edit_part"), "Edit part\u{2026}");
    assert!(s.banner().is_none(), "said once, in the toast");
    done(
        &mut s,
        "notifications.run",
        json!({"entry": toast.serial, "action": "edit_part"}),
    );
    assert_eq!(
        opened_at(&s),
        ("ENC-8-10".to_owned(), Some("sections.enclosure.depth"))
    );
    s.shutdown();
}

/// Save of the open part, with its card applied where the project's devices use the part.
fn save(s: &mut ShellModel) {
    done(s, "parts.save", json!({}));
    if s.properties().card().is_some() {
        done(s, "preview.apply", json!({}));
    }
}
