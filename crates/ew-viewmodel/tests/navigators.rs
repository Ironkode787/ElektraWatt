// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The navigators of WP-S2.06 part 1 on Golden A (acceptance checks NV3, NV5, NV7, NV8 and NV10's model half): the
//! Pages lines byte-equal to the as-built text, the filter as a superset of the as-built Pages filter, the Devices
//! navigator with selection synchronisation both ways and the chip line, multi-selection and reorder of pages with the
//! card, the New page pre-fill (FT-461), the describe window and the transcript's `focus` values.

mod common;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use ew_viewmodel::navigator::{NavMode, PageNavigator, RowKind};
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::{Value, json};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
const EXPECTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/expected");

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-vm-navigators-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    /// A headless shell with a copy of Golden A open (read-only unless `writable`).
    fn shell(&self, writable: bool) -> ShellModel {
        let dir = self.0.join("J-E-2416.ew");
        if !dir.exists() {
            copy_dir(Path::new(GOLDEN_A), &dir);
        }
        let mut shell = ShellModel::new(
            Mode::Headless,
            ShellConfig::in_dir(&self.0.join("config"), "0.0.0-test"),
        );
        done(
            &mut shell,
            "file.open",
            json!({"dir": dir.to_string_lossy(), "read_only": !writable}),
        );
        shell
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[track_caller]
fn done(shell: &mut ShellModel, action: &str, params: Value) {
    assert_eq!(
        shell.dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

/// The Pages navigator's lines of the describe text: its header and its row lines.
fn pages_lines(shell: &ShellModel) -> String {
    let d = shell.describe();
    let mut out = Vec::new();
    let mut inside = false;
    for l in d.lines() {
        if l.starts_with("navigator \"Pages\"") {
            inside = true;
        } else if inside && !l.starts_with("  row ") {
            break;
        }
        if inside {
            out.push(l);
        }
    }
    out.join("\n")
}

/// NV10 (D5): the Pages lines are byte-equal to the as-built text (WP-S2.03 §6.10) in the views and states a recording
/// reaches: the section view, a collapsed section, a filter, the flat view, the type order and a moved cursor.
#[test]
fn the_pages_lines_are_the_as_built_lines() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell(false);
    let mut text = String::new();
    let mut take = |shell: &ShellModel, what: &str| {
        text.push_str(&format!("## {what}\n{}\n", pages_lines(shell)));
    };
    take(&shell, "section view");
    done(&mut shell, "pages.toggle", json!({"key": "s:1"}));
    take(&shell, "section 1 collapsed");
    done(&mut shell, "pages.toggle", json!({"key": "s:1"}));
    done(&mut shell, "pages.filter", json!({"text": "power supply"}));
    take(&shell, "filter power supply");
    done(&mut shell, "pages.filter", json!({"text": ""}));
    done(&mut shell, "pages.mode", json!({"mode": "flat"}));
    take(&shell, "flat view");
    done(&mut shell, "pages.sort", json!({"by": "type"}));
    take(&shell, "flat view by type");
    done(&mut shell, "pages.sort", json!({"by": "folio"}));
    done(&mut shell, "pages.mode", json!({"mode": "section"}));
    done(&mut shell, "page.goto", json!({"folio": "103"}));
    done(&mut shell, "pages.row", json!({"step": 1}));
    take(&shell, "103 shown, cursor moved down");
    let path = Path::new(EXPECTED).join("navigator_pages.txt");
    if common::bless() {
        std::fs::write(&path, &text).unwrap();
    }
    let expected = std::fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
    assert_eq!(text, expected);
}

fn state(shell: &ShellModel) -> Value {
    serde_json::from_str(&ew_viewmodel::script::state_json(shell)).unwrap()
}

#[track_caller]
fn failed(shell: &mut ShellModel, action: &str, params: Value) -> String {
    match shell.dispatch(action, &params) {
        Outcome::Failed { message } => message,
        other => panic!("{action}: {other:?}"),
    }
}

/// The key of the Pages row of folio `folio` in the current view.
fn page_key(shell: &ShellModel, folio: &str) -> String {
    let rows = shell.page_rows();
    shell
        .navigator_rows()
        .into_iter()
        .find(|r| matches!(r.kind, RowKind::Page(i) if rows[i].folio == folio))
        .map(|r| r.key)
        .unwrap_or_else(|| panic!("no Pages row {folio}"))
}

/// The key of the row labelled `label` with role `role` in an open navigator.
fn row_key(shell: &ShellModel, kind: NavKind, role: NavRole, label: &str) -> String {
    let t = shell
        .nav(kind)
        .and_then(|n| n.table())
        .expect("an open navigator with its table");
    t.nodes
        .iter()
        .find(|n| n.role == role && n.label == label)
        .map(|n| n.key.clone())
        .unwrap_or_else(|| panic!("no {role:?} row {label}"))
}

/// The lines of the describe text that start with `prefix`.
fn lines_with(shell: &ShellModel, prefix: &str) -> Vec<String> {
    shell
        .describe()
        .lines()
        .filter(|l| l.starts_with(prefix))
        .map(str::to_owned)
        .collect()
}

/// The row lines under the header of the navigator titled `title`.
fn nav_rows(shell: &ShellModel, title: &str) -> Vec<String> {
    let d = shell.describe();
    let mut out = Vec::new();
    let mut inside = false;
    for l in d.lines() {
        if l.starts_with(&format!("navigator \"{title}\"")) {
            inside = true;
            continue;
        }
        if inside && !l.starts_with("  row ") {
            break;
        }
        if inside {
            out.push(l.to_owned());
        }
    }
    out
}

/// The folios of the pages of section 1 in page order.
fn section1(shell: &ShellModel) -> Vec<String> {
    shell
        .page_rows()
        .iter()
        .filter(|r| r.section.as_ref().is_some_and(|(n, _)| *n == 1) && !r.generated)
        .map(|r| format!("{} {}", r.folio, r.title))
        .collect()
}

/// NV3 (D6): the Pages filter keeps every as-built match (a folio or title substring, case-folded) for every folio and
/// title substring of Golden A, and the ancestors of a match stay shown and expanded.
#[test]
fn the_pages_filter_is_a_superset_of_the_as_built_filter() {
    let tmp = Tmp::new();
    let shell = tmp.shell(false);
    let rows = shell.page_rows().to_vec();
    let mut queries = std::collections::BTreeSet::new();
    for r in &rows {
        for text in [&r.folio, &r.title] {
            let chars: Vec<char> = text.chars().collect();
            for a in 0..chars.len() {
                for b in a + 1..=chars.len() {
                    queries.insert(chars[a..b].iter().collect::<String>());
                }
            }
        }
    }
    let mut nav = PageNavigator::new();
    nav.mode = NavMode::Flat;
    for q in &queries {
        if q.trim().is_empty() {
            continue;
        }
        nav.filter = q.clone();
        let shown: Vec<usize> = nav
            .visible(&rows)
            .iter()
            .filter_map(|r| match r.kind {
                RowKind::Page(i) => Some(i),
                RowKind::Node { .. } => None,
            })
            .collect();
        let f = q.to_lowercase();
        for (i, r) in rows.iter().enumerate() {
            if r.folio.to_lowercase().contains(&f) || r.title.to_lowercase().contains(&f) {
                assert!(shown.contains(&i), "filter {q:?} lost page {}", r.folio);
            }
        }
    }
    // In the section view a match keeps its section, expanded even when it was collapsed.
    nav.mode = NavMode::Section;
    nav.filter = "control current distribution".into();
    nav.toggle("s:1");
    let v = nav.visible(&rows);
    assert_eq!(v[0].key, "s:1");
    assert!(v[0].expanded);
    assert!(
        v.iter()
            .any(|r| matches!(r.kind, RowKind::Page(i) if rows[i].folio == "103"))
    );
}

/// NV10 and NV11's model half (§6.10, §9 item 4): the Devices navigator prints a header with its kind, view, sort,
/// filter, row count and window, and at most 50 rows around the cursor; the `focus` member is `navigator.devices`;
/// opening it and a new revision make one table call, filtering, sorting, the view, moving and marking none, and a
/// closed navigator none.
#[test]
fn the_devices_navigator_describes_a_window_and_counts_its_table_calls() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell(true);
    let calls = shell.session_calls().navigator_table;
    done(&mut shell, "navigator.show", json!({"kind": "devices"}));
    assert_eq!(
        shell.session_calls().navigator_table,
        calls + 1,
        "one table on opening"
    );
    assert_eq!(state(&shell)["focus"], "navigator.devices");
    assert!(shell.describe().contains("\nfocus navigator kind devices\n"));
    let n = shell.shown_rows(NavKind::Devices).len();
    assert!(n > 50, "Golden A has {n} rows in the Devices tree");
    assert_eq!(
        lines_with(&shell, "navigator \"Devices\""),
        [format!(
            "navigator \"Devices\" kind devices view tree sort \"Tag\" ascending filter \"\" rows {n} showing 1 to 50"
        )]
    );
    let lines = nav_rows(&shell, "Devices");
    assert_eq!(lines.len(), 50);
    assert!(
        lines[0].starts_with("  row level 1 function \"=P01\" expanded"),
        "{}",
        lines[0]
    );
    assert!(lines[0].ends_with(" cursor"));
    // The window follows the cursor: at the last row it shows the last 50.
    done(
        &mut shell,
        "navigator.row",
        json!({"kind": "devices", "to": "last"}),
    );
    let header = lines_with(&shell, "navigator \"Devices\"");
    assert!(
        header[0].ends_with(&format!("rows {n} showing {} to {n}", n - 49)),
        "{}",
        header[0]
    );
    assert!(nav_rows(&shell, "Devices").last().unwrap().ends_with(" cursor"));
    // A row line: level, role, label, badges, expansion, mark, cursor.
    let k = row_key(&shell, NavKind::Devices, NavRole::Device, "-201U3");
    done(&mut shell, "navigator.row", json!({"kind": "devices", "key": k}));
    let line = nav_rows(&shell, "Devices")
        .into_iter()
        .find(|l| l.contains("device \"-201U3\""))
        .unwrap();
    // -201U3 is a child device of -201U1 (level 3), so its row is at level 4.
    assert_eq!(line, "  row level 4 device \"-201U3\" expanded cursor");
    // Filter, sort, view, expansion, cursor, marks and type-ahead: no table call.
    let calls = shell.session_calls().navigator_table;
    done(
        &mut shell,
        "navigator.toggle",
        json!({"kind": "devices", "key": k}),
    );
    done(&mut shell, "navigator.mark", json!({"kind": "devices", "key": k}));
    done(
        &mut shell,
        "navigator.jump",
        json!({"kind": "devices", "prefix": "-1"}),
    );
    done(
        &mut shell,
        "navigator.filter",
        json!({"kind": "devices", "text": "201u"}),
    );
    done(
        &mut shell,
        "navigator.sort",
        json!({"kind": "devices", "by": "label", "descending": true}),
    );
    done(
        &mut shell,
        "navigator.view",
        json!({"kind": "devices", "view": "flat"}),
    );
    assert_eq!(shell.session_calls().navigator_table, calls);
    let header = lines_with(&shell, "navigator \"Devices\"");
    assert!(
        header[0].starts_with(
            "navigator \"Devices\" kind devices view flat sort \"Tag\" descending filter \"201u\""
        ),
        "{}",
        header[0]
    );
    // A new revision: one call; a closed navigator: none.
    done(
        &mut shell,
        "pages.reorder",
        json!({"pages": ["106"], "before": "103"}),
    );
    done(&mut shell, "navigator.apply", json!({}));
    assert_eq!(shell.session_calls().navigator_table, calls + 1);
    done(
        &mut shell,
        "navigator.show",
        json!({"kind": "devices", "on": false}),
    );
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(shell.session_calls().navigator_table, calls + 1);
    assert!(lines_with(&shell, "navigator \"Devices\"").is_empty());
    // Unknown kinds, keys and views refuse with the valid values.
    let m = failed(
        &mut shell,
        "navigator.view",
        json!({"kind": "wires", "view": "tree"}),
    );
    assert!(m.contains("pages, devices, connections"), "{m}");
    let m = failed(
        &mut shell,
        "navigator.view",
        json!({"kind": "devices", "view": "tree"}),
    );
    assert!(m.contains("not open"), "{m}");
    shell.shutdown();
}

/// NV5 (FR-UI-007 as amended by D16; §6.5): with folio 102 shown, marking -201U3 in Devices selects nothing on 102 but
/// the device's record (the property grid shows -201U3) and shows the chips 201-E4 and 203-B4; Enter opens 201 with
/// its representation selected (one back-history entry); a selection made elsewhere marks and reveals its row, the
/// focus stays where it is, and no change is applied twice.
#[test]
fn devices_and_the_canvas_keep_the_selection_in_step() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell(false);
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    done(&mut shell, "navigator.show", json!({"kind": "devices"}));
    let k = row_key(&shell, NavKind::Devices, NavRole::Device, "-201U3");
    done(
        &mut shell,
        "navigator.select",
        json!({"kind": "devices", "keys": [k]}),
    );
    assert_eq!(shell.current_folio(), Some("102"), "the page is never switched");
    let device = shell
        .nav(NavKind::Devices)
        .unwrap()
        .node_of(&k)
        .unwrap()
        .1
        .records[0];
    assert_eq!(
        shell.selection().items.iter().map(|e| e.0).collect::<Vec<_>>(),
        [device],
        "nothing of -201U3 is drawn on 102: the selection is its record"
    );
    let chips: Vec<String> = shell.chips().iter().map(|c| c.text()).collect();
    assert_eq!(chips, ["201-E4", "203-B4"]);
    assert!(
        shell.describe().contains("\n  elsewhere \"201-E4\" \"203-B4\"\n"),
        "the canvas section's chip line"
    );
    assert_eq!(state(&shell)["properties"]["title"], "-201U3");
    let generation = shell.selection().generation;
    // Enter opens 201 with the representation selected, as a jump.
    let back = state(&shell)["tabs"]["groups"][0][0]["back"].as_u64().unwrap();
    done(&mut shell, "navigator.open", json!({"kind": "devices", "key": k}));
    assert_eq!(shell.current_folio(), Some("201"));
    assert_eq!(
        state(&shell)["tabs"]["groups"][0][0]["back"].as_u64().unwrap(),
        back + 1
    );
    let on201 = shell.selection().items.clone();
    assert_eq!(on201.len(), 1);
    assert!(shell.chips().is_empty(), "a page switch clears the chips");
    assert!(shell.selection().generation > generation);
    // A selection made elsewhere (a canvas click, here `select.set`) marks and reveals the representation's row.
    done(
        &mut shell,
        "navigator.toggle",
        json!({"kind": "devices", "key": k}),
    );
    let rep203 = row_key(&shell, NavKind::Devices, NavRole::Representation, "203-B4");
    let rep_id = shell
        .nav(NavKind::Devices)
        .unwrap()
        .node_of(&rep203)
        .unwrap()
        .1
        .records[0];
    done(&mut shell, "ui.focus", json!({"area": "canvas"}));
    done(&mut shell, "select.set", json!({"ids": [rep_id.to_string()]}));
    let nav = shell.nav(NavKind::Devices).unwrap();
    assert_eq!(
        nav.marked().iter().cloned().collect::<Vec<_>>(),
        std::slice::from_ref(&rep203)
    );
    assert_eq!(
        shell.nav_cursor(NavKind::Devices).as_deref(),
        Some(rep203.as_str())
    );
    assert!(
        shell.shown_rows(NavKind::Devices).iter().any(|r| r.key == rep203),
        "the collapsed device was expanded to reveal the row"
    );
    assert_eq!(state(&shell)["focus"], "canvas", "the focus stays on the canvas");
    // The generation guard: the next dispatch applies nothing again (the selection stays as it is).
    let g = shell.selection().generation;
    done(&mut shell, "ui.describe", json!({}));
    assert_eq!(shell.selection().generation, g);
    assert_eq!(shell.nav(NavKind::Devices).unwrap().marked().len(), 1);
    shell.shutdown();
}

