// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew export` in its kinds, `ew export-ewz`, `ew extract`, `ew export pages` and `ew render`.

use clap::Args;

use super::*;

impl Leaf for CablesCli {
    const EXITS: &'static [Exit] = &[OK, ROW_EXPORT_ERRORS];
    const RESULT: Option<&'static str> = Some(results::ExportRowsResult::CABLES);

    fn check(&self) -> Result<(), clap::Error> {
        self.rows.export("cables", ExportKind::Cables).map(drop)
    }
}

impl Leaf for BomCli {
    const EXITS: &'static [Exit] = &[OK, ROW_EXPORT_ERRORS];
    const RESULT: Option<&'static str> = Some(results::ExportRowsResult::BOM);

    fn check(&self) -> Result<(), clap::Error> {
        self.export().map(drop)
    }
}

impl Leaf for WiresCli {
    const EXITS: &'static [Exit] = &[OK, ROW_EXPORT_ERRORS];
    const RESULT: Option<&'static str> = Some(results::ExportRowsResult::WIRES);

    fn check(&self) -> Result<(), clap::Error> {
        self.rows.export("wires", ExportKind::Wires).map(drop)
    }
}

impl Leaf for TerminalsCli {
    const EXITS: &'static [Exit] = &[
        OK,
        exit(
            2,
            "usage and open errors, a project without the block, an unknown or wrong block, an unknown strip",
        ),
    ];
    const RESULT: Option<&'static str> = Some(results::ExportTerminalsResult::SCHEMA);

    fn check(&self) -> Result<(), clap::Error> {
        self.export().map(drop)
    }
}

impl Leaf for ReportCli {
    const EXITS: &'static [Exit] = &[
        OK,
        exit(
            2,
            "usage, open, I/O and writer errors, an unknown block, a block without rows or pages",
        ),
    ];
    const RESULT: Option<&'static str> = Some(results::ExportReportResult::SCHEMA);

    fn check(&self) -> Result<(), clap::Error> {
        self.export().map(drop)
    }
}

/// `ew export cables|bom|wires|terminals|report` of its line ([`RowsCli::export`] and its kin).
/// What a row or table export says once its file is written.
pub(super) fn export_written(path: &Path) -> String {
    format!("export written to {}", path.display())
}

pub(super) fn run_export(a: &ExportArgs, io: &mut Io<'_>) -> i32 {
    let Some(dir) = reader_dir(a.project.as_ref(), io) else {
        return 2;
    };
    let (session, _) = match open_reader(&dir, "export", io) {
        Ok(r) => r,
        Err(e) => {
            io.error(format_args!("{}: {e}", dir.display()));
            return 2;
        }
    };
    let options = match a.format {
        RowFormat::Csv(options) if options.preset == Preset::Ew => options,
        _ if a.kind == (ExportKind::Report { pdf: true }) => CsvOptions::default(),
        _ => return run_export_table(a, &session, io),
    };
    let rev = session.world().rev.0;
    let rows = match (&a.kind, &a.block) {
        (ExportKind::Report { pdf: true }, Some(block)) => {
            return run_export_report_pdf(a, &session, block, io);
        }
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
                        errln!(io, "{notice}");
                    }
                    if let ExportKind::Report { .. } | ExportKind::ReportKind(_) = kind {
                        return finish_export_report_csv(a, &r, &options, rev, io);
                    }
                    (r.csv, r.rows, r.strips)
                }
                Err(e) => {
                    io.error(rows_refusal(&e));
                    return 2;
                }
            }
        }
    };
    let (csv, count, strips) = rows;
    let csv = restyle(&csv, &options);
    if write_rows(a, &csv, count, io) != 0 {
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
        out!(io, "{}", rows_json(&a.kind, summary));
    }
    0
}

/// `csv` (the `ew` dialect) with the byte-order mark and line end of `options`.
pub(super) fn restyle(csv: &str, options: &CsvOptions) -> String {
    ew_api::output::restyle(csv, options.writes_bom(), options.writes_line_end())
}

/// What the `--json` result of `export cables|bom|wires|terminals` says.
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
            schema: results::ExportTerminalsResult::SCHEMA.to_owned(),
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
                ExportKind::Cables => results::ExportRowsResult::CABLES,
                ExportKind::Bom(_) => results::ExportRowsResult::BOM,
                _ => results::ExportRowsResult::WIRES,
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

