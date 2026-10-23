// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! End-to-end tests of the headless mode for the actions of WP-S2.01 part 2 (FT-307 to FT-312, FT-313's first part):
//! New project, the sample project, the tutorials, the QElectroTech import, the journal discard, the first-run flow,
//! the company-standard wizard with its preview snapshot compared with the committed golden PNGs, workspaces and F1.
//! No window, no GPU, no display; the sample project and the starter content are named through the environment.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::{Value, json};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
const CONTENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
const EXPECTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416/expected");

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-headless2-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn script_text(steps: &[(&str, Value)]) -> String {
    let mut text = String::from("{\"$schema\":\"ew.uiscript/1\",\"label\":\"part2\"}\n");
    for (action, params) in steps {
        text.push_str(&json!({"action": action, "params": params}).to_string());
        text.push('\n');
    }
    text
}

/// Runs a script with the sample project and the content of the source tree as resources.
fn run(tmp: &Tmp, steps: &[(&str, Value)]) -> Output {
    let file = tmp.path("script.jsonl");
    std::fs::write(&file, script_text(steps)).unwrap();
    Command::new(bin_exe())
        .args(["--headless", "--script"])
        .arg(&file)
        .env("EW_CONFIG_DIR", tmp.path("config"))
        .env("EW_SAMPLE_DIR", GOLDEN_A)
        .env("EW_CONTENT_DIR", CONTENT)
        .env_remove("EW_DOCS_DIR")
        .env_remove("SOURCE_DATE_EPOCH")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn steps_of(out: &Output) -> Vec<Value> {
    let text = String::from_utf8(out.stdout.clone()).unwrap();
    let mut lines: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{l}: {e}")))
        .collect();
    lines.remove(0); // the header
    lines
}

#[track_caller]
fn run_ok(tmp: &Tmp, steps: &[(&str, Value)]) -> Vec<Value> {
    let out = run(tmp, steps);
    let steps_out = steps_of(&out);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    for s in &steps_out {
        assert_eq!(s["outcome"], "done", "{s}");
    }
    steps_out
}

fn describe(step: &Value) -> &str {
    step["describe"].as_str().expect("a describe step")
}

fn read(dir: &Path, file: &str) -> String {
    std::fs::read_to_string(dir.join(file)).unwrap_or_else(|e| panic!("{file}: {e}"))
}

// ---------------------------------------------------------------------------------------------------------------

#[test]
fn file_new_makes_a_project_that_edits_saves_and_reopens() {
    let tmp = Tmp::new();
    let dir = tmp.path("first.ew");
    let steps = run_ok(
        &tmp,
        &[
            ("file.new", json!({ "dir": path(&dir) })),
            (
                "command.run",
                json!({"cmd": "page.create", "params": {"number": 2}}),
            ),
            ("file.save", json!({})),
            ("file.close", json!({})),
            ("file.open", json!({ "dir": path(&dir) })),
        ],
    );
    let state = |i: usize| &steps[i]["state"];
    assert_eq!(state(0)["project"], "first.ew");
    assert_eq!(state(0)["page_count"], 1);
    assert_eq!(state(0)["dirty"], false);
    assert_eq!(state(0)["can_undo"], false);
    assert_eq!(state(1)["page_count"], 2);
    assert_eq!(state(4)["page_count"], 2);
    assert!(dir.join("manifest.json").is_file());
}

#[test]
fn the_sample_opens_read_only_and_the_tutorials_make_a_copy() {
    let tmp = Tmp::new();
    let copy = tmp.path("mine.ew");
    let tutorial = tmp.path("tutorial.ew");
    let steps = run_ok(
        &tmp,
        &[
            ("file.open_sample", json!({})),
            ("ui.describe", json!({})),
            ("file.save_as", json!({ "dir": path(&copy) })),
            ("file.close", json!({})),
            ("help.tutorials", json!({ "dir": path(&tutorial) })),
            ("ui.describe", json!({})),
        ],
    );
    assert_eq!(steps[0]["state"]["project"], "J-E-2416.ew");
    assert_eq!(steps[0]["state"]["access"], "read-only");
    assert!(describe(&steps[1]).contains("banner: This is the sample project."));
    assert_eq!(steps[2]["state"]["access"], "edit");
    assert_eq!(steps[2]["state"]["project"], "mine.ew");
    assert!(copy.join("manifest.json").is_file());
    assert_eq!(steps[4]["state"]["project"], "tutorial.ew");
    assert_eq!(steps[4]["state"]["access"], "edit");
    assert!(describe(&steps[5]).contains("help: tutorials/index.html"));
    // The sample itself was not touched: no lock and no journal in the installation.
    assert!(!Path::new(GOLDEN_A).join(".~ew.lock").exists());
}

#[test]
fn a_qelectrotech_project_is_imported_by_a_script() {
    let tmp = Tmp::new();
    let qet = tmp.path("example.qet");
    std::fs::write(
        &qet,
        r#"<project version="0.100" title="Example"><diagram title="First page" folio="1" cols="17" colsize="60" rows="8" rowsize="80" height="640"/></project>"#,
    )
    .unwrap();
    let dir = tmp.path("imported.ew");
    let steps = run_ok(
        &tmp,
        &[
            (
                "file.import_qet",
                json!({ "file": path(&qet), "dir": path(&dir) }),
            ),
            ("ui.describe", json!({})),
        ],
    );
    assert_eq!(steps[0]["state"]["project"], "imported.ew");
    assert_eq!(steps[0]["state"]["page_count"], 1);
    assert!(
        steps[0]["state"]["banner"]
            .as_str()
            .unwrap()
            .starts_with("Imported example.qet: 1 pages")
    );
    assert!(dir.join("manifest.json").is_file());
    // A file that is no project fails the step with a message that has no path (exit code 1).
    let bad = tmp.path("bad.qet");
    std::fs::write(&bad, "<other/>").unwrap();
    let out = run(
        &tmp,
        &[(
            "file.import_qet",
            json!({ "file": path(&bad), "dir": path(&tmp.path("nope.ew")) }),
        )],
    );
    assert_eq!(out.status.code(), Some(1));
    let failed = &steps_of(&out)[0];
    assert_eq!(failed["outcome"], "failed");
    assert!(
        !failed["message"]
            .as_str()
            .unwrap()
            .contains(&*tmp.0.to_string_lossy())
    );
}

#[test]
fn the_journal_of_an_earlier_session_is_discarded_by_a_script() {
    let tmp = Tmp::new();
    let project = tmp.path("j.ew");
    // A script that ends with unsaved changes exits 1 and keeps the journal (decision log ID-475).
    run_ok(&tmp, &[("file.new", json!({ "dir": path(&project) }))]);
    let out = run(
        &tmp,
        &[
            ("file.open", json!({ "dir": path(&project) })),
            (
                "command.run",
                json!({"cmd": "page.create", "params": {"number": 77}}),
            ),
        ],
    );
    assert_eq!(out.status.code(), Some(1), "dirty at the end");
    // The next open is read-only with the banner; the discard needs `confirm`.
    let out = run(
        &tmp,
        &[
            ("file.open", json!({ "dir": path(&project) })),
            ("file.discard_journal", json!({})),
        ],
    );
    assert_eq!(out.status.code(), Some(1));
    let steps = steps_of(&out);
    assert_eq!(steps[0]["state"]["access"], "read-only");
    assert!(
        steps[0]["state"]["banner"]
            .as_str()
            .unwrap()
            .contains("unsaved changes")
    );
    assert_eq!(steps[1]["outcome"], "failed");
    let steps = run_ok(
        &tmp,
        &[
            ("file.open", json!({ "dir": path(&project) })),
            ("file.discard_journal", json!({"confirm": true})),
            (
                "command.run",
                json!({"cmd": "page.create", "params": {"number": 78}}),
            ),
            ("file.save", json!({})),
        ],
    );
    assert_eq!(steps[1]["state"]["access"], "edit");
    assert_eq!(
        steps[1]["state"]["page_count"], 1,
        "the discarded page 77 is not there"
    );
    assert!(
        steps[1]["state"]["banner"]
            .as_str()
            .unwrap()
            .contains("discarded")
    );
    assert_eq!(steps[3]["state"]["page_count"], 2);
}

#[test]
fn the_first_run_flow_is_scripted_and_leaves_its_answers_in_the_settings() {
    let tmp = Tmp::new();
    let steps = run_ok(
        &tmp,
        &[
            ("help.first_run", json!({})),
            ("ui.describe", json!({})),
            ("first_run.set", json!({"field": "language", "value": "en"})),
            ("first_run.next", json!({})),
            ("first_run.set", json!({"field": "comes_from", "value": "qet"})),
            ("first_run.next", json!({})),
            ("first_run.set", json!({"field": "update_check", "value": false})),
            ("first_run.next", json!({})),
            ("first_run.set", json!({"field": "theme", "value": "light"})),
            ("first_run.set", json!({"field": "ui_scale", "value": 1.25})),
            ("first_run.next", json!({})),
            ("ui.describe", json!({})),
            ("first_run.finish", json!({"start": "sample"})),
        ],
    );
    assert!(describe(&steps[1]).contains("first run \"Welcome to ElektraWatt\": step 1 of 5 \"Language\""));
    assert!(describe(&steps[11]).contains("step 5 of 5 \"Where to start\""));
    assert_eq!(steps[12]["state"]["project"], "J-E-2416.ew");
    let settings = read(&tmp.path("config"), "settings.toml");
    for needle in [
        "first_run_done = true",
        "comes_from = \"qet\"",
        "theme = \"light\"",
        "ui_scale = 1.25",
        "update_check = false",
        "keymap = \"ew\"",
    ] {
        assert!(settings.contains(needle), "{needle}\n{settings}");
    }
}

fn decode_rgba(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("valid png");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("png frame");
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        other => panic!("unexpected PNG colour type {other:?}"),
    };
    (info.width, info.height, rgba)
}

