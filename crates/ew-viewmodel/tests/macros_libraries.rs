// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.19 part 6 (§11): the Macros and Libraries navigators in the shell on copies of Golden A with a fixture user
//! library. UI-1 (the detail of the PSU macro), UI-2's model half (insert the PSU macro on folio 105 from the
//! navigator: the card lists the conflicts, Apply is one undo step; the whole block with its interruption points lists
//! the pairings and is refused by EW-CORE-052), UI-3 (Save as macro… writes into a user library, refuses a system
//! library and refuses headless), UI-4's Update… (the bulk card of `library.update`), Make variable, and FT-720(b)
//! (`navigator.show` shows the pane asked for).

mod common;

use std::path::Path;

use common::{Tmp, done, refused};
use ew_viewmodel::ew_api::library::{LibraryTiers, Registry};
use ew_viewmodel::ew_api::output::Uuid;
use ew_viewmodel::layout::PaneKind;
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::properties::CardOrigin;
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellConfig, ShellModel};
use ew_viewmodel::strings;
use serde_json::json;

/// The tiers of a fixture under `root`: no system path (the compiled-in libraries), a user directory.
fn tiers(root: &Path) -> LibraryTiers {
    LibraryTiers {
        system: Vec::new(),
        community: root.join("data").join("packs"),
        company: Vec::new(),
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    }
}

/// A fresh user library "Mine" under `root`: the tiers and its ID.
fn user_library(root: &Path) -> (LibraryTiers, Uuid) {
    let t = tiers(root);
    let mut reg = Registry::open(&t).unwrap();
    let lib = reg
        .create(&t.user.join("mine.ewlib"), "mine", "Mine", "CC0-1.0")
        .unwrap();
    (t, lib)
}

/// A shell in `mode` with the tiers `t` and a writable copy of Golden A (`copy` names the copy's directory) on
/// `folio`.
fn shell_with(tmp: &Tmp, copy: &str, mode: Mode, t: &LibraryTiers, folio: &str) -> ShellModel {
    let dir = tmp.0.join(copy);
    if !dir.exists() {
        common::copy_dir(Path::new(common::GOLDEN_A), &dir);
    }
    let config = ShellConfig::in_dir(&tmp.0.join(format!("config-{copy}")), "0.0.0-test")
        .with_libraries(Some(t.clone()));
    let mut shell = ShellModel::new(mode, config);
    done(
        &mut shell,
        "file.open",
        json!({"dir": dir.to_string_lossy(), "read_only": false}),
    );
    done(&mut shell, "page.goto", json!({ "folio": folio }));
    shell
}

/// The IDs of folio 102's PSU block: with `ips`, every representation, connection and junction; without, the
/// interruption points left out (the fixture of `ew-api`'s macro tests).
fn psu_ids(shell: &ShellModel, ips: bool) -> Vec<String> {
    let tables = &shell.canvas().expect("a page").page().tables;
    tables
        .reps
        .iter()
        .filter(|r| ips || r.device.is_some())
        .map(|r| r.id)
        .chain(tables.connections.iter().map(|c| c.id))
        .chain(tables.junctions.iter().map(|j| j.id))
        .map(|id| id.to_string())
        .collect()
}

/// Saves folio 102's PSU block (`ips`: with its interruption points) as the macro `name` into "Mine" through Save
/// as macro… in the window.
fn save_psu(shell: &mut ShellModel, name: &str, ips: bool) {
    done(shell, "page.goto", json!({"folio": "102"}));
    let ids = psu_ids(shell, ips);
    done(shell, "select.set", json!({ "ids": ids }));
    done(
        shell,
        "macros.save",
        json!({"name": name, "library": "Mine", "description": "Power supply with two output breakers", "confirm": true}),
    );
}

/// The key of the table row of navigator `kind` with role `role` labelled `label`.
fn row_key(shell: &ShellModel, kind: NavKind, role: NavRole, label: &str) -> String {
    let t = shell
        .nav(kind)
        .and_then(|n| n.table())
        .expect("an open navigator");
    t.nodes
        .iter()
        .find(|n| n.role == role && n.label == label)
        .map(|n| n.key.clone())
        .unwrap_or_else(|| panic!("no {role:?} row {label}"))
}

/// A new folio 105 after 104, shown.
fn add_folio_105(shell: &mut ShellModel) {
    done(shell, "page.goto", json!({"folio": "102"}));
    done(
        shell,
        "page.new",
        json!({"folio": "105", "type": "multi_line", "title": "Power Supply 5"}),
    );
    done(shell, "page.goto", json!({"folio": "105"}));
}

