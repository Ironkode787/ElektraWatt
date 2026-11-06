// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tutorials of the user guide as scripted tests (WP-S7.05 D10, D12, NFR-DOC-03): each tutorial's script
//! (`docs/book/tutorial-scripts/<page>.ewui`) runs headless through the built binary on a fresh configuration folder,
//! every step is `done` and the end state is the one its page states; and the page lint (T6) checks that the menu
//! paths and keys a page names are the window's own.
//!
//! **The page lint.** A page writes every menu path in bold (`**File ▸ Save**`), a key in brackets right after it
//! (`(`Ctrl+S`)`). The paths of a page's numbered steps, in order, are the script's menu steps, in order: the steps
//! whose action and parameters are a menu item; each must be that item's en-GB label path, and its key that action's
//! default binding. Every other path of a page, and every path of a page without a script, must be the label path of a
//! menu item or of a menu or group above one.

use crate::support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use ew_ui::ew_viewmodel::actions::{ACTIONS, MenuEntry, menu_model};
use ew_ui::ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::{Value, json};
use support::{CONTENT, Tmp, USER_DIRS};

/// The book's directory.
const BOOK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/book");

/// The path of the `elektrawatt` binary under test (nextest sets it at run time; see `tests/headless.rs`).
// Allowed: the program boundary of a test — nextest's binary path is read once, never persisted or printed.
#[allow(clippy::disallowed_methods)]
fn bin_exe() -> PathBuf {
    std::env::var_os("NEXTEST_BIN_EXE_elektrawatt")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_elektrawatt"))) // bin-exe-fallback
}