/// The share of pixels (in per cent) where any channel differs by more than `threshold`.
fn differing_percent(a: &[u8], b: &[u8], threshold: u8) -> f64 {
    assert_eq!(a.len(), b.len());
    let bad = a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| x.abs_diff(*y) > threshold))
        .count();
    100.0 * bad as f64 / (a.len() / 4) as f64
}

fn compare_with_golden(produced: &Path, folio: &str) -> f64 {
    let committed = std::fs::read(Path::new(EXPECTED).join(format!("png/{folio}.png"))).unwrap();
    let (w0, h0, a) = decode_rgba(&committed);
    let (w1, h1, b) = decode_rgba(&std::fs::read(produced).unwrap());
    assert_eq!((w0, h0), (w1, h1), "the size of the preview snapshot of {folio}");
    differing_percent(&a, &b, 16)
}

#[test]
fn the_wizards_defaults_reproduce_the_sample_pages_and_a_choice_changes_the_preview() {
    let tmp = Tmp::new();
    let basic = tmp.path("company.ew");
    let cover = tmp.path("cover.png");
    let page = tmp.path("102.png");
    let changed = tmp.path("102-changed.png");
    let steps = run_ok(
        &tmp,
        &[
            ("tools.company_standard", json!({})),
            ("ui.describe", json!({})),
            // The preview's two pages with the default draft: the sample project's folio 1 and 102.
            ("wizard.set", json!({"field": "preview", "value": "cover"})),
            ("view.snapshot", json!({"path": path(&cover), "dpi": 150})),
            ("wizard.set", json!({"field": "preview", "value": "page"})),
            ("view.snapshot", json!({"path": path(&page), "dpi": 150})),
            // A numbering format that the sample does not use changes the device tags on the page.
            ("wizard.goto", json!({"step": 4})),
            (
                "wizard.set",
                json!({"field": "device_format", "value": "{page}-{letter}{counter}"}),
            ),
            ("view.snapshot", json!({"path": path(&changed), "dpi": 150})),
            (
                "wizard.set",
                json!({"field": "device_format", "value": "{page}{letter}{counter}"}),
            ),
            (
                "wizard.set",
                json!({"field": "company_name", "value": "Example Panels"}),
            ),
            ("wizard.goto", json!({"step": 9})),
            ("ui.describe", json!({})),
            ("wizard.finish", json!({ "dir": path(&basic) })),
            // FT-336: the finished wizard stays on the review step until it is closed.
            ("wizard.cancel", json!({})),
            ("ui.describe", json!({})),
        ],
    );
    let first = describe(&steps[1]);
    assert!(
        first.contains("wizard \"Company standard\": step 1 of 10 \"Company\" (#1)"),
        "{first}"
    );
    assert!(first.contains("preview \"Folio 102\": ready"), "{first}");
    // NFR-UX-08 in miniature: the default draft's preview is the sample's folio 102 and its cover.
    assert!(
        compare_with_golden(&page, "102") <= 0.5,
        "folio 102 differs from the golden"
    );
    assert!(
        compare_with_golden(&cover, "1") <= 0.5,
        "the cover differs from the golden"
    );
    assert!(
        compare_with_golden(&changed, "102") > 0.0,
        "the numbering format reaches the preview page"
    );
    let review = describe(&steps[12]);
    assert!(
        review.contains("review \"Company name\": \"Example Panels\" written"),
        "{review}"
    );
    // FT-336: the check scheme stays in the draft (its command is `HumanOnly`, ADR-0008 amendment 3).
    assert!(
        review.contains("review \"Check scheme\": \"pre-delivery\" kept"),
        "{review}"
    );
    assert!(review.contains("\" written"), "{review}");
    assert!(
        steps[13]["state"]["banner"]
            .as_str()
            .unwrap()
            .contains("company.ew")
    );
    let after = describe(&steps[15]);
    assert!(after.contains("label \"Template: company.ew\""), "{after}");
    assert!(after.contains("button \"Edit company standard\""), "{after}");
    // The basic project is on disk, of kind basic, with the draft's settings.
    let project = read(&basic, "project.jsonl");
    assert!(project.contains("\"kind\":\"basic\""));
    assert!(project.contains("company.dbu-wh-24v"));
    assert!(
        !basic.join(".~ew.lock").exists(),
        "the wizard closed what it wrote"
    );
    // The draft is gone and the settings name the new default.
    assert!(!tmp.path("config").join("wizard-draft.json").exists());
    assert!(read(&tmp.path("config"), "settings.toml").contains("company.ew"));
}

