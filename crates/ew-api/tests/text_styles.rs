// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Per-project text styles through the session (ADR-0032; WP-S1.12 text styles, acceptance TS-1, TS-2, TS-3, TS-5,
//! TS-6): the member in `project.jsonl`, the export job's scenes, EW-CORE-061 and `project.settings.set`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ew_api::StoreOptions;
use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::output::ew_dl::{Item, ItemKind};
use ew_api::{
    AnyCommand, CmdError, NewProjectKind, Origin, ProjectTemplate, Recovery, Session, SessionOptions,
};
use ew_commands::testkit::seeded_ids;
use serde_json::{Value, json};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

/// The em heights of the built-in table that the tests follow: 5, 7 and 13 QET px.
const PX5: i64 = 1_322_917;
const PX7: i64 = 1_852_083;
const PX13: i64 = 3_439_583;

/// The text the built-in basic project writes into `project.jsonl` (ADR-0032 decision 1's example).
const NEW_PROJECT_MEMBER: &str = r#""text_styles":{"cp.small":{"size":1852083},"marker":{"size":1852083}}"#;

fn temp(tag: &str) -> PathBuf {
    ew_testdir::fresh_dir(&format!("ew-api-textstyles-{tag}"))
}

fn store(state: &Path) -> StoreOptions {
    let mut o = StoreOptions::new("ElektraWatt test");
    o.state_dir = Some(state.to_path_buf());
    o
}

fn options(seed: u64) -> SessionOptions {
    SessionOptions {
        ids: Some(seeded_ids(seed)),
        ..SessionOptions::default()
    }
}

fn project_text(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("project.jsonl")).unwrap()
}

fn copy_of_golden(seed: u64) -> Session {
    Session::new_project(
        ProjectTemplate::Directory(Path::new(GOLDEN_A)),
        NewProjectKind::Schematic,
        options(seed),
    )
    .unwrap()
}

fn set_styles(value: Value) -> AnyCommand {
    AnyCommand {
        name: "project.settings.set".into(),
        params: json!({"member": "text_styles", "value": value}),
    }
}

/// Previews and commits `project.settings.set text_styles` (a bulk command).
fn commit_styles(s: &mut Session, value: Value) {
    let p = s.preview(&set_styles(value)).unwrap();
    s.commit(p).unwrap();
}

fn count_runs(items: &[Item], out: &mut BTreeMap<i64, usize>) {
    for item in items {
        match &item.kind {
            ItemKind::Glyphs(run) => *out.entry(run.size.0).or_default() += 1,
            ItemKind::Group(g) => count_runs(&g.items, out),
            _ => {}
        }
    }
}

/// The number of glyph runs per em height over every page of the export job (`ew_api::output::page_scene`, the scene
/// the PDF and SVG writers draw).
fn run_sizes(s: &Session) -> BTreeMap<i64, usize> {
    let q = s.query();
    let order = q.get::<ew_derive::PageOrder>(());
    let mut out = BTreeMap::new();
    for entry in &order.entries {
        let scene = ew_api::output::page_scene(&q, entry);
        for (_, list) in &scene.layers {
            count_runs(&list.items, &mut out);
        }
    }
    out
}

fn n(sizes: &BTreeMap<i64, usize>, em: i64) -> usize {
    sizes.get(&em).copied().unwrap_or(0)
}

/// The subjects of the EW-CORE-061 findings.
fn core061(s: &Session) -> Vec<String> {
    s.check(&PRE_DELIVERY, CheckMode::Release)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-CORE-061")
        .map(|f| f.discriminator)
        .collect()
}

/// TS-1: Golden A writes no member and keeps EF-12's two EW-CORE-061 findings.
#[test]
fn ts1_golden_a_has_no_member_and_keeps_its_two_findings() {
    assert!(!project_text(Path::new(GOLDEN_A)).contains("text_styles"));
    let (golden, _) = Session::open_read_only(Path::new(GOLDEN_A), "ElektraWatt test").unwrap();
    assert_eq!(core061(&golden), ["cp.small", "marker"]);
}

/// TS-2: a new project's `project.jsonl` holds exactly the two overrides, and they round-trip.
#[test]
fn ts2_a_new_project_writes_the_two_overrides() {
    let work = temp("ts2");
    let mut s =
        Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Schematic, options(1)).unwrap();
    let styles = ew_derive::text_styles(&s.query());
    let em = |key: &str| styles.iter().find(|r| r.key.as_str() == key).unwrap().em.0;
    assert_eq!((em("cp.small"), em("marker"), em("pin")), (PX7, PX7, 2_116_667));
    let dir = work.join("p.ew");
    s.create(&dir, store(&work.join("state"))).unwrap();
    s.close();
    let text = project_text(&dir);
    assert_eq!(text.matches("text_styles").count(), 1, "{text}");
    assert!(text.contains(NEW_PROJECT_MEMBER), "{text}");
    let (reopened, _) = Session::open(
        &dir,
        store(&work.join("state")),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .unwrap();
    assert!(core061(&reopened).is_empty());
    reopened.close();
    assert_eq!(project_text(&dir), text);
    let _ = std::fs::remove_dir_all(&work);
}

