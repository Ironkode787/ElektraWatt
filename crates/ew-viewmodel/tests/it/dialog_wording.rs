// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Headless: every dialog specification has exactly one Primary that is never destructive
//! and a title that ends with "?" exactly when it asks; the wording lint of doc 16a §8 over the registry labels,
//! the dialog specifications and the UI strings; the palette lists no action it cannot run from the window and
//! gives each result its breadcrumb; the outcome words.

use crate::common;
use ew_viewmodel::strings::Key;

use common::{Tmp, done};
use ew_viewmodel::actions::{ACTIONS, PaletteRun};
use ew_viewmodel::dialogs::{DialogAnswer, DialogSpec, spec};
use ew_viewmodel::shell::{ConfirmAnswer, Dialog, Mode, Pending, ShellModel};
use ew_viewmodel::strings;
use serde_json::{Value, json};

/// Every dialog of the shell: the constructible ones directly, the forms through their actions on the sample.
fn every_dialog(tmp: &Tmp) -> (ShellModel, Vec<Dialog>) {
    let mut shell = tmp.sample_in(Mode::Gui, true, "101");
    let mut dialogs = vec![
        Dialog::About,
        Dialog::Shortcuts,
        Dialog::Notices,
        Dialog::ConfirmDiscard {
            pending: Pending {
                action: "file.close",
                params: Value::Null,
                cancelled: None,
            },
            symbol: false,
        },
        Dialog::GotoPage { text: String::new() },
        Dialog::ConfirmJournalDiscard,
        Dialog::SaveWorkspace { text: String::new() },
        Dialog::ConfirmPurge {
            files: vec!["a.svg".into()],
            assets: vec!["a".into()],
        },
        Dialog::Recovery {
            name: "g.ew".into(),
            transactions: 2,
            base_matches: false,
            last_change_ms: 0,
            failed: None,
        },
        Dialog::Recovery {
            name: "g.ew".into(),
            transactions: 2,
            base_matches: true,
            last_change_ms: 0,
            failed: Some("the journal could not be replayed".into()),
        },
        // Quitting with an unsaved symbol draft.
        Dialog::ConfirmDiscard {
            pending: Pending {
                action: "app.quit",
                params: Value::Null,
                cancelled: None,
            },
            symbol: true,
        },
        // Removing a part from My parts.
        Dialog::ConfirmRemovePart {
            number: "SW-6".into(),
        },
        // Leaving a part with changes not saved.
        Dialog::ConfirmLeavePart {
            number: "SW-6".into(),
            next: None,
        },
        // Opening another part while the card of the open part's Save waits.
        Dialog::ConfirmLeavePart {
            number: "SW-6".into(),
            next: Some(("parts.open", serde_json::Map::new())),
        },
        // Replacing an earlier DXF set.
        Dialog::ConfirmReplaceDxf {
            dir: "dxf".into(),
            folder: "dxf".into(),
            files: 3,
        },
    ];
    // Every action whose label ends with "…" asks before it acts (doc 16a §8 item 8), so with no parameters it changes
    // nothing: the registry is walked, each such action is dispatched bare, the dialog it opens is taken for the
    // lint, and the project's revision stays as it was. An action that needs more than this page to ask (a
    // selection, a finding) is refused, which changes nothing either. No list of these actions is kept here.
    let rev = shell.revision();
    for meta in ACTIONS {
        if !strings::text(Key::run_time(&meta.label_key())).ends_with('…') {
            continue;
        }
        // The print pre-dialog (no build waits in the window's mode; the set is not built yet).
        let params = if meta.id == "file.print" {
            json!({ "wait": false })
        } else {
            json!({})
        };
        let _ = shell.dispatch(meta.id, &params);
        if let Some(d @ (Dialog::Form(_) | Dialog::Print(_) | Dialog::NewProject(_) | Dialog::Templates(_))) =
            shell.dialog()
        {
            dialogs.push(d.clone());
        }
        back_to_rest(&mut shell);
        assert_eq!(
            shell.revision(),
            rev,
            "{}: its label ends with … but it acted without asking",
            meta.id
        );
    }
    assert!(
        dialogs.iter().filter(|d| matches!(d, Dialog::Form(_))).count() >= 3,
        "the creation and renumber forms open"
    );
    (shell, dialogs)
}

/// Closes whatever an action left open (a dialog, a picker, the palette, a card, the wizard, the first run, a drawing
/// and Parts) and arms Select, so that the next action starts from the project view.
const REST: &[&str] = &[
    "wizard.cancel",
    "first_run.finish",
    "parts.back",
    "parts.back",
    "find.close",
];