/// A page of the book's source.
fn page(rel: &str) -> String {
    let path = Path::new(BOOK).join("src").join(rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .replace("\r\n", "\n")
}

/// The steps of a tutorial script, without its header.
fn script(name: &str) -> Vec<(String, Value)> {
    let path = Path::new(BOOK)
        .join("tutorial-scripts")
        .join(format!("{name}.ewui"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .skip(1)
        .map(|l| {
            let v: Value = serde_json::from_str(l).unwrap();
            (
                v["action"].as_str().unwrap().to_owned(),
                v.get("params").cloned().unwrap_or_else(|| json!({})),
            )
        })
        .collect()
}

/// Runs the tutorial script `name` headless in `tmp` (its working folder, so the script's relative paths land there),
/// with a fresh configuration folder and the user folders inside `tmp`.
fn run_tutorial(tmp: &Tmp, name: &str) -> Output {
    run_tutorial_with(tmp, name, None)
}

/// [`run_tutorial`] with the sample project `sample` (Help ▸ Tutorials copies it; absent: none). No data directory is
/// given (`EW_DATA_DIR`): the tutorials run on the compiled-in starter set alone, as a new installation does (FT-1401).
fn run_tutorial_with(tmp: &Tmp, name: &str, sample: Option<&str>) -> Output {
    let path = Path::new(BOOK)
        .join("tutorial-scripts")
        .join(format!("{name}.ewui"));
    let mut cmd = Command::new(bin_exe());
    cmd.args(["--headless", "--script"])
        .arg(&path)
        .current_dir(tmp.path(""));
    for d in USER_DIRS {
        let dir = tmp.path("home").join(d.to_ascii_lowercase());
        std::fs::create_dir_all(&dir).unwrap();
        cmd.env(d, dir);
    }
    match sample {
        Some(dir) => cmd.env("EW_SAMPLE_DIR", dir),
        None => cmd.env_remove("EW_SAMPLE_DIR"),
    };
    cmd.env("EW_CONFIG_DIR", tmp.path(&format!("config-{name}")))
        .env("EW_CONTENT_DIR", CONTENT)
        .env_remove("EW_DATA_DIR")
        .env_remove("EW_DOCS_DIR")
        .env_remove("SOURCE_DATE_EPOCH")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

/// The transcript of a run in which every step was done.
#[track_caller]
fn all_done(out: &Output) -> String {
    let text = String::from_utf8(out.stdout.clone()).unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}\n{text}",
        String::from_utf8_lossy(&out.stderr)
    );
    for line in text.lines().skip(1) {
        let step: Value = serde_json::from_str(line).unwrap();
        assert_eq!(step["outcome"], "done", "{step}");
    }
    text
}

/// The `describe` texts of the transcript's `ui.describe` steps, in order.
fn describes(transcript: &str) -> Vec<String> {
    transcript
        .lines()
        .skip(1)
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter_map(|s| s["describe"].as_str().map(str::to_owned))
        .collect()
}

/// The `describe` text of the transcript's last `ui.describe` step.
fn last_describe(transcript: &str) -> String {
    describes(transcript).pop().expect("a ui.describe step")
}

/// The lines of a `describe` text that start with one of `starts`, trimmed.
fn lines_of(describe: &str, starts: &[&str]) -> Vec<String> {
    describe
        .lines()
        .map(str::trim)
        .filter(|l| starts.iter().any(|s| l.starts_with(s)))
        .map(str::to_owned)
        .collect()
}

// -------------------------------------------------------------------------------------------------------------------
// The page lint (T6)

/// A menu item of the window: its action, parameters, en-GB label path and the action's default bindings.
struct Item {
    action: String,
    params: Value,
    path: String,
    keys: Vec<String>,
}

fn walk(entry: &MenuEntry, path: &mut Vec<String>, out: &mut Vec<Item>) {
    let mut item = |i: &ew_ui::ew_viewmodel::actions::ActionItem, path: &[String]| {
        let keys = ACTIONS
            .iter()
            .find(|a| a.id == i.id)
            .map(|a| a.shortcut.iter().map(|k| (*k).to_owned()).collect())
            .unwrap_or_default();
        out.push(Item {
            action: i.id.to_owned(),
            params: i.params.clone(),
            path: format!("{} ▸ {}", path.join(" ▸ "), i.label),
            keys,
        });
    };
    match entry {
        MenuEntry::Separator => {}
        MenuEntry::Item(i) => item(i, path),
        MenuEntry::Submenu { label, items, .. } => {
            path.push(label.clone());
            for i in items {
                item(i, path);
            }
            path.pop();
        }
        MenuEntry::Group { label, entries, .. } => {
            path.push(label.clone());
            for e in entries {
                walk(e, path, out);
            }
            path.pop();
        }
    }
}

/// The window's menu items, read from the menus of a new project in a fresh configuration.
fn items(tmp: &Tmp) -> Vec<Item> {
    items_in(tmp, "en")
}

/// The window's menu items in the UI language `language` (a settings value: `en`, `nl`).
fn items_in(tmp: &Tmp, language: &str) -> Vec<Item> {
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.path(&format!("menu-config-{language}")), "0.0.0-test"),
    );
    let set = json!({"key": "language", "value": language});
    assert_eq!(shell.dispatch("settings.set", &set), Outcome::Done);
    let dir = tmp.path(&format!("menu-{language}.ew"));
    assert_eq!(
        shell.dispatch("file.new", &json!({"dir": dir.to_string_lossy()})),
        Outcome::Done
    );
    let mut out = Vec::new();
    for menu in menu_model(&shell) {
        let mut path = vec![menu.label.clone()];
        for e in &menu.entries {
            walk(e, &mut path, &mut out);
        }
    }
    let set = json!({"key": "language", "value": "en"});
    assert_eq!(shell.dispatch("settings.set", &set), Outcome::Done);
    shell.shutdown();
    out
}

/// A menu path a page names: the line it starts on, its text, the key in brackets after it, and whether it stands in
/// a numbered step.
#[derive(Debug)]
struct Named {
    line: usize,
    text: String,
    key: Option<String>,
    in_step: bool,
}

