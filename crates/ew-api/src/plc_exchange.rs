// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC exchange of a session (doc 10 §4, §7): the files a configuration project is exported as, the preview of
//! a file that comes back, the exchange status and the instrument list.
//!
//! Nothing here writes: an export returns the file's bytes and the `plc.record_exchange` command that records them
//! as the base of the next merge; an import returns its preview (the merge of `ew-plc` on the session's snapshot)
//! and the `plc.apply_exchange` command whose own run makes the same merge, so what was previewed is what is
//! applied. The host writes the file, runs the command and saves.
//!
//! - [`Session::plc_export`]: AML AR APC 1.4.0 or 1.2.0 on CAEX 2.15, the TIA "PLC Tags" workbook, or the PLCopen XML
//!   global variable list. An AML or PLCopen export is refused while a symbol is no IEC identifier under the
//!   configuration project's strict policy (EW-PLC-005, FR-PLC-047). The dates of a file are the export's document
//!   date: the caller's, else the model's edited date, never the clock (NFR-REL-07), so the same model and options
//!   give the same bytes (FR-EXP-006). No file holds a person field, with or without the personal-data option
//!   (FR-EXP-009).
//! - [`Session::plc_import`]: an AML file (CAEX 2.15 or 3.0) or a TIA tag workbook, read under the limits of doc 14
//!   §11 and inside `catch_unwind`.
//! - [`Session::plc_status`]: what changed since the last exchange (the drift of EW-PLC-016), and whether a file is
//!   the one last exchanged.
//! - [`Session::instrument_list`]: an instrument list (CSV or XLSX) as the `plc.apply_instrument_list` command.

use std::collections::BTreeMap;
use std::fmt;

use ew_checks::Rule as _;
use ew_derive::Q;
use ew_derive::ew_ids::Id;
use ew_import_catalog::SourceFormat;
use ew_import_catalog::ew_interop_api::Limits;
use ew_import_catalog::sheet::{read_rows, sheet_names};
use ew_plc::ew_caex::{AmlOptions, ExchangeImage, read_aml, write_aml};
use ew_plc::{MergeOptions, Plan, Projection, Vocabulary};
use ew_schema::{Device, ExchangeStamp, PlcConfigProject, Sha256};
use serde_json::{Value, json};
use sha2::Digest as _;

use crate::{AnyCommand, Session};

pub use ew_plc::ew_caex::ApcVersion;
pub use ew_plc::{field_words, object_words};
pub use ew_schema::{Date, ExchangeDirection, ExchangeFormat, ExchangeObject};

/// The largest exchange file a host reads (doc 14 §11): the input limit of the AML reader.
pub fn max_file_bytes() -> u64 {
    Limits::default().max_input_bytes
}

/// A file format of the PLC exchange.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlcFormat {
    /// AutomationML AR APC in the given version, on CAEX 2.15.
    Aml(ApcVersion),
    /// The TIA Portal "PLC Tags" workbook.
    TiaXlsx,
    /// PLCopen XML with the global variable list.
    Plcopen,
}

impl PlcFormat {
    fn stamp(self) -> (ExchangeFormat, Option<String>) {
        match self {
            PlcFormat::Aml(v) => (ExchangeFormat::Aml, Some(v.text().to_owned())),
            PlcFormat::TiaXlsx => (ExchangeFormat::TiaXlsx, None),
            PlcFormat::Plcopen => (ExchangeFormat::Plcopen, Some("2.01".to_owned())),
        }
    }

    fn extension(self) -> &'static str {
        match self {
            PlcFormat::Aml(_) => "aml",
            PlcFormat::TiaXlsx => "xlsx",
            PlcFormat::Plcopen => "xml",
        }
    }
}

/// What an export or import is asked.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PlcOptions {
    /// The configuration project by name; `None`: the project's only one.
    pub config: Option<String>,
    /// The stations to export, by tag; empty: every station of the configuration project.
    pub stations: Vec<String>,
    /// The document date; `None`: the model's edited date.
    pub date: Option<Date>,
    /// The language of comments in a tag table and of the documentation in PLCopen XML; `None`: the text without a
    /// language.
    pub lang: Option<String>,
    /// An import's file is complete: what it does not hold was removed in the PLC software.
    pub complete: bool,
    /// An import's choices: the key of a change or the ID of a parked object, with `ours`, `theirs` or `create`.
    pub resolutions: BTreeMap<String, String>,
}

