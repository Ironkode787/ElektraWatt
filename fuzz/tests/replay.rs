// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Replays the committed inputs through the targets' `check` functions without the fuzzer: every seed of
//! `corpus/<target>/` must pass (a seed that fails is a defect, or a property that is wrong), every input of
//! `regressions/<target>/` is a reported finding and is expected to fail until its defect is fixed (the test
//! lists them and asserts only that they do not abort the process), and the seeds must get as far as the
//! properties they were made for (the target is not vacuous). Run from `fuzz/`: `cargo test --release`.

use std::path::{Path, PathBuf};

use ew_fuzz::loader;
use ew_fuzz::open_path::{self, Outcome};

fn inputs(dir: &str) -> Vec<(String, Vec<u8>)> {
    let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries {
            let path = entry.expect("a directory entry").path();
            if path.is_file() && path.file_name().is_some_and(|n| n != ".gitkeep") {
                let name = path
                    .file_name()
                    .expect("a file name")
                    .to_string_lossy()
                    .into_owned();
                out.push((name, std::fs::read(&path).expect("a readable seed")));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_loader_seeds_pass_and_reach_the_round_trip() {
    let seeds = inputs("corpus/jsonl_loader");
    assert!(seeds.len() >= 8, "the loader's seed corpus is missing");
    // WP-S2.04 part 4a: the page items and the structure box read typed (an unknown variant verbatim) and
    // round-trip.
    let items = seeds
        .iter()
        .find(|(n, _)| n == "page-items")
        .map(|(_, b)| loader::check(b))
        .expect("the page-items seed");
    assert!(items.round_tripped && items.valid, "{items:?}");
    let mut round_tripped = 0;
    let mut valid = 0;
    for (name, bytes) in &seeds {
        let outcome = loader::check(bytes);
        eprintln!("{name}: {outcome:?}");
        round_tripped += usize::from(outcome.round_tripped);
        valid += usize::from(outcome.valid);
    }
    assert!(
        round_tripped >= 4,
        "only {round_tripped} loader seeds reached the round trip"
    );
    assert!(
        valid >= 2,
        "only {valid} loader seeds are valid inputs (the strong round-trip check)"
    );
}

#[test]
fn the_migration_chain_seeds_pass_and_rewrite_old_shapes() {
    let seeds = inputs("corpus/migration_chain");
    assert!(seeds.len() >= 15, "the migration chain's seed corpus is missing");
    let outcomes: Vec<(String, Outcome)> = seeds
        .iter()
        .map(|(name, bytes)| (name.clone(), open_path::check(bytes)))
        .collect();
    for (name, outcome) in &outcomes {
        eprintln!("{name}: {outcome:?}");
    }
    let get = |name: &str| {
        outcomes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, o)| *o)
            .expect("a seed")
    };
    assert_eq!(get("golden-as-it-is"), Outcome::Reopened { rewritten: 0 });
    assert_eq!(get("golden-read-only"), Outcome::ReadOnly);
    // The older build's revisions open, and the save rewrites their file (the typed revisions, FT-165).
    assert!(
        matches!(get("old-shape-revisions"), Outcome::Reopened { rewritten } if rewritten >= 1),
        "{:?}",
        get("old-shape-revisions")
    );
    // The findings of cycle 22. A BOM before a library copy is a shape: the project opens with the golden model,
    // and the save rewrites the copy (and the revision whose member order the input changed as well).
    let bom = open_path::decode(&inputs_named(&seeds, "lib-document-bom"));
    assert!(bom.ops.iter().all(|op| open_path::shape_only(*op)), "{bom:?}");
    assert!(
        matches!(get("lib-document-bom"), Outcome::Reopened { rewritten } if rewritten >= 1),
        "{:?}",
        get("lib-document-bom")
    );
    assert_eq!(get("lib-document-bom-read-only"), Outcome::ReadOnly);
    // A swap of two lines inside a library document is no shape (it moved `sha256` into `meta`): the oracle no
    // longer asks for the golden model, and the project still opens, read-only as the mode byte asks.
    let swap = open_path::decode(&inputs_named(&seeds, "lib-document-line-swap"));
    assert!(!swap.ops.iter().all(|op| open_path::shape_only(*op)), "{swap:?}");
    assert_eq!(get("lib-document-line-swap"), Outcome::ReadOnly);
}

fn inputs_named(seeds: &[(String, Vec<u8>)], name: &str) -> Vec<u8> {
    seeds
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, bytes)| bytes.clone())
        .unwrap_or_else(|| panic!("no seed {name}"))
}

