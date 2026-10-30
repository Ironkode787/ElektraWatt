// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.08 part 1, the palette (FR-UI-012, NFR-UX-03, FT-361), on Golden A: PL-1 ranking with the shortcut, PL-2
//! determinism and the disabled tail, PL-3 the glossary aliases (one row per NFR-DOC-05 term), PL-4 the recents, PL-5
//! the prefixes, PL-6 the parameter forms, and PL-7's describe lines of an open form.

mod common;

use common::{Tmp, done};
use ew_viewmodel::actions::{ACTIONS, PaletteEntry, PaletteRun};
use ew_viewmodel::palette::{FormPlan, form_plan};
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellModel};
use ew_viewmodel::strings;
use serde_json::json;

fn query(shell: &mut ShellModel, q: &str) -> Vec<PaletteEntry> {
    if shell.palette().is_none() {
        done(shell, "tools.palette", json!({}));
    }
    shell.set_palette_query(q);
    shell.palette_entries()
}

fn ids(entries: &[PaletteEntry]) -> Vec<&str> {
    entries.iter().map(|e| e.id.as_str()).collect()
}

#[test]
fn pl1_renum_ranks_both_renumber_actions_first_and_a_subsequence_still_finds() {
    let tmp = Tmp::new("pal-pl1");
    let mut shell = tmp.shell_in(Mode::Gui, true, "103");
    let entries = query(&mut shell, "renum");
    assert_eq!(ids(&entries)[..2], ["devices.renumber", "pages.renumber"]);
    assert_eq!(entries[0].label, "Renumber devices…");
    assert_eq!(entries[1].label, "Renumber pages…");
    // The shortcut text each entry carries (empty for devices.renumber until part 2's D7).
    let shortcut = |id: &str| {
        ACTIONS
            .iter()
            .find(|a| a.id == id)
            .and_then(|a| a.shortcut.first().copied())
            .unwrap_or("")
    };
    assert_eq!(entries[0].shortcut, shortcut("devices.renumber"));
    assert_eq!(entries[1].shortcut, shortcut("pages.renumber"));
    let entries = query(&mut shell, "rnmdv");
    assert_eq!(entries[0].id, "devices.renumber", "{:?}", ids(&entries));
    shell.shutdown();
}

#[test]
fn pl2_the_order_is_deterministic_ties_by_label_then_id_and_disabled_last() {
    let tmp = Tmp::new("pal-pl2");
    let mut shell = tmp.shell_in(Mode::Gui, false, "103");
    for q in ["", "page", "re", "zoom", "save"] {
        let a = query(&mut shell, q);
        let b = query(&mut shell, q);
        assert_eq!(a, b, "{q}");
        let first_disabled = a.iter().position(|e| !e.enabled).unwrap_or(a.len());
        assert!(a[first_disabled..].iter().all(|e| !e.enabled), "{q}");
        assert!(
            a.iter().filter(|e| !e.enabled).all(|e| e.reason.is_some()),
            "{q}: a disabled entry carries its reason"
        );
    }
    // An empty query gives every entry the same score: the enabled ones are in label order (code points of the
    // lower-cased label, then the ID).
    let all = query(&mut shell, "");
    let enabled: Vec<&PaletteEntry> = all.iter().filter(|e| e.enabled).collect();
    for w in enabled.windows(2) {
        let (a, b) = (w[0].label.to_lowercase(), w[1].label.to_lowercase());
        assert!(
            a < b || (a == b && w[0].id < w[1].id),
            "{} before {}",
            w[0].label,
            w[1].label
        );
    }
    // Read-only: Save is listed after the enabled entries, with its reason.
    let save = query(&mut shell, "save");
    let file_save = save.iter().find(|e| e.id == "file.save").unwrap();
    assert!(!file_save.enabled);
    shell.shutdown();
}

/// PL-3 (NFR-UX-03): each NFR-DOC-05 term finds its EW action through a glossary alias, one row per term. A term the
/// glossary maps to no listable action is listed here with `None` and fails nothing.
#[test]
fn pl3_each_glossary_term_finds_its_action() {
    let tmp = Tmp::new("pal-pl3");
    let mut shell = tmp.shell_in(Mode::Gui, true, "103");
    let rows: &[(&str, Option<&str>)] = &[
        ("folio report", Some("tool.ip")),
        ("interruption point", Some("tool.ip")),
        ("master/slave", Some("tool.place")),
        ("message management", Some("messages.show")),
        ("CDP", Some("tool.marker")),
        ("conductor", Some("tool.wire")),
        ("plot frame", Some("tools.company_standard")),
        ("form", Some("tools.company_standard")),
        ("basic project", Some("file.new")),
        ("element", Some("tool.place")),
        ("function", Some("tool.place")),
        ("device", Some("tool.place_part")),
        ("part", Some("tool.place_part")),
        // Terms with no action (the glossary keeps their rows; the palette has nothing to run).
        ("update reports", None),
        ("potential definition point", None),
        ("contact image", None),
    ];
    for (term, action) in rows {
        let entries = query(&mut shell, term);
        match action {
            Some(id) => assert!(
                ids(&entries).iter().take(3).any(|e| e == id),
                "{term} → {id}: {:?}",
                &ids(&entries)[..entries.len().min(5)]
            ),
            None => {}
        }
    }
    // The headline terms rank their action first.
    for (term, id) in [
        ("folio report", "tool.ip"),
        ("message management", "messages.show"),
        ("CDP", "tool.marker"),
        ("conductor", "tool.wire"),
    ] {
        assert_eq!(query(&mut shell, term)[0].id, id, "{term}");
    }
    shell.shutdown();
}

