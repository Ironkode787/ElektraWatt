// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The imports: a table of rows, a parts catalogue, a QElectroTech project, a CSV.

use super::*;

/// `ew import table`'s line for a refused file: `error: ` and the refusal with its first letter in lower case, as every
/// `ew` error line (S2 of WP-S6.04 for a read that panicked; ruling of C51-p1#2).
pub(super) fn import_table_error_line(e: &ew_api::output::TableImportError) -> String {
    let text = e.to_string();
    let mut chars = text.chars();
    let first = chars
        .next()
        .map(|c| c.to_lowercase().collect::<String>())
        .unwrap_or_default();
    format!("error: {first}{}", chars.as_str())
}

/// `ew import table` (WP-S3.08 D11): the preview of a round-trip file against the project, and with `--apply` its one
/// `fields.set_many` committed and the project saved.
pub(super) fn run_import_table(a: &ImportTableArgs) -> i32 {
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
    let options = ew_api::output::ImportTableOptions {
        table: a.table.clone(),
        preset: a.preset,
        take_file: a.take_file,
    };
    let origin = ew_api::Origin::Cli {
        subcommand: "import table".to_owned(),
        confirmed: false,
    };
    let mut session = if a.apply {
        match open_for_writing(&dir, origin.clone()) {
            Ok(s) => s,
            Err(code) => return code,
        }
    } else {
        match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "import table") {
            Ok((s, _)) => s,
            Err(e) => {
                eprintln!("error: {}: {e}", dir.display());
                return 2;
            }
        }
    };
    let file = a.file.file_name().map_or_else(
        || a.file.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let import = match ew_api::output::guard_table_read(&file, || {
        debug_panic("table-read");
        session.table_import(&bytes, &options)
    }) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("{}", import_table_error_line(&e));
            session.close();
            return 2;
        }
    };
    let mut applied = false;
    if a.apply && import.command.is_some() {
        if let Err(e) = session.apply_table_import(&import, ew_api::Submit::from(origin)) {
            eprintln!("error: {e}");
            session.close();
            return 2;
        }
        if let Err(e) = session
            .save()
            .map(|r| warn_unreleased(r.unstable, &mut std::io::stderr()))
        {
            eprintln!("error: the save failed: {e:?}");
            session.close();
            return 2;
        }
        applied = true;
    }
    session.close();
    if a.json {
        print!("{}", import_table_json(&import, applied));
    } else {
        print!("{}", import_table_text(&import, a.apply, applied));
    }
    0
}

/// The text preview of `ew import table`: one line per changed cell, the notices, the findings, the missing rows and
/// the counts.
pub(super) fn import_table_text(import: &ew_api::output::TableImport, apply: bool, applied: bool) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for notice in &import.notices {
        let _ = writeln!(out, "{notice}");
    }
    for cell in &import.cells {
        let base = cell
            .base
            .as_ref()
            .map_or_else(String::new, |b| format!("exported \"{b}\", "));
        let reason = cell
            .reason
            .as_ref()
            .map_or_else(String::new, |r| format!(" ({r})"));
        let _ = writeln!(
            out,
            "{}, {}: {base}project \"{}\", file \"{}\": {}{reason}",
            cell.row_label,
            cell.column_label,
            cell.ew,
            cell.file,
            cell.outcome.word()
        );
    }
    for finding in &import.findings {
        let _ = writeln!(out, "{}: {}", finding.rule, finding.message);
    }
    if !import.missing.is_empty() {
        let _ = writeln!(out, "not in the file (kept): {}", import.missing.join(", "));
    }
    let c = import.counts();
    let end = match (apply, applied) {
        (_, true) => "applied in one step; saved",
        (true, false) => "nothing to apply",
        (false, false) if import.command.is_some() => {
            "a preview: nothing changed (give `--apply` to apply it)"
        }
        (false, false) => "a preview: nothing to apply",
    };
    let _ = writeln!(
        out,
        "{} changed, {} kept, {} conflicts, {} refused, {} unknown, {} missing; {end}",
        c.changed, c.kept, c.conflicts, c.refused, c.unknown, c.missing
    );
    out
}