/// The bold menu paths of a page.
fn named(text: &str) -> Vec<Named> {
    // The line and the numbered-step state of each byte offset's line.
    let mut starts = Vec::new();
    let mut in_step = false;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_end();
        let number = trimmed
            .split_once(". ")
            .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
        if number {
            in_step = true;
        } else if !trimmed.is_empty() && !line.starts_with("   ") {
            in_step = false;
        }
        starts.push((offset, in_step));
        offset += line.len();
    }
    let line_of = |at: usize| starts.iter().rposition(|(o, _)| *o <= at).unwrap();
    let mut out = Vec::new();
    let mut rest = 0;
    while let Some(open) = text[rest..].find("**").map(|i| rest + i) {
        let Some(close) = text[open + 2..].find("**").map(|i| open + 2 + i) else {
            break;
        };
        let inner = text[open + 2..close]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        rest = close + 2;
        if !inner.contains(" ▸ ") {
            continue;
        }
        let key = text[rest..]
            .strip_prefix(" (`")
            .and_then(|k| k.split_once("`)"))
            .map(|(k, _)| k.to_owned());
        let line = line_of(open);
        out.push(Named {
            line: line + 1,
            text: inner,
            key,
            in_step: starts[line].1,
        });
    }
    out
}

/// Whether `params` holds every member of `item` with the same value.
fn holds(params: &Value, item: &Value) -> bool {
    item.as_object()
        .is_none_or(|m| m.iter().all(|(k, v)| params.get(k) == Some(v)))
}

/// The T6 problems of the page `name` (its text `text`), against the menu steps of `script` when it has one.
fn lint(name: &str, text: &str, script: Option<&[(String, Value)]>, items: &[Item]) -> Vec<String> {
    let t6 = |n: &Named, what: &str, action: &str, expected: &str| {
        format!(
            "{name}:{}: \"{what}\" is not how the window names {action} (\"{expected}\")",
            n.line
        )
    };
    let named = named(text);
    let mut problems = Vec::new();
    let mut free: Vec<&Named> = named.iter().filter(|n| !n.in_step || script.is_none()).collect();
    if let Some(script) = script {
        let steps: Vec<&Item> = script
            .iter()
            .filter_map(|(action, params)| {
                items
                    .iter()
                    .filter(|i| i.action == *action && holds(params, &i.params))
                    .max_by_key(|i| i.params.as_object().map_or(0, serde_json::Map::len))
            })
            .collect();
        let in_steps: Vec<&Named> = named.iter().filter(|n| n.in_step).collect();
        for (n, item) in in_steps.iter().zip(&steps) {
            if n.text != item.path {
                problems.push(t6(n, &n.text, &item.action, &item.path));
            }
            if let Some(key) = &n.key
                && !item.keys.contains(key)
            {
                problems.push(t6(n, key, &item.action, &item.keys.join(", ")));
            }
        }
        if in_steps.len() != steps.len() {
            problems.push(format!(
                "{name}: the steps name {} menu paths, the script runs {} menu steps",
                in_steps.len(),
                steps.len()
            ));
        }
    } else {
        free = named.iter().collect();
    }
    for n in free {
        let known = items
            .iter()
            .any(|i| i.path == n.text || i.path.starts_with(&format!("{} ▸ ", n.text)));
        if !known {
            let last = n.text.rsplit(" ▸ ").next().unwrap_or_default();
            match items.iter().find(|i| i.path.ends_with(&format!(" ▸ {last}"))) {
                Some(i) => problems.push(t6(n, &n.text, &i.action, &i.path)),
                None => problems.push(t6(n, &n.text, "any action", "no menu path")),
            }
        }
    }
    problems
}