#[test]
fn the_command_seeds_pass_and_commit_commands() {
    let seeds = inputs("corpus/command_sequence");
    assert!(seeds.len() >= 4, "the command target's seed corpus is missing");
    for (name, bytes) in &seeds {
        let committed = ew_fuzz::command::check(bytes);
        eprintln!("{name}: {committed} commands committed");
        assert!(
            committed >= 1,
            "{name}: no command committed, the generator is degenerate"
        );
    }
}

#[test]
fn the_png_pdf_seeds_decode_and_print() {
    let seeds = inputs("corpus/png_pdf");
    assert!(seeds.len() >= 4, "the PNG-to-PDF target's seed corpus is missing");
    for (name, bytes) in &seeds {
        let decoded = ew_fuzz::png_pdf::check(bytes).decoded;
        eprintln!("{name}: decoded = {decoded}");
        assert!(decoded, "{name}: the seed did not decode as a PNG");
    }
}

/// The `.ewz` reader's seeds (`ew export ewz` of `golden/fixtures/release-min`, with and without the two git files)
/// are accepted and pack again.
#[test]
fn the_ewz_read_seeds_are_accepted() {
    let seeds = inputs("corpus/ewz_read");
    assert!(seeds.len() >= 2, "the .ewz reader's seed corpus is missing");
    for (name, bytes) in &seeds {
        assert_eq!(
            ew_fuzz::ewz_read::check(bytes),
            ew_fuzz::ewz_read::Outcome { accepted: true },
            "{name}"
        );
    }
    assert_eq!(
        ew_fuzz::ewz_read::check(b"PK"),
        ew_fuzz::ewz_read::Outcome { accepted: false }
    );
}

/// The merge seeds (hand-made from W20 of Golden A's page 102 and its manifest, AC-41's data) reach the reload
/// property, where they merge cleanly and where they conflict (decision log ID-1273); the input decodes as documented.
/// A record at the depth limit that both sides change at its leaf reloads too: its conflict record would nest one level
/// deeper, so ours is kept.
#[test]
fn the_merge_file_seeds_reach_the_reload_and_the_conflict() {
    use ew_fuzz::merge_file::{Outcome, check, decode, encode};
    let seeds = inputs("corpus/merge_file");
    for (_, bytes) in &seeds {
        check(bytes);
    }
    let clean = Outcome {
        conflicts: 0,
        reloaded: true,
    };
    let conflict = Outcome {
        conflicts: 1,
        reloaded: true,
    };
    // 64 levels, the loader's limit: the record and 63 arrays.
    let deep = |n: u8| {
        format!(
            "{{\"$k\":\"bond\",\"id\":\"0190d8a4-6d3e-7c41-8f00-000000000001\",\"x\":{}{n}{}}}\n",
            "[".repeat(63),
            "]".repeat(63)
        )
    };
    let input = encode(0, deep(1).as_bytes(), deep(2).as_bytes(), deep(3).as_bytes());
    assert_eq!(check(&input), conflict);
    for (name, outcome) in [
        ("manifest", clean),
        ("w20-clean", clean),
        ("w20-conflict", conflict),
    ] {
        assert_eq!(check(&inputs_named(&seeds, name)), outcome, "{name}");
    }
    let input = encode(10, b"o", b"aa", b"bbb");
    let case = decode(&input);
    assert_eq!(
        (case.path, case.base, case.ours, case.theirs),
        ("manifest.json", &b"o"[..], &b"aa"[..], &b"bbb"[..])
    );
    assert_eq!(decode(&[1, 9]).path, "project.jsonl");
}

