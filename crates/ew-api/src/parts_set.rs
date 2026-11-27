// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The set an import brings into Parts (doc 07 §3.6, §5.9): a table mapped to the part's declared fields, or an EPLAN
//! parts file, read against My parts. Each row is what it is to what he has: new, changed, the same, or refused with its
//! reason, and a row with a part says what it lacks. Nothing is written until [`Session::save_set`] writes the new and
//! the changed parts into My parts, reading the file again against My parts as it is then.
//!
//! A row of a table is laid over the part My parts holds of its maker and number, else over the part its Like column
//! names, else over a new part of its kind, and its texts are typed into that part's sheet as a person types them
//! ([`Session::set_draft_field`]): the sheet reads and checks them, and the sheet's words say why a text does not read.

use std::cell::RefCell;
use std::collections::BTreeMap;

use ew_ids::Uuid;
use ew_import_catalog::{Build, Member, RowRefusal, RowStatus};
use ew_schema::{Kind, LibCopy, LibDocument, Manufacturer, Part};
use serde_json::{Map, Value};

use crate::catalog::{
    CatalogError, CatalogTarget, ColumnMap, ImportInput, Limits, PartsImport, manufacturer_master, table_limits,
};
use crate::eplan::FndefMap;
use crate::part_editor::{DraftMessage, PartDraftError, PartSheet};
use crate::{Session, library};

/// The words a column names a size by that several kinds share: each goes to the field of that name the row's kind has
/// (a cabinet's outer size, a plate's size, a device's size in the cabinet).
pub const SHARED_SIZES: [&str; 3] = ["width", "height", "depth"];

/// The fields a column of a table goes to (doc 07 §3.6), in the declaration's order: every field of a part that is one
/// value typed or chosen (the maker included), the sizes several kinds share once as [`SHARED_SIZES`], and the named
/// technical values as `tech_data.<key>`. A reference to another part (a cabinet's plate) is chosen in the place, not
/// typed in a table.
pub fn field_columns() -> Vec<&'static str> {
    use ew_derive::part_fields::{Entry, Form, PART_FIELDS};
    let mut out: Vec<&'static str> = Vec::new();
    for f in PART_FIELDS {
        if f.path.contains('[') || !matches!(f.entry, Entry::Field | Entry::Form(Form::Maker)) {
            continue;
        }
        // The maker is typed by name; another part is chosen.
        if f.entry == Entry::Field && crate::part_rows::names_a_part(f.path) {
            continue;
        }
        let last = f.path.rsplit('.').next().unwrap_or(f.path);
        let path = match SHARED_SIZES.iter().find(|s| **s == last) {
            Some(shared) if f.path.contains('.') => *shared,
            _ => f.path,
        };
        if !out.contains(&path) {
            out.push(path);
        }
    }
    out
}

/// The field of a shared size `word` ([`SHARED_SIZES`]) a part of kind `kind` has (`sections.enclosure.width` of a
/// cabinet, `mech.width` of a device); `None` where the kind has none (a plate's depth, a wire's width).
fn shared_size(word: &str, part: &Part) -> Option<String> {
    use ew_derive::part_fields::{Entry, PART_FIELDS};
    PART_FIELDS
        .iter()
        .filter(|f| f.entry == Entry::Field && f.path.contains('.') && !f.path.contains('['))
        .filter(|f| f.path.rsplit('.').next() == Some(word))
        .find(|f| f.has.holds(part))
        .map(|f| f.path.to_owned())
}

/// What a row of a set is to what he has.
#[derive(Clone, PartialEq, Debug)]
pub enum SetStatus {
    /// A part My parts does not hold.
    New,
    /// A part of My parts the row changes.
    Changed,
    /// A part of My parts the row leaves as it is.
    Same,
    /// The row repeats an earlier row of the same maker and number, line `0`.
    Repeats(u32),
    /// The row makes no part, with why.
    Refused(SetRefusal),
}

