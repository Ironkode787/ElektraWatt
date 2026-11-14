// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EPLAN report spreadsheets, as a table and a column map, into a project without drawings (doc 14 §5.4, FR-IMP-029).
//!
//! The host reads the XLSX or CSV (`ew-import-catalog`'s readers) and hands the importer the header row and the data
//! rows; the importer keeps to the contract: bytes in a table, a [`ReportMap`] of the user's own columns, records out.
//!
//! - **devices** (`designation`, `part`, `description`): one device each, unplaced, a black-box function, the tag the
//!   designation (a frozen tag, its structure prefix kept in `extra`);
//! - **terminals** (`strip`, `designation`, `level`): the strips and their terminals in file order; consecutive rows of
//!   one designation with rising levels are one multi-level terminal (EW-IMP-207);
//! - **connections** (`from`, `to`): `<strip designation>:<terminal designation>` on both ends; a connection joins the
//!   two terminals' connection points (the first side of the first, the second side of the other). A connection with
//!   an end that no terminal of the file answers is dropped (EW-IMP-003): a device's connection points need its part,
//!   which a report does not give.
//!
//! The identity of every record is `v5(lineage namespace, "eplan:<table>:<row key>")` in the lineage `eplan` anchored on
//! the map's key, so a newer export of the same list is the same records (a re-import updates in place).

use std::collections::BTreeMap;

use ew_interop_api::ew_ids::{Id, Uuid, v5};
use ew_interop_api::ew_schema::{
    Connection, ConnectionKind, Device, DeviceAspects, DeviceKind, Endpoint, Extra, FnRole, FrozenTag,
    Function, FunctionKind, ImportLineage, LevelSpec, LibDocument, LineageAnchor, MasterData, NumberSpec,
    Project, Record, Side, Strip, TagSpec, Terminal, WireOverrides,
};
use ew_interop_api::{
    Code, ImportBatch, ImportError, ImportReport, ReportEntry, Severity, SourceInfo, SourceLocation,
    SourceRef,
};
use ew_mltext::{Ident, MlText};
use serde::{Deserialize, Serialize};

/// The importer's lineage name.
pub const REPORT_IMPORTER_ID: &str = "eplan";

/// The importer's version, recorded in the report.
pub const REPORT_IMPORTER_VERSION: &str = "1";

/// The key of the project record a report import creates.
const PROJECT_KEY: &str = "eplan:project";

/// The most findings one report import lists (doc 14 §11 "Report volume").
const MAX_ENTRIES: usize = 1_000;

/// What a report spreadsheet lists.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportTarget {
    /// A device list.
    Devices,
    /// A terminal diagram.
    Terminals,
    /// A wire or connection list.
    Connections,
}

impl ReportTarget {
    /// The members a map of this target names, the required ones first.
    pub fn members(self) -> &'static [(&'static str, bool)] {
        match self {
            ReportTarget::Devices => &[("designation", true), ("part", false), ("description", false)],
            ReportTarget::Terminals => &[
                ("strip", true),
                ("designation", true),
                ("level", false),
                ("part", false),
            ],
            ReportTarget::Connections => &[("from", true), ("to", true)],
        }
    }
}

/// The saved mapping of one kind of report (doc 14 §5.4): the target, the sheet and header row where the file has them,
/// and the column of each member by its header text.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportMap {
    /// What the report lists.
    pub target: ReportTarget,
    /// The XLSX sheet (the first when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet: Option<String>,
    /// The 1-based row of the header among the rows with text (detected in the first 20 when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_row: Option<u32>,
    /// Member → header text.
    pub columns: BTreeMap<String, String>,
}

impl ReportMap {
    /// Reads a map from TOML.
    ///
    /// # Errors
    ///
    /// The text of the problem: not TOML of this shape, an unknown member, or a required member without a column.
    pub fn from_toml(text: &str) -> Result<ReportMap, String> {
        let map: ReportMap = toml::from_str(text).map_err(|e| e.to_string())?;
        for key in map.columns.keys() {
            if !map.target.members().iter().any(|(m, _)| m == key) {
                return Err(format!("`{key}` is not a column of a {:?} report", map.target));
            }
        }
        for (m, required) in map.target.members() {
            if *required && map.columns.get(*m).is_none_or(|h| h.trim().is_empty()) {
                return Err(format!("the column of `{m}` is missing"));
            }
        }
        Ok(map)
    }

