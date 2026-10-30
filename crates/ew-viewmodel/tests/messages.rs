// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The message panel of WP-S2.09 part 2 on copies of Golden A (acceptance checks MS1's model half, MS2, MS3, MS4 and
//! MS6's model half): the pane in the bottom dock with its header, rows and detail lines; Enter on EF-08 opening -3W1;
//! the pending rule of FR-UI-041 (a new unpaired interruption point on 101, the tab switch, Run checks, a conflict
//! that is never pending); the quick fix of EW-CORE-023 as a previewed card with one undo step; F8 and Shift+F8; F1 on
//! a finding; the exact counts of §7.5; the 16 words and the 5 actions.

mod common;

use std::collections::BTreeSet;

use common::{Tmp, at, done, refused};
use ew_viewmodel::layout::PaneKind;
use ew_viewmodel::navigators::{NavKind, NavNode, NavRole};
use ew_viewmodel::properties::CardOrigin;
use ew_viewmodel::shell::{Focus, Mode, Outcome, ShellModel};
use serde_json::json;

fn table(shell: &ShellModel) -> std::sync::Arc<ew_viewmodel::navigators::NavTable> {
    shell
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .cloned()
        .expect("the Messages pane with its table")
}

/// The finding rows of rule `rule` whose subject ends with `subject`.
fn findings(shell: &ShellModel, rule: &str, subject: &str) -> Vec<NavNode> {
    table(shell)
        .nodes
        .iter()
        .filter(|n| n.role == NavRole::Finding && n.cells[1] == rule && n.cells[3].ends_with(subject))
        .cloned()
        .collect()
}

/// The open completeness findings on page `folio`: the rows that turn pending while it is the editing page (D16).
fn completeness_on(shell: &ShellModel, folio: &str) -> usize {
    let has = |n: &NavNode, k: &str, v: &str| n.facets.iter().any(|(a, b)| a == k && b == v);
    table(shell)
        .nodes
        .iter()
        .filter(|n| {
            n.role == NavRole::Finding
                && has(n, "kind", "completeness")
                && has(n, "state", "open")
                && has(n, "page", folio)
        })
        .count()
}

fn finding(shell: &ShellModel, rule: &str, subject: &str) -> NavNode {
    let v = findings(shell, rule, subject);
    assert_eq!(v.len(), 1, "one {rule} finding on {subject}");
    v.into_iter().next().unwrap()
}

/// The folio the focused tab shows.
fn folio(shell: &ShellModel) -> String {
    let page = shell.current_page().expect("a page");
    shell
        .page_rows()
        .iter()
        .find(|r| ew_viewmodel::canvas::PageId::of(r) == page)
        .map(|r| r.folio.clone())
        .unwrap()
}

fn line(shell: &ShellModel, prefix: &str) -> String {
    shell
        .describe()
        .lines()
        .find(|l| l.starts_with(prefix))
        .map(str::to_owned)
        .unwrap_or_else(|| panic!("no line {prefix}:\n{}", shell.describe()))
}

/// The open findings of the table by severity, less none (no editing page).
fn errors(shell: &ShellModel) -> usize {
    shell.messages_counts().expect("the counts").errors
}

// ---------------------------------------------------------------------------------------------------------------
// The IP tool (as in `tests/part3.rs`)

fn sheet(s: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    s.canvas().unwrap().page().snapper.page_to_sheet(p)
}

fn grid_point(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.page_of_index(k.0), sn.page_of_index(k.1))
}