/// Why a row of a set makes no part.
#[derive(Clone, PartialEq, Debug)]
pub enum SetRefusal {
    /// The reader's reason ([`RowRefusal`]).
    Read(RowRefusal),
    /// A text of the row does not read as its field: the column's header and the sheet's message.
    Field {
        /// The column's header (the member's name where the column is not known).
        column: String,
        /// The sheet's message.
        message: DraftMessage,
    },
    /// The row's Like column names no part.
    Like(String),
}

/// One row of a set, in the file's order.
#[derive(Clone, PartialEq, Debug)]
pub struct SetRow {
    /// The row's line in the file.
    pub line: u32,
    /// Its part number (empty where it has none).
    pub number: String,
    /// The part's maker by name; empty for a refused row.
    pub maker: String,
    /// The description.
    pub description: String,
    /// The part's kind as the file spells it (`terminal`); empty for a refused row.
    pub kind: String,
    /// What it is to what he has.
    pub status: SetStatus,
    /// The part as it will be saved; `None` for a refused row.
    pub part: Option<Part>,
}

impl SetRow {
    /// Whether the part lacks a datum a feature reads.
    pub fn lacks(&self) -> bool {
        self.part
            .as_ref()
            .is_some_and(|p| !ew_derive::part_fields::lacks(p).is_empty())
    }
}

/// Where a set came from, to read again when it is saved.
#[derive(Clone, Debug)]
enum SetSource {
    /// A table and its column map.
    Table { input: ImportInput, map: ColumnMap },
    /// An EPLAN parts file (an EDZ or a parts XML) and the function definitions it maps to.
    Eplan { input: ImportInput, fndefs: FndefMap },
}

/// The set an import brought (doc 07 §5.9): its rows and the read they come from. Saved by [`Session::save_set`].
#[derive(Clone, Debug)]
pub struct PartsSet {
    /// The file's name (never its path).
    pub file: String,
    /// The rows, in the file's order.
    pub rows: Vec<SetRow>,
    /// The read: the parts, the manufacturers and the report.
    pub import: PartsImport,
    source: SetSource,
}

/// Two sets are the same when they are of one file with the same rows (what the place shows).
impl PartialEq for PartsSet {
    fn eq(&self, other: &PartsSet) -> bool {
        self.file == other.file && self.rows == other.rows
    }
}

impl PartsSet {
    /// How many rows are new, changed, the same (repeats counted with them) and refused.
    pub fn counts(&self) -> (usize, usize, usize, usize) {
        let mut c = (0, 0, 0, 0);
        for r in &self.rows {
            match r.status {
                SetStatus::New => c.0 += 1,
                SetStatus::Changed => c.1 += 1,
                SetStatus::Same | SetStatus::Repeats(_) => c.2 += 1,
                SetStatus::Refused(_) => c.3 += 1,
            }
        }
        c
    }

    /// Whether the set is an EPLAN parts file's.
    pub fn is_eplan(&self) -> bool {
        matches!(self.source, SetSource::Eplan { .. })
    }
}

/// What a set reads of the registry: My parts (its ID, none before it is made), the manufacturer master, the
/// libraries it shares manufacturers with, and the parts it holds by the item ID an import mints for their maker and
/// number, so that a part made by hand is found as an imported one is. Taken on the thread that holds the registry; an
/// EPLAN file is read against it on another ([`SetTarget::read_eplan`]).
#[derive(Clone, Debug)]
pub struct SetTarget {
    library: Uuid,
    master: Vec<LibCopy<Manufacturer>>,
    shared: Vec<Uuid>,
    held: BTreeMap<Uuid, LibCopy<Part>>,
}

impl SetTarget {
    /// The registry's part for a set read against My parts.
    pub fn of(reg: &library::Registry) -> SetTarget {
        let library = crate::eplan::find_user_parts_library(reg).unwrap_or_else(Uuid::nil);
        let master = manufacturer_master(reg);
        let mut held = BTreeMap::new();
        if !library.is_nil() {
            for doc in reg.documents(library, Kind::Part) {
                let LibDocument::Part(p) = doc else {
                    continue;
                };
                let key = match reg.resolve(&p.item.manufacturer) {
                    Ok(LibDocument::Manufacturer(m)) => m.item.key.to_string(),
                    _ => continue,
                };
                let norm = ew_interop_api::pn_norm(p.item.part_number.as_str());
                held.insert(ew_import_catalog::part_item(library, &key, &norm), p);
            }
        }
        SetTarget {
            library,
            shared: if library.is_nil() {
                Vec::new()
            } else {
                reg.depends(library)
            },
            master,
            held,
        }
    }

