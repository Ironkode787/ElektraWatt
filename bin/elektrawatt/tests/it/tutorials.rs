// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tutorials of the user guide as scripted tests (NFR-DOC-03): each tutorial's script
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

/// The path of the `elektrawatt` binary under test (nextest sets it at run time; see `tests/it/headless.rs`).
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
/// given (`EW_DATA_DIR`): the tutorials run on the shipped tier alone, the compiled-in starter parts and the libraries
/// of `EW_CONTENT_DIR` (the sample parts library among them), as a new installation does.
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

/// Tutorial 1: a new project from the basic project, a transformer and two lamps placed and wired, the two
/// potentials defined, the Potentials navigator and the Messages pane read, saved: every step done, no error, the
/// findings the page names, and the page passes the lint.
#[test]
fn tutorial_1_a_first_schematic_page_runs_as_its_page_says() {
    let tmp = Tmp::new();
    let out = run_tutorial(&tmp, "first-page");
    let transcript = all_done(&out);
    // A finished wire answers with what it changed, never a bare "done".
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
    // Step 8: "the Messages header shows E 0, W 0 and pending 5".
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

/// Tutorial 2 (D10, DC-6): on tutorial 1's project, a strip with two terminals, the cabinet and the field
/// side wired, the field wires made a cable 3G1.5 by Cable… (its tag from its first core's page), its length set,
/// the strip editor and the Cables navigator read, checked and saved: every step done, the end state the page names,
/// and the lint.
#[test]
fn tutorial_2_a_terminal_strip_and_a_cable_runs_as_its_page_says() {
    let tmp = Tmp::new();
    all_done(&run_tutorial(&tmp, "first-page"));
    let transcript = all_done(&run_tutorial(&tmp, "terminal-strip-and-cable"));
    // The toast that step 6 quotes.
    let [connected, describe] = <[String; 2]>::try_from(describes(&transcript)).unwrap();
    assert_eq!(
        lines_of(&connected, &["toast"]),
        ["toast \"Cable -1W1 made from 2 wires.\" undo"]
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
            "toast \"Check: 0 errors, 8 warnings, 2 infos.\"",
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

/// Tutorial 4 (D10, DC-6): on Help ▸ Tutorials' copy of the sample project, a parts list and a terminal
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
    // Step 2: the report pages of section 9, marked generated, from the parts list to the revision overview.
    assert_eq!(
        lines_of(&copied, &["row level 2 page \"90", "row level 2 page \"91"]),
        [
            "row level 2 page \"900\" \"=P01+J-E-2416 - Section 9 - Page 900 - 999 - Nomenclature\" changed \"V1.0\"",
            "row level 2 page \"901\" \"Partlist 1\" generated changed \"V1.0\"",
            "row level 2 page \"901.1\" \"Partlist 2\" generated changed \"V1.0\"",
            "row level 2 page \"902\" \"Cabinet Layout\" badge \"QET 10 px\" changed \"V1.0\"",
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
    // Step 1's toast, which says where the copy is (F4), and step 3's.
    assert_eq!(
        lines_of(&parts, &["toast"]),
        [
            "toast \"The tutorial's practice project is under Tutorials in your projects folder.\" show_folder",
            "toast \"Added the report block Parts list from folio 912.\" undo"
        ]
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
    // A part chosen answers with what it changed, never a bare "done".
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
    // Step 1: the card says what its button makes, gives its verdict and lists the kinds of findings.
    assert_eq!(
        lines_of(
            &preview,
            &[
                "label \"Create project makes",
                "label \"The file was",
                "group \"",
                "kind \""
            ]
        )
        .into_iter()
        .skip_while(|l| !l.starts_with("label"))
        .collect::<Vec<_>>(),
        [
            "label \"Create project makes a new project in the folder lamp-circuit.ew, with 1 page, 4 devices and 3 connections.\"",
            "label \"The file was imported. 7 points are worth a look, and none stops you from working.\"",
            "group \"Worth a look\"",
            "kind \"4 × Device without part\"",
            "kind \"2 × Unconnected CP\"",
            "kind \"1 × Element without function semantics\"",
            "group \"For your information\"",
            "kind \"2 × Label kept as a frozen tag\"",
            "kind \"2 × Tag deviates from the scheme\"",
            "kind \"1 × Font substituted\"",
            "kind \"1 × Default folio-index formula\"",
            "kind \"1 × Unlabelled element, tag synthesised\"",
        ]
    );
    let report = ["messages scheme", "row level 2 rule"];
    assert_eq!(
        lines_of(&imported, &report),
        [
            "messages scheme \"Migration report\" errors 0 warnings 7 infos 7 waived 0 pending 0",
            "row level 2 rule \"EW-CORE-041 Device without part\" badge \"4\" expanded",
            "row level 2 rule \"EW-IMP-119 Element without function semantics\" badge \"1\" expanded",
            "row level 2 rule \"EW-POT-009 Unconnected CP\" badge \"2\" expanded",
            "row level 2 rule \"EW-IMP-006 Font substituted\" badge \"1\" expanded",
            "row level 2 rule \"EW-IMP-103 Label kept as a frozen tag\" badge \"2\" expanded",
            "row level 2 rule \"EW-IMP-117 Default folio-index formula\" badge \"1\" expanded",
            "row level 2 rule \"EW-IMP-122 Unlabelled element, tag synthesised\" badge \"1\" expanded",
            "row level 2 rule \"EW-NUM-001 Tag deviates from the scheme\" badge \"2\" expanded",
        ]
    );
    // Step 4: the return wire drawn, the Messages pane on the project's own scheme, and EW-POT-009 gone.
    assert_eq!(
        lines_of(&wired, &["messages scheme", "row level 2 rule \"EW-POT-009"]),
        ["messages scheme \"Drafting\" errors 0 warnings 1 infos 7 waived 0 pending 4"]
    );
    assert_eq!(
        lines_of(&end, &report),
        [
            "messages scheme \"Migration report\" errors 0 warnings 1 infos 7 waived 0 pending 0",
            "row level 2 rule \"EW-IMP-119 Element without function semantics\" badge \"1\" expanded",
            "row level 2 rule \"EW-IMP-006 Font substituted\" badge \"1\" expanded",
            "row level 2 rule \"EW-IMP-103 Label kept as a frozen tag\" badge \"2\" expanded",
            "row level 2 rule \"EW-IMP-117 Default folio-index formula\" badge \"1\" expanded",
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

/// The symbol editor's tutorial (G-M2.5-05; M25-p3#2): a six-pin power block drawn in the symbol editor,
/// refused off the grid in words, saved to a library, exported, tested on a new page, the project saved and the
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
    // Steps 8 to 11: the library, the portable draft and the saved project with its new page. A name without a
    // folder is read from the user library folder, never the working directory.
    assert!(!tmp.path("symbols.ewlib").exists() && !tmp.path("power.symbol.json").exists());
    let library = find_named(&tmp.path(""), "symbols.ewlib").expect("the library");
    assert!(library.is_dir());
    let user = library.parent().unwrap();
    assert_eq!(user.file_name().unwrap(), "libraries");
    assert!(user.join("power.symbol.json").is_file());
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

/// The line of `describe` that starts with `start`, trimmed (empty when none does).
fn line_of(describe: &str, start: &str) -> String {
    lines_of(describe, &[start])
        .into_iter()
        .next()
        .unwrap_or_default()
}

/// The PLC tutorial (rc.2, F5): a position switch and a lamp, a station made in the New station dialog with an input
/// and an output module and drawn on the page, the two wired to the first channels, their symbols typed in the PLC
/// grid, the PLC overview added, the export to PLC software and the save: every step done, the end state the page
/// names, and the page passes the lint.
#[test]
fn tutorial_a_plc_with_its_modules_runs_as_its_page_says() {
    let tmp = Tmp::new();
    let transcript = all_done(&run_tutorial(&tmp, "plc-station"));
    let [rack, overview, exported, end] = <[String; 4]>::try_from(describes(&transcript)).unwrap();
    // Steps 5 to 7: the head part and the two modules in their slots, slot 3 free.
    assert_eq!(
        lines_of(&rack, &["dialog \"New station\"", "head \"", "slot "]),
        [
            "dialog \"New station\"",
            "head \"EW-GEN-CPU-1 · PLC CPU without on-board I/O\"",
            "slot 1 \"\" \"EW-GEN-DI-1 · PLC digital input card, 8 channels\" gives \"8 DI\"",
            "slot 2 \"\" \"EW-GEN-DO-1 · PLC digital output card, 8 channels\" gives \"8 DO\"",
            "slot 3 free",
        ]
    );
    // Steps 11 and 12: the first channel of each module with its symbol.
    let input = line_of(&overview, "row \"I0.0\"");
    assert!(
        input.contains("\"Door_closed\"") && input.contains("\"-1A1-A1\" \"0\" \"DI\""),
        "{input}"
    );
    // Step 10: the modules drawn from the rack are bound to their pins, so the switch wired to pin 1 is the target of
    // channel I0.0.
    assert!(input.contains("-1B1"), "{input}");
    let output = line_of(&overview, "row \"Q0.0\"");
    assert!(
        output.contains("\"Lamp_door\"") && output.contains("\"-1A1-A2\" \"0\" \"DO\""),
        "{output}"
    );
    // Step 13: the overview page and its toast.
    assert_eq!(
        line_of(&overview, "row level 2 page \"2\""),
        "row level 2 page \"2\" \"PLC overview\" generated current cursor"
    );
    assert_eq!(
        line_of(&overview, "toast"),
        "toast \"Added the report block PLC overview from folio 2.\" undo"
    );
    // Step 14: the export.
    assert_eq!(
        line_of(&exported, "toast"),
        "toast \"Exported plc-station.aml: 1 station, 2 modules, 16 channels, 2 symbols.\""
    );
    assert!(tmp.path("plc-station.aml").is_file());
    assert_eq!(line_of(&end, "title:"), "title: plc-station.ew - ElektraWatt");
    assert!(tmp.path("plc-station.ew/project.jsonl").is_file());
    let items = items(&tmp);
    assert_eq!(
        lint(
            "tutorials/plc-station.md",
            &page("tutorials/plc-station.md"),
            Some(&script("plc-station")),
            &items
        ),
        Vec::<String>::new()
    );
}

/// The bus tutorial (rc.2, F5): a bus-topology page with the generic CPU, remote I/O station and switch, the bus
/// lines, the node addresses, a bus cable, a PROFINET device of one's own made in the part editor, the check and the
/// save: every step done, the end state the page names, and the page passes the lint. The plugs of step 10 are chosen
/// in the window only (a script names a plug by its record, not its part number).
#[test]
fn tutorial_a_bus_network_runs_as_its_page_says() {
    let tmp = Tmp::new();
    let transcript = all_done(&run_tutorial(&tmp, "bus-network"));
    let [cpu, stations, cable, end] = <[String; 4]>::try_from(describes(&transcript)).unwrap();
    // Step 3: the CPU as a box per interface, and its rack.
    assert_eq!(
        lines_of(&cpu, &["toast", "dialog "]),
        [
            "toast \"Placed as 2 boxes, one for each bus interface.\" undo",
            "dialog \"Rack and slots of -2A1\"",
        ]
    );
    // Step 8: both stations belong to the one PLC; the remote one carries its module.
    assert_eq!(
        lines_of(&stations, &["row level 2 station", "row level 3 module"]),
        [
            "row level 2 station \"-2A1\" badge \"CPU\" expanded",
            "row level 2 station \"-2A2\" badge \"head station\" expanded",
            "row level 3 module \"-2A2-A1\" badge \"not drawn\" expanded",
        ]
    );
    // Step 10: the cable.
    assert_eq!(
        line_of(&cable, "toast"),
        "toast \"Cable -W1 carries the bus line.\" undo"
    );
    // Steps 9, 13 and 14: every node's address, and the check.
    assert_eq!(
        lines_of(&end, &["row \"-2", "toast", "title:"]),
        [
            "title: bus-network.ew - ElektraWatt",
            "row \"-2A1\" \"DP\" \"TBD\" \"\" \"PROFIBUS DP\" \"master\" \"\" \"\"",
            "row \"-2A1\" \"X1\" \"plc1 192.168.0.1\" \"\" \"PROFINET\" \"controller\" \"PROFINET\" \"\"",
            "row \"-2A2\" \"DP\" \"TBD\" \"\" \"PROFIBUS DP\" \"slave\" \"\" \"\"",
            "row \"-2A2\" \"X1\" \"rio1 192.168.0.2\" \"\" \"PROFINET\" \"device\" \"PROFINET\" \"\"",
            "row \"-2K1\" \"X1\" \"\" \"\" \"PROFINET\" \"passive\" \"\" \"\"",
            "row \"-2K2\" \"X1\" \"drive1 192.168.0.3\" \"\" \"PROFINET\" \"device\" \"PROFINET\" \"\" cursor \"Address\"",
            "toast \"Check: 0 errors, 0 warnings, 5 infos.\"",
        ]
    );
    assert!(tmp.path("bus-network.ew/project.jsonl").is_file());
    let items = items(&tmp);
    assert_eq!(
        lint(
            "tutorials/bus-network.md",
            &page("tutorials/bus-network.md"),
            Some(&script("bus-network")),
            &items
        ),
        Vec::<String>::new()
    );
}

/// The pneumatic tutorial (rc.2, F5): a fluid page with the air supply and its medium, the service unit, a 5/2 valve
/// and a cylinder with flow controls, the tubes, the valve's solenoid drawn on page 1 and wired to a PLC output, a
/// valve terminal with two positions filled, the default tube and the tube table, and the save: every step done, the
/// end state the page names, and the page passes the lint.
#[test]
fn tutorial_a_pneumatic_circuit_runs_as_its_page_says() {
    let tmp = Tmp::new();
    let transcript = all_done(&run_tutorial(&tmp, "pneumatic-circuit"));
    let [medium, valve, terminal, settings, tubes, end] =
        <[String; 6]>::try_from(describes(&transcript)).unwrap();
    // Step 10: the supply point and its medium.
    assert_eq!(
        lines_of(&medium, &["properties what", "row \"device/derived.medium\""]),
        [
            "properties what \"Compressed-air supply point · pneumatic\"",
            "row \"device/derived.medium\" \"Medium\" \"Compressed air\" derived \"medium definition at -2X1:1\" \
             actions edit",
        ]
    );
    // Step 15: the valve names where its solenoid is drawn.
    let solenoid = line_of(&valve, "row \"device/derived.solenoid[0]\"");
    assert!(
        solenoid.starts_with("row \"device/derived.solenoid[0]\" \"Solenoid 14\" \"1-A4\""),
        "{solenoid}"
    );
    // Steps 17 and 18: the terminal's positions.
    let positions = line_of(&terminal, "row \"device/derived.valve_positions\"");
    assert!(
        positions.contains("\"Valve positions\" \"1 -2Q2 · 2 -2Q3 · 3 empty · 4 empty\""),
        "{positions}"
    );
    // Step 19: the default tube.
    assert_eq!(
        line_of(&settings, "toast"),
        "toast \"The default tube is 6 × 4 mm PU (polyurethane) now: every tube without one of its own takes it, the \
         tubes drawn already too.\" undo"
    );
    // Step 20: the tube table, a row per tube with its ends, medium and size.
    assert_eq!(
        lines_of(&tubes, &["row \"\" \"-2"]),
        [
            "row \"\" \"-2M1:A\" \"consumer\" \"-2R1:1\" \"port\" \"Compressed air\" \"working\" \"6×4\" \"PU\" \"\" \"\" \
             \"—\" \"missing\" \"\" \"\" \"\" \"2\" cursor \"Tag\"",
            "row \"\" \"-2M1:B\" \"consumer\" \"-2R2:1\" \"port\" \"Compressed air\" \"working\" \"6×4\" \"PU\" \"\" \"\" \
             \"—\" \"missing\" \"\" \"\" \"\" \"2\"",
            "row \"\" \"-2Q1:2\" \"working\" \"-2R2:2\" \"port\" \"Compressed air\" \"working\" \"6×4\" \"PU\" \"\" \"\" \
             \"—\" \"missing\" \"\" \"\" \"\" \"2\"",
            "row \"\" \"-2Q1:4\" \"working\" \"-2R1:2\" \"port\" \"Compressed air\" \"working\" \"6×4\" \"PU\" \"\" \"\" \
             \"—\" \"missing\" \"\" \"\" \"\" \"2\"",
            "row \"\" \"-2V1:2\" \"working\" \"-2Q1:1\" \"supply\" \"Compressed air\" \"working\" \"6×4\" \"PU\" \"\" \"\" \
             \"—\" \"missing\" \"\" \"\" \"\" \"2\"",
            "row \"\" \"-2X1:1\" \"port\" \"-2V1:1\" \"supply\" \"Compressed air\" \"working\" \"6×4\" \"PU\" \"\" \"\" \
             \"—\" \"missing\" \"\" \"\" \"\" \"2\"",
        ]
    );
    assert_eq!(
        line_of(&end, "title:"),
        "title: pneumatic-circuit.ew - ElektraWatt"
    );
    assert!(tmp.path("pneumatic-circuit.ew/project.jsonl").is_file());
    let items = items(&tmp);
    assert_eq!(
        lint(
            "tutorials/pneumatic-circuit.md",
            &page("tutorials/pneumatic-circuit.md"),
            Some(&script("pneumatic-circuit")),
            &items
        ),
        Vec::<String>::new()
    );
}

/// The first file or folder named `name` under `dir`, depth first.
fn find_named(dir: &Path, name: &str) -> Option<std::path::PathBuf> {
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.file_name().is_some_and(|n| n == name) {
            return Some(path);
        }
        if path.is_dir()
            && let Some(found) = find_named(&path, name)
        {
            return Some(found);
        }
    }
    None
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

/// D9, D12, D13: the how-to and explanation pages that name menu paths pass the lint, as does the symbol editor's
/// reference page.
#[test]
fn the_how_to_and_explanation_pages_name_the_windows_menu_paths() {
    let tmp = Tmp::new();
    let items = items(&tmp);
    for rel in [
        "how-to/choose-parts.md",
        "how-to/upgrade-a-project.md",
        // FT-1687: the panel layout's how-to.
        "how-to/panel-layout.md",
        // rc.2 (F16): the PLC and the fluid guides.
        "how-to/plc-workspace.md",
        "how-to/fluid-workspace.md",
        "explanation/eplan-and-ew.md",
        "explanation/qet-and-ew.md",
        // M25-p3#9: the symbol editor's reference names the window's menu paths.
        "reference/symbol-editor.md",
    ] {
        let text = page(rel);
        assert!(!named(&text).is_empty(), "{rel} names no menu path");
        assert_eq!(lint(rel, &text, None, &items), Vec::<String>::new(), "{rel}");
    }
}
