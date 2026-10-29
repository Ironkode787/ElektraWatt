// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Build path (b) of Golden A (doc 20 §3.2, WP-S6.01): the builder DSL constructs the golden project
//! through commands only, from the shell; the acceptance SQL over it is compared with the expected CSVs.
//!
//! The acceptance runner (`crates/ew-api/tests/acceptance.rs`, AC-15) makes the comparison with the expected
//! CSVs (the six SQL results and both parts lists); these tests pin what the builder writes and leaves out,
//! record for record, its determinism, the replay of its command log, the undo of every step and a save after it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU32;

use ew_api::{AnyCommand, CmdLog, HistoryLimits, Session, SessionOptions};
use ew_commands::Cost;
use ew_testkit::golden::dsl::Done;
use ew_testkit::golden::{self, Built};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Runs one command on `session`: a bulk command as preview and commit.
fn run(session: &mut Session, name: &str, params: serde_json::Value) -> Result<Done, String> {
    let cmd = AnyCommand {
        name: name.to_owned(),
        params,
    };
    let bulk = session
        .registry()
        .get(name)
        .is_some_and(|e| e.meta().cost == Cost::Bulk);
    let receipt = if bulk {
        session.preview(&cmd).and_then(|p| session.commit(p))
    } else {
        session.execute(&cmd)
    };
    receipt
        .map(|r| Done {
            primary: r.outcome.primary,
            created: r.created,
        })
        .map_err(|e| format!("{e:?}"))
}

/// A session over the shell of path (a)'s batch, IDs from `seed`, without the memo.
fn shell_session(seed: u64) -> Session {
    let batch = golden::bootstrap(&repo()).expect("path (a) imports");
    session_over(golden::shell(&batch), seed)
}

/// A session over `shell`, IDs from `seed`, without the memo.
fn session_over(shell: golden::Shell, seed: u64) -> Session {
    Session::from_records(
        shell.project,
        [],
        shell.lib,
        SessionOptions {
            ids: Some(Box::new(ew_testkit::synth::SynthSource::new(seed))),
            memo: ew_api::MemoMode::Off,
            history: HistoryLimits {
                max_steps: 10_000,
                ..HistoryLimits::default()
            },
            ..SessionOptions::default()
        },
    )
    .expect("the shell is a valid model")
}

fn built(seed: u64) -> (Session, Built) {
    let batch = golden::bootstrap(&repo()).expect("path (a) imports");
    let mut session = shell_session(seed);
    let built = golden::build(&batch, &mut |name: &str, params| run(&mut session, name, params))
        .unwrap_or_else(|e| panic!("path (b): {e}"));
    (session, built)
}

/// The same seed builds the same model and the same script. That the SQL results do not depend on the minted IDs is
/// AC-15's comparison of paths (a), (b) and (c), which mint different IDs (test audit g7).
#[test]
fn path_b_is_deterministic_and_compares_by_content() {
    let (a, built_a) = built(1);
    let (b, built_b) = built(1);
    assert!(
        a.world().model.content_eq(&b.world().model),
        "same seed, same model"
    );
    assert_eq!(built_a.script, built_b.script);
}

