// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `page.describe` on a temporary copy of the sample. The header counts of folios
//! 101 to 103, the expected texts (`tests/expected/page_describe_10{1,2,3}.txt`, rewritten by `EW_BLESS=1`), no
//! record ID anywhere in the text, and the refusals.
//! Every test works on a read-only temporary copy of the sample.

use crate::common;

use std::path::PathBuf;

use common::{Tmp, done, refused};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::{Value, json};

fn expected_path(folio: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("expected")
        .join(format!("page_describe_{folio}.txt"))
}

/// The text `page.describe` gives for `folio` (the page on show without one).
fn page_text(shell: &mut ShellModel, folio: Option<&str>) -> String {
    let params = folio.map_or_else(|| json!({}), |f| json!({ "folio": f }));
    done(shell, "page.describe", params);
    shell
        .take_page_text()
        .expect("a done page.describe leaves its text")
}

#[test]
fn the_texts_of_folios_101_to_103_equal_the_expected_files() {
    let tmp = Tmp::new("pagedesc");
    let mut shell = tmp.sample_shell("101");
    for folio in ["101", "102", "103"] {
        let text = page_text(&mut shell, Some(folio));
        let first = text.lines().next().unwrap();
        assert!(first.starts_with(&format!("page \"{folio}\" \"")), "{first}");
        let path = expected_path(folio);
        if ew_testdir::bless() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, format!("{text}\n")).unwrap();
        }
        let want = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{}: {e} (EW_BLESS=1 generates it)", path.display()));
        assert_eq!(
            format!("{text}\n"),
            want.replace("\r\n", "\n"),
            "{folio} differs from {} (EW_BLESS=1 rewrites it)",
            path.display()
        );
        // The line counts follow the header: one `rep` line per representation, one `wire` line per connection,
        // one `junction` line per junction.
        let count = |word: &str| text.lines().filter(|l| l.trim_start().starts_with(word)).count();
        let n = |key: &str| -> usize {
            let at = first.find(&format!(" {key} ")).unwrap() + key.len() + 2;
            first[at..].split(' ').next().unwrap().parse().unwrap()
        };
        assert_eq!(count("rep "), n("reps"));
        assert_eq!(count("wire "), n("connections"));
        assert_eq!(count("junction "), n("junctions"));
    }
    // The page on show is the default.
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    let shown = page_text(&mut shell, None);
    assert_eq!(shown, page_text(&mut shell, Some("102")));
    shell.shutdown();
}

#[test]
fn no_record_id_reaches_the_text() {
    let tmp = Tmp::new("pagewords");
    let mut shell = tmp.sample_shell("101");
    for folio in ["101", "102", "103"] {
        let text = page_text(&mut shell, Some(folio));
        assert_eq!(ew_viewmodel::alias::find_uuid(&text), None, "{folio}");
    }
    shell.shutdown();
}

#[test]
fn a_page_describe_step_carries_its_text_and_changes_no_state() {
    let tmp = Tmp::new("pagestep");
    let mut shell = tmp.sample_shell("101");
    let before = ew_viewmodel::script::state_json(&shell);
    let step = ew_viewmodel::script::Step {
        line: 1,
        action: "page.describe".to_owned(),
        params: json!({"folio": "102"}),
    };
    let result = ew_viewmodel::script::run_step(&mut shell, &mut ew_viewmodel::script::NoExtension, 1, &step);
    assert!(result.ok);
    let line: Value = serde_json::from_str(&result.line).unwrap();
    assert!(
        line["describe"].as_str().unwrap().starts_with("page \"102\" \""),
        "{line}"
    );
    // The transcript's state after the step equals the state before it (the page on show is still 101).
    assert_eq!(line["state"], serde_json::from_str::<Value>(&before).unwrap());
    // A refused or failed step has no `describe` member.
    let unknown = ew_viewmodel::script::Step {
        line: 2,
        action: "page.describe".to_owned(),
        params: json!({"folio": "999"}),
    };
    let result =
        ew_viewmodel::script::run_step(&mut shell, &mut ew_viewmodel::script::NoExtension, 2, &unknown);
    assert!(!result.ok);
    assert!(!result.line.contains("\"describe\""), "{}", result.line);
    shell.shutdown();
}