/// A row export in XLSX or a CSV preset other than `ew`: [`ew_api::output::row_output`] over the
/// row choice of the `ew` path, the file written atomically (`<file>.tmp`, synced, renamed).
pub(super) fn run_export_table(a: &ExportArgs, session: &ew_api::Session, io: &mut Io<'_>) -> i32 {
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
            let date = match source_date(io.env) {
                Ok(d) => d,
                Err(e) => {
                    io.error(e);
                    return 2;
                }
            };
            (RowsFormat::Xlsx(session.workbook_meta(date)), None)
        }
    };
    let file = match ew_api::output::row_output(&session.query(), &what, a.block.as_deref(), &format) {
        Ok(f) => f,
        Err(e) => {
            io.error(rows_refusal(&e));
            return 2;
        }
    };
    for notice in &file.notices {
        errln!(io, "{notice}");
    }
    let (bytes, text, format_name) = match file.output {
        RowsOutput::Csv(csv) => (csv.clone().into_bytes(), Some(csv), "csv"),
        RowsOutput::Xlsx(bytes) => (bytes, None, "xlsx"),
    };
    match &a.out {
        Some(path) => {
            if let Err(e) = write_atomically(path, &bytes) {
                io.error(format_args!("{}: {e}", path.display()));
                return 2;
            }
            if !a.json {
                outln!(io, "{}", export_written(path));
            }
        }
        // `--format xlsx` needs `--out` (its line is refused without); a CSV goes to stdout.
        None => match &text {
            Some(csv) if !a.json => {
                out!(io, "{csv}");
                errln!(io, "{} rows", file.rows);
            }
            Some(_) => {}
            None => {
                io.error(XLSX_NEEDS_OUT);
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
                    schema: results::ExportReportResult::SCHEMA.to_owned(),
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
        out!(io, "{json}");
    }
    0
}

impl Leaf for TableCli {
    const EXITS: &'static [Exit] = &[
        OK,
        exit(
            2,
            "usage, open, I/O and writer errors, an unknown navigator or strip",
        ),
    ];
    const RESULT: Option<&'static str> = Some(results::ExportTableResult::SCHEMA);

    fn check(&self) -> Result<(), clap::Error> {
        self.row_format().map(drop)
    }
}