/// The command log of path (b) replays with `Session::run_script` (`ew apply`) on the shell with the same
/// ID source to the same model; undoing every step, `library.embed` included (FT-316), returns to the shell,
/// and a save after the undo reloads the shell.
#[test]
fn the_command_log_replays_and_every_step_undoes() {
    let (mut session, built) = built(1);
    let log = CmdLog::parse(&built.command_log("Golden A, path (b)")).expect("the log parses");
    assert_eq!(log.len(), built.script.len());
    let mut replay = shell_session(1);
    replay
        .run_script(&log, "golden-a-path-b.ewcmd")
        .unwrap_or_else(|e| panic!("replay: {e}"));
    assert!(
        replay.world().model.content_eq(&session.world().model),
        "the replay builds the same model"
    );
    // The built project is saved before the undo, so that the save after it is the incremental one of
    // every reverted entity (deleted files, the library copies gone).
    let dir = scratch_dir("undo-all");
    session
        .create(&dir, store_options())
        .unwrap_or_else(|e| panic!("create: {e:?}"));
    let mut steps = 0;
    let mut last = None;
    while let Ok(restored) = session.undo(&ew_api::Origin::Gui) {
        steps += 1;
        last = Some(restored.label);
    }
    // One undo step per command that changed the model: two of the commands leave the model as it was (a
    // value the record already had), so they leave no step. FT-316 (cycle 19): the last step undone is the
    // first command, `library.embed`, although the shell's project-local parts name the embedded function
    // definitions (`part.function_template[].fndef`): the `LibRef` keep-alive is the delete-time rule
    // EW-INV-021, which undo skips, so the state before the embedding (a `LibRef` to an item the project
    // does not embed names nothing) is restored.
    assert_eq!((steps, built.script.len()), (607, 609), "undo steps, commands");
    assert_eq!(
        last.as_deref(),
        Some("library.embed"),
        "the first command is undone last"
    );
    let refused = session
        .undo(&ew_api::Origin::Gui)
        .expect_err("the history is empty");
    assert!(
        matches!(refused, ew_api::CmdError::History { .. }),
        "undo stops only because nothing is left: {refused:?}"
    );
    let batch = golden::bootstrap(&repo()).expect("path (a) imports");
    let shell = session_over(golden::shell(&batch), 1).world().model;
    let model = session.world().model;
    assert!(model.content_eq(&shell), "undo-all restores the shell");
    model
        .integrity_check()
        .unwrap_or_else(|e| panic!("the shell after undo-all: {e:?}"));
    // A save after undo-all round-trips: the reloaded project is the shell, without an integrity finding.
    session
        .save()
        .unwrap_or_else(|e| panic!("save after undo-all: {e:?}"));
    let (reader, report) =
        Session::open_reader_with(&dir, "ElektraWatt test", "check", ew_api::MemoMode::Off)
            .unwrap_or_else(|e| panic!("reload: {e:?}"));
    assert!(report.findings.is_empty(), "{:?}", report.findings);
    assert!(
        reader.world().model.content_eq(&shell),
        "the project saved after undo-all reloads as the shell"
    );
    drop((reader, session));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A fresh scratch project directory under the test target directory.
fn scratch_dir(name: &str) -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    ew_testdir::fresh_dir_in(
        Path::new(env!("CARGO_TARGET_TMPDIR")),
        &format!("ew-golden-dsl-{name}"),
        &COUNTER,
    )
}

/// Store options without a journal (the test is not a crash test).
fn store_options() -> ew_api::StoreOptions {
    let mut o = ew_api::StoreOptions::new("ElektraWatt test");
    o.state_dir = None;
    o
}

/// FT-278: the shell holds the project row and the project-local parts only; the non-local copies come
/// from one `library.embed`, the first command, and path (b) holds the library of path (a) in the end.
#[test]
fn the_builder_embeds_the_non_local_library_copies() {
    let batch = golden::bootstrap(&repo()).expect("path (a) imports");
    let shell = golden::shell(&batch);
    let project = shell.project.id.as_uuid();
    let local = |d: &ew_schema::LibDocument| match d {
        ew_schema::LibDocument::Part(c) => c.library_id.as_uuid() == project,
        // WP-S1.19 D10: the manufacturer copies the parts name are project-local like them.
        ew_schema::LibDocument::Manufacturer(c) => c.library_id.as_uuid() == project,
        _ => false,
    };
    assert!(
        !shell.lib.is_empty() && shell.lib.iter().all(local),
        "the shell keeps only project-local parts and manufacturers"
    );
    let copies = golden::embedded_copies(&batch);
    assert!(!copies.is_empty() && !copies.iter().any(local));
    assert_eq!(
        shell.lib.len() + copies.len(),
        batch.lib.len(),
        "nothing else is dropped"
    );
    // The build half (the first command is the one `library.embed` of all the copies; path (b) holds the library
    // of path (a)) is asserted on the one build of `path_b_equals_path_a_record_by_record_up_to_ids`.
}

