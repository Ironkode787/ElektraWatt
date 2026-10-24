// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Session::new_project` (FT-307, FR-PRJ-009): the built-in basic project, a company basic project directory,
//! the kind, and the new identities of a copy (doc 07 §2.8).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use ew_api::{NewProjectKind, ProjectTemplate, Recovery, Session, SessionOptions, StoreOptions};
use ew_commands::testkit::seeded_ids;

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

fn temp(tag: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ew-api-newproject-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
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

/// Every UUID that appears in the lines of a session's model.
fn ids_of(session: &Session) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (_, line) in ew_commands::rows::model_lines(&session.world().model).unwrap() {
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        if let Some(id) = v.get("id").and_then(serde_json::Value::as_str) {
            out.insert(id.to_owned());
        }
    }
    out
}

#[test]
fn the_built_in_basic_project_has_one_empty_page_and_round_trips() {
    let work = temp("builtin");
    let mut session =
        Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Schematic, options(1)).unwrap();
    assert_eq!(session.page_labels(), ["1"]);
    assert!(!session.is_dirty());
    let dir = work.join("p.ew");
    session.create(&dir, store(&work.join("state"))).unwrap();
    assert!(!session.is_dirty(), "a created project is saved");
    session.close();
    let text = project_text(&dir);
    assert!(text.contains("\"derivation_profile\":\"2027.1\""), "{text}");
    assert!(text.contains("EN81346-2:2009"), "{text}");
    assert!(
        !text.contains("\"kind\""),
        "a schematic project writes no kind: {text}"
    );

    let (reopened, report) = Session::open(
        &dir,
        store(&work.join("state")),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .unwrap();
    assert!(matches!(report.access, ew_api::Access::Edit));
    assert_eq!(reopened.page_labels(), ["1"]);
    // The page derives and exports on the built-in frame.
    let pdf = reopened
        .export_pdf(&ew_api::output::ExportOptions::default())
        .unwrap();
    assert!(pdf.bytes.starts_with(b"%PDF"));
    reopened.close();
    let _ = std::fs::remove_dir_all(&work);
}

#[test]
fn the_kind_basic_is_written_and_a_basic_project_is_a_template() {
    let work = temp("basic");
    let mut basic =
        Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Basic, options(2)).unwrap();
    let basic_dir = work.join("company.ew");
    basic.create(&basic_dir, store(&work.join("state"))).unwrap();
    basic.close();
    assert!(project_text(&basic_dir).contains("\"kind\":\"basic\""));

    let mut copy = Session::new_project(
        ProjectTemplate::Directory(&basic_dir),
        NewProjectKind::Schematic,
        options(3),
    )
    .unwrap();
    let copy_dir = work.join("new.ew");
    copy.create(&copy_dir, store(&work.join("state"))).unwrap();
    copy.close();
    let text = project_text(&copy_dir);
    assert!(
        !text.contains("\"kind\""),
        "the copy is a schematic project: {text}"
    );
    assert_ne!(text, project_text(&basic_dir), "a new project ID");
    let _ = std::fs::remove_dir_all(&work);
}

