// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.09 part 3 (§10.3): the Preview tab (PV2, PV3), the renumber dialogs on Golden B (PV2), the history panel and
//! context restore (PV5), the GUI purge step (PV6) and the exact counts of §9 item 5. Every test works on a copy of
//! Golden A or on a new project; Golden A itself is only read.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use common::{Tmp, done, entity_points, refused, state};
use ew_viewmodel::layout::PaneKind;
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellConfig, ShellModel};
use serde_json::{Value, json};

/// Every file of the project directory `dir` by its relative path, the manifest's `generation` and `base` lines left
/// out (a save after an undo equals the save before the step but for the generation and its link to the previous one).
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                walk(root, &path, out);
                continue;
            }
            let name = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if name.starts_with('.') || name.contains("/.") {
                continue;
            }
            let mut bytes = std::fs::read(&path).unwrap();
            if name == "manifest.json" {
                let text = String::from_utf8(bytes).unwrap();
                bytes = text
                    .lines()
                    .filter(|l| !l.contains("\"generation\"") && !l.contains("\"base\""))
                    .collect::<Vec<_>>()
                    .join("\n")
                    .into_bytes();
            }
            out.insert(name, bytes);
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// The files of two snapshots are equal; the message names those that differ.
fn assert_same(a: &BTreeMap<String, Vec<u8>>, b: &BTreeMap<String, Vec<u8>>, what: &str) {
    let differ: Vec<&String> = a
        .keys()
        .chain(b.keys())
        .filter(|k| a.get(*k) != b.get(*k))
        .collect();
    // The lines of the first differing file that the other lacks.
    let lines = |m: &BTreeMap<String, Vec<u8>>, k: &str| -> Vec<String> {
        m.get(k)
            .map(|b| String::from_utf8_lossy(b).lines().map(str::to_owned).collect())
            .unwrap_or_default()
    };
    let only = |x: &[String], y: &[String]| -> Vec<String> {
        x.iter().filter(|l| !y.contains(l)).cloned().collect()
    };
    let (left, right) = differ.first().map_or((Vec::new(), Vec::new()), |k| {
        let (l, r) = (lines(a, k), lines(b, k));
        (only(&l, &r), only(&r, &l))
    });
    assert!(
        differ.is_empty(),
        "{what}: {differ:?}
{left:#?}
{right:#?}"
    );
}

fn lines_with(shell: &ShellModel, prefix: &str) -> Vec<String> {
    shell
        .describe()
        .lines()
        .filter(|l| l.starts_with(prefix))
        .map(str::to_owned)
        .collect()
}

/// The `preview` section of the describe text: its header and its indented lines.
fn preview_section(shell: &ShellModel) -> Vec<String> {
    let d = shell.describe();
    let mut out = Vec::new();
    let mut inside = false;
    for l in d.lines() {
        if l.starts_with("preview ") {
            inside = true;
        } else if !l.starts_with("  ") {
            inside = false;
        }
        if inside {
            out.push(l.to_owned());
        }
    }
    out
}

fn history_cursor(shell: &mut ShellModel) -> usize {
    if !shell.history_shown() {
        done(shell, "history.show", json!({"on": true}));
    }
    shell.history_pane().view().expect("a view").cursor
}

/// B1's settings step (`project.settings.set`, not this package's command) through `command.run`.
fn b1_settings(shell: &mut ShellModel) {
    done(
        shell,
        "command.run",
        json!({"cmd": "project.settings.set", "params": {"member": "numbering", "value": {"default": "page-based",
            "schemes": [{"key": "page-based", "format": "{page}{letter}{counter}", "rules": [], "child_separator": "-"}]}}}),
    );
}

const B1_TAGS: [&str; 11] = [
    "  tag \"-XPE\" \"-101X2\"",
    "  tag \"1X1\" \"-101X1\"",
    "  tag \"1X1:1\" \"-101X1:1\"",
    "  tag \"1X1:2\" \"-101X1:2\"",
    "  tag \"1X1:PE\" \"-101X1:PE\"",
    "  tag \"E55\" \"-101E1\"",
    "  tag \"E56\" \"-101E2\"",
    "  tag \"F55\" \"-101F2\"",
    "  tag \"S51\" \"-101S1\"",
    "  tag \"S56\" \"-101S2\"",
    "  tag \"T55\" \"-101T1\"",
];

/// Golden B through the window's steps in `mode`: B1's renumber previews its tag changes in the Preview tab, B2's
/// renumber its 2 tags and 14 cross-reference texts with the page list; each Accept commits one transaction and a
/// save after its undo equals the save before it, `generation` aside.
fn golden_b(mode: Mode) {
    let tmp = Tmp::new("pv2");
    let mut shell = tmp.shell_in(mode, true, "101");
    let dir = tmp.golden_copy();
    b1_settings(&mut shell);
    done(&mut shell, "file.save", json!({}));
    let before_b1 = snapshot(&dir);
    let cursor = history_cursor(&mut shell);
    let calls = shell.session_calls();
    match mode {
        Mode::Headless => done(
            &mut shell,
            "devices.renumber",
            json!({"scope": "project", "scheme": "page_based", "include_frozen": true, "only_nonconforming": true}),
        ),
        Mode::Gui => {
            // Without a scheme the window opens the dialog with its defaults.
            done(&mut shell, "devices.renumber", json!({}));
            let d = shell.describe();
            assert!(
                d.contains(
                    "dialog \"Renumber devices\"\n  field \"Scope\": \"project\"\n  field \"Scheme\": \"page_based\"\n  field \"Include frozen tags\": \"false\"\n  field \"Only non-conforming\": \"true\"\n  button \"Renumber\"\n  button \"Cancel\""
                ),
                "{d}"
            );
            shell.set_form_field("include_frozen", "true");
            assert_eq!(shell.submit_form(), Outcome::Done);
            assert!(shell.dialog().is_none(), "the card replaces the dialog");
        }
    }
    let after = shell.session_calls();
    assert_eq!(after.preview_entities, calls.preview_entities + 1, "one per card");
    assert!(shell.layout().panes().contains(&PaneKind::Preview));
    let section = preview_section(&shell);
    let rev = shell.revision().unwrap();
    assert_eq!(
        section[..2],
        [
            format!("preview \"numbering.renumber\" ready rev {rev}"),
            "  summary records 8 tags 11 xrefs 0 pages 1".to_owned(),
        ],
        "{section:#?}"
    );
    let entities: Vec<&str> = section
        .iter()
        .filter(|l| l.starts_with("  entity "))
        .map(String::as_str)
        .collect();
    assert_eq!(entities.len(), 8, "{section:#?}");
    assert!(
        entities
            .iter()
            .all(|l| l.starts_with("  entity changed \"device\" "))
    );
    let tags: Vec<&str> = section
        .iter()
        .filter(|l| l.starts_with("  tag "))
        .map(String::as_str)
        .collect();
    assert_eq!(tags, B1_TAGS, "natural order of the old tags");
    assert_eq!(
        lines_with(&shell, "card ready \"numbering.renumber\""),
        ["card ready \"numbering.renumber\" targets 8 changes 8 tags 11 xrefs 0"]
    );
    done(&mut shell, "preview.apply", json!({}));
    assert_eq!(history_cursor(&mut shell), cursor + 1, "one transaction");
    assert!(shell.properties().card().is_none());
    assert_eq!(preview_section(&shell), ["preview none"]);
    done(&mut shell, "file.save", json!({}));
    assert_ne!(snapshot(&dir), before_b1);
    done(&mut shell, "edit.undo", json!({}));
    done(&mut shell, "file.save", json!({}));
    assert_same(&snapshot(&dir), &before_b1, "the undo restores B1's bytes");
    done(&mut shell, "edit.redo", json!({}));
    done(&mut shell, "file.save", json!({}));
    let before_b2 = snapshot(&dir);

    // B2: section 1 from 101.
    let cursor = history_cursor(&mut shell);
    match mode {
        Mode::Headless => done(
            &mut shell,
            "pages.renumber",
            json!({"section": "1", "start": 101}),
        ),
        Mode::Gui => {
            done(&mut shell, "pages.renumber", json!({}));
            let d = shell.describe();
            assert!(
                d.contains(
                    "dialog \"Renumber pages\"\n  field \"Section (number, or front)\": \"1\"\n  field \"Start\": \"101\"\n  field \"Step\": \"1\"\n  field \"Keep gaps\": \"false\"\n  button \"Renumber\""
                ),
                "{d}"
            );
            assert_eq!(shell.submit_form(), Outcome::Done);
        }
    }
    let section = preview_section(&shell);
    let rev = shell.revision().unwrap();
    assert_eq!(section[0], format!("preview \"page.renumber\" ready rev {rev}"));
    assert!(
        section[1].starts_with("  summary records 1 tags 2 xrefs 14 pages "),
        "{section:#?}"
    );
    assert!(
        section
            .iter()
            .any(|l| l.starts_with("  entity changed \"page\" \"106"))
    );
    let tags: Vec<&str> = section
        .iter()
        .filter(|l| l.starts_with("  tag "))
        .map(String::as_str)
        .collect();
    assert_eq!(
        tags,
        ["  tag \"-106W1\" \"-104W1\"", "  tag \"-106W2\" \"-104W2\""]
    );
    assert_eq!(section.iter().filter(|l| l.starts_with("  xref ")).count(), 14);
    let pages = shell.preview_pane().pages().to_vec();
    assert!(!pages.is_empty() && pages.iter().all(|f| shell.page_labels().contains(f)));
    assert_eq!(
        section.iter().filter(|l| l.starts_with("  page ")).count(),
        pages.len()
    );
    done(&mut shell, "preview.apply", json!({}));
    assert_eq!(history_cursor(&mut shell), cursor + 1, "one transaction");
    assert!(shell.page_labels().contains(&"104".to_owned()));
    done(&mut shell, "file.save", json!({}));
    done(&mut shell, "edit.undo", json!({}));
    done(&mut shell, "file.save", json!({}));
    assert_same(&snapshot(&dir), &before_b2, "the undo restores B2's bytes");
    shell.shutdown();
}

/// PV2 headless: the renumber actions with their parameters.
#[test]
fn pv2_golden_b_headless() {
    golden_b(Mode::Headless);
}

/// PV2 in the window's mode: the dialogs, their defaults and OK.
#[test]
fn pv2_golden_b_through_the_dialogs() {
    golden_b(Mode::Gui);
}

/// PV2: a dialog keeps a refusal in its error line; headless a missing start or scheme fails naming it.
#[test]
fn pv2_the_dialogs_refuse_in_place_and_headless_names_the_missing_parameter() {
    let tmp = Tmp::new("pv2-refuse");
    let mut gui = tmp.shell_in(Mode::Gui, true, "101");
    done(&mut gui, "pages.renumber", json!({}));
    gui.set_form_field("section", "99");
    let out = gui.submit_form();
    assert!(
        matches!(out, Outcome::Failed { .. }),
        "{out:?} {}",
        gui.describe()
    );
    assert!(
        gui.describe()
            .contains("  error: The project has no pages in section 99."),
        "{}",
        gui.describe()
    );
    done(&mut gui, "ui.dismiss", json!({}));
    assert!(gui.dialog().is_none());
    gui.shutdown();
    let tmp = Tmp::new("pv2-headless");
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    for (action, name) in [("pages.renumber", "start"), ("devices.renumber", "scheme")] {
        match s.dispatch(action, &json!({})) {
            Outcome::Failed { message } => assert!(message.contains(name), "{message}"),
            other => panic!("{action}: {other:?}"),
        }
    }
    assert_eq!(
        refused(
            &mut s,
            "devices.renumber",
            json!({"scope": "selection", "scheme": "page_based"})
        ),
        "reason-no-devices-selected"
    );
    s.shutdown();
}

/// PV3: every card origin opens in the Preview tab (the focus stays), its `card` line unchanged; a commit while it
/// is open makes it stale; closing the tab cancels the card.
#[test]
fn pv3_every_origin_opens_the_tab_and_closing_it_cancels() {
    let tmp = Tmp::new("pv3");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    // Properties: a tag counter on two devices.
    done(
        &mut shell,
        "select.set",
        json!({"refs": ["tag:-101F1", "tag:-102F1"]}),
    );
    let focus = shell.focus();
    done(
        &mut shell,
        "props.set",
        json!({"row": "device/tag.counter", "value": "9"}),
    );
    assert_eq!(state(&shell)["card"]["origin"], "properties");
    assert!(shell.preview_shown());
    assert_eq!(shell.focus(), focus, "the focus stays");
    let card = lines_with(&shell, "card ");
    assert_eq!(card.len(), 1);
    assert!(
        card[0].starts_with("card ready \"fields.set\" targets 2 changes 2"),
        "{card:?}"
    );
    assert!(preview_section(&shell)[0].starts_with("preview \"fields.set\" ready rev "));
    // Stale: a commit while the card is open.
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-102F2"}],
            "path": "function_text", "value": "x"}}),
    );
    assert!(
        preview_section(&shell)[0].contains(" stale rev "),
        "{:?}",
        preview_section(&shell)
    );
    // Closing the tab cancels the card.
    done(&mut shell, "preview.show", json!({"on": false}));
    assert!(shell.properties().card().is_none(), "the card went with the tab");
    assert!(!shell.preview_shown());
    // The window's dock closing the tab (`set_layout`) cancels the card too.
    done(
        &mut shell,
        "props.set",
        json!({"row": "device/tag.counter", "value": "9"}),
    );
    assert!(shell.preview_shown() && shell.properties().card().is_some());
    let closed = shell.layout().without(PaneKind::Preview);
    shell.set_layout(closed, 1.0);
    assert!(shell.properties().card().is_none(), "closed in the window");
    // Find: a replace card.
    done(&mut shell, "find.open_replace", json!({}));
    done(&mut shell, "find.text", json!({"text": "Spare"}));
    done(&mut shell, "find.replacement", json!({"text": "Reserve"}));
    done(&mut shell, "find.fields", json!({"fields": ["function_text"]}));
    done(
        &mut shell,
        "find.folios",
        json!({"folios": ["204", "205", "206"]}),
    );
    done(&mut shell, "find.run", json!({}));
    done(&mut shell, "find.replace_all", json!({}));
    assert_eq!(state(&shell)["card"]["origin"], "find");
    assert!(shell.preview_shown());
    assert_eq!(
        lines_with(&shell, "card ")[0],
        "card ready \"fields.set_many\" targets 20 changes 20 tags 0 xrefs 0"
    );
    assert!(
        preview_section(&shell)
            .iter()
            .any(|l| l == "  summary records 20 tags 0 xrefs 0 pages 3"),
        "{:?}",
        preview_section(&shell)
    );
    // 20 entities print without a `more` line; the pages close the section.
    assert!(!preview_section(&shell).iter().any(|l| l.starts_with("  more ")));
    assert_eq!(preview_section(&shell).last().unwrap(), "  page \"206\"");
    done(&mut shell, "preview.cancel", json!({}));
    assert_eq!(preview_section(&shell), ["preview none"], "the tab stays");
    done(&mut shell, "find.close", json!({}));
    // Reorder: the Pages navigator's card.
    done(
        &mut shell,
        "pages.reorder",
        json!({"pages": ["106"], "before": "103"}),
    );
    assert_eq!(state(&shell)["card"]["origin"], "reorder");
    assert!(preview_section(&shell)[0].starts_with("preview \"page.reorder\" ready rev "));
    assert!(
        lines_with(&shell, "card ready \"page.reorder\"")[0]
            .ends_with(" reorder \"106 → 103\" \"103 → 106\"")
    );
    done(&mut shell, "preview.cancel", json!({}));
    // Bulk: Renumber devices.
    done(
        &mut shell,
        "devices.renumber",
        json!({"scope": "page", "scheme": "page_based"}),
    );
    assert_eq!(state(&shell)["card"]["origin"], "bulk");
    done(&mut shell, "preview.cancel", json!({}));
    shell.shutdown();

    // Import: the parts table's card on a new project.
    let tmp = Tmp::new("pv3-import");
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("p.ew").to_string_lossy(), "template": "builtin"}),
    );
    let parts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416/tables/parts.csv");
    done(&mut s, "parts.import", json!({"path": parts.to_string_lossy()}));
    assert!(s.preview_shown());
    assert_eq!(
        lines_with(&s, "card ")[0],
        "card ready \"parts.import\" created 29 skipped 0 errors 0"
    );
    let section = preview_section(&s);
    assert!(section[1].ends_with(" created 29 skipped 0"), "{section:#?}");
    s.shutdown();
}