/// Batch ID text to built ID text over a JSON value, the way the builder remaps a record: a sorted list
/// of IDs (a set) stays sorted under the new IDs.
fn remap_value(ids: &BTreeMap<String, String>, v: &mut serde_json::Value) {
    use serde_json::Value;
    match v {
        Value::String(s) => {
            if let Some(to) = ids.get(s.as_str()) {
                *s = to.clone();
            }
        }
        Value::Array(items) => {
            let sorted_ids = items.len() > 1
                && items
                    .iter()
                    .any(|x| x.as_str().is_some_and(|s| ids.contains_key(s)))
                && items.iter().all(Value::is_string)
                && items.windows(2).all(|w| w[0].as_str() < w[1].as_str());
            items.iter_mut().for_each(|x| remap_value(ids, x));
            if sorted_ids {
                items.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
            }
        }
        Value::Object(m) => m.values_mut().for_each(|x| remap_value(ids, x)),
        _ => {}
    }
}

/// Every row of a session's model as `kind:id` → its JSON without `source_ref`.
fn rows_by_key(session: &Session) -> BTreeMap<String, serde_json::Value> {
    ew_commands::rows::model_lines(&session.world().model)
        .expect("every row has a JSON form")
        .into_iter()
        .map(|(k, text)| {
            let mut v: serde_json::Value = serde_json::from_str(&text).expect("a JSON row");
            if let Some(m) = v.as_object_mut() {
                m.remove("source_ref");
            }
            (format!("{}:{}", k.0.name(), k.1), v)
        })
        .collect()
}

/// FT-337 and FT-341: the DSL verbs for the logo and for a block whose form comes from the library send the
/// parameters of their commands.
#[test]
fn the_logo_and_library_form_verbs_send_their_command_parameters() {
    use ew_schema::{Kind, ReportBlock, ReportKind, ReportSource};
    let block_id = ew_ids::Id::<ReportBlock>::from_uuid(ew_ids::Uuid::from_u128(7));
    let mut sink = |name: &str, _params: serde_json::Value| -> Result<Done, String> {
        Ok(Done {
            primary: (name == "report_block.create").then_some((Kind::ReportBlock, block_id.cast())),
            ..Done::default()
        })
    };
    let mut b = golden::dsl::Builder::new(&mut sink);
    b.company_logo(b"<svg/>").unwrap();
    let block = ReportBlock {
        id: block_id,
        v: 1,
        source: ReportSource::Kind(ReportKind::Toc),
        manual: Default::default(),
        sort: Vec::new(),
        group: None,
        form: ew_ids::Id::from_uuid(ew_ids::Uuid::from_u128(8)),
        section: None,
        first_folio: Some(2),
        continuation: Default::default(),
        title: Default::default(),
        empty: Default::default(),
        embedded: None,
        source_ref: None,
        extra: Default::default(),
    };
    let copy = serde_json::json!({"$k": "form", "id": "00000000-0000-0000-0000-000000000008"});
    assert_eq!(
        b.report_block_from_library(&block, std::slice::from_ref(&copy))
            .unwrap(),
        block_id
    );
    let script = b.script();
    assert_eq!(script[0].0, "project.company.set");
    assert_eq!(script[0].1, serde_json::json!({"logo": "PHN2Zy8+"}));
    assert_eq!(script[1].0, "report_block.create");
    assert_eq!(script[1].1["form_copies"], serde_json::json!([copy]));
    assert_eq!(script[1].1["first_folio"], 2);
}

