// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.17 part 4 headless (§12.4): CP1 every dialog specification has exactly one Primary that is never destructive
//! and a title that ends with "?" exactly when it asks; CP3 the wording lint of §8 item 8 over the registry labels,
//! the dialog specifications and the UI strings; CP4 the palette lists no action it cannot run from the window and
//! gives each result its breadcrumb; FT-510's outcome words.

mod common;

use std::collections::BTreeSet;

use common::{Tmp, done};
use ew_viewmodel::actions::{ACTIONS, PaletteRun};
use ew_viewmodel::dialogs::{DialogAnswer, DialogSpec, spec};
use ew_viewmodel::shell::{ConfirmAnswer, Dialog, Mode, Pending, ShellModel};
use ew_viewmodel::strings;
use serde_json::{Value, json};

/// Every dialog of the shell: the constructible ones directly, the forms through their actions on Golden A.
fn every_dialog(tmp: &Tmp) -> (ShellModel, Vec<Dialog>) {
    let mut shell = tmp.shell_in(Mode::Gui, true, "101");
    let mut dialogs = vec![
        Dialog::About,
        Dialog::Shortcuts,
        Dialog::Notices,
        Dialog::ConfirmDiscard {
            pending: Pending {
                action: "file.close",
                params: Value::Null,
            },
        },
        Dialog::GotoPage { text: String::new() },
        Dialog::ConfirmJournalDiscard,
        Dialog::ConfirmCheckScheme {
            scheme: "full".into(),
        },
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
    ];
    for (action, params) in [
        ("page.new", json!({})),
        ("structure.new", json!({})),
        ("external_ref.new", json!({})),
        ("pages.renumber", json!({})),
        ("devices.renumber", json!({})),
    ] {
        let _ = shell.dispatch(action, &params);
        if let Some(d @ Dialog::Form(_)) = shell.dialog() {
            dialogs.push(d.clone());
        }
        let _ = shell.dispatch("ui.dismiss", &Value::Null);
    }
    assert!(
        dialogs.iter().filter(|d| matches!(d, Dialog::Form(_))).count() >= 3,
        "the creation and renumber forms open"
    );
    (shell, dialogs)
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
fn cp1_every_dialog_has_one_primary_never_destructive_and_asks_with_a_question_mark() {
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
    // The quit prompt and the recovery dialog read as specified (§8 item 1).
    let quit = spec(&dialogs[3], &shell);
    assert_eq!(quit.title, "Save changes to J-E-2416.ew before closing?");
    let labels: Vec<&str> = quit.buttons().iter().map(|b| b.label.as_str()).collect();
    assert_eq!(labels, ["Save", "Cancel", "Don't save"]);
    let recovery = spec(&dialogs[9], &shell);
    assert_eq!(recovery.title, "Recover unsaved changes?");
    let labels: Vec<&str> = recovery.buttons().iter().map(|b| b.label.as_str()).collect();
    assert_eq!(labels, ["Recover", "Open read-only", "Discard…"]);
    let failed = spec(&dialogs[10], &shell);
    assert_eq!(
        failed.primary.label, "Open read-only",
        "after a failure the way out leads"
    );
    // Purge confirms a destructive step: Cancel is the Primary, Purge the danger button.
    let purge = spec(&dialogs[8], &shell);
    assert!(purge.confirms_destruction());
    assert_eq!(purge.danger.as_ref().map(|b| b.label.as_str()), Some("Purge"));
    shell.shutdown();
}

/// The proper nouns, codes and acronyms that keep their case inside a sentence-case text (§8 item 8, doc 16a §8).
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

/// Every string literal value of `strings::lookup` (read from its source: the texts are a `match`).
fn ui_strings() -> Vec<String> {
    let source = include_str!("../src/strings.rs");
    let body = source.split("pub fn lookup").nth(1).unwrap();
    let body = body.split("_ => return None").next().unwrap();
    let mut out = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.starts_with("//") {
            continue;
        }
        // `"key" => "value",` or a continuation line `"value"`.
        let value = match line.split_once("=> ") {
            Some((_, v)) => v,
            None => line,
        };
        if let Some(v) = value.strip_prefix('"')
            && let Some(end) = v.rfind('"')
        {
            out.push(v[..end].replace("\\\"", "\""));
        }
    }
    out
}