    /// The target of the importer's placer over this part of the registry.
    fn target<'a>(&'a self, existing: &'a dyn Fn(Uuid) -> Option<LibCopy<Part>>, limits: Limits) -> CatalogTarget<'a> {
        CatalogTarget {
            library: self.library,
            as_of: None,
            bump: true,
            limits,
            existing,
            local_manufacturers: true,
            shared: &self.shared,
        }
    }

    /// Reads an EPLAN parts file against this part of the registry (no session: a job's thread).
    ///
    /// # Errors
    ///
    /// The front door's refusal of a file refused as a whole or an importer that panicked.
    pub fn read_eplan(&self, input: &ImportInput, fndefs: &FndefMap) -> Result<PartsImport, crate::import::Refusal> {
        let held = |item| self.held.get(&item).cloned();
        let target = self.target(&held, Limits::default());
        let sizes = [input.bytes().len() as u64];
        crate::import::front_door(input.name(), &sizes, &target.limits, None, || {
            crate::eplan::parts_import(input, fndefs, &self.master, &target)
        })
    }
}

impl Session {
    /// The set of a table read through `map` against My parts (doc 07 §5.9), each row's part built on its sheet.
    ///
    /// # Errors
    ///
    /// [`CatalogError`] for a table refused as a whole or an importer that panicked.
    pub fn table_set(&self, input: &ImportInput, map: &ColumnMap) -> Result<PartsSet, CatalogError> {
        let at = SetTarget::of(self.libraries());
        let (import, refused) = self.read_table(&at, input, map)?;
        Ok(self.set_of(
            input.name(),
            import,
            &refused,
            SetSource::Table {
                input: input.clone(),
                map: map.clone(),
            },
        ))
    }

    /// The set of an EPLAN parts file read against My parts ([`SetTarget::read_eplan`] on a job's thread): its rows.
    pub fn eplan_set(&self, input: &ImportInput, fndefs: &FndefMap, import: PartsImport) -> PartsSet {
        self.set_of(
            input.name(),
            import,
            &[],
            SetSource::Eplan {
                input: input.clone(),
                fndefs: fndefs.clone(),
            },
        )
    }

    /// Reads a table against `at`, each row's part built on its sheet; the sheet's refusals in the order the rows were
    /// refused for them.
    fn read_table(
        &self,
        at: &SetTarget,
        input: &ImportInput,
        map: &ColumnMap,
    ) -> Result<(PartsImport, Vec<SetRefusal>), CatalogError> {
        let headers: BTreeMap<Member, String> = map.columns().map(|(m, h)| (m.clone(), h.to_owned())).collect();
        let refused: RefCell<Vec<SetRefusal>> = RefCell::new(Vec::new());
        let build = |texts: &BTreeMap<Member, String>, held: Option<&LibCopy<Part>>| {
            self.row_part(texts, held, &headers).map_err(|why| {
                let english = match &why {
                    SetRefusal::Field { column, message } => format!("column `{column}`: {}", message.key),
                    SetRefusal::Like(n) => format!("the part `{n}` the Like column names is not known"),
                    SetRefusal::Read(r) => r.to_string(),
                };
                refused.borrow_mut().push(why);
                english
            })
        };
        let build: &Build<'_> = &build;
        let held = |item| at.held.get(&item).cloned();
        let target = at.target(&held, table_limits());
        let sizes = [input.bytes().len() as u64];
        let import = crate::import::front_door(input.name(), &sizes, &target.limits, None, || {
            ew_import_catalog::parse_parts_built(input, map, &at.master, &target, Some(build))
        })?;
        Ok((import, refused.into_inner()))
    }

