// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew export` in its kinds, `ew export-ewz`, `ew extract`, `ew export pages` and `ew render`.

use super::*;

/// The message of a refused export (FT-465). A project without the report block of the export's kind has no row source:
/// the export is the rows of that block (doc 17 §3.3), so it is refused, not empty, and the message names the block, the
/// export and where the block comes from. A block that does not derive keeps the report layer's own text.
pub(super) fn export_refusal(what: Export, e: &ew_api::ExportError) -> String {
    match e {
        ew_api::ExportError::NoBlock(kind) => {
            let (export, made) = match what {
                Export::Cables => (
                    "the cable list",
                    "the company-standard wizard creates it as folio 906",
                ),
                Export::Bom(_) => (
                    "the bill of materials",
                    "the company-standard wizard creates it as folio 901",
                ),
            };
            format!(
                "the project has no report block of kind `{kind}`: {export} is the rows of that block ({made}; a project from the built-in template has none), so there is nothing to export"
            )
        }
        other => other.to_string(),
    }
}

pub(super) fn run_export(a: &ExportArgs) -> i32 {
    let Some(dir) = reader_dir(a.project.as_ref()) else {
        return 2;
    };
    let (session, _) = match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "export") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let options = match a.format {
        RowFormat::Csv(options) if options.preset == Preset::Ew => options,
        _ if a.kind == (ExportKind::Report { pdf: true }) => CsvOptions::default(),
        _ => return run_export_table(a, &session),
    };
    let rev = session.world().rev.0;
    let rows = match (&a.kind, &a.block) {
        // The default blocks of `cables` and `bom`: the report layer's export, as before `--block`.
        (ExportKind::Cables, None) | (ExportKind::Bom(_), None) => {
            let what = match a.kind {
                ExportKind::Bom(mode) => Export::Bom(mode),
                _ => Export::Cables,
            };
            match session.export(what) {
                Ok(e) => (e.csv, e.rows, Vec::new()),
                Err(e) => {
                    eprintln!("error: {}", export_refusal(what, &e));
                    return 2;
                }
            }
        }
        (ExportKind::Report { pdf: true }, Some(block)) => return run_export_report_pdf(a, &session, block),
        (kind, block) => {
            let what = match kind {
                ExportKind::Cables => RowExport::Cables,
                ExportKind::Bom(mode) => RowExport::Bom(*mode),
                ExportKind::Wires => RowExport::Wires,
                ExportKind::Terminals(strip) => RowExport::Terminals { strip: strip.clone() },
                ExportKind::Report { .. } => RowExport::Report,
                ExportKind::ReportKind(name) => RowExport::Kind(name.clone()),
            };
            match ew_api::output::row_export(&session.query(), &what, block.as_deref()) {
                Ok(r) => {
                    for notice in &r.notices {
                        eprintln!("{notice}");
                    }
                    if let ExportKind::Report { .. } | ExportKind::ReportKind(_) = kind {
                        return finish_export_report_csv(a, &r, &options, rev);
                    }
                    (r.csv, r.rows, r.strips)
                }
                Err(e) => {
                    eprintln!("error: {}", rows_refusal(&e));
                    return 2;
                }
            }
        }
    };
    let (csv, count, strips) = rows;
    let csv = restyle(&csv, &options);
    if write_rows(a, &csv, count) != 0 {
        return 2;
    }
    if a.json {
        let summary = RowsSummary {
            rows: count,
            strips,
            out: a.out.as_ref().map(|p| p.display().to_string()),
            csv: a.out.is_none().then_some(csv),
            format: "csv",
            preset: Some(options.preset.name()),
            rev,
        };
        print!("{}", rows_json(&a.kind, summary));
    }
    0
}

/// `csv` (the golden dialect) with the byte-order mark and line end of `options` (WP-S3.08 D6).
pub(super) fn restyle(csv: &str, options: &CsvOptions) -> String {
    ew_api::output::restyle(csv, options.writes_bom(), options.writes_line_end())
}

