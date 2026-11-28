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
    CatalogError, CatalogTarget, ColumnMap, ImportInput, Limits, PartsImport, manufacturer_master,
    table_limits,
};
use crate::eplan::FndefMap;
use crate::part_editor::{DraftMessage, PartDraftError, PartSheet};
use crate::{Session, library};

/// The words a column names a size by that several kinds share: each goes to the field of that name the row's kind has
/// (a cabinet's outer size, a plate's size, a device's size in the cabinet).
pub const SHARED_SIZES: [&str; 3] = ["width", "height", "depth"];

/// The columns of a cable's cores (Cable ▸ Cores and the cores' cross-section): the sheet's Cores form as "Cores: n,
/// Named: Numbers", without a protective conductor or a shield ([`crate::part_cable`]); a column of each.
pub const CABLE_CORES: [&str; 2] = ["cable.cores", "cable.csa"];

/// The cable's outer diameter, whose one home on the sheet is its Cores form.
const OUTER_DIAMETER: &str = "sections.cable.outer_diameter";

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
        // A cable's cores beside its outer diameter.
        if f.path == OUTER_DIAMETER {
            out.extend(CABLE_CORES);
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
    /// The row names a part he can read that is not his to change (a starter part, a part of another library, a part
    /// only in this project), and differs from it: the part's number and where it lives (the way in: Make my own from
    /// it, or Keep in my parts first).
    NotHis {
        /// The part's number.
        number: String,
        /// Where it lives.
        lives: crate::parts::Lives,
    },
}

/// What the sheet made of a row the importer had no part of My parts for: a refusal, or the part he can read elsewhere
/// that the row is the same as.
#[derive(Clone, Debug)]
enum HostRow {
    Refused(SetRefusal),
    Same(Box<Part>),
}

/// Why a set's Save wrote nothing.
#[derive(Debug)]
pub enum SetSaveError {
    /// My parts changed since the set was read: the set as it reads now, to show and judge again.
    Changed(Box<PartsSet>),
    /// The file no longer reads, or My parts cannot be made or written.
    Catalog(CatalogError),
}

impl From<CatalogError> for SetSaveError {
    fn from(e: CatalogError) -> SetSaveError {
        SetSaveError::Catalog(e)
    }
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
    /// A changed row's members that change, each with what it holds now and what it will hold.
    pub changes: Vec<crate::FieldChange>,
    /// Whether the row names a Like part the row does not use: the part exists already.
    pub like_unused: bool,
}

