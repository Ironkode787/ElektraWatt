// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! [`parse_parts`]: a mapped parts table into part documents, the new manufacturers they name, and the row report
//! (FR-PRT-016).

use std::collections::BTreeMap;

use ew_interop_api::ew_ids::v5::{self, LineageAnchor};
use ew_interop_api::ew_ids::{Id, Uuid};
use ew_interop_api::ew_schema::{
    Date, LibCopy, LibDocument, LibItem, Library, Manufacturer, Part, Sha256, Unit, normalize_nfc,
};
use ew_interop_api::pn_norm;
use ew_interop_api::{
    Code, ImportError, ImportInput, ImportReport, Limits, PartDraft, ReportEntry, Severity, SourceInfo,
    SourceLocation,
};
use serde::Serialize;
use serde_json::{Map, Value, json};
use sha2::Digest;

use crate::map::{ColumnMap, Member};
use crate::table::{self, Cell, Sheet, SourceFormat};

/// The importer's ID: the `ImportLineage.importer` of its namespace and the report's importer.
pub const IMPORTER_ID: &str = "catalog";

/// The importer's version, recorded in the report; bumped on every mapping change.
pub const IMPORTER_VERSION: &str = "1";

/// The version of a part or manufacturer the target does not hold yet.
pub const FIRST_VERSION: &str = "1.0.0";

/// The namespace of the item IDs a parts import mints for library `library` (ADR-0004 amendment 4): the lineage of
/// importer [`IMPORTER_ID`] anchored on the library ID, so a re-import of a list updates its items in place.
pub fn namespace(library: Uuid) -> Uuid {
    v5::lineage_namespace(
        IMPORTER_ID,
        &LineageAnchor::Fixed(&library.hyphenated().to_string()),
    )
    .unwrap_or_else(|_| unreachable!("the importer ID is a valid lineage name"))
}

/// The item ID of the part `pn_norm` of manufacturer `key` in library `library`:
/// `v5(namespace(library), "parts|<key>|<pn_norm>")`.
pub fn part_item(library: Uuid, key: &str, pn_norm: &str) -> Uuid {
    v5::imported_id::<()>(namespace(library), &format!("parts|{key}|{pn_norm}")).as_uuid()
}

/// The item ID of a manufacturer the import creates in library `library`: `v5(namespace(library),
/// "manufacturers|<key>")`.
pub fn manufacturer_item(library: Uuid, key: &str) -> Uuid {
    v5::imported_id::<()>(namespace(library), &format!("manufacturers|{key}")).as_uuid()
}

fn no_existing(_: Uuid) -> Option<LibCopy<Part>> {
    None
}

/// Where the parts go, and what is there already.
pub struct CatalogTarget<'a> {
    /// The library's ID: a writable library, or the project's ID for project-local copies (`parts.import`).
    pub library: Uuid,
    /// The import date, stamped on new and changed parts (`meta.as_of`); a parameter, never the clock (doc 07 §3.6).
    pub as_of: Option<Date>,
    /// Give a changed part the next patch version instead of keeping the held one (`--bump`): a library refuses other
    /// content under a published version (doc 05 §13.1).
    pub bump: bool,
    /// The input limits.
    pub limits: Limits,
    /// The target's current part of an item ID, if it holds one.
    pub existing: &'a dyn Fn(Uuid) -> Option<LibCopy<Part>>,
    /// Give every manufacturer a master record of another library resolves to a copy in the target (a project, whose
    /// parts name embedded manufacturer copies): item [`manufacturer_item`]`(library, key)`, version
    /// [`FIRST_VERSION`], the record's content; the parts name the copy. Without it the parts name the master record.
    pub local_manufacturers: bool,
    /// With [`CatalogTarget::local_manufacturers`], the libraries whose master records the parts name directly
    /// instead of a copy: a library target's manifest `depends`.
    pub shared: &'a [Uuid],
}

impl CatalogTarget<'static> {
    /// A target `library` that holds no part yet, without a date, no bump, the table limits
    /// ([`crate::table_limits`]: cells of at most 32 KiB), the parts naming the master's manufacturer records.
    pub fn new(library: Uuid) -> CatalogTarget<'static> {
        CatalogTarget {
            library,
            as_of: None,
            bump: false,
            limits: crate::table_limits(),
            existing: &no_existing,
            local_manufacturers: false,
            shared: &[],
        }
    }
}

/// What became of one part of a row.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RowStatus {
    /// A part the target does not hold.
    New,
    /// A part the target holds with other content. `version` is the held one unless the import bumps it; a library
    /// write of an unbumped change is refused ([`RowOutcome::needs_bump`]).
    Updated {
        /// The held version.
        from: String,
        /// The version the part is written with.
        version: String,
    },
    /// A part the target holds with the same content (the import date and source aside); written as held.
    Unchanged,
    /// The row names the same manufacturer and part number as an earlier row, with the same content.
    Duplicate {
        /// The earlier row's line.
        of_line: u32,
    },
    /// The row was not imported, with the reason.
    Rejected(RowRefusal),
}

/// Why a row was not imported. Its text (`Display`) is the report's, in English; a window says each in its own words.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RowRefusal {
    /// The row has no part number.
    NoPartNumber,
    /// The row has no manufacturer.
    NoManufacturer,
    /// A mapped column's cell holds a spreadsheet error.
    ErrorCell {
        /// The column's header.
        column: String,
        /// The error the cell holds (`#DIV/0!`).
        error: String,
    },
    /// A value of the row does not read, in the reader's words (the column, the cell and why).
    Value(String),
    /// The manufacturer names several records of the master list.
    Ambiguous {
        /// The cell's text.
        maker: String,
        /// The keys of the records it names.
        keys: Vec<String>,
    },
    /// The manufacturer has no letters or digits.
    MakerWithoutLetters(String),
    /// The part number has no letters or digits.
    NumberWithoutLetters(String),
    /// The row repeats an earlier row's manufacturer and part number with other content.
    Repeats {
        /// The earlier row's line.
        line: u32,
    },
    /// The held version of the part cannot be bumped.
    NoNextVersion(String),
    /// The host refused the row's part (its own words are its own; here, in English).
    Host(String),
}

