// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The shell frame of WP-S2.01 part 2 without rendering: the start page's new entries, the first-run flow, the
//! company-standard wizard with its live preview, docking and workspaces, F1 and the complete About, and the
//! journal discard banner, each followed by the accessible-name walker (ADR-0014 amendment 2).

mod support;

use egui::{Key, Modifiers};
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::Pick;
use ew_ui::ew_viewmodel::layout::{Layout, LayoutNode, PaneKind};
use ew_ui::ew_viewmodel::shell::{AccessMode, Dialog, Mode, Prompt, ShellModel};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness_with, settle, wait_for_page, wait_for_preview};

fn gui_shell(tmp: &Tmp) -> ShellModel {
    ShellModel::new(Mode::Gui, tmp.config_with_resources())
}

fn done(shell: &mut ShellModel, action: &str, params: Value) {
    let out = shell.dispatch(action, &params);
    assert_eq!(
        out,
        ew_ui::ew_viewmodel::shell::Outcome::Done,
        "{action}: {out:?}"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// The start page

#[test]
fn the_start_page_has_every_entry_of_doc_16_section_3_7() {
    let tmp = Tmp::new();
    let mut h = harness_with(gui_shell(&tmp), Scripted::default(), None);
    settle(&mut h);
    for label in [
        "New project",
        "Template: the built-in basic project",
        "Open…",
        "Open sample project",
        "Import QElectroTech project",
        "Create company standard",
        "Tutorials",
        "Language",
        "Keymap",
        "English",
        "EW",
        "KiCad-like",
        "Update check: off",
        "change",
    ] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert!(
        h.query_by_label("Edit company standard").is_none(),
        "no company standard yet"
    );
}

#[test]
fn new_project_asks_for_a_directory_and_opens_the_built_in_page() {
    let tmp = Tmp::new();
    let dir = tmp.0.join("first.ew");
    let platform = Scripted::default();
    platform.answers.borrow_mut().push_back(Pick::Path(dir.clone()));
    let mut h = harness_with(gui_shell(&tmp), platform.clone(), None);
    settle(&mut h);
    h.get_by_label("New project").click();
    settle(&mut h);
    assert_eq!(platform.asked.borrow().as_slice(), [Prompt::NewFolder]);
    assert_eq!(h.state().shell().project_name(), Some("first.ew"));
    assert_eq!(h.state().shell().page_labels(), ["1"]);
    assert!(dir.join("manifest.json").is_file());
    wait_for_page(&mut h);
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn the_sample_project_opens_read_only_from_the_start_page() {
    let tmp = Tmp::new();
    let mut h = harness_with(gui_shell(&tmp), Scripted::default(), None);
    settle(&mut h);
    h.get_by_label("Open sample project").click();
    settle(&mut h);
    let shell = h.state().shell();
    assert_eq!(shell.project_name(), Some("J-E-2416.ew"));
    assert!(matches!(
        shell.access(),
        Some(AccessMode::ReadOnly {
            reason_key: "readonly-sample"
        })
    ));
    assert!(
        h.query_by_label_contains("sample project. It is open read-only")
            .is_some()
    );
    wait_for_page(&mut h);
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn a_missing_sample_greys_its_button() {
    let tmp = Tmp::new();
    let shell = ShellModel::new(Mode::Gui, tmp.config());
    let mut h = harness_with(shell, Scripted::default(), None);
    settle(&mut h);
    assert!(
        h.get_by_label("Open sample project")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn the_quick_switches_and_the_privacy_link_change_settings() {
    let tmp = Tmp::new();
    let mut h = harness_with(gui_shell(&tmp), Scripted::default(), None);
    settle(&mut h);
    h.get_by_label("KiCad-like").click();
    settle(&mut h);
    assert_eq!(h.state().shell().settings().keymap, "kicad");
    h.get_by_label("EW").click();
    settle(&mut h);
    assert_eq!(h.state().shell().settings().keymap, "ew");
    // "change" leads to the privacy step of the first-run flow.
    h.get_by_label("change").click();
    settle(&mut h);
    assert!(h.state().shell().first_run().is_some());
    assert!(h.query_by_label("Check for updates (off by default)").is_some());
    h.get_by_label("Check for updates (off by default)").click();
    settle(&mut h);
    assert!(h.state().shell().settings().update_check);
}

// ---------------------------------------------------------------------------------------------------------------
// First run

#[test]
fn the_first_run_flow_walks_its_steps_and_starts_with_the_sample() {
    let tmp = Tmp::new();
    let mut shell = gui_shell(&tmp);
    shell.start_first_run_if_needed();
    let mut h = harness_with(shell, Scripted::default(), None);
    settle(&mut h);
    assert!(h.query_by_label("Welcome to ElektraWatt").is_some());
    assert!(h.query_by_label_contains("1 / 5: Language").is_some());
    h.get_by_label("Next").click();
    settle(&mut h);
    assert!(h.query_by_label_contains("2 / 5: I come from").is_some());
    h.get_by_label("KiCad").click();
    settle(&mut h);
    assert_eq!(h.state().shell().settings().comes_from, "kicad");
    assert_eq!(h.state().shell().settings().keymap, "kicad");
    h.get_by_label("Skip").click();
    settle(&mut h);
    assert!(h.query_by_label_contains("3 / 5: Privacy").is_some());
    assert!(
        !h.state().shell().settings().update_check,
        "off until the user says otherwise"
    );
    h.get_by_label("Next").click();
    settle(&mut h);
    h.get_by_label("Dark").click();
    settle(&mut h);
    assert_eq!(h.state().shell().settings().theme, "dark");
    h.get_by_label("125 %").click();
    settle(&mut h);
    assert_eq!(h.state().shell().settings().ui_scale, 1.25);
    h.get_by_label("Back").click();
    settle(&mut h);
    h.get_by_label("Next").click();
    settle(&mut h);
    h.get_by_label("Next").click();
    settle(&mut h);
    assert!(h.query_by_label_contains("5 / 5: Where to start").is_some());
    h.get_by_label("Open the sample project").click();
    settle(&mut h);
    let shell = h.state().shell();
    assert!(shell.first_run().is_none());
    assert!(shell.settings().first_run_done);
    assert_eq!(shell.project_name(), Some("J-E-2416.ew"));
    h.state_mut().shell_mut().shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// The wizard

#[test]
fn the_wizard_shows_a_preview_follows_a_typed_value_and_writes_the_basic_project() {
    let tmp = Tmp::new();
    let basic = tmp.0.join("company.ew");
    let platform = Scripted::default();
    platform.answers.borrow_mut().push_back(Pick::Path(basic.clone()));
    let mut h = harness_with(gui_shell(&tmp), platform.clone(), None);
    settle(&mut h);
    h.get_by_label("Create company standard").click();
    settle(&mut h);
    assert!(h.state().shell().wizard().is_some());
    assert!(h.query_all_by_label("1. Company").count() >= 1);
    assert!(h.query_by_label("Preview").is_some());
    wait_for_preview(&mut h);
    let first = h.state().shell().wizard().unwrap().preview_rev();

    // A step with a text field: type a numbering format and commit it with Enter.
    h.get_by_label("5. Identifiers and numbering").click();
    settle(&mut h);
    let field = h.get_by_role_and_label(egui::Role::TextInput, "Device numbering format");
    field.focus();
    h.run_ok();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run_ok();
    h.get_by_role_and_label(egui::Role::TextInput, "Device numbering format")
        .type_text("{page}-{letter}{counter}");
    h.run_ok();
    h.key_press(Key::Enter);
    settle(&mut h);
    let w = h.state().shell().wizard().unwrap();
    assert_eq!(w.draft().get("device_format"), "{page}-{letter}{counter}");
    assert!(
        w.preview_rev() > first,
        "the preview was rebuilt for the new format"
    );
    wait_for_preview(&mut h);
    // A choice: the cross-reference style.
    h.get_by_label("EPLAN style (/102.A1)").click();
    settle(&mut h);
    assert_eq!(h.state().shell().wizard().unwrap().draft().get("xref"), "eplan");
    // An invalid value is refused with its reason on the screen and leaves the draft as it was.
    h.get_by_label("Next").click();
    settle(&mut h);
    h.get_by_label("Back").click();
    settle(&mut h);
    // The cover is the other page of the preview.
    h.get_by_label("Cover").click();
    settle(&mut h);
    assert_eq!(h.state().shell().wizard().unwrap().preview_folio(), "1");
    wait_for_preview(&mut h);

    // The review lists what is written and what stays in the draft; Create asks for the directory.
    h.get_by_label("11. Review and create").click();
    settle(&mut h);
    assert!(h.query_all_by_label("kept: apply after finishing").count() > 0);
    assert!(h.query_all_by_label("written to the basic project").count() > 0);
    // WP-S2.17 part 1: controls of `size.control` (28 px) push the button under the fold of this window.
    h.get_by_label("Create the basic project…").scroll_to_me();
    settle(&mut h);
    h.get_by_label("Create the basic project…").click();
    settle(&mut h);
    assert_eq!(platform.asked.borrow().as_slice(), [Prompt::NewFolder]);
    let shell = h.state().shell();
    assert!(
        shell.wizard().is_some_and(|w| w.finished_dir().is_some()),
        "{:?}",
        shell.banner()
    );
    assert!(
        h.query_by_label_contains("The company standard was written to company.ew")
            .is_some()
    );
    // FT-336 ruling: the check scheme is applied after finishing, by a person, after a confirmation.
    h.get_by_label("Apply the check scheme").scroll_to_me();
    settle(&mut h);
    h.get_by_label("Apply the check scheme").click();
    settle(&mut h);
    assert!(matches!(
        h.state().shell().dialog(),
        Some(Dialog::ConfirmCheckScheme { .. })
    ));
    assert!(
        h.query_by_label_contains("decides which rules the release gate runs")
            .is_some()
    );
    h.get_by_label("Apply").click();
    settle(&mut h);
    let text = std::fs::read_to_string(basic.join("project.jsonl")).unwrap();
    assert!(
        text.contains("\"checks\":{\"release_scheme\":\"pre-delivery\"}"),
        "{text}"
    );
    assert!(h.query_all_by_label("kept: apply after finishing").count() == 0);
    assert!(h.query_by_label("Apply the check scheme").is_none());
    h.get_by_label("Close").scroll_to_me();
    settle(&mut h);
    h.get_by_label("Close").click();
    settle(&mut h);
    let shell = h.state().shell();
    assert!(shell.wizard().is_none());
    assert_eq!(
        shell.settings().basic_project.as_deref(),
        Some(basic.to_string_lossy().as_ref())
    );
    let text = std::fs::read_to_string(basic.join("project.jsonl")).unwrap();
    assert!(text.contains("\"kind\":\"basic\""));
    assert!(text.contains("{page}-{letter}{counter}"));
    assert!(text.contains("eplan-style"));
    // The start page now offers to edit it and says where new projects start.
    assert!(h.query_by_label("Edit company standard").is_some());
    assert!(h.query_by_label("Template: company.ew").is_some());
}

#[test]
fn closing_the_wizard_keeps_the_draft_and_the_screen_returns_to_the_start_page() {
    let tmp = Tmp::new();
    let mut h = harness_with(gui_shell(&tmp), Scripted::default(), None);
    settle(&mut h);
    support::open_menu(&mut h, &["Project"]);
    settle(&mut h);
    h.get_by_label("Company standard wizard…").click();
    settle(&mut h);
    assert!(h.state().shell().wizard().is_some());
    h.get_by_label("Close (the draft is kept)").click();
    settle(&mut h);
    assert!(h.state().shell().wizard().is_none());
    assert!(h.query_by_label("Create company standard").is_some());
}

// ---------------------------------------------------------------------------------------------------------------
// Docking and workspaces

#[test]
fn the_project_view_is_a_dock_and_a_changed_layout_becomes_the_workspace_file() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness_with(gui_shell(&tmp), Scripted::default(), Some(dir));
    settle(&mut h);
    assert_eq!(
        h.state().dock_panes(),
        [PaneKind::Pages, PaneKind::Properties, PaneKind::PageView]
    );
    // WP-S2.17 part 3: the navigator group's header is its rail entry's ("Project"), the page view has none.
    assert!(h.query_all_by_label("Project").count() >= 1);
    assert!(h.query_all_by_label("Pages").next().is_some());
    assert!(h.query_by_label("Page view").is_none());
    assert!(h.query_all_by_label("Properties").next().is_some());
    // View ▸ Properties closes the property grid (WP-S2.05): the two-pane layout this test drags. (A tab group nested
    // in a split shows egui_tiles' unnamed tab-bar area, which the name walker counts with the splitters, FT-338.)
    h.state_mut()
        .shell_mut()
        .dispatch("props.show", &serde_json::json!({"on": false}));
    settle(&mut h);
    assert_eq!(h.state().dock_panes(), [PaneKind::Pages, PaneKind::PageView]);
    h.state_mut().shell_mut().persist_layout_if_due(1.0e9);
    let _ = std::fs::remove_file(tmp.0.join("config/workspaces/Schematic.json"));
    assert!(!h.state().shell().layout_pending());

    // The user drags the page list onto the page view: one tab group. Two frames (half a second of the harness's
    // clock): the canvas's tiles arriving for the new pane would run more frames, and the file waits one second.
    h.state_mut()
        .dock_tab_together(PaneKind::Pages, PaneKind::PageView);
    h.step();
    h.step();
    let layout = h.state().shell().layout().clone();
    assert!(matches!(layout.root, LayoutNode::Tabs { .. }), "{layout:?}");
    assert_eq!(layout.panes().len(), 2);
    assert!(
        h.state().shell().layout_pending(),
        "the change waits one second for its file"
    );
    let ws = tmp.0.join("config/workspaces/Schematic.json");
    assert!(!ws.exists());
    assert!(h.state_mut().shell_mut().persist_layout_if_due(1.0e9));
    let text = std::fs::read_to_string(&ws).unwrap();
    assert_eq!(Layout::parse(&text).unwrap().1, layout);

    // Save it under a name through the dialog.
    support::open_menu(&mut h, &["View", "Layout", "Workspace"]);
    settle(&mut h);
    support::menu_node(&h, "Save layout as…").click();
    settle(&mut h);
    assert!(matches!(
        h.state().shell().dialog(),
        Some(Dialog::SaveWorkspace { .. })
    ));
    // The dialog's field (the page navigator has a filter field too).
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().data().placeholder() == Some("Workspace name"))
        .expect("the workspace name field")
        .focus();
    h.run_ok();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run_ok();
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().data().placeholder() == Some("Workspace name"))
        .expect("the workspace name field")
        .type_text("Tabbed");
    settle(&mut h);
    h.get_by_label("Save layout").click();
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none());
    assert_eq!(h.state().shell().workspace(), "Tabbed");
    assert!(tmp.0.join("config/workspaces/Tabbed.json").is_file());

    // Reset puts the built-in layout back and the frame follows.
    support::open_menu(&mut h, &["View", "Layout"]);
    settle(&mut h);
    support::menu_node(&h, "Reset layout").click();
    settle(&mut h);
    settle(&mut h);
    assert_eq!(h.state().shell().layout(), &Layout::schematic());
    assert_eq!(
        h.state().dock_panes(),
        [PaneKind::Pages, PaneKind::Properties, PaneKind::PageView]
    );
    // The reset was written to the active workspace's file, "Tabbed"; Schematic still has the tabs the user made.
    done(
        h.state_mut().shell_mut(),
        "view.workspace_load",
        json!({"name": "Schematic"}),
    );
    settle(&mut h);
    settle(&mut h);
    assert!(matches!(h.state().shell().layout().root, LayoutNode::Tabs { .. }));
    h.state_mut().shell_mut().shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// F1 and the About dialog

#[test]
fn f1_opens_the_page_of_the_context_in_the_system_browser() {
    let tmp = Tmp::new();
    let docs = tmp.0.join("book");
    std::fs::create_dir_all(&docs).unwrap();
    let mut config = tmp.config_with_resources();
    config.resources.docs_dir = Some(docs);
    let platform = Scripted::default();
    let mut h = harness_with(ShellModel::new(Mode::Gui, config), platform.clone(), None);
    settle(&mut h);
    h.key_press(Key::F1);
    settle(&mut h);
    {
        let opened = platform.opened.borrow();
        assert_eq!(opened.len(), 1, "{opened:?}");
        assert!(opened[0].starts_with("file://"), "{}", opened[0]);
        assert!(
            opened[0].ends_with("/book/how-to/application-window.html#start-page"),
            "{}",
            opened[0]
        );
    }
    // In a dialog F1 opens the dialog's page.
    done(h.state_mut().shell_mut(), "help.about", json!({}));
    settle(&mut h);
    h.key_press(Key::F1);
    settle(&mut h);
    assert!(platform.opened.borrow()[1].ends_with("#about"));
    // Over a menu item with a documentation topic, F1 opens that item's page, and its tooltip says so.
    done(h.state_mut().shell_mut(), "ui.dismiss", json!({}));
    settle(&mut h);
    support::open_menu(&mut h, &["File"]);
    settle(&mut h);
    let item = format!("New project… {}", ew_ui::keys::display("Ctrl+N"));
    h.get_by_label(&item).hover();
    h.run_ok();
    h.key_press(Key::F1);
    settle(&mut h);
    let opened = platform.opened.borrow();
    assert!(
        opened.last().is_some_and(|u| u.ends_with("#new-project")),
        "{opened:?}"
    );
}

#[test]
fn f1_without_an_installed_guide_says_so() {
    let tmp = Tmp::new();
    let platform = Scripted::default();
    let mut h = harness_with(gui_shell(&tmp), platform.clone(), None);
    settle(&mut h);
    h.key_press(Key::F1);
    settle(&mut h);
    assert!(platform.opened.borrow().is_empty());
    assert!(h.query_by_label_contains("user guide is not installed").is_some());
}

#[test]
fn the_about_dialog_is_complete_and_leads_to_the_notices() {
    let tmp = Tmp::new();
    let platform = Scripted::default();
    let mut h = harness_with(gui_shell(&tmp), platform.clone(), None);
    settle(&mut h);
    support::open_menu(&mut h, &["Help"]);
    settle(&mut h);
    h.get_by_label("About ElektraWatt").click();
    settle(&mut h);
    for label in [
        "Page view: CPU renderer",
        "Update check: off. Crash reports: none",
        "Licence: GPL-3.0-or-later",
        "ElektraWatt's checks do not replace engineering verification by a competent person.",
        "Support: pre-release build; only the latest 0.x release is supported.",
    ] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    assert!(h.query_by_label_contains("lists every data flow").is_some());
    h.get_by_label("Privacy (PRIVACY.md)").click();
    settle(&mut h);
    assert_eq!(
        *platform.opened.borrow(),
        [ew_ui::ew_viewmodel::about::PRIVACY_URL]
    );
    assert!(ew_ui::ew_viewmodel::about::PRIVACY_URL.ends_with("/blob/main/PRIVACY.md"));
    h.get_by_label("Third-party notices").click();
    settle(&mut h);
    assert!(matches!(h.state().shell().dialog(), Some(Dialog::Notices)));
    assert!(
        h.query_by_label_contains("Noto Sans").is_some(),
        "the font notices"
    );
    h.get_by_label("Close").click();
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none());
}

// ---------------------------------------------------------------------------------------------------------------
// The journal discard

#[test]
fn a_leftover_journal_is_discarded_from_the_banner() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    {
        let mut crashed = gui_shell(&tmp);
        done(&mut crashed, "file.open", json!({ "dir": dir.to_string_lossy() }));
        done(
            &mut crashed,
            "command.run",
            json!({"cmd": "page.create", "params": {"number": 996}}),
        );
        // Dropped without `shutdown`: the journal stays.
    }
    let mut h = harness_with(gui_shell(&tmp), Scripted::default(), Some(dir));
    settle(&mut h);
    assert!(h.state().shell().can_discard_journal());
    // The banner, and since WP-S2.09 part 1 the recovery dialog too, say it.
    assert!(
        h.query_all_by_label_contains("unsaved changes from a session that ended")
            .next()
            .is_some()
    );
    // WP-S2.17 part 4: the recovery dialog is modal; Escape closes it and the banner stays.
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none());
    h.get_by_label("Discard unsaved changes of the earlier session…")
        .click();
    settle(&mut h);
    assert!(matches!(
        h.state().shell().dialog(),
        Some(Dialog::ConfirmJournalDiscard)
    ));
    assert!(
        h.query_by_label_contains("Discarding them deletes that work")
            .is_some()
    );
    h.get_by_label("Cancel").click();
    settle(&mut h);
    assert!(
        h.state().shell().can_discard_journal(),
        "Cancel keeps the journal"
    );
    // WP-S2.17 part 4: the recovery dialog is modal; Escape closes it and the banner stays.
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none());
    h.get_by_label("Discard unsaved changes of the earlier session…")
        .click();
    settle(&mut h);
    h.get_by_label("Discard").click();
    settle(&mut h);
    let shell = h.state().shell();
    assert!(!shell.can_discard_journal());
    assert_eq!(shell.access(), Some(AccessMode::Edit));
    assert!(!shell.page_labels().iter().any(|l| l == "996"));
    assert!(h.query_by_label_contains("were discarded").is_some());
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn the_first_run_privacy_step_links_privacy_md() {
    let tmp = Tmp::new();
    let mut shell = gui_shell(&tmp);
    shell.start_first_run_if_needed();
    let platform = Scripted::default();
    let mut h = harness_with(shell, platform.clone(), None);
    settle(&mut h);
    h.get_by_label("Next").click();
    settle(&mut h);
    h.get_by_label("Skip").click();
    settle(&mut h);
    assert!(h.query_by_label_contains("3 / 5: Privacy").is_some());
    h.get_by_label("Privacy (PRIVACY.md)").click();
    settle(&mut h);
    assert_eq!(
        *platform.opened.borrow(),
        [ew_ui::ew_viewmodel::about::PRIVACY_URL]
    );
}