/// T6 on fixture pages, whole: a step's path that is not the window's, a key that is not the action's binding, a
/// path outside the steps that names no menu item, and a page with a step too many.
#[test]
fn the_page_lint_names_the_windows_words_for_each_step() {
    let tmp = Tmp::new();
    let items = items(&tmp);
    let script = [
        ("file.new".to_owned(), json!({"dir": "x.ew"})),
        ("cursor.set".to_owned(), json!({"x_mm": 1, "y_mm": 1})),
        ("file.save".to_owned(), json!({})),
    ];
    let good = "1. Choose **File ▸ New project…** (`Ctrl+N`).\n2. Click, then choose **File ▸ Save**.\n\n\
                Later, **Help ▸ Documentation** helps.\n";
    assert_eq!(lint("p.md", good, Some(&script), &items), Vec::<String>::new());
    let bad = "1. Choose **File ▸ New Project…** (`Ctrl+M`).\n2. Choose\n   **File ▸ Save**.\n\n\
               Later, **Help ▸ Manual** helps.\n";
    assert_eq!(
        lint("p.md", bad, Some(&script), &items),
        [
            "p.md:1: \"File ▸ New Project…\" is not how the window names file.new (\"File ▸ New project…\")",
            "p.md:1: \"Ctrl+M\" is not how the window names file.new (\"Ctrl+N\")",
            "p.md:5: \"Help ▸ Manual\" is not how the window names any action (\"no menu path\")",
        ]
    );
    let extra = format!("{good}\n3. Choose **File ▸ Save** again.\n");
    assert_eq!(
        lint("p.md", &extra, Some(&script), &items),
        ["p.md: the steps name 3 menu paths, the script runs 2 menu steps"]
    );
    assert_eq!(
        lint("q.md", "See **Draw ▸ Bond**.\n", None, &items),
        ["q.md:1: \"Draw ▸ Bond\" is not how the window names any action (\"no menu path\")"]
    );
}

/// Tutorial 1 (FT-613): a new project from the basic project, a transformer and two lamps placed and wired, the two
/// potentials defined, the Potentials navigator and the Messages pane read, saved: every step done, no error, the
/// findings the page names, and the page passes the lint.
#[test]
fn tutorial_1_a_first_schematic_page_runs_as_its_page_says() {
    let tmp = Tmp::new();
    let out = run_tutorial(&tmp, "first-page");
    let transcript = all_done(&out);
    // FT-1440 (ID-1397): a finished wire answers with what it changed, never a bare "done".
    let changed: Vec<String> = transcript
        .lines()
        .skip(1)
        .filter_map(|l| {
            let step: Value = serde_json::from_str(l).unwrap();
            (step["action"] == "canvas.activate" && step["changed"].is_array())
                .then(|| step["changed"].to_string())
        })
        .collect();
    assert_eq!(
        changed,
        [
            r#"["Place device on 1"]"#,
            r#"["Place device on 1"]"#,
            r#"["Place device on 1"]"#,
            r#"["Draw wire on 1"]"#,
            r#"["Draw wire on 1"]"#,
            r#"["Draw wire on 1"]"#,
            r#"["Draw wire on 1"]"#,
        ]
    );
    let describe = last_describe(&transcript);
    let rows: Vec<&str> = describe
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("row level 1 potential") || l.starts_with("row level 2 rule"))
        .collect();
    assert_eq!(
        rows,
        [
            "row level 1 potential \"0VAC\" expanded",
            "row level 1 potential \"24VAC\" expanded",
            "row level 2 rule \"EW-CORE-041 Device without part\" badge \"3\" expanded",
            "row level 2 rule \"EW-POT-009 Unconnected CP\" badge \"2\" expanded",
        ]
    );
    // Step 8: "the Messages header shows E 0, W 0 and pending 5" (ID-1376, FT-1407).
    assert_eq!(
        lines_of(&describe, &["messages scheme"]),
        ["messages scheme \"Drafting\" errors 0 warnings 0 infos 0 waived 0 pending 5"]
    );
    assert!(
        describe.contains("title: first-page.ew - ElektraWatt"),
        "saved: {describe}"
    );
    assert!(tmp.path("first-page.ew/project.jsonl").is_file());
    let items = items(&tmp);
    assert_eq!(
        lint(
            "tutorials/first-page.md",
            &page("tutorials/first-page.md"),
            Some(&script("first-page")),
            &items
        ),
        Vec::<String>::new()
    );
}