impl std::fmt::Display for RowRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RowRefusal::NoPartNumber => f.write_str("no part number"),
            RowRefusal::NoManufacturer => f.write_str("no manufacturer"),
            RowRefusal::ErrorCell { column, error } => {
                write!(f, "column `{column}`: the cell holds the error {error}")
            }
            RowRefusal::Value(why) | RowRefusal::Host(why) => f.write_str(why),
            RowRefusal::Ambiguous { maker, keys } => write!(
                f,
                "the manufacturer `{maker}` is ambiguous: it names {}",
                keys.join(", ")
            ),
            RowRefusal::MakerWithoutLetters(t) => write!(f, "the manufacturer `{t}` has no letters or digits"),
            RowRefusal::NumberWithoutLetters(t) => write!(f, "the part number `{t}` has no letters or digits"),
            RowRefusal::Repeats { line } => write!(
                f,
                "same manufacturer and part number as line {line}, with other content"
            ),
            RowRefusal::NoNextVersion(v) => write!(f, "the held version `{v}` cannot be bumped"),
        }
    }
}

/// One part of a row: a row is one part, or one candidate of an "A / B" part number.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RowOutcome {
    /// The row's line (CSV) or row number (XLSX).
    pub line: u32,
    /// The part number, as the row has it (one candidate of a split cell).
    pub part_number: String,
    /// The part's item ID, once it is known.
    pub item: Option<Uuid>,
    /// One of the candidates of an "A / B" part number (FR-PRT-004), flagged for review.
    pub candidate: bool,
    /// What became of it.
    pub status: RowStatus,
}

impl RowOutcome {
    /// Whether the part changed under its held version: a library write refuses it unless the import bumps it.
    pub fn needs_bump(&self) -> bool {
        matches!(&self.status, RowStatus::Updated { from, version } if from == version)
    }
}

/// The counts of a parts import, by status.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct RowCounts {
    /// New parts.
    pub new: usize,
    /// Updated parts.
    pub updated: usize,
    /// Unchanged parts.
    pub unchanged: usize,
    /// Duplicate rows.
    pub duplicate: usize,
    /// Rejected rows (or candidates).
    pub rejected: usize,
}

/// The result of [`parse_parts`].
#[derive(Clone, PartialEq, Debug)]
pub struct PartsImport {
    /// The part documents, new, updated and unchanged, in row order.
    pub parts: Vec<LibDocument>,
    /// The manufacturer copies the parts name in the target, by item ID: the manufacturers the import creates (names
    /// no master record matched) and, with [`CatalogTarget::local_manufacturers`], the copies of the master records
    /// the parts resolve to.
    pub manufacturers: Vec<LibDocument>,
    /// One outcome per part of a row (two for an "A / B" part number), in row order; empty rows are skipped.
    pub rows: Vec<RowOutcome>,
    /// The report: unmapped columns, rejected rows, split part numbers, aliases and new manufacturers.
    pub report: ImportReport,
}

impl PartsImport {
    /// The counts by status.
    pub fn counts(&self) -> RowCounts {
        let mut c = RowCounts::default();
        for r in &self.rows {
            match r.status {
                RowStatus::New => c.new += 1,
                RowStatus::Updated { .. } => c.updated += 1,
                RowStatus::Unchanged => c.unchanged += 1,
                RowStatus::Duplicate { .. } => c.duplicate += 1,
                RowStatus::Rejected(_) => c.rejected += 1,
            }
        }
        c
    }

    /// The rows whose part changed under its held version (see [`RowOutcome::needs_bump`]).
    pub fn needs_bump(&self) -> Vec<&RowOutcome> {
        self.rows.iter().filter(|r| r.needs_bump()).collect()
    }
}

/// The SHA-256 of an item's compact JSON (doc 05 §13.2, EW-CORE-014).
fn item_sha256<T: Serialize>(item: &T) -> Sha256 {
    let json = serde_json::to_string(item).unwrap_or_default();
    Sha256(sha2::Sha256::digest(json.as_bytes()).into())
}

/// A copy of `item` in `library` as the item `item_id` at `version`.
fn copy<T: Serialize>(library: Uuid, item_id: Uuid, version: &str, item: T) -> LibCopy<T> {
    LibCopy {
        id: v5::lib_copy_id(library, item_id),
        v: 1,
        library_id: Id::<Library>::from_uuid(library),
        item_id: Id::<LibItem>::from_uuid(item_id),
        version: version.to_owned(),
        sha256: item_sha256(&item),
        item,
    }
}

/// `^MAJOR` for a version 1 or above, `^0.MINOR` below (the requirement forms of library references).
fn caret(version: &str) -> String {
    let mut parts = version.split('.');
    match (parts.next(), parts.next()) {
        (Some("0"), Some(minor)) => format!("^0.{minor}"),
        (Some(major), _) => format!("^{major}"),
        _ => "*".to_owned(),
    }
}

/// A cell's text as the format stores text: trimmed, NFC.
fn clean(s: &str) -> String {
    let mut v = Value::String(s.trim().to_owned());
    normalize_nfc(&mut v);
    match v {
        Value::String(s) => s,
        _ => unreachable!("a string stays a string"),
    }
}

/// The comparison form of a name: NFC, lower case, inner white space collapsed.
fn fold(s: &str) -> String {
    clean(s)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The texts of a multi-language text's JSON (a neutral string or an object of languages).
fn ml_texts(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => vec![s.clone()],
        Value::Object(m) => m.values().filter_map(|v| v.as_str().map(str::to_owned)).collect(),
        _ => Vec::new(),
    }
}

