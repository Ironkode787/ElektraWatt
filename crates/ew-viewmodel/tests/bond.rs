// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Draw ▸ Bond (`bond.draw`, G-PP triage row 7) on copies of Golden A with the bond B1 removed: drawn again on the
//! 0 V–PE wire W27 it is previewed on a card, Apply commits one `bond.create` (one undo step), 0 V keeps its colour and
//! PE stays PE, and EW-POT-001 and EW-POT-002 no longer report; the CP form `a`/`b`, the kinds, the refusals and the
//! Bond submenu.

mod common;

use common::{Tmp, done, refused};
use ew_viewmodel::actions::{Menu, Submenu, menu_model};
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::{Value, json};

/// The 0 V–PE wire W27 on folio 103, as the page text names it.
const W27: &str = "wire:\"PE-0VDC\" -> \"-X1032:2/a\"";

fn failed(shell: &mut ShellModel, action: &str, params: Value) -> String {
    match shell.dispatch(action, &params) {
        Outcome::Failed { message } => message,
        other => panic!("{action} {params}: {other:?}"),
    }
}

/// The open findings of rule `rule` after Run checks.
fn findings(shell: &mut ShellModel, rule: &str) -> usize {
    done(shell, "messages.show", json!({}));
    done(shell, "messages.check", json!({}));
    let t = shell
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .cloned()
        .expect("the Messages table");
    t.nodes
        .iter()
        .filter(|n| n.role == NavRole::Finding && n.cells[1] == rule)
        .count()
}

/// The Potentials rows: (label, kind, colour cell).
fn potentials(shell: &mut ShellModel) -> Vec<(String, String, String)> {
    done(shell, "navigator.show", json!({"kind": "potentials"}));
    let n = shell.nav(NavKind::Potentials).expect("the Potentials navigator");
    let t = n.table().expect("its table");
    let colour = t.columns.iter().position(|c| c.key == "colour").unwrap();
    t.nodes
        .iter()
        .filter(|n| n.role == NavRole::Potential)
        .map(|n| {
            (
                n.label.clone(),
                n.kinds.first().cloned().unwrap_or_default(),
                n.cells[colour].clone(),
            )
        })
        .collect()
}

/// A writable copy of Golden A on folio 103 with the bond B1 deleted (one committed step).
fn without_b1(tmp: &Tmp) -> ShellModel {
    let mut shell = tmp.shell_in(Mode::Headless, true, "103");
    done(&mut shell, "navigator.show", json!({"kind": "potentials"}));
    let bond = {
        let t = shell.nav(NavKind::Potentials).and_then(|n| n.table()).unwrap();
        let rows: Vec<_> = t.nodes.iter().filter(|n| n.kinds == ["bond"]).collect();
        assert_eq!(rows.len(), 1, "Golden A has the one bond B1");
        rows[0].records[0]
    };
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "edit.delete", "params": {"targets": [{"kind": "bond", "id": bond.to_string()}]}}),
    );
    shell
}

/// D-22's situation: without B1 the wire W27 joins 0 V and PE into one potential, EW-POT-001 reports, and the merged
/// potential takes the first of its two definitions in natural order of name, 0V (WP-S1.16 D9, FT-587);
/// Draw ▸ Bond on W27 previews `bond.create` (pelv_0v_pe, via W27), Apply commits it in one step, EW-POT-001 and
/// EW-POT-002 are gone, 0 V is WH and PE stays PE; Undo brings the merge back.
#[test]
fn a_bond_drawn_on_w27_keeps_0v_and_pe_apart() {
    let tmp = Tmp::new("bond-w27");
    let mut shell = without_b1(&tmp);
    assert!(findings(&mut shell, "EW-POT-001") >= 1, "the merge reports");
    let merged = potentials(&mut shell);
    assert!(
        merged.iter().any(|(l, _, _)| l == "0V") && !merged.iter().any(|(l, _, _)| l == "PE"),
        "0 V and PE are one potential, named 0V (D9): {merged:?}"
    );

    done(&mut shell, "select.set", json!({"refs": [W27]}));
    let rev = shell.revision();
    let steps = shell.recorded_commands().len();
    done(&mut shell, "bond.draw", json!({}));
    assert_eq!(shell.revision(), rev, "the bond is previewed, not committed");
    let card = shell.properties().card().expect("the bond's card");
    assert_eq!(card.preview.name, "bond.create");
    assert_eq!(card.reason, "reason-bond");
    assert!(card.preview.result.is_ok(), "{:?}", card.preview.result);
    assert!(
        shell
            .describe()
            .lines()
            .any(|l| l.starts_with("card ready \"bond.create\"")),
        "{}",
        shell.describe()
    );

    done(&mut shell, "preview.apply", json!({}));
    assert_eq!(shell.revision(), rev.map(|r| r + 1), "one undo step");
    assert_eq!(shell.recorded_commands().len(), steps + 1);
    let last = shell.recorded_commands().last().unwrap();
    assert_eq!(last.cmd, "bond.create");
    assert_eq!(last.params["kind"], "pelv_0v_pe");
    assert!(
        last.params["via"].is_string(),
        "W27 is the bond's via: {}",
        last.params
    );

    assert_eq!(findings(&mut shell, "EW-POT-001"), 0, "no merge of 0 V into PE");
    assert_eq!(
        findings(&mut shell, "EW-POT-002"),
        0,
        "0 V has its one 0 V–PE bond"
    );
    let rows = potentials(&mut shell);
    let row = |name: &str| {
        rows.iter()
            .find(|(l, _, _)| l == name)
            .unwrap_or_else(|| panic!("no potential {name}: {rows:?}"))
            .clone()
    };
    assert_eq!(row("0V").2, "WH", "0 V keeps its colour");
    assert_eq!(row("0V").1, "potential_def");
    assert!(row("PE").2.contains("GN"), "PE stays PE: {:?}", row("PE"));
    assert!(
        rows.iter()
            .any(|(l, k, _)| k == "bond" && l.contains("0V") && l.contains("PE")),
        "the bond row names 0V and PE: {rows:?}"
    );

    done(&mut shell, "edit.undo", json!({}));
    assert!(
        !potentials(&mut shell).iter().any(|(_, k, _)| k == "bond"),
        "Undo takes the bond back in one step"
    );
    assert!(findings(&mut shell, "EW-POT-001") >= 1);
    shell.shutdown();
}