    /// The part a table's row makes (module documentation): its texts typed into the sheet of the part it lays over;
    /// its JSON form, or why not.
    fn row_part(
        &self,
        texts: &BTreeMap<Member, String>,
        held: Option<&LibCopy<Part>>,
        headers: &BTreeMap<Member, String>,
    ) -> Result<Map<String, Value>, SetRefusal> {
        let q = self.query();
        let column = |m: &Member| headers.get(m).cloned().unwrap_or_else(|| m.name());
        let refused = |m: &Member, e: PartDraftError| match e {
            PartDraftError::Field(message) => SetRefusal::Field {
                column: column(m),
                message,
            },
            PartDraftError::Refused(key) => SetRefusal::Field {
                column: column(m),
                message: crate::part_editor::message(&m.name(), key, &[]),
            },
        };
        // The kind as a table spells it: any case, spaces or `-` for `_`.
        let kind = texts
            .get(&Member::field("kind"))
            .map(|t| t.trim().to_lowercase().replace([' ', '-'], "_"))
            .filter(|t| !t.is_empty());
        let mut sheet: PartSheet = match (held, texts.get(&Member::Like)) {
            (Some(h), _) => self.unheld_draft(&q, h.clone()),
            (None, Some(number)) => {
                let liked = self
                    .part_numbered(number)
                    .ok_or_else(|| SetRefusal::Like(number.clone()))?;
                let mut s = self
                    .new_part_draft_copying(&liked)
                    .map_err(|e| refused(&Member::Like, e))?;
                // An article's own numbers are its own: a part like another has none of the other's.
                for f in ["order_number", "gtin"] {
                    let _ = self.set_draft_field(&mut s, f, "");
                }
                s
            }
            (None, None) => {
                let start = kind.as_deref().and_then(|k| {
                    crate::parts::STARTS
                        .iter()
                        .find(|s| s.card.is_some() && s.presets.first() == Some(&("kind", k)))
                });
                match start {
                    Some(s) => self.start_sheet(s),
                    None => self.new_part_draft(),
                }
                .map_err(|e| refused(&Member::field("kind"), e))?
            }
        };
        if let Some(k) = &kind {
            self.set_draft_field(&mut sheet, "kind", k)
                .map_err(|e| refused(&Member::field("kind"), e))?;
        }
        // What the row's kind has, by the part it lays over with the kind its sheet now names (the sheet's part does
        // not read before its maker and number are typed).
        let mut shape = sheet.original.clone();
        if let Ok(k) = serde_json::from_value(Value::String(sheet.text("kind").to_owned())) {
            shape.kind = k;
        }
        for (m, text) in texts {
            let field = match m {
                Member::Field(f) if f == "kind" => continue,
                // A held part keeps its maker and its number: they are what found it.
                Member::Field(f) if held.is_some() && (f == "manufacturer" || f == "part_number") => continue,
                Member::Field(f) if SHARED_SIZES.contains(&f.as_str()) => match shared_size(f, &shape) {
                    Some(path) => path,
                    None => continue,
                },
                Member::Field(f) => {
                    // A field the row's kind has not is no value of its part.
                    if ew_derive::part_fields::field(f).is_some_and(|d| !d.has.holds(&shape)) {
                        continue;
                    }
                    f.clone()
                }
                Member::TechData(key) => format!("tech:{key}"),
                Member::Description(_) | Member::FreeProp(_) | Member::Like => continue,
            };
            self.set_draft_field(&mut sheet, &field, text)
                .map_err(|e| refused(m, e))?;
        }
        let part = self.sectioned_item(&sheet).map_err(|e| {
            let m = match &e {
                PartDraftError::Field(msg) => texts
                    .keys()
                    .find(|m| m.name() == msg.field)
                    .cloned()
                    .unwrap_or_else(|| Member::field(&msg.field)),
                PartDraftError::Refused(_) => Member::field("part"),
            };
            refused(&m, e)
        })?;
        let mut json = serde_json::to_value(&part).unwrap_or(Value::Null);
        // A description per language, where it reads otherwise than the part's neutral one.
        let langs: Vec<(&String, &String)> = texts
            .iter()
            .filter_map(|(m, t)| match m {
                Member::Description(lang) => Some((lang, t)),
                _ => None,
            })
            .collect();
        let neutral = part.description.as_neutral().map(str::to_owned);
        if !langs.is_empty() && !langs.iter().all(|(_, t)| neutral.as_deref() == Some(t.as_str())) {
            let mut description = match json.get("description") {
                Some(Value::Object(o)) => o.clone(),
                _ => Map::new(),
            };
            for (lang, t) in langs {
                description.insert(lang.clone(), Value::String(t.clone()));
            }
            json["description"] = Value::Object(description);
        }
        // Free properties, kept as text.
        let free: Vec<Value> = texts
            .iter()
            .filter_map(|(m, t)| match m {
                Member::FreeProp(key) => Some(serde_json::json!({"key": key, "value": t})),
                _ => None,
            })
            .collect();
        if !free.is_empty() {
            let mut held_free: Vec<Value> = json
                .get("free_props")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for f in free {
                held_free.retain(|h| h.get("key") != f.get("key"));
                held_free.push(f);
            }
            json["free_props"] = Value::Array(held_free);
        }
        match json {
            Value::Object(o) => Ok(o),
            _ => Err(SetRefusal::Read(RowRefusal::Value(String::new()))),
        }
    }