/// How a manufacturer cell resolved.
#[derive(Clone)]
struct Maker {
    key: String,
    library: Uuid,
    item: Uuid,
    version: String,
}

/// The manufacturer master as lookup tables, plus the manufacturers this import creates.
struct Makers<'m> {
    /// Folded key or name → the records that have it.
    direct: BTreeMap<String, Vec<usize>>,
    /// Folded alias or brand → the records that have it.
    alias: BTreeMap<String, Vec<usize>>,
    master: &'m [LibCopy<Manufacturer>],
    /// The target library, and whether a master record of another library becomes a copy in it.
    library: Uuid,
    local: bool,
    /// The libraries whose records the parts name directly even when `local`.
    shared: &'m [Uuid],
    /// Folded name → the created manufacturer.
    created: BTreeMap<String, Maker>,
    /// Master index → its copy in the target.
    copied: BTreeMap<usize, Maker>,
    /// The manufacturer copies of the target, created or copied, by item ID.
    emitted: BTreeMap<Uuid, LibCopy<Manufacturer>>,
    /// Cell texts already reported (alias, new).
    reported: BTreeMap<String, ()>,
}

impl<'m> Makers<'m> {
    fn new(
        master: &'m [LibCopy<Manufacturer>],
        library: Uuid,
        local: bool,
        shared: &'m [Uuid],
    ) -> Makers<'m> {
        let mut direct: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut alias: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, m) in master.iter().enumerate() {
            let name = serde_json::to_value(&m.item.name).unwrap_or_default();
            let mut names: Vec<String> = ml_texts(&name);
            names.push(m.item.key.to_string());
            for n in names {
                let v = direct.entry(fold(&n)).or_default();
                if !v.contains(&i) {
                    v.push(i);
                }
            }
            for a in m.item.aliases.iter().chain(&m.item.brands) {
                let v = alias.entry(fold(a.as_ref())).or_default();
                if !v.contains(&i) {
                    v.push(i);
                }
            }
        }
        Makers {
            direct,
            alias,
            master,
            library,
            local,
            shared,
            created: BTreeMap::new(),
            copied: BTreeMap::new(),
            emitted: BTreeMap::new(),
            reported: BTreeMap::new(),
        }
    }

    /// The manufacturer of master record `i`: the record itself, or its copy in the target (made on first use) when
    /// the target keeps local copies and the record is of another library.
    fn maker(&mut self, i: usize) -> Maker {
        let master = self.master;
        let m = &master[i];
        let key = m.item.key.to_string();
        let lib = m.library_id.as_uuid();
        if !self.local || lib == self.library || self.shared.contains(&lib) {
            return Maker {
                key,
                library: m.library_id.as_uuid(),
                item: m.item_id.as_uuid(),
                version: m.version.clone(),
            };
        }
        if let Some(made) = self.copied.get(&i) {
            return made.clone();
        }
        let item = manufacturer_item(self.library, &key);
        let made = Maker {
            key,
            library: self.library,
            item,
            version: FIRST_VERSION.to_owned(),
        };
        self.emitted
            .entry(item)
            .or_insert_with(|| copy(self.library, item, FIRST_VERSION, m.item.clone()));
        self.copied.insert(i, made.clone());
        made
    }

    /// Resolves a manufacturer cell: a master record's key or name, then its aliases and brands (EW-IMP-007), then a
    /// manufacturer this import created, else a new one (EW-IMP-005). Two master records for one text reject the row.
    fn resolve(
        &mut self,
        text: &str,
        library: Uuid,
        report: &mut ImportReport,
        at: &SourceLocation,
    ) -> Result<Maker, RowRefusal> {
        let f = fold(text);
        let own = self.library;
        let shared = self.shared;
        let local = self.local;
        let pick = |hits: &[usize], master: &[LibCopy<Manufacturer>]| -> Result<usize, RowRefusal> {
            // The target's own record wins over another library's of the same name (the copy a first import
            // wrote into the target beside the master record it copies), then a single record of a library the
            // target depends on; when the parts name a copy anyway and every hit has the same key, the first hit
            // in master order is copied (the copies other imports wrote are not a choice the user has to make).
            let of = |keep: &dyn Fn(Uuid) -> bool| -> Vec<usize> {
                hits.iter()
                    .copied()
                    .filter(|i| keep(master[*i].library_id.as_uuid()))
                    .collect()
            };
            let mine = of(&|l| l == own);
            let depended = of(&|l| shared.contains(&l));
            let one_key = hits
                .windows(2)
                .all(|w| master[w[0]].item.key == master[w[1]].item.key);
            match (hits, mine.as_slice(), depended.as_slice()) {
                ([one], _, _) | (_, [one], _) | (_, [], [one]) => Ok(*one),
                ([first, ..], [], []) if local && one_key => Ok(*first),
                _ => Err(RowRefusal::Ambiguous {
                    maker: text.to_owned(),
                    keys: hits.iter().map(|i| master[*i].item.key.to_string()).collect(),
                }),
            }
        };
        if let Some(hits) = self.direct.get(&f) {
            let i = pick(hits, self.master)?;
            return Ok(self.maker(i));
        }
        if let Some(hits) = self.alias.get(&f) {
            let i = pick(hits, self.master)?;
            let m = self.maker(i);
            if self.reported.insert(format!("alias|{f}"), ()).is_none() {
                let name = self.master[i].item.name.clone();
                let name = ml_texts(&serde_json::to_value(name).unwrap_or_default())
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| m.key.clone());
                report.push(
                    ReportEntry::new(
                        Code::ALIAS_MAPPED,
                        Severity::Info,
                        format!(
                            "manufacturer `{text}` mapped to {name} ({}) through its aliases",
                            m.key
                        ),
                    )
                    .at(at.clone()),
                );
            }
            return Ok(m);
        }
        if let Some(m) = self.created.get(&f) {
            return Ok(m.clone());
        }
        let name = clean(text);
        let key = pn_norm(&name);
        if key.is_empty() {
            return Err(RowRefusal::MakerWithoutLetters(text.to_owned()));
        }
        let item = manufacturer_item(library, &key);
        let maker = Maker {
            key: key.clone(),
            library,
            item,
            version: FIRST_VERSION.to_owned(),
        };
        if self.emitted.contains_key(&item) {
            // Another text with the same key (`A-B`, `AB`): the copy made for it.
            self.created.insert(f, maker.clone());
            return Ok(maker);
        }
        let doc: Manufacturer = serde_json::from_value(json!({"key": key, "name": name}))
            .map_err(|e| RowRefusal::Value(format!("the manufacturer `{text}` cannot be recorded: {e}")))?;
        report.push(
            ReportEntry::new(
                Code::imp(5),
                Severity::Warning,
                format!(
                    "manufacturer `{name}` is not in the master list: recorded as a new manufacturer `{key}`; review"
                ),
            )
            .at(at.clone()),
        );
        report.stats.inferred += 1;
        self.emitted.insert(item, copy(library, item, FIRST_VERSION, doc));
        self.created.insert(f, maker.clone());
        Ok(maker)
    }
}