/// Places an interruption point named `name` at grid index `k` of the shown page, with direction `direction`.
fn place_ip(s: &mut ShellModel, name: &str, k: (i64, i64), direction: &str) {
    arm_ip(s, name, k, direction);
    let rev = s.revision();
    done(s, "canvas.activate", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one IP placed");
    done(s, "ui.dismiss", json!({}));
}

/// Arms the IP tool with name `name` and direction `direction` at grid index `k` of the shown page.
fn arm_ip(s: &mut ShellModel, name: &str, k: (i64, i64), direction: &str) {
    done(s, "tool.ip", json!({}));
    let p = grid_point(s, k);
    let q = sheet(s, p);
    done(s, "cursor.set", at(q, json!({})));
    done(s, "ip.name", json!({ "text": name }));
    for _ in 0..3 {
        let st = common::state(s);
        if st["canvas"]["ghost"]["direction"] == direction {
            break;
        }
        done(s, "tool.ip_direction", json!({}));
    }
    assert_eq!(common::state(s)["canvas"]["ghost"]["direction"], direction);
}

// ---------------------------------------------------------------------------------------------------------------
// MS1: the pane, its rows and Enter

#[test]
fn ms1_the_pane_lists_the_findings_and_enter_opens_ef08() {
    let tmp = Tmp::new("msg-ms1");
    let mut shell = tmp.shell("102");
    let calls = shell.session_calls().navigator_table;
    done(&mut shell, "messages.show", json!({}));
    assert!(shell.navigator_open(NavKind::Messages));
    assert_eq!(shell.focus(), Some(Focus::Navigator(NavKind::Messages)));
    // The bottom dock: below the page view, beside the find pane when it is open.
    assert!(
        shell
            .layout()
            .panes()
            .contains(&PaneKind::Navigator(NavKind::Messages))
    );
    assert_eq!(
        shell.layout().describe(),
        "split horizontal [split vertical [\"Pages\" 50%, \"Properties\" 50%] 23%, split vertical [\"Page view\" 65%, \"Messages\" 35%] 77%]"
    );
    assert_eq!(shell.session_calls().navigator_table, calls + 1);
    let t = table(&shell);
    let counts = shell.messages_counts().unwrap();
    let open = |sev: &str| {
        t.nodes
            .iter()
            .filter(|n| n.role == NavRole::Finding && n.cells[0] == sev && n.cells[6] == "open")
            .count()
    };
    assert_eq!(
        (counts.errors, counts.warnings, counts.infos, counts.pending),
        (open("error"), open("warning"), open("info"), 0)
    );
    let header = line(&shell, "navigator \"Messages\"");
    assert!(
        header.starts_with(
            "navigator \"Messages\" kind messages view tree sort \"Label\" ascending filter \"\" rows "
        ),
        "{header}"
    );
    assert_eq!(
        line(&shell, "  messages scheme "),
        format!(
            "  messages scheme \"drafting\" errors {} warnings {} infos {} waived {} pending 0",
            counts.errors, counts.warnings, counts.infos, counts.waived
        )
    );
    // The provider's order in the default label sort: Errors first, its rules in ID order.
    let shown = shell.shown_rows(NavKind::Messages);
    assert_eq!(shown[0].text, "Errors");
    assert_eq!(shown[0].role, "severity");
    assert_eq!(shown[1].role, "rule");
    // EF-08: Enter opens the page of -3W1's first drawable with it selected.
    let ef08 = finding(&shell, "EW-CORE-033", "-3W1");
    let first = ef08.drawables.first().cloned().expect("-3W1 is drawn");
    done(
        &mut shell,
        "navigator.open",
        json!({"kind": "messages", "key": ef08.key}),
    );
    assert_eq!(folio(&shell), first.folio);
    let selected: Vec<_> = shell.selection().items.iter().map(|e| e.0).collect();
    assert_eq!(selected, ef08.open.as_ref().unwrap().select);
    assert!(selected.contains(&first.record));
    // The cursor's finding has its detail lines; its row line reads as the framework's with the role `finding`.
    let row = shell
        .describe()
        .lines()
        .find(|l| l.contains("finding \"") && l.contains("-3W1"))
        .map(str::to_owned)
        .unwrap();
    assert!(row.starts_with("  row level 3 finding \""), "{row}");
    assert!(row.contains(" badge \"error\""), "{row}");
    assert!(row.ends_with(" cursor"), "{row}");
    assert_eq!(
        line(&shell, "  detail rule "),
        format!(
            "  detail rule \"EW-CORE-033\" title \"{}\" kind completeness severity error",
            ef08_title()
        )
    );
    assert_eq!(
        line(&shell, "  detail message "),
        format!("  detail message \"{}\"", ef08.cells[2])
    );
    // A no-op dispatch makes no call; a revision makes one; a closed pane makes none.
    let calls = shell.session_calls().navigator_table;
    done(
        &mut shell,
        "navigator.row",
        json!({"kind": "messages", "step": 1}),
    );
    done(&mut shell, "ui.describe", json!({}));
    assert_eq!(shell.session_calls().navigator_table, calls);
    done(&mut shell, "messages.show", json!({"on": false}));
    assert!(!shell.navigator_open(NavKind::Messages));
    shell.shutdown();
}

fn ef08_title() -> String {
    ew_viewmodel::ew_api::checks::rule("EW-CORE-033")
        .unwrap()
        .meta()
        .title
        .to_owned()
}

// ---------------------------------------------------------------------------------------------------------------
// MS2: pending (FR-UI-041, amended by D16)

#[test]
fn ms2_a_new_unpaired_point_on_the_editing_page_is_pending_until_the_tab_moves() {
    let tmp = Tmp::new("msg-ms2");
    let mut shell = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut shell, "view.zoom_actual", json!({}));
    done(&mut shell, "messages.show", json!({}));
    let e0 = errors(&shell);
    // Golden A's page 101 already holds open completeness findings (EW-CORE-034, EW-CORE-041): they turn pending too.
    let p0 = completeness_on(&shell, "101");
    assert!(findings(&shell, "EW-CORE-051", "MSGTEST").is_empty());
    place_ip(&mut shell, "MSGTEST", (60, 3), "target");
    assert_eq!(shell.editing_folio().as_deref(), Some("101"));
    let row = finding(&shell, "EW-CORE-051", "MSGTEST");
    assert_eq!(row.cells[0], "error");
    assert!(shell.row_pending(NavKind::Messages, &row.key));
    let counts = shell.messages_counts().unwrap();
    assert_eq!(
        (counts.errors, counts.pending),
        (e0, p0 + 1),
        "errors unchanged, the new finding pending"
    );
    let shown = shell
        .shown_rows(NavKind::Messages)
        .into_iter()
        .find(|r| r.key == row.key)
        .unwrap();
    assert!(shown.finding.is_some_and(|f| f.pending && f.severity == "error"));
    assert!(
        shown.name.ends_with(", finding, error, pending"),
        "{}",
        shown.name
    );
    assert!(
        shell
            .describe()
            .lines()
            .any(|l| l.contains("MSGTEST") && l.contains(" badge \"error\" badge \"pending\"")),
        "{}",
        shell.describe()
    );
    assert!(line(&shell, "  messages scheme ").ends_with(&format!(" pending {}", p0 + 1)));
    // The tab shows 102: the editing page is gone and the finding is an error.
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    assert_eq!(shell.editing_folio(), None);
    assert!(!shell.row_pending(NavKind::Messages, &row.key));
    assert_eq!(shell.messages_counts().unwrap().errors, e0 + 1);
    assert_eq!(shell.messages_counts().unwrap().pending, 0);
    // Back on 101 nothing was committed there since: still an error.
    done(&mut shell, "page.goto", json!({"folio": "101"}));
    assert!(!shell.row_pending(NavKind::Messages, &row.key));
    shell.shutdown();
}