/// What the `--json` result of `export cables|bom|wires|terminals` says (WP-S3.08 TX-7).
pub(super) struct RowsSummary {
    rows: usize,
    strips: Vec<String>,
    out: Option<String>,
    csv: Option<String>,
    format: &'static str,
    preset: Option<&'static str>,
    rev: u64,
}

/// The `--json` result of a row export of `kind` other than `report`.
pub(super) fn rows_json(kind: &ExportKind, s: RowsSummary) -> String {
    match kind {
        ExportKind::Terminals(_) => results::to_text(&results::ExportTerminalsResult {
            schema: results::EXPORT_TERMINALS.to_owned(),
            rows: s.rows,
            strips: s.strips,
            out: s.out,
            csv: s.csv,
            format: Some(s.format.to_owned()),
            preset: s.preset.map(str::to_owned),
            rev: Some(s.rev),
        }),
        kind => results::to_text(&results::ExportRowsResult {
            schema: match kind {
                ExportKind::Cables => results::EXPORT_CABLES,
                ExportKind::Bom(_) => results::EXPORT_BOM,
                _ => results::EXPORT_WIRES,
            }
            .to_owned(),
            rows: s.rows,
            out: s.out,
            csv: s.csv,
            format: Some(s.format.to_owned()),
            preset: s.preset.map(str::to_owned),
            rev: Some(s.rev),
        }),
    }
}

/// A row export in XLSX or a CSV preset other than `ew` (WP-S3.08 D6, D7): [`ew_api::output::row_output`] over the
/// row choice of the golden path, the file written atomically (`<file>.tmp`, synced, renamed).
pub(super) fn run_export_table(a: &ExportArgs, session: &ew_api::Session) -> i32 {
    let what = match &a.kind {
        ExportKind::Cables => RowExport::Cables,
        ExportKind::Bom(mode) => RowExport::Bom(*mode),
        ExportKind::Wires => RowExport::Wires,
        ExportKind::Terminals(strip) => RowExport::Terminals { strip: strip.clone() },
        ExportKind::Report { .. } => RowExport::Report,
        ExportKind::ReportKind(name) => RowExport::Kind(name.clone()),
    };
    let (format, preset) = match a.format {
        RowFormat::Csv(options) => (RowsFormat::Csv(options), Some(options.preset.name())),
        RowFormat::Xlsx => {
            let date = match source_date() {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            };
            (RowsFormat::Xlsx(session.workbook_meta(date)), None)
        }
    };
    let file = match ew_api::output::row_output(&session.query(), &what, a.block.as_deref(), &format) {
        Ok(f) => f,
        Err(e) => {
            // `cables` and `bom` without `--block` refuse a project without the block as the golden path does.
            let text = match (&e, &a.kind, &a.block) {
                (ew_api::output::RowsError::NoBlock(kind), ExportKind::Cables, None) => {
                    export_refusal(Export::Cables, &ew_api::ExportError::NoBlock(kind))
                }
                (ew_api::output::RowsError::NoBlock(kind), ExportKind::Bom(mode), None) => {
                    export_refusal(Export::Bom(*mode), &ew_api::ExportError::NoBlock(kind))
                }
                (e, _, _) => rows_refusal(e),
            };
            eprintln!("error: {text}");
            return 2;
        }
    };
    for notice in &file.notices {
        eprintln!("{notice}");
    }
    let (bytes, text, format_name) = match file.output {
        RowsOutput::Csv(csv) => (csv.clone().into_bytes(), Some(csv), "csv"),
        RowsOutput::Xlsx(bytes) => (bytes, None, "xlsx"),
    };
    match &a.out {
        Some(path) => {
            if let Err(e) = write_atomically(path, &bytes) {
                eprintln!("error: {}: {e}", path.display());
                return 2;
            }
            if !a.json {
                println!("export written to {}", path.display());
            }
        }
        // `--format xlsx` needs `--out` (the lowering refuses it without); a CSV goes to stdout.
        None => match &text {
            Some(csv) if !a.json => {
                print!("{csv}");
                eprintln!("{} rows", file.rows);
            }
            Some(_) => {}
            None => {
                eprintln!("error: {XLSX_NEEDS_OUT}");
                return 2;
            }
        },
    }
    if a.json {
        let out = a.out.as_ref().map(|p| p.display().to_string());
        let csv = if a.out.is_none() { text } else { None };
        let rev = session.world().rev.0;
        let json = match &a.kind {
            ExportKind::Report { .. } | ExportKind::ReportKind(_) => {
                results::to_text(&results::ExportReportResult {
                    schema: results::EXPORT_REPORT.to_owned(),
                    block: file.blocks.first().cloned().unwrap_or_default(),
                    kind: file.kind.clone(),
                    format: format_name.to_owned(),
                    rows: Some(file.rows),
                    folios: None,
                    out,
                    csv,
                    preset: preset.map(str::to_owned),
                    rev: Some(rev),
                })
            }
            kind => rows_json(
                kind,
                RowsSummary {
                    rows: file.rows,
                    strips: file.strips,
                    out,
                    csv,
                    format: format_name,
                    preset,
                    rev,
                },
            ),
        };
        print!("{json}");
    }
    0
}