#[test]
fn a_directory_template_copies_the_records_under_new_ids() {
    let work = temp("golden");
    let (golden, _) = Session::open_read_only(Path::new(GOLDEN_A), "ElektraWatt test").unwrap();
    let mut copy = Session::new_project(
        ProjectTemplate::Directory(Path::new(GOLDEN_A)),
        NewProjectKind::Schematic,
        options(4),
    )
    .unwrap();
    assert_eq!(copy.page_labels(), golden.page_labels(), "the same pages");
    let (old, new) = (ids_of(&golden), ids_of(&copy));
    assert!(old.len() > 100, "{} records", old.len());
    assert_eq!(old.len(), new.len(), "the same records");
    // Instantiated records are re-identified; library copies keep their IDs (their IDs derive from library and item).
    let shared = old.intersection(&new).count();
    assert!(
        shared < old.len() / 2,
        "{shared} of {} IDs survived; only library copies and the like may",
        old.len()
    );
    // The result is a sound project: it saves, reopens without integrity findings and derives the same page texts.
    let dir = work.join("copy.ew");
    copy.create(&dir, store(&work.join("state"))).unwrap();
    copy.close();
    let (reopened, report) = Session::open(
        &dir,
        store(&work.join("state")),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .unwrap();
    assert!(matches!(report.access, ew_api::Access::Edit));
    assert!(
        ew_api::integrity_items(&report)
            .iter()
            .all(|i| i.severity == ew_api::checks::Severity::Info),
        "{:?}",
        ew_api::integrity_items(&report)
    );
    assert_eq!(reopened.page_labels(), golden.page_labels());
    reopened.close();
    let _ = std::fs::remove_dir_all(&work);
}

#[test]
fn a_directory_that_is_no_project_is_an_error() {
    let work = temp("missing");
    let err = Session::new_project(
        ProjectTemplate::Directory(&work.join("nope.ew")),
        NewProjectKind::Schematic,
        options(5),
    )
    .unwrap_err();
    assert!(matches!(err, ew_api::OpenError::NotADirectory(_)), "{err}");
    let err = Session::new_project(
        ProjectTemplate::Directory(&work),
        NewProjectKind::Schematic,
        options(5),
    )
    .unwrap_err();
    assert!(matches!(err, ew_api::OpenError::NoProject(_)), "{err}");
    let _ = std::fs::remove_dir_all(&work);
}

// ---------------------------------------------------------------------------------------------------------------
// The review's rulings on the copy: ordered lists, derived channel IDs, determinism, page texts

fn lines_of(session: &Session) -> Vec<serde_json::Value> {
    ew_commands::rows::model_lines(&session.world().model)
        .unwrap()
        .into_iter()
        .map(|(key, line)| {
            let mut v: serde_json::Value = serde_json::from_str(&line).unwrap();
            // The row text has no `$k`; the key names the kind.
            v["$k"] = serde_json::Value::from(key.0.name());
            v
        })
        .collect()
}

fn uuid_of(v: &serde_json::Value, member: &str) -> Option<ew_ids::Uuid> {
    v.get(member)
        .and_then(serde_json::Value::as_str)
        .and_then(|s| s.parse().ok())
}

/// The ID map of a copy: `v5(new project, old ID)`, and for a PLC channel `plc_channel_id(new module, number)`.
fn id_map(
    old: &[serde_json::Value],
    new_project: ew_ids::Uuid,
) -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    for v in old
        .iter()
        .filter(|v| v["$k"] != "channel" && v["$k"] != "project")
    {
        if let Some(id) = uuid_of(v, "id") {
            let new: ew_ids::Id<()> = ew_ids::v5::imported_id(new_project, &id.hyphenated().to_string());
            map.insert(id.to_string(), new.as_uuid().to_string());
        }
    }
    for v in old.iter().filter(|v| v["$k"] == "channel") {
        let module = map[&uuid_of(v, "module").unwrap().to_string()].parse().unwrap();
        let number = u16::try_from(v["number"].as_u64().unwrap()).unwrap();
        let new: ew_ids::Id<()> = ew_ids::v5::plc_channel_id(ew_ids::Id::<()>::from_uuid(module), number);
        map.insert(uuid_of(v, "id").unwrap().to_string(), new.as_uuid().to_string());
    }
    map
}

fn copy_of_golden(seed: u64) -> Session {
    Session::new_project(
        ProjectTemplate::Directory(Path::new(GOLDEN_A)),
        NewProjectKind::Schematic,
        options(seed),
    )
    .unwrap()
}