/// The status bar's findings field (WP-S2.17 D12), empty when it has none.
fn status_findings(shell: &ShellModel) -> String {
    ew_viewmodel::status::bar(shell, false)
        .into_iter()
        .find(|f| f.kind == ew_viewmodel::status::StatusKind::Findings)
        .map(|f| f.text)
        .unwrap_or_default()
}

/// The `n <word>` count of the field `text` (`6 warnings · 2 pending on this page`), 0 when it has none.
fn count_in(text: &str, one: &str, many: &str) -> usize {
    text.split(" · ")
        .find_map(|p| {
            if p == format!("1 {one}") {
                return Some(1);
            }
            p.strip_suffix(&format!(" {many}"))?.parse().ok()
        })
        .unwrap_or(0)
}

/// FT-710 (rv-vis-1 frictions 18 and 23): the check's toast, the status bar and the Messages tab's count chip give
/// the same numbers for the same scope, through one rule ([`MessageCounts::open`]: the project's open findings without
/// the pending and the waived ones); the status bar adds the pending ones as their own part, so a count that changes
/// when the tab leaves the page says why.
#[test]
fn ft710_the_toast_the_status_bar_and_the_chip_agree() {
    let tmp = Tmp::new("msg-ft710");
    let mut shell = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut shell, "view.zoom_actual", json!({}));
    done(&mut shell, "messages.show", json!({}));
    let agree = |shell: &ShellModel, state: &str| {
        let c = shell.messages_counts().expect("the counts");
        let status = status_findings(shell);
        assert_eq!(
            count_in(&status, "error", "errors"),
            c.errors,
            "{state}: {status}"
        );
        assert_eq!(
            count_in(&status, "warning", "warnings"),
            c.warnings,
            "{state}: {status}"
        );
        assert_eq!(count_in(&status, "info", "infos"), c.infos, "{state}: {status}");
        assert_eq!(
            count_in(&status, "pending on this page", "pending on this page"),
            c.pending,
            "{state}: {status}"
        );
        assert_eq!(
            shell.messages_badge(),
            c.errors + c.warnings + c.infos,
            "{state}: the chip"
        );
        c
    };
    // After Run checks: nothing pending, and the toast says the status bar's numbers.
    done(&mut shell, "messages.check", json!({}));
    let c = agree(&shell, "after the check");
    assert_eq!(c.pending, 0);
    let toast = shell.toast().expect("the check toast").text.clone();
    assert_eq!(
        count_in(
            &toast.replace(", ", " · ").replace("Check: ", ""),
            "warning",
            "warnings"
        ),
        c.warnings,
        "{toast}"
    );
    // An edit on 101 makes its open completeness findings pending: the status bar says so as its own part.
    place_ip(&mut shell, "MSGTEST", (60, 3), "target");
    let pending = agree(&shell, "after an edit on 101");
    assert!(pending.pending > 0, "{pending:?}");
    assert!(
        status_findings(&shell).contains("pending on this page"),
        "{}",
        status_findings(&shell)
    );
    // Leaving the page ends them: they count again, and the field no longer names a pending part.
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    let left = agree(&shell, "on 102");
    assert_eq!(left.pending, 0);
    assert_eq!(
        left.open(),
        pending.open() + pending.pending,
        "the pending findings count again"
    );
    // No count changes without an edit or a check: back on 101 it stays.
    done(&mut shell, "page.goto", json!({"folio": "101"}));
    assert_eq!(agree(&shell, "back on 101"), left);
    shell.shutdown();
}