impl PartsSet {
    /// Whether the set's rows are judged as `other`'s are: each row's line, number, status and changes the same (the
    /// parts a read makes are made anew each time).
    pub fn judged_as(&self, other: &PartsSet) -> bool {
        let judged = |r: &SetRow| (r.line, r.number.clone(), r.status.clone(), r.changes.clone());
        self.rows.len() == other.rows.len()
            && self
                .rows
                .iter()
                .zip(&other.rows)
                .all(|(a, b)| judged(a) == judged(b))
    }
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
    fn target<'a>(
        &'a self,
        existing: &'a dyn Fn(Uuid) -> Option<LibCopy<Part>>,
        limits: Limits,
    ) -> CatalogTarget<'a> {
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
    pub fn read_eplan(
        &self,
        input: &ImportInput,
        fndefs: &FndefMap,
    ) -> Result<PartsImport, crate::import::Refusal> {
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
        let (import, host, likes) = self.read_table(&at, input, map)?;
        Ok(self.set_of(
            input.name(),
            import,
            (&host, &likes),
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
            (&[], &[]),
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
    ) -> Result<(PartsImport, Vec<HostRow>, Vec<String>), CatalogError> {
        let headers: BTreeMap<Member, String> =
            map.columns().map(|(m, h)| (m.clone(), h.to_owned())).collect();
        let host: RefCell<Vec<HostRow>> = RefCell::new(Vec::new());
        let likes_unused: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let refuse = |why: SetRefusal| {
            let english = match &why {
                SetRefusal::Field { column, message } => format!("column `{column}`: {}", message.key),
                SetRefusal::Like(n) => format!("the part `{n}` the Like column names is not known"),
                SetRefusal::NotHis { number, .. } => format!("`{number}` is not his to change"),
                SetRefusal::Read(r) => r.to_string(),
            };
            host.borrow_mut().push(HostRow::Refused(why));
            english
        };
        let build = |texts: &BTreeMap<Member, String>, held: Option<&LibCopy<Part>>| {
            let text = |m: &str| texts.get(&Member::field(m)).map_or("", String::as_str);
            if held.is_some() && texts.contains_key(&Member::Like) {
                likes_unused
                    .borrow_mut()
                    .push(text("part_number").trim().to_owned());
            }
            // A row of a part he can read that is not his: the same, or refused with the way in.
            if held.is_none()
                && let Some((other, lives, in_project)) =
                    self.part_by_maker_number(text("manufacturer"), text("part_number"))
                && lives != crate::parts::Lives::Mine
            {
                if self.row_reads_as(texts, &other, in_project) {
                    host.borrow_mut()
                        .push(HostRow::Same(Box::new(other.item.clone())));
                    return Err("the same as a part he can read".to_owned());
                }
                return Err(refuse(SetRefusal::NotHis {
                    number: other.item.part_number.as_str().to_owned(),
                    lives,
                }));
            }
            self.row_part(texts, held, &headers).map_err(refuse)
        };
        let build: &Build<'_> = &build;
        let held = |item| at.held.get(&item).cloned();
        let target = at.target(&held, table_limits());
        let sizes = [input.bytes().len() as u64];
        let import = crate::import::front_door(input.name(), &sizes, &target.limits, None, || {
            ew_import_catalog::parse_parts_built(input, map, &at.master, &target, Some(build))
        })?;
        Ok((import, host.into_inner(), likes_unused.into_inner()))
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
                let maker = texts
                    .get(&Member::field("manufacturer"))
                    .map_or("", String::as_str);
                let liked = self
                    .part_by_maker_number(maker, number)
                    .and_then(|(c, _, held)| {
                        if held {
                            self.part_draft(c.id.as_uuid()).ok()
                        } else {
                            self.library_part_draft(c.library_id.as_uuid(), c.item_id.as_uuid())
                                .ok()
                        }
                    })
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
        // A cable's cores: a count gives that many numbered cores of the cross-section's column, as the sheet's Cores
        // form makes them ("Cores: n, Named: Numbers"); an empty count leaves the cores as they are.
        let cable = shape.kind == ew_schema::PartKind::Cable;
        let cell = |m: &str| {
            texts
                .get(&Member::field(m))
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
        };
        let (count, csa) = (cell(CABLE_CORES[0]), cell(CABLE_CORES[1]));
        if cable && let Some(n) = count {
            let cores = Member::field(CABLE_CORES[0]);
            for (f, v) in [
                ("cable.marking", "numbers"),
                ("cable.pe", "false"),
                ("cable.shield", "false"),
                ("cable.cores", n),
            ] {
                self.set_draft_field(&mut sheet, f, v)
                    .map_err(|e| refused(&cores, e))?;
            }
        }
        if cable && let Some(c) = csa {
            self.set_draft_field(&mut sheet, "cable.csa", c)
                .map_err(|e| refused(&Member::field(CABLE_CORES[1]), e))?;
        }
        for (m, text) in texts {
            let field = match m {
                Member::Field(f) if f == "kind" => continue,
                Member::Field(f) if CABLE_CORES.contains(&f.as_str()) => continue,
                // A cable's outer diameter is typed into its Cores form.
                Member::Field(f) if cable && f == OUTER_DIAMETER => "cable.diameter".to_owned(),
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
        // A new cable a row gives no cores has none: the form's three cores are the window's start, not the row's.
        if cable
            && count.is_none()
            && held.is_none()
            && !texts.contains_key(&Member::Like)
            && let Some(sections) = json.get_mut("sections").and_then(Value::as_object_mut)
        {
            sections.remove("cable_cores");
        }
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

    /// Whether every cell of a row reads as the table Export writes of part `other` (held by the project where
    /// `in_project`) reads it: the row is that part, unchanged.
    fn row_reads_as(
        &self,
        texts: &BTreeMap<Member, String>,
        other: &LibCopy<Part>,
        in_project: bool,
    ) -> bool {
        let sheet = if in_project {
            self.part_draft(other.id.as_uuid()).ok()
        } else {
            self.library_part_draft(other.library_id.as_uuid(), other.item_id.as_uuid())
                .ok()
        };
        let Some(sheet) = sheet else {
            return false;
        };
        let columns = crate::parts_many::table_columns(std::slice::from_ref(&sheet));
        let kind = |t: &str| t.trim().to_lowercase().replace([' ', '-'], "_");
        texts.iter().all(|(m, t)| match m {
            Member::Like => true,
            Member::Description(lang) => {
                let held = match serde_json::to_value(&other.item.description) {
                    Ok(Value::Object(o)) => o.get(lang).and_then(Value::as_str).map(str::to_owned),
                    Ok(Value::String(neutral)) => Some(neutral),
                    _ => None,
                };
                held.is_some_and(|h| h.trim() == t.trim())
            }
            _ => {
                let name = m.name();
                columns.iter().find(|(c, _)| *c == name).is_some_and(|(_, v)| {
                    let held = v.first().map_or("", |h| h.trim());
                    if name == "kind" {
                        kind(held) == kind(t)
                    } else {
                        held == t.trim()
                    }
                })
            }
        })
    }

    /// The part of maker `maker` (its key or its name, any case) and number `number` (as `pn_norm` reads it) he can read:
    /// one of My parts first, then one the project holds, then one of the other libraries; with where it lives and
    /// whether the project holds it. The one match of a row's part and of its Like column.
    pub(crate) fn part_by_maker_number(
        &self,
        maker: &str,
        number: &str,
    ) -> Option<(LibCopy<Part>, crate::parts::Lives, bool)> {
        let norm = ew_interop_api::pn_norm(number);
        if norm.is_empty() {
            return None;
        }
        let maker = maker.trim().to_lowercase();
        let reg = self.libraries();
        let q = self.query();
        let named = |m: &ew_schema::LibRef| {
            let by_library = match reg.resolve(m) {
                Ok(LibDocument::Manufacturer(d)) => {
                    d.item.key.as_str().to_lowercase() == maker
                        || crate::part_offers::text_of(&d.item.name).to_lowercase() == maker
                }
                _ => false,
            };
            by_library || ew_derive::manufacturer_name(&q, m).to_lowercase() == maker
        };
        let matches =
            |p: &Part| ew_interop_api::pn_norm(p.part_number.as_str()) == norm && named(&p.manufacturer);
        let mine = crate::eplan::find_user_parts_library(reg);
        let in_library = |lib: Uuid| {
            reg.documents(lib, Kind::Part).into_iter().find_map(|d| match d {
                LibDocument::Part(c) if matches(&c.item) => Some(c),
                _ => None,
            })
        };
        if let Some(c) = mine.and_then(in_library) {
            return Some((c, crate::parts::Lives::Mine, false));
        }
        if let Some((_, c)) = q.parts().iter().find(|(_, c)| matches(&c.item)) {
            let c: LibCopy<Part> = (**c).clone();
            let lives = self.lives_in(c.library_id.as_uuid());
            return Some((c, lives, true));
        }
        reg.libraries()
            .into_iter()
            .filter(|l| Some(l.id) != mine)
            .find_map(|l| in_library(l.id).map(|c| (c, self.lives_in(l.id), false)))
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
                // A part is sold by the piece unless it says otherwise; a length reads with its unit.
                if text.is_empty() || (f.path == "unit" && text == "piece") {
                    return None;
                }
                let length = sheet
                    .row(f.path)
                    .is_some_and(|r| matches!(r.ty.inner(), crate::output::FieldType::Nm));
                Some((f.path, if length { format!("{text} mm") } else { text }))
            })
            .collect()
    }

    /// The rows of `import`, the sheet's refusals (`refused`) given in order to the rows refused for them.
    fn set_of(
        &self,
        file: &str,
        import: PartsImport,
        (host, likes): (&[HostRow], &[String]),
        source: SetSource,
    ) -> PartsSet {
        let reg = self.libraries();
        let makers: BTreeMap<(Uuid, Uuid), String> = manufacturer_master(reg)
            .into_iter()
            .chain(import.manufacturers.iter().filter_map(|d| match d {
                LibDocument::Manufacturer(m) => Some(m.clone()),
                _ => None,
            }))
            .map(|m| {
                let name = crate::part_offers::text_of(&m.item.name);
                let name = if name.is_empty() {
                    m.item.key.to_string()
                } else {
                    name
                };
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
        let mut host = host.iter();
        let rows = import
            .rows
            .iter()
            .map(|r| {
                let mut part = r.item.and_then(|i| parts.get(&i)).map(|p| (*p).clone());
                let status = match &r.status {
                    RowStatus::New => SetStatus::New,
                    RowStatus::Updated { .. } => SetStatus::Changed,
                    RowStatus::Unchanged => SetStatus::Same,
                    RowStatus::Duplicate { of_line } => SetStatus::Repeats(*of_line),
                    RowStatus::Rejected(RowRefusal::Host(_)) => match host.next() {
                        Some(HostRow::Same(p)) => {
                            part = Some((**p).clone());
                            SetStatus::Same
                        }
                        Some(HostRow::Refused(why)) => SetStatus::Refused(why.clone()),
                        None => SetStatus::Refused(SetRefusal::Read(RowRefusal::NoPartNumber)),
                    },
                    RowStatus::Rejected(why) => SetStatus::Refused(SetRefusal::Read(why.clone())),
                };
                // A changed row's members that change: My parts' item beside the row's.
                let changes = match (
                    &status,
                    r.item.and_then(|i| {
                        import.parts.iter().find(|d| {
                            d.copy_meta().is_some()
                                && matches!(d, LibDocument::Part(p) if p.item_id.as_uuid() == i)
                        })
                    }),
                ) {
                    (SetStatus::Changed, Some(LibDocument::Part(new))) => {
                        let held = reg.resolve(&crate::part_offers::lib_ref(
                            new.library_id.as_uuid(),
                            new.item_id.as_uuid(),
                        ));
                        match held {
                            Ok(old @ LibDocument::Part(_)) => {
                                crate::classify_update(&old, &LibDocument::Part(new.clone())).fields
                            }
                            _ => Vec::new(),
                        }
                    }
                    _ => Vec::new(),
                };
                let maker = part
                    .as_ref()
                    .and_then(|p| {
                        makers.get(&(p.manufacturer.library.as_uuid(), p.manufacturer.item.as_uuid()))
                    })
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
                    like_unused: likes.iter().any(|n| *n == r.part_number.trim()),
                    status,
                    part,
                    changes,
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
    /// [`SetSaveError::Changed`] with the set as it reads now where it is not the set `set` shown (nothing written);
    /// [`SetSaveError::Catalog`] when My parts cannot be made or written, or the file no longer reads.
    pub fn save_set(&mut self, set: &PartsSet) -> Result<(usize, usize), SetSaveError> {
        let lib = crate::eplan::user_parts_library(self.libraries_mut())?;
        let at = SetTarget::of(self.libraries());
        let (import, host, likes) = match &set.source {
            SetSource::Table { input, map } => self.read_table(&at, input, map)?,
            SetSource::Eplan { input, fndefs } => (
                at.read_eplan(input, fndefs).map_err(CatalogError::from)?,
                Vec::new(),
                Vec::new(),
            ),
        };
        // What he looked at is what is written: where My parts changed since, the rows are judged anew and shown.
        let fresh = self.set_of(&set.file, import.clone(), (&host, &likes), set.source.clone());
        if !fresh.judged_as(set) {
            return Err(SetSaveError::Changed(Box::new(fresh)));
        }
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
                LibDocument::Part(p) => Some((
                    p.item.manufacturer.library.as_uuid(),
                    p.item.manufacturer.item.as_uuid(),
                )),
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
                .map_err(|e| SetSaveError::Catalog(CatalogError::Library(e)))?;
        }
        Ok((counts.new, counts.updated))
    }
}