#[test]
fn ordered_lists_keep_their_order_in_a_copy() {
    let (golden, _) = Session::open_read_only(Path::new(GOLDEN_A), "ElektraWatt test").unwrap();
    let copy = copy_of_golden(4);
    let (old, new) = (lines_of(&golden), lines_of(&copy));
    let new_project = uuid_of(new.iter().find(|v| v["$k"] == "project").unwrap(), "id").unwrap();
    let map = id_map(&old, new_project);
    let by_id: std::collections::BTreeMap<String, &serde_json::Value> = new
        .iter()
        .filter_map(|v| uuid_of(v, "id").map(|id| (id.to_string(), v)))
        .collect();
    let mapped = |list: &serde_json::Value| -> Vec<String> {
        list.as_array()
            .unwrap()
            .iter()
            .map(|e| {
                map.get(e.as_str().unwrap())
                    .cloned()
                    .unwrap_or_else(|| e.as_str().unwrap().to_owned())
            })
            .collect()
    };
    let (mut checked, mut unsorted_images) = (0, 0);
    for (kind, member) in [("junction", "sequence"), ("representation", "functions")] {
        for v in old.iter().filter(|v| v["$k"] == kind && v.get(member).is_some()) {
            let expected = mapped(&v[member]);
            let id = map[&uuid_of(v, "id").unwrap().to_string()].clone();
            let copy_line = by_id
                .get(&id)
                .unwrap_or_else(|| panic!("the copy has {kind} {id}"));
            let got: Vec<String> = copy_line[member]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e.as_str().unwrap().to_owned())
                .collect();
            assert_eq!(got, expected, "{kind}.{member} of {id} keeps its order");
            checked += 1;
            let mut sorted = expected.clone();
            sorted.sort();
            if sorted != expected {
                unsorted_images += 1;
            }
        }
    }
    assert!(checked > 50, "{checked} ordered lists compared");
    assert!(
        unsorted_images > 0,
        "the data has an ordered list whose image is not in sorted order, so that this test can catch a re-sort"
    );
}

#[test]
fn plc_channel_ids_are_derived_from_the_new_module() {
    let copy = copy_of_golden(4);
    let lines = lines_of(&copy);
    let channels: Vec<_> = lines.iter().filter(|v| v["$k"] == "channel").collect();
    assert!(!channels.is_empty(), "the sample has PLC channels");
    for c in channels {
        let module = uuid_of(c, "module").unwrap();
        let number = u16::try_from(c["number"].as_u64().unwrap()).unwrap();
        let expected: ew_ids::Id<()> =
            ew_ids::v5::plc_channel_id(ew_ids::Id::<()>::from_uuid(module), number);
        assert_eq!(
            uuid_of(c, "id").unwrap(),
            expected.as_uuid(),
            "channel {number} of {module}"
        );
    }
}

fn files_of(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                let name = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(name, std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

#[test]
fn two_copies_with_one_seed_are_byte_equal() {
    let work = temp("determinism");
    let mut written = Vec::new();
    for n in 0..2 {
        let mut copy = copy_of_golden(9);
        let dir = work.join(format!("copy-{n}.ew"));
        copy.create(&dir, store(&work.join("state"))).unwrap();
        copy.close();
        written.push(files_of(&dir));
    }
    assert!(written[0].len() > 100);
    assert_eq!(
        written[0].keys().collect::<Vec<_>>(),
        written[1].keys().collect::<Vec<_>>()
    );
    for (name, bytes) in &written[0] {
        assert!(
            bytes == &written[1][name],
            "{name} differs between the two copies"
        );
    }
    let _ = std::fs::remove_dir_all(&work);
}

#[test]
fn a_copy_derives_the_page_texts_of_the_original() {
    let (golden, _) = Session::open_read_only(Path::new(GOLDEN_A), "ElektraWatt test").unwrap();
    let copy = copy_of_golden(4);
    let options = ew_api::output::ExportOptions::default();
    let (a, b) = (
        golden.export_pages(&options).unwrap(),
        copy.export_pages(&options).unwrap(),
    );
    assert_eq!(a.pages.len(), b.pages.len());
    assert!(a.pages.len() > 30);
    // The page texts: every quoted text of a page's display list (tags, cross references, titles, report rows) as
    // a sorted list, because the drawing order inside a page may follow record IDs.
    let texts = |content: &dyn std::fmt::Debug| -> Vec<String> {
        let debug = format!("{content:?}");
        let mut out: Vec<String> = debug.split('"').skip(1).step_by(2).map(str::to_owned).collect();
        out.sort();
        out
    };
    let mut total = 0;
    for (i, (x, y)) in a.pages.iter().zip(&b.pages).enumerate() {
        let (tx, ty) = (texts(&x.content), texts(&y.content));
        total += tx.len();
        assert_eq!(tx, ty, "the texts of page {i} ({})", golden.page_labels()[i]);
    }
    assert!(total > 1000, "{total} texts compared");
}

/// The shipped frame and title-block forms (`content/forms`), the `lib/` JSON form `library.embed` takes.
fn shipped_forms() -> Vec<serde_json::Value> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/forms");
    let mut names: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    names.sort();
    names
        .iter()
        .map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap())
        .collect()
}

