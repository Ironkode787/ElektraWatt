// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Output ▸ Workshop ▸ Package… on a copy of the sample (doc 13-package §8.5): in a script `package.export` writes the
//! package and its toast names it; in the window the dialog lists what goes in and what stood in the release check,
//! a way that no longer holds first, whose fix closes the dialog and opens Route's card; a member without content has
//! no tick, and a file that exists is asked about first. A line about a part's missing field offers Edit part… at it.

use crate::common;

use common::{Tmp, done, refused};
use ew_viewmodel::shell::Dialog;
use ew_viewmodel::shell::Mode;
use ew_viewmodel::shell::package::PackageDialog;
use serde_json::json;

fn package(s: &ew_viewmodel::shell::ShellModel) -> &PackageDialog {
    match s.dialog() {
        Some(Dialog::Package(d)) => d,
        other => panic!("no Package… dialog: {other:?}"),
    }
}

/// In a script `package.export` writes the draft package to its path, whole, and the toast names its file.
#[test]
fn a_script_writes_the_package_and_the_toast_names_it() {
    let tmp = Tmp::new("package-script");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    let out = tmp.0.join("workshop.zip");
    done(
        &mut s,
        "package.export",
        json!({ "path": out.to_string_lossy(), "leave_out": ["drilling"] }),
    );
    let bytes = std::fs::read(&out).expect("the package");
    assert!(bytes.starts_with(b"PK"));
    let toast = s.toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(
        toast.starts_with("Wrote the draft package workshop.zip: "),
        "{toast}"
    );
    // A file of that name is not replaced without `replace`.
    assert_eq!(
        refused(&mut s, "package.export", json!({ "path": out.to_string_lossy() })),
        "reason-file-exists"
    );
    s.shutdown();
}

/// The dialog on a copy whose -X1031 turned half round after its wires were laid: what goes in, each row ticked; the
/// check names the way that no longer holds first, as an error; its fix closes the dialog and opens Route's card; the
/// draft's revision line; a file that exists is asked about before it is replaced.
#[test]
fn the_dialog_lists_a_stale_way_first_and_its_fix_opens_route() {
    let tmp = Tmp::new("package-dialog");
    let mut s = tmp.sample_in(Mode::Gui, true, "902");
    let (session, _) =
        ew_viewmodel::ew_api::Session::open_read_only(&tmp.sample_copy(), "test").expect("the copy");
    let x1031 = session
        .resolve_reference("tag:-X1031")
        .expect("-X1031")
        .to_string();
    drop(session);
    // -X1031's placement record as the copy holds it: its ID and its place along the rail.
    let line = std::fs::read_dir(tmp.sample_copy().join("layout"))
        .expect("the layout")
        .flat_map(|f| {
            let text = std::fs::read_to_string(f.expect("a file").path()).expect("a record file");
            text.lines().map(str::to_owned).collect::<Vec<_>>()
        })
        .find(|l| l.contains("\"$k\":\"placement\"") && l.contains(&format!("\"device\":\"{x1031}\"")))
        .expect("-X1031 placed");
    let record: serde_json::Value = serde_json::from_str(&line).expect("a record");
    let placement = record["id"].as_str().expect("its ID").to_owned();
    let x = record["pose"]["t"][0].as_i64().expect("its place");
    done(
        &mut s,
        "command.run",
        json!({"cmd": "layout.move", "params": {
            "placement": placement,
            "pose": {"t": [x, 0, 0], "q": [0, 0, 0, 1_000_000_000]},
            "mode": "free",
        }}),
    );
    if s.properties().card().is_some() {
        done(&mut s, "preview.apply", json!({}));
    }
    done(&mut s, "package.export", json!({}));
    let d = package(&s);
    assert!(d.draft);
    // The field wires without a length, which no finding names, in a line with their count.
    assert!(
        d.lengths
            .iter()
            .any(|l| l.ends_with("wires have no length: one end of each is made on site.")),
        "{:?}",
        d.lengths
    );
    assert!(d.rows.iter().filter(|r| r.content).all(|r| r.on));
    let first = d.findings.first().expect("a finding");
    assert_eq!(first.rule, "EW-LAY-011");
    assert!(
        !d.clean && d.refused.is_none(),
        "a draft is written whatever stood"
    );
    assert!(d.revision.0.contains("V1.0"), "{:?}", d.revision);
    // Show lists the finding's records under it.
    let key = first.key.clone();
    done(&mut s, "package.show_finding", json!({ "key": key }));
    assert!(package(&s).findings[0].shown);
    // A member left out stays out; the file that exists is asked about first.
    done(
        &mut s,
        "package.tick",
        json!({ "member": "drilling", "on": false }),
    );
    assert!(package(&s).left_out().contains(&"drilling"));
    let out = tmp.0.join("exists.zip");
    std::fs::write(&out, b"old").expect("a file");
    done(&mut s, "package.file", json!({ "path": out.to_string_lossy() }));
    let params = package(&s).export_params(false);
    done(&mut s, "package.export", params);
    assert!(package(&s).asks_replace, "Replace it?");
    assert_eq!(std::fs::read(&out).expect("the file"), b"old");
    // The fix: the dialog closes and Route's card opens.
    done(&mut s, "package.fix", json!({ "key": key }));
    assert!(s.dialog().is_none());
    let card = s.properties().card().expect("Route's card");
    assert_eq!(card.preview.name, "layout.route");
    s.shutdown();
}