    /// The map as TOML.
    pub fn to_toml(&self) -> String {
        toml::to_string(self).unwrap_or_default()
    }
}

/// The presets that ship, by name (`devices-de`, `terminals-en`, ...): the headers of EPLAN's report exports in
/// German and English (`data/report-presets.toml`).
pub fn report_presets() -> BTreeMap<String, ReportMap> {
    #[derive(Deserialize)]
    struct File {
        preset: BTreeMap<String, ReportMap>,
    }
    toml::from_str::<File>(include_str!("../data/report-presets.toml"))
        .map(|f| f.preset)
        .unwrap_or_else(|e| unreachable!("the shipped report presets read: {e}"))
}

/// A report spreadsheet read as a table: the header row and the data rows, one cell per header.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReportTable {
    /// The headers.
    pub headers: Vec<String>,
    /// The data rows.
    pub rows: Vec<Vec<String>>,
}

impl ReportTable {
    /// The table of `records` (every row of a sheet with text): the header row is the 1-based `header_row`, else the one
    /// of the first 20 that holds most of the `wanted` headers (the earliest on a tie); the rows after it are the data.
    pub fn from_records(
        records: &[Vec<String>],
        header_row: Option<u32>,
        wanted: &[&str],
    ) -> Option<ReportTable> {
        let at = match header_row {
            Some(n) => usize::try_from(n).ok()?.checked_sub(1)?,
            None => {
                let score = |r: &Vec<String>| {
                    wanted
                        .iter()
                        .filter(|w| r.iter().any(|c| c.trim() == **w))
                        .count()
                };
                let best = records.iter().take(20).map(score).max()?;
                records.iter().take(20).position(|r| score(r) == best)?
            }
        };
        let headers: Vec<String> = records.get(at)?.iter().map(|h| h.trim().to_owned()).collect();
        let rows = records[at + 1..].iter().map(|r| r.to_vec()).collect();
        Some(ReportTable { headers, rows })
    }
}

/// The findings of one import, capped.
struct Notes {
    file: String,
    entries: Vec<ReportEntry>,
    suppressed: BTreeMap<Code, u32>,
}

impl Notes {
    fn push(&mut self, code: Code, severity: Severity, line: usize, message: String) {
        if self.entries.len() >= MAX_ENTRIES {
            *self.suppressed.entry(code).or_default() += 1;
            return;
        }
        self.entries
            .push(ReportEntry::new(code, severity, message).at(SourceLocation {
                file: self.file.clone(),
                line: u32::try_from(line).ok(),
                ..SourceLocation::default()
            }));
    }
}

/// Record IDs of the import.
#[derive(Clone, Copy)]
struct Ids {
    namespace: Uuid,
    lineage: Id<ImportLineage>,
}

impl Ids {
    fn id<T>(&self, key: &str) -> Id<T> {
        v5::imported_id(self.namespace, key)
    }
    fn source_ref(&self, key: &str) -> SourceRef {
        SourceRef {
            lineage: self.lineage,
            key: key.to_owned(),
            extra: BTreeMap::new(),
        }
    }
}

/// The product designation of a designation such as `-X1031`, `=P01+J-E-2416-X1031` or `X1031`: after the last `-`,
/// else the text itself; the part before is the structure prefix.
fn split_dt(text: &str) -> (&str, &str) {
    let t = text.trim();
    match t.rfind('-') {
        Some(i) if i + 1 < t.len() => (&t[..i], &t[i + 1..]),
        _ => (t.trim_start_matches('-'), t.trim_start_matches('-')),
    }
}

fn ident(text: &str) -> Option<Ident> {
    Ident::from_input(text.trim()).ok()
}