fn back_to_rest(shell: &mut ShellModel) {
    for _ in 0..4 {
        let _ = shell.dispatch("ui.dismiss", &Value::Null);
    }
    for close in REST {
        // Parts closes without its question about the part.
        let _ = shell.dispatch(close, &json!({ "discard": true }));
    }
    let _ = shell.dispatch("tool.select", &json!({}));
}

fn destructive(spec: &DialogSpec, answer: &DialogAnswer) -> bool {
    spec.danger.as_ref().is_some_and(|d| &d.answer == answer)
        || matches!(
            answer,
            DialogAnswer::Confirm(ConfirmAnswer::Discard)
                | DialogAnswer::Action("file.discard_journal" | "project.purge_assets", _)
        )
}

#[test]
fn every_dialog_has_one_primary_never_destructive_and_asks_with_a_question_mark() {
    let tmp = Tmp::new("part4-cp1");
    let (mut shell, dialogs) = every_dialog(&tmp);
    for d in &dialogs {
        let s = spec(d, &shell);
        assert!(
            !destructive(&s, &s.primary.answer),
            "{}: a destructive Primary",
            s.title
        );
        assert_eq!(
            s.buttons().iter().filter(|b| **b == &s.primary).count(),
            1,
            "{}: exactly one Primary",
            s.title
        );
        assert_eq!(
            s.title.ends_with('?'),
            s.asks,
            "{}: asks iff it ends with ?",
            s.title
        );
        // A destructive step is the danger button, never a secondary one.
        for b in &s.secondary {
            assert!(
                !destructive(&s, &b.answer),
                "{}: {} is destructive",
                s.title,
                b.label
            );
        }
        // Every dialog has a way out: a button that cancels (Escape does the same, `ui.dismiss`).
        assert!(s.buttons().iter().any(|b| b.cancels()), "{}: no way out", s.title);
    }
    // The quit prompt and the recovery dialog read as specified.
    let quit = spec(&dialogs[3], &shell);
    assert_eq!(quit.title, "Save changes to sample.ew before closing?");
    let labels: Vec<&str> = quit.buttons().iter().map(|b| b.label.as_str()).collect();
    assert_eq!(labels, ["Save", "Cancel", "Don't save"]);
    let recovery = spec(&dialogs[8], &shell);
    assert_eq!(recovery.title, "Recover unsaved changes?");
    let labels: Vec<&str> = recovery.buttons().iter().map(|b| b.label.as_str()).collect();
    assert_eq!(labels, ["Recover", "Open read-only", "Discard…"]);
    let failed = spec(&dialogs[9], &shell);
    assert_eq!(
        failed.primary.label, "Open read-only",
        "after a failure the way out leads"
    );
    // Purge confirms a destructive step: Cancel is the Primary, Purge the danger button.
    let purge = spec(&dialogs[7], &shell);
    assert!(purge.confirms_destruction());
    assert_eq!(purge.danger.as_ref().map(|b| b.label.as_str()), Some("Purge"));
    // So does taking a part out of My parts: Cancel the Primary, Remove the danger button.
    let remove = spec(&dialogs[11], &shell);
    assert!(remove.confirms_destruction());
    assert_eq!(remove.title, "Remove SW-6 from your parts library?");
    assert_eq!(remove.danger.as_ref().map(|b| b.label.as_str()), Some("Remove"));
    shell.shutdown();
}

/// The proper nouns, codes and acronyms that keep their case inside a sentence-case text (doc 16a §8).
const PROPER: &[&str] = &[
    "PDF",
    "QET",
    "EPLAN",
    "IEC",
    "PLC",
    "CSV",
    "CP",
    "IP",
    "PE",
    "GPU",
    "CPU",
    "WARP",
    "ElektraWatt",
    "QElectroTech",
    "UI",
    "ID",
    "IDs",
    "SVG",
    "PNG",
    "DXF",
    "BOM",
    "PRIVACY.md",
    "Ctrl",
    "Alt",
    "Shift",
    "F1",
    "Esc",
    "Enter",
    "Del",
    "N",
    "L1",
    "L2",
    "L3",
    "OK",
    "UTC",
    "KiB",
    "Windows",
    "Linux",
    "macOS",
    "I",
    "R",
    "V",
    "Q",
    "X",
    "PATH",
];

