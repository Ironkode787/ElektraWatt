// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Output ▸ Workshop ▸ Package… on a copy of the sample (doc 13-package §8.5): in a script `package.export` writes the
//! package and its toast names it; in the window the dialog lists what goes in and what stood in the release check,
//! a way that no longer holds first, whose fix closes the dialog and opens Route's card; a member without content has
//! no tick, and a file that exists is asked about first.

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
