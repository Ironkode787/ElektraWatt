// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC exchange (doc 10 §7): `ew export plc-aml`, `plc-xlsx` and `plcopen`, `ew import plc-aml`, `plc-xlsx` and
//! `instrument-list`, and `ew plc status`.

use super::*;

use ew_api::plc_exchange::{PlcExchangeError, PlcImport, PlcOptions};

/// The line of a refused exchange.
fn refusal(e: &PlcExchangeError) -> String {
    format!("error: {e}")
}

fn origin(subcommand: &str) -> ew_api::Origin {
    ew_api::Origin::Cli {
        subcommand: subcommand.to_owned(),
        confirmed: false,
    }
}

/// Saves after an exchange command; `false`, with the error printed, when the save fails.
fn save(session: &mut ew_api::Session) -> bool {
    match session.save() {
        Ok(r) => {
            warn_unreleased(r.unstable, &mut std::io::stderr());
            true
        }
        Err(e) => {
            eprintln!("error: the save failed: {e:?}");
            false
        }
    }
}

/// The document date of an exchange: `SOURCE_DATE_EPOCH`, else the model's (`None`).
fn document_date() -> Result<Option<ew_api::plc_exchange::Date>, String> {
    Ok(source_date()?.and_then(|d| ew_api::plc_exchange::Date::new(d.year, d.month, d.day).ok()))
}

/// `ew export plc-aml | plc-xlsx | plcopen`: the configuration project as an exchange file in `--out`, written whole
/// or not at all, and recorded in the project as the base of the next import's merge (`plc.record_exchange`), which is
/// saved.
pub(super) fn run_plc_export(a: &PlcExportArgs) -> i32 {
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    let date = match document_date() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let mut session = match open_for_writing(&dir, origin("export plc")) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let options = PlcOptions {
        config: a.config.clone(),
        stations: a.stations.clone(),
        date,
        lang: a.lang.clone(),
        ..PlcOptions::default()
    };
    let export = match session.plc_export(a.format, &options) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{}", refusal(&e));
            session.close();
            return 2;
        }
    };
    if let Err(e) = write_atomically(&a.out, &export.bytes) {
        eprintln!("error: {}: {e}", a.out.display());
        session.close();
        return 2;
    }
    if let Err(e) = session.execute(&export.record) {
        eprintln!("error: the file is written, but the exchange is not recorded: {e}");
        session.close();
        return 2;
    }
    let saved = save(&mut session);
    session.close();
    if !saved {
        return 2;
    }
    for w in &export.warnings {
        eprintln!("warning: {w}");
    }
    let [stations, modules, channels, tags] = export.counts;
    if a.json {
        println!(
            "{}",
            serde_json::json!({
                "stations": stations,
                "modules": modules,
                "channels": channels,
                "tags": tags,
                "warnings": export.warnings,
            })
        );
    } else {
        println!(
            "wrote {stations} station(s), {modules} module(s), {channels} channel(s) and {tags} tag(s); the exchange \
             is recorded in the project"
        );
    }
    0
}

/// The text preview of an import: one line per change with its key, the parked objects and the removed ones, then
/// the counts.
fn import_text(import: &PlcImport, applied: bool) -> String {
    use std::fmt::Write as _;
    let side = |v: &Option<String>| v.clone().unwrap_or_else(|| "nothing".to_owned());
    let mut out = String::new();
    for c in &import.plan.changes {
        let _ = writeln!(
            out,
            "{}: {}: {} {} → {}  [{}]",
            c.state.word(),
            c.subject,
            c.field,
            side(&c.ours),
            side(&c.theirs),
            c.id
        );
    }
    for p in &import.plan.parking {
        let _ = writeln!(
            out,
            "parked: {} {} ({}){}  [{}]",
            p.object.name(),
            p.name,
            if p.ambiguous {
                "several objects of the project match it"
            } else {
                "nothing in the project matches it"
            },
            if p.creatable {
                "; `create` creates it unplaced"
            } else {
                ""
            },
            p.id
        );
    }
    for (object, _, name) in &import.plan.removed {
        let _ = writeln!(out, "removed: {} {name} is no longer in the file", object.name());
    }
    let _ = writeln!(
        out,
        "{} change(s), {} parked, {} removed{}",
        import.plan.changes.len(),
        import.plan.parking.len(),
        import.plan.removed.len(),
        if applied {
            "; applied and saved"
        } else {
            "; nothing is written: apply with --accept <choices.json> (`{}` takes every default)"
        }
    );
    out
}