/// The `--json` result of `ew import table` ([`results::ImportTableResult`]).
pub(super) fn import_table_json(import: &ew_api::output::TableImport, applied: bool) -> String {
    let c = import.counts();
    results::to_text(&results::ImportTableResult {
        schema: results::IMPORT_TABLE.to_owned(),
        table: import.table.clone(),
        three_way: import.three_way,
        notices: import.notices.clone(),
        changed: c.changed,
        kept: c.kept,
        conflicts: c.conflicts,
        refused: c.refused,
        unknown: c.unknown,
        cells: import
            .cells
            .iter()
            .map(|cell| results::ImportTableCell {
                row: cell.row.clone(),
                label: cell.row_label.clone(),
                column: cell.column.clone(),
                column_label: cell.column_label.clone(),
                base: cell.base.clone(),
                ew: cell.ew.clone(),
                file: cell.file.clone(),
                outcome: cell.outcome.word().to_owned(),
                reason: cell.reason.clone(),
            })
            .collect(),
        findings: import
            .findings
            .iter()
            .map(|f| results::ImportTableFinding {
                rule: f.rule.to_owned(),
                row: f.row.clone(),
                column: f.column.clone(),
                message: f.message.clone(),
            })
            .collect(),
        missing: import.missing.clone(),
        command: import
            .command
            .as_ref()
            .map(|cmd| serde_json::json!({"cmd": cmd.name, "params": cmd.params})),
        applied,
        rev: import.rev,
    })
}

/// Reads the column map and the table of a mapped parts import (exit code 2 on failure).
pub(super) fn read_map_and_table(
    map: &Path,
    file: &Path,
) -> Result<(ew_api::catalog::ColumnMap, ew_api::catalog::ImportInput), i32> {
    let map = ew_api::catalog::read_column_map(map).map_err(|e| {
        eprintln!("error: the column map: {e}");
        2
    })?;
    let input = ew_api::catalog::read_input(file, &ew_api::catalog::Limits::default()).map_err(|e| {
        eprintln!("error: {e}");
        2
    })?;
    Ok((map, input))
}

/// The counts of a mapped parts import, as text.
pub(super) fn counts_text(i: &ew_api::catalog::PartsImport) -> String {
    let c = i.counts();
    format!(
        "{} new, {} updated, {} unchanged, {} duplicate, {} rejected",
        c.new, c.updated, c.unchanged, c.duplicate, c.rejected
    )
}

/// The row counts of a mapped parts import, as its JSON result names them.
pub(super) fn row_counts(i: &ew_api::catalog::PartsImport) -> results::RowCounts {
    let c = i.counts();
    results::RowCounts {
        new: c.new,
        updated: c.updated,
        unchanged: c.unchanged,
        duplicate: c.duplicate,
        rejected: c.rejected,
    }
}

/// The report of a mapped parts import, one JSON row per finding.
pub(super) fn report_rows(i: &ew_api::catalog::PartsImport) -> Vec<results::ReportRow> {
    i.report
        .entries
        .iter()
        .map(|e| results::ReportRow {
            code: e.code.to_string(),
            severity: match e.severity {
                ew_api::catalog::Severity::Info => results::ReportSeverity::Info,
                ew_api::catalog::Severity::Warning => results::ReportSeverity::Warning,
                ew_api::catalog::Severity::Error => results::ReportSeverity::Error,
            },
            line: e.location.as_ref().and_then(|l| l.line).map(u64::from),
            message: e.message.clone(),
        })
        .collect()
}