/// Why an exchange was refused.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PlcExchangeError {
    /// The project has no configuration project of that name (the name, the names it has).
    NoConfig(Option<String>, Vec<String>),
    /// No station of the configuration project has that tag.
    NoStation(String),
    /// Symbols that are no IEC identifiers block the export (EW-PLC-005): the findings' texts.
    InvalidSymbols(Vec<String>),
    /// The file cannot be read: the reason.
    Unreadable(String),
    /// The workbook cannot be written: the reason.
    Unwritable(String),
    /// Reading the file panicked; the model is as it was (NFR-REL-12).
    Panicked,
}

impl fmt::Display for PlcExchangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlcExchangeError::NoConfig(None, names) if names.is_empty() => {
                f.write_str("the project has no PLC configuration project: there is nothing to exchange")
            }
            PlcExchangeError::NoConfig(None, names) => write!(
                f,
                "the project has several PLC configuration projects ({}); name one with --config",
                names.join(", ")
            ),
            PlcExchangeError::NoConfig(Some(name), names) => write!(
                f,
                "the project has no PLC configuration project `{name}`; it has {}",
                if names.is_empty() {
                    "none".to_owned()
                } else {
                    names.join(", ")
                }
            ),
            PlcExchangeError::NoStation(tag) => {
                write!(f, "no station of the configuration project has the tag `{tag}`")
            }
            PlcExchangeError::InvalidSymbols(findings) => {
                write!(f, "the export is blocked by EW-PLC-005: {}", findings.join("; "))
            }
            PlcExchangeError::Unreadable(why) => write!(f, "the file cannot be read: {why}"),
            PlcExchangeError::Unwritable(why) => f.write_str(why),
            PlcExchangeError::Panicked => f.write_str(
                "reading the file failed inside the program; the project is unchanged: report the file",
            ),
        }
    }
}

impl std::error::Error for PlcExchangeError {}

/// An exported file with the command that records it.
#[derive(Clone, PartialEq, Debug)]
pub struct PlcExport {
    /// The file's bytes.
    pub bytes: Vec<u8>,
    /// A name for the file: the configuration project's name with the format's extension.
    pub file_name: String,
    /// `plc.record_exchange` for this file: run it once the file is written.
    pub record: AnyCommand,
    /// What the reader of the file should know (addresses not confirmed by the PLC configuration).
    pub warnings: Vec<String>,
    /// The stations, modules, channels and tags in the file.
    pub counts: [usize; 4],
}

/// The preview of an import with the command that applies it.
#[derive(Clone, PartialEq, Debug)]
pub struct PlcImport {
    /// The merge: the changes, the parked objects and the removed ones.
    pub plan: Plan,
    /// `plc.apply_exchange` for this file with the given resolutions.
    pub apply: AnyCommand,
}

impl PlcImport {
    /// The preview as the result of `ew import plc-aml --json` or `plc-xlsx --json` (`schema` names which): every
    /// change with its key, its sides and what the merge makes of it, the parked objects and the removed ones.
    pub fn result(&self, schema: &str, applied: bool) -> crate::cli_results::ImportPlcResult {
        use crate::cli_results::{ImportPlcChange, ImportPlcParked, ImportPlcRemoved, ImportPlcResult};
        let word = |v: Value| v.as_str().map(str::to_owned);
        ImportPlcResult {
            schema: schema.to_owned(),
            changes: self
                .plan
                .changes
                .iter()
                .map(|c| ImportPlcChange {
                    id: c.id.clone(),
                    object: c.object.name().to_owned(),
                    subject: c.subject.clone(),
                    field: c.field.clone(),
                    label: field_words(&c.object, &c.field),
                    base: c.base.clone(),
                    ours: c.ours.clone(),
                    theirs: c.theirs.clone(),
                    group: c.group.as_ref().and_then(|g| word(json!(g))),
                    owner: c.owner.as_ref().and_then(|o| word(json!(o))),
                    state: c.state.word().to_owned(),
                    default: match c.default {
                        ew_plc::Choice::Ours => "ours",
                        ew_plc::Choice::Theirs => "theirs",
                    }
                    .to_owned(),
                    acceptable: c.acceptable,
                })
                .collect(),
            parking: self
                .plan
                .parking
                .iter()
                .map(|p| ImportPlcParked {
                    id: p.id.clone(),
                    object: p.object.name().to_owned(),
                    name: p.name.clone(),
                    reason: if p.ambiguous { "ambiguous" } else { "unmatched" }.to_owned(),
                    creatable: p.creatable,
                })
                .collect(),
            removed: self
                .plan
                .removed
                .iter()
                .map(|(o, _, name)| ImportPlcRemoved {
                    object: o.name().to_owned(),
                    name: name.clone(),
                })
                .collect(),
            applied,
        }
    }
}