/// `ew export table` (WP-S3.08 D7, D8): a navigator's table unfiltered, as CSV in a preset or as a workbook with the
/// round-trip sheets (`_ew`, and `_ew_base` with its digest), the file written atomically.
pub(super) fn run_export_nav_table(a: &ExportTableArgs) -> i32 {
    let Some(dir) = reader_dir(a.project.as_ref()) else {
        return 2;
    };
    let (session, _) = match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "export") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let table = match session
        .navigator_table_key(&a.navigator, a.strip.as_deref())
        .and_then(|key| session.table_export(&key))
    {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let (bytes, text, format, preset) = match a.format {
        RowFormat::Csv(options) => {
            let exporter = ew_api::output::CsvExporter { options };
            let csv = match ew_api::output::export_tables(
                &exporter,
                session.world().rev.0,
                std::slice::from_ref(&table),
            ) {
                Ok((bytes, _)) => String::from_utf8(bytes).unwrap_or_default(),
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            };
            (
                csv.clone().into_bytes(),
                Some(csv),
                "csv",
                Some(options.preset.name()),
            )
        }
        RowFormat::Xlsx => {
            let date = match source_date() {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            };
            let tables = std::slice::from_ref(&table);
            let meta = ew_api::output::with_base_digest(&session.workbook_meta(date), tables);
            let rev = meta.rev;
            match ew_api::output::export_tables(&ew_api::output::XlsxExporter { meta }, rev, tables) {
                Ok((b, _)) => (b, None, "xlsx", None),
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            }
        }
    };
    match (&a.out, &text) {
        (Some(path), _) => {
            if let Err(e) = write_atomically(path, &bytes) {
                eprintln!("error: {}: {e}", path.display());
                return 2;
            }
            if !a.json {
                println!("export written to {}", path.display());
            }
        }
        (None, Some(csv)) if !a.json => {
            print!("{csv}");
            eprintln!("{} rows", table.rows.len());
        }
        (None, Some(_)) => {}
        // The lowering refuses `--format xlsx` without `--out`.
        (None, None) => {
            eprintln!("error: {XLSX_NEEDS_OUT}");
            return 2;
        }
    }
    if a.json {
        print!(
            "{}",
            results::to_text(&results::ExportTableResult {
                schema: results::EXPORT_TABLE.to_owned(),
                table: table.key.clone(),
                format: format.to_owned(),
                preset: preset.map(str::to_owned),
                rows: table.rows.len(),
                editable: table
                    .columns
                    .iter()
                    .filter(|c| c.editable)
                    .map(|c| c.key.clone())
                    .collect(),
                out: a.out.as_ref().map(|p| p.display().to_string()),
                csv: if a.out.is_none() { text } else { None },
                rev: session.world().rev.0,
            })
        );
    }
    0
}