#[test]
fn ms2_run_checks_ends_pending_and_a_conflict_is_never_pending() {
    let tmp = Tmp::new("msg-ms2b");
    let mut shell = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut shell, "view.zoom_actual", json!({}));
    done(&mut shell, "messages.show", json!({}));
    place_ip(&mut shell, "MSGTEST", (60, 3), "target");
    let row = finding(&shell, "EW-CORE-051", "MSGTEST");
    assert!(shell.row_pending(NavKind::Messages, &row.key));
    let calls = shell.session_calls().navigator_table;
    done(&mut shell, "messages.check", json!({}));
    assert_eq!(
        shell.session_calls().navigator_table,
        calls + 1,
        "Run checks fetches the table again"
    );
    assert!(!shell.row_pending(NavKind::Messages, &row.key));
    assert_eq!(shell.messages_counts().unwrap().pending, 0);
    // FT-549: the check reports its findings by severity, the same counts as the pane's header.
    let c = shell.messages_counts().unwrap();
    let toast = shell.toast().expect("the check toast").text.clone();
    assert!(
        toast.starts_with(&format!("Check: {} error", c.errors))
            && toast.contains(&format!("{} warning", c.warnings)),
        "{toast} / {c:?}"
    );
    // A second source of an existing name on 101 is EW-CORE-052, which prevents the commit (FT-194).
    let e_before = errors(&shell);
    done(&mut shell, "navigator.show", json!({"kind": "ips"}));
    let ips = shell.nav(NavKind::Ips).and_then(|n| n.table()).cloned().unwrap();
    let source = ips
        .nodes
        .iter()
        .find(|n| n.role == NavRole::Ip && n.cells[2] == "source" && n.cells[4] == "no")
        .map(|n| n.cells[0].clone())
        .expect("a source point in Golden A");
    done(&mut shell, "navigator.show", json!({"kind": "ips", "on": false}));
    let rev = shell.revision();
    arm_ip(&mut shell, &source, (60, 6), "source");
    match shell.dispatch("canvas.activate", &json!({})) {
        Outcome::Failed { message } => assert!(message.contains("EW-CORE-052"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(shell.revision(), rev, "nothing committed");
    done(&mut shell, "ui.dismiss", json!({}));
    // Page 101 renumbered to 250, outside section 1's range: EW-CORE-021 (conflict), never pending, while the commit
    // makes the page the editing page again (so MSGTEST's finding is pending once more).
    let page = shell
        .page_rows()
        .iter()
        .find(|r| r.folio == "101")
        .map(|r| r.id.to_string())
        .unwrap();
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "page.renumber", "params": {"scope": {"pages": [page]}, "start": 250}}),
    );
    let conflict = finding(&shell, "EW-CORE-021", "folio 250");
    assert!(
        conflict
            .facets
            .contains(&("kind".to_owned(), "conflict".to_owned()))
    );
    assert!(!shell.row_pending(NavKind::Messages, &conflict.key));
    assert!(shell.row_pending(NavKind::Messages, &row.key));
    // The conflict counts as an error; MSGTEST's finding left the errors for `pending`.
    assert_eq!(errors(&shell), e_before);
    // Pending: MSGTEST's finding and the open completeness findings Golden A already has on the page.
    assert_eq!(
        shell.messages_counts().unwrap().pending,
        completeness_on(&shell, "250")
    );
    assert!(completeness_on(&shell, "250") >= 1);
    shell.shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// MS3: the quick fix (D10)