/// The exchange status of a configuration project (`ew plc status`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlcStatus {
    /// The configuration project's name.
    pub config: String,
    /// The stamp of the last exchange; `None` before the first.
    pub stamp: Option<ExchangeStamp>,
    /// The objects that differ from the last exchange: the object's kind, its name and its differing fields (none
    /// for an object added or gone since).
    pub drift: Vec<(ExchangeObject, String, Vec<String>)>,
    /// Whether the given file is the one last exchanged (its SHA-256 is the stamp's); `None` without a file or a
    /// stamp.
    pub same_file: Option<bool>,
}

fn sha256(bytes: &[u8]) -> Sha256 {
    Sha256(sha2::Sha256::digest(bytes).into())
}

/// The model's date (FR-OUT-026): edited, else created, else 1980-01-01.
fn model_date(q: &Q) -> Date {
    let m = &q.project().master;
    m.edited
        .or(m.created)
        .or_else(|| Date::new(1980, 1, 1).ok())
        .expect("1980-01-01 is a date")
}

/// The configuration project named `name`, or the project's only one.
fn config_project(q: &Q, name: Option<&str>) -> Result<Id<PlcConfigProject>, PlcExchangeError> {
    let all: Vec<(Id<PlcConfigProject>, String)> = q
        .table::<PlcConfigProject>()
        .iter()
        .map(|(id, c)| (id, c.name.to_string()))
        .collect();
    let names = || all.iter().map(|(_, n)| n.clone()).collect();
    match name {
        Some(name) => all
            .iter()
            .find(|(_, n)| n == name)
            .map(|(id, _)| *id)
            .ok_or_else(|| PlcExchangeError::NoConfig(Some(name.to_owned()), names())),
        None => match all.as_slice() {
            [(id, _)] => Ok(*id),
            _ => Err(PlcExchangeError::NoConfig(None, names())),
        },
    }
}