/// Writes the CSV of a row export to `--out` or stdout (nothing on stdout with `--json`): 0, or 2 on a write error.
pub(super) fn write_rows(a: &ExportArgs, csv: &str, rows: usize) -> i32 {
    match &a.out {
        Some(path) => match write_report(path, csv) {
            Ok(()) if a.json => {}
            Ok(()) => println!("export written to {}", path.display()),
            Err(e) => {
                eprintln!("error: {}: {e}", path.display());
                return 2;
            }
        },
        None if a.json => {}
        None => {
            print!("{csv}");
            eprintln!("{rows} rows");
        }
    }
    0
}

/// The message of a refused row export of a chosen block (WP-S1.22 D8): as [`export_refusal`] for a project without
/// the block of the kind, else the report layer's own text.
pub(super) fn rows_refusal(e: &ew_api::output::RowsError) -> String {
    match e {
        ew_api::output::RowsError::NoBlock(kind) => {
            let export = match *kind {
                "wire_list" => "the wire list",
                "terminal_diagram" => "the terminal diagram",
                "cable_overview_diagrams" => "the cable list",
                "parts_summary" => "the bill of materials",
                _ => "the export",
            };
            format!(
                "the project has no report block of kind `{kind}`: {export} is the rows of that block (a project from the built-in template has none), so there is nothing to export"
            )
        }
        // WP-S3.06 D4: each refusal of the export by kind is its own whole text.
        e @ (ew_api::output::RowsError::KindAndBlock
        | ew_api::output::RowsError::KindPages
        | ew_api::output::RowsError::UnknownKind(..)
        | ew_api::output::RowsError::KindOneStrip) => e.to_string(),
        other => other.to_string(),
    }
}

/// `ew export report --format csv`: the rows of the block (or of the kind, `--kind`), then the `--json` result; its
/// `block` is empty for `--kind` (the orchestrator's ruling 2 of cycle 42).
pub(super) fn finish_export_report_csv(
    a: &ExportArgs,
    r: &ew_api::output::RowsCsv,
    options: &CsvOptions,
    rev: u64,
) -> i32 {
    let csv = restyle(&r.csv, options);
    if write_rows(a, &csv, r.rows) != 0 {
        return 2;
    }
    if a.json {
        print!(
            "{}",
            results::to_text(&results::ExportReportResult {
                schema: results::EXPORT_REPORT.to_owned(),
                block: r.blocks.first().cloned().unwrap_or_default(),
                kind: r.kind.clone(),
                format: "csv".to_owned(),
                rows: Some(r.rows),
                folios: None,
                out: a.out.as_ref().map(|p| p.display().to_string()),
                csv: a.out.is_none().then(|| csv.clone()),
                preset: Some(options.preset.name().to_owned()),
                rev: Some(rev),
            })
        );
    }
    0
}

/// `ew export report --format pdf`: the page export restricted to the block's folios (WP-S1.22 D8).
pub(super) fn run_export_report_pdf(a: &ExportArgs, session: &ew_api::Session, block: &str) -> i32 {
    let Some(out) = &a.out else {
        eprintln!("error: `export report --format pdf` needs `--out <file.pdf>`");
        return 2;
    };
    let q = session.query();
    let found = ew_api::output::find_block(&q, block)
        .and_then(|(id, b)| ew_api::output::report_folios(&q, block).map(|f| (id, b, f)));
    let (id, b, folios) = match found {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let date = match source_date() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let options = ExportOptions {
        pages: PageSelection::Folios(folios.clone()),
        parallelism: Parallelism::threads(
            std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get),
        ),
        monochrome: false,
        date,
    };
    let pdf = match session.export_pdf(&options) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    if let Err(e) = write_atomically(out, &pdf.bytes) {
        eprintln!("error: {}: {e}", out.display());
        return 2;
    }
    if a.json {
        print!(
            "{}",
            results::to_text(&results::ExportReportResult {
                schema: results::EXPORT_REPORT.to_owned(),
                block: id.to_string(),
                kind: ew_api::output::block_kind(&b).to_owned(),
                format: "pdf".to_owned(),
                rows: None,
                folios: Some(folios),
                out: Some(out.display().to_string()),
                csv: None,
                preset: None,
                rev: Some(session.world().rev.0),
            })
        );
    } else {
        println!(
            "PDF written to {}: {} pages ({})",
            out.display(),
            pdf.pages,
            folios.join(", ")
        );
    }
    if !pdf.findings.is_empty() {
        eprintln!(
            "{} page findings (unresolved wire ends, unknown styles, overflowing table texts)",
            pdf.findings.len()
        );
    }
    0
}