#[test]
fn pl4_an_empty_query_lists_the_recent_actions_first_and_a_shortcut_counts() {
    let tmp = Tmp::new("pal-pl4");
    let mut shell = tmp.shell_in(Mode::Gui, false, "103");
    let run = |shell: &mut ShellModel, q: &str, id: &str| {
        let entries = query(shell, q);
        let at = entries
            .iter()
            .position(|e| e.id == id)
            .unwrap_or_else(|| panic!("{id}"));
        shell.select_palette_entry(at);
        assert_eq!(shell.run_palette_selection(), Outcome::Done, "{id}");
        assert!(shell.palette().is_none() || id == "tools.palette");
    };
    run(&mut shell, "zoom in", "view.zoom_in");
    run(&mut shell, "grid", "view.grid");
    run(&mut shell, "zoom in", "view.zoom_in");
    shell.note_shortcut_use("view.fit_page");
    done(&mut shell, "view.fit_page", json!({}));
    let entries = query(&mut shell, "");
    assert_eq!(ids(&entries)[..3], ["view.fit_page", "view.zoom_in", "view.grid"]);
    // Ten distinct uses: only the eight most recent lead.
    for id in [
        "view.zoom_out",
        "view.zoom_actual",
        "page.next",
        "page.previous",
        "page.first",
        "page.last",
        "view.pan_left",
    ] {
        shell.note_shortcut_use(id);
    }
    let entries = query(&mut shell, "");
    assert_eq!(
        ids(&entries)[..8],
        [
            "view.pan_left",
            "page.last",
            "page.first",
            "page.previous",
            "page.next",
            "view.zoom_actual",
            "view.zoom_out",
            "view.fit_page"
        ]
    );
    // The rest follow in rank order (enabled, then label).
    assert!(entries[8].enabled);
    // A use raises the rank of a non-empty query too: "zoom" now has Zoom in ahead of its label-order place.
    let zoom = query(&mut shell, "zoom");
    assert_eq!(zoom[0].id, "view.zoom_in", "{:?}", ids(&zoom));
    shell.shutdown();
}