impl Session {
    /// The configuration project `config` exported in `format` (doc 10 §4.3, §4.5, §4.6), with the command that
    /// records the export. `writer_version` is the program's version in the file's header.
    ///
    /// # Errors
    ///
    /// [`PlcExchangeError::NoConfig`], [`PlcExchangeError::NoStation`]; [`PlcExchangeError::InvalidSymbols`] for an
    /// AML or PLCopen export under the strict symbol policy; [`PlcExchangeError::Unwritable`] for a tag table larger
    /// than a sheet.
    pub fn plc_export(&self, format: PlcFormat, options: &PlcOptions) -> Result<PlcExport, PlcExchangeError> {
        let q = self.query();
        let cfg = config_project(&q, options.config.as_deref())?;
        let stations: Vec<Id<Device>> = options
            .stations
            .iter()
            .map(|tag| {
                let station: Option<Id<Device>> = self.resolve_reference(tag).ok().map(Id::from_uuid);
                station
                    .filter(|s| {
                        q.devices()
                            .get(*s)
                            .and_then(|d| d.aspects.plc_station.as_ref())
                            .is_some_and(|p| p.config_project == cfg)
                    })
                    .ok_or_else(|| PlcExchangeError::NoStation(tag.clone()))
            })
            .collect::<Result<_, _>>()?;
        if format != PlcFormat::TiaXlsx {
            let findings = ew_checks::rules::plc_wiring::InvalidSymbol.run(&q, &ew_checks::Scope::Cfg(cfg));
            if !findings.is_empty() {
                return Err(PlcExchangeError::InvalidSymbols(
                    findings.into_iter().map(|f| f.message).collect(),
                ));
            }
        }
        let scope = (!stations.is_empty()).then_some(stations.as_slice());
        let projection = ew_plc::exchange_image(&q, cfg, scope)
            .ok_or_else(|| PlcExchangeError::NoConfig(options.config.clone(), Vec::new()))?;
        let date = options.date.unwrap_or_else(|| model_date(&q));
        let name = projection.image.project.name.clone();
        let file_name = format!("{name}.{}", format.extension());
        let mut warnings = Vec::new();
        let bytes = match format {
            PlcFormat::Aml(apc) => write_aml(
                &projection.image,
                &AmlOptions {
                    apc,
                    file_name: file_name.clone(),
                    date: date.to_string(),
                    writer_version: env!("CARGO_PKG_VERSION").to_owned(),
                    project_title: name.clone(),
                    project_id: q.project().id.to_string(),
                },
            )
            .into_bytes(),
            PlcFormat::Plcopen => ew_plc::plcopen::write_gvl(
                &projection,
                &ew_plc::plcopen::GvlOptions {
                    date: date.to_string(),
                    writer_version: env!("CARGO_PKG_VERSION").to_owned(),
                    lang: options.lang.clone(),
                },
            )
            .into_bytes(),
            PlcFormat::TiaXlsx => {
                let confirmed = q
                    .table::<PlcConfigProject>()
                    .get(cfg)
                    .and_then(|c| c.ownership.as_ref())
                    .is_some_and(|o| o.imported.is_some());
                if !confirmed {
                    warnings.push(
                        "addresses not confirmed by the PLC configuration: no PLC import has set the module starts"
                            .to_owned(),
                    );
                }
                let rows = ew_plc::tia::tag_rows(&projection, options.lang.as_deref());
                ew_reports::xlsx::plain_xlsx(
                    ew_plc::tia::SHEET,
                    &rows,
                    ew_plc::tia::COLUMNS.len(),
                    ew_reports::xlsx::DateTime {
                        year: date.year(),
                        month: date.month(),
                        day: date.day(),
                        hour: 0,
                        minute: 0,
                        second: 0,
                    },
                )
                .map_err(|e| PlcExchangeError::Unwritable(e.to_string()))?
            }
        };
        let (stamp_format, version) = format.stamp();
        let record = AnyCommand {
            name: "plc.record_exchange".to_owned(),
            params: json!({
                "config_project": cfg.to_string(),
                "stations": stations.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "format": stamp_format,
                "version": version,
                "file_sha256": sha256(&bytes),
                "date": date,
            }),
        };
        Ok(PlcExport {
            bytes,
            file_name,
            record,
            warnings,
            counts: counts(&projection),
        })
    }

    /// The preview of the exchange file `bytes` (an AML file, or a TIA tag workbook) against configuration project
    /// `config` (doc 10 §4.4, §4.5), with the command that applies it under `options.resolutions`.
    ///
    /// # Errors
    ///
    /// [`PlcExchangeError::NoConfig`]; [`PlcExchangeError::Unreadable`] for a file that is neither, is beyond a limit
    /// or is not well-formed, with the reader's reason; [`PlcExchangeError::Panicked`].
    pub fn plc_import(&self, bytes: &[u8], options: &PlcOptions) -> Result<PlcImport, PlcExchangeError> {
        let q = self.query();
        let cfg = config_project(&q, options.config.as_deref())?;
        let (image, format, version) = guard(|| read_exchange_file(bytes, options.lang.as_deref()))?;
        let vocabulary = match format {
            ExchangeFormat::TiaXlsx => Vocabulary::Tags {
                comment: ew_plc::tia::comment_field(options.lang.as_deref()),
            },
            _ => Vocabulary::All,
        };
        let projection = ew_plc::exchange_image(&q, cfg, None)
            .ok_or_else(|| PlcExchangeError::NoConfig(options.config.clone(), Vec::new()))?;
        let ownership = q
            .table::<PlcConfigProject>()
            .get(cfg)
            .and_then(|c| c.ownership.clone());
        let plan = ew_plc::merge(
            &projection,
            &ew_plc::snapshot(&q, cfg),
            &image,
            &MergeOptions {
                ownership: ownership.as_ref(),
                complete: options.complete,
                vocabulary,
            },
        );
        let apply = AnyCommand {
            name: "plc.apply_exchange".to_owned(),
            params: json!({
                "config_project": cfg.to_string(),
                "image": image,
                "format": format,
                "version": version,
                "file_sha256": sha256(bytes),
                "date": options.date.unwrap_or_else(|| model_date(&q)),
                "complete": options.complete,
                "comment_lang": options.lang,
                "resolutions": options.resolutions,
            }),
        };
        Ok(PlcImport { plan, apply })
    }