// The program boundary: the one environment input of the exports, and there is no wall-clock fallback (no
// `SOURCE_DATE_EPOCH` means no creation date), so the same input and variable give the same bytes (doc 18 §3.5).
#[allow(clippy::disallowed_methods)]
pub(super) fn source_date_epoch() -> Option<std::ffi::OsString> {
    std::env::var_os("SOURCE_DATE_EPOCH").filter(|v| !v.is_empty())
}

// The program boundary: `$APPIMAGE` and `$APPDIR`, the AppImage this `ew` may run from (ID-1187's Linux package) and its
// mount, read once and passed in, so that `ew git setup` makes git run the AppImage and never its transient mount
// (WP-S1.17 D28, C49-p9#5, C49-p10#2).
#[allow(clippy::disallowed_methods)]
pub(super) fn appimage() -> [Option<std::path::PathBuf>; 2] {
    ["APPIMAGE", "APPDIR"].map(|name| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(std::path::PathBuf::from)
    })
}

/// The creation date of `SOURCE_DATE_EPOCH` (doc 17 section 3.1, FR-OUT-026); `Ok(None)` when it is not set.
pub(super) fn source_date() -> Result<Option<ew_api::output::PdfDate>, String> {
    let Some(v) = source_date_epoch() else {
        return Ok(None);
    };
    let text = v.to_string_lossy();
    text.trim()
        .parse::<i64>()
        .ok()
        .and_then(date_from_epoch)
        .map(Some)
        .ok_or_else(|| format!("SOURCE_DATE_EPOCH is not a date in seconds since 1970: `{text}`"))
}

/// `ew export ewz` (WP-S1.17 D8, ID-1175 ruling 5): the project's saved generation, read back from the disk and
/// verified, packed into `--out` ([`write_atomically`]). The project opens as a reader (no lock); an `.ewz` given as
/// the project is refused ([`project_dir`]).
pub(super) fn run_export_ewz(a: &ExportEwzArgs) -> i32 {
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    let (session, _) = match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "export ewz") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let packed = match session.export_ewz() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    if let Err(e) = write_atomically(&a.out, &packed.bytes) {
        eprintln!("error: {}: {e}", a.out.display());
        return 2;
    }
    if a.json {
        print!(
            "{}",
            results::to_text(&results::ExportEwzResult {
                schema: results::EXPORT_EWZ.to_owned(),
                out: a.out.display().to_string(),
                generation: packed.generation,
                files: packed.files,
                bytes: packed.bytes.len() as u64,
            })
        );
    } else {
        println!(
            ".ewz written to {}: generation {}, {}",
            a.out.display(),
            packed.generation,
            plural(packed.files, "file", "files")
        );
    }
    0
}