/// WP-S6.04 D9, SE-6: the hostile SQL of the corpus is `sql_sandbox`'s seed corpus, byte for byte; every hostile file
/// is refused or stopped against the golden project's projection too, and the queries of the views return rows.
#[test]
fn the_sql_sandbox_seeds_are_the_hostile_corpus_and_the_queries_run() {
    let hostile = inputs("../crates/ew-testkit/hostile/sql");
    let seeds = inputs("corpus/sql_sandbox");
    assert!(hostile.len() >= 10, "the hostile SQL corpus is missing");
    for (name, bytes) in &hostile {
        assert_eq!(
            seeds.iter().find(|(n, _)| n == name).map(|(_, b)| b),
            Some(bytes),
            "{name}: the seed is the hostile file"
        );
        let outcome = ew_fuzz::sql_sandbox::check(bytes);
        assert_eq!(outcome.rows, None, "{name}: the sandbox ran it");
    }
    for (name, bytes) in seeds.iter().filter(|(n, _)| n.starts_with("query-")) {
        let rows = ew_fuzz::sql_sandbox::check(bytes).rows;
        eprintln!("{name}: {rows:?} rows");
        assert!(rows.is_some_and(|n| n >= 1), "{name}: {rows:?}");
    }
}

/// WP-S6.04 D9: the SVG seeds reach the drawing path where the sniff accepts them, and the script is refused.
#[test]
fn the_svg_asset_seeds_draw_or_are_refused() {
    use ew_fuzz::svg_asset::{Outcome, check};
    let seeds = inputs("corpus/svg_asset");
    let drawn = Outcome {
        svg: true,
        drawn: true,
    };
    for (name, outcome) in [
        ("arc-gradient.svg", drawn),
        ("rect.svg", drawn),
        (
            "script.svg",
            Outcome {
                svg: false,
                drawn: false,
            },
        ),
    ] {
        assert_eq!(check(&inputs_named(&seeds, name)), outcome, "{name}");
    }
}

/// WP-S6.04 D9: every raster seed decodes, the PNGs behind the PNG selector and the JPEG behind the JPEG one.
#[test]
fn the_image_decode_seeds_decode() {
    let seeds = inputs("corpus/image_decode");
    assert!(seeds.len() >= 5, "the image decoder's seed corpus is missing");
    for (name, bytes) in &seeds {
        assert!(ew_fuzz::image_decode::check(bytes).decoded, "{name}");
    }
}

/// WP-S6.04 D9: textconv prints every seed; with the session it names the golden project's records.
#[test]
fn the_textconv_seeds_print_and_name_records() {
    let seeds = inputs("corpus/textconv");
    for (name, bytes) in &seeds {
        let outcome = ew_fuzz::textconv::check(bytes);
        eprintln!("{name}: {outcome:?}");
        assert!(outcome.lines >= 1, "{name}");
    }
    assert!(ew_fuzz::textconv::check(&inputs_named(&seeds, "structure-with-session")).named >= 1);
}

/// WP-S6.04 D9: the exported tables read back against the golden project with their key, given or from the workbook.
#[test]
fn the_table_round_trip_seeds_read_back() {
    let seeds = inputs("corpus/table_round_trip");
    assert_eq!(ew_fuzz::table_round_trip::key(0).as_deref(), Some("nav:pages"));
    assert_eq!(ew_fuzz::table_round_trip::key(2).as_deref(), Some("nav:devices"));
    assert_eq!(ew_fuzz::table_round_trip::key(1), None);
    for (name, bytes) in &seeds {
        let outcome = ew_fuzz::table_round_trip::check(bytes);
        eprintln!("{name}: {outcome:?}");
        assert!(outcome.read, "{name}");
    }
}

/// WP-S6.04 D9: Golden B's variants replay on the golden project, and a log that names `revision.approve` is refused
/// before its first line.
#[test]
fn the_command_log_seeds_commit_or_are_refused() {
    use ew_fuzz::command_log::{Outcome, check};
    let seeds = inputs("corpus/command_log");
    for name in ["b1-page-based-tags.ewcmd", "b2-renumber-pages.ewcmd"] {
        let outcome = check(&inputs_named(&seeds, name));
        eprintln!("{name}: {outcome:?}");
        assert!(outcome.parsed && outcome.committed >= 1, "{name}: {outcome:?}");
    }
    assert_eq!(
        check(&inputs_named(&seeds, "human-only.ewcmd")),
        Outcome {
            parsed: true,
            committed: 0
        }
    );
}