/// The keys of the frame form the page output gives folio 1 and of its title block.
fn frame_of_folio_1(session: &Session) -> (Option<String>, Option<String>) {
    let q = session.query();
    let order = q.get::<ew_derive::PageOrder>(());
    let entry = order.entries.first().expect("folio 1");
    let frame = (*q.get::<ew_api::output::PageFrameOf>(entry.page)).clone();
    let key = |f: &ew_schema::FormDef| f.meta.key.as_ref().map(|k| k.as_str().to_owned());
    match frame {
        None => (None, None),
        Some(f) => (key(&f.form), f.title_block.as_deref().and_then(key)),
    }
}

/// Runs one command, previewed and committed when it is Bulk.
fn run_cmd(session: &mut Session, name: &str, params: serde_json::Value) {
    let cmd = ew_api::AnyCommand {
        name: name.into(),
        params,
    };
    let bulk = session
        .registry()
        .get(name)
        .is_some_and(|c| c.meta().cost == ew_commands::Cost::Bulk);
    if bulk {
        let p = session.preview(&cmd).unwrap();
        session.commit(p).unwrap();
    } else {
        session.execute(&cmd).unwrap();
    }
}

/// FT-336: a drawn page without a form of its own takes the frame of the frame defaults (doc 03 §5.1) in the page
/// output, as in the cell grid; without frame defaults it keeps the no-grid frame (ID-144).
#[test]
fn a_page_without_a_form_takes_the_frame_defaults_in_the_page_output() {
    let mut session =
        Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Basic, options(3)).unwrap();
    assert_eq!(frame_of_folio_1(&session), (None, None), "no form in the project");
    let forms = shipped_forms();
    let id_of = |key: &str| {
        forms
            .iter()
            .find(|f| f["meta"]["key"] == key)
            .unwrap_or_else(|| panic!("{key}"))["id"]
            .clone()
    };
    let (grid, cover) = (
        id_of("ew.frame.qet-a4-landscape-grid"),
        id_of("ew.frame.qet-a4-landscape-cover"),
    );
    run_cmd(
        &mut session,
        "library.embed",
        serde_json::json!({ "copies": forms }),
    );
    let nogrid = (
        Some("ew.frame.qet-a4-landscape-nogrid".to_owned()),
        Some("ew.titleblock.qet-a4".to_owned()),
    );
    assert_eq!(frame_of_folio_1(&session), nogrid, "the no-grid frame by key");
    run_cmd(
        &mut session,
        "project.frames.set",
        serde_json::json!({ "default": grid }),
    );
    assert_eq!(
        frame_of_folio_1(&session).0.as_deref(),
        Some("ew.frame.qet-a4-landscape-grid"),
        "the default of the frame defaults"
    );
    run_cmd(
        &mut session,
        "project.frames.set",
        serde_json::json!({
            "default": grid,
            "per_page_type": [{ "page_type": "multi_line", "form": cover }],
        }),
    );
    assert_eq!(
        frame_of_folio_1(&session).0.as_deref(),
        Some("ew.frame.qet-a4-landscape-cover"),
        "the entry of the page's type wins over the default"
    );
    session.undo(&ew_api::Origin::Gui).unwrap();
    session.undo(&ew_api::Origin::Gui).unwrap();
    assert_eq!(
        frame_of_folio_1(&session),
        nogrid,
        "undo restores the no-grid frame"
    );
}

/// The ID of the shipped form with `key`.
fn shipped_id(key: &str) -> serde_json::Value {
    shipped_forms()
        .into_iter()
        .find(|f| f["meta"]["key"] == key)
        .unwrap_or_else(|| panic!("{key}"))["id"]
        .clone()
}