/// The selection as alias-free entity UUID texts.
fn selection(shell: &ShellModel) -> Vec<String> {
    shell.selection().items.iter().map(|e| e.0.to_string()).collect()
}

/// One edit of -101F1's function text while `folio` is on show with `refs` selected.
fn edit_on(shell: &mut ShellModel, folio: &str, refs: Value, text: &str) -> Vec<String> {
    done(shell, "page.goto", json!({ "folio": folio }));
    done(shell, "select.set", json!({ "refs": refs }));
    let sel = selection(shell);
    assert!(!sel.is_empty());
    done(
        shell,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-101F1"}],
            "path": "function_text", "value": text}}),
    );
    sel
}

/// PV5 (FR-UI-023, D13): three edits on 101, 102 and 103, each with a selection; undo shows 103 with its selection,
/// a second 102 with its; redo returns to 102's step context; Undo to here walks back and restores the last step's
/// context; the panel's rows and its row cursor.
#[test]
fn pv5_undo_and_redo_restore_the_page_view_and_selection() {
    let tmp = Tmp::new("pv5");
    let mut shell = tmp.shell_in(Mode::Headless, true, "101");
    let s101 = edit_on(&mut shell, "101", json!(["tag:-101F1"]), "one");
    let s102 = edit_on(&mut shell, "102", json!(["tag:-102F1", "tag:-102F2"]), "two");
    done(&mut shell, "page.goto", json!({"folio": "103"}));
    let on103 = entity_points(shell.canvas().unwrap());
    let id103 = format!("id:{}", on103[0].0.0);
    let s103 = edit_on(&mut shell, "103", json!([id103]), "three");
    // Somewhere else, nothing selected.
    done(&mut shell, "page.goto", json!({"folio": "201"}));
    done(&mut shell, "select.set", json!({"refs": []}));
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(shell.current_folio(), Some("103"));
    assert_eq!(selection(&shell), s103);
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(shell.current_folio(), Some("102"));
    assert_eq!(selection(&shell), s102);
    // Away again, then redo: the redone step is 102's.
    done(&mut shell, "page.goto", json!({"folio": "201"}));
    done(&mut shell, "edit.redo", json!({}));
    assert_eq!(shell.current_folio(), Some("102"));
    assert_eq!(selection(&shell), s102);
    done(&mut shell, "edit.redo", json!({}));
    assert_eq!(shell.current_folio(), Some("103"));
    // The panel: newest first, the cursor at 3.
    done(&mut shell, "history.show", json!({}));
    let d = shell.describe();
    let rows: Vec<&str> = d
        .lines()
        .skip_while(|l| !l.starts_with("history "))
        .take(4)
        .collect();
    assert_eq!(rows[0], "history entries 3 cursor 3");
    assert!(rows[1].starts_with("  entry 3 now \""), "{rows:?}");
    assert!(rows[1].contains(" origin \"script "), "{rows:?}");
    assert!(rows[2].starts_with("  entry 2 done \""), "{rows:?}");
    assert!(rows[3].starts_with("  entry 1 done \""), "{rows:?}");
    // The row cursor: first (newest), then one down.
    done(&mut shell, "history.row", json!({"to": "first"}));
    done(&mut shell, "history.row", json!({"step": 1}));
    assert_eq!(shell.history_pane().row_cursor(), Some(1));
    assert!(
        shell.describe().contains("  entry 2 done "),
        "{}",
        shell.describe()
    );
    assert!(
        lines_with(&shell, "  entry 2 ")[0].ends_with(" cursor"),
        "the row cursor"
    );
    // Undo to here on entry 1: steps 3 and 2 are undone, step 2's context comes back.
    done(&mut shell, "page.goto", json!({"folio": "201"}));
    done(&mut shell, "history.undo_to", json!({"entry": 1}));
    assert_eq!(history_cursor(&mut shell), 1);
    assert_eq!(shell.current_folio(), Some("102"), "the last step crossed");
    assert_eq!(selection(&shell), s102);
    assert!(lines_with(&shell, "  entry 3 ")[0].starts_with("  entry 3 redo "));
    // One more undo: 101 with its selection.
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(shell.current_folio(), Some("101"));
    assert_eq!(selection(&shell), s101);
    // Undo to here forward: redo to entry 3.
    done(&mut shell, "history.undo_to", json!({"entry": 3}));
    assert_eq!(history_cursor(&mut shell), 3);
    assert_eq!(shell.current_folio(), Some("103"));
    // Show changes: the entry's record, labelled; Escape closes the popup.
    done(&mut shell, "history.changes", json!({"entry": 3}));
    let changes = lines_with(&shell, "  changes ");
    assert_eq!(changes, ["  changes entry 3 records 1"]);
    assert!(
        shell
            .describe()
            .contains("    entity changed \"device\" \"-101F1\" fields \"function_text\""),
        "{}",
        shell.describe()
    );
    done(&mut shell, "ui.dismiss", json!({}));
    assert!(lines_with(&shell, "  changes ").is_empty());
    // An entry beyond the history.
    match shell.dispatch("history.undo_to", &json!({"entry": 9})) {
        Outcome::Failed { message } => {
            assert_eq!(message, "There is no history entry 9; the history holds 3.")
        }
        other => panic!("{other:?}"),
    }
    shell.shutdown();
}