/// The fuzz finding of CI run 40897313649 (`regressions/png_pdf/plte-length-not-a-multiple-of-3`, 93 bytes): a 2 × 2
/// palette PNG whose `PLTE` is 14 bytes long, which made `png` 0.18.1 panic in its palette code (`range end index 3
/// out of range for slice of length 2`). Fixed by the chunk check of `ew_render_cpu`: the target's entry point refuses
/// it cleanly (the decoder does not accept it, so nothing is printed), with no panic.
#[test]
fn the_png_pdf_plte_finding_is_refused_without_a_panic() {
    let bytes = inputs_named(&inputs("regressions/png_pdf"), "plte-length-not-a-multiple-of-3");
    assert_eq!(bytes.len(), 93);
    assert_eq!(
        ew_fuzz::png_pdf::check(&bytes),
        ew_fuzz::png_pdf::Outcome { decoded: false }
    );
}

/// The fuzz finding of CI run 42079778094 (`regressions/migration_chain/missing-symbol-copy`, 5 bytes): the golden
/// project with the library copy of one symbol deleted (its manifest listing kept), opened for editing. Since FT-1383
/// (ID-1395) the loader names the missing copy and the project opens read-only, its model failing EW-INV-001 for the
/// representations that name the symbol; the oracle asked every opened model to pass its integrity check. It now
/// accepts that state only read-only, with the failing record naming a copy a loader finding names, and a save refused.
#[test]
fn the_migration_chain_missing_symbol_copy_opens_read_only() {
    let bytes = inputs_named(&inputs("regressions/migration_chain"), "missing-symbol-copy");
    assert_eq!(bytes, [0x00, 0x93, 0x01, 0x0d, 0x21]);
    let paths = ew_fuzz::golden::paths();
    assert_eq!(
        paths[0x93 % paths.len()],
        "lib/symbols/4f6bee27-45fb-5898-a9f7-48a91ddce41c.json"
    );
    assert_eq!(open_path::check(&bytes), Outcome::ReadOnly);
}

/// C53-p3#1 (ID-1481): the open-path oracle judges every violation (FT-1540) and accepts, beside an EW-INV-001 naming
/// a dropped copy, a violation whose records include one that refers to that copy: EW-INV-024 on a fluid line ending at
/// a function whose definition copy was dropped. A violation unrelated to a dropped copy still fails, and nothing is
/// accepted when no loader finding names the copy.
#[test]
fn the_open_path_oracle_accepts_a_violation_of_a_dropped_copys_referrer() {
    use ew_format::ew_schema::{AnyId, Kind};
    use ew_model::ModelError;
    use std::collections::BTreeSet;
    let id = |s: &str| s.parse::<AnyId>().expect("a UUID");
    let (fndef, function) = (
        id("7b85e9bb-116f-5c3c-a7b6-d5260aa1c219"),
        id("f47a4bc8-4c73-5bba-b96c-ed7542db1f1a"),
    );
    let (line, other_line, other_function) = (
        id("bc567040-5fda-53cd-8063-952499b3c9f3"),
        id("00000000-0000-4000-8000-000000000001"),
        id("00000000-0000-4000-8000-000000000002"),
    );
    let invariant = |rule: &'static str, keys: Vec<(Kind, AnyId)>| ModelError::Invariant {
        id: rule,
        message: "m".into(),
        keys,
    };
    let dangling = invariant(
        ew_model::REFS_RESOLVE,
        vec![(Kind::Function, function), (Kind::Fndef, fndef)],
    );
    let follows = invariant(
        ew_model::FLUID_LINES,
        vec![(Kind::Connection, line), (Kind::Function, function)],
    );
    let unrelated = invariant(
        ew_model::FLUID_LINES,
        vec![(Kind::Connection, other_line), (Kind::Function, other_function)],
    );
    let named = BTreeSet::from([fndef]);
    let judged = [
        open_path::dropped_copy_violations(&[dangling.clone()], &named),
        open_path::dropped_copy_violations(&[dangling.clone(), follows.clone()], &named),
        open_path::dropped_copy_violations(&[dangling.clone(), follows.clone(), unrelated], &named),
        open_path::dropped_copy_violations(&[dangling, follows], &BTreeSet::new()),
    ];
    assert_eq!(judged, [true, true, false, false]);
}