/// Prints the report of a mapped parts import: one line per finding (unmapped columns, rejected rows, split part
/// numbers, aliases, new manufacturers).
pub(super) fn print_catalog_rows(i: &ew_api::catalog::PartsImport) {
    for e in &i.report.entries {
        let severity = match e.severity {
            ew_api::catalog::Severity::Info => "info",
            ew_api::catalog::Severity::Warning => "warning",
            ew_api::catalog::Severity::Error => "error",
        };
        let at = e
            .location
            .as_ref()
            .and_then(|l| l.line)
            .map_or_else(String::new, |n| format!("line {n}: "));
        println!("{} {severity}: {at}{}", e.code, e.message);
    }
}

/// `ew import parts --map`: a supplier's table through a column map into the project.
pub(super) fn run_import_parts_mapped(a: &ImportPartsArgs, map: &Path, dir: &Path) -> i32 {
    let (map, input) = match read_map_and_table(map, &a.file) {
        Ok(r) => r,
        Err(code) => return code,
    };
    let master = match open_libraries(&a.company) {
        Ok(reg) => ew_api::catalog::manufacturer_master(&reg),
        Err(code) => return code,
    };
    let origin = ew_api::Origin::Cli {
        subcommand: "import parts".into(),
        confirmed: false,
    };
    let Ok(mut session) = open_for_writing(dir, origin.clone()) else {
        return 2;
    };
    let (import, cmd) = match session.catalog_parts(&input, &map, &master, a.as_of) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            session.close();
            return 2;
        }
    };
    if !a.json {
        print_catalog_rows(&import);
    }
    let (mut created, mut skipped) = (0, 0);
    if let Some(cmd) = cmd {
        let receipt = match session
            .preview_with(&cmd, ew_api::Submit::from(origin))
            .and_then(|preview| session.commit(preview))
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("error: {e}");
                session.close();
                return 2;
            }
        };
        (created, skipped) =
            ew_api::import::parts_import_counts(receipt.outcome.note.as_deref().unwrap_or_default())
                .unwrap_or((0, 0));
        if let Err(e) = session
            .save()
            .map(|r| warn_unreleased(r.unstable, &mut std::io::stderr()))
        {
            eprintln!("error: the save failed: {e:?}");
            session.close();
            return 2;
        }
    }
    if a.json {
        print!(
            "{}",
            results::to_text(&results::ImportPartsResult {
                schema: results::IMPORT_PARTS.to_owned(),
                source: input.name().to_string(),
                created,
                skipped,
                rows: Some(row_counts(&import)),
                report: report_rows(&import),
            })
        );
    } else {
        println!(
            "imported {}: {}; {created} created, {skipped} skipped; saved",
            input.name(),
            counts_text(&import)
        );
    }
    session.close();
    0
}

pub(super) fn run_import_parts(a: &ImportPartsArgs) -> i32 {
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    if let Some(map) = &a.map {
        return run_import_parts_mapped(a, map, &dir);
    }
    let origin = ew_api::Origin::Cli {
        subcommand: "import parts".into(),
        confirmed: false,
    };
    let Ok(mut session) = open_for_writing(&dir, origin.clone()) else {
        return 2;
    };
    let id = session.query().project().id.as_uuid();
    let parts = match ew_api::import::read_parts_tables(&a.file, id) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            session.close();
            return 2;
        }
    };
    let cmd = ew_api::AnyCommand {
        name: ew_api::import::PARTS_IMPORT.into(),
        params: parts.params(),
    };
    let receipt = match session
        .preview_with(&cmd, ew_api::Submit::from(origin))
        .and_then(|preview| session.commit(preview))
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            session.close();
            return 2;
        }
    };
    let (created, skipped) =
        ew_api::import::parts_import_counts(receipt.outcome.note.as_deref().unwrap_or_default())
            .unwrap_or((0, 0));
    if let Err(e) = session
        .save()
        .map(|r| warn_unreleased(r.unstable, &mut std::io::stderr()))
    {
        eprintln!("error: the save failed: {e:?}");
        session.close();
        return 2;
    }
    if a.json {
        print!(
            "{}",
            results::to_text(&results::ImportPartsResult {
                schema: results::IMPORT_PARTS.to_owned(),
                source: parts.source.to_string(),
                created,
                skipped,
                rows: None,
                report: Vec::new(),
            })
        );
    } else {
        println!(
            "imported {}: {created} created, {skipped} skipped, 0 errors; saved",
            parts.source
        );
    }
    session.close();
    0
}

