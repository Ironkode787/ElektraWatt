// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew apply` and its preview, `ew update-copies`, `ew commands` and `ew schema`.

use super::*;

pub(super) fn run_apply(a: &ApplyArgs) -> i32 {
    let text = match std::fs::read_to_string(&a.log) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {}: {e}", a.log.display());
            return 2;
        }
    };
    let log = match CmdLog::parse(&text) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: {e}");
            return i32::from(e.exit_code());
        }
    };
    // The origin names the log by its file name, never its path.
    let file = a.log.file_name().map_or_else(
        || a.log.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    if a.preview {
        return run_apply_preview(&dir, &log, &file, a.json);
    }
    let print_result = |committed: usize, saved: bool| {
        if a.json {
            print!(
                "{}",
                results::to_text(&results::ApplyResult {
                    schema: results::APPLY.to_owned(),
                    lines: log.len(),
                    committed,
                    saved,
                    preview: None,
                })
            );
        }
    };
    let origin = ew_api::Origin::Cli {
        subcommand: "apply".to_owned(),
        confirmed: false,
    };
    // A log that pins an address profile reads the libraries of `ew lib` (the content's system
    // libraries, the user's), so that a line naming a library's profile embeds it first; any other log reads none.
    let tiers = log
        .commands()
        .any(|c| c.name == "plc.set_profile")
        .then(|| {
            let system: Vec<PathBuf> = content_dir().map(|d| d.join("libraries")).into_iter().collect();
            ew_api::library::LibraryTiers::for_user(system, Vec::new())
        })
        .flatten();
    let store = ew_api::StoreOptions::new(env!("CARGO_PKG_VERSION"));
    let mut session = match open_for_writing_from(&dir, origin, store, tiers) {
        Ok((s, _)) => s,
        Err(code) => return code,
    };
    let result = session.run_script(&log, &file);
    let (code, committed) = match &result {
        Ok(receipts) => (0, receipts.len()),
        Err(e) => {
            eprintln!("error: {e}");
            (i32::from(e.exit_code()), e.committed())
        }
    };
    if code != 0 && (a.one_step || committed == 0) {
        if a.one_step && committed > 0 {
            eprintln!("--one-step: nothing saved ({committed} lines were replayed and discarded)");
        }
        session.close();
        print_result(committed, false);
        return code;
    }
    match session.save() {
        Ok(saved) => {
            warn_unreleased(saved.unstable, &mut std::io::stderr());
            if a.json {
                print_result(committed, true);
            } else {
                println!("applied {committed} of {} lines; saved", log.len());
            }
            session.close();
            code
        }
        Err(e) => {
            eprintln!("error: the save failed: {e:?}");
            session.close();
            2
        }
    }
}

/// `ew update-copies` (FT-1499, decision log ID-1486): every damaged library copy that a library of `ew lib` holds is
/// written from it, as the window's Update from library writes it, and the project opens again; a copy no library
/// holds is named. A refusal is printed in its words, nothing written.
pub(super) fn run_update_copies(a: &UpdateCopiesArgs) -> i32 {
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    let system: Vec<PathBuf> = content_dir().map(|d| d.join("libraries")).into_iter().collect();
    let Some(tiers) = ew_api::library::LibraryTiers::for_user(system, a.company.clone()) else {
        eprintln!("error: no data and cache directories (set EW_DATA_DIR and EW_CACHE_DIR)");
        return 2;
    };
    let open = || {
        let options = ew_api::SessionOptions {
            origin: ew_api::Origin::Cli {
                subcommand: "update-copies".to_owned(),
                confirmed: false,
            },
            libraries: Some(tiers.clone()),
            ..ew_api::SessionOptions::default()
        };
        let store = ew_api::StoreOptions::new(env!("CARGO_PKG_VERSION"));
        ew_api::Session::open(&dir, store, options, ew_api::Recovery::Keep).map_err(|e| {
            eprintln!("error: {}: {e}", dir.display());
            2
        })
    };
    let (mut session, _) = match open() {
        Ok(opened) => opened,
        Err(code) => return code,
    };
    let done = session.update_damaged_copies();
    session.close();
    let done = match done {
        Ok(done) => done,
        Err(refusal) => {
            eprintln!("error: {}: {refusal}", dir.display());
            return if refusal.key() == "refusal-update-copies-save" {
                2
            } else {
                1
            };
        }
    };
    for copy in &done.updated {
        println!("updated {copy} from its library");
    }
    for copy in &done.not_updated {
        eprintln!("not updated, as no library on this computer holds a readable version: {copy}");
    }
    // The project again: writable when the integrity check passes.
    let (session, report) = match open() {
        Ok(opened) => opened,
        Err(code) => return code,
    };
    session.close();
    if report.access != ew_api::Access::Edit {
        eprintln!("the project still opens read-only");
        return 1;
    }
    if done.updated.is_empty() {
        println!("the project has no damaged library copy");
    } else {
        println!("the project opens for editing");
    }
    i32::from(!done.not_updated.is_empty())
}