#[test]
fn ms3_the_quick_fix_is_previewed_applied_in_one_step_and_undone() {
    let tmp = Tmp::new("msg-ms3");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    let page = shell
        .page_rows()
        .iter()
        .find(|r| r.folio == "102")
        .map(|r| r.id.to_string())
        .unwrap();
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "page", "targets": [page], "path": "description_override",
            "value": "Overridden"}}),
    );
    done(&mut shell, "messages.show", json!({}));
    let row = finding(&shell, "EW-CORE-023", "102");
    assert!(row.badges.contains(&"fix".to_owned()));
    assert_eq!(row.cells[5], "drop override");
    // The detail of the cursor's finding names the fix.
    done(
        &mut shell,
        "navigator.row",
        json!({"kind": "messages", "key": row.key}),
    );
    assert_eq!(line(&shell, "  detail fix "), "  detail fix \"drop override\"");
    // A finding without a fix refuses.
    let ef08 = finding(&shell, "EW-CORE-033", "-3W1");
    assert_eq!(
        refused(&mut shell, "messages.fix", json!({"key": ef08.key})),
        "reason-no-fix"
    );
    // The fix opens the card, whatever FR-UI-044 would say; nothing is committed yet.
    let rev = shell.revision();
    done(&mut shell, "messages.fix", json!({"key": row.key}));
    assert_eq!(shell.revision(), rev);
    let card = shell.properties().card().expect("the quick-fix card");
    assert_eq!(
        card.origin,
        CardOrigin::QuickFix {
            rule: "EW-CORE-023".into()
        }
    );
    assert_eq!(card.preview.name, "fields.set");
    let card_line = line(&shell, "card ");
    assert!(
        card_line.starts_with("card ready \"fields.set\" targets 1 changes 1 "),
        "{card_line}"
    );
    assert!(card_line.ends_with(" fix \"EW-CORE-023\""), "{card_line}");
    // WP-S2.09 part 3 (§8.1): the card opens in the Preview tab like every origin; the Messages pane stays docked.
    assert!(shell.preview_shown());
    assert!(shell.navigator_open(ew_viewmodel::navigators::NavKind::Messages));
    // Apply: one transaction removes the finding; the origin annotation is kept for the history panel.
    let undo_before = shell.can_undo();
    done(&mut shell, "navigator.apply", json!({}));
    assert_eq!(shell.revision(), rev.map(|r| r + 1));
    assert!(shell.properties().card().is_none());
    assert!(undo_before);
    assert!(findings(&shell, "EW-CORE-023", "102").is_empty());
    assert_eq!(
        shell.origin_annotation(shell.revision().unwrap()),
        Some("quick fix EW-CORE-023")
    );
    // WP-S2.09 part 3 (§8.4): the history panel shows the annotation as the entry's origin.
    done(&mut shell, "history.show", json!({"on": true}));
    let entry = line(&shell, "  entry ");
    assert!(entry.contains(" origin \"quick fix EW-CORE-023\""), "{entry}");
    done(&mut shell, "history.show", json!({"on": false}));
    // One undo step restores it.
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(findings(&shell, "EW-CORE-023", "102").len(), 1);
    shell.shutdown();
}