pub(super) fn run_import_qet(a: &ImportQetArgs) -> i32 {
    let content = a.content.clone().or_else(content_dir);
    let library = match &content {
        Some(dir) => match StarterLibrary::from_dir(dir) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("error: starter content: {e}");
                return 2;
            }
        },
        None => {
            eprintln!(
                "warning: no starter content (--content or EW_CONTENT_DIR); folio reports are dropped and pages get the default frame"
            );
            StarterLibrary::empty()
        }
    };
    let bytes = match std::fs::read(&a.file) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: {}: {e}", a.file.display());
            return 2;
        }
    };
    let name = a.file.file_name().map_or_else(
        || a.file.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    if let Some(project) = &a.into {
        return run_reimport_qet(a, project, &name, bytes, &library);
    }
    let imported = match import_qet(&name, bytes, &library) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let c = imported.counts();
    let report = imported.migration_report();
    // With the report on stdout, stdout is exactly the report: the summary goes to stderr.
    let to_stderr = a.json || (a.report.is_some() && a.report_out.is_none());
    let say = |line: String| {
        if to_stderr {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
    };
    say(format!(
        "imported {name}: {} pages, {} devices ({} strips, {} terminals), {} functions, {} representations, {} connections, {} junctions",
        c.pages, c.devices, c.strips, c.terminals, c.functions, c.representations, c.connections, c.junctions
    ));
    say(format!(
        "report: {} findings, {} lost connections",
        report.total, report.lost_connections
    ));
    if !report.live.is_empty() {
        let n: u32 = report.live.iter().map(|g| g.count).sum();
        say(format!("checks: {n} lenient findings"));
    }
    let mut code = 0;
    let lenient: u32 = report.live.iter().map(|g| g.count).sum();
    let (derived_pages, model_check_error) = match &imported.session {
        Ok(s) => {
            say(format!("derived: {} pages in page order", s.page_labels().len()));
            (Some(s.page_labels().len()), None)
        }
        Err(msg) => {
            say(format!("model check failed: {msg}"));
            code = 1;
            (None, Some(msg.to_string()))
        }
    };
    // ID-1374 (C51-p2#7): as `import csv --strict`, a failed `--strict` writes no project.
    let mut strict_failed = false;
    if a.strict {
        let errors = imported
            .batch
            .report
            .entries
            .iter()
            .filter(|e| e.severity == Severity::Error)
            .count();
        if report.lost_connections > 0 {
            eprintln!("strict: {} lost connections", report.lost_connections);
            strict_failed = true;
        }
        if errors > 0 {
            eprintln!("strict: {errors} error findings");
            strict_failed = true;
        }
        if strict_failed {
            code = 1;
        }
    }
    if let Some(fmt) = a.report {
        let text = report.render(fmt);
        match &a.report_out {
            Some(path) => match write_report(path, &text) {
                Ok(()) => say(format!("report written to {}", path.display())),
                Err(e) => {
                    eprintln!("error: {}: {e}", path.display());
                    return 2;
                }
            },
            None => print!("{text}"),
        }
    }
    if a.out.is_some() && strict_failed {
        eprintln!("strict: nothing was written");
    } else if let Some(out) = &a.out {
        match imported.session {
            Ok(session) => {
                match ew_api::import::create_project_from(
                    session,
                    out,
                    ew_api::StoreOptions::new(env!("CARGO_PKG_VERSION")),
                ) {
                    Ok(unstable) => {
                        say(format!("project written to {}", out.display()));
                        warn_unreleased(unstable, &mut std::io::stderr());
                    }
                    Err(e) => {
                        eprintln!("error: {e}");
                        return 2;
                    }
                }
            }
            Err(_) => {
                eprintln!("error: the imported records do not make a project; nothing was written");
                return 2;
            }
        }
    }
    if a.json {
        print!(
            "{}",
            results::to_text(&results::ImportQetResult {
                schema: results::IMPORT_QET.to_owned(),
                source: name.clone(),
                counts: results::QetCounts {
                    pages: c.pages,
                    devices: c.devices,
                    strips: c.strips,
                    terminals: c.terminals,
                    functions: c.functions,
                    representations: c.representations,
                    connections: c.connections,
                    junctions: c.junctions,
                },
                findings: report.total,
                lost_connections: report.lost_connections,
                lenient_findings: lenient,
                derived_pages,
                model_check_error,
                report_out: a
                    .report
                    .and(a.report_out.as_ref())
                    .map(|p| p.display().to_string()),
            })
        );
    }
    code
}

/// `ew import qet <file> --into <project> [--apply]` (WP-S3.09 D10 to D12): the file re-imported into its earlier
/// import in the project. Prints T6 when the import is matched by its items, T8 and the rows of the records added,
/// changed in the source, changed on both sides (each member with this project's and the file's value) and removed
/// in the source; without `--apply` nothing is written (T12), with it the re-import is one transaction, saved. Exit 2
/// when the file is refused or is not an earlier import of the project (T7).
pub(super) fn run_reimport_qet(
    a: &ImportQetArgs,
    project: &Path,
    name: &str,
    bytes: Vec<u8>,
    library: &StarterLibrary,
) -> i32 {
    use ew_api::import::{ReimportClass as C, member_text};
    let opened = if a.apply {
        let origin = ew_api::Origin::Cli {
            subcommand: "import qet".into(),
            confirmed: false,
        };
        open_for_writing(project, origin)
    } else {
        ew_api::Session::open_read_only(project, env!("CARGO_PKG_VERSION"))
            .map(|(s, _)| s)
            .map_err(|e| {
                eprintln!("error: {}: {e}", project.display());
                2
            })
    };
    let Ok(mut session) = opened else {
        return 2;
    };
    let detected = match session.detect_qet_lineage(name, bytes.clone(), library) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e}");
            session.close();
            return 2;
        }
    };
    if let Some(question) = detected.question(name) {
        println!("{question}");
    }
    let reimport = match session.reimport_qet(name, bytes, library, detected.lineage()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            session.close();
            return 2;
        }
    };
    println!("{}", reimport.summary());
    for row in &reimport.plan.rows {
        let class = match row.class {
            C::Added => "added",
            C::SourceChanged => "changed in the source",
            C::Conflict => "changed on both sides",
            C::Removed => "removed in the source",
            C::EwChanged | C::Unchanged => continue,
        };
        println!("{class}: {}", row.record);
        for m in &row.members {
            println!(
                "  {}: here {}, in {name} {}",
                m.path,
                member_text(m.ours.as_ref()),
                member_text(m.theirs.as_ref())
            );
        }
    }
    let writes = reimport.plan.writes();
    if !a.apply {
        if writes {
            println!("nothing was written: add --apply to commit the re-import");
        }
        session.close();
        return 0;
    }
    if let Err(e) = session.apply_reimport(reimport) {
        eprintln!("error: {e}");
        session.close();
        return 2;
    }
    if writes {
        if let Err(e) = session
            .save()
            .map(|r| warn_unreleased(r.unstable, &mut std::io::stderr()))
        {
            eprintln!("error: the save failed: {e:?}");
            session.close();
            return 2;
        }
        println!("re-import applied; saved");
    } else {
        println!("nothing to apply");
    }
    session.close();
    0
}