/// A decimal in the canonical `Dec3` form: `,` or `.` as the separator, at most three decimals, no needless zeros.
fn decimal(s: &str) -> Option<String> {
    let (neg, digits) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    let digits = digits.replace(',', ".");
    let (int, frac) = digits.split_once('.').unwrap_or((&digits, ""));
    let ok = |p: &str| p.bytes().all(|b| b.is_ascii_digit());
    if int.is_empty() && frac.is_empty() || !ok(int) || !ok(frac) || frac.len() > 3 {
        return None;
    }
    let int = int.trim_start_matches('0');
    let frac = frac.trim_end_matches('0');
    let int = if int.is_empty() { "0" } else { int };
    let mut out = String::new();
    if neg && (int != "0" || !frac.is_empty()) {
        out.push('-');
    }
    out.push_str(int);
    if !frac.is_empty() {
        out.push('.');
        out.push_str(frac);
    }
    Some(out)
}

/// A technical value: `<number> <unit>` (the space optional, `,` or `.` as the separator; the unit by its spelling or
/// its symbol, `Unit::named`) as a quantity, else an identifier. A number with a unit the format does not know does
/// not parse.
fn tech_value(key: &str, cell: &str) -> Result<Value, String> {
    let split = cell
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | ',' | '-')))
        .unwrap_or(cell.len());
    let (number, unit) = cell.split_at(split);
    let unit = unit.trim();
    let value = match decimal(number) {
        Some(n) if !unit.is_empty() => {
            let Some(u) = Unit::named(unit) else {
                return Err(format!("`{cell}`: `{unit}` is not a unit of the format"));
            };
            json!({"value": n, "unit": u.spelling()})
        }
        _ => Value::String(cell.to_owned()),
    };
    let probe = json!({"key": key, "value": value});
    serde_json::from_value::<ew_interop_api::ew_schema::TechValue>(probe.clone())
        .map_err(|e| format!("`{cell}` is not a technical value: {e}"))?;
    Ok(probe)
}

/// The part kinds by name.
const KINDS: &[&str] = &[
    "device",
    "terminal",
    "cable",
    "wire",
    "ferrule",
    "marker",
    "duct",
    "rail",
    "busbar",
    "end_bracket",
    "enclosure",
    "mounting_plate",
    "tube",
    "accessory",
    "configured",
];

/// A part kind cell: a kind's name in any case, spaces or `-` for `_`; `None` for an empty cell (the default).
fn kind(cell: &str) -> Result<Option<&'static str>, String> {
    if cell.is_empty() {
        return Ok(None);
    }
    let k = cell.to_lowercase().replace([' ', '-'], "_");
    KINDS
        .iter()
        .find(|n| **n == k)
        .map(|n| Some(*n))
        .ok_or_else(|| format!("unknown kind `{cell}`"))
}

/// An order unit cell: `piece` (`pc`, `pcs`), `m` (`metre`, `meter`) or `set`; `None` for an empty cell.
fn unit(cell: &str) -> Result<Option<&'static str>, String> {
    Ok(match cell.to_lowercase().as_str() {
        "" => None,
        "piece" | "pc" | "pcs" => Some("piece"),
        "m" | "metre" | "meter" => Some("m"),
        "set" => Some("set"),
        _ => return Err(format!("unknown order unit `{cell}` (piece, m, set)")),
    })
}

/// The candidates of a part number cell: "A / B" (a `/` with white space on both sides) names two parts
/// (FR-PRT-004), never one.
fn candidates(pn: &str) -> Vec<String> {
    let parts: Vec<String> = pn.split(" / ").map(clean).filter(|p| !p.is_empty()).collect();
    if parts.len() > 1 { parts } else { vec![clean(pn)] }
}