/// The words of `text` that break sentence case: a capitalised word after the first that is not a proper noun, a
/// code (it holds a digit, a hyphen-prefixed tag or a second capital) or the start of a new sentence.
fn case_breaks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut sentence_start = true;
    for raw in text.split_whitespace() {
        let word = raw.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '-');
        let word = word.trim_end_matches('.');
        if word.is_empty() {
            continue;
        }
        let starts_upper = word.chars().next().is_some_and(char::is_uppercase);
        let code = word.chars().any(|c| c.is_ascii_digit())
            || word.chars().skip(1).any(char::is_uppercase)
            || word.starts_with('-');
        if starts_upper && !sentence_start && !code && !PROPER.contains(&word) && !raw.starts_with('{') {
            out.push(word.to_owned());
        }
        sentence_start = raw.ends_with(['.', ':', '?', '!', '▸']) || raw == "·";
    }
    out
}

/// Every text of `strings::KEYS`.
fn ui_strings() -> Vec<String> {
    strings::KEYS
        .iter()
        .map(|key| strings::text(strings::Key::run_time(key)).to_owned())
        .collect()
}

#[test]
fn the_wording_lint() {
    let tmp = Tmp::new("part4-cp3");
    let (mut shell, dialogs) = every_dialog(&tmp);
    let mut texts: Vec<(String, String)> = Vec::new();
    for meta in ACTIONS {
        texts.push((
            meta.id.to_owned(),
            strings::text(Key::run_time(&meta.label_key())).to_owned(),
        ));
    }
    for d in &dialogs {
        let s = spec(d, &shell);
        texts.push(("dialog title".into(), s.title.clone()));
        for b in s.buttons() {
            texts.push(("dialog button".into(), b.label.clone()));
        }
        for p in &s.body {
            texts.push(("dialog body".into(), p.clone()));
        }
    }
    for s in ui_strings() {
        texts.push(("string".into(), s));
    }
    // No three full stops anywhere.
    let dots: Vec<&(String, String)> = texts.iter().filter(|(_, t)| t.contains("...")).collect();
    assert!(dots.is_empty(), "three full stops instead of …: {dots:#?}");
    // Sentence case in the registry labels, the dialog titles and buttons.
    let mut breaks = Vec::new();
    for (what, text) in &texts {
        if what == "string" || what == "dialog body" {
            continue;
        }
        let words = case_breaks(text);
        if !words.is_empty() {
            breaks.push(format!("{what}: \"{text}\" {words:?}"));
        }
    }
    assert!(breaks.is_empty(), "not sentence case:\n{}", breaks.join("\n"));
    // A dialog's buttons are verbs, never "OK" or "Yes" (doc 16a §8 item 9).
    for d in &dialogs {
        for b in spec(d, &shell).buttons() {
            assert!(!matches!(b.label.as_str(), "OK" | "Yes" | "No"), "{}", b.label);
        }
    }
    shell.shutdown();
}

#[test]
fn the_palette_lists_only_what_the_window_can_run_with_breadcrumbs() {
    let tmp = Tmp::new("part4-cp4");
    let mut shell = tmp.sample_in(Mode::Gui, true, "101");
    done(&mut shell, "tools.palette", json!({}));
    let entries = shell.palette_entries();
    assert!(!entries.is_empty());
    for e in &entries {
        let PaletteRun::Action(id) = &e.run else {
            panic!("a model command is listed: {}", e.id);
        };
        let meta = ACTIONS.iter().find(|m| m.id == *id).unwrap();
        // Actions that need parameters are listed, with a parameter form
        // or disabled with their reason.
        if let ew_viewmodel::palette::FormPlan::Disabled(_) = ew_viewmodel::palette::form_plan(meta) {
            assert!(
                !e.enabled && e.reason.is_some(),
                "{id} is disabled with its reason"
            );
        }
        assert!(!meta.headless_only && !meta.hidden, "{id}");
        assert!(!e.breadcrumb.is_empty(), "{id} has a breadcrumb");
    }
    // Enabled entries first.
    let first_disabled = entries.iter().position(|e| !e.enabled).unwrap_or(entries.len());
    assert!(entries[first_disabled..].iter().all(|e| !e.enabled));
    // A placed action's breadcrumb is its tab and group ("Draw · Place"); an unplaced one names its menu.
    let place = entries.iter().find(|e| e.id == "tool.place").expect("Symbol…");
    assert_eq!(place.breadcrumb, "Draw · Place");
    let about = entries.iter().find(|e| e.id == "help.about").expect("About");
    assert_eq!(about.breadcrumb, "Help");
    // Every action that needs parameters is listed (with a form, or disabled with its reason);
    // the hidden and headless-only ones stay out.
    let needs: Vec<&str> = ACTIONS
        .iter()
        .filter(|m| m.needs_parameters() && !m.headless_only && !m.hidden)
        .map(|m| m.id)
        .collect();
    assert!(needs.iter().all(|id| entries.iter().any(|e| e.id == *id)));
    shell.shutdown();
}
