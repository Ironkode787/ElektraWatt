// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew import dxf` (doc 14 §13): a DXF drawing into the project as a graphic page, or its blocks as symbol drafts.

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
}

/// The arguments of `ew import dxf`.
#[derive(clap::Args, Clone, PartialEq, Eq, Debug)]
pub struct ImportDxfArgs {
    /// The DXF file (ASCII or binary, R12 to 2018).
    #[arg(value_name = "file.dxf")]
    pub file: PathBuf,
    /// What the drawing becomes: page or symbol.
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
        DxfTarget::Page => {
            let Some(dir) = project_dir(a.project.as_ref()) else {
                return 2;
            };
            let origin = ew_api::Origin::Cli {
                subcommand: "import dxf".into(),
                confirmed: false,
            };
            let Ok(mut session) = open_for_writing(&dir, origin) else {
                return 2;
            };
            let page = match session.import_dxf_page(&name, bytes, &a.options()) {
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