/// The columns of the sheet each mapped member reads, with the report entries of unmapped and missing columns.
fn bind(
    sheet: &Sheet,
    map: &ColumnMap,
    file: &str,
    report: &mut ImportReport,
    host: bool,
) -> Result<Vec<(Member, usize)>, ImportError> {
    // Without a host only the plain members are read: any other is refused before a row is.
    if !host && let Some((member, _)) = map.columns().find(|(m, _)| !m.plain()) {
        return Err(ImportError::Malformed {
            message: format!(
                "{file}: the map's member `{member}` is read only where the part's fields are known (the window's Parts)"
            ),
            line: None,
        });
    }
    let mut by_header: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, h) in sheet.headers.iter().enumerate() {
        if !h.is_empty() {
            by_header.entry(h.as_str()).or_default().push(i);
        }
    }
    let mut bound = Vec::new();
    let mut used = vec![false; sheet.headers.len()];
    for (member, header) in map.columns() {
        match by_header.get(header).map(Vec::as_slice) {
            Some([i]) => {
                bound.push((member.clone(), *i));
                used[*i] = true;
            }
            Some(_) => {
                return Err(ImportError::Malformed {
                    message: format!("{file}: the column `{header}` appears more than once"),
                    line: None,
                });
            }
            None if member.required() => {
                return Err(ImportError::Malformed {
                    message: format!("{file}: the map's column `{header}` ({member}) is not in the table"),
                    line: None,
                });
            }
            None => report.push(
                ReportEntry::new(
                    Code::DROPPED_BY_DESIGN,
                    Severity::Warning,
                    format!("the map's column `{header}` ({member}) is not in the table"),
                )
                .at(SourceLocation {
                    file: file.to_owned(),
                    member: Some(header.to_owned()),
                    ..SourceLocation::default()
                }),
            ),
        }
    }
    for (i, h) in sheet.headers.iter().enumerate() {
        if !used[i] && !h.is_empty() {
            report.push(
                ReportEntry::new(
                    Code::DROPPED_BY_DESIGN,
                    Severity::Info,
                    format!("the column `{h}` is not mapped: its values are not imported"),
                )
                .at(SourceLocation {
                    file: file.to_owned(),
                    member: Some(h.clone()),
                    ..SourceLocation::default()
                }),
            );
        }
    }
    if sheet.overhang > 0 {
        report.push(
            ReportEntry::new(
                Code::DROPPED_BY_DESIGN,
                Severity::Info,
                format!(
                    "{} rows have values beyond the last header: they are not imported",
                    sheet.overhang
                ),
            )
            .at(SourceLocation {
                file: file.to_owned(),
                ..SourceLocation::default()
            }),
        );
    }
    Ok(bound)
}

/// One row's values by member, cleaned; an error cell of a mapped column rejects the row.
fn values(
    row: &table::Row,
    bound: &[(Member, usize)],
    headers: &[String],
) -> Result<BTreeMap<Member, String>, RowRefusal> {
    let mut out = BTreeMap::new();
    for (m, i) in bound {
        match &row.cells[*i] {
            Cell::Error(e) => {
                return Err(RowRefusal::ErrorCell {
                    column: headers[*i].clone(),
                    error: e.to_string(),
                });
            }
            Cell::Text(t) => {
                let t = clean(t);
                if !t.is_empty() {
                    out.insert(m.clone(), t);
                }
            }
        }
    }
    Ok(out)
}

/// The part's JSON members that do not depend on the part number (all but `meta`, `manufacturer` and
/// `part_number`).
fn body(
    v: &BTreeMap<Member, String>,
    headers: &BTreeMap<Member, String>,
) -> Result<Map<String, Value>, String> {
    let col = |m: &Member| headers.get(m).cloned().unwrap_or_else(|| m.name());
    let get = |f: &str| v.get(&Member::field(f));
    let mut out = Map::new();
    if let Some(k) = kind(get("kind").map_or("", String::as_str))
        .map_err(|e| format!("column `{}`: {e}", col(&Member::field("kind"))))?
        && k != "device"
    {
        out.insert("kind".into(), json!(k));
    }
    for name in ["type_designation", "order_number"] {
        if let Some(t) = get(name) {
            out.insert(name.into(), json!(t));
        }
    }
    if let Some(g) = get("gtin") {
        if !(g.bytes().all(|b| b.is_ascii_digit()) && [8, 12, 13, 14].contains(&g.len())) {
            return Err(format!(
                "column `{}`: `{g}` is not a GTIN (8, 12, 13 or 14 digits)",
                col(&Member::field("gtin"))
            ));
        }
        out.insert("gtin".into(), json!(g));
    }
    if let Some(t) = get("description") {
        out.insert("description".into(), json!(t));
    }
    let mut langs = Map::new();
    for (m, t) in v {
        if let Member::Description(lang) = m {
            langs.insert(lang.clone(), json!(t));
        }
    }
    if !langs.is_empty() {
        out.insert("description".into(), Value::Object(langs));
    }
    let mut tech = Vec::new();
    let mut free = Vec::new();
    for (m, t) in v {
        match m {
            Member::TechData(key) => {
                tech.push(tech_value(key, t).map_err(|e| format!("column `{}`: {e}", col(m)))?)
            }
            Member::FreeProp(key) => free.push(json!({"key": key, "value": t})),
            _ => {}
        }
    }
    if !tech.is_empty() {
        out.insert("tech_data".into(), Value::Array(tech));
    }
    if !free.is_empty() {
        out.insert("free_props".into(), Value::Array(free));
    }
    if let Some(u) = unit(get("unit").map_or("", String::as_str))
        .map_err(|e| format!("column `{}`: {e}", col(&Member::field("unit"))))?
        && u != "piece"
    {
        out.insert("unit".into(), json!(u));
    }
    Ok(out)
}

/// Whether two parts are equal but for their import date and source.
fn same_content(new: &Part, held: &Part) -> bool {
    let mut a = new.clone();
    a.meta.as_of = held.meta.as_of;
    a.meta.sources = held.meta.sources.clone();
    a == *held
}