/// `ew extract <file.ewz> [--into <dir>]` (WP-S1.17 D9, ID-1175 ruling 4): the archive read whole and checked before
/// anything is written, then extracted into a new directory, all or nothing (`ew_api::ewz::extract`); without `--into`
/// the directory is `<stem>.ew` beside the archive.
pub(super) fn run_extract(a: &ExtractArgs) -> i32 {
    let vfs = ew_api::OsVfs;
    let archive = match ew_api::ewz::read_file(&vfs, &a.file) {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "error: {}: the .ewz is refused: {e}; nothing was written",
                a.file.display()
            );
            return 2;
        }
    };
    let Some(into) = a.into.clone().or_else(|| ew_api::ewz::default_target(&a.file)) else {
        eprintln!("error: {}: not an .ewz file name", a.file.display());
        return 2;
    };
    if let Err(e) = ew_api::ewz::extract(&vfs, &archive, &into) {
        eprintln!("error: {e}");
        return 2;
    }
    let files = archive.entries.len();
    if a.json {
        print!(
            "{}",
            results::to_text(&results::ExtractResult {
                schema: results::EXTRACT.to_owned(),
                into: into.display().to_string(),
                generation: archive.generation(),
                files,
            })
        );
    } else {
        println!(
            "extracted into {}: generation {}, {}",
            into.display(),
            archive.generation(),
            plural(files, "file", "files")
        );
    }
    0
}

/// Writes an SVG set into the directory `out` as a whole ([`write_file_set`]).
pub(super) fn write_svg_set(out: &Path, files: &[ew_api::output::SvgFile]) -> Result<(), String> {
    let files: Vec<(&str, &[u8])> = files
        .iter()
        .map(|f| (f.name.as_str(), f.text.as_bytes()))
        .collect();
    write_file_set(out, "svg", "an SVG", &files)
}

/// Writes a set of files with the extension `ext` (`svg`, `png`; `a_file` names one: `an SVG`) into the directory
/// `out` as a whole: the files go to a fresh sibling directory first, which then replaces `out`, so `out` holds exactly
/// this export (no stale page of an earlier export) and a failure leaves the earlier set as it was. A directory that
/// holds anything but `.<ext>` files is refused, so no other file is ever removed (`ew export svg`, `ew export png`).
pub(super) fn write_file_set(
    out: &Path,
    ext: &str,
    a_file: &str,
    files: &[(&str, &[u8])],
) -> Result<(), String> {
    let label = a_file.rsplit(' ').next().unwrap_or(a_file);
    let at = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
    if out.exists() {
        let entries = std::fs::read_dir(out).map_err(|e| at(out, e))?;
        for e in entries {
            let path = e.map_err(|e| at(out, e))?.path();
            if !path.is_file() || path.extension().is_none_or(|x| x != ext) {
                return Err(format!(
                    "{}: the directory holds {}, which is not {a_file} file; choose an empty or {label}-only directory",
                    out.display(),
                    path.file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
                ));
            }
        }
    }
    let tmp = sibling(out, &format!(".tmp-{}", std::process::id()));
    let old = sibling(out, &format!(".old-{}", std::process::id()));
    let written = std::fs::create_dir_all(&tmp)
        .map_err(|e| at(&tmp, e))
        .and_then(|()| {
            files.iter().try_for_each(|(name, bytes)| {
                let path = tmp.join(name);
                write_synced(&path, bytes).map_err(|e| at(&path, e))
            })
        });
    if let Err(e) = written {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    let had = out.exists();
    if had && let Err(e) = std::fs::rename(out, &old) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(at(out, e));
    }
    if let Err(e) = std::fs::rename(&tmp, out) {
        if had {
            let _ = std::fs::rename(&old, out);
        }
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(at(out, e));
    }
    if had {
        let _ = std::fs::remove_dir_all(&old);
    }
    Ok(())
}