/// The number of representations of the page on show.
fn reps_on_show(shell: &ShellModel) -> usize {
    shell.canvas().map_or(0, |c| c.page().tables.reps.len())
}

/// UI-1: the Macros navigator lists the saved PSU macro under "User" and its detail shows its name, version,
/// library, no thumbnail ("No preview" in the window) and its three devices.
#[test]
fn ui1_the_macros_navigator_shows_the_psu_macro_and_its_detail() {
    let tmp = Tmp::new("macros-ui1");
    let (t, _) = user_library(&tmp.0);
    let mut shell = shell_with(&tmp, "a", Mode::Gui, &t, "102");
    save_psu(&mut shell, "PSU 24 V DC", false);
    done(&mut shell, "navigator.show", json!({"kind": "macros"}));
    let key = row_key(&shell, NavKind::Macros, NavRole::Macro, "PSU 24 V DC");
    row_key(&shell, NavKind::Macros, NavRole::Tier, "User");
    done(&mut shell, "navigator.row", json!({"kind": "macros", "key": key}));
    let d = shell.macro_detail().expect("the detail of the cursor row");
    assert_eq!(d.name, "PSU 24 V DC");
    assert_eq!(d.version, "1.0.0");
    assert_eq!(d.library, "Mine");
    assert!(d.thumbnail.is_none());
    assert_eq!(d.devices.len(), 3, "{:?}", d.devices);
    assert!(d.devices.iter().any(|x| x.starts_with("-G")), "{:?}", d.devices);
    // The describe text names the navigator and its rows' roles in words (`LIBRARY_WORDS`).
    let text = ew_viewmodel::describe::describe(&shell);
    for word in ["macros", "tier", "macro"] {
        assert!(
            text.split_whitespace()
                .any(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '_') == word),
            "{word}"
        );
    }
    done(&mut shell, "navigator.show", json!({"kind": "libraries"}));
    let text = ew_viewmodel::describe::describe(&shell);
    for word in ["libraries", "library", "project_copies", "library_copy"] {
        assert!(
            text.split_whitespace()
                .any(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '_') == word),
            "{word}"
        );
    }
}

/// UI-2 (model half): Enter on the macro row arms the macro tool; its click on folio 105 opens the `macro` card
/// with no conflicts; Apply is one undo step. The whole block with its interruption points lists the pairings of
/// L-1 and N-1 and is refused by EW-CORE-052 (Apply fails with the rule's text and the card stays).
#[test]
fn ui2_insert_the_psu_macro_on_folio_105_from_the_navigator() {
    let tmp = Tmp::new("macros-ui2");
    let (t, _) = user_library(&tmp.0);
    let mut shell = shell_with(&tmp, "a", Mode::Gui, &t, "102");
    save_psu(&mut shell, "PSU 24 V DC", false);
    save_psu(&mut shell, "PSU block", true);
    add_folio_105(&mut shell);
    done(&mut shell, "navigator.show", json!({"kind": "macros"}));

    // The whole block: the pairings, the refusal.
    let block = row_key(&shell, NavKind::Macros, NavRole::Macro, "PSU block");
    done(
        &mut shell,
        "navigator.open",
        json!({"kind": "macros", "key": block}),
    );
    assert_eq!(shell.tool().name(), "macro");
    done(
        &mut shell,
        "macros.place",
        json!({"key": block, "x_mm": 100.0, "y_mm": 100.0}),
    );
    let card = shell.properties().card().expect("the macro card");
    let CardOrigin::Macro(m) = &card.origin else {
        panic!("a macro card: {:?}", card.origin)
    };
    assert!(card.preview.result.is_err(), "EW-CORE-052 refuses the block");
    // FX38d (window check W38): the card leads with what blocks the apply in words, not with the rule code, which
    // follows as secondary text; no conflict row repeats the lead.
    let lead = card.failed_lead().expect("the failed card's lead");
    assert!(
        !lead.text.starts_with("EW-") && !lead.text.starts_with("failed:"),
        "{lead:?}"
    );
    assert!(!lead.text.contains("EW-CORE-052"), "{lead:?}");
    assert!(lead.text.contains(&m.conflicts.len().to_string()), "{lead:?}");
    assert_eq!(lead.rule.as_deref(), Some("EW-CORE-052"));
    assert!(m.conflicts.iter().all(|c| *c != lead.text), "{:?}", m.conflicts);
    let summary = ew_viewmodel::script::card_summary(&shell).expect("the card's line");
    assert!(summary.starts_with(&lead.text), "{summary}");
    for name in ["L-1", "N-1"] {
        assert!(
            m.conflicts
                .iter()
                .any(|c| c.contains(name) && c.contains("pairs with")),
            "{name}: {:?}",
            m.conflicts
        );
    }
    assert!(matches!(
        shell.dispatch("preview.apply", &json!({})),
        Outcome::Failed { .. }
    ));
    assert!(shell.properties().card().is_some(), "the refused card stays");
    done(&mut shell, "preview.cancel", json!({}));

    // The core macro: Enter, a click, the card, Apply as one undo step.
    let core = row_key(&shell, NavKind::Macros, NavRole::Macro, "PSU 24 V DC");
    done(
        &mut shell,
        "navigator.row",
        json!({"kind": "macros", "key": core}),
    );
    done(&mut shell, "navigator.open", json!({"kind": "macros"}));
    assert_eq!(shell.tool().name(), "macro");
    let before = reps_on_show(&shell);
    done(&mut shell, "canvas.press", json!({"x_mm": 100.0, "y_mm": 100.0}));
    done(
        &mut shell,
        "canvas.release",
        json!({"x_mm": 100.0, "y_mm": 100.0}),
    );
    assert_eq!(shell.tool().name(), "select", "the tool returns to Select");
    let card = shell.properties().card().expect("the macro card");
    let CardOrigin::Macro(m) = &card.origin else {
        panic!("a macro card")
    };
    assert!(
        card.preview.result.is_ok(),
        "{:?}",
        card.preview.result.as_ref().err()
    );
    assert!(m.conflicts.is_empty(), "{:?}", m.conflicts);
    assert!(shell.layout().panes().contains(&PaneKind::Preview));
    let rev = shell.revision().unwrap();
    done(&mut shell, "preview.apply", json!({}));
    assert_eq!(shell.revision(), Some(rev + 1), "one transaction");
    assert!(reps_on_show(&shell) > before);
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(reps_on_show(&shell), before, "one undo step removes the macro");
}