#[test]
fn ms3_a_stale_card_previews_again_and_a_read_only_project_refuses() {
    let tmp = Tmp::new("msg-ms3b");
    let mut ro = tmp.shell("102");
    done(&mut ro, "messages.show", json!({}));
    assert_eq!(refused(&mut ro, "messages.fix", json!({})), "reason-read-only");
    ro.shutdown();
    let tmp = Tmp::new("msg-ms3c");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    let page = shell
        .page_rows()
        .iter()
        .find(|r| r.folio == "102")
        .map(|r| r.id.to_string())
        .unwrap();
    let set = |v: &str| {
        json!({"cmd": "fields.set", "params": {"kind": "page", "targets": [page], "path": "description_override",
            "value": v}})
    };
    done(&mut shell, "command.run", set("A"));
    done(&mut shell, "messages.show", json!({}));
    let row = finding(&shell, "EW-CORE-023", "102");
    done(&mut shell, "messages.fix", json!({"key": row.key}));
    // A commit meanwhile makes the card stale: Apply previews again and shows a fresh card.
    done(&mut shell, "command.run", set("B"));
    assert!(line(&shell, "card ").starts_with("card stale "));
    let rev = shell.revision();
    done(&mut shell, "preview.apply", json!({}));
    assert_eq!(shell.revision(), rev, "a stale card commits nothing");
    assert!(line(&shell, "card ").starts_with("card ready "));
    done(&mut shell, "preview.apply", json!({}));
    assert!(findings(&shell, "EW-CORE-023", "102").is_empty());
    shell.shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// MS4: F8, Shift+F8 and F1

#[test]
fn ms4_f8_walks_and_wraps_and_opens_the_pane_and_f1_opens_the_rule_page() {
    let tmp = Tmp::new("msg-ms4");
    let mut shell = tmp.shell("102");
    assert!(!shell.navigator_open(NavKind::Messages));
    let next = ew_viewmodel::actions::ACTIONS
        .iter()
        .find(|a| a.id == "messages.next")
        .unwrap();
    assert_eq!(next.shortcut, ["F8"]);
    let previous = ew_viewmodel::actions::ACTIONS
        .iter()
        .find(|a| a.id == "messages.previous")
        .unwrap();
    assert_eq!(previous.shortcut, ["Shift+F8"]);
    // F8 with the pane closed docks it (the focus stays) and goes to the first finding.
    let focus = shell.focus();
    done(&mut shell, "messages.next", json!({}));
    assert!(shell.navigator_open(NavKind::Messages));
    assert_eq!(shell.focus(), focus);
    let t = table(&shell);
    let order: Vec<String> = shell
        .shown_rows(NavKind::Messages)
        .iter()
        .filter(|r| r.role == "finding")
        .map(|r| r.key.clone())
        .collect();
    assert!(order.len() >= 2);
    assert_eq!(
        shell.nav_cursor(NavKind::Messages).as_deref(),
        Some(order[0].as_str())
    );
    // It opened the finding: the page of its open target.
    let first = t.nodes.iter().find(|n| n.key == order[0]).unwrap();
    if let Some(open) = &first.open {
        let row = shell
            .page_rows()
            .iter()
            .find(|r| r.id == open.page)
            .unwrap()
            .folio
            .clone();
        assert_eq!(folio(&shell), row);
    }
    done(&mut shell, "messages.next", json!({}));
    assert_eq!(
        shell.nav_cursor(NavKind::Messages).as_deref(),
        Some(order[1].as_str())
    );
    // Shift+F8 twice wraps to the last finding.
    done(&mut shell, "messages.previous", json!({}));
    // The last finding has no page (EW-RDS-002 on a structure node): the cursor goes to it and nothing opens.
    done(&mut shell, "messages.previous", json!({}));
    assert_eq!(
        shell.nav_cursor(NavKind::Messages).as_deref(),
        order.last().map(String::as_str)
    );
    // F8 from the last wraps to the first; a collapsed group's findings are walked too (and revealed).
    done(&mut shell, "messages.next", json!({}));
    assert_eq!(
        shell.nav_cursor(NavKind::Messages).as_deref(),
        Some(order[0].as_str())
    );
    // F1 on EW-CORE-033 in the Messages context opens its rule page.
    let ef08 = finding(&shell, "EW-CORE-033", "-3W1");
    done(
        &mut shell,
        "navigator.row",
        json!({"kind": "messages", "key": ef08.key}),
    );
    done(
        &mut shell,
        "ui.focus",
        json!({"area": "navigator", "kind": "messages"}),
    );
    done(&mut shell, "help.documentation", json!({}));
    assert_eq!(
        line(&shell, "help: "),
        "help: reference/rules/ew-core.html#ew-core-033"
    );
    assert_eq!(
        ew_viewmodel::help::rule_page("EW-CORE-033"),
        Some(("reference/rules/ew-core.md".to_owned(), "ew-core-033".to_owned()))
    );
    // On a severity group, F1 falls back to the context's topic.
    let group = t.nodes.iter().find(|n| n.role == NavRole::Severity).unwrap();
    done(
        &mut shell,
        "navigator.row",
        json!({"kind": "messages", "key": group.key}),
    );
    done(&mut shell, "help.documentation", json!({}));
    assert_ne!(
        line(&shell, "help: "),
        "help: reference/rules/ew-core.html#ew-core-033"
    );
    shell.shutdown();
}

#[test]
fn ms4_f8_with_no_findings_on_show_refuses() {
    let tmp = Tmp::new("msg-ms4b");
    let mut shell = tmp.shell("102");
    done(&mut shell, "messages.show", json!({}));
    done(
        &mut shell,
        "navigator.filter",
        json!({"kind": "messages", "text": "nothing-matches-this-text"}),
    );
    assert_eq!(
        refused(&mut shell, "messages.next", json!({})),
        "reason-no-findings"
    );
    shell.shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// MS6: the words, the actions, the counts

#[test]
fn ms6_the_sixteen_words_the_five_actions_and_the_counts() {
    use ew_viewmodel::describe::{MSG_WORDS, VOCABULARY};
    for w in MSG_WORDS {
        assert!(VOCABULARY.contains(w), "{w}");
    }
    for id in [
        "messages.show",
        "messages.next",
        "messages.previous",
        "messages.fix",
        "messages.check",
    ] {
        assert!(ew_viewmodel::actions::ACTIONS.iter().any(|a| a.id == id), "{id}");
    }
    // The words the describe text reaches on Golden A with a fix card open (the kinds `safety` and `integrity` need
    // findings of those kinds, which Golden A's drafting run has none of).
    let tmp = Tmp::new("msg-words");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    let page = shell
        .page_rows()
        .iter()
        .find(|r| r.folio == "102")
        .map(|r| r.id.to_string())
        .unwrap();
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "page", "targets": [page], "path": "description_override",
            "value": "Overridden"}}),
    );
    done(&mut shell, "messages.show", json!({}));
    let row = finding(&shell, "EW-CORE-023", "102");
    done(
        &mut shell,
        "navigator.row",
        json!({"kind": "messages", "key": row.key}),
    );
    let mut words: BTreeSet<String> = BTreeSet::new();
    let mut collect = |shell: &ShellModel| {
        for l in shell.describe().lines() {
            words.extend(common::structural_tokens(l));
        }
    };
    collect(&shell);
    done(&mut shell, "messages.fix", json!({}));
    collect(&shell);
    let ef08 = finding(&shell, "EW-CORE-033", "-3W1");
    done(
        &mut shell,
        "navigator.row",
        json!({"kind": "messages", "key": ef08.key}),
    );
    collect(&shell);
    // `conflict` needs a conflict finding (MS2's second test reaches it on its row's facet, not in the text: the detail
    // line of an EW-CORE-052 finding prints it); `safety` and `integrity` need findings of those kinds, which the
    // drafting run of Golden A has none of.
    let unreachable = ["safety", "conflict", "integrity"];
    for w in MSG_WORDS.iter().filter(|w| !unreachable.contains(w)) {
        assert!(words.contains(*w), "`{w}` is reached");
    }
    shell.shutdown();
}