/// `ew import plc-aml | plc-xlsx <file>`: the preview of an exchange file against the configuration project, and
/// with `--accept <choices.json>` its `plc.apply_exchange` committed and the project saved.
pub(super) fn run_plc_import(a: &PlcImportArgs) -> i32 {
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    let limit = ew_api::plc_exchange::max_file_bytes();
    let bytes = match read_bounded(&a.file, limit) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: {}: {e}", a.file.display());
            return 2;
        }
    };
    let resolutions = match &a.accept {
        None => None,
        Some(path) => match std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
        {
            Ok(r) => Some(r),
            Err(e) => {
                eprintln!(
                    "error: {}: {e}; the choices are a JSON object from a change's key or a parked object's ID to \
                     `ours`, `theirs` or `create`",
                    path.display()
                );
                return 2;
            }
        },
    };
    let date = match document_date() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let apply = resolutions.is_some();
    let mut session = if apply {
        match open_for_writing(&dir, origin("import plc")) {
            Ok(s) => s,
            Err(code) => return code,
        }
    } else {
        match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "import plc") {
            Ok((s, _)) => s,
            Err(e) => {
                eprintln!("error: {}: {e}", dir.display());
                return 2;
            }
        }
    };
    let options = PlcOptions {
        config: a.config.clone(),
        date,
        lang: a.lang.clone(),
        complete: a.complete,
        resolutions: resolutions.unwrap_or_default(),
        ..PlcOptions::default()
    };
    let import = match session.plc_import(&bytes, &options) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("{}", refusal(&e));
            session.close();
            return 2;
        }
    };
    if apply {
        let committed = session
            .preview(&import.apply)
            .and_then(|preview| session.commit(preview));
        if let Err(e) = committed {
            eprintln!("error: {e}");
            session.close();
            return 2;
        }
        if !save(&mut session) {
            session.close();
            return 2;
        }
    }
    session.close();
    if a.json {
        let mut value = import.to_json();
        value["applied"] = serde_json::Value::Bool(apply);
        println!("{value}");
    } else {
        print!("{}", import_text(&import, apply));
    }
    0
}

/// `ew import instrument-list <file>`: the rows of an instrument list against the configuration project, and with
/// `--apply` its `plc.apply_instrument_list` committed and the project saved.
pub(super) fn run_instrument_list(a: &InstrumentListArgs) -> i32 {
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    let bytes = match read_bounded(&a.file, ew_api::catalog::table_limits().max_input_bytes) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: {}: {e}", a.file.display());
            return 2;
        }
    };
    let mut session = match open_for_writing(&dir, origin("import instrument-list")) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let preview = session
        .instrument_list(a.config.as_deref(), &bytes)
        .map_err(|e| refusal(&e))
        .and_then(|command| session.preview(&command).map_err(|e| format!("error: {e}")));
    let preview = match preview {
        Ok(p) => p,
        Err(line) => {
            eprintln!("{line}");
            session.close();
            return 2;
        }
    };
    let notices = preview.outcome().notices.clone();
    if a.apply {
        if let Err(e) = session.commit(preview) {
            eprintln!("error: {e}");
            session.close();
            return 2;
        }
        if !save(&mut session) {
            session.close();
            return 2;
        }
    }
    session.close();
    if a.json {
        println!("{}", serde_json::json!({"rows": notices, "applied": a.apply}));
    } else {
        for n in &notices {
            println!("{n}");
        }
        println!(
            "{}",
            if a.apply {
                "applied and saved"
            } else {
                "nothing is written: apply with --apply"
            }
        );
    }
    0
}

/// `ew plc status [<file>]`: the last exchange of the configuration project, what changed in the project since
/// (EW-PLC-016), and whether `<file>` is the file last exchanged. Exit 0 with the status, 2 on usage and open errors.
pub(super) fn run_plc_status(a: &PlcStatusArgs) -> i32 {
    let Some(dir) = reader_dir(a.project.as_ref()) else {
        return 2;
    };
    let file = match &a.file {
        None => None,
        Some(path) => match read_bounded(path, ew_api::plc_exchange::max_file_bytes()) {
            Ok(b) => Some(b),
            Err(e) => {
                eprintln!("error: {}: {e}", path.display());
                return 2;
            }
        },
    };
    let session = match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "plc status") {
        Ok((s, _)) => s,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let status = session.plc_status(a.config.as_deref(), file.as_deref());
    session.close();
    let status = match status {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", refusal(&e));
            return 2;
        }
    };
    if a.json {
        println!(
            "{}",
            serde_json::json!({
                "config": status.config,
                "exchange": status.stamp,
                "drift": status.drift.iter().map(|(object, fields)| serde_json::json!({
                    "object": object,
                    "fields": fields,
                })).collect::<Vec<_>>(),
                "same_file": status.same_file,
            })
        );
        return 0;
    }
    match &status.stamp {
        None => println!("{}: not exchanged yet", status.config),
        Some(s) => println!(
            "{}: last exchange of {}: {} {}{}",
            status.config,
            s.date,
            serde_json::to_value(&s.direction)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            serde_json::to_value(&s.format)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            s.version.as_ref().map(|v| format!(" {v}")).unwrap_or_default()
        ),
    }
    if status.stamp.is_some() {
        if status.drift.is_empty() {
            println!("the project is as exchanged");
        }
        for (object, fields) in &status.drift {
            if fields.is_empty() {
                println!("changed since: {object} (added or gone)");
            } else {
                println!("changed since: {object}: {}", fields.join(", "));
            }
        }
    }
    match status.same_file {
        Some(true) => println!("the file is the one last exchanged"),
        Some(false) => println!(
            "the file is not the one last exchanged: `ew import plc-aml` (or `plc-xlsx`) previews what it changes"
        ),
        None => {}
    }
    0
}