/// NV7 (FR-PAG-024's rest, FT-360; D8): Shift+Down marks 103 and 106; `pages.reorder` of 106 before 103 opens the
/// card "106 → 103, 103 → 106" with its tag and cross-reference counts; Apply commits one `page.reorder`, one undo
/// step; a generated page refuses; Alt+Up on 106 opens the same card.
#[test]
fn pages_are_marked_and_reordered_through_one_previewed_command() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell(true);
    let before = section1(&shell);
    let k103 = page_key(&shell, "103");
    done(&mut shell, "pages.row", json!({ "key": k103 }));
    done(
        &mut shell,
        "navigator.row",
        json!({"kind": "pages", "step": 1, "extend": true}),
    );
    let marked: Vec<String> = shell.navigator().marked().iter().cloned().collect();
    let mut want = vec![page_key(&shell, "103"), page_key(&shell, "106")];
    want.sort();
    assert_eq!(marked, want, "Shift+Down marks both");
    assert_eq!(shell.selection().items.len(), 2, "the selection is the two pages");
    let lines = lines_with(&shell, "  row level 2 page \"10");
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("  row level 2 page \"103\"") && l.contains(" selected"))
    );
    // A generated page refuses.
    let m = failed(
        &mut shell,
        "pages.reorder",
        json!({"pages": ["2"], "before": "1"}),
    );
    assert_eq!(m, "A generated page has no folio of its own.");
    // The drag of 106 before 103: the card.
    done(
        &mut shell,
        "pages.reorder",
        json!({"pages": ["106"], "before": "103"}),
    );
    let card = state(&shell)["card"].clone();
    assert_eq!(card["command"], "page.reorder");
    assert_eq!(card["origin"], "reorder");
    assert_eq!(card["changes"], 2);
    let line = lines_with(&shell, "card ready \"page.reorder\"");
    assert_eq!(line.len(), 1);
    assert!(
        line[0].ends_with(" reorder \"106 → 103\" \"103 → 106\""),
        "{}",
        line[0]
    );
    assert_eq!(section1(&shell), before, "nothing changes before Apply");
    let undo_before = shell.can_undo();
    assert!(!undo_before);
    done(&mut shell, "navigator.apply", json!({}));
    assert!(state(&shell)["card"].is_null());
    let after = section1(&shell);
    let titles = |v: &[String]| {
        v.iter()
            .map(|s| s.split_once(' ').unwrap().1.to_owned())
            .collect::<Vec<_>>()
    };
    let folios = |v: &[String]| {
        v.iter()
            .map(|s| s.split_once(' ').unwrap().0.to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(folios(&after), folios(&before), "the folio keys stay in place");
    let (b, a) = (titles(&before), titles(&after));
    let i = folios(&before).iter().position(|f| f == "103").unwrap();
    assert_eq!(
        (a[i].clone(), a[i + 1].clone()),
        (b[i + 1].clone(), b[i].clone()),
        "103 and 106 swapped"
    );
    // One undo step restores every folio.
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(section1(&shell), before);
    assert!(!shell.can_undo());
    // Alt+Up on 106 gives the same card.
    let k106 = page_key(&shell, "106");
    done(
        &mut shell,
        "navigator.select",
        json!({"kind": "pages", "keys": [k106]}),
    );
    done(&mut shell, "pages.move_up", json!({}));
    let line = lines_with(&shell, "card ready \"page.reorder\"");
    assert!(
        line[0].ends_with(" reorder \"106 → 103\" \"103 → 106\""),
        "{}",
        line[0]
    );
    done(&mut shell, "preview.cancel", json!({}));
    shell.shutdown();
}

/// NV8 (FT-461): Page ▸ New page proposes the next free folio after a page record and the section of the marked
/// section node, else of the current page.
#[test]
fn new_page_proposes_the_next_free_folio_and_the_section() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell(false);
    done(&mut shell, "page.goto", json!({"folio": "103"}));
    assert_eq!(shell.new_page_proposal(), ("104".to_owned(), "1".to_owned()));
    done(&mut shell, "page.goto", json!({"folio": "1"}));
    // 2 is the generated table of contents and 3 a page record.
    assert_eq!(shell.new_page_proposal().0, "4");
    done(&mut shell, "page.goto", json!({"folio": "2"}));
    assert_eq!(
        shell.new_page_proposal().0,
        "",
        "a generated page proposes nothing"
    );
    done(&mut shell, "page.goto", json!({"folio": "103"}));
    done(
        &mut shell,
        "navigator.select",
        json!({"kind": "pages", "keys": ["s:2"]}),
    );
    assert_eq!(shell.new_page_proposal(), ("104".to_owned(), "2".to_owned()));
    shell.shutdown();
}

