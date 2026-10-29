// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! [`parse_parts`]: a mapped parts table into part documents, the new manufacturers they name, and the row report
//! (WP-S1.19 §9, D11, FR-PRT-016).

use std::collections::BTreeMap;

use ew_interop_api::ew_ids::v5::{self, LineageAnchor};
use ew_interop_api::ew_ids::{Id, Uuid};
use ew_interop_api::ew_schema::{
    Date, LibCopy, LibDocument, LibItem, Library, Manufacturer, Part, Sha256, normalize_nfc,
};
use ew_interop_api::{
    Code, ImportError, ImportInput, ImportReport, Limits, ReportEntry, Severity, SourceInfo, SourceLocation,
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

/// `pn_norm` (doc 07 §4.3), the rule `ew_library::pn_norm` follows: upper case, without white space, `-`, `.`, `/`
/// and `_`. Parts are deduplicated on the manufacturer key and this form.
pub fn pn_norm(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '-' | '.' | '/' | '_'))
        .flat_map(char::to_uppercase)
        .collect()
}

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
}

impl CatalogTarget<'static> {
    /// A target `library` that holds no part yet, without a date, no bump, the default limits.
    pub fn new(library: Uuid) -> CatalogTarget<'static> {
        CatalogTarget {
            library,
            as_of: None,
            bump: false,
            limits: Limits::default(),
            existing: &no_existing,
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
    Rejected(String),
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
    /// The manufacturers the import creates in the target (names no master record matched), by item ID.
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

/// The next patch version of `version` (`1.0.0` → `1.0.1`).
fn bump_patch(version: &str) -> Option<String> {
    let mut it = version.split('.');
    let (major, minor, patch) = (it.next()?, it.next()?, it.next()?);
    if it.next().is_some() {
        return None;
    }
    let n = |s: &str| s.parse::<u64>().ok();
    Some(format!(
        "{}.{}.{}",
        n(major)?,
        n(minor)?,
        n(patch)?.checked_add(1)?
    ))
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
    /// Folded name → the created manufacturer.
    created: BTreeMap<String, (Maker, LibCopy<Manufacturer>)>,
    /// Cell texts already reported (alias, new).
    reported: BTreeMap<String, ()>,
}

impl<'m> Makers<'m> {
    fn new(master: &'m [LibCopy<Manufacturer>]) -> Makers<'m> {
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
            created: BTreeMap::new(),
            reported: BTreeMap::new(),
        }
    }

    fn maker(&self, i: usize) -> Maker {
        let m = &self.master[i];
        Maker {
            key: m.item.key.to_string(),
            library: m.library_id.as_uuid(),
            item: m.item_id.as_uuid(),
            version: m.version.clone(),
        }
    }

    /// Resolves a manufacturer cell: a master record's key or name, then its aliases and brands (EW-IMP-007), then a
    /// manufacturer this import created, else a new one (EW-IMP-005). Two master records for one text reject the row.
    fn resolve(
        &mut self,
        text: &str,
        library: Uuid,
        report: &mut ImportReport,
        at: &SourceLocation,
    ) -> Result<Maker, String> {
        let f = fold(text);
        let pick = |hits: &[usize], master: &[LibCopy<Manufacturer>]| -> Result<usize, String> {
            match hits {
                [one] => Ok(*one),
                _ => {
                    let keys: Vec<String> = hits.iter().map(|i| master[*i].item.key.to_string()).collect();
                    Err(format!(
                        "the manufacturer `{text}` is ambiguous: it names {}",
                        keys.join(", ")
                    ))
                }
            }
        };
        if let Some(hits) = self.direct.get(&f) {
            return Ok(self.maker(pick(hits, self.master)?));
        }
        if let Some(hits) = self.alias.get(&f) {
            let m = self.maker(pick(hits, self.master)?);
            if self.reported.insert(format!("alias|{f}"), ()).is_none() {
                let name = self.master[pick(hits, self.master)?].item.name.clone();
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
        if let Some((m, _)) = self.created.get(&f) {
            return Ok(m.clone());
        }
        let name = clean(text);
        let key = pn_norm(&name);
        if key.is_empty() {
            return Err(format!("the manufacturer `{text}` has no letters or digits"));
        }
        let item = manufacturer_item(library, &key);
        let doc: Manufacturer = serde_json::from_value(json!({"key": key, "name": name}))
            .map_err(|e| format!("the manufacturer `{text}` cannot be recorded: {e}"))?;
        let maker = Maker {
            key: key.clone(),
            library,
            item,
            version: FIRST_VERSION.to_owned(),
        };
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
        self.created
            .insert(f, (maker.clone(), copy(library, item, FIRST_VERSION, doc)));
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

/// The unit symbols a cell may use for each unit of the format.
const UNITS: &[(&str, &str)] = &[
    ("mm2", "mm2"),
    ("mm²", "mm2"),
    ("A", "A"),
    ("V", "V"),
    ("Hz", "Hz"),
    ("bar", "bar"),
    ("degC", "degC"),
    ("°C", "degC"),
    ("kg", "kg"),
    ("W", "W"),
    ("m", "m"),
];

/// A technical value: `<number> <unit>` (the space optional, `,` or `.` as the separator) as a quantity, else an
/// identifier. A number with a unit the format does not know does not parse.
fn tech_value(key: &str, cell: &str) -> Result<Value, String> {
    let split = cell
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | ',' | '-')))
        .unwrap_or(cell.len());
    let (number, unit) = cell.split_at(split);
    let unit = unit.trim();
    let value = match decimal(number) {
        Some(n) if !unit.is_empty() => {
            let Some((_, u)) = UNITS.iter().find(|(s, _)| *s == unit) else {
                return Err(format!("`{cell}`: `{unit}` is not a unit of the format"));
            };
            json!({"value": n, "unit": u})
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
) -> Result<Vec<(Member, usize)>, ImportError> {
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
            None if matches!(member, Member::Manufacturer | Member::PartNumber) => {
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
) -> Result<BTreeMap<Member, String>, String> {
    let mut out = BTreeMap::new();
    for (m, i) in bound {
        match &row.cells[*i] {
            Cell::Error(e) => return Err(format!("column `{}`: the cell holds the error {e}", headers[*i])),
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
    let mut out = Map::new();
    if let Some(k) = kind(v.get(&Member::Kind).map_or("", String::as_str))
        .map_err(|e| format!("column `{}`: {e}", col(&Member::Kind)))?
        && k != "device"
    {
        out.insert("kind".into(), json!(k));
    }
    for (m, name) in [
        (Member::TypeDesignation, "type_designation"),
        (Member::OrderNumber, "order_number"),
    ] {
        if let Some(t) = v.get(&m) {
            out.insert(name.into(), json!(t));
        }
    }
    if let Some(g) = v.get(&Member::Gtin) {
        if !(g.bytes().all(|b| b.is_ascii_digit()) && [8, 12, 13, 14].contains(&g.len())) {
            return Err(format!(
                "column `{}`: `{g}` is not a GTIN (8, 12, 13 or 14 digits)",
                col(&Member::Gtin)
            ));
        }
        out.insert("gtin".into(), json!(g));
    }
    let mut langs = Map::new();
    for (m, t) in v {
        match m {
            Member::Description(None) => {
                out.insert("description".into(), json!(t));
            }
            Member::Description(Some(lang)) => {
                langs.insert(lang.clone(), json!(t));
            }
            _ => {}
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
    if let Some(u) = unit(v.get(&Member::Unit).map_or("", String::as_str))
        .map_err(|e| format!("column `{}`: {e}", col(&Member::Unit)))?
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
/// (WP-S1.19 §9, D11, FR-PRT-016).
///
/// Every part is `LibCopy<Part>` with the item ID [`part_item`]`(library, manufacturer key, pn_norm)`, the copy ID
/// `v5(NS_LIBCOPY, library ‖ item)`, `meta.key = "<manufacturer key>.<pn_norm>"` in lower case, `meta.sources` the
/// file's name and `meta.as_of` the target's date. The manufacturer cell resolves through `master` (key or name;
/// aliases and brands, reported EW-IMP-007); a name no record knows becomes a new manufacturer of the target
/// (EW-IMP-005), returned in [`PartsImport::manufacturers`]. The part's `manufacturer` reference names the record's
/// library and item and keeps the key in `extra["key"]`, which the derivations read until manufacturer copies are
/// resolved (FT-39).
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
    let bound = bind(&sheet, map, &file, &mut report)?;
    let headers: BTreeMap<Member, String> = bound
        .iter()
        .map(|(m, i)| (m.clone(), sheet.headers[*i].clone()))
        .collect();
    let library = target.library;
    let mut makers = Makers::new(master);
    let mut rows: Vec<RowOutcome> = Vec::new();
    let mut parts: Vec<LibDocument> = Vec::new();
    // (manufacturer key, pn_norm) → (line, the part as first read).
    let mut seen: BTreeMap<(String, String), (u32, Part)> = BTreeMap::new();
    for row in &sheet.rows {
        if row
            .cells
            .iter()
            .all(|c| c.text().trim().is_empty() && matches!(c, Cell::Text(_)))
        {
            continue;
        }
        let at = |item: Option<&str>| SourceLocation {
            file: file.clone(),
            item: item.map(str::to_owned),
            line: Some(row.line),
            ..SourceLocation::default()
        };
        let pn_cell = match &row.cells[bound
            .iter()
            .find(|(m, _)| *m == Member::PartNumber)
            .map_or(0, |(_, i)| *i)]
        {
            Cell::Text(t) => clean(t),
            Cell::Error(_) => String::new(),
        };
        let reject =
            |rows: &mut Vec<RowOutcome>, report: &mut ImportReport, pn: &str, candidate, why: String| {
                report.push(
                    ReportEntry::new(
                        Code::MALFORMED_ITEM,
                        Severity::Error,
                        format!("row rejected: {why}"),
                    )
                    .at(at((!pn.is_empty()).then_some(pn))),
                );
                report.stats.dropped += 1;
                rows.push(RowOutcome {
                    line: row.line,
                    part_number: pn.to_owned(),
                    item: None,
                    candidate,
                    status: RowStatus::Rejected(why),
                });
            };
        let v = match values(row, &bound, &sheet.headers) {
            Ok(v) => v,
            Err(why) => {
                reject(&mut rows, &mut report, &pn_cell, false, why);
                continue;
            }
        };
        let Some(pn) = v.get(&Member::PartNumber).cloned() else {
            reject(&mut rows, &mut report, "", false, "no part number".into());
            continue;
        };
        let Some(maker_text) = v.get(&Member::Manufacturer).cloned() else {
            reject(&mut rows, &mut report, &pn, false, "no manufacturer".into());
            continue;
        };
        let body = match body(&v, &headers) {
            Ok(b) => b,
            Err(why) => {
                reject(&mut rows, &mut report, &pn, false, why);
                continue;
            }
        };
        let maker = match makers.resolve(&maker_text, library, &mut report, &at(Some(&pn))) {
            Ok(m) => m,
            Err(why) => {
                reject(&mut rows, &mut report, &pn, false, why);
                continue;
            }
        };
        let names = candidates(&pn);
        let split = names.len() > 1;
        if split {
            report.push(
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
                .at(at(Some(&pn))),
            );
            report.stats.inferred += 1;
        }
        for name in names {
            let norm = pn_norm(&name);
            if norm.is_empty() {
                reject(
                    &mut rows,
                    &mut report,
                    &name,
                    split,
                    format!("the part number `{name}` has no letters or digits"),
                );
                continue;
            }
            let item = part_item(library, &maker.key, &norm);
            let manufacturer = json!({
                "library": maker.library.hyphenated().to_string(),
                "item": maker.item.hyphenated().to_string(),
                "version_req": caret(&maker.version),
                "key": maker.key,
            });
            let mut meta = json!({
                "key": format!("{}.{}", maker.key.to_lowercase(), norm.to_lowercase()),
                "sources": [file],
            });
            if let Some(d) = target.as_of {
                meta["as_of"] = json!(d);
            }
            let mut part = body.clone();
            part.insert("meta".into(), meta);
            part.insert("manufacturer".into(), manufacturer);
            part.insert("part_number".into(), json!(name));
            let part: Part = match serde_json::from_value(Value::Object(part)) {
                Ok(p) => p,
                Err(e) => {
                    reject(
                        &mut rows,
                        &mut report,
                        &name,
                        split,
                        format!("a value does not parse: {e}"),
                    );
                    continue;
                }
            };
            let dedupe = (maker.key.clone(), norm.clone());
            if let Some((first, earlier)) = seen.get(&dedupe) {
                if *earlier == part {
                    report.push(
                        ReportEntry::new(
                            Code::DROPPED_BY_DESIGN,
                            Severity::Info,
                            format!("row repeats line {first} (same manufacturer and part number): skipped"),
                        )
                        .at(at(Some(&name))),
                    );
                    rows.push(RowOutcome {
                        line: row.line,
                        part_number: name,
                        item: Some(item),
                        candidate: split,
                        status: RowStatus::Duplicate { of_line: *first },
                    });
                } else {
                    let why =
                        format!("same manufacturer and part number as line {first}, with other content");
                    reject(&mut rows, &mut report, &name, split, why);
                }
                continue;
            }
            seen.insert(dedupe, (row.line, part.clone()));
            let (status, doc) = match (target.existing)(item) {
                None => (RowStatus::New, copy(library, item, FIRST_VERSION, part)),
                Some(held) if same_content(&part, &held.item) => (RowStatus::Unchanged, held),
                Some(held) => {
                    let version = if target.bump {
                        match bump_patch(&held.version) {
                            Some(v) => v,
                            None => {
                                let why = format!("the held version `{}` cannot be bumped", held.version);
                                reject(&mut rows, &mut report, &name, split, why);
                                continue;
                            }
                        }
                    } else {
                        held.version.clone()
                    };
                    (
                        RowStatus::Updated {
                            from: held.version.clone(),
                            version: version.clone(),
                        },
                        copy(library, item, &version, part),
                    )
                }
            };
            rows.push(RowOutcome {
                line: row.line,
                part_number: name,
                item: Some(item),
                candidate: split,
                status,
            });
            parts.push(LibDocument::Part(doc));
        }
    }
    let mut manufacturers: Vec<LibCopy<Manufacturer>> =
        makers.created.into_values().map(|(_, c)| c).collect();
    manufacturers.sort_by_key(|c| c.item_id.as_uuid());
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
    Ok(PartsImport {
        parts,
        manufacturers: manufacturers.into_iter().map(LibDocument::Manufacturer).collect(),
        rows,
        report,
    })
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
    }

    #[test]
    fn kinds_units_and_versions() {
        assert_eq!(kind("End bracket"), Ok(Some("end_bracket")));
        assert_eq!(kind(""), Ok(None));
        assert!(kind("gizmo").is_err());
        assert_eq!(unit("PCS"), Ok(Some("piece")));
        assert!(unit("box").is_err());
        assert_eq!(bump_patch("1.0.9").as_deref(), Some("1.0.10"));
        assert_eq!(bump_patch("1.0"), None);
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