/// TS-2: with the new-project overrides, every wire marker and small pin number prints at 7 px in the export job's
/// scenes, and EW-CORE-061 reports nothing (over Golden A's pages, which print both).
#[test]
fn ts2_the_new_project_overrides_print_markers_and_pin_numbers_at_7_px() {
    let mut s = copy_of_golden(2);
    let before = run_sizes(&s);
    assert!(n(&before, PX5) > 0, "{before:?}");
    assert_eq!(core061(&s), ["cp.small", "marker"]);
    commit_styles(
        &mut s,
        json!({"cp.small": {"size": PX7}, "marker": {"size": PX7}}),
    );
    let after = run_sizes(&s);
    assert_eq!(n(&after, PX5), 0, "{after:?}");
    assert_eq!(n(&after, PX7), n(&before, PX7) + n(&before, PX5));
    assert!(core061(&s).is_empty());
}

/// TS-3 (FR-SYM-013's acceptance): `tag.main` at 4 mm changes every device tag on every page in one undo step; undo
/// restores; a size of 100 nm is refused at `/text_styles/tag.main/size`.
#[test]
fn ts3_a_tag_style_override_resizes_every_tag_in_one_undo_step() {
    let mut s = copy_of_golden(3);
    let before = run_sizes(&s);
    let tags = n(&before, PX13);
    assert!(tags > 10, "{before:?}");
    assert_eq!(n(&before, 4_000_000), 0);
    commit_styles(&mut s, json!({"tag.main": {"size": 4_000_000}}));
    let after = run_sizes(&s);
    assert_eq!((n(&after, PX13), n(&after, 4_000_000)), (0, tags));
    s.undo(&Origin::Gui).unwrap();
    assert_eq!(run_sizes(&s), before);
    match s.preview(&set_styles(json!({"tag.main": {"size": 100}}))) {
        Err(CmdError::InvalidParams { path, .. }) => assert_eq!(path, "/text_styles/tag.main/size"),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// TS-5: an entry's unknown member is written back as read, through a save and a reopen.
#[test]
fn ts5_an_unknown_member_of_an_entry_survives_a_save() {
    let work = temp("ts5");
    let mut s =
        Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Schematic, options(5)).unwrap();
    let value =
        json!({"marker": {"size": PX7, "weight": "bold", "family": "mono"}, "note.big": {"size": 6_000_000}});
    commit_styles(&mut s, value);
    let dir = work.join("p.ew");
    s.create(&dir, store(&work.join("state"))).unwrap();
    s.close();
    let text = project_text(&dir);
    let member = r#""text_styles":{"marker":{"size":1852083,"weight":"bold","family":"mono"},"note.big":{"size":6000000}}"#;
    assert!(text.contains(member), "{text}");
    let (reopened, _) = Session::open(
        &dir,
        store(&work.join("state")),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .unwrap();
    let styles = ew_derive::text_styles(&reopened.query());
    let big = styles.iter().find(|r| r.key.as_str() == "note.big").unwrap();
    assert_eq!(big.em.0, 6_000_000);
    reopened.close();
    assert_eq!(project_text(&dir), text);
    let _ = std::fs::remove_dir_all(&work);
}

/// TS-6: a company basic project without the member makes a project without it; `save_as_basic` of a new project
/// keeps the overrides, and a project made from that basic project has them.
#[test]
fn ts6_a_basic_project_is_copied_as_it_is() {
    let work = temp("ts6");
    let state = work.join("state");
    let copy = copy_of_golden(6);
    assert!(copy.query().project().settings.text_styles.is_empty());
    let fresh =
        Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Schematic, options(7)).unwrap();
    let basic = work.join("basic.ew");
    fresh.save_as_basic(&basic, false, false, store(&state)).unwrap();
    assert!(project_text(&basic).contains(NEW_PROJECT_MEMBER));
    let from_basic = Session::new_project(
        ProjectTemplate::Directory(&basic),
        NewProjectKind::Schematic,
        options(8),
    )
    .unwrap();
    assert_eq!(
        from_basic.query().project().settings.text_styles,
        fresh.query().project().settings.text_styles
    );
    let _ = std::fs::remove_dir_all(&work);
}