    /// The sheet of the part numbered `number` a Like column names: one of My parts first, else of the other
    /// libraries, matched without regard to case.
    fn part_numbered(&self, number: &str) -> Option<PartSheet> {
        let reg = self.libraries();
        let mine = crate::eplan::find_user_parts_library(reg);
        let mut libraries: Vec<Uuid> = reg.libraries().into_iter().map(|l| l.id).collect();
        libraries.sort_by_key(|l| Some(*l) != mine);
        let (library, item) = libraries.into_iter().find_map(|lib| {
            reg.documents(lib, Kind::Part).into_iter().find_map(|d| match d {
                LibDocument::Part(p) if p.item.part_number.as_str().eq_ignore_ascii_case(number.trim()) => {
                    Some((lib, p.item_id.as_uuid()))
                }
                _ => None,
            })
        })?;
        self.library_part_draft(library, item).ok()
    }

    /// The fields of a set's row as its sheet would read them (doc 07 §5.9, the row as it will be): each field typed or
    /// chosen that has a value, by its path, in the declaration's order, but the maker, the number, the description and
    /// the kind, which the row names already.
    pub fn set_row_texts(&self, part: &Part) -> Vec<(&'static str, String)> {
        use ew_derive::part_fields::{Entry, PART_FIELDS};
        let nil = Uuid::nil();
        let copy = LibCopy {
            id: ew_ids::Id::from_uuid(nil),
            v: 1,
            library_id: ew_ids::Id::from_uuid(nil),
            item_id: ew_ids::Id::from_uuid(nil),
            version: String::new(),
            sha256: ew_schema::Sha256([0; 32]),
            item: part.clone(),
        };
        let sheet = self.draft_of(&self.query(), copy, nil);
        PART_FIELDS
            .iter()
            .filter(|f| f.entry == Entry::Field && !f.path.contains('[') && f.has.holds(part))
            .filter(|f| !matches!(f.path, "manufacturer" | "part_number" | "description" | "kind"))
            .filter_map(|f| {
                let field = match f.path.strip_prefix("tech_data.") {
                    Some(key) => format!("tech:{key}"),
                    None => f.path.to_owned(),
                };
                let text = sheet.text(&field).trim().to_owned();
                (!text.is_empty()).then_some((f.path, text))
            })
            .collect()
    }