/// The CP form: `a` and `b` name two CPs as the page text prints them; the wire between them is the `via`. A kind
/// other than the default is taken; a CP that the page does not print, two equal ends, a missing end and an unknown
/// kind fail; without a selection and without CPs the action refuses.
#[test]
fn the_cp_form_the_kinds_and_the_refusals() {
    let tmp = Tmp::new("bond-cps");
    let mut shell = without_b1(&tmp);
    assert_eq!(
        refused(&mut shell, "bond.draw", json!({})),
        "reason-bond-selection"
    );
    let m = failed(&mut shell, "bond.draw", json!({"a": "PE-0VDC", "b": "-X9:9"}));
    assert!(m.contains("-X9:9"), "{m}");
    let m = failed(&mut shell, "bond.draw", json!({"a": "PE-0VDC", "b": "PE-0VDC"}));
    assert!(m.contains("two different"), "{m}");
    failed(&mut shell, "bond.draw", json!({"a": "PE-0VDC"}));
    // Without a selection and without ends the action is disabled before its parameters are read (FT-578).
    assert_eq!(
        refused(&mut shell, "bond.draw", json!({"kind": "copper"})),
        "reason-bond-selection"
    );
    done(&mut shell, "select.set", json!({"refs": [W27]}));
    failed(&mut shell, "bond.draw", json!({"kind": "copper"}));
    done(&mut shell, "ui.dismiss", json!({}));
    assert!(shell.properties().card().is_none());

    done(
        &mut shell,
        "bond.draw",
        json!({"a": "PE-0VDC", "b": "\"-X1032:2/a\"", "kind": "equipotential"}),
    );
    let card = shell.properties().card().expect("the card");
    assert_eq!(card.preview.command["kind"], "equipotential");
    assert!(
        card.preview.command["via"].is_string(),
        "{}",
        card.preview.command
    );
    assert!(card.preview.command["a"]["cp"].is_object());
    done(&mut shell, "preview.cancel", json!({}));
    shell.shutdown();
}

/// Draw ▸ Connect ▸ Bond lists the four kinds of `bond.create`, the default first, each dispatching `bond.draw` with
/// its kind; they are disabled with their reason until the selection gives the bond two ends (FT-578).
#[test]
fn the_bond_submenu_lists_the_four_kinds() {
    let tmp = Tmp::new("bond-menu");
    let mut shell = tmp.shell_in(Mode::Headless, true, "103");
    let bond_items = |shell: &ShellModel| {
        let draw = menu_model(shell)
            .into_iter()
            .find(|m| m.menu == Menu::Draw)
            .expect("the Draw menu");
        draw.submenu(Submenu::Bond).expect("the Bond submenu").to_vec()
    };
    let items = bond_items(&shell);
    let reason = ew_viewmodel::strings::text("reason-bond-selection");
    assert!(
        items
            .iter()
            .all(|i| i.id == "bond.draw" && !i.enabled && i.reason.as_deref() == Some(reason)),
        "{items:?}"
    );
    done(&mut shell, "select.set", json!({"refs": [W27]}));
    let items = bond_items(&shell);
    assert!(items.iter().all(|i| i.id == "bond.draw" && i.enabled));
    let kinds: Vec<&str> = items.iter().map(|i| i.params["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["pelv_0v_pe", "fe_pe", "equipotential", "shield"]);
    assert_eq!(items[0].label, "0 V–PE bond (PELV)");
}
