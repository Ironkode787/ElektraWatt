// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew import dxf`, `ew import dwg` and `ew import pdf` (doc 14 §13): a drawing into the project as a graphic page or
//! an underlay, its blocks as symbol drafts, its lines as a part's front outline; a DWG through the user's ODA File
//! Converter first; a PDF page as an underlay.

use std::io::Read;
use std::path::{Path, PathBuf};

use ew_api::dxf::{DxfError, DxfOptions, Space, Unit};

use super::open::{open_for_writing, project_dir};
use super::revision::warn_unreleased;

/// What a DXF drawing becomes (`--as`).
#[derive(clap::ValueEnum, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DxfTarget {
    /// A new graphic page of the project.
    #[default]
    Page,
    /// Symbol drafts, one per named block, printed as JSON (as `ew symbol import`).
    Symbol,
    /// A locked underlay on the page `--on`.
    Underlay,
    /// The front outline of the part `--part` of the library `--library`.
    Outline,
}

/// The arguments of `ew import dxf`.
#[derive(clap::Args, Clone, PartialEq, Eq, Debug)]
pub struct ImportDxfArgs {
    /// The drawing: a DXF (ASCII or binary, R12 to 2018), or with `import dwg` a DWG.
    #[arg(value_name = "file")]
    pub file: PathBuf,
    /// What the drawing becomes: page, underlay, symbol or outline.
    #[arg(long = "as", value_name = "target", value_enum, default_value_t = DxfTarget::Page, overrides_with = "target")]
    pub target: DxfTarget,
    /// The project directory (default: the nearest ancestor of the current directory holding `manifest.json`).
    #[arg(long, value_name = "dir", allow_hyphen_values = true, overrides_with = "project")]
    pub project: Option<PathBuf>,
    /// The drawing's unit when its header names none: mm, cm, m, in, ft, um, dm or yd.
    #[arg(long, value_name = "unit", value_parser = unit, overrides_with = "units")]
    pub units: Option<Unit>,
    /// The paper-space layout to read (default: the model space).
    #[arg(long, value_name = "name", allow_hyphen_values = true, overrides_with = "layout")]
    pub layout: Option<String>,
    /// A layer that is off or frozen, imported anyway (repeatable).
    #[arg(long = "show-layer", value_name = "name", allow_hyphen_values = true)]
    pub show_layer: Vec<String>,
    /// The symbol drafts' key (with `--as symbol`; default: the file's name).
    #[arg(long, value_name = "key", allow_hyphen_values = true, overrides_with = "key")]
    pub key: Option<String>,
    /// The folio of the page the underlay lies on (with `--as underlay`).
    #[arg(long, value_name = "folio", allow_hyphen_values = true, overrides_with = "on")]
    pub on: Option<String>,
    /// Embed the source in the project (with `--as underlay`): a customer drawing may be confidential.
    #[arg(long, overrides_with = "embed")]
    pub embed: bool,
    /// The writable library of the part (with `--as outline`).
    #[arg(long, value_name = "dir", allow_hyphen_values = true, overrides_with = "library")]
    pub library: Option<PathBuf>,
    /// The part number of the part (with `--as outline`).
    #[arg(long, value_name = "part-number", allow_hyphen_values = true, overrides_with = "part")]
    pub part: Option<String>,
}

/// The arguments of `ew import dwg`: those of `import dxf` and the converter.
#[derive(clap::Args, Clone, PartialEq, Eq, Debug)]
pub struct ImportDwgArgs {
    /// As for `import dxf`.
    #[command(flatten)]
    pub dxf: ImportDxfArgs,
    /// The program of the ODA File Converter (default: `EW_DWG_CONVERTER`); never bundled or downloaded.
    #[arg(long, value_name = "program", allow_hyphen_values = true, overrides_with = "converter")]
    pub converter: Option<PathBuf>,
}

/// The arguments of `ew import pdf`.
#[derive(clap::Args, Clone, PartialEq, Eq, Debug)]
pub struct ImportPdfArgs {
    /// The PDF file.
    #[arg(value_name = "file.pdf")]
    pub file: PathBuf,
    /// The folio of the page the underlay lies on.
    #[arg(long, value_name = "folio", allow_hyphen_values = true, overrides_with = "on")]
    pub on: String,
    /// The PDF's page, 1 for the first.
    #[arg(long, value_name = "n", default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..), overrides_with = "page")]
    pub page: u32,
    /// Embed the source in the project: a customer drawing may be confidential.
    #[arg(long, overrides_with = "embed")]
    pub embed: bool,
    /// The project directory (default: the nearest ancestor of the current directory holding `manifest.json`).
    #[arg(long, value_name = "dir", allow_hyphen_values = true, overrides_with = "project")]
    pub project: Option<PathBuf>,
}