    /// The rows of `import`, the sheet's refusals (`refused`) given in order to the rows refused for them.
    fn set_of(&self, file: &str, import: PartsImport, refused: &[SetRefusal], source: SetSource) -> PartsSet {
        let reg = self.libraries();
        let makers: BTreeMap<(Uuid, Uuid), String> = manufacturer_master(reg)
            .into_iter()
            .chain(import.manufacturers.iter().filter_map(|d| match d {
                LibDocument::Manufacturer(m) => Some(m.clone()),
                _ => None,
            }))
            .map(|m| {
                let name = crate::part_offers::text_of(&m.item.name);
                let name = if name.is_empty() { m.item.key.to_string() } else { name };
                ((m.library_id.as_uuid(), m.item_id.as_uuid()), name)
            })
            .collect();
        let parts: BTreeMap<Uuid, &Part> = import
            .parts
            .iter()
            .filter_map(|d| match d {
                LibDocument::Part(p) => Some((p.item_id.as_uuid(), &p.item)),
                _ => None,
            })
            .collect();
        let mut host = refused.iter();
        let rows = import
            .rows
            .iter()
            .map(|r| {
                let part = r.item.and_then(|i| parts.get(&i)).map(|p| (*p).clone());
                let status = match &r.status {
                    RowStatus::New => SetStatus::New,
                    RowStatus::Updated { .. } => SetStatus::Changed,
                    RowStatus::Unchanged => SetStatus::Same,
                    RowStatus::Duplicate { of_line } => SetStatus::Repeats(*of_line),
                    RowStatus::Rejected(RowRefusal::Host(_)) => {
                        SetStatus::Refused(host.next().cloned().unwrap_or(SetRefusal::Read(RowRefusal::NoPartNumber)))
                    }
                    RowStatus::Rejected(why) => SetStatus::Refused(SetRefusal::Read(why.clone())),
                };
                let maker = part
                    .as_ref()
                    .and_then(|p| makers.get(&(p.manufacturer.library.as_uuid(), p.manufacturer.item.as_uuid())))
                    .cloned()
                    .unwrap_or_default();
                SetRow {
                    line: r.line,
                    number: r.part_number.clone(),
                    maker,
                    description: part
                        .as_ref()
                        .map(|p| crate::part_offers::text_of(&p.description))
                        .unwrap_or_default(),
                    kind: part
                        .as_ref()
                        .map(|p| crate::part_offers::kind_word(&p.kind))
                        .unwrap_or_default(),
                    status,
                    part,
                }
            })
            .collect();
        PartsSet {
            file: file.to_owned(),
            rows,
            import,
            source,
        }
    }

    /// Saves the set's new and changed parts into My parts (made where there is none), the file read again against My
    /// parts as it is now (doc 07 §5.9): the parts and the manufacturers they name, written in one write. Returns how
    /// many were new and how many changed.
    ///
    /// # Errors
    ///
    /// [`CatalogError`] when My parts cannot be made or written, or the file no longer reads.
    pub fn save_set(&mut self, set: &PartsSet) -> Result<(usize, usize), CatalogError> {
        let lib = crate::eplan::user_parts_library(self.libraries_mut())?;
        let at = SetTarget::of(self.libraries());
        let import = match &set.source {
            SetSource::Table { input, map } => self.read_table(&at, input, map)?.0,
            SetSource::Eplan { input, fndefs } => at.read_eplan(input, fndefs)?,
        };
        let written: Vec<Uuid> = import
            .rows
            .iter()
            .filter(|r| matches!(r.status, RowStatus::New | RowStatus::Updated { .. }))
            .filter_map(|r| r.item)
            .collect();
        let parts: Vec<&LibDocument> = import
            .parts
            .iter()
            .filter(|d| matches!(d, LibDocument::Part(p) if written.contains(&p.item_id.as_uuid())))
            .collect();
        let named: Vec<(Uuid, Uuid)> = parts
            .iter()
            .filter_map(|d| match d {
                LibDocument::Part(p) => Some((p.item.manufacturer.library.as_uuid(), p.item.manufacturer.item.as_uuid())),
                _ => None,
            })
            .collect();
        let docs: Vec<LibDocument> = import
            .manufacturers
            .iter()
            .filter(|d| {
                matches!(d, LibDocument::Manufacturer(m)
                    if m.library_id.as_uuid() == lib && named.contains(&(lib, m.item_id.as_uuid())))
            })
            .chain(parts)
            .cloned()
            .collect();
        let counts = import.counts();
        if !docs.is_empty() {
            self.libraries_mut()
                .write(lib, &docs)
                .map_err(CatalogError::Library)?;
        }
        Ok((counts.new, counts.updated))
    }
}