/// Tutorial 2 (D10, DC-6, FT-1371): on tutorial 1's project, a strip with two terminals, the cabinet and the field
/// side wired, a cable from the starter set's part with the field wires as its cores (its tag from its first core's
/// page, FT-1406), its length set, the strip editor and the Cables navigator read, checked and saved: every step done,
/// the end state the page names, and the lint.
#[test]
fn tutorial_2_a_terminal_strip_and_a_cable_runs_as_its_page_says() {
    let tmp = Tmp::new();
    all_done(&run_tutorial(&tmp, "first-page"));
    let transcript = all_done(&run_tutorial(&tmp, "terminal-strip-and-cable"));
    // The toasts that steps 6 and 7 quote (ID-1376).
    let [created, connected, describe] = <[String; 3]>::try_from(describes(&transcript)).unwrap();
    assert_eq!(
        lines_of(&created, &["toast"]),
        ["toast \"Created cable -W1 with 3 cores\" undo"]
    );
    assert_eq!(
        lines_of(&connected, &["toast"]),
        ["toast \"Wires connected to the cores of -1W1: 2\" undo"]
    );
    assert_eq!(
        lines_of(
            &describe,
            &[
                "row \"-1X1\"",
                "row level 1 cable",
                "row level 2 conductor",
                "toast",
                "title:"
            ]
        ),
        [
            "title: first-page.ew - ElektraWatt",
            "row \"-1X1\" \"1\" \"1\" \"\" \"\" \"\" \"24VAC\" \"-1E2:X1\" \"-1E3:X1 via -1W1 BN\" \"\" \"\" \"used\" \
             cursor \"Strip\"",
            "row \"-1X1\" \"2\" \"2\" \"\" \"\" \"\" \"0VAC\" \"-1E2:X2\" \"-1E3:X2 via -1W1 BU\" \"\" \"\" \"used\"",
            "row level 1 cable \"-1W1\" expanded cursor",
            "row level 2 conductor \"BN\"",
            "row level 2 conductor \"BU\"",
            "row level 2 conductor \"GNYE\"",
            "toast \"Check: 0 errors, 8 warnings, 2 infos\"",
        ]
    );
    let items = items(&tmp);
    assert_eq!(
        lint(
            "tutorials/terminal-strip-and-cable.md",
            &page("tutorials/terminal-strip-and-cable.md"),
            Some(&script("terminal-strip-and-cable")),
            &items
        ),
        Vec::<String>::new()
    );
}