/// A new one-page project (the builtin template, folio 101) in `mode`.
fn small(tmp: &Tmp, mode: Mode) -> ShellModel {
    let mut s = ShellModel::new(mode, ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"));
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("small.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut s,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Power"}),
    );
    s
}

/// PV5: the markers of an export and a save sit after the entry of their revision, newest first; the panel fetches
/// one view per revision while open and none while closed (§9 item 5); moving its cursor calls nothing.
#[test]
fn pv5_markers_sit_at_their_revisions_and_the_counts_are_exact() {
    let tmp = Tmp::new("pv5-markers");
    let mut s = small(&tmp, Mode::Headless);
    let calls = s.session_calls();
    done(
        &mut s,
        "page.new",
        json!({"folio": "102", "type": "multi_line", "title": "Control"}),
    );
    assert_eq!(s.session_calls().history_view, calls.history_view, "closed: none");
    done(&mut s, "history.show", json!({}));
    let open = s.session_calls().history_view;
    assert_eq!(open, calls.history_view + 1, "one on opening");
    done(&mut s, "ui.describe", json!({}));
    done(&mut s, "history.row", json!({"to": "last"}));
    done(&mut s, "history.row", json!({"step": -1}));
    assert_eq!(s.session_calls().history_view, open, "the same revision: none");
    let target = tmp.0.join("out.pdf");
    done(
        &mut s,
        "file.export_pdf",
        json!({"path": target.to_string_lossy()}),
    );
    done(&mut s, "file.save", json!({}));
    done(
        &mut s,
        "page.new",
        json!({"folio": "103", "type": "multi_line", "title": "Field"}),
    );
    assert_eq!(s.session_calls().history_view, open + 1, "one per new revision");
    let d = s.describe();
    let rows: Vec<&str> = d
        .lines()
        .skip_while(|l| !l.starts_with("history "))
        .take_while(|l| l.starts_with("history ") || l.starts_with("  "))
        .collect();
    assert_eq!(rows[0], "history entries 3 cursor 3");
    assert!(rows[1].starts_with("  entry 3 now "), "{rows:#?}");
    assert_eq!(rows[2], "  marker \"Saved\"");
    assert_eq!(rows[3], "  marker \"Exported PDF to out.pdf\"");
    assert!(rows[4].starts_with("  entry 2 done "), "{rows:#?}");
    assert!(rows[5].starts_with("  entry 1 done "), "{rows:#?}");
    assert!(rows[4].contains(" pages \"102\""), "{rows:#?}");
    done(&mut s, "history.show", json!({"on": false}));
    done(
        &mut s,
        "page.new",
        json!({"folio": "104", "type": "multi_line", "title": "Spare"}),
    );
    assert_eq!(s.session_calls().history_view, open + 1, "closed: none");
    assert_eq!(
        refused(&mut s, "history.row", json!({"step": 1})),
        "reason-history-closed"
    );
    s.shutdown();
}