/// Reads a mapped parts table (CSV or XLSX, told apart by content) into part documents of `target.library`
/// (FR-PRT-016).
///
/// Every part is `LibCopy<Part>` with the item ID [`part_item`]`(library, manufacturer key, pn_norm)`, the copy ID
/// `v5(NS_LIBCOPY, library ‖ item)`, `meta.key = "<manufacturer key>.<pn_norm>"` in lower case, `meta.sources` the
/// file's name and `meta.as_of` the target's date. The manufacturer cell resolves through `master` (key or name;
/// aliases and brands, reported EW-IMP-007); a name no record knows becomes a new manufacturer of the target
/// (EW-IMP-005), returned in [`PartsImport::manufacturers`]. The part's `manufacturer` reference names the record's
/// library and item; with [`CatalogTarget::local_manufacturers`] a record of another library is copied into the target
/// and the reference names the copy, which is returned in [`PartsImport::manufacturers`] too.
///
/// Rows: **new**; **updated** (the target holds the item with other content: the held version, or its next patch
/// version with `target.bump`); **unchanged** (the held document is returned as it is); **duplicate** (an earlier row
/// with the same manufacturer and `pn_norm` and the same content); **rejected**, with the reason: no part number, no
/// manufacturer, an unknown kind or order unit, a value that does not parse, an error cell, a duplicate with other
/// content (EW-IMP-002). A part number "A / B" gives two candidates, each flagged (EW-IMP-005), never one number.
/// Unmapped columns are reported (EW-IMP-004), not guessed. Pure: the same bytes, map, master and target give the
/// same result.
///
/// # Errors
///
/// [`ImportError::LimitExceeded`] for an input above `target.limits.max_input_bytes` (checked before parsing), a
/// workbook that unpacks to more, too many rows, columns or too long a cell; [`ImportError::Malformed`] for a table
/// that cannot be read (not UTF-8, a broken quote, not a workbook, no such sheet, no header row) or whose header row
/// lacks the map's manufacturer or part-number column or repeats a mapped column; [`ImportError::Unsupported`] for a
/// ZIP64 workbook.
pub fn parse_parts(
    input: &ImportInput,
    map: &ColumnMap,
    master: &[LibCopy<Manufacturer>],
    target: &CatalogTarget<'_>,
) -> Result<PartsImport, ImportError> {
    parse_parts_built(input, map, master, target, None)
}

/// What builds a row's part where the host knows a part's fields (the window's Parts): from the row's texts by member
/// and the part the target holds of the row's maker and number (the row's columns lay over it), the part's JSON form,
/// or why not, in English (the host keeps its own words). It reads every member, [`Member::Like`] and the fields
/// beyond [`Member::PLAIN`] too.
pub type Build<'a> =
    dyn Fn(&BTreeMap<Member, String>, Option<&LibCopy<Part>>) -> Result<Map<String, Value>, String> + 'a;

/// [`parse_parts`] with the host's `build`: each row's part is built by it from the row's texts over the held part,
/// instead of from the plain members alone; the maker, the number, the item ID and what the row is to the target (new,
/// updated, unchanged, duplicate, rejected) are told as [`parse_parts`] tells them. Without `build` a map that names a
/// member beyond the plain ones is refused.
///
/// # Errors
///
/// As [`parse_parts`].
pub fn parse_parts_built(
    input: &ImportInput,
    map: &ColumnMap,
    master: &[LibCopy<Manufacturer>],
    target: &CatalogTarget<'_>,
    build: Option<&Build<'_>>,
) -> Result<PartsImport, ImportError> {
    let bytes = input.bytes();
    let size = bytes.len() as u64;
    if size > target.limits.max_input_bytes {
        return Err(ImportError::LimitExceeded {
            what: "input size".into(),
            limit: target.limits.max_input_bytes,
        });
    }
    let format = SourceFormat::detect(bytes);
    let sheet = match format {
        SourceFormat::Csv => table::read_csv(bytes, &target.limits)?,
        SourceFormat::Xlsx => table::read_xlsx(bytes, map.sheet.as_deref(), &target.limits)?,
    };
    let file = input.name().to_owned();
    let mut report = ImportReport::new(
        IMPORTER_ID,
        IMPORTER_VERSION,
        SourceInfo {
            name: file.clone(),
            sha256: input.sha256(),
            size,
            format: format.name().to_owned(),
            format_version: None,
            detail: None,
        },
    );
    let bound = bind(&sheet, map, &file, &mut report, build.is_some())?;
    let headers: BTreeMap<Member, String> = bound
        .iter()
        .map(|(m, i)| (m.clone(), sheet.headers[*i].clone()))
        .collect();
    let mut placer = Placer::new(report, &file, master, target);
    for row in &sheet.rows {
        if row
            .cells
            .iter()
            .all(|c| c.text().trim().is_empty() && matches!(c, Cell::Text(_)))
        {
            continue;
        }
        let pn_cell = match &row.cells[bound
            .iter()
            .find(|(m, _)| *m == Member::field("part_number"))
            .map_or(0, |(_, i)| *i)]
        {
            Cell::Text(t) => clean(t),
            Cell::Error(_) => String::new(),
        };
        let v = match values(row, &bound, &sheet.headers) {
            Ok(v) => v,
            Err(why) => {
                placer.reject(row.line, &pn_cell, false, why);
                continue;
            }
        };
        let Some(pn) = v.get(&Member::field("part_number")).cloned() else {
            placer.reject(row.line, "", false, RowRefusal::NoPartNumber);
            continue;
        };
        let Some(maker_text) = v.get(&Member::field("manufacturer")).cloned() else {
            placer.reject(row.line, &pn, false, RowRefusal::NoManufacturer);
            continue;
        };
        if let Some(build) = build {
            placer.place_built(
                &PartDraft {
                    line: row.line,
                    part_number: pn,
                    manufacturer: maker_text,
                    body: Map::new(),
                    meta: Map::new(),
                    sources: vec![file.clone()],
                },
                &v,
                build,
            );
            continue;
        }
        let body = match body(&v, &headers) {
            Ok(b) => b,
            Err(why) => {
                placer.reject(row.line, &pn, false, RowRefusal::Value(why));
                continue;
            }
        };
        placer.place(&PartDraft {
            line: row.line,
            part_number: pn,
            manufacturer: maker_text,
            body,
            meta: Map::new(),
            sources: vec![file.clone()],
        });
    }
    Ok(placer.finish())
}