pub(super) fn run_export_pages(a: &ExportPagesArgs) -> i32 {
    let Some(dir) = reader_dir(a.project.as_ref()) else {
        return 2;
    };
    let date = match source_date() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let (session, _) = match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "export") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let threads = a
        .threads
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get));
    let options = ExportOptions {
        pages: a.pages.clone(),
        parallelism: Parallelism::threads(threads),
        monochrome: false,
        date,
    };
    let (written, findings) = match a.format {
        PageFormat::Pdf => {
            let pdf = match session.export_pdf(&options) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            };
            if let Err(e) = write_atomically(&a.out, &pdf.bytes) {
                eprintln!("error: {}: {e}", a.out.display());
                return 2;
            }
            if a.json {
                print!(
                    "{}",
                    results::to_text(&results::ExportPdfResult {
                        schema: results::EXPORT_PDF.to_owned(),
                        out: a.out.display().to_string(),
                        pages: pdf.pages,
                        links: pdf.report.links,
                        findings: pdf.findings.len(),
                    })
                );
            }
            (
                format!(
                    "PDF written to {}: {} pages, {} links",
                    a.out.display(),
                    pdf.pages,
                    pdf.report.links
                ),
                pdf.findings.len(),
            )
        }
        PageFormat::Svg => {
            let svg = match session.export_svg(&options) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            };
            if let Err(e) = write_svg_set(&a.out, &svg.files) {
                eprintln!("error: {e}");
                return 2;
            }
            if a.json {
                print!(
                    "{}",
                    results::to_text(&results::ExportSvgResult {
                        schema: results::EXPORT_SVG.to_owned(),
                        out: a.out.display().to_string(),
                        files: svg.files.iter().map(|f| f.name.clone()).collect(),
                        findings: svg.findings.len(),
                    })
                );
            }
            (
                format!("SVG written to {}: {} files", a.out.display(), svg.files.len()),
                svg.findings.len(),
            )
        }
        PageFormat::Png => {
            let png = match session.export_png(&options, a.dpi) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            };
            let files: Vec<(&str, &[u8])> = png
                .files
                .iter()
                .map(|f| (f.name.as_str(), f.bytes.as_slice()))
                .collect();
            if let Err(e) = write_file_set(&a.out, "png", "a PNG", &files) {
                eprintln!("error: {e}");
                return 2;
            }
            if a.json {
                print!(
                    "{}",
                    results::to_text(&results::ExportPngResult {
                        schema: results::EXPORT_PNG.to_owned(),
                        out: a.out.display().to_string(),
                        dpi: a.dpi,
                        files: png.files.iter().map(|f| f.name.clone()).collect(),
                        findings: png.findings.len(),
                    })
                );
            }
            (
                format!(
                    "PNG written to {}: {} files at {} dpi",
                    a.out.display(),
                    png.files.len(),
                    a.dpi
                ),
                png.findings.len(),
            )
        }
    };
    if !a.json {
        println!("{written}");
    }
    if findings > 0 {
        eprintln!("{findings} page findings (unresolved wire ends, unknown styles, overflowing table texts)");
    }
    0
}

/// `ew render --page <folio> --png` (WP-S1.22 D7, FR-EXT-004): one page as PNG, to `--out` or `<folio>.png` in the
/// current directory, never to stdout.
pub(super) fn run_render(a: &RenderArgs) -> i32 {
    let Some(dir) = reader_dir(a.project.as_ref()) else {
        return 2;
    };
    let (session, _) = match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "render") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let png = match session.render_png(&a.page, a.dpi) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let Some(file) = png.files.first() else {
        eprintln!("error: the page order has no folio `{}`", a.page);
        return 2;
    };
    let out = a.out.clone().unwrap_or_else(|| PathBuf::from(&file.name));
    if let Err(e) = write_atomically(&out, &file.bytes) {
        eprintln!("error: {}: {e}", out.display());
        return 2;
    }
    if a.json {
        print!(
            "{}",
            results::to_text(&results::RenderResult {
                schema: results::RENDER.to_owned(),
                page: file.folio.clone(),
                out: out.display().to_string(),
                dpi: a.dpi,
                width: file.width,
                height: file.height,
                findings: png.findings.len(),
            })
        );
    } else {
        println!(
            "PNG written to {}: folio {}, {} x {} pixels at {} dpi",
            out.display(),
            file.folio,
            file.width,
            file.height,
            a.dpi
        );
    }
    if !png.findings.is_empty() {
        eprintln!(
            "{} page findings (unresolved wire ends, unknown styles, overflowing table texts)",
            png.findings.len()
        );
    }
    0
}