/// C53-p3#1 (ID-1481, `regressions/migration_chain/missing-fluid-fndef-copy`, 5 bytes): the golden project with the
/// definition copy of the pneumatic cylinders deleted (its manifest listing kept), opened for editing. Folio 203 is a
/// fluid page, so its fluid lines at those cylinders fail EW-INV-024 beside the EW-INV-001 that names the copy. The
/// load tolerates both (`Model::from_records_missing`, ID-1484), so the project opens read-only as FT-1383 wants, and
/// the oracle above accepts the model; before ID-1484 the open was refused (`OpenError::Model`).
#[test]
fn the_migration_chain_missing_fluid_definition_copy_does_not_abort() {
    let bytes = inputs_named(&inputs("regressions/migration_chain"), "missing-fluid-fndef-copy");
    assert_eq!(bytes, [0x00, 0x5b, 0x01, 0x0d, 0x21]);
    let paths = ew_fuzz::golden::paths();
    assert_eq!(
        paths[0x5b % paths.len()],
        "lib/fndefs/7b85e9bb-116f-5c3c-a7b6-d5260aa1c219.json"
    );
    assert_eq!(open_path::check(&bytes), Outcome::ReadOnly);
}

/// The importer seeds (WP-S3.09 D15, IC-17) pass and reach their properties: every QET project imports and commits;
/// the element fixtures that are definitions import; the golden tables import and the smallest set opens as a
/// project; both supplier tables import parts.
#[test]
fn the_importer_seeds_pass_and_import() {
    use ew_fuzz::importers::{golden_tables, parts_table, qet_element, qet_project};
    let projects = inputs("corpus/qet_project");
    assert!(projects.len() >= 2, "the QET project seeds are missing");
    for (name, bytes) in &projects {
        let o = qet_project(bytes);
        eprintln!("qet_project/{name}: {o:?}");
        assert!(o.imported && o.committed && o.emitted > 0, "{name}: {o:?}");
    }
    let elements = inputs("corpus/qet_element");
    assert!(elements.len() >= 6, "the element seeds are missing");
    let imported = elements
        .iter()
        .filter(|(name, bytes)| {
            let o = qet_element(bytes);
            eprintln!("qet_element/{name}: {o:?}");
            o.imported && o.emitted > 0
        })
        .count();
    assert!(imported >= 4, "only {imported} element seeds import");
    let tables = inputs("corpus/golden_tables");
    assert!(tables.len() >= 3, "the golden table seeds are missing");
    for (name, bytes) in &tables {
        let o = golden_tables(bytes);
        eprintln!("golden_tables/{name}: {o:?}");
        assert!(o.imported && o.emitted > 0, "{name}: {o:?}");
    }
    assert!(golden_tables(&inputs_named(&tables, "project-pages")).committed);
    for (name, bytes) in inputs("corpus/parts_table") {
        let o = parts_table(&bytes);
        eprintln!("parts_table/{name}: {o:?}");
        assert!(o.imported && o.emitted >= 2, "{name}: {o:?}");
    }
}

/// WP-S6.04 (ID-1354): the `.ewlib` reader's seeds pass; under the computed manifest the sample shards are listed and
/// hand their documents out, and another session's lock is never taken.
#[test]
fn the_ewlib_read_seeds_pass_and_read_documents() {
    use ew_fuzz::ewlib_read::check;
    let seeds = inputs("corpus/ewlib_read");
    assert!(seeds.len() >= 4, "the .ewlib reader's seeds are missing");
    for (name, bytes) in &seeds {
        eprintln!("ewlib_read/{name}: {:?}", check(bytes));
    }
    for name in ["sample-computed", "sample-computed-locked"] {
        let o = check(&inputs_named(&seeds, name));
        assert!(o.manifest && o.listed && o.documents >= 3, "{name}: {o:?}");
    }
    assert!(check(&inputs_named(&seeds, "sample-manifest")).manifest);
}