/// A finding that names one part's missing field offers Edit part… in the dialog, by the one path Messages takes: his
/// own pilot lamp, made here without its hole and given to -102E1 in the door, makes EW-LAY-017; its button opens his
/// lamp in Parts at the holes, and the dialog closes.
#[test]
fn a_finding_about_his_own_part_offers_edit_part_at_the_field() {
    let tmp = Tmp::new("package-edit-part");
    let mut s = tmp.sample_in(Mode::Gui, true, "904");
    common::own_of(&mut s, "EW-GEN-LMP-1", "PL-22-GN");
    done(&mut s, "parts.open", json!({"part": "PL-22-GN"}));
    done(&mut s, "parts.row", json!({"table": "mech.holes", "remove": 1}));
    done(&mut s, "parts.save", json!({}));
    if s.properties().card().is_some() {
        done(&mut s, "preview.apply", json!({}));
    }
    done(&mut s, "parts.back", json!({}));
    done(&mut s, "select.set", json!({"refs": ["tag:-102E1"]}));
    done(&mut s, "part.choose", json!({"use": 0}));
    done(&mut s, "part.pick", json!({"part": "PL-22-GN"}));
    done(&mut s, "package.export", json!({}));
    let finding = package(&s)
        .findings
        .iter()
        .find(|f| f.rule == "EW-LAY-017")
        .cloned()
        .expect("EW-LAY-017 about his lamp");
    assert_eq!(finding.fix.as_deref(), Some("Edit part\u{2026}"));
    done(&mut s, "package.fix", json!({ "key": finding.key }));
    assert!(s.dialog().is_none(), "the dialog closes");
    let open = s
        .parts_place()
        .and_then(|p| p.sheet.as_ref())
        .expect("Parts on his lamp");
    assert_eq!(open.sheet.text("part_number"), "PL-22-GN");
    assert!(
        open.field.is_some_and(|f| f.starts_with("mech.holes[].")),
        "{:?}",
        open.field
    );
    s.shutdown();
}

/// The cable markers' line says the stock has no marker that marks cables and offers Workshop stock…, which opens Parts
/// on the stock in the dialog's place.
#[test]
fn a_missing_marker_part_offers_the_workshop_stock() {
    let tmp = Tmp::new("package-stock");
    let mut s = tmp.sample_in(Mode::Gui, true, "907");
    done(&mut s, "package.export", json!({}));
    let lacks = s
        .describe()
        .lines()
        .map(str::trim_start)
        .find(|l| l.starts_with("lacks ") && l.contains("Cable markers"))
        .map(str::to_owned)
        .expect("a lack");
    assert!(
        lacks.contains("Cable markers") && lacks.ends_with("offers parts.stock"),
        "{lacks}"
    );
    done(&mut s, "parts.stock", json!({}));
    assert!(s.dialog().is_none(), "{:?}", s.dialog());
    let place = s.parts_place().expect("Parts");
    assert_eq!(place.scope, ew_viewmodel::ew_api::parts::PartScope::Stock);
    assert!(!place.found.is_empty(), "the sample's stock");
    s.shutdown();
}