/// `ew import csv` (WP-S1.22 part 3, FT-27): the golden-dialect tables of a directory with the starter content as a new
/// project in `--out`. Every finding is printed; `--strict` (the default) writes nothing when the import made error
/// findings (exit 1), `--lenient` writes the project with them.
pub(super) fn run_import_csv(a: &ImportCsvArgs) -> i32 {
    use ew_api::import::{CreateError, ImportMode, create_project, import_tables, read_tables};
    let Some(content) = a.content.clone().or_else(content_dir) else {
        eprintln!(
            "error: `import csv` needs the starter content (`--content <dir>` or EW_CONTENT_DIR): the tables name its \
             symbols, function definitions and forms; nothing was written"
        );
        return 2;
    };
    let library = match StarterLibrary::from_dir(&content) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: starter content: {}: {e}", content.display());
            return 2;
        }
    };
    let inputs = match read_tables(&a.tables) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let mode = if a.lenient {
        ImportMode::Lenient
    } else {
        ImportMode::Strict
    };
    let batch = match import_tables(&inputs, &library, mode) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let rows = table_report_rows(&batch.report.entries);
    // With `--json` stdout is the result: the human lines go to stderr.
    let say = |line: String| {
        if a.json {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
    };
    for r in &rows {
        let severity = match r.severity {
            results::ReportSeverity::Info => "info",
            results::ReportSeverity::Warning => "warning",
            results::ReportSeverity::Error => "error",
        };
        let at = match (&r.file, r.line) {
            (Some(f), Some(l)) => format!("{}:{l}: ", printable(f)),
            (Some(f), None) => format!("{}: ", printable(f)),
            _ => String::new(),
        };
        say(format!("{} {severity}: {at}{}", r.code, printable(&r.message)));
    }
    let errors = rows
        .iter()
        .filter(|r| r.severity == results::ReportSeverity::Error)
        .count();
    let tables = inputs.len();
    let (records, documents) = (batch.records.len(), batch.lib.len());
    let (code, written) = if !a.lenient && errors > 0 {
        eprintln!(
            "error: the import made {} under `--strict` (the default); nothing was written",
            plural(errors, "error finding", "error findings")
        );
        (1, false)
    } else {
        match create_project(
            &batch,
            &a.out,
            ew_api::StoreOptions::new(env!("CARGO_PKG_VERSION")),
        ) {
            Ok(unstable) => {
                say(format!(
                    "imported {} into {}: {}, {}",
                    plural(tables, "table", "tables"),
                    a.out.display(),
                    plural(records, "record", "records"),
                    plural(documents, "library document", "library documents")
                ));
                warn_unreleased(unstable, &mut std::io::stderr());
                (0, true)
            }
            Err(e @ CreateError::Model(_)) => {
                eprintln!("error: {e}; nothing was written");
                (1, false)
            }
            Err(e) => {
                eprintln!("error: {e}");
                return 2;
            }
        }
    };
    if a.json {
        print!(
            "{}",
            results::to_text(&results::ImportCsvResult {
                schema: results::IMPORT_CSV.to_owned(),
                tables: inputs.iter().map(|i| i.name().to_owned()).collect(),
                mode: if a.lenient {
                    results::ImportCsvMode::Lenient
                } else {
                    results::ImportCsvMode::Strict
                },
                out: a.out.display().to_string(),
                written,
                records,
                documents,
                report: rows,
            })
        );
    }
    code
}

/// The findings of a table import as result rows, in the report's order.
pub(super) fn table_report_rows(entries: &[ew_api::import::ReportEntry]) -> Vec<results::TableReportRow> {
    entries
        .iter()
        .map(|e| results::TableReportRow {
            code: e.code.to_string(),
            severity: match e.severity {
                Severity::Info => results::ReportSeverity::Info,
                Severity::Warning => results::ReportSeverity::Warning,
                Severity::Error => results::ReportSeverity::Error,
            },
            file: e.location.as_ref().map(|l| l.file.clone()),
            line: e.location.as_ref().and_then(|l| l.line).map(u64::from),
            message: e.message.clone(),
        })
        .collect()
}