/// Path (b) holds the records of path (a) record for record, up to the IDs the commands minted: every
/// record of the bootstrap (its `source_ref` aside), with its references remapped to the built IDs,
/// equals the built record under the built ID, and path (b) holds no other record. The import lineage is
/// path (a)'s own. The revisions (typed since FT-165) are compared like every other record, and so are the
/// library copies (FT-278).
///
/// On the same build: what path (b) issues and what it cannot write yet (no command), pinned so that a new command
/// or a table change shows here (since FT-165 path (b) writes every record: nothing is left out); and FT-278's
/// build half: the first command is the one `library.embed` of every non-local copy.
#[test]
fn path_b_equals_path_a_record_by_record_up_to_ids() {
    use ew_schema::Record;
    let batch = golden::bootstrap(&repo()).expect("path (a) imports");
    let project = batch
        .records
        .iter()
        .find_map(|r| match r {
            Record::Project(p) => Some(p.clone()),
            _ => None,
        })
        .expect("the project row");
    let records = batch
        .records
        .iter()
        .filter(|r| !matches!(r, Record::Project(_)))
        .cloned();
    let a = Session::from_records(project, records, batch.lib.clone(), SessionOptions::default())
        .expect("path (a) is a valid model");
    let (b, built) = built(1);
    let commands: Vec<String> = built
        .by_command
        .iter()
        .map(|(k, n)| format!("{k}: {n}"))
        .collect();
    assert_eq!(
        commands.join(", "),
        "assembly.attach: 2, bond.create: 1, box.black: 11, cable.create: 3, connection.draw: 87, \
         device.create: 37, device.link: 8, device.place: 76, external_ref.create: 4, fields.set: 204, \
         ip.place: 48, junction.edit: 6, library.embed: 1, page.create: 13, plc.bind_channel: 28, \
         plc.config_project.create: 1, potential.define: 19, project.settings.set: 3, \
         project.supply.set: 1, report_block.create: 15, revision.create: 2, section.create: 3, \
         structure.create_node: 4, terminal.add: 4, valve.add: 28"
    );
    assert_eq!(built.unwritten_summary(), "");
    assert!(built.unwritten.is_empty());
    let (name, params) = &built.script[0];
    assert_eq!(name, "library.embed", "the first command embeds the copies");
    assert_eq!(
        params["copies"].as_array().map(Vec::len),
        Some(golden::embedded_copies(&batch).len()),
        "one library.embed of every non-local copy"
    );
    let ids: BTreeMap<String, String> = built.ids.clone();
    let a_rows = rows_by_key(&a);
    let b_rows = rows_by_key(&b);
    let built_key = |key: &str| {
        let (kind, id) = key.split_once(':').expect("kind:id");
        format!("{kind}:{}", ids.get(id).map_or(id, String::as_str))
    };
    let mut differ = Vec::new();
    let mut only_a = BTreeMap::<String, usize>::new();
    for (key, v) in &a_rows {
        let kind = key.split_once(':').map_or("", |(k, _)| k);
        if kind == "import_lineage" {
            continue;
        }
        let mut v = v.clone();
        remap_value(&ids, &mut v);
        if let (true, Some(m)) = (kind == "project", v.as_object_mut()) {
            m.remove("lineage"); // the import provenance of the project row (path (a) only), if any
        }
        match b_rows.get(&built_key(key)) {
            Some(bv) if *bv == v => {}
            Some(bv) => differ.push(format!("{}\n  a: {v}\n  b: {bv}", built_key(key))),
            None => *only_a.entry(kind.to_owned()).or_default() += 1,
        }
    }
    let count = |rows: &BTreeMap<String, serde_json::Value>, kind: &str| {
        rows.keys().filter(|k| k.starts_with(&format!("{kind}:"))).count()
    };
    for (kind, n) in [
        ("device", 96), // the 28 valves and the mounting plate included
        ("channel", 56),
        ("device_link", 8),
        ("plc_config_project", 1),
        ("revision", 2),
    ] {
        assert_eq!((kind, count(&b_rows, kind)), (kind, n), "rows of path (b)");
    }
    let a_keys: std::collections::BTreeSet<String> = a_rows.keys().map(|k| built_key(k)).collect();
    let only_b: Vec<&String> = b_rows.keys().filter(|k| !a_keys.contains(*k)).collect();
    assert!(
        differ.is_empty() && only_a.is_empty() && only_b.is_empty(),
        "{} records differ:\n{}\nonly in path (a): {only_a:?}\nonly in path (b): {only_b:?}",
        differ.len(),
        differ.join("\n")
    );
}