/// The frozen tag of a product designation: its letters as the letter code, the designation as printed.
fn tag_of(product: &str) -> Option<TagSpec> {
    let letters: String = product.chars().take_while(|c| c.is_alphabetic()).collect();
    let letter = ident(if letters.is_empty() { "X" } else { &letters })?;
    Some(TagSpec {
        letter,
        counter: None,
        scheme: Default::default(),
        frozen: Some(FrozenTag {
            text: ident(product)?,
            sign: Default::default(),
            extra: BTreeMap::new(),
        }),
        extra: BTreeMap::new(),
    })
}

fn find<'t>(row: &'t [String], headers: &[String], map: &ReportMap, member: &str) -> &'t str {
    map.columns
        .get(member)
        .and_then(|h| headers.iter().position(|x| x == h.trim()))
        .and_then(|i| row.get(i))
        .map_or("", |c| c.trim())
}

/// Reads `table` through `map` into the records of a project without drawings: `source` names the file (never a path),
/// `key` the lineage (the map's key: the same report of another date is the same lineage), `sha256` the file's hash,
/// `fndefs` the starter function definitions the records name (`ew.fn.terminal`, `ew.fn.terminal-pe`, `ew.fn.blackbox`).
///
/// # Errors
///
/// [`ImportError::Malformed`] when a column of the map is not in the table's headers or a function definition is
/// missing; item-level problems are report entries.
pub fn read_report(
    table: &ReportTable,
    map: &ReportMap,
    source: (&str, [u8; 32], u64),
    key: &str,
    fndefs: &[LibDocument],
) -> Result<ImportBatch, ImportError> {
    let (file, sha256, size) = source;
    for (member, header) in &map.columns {
        if !table.headers.iter().any(|h| h == header.trim()) {
            return Err(ImportError::Malformed {
                message: format!("the column `{header}` ({member}) is not in the table"),
                line: None,
            });
        }
    }
    let def = |k: &str| -> Result<(Id<ew_interop_api::ew_schema::FnDef>, LibDocument), ImportError> {
        fndefs
            .iter()
            .find_map(|d| match d {
                LibDocument::Fndef(c) if c.item.meta.key.as_ref().is_some_and(|m| m.as_str() == k) => {
                    Some((c.id, d.clone()))
                }
                _ => None,
            })
            .ok_or_else(|| ImportError::Malformed {
                message: format!("the function definition `{k}` is missing"),
                line: None,
            })
    };
    let (terminal, terminal_doc) = def("ew.fn.terminal")?;
    let (terminal_pe, pe_doc) = def("ew.fn.terminal-pe")?;
    let (blackbox, box_doc) = def("ew.fn.blackbox")?;
    let namespace =
        v5::lineage_namespace(REPORT_IMPORTER_ID, &v5::LineageAnchor::Fixed(key)).map_err(|_| {
            ImportError::Malformed {
                message: "the map's key is not a name".into(),
                line: None,
            }
        })?;
    let lineage_id: Id<ImportLineage> = v5::imported_id(namespace, "eplan:lineage");
    let ids = Ids {
        namespace,
        lineage: lineage_id,
    };
    let mut notes = Notes {
        file: file.to_owned(),
        entries: Vec::new(),
        suppressed: BTreeMap::new(),
    };
    for h in &table.headers {
        if !h.is_empty() && !map.columns.values().any(|m| m.trim() == h) {
            notes.push(
                Code::imp(206),
                Severity::Warning,
                1,
                format!("the column `{h}` is not mapped: its values are not imported"),
            );
        }
    }
    let mut records: Vec<Record> = Vec::new();
    let mut used: Vec<LibDocument> = Vec::new();
    // Terminals by (strip designation, terminal designation) → the function and the sides' CP.
    let mut answers: BTreeMap<(String, String), Id<Function>> = BTreeMap::new();
    let (mut dropped, mut folded) = (0u32, 0u32);
    match map.target {
        ReportTarget::Devices => {
            used.push(box_doc);
            for (n, row) in table.rows.iter().enumerate() {
                let line = n + 2;
                let designation = find(row, &table.headers, map, "designation");
                if designation.is_empty() {
                    continue;
                }
                let (structure, product) = split_dt(designation);
                let Some(tag) = tag_of(product) else {
                    notes.push(
                        Code::MALFORMED_ITEM,
                        Severity::Error,
                        line,
                        format!("`{designation}` is not a designation: row dropped"),
                    );
                    dropped += 1;
                    continue;
                };
                let dkey = format!("eplan:device:{designation}");
                if records.iter().any(|r| matches!(r, Record::Device(d) if d.source_ref.as_ref().is_some_and(|s| s.key == dkey))) {
                    notes.push(Code::DROPPED_BY_DESIGN, Severity::Info, line, format!("`{designation}` is listed twice: kept once"));
                    continue;
                }
                let mut extra = Extra::new();
                if !structure.is_empty() {
                    extra.insert("eplan.structure".into(), serde_json::json!(structure));
                }
                let part = find(row, &table.headers, map, "part");
                if !part.is_empty() {
                    extra.insert("eplan.part".into(), serde_json::json!(part));
                }
                let device: Id<Device> = ids.id(&dkey);
                records.push(Record::Device(Device {
                    id: device,
                    v: 1,
                    tag,
                    kind: None,
                    parent: None,
                    position: None,
                    structure: Default::default(),
                    parts: Vec::new(),
                    function_text: MlText::neutral(find(row, &table.headers, map, "description")),
                    process_tag: None,
                    tech_data: Default::default(),
                    state: Default::default(),
                    source_ref: Some(ids.source_ref(&dkey)),
                    aspects: DeviceAspects::default(),
                    extra,
                }));
                records.push(Record::Function(function(&ids, &dkey, device, blackbox)));
            }
        }
        ReportTarget::Terminals | ReportTarget::Connections => {
            // The terminals of the file: from a terminal diagram, or for a connection list the terminals its ends name.
            used.push(terminal_doc);
            used.push(pe_doc);
            let mut seq: BTreeMap<String, u32> = BTreeMap::new();
            let mut strips: BTreeMap<String, Id<Device>> = BTreeMap::new();
            let mut last: Option<(String, String, u8)> = None;
            let mut rows: Vec<(usize, String, String, u8)> = Vec::new();
            match map.target {
                ReportTarget::Terminals => {
                    for (n, row) in table.rows.iter().enumerate() {
                        let (strip, designation) = (
                            find(row, &table.headers, map, "strip"),
                            find(row, &table.headers, map, "designation"),
                        );
                        if strip.is_empty() && designation.is_empty() {
                            continue;
                        }
                        let level = find(row, &table.headers, map, "level").parse::<u8>().unwrap_or(1);
                        rows.push((n + 2, strip.to_owned(), designation.to_owned(), level));
                    }
                }
                _ => {
                    for (n, row) in table.rows.iter().enumerate() {
                        for member in ["from", "to"] {
                            // Only a terminal strip ("X…") names terminals; a device pin is no terminal.
                            if let Some((s, d)) = find(row, &table.headers, map, member).rsplit_once(':')
                                && split_dt(s).1.starts_with(['X', 'x'])
                            {
                                rows.push((n + 2, s.trim().to_owned(), d.trim().to_owned(), 1));
                            }
                        }
                    }
                }
            }
            for (line, strip, designation, level) in rows {
                let (structure, product) = split_dt(&strip);
                let (Some(tag), Some(desig)) = (tag_of(product), ident(&designation)) else {
                    notes.push(
                        Code::MALFORMED_ITEM,
                        Severity::Error,
                        line,
                        format!("`{strip}:{designation}` is not a strip and a terminal: row dropped"),
                    );
                    dropped += 1;
                    continue;
                };
                let skey = format!("eplan:strip:{strip}");
                let strip_id = *strips.entry(strip.clone()).or_insert_with(|| {
                    let id: Id<Device> = ids.id(&skey);
                    let mut extra = Extra::new();
                    if !structure.is_empty() {
                        extra.insert("eplan.structure".into(), serde_json::json!(structure));
                    }
                    records.push(Record::Device(Device {
                        id,
                        v: 1,
                        tag,
                        kind: None,
                        parent: None,
                        position: None,
                        structure: Default::default(),
                        parts: Vec::new(),
                        function_text: MlText::default(),
                        process_tag: None,
                        tech_data: Default::default(),
                        state: Default::default(),
                        source_ref: Some(ids.source_ref(&skey)),
                        aspects: DeviceAspects {
                            strip: Some(Strip {
                                label_mode: Default::default(),
                                designation_scheme: None,
                                accessories: Vec::new(),
                                tag_print: Default::default(),
                                groups: Vec::new(),
                                field_side: Side::Side2,
                                extra: BTreeMap::new(),
                            }),
                            ..DeviceAspects::default()
                        },
                        extra,
                    }));
                    id
                });
                // Rising levels of one designation fold into the terminal they continue (EW-IMP-207).
                let previous = last.clone();
                if let Some((s, d, l)) = previous
                    && s == strip
                    && d == designation
                    && level > l
                {
                    let tkey = format!("eplan:terminal:{strip}:{designation}");
                    for r in &mut records {
                        if let Record::Device(dev) = r
                            && dev.source_ref.as_ref().is_some_and(|x| x.key == tkey)
                            && let Some(t) = dev.aspects.terminal.as_mut()
                        {
                            if t.levels.is_empty() {
                                t.levels.push(LevelSpec {
                                    level: l,
                                    role: Default::default(),
                                    extra: BTreeMap::new(),
                                });
                            }
                            t.levels.push(LevelSpec {
                                level,
                                role: Default::default(),
                                extra: BTreeMap::new(),
                            });
                        }
                    }
                    folded += 1;
                    notes.push(
                        Code::imp(207),
                        Severity::Info,
                        line,
                        format!("`{strip}:{designation}` level {level} folded into one multi-level terminal"),
                    );
                    last = Some((strip, designation, level));
                    continue;
                }
                last = Some((strip.clone(), designation.clone(), level));
                let tkey = format!("eplan:terminal:{strip}:{designation}");
                if answers.contains_key(&(strip.clone(), designation.clone())) {
                    continue;
                }
                let n = seq.entry(strip.clone()).or_insert(0);
                *n += 1;
                let device: Id<Device> = ids.id(&tkey);
                let pe = designation.eq_ignore_ascii_case("PE");
                records.push(Record::Device(Device {
                    id: device,
                    v: 1,
                    tag: TagSpec::new(ident("X").unwrap_or(desig.clone()), None),
                    kind: Some(DeviceKind::Terminal),
                    parent: None,
                    position: None,
                    structure: Default::default(),
                    parts: Vec::new(),
                    function_text: MlText::default(),
                    process_tag: None,
                    tech_data: Default::default(),
                    state: Default::default(),
                    source_ref: Some(ids.source_ref(&tkey)),
                    aspects: DeviceAspects {
                        terminal: Some(Terminal {
                            strip: strip_id,
                            seq: *n,
                            designation: desig,
                            levels: Vec::new(),
                            rail_bonded: pe,
                            extra: BTreeMap::new(),
                        }),
                        ..DeviceAspects::default()
                    },
                    extra: Extra::new(),
                }));
                let f = function(&ids, &tkey, device, if pe { terminal_pe } else { terminal });
                answers.insert((strip, designation), f.id);
                records.push(Record::Function(f));
            }
            if map.target == ReportTarget::Connections {
                let mut pairs = 0u32;
                for (n, row) in table.rows.iter().enumerate() {
                    let line = n + 2;
                    let (from, to) = (
                        find(row, &table.headers, map, "from"),
                        find(row, &table.headers, map, "to"),
                    );
                    if from.is_empty() && to.is_empty() {
                        continue;
                    }
                    let end = |text: &str| -> Option<Id<Function>> {
                        let (s, d) = text.rsplit_once(':')?;
                        answers.get(&(s.trim().to_owned(), d.trim().to_owned())).copied()
                    };
                    let (Some(a), Some(b)) = (end(from), end(to)) else {
                        notes.push(
                            Code::UNRESOLVED_REFERENCE,
                            Severity::Warning,
                            line,
                            format!("`{from}` to `{to}`: an end is no terminal of this list (a device's connection points need its part): connection dropped"),
                        );
                        dropped += 1;
                        continue;
                    };
                    if a == b {
                        notes.push(
                            Code::MALFORMED_ITEM,
                            Severity::Warning,
                            line,
                            format!("`{from}` to itself: dropped"),
                        );
                        dropped += 1;
                        continue;
                    }
                    let ckey = format!("eplan:connection:{from}>{to}");
                    if records.iter().any(|r| matches!(r, Record::Connection(c) if c.source_ref.as_ref().is_some_and(|s| s.key == ckey))) {
                        continue;
                    }
                    pairs += 1;
                    records.push(Record::Connection(Connection {
                        id: ids.id(&ckey),
                        v: 1,
                        a: Endpoint::Cp(a, 0),
                        b: Endpoint::Cp(b, 1),
                        page: None,
                        route: Vec::new(),
                        kind: ConnectionKind::default(),
                        overrides: WireOverrides::default(),
                        number: NumberSpec::default(),
                        source_ref: Some(ids.source_ref(&ckey)),
                        extra: Extra::new(),
                    }));
                }
                if pairs > 0 {
                    notes.push(
                        Code::DROPPED_BY_DESIGN,
                        Severity::Info,
                        0,
                        "a connection joins the first side of its first terminal and the second side of its other: the sides are not in the list".to_owned(),
                    );
                }
            }
        }
    }
    let lineage = ImportLineage {
        id: lineage_id,
        v: 1,
        importer: ident(REPORT_IMPORTER_ID).unwrap_or_else(|| unreachable!("a valid importer name")),
        source_name: file.to_owned(),
        anchor: LineageAnchor::Fixed(ident(key).ok_or_else(|| ImportError::Malformed {
            message: "the map's key is not a name".into(),
            line: None,
        })?),
        namespace: Id::from_uuid(namespace),
        last_source_sha256: Some(ew_interop_api::ew_schema::Sha256(sha256)),
        last_run: None,
        extra: BTreeMap::new(),
    };
    records.push(Record::ImportLineage(lineage));
    records.push(Record::Project(Project {
        id: ids.id(PROJECT_KEY),
        v: 1,
        master: MasterData::default(),
        parties: Vec::new(),
        supply: None,
        control_voltage: None,
        rated_consumption: None,
        settings: Default::default(),
        languages: None,
        derivation_profile: ident("2027.1").unwrap_or_else(|| unreachable!("a valid profile key")),
        kind: Default::default(),
        source_ref: Some(ids.source_ref(PROJECT_KEY)),
        basic_project: None,
        extra: Extra::new(),
    }));
    records.sort_by_key(|r| (r.kind().and_then(|k| k.rank()), r.id()));
    used.sort_by_key(|d| match d {
        LibDocument::Fndef(c) => c.id.as_uuid(),
        _ => Uuid::nil(),
    });
    used.dedup_by_key(|d| match d {
        LibDocument::Fndef(c) => c.id.as_uuid(),
        _ => Uuid::nil(),
    });
    let mut report = ImportReport::new(
        REPORT_IMPORTER_ID,
        REPORT_IMPORTER_VERSION,
        SourceInfo {
            name: file.to_owned(),
            sha256,
            size,
            format: "eplan-report".to_owned(),
            format_version: None,
            detail: Some(format!("{:?}", map.target)),
        },
    );
    for r in &records {
        *report
            .stats
            .per_kind
            .entry(r.kind_name().unwrap_or("?").to_owned())
            .or_default() += 1;
    }
    report.stats.dropped = dropped;
    report.stats.inferred = folded;
    report.entries = notes.entries;
    report.stats.suppressed = notes.suppressed;
    Ok(ImportBatch {
        records,
        lib: used,
        assets: Vec::new(),
        report,
    })
}

/// A main function of `device` of the function definition `def`.
fn function(ids: &Ids, key: &str, device: Id<Device>, def: Id<ew_interop_api::ew_schema::FnDef>) -> Function {
    Function {
        id: ids.id(&format!("{key}#function")),
        v: 1,
        device: Some(device),
        def,
        role: FnRole::Main,
        kind: FunctionKind::default(),
        cp_overrides: Default::default(),
        source_ref: Some(ids.source_ref(&format!("{key}#function"))),
        template_entry: None,
        bus_port: None,
        extra: Extra::new(),
    }
}