/// Tutorial 4 (D10, DC-6, FT-1372): on Help ▸ Tutorials' copy of the sample project, a parts list and a terminal
/// overview added as report blocks, the pages exported as PDF, the print preview stepped through and the project
/// saved: every step done, the pages, the PDF's page count and the preview the page names, and the lint.
#[test]
fn tutorial_4_reports_and_pdf_runs_as_its_page_says() {
    let tmp = Tmp::new();
    let out = run_tutorial_with(&tmp, "reports-and-pdf", Some(support::GOLDEN_A));
    let transcript = all_done(&out);
    let [copied, parts, preview, end] = <[String; 4]>::try_from(describes(&transcript)).unwrap();
    assert_eq!(
        lines_of(&copied, &["banner:"]),
        ["banner: A fresh copy of the sample project was made for the tutorials."]
    );
    // Step 2: the report pages of section 9, marked generated, from the parts list to the revision overview (ID-1376).
    assert_eq!(
        lines_of(&copied, &["row level 2 page \"90", "row level 2 page \"91"]),
        [
            "row level 2 page \"900\" \"=P01+J-E-2416 - Section 9 - Page 900 - 999 - Nomenclature\" changed \"V1.0\"",
            "row level 2 page \"901\" \"Partlist 1\" generated changed \"V1.0\"",
            "row level 2 page \"901.1\" \"Partlist 2\" generated changed \"V1.0\"",
            "row level 2 page \"902\" \"Cabinet Layout\" badge \"QET grid\" changed \"V1.0\"",
            "row level 2 page \"903\" \"Terminal Diagram 1X1\" generated changed \"V1.0\"",
            "row level 2 page \"904\" \"Terminal Diagram -X1031\" generated changed \"V1.0\"",
            "row level 2 page \"905\" \"Terminal Diagram -X1032\" generated changed \"V1.0\"",
            "row level 2 page \"906\" \"Cable Overview and Diagrams\" generated changed \"V1.0\"",
            "row level 2 page \"907\" \"Wire List 1\" generated changed \"V1.0\"",
            "row level 2 page \"907.1\" \"Wire List 2\" generated changed \"V1.0\"",
            "row level 2 page \"907.2\" \"Wire List 3\" generated changed \"V1.0\"",
            "row level 2 page \"908\" \"PLC Overview -201U1 1\" generated changed \"V1.0\"",
            "row level 2 page \"908.1\" \"PLC Overview -201U1 2\" generated changed \"V1.0\"",
            "row level 2 page \"908.2\" \"PLC Overview -201U1 3\" generated changed \"V1.0\"",
            "row level 2 page \"909\" \"Potential Overview\" generated changed \"V1.0\"",
            "row level 2 page \"910\" \"Device Tag Overview 1\" generated changed \"V1.0\"",
            "row level 2 page \"910.1\" \"Device Tag Overview 2\" generated changed \"V1.0\"",
            "row level 2 page \"910.2\" \"Device Tag Overview 3\" generated changed \"V1.0\"",
            "row level 2 page \"910.3\" \"Device Tag Overview 4\" generated changed \"V1.0\"",
            "row level 2 page \"911\" \"Revision Overview\" generated changed \"V1.0\"",
        ]
    );
    // Step 3's toast.
    assert_eq!(
        lines_of(&parts, &["toast"]),
        ["toast \"Added the report block Parts list from folio 912\" undo"]
    );
    assert_eq!(
        lines_of(&preview, &["dialog \"Print\"", "preview \""]),
        ["dialog \"Print\"", "preview \"2 of 41\" page 2"]
    );
    assert_eq!(
        lines_of(&end, &["row level 2 page \"91", "title:"]),
        [
            "title: reports.ew - ElektraWatt",
            "row level 2 page \"910\" \"Device Tag Overview 1\" generated changed \"V1.0\"",
            "row level 2 page \"910.1\" \"Device Tag Overview 2\" generated changed \"V1.0\"",
            "row level 2 page \"910.2\" \"Device Tag Overview 3\" generated changed \"V1.0\"",
            "row level 2 page \"910.3\" \"Device Tag Overview 4\" generated changed \"V1.0\"",
            "row level 2 page \"911\" \"Revision Overview\" generated changed \"V1.0\"",
            "row level 2 page \"912\" \"Parts list\" generated changed \"V1.0\"",
            "row level 2 page \"912.1\" \"Parts list\" generated changed \"V1.0\"",
            "row level 2 page \"912.2\" \"Parts list\" generated changed \"V1.0\"",
            "row level 2 page \"912.3\" \"Parts list\" generated changed \"V1.0\"",
            "row level 2 page \"913\" \"Terminal overview\" generated changed \"V1.0\"",
        ]
    );
    let toast = lines_of(&end, &["toast \"Exported"]);
    assert!(
        toast.len() == 1 && toast[0].starts_with(
                "toast \"Exported reports.pdf: 13 drawing pages, 3 section divider pages and 25 report pages, "
            ),
        "{toast:?}"
    );
    assert!(tmp.path("reports.pdf").is_file());
    assert!(tmp.path("reports.ew/project.jsonl").is_file());
    let items = items(&tmp);
    assert_eq!(
        lint(
            "tutorials/reports-and-pdf.md",
            &page("tutorials/reports-and-pdf.md"),
            Some(&script("reports-and-pdf")),
            &items
        ),
        Vec::<String>::new()
    );
}