fn unit(s: &str) -> Result<Unit, String> {
    Unit::parse(s).ok_or_else(|| {
        let names: Vec<&str> = Unit::ALL.iter().map(|u| u.name()).collect();
        format!("`{s}` is no unit; give one of {}", names.join(", "))
    })
}

impl ImportDxfArgs {
    /// The reading options of the arguments.
    pub fn options(&self) -> DxfOptions {
        DxfOptions {
            unit: self.units,
            space: self.layout.clone().map_or(Space::Model, Space::Layout),
            show: self.show_layer.iter().cloned().collect(),
        }
    }
}

/// The bytes of an input file, at most the importers' input limit.
pub(super) fn read_input(path: &Path) -> Result<(String, Vec<u8>), i32> {
    let limit = ew_api::import::Limits::default().max_input_bytes;
    let name = path
        .file_name()
        .map_or_else(|| "drawing".to_owned(), |n| n.to_string_lossy().into_owned());
    let mut bytes = Vec::new();
    let read = std::fs::File::open(path).and_then(|f| f.take(limit + 1).read_to_end(&mut bytes));
    if let Err(e) = read {
        eprintln!("error: {}: {e}", path.display());
        return Err(2);
    }
    if bytes.len() as u64 > limit {
        eprintln!("error: {name} is larger than the {limit} bytes an import reads");
        return Err(2);
    }
    Ok((name, bytes))
}

/// The words of a refused DXF import, with the flag that answers a missing unit.
fn refused(e: &DxfError) -> String {
    match e {
        DxfError::NoUnit => "error: the drawing names no unit; give it with --units (mm, cm, m, in, …)".to_owned(),
        other => format!("error: {other}"),
    }
}

/// Prints an import's findings, one `<code> <severity>: <message>` each.
pub(super) fn print_findings(entries: &[ew_api::import::ReportEntry]) {
    for e in entries {
        let severity = match e.severity {
            ew_api::import::Severity::Info => "info",
            ew_api::import::Severity::Warning => "warning",
            ew_api::import::Severity::Error => "error",
        };
        println!("{} {severity}: {}", e.code, e.message);
    }
}

/// `ew import dxf`: exit 0 when imported (and saved), 2 on usage, a refused file, a refused import or a failed save.
pub(super) fn run_import_dxf(a: &ImportDxfArgs) -> i32 {
    let Ok((name, bytes)) = read_input(&a.file) else {
        return 2;
    };
    import(a, "import dxf", &name, bytes, Vec::new())
}

/// `ew import dwg`: the drawing converted by the user's ODA File Converter (`--converter`, else `EW_DWG_CONVERTER`),
/// then as `ew import dxf`; without a converter the refusal says how to give one (EW-IMP-257).
pub(super) fn run_import_dwg(a: &ImportDwgArgs) -> i32 {
    let Ok((name, bytes)) = read_input(&a.dxf.file) else {
        return 2;
    };
    let converter = a.converter.clone().or_else(dwg_converter_variable);
    let Some(converter) = converter else {
        eprintln!("error: {}", ew_api::dxf::DwgError::NoConverter);
        return 2;
    };
    let dxf = match ew_api::dxf::convert_dwg(&converter, &name, &bytes, ew_api::dxf::DWG_TIMEOUT) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let converted = Path::new(&name).with_extension("dxf").to_string_lossy().into_owned();
    import(&a.dxf, "import dwg", &converted, dxf, vec![ew_api::dxf::dwg_converted(&name)])
}