/// The parts of a source placed in a target one by one: the manufacturer resolved, the part number split and
/// normalised, the item ID minted, duplicates and held parts told (new, updated, unchanged, duplicate, rejected). The
/// one rule of every parts import ([`parse_parts`], the EPLAN readers): a source only decides what a part is, as a
/// [`PartDraft`].
pub struct Placer<'a> {
    file: String,
    target: &'a CatalogTarget<'a>,
    makers: Makers<'a>,
    rows: Vec<RowOutcome>,
    parts: Vec<LibDocument>,
    /// (manufacturer key, pn_norm) → (line, the part as first read).
    seen: BTreeMap<(String, String), (u32, Part)>,
    report: ImportReport,
    /// How a part the target holds takes what the source knows of it: the held part and the source's, the merged.
    merge: Option<fn(&Part, &Part) -> Part>,
}

impl<'a> Placer<'a> {
    /// A placer into `target` for the source `file` (its name, for locations), with the manufacturer master `master`
    /// and the report `report` the source's reader began.
    pub fn new(
        report: ImportReport,
        file: &str,
        master: &'a [LibCopy<Manufacturer>],
        target: &'a CatalogTarget<'a>,
    ) -> Placer<'a> {
        Placer {
            file: file.to_owned(),
            target,
            makers: Makers::new(master, target.library, target.local_manufacturers, target.shared),
            rows: Vec::new(),
            parts: Vec::new(),
            seen: BTreeMap::new(),
            report,
            merge: None,
        }
    }

    /// A source that does not carry the whole part (EPLAN's) lays what it knows over the part the target holds with
    /// `merge` (the held part, the source's) instead of replacing it: what a user added to the held part stays.
    #[must_use]
    pub fn merging(mut self, merge: fn(&Part, &Part) -> Part) -> Placer<'a> {
        self.merge = Some(merge);
        self
    }

    /// The report so far, for the reader's own findings.
    pub fn report_mut(&mut self) -> &mut ImportReport {
        &mut self.report
    }

    fn at(&self, line: u32, item: Option<&str>) -> SourceLocation {
        SourceLocation {
            file: self.file.clone(),
            item: item.map(str::to_owned),
            line: Some(line),
            ..SourceLocation::default()
        }
    }

    /// Rejects a part (EW-IMP-002) with the reason; `candidate`: one of the candidates of an "A / B" number.
    pub fn reject(&mut self, line: u32, pn: &str, candidate: bool, why: RowRefusal) {
        let entry = ReportEntry::new(
            Code::MALFORMED_ITEM,
            Severity::Error,
            format!("row rejected: {why}"),
        )
        .at(self.at(line, (!pn.is_empty()).then_some(pn)));
        self.report.push(entry);
        self.report.stats.dropped += 1;
        self.rows.push(RowOutcome {
            line,
            part_number: pn.to_owned(),
            item: None,
            candidate,
            status: RowStatus::Rejected(why),
        });
    }

    /// Places the part (or the candidates of its "A / B" number) of `src`.
    pub fn place(&mut self, src: &PartDraft) {
        self.place_with(src, None);
    }

    /// Places the part of a row whose part the host builds ([`Build`]) from `texts`, over the part the target holds of
    /// its maker and number; `src` gives the row's line, number, maker and sources.
    pub fn place_built(&mut self, src: &PartDraft, texts: &BTreeMap<Member, String>, build: &Build<'_>) {
        self.place_with(src, Some((texts, build)));
    }

    fn place_with(&mut self, src: &PartDraft, built: Option<(&BTreeMap<Member, String>, &Build<'_>)>) {
        let (line, pn) = (src.line, src.part_number.as_str());
        if pn.trim().is_empty() {
            self.reject(line, "", false, RowRefusal::NoPartNumber);
            return;
        }
        if src.manufacturer.trim().is_empty() {
            self.reject(line, pn, false, RowRefusal::NoManufacturer);
            return;
        }
        let target = self.target;
        let library = target.library;
        let at_pn = self.at(line, Some(pn));
        let maker = match self
            .makers
            .resolve(&src.manufacturer, library, &mut self.report, &at_pn)
        {
            Ok(m) => m,
            Err(why) => {
                self.reject(line, pn, false, why);
                return;
            }
        };
        let names = candidates(pn);
        let split = names.len() > 1;
        if split {
            self.report.push(
                ReportEntry::new(
                    Code::imp(5),
                    Severity::Warning,
                    format!(
                        "the part number `{pn}` names {} parts: imported as the candidates {}; review",
                        names.len(),
                        names
                            .iter()
                            .map(|n| format!("`{n}`"))
                            .collect::<Vec<_>>()
                            .join(" and ")
                    ),
                )
                .at(at_pn),
            );
            self.report.stats.inferred += 1;
        }
        for name in names {
            let norm = pn_norm(&name);
            if norm.is_empty() {
                self.reject(line, &name, split, RowRefusal::NumberWithoutLetters(name.clone()));
                continue;
            }
            let item = part_item(library, &maker.key, &norm);
            let manufacturer = json!({
                "library": maker.library.hyphenated().to_string(),
                "item": maker.item.hyphenated().to_string(),
                "version_req": caret(&maker.version),
            });
            let mut meta = src.meta.clone();
            meta.insert(
                "key".into(),
                json!(format!("{}.{}", maker.key.to_lowercase(), norm.to_lowercase())),
            );
            meta.insert("sources".into(), json!(src.sources));
            if let Some(d) = target.as_of {
                meta.insert("as_of".into(), json!(d));
            }
            // A built part lays the row over the held part, whose identity (its facts, maker and number) it keeps.
            let held_first = built.and_then(|_| (target.existing)(item));
            let mut part = match built {
                Some((texts, build)) => match build(texts, held_first.as_ref()) {
                    Ok(p) => p,
                    Err(why) => {
                        self.reject(line, &name, split, RowRefusal::Host(why));
                        continue;
                    }
                },
                None => src.body.clone(),
            };
            match &held_first {
                Some(h) => {
                    for m in ["meta", "manufacturer", "part_number"] {
                        let held = serde_json::to_value(&h.item).unwrap_or_default();
                        if let Some(v) = held.get(m) {
                            part.insert(m.into(), v.clone());
                        }
                    }
                }
                None => {
                    part.insert("meta".into(), Value::Object(meta));
                    part.insert("manufacturer".into(), manufacturer);
                    part.insert("part_number".into(), json!(name));
                }
            }
            let part: Part = match serde_json::from_value(Value::Object(part)) {
                Ok(p) => p,
                Err(e) => {
                    let why = RowRefusal::Value(format!("a value does not parse: {e}"));
                    self.reject(line, &name, split, why);
                    continue;
                }
            };
            let dedupe = (maker.key.clone(), norm.clone());
            if let Some((first, earlier)) = self.seen.get(&dedupe) {
                let first = *first;
                if *earlier == part {
                    let entry = ReportEntry::new(
                        Code::DROPPED_BY_DESIGN,
                        Severity::Info,
                        format!("row repeats line {first} (same manufacturer and part number): skipped"),
                    )
                    .at(self.at(line, Some(&name)));
                    self.report.push(entry);
                    self.rows.push(RowOutcome {
                        line,
                        part_number: name,
                        item: Some(item),
                        candidate: split,
                        status: RowStatus::Duplicate { of_line: first },
                    });
                } else {
                    self.reject(line, &name, split, RowRefusal::Repeats { line: first });
                }
                continue;
            }
            self.seen.insert(dedupe, (line, part.clone()));
            let held = held_first.or_else(|| (target.existing)(item));
            let part = match (&held, self.merge) {
                (Some(h), Some(merge)) => merge(&h.item, &part),
                _ => part,
            };
            let (status, doc) = match held {
                None => (RowStatus::New, copy(library, item, FIRST_VERSION, part)),
                Some(held) if same_content(&part, &held.item) => (RowStatus::Unchanged, held),
                Some(held) => {
                    let version = if target.bump {
                        match ew_interop_api::ew_schema::next_patch(&held.version) {
                            Some(v) => v,
                            None => {
                                let why = RowRefusal::NoNextVersion(held.version.clone());
                                self.reject(line, &name, split, why);
                                continue;
                            }
                        }
                    } else {
                        held.version.clone()
                    };
                    // The held part's own item, which the target may hold under another ID than the one minted here
                    // (a part made by hand, found by its maker and number).
                    (
                        RowStatus::Updated {
                            from: held.version.clone(),
                            version: version.clone(),
                        },
                        copy(library, held.item_id.as_uuid(), &version, part),
                    )
                }
            };
            let item = doc.item_id.as_uuid();
            self.rows.push(RowOutcome {
                line,
                part_number: name,
                item: Some(item),
                candidate: split,
                status,
            });
            self.parts.push(LibDocument::Part(doc));
        }
    }

    /// The import: the parts and the manufacturers they name, the outcome of every part and the report.
    pub fn finish(self) -> PartsImport {
        let Placer {
            makers,
            rows,
            parts,
            mut report,
            ..
        } = self;
        // By item ID (the map's order).
        let manufacturers: Vec<LibCopy<Manufacturer>> = makers.emitted.into_values().collect();
        report
            .stats
            .per_kind
            .insert("part".into(), u32::try_from(parts.len()).unwrap_or(u32::MAX));
        if !manufacturers.is_empty() {
            report.stats.per_kind.insert(
                "manufacturer".into(),
                u32::try_from(manufacturers.len()).unwrap_or(u32::MAX),
            );
        }
        PartsImport {
            parts,
            manufacturers: manufacturers.into_iter().map(LibDocument::Manufacturer).collect(),
            rows,
            report,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimals_become_canonical() {
        for (cell, want) in [
            ("2,5", Some("2.5")),
            ("2.50", Some("2.5")),
            ("040", Some("40")),
            ("0.000", Some("0")),
            ("-1,25", Some("-1.25")),
            ("1.2345", None),
            ("1.2.3", None),
            ("", None),
            ("x", None),
        ] {
            assert_eq!(decimal(cell).as_deref(), want, "{cell}");
        }
    }

    #[test]
    fn tech_values_are_quantities_or_identifiers() {
        assert_eq!(
            tech_value("i_n", "40 A").unwrap(),
            json!({"key": "i_n", "value": {"value": "40", "unit": "A"}})
        );
        assert_eq!(
            tech_value("csa", "2,5mm²").unwrap()["value"],
            json!({"value": "2.5", "unit": "mm2"})
        );
        assert_eq!(tech_value("curve", "C").unwrap()["value"], json!("C"));
        assert!(tech_value("i_n", "40 Amp").unwrap_err().contains("not a unit"));
        // Every unit of the format, by its spelling or its symbol (`mm`, `piece` and `set` were refused).
        assert_eq!(
            tech_value("d_outer", "8.5 mm").unwrap()["value"],
            json!({"value": "8.5", "unit": "mm"})
        );
        assert_eq!(
            tech_value("pack", "10 pc").unwrap()["value"],
            json!({"value": "10", "unit": "piece"})
        );
    }

    #[test]
    fn kinds_units_and_versions() {
        assert_eq!(kind("End bracket"), Ok(Some("end_bracket")));
        assert_eq!(kind(""), Ok(None));
        assert!(kind("gizmo").is_err());
        assert_eq!(unit("PCS"), Ok(Some("piece")));
        assert!(unit("box").is_err());
        assert_eq!(caret("0.3.1"), "^0.3");
        assert_eq!(caret("2.0.0"), "^2");
    }

    #[test]
    fn a_slash_with_spaces_splits_a_part_number() {
        assert_eq!(candidates("A9P52610 / A9P52616"), ["A9P52610", "A9P52616"]);
        assert_eq!(candidates("1492-SP1B/060"), ["1492-SP1B/060"]);
        assert_eq!(pn_norm("5SY4 204-7"), "5SY42047");
    }
}