const LOGO_X: &[u8] = br#"<svg viewBox="0 0 8 2"><rect width="8" height="2"/></svg>"#;
const LOGO_Y: &[u8] = br#"<svg viewBox="0 0 3 3"><circle r="1"/></svg>"#;

fn set_logo(s: &mut ShellModel, bytes: &[u8]) {
    done(
        s,
        "command.run",
        json!({"cmd": "project.company.set", "params": {"logo": ew_viewmodel::ew_api::base64_encode(bytes)}}),
    );
}

/// PV6 (FT-417): the dialog lists `purge_preview`'s files; Purge commits the command's marker from the GUI origin; an
/// empty plan refuses; `confirm` without the open dialog refuses `reason-no-purge-dialog`; headless the action
/// refuses `reason-human-only`.
#[test]
fn pv6_the_purge_step() {
    let tmp = Tmp::new("pv6");
    let dir = tmp.0.join("small.ew");
    let mut s = small(&tmp, Mode::Gui);
    set_logo(&mut s, LOGO_X);
    done(&mut s, "file.save", json!({}));
    set_logo(&mut s, LOGO_Y);
    done(&mut s, "file.save", json!({}));
    // The undo history still names X: nothing to purge in this session.
    assert_eq!(
        refused(&mut s, "project.purge_assets", json!({})),
        "reason-nothing-to-purge"
    );
    done(&mut s, "file.close", json!({}));
    done(&mut s, "file.open", json!({"dir": dir.to_string_lossy()}));
    // `confirm` without the open confirmation purges nothing (R1-5 of cycle 31).
    let unused: Vec<_> = std::fs::read_dir(dir.join("assets"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(
        refused(&mut s, "project.purge_assets", json!({"confirm": true})),
        "reason-no-purge-dialog"
    );
    assert_eq!(
        ew_viewmodel::strings::text("reason-no-purge-dialog"),
        "Open Purge unused images... first"
    );
    assert!(unused.iter().all(|f| f.exists()), "nothing was purged");
    done(&mut s, "project.purge_assets", json!({}));
    let Some(Dialog::ConfirmPurge { files, assets }) = s.dialog().cloned() else {
        panic!("{:?}", s.dialog());
    };
    assert_eq!((files.len(), assets.len()), (1, 1));
    assert!(files[0].ends_with(".svg") && !files[0].contains('/'), "{files:?}");
    let file = dir.join("assets").join(&files[0]);
    assert!(file.exists());
    let d = s.describe();
    assert!(
        d.contains(&format!(
            "dialog \"Purge unused images\"\n  label \"These files are deleted and do not come back by undo.\"\n  item \"{}\"\n  button \"Purge\"\n  button \"Cancel\"",
            files[0]
        )),
        "{d}"
    );
    done(&mut s, "project.purge_assets", json!({"confirm": true}));
    assert!(s.dialog().is_none());
    assert!(!file.exists(), "the file is deleted");
    done(&mut s, "history.show", json!({}));
    let entry = lines_with(&s, "  entry 1 ");
    assert_eq!(entry.len(), 1);
    assert!(
        entry[0].starts_with("  entry 1 now \"project.assets.purge\" origin \"GUI\""),
        "{entry:?}"
    );
    assert_eq!(
        refused(&mut s, "project.purge_assets", json!({})),
        "reason-nothing-to-purge"
    );
    s.shutdown();
    let tmp = Tmp::new("pv6-headless");
    let mut h = small(&tmp, Mode::Headless);
    assert_eq!(
        refused(&mut h, "project.purge_assets", json!({})),
        "reason-human-only"
    );
    h.shutdown();
}