/// NV10 (§6.10): every navigator opens from View ▸ Navigators, and the describe texts reach the navigator words (all but
/// `aux`, which a badge prints quoted, and `pages`, which the Pages header leaves out).
#[test]
fn every_navigator_opens_and_the_navigator_words_are_reached() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell(true);
    let mut texts = Vec::new();
    for k in NavKind::ALL {
        done(&mut shell, "navigator.show", json!({"kind": k.word()}));
        assert!(shell.navigator_open(k), "{}", k.word());
    }
    let menu: Vec<String> = shell
        .describe()
        .lines()
        .filter(|l| l.contains("navigator.show"))
        .map(str::to_owned)
        .collect();
    assert_eq!(menu.len(), 11, "{menu:?}");
    assert!(menu.iter().all(|l| l.contains("(checked)")), "{menu:?}");
    done(
        &mut shell,
        "navigator.sort",
        json!({"kind": "devices", "by": "label", "descending": true}),
    );
    texts.push(shell.describe());
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    let k = row_key(&shell, NavKind::Devices, NavRole::Device, "-201U3");
    done(
        &mut shell,
        "navigator.select",
        json!({"kind": "devices", "keys": [k]}),
    );
    texts.push(shell.describe());
    done(
        &mut shell,
        "pages.reorder",
        json!({"pages": ["106"], "before": "103"}),
    );
    texts.push(shell.describe());
    let words: std::collections::BTreeSet<&str> = texts
        .iter()
        .flat_map(|t| t.lines())
        .flat_map(|l| {
            // The quoted texts are user text.
            let mut out = Vec::new();
            let mut quoted = false;
            for part in l.split('"') {
                if !quoted {
                    out.extend(part.split_whitespace());
                }
                quoted = !quoted;
            }
            out
        })
        .collect();
    let missing: Vec<&&str> = ew_viewmodel::describe::NAV_WORDS
        .iter()
        // `aux` is printed quoted (a badge), and `pages` is the kind word the Pages header leaves out (D5).
        .filter(|w| !words.contains(**w) && !matches!(**w, "aux" | "pages"))
        .collect();
    assert!(missing.is_empty(), "{missing:?}");
    for w in ew_viewmodel::describe::NAV_WORDS {
        assert!(ew_viewmodel::describe::VOCABULARY.contains(w), "{w}");
    }
    assert_eq!(ew_viewmodel::describe::NAV_WORDS.len(), 18);
    shell.shutdown();
}