/// FT-1452: the `.ewui` target's seeds pass, and so do the tutorial scripts of the user guide, read where they are
/// (`docs/book/tutorial-scripts/`), each parsed whole.
#[test]
fn the_ewui_script_seeds_and_the_tutorial_scripts_pass() {
    use ew_fuzz::ewui_script::check;
    let seeds = inputs("corpus/ewui_script");
    assert!(!seeds.is_empty(), "the .ewui target's seeds are missing");
    for (name, bytes) in &seeds {
        let o = check(bytes);
        assert!(o.parsed && o.steps > 0, "ewui_script/{name}: {o:?}");
    }
    let tutorials = inputs("../docs/book/tutorial-scripts");
    assert!(tutorials.len() >= 4, "the tutorial scripts are missing");
    for (name, bytes) in &tutorials {
        let o = check(bytes);
        assert!(o.parsed && o.steps > 0, "{name}: {o:?}");
    }
}

/// The reported findings: each input of `regressions/<target>/` is replayed in this ordinary test process, where a
/// panic is caught and the generator's own failure text (the seed, the step, the oracle and the command log of
/// `command_sequence`) is printed, which libFuzzer's aborting panic hook never lets the fuzz target do. Nothing is
/// asserted about the outcome: a finding fails by definition, until its defect is fixed and the input becomes a plain
/// test of the crate that was fixed. Run with `--nocapture`.
#[test]
fn the_regressions_are_replayed_and_reported() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    type Check = fn(&[u8]);
    let targets: &[(&str, Check)] = &[
        ("regressions/jsonl_loader", |b| {
            let _ = loader::check(b);
        }),
        ("regressions/migration_chain", |b| {
            let _ = open_path::check(b);
        }),
        ("regressions/command_sequence", |b| {
            let _ = ew_fuzz::command::check(b);
        }),
        ("regressions/png_pdf", |b| {
            let _ = ew_fuzz::png_pdf::check(b);
        }),
        ("regressions/ewz_read", |b| {
            let _ = ew_fuzz::ewz_read::check(b);
        }),
        ("regressions/merge_file", |b| {
            let _ = ew_fuzz::merge_file::check(b);
        }),
        ("regressions/sql_sandbox", |b| {
            let _ = ew_fuzz::sql_sandbox::check(b);
        }),
        ("regressions/svg_asset", |b| {
            let _ = ew_fuzz::svg_asset::check(b);
        }),
        ("regressions/image_decode", |b| {
            let _ = ew_fuzz::image_decode::check(b);
        }),
        ("regressions/textconv", |b| {
            let _ = ew_fuzz::textconv::check(b);
        }),
        ("regressions/table_round_trip", |b| {
            let _ = ew_fuzz::table_round_trip::check(b);
        }),
        ("regressions/command_log", |b| {
            let _ = ew_fuzz::command_log::check(b);
        }),
        ("regressions/qet_project", |b| {
            let _ = ew_fuzz::importers::qet_project(b);
        }),
        ("regressions/qet_element", |b| {
            let _ = ew_fuzz::importers::qet_element(b);
        }),
        ("regressions/golden_tables", |b| {
            let _ = ew_fuzz::importers::golden_tables(b);
        }),
        ("regressions/parts_table", |b| {
            let _ = ew_fuzz::importers::parts_table(b);
        }),
        ("regressions/ewlib_read", |b| {
            let _ = ew_fuzz::ewlib_read::check(b);
        }),
        ("regressions/ewui_script", |b| {
            let _ = ew_fuzz::ewui_script::check(b);
        }),
    ];
    for &(dir, check) in targets {
        for (name, bytes) in inputs(dir) {
            let failed = catch_unwind(AssertUnwindSafe(|| check(&bytes))).is_err();
            eprintln!(
                "regression {dir}/{name}: {} bytes, {}",
                bytes.len(),
                if failed {
                    "fails (a reported finding)"
                } else {
                    "passes (fixed: make it a plain test of the crate)"
                }
            );
        }
    }
}