/// A 0.75 mm² ferrule he makes in Parts without its sleeve length, first among the stock's ferrules: the wire list's line
/// about it offers Edit part…, which closes the dialog and opens Parts on his ferrule at its sleeve length; a line about
/// the stock keeps Workshop stock…. In the Dutch window the line and its offer are Dutch.
#[test]
fn a_part_without_a_field_the_package_reads_offers_edit_part_at_the_field() {
    let tmp = Tmp::new("package-lack-part");
    let mut s = tmp.sample_in(Mode::Gui, true, "907");
    done(&mut s, "parts.open", json!({}));
    done(&mut s, "parts.new", json!({"start": "ferrule"}));
    for (field, value) in [
        ("manufacturer", "Example"),
        ("part_number", "MY-FER-075"),
        ("sections.ferrule.csa_min", "0.75"),
        ("sections.ferrule.csa_max", "0.75"),
    ] {
        done(&mut s, "parts.field", json!({"field": field, "value": value}));
    }
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "parts.stock", json!({"op": "add"}));
    done(&mut s, "parts.stock", json!({"op": "move", "to": 1}));
    done(&mut s, "parts.back", json!({}));
    let line = |s: &ew_viewmodel::shell::ShellModel| {
        package(s)
            .rows
            .iter()
            .flat_map(|r| &r.lacks)
            .find(|l| l.text.starts_with("MY-FER-075: "))
            .cloned()
            .expect("the line about his ferrule")
    };
    done(&mut s, "package.export", json!({}));
    let lack = line(&s);
    assert_eq!(
        (lack.text.as_str(), lack.offer.as_str()),
        (
            "MY-FER-075: Without a sleeve length the wire list prints no strip length.",
            "Edit part\u{2026}"
        )
    );
    assert_eq!(lack.action.0, "parts.open");
    assert_eq!(lack.action.1["field"], "sections.ferrule.sleeve");
    let stock: Vec<&str> = package(&s)
        .rows
        .iter()
        .flat_map(|r| &r.lacks)
        .filter(|l| l.action.0 == "parts.stock")
        .map(|l| l.offer.as_str())
        .collect();
    assert!(
        !stock.is_empty() && stock.iter().all(|o| *o == "Workshop stock\u{2026}"),
        "{stock:?}"
    );
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "view.language", json!({ "value": "nl" }));
    done(&mut s, "package.export", json!({}));
    let nl = line(&s);
    assert_eq!(
        (nl.text.as_str(), nl.offer.as_str()),
        (
            "MY-FER-075: Zonder hulslengte drukt de draadlijst geen striplengte af.",
            "Artikel bewerken\u{2026}"
        )
    );
    done(&mut s, nl.action.0, nl.action.1.clone());
    assert!(s.dialog().is_none(), "the dialog closes: {:?}", s.dialog());
    let open = s
        .parts_place()
        .and_then(|p| p.sheet.as_ref())
        .expect("Parts on his ferrule");
    assert_eq!(open.sheet.text("part_number"), "MY-FER-075");
    assert_eq!(open.field, Some("sections.ferrule.sleeve"));
    s.shutdown();
}

/// A fix of one of the dialog's findings (Mark not used on an open connection point of a pushbutton placed here) shows
/// its card in front of the dialog; once applied, the dialog is back, read again: that finding has nothing left to
/// mend (it reads "marked not used"), a member left out stays out.
#[test]
fn the_dialog_comes_back_after_a_fix_without_the_finding_it_mended() {
    let tmp = Tmp::new("package-fix-back");
    let mut s = tmp.sample_in(Mode::Gui, true, "102");
    common::place_free_button(&mut s);
    done(&mut s, "package.export", json!({}));
    done(
        &mut s,
        "package.tick",
        json!({ "member": "drilling", "on": false }),
    );
    let finding = package(&s)
        .findings
        .iter()
        .find(|f| f.fix.as_deref() == Some("Mark not used"))
        .cloned()
        .expect("an open connection point with Mark not used");
    done(&mut s, "package.fix", json!({ "key": finding.key }));
    assert!(s.properties().card().is_some(), "the fix's card");
    assert!(s.dialog().is_none(), "the card stands in front");
    done(&mut s, "preview.apply", json!({}));
    assert!(
        matches!(s.dialog(), Some(Dialog::Package(_))),
        "the dialog is back"
    );
    // What the fix mended reads as done: no open connection point of it left to mark.
    assert!(
        !package(&s)
            .findings
            .iter()
            .any(|f| f.key == finding.key && f.fix.is_some()),
        "{:?}",
        package(&s)
            .findings
            .iter()
            .map(|f| f.message.clone())
            .collect::<Vec<_>>()
    );
    assert!(package(&s).left_out().contains(&"drilling"));
    s.shutdown();
}

/// The dialog a fix left for its card belongs to its project: another project opened while the card stands (or the
/// same one opened again) never brings it back, with the old project's path and choices.
#[test]
fn the_dialog_left_for_a_fix_stays_with_its_project() {
    let tmp = Tmp::new("package-fix-other");
    let mut s = tmp.sample_in(Mode::Gui, true, "102");
    common::place_free_button(&mut s);
    done(&mut s, "package.export", json!({}));
    let finding = package(&s)
        .findings
        .iter()
        .find(|f| f.fix.as_deref() == Some("Mark not used"))
        .cloned()
        .expect("an open connection point with Mark not used");
    done(&mut s, "package.fix", json!({ "key": finding.key }));
    assert!(s.properties().card().is_some(), "the fix's card");
    let dir = tmp.sample_copy();
    done(
        &mut s,
        "file.open",
        json!({ "dir": dir.to_string_lossy(), "discard": true }),
    );
    done(&mut s, "page.goto", json!({ "folio": "101" }));
    assert!(
        !matches!(s.dialog(), Some(Dialog::Package(_))),
        "{:?}",
        s.dialog()
    );
    s.shutdown();
}