/// `ew export table`: a navigator's table unfiltered, as CSV in a preset or as a workbook with the
/// round-trip sheets (`_ew`, and `_ew_base` with its digest), the file written atomically.
pub(super) fn run_export_nav_table(a: &TableCli, io: &mut Io<'_>) -> i32 {
    let row_format = match a.row_format() {
        Ok(f) => f,
        Err(e) => return usage(&e, io),
    };
    let Some(dir) = reader_dir(a.project.as_ref(), io) else {
        return 2;
    };
    let (session, _) = match open_reader(&dir, "export", io) {
        Ok(r) => r,
        Err(e) => {
            io.error(format_args!("{}: {e}", dir.display()));
            return 2;
        }
    };
    let table = match session
        .navigator_table_key(&a.navigator, a.strip.as_deref())
        .and_then(|key| session.table_export(&key))
    {
        Ok(t) => t,
        Err(e) => {
            io.error(e);
            return 2;
        }
    };
    let (bytes, text, format, preset) = match row_format {
        RowFormat::Csv(options) => {
            let csv = ew_api::output::table_csv(&table, &options);
            (
                csv.clone().into_bytes(),
                Some(csv),
                "csv",
                Some(options.preset.name()),
            )
        }
        RowFormat::Xlsx => {
            let date = match source_date(io.env) {
                Ok(d) => d,
                Err(e) => {
                    io.error(e);
                    return 2;
                }
            };
            let tables = std::slice::from_ref(&table);
            let meta = ew_api::output::with_base_digest(&session.workbook_meta(date), tables);
            match ew_api::output::xlsx(tables, &meta) {
                Ok(b) => (b, None, "xlsx", None),
                Err(e) => {
                    io.error(e);
                    return 2;
                }
            }
        }
    };
    match (&a.out, &text) {
        (Some(path), _) => {
            if let Err(e) = write_atomically(path, &bytes) {
                io.error(format_args!("{}: {e}", path.display()));
                return 2;
            }
            if !a.json {
                outln!(io, "{}", export_written(path));
            }
        }
        (None, Some(csv)) if !a.json => {
            out!(io, "{csv}");
            errln!(io, "{} rows", table.rows.len());
        }
        (None, Some(_)) => {}
        // A line with `--format xlsx` and no `--out` is refused before this runs.
        (None, None) => {
            io.error(XLSX_NEEDS_OUT);
            return 2;
        }
    }
    if a.json {
        out!(
            io,
            "{}",
            results::to_text(&results::ExportTableResult {
                schema: results::ExportTableResult::SCHEMA.to_owned(),
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
pub(super) fn write_rows(a: &ExportArgs, csv: &str, rows: usize, io: &mut Io<'_>) -> i32 {
    match &a.out {
        Some(path) => match write_report(path, csv) {
            Ok(()) if a.json => {}
            Ok(()) => outln!(io, "{}", export_written(path)),
            Err(e) => {
                io.error(format_args!("{}: {e}", path.display()));
                return 2;
            }
        },
        None if a.json => {}
        None => {
            out!(io, "{csv}");
            errln!(io, "{rows} rows");
        }
    }
    0
}

/// The message of a refused row export. A project without the report block of the export's kind has no block rows:
/// the export is the rows of that block (doc 17 §3.3), so it is refused, not empty, and the message names the block,
/// the export and, for a kind with rows of the whole project, the command that writes those (`ew export report
/// --kind`). Each other refusal is the report layer's own text.
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
            let instead = match *kind {
                // The terminal diagram lists one strip: no rows of the whole project.
                "terminal_diagram" => "so there is nothing to export".to_owned(),
                _ => {
                    format!("`ew export report --kind {kind}` exports the rows of the whole project instead")
                }
            };
            format!(
                "the project has no report block of kind `{kind}`: {export} is the rows of that block (a project from the built-in template has none); {instead}"
            )
        }
        // Each other refusal of the export is its own whole text.
        other => other.to_string(),
    }
}

/// `ew export report --format csv`: the rows of the block (or of the kind, `--kind`), then the `--json` result; its
/// `block` is empty for `--kind`.
pub(super) fn finish_export_report_csv(
    a: &ExportArgs,
    r: &ew_api::output::RowsCsv,
    options: &CsvOptions,
    rev: u64,
    io: &mut Io<'_>,
) -> i32 {
    let csv = restyle(&r.csv, options);
    if write_rows(a, &csv, r.rows, io) != 0 {
        return 2;
    }
    if a.json {
        out!(
            io,
            "{}",
            results::to_text(&results::ExportReportResult {
                schema: results::ExportReportResult::SCHEMA.to_owned(),
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

/// `ew export report --format pdf`: the page export restricted to the block's folios.
pub(super) fn run_export_report_pdf(
    a: &ExportArgs,
    session: &ew_api::Session,
    block: &str,
    io: &mut Io<'_>,
) -> i32 {
    let Some(out) = &a.out else {
        io.error("`export report --format pdf` needs `--out <file.pdf>`");
        return 2;
    };
    let q = session.query();
    let found = ew_api::output::find_block(&q, block)
        .and_then(|(id, b)| ew_api::output::report_folios(&q, block).map(|f| (id, b, f)));
    let (id, b, folios) = match found {
        Ok(f) => f,
        Err(e) => {
            io.error(e);
            return 2;
        }
    };
    let date = match source_date(io.env) {
        Ok(d) => d,
        Err(e) => {
            io.error(e);
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
        underlays: false,
        screen: false,
    };
    let pdf = match session.export_pdf(&options) {
        Ok(p) => p,
        Err(e) => {
            io.error(e);
            return 2;
        }
    };
    if let Err(e) = write_atomically(out, &pdf.bytes) {
        io.error(format_args!("{}: {e}", out.display()));
        return 2;
    }
    if a.json {
        out!(
            io,
            "{}",
            results::to_text(&results::ExportReportResult {
                schema: results::ExportReportResult::SCHEMA.to_owned(),
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
        outln!(
            io,
            "PDF written to {}: {} pages ({})",
            out.display(),
            pdf.pages,
            folios.join(", ")
        );
    }
    if let Some(findings) = ew_api::output::page_findings_summary(&pdf.findings) {
        errln!(io, "{findings}");
    }
    0
}

// The one environment input of the exports, and there is no wall-clock fallback (no `SOURCE_DATE_EPOCH` means no
// creation date), so the same input and variable give the same bytes (doc 18 §3.5).
pub(super) fn source_date_epoch(env: &Env) -> Option<std::ffi::OsString> {
    env.var("SOURCE_DATE_EPOCH")
}

// `$APPIMAGE` and `$APPDIR`, the AppImage this `ew` may run from (the Linux package) and its mount, read once and
// passed in, so that `ew git setup` makes git run the AppImage and never its transient mount.
pub(super) fn appimage(env: &Env) -> [Option<std::path::PathBuf>; 2] {
    ["APPIMAGE", "APPDIR"].map(|name| env.var(name).map(std::path::PathBuf::from))
}

/// The creation date of `SOURCE_DATE_EPOCH` (doc 17 section 3.1, FR-OUT-026); `Ok(None)` when it is not set.
pub(super) fn source_date(env: &Env) -> Result<Option<ew_api::output::PdfDate>, String> {
    let Some(v) = source_date_epoch(env) else {
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

/// The day of `SOURCE_DATE_EPOCH` as an editing session's date (`SessionOptions::today`); `None` when it is not set
/// or not a date.
pub(super) fn session_today(env: &Env) -> Option<ew_api::catalog::Date> {
    let d = source_date(env).ok().flatten()?;
    ew_api::catalog::Date::new(d.year, d.month, d.day).ok()
}

impl Leaf for EwzCli {
    const EXITS: &'static [Exit] = &[
        exit(0, "written"),
        exit(
            2,
            "usage, open and write errors, a project changed on disk or one that cannot be packed",
        ),
    ];
    const RESULT: Option<&'static str> = Some(results::ExportEwzResult::SCHEMA);

    fn check(&self) -> Result<(), clap::Error> {
        self.file().map(drop)
    }
}

/// `ew export ewz`: the project's saved generation, read back from the disk and
/// verified, packed into `--out` ([`write_atomically`]). The project opens as a reader (no lock); an `.ewz` given as
/// the project is refused ([`project_dir`]).
pub(super) fn run_export_ewz(a: &EwzCli, io: &mut Io<'_>) -> i32 {
    let out = match a.file() {
        Ok(out) => out,
        Err(e) => return usage(&e, io),
    };
    let Some(dir) = project_dir(a.project.as_ref(), io) else {
        return 2;
    };
    let (session, _) = match open_reader(&dir, "export ewz", io) {
        Ok(r) => r,
        Err(e) => {
            io.error(format_args!("{}: {e}", dir.display()));
            return 2;
        }
    };
    let packed = match session.export_ewz() {
        Ok(p) => p,
        Err(e) => {
            io.error(format_args!("{}: {e}", dir.display()));
            return 2;
        }
    };
    if let Err(e) = write_atomically(out, &packed.bytes) {
        io.error(format_args!("{}: {e}", out.display()));
        return 2;
    }
    if a.json {
        out!(
            io,
            "{}",
            results::to_text(&results::ExportEwzResult {
                schema: results::ExportEwzResult::SCHEMA.to_owned(),
                out: out.display().to_string(),
                generation: packed.generation,
                files: packed.files,
                bytes: packed.bytes.len() as u64,
            })
        );
    } else {
        outln!(
            io,
            ".ewz written to {}: generation {}, {}",
            out.display(),
            packed.generation,
            plural(packed.files, "file", "files")
        );
    }
    0
}

impl Leaf for ReworkCli {
    const EXITS: &'static [Exit] = &[
        exit(0, "written"),
        exit(
            2,
            "usage, open errors, no release or no archive found, an archive that does not open, write errors",
        ),
    ];

    fn check(&self) -> Result<(), clap::Error> {
        self.file().map(drop)
    }
}

/// The release labelled `since`, else the last one whose archive is found beside the project, as a rework list's
/// base; `None` (said on `io`) when there is none.
fn release_base(
    session: &ew_api::Session,
    since: Option<&str>,
    dir: &Path,
    io: &mut Io<'_>,
) -> Option<ew_api::ReworkBase> {
    let releases = session.rework_releases();
    let release = match since {
        Some(label) => releases.iter().find(|r| r.label == label),
        None => releases.first(),
    };
    match (release, since) {
        (Some(r), _) => Some(ew_api::ReworkBase::Release(r.revision)),
        (None, Some(label)) => {
            io.error(format_args!(
                "{}: no release {label} whose archive is found beside the project; give the archive with \
                 `--archive <file>`",
                dir.display()
            ));
            None
        }
        (None, None) => {
            io.error(format_args!(
                "{}: {}",
                dir.display(),
                ew_api::ReworkError::NoRelease
            ));
            None
        }
    }
}

impl Leaf for LabelsCli {
    const EXITS: &'static [Exit] = &[
        exit(0, "written"),
        exit(
            2,
            "usage, open errors, no such release or its archive not found, an archive that does not open, write errors",
        ),
    ];

    fn check(&self) -> Result<(), clap::Error> {
        self.folder().map(drop)
    }
}

/// `ew export labels`: the label files of every label, or of those the rework list since the release `--since` (or the
/// archive file `--archive`) names, written into `--dir` as CSV or XLSX, each file whole.
pub(super) fn run_export_labels(a: &LabelsCli, io: &mut Io<'_>) -> i32 {
    let folder = match a.folder() {
        Ok(f) => f,
        Err(e) => return usage(&e, io),
    };
    let Some(dir) = project_dir(a.project.as_ref(), io) else {
        return 2;
    };
    let (session, _) = match open_reader(&dir, "export labels", io) {
        Ok(r) => r,
        Err(e) => {
            io.error(format_args!("{}: {e}", dir.display()));
            return 2;
        }
    };
    let scope = match (&a.archive, &a.since) {
        (Some(file), _) => ew_api::LabelScope::Changes(ew_api::ReworkBase::File(file.clone())),
        (None, None) => ew_api::LabelScope::All,
        (None, Some(label)) => match release_base(&session, Some(label), &dir, io) {
            Some(base) => ew_api::LabelScope::Changes(base),
            None => return 2,
        },
    };
    let format = match a.format {
        Some(ReportPages::Xlsx) => ew_api::LabelFileFormat::Xlsx,
        _ => ew_api::LabelFileFormat::Csv,
    };
    let files = match session.label_files(&scope, format, None) {
        Ok(f) => f,
        Err(e) => {
            io.error(format_args!("{}: {e}", dir.display()));
            return 2;
        }
    };
    if let Err(e) = ew_api::Session::write_label_files(folder, &files) {
        io.error(format_args!("{}: {e}", folder.display()));
        return 2;
    }
    for f in &files {
        outln!(io, "{}: {}", f.name, plural(f.count, "label", "labels"));
    }
    outln!(
        io,
        "label files written to {}: {}",
        folder.display(),
        plural(files.len(), "file", "files")
    );
    0
}

/// `ew export rework`: the rework list between a release (`--since`, else the last one whose archive is found; or the
/// archive file `--archive`) and the project as it is, written to `--out` as CSV or XLSX, all or nothing.
pub(super) fn run_export_rework(a: &ReworkCli, io: &mut Io<'_>) -> i32 {
    let out = match a.file() {
        Ok(out) => out,
        Err(e) => return usage(&e, io),
    };
    let Some(dir) = project_dir(a.project.as_ref(), io) else {
        return 2;
    };
    let (session, _) = match open_reader(&dir, "export rework", io) {
        Ok(r) => r,
        Err(e) => {
            io.error(format_args!("{}: {e}", dir.display()));
            return 2;
        }
    };
    let base = match &a.archive {
        Some(file) => ew_api::ReworkBase::File(file.clone()),
        None => match release_base(&session, a.since.as_deref(), &dir, io) {
            Some(base) => base,
            None => return 2,
        },
    };
    let rework = match session.rework(&base) {
        Ok(r) => r,
        Err(e) => {
            io.error(format_args!("{}: {e}", dir.display()));
            return 2;
        }
    };
    let bytes = match a.format {
        Some(ReportPages::Xlsx) => match session.rework_xlsx(&rework, None) {
            Ok(b) => b,
            Err(e) => {
                io.error(format_args!("{}: {e}", out.display()));
                return 2;
            }
        },
        _ => session.rework_csv(&rework).into_bytes(),
    };
    if let Err(e) = write_atomically(out, &bytes) {
        io.error(format_args!("{}: {e}", out.display()));
        return 2;
    }
    outln!(
        io,
        "rework list written to {}: {}",
        out.display(),
        plural(rework.set.rows.len(), "row", "rows")
    );
    0
}

impl Leaf for ExtractArgs {
    const EXITS: &'static [Exit] = &[
        exit(0, "extracted"),
        exit(
            2,
            "usage, an unreadable or refused archive, an existing folder, write errors",
        ),
    ];
    const RESULT: Option<&'static str> = Some(results::ExtractResult::SCHEMA);
}

/// `ew extract <file.ewz> [--into <dir>]`: the archive read whole and checked before
/// anything is written, then extracted into a new directory, all or nothing (`ew_api::ewz::extract`); without `--into`
/// the directory is `<stem>.ew` beside the archive.
pub(super) fn run_extract(a: &ExtractArgs, io: &mut Io<'_>) -> i32 {
    let vfs = ew_api::OsVfs;
    let archive = match ew_api::ewz::read_file(&vfs, &a.file) {
        Ok(r) => r,
        Err(e) => {
            io.error(format_args!(
                "{}: the .ewz is refused: {e}; nothing was written",
                a.file.display()
            ));
            return 2;
        }
    };
    let Some(into) = a.into.clone().or_else(|| ew_api::ewz::default_target(&a.file)) else {
        io.error(format_args!("{}: not an .ewz file name", a.file.display()));
        return 2;
    };
    if let Err(e) = ew_api::ewz::extract(&vfs, &archive, &into) {
        io.error(e);
        return 2;
    }
    let files = archive.entries.len();
    if a.json {
        out!(
            io,
            "{}",
            results::to_text(&results::ExtractResult {
                schema: results::ExtractResult::SCHEMA.to_owned(),
                into: into.display().to_string(),
                generation: archive.generation(),
                files,
            })
        );
    } else {
        outln!(
            io,
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
    write_file_set(out, &["svg"], "SVG", &files)
}

/// Writes a set of files with the extensions `exts` (`svg`; `png`; `dxf` with its pictures; `kind` names the export:
/// `SVG`) into the directory `out` as a whole ([`ew_api::file::write_file_set`]): `out` holds exactly this
/// export, a failure leaves the earlier set as it was, and a directory that holds a file no earlier export of the kind
/// wrote there is refused, whatever its extension (`ew export svg`, `ew export png`, `ew export dxf`).
pub(super) fn write_file_set(
    out: &Path,
    exts: &[&str],
    kind: &str,
    files: &[(&str, &[u8])],
) -> Result<(), String> {
    use ew_api::file::FileSetError;
    ew_api::file::write_file_set(out, exts, files).map_err(|e| match e {
        FileSetError::Foreign(name) => format!(
            "{}: the directory holds {name}, which no earlier {kind} export wrote there; choose an empty directory or \
             one an earlier {kind} export wrote",
            out.display()
        ),
        FileSetError::Io { path, error } => format!("{}: {error}", path.display()),
    })
}

impl Leaf for PdfCli {
    const EXITS: &'static [Exit] = &[OK, USAGE_ETC];
    const RESULT: Option<&'static str> = Some(results::ExportPdfResult::SCHEMA);

    fn check(&self) -> Result<(), clap::Error> {
        self.pages.export(PageFormat::Pdf).map(drop)
    }
}

impl Leaf for SvgCli {
    const EXITS: &'static [Exit] = &[OK, USAGE_ETC];
    const RESULT: Option<&'static str> = Some(results::ExportSvgResult::SCHEMA);

    fn check(&self) -> Result<(), clap::Error> {
        self.pages.export(PageFormat::Svg).map(drop)
    }
}

impl Leaf for PngCli {
    const EXITS: &'static [Exit] = &[OK, USAGE_ETC];
    const RESULT: Option<&'static str> = Some(results::ExportPngResult::SCHEMA);

    fn check(&self) -> Result<(), clap::Error> {
        self.export().map(drop)
    }
}

impl Leaf for DxfCli {
    const EXITS: &'static [Exit] = &[OK, USAGE_ETC];
    const RESULT: Option<&'static str> = Some(results::ExportDxfResult::SCHEMA);

    fn check(&self) -> Result<(), clap::Error> {
        self.export().map(drop)
    }
}

/// `ew export pdf|svg|png|dxf` of its line ([`PagesCli::export`] and its kin).
pub(super) fn run_export_pages(a: &ExportPagesArgs, io: &mut Io<'_>) -> i32 {
    let Some(dir) = reader_dir(a.project.as_ref(), io) else {
        return 2;
    };
    let date = match source_date(io.env) {
        Ok(d) => d,
        Err(e) => {
            io.error(e);
            return 2;
        }
    };
    let (session, _) = match open_reader(&dir, "export", io) {
        Ok(r) => r,
        Err(e) => {
            io.error(format_args!("{}: {e}", dir.display()));
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
        underlays: a.underlays,
        screen: false,
    };
    let (written, findings) = match a.format {
        PageFormat::Pdf => {
            let pdf = match session.export_pdf(&options) {
                Ok(p) => p,
                Err(e) => {
                    io.error(e);
                    return 2;
                }
            };
            if let Err(e) = write_atomically(&a.out, &pdf.bytes) {
                io.error(format_args!("{}: {e}", a.out.display()));
                return 2;
            }
            if a.json {
                out!(
                    io,
                    "{}",
                    results::to_text(&results::ExportPdfResult {
                        schema: results::ExportPdfResult::SCHEMA.to_owned(),
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
                ew_api::output::page_findings_summary(&pdf.findings),
            )
        }
        PageFormat::Svg => {
            let svg = match session.export_svg(&options) {
                Ok(s) => s,
                Err(e) => {
                    io.error(e);
                    return 2;
                }
            };
            if let Err(e) = write_svg_set(&a.out, &svg.files) {
                io.error(e);
                return 2;
            }
            if a.json {
                out!(
                    io,
                    "{}",
                    results::to_text(&results::ExportSvgResult {
                        schema: results::ExportSvgResult::SCHEMA.to_owned(),
                        out: a.out.display().to_string(),
                        files: svg.files.iter().map(|f| f.name.clone()).collect(),
                        findings: svg.findings.len(),
                    })
                );
            }
            (
                format!("SVG written to {}: {} files", a.out.display(), svg.files.len()),
                ew_api::output::page_findings_summary(&svg.findings),
            )
        }
        PageFormat::Dxf => {
            let dxf = match session.export_dxf(&options, &a.dxf_layers, a.dxf_dwg_links) {
                Ok(d) => d,
                Err(e) => {
                    io.error(e);
                    return 2;
                }
            };
            let files: Vec<(&str, &[u8])> = dxf
                .files
                .iter()
                .map(|f| (f.name.as_str(), f.text.as_bytes()))
                .chain(dxf.pictures.iter().map(|p| (p.name.as_str(), &*p.bytes)))
                .collect();
            if let Err(e) = write_file_set(&a.out, &ew_api::output::DXF_SET_EXTENSIONS, "DXF", &files) {
                io.error(e);
                return 2;
            }
            if a.json {
                out!(
                    io,
                    "{}",
                    results::to_text(&results::ExportDxfResult {
                        schema: results::ExportDxfResult::SCHEMA.to_owned(),
                        out: a.out.display().to_string(),
                        files: dxf.files.iter().map(|f| f.name.clone()).collect(),
                        pictures: dxf.pictures.iter().map(|p| p.name.clone()).collect(),
                        findings: dxf.findings.len(),
                    })
                );
            }
            let pictures = match dxf.pictures.len() {
                0 => String::new(),
                1 => " and the picture they name".to_owned(),
                n => format!(" and the {n} pictures they name"),
            };
            (
                format!(
                    "DXF written to {}: {} files{pictures}",
                    a.out.display(),
                    dxf.files.len()
                ),
                ew_api::output::page_findings_summary(&dxf.findings),
            )
        }
        PageFormat::Png => {
            let png = match session.export_png(&options, a.dpi) {
                Ok(p) => p,
                Err(e) => {
                    io.error(e);
                    return 2;
                }
            };
            let files: Vec<(&str, &[u8])> = png
                .files
                .iter()
                .map(|f| (f.name.as_str(), f.bytes.as_slice()))
                .collect();
            if let Err(e) = write_file_set(&a.out, &["png"], "PNG", &files) {
                io.error(e);
                return 2;
            }
            if a.json {
                out!(
                    io,
                    "{}",
                    results::to_text(&results::ExportPngResult {
                        schema: results::ExportPngResult::SCHEMA.to_owned(),
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
                ew_api::output::page_findings_summary(&png.findings),
            )
        }
    };
    if !a.json {
        outln!(io, "{written}");
    }
    if let Some(findings) = findings {
        errln!(io, "{findings}");
    }
    0
}

impl Leaf for RenderArgs {
    const EXITS: &'static [Exit] = &[
        OK,
        exit(2, "usage, open, I/O and writer errors, an unknown folio"),
    ];
    const RESULT: Option<&'static str> = Some(results::RenderResult::SCHEMA);
}

/// `ew render --page <folio> --png` (FR-EXT-004): one page as PNG, to `--out` or `<folio>.png` in the
/// current directory, never to stdout.
pub(super) fn run_render(a: &RenderArgs, io: &mut Io<'_>) -> i32 {
    let Some(dir) = reader_dir(a.project.as_ref(), io) else {
        return 2;
    };
    let (session, _) = match open_reader(&dir, "render", io) {
        Ok(r) => r,
        Err(e) => {
            io.error(format_args!("{}: {e}", dir.display()));
            return 2;
        }
    };
    let png = match session.render_png(&a.page, a.dpi) {
        Ok(p) => p,
        Err(e) => {
            io.error(e);
            return 2;
        }
    };
    let Some(file) = png.files.first() else {
        io.error(format_args!("the page order has no folio `{}`", a.page));
        return 2;
    };
    let out = a.out.clone().unwrap_or_else(|| PathBuf::from(&file.name));
    if let Err(e) = write_atomically(&out, &file.bytes) {
        io.error(format_args!("{}: {e}", out.display()));
        return 2;
    }
    if a.json {
        out!(
            io,
            "{}",
            results::to_text(&results::RenderResult {
                schema: results::RenderResult::SCHEMA.to_owned(),
                page: file.folio.clone(),
                out: out.display().to_string(),
                dpi: a.dpi,
                width: file.width,
                height: file.height,
                findings: png.findings.len(),
            })
        );
    } else {
        outln!(
            io,
            "PNG written to {}: folio {}, {} x {} pixels at {} dpi",
            out.display(),
            file.folio,
            file.width,
            file.height,
            a.dpi
        );
    }
    if let Some(findings) = ew_api::output::page_findings_summary(&png.findings) {
        errln!(io, "{findings}");
    }
    0
}

/// `ew export rework`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct ReworkCli {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The release to compare with, by its label (default: the last one whose archive is found).
    #[arg(long, value_name = "revision", conflicts_with = "archive")]
    pub(super) since: Option<String>,
    /// An archive file to compare with.
    #[arg(long, value_name = "file", allow_hyphen_values = true)]
    pub(super) archive: Option<PathBuf>,
    /// The format: csv (default) or xlsx.
    #[arg(long, value_name = "format", allow_hyphen_values = true, overrides_with = "format", value_parser = table_format())]
    pub(super) format: Option<ReportPages>,
    /// The file to write (required).
    #[arg(long, value_name = "file", allow_hyphen_values = true, overrides_with = "out")]
    pub(super) out: Option<PathBuf>,
}

impl ReworkCli {
    /// The file to write: `--out` is needed.
    pub(super) fn file(&self) -> Result<&PathBuf, clap::Error> {
        self.out.as_ref().ok_or_else(|| {
            usage_error(
                &["export", "rework"],
                "`ew export rework` writes a file: give `--out <file>`",
            )
        })
    }
}

/// `ew export drilling`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct DrillingCli {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// A surface by its words ("Door", "Bottom") or its ID; repeatable; absent: every surface with holes.
    #[arg(long, value_name = "surface", allow_hyphen_values = true)]
    pub(super) surface: Vec<String>,
    /// The templates' paper: a4 (default) or a3.
    #[arg(long, value_name = "paper", allow_hyphen_values = true)]
    pub(super) template: Option<String>,
    /// Leave the DXFs out.
    #[arg(long)]
    pub(super) no_dxf: bool,
    /// Leave the templates out.
    #[arg(long)]
    pub(super) no_template: bool,
    /// The folder to write into (required).
    #[arg(
        long,
        value_name = "folder",
        allow_hyphen_values = true,
        overrides_with = "dir"
    )]
    pub(super) dir: Option<PathBuf>,
}

impl Leaf for DrillingCli {
    const EXITS: &'static [Exit] = &[
        exit(0, "written"),
        exit(
            2,
            "usage, open errors, no surface with holes, an unknown surface or paper, write errors",
        ),
    ];

    fn check(&self) -> Result<(), clap::Error> {
        self.dir.as_ref().map(drop).ok_or_else(|| {
            usage_error(
                &["export", "drilling"],
                "`ew export drilling` writes files: give `--dir <folder>`",
            )
        })
    }
}

/// `ew export drilling`: the drawings for drilling of the surfaces asked (every surface with holes without
/// `--surface`), written into `--dir`, each file whole.
pub(super) fn run_export_drilling(a: &DrillingCli, io: &mut Io<'_>) -> i32 {
    let Some(folder) = a.dir.as_ref() else {
        return 2;
    };
    let paper = match a.template.as_deref() {
        None => ew_api::output::Paper::A4,
        Some(w) => match ew_api::output::Paper::of_word(w) {
            Some(p) => p,
            None => {
                io.error(format_args!("--template {w}: the paper is a4 or a3"));
                return 2;
            }
        },
    };
    let Some(dir) = project_dir(a.project.as_ref(), io) else {
        return 2;
    };
    let (session, _) = match open_reader(&dir, "export drilling", io) {
        Ok(r) => r,
        Err(e) => {
            io.error(format_args!("{}: {e}", dir.display()));
            return 2;
        }
    };
    let offered = session.drilling_surfaces(paper);
    if offered.is_empty() {
        io.error(format_args!(
            "{}: no surface of the project has holes",
            dir.display()
        ));
        return 2;
    }
    let mut surfaces = Vec::new();
    for name in &a.surface {
        let found = offered.iter().find(|o| {
            o.surface.to_string() == *name
                || ew_api::catalogue()
                    .text_for(&o.words, None)
                    .eq_ignore_ascii_case(name.trim())
        });
        match found {
            Some(o) => surfaces.push(o.surface),
            None => {
                io.error(format_args!(
                    "--surface {name}: no surface with holes is called so"
                ));
                return 2;
            }
        }
    }
    if surfaces.is_empty() {
        surfaces = offered.iter().map(|o| o.surface).collect();
    }
    let ask = ew_api::output::DrillingAsk {
        surfaces,
        paper,
        template: !a.no_template,
        dxf: !a.no_dxf,
    };
    let files = match session.drilling_files(&ask) {
        Ok(f) => f,
        Err(e) => {
            io.error(format_args!("{}: {e}", dir.display()));
            return 2;
        }
    };
    if let Err(e) = ew_api::Session::write_drilling_files(folder, &files) {
        io.error(format_args!("{}: {e}", folder.display()));
        return 2;
    }
    for f in &files {
        outln!(io, "{}", f.name);
    }
    outln!(
        io,
        "drawings for drilling written to {}: {}",
        folder.display(),
        plural(files.len(), "file", "files")
    );
    0
}

/// `ew export labels`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct LabelsCli {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// Only the labels the rework list against this release (by its label) names.
    #[arg(long, value_name = "revision", conflicts_with = "archive")]
    pub(super) since: Option<String>,
    /// Only the labels the rework list against this archive file names.
    #[arg(long, value_name = "file", allow_hyphen_values = true)]
    pub(super) archive: Option<PathBuf>,
    /// The format: csv (default) or xlsx.
    #[arg(long, value_name = "format", allow_hyphen_values = true, overrides_with = "format", value_parser = table_format())]
    pub(super) format: Option<ReportPages>,
    /// The folder to write into (required).
    #[arg(
        long,
        value_name = "folder",
        allow_hyphen_values = true,
        overrides_with = "dir"
    )]
    pub(super) dir: Option<PathBuf>,
}

impl LabelsCli {
    /// The folder to write into: `--dir` is needed.
    pub(super) fn folder(&self) -> Result<&PathBuf, clap::Error> {
        self.dir.as_ref().ok_or_else(|| {
            usage_error(
                &["export", "labels"],
                "`ew export labels` writes files: give `--dir <folder>`",
            )
        })
    }
}