/// NV4 (NFR-PERF-15, structural; §11): a synthetic table of 100,000 nodes built in the test (no `Session`): the tree and
/// the flat list, a filter keystroke and a descending sort give the right rows. Timings are not measured (owner's
/// rule, ID-512).
#[test]
fn a_hundred_thousand_nodes_flatten_filter_and_sort() {
    use ew_api::output::{NavColumn, NavNode, NavTable, nav_natural_key, nav_tokens};
    use ew_viewmodel::navigators::{NavView, Query, SortBy, VisRow, visible_rows};
    let node = |key: String, parent: Option<u32>, role: NavRole, label: String| {
        let mut tokens = nav_tokens(&label);
        tokens.sort();
        tokens.dedup();
        NavNode {
            natural: vec![nav_natural_key(&label)],
            key,
            parent,
            role,
            label,
            badges: Vec::new(),
            records: Vec::new(),
            kinds: Vec::new(),
            drawables: Vec::new(),
            open: None,
            cells: Vec::new(),
            table_row: role == NavRole::Device,
            tokens,
            facets: Vec::new(),
        }
    };
    // 1,000 groups of 99 devices: 100,000 nodes; the devices in reverse natural order within a group.
    let mut nodes = Vec::with_capacity(100_000);
    for g in 0..1000u32 {
        let gi = u32::try_from(nodes.len()).unwrap();
        nodes.push(node(
            format!("function:=G{g}"),
            None,
            NavRole::Function,
            format!("=G{g}"),
        ));
        for d in (0..99u32).rev() {
            nodes.push(node(
                format!("function:=G{g}/device:{g}-{d}"),
                Some(gi),
                NavRole::Device,
                format!("-{g}K{d}"),
            ));
        }
    }
    assert_eq!(nodes.len(), 100_000);
    let table = NavTable {
        kind: NavKind::Devices,
        rev: 1,
        columns: Vec::<NavColumn>::new(),
        nodes,
        index: std::collections::BTreeMap::new(),
    };
    let none = std::collections::BTreeSet::new();
    let label = |r: &VisRow| table.nodes[r.node as usize].label.clone();
    let tree = visible_rows(
        &table,
        NavView::Tree,
        &SortBy::Label,
        false,
        &Query::default(),
        &none,
    );
    assert_eq!(tree.len(), 100_000);
    // Natural order: =G2 before =G10, -0K2 before -0K10.
    assert_eq!(label(&tree[0]), "=G0");
    assert_eq!(label(&tree[1]), "-0K0");
    assert_eq!(label(&tree[3]), "-0K2");
    assert_eq!(label(&tree[100]), "=G1");
    assert_eq!(label(&tree[200]), "=G2");
    // A filter keystroke: `-123k45` (tokens `123`, `k`, `45`, each a prefix of a node token) keeps -123K45 under =G123.
    let hit = visible_rows(
        &table,
        NavView::Tree,
        &SortBy::Label,
        false,
        &Query::parse("-123k45"),
        &none,
    );
    let labels: Vec<String> = hit.iter().map(label).collect();
    assert_eq!(labels, ["=G123", "-123K45"]);
    // One keystroke more on a shorter query: `-12k4` keeps -12K4 and -12K40 to -12K49 under =G12, and the groups whose
    // number starts with 4 that have a device starting with 12 (=G4: -4K12; =G40 to =G49: -4nK12): the tokens are
    // matched in any order.
    let hit = visible_rows(
        &table,
        NavView::Tree,
        &SortBy::Label,
        false,
        &Query::parse("-12k4"),
        &none,
    );
    let labels: Vec<String> = hit.iter().map(label).collect();
    assert_eq!(labels[0], "=G4");
    assert!(labels.contains(&"-12K49".to_owned()));
    assert!(labels.contains(&"-49K12".to_owned()));
    // The flat list, sorted descending: 99,000 devices, the last group's last device first.
    let flat = visible_rows(
        &table,
        NavView::Flat,
        &SortBy::Label,
        true,
        &Query::default(),
        &none,
    );
    assert_eq!(flat.len(), 99_000);
    assert_eq!(label(&flat[0]), "-999K98");
    assert_eq!(label(&flat[98_999]), "-0K0");
}