/// UI-3: Save as macro… writes into a user library (the Macros navigator shows the macro at once); a system library
/// refuses; headless the action refuses (a library write is a front-end operation); without a name the window opens
/// the dialog, whose OK is the confirmation.
#[test]
fn ui3_save_as_macro_writes_a_user_library_and_refuses_a_system_one() {
    let tmp = Tmp::new("macros-ui3");
    let (t, _) = user_library(&tmp.0);
    let mut shell = shell_with(&tmp, "a", Mode::Gui, &t, "102");
    done(&mut shell, "navigator.show", json!({"kind": "macros"}));
    let ids = psu_ids(&shell, false);
    done(&mut shell, "select.set", json!({ "ids": ids }));
    // The system library refuses before anything is written.
    let system = refused(
        &mut shell,
        "macros.save",
        json!({"name": "PSU", "library": "ElektraWatt starter library", "confirm": true}),
    );
    assert_eq!(system, "reason-library-read-only");
    // FT-755: the refusal leaves its reason in the window's banner.
    assert_eq!(shell.banner(), Some(strings::text("reason-library-read-only")));
    // The dialog: prefilled with the first writable library; OK saves.
    done(&mut shell, "macros.save", json!({}));
    let Some(Dialog::Form(f)) = shell.dialog() else {
        panic!("the Save as macro dialog")
    };
    assert_eq!(f.action, "macros.save");
    let library = f.fields.iter().find(|x| x.name == "library").unwrap();
    assert_eq!(library.value, "Mine");
    done(
        &mut shell,
        "macros.save",
        json!({"name": "PSU 24 V DC", "library": "Mine"}),
    );
    assert!(shell.dialog().is_none());
    row_key(&shell, NavKind::Macros, NavRole::Macro, "PSU 24 V DC");
    // Headless: refused.
    let mut headless = shell_with(&tmp, "b", Mode::Headless, &t, "102");
    let ids = psu_ids(&headless, false);
    done(&mut headless, "select.set", json!({ "ids": ids }));
    let reason = refused(
        &mut headless,
        "macros.save",
        json!({"name": "PSU", "library": "Mine", "confirm": true}),
    );
    assert_eq!(reason, "reason-library-write-window");
}