    /// The exchange status of configuration project `config` (doc 10 §4.4, FR-PLC-035): its stamp, what differs
    /// from the last exchange, and whether `file` is the file last exchanged.
    ///
    /// # Errors
    ///
    /// [`PlcExchangeError::NoConfig`].
    pub fn plc_status(
        &self,
        config: Option<&str>,
        file: Option<&[u8]>,
    ) -> Result<PlcStatus, PlcExchangeError> {
        let q = self.query();
        let cfg = config_project(&q, config)?;
        let record = q.table::<PlcConfigProject>().get(cfg).cloned();
        let stamp = record.as_ref().and_then(|c| c.exchange.clone());
        Ok(PlcStatus {
            config: record.map(|c| c.name.to_string()).unwrap_or_default(),
            drift: ew_plc::drift(&q, cfg)
                .into_iter()
                .map(|(object, _, name, fields)| (object, name, fields))
                .collect(),
            same_file: file
                .zip(stamp.as_ref())
                .map(|(bytes, s)| sha256(bytes) == s.file_sha256),
            stamp,
        })
    }

    /// The instrument list `bytes` (a CSV, or the first sheet of a workbook; doc 10 §4.7, FR-PLC-050) as the
    /// `plc.apply_instrument_list` command for configuration project `config`.
    ///
    /// # Errors
    ///
    /// [`PlcExchangeError::NoConfig`]; [`PlcExchangeError::Unreadable`] for a file that is no table or no instrument
    /// list, with the reason; [`PlcExchangeError::Panicked`].
    pub fn instrument_list(
        &self,
        config: Option<&str>,
        bytes: &[u8],
    ) -> Result<AnyCommand, PlcExchangeError> {
        let q = self.query();
        let cfg = config_project(&q, config)?;
        let rows = guard(|| {
            let limits = ew_import_catalog::table_limits();
            let format = SourceFormat::detect(bytes);
            let unreadable = |e: &dyn fmt::Display| PlcExchangeError::Unreadable(e.to_string());
            let sheet = match format {
                SourceFormat::Xlsx => sheet_names(bytes, &limits)
                    .map_err(|e| unreadable(&e))?
                    .into_iter()
                    .next()
                    .unwrap_or_default(),
                _ => String::new(),
            };
            let table = read_rows(bytes, format, &sheet, &limits).map_err(|e| unreadable(&e))?;
            ew_plc::instrument::read_rows(&table).map_err(PlcExchangeError::Unreadable)
        })?;
        Ok(AnyCommand {
            name: "plc.apply_instrument_list".to_owned(),
            params: json!({"config_project": cfg.to_string(), "rows": rows}),
        })
    }
}

/// The stations, modules, channels and tags of a projection's image.
fn counts(projection: &Projection) -> [usize; 4] {
    let objects = ew_plc::objects(&projection.image);
    let of = |name: &str| objects.iter().filter(|o| o.object.name() == name).count();
    [of("station"), of("module"), of("channel"), of("tag")]
}

/// Runs a read of foreign bytes inside `catch_unwind` (NFR-REL-12).
fn guard<T>(read: impl FnOnce() -> Result<T, PlcExchangeError>) -> Result<T, PlcExchangeError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(read)).unwrap_or(Err(PlcExchangeError::Panicked))
}

/// An exchange file as its image, with the format and version it is: a workbook (a ZIP) is a TIA tag table, in
/// which the comment column is the comment in `lang`; everything else is read as AML.
fn read_exchange_file(
    bytes: &[u8],
    lang: Option<&str>,
) -> Result<(ExchangeImage, ExchangeFormat, Option<String>), PlcExchangeError> {
    let unreadable = |e: &dyn fmt::Display| PlcExchangeError::Unreadable(e.to_string());
    if SourceFormat::detect(bytes) == SourceFormat::Xlsx {
        let limits = ew_import_catalog::table_limits();
        let rows =
            read_rows(bytes, SourceFormat::Xlsx, ew_plc::tia::SHEET, &limits).map_err(|e| unreadable(&e))?;
        let image = ew_plc::tia::read_tags(&rows, lang).map_err(PlcExchangeError::Unreadable)?;
        return Ok((image, ExchangeFormat::TiaXlsx, None));
    }
    let image = read_aml(bytes, &Limits::default()).map_err(|e| unreadable(&e))?;
    Ok((image, ExchangeFormat::Aml, None))
}