#[test]
fn pl5_the_prefixes_find_pages_cells_objects_and_rules() {
    let tmp = Tmp::new("pal-pl5");
    let mut shell = tmp.shell_in(Mode::Gui, false, "101");
    // `#103` lists folio 103 first; Enter opens it.
    let pages = query(&mut shell, "#103");
    assert_eq!(pages[0].run, PaletteRun::Page("103".into()));
    assert_eq!(shell.run_palette_selection(), Outcome::Done);
    assert_eq!(shell.current_folio(), Some("103"));
    // `#pneu` lists 106 and 200 (doc 16 §4.3's example).
    let pneu = query(&mut shell, "#pneu");
    let folios: Vec<&str> = pneu
        .iter()
        .filter_map(|e| match &e.run {
            PaletteRun::Page(f) => Some(f.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(folios[..2], ["106", "200"], "{folios:?}");
    done(&mut shell, "ui.dismiss", json!({}));
    // `:103-A1` opens folio 103 with A1 at the centre.
    done(&mut shell, "page.goto", json!({"folio": "101"}));
    let cell = query(&mut shell, ":103-A1");
    assert_eq!(cell.len(), 1);
    assert_eq!(cell[0].run, PaletteRun::Cell("103-A1".into()));
    assert_eq!(shell.run_palette_selection(), Outcome::Done);
    assert_eq!(shell.current_folio(), Some("103"));
    // `@-102F` lists -102F1 and -102F2; Enter selects.
    let objects = query(&mut shell, "@-102F");
    let labels: Vec<&str> = objects.iter().map(|e| e.label.as_str()).collect();
    assert!(
        labels.contains(&"-102F1") && labels.contains(&"-102F2"),
        "{labels:?}"
    );
    let at = objects.iter().position(|e| e.label == "-102F1").unwrap();
    shell.select_palette_entry(at);
    assert_eq!(shell.run_palette_selection(), Outcome::Done);
    assert_eq!(shell.selection().items.len(), 1);
    assert_eq!(shell.current_folio(), Some("102"));
    // `?EW-POT-013` lists the rule with its title; Enter shows Messages filtered to it.
    let rules = query(&mut shell, "?EW-POT-013");
    assert_eq!(rules[0].run, PaletteRun::Rule("EW-POT-013".into()));
    assert!(rules[0].label.starts_with("EW-POT-013 "), "{}", rules[0].label);
    assert_eq!(shell.run_palette_selection(), Outcome::Done);
    let describe = shell.describe();
    assert!(describe.contains("EW-POT-013"), "{describe}");
    // `?` also lists the Help menu's actions.
    let help = query(&mut shell, "?about");
    assert!(help.iter().any(|e| e.id == "help.about"));
    // Unknown cell, no object: nothing listed.
    assert!(query(&mut shell, ":999-Z9").is_empty());
    assert!(query(&mut shell, "@zzzz-no-such").is_empty());
    shell.shutdown();
}

#[test]
fn pl6_forms_open_prefilled_run_with_enter_and_a_path_parameter_is_disabled() {
    let tmp = Tmp::new("pal-pl6");
    let mut shell = tmp.shell_in(Mode::Gui, true, "103");
    // devices.renumber: Enter opens its form pre-filled from the defaults; Enter (submit) runs it.
    let entries = query(&mut shell, "renumber devices");
    assert_eq!(entries[0].id, "devices.renumber");
    assert_eq!(shell.run_palette_selection(), Outcome::Done);
    assert!(shell.palette().is_none());
    let Some(Dialog::Form(form)) = shell.dialog() else {
        panic!("the renumber form: {:?}", shell.dialog());
    };
    let values: Vec<(&str, &str)> = form.fields.iter().map(|f| (f.name, f.value.as_str())).collect();
    assert_eq!(values[0], ("scope", "project"));
    assert_eq!(values[2], ("include_frozen", "false"));
    assert_eq!(values[3], ("only_nonconforming", "true"));
    assert_eq!(shell.submit_form(), Outcome::Done);
    // A signature-typed action that needs parameters: an inline form in the palette (pages.sort, by:enum).
    let sort = query(&mut shell, "pages.sort");
    let at = sort
        .iter()
        .position(|e| e.id == "pages.sort")
        .expect("pages.sort is listed");
    assert!(sort[at].enabled, "{:?}", sort[at].reason);
    shell.select_palette_entry(at);
    assert_eq!(shell.run_palette_selection(), Outcome::Done);
    let form = shell
        .palette()
        .and_then(|p| p.form.clone())
        .expect("the inline form");
    assert_eq!(form.action, "pages.sort");
    assert_eq!(form.fields[0].value, "folio");
    // The describe text shows the form.
    let describe = shell.describe();
    assert!(describe.contains("  action pages.sort"), "{describe}");
    assert!(describe.contains("  field \"By\": \"folio\""), "{describe}");
    // Escape returns to the list; Enter on the entry opens the form again; a bad value keeps the form with its error.
    done(&mut shell, "ui.dismiss", json!({}));
    assert!(shell.palette().is_some_and(|p| p.form.is_none()));
    assert_eq!(shell.run_palette_selection(), Outcome::Done);
    shell.set_palette_field(0, "nope");
    assert!(matches!(shell.run_palette_selection(), Outcome::Failed { .. }));
    assert!(
        shell
            .palette()
            .and_then(|p| p.form.as_ref())
            .is_some_and(|f| f.error.is_some())
    );
    shell.set_palette_field(0, "type");
    assert_eq!(shell.run_palette_selection(), Outcome::Done);
    assert!(shell.palette().is_none());
    // Every action with a path or reference parameter that needs parameters is disabled with D5's reason; the others
    // that need parameters have a form or the later-version reason.
    let all = query(&mut shell, "");
    for meta in ACTIONS
        .iter()
        .filter(|m| m.needs_parameters && !m.hidden && !m.headless_only)
    {
        let entry = all
            .iter()
            .find(|e| e.id == meta.id)
            .unwrap_or_else(|| panic!("{} listed", meta.id));
        match form_plan(meta) {
            FormPlan::Disabled(key) => {
                assert!(!entry.enabled, "{}", meta.id);
                assert_eq!(entry.reason.as_deref(), Some(strings::text(key)), "{}", meta.id);
            }
            FormPlan::Form(_) => {}
            FormPlan::Direct => panic!("{} needs parameters", meta.id),
        }
    }
    assert_eq!(
        strings::text("reason-palette-needs-file"),
        "Needs a file or a selection: use its menu"
    );
    shell.shutdown();
}