/// FX38b: a name with no ASCII letter or digit gives no key and is refused in words before any write (the library
/// directory is as it was); a typed key still saves it.
#[test]
fn fx38b_a_name_that_gives_no_key_is_refused_before_any_write() {
    let tmp = Tmp::new("macros-nokey");
    let (t, _) = user_library(&tmp.0);
    let mut shell = shell_with(&tmp, "a", Mode::Gui, &t, "102");
    let ids = psu_ids(&shell, false);
    done(&mut shell, "select.set", json!({ "ids": ids }));
    let listing = |root: &Path| -> Vec<String> {
        let mut out: Vec<String> = walk(root)
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        out.sort();
        out
    };
    let before = listing(&t.user);
    let reason = refused(
        &mut shell,
        "macros.save",
        json!({"name": "Ωμέγα", "library": "Mine", "confirm": true}),
    );
    assert_eq!(reason, "reason-macro-no-key");
    assert_eq!(listing(&t.user), before, "nothing written");
    done(
        &mut shell,
        "macros.save",
        json!({"name": "Ωμέγα", "key": "omega", "library": "Mine", "confirm": true}),
    );
    assert_ne!(listing(&t.user), before, "a typed key saves the macro");
}

/// FX38b: Save with an empty library field and no writable library installed says so, not "no library called \"\"".
#[test]
fn fx38b_save_without_a_writable_library_names_the_reason() {
    let tmp = Tmp::new("macros-nowritable");
    let t = tiers(&tmp.0);
    let mut shell = shell_with(&tmp, "a", Mode::Gui, &t, "102");
    let ids = psu_ids(&shell, false);
    done(&mut shell, "select.set", json!({ "ids": ids }));
    let reason = refused(&mut shell, "macros.save", json!({"name": "PSU", "confirm": true}));
    assert_eq!(reason, "reason-no-writable-library");
}