/// FT-336 ruling: the check scheme's `HumanOnly` command is never applied from a UI script: the headless mode refuses
/// `wizard.apply_check_scheme` (confirmed or not) with the human-only reason, which the transcript records, and the
/// run ends with exit code 1.
#[test]
fn a_ui_script_cannot_apply_the_check_scheme() {
    let tmp = Tmp::new();
    let basic = tmp.path("company.ew");
    let out = run(
        &tmp,
        &[
            ("tools.company_standard", json!({})),
            ("wizard.finish", json!({ "dir": path(&basic) })),
            ("wizard.apply_check_scheme", json!({ "confirm": true })),
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let steps = steps_of(&out);
    assert_eq!(steps.len(), 3);
    assert_eq!(steps[1]["outcome"], "done");
    let refused = &steps[2];
    assert_eq!(refused["action"], "wizard.apply_check_scheme");
    assert_eq!(refused["outcome"], "refused", "{refused}");
    assert_eq!(refused["reason"], "reason-human-only", "{refused}");
    assert!(!read(&basic, "project.jsonl").contains("\"checks\""));
}

#[test]
fn new_projects_start_from_the_wizards_basic_project() {
    let tmp = Tmp::new();
    let basic = tmp.path("company.ew");
    let job = tmp.path("job.ew");
    run_ok(
        &tmp,
        &[
            ("tools.company_standard", json!({})),
            ("wizard.set", json!({"field": "csa_mains", "value": "2.5"})),
            ("wizard.set", json!({"field": "first_revision", "value": "V0.1"})),
            ("wizard.finish", json!({ "dir": path(&basic) })),
        ],
    );
    // A second run: the default template comes from the settings written by the first.
    let steps = run_ok(
        &tmp,
        &[
            ("file.new", json!({ "dir": path(&job) })),
            ("file.close", json!({})),
        ],
    );
    assert_eq!(steps[0]["state"]["project"], "job.ew");
    assert!(
        read(&job, "project.jsonl").contains("company.0.75-2.5"),
        "the size profile of the company standard"
    );
    assert!(read(&job, "revisions.jsonl").contains("V0.1"));
    assert!(
        !read(&job, "project.jsonl").contains("\"kind\""),
        "a schematic project"
    );
    // `template: builtin` is the shipped one.
    let plain = tmp.path("plain.ew");
    run_ok(
        &tmp,
        &[
            ("file.new", json!({ "dir": path(&plain), "template": "builtin" })),
            ("file.close", json!({})),
        ],
    );
    assert!(!read(&plain, "project.jsonl").contains("company."));
}

#[test]
fn workspaces_are_saved_listed_and_loaded_by_a_script() {
    let tmp = Tmp::new();
    let steps = run_ok(
        &tmp,
        &[
            ("file.new", json!({ "dir": path(&tmp.path("p.ew")) })),
            ("ui.describe", json!({})),
            ("view.workspace_save", json!({"name": "Review"})),
            ("view.workspace_load", json!({"name": "Schematic"})),
            ("view.workspace_load", json!({"name": "Review"})),
            ("view.workspace_reset", json!({})),
            ("ui.describe", json!({})),
        ],
    );
    let first = describe(&steps[1]);
    assert!(first.contains("workspace \"Schematic\""), "{first}");
    assert!(
        first.contains("layout: split horizontal [\"Pages\" 16%, \"Page view\" 64%, \"Properties\" 20%]"),
        "{first}"
    );
    assert!(
        first.contains("item view.workspace_load \"Schematic\" (checked) enabled"),
        "{first}"
    );
    let last = describe(&steps[6]);
    assert!(last.contains("workspace \"Review\""), "{last}");
    assert!(
        last.contains("item view.workspace_load \"Review\" (checked) enabled"),
        "{last}"
    );
    assert!(
        last.contains("item view.workspace_load \"Schematic\" enabled"),
        "{last}"
    );
    let file = read(&tmp.path("config/workspaces"), "Review.json");
    assert!(
        file.starts_with("{\"$schema\":\"ew.workspace/1\",\"name\":\"Review\""),
        "{file}"
    );
    // A bad name and a missing workspace fail the step.
    let out = run(&tmp, &[("view.workspace_load", json!({"name": "Nope"}))]);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn f1_and_the_quick_switches_show_in_the_describe_text() {
    let tmp = Tmp::new();
    let steps = run_ok(
        &tmp,
        &[
            ("help.documentation", json!({})),
            ("ui.describe", json!({})),
            ("help.about", json!({})),
            ("help.documentation", json!({})),
            ("ui.describe", json!({})),
            ("ui.dismiss", json!({})),
            ("settings.set", json!({"key": "keymap", "value": "kicad"})),
            ("ui.describe", json!({})),
        ],
    );
    assert!(describe(&steps[1]).contains("help: how-to/application-window.html#start-page"));
    let about = describe(&steps[4]);
    assert!(
        about.contains("help: how-to/application-window.html#about"),
        "{about}"
    );
    assert!(about.contains("Support: pre-release build"), "{about}");
    assert!(
        about.contains("ElektraWatt's checks do not replace engineering verification"),
        "{about}"
    );
    assert!(describe(&steps[7]).contains("quick \"Keymap\": \"KiCad-like\""));
}

#[test]
fn the_part_two_transcripts_are_deterministic() {
    let script = |tmp: &Tmp| {
        let basic = tmp.path("company.ew");
        let steps = [
            ("help.first_run", json!({})),
            ("first_run.set", json!({"field": "comes_from", "value": "eplan"})),
            ("first_run.finish", json!({"start": "wizard"})),
            ("wizard.set", json!({"field": "csa_control", "value": "1"})),
            ("ui.describe", json!({})),
            ("wizard.finish", json!({ "dir": path(&basic) })),
            ("file.new", json!({ "dir": path(&tmp.path("a.ew")) })),
            ("view.workspace_save", json!({"name": "Mine"})),
            ("help.documentation", json!({})),
            ("ui.describe", json!({})),
            ("file.close", json!({})),
        ];
        run(tmp, &steps).stdout
    };
    let (a, b) = (Tmp::new(), Tmp::new());
    let (first, second) = (script(&a), script(&b));
    // The two runs name different temporary directories; the transcripts carry no path, so they are equal.
    assert_eq!(String::from_utf8_lossy(&first), String::from_utf8_lossy(&second));
    assert!(!String::from_utf8_lossy(&first).contains(&*a.0.to_string_lossy()));
}

/// The path of the `elektrawatt` binary under test. nextest sets `NEXTEST_BIN_EXE_elektrawatt` at run time (the binaries of a
/// `--archive-file` run are unpacked elsewhere than the compile-time path, FT-353); a plain `cargo test` falls back
/// to the compile-time path.
// Allowed: the program boundary of a test — nextest's binary path is read once, never persisted or printed.
#[allow(clippy::disallowed_methods)]
fn bin_exe() -> std::path::PathBuf {
    std::env::var_os("NEXTEST_BIN_EXE_elektrawatt")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_BIN_EXE_elektrawatt"))) // bin-exe-fallback
}

/// FT-384, FT-406: the company logo is visible on the cover. The wizard's preview of the cover (the sample's folio 1
/// with the draft applied, rendered through the page view's CPU path) shows an SVG logo as dark vector paths and a PNG
/// and a JPEG logo as their pixels in the title block, where the cover without a logo is light; the project the wizard
/// writes and a new project made from it print the logo on folio 1 too. Placeholder logos only (a black rectangle, a
/// red PNG, a red-and-blue JPEG); the PDF of a PNG and a JPEG logo on the cover is `ew-api`'s `tests/images.rs`.
#[test]
fn the_company_logo_is_visible_on_the_cover() {
    let tmp = Tmp::new();
    let svg = tmp.path("logo.svg");
    std::fs::write(
        &svg,
        br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 10"><rect width="40" height="10"/></svg>"#,
    )
    .unwrap();
    let png = tmp.path("logo.png");
    {
        let mut bytes = Vec::new();
        let mut enc = png::Encoder::new(&mut bytes, 8, 2);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&[255, 0, 0, 255].repeat(16)).unwrap();
        w.finish().unwrap();
        std::fs::write(&png, bytes).unwrap();
    }
    // A 16 × 8 baseline JPEG (4:4:4, all quantisers 1, DC-only blocks, written by hand): left half red, right half blue.
    let jpeg = tmp.path("logo.jpg");
    {
        let mut b = vec![0xff, 0xd8, 0xff, 0xdb, 0x00, 0x84, 0x00];
        b.extend([1u8; 64]);
        b.push(0x01);
        b.extend([1u8; 64]);
        b.extend_from_slice(&[
            0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0x08, 0x00, 0x10, 0x03, 0x01, 0x11, 0x00, 0x02, 0x11, 0x01,
            0x03, 0x11, 0x01, 0xff, 0xc4, 0x00, 0x1f, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
            0x08, 0x09, 0x0a, 0x0b, 0xff, 0xc4, 0x00, 0x14, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xda, 0x00, 0x0c, 0x03, 0x01,
            0x00, 0x02, 0x00, 0x03, 0x00, 0x00, 0x3f, 0x00, 0x92, 0xfa, 0x54, 0xea, 0xfe, 0x12, 0x87, 0x5d,
            0x50, 0x5b, 0x5f, 0x7f, 0xff, 0xd9,
        ]);
        std::fs::write(&jpeg, b).unwrap();
    }
    let basic = tmp.path("logo-company.ew");
    let job = tmp.path("job.ew");
    let (with_svg, with_png, with_jpeg, job_cover) = (
        tmp.path("cover-svg.png"),
        tmp.path("cover-png.png"),
        tmp.path("cover-jpeg.png"),
        tmp.path("job-1.png"),
    );
    run_ok(
        &tmp,
        &[
            ("tools.company_standard", json!({})),
            (
                "wizard.set",
                json!({"field": "company_logo", "value": path(&svg)}),
            ),
            ("wizard.set", json!({"field": "preview", "value": "cover"})),
            ("view.snapshot", json!({"path": path(&with_svg), "dpi": 150})),
            (
                "wizard.set",
                json!({"field": "company_logo", "value": path(&png)}),
            ),
            ("view.snapshot", json!({"path": path(&with_png), "dpi": 150})),
            (
                "wizard.set",
                json!({"field": "company_logo", "value": path(&jpeg)}),
            ),
            ("view.snapshot", json!({"path": path(&with_jpeg), "dpi": 150})),
            (
                "wizard.set",
                json!({"field": "company_logo", "value": path(&svg)}),
            ),
            ("wizard.goto", json!({"step": 9})),
            ("wizard.finish", json!({ "dir": path(&basic) })),
            ("wizard.cancel", json!({})),
            ("file.new", json!({ "dir": path(&job) })),
            ("page.first", json!({})),
            ("view.snapshot", json!({"path": path(&job_cover), "dpi": 150})),
        ],
    );
    let committed = std::fs::read(Path::new(EXPECTED).join("png/1.png")).unwrap();
    let (_, _, plain) = decode_rgba(&committed);
    let newly = |file: &Path, hit: &dyn Fn(&[u8]) -> bool| -> usize {
        let (_, _, img) = decode_rgba(&std::fs::read(file).unwrap());
        assert_eq!(img.len(), plain.len(), "{}: the cover's size", file.display());
        img.as_chunks::<4>()
            .0
            .iter()
            .zip(plain.as_chunks::<4>().0)
            .filter(|(p, q)| hit(&p[..]) && q[..3].iter().all(|c| *c > 200))
            .count()
    };
    let dark = |p: &[u8]| p[..3].iter().all(|c| *c < 80);
    let red = |p: &[u8]| p[0] > 200 && p[1] < 60 && p[2] < 60;
    let svg_pixels = newly(&with_svg, &dark);
    assert!(
        svg_pixels > 300,
        "the SVG logo on the preview's cover: {svg_pixels} dark pixels"
    );
    let png_pixels = newly(&with_png, &red);
    assert!(
        png_pixels > 300,
        "the PNG logo on the preview's cover: {png_pixels} red pixels"
    );
    let blue = |p: &[u8]| p[2] > 200 && p[0] < 60 && p[1] < 60;
    let (jpeg_red, jpeg_blue) = (newly(&with_jpeg, &red), newly(&with_jpeg, &blue));
    assert!(
        jpeg_red > 100 && jpeg_blue > 100,
        "the JPEG logo on the preview's cover: {jpeg_red} red and {jpeg_blue} blue pixels"
    );
    // The new project's folio 1 differs from the sample's cover elsewhere, so its logo is looked for where the
    // preview drew it: the pixels the SVG logo darkened are dark there too.
    let (width, _, svg_img) = decode_rgba(&std::fs::read(&with_svg).unwrap());
    let (_, _, job_img) = decode_rgba(&std::fs::read(&job_cover).unwrap());
    let logo: Vec<usize> = (0..plain.len() / 4)
        .filter(|&k| dark(&svg_img[4 * k..4 * k + 4]) && plain[4 * k..4 * k + 3].iter().all(|c| *c > 200))
        .collect();
    assert!(width > 0);
    let job_pixels = logo.iter().filter(|&&k| dark(&job_img[4 * k..4 * k + 4])).count();
    assert!(
        job_pixels * 10 >= logo.len() * 9,
        "the logo on folio 1 of a new project from the basic project: {job_pixels} of {} pixels",
        logo.len()
    );
}