/// Tutorial 6 (D10, D17, DC-7): the tutorial's QElectroTech project imported through the front door and its preview,
/// the migration report read, the return wire drawn and the four parts chosen, checked and saved: every step done, the
/// report's counts before and after the fixes as the page states them, and the page passes the lint.
#[test]
fn tutorial_6_importing_a_qelectrotech_project_runs_as_its_page_says() {
    let tmp = Tmp::new();
    std::fs::copy(
        Path::new(BOOK).join("src/tutorials/files/lamp-circuit.qet"),
        tmp.path("lamp-circuit.qet"),
    )
    .unwrap();
    let out = run_tutorial(&tmp, "import-qet");
    let transcript = all_done(&out);
    // FT-1440 (ID-1397): a part chosen answers with what it changed, never a bare "done".
    let picked: Vec<String> = transcript
        .lines()
        .skip(1)
        .filter_map(|l| {
            let step: Value = serde_json::from_str(l).unwrap();
            (step["action"] == "part.pick").then(|| step["changed"].to_string())
        })
        .collect();
    assert_eq!(picked, [r#"["Assign part on 1"]"#; 4]);
    let [preview, imported, wired, end] = <[String; 4]>::try_from(describes(&transcript)).unwrap();
    assert_eq!(
        lines_of(&preview, &["label \"1 page,"]),
        [
            "label \"1 page, 4 devices, 3 connections; 0 errors, 8 warnings and 5 infos in the migration report\""
        ]
    );
    let report = ["messages scheme", "row level 2 rule"];
    assert_eq!(
        lines_of(&imported, &report),
        [
            "messages scheme \"Migration report\" errors 0 warnings 8 infos 5 waived 0 pending 0",
            "row level 2 rule \"EW-CORE-041 Device without part\" badge \"4\" expanded",
            "row level 2 rule \"EW-IMP-008 Value kept as foreign or opaque text\" badge \"1\" expanded",
            "row level 2 rule \"EW-IMP-119 Element without function semantics\" badge \"1\" expanded",
            "row level 2 rule \"EW-POT-009 Unconnected CP\" badge \"2\" expanded",
            "row level 2 rule \"EW-IMP-103 Label kept as a frozen tag\" badge \"2\" expanded",
            "row level 2 rule \"EW-IMP-122 Unlabelled element, tag synthesised\" badge \"1\" expanded",
            "row level 2 rule \"EW-NUM-001 Tag deviates from the scheme\" badge \"2\" expanded",
        ]
    );
    // Step 4: the return wire drawn, the Messages pane on the project's own scheme, and EW-POT-009 gone (ID-1376).
    assert_eq!(
        lines_of(&wired, &["messages scheme", "row level 2 rule \"EW-POT-009"]),
        ["messages scheme \"Drafting\" errors 0 warnings 2 infos 5 waived 0 pending 4"]
    );
    assert_eq!(
        lines_of(&end, &report),
        [
            "messages scheme \"Migration report\" errors 0 warnings 2 infos 5 waived 0 pending 0",
            "row level 2 rule \"EW-IMP-008 Value kept as foreign or opaque text\" badge \"1\" expanded",
            "row level 2 rule \"EW-IMP-119 Element without function semantics\" badge \"1\" expanded",
            "row level 2 rule \"EW-IMP-103 Label kept as a frozen tag\" badge \"2\" expanded",
            "row level 2 rule \"EW-IMP-122 Unlabelled element, tag synthesised\" badge \"1\" expanded",
            "row level 2 rule \"EW-NUM-001 Tag deviates from the scheme\" badge \"2\" expanded",
        ]
    );
    assert_eq!(
        lines_of(&end, &["title:"]),
        ["title: lamp-circuit.ew - ElektraWatt"]
    );
    assert!(tmp.path("lamp-circuit.ew/project.jsonl").is_file());
    let items = items(&tmp);
    assert_eq!(
        lint(
            "tutorials/import-qet.md",
            &page("tutorials/import-qet.md"),
            Some(&script("import-qet")),
            &items
        ),
        Vec::<String>::new()
    );
}

/// The symbol editor's tutorial (WP-S2.10, G-M2.5-05; M25-p3#2): a six-pin power block drawn in the symbol editor,
/// refused off the grid in words, saved to a library, exported, tested on a scratch page, the project saved and the
/// exported draft reopened: every step done, the end state the page names, and the page passes the lint.
#[test]
fn tutorial_draw_a_six_pin_power_block_runs_as_its_page_says() {
    let tmp = Tmp::new();
    let out = run_tutorial(&tmp, "symbol-editor");
    let transcript = all_done(&out);
    let [drawn, off_grid, saved, reopened] = <[String; 4]>::try_from(describes(&transcript)).unwrap();
    let editor = ["dialog ", "field ", "error "];
    // Step 5: "The validation says Ready to save" (no finding).
    assert_eq!(
        lines_of(&drawn, &editor),
        ["dialog \"Symbol editor\"", "field \"Key\": \"example.power\""]
    );
    // Step 7: the refusal in words.
    assert_eq!(
        lines_of(&off_grid, &editor),
        [
            "dialog \"Symbol editor\"",
            "field \"Key\": \"example.power\"",
            "error \"Connection point 1 is off the connection grid. Move it onto the grid before saving.\"",
        ]
    );
    // Steps 8 to 11: the library, the portable draft and the saved project with its scratch page.
    assert!(tmp.path("symbols.ewlib").is_dir());
    assert!(tmp.path("power.symbol.json").is_file());
    assert_eq!(
        lines_of(&saved, &["title:"]),
        ["title: symbol-scratch.ew - ElektraWatt"]
    );
    // What you learnt: the exported draft opens again, without a finding.
    assert_eq!(
        lines_of(&reopened, &editor),
        ["dialog \"Symbol editor\"", "field \"Key\": \"example.power\""]
    );
    let items = items(&tmp);
    assert_eq!(
        lint(
            "tutorials/symbol-editor.md",
            &page("tutorials/symbol-editor.md"),
            Some(&script("symbol-editor")),
            &items
        ),
        Vec::<String>::new()
    );
}

/// The translations of the Dutch guide (`docs/book/po/nl.po`): (the entry's first reference, its translation), for
/// every translated entry.
fn dutch_translations() -> Vec<(String, String)> {
    let path = Path::new(BOOK).join("po/nl.po");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let unquote = |s: &str| {
        let inner = s
            .trim()
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap();
        inner
            .replace("\\n", "\n")
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
    };
    let mut out = Vec::new();
    for entry in text.replace("\r\n", "\n").split("\n\n") {
        let reference = entry
            .lines()
            .find_map(|l| l.strip_prefix("#: "))
            .and_then(|r| r.split_whitespace().next())
            .unwrap_or_default()
            .to_owned();
        let mut msgstr = None::<String>;
        for line in entry.lines() {
            if let Some(rest) = line.strip_prefix("msgstr ") {
                msgstr = Some(unquote(rest));
            } else if let Some(m) = msgstr.as_mut()
                && line.starts_with('"')
            {
                m.push_str(&unquote(line));
            }
        }
        if let Some(m) = msgstr.filter(|m| !m.is_empty() && !reference.is_empty()) {
            out.push((reference, m));
        }
    }
    out
}

/// D14: every menu path that a Dutch translation names is the label path of a menu item of the window in Dutch, or of
/// a menu or group above one (T6 for the Dutch guide).
#[test]
fn the_dutch_guide_names_the_dutch_windows_menu_paths() {
    let tmp = Tmp::new();
    let items = items_in(&tmp, "nl");
    let translations = dutch_translations();
    let named_paths: usize = translations.iter().map(|(_, t)| named(t).len()).sum();
    assert!(named_paths > 0, "nl.po names no menu path");
    let problems: Vec<String> = translations
        .iter()
        .flat_map(|(reference, text)| lint(&format!("po/nl.po ({reference})"), text, None, &items))
        .collect();
    assert_eq!(problems, Vec::<String>::new());
}

/// D9, D12, D13: the how-to and explanation pages that name menu paths pass the lint.
#[test]
fn the_how_to_and_explanation_pages_name_the_windows_menu_paths() {
    let tmp = Tmp::new();
    let items = items(&tmp);
    for rel in [
        "how-to/choose-parts.md",
        "how-to/upgrade-a-project.md",
        "explanation/eplan-and-ew.md",
        "explanation/qet-and-ew.md",
    ] {
        let text = page(rel);
        assert!(!named(&text).is_empty(), "{rel} names no menu path");
        assert_eq!(lint(rel, &text, None, &items), Vec::<String>::new(), "{rel}");
    }
}