/// The actions whose dispatch opens a dialog, a picker, a pane that takes the input, or the system's file dialog
/// before it acts (§8 item 8: exactly these end with "…"). A label change that adds or drops an ellipsis changes this
/// list in the same reviewed commit.
const OPENS_INPUT: &[&str] = &[
    "file.new",
    // WP-S2.08 part 2: the keymap editor's Import… and Export… ask for a file.
    "keymap.import",
    "keymap.export",
    "file.open",
    "file.save_as",
    "file.import_qet",
    "parts.import",
    "file.discard_journal",
    "file.export_pdf",
    "find.open",
    "find.open_replace",
    "page.new",
    "pages.renumber",
    "nav.goto",
    "page.goto",
    "view.workspace_save",
    "tools.company_standard",
    "help.first_run",
    "tool.place",
    "tool.place_part",
    "strip.new",
    "rep.exchange",
    "junction.open",
    "tool.black_box",
    "ip.continue_on",
    "tool.image",
    "structure.new",
    "external_ref.new",
    "devices.renumber",
    "project.purge_assets",
    "table.pattern_fill",
    "messages.fix",
    // FT-674: Waive… opens the reason and author prompt.
    "messages.waive",
    // WP-S2.18 part 1: J opens the Join chooser.
    "place.join",
    // WP-S1.19 part 6 (§11): Insert macro… shows the Macros navigator (or floats the macro under its cursor), Save as
    // macro… opens its dialog, Update… opens the bulk card.
    "macros.insert",
    "macros.save",
    "libraries.update",
    // WP-S2.07 part 4a (D19, D20): Choose part… and Edit part… open the part dialog and the part editor.
    "part.choose",
    "part.edit",
];

#[test]
fn cp3_the_wording_lint() {
    let tmp = Tmp::new("part4-cp3");
    let (mut shell, dialogs) = every_dialog(&tmp);
    let mut texts: Vec<(String, String)> = Vec::new();
    for meta in ACTIONS {
        texts.push((meta.id.to_owned(), strings::text(meta.label_key).to_owned()));
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
    // The ellipsis ends exactly the labels of the actions that ask for more input first.
    let with: BTreeSet<&str> = ACTIONS
        .iter()
        .filter(|m| strings::text(m.label_key).ends_with('…'))
        .map(|m| m.id)
        .collect();
    let expected: BTreeSet<&str> = OPENS_INPUT
        .iter()
        .copied()
        .filter(|id| ACTIONS.iter().any(|m| m.id == *id))
        .collect();
    assert_eq!(with, expected, "the actions whose label ends with …");
    // A dialog's buttons are verbs, never "OK" or "Yes" (doc 16a §8 item 9).
    for d in &dialogs {
        for b in spec(d, &shell).buttons() {
            assert!(!matches!(b.label.as_str(), "OK" | "Yes" | "No"), "{}", b.label);
        }
    }
    shell.shutdown();
}

#[test]
fn cp4_the_palette_lists_only_what_the_window_can_run_with_breadcrumbs() {
    let tmp = Tmp::new("part4-cp4");
    let mut shell = tmp.shell_in(Mode::Gui, true, "101");
    done(&mut shell, "tools.palette", json!({}));
    let entries = shell.palette_entries();
    assert!(!entries.is_empty());
    for e in &entries {
        let PaletteRun::Action(id) = &e.run else {
            panic!("a model command is listed: {}", e.id);
        };
        let meta = ACTIONS.iter().find(|m| m.id == *id).unwrap();
        // WP-S2.08 D5 (a) supersedes P3 for actions that need parameters: they are listed, with a parameter form
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
    // WP-S2.08 D5 (a): every action that needs parameters is listed now (with a form, or disabled with its reason);
    // the hidden and headless-only ones stay out.
    let needs: Vec<&str> = ACTIONS
        .iter()
        .filter(|m| m.needs_parameters && !m.headless_only && !m.hidden)
        .map(|m| m.id)
        .collect();
    assert!(needs.iter().all(|id| entries.iter().any(|e| e.id == *id)));
    shell.shutdown();
}