// The program boundary: `EW_DWG_CONVERTER`, the converter program the user installed, read once and passed in.
#[allow(
    clippy::disallowed_methods,
    reason = "the environment is read at the program boundary"
)]
fn dwg_converter_variable() -> Option<PathBuf> {
    std::env::var_os("EW_DWG_CONVERTER")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// The import of a drawing read (a DXF, or a DWG's DXF with `notes` naming the conversion).
fn import(
    a: &ImportDxfArgs,
    subcommand: &str,
    name: &str,
    bytes: Vec<u8>,
    notes: Vec<ew_api::import::ReportEntry>,
) -> i32 {
    let name = name.to_owned();
    match a.target {
        DxfTarget::Symbol => {
            let key = a.key.clone().unwrap_or_else(|| ew_api::symbols::file_key(&a.file));
            match ew_api::symbols::import_dxf(&bytes, &name, &key, a.units) {
                Ok(drafts) => {
                    println!("{}", serde_json::to_string_pretty(&drafts).expect("drafts have a JSON form"));
                    0
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    2
                }
            }
        }
        DxfTarget::Outline => {
            let (Some(library), Some(part)) = (&a.library, &a.part) else {
                eprintln!("error: `--as outline` needs `--library <dir>` and `--part <part-number>`");
                return 2;
            };
            match ew_api::dxf::import_part_outline(library, part, &name, &bytes, a.units) {
                Ok(o) => {
                    println!("The outline of {name} is the front outline of {part}, version {} ({} lines).", o.version, o.lines);
                    0
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    2
                }
            }
        }
        DxfTarget::Underlay => {
            let Some(folio) = &a.on else {
                eprintln!("error: `--as underlay` needs `--on <folio>`");
                return 2;
            };
            let source = ew_api::output::UnderlaySource::Dxf(a.options());
            place(a.project.as_ref(), subcommand, folio, &name, bytes, &source, a.embed, notes)
        }
        DxfTarget::Page => {
            let Some(dir) = project_dir(a.project.as_ref()) else {
                return 2;
            };
            let origin = ew_api::Origin::Cli {
                subcommand: subcommand.into(),
                confirmed: false,
            };
            let Ok(mut session) = open_for_writing(&dir, origin) else {
                return 2;
            };
            let page = match session.dxf_page_batch(&name, bytes, &a.options()).and_then(|mut batch| {
                batch.report.entries.extend(notes);
                session.apply_page_batch(batch)
            }) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{}", refused(&e));
                    session.close();
                    return 2;
                }
            };
            print_findings(&page.report.entries);
            let items = page.report.stats.per_kind.get("page_item").copied().unwrap_or(0);
            if let Err(e) = session.save().map(|r| warn_unreleased(r.unstable, &mut std::io::stderr())) {
                eprintln!("error: the save failed: {e:?}");
                session.close();
                return 2;
            }
            println!("Imported {name} as page {} with {items} items.", page.folio);
            0
        }
    }
}

/// Places an underlay on the page `folio` of the project and saves: exit 0, 2 on a refused source or page, open and
/// save errors.
#[allow(clippy::too_many_arguments)]
fn place(
    project: Option<&PathBuf>,
    subcommand: &str,
    folio: &str,
    name: &str,
    bytes: Vec<u8>,
    source: &ew_api::output::UnderlaySource,
    embed: bool,
    notes: Vec<ew_api::import::ReportEntry>,
) -> i32 {
    let Some(dir) = project_dir(project) else {
        return 2;
    };
    let origin = ew_api::Origin::Cli {
        subcommand: subcommand.into(),
        confirmed: false,
    };
    let Ok(mut session) = open_for_writing(&dir, origin) else {
        return 2;
    };
    let report = match session.place_underlay(folio, name, bytes, source, embed, notes) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", refused(&e));
            session.close();
            return 2;
        }
    };
    print_findings(&report.entries);
    if let Err(e) = session.save().map(|r| warn_unreleased(r.unstable, &mut std::io::stderr())) {
        eprintln!("error: the save failed: {e:?}");
        session.close();
        return 2;
    }
    let kept = if embed { "embedded in the project" } else { "kept outside the project" };
    println!("Placed {name} as an underlay on page {folio}, {kept}.");
    0
}

/// `ew import pdf`: a PDF page as a locked underlay; exit 0 when placed and saved, 2 otherwise.
pub(super) fn run_import_pdf(a: &ImportPdfArgs) -> i32 {
    let Ok((name, bytes)) = read_input(&a.file) else {
        return 2;
    };
    let source = ew_api::output::UnderlaySource::Pdf { page: a.page };
    place(a.project.as_ref(), "import pdf", &a.on, &name, bytes, &source, a.embed, Vec::new())
}