#[test]
fn page_describe_is_refused_without_a_project_and_fails_for_an_unknown_folio() {
    let tmp = Tmp::new("pagerefused");
    let mut none = tmp.empty_shell(Mode::Headless);
    assert_eq!(
        refused(&mut none, "page.describe", json!({})),
        "reason-no-project"
    );
    none.shutdown();

    let mut shell = tmp.sample_shell("101");
    match shell.dispatch("page.describe", &json!({"folio": "999"})) {
        Outcome::Failed { message } => assert!(message.contains("999"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert!(shell.take_page_text().is_none());
    // A parameter of the wrong type is a failed step, not a refusal.
    assert!(matches!(
        shell.dispatch("page.describe", &json!({"folio": 101})),
        Outcome::Failed { .. }
    ));
    shell.shutdown();

    // A new project describes its first page.
    let mut fresh = ShellModel::new(
        Mode::Headless,
        ew_viewmodel::shell::ShellConfig::in_dir(&tmp.0.join("config2"), "0.0.0-test"),
    );
    crate::common::make_project(&mut fresh, &tmp.0.join("fresh"), &[]);
    let text = page_text(&mut fresh, None);
    assert!(text.starts_with("page \""), "{text}");
    assert_eq!(ew_viewmodel::alias::find_uuid(&text), None);
    fresh.shutdown();
}

/// BD5 (the window host reads the same cache): the first `page.describe` of a folio makes one `page_tables`
/// call with the page content that carries it, a second makes none and re-executes no memoised query. On folio 102,
/// which draws a strip: its bridge lines are asked again and answered from the memo (hits, no re-execution; FT-1914).
#[test]
fn page_describe_reads_the_cached_page_tables() {
    let tmp = Tmp::new("pagecount");
    let mut shell = tmp.sample_shell("101");
    let before = shell.session_calls();
    let text = page_text(&mut shell, Some(FOLIO));
    let first = shell.session_calls();
    assert_eq!(
        first.page_tables,
        before.page_tables + 1,
        "one page_tables call for {FOLIO}"
    );
    assert_eq!(
        first.page_content,
        before.page_content + 1,
        "with the page content that carries it"
    );
    let re_executions = |s: &ew_viewmodel::shell::ShellModel| s.memo_totals().map(|(_, re)| re);
    let memo = re_executions(&shell);
    let again = page_text(&mut shell, Some(FOLIO));
    assert_eq!(
        shell.session_calls(),
        first,
        "a cached page costs no Session call"
    );
    assert_eq!(re_executions(&shell), memo, "and no memo re-execution");
    assert!(again.starts_with(&format!("page \"{FOLIO}\"")));
    assert_eq!(again, text, "the cached page describes as the first time");
    // The page on show is cached already.
    let shown = shell.session_calls();
    let _ = page_text(&mut shell, Some("101"));
    assert_eq!(shell.session_calls(), shown);
    shell.shutdown();
}

/// The folio [`page_describe_reads_the_cached_page_tables`] describes twice.
const FOLIO: &str = "102";

/// A generated list page prints its table: on the sample's parts list, the `table` line with the column and row
/// counts, the column headers and one `row` line per printed row, each data row with one quoted cell per column; a
/// drawn page prints no table.
#[test]
fn a_generated_list_page_prints_its_rows() {
    let tmp = Tmp::new("pagelist");
    let mut shell = tmp.sample_shell("101");
    let parts = shell
        .page_rows()
        .iter()
        .find(|r| r.generated && r.title.to_lowercase().contains("part"))
        .map(|r| r.folio.clone())
        .expect("The sample has a parts list page");
    let text = page_text(&mut shell, Some(&parts));
    let lines: Vec<&str> = text.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.starts_with("table columns "))
        .unwrap_or_else(|| panic!("no table line:\n{text}"));
    let mut words = lines[at].split(' ').skip(2);
    let columns: usize = words.next().unwrap().parse().unwrap();
    assert_eq!(words.next(), Some("rows"));
    let rows: usize = words.next().unwrap().parse().unwrap();
    assert!(columns > 0 && rows > 0, "{}", lines[at]);
    assert!(lines[at + 1].starts_with("  columns \""), "{}", lines[at + 1]);
    let row_lines: Vec<&&str> = lines[at + 2..]
        .iter()
        .filter(|l| l.starts_with("  row "))
        .collect();
    assert_eq!(row_lines.len(), rows, "{text}");
    let data = row_lines
        .iter()
        .find(|l| !l.starts_with("  row group "))
        .expect("a data row");
    assert_eq!(data.matches('"').count(), 2 * columns, "{data}");
    assert!(!page_text(&mut shell, Some("101")).contains("table columns"));
    shell.shutdown();
}