/// `ew apply --preview`: the replay on a read-only session (no lock, no journal, never saved), then the changes.
pub(super) fn run_apply_preview(dir: &Path, log: &CmdLog, file: &str, json: bool) -> i32 {
    let (mut session, _) = match ew_api::Session::open_reader(dir, env!("CARGO_PKG_VERSION"), "apply") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let before = session.world();
    let result = session.run_script(log, file);
    let (code, committed) = match &result {
        Ok(receipts) => (0, receipts.len()),
        Err(e) => {
            eprintln!("error: {e}");
            (i32::from(e.exit_code()), e.committed())
        }
    };
    let diff = match session.diff_from(&before) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e:?}");
            return 2;
        }
    };
    if json {
        fn row<K: std::fmt::Display>(r: &ew_api::DerivedChange<K>) -> results::DerivedRowChange {
            results::DerivedRowChange {
                key: r.key.to_string(),
                before: r.before.clone(),
                after: r.after.clone(),
            }
        }
        let records = diff
            .model
            .changes
            .iter()
            .map(|c| results::RecordChange {
                change: format!("{:?}", c.change),
                kind: c.key.0.to_string(),
                id: c.key.1.to_string(),
                fields: c.fields.iter().map(ToString::to_string).collect(),
            })
            .collect();
        print!(
            "{}",
            results::to_text(&results::ApplyResult {
                schema: results::APPLY.to_owned(),
                lines: log.len(),
                committed,
                saved: false,
                preview: Some(results::ApplyChanges {
                    records,
                    tags: diff.derived.tags.iter().map(row).collect(),
                    xrefs: diff.derived.xrefs.iter().map(row).collect(),
                }),
            })
        );
        return code;
    }
    println!(
        "preview: {committed} of {} lines commit; {} records, {} tags and {} cross-references change; nothing is saved",
        log.len(),
        diff.model.changes.len(),
        diff.derived.tags.len(),
        diff.derived.xrefs.len()
    );
    for c in &diff.model.changes {
        let (kind, id) = c.key;
        let fields = if c.fields.is_empty() {
            String::new()
        } else {
            format!(": {}", c.fields.join(", "))
        };
        println!("{:?} {kind} {id}{fields}", c.change);
    }
    for t in &diff.derived.tags {
        println!("tag {}: {} -> {}", t.key, t.before, t.after);
    }
    for x in &diff.derived.xrefs {
        println!("xref {}: {} -> {}", x.key, x.before, x.after);
    }
    code
}

/// `ew commands [--json]`: the command registry, one line per command (name, parameters version,
/// exposure, cost, confirmation, undoable, stability, aliases), or its JSON result.
pub(super) fn run_commands(json: bool) -> i32 {
    let list = results::commands();
    if json {
        print!("{}", results::to_text(&list));
        return 0;
    }
    for c in &list.commands {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            c.name,
            c.params_version,
            c.exposure,
            c.cost,
            c.confirmation,
            if c.undoable { "undoable" } else { "not undoable" },
            c.stability,
            c.aliases.join(",")
        );
    }
    0
}

/// `ew schema <name>`: the parameter schema of a command (or an alias), byte for byte the file of
/// `crates/ew-commands/schemas/`, or a result schema (`ew.<command>-result/<n>`), the file of `bin/ew/schemas/`. An
/// unknown name exits 2 naming it.
pub(super) fn run_schema(name: &str) -> i32 {
    let text = if name.starts_with("ew.") && name.contains("-result/") {
        results::schema_text(name)
    } else {
        results::command_schema_text(name)
    };
    match text {
        Some(t) => {
            print!("{t}");
            0
        }
        None => {
            eprintln!(
                "error: unknown schema `{}` (a command name of `ew commands`, or a result schema `ew.<command>-result/<n>`)",
                name.escape_debug()
            );
            2
        }
    }
}