/// FT-528: `navigator.row {key}` to a finding under a collapsed group expands its path and puts the cursor on it; a key
/// the filter hides is refused and leaves the collapsed groups as they were.
#[test]
fn ft528_navigator_row_into_a_collapsed_group_expands_its_path() {
    let tmp = Tmp::new("msg-ft528");
    let mut shell = tmp.shell("102");
    done(&mut shell, "messages.show", json!({}));
    let ef08 = finding(&shell, "EW-CORE-033", "-3W1");
    let rows = shell.shown_rows(NavKind::Messages);
    let row = rows.iter().find(|r| r.key == ef08.key).expect("EF-08 on show");
    let group = row.parent.clone().expect("EF-08 under a group");
    done(
        &mut shell,
        "navigator.toggle",
        json!({"kind": "messages", "key": group}),
    );
    assert!(
        !shell
            .shown_rows(NavKind::Messages)
            .iter()
            .any(|r| r.key == ef08.key),
        "hidden under its collapsed group"
    );
    done(
        &mut shell,
        "navigator.row",
        json!({"kind": "messages", "key": ef08.key}),
    );
    assert_eq!(
        shell.nav_cursor(NavKind::Messages).as_deref(),
        Some(ef08.key.as_str())
    );
    let shown = shell.shown_rows(NavKind::Messages);
    assert_eq!(
        shown.iter().find(|r| r.key == group).and_then(|r| r.expanded),
        Some(true),
        "the group is expanded again"
    );
    // A filter that hides EF-08: the key is refused and the collapsed group stays collapsed.
    done(
        &mut shell,
        "navigator.toggle",
        json!({"kind": "messages", "key": group}),
    );
    done(
        &mut shell,
        "navigator.filter",
        json!({"kind": "messages", "text": "F55"}),
    );
    assert!(matches!(
        shell.dispatch("navigator.row", &json!({"kind": "messages", "key": ef08.key})),
        ew_viewmodel::shell::Outcome::Failed { .. }
    ));
    done(
        &mut shell,
        "navigator.filter",
        json!({"kind": "messages", "text": ""}),
    );
    assert_eq!(
        shell
            .shown_rows(NavKind::Messages)
            .iter()
            .find(|r| r.key == group)
            .and_then(|r| r.expanded),
        Some(false),
        "the refused move left the group collapsed"
    );
    shell.shutdown();
}

/// FT-549: with the Messages pane closed, Run checks still reports what it found, errors and warnings always named
/// (`Check: 0 errors, 1 warning`), and the summary text follows the singular and plural forms.
#[test]
fn ft549_check_reports_its_counts_with_the_pane_closed() {
    let tmp = Tmp::new("msg-ft549");
    let mut shell = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut shell, "messages.check", json!({}));
    let toast = shell.toast().expect("the check toast").text.clone();
    assert!(toast.starts_with("Check: "), "{toast}");
    assert!(toast.contains("error") && toast.contains("warning"), "{toast}");
}