/// Every file under `root`, recursively (none when it does not exist).
fn walk(root: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

/// Make variable: the process tag row of -102G1 becomes the variable `process_tag` of the next save, which the
/// detail then lists.
#[test]
fn make_variable_binds_a_property_row() {
    let tmp = Tmp::new("macros-var");
    let (t, _) = user_library(&tmp.0);
    let mut shell = shell_with(&tmp, "a", Mode::Gui, &t, "102");
    done(&mut shell, "select.set", json!({"refs": ["tag:-102G1"]}));
    done(&mut shell, "props.show", json!({"on": true}));
    done(
        &mut shell,
        "props.make_variable",
        json!({"row": "device/process_tag", "name": "pt"}),
    );
    assert_eq!(shell.macro_variables().len(), 1);
    assert_eq!(shell.macro_variables()[0].field, "device.process_tag");
    assert_eq!(shell.macro_variables()[0].ty, "process_tag");
    save_psu(&mut shell, "PSU with tag", false);
    assert!(shell.macro_variables().is_empty(), "a save takes the variables");
    done(&mut shell, "navigator.show", json!({"kind": "macros"}));
    let key = row_key(&shell, NavKind::Macros, NavRole::Macro, "PSU with tag");
    done(&mut shell, "navigator.row", json!({"kind": "macros", "key": key}));
    let d = shell.macro_detail().unwrap();
    assert!(
        d.variables.iter().any(|v| v.starts_with("pt")),
        "{:?}",
        d.variables
    );
}

/// UI-4 (Update…): with the frames' and title block's copies without a source, Update… of every copy refuses (no
/// update); the libraries stand in words, the badge of a copy without a source is "Source missing".
#[test]
fn ui4_update_refuses_without_an_update_and_badges_are_words() {
    let tmp = Tmp::new("macros-ui4");
    let (t, _) = user_library(&tmp.0);
    let mut shell = shell_with(&tmp, "a", Mode::Gui, &t, "102");
    done(&mut shell, "navigator.show", json!({"kind": "libraries"}));
    let rows = shell.shown_rows(NavKind::Libraries);
    assert!(rows.iter().any(|r| r.label == "Mine"), "the user library");
    assert_eq!(
        refused(&mut shell, "libraries.update", json!({})),
        "reason-no-updates"
    );
    let key = row_key(&shell, NavKind::Libraries, NavRole::LibCopy, "Title block QET A4");
    done(
        &mut shell,
        "navigator.row",
        json!({"kind": "libraries", "key": key}),
    );
    // FT-755: the cursor row's detail, in words, the key secondary.
    let detail = shell.library_copy_detail().expect("the copy's detail");
    assert_eq!(detail.name, "Title block QET A4");
    assert_eq!(detail.key, "ew.titleblock.qet-a4");
    assert_eq!(detail.state, "Source missing");
    let row = shell
        .shown_rows(NavKind::Libraries)
        .into_iter()
        .find(|r| r.key == key)
        .expect("the title block's row on show");
    assert_eq!(row.badges, ["Source missing"]);
    assert_eq!(
        shell.dispatch("libraries.update", &json!({"keys": [key]})),
        Outcome::Refused {
            reason_key: "reason-no-updates"
        }
    );
}

/// FT-720(b): `navigator.show` of a navigator already docked behind another tab shows it.
#[test]
fn ft720b_navigator_show_shows_the_pane_asked_for() {
    let tmp = Tmp::new("macros-ft720b");
    let mut shell = tmp.shell_in(Mode::Gui, false, "102");
    done(&mut shell, "navigator.show", json!({"kind": "devices"}));
    done(&mut shell, "navigator.show", json!({"kind": "connections"}));
    assert_eq!(
        shell.layout().shown_in_place_of(PaneKind::Pages),
        Some(PaneKind::Navigator(NavKind::Connections))
    );
    done(&mut shell, "navigator.show", json!({"kind": "devices"}));
    assert_eq!(
        shell.layout().shown_in_place_of(PaneKind::Pages),
        Some(PaneKind::Navigator(NavKind::Devices))
    );
}

/// FT-754 (f): the company libraries of the settings key `libraries.company` are the company tier: a library in a
/// directory the settings name stands under "Company", writable, in the Libraries navigator, and Save as macro…
/// writes into it. A relative path is read against the settings' directory.
#[test]
fn ft754f_company_libraries_of_the_settings_are_the_company_tier() {
    let tmp = Tmp::new("macros-company");
    let (t, _) = user_library(&tmp.0);
    let company = tmp.0.join("config-a").join("shared");
    let mut reg = Registry::open(&t).unwrap();
    reg.create(&company.join("plant.ewlib"), "plant", "Plant standard", "CC0-1.0")
        .unwrap();
    drop(reg);
    std::fs::write(
        tmp.0.join("config-a").join("settings.toml"),
        "version = 1\n\n[libraries]\ncompany = [\"shared\"]\n",
    )
    .unwrap();
    let mut shell = shell_with(&tmp, "a", Mode::Gui, &t, "102");
    done(&mut shell, "navigator.show", json!({"kind": "libraries"}));
    let tier = row_key(&shell, NavKind::Libraries, NavRole::Tier, "Company");
    let lib = row_key(&shell, NavKind::Libraries, NavRole::Library, "Plant standard");
    assert!(lib.starts_with(&format!("{tier}/")), "{lib} under {tier}");
    let ids = psu_ids(&shell, false);
    done(&mut shell, "select.set", json!({ "ids": ids }));
    done(
        &mut shell,
        "macros.save",
        json!({"name": "PSU plant", "library": "Plant standard", "confirm": true}),
    );
    done(&mut shell, "navigator.show", json!({"kind": "macros"}));
    let m = row_key(&shell, NavKind::Macros, NavRole::Macro, "PSU plant");
    assert!(m.starts_with("tier:company/"), "{m}");
}

/// FT-754 (h): Make variable is an entry of the property row's actions menu: the process tag row of -102G1 lists it
/// after the row's own actions, and choosing it remembers the row as a variable of the next save; a row of several
/// records does not list it.
#[test]
fn ft754h_make_variable_is_in_the_row_menu() {
    use ew_viewmodel::properties::RowMenuItem;
    let tmp = Tmp::new("macros-rowmenu");
    let (t, _) = user_library(&tmp.0);
    let mut shell = shell_with(&tmp, "a", Mode::Gui, &t, "102");
    done(&mut shell, "select.set", json!({"refs": ["tag:-102G1"]}));
    done(&mut shell, "props.show", json!({"on": true}));
    done(&mut shell, "props.row", json!({"to": "device/process_tag"}));
    let menu = shell.row_menu("device/process_tag");
    let at = menu
        .iter()
        .position(|i| *i == RowMenuItem::MakeVariable)
        .unwrap_or_else(|| panic!("Make variable in {menu:?}"));
    assert_eq!(at, menu.len() - 1, "after the row's own actions: {menu:?}");
    assert_eq!(
        ew_viewmodel::properties::menu_label(RowMenuItem::MakeVariable),
        "Make variable"
    );
    done(&mut shell, "props.row_menu", json!({}));
    done(&mut shell, "props.choose", json!({ "index": at }));
    assert!(shell.properties().popup().is_none(), "the menu closes");
    assert_eq!(shell.macro_variables().len(), 1);
    assert_eq!(shell.macro_variables()[0].field, "device.process_tag");
    // Two devices: the row stands for two records, so no Make variable.
    done(
        &mut shell,
        "select.set",
        json!({"refs": ["tag:-102G1", "tag:-102F1"]}),
    );
    let menu = shell.row_menu("device/process_tag");
    assert!(!menu.contains(&RowMenuItem::MakeVariable), "{menu:?}");
}