/// A basic project with folio 1 (multi-line, no form of its own), section 1 with a divider, the shipped forms, and
/// `frames` as its frame defaults, written as records (the shim can name copies the project does not hold, which no
/// command writes).
fn with_frames(frames: serde_json::Value) -> Session {
    let mut built =
        Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Basic, options(5)).unwrap();
    run_cmd(
        &mut built,
        "section.create",
        serde_json::json!({"number": 1, "folio_range": [100, 199], "title": "A", "divider": true}),
    );
    let mut project = None;
    let mut records = Vec::new();
    for (key, line) in ew_commands::rows::model_lines(&built.world().model).unwrap() {
        let mut v: serde_json::Value = serde_json::from_str(&line).unwrap();
        if key.0 == ew_schema::Kind::Project {
            v.as_object_mut().unwrap().remove("$k");
            v["settings"]["frames"] = frames.clone();
            project = Some(serde_json::from_value::<ew_schema::Project>(v).unwrap());
        } else {
            v["$k"] = serde_json::Value::String(key.0.to_string());
            records.push(ew_schema::Record::from_value(v).unwrap());
        }
    }
    let lib = shipped_forms()
        .into_iter()
        .map(|f| ew_schema::LibDocument::from_value(f).unwrap());
    Session::from_records(project.unwrap(), records, lib, options(5)).unwrap()
}

/// The frame key the page output gives each page of the order (`"-"` without a frame), the drawn pages' cell grids
/// and whether the cell grid fell back to the built-in frame.
fn frames_of(session: &Session) -> (Vec<String>, Vec<Option<ew_schema::FrameGrid>>, bool) {
    let mut q = session.query();
    let order = q.get::<ew_derive::PageOrder>(());
    let mut keys = Vec::new();
    let mut grids = Vec::new();
    for e in &order.entries {
        let frame = (*q.get::<ew_api::output::PageFrameOf>(e.page)).clone();
        keys.push(
            frame
                .and_then(|f| f.form.meta.key.as_ref().map(|k| k.as_str().to_owned()))
                .unwrap_or_else(|| "-".to_owned()),
        );
        if let ew_derive::PageRef::Drawn(p) = e.page {
            grids.push((*q.get::<ew_derive::FrameGridOf>(p)).clone());
        }
    }
    let built_in = q
        .diagnostics()
        .notes()
        .contains(&ew_api::DeriveNote::DefaultFrame);
    (keys, grids, built_in)
}

/// FT-336 review ruling: a frame-defaults rule that names a copy the project does not hold falls through to the next
/// rule, identically in the page output and the cell grid (one resolution, `ew_derive::frame_form_of`); a divider
/// takes the `section_divider` entry.
#[test]
fn a_missing_frame_copy_falls_through_in_the_page_output_and_the_cell_grid_alike() {
    let missing = "0190d1a0-0000-7000-8000-00000000f00d";
    let (grid, cover) = (
        shipped_id("ew.frame.qet-a4-landscape-grid"),
        shipped_id("ew.frame.qet-a4-landscape-cover"),
    );
    let grid_form: ew_schema::FrameGrid = serde_json::from_value(
        shipped_forms().into_iter().find(|f| f["id"] == grid).unwrap()["grid"].clone(),
    )
    .unwrap();
    // The entry of the page's type is missing: the default decides, in both.
    let s = with_frames(serde_json::json!({"default": grid, "per_page_type": [
        {"page_type": "multi_line", "form": missing},
        {"page_type": "section_divider", "form": cover},
    ]}));
    let (keys, grids, built_in) = frames_of(&s);
    assert_eq!(
        keys,
        [
            "ew.frame.qet-a4-landscape-grid",
            "ew.frame.qet-a4-landscape-cover"
        ],
        "folio 1, then the divider of section 1"
    );
    assert_eq!(grids, [Some(grid_form.clone())]);
    assert!(!built_in);
    // The default is missing too: the built-in frame, in both (the no-grid frame of the page output, the built-in
    // QET grid of the cell grid); the divider without its entry takes the no-grid frame.
    let s = with_frames(serde_json::json!({"default": missing, "per_page_type": [
        {"page_type": "multi_line", "form": missing},
        {"page_type": "section_divider", "form": missing},
    ]}));
    let (keys, grids, built_in) = frames_of(&s);
    assert_eq!(
        keys,
        [
            "ew.frame.qet-a4-landscape-nogrid",
            "ew.frame.qet-a4-landscape-nogrid"
        ]
    );
    assert_eq!(grids, [Some(ew_derive::default_frame_grid())]);
    assert!(built_in, "the cell grid says it used the built-in frame");
}
