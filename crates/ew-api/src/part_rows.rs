// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The sheet's groups and their declared rows (doc 07 §5.4): the editor lists the groups of the part's sheet
//! ([`ew_derive::part_fields::Group`]) on its left and shows one at a time. Its rows and tables are the declaration's
//! ([`ew_derive::part_fields::PART_FIELDS`]): a field typed in the sheet is a row, the item members of a list are a
//! table's columns, and each value type is the part schema's ([`crate::output::library_fields`]), so a text is read
//! and checked by the property grid's rules; an item's [`Hole`] is two cells, its Kind and its Size. Identity,
//! Technical data, the pins, the network ports, the PLC and the cable keep forms of their own. In the cabinet adds the
//! front's imported outline and its preview.

use std::sync::OnceLock;

use ew_derive::Q;
use ew_derive::part_fields::{Entry, Form, Group, Limit, PART_FIELDS, PartField};
use ew_ids::Id;
use ew_schema::{Hole, HoleKind, Kind, LibCopy, LibDocument, LibRef, Part, PartKind};
use serde_json::{Map, Value};

use crate::Session;
use crate::library::PartQuery;
use crate::output::{FieldType, InputError, format_field_value};
use crate::part_editor::{DraftMessage, PartDraftError, PartSheet, message};
use crate::part_offers::lib_ref;

/// The cell of a table's hole that holds its kind ([`DraftTable::hole`]).
pub const HOLE_KIND: &str = "hole.kind";

/// The cell of a table's hole that holds its size as text ("22 mm", "M5", "Ø 4.5", "92 × 92 R 2").
pub const HOLE_SIZE: &str = "hole.size";

/// The member of a list's item that holds a [`Hole`], edited in two cells ([`HOLE_KIND`], [`HOLE_SIZE`]).
const HOLE: &str = "hole";

/// Whether the editor writes the fields of `group` through a form of its own (the identity, the technical data, the
/// cable, the PLC and the network ports), not as declared rows.
fn by_form(group: Group) -> bool {
    matches!(
        group,
        Group::Identity | Group::Technical | Group::Cable | Group::Plc | Group::Ports
    )
}

/// The declared rows: each field typed as one value, outside the groups with forms of their own.
fn declared_rows() -> impl Iterator<Item = &'static PartField> {
    PART_FIELDS
        .iter()
        .filter(|f| f.entry == Entry::Field && !by_form(f.group) && !f.path.contains('['))
}

/// A declared table: a list member and the members of its item that are its columns.
#[derive(Clone, PartialEq, Eq, Debug)]
struct TableDecl {
    /// The list member (`accessories`).
    path: &'static str,
    /// The group it stands in.
    group: Group,
    /// The item's members its columns edit, in the declaration's order.
    columns: Vec<&'static str>,
    /// Whether its items hold a [`Hole`] in `hole`.
    hole: bool,
}

/// The declared tables: each list whose item members are columns of the sheet (the front's holes too), outside the
/// groups with forms of their own, in the declaration's order.
fn declared_tables() -> &'static [TableDecl] {
    static TABLES: OnceLock<Vec<TableDecl>> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut out: Vec<TableDecl> = Vec::new();
        for f in PART_FIELDS.iter().filter(|f| !by_form(f.group)) {
            let list = match f.entry {
                Entry::Column(list) => list,
                Entry::Form(Form::Holes) => f.path.split_once("[].").map_or(f.path, |(l, _)| l),
                _ => continue,
            };
            let Some(member) = f.path.strip_prefix(list).and_then(|m| m.strip_prefix("[].")) else {
                continue;
            };
            let t = match out.iter_mut().position(|t| t.path == list) {
                Some(i) => &mut out[i],
                None => {
                    out.push(TableDecl {
                        path: list,
                        group: f.group,
                        columns: Vec::new(),
                        hole: false,
                    });
                    out.last_mut().expect("pushed")
                }
            };
            if member == HOLE {
                t.hole = true;
            } else {
                t.columns.push(member);
            }
        }
        out
    })
}

/// The key column of a table whose rows a form makes (the pins' names, which the pins' form edits): its rows are
/// neither added nor removed here.
fn key_of(table: &str) -> Option<&'static str> {
    (table == "cp_pattern").then_some("designation")
}

/// The group a declared row or table cell stands in.
fn group_of(field: &str) -> Option<Group> {
    if let Some(r) = declared_rows().find(|r| r.path == field) {
        return Some(r.group);
    }
    let (path, _, _) = cell(field)?;
    declared_tables().iter().find(|t| t.path == path).map(|t| t.group)
}

/// The value type of the part member at `path` (`accessories[].qty`), from the part schema.
fn field_type(path: &str) -> Option<FieldType> {
    static FIELDS: OnceLock<Vec<crate::output::FieldSpec>> = OnceLock::new();
    FIELDS
        .get_or_init(|| crate::output::library_fields(Kind::Part))
        .iter()
        .find(|f| f.path == path)
        .map(|f| f.ty.clone())
}

/// Whether a column holds a reference to a part (an accessory's `part`), which the editor reads and types as the
/// part's number.
fn is_part_ref(ty: &FieldType) -> bool {
    matches!(ty.inner(), FieldType::Group(m) if ["library", "item", "version_req"]
        .iter()
        .all(|n| m.iter().any(|c| c.name == *n)))
}

/// Whether the part member at `path` names another part (a cabinet's plate), which the place chooses by number.
pub(crate) fn names_a_part(path: &str) -> bool {
    field_type(path).is_some_and(|t| is_part_ref(&t))
}

/// A value's text in a row: a length in millimetres as the layout writes them (`6.2`), anything else as the grid
/// shows it.
fn display(ty: &FieldType, v: &Value) -> String {
    match (ty.inner(), v.as_i64()) {
        (FieldType::Nm, Some(n)) => ew_derive::mm_exact(i128::from(n)),
        _ => format_field_value(ty, v),
    }
}

/// One declared row of a draft: its path, its group, its value type and its text.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DraftRow {
    /// The member's path.
    pub path: &'static str,
    /// The group it stands in.
    pub group: Group,
    /// Its value type.
    pub ty: FieldType,
    /// Its text as typed (the value's display text when opened).
    pub text: String,
    /// Its text when opened: a row left so keeps its value byte for byte.
    pub(crate) opened: String,
}

/// One declared table of a draft: its columns with their types and the texts of its rows.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DraftTable {
    /// The list member.
    pub path: &'static str,
    /// The group it stands in.
    pub group: Group,
    /// The key column's member, if any: the pins' names, whose rows the pins' form makes.
    pub key: Option<&'static str>,
    /// The key column's text per row (empty without a key column).
    pub keys: Vec<String>,
    /// The columns: the item's member and its value type.
    pub columns: Vec<(&'static str, FieldType)>,
    /// Whether its items hold a [`Hole`], edited in two cells after the columns: its Kind ([`HOLE_KIND`], a choice of
    /// [`HoleKind`]'s words) and its Size ([`HOLE_SIZE`], the text [`Hole::parse`] reads).
    pub hole: bool,
    /// The texts of each row, one per column.
    pub rows: Vec<Vec<String>>,
    /// The texts when opened: a cell left so keeps its value byte for byte.
    pub(crate) opened: Vec<Vec<String>>,
    /// Each row's position when opened (`None` for a row added since), over whose item it is written.
    pub(crate) origin: Vec<Option<usize>>,
}

/// The value at `path` (members joined by `.`) of `v`; `None` where a member is absent.
fn value_at<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |v, m| v.get(m))
}

/// Sets the value at `path` of `v`, making the objects on the way; `Value::Null` removes the member, and an object
/// left empty by it goes too.
fn set_at(v: &mut Value, path: &str, value: Value) {
    let members: Vec<&str> = path.split('.').collect();
    fn set(v: &mut Value, members: &[&str], value: Value) {
        let Some(obj) = v.as_object_mut() else {
            return;
        };
        let [first, rest @ ..] = members else {
            return;
        };
        if rest.is_empty() {
            if value.is_null() {
                obj.remove(*first);
            } else {
                obj.insert((*first).to_owned(), value);
            }
            return;
        }
        if value.is_null() && !obj.contains_key(*first) {
            return;
        }
        let child = obj
            .entry((*first).to_owned())
            .or_insert_with(|| Value::Object(Map::new()));
        set(child, rest, value);
        if child.as_object().is_some_and(Map::is_empty) {
            obj.remove(*first);
        }
    }
    set(v, &members, value);
}

/// Whether a sheet's group shown on demand holds a value.
type Holds = fn(&PartSheet) -> bool;

/// The groups a sheet shows only on demand (doc 07 §5.4), each with whether it holds a value: the network ports and the
/// PLC section, which a part shows where its start asked for them, where they hold a value or where the person added
/// them from the contents list ([`PartSheet::groups_to_add`]).
static ON_DEMAND: [(Group, Holds); 2] = [
    (Group::Ports, |s| !s.bus.is_empty()),
    (Group::Plc, |s| {
        !s.plc.role.is_empty() || s.original.sections.plc.is_some()
    }),
];

impl PartSheet {
    /// The pins' table of Connections follows the pins (doc 07 §5.4): one row per pin in the pins' order, named by it; a
    /// pin's row keeps its texts, a new pin's row is empty but for its name, and a pin removed takes its row with it.
    pub(crate) fn pins_to_table(&mut self) {
        let held: Vec<u16> = self.original.cp_pattern.iter().map(|c| c.idx).collect();
        let pins = &self.pins;
        let Some(t) = self.tables.iter_mut().find(|t| t.key.is_some()) else {
            return;
        };
        let current: Vec<Option<u16>> = (0..t.rows.len())
            .map(|i| match t.origin.get(i).copied().flatten() {
                Some(o) => held.get(o).copied(),
                None => pins
                    .iter()
                    .find(|p| t.keys.get(i) == Some(&p.name))
                    .map(|p| p.idx),
            })
            .collect();
        let (mut rows, mut origin) = (Vec::new(), Vec::new());
        for pin in pins {
            match current.iter().position(|c| *c == Some(pin.idx)) {
                Some(i) => {
                    rows.push(t.rows[i].clone());
                    origin.push(t.origin.get(i).copied().flatten());
                }
                None => {
                    rows.push(vec![String::new(); t.columns.len()]);
                    origin.push(None);
                }
            }
        }
        t.keys = pins.iter().map(|p| p.name.clone()).collect();
        t.rows = rows;
        t.origin = origin;
    }

    /// The groups the sheet and its contents list, in the sheet's order: Identity always, Technical data where it shows
    /// a value ([`PartSheet::tech_rows`]: a ferrule's sheet, whose kind names none, has none); a group of
    /// declared rows or tables where a field of it holds for a part of the draft's kind; the pins where the kind has
    /// them; the cable's form for a cable; the network ports and the PLC section where the start asked for them
    /// (doc 07 §5.5), where they hold a value, or where the person added them ([`PartSheet::groups_to_add`]).
    pub fn sections(&self) -> Vec<Group> {
        // A group shown on demand: where the start asks for it or the person added it, else where it holds a value.
        let part = self.of_kind();
        let asks =
            |g: Group| self.start.is_some_and(|start| start.asks.contains(&g)) || self.added.contains(&g);
        let mut out: Vec<Group> = ew_derive::part_fields::fields_of(&part)
            .into_iter()
            .filter(|f| match f.group {
                Group::Identity | Group::Technical | Group::About => false,
                Group::Schematic | Group::Cable => true,
                g => match ON_DEMAND.iter().find(|(d, _)| *d == g) {
                    Some((_, holds)) => asks(g) || holds(self),
                    None => {
                        declared_rows().any(|r| r.path == f.path)
                            || declared_tables()
                                .iter()
                                .any(|t| f.path.starts_with(&format!("{}[]", t.path)))
                    }
                },
            })
            .map(|f| f.group)
            .chain([Group::Identity])
            .chain((!self.tech_rows().is_empty()).then_some(Group::Technical))
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// The technical values Technical data shows, as `(key, text)`: the named ones the draft's kind has (doc 07 §2.5,
    /// decision 15: a field each, typed into where it is empty), in the declaration's order, then every other the draft
    /// holds, in key order; a cable's outer diameter is its Cable group's. A part whose kind names none and that holds
    /// none (a ferrule) has no such group.
    pub fn tech_rows(&self) -> Vec<(String, String)> {
        let part = self.of_kind();
        let named: Vec<&str> = ew_derive::part_fields::PART_FIELDS
            .iter()
            .filter(|f| f.has.holds(&part))
            .filter_map(|f| f.path.strip_prefix("tech_data."))
            .collect();
        let cable = self.text("kind") == "cable";
        named
            .iter()
            .map(|k| ((*k).to_owned(), self.text(&format!("tech:{k}")).to_owned()))
            .chain(
                self.tech
                    .iter()
                    .filter(|(k, _)| !named.contains(&k.as_str()))
                    .cloned(),
            )
            .filter(|(k, _)| !(cable && k == "outer_diameter"))
            .collect()
    }

    /// The groups a part of the draft's kind may have that the sheet does not show yet, which its contents list offers
    /// to add: the groups shown on demand ([`ON_DEMAND`]) that it has not.
    pub fn groups_to_add(&self) -> Vec<Group> {
        let part = self.of_kind();
        let shown = self.sections();
        ON_DEMAND
            .iter()
            .map(|(g, _)| *g)
            .filter(|g| !shown.contains(g))
            .filter(|g| {
                ew_derive::part_fields::fields_of(&part)
                    .iter()
                    .any(|f| f.group == *g)
            })
            .collect()
    }

    /// The part as opened, of the kind its Kind field says now: what the declaration's [`ew_derive::part_fields::Has`]
    /// asks of it.
    fn of_kind(&self) -> Part {
        let mut part = self.original.clone();
        part.kind = serde_json::from_value::<PartKind>(Value::String(self.text("kind").to_owned()))
            .unwrap_or(PartKind::Unknown(Value::Null));
        part
    }

    /// Whether the sheet shows the declared row or table `path` (`mech.width`, `accessories`): a part of the draft's
    /// kind has it.
    pub fn shows(&self, path: &str) -> bool {
        let part = self.of_kind();
        let list = format!("{path}[].");
        PART_FIELDS
            .iter()
            .filter(|f| f.path == path || f.path.starts_with(&list))
            .any(|f| f.has.holds(&part))
    }

    /// Whether the rows of group `group` may be changed: a wire's, a ferrule's and a marker's own data on a new part or
    /// one of one's own that is not a library copy being forked; the pins' clamps and what goes with the part on any part
    /// the editor saves; the others on a part of one's own or a new one.
    pub fn section_editable(&self, group: Group) -> bool {
        match group {
            Group::Connections | Group::GoesWith => true,
            Group::Wire | Group::Ferrule | Group::Marker => (self.new || self.own) && !self.fork,
            _ => self.own || self.new,
        }
    }

    /// Remove of the front's outline: the imported outline goes, and the front is drawn by its shape again; `false`
    /// where In the cabinet is not the person's to change.
    pub fn remove_outline(&mut self) -> bool {
        if !self.section_editable(Group::Cabinet) {
            return false;
        }
        self.outline.clear();
        true
    }

    /// Also stored: each member nothing reads ([`Entry::Stored`]) that the part has a value of, by its word's key with
    /// its value as text (a list's values joined). Shown, never typed: Save starts from the part as opened, so these
    /// are written back as they were.
    pub fn also_stored(&self) -> Vec<(&'static str, String)> {
        let json = serde_json::to_value(&self.original).unwrap_or(Value::Null);
        PART_FIELDS
            .iter()
            .filter(|f| matches!(f.entry, Entry::Stored(_)) && f.has.holds(&self.original))
            .filter_map(|f| {
                let ty = field_type(f.path)?;
                let mut values: Vec<&Value> = vec![&json];
                for segment in f.path.split('.') {
                    let (name, list) = match segment.strip_suffix("[]") {
                        Some(n) => (n, true),
                        None => (segment, false),
                    };
                    values = values
                        .into_iter()
                        .filter_map(|v| v.get(name))
                        .flat_map(|v| match (list, v.as_array()) {
                            (true, Some(items)) => items.iter().collect(),
                            _ => vec![v],
                        })
                        .filter(|v| !v.is_null())
                        .collect();
                }
                let texts: Vec<String> = values.into_iter().map(|v| display(&ty, v)).collect();
                (!texts.is_empty()).then(|| (f.word, texts.join(", ")))
            })
            .collect()
    }

    /// The front as Front view and holes previews it ([`ew_scene::panel::part_front`]): the width, height and shape as
    /// typed, the imported outline and every hole whose cells read; `None` without a width and a height.
    pub fn front_preview(&self) -> Option<(ew_dl::DisplayList, (ew_units::Nm, ew_units::Nm))> {
        let mm = |path: &str| {
            crate::output::parse_mm(&self.row(path)?.text)
                .and_then(|n| i64::try_from(n).ok())
                .map(ew_units::Nm)
        };
        let holes = self
            .table("mech.holes")
            .map(|t| {
                (0..t.rows.len())
                    .filter_map(|i| {
                        let text = |m: &str| {
                            let c = t.columns.iter().position(|(x, _)| *x == m)?;
                            t.rows[i].get(c).map(String::as_str)
                        };
                        let (kind, size) = hole_texts(t, i)?;
                        Some(ew_schema::PartHole {
                            x: mm_of(text("x")?)?,
                            y: mm_of(text("y")?)?,
                            hole: hole_of("", &kind, &size).ok()??,
                            extra: Default::default(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let shape = self
            .row("mech.front_shape")
            .map(|r| r.text.as_str())
            .unwrap_or_default();
        // A cabinet's front is its own size, its enclosure's; any other part's its size in the cabinet.
        let (width, height) = if self.text("kind") == "enclosure" {
            ("sections.enclosure.width", "sections.enclosure.height")
        } else {
            ("mech.width", "mech.height")
        };
        let mech = ew_schema::MechData {
            width: mm(width),
            height: mm(height),
            front_shape: (shape == "round").then_some(ew_schema::FrontShape::Round),
            front_2d: self.outline.clone(),
            holes,
            ..Default::default()
        };
        ew_scene::panel::part_front(&mech)
    }

    /// The draft's declared row at `path`.
    pub fn row(&self, path: &str) -> Option<&DraftRow> {
        self.rows.iter().find(|r| r.path == path)
    }

    /// The draft's declared table of the list member `path`.
    pub fn table(&self, path: &str) -> Option<&DraftTable> {
        self.tables.iter().find(|t| t.path == path)
    }

    /// Adds an empty row to table `path` (one whose rows the editor adds); `false` when it is no such table.
    pub fn add_table_row(&mut self, path: &str) -> bool {
        match self.tables.iter_mut().find(|t| t.path == path && t.key.is_none()) {
            Some(t) => {
                t.rows.push(vec![String::new(); t.columns.len()]);
                t.origin.push(None);
                true
            }
            None => false,
        }
    }

    /// Removes row `row` (0-based) of table `path` and the messages of that table; `false` when there is none.
    pub fn remove_table_row(&mut self, path: &str, row: usize) -> bool {
        let Some(t) = self
            .tables
            .iter_mut()
            .find(|t| t.path == path && t.key.is_none() && row < t.rows.len())
        else {
            return false;
        };
        t.rows.remove(row);
        t.origin.remove(row);
        let prefix = format!("{path}[");
        self.messages.retain(|m| !m.field.starts_with(&prefix));
        true
    }

    /// The text of a declared row (`mech.width`) or table cell (`accessories[0].qty`); `None` for another field.
    pub(crate) fn row_text(&self, field: &str) -> Option<&str> {
        if let Some(r) = self.row(field) {
            return Some(&r.text);
        }
        let (path, row, column) = cell(field)?;
        let t = self.table(path)?;
        let c = t.columns.iter().position(|(m, _)| *m == column)?;
        t.rows.get(row)?.get(c).map(String::as_str)
    }
}

/// The text of a hole's cell `member` ([`HOLE_KIND`], [`HOLE_SIZE`]) of a table's `item`: the kind's word and the
/// size's text form; a hole of a newer format gives no kind and its stored form.
fn hole_cell(item: &Value, member: &str) -> String {
    let Some(v) = item.get("hole").filter(|v| !v.is_null()) else {
        return String::new();
    };
    let hole: Hole = serde_json::from_value(v.clone()).unwrap_or(Hole::Unknown(v.clone()));
    match member {
        HOLE_KIND => hole.kind().map(|k| k.word().to_owned()).unwrap_or_default(),
        _ => hole.to_string(),
    }
}

/// A length in millimetres as a cell holds it.
fn mm_of(text: &str) -> Option<ew_units::Nm> {
    crate::output::parse_mm(text)
        .and_then(|n| i64::try_from(n).ok())
        .map(ew_units::Nm)
}

/// The kind and size texts of row `row` of a table with a hole.
fn hole_texts(t: &DraftTable, row: usize) -> Option<(String, String)> {
    let text = |m: &str| {
        let c = t.columns.iter().position(|(x, _)| *x == m)?;
        t.rows.get(row)?.get(c).cloned()
    };
    Some((text(HOLE_KIND)?, text(HOLE_SIZE)?))
}

/// The hole of a row's kind and size texts; `None` while either is empty (Save asks for both); the size's message,
/// in words, on `field` where it does not read for its kind.
fn hole_of(field: &str, kind: &str, size: &str) -> Result<Option<Hole>, DraftMessage> {
    let (Some(kind), false) = (HoleKind::of_word(kind.trim()), size.trim().is_empty()) else {
        return Ok(None);
    };
    Hole::parse(kind, size).map(Some).map_err(|e| {
        let why = ew_derive::text::hole_error(&e);
        let args: Vec<String> = why.args().iter().map(ToString::to_string).collect();
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        message(field, why.key(), &args)
    })
}

/// A gland's section, once begun, needs its thread and its clamping range, the smaller first (doc 12 §3.9).
fn check_gland(json: &Value) -> Result<(), PartDraftError> {
    let Some(g) = value_at(json, "sections.gland") else {
        return Ok(());
    };
    let refuse = |field: &str, k: &'static str| Err(PartDraftError::Field(message(field, k, &[])));
    for m in ["thread", "clamp_min", "clamp_max"] {
        if g.get(m).is_none_or(Value::is_null) {
            return refuse(
                &format!("sections.gland.{m}"),
                key!("error-part-gland-incomplete"),
            );
        }
    }
    let n = |m: &str| g.get(m).and_then(Value::as_i64).unwrap_or_default();
    if n("clamp_min") > n("clamp_max") {
        return refuse("sections.gland.clamp_max", key!("error-part-gland-range"));
    }
    Ok(())
}

/// Front view and holes written into the part's `json` (doc 07 §5): the imported outline where Import outline… or
/// Remove changed it; then its checks, in words: a round front needs the same width and height, and a hole's centre
/// lies on the front.
fn apply_front(draft: &PartSheet, json: &mut Value) -> Result<(), PartDraftError> {
    let held = draft.original.mech.as_ref().map(|m| &m.front_2d);
    if held.map_or(!draft.outline.is_empty(), |h| *h != draft.outline) {
        let outline = if draft.outline.is_empty() {
            Value::Null
        } else {
            serde_json::to_value(&draft.outline).expect("an outline has a JSON form")
        };
        set_at(json, "mech.front_2d", outline);
    }
    let mech = json.get("mech");
    let nm = |v: Option<&Value>, m: &str| v.and_then(|v| v.get(m)).and_then(Value::as_i64);
    let (w, h) = (nm(mech, "width"), nm(mech, "height"));
    let round = mech.and_then(|m| m.get("front_shape")).and_then(Value::as_str) == Some("round");
    if round && (w.is_none() || w != h) {
        return Err(PartDraftError::Field(message(
            "mech.front_shape",
            key!("error-part-front-round"),
            &[],
        )));
    }
    let (Some(w), Some(h)) = (w, h) else {
        return Ok(());
    };
    let holes = mech.and_then(|m| m.get("holes")).and_then(Value::as_array);
    for (i, hole) in holes.into_iter().flatten().enumerate() {
        let (Some(x), Some(y)) = (nm(Some(hole), "x"), nm(Some(hole), "y")) else {
            continue;
        };
        if !(0..=w).contains(&x) || !(0..=h).contains(&y) {
            let mm = |n: i64| ew_derive::mm_exact(i128::from(n));
            return Err(PartDraftError::Field(message(
                &format!("mech.holes[{i}].x"),
                key!("error-part-hole-off"),
                &[&(i + 1).to_string(), &mm(x), &mm(y), &mm(w), &mm(h)],
            )));
        }
    }
    Ok(())
}

/// A table cell's field `<list>[<row>].<member>` as (list, row, member).
pub(crate) fn cell(field: &str) -> Option<(&str, usize, &str)> {
    let (head, member) = field.rsplit_once("].")?;
    let (path, row) = head.split_once('[')?;
    Some((path, row.parse().ok()?, member))
}

impl Session {
    /// The text of value `v` of type `ty` in a row or a cell: a part's number for a reference to a part.
    fn value_text(&self, q: &Q, ty: &FieldType, v: &Value) -> String {
        if is_part_ref(ty) {
            return serde_json::from_value::<LibRef>(v.clone())
                .ok()
                .and_then(|r| self.ref_number(q, &r))
                .unwrap_or_default();
        }
        display(ty, v)
    }

    /// The declared rows and tables of part `p`, each with its texts.
    pub(crate) fn part_rows(&self, q: &Q, p: &Part) -> (Vec<DraftRow>, Vec<DraftTable>) {
        let json = serde_json::to_value(p).unwrap_or(Value::Null);
        let mut rows = Vec::new();
        for r in declared_rows() {
            let Some(ty) = field_type(r.path) else {
                continue;
            };
            let text = value_at(&json, r.path)
                .filter(|v| !v.is_null())
                .map(|v| self.value_text(q, &ty, v))
                .unwrap_or_default();
            rows.push(DraftRow {
                path: r.path,
                group: r.group,
                ty,
                opened: text.clone(),
                text,
            });
        }
        let mut tables = Vec::new();
        for t in declared_tables() {
            let mut columns: Vec<(&'static str, FieldType)> = t
                .columns
                .iter()
                .filter_map(|c| Some((*c, field_type(&format!("{}[].{c}", t.path))?)))
                .collect();
            if t.hole {
                columns.push((
                    HOLE_KIND,
                    FieldType::Enum {
                        options: HoleKind::ALL.iter().map(|k| k.word().to_owned()).collect(),
                    },
                ));
                columns.push((HOLE_SIZE, FieldType::Text));
            }
            let items = value_at(&json, t.path)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let key = key_of(t.path);
            let keys = key
                .map(|k| {
                    items
                        .iter()
                        .map(|i| i.get(k).and_then(Value::as_str).unwrap_or_default().to_owned())
                        .collect()
                })
                .unwrap_or_default();
            let texts: Vec<Vec<String>> = items
                .iter()
                .map(|item| {
                    columns
                        .iter()
                        .map(|(m, ty)| match item.get(*m).filter(|v| !v.is_null()) {
                            _ if matches!(*m, HOLE_KIND | HOLE_SIZE) => hole_cell(item, m),
                            Some(v) => self.value_text(q, ty, v),
                            None => String::new(),
                        })
                        .collect()
                })
                .collect();
            tables.push(DraftTable {
                path: t.path,
                group: t.group,
                key,
                keys,
                columns,
                hole: t.hole,
                origin: (0..texts.len()).map(Some).collect(),
                opened: texts.clone(),
                rows: texts,
            });
        }
        (rows, tables)
    }

    /// Import outline… of Front view and holes: the front outline of the DXF drawing at `path` into `draft`, read as
    /// the command line's outline import reads it, in millimetres where the drawing names no unit (a front is drawn in
    /// millimetres); the number of its lines. A file larger than an import takes is refused before it is read.
    ///
    /// # Errors
    ///
    /// In words a window renders in its language: the section is not the person's to change, the file cannot be read,
    /// the front door's refusal, a drawing without a line.
    pub fn draft_outline_import(
        &self,
        draft: &mut PartSheet,
        path: &std::path::Path,
    ) -> Result<usize, crate::Text> {
        if !draft.section_editable(Group::Cabinet) {
            return Err(text!("error-part-outline-locked"));
        }
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let limit = crate::import::Limits::default().max_input_bytes;
        let read = std::fs::metadata(path).and_then(|m| {
            if m.len() > limit {
                Ok(None)
            } else {
                std::fs::read(path).map(Some)
            }
        });
        let bytes = match read {
            Ok(Some(b)) => b,
            // In the front door's words, which say what an import takes.
            Ok(None) => {
                let refusal = crate::import::Refusal::Import(crate::import::ImportError::LimitExceeded {
                    what: "input size".to_owned(),
                    limit,
                });
                return Err(crate::dxf::DxfError::from(refusal).text());
            }
            Err(e) => {
                return Err(text!(
                    "error-part-outline-unreadable",
                    name.as_str(),
                    e.to_string()
                ));
            }
        };
        let lines = crate::dxf::read_outline(&name, &bytes, Some(ew_import_dxf::Unit::Millimetre))?;
        let n = lines.len();
        draft.outline = lines;
        Ok(n)
    }

    /// The text the sheet of Parts shows for value `v` of the part member at `path` (`mech.width`,
    /// `accessories[].part`), with the member's value type: a length in millimetres as typed, a part by its number, a
    /// choice as the file spells it (the window words it); `None` for a path the part schema does not have.
    pub fn part_value_text(&self, path: &str, v: &Value) -> Option<(String, FieldType)> {
        let ty = field_type(path)?;
        Some((self.value_text(&self.query(), &ty, v), ty))
    }

    /// The part number of the part `r` names: the held copy's, else the library's.
    fn ref_number(&self, q: &Q, r: &LibRef) -> Option<String> {
        let id: Id<Part> = ew_ids::v5::lib_copy_id(r.library.as_uuid(), r.item.as_uuid());
        if let Some(c) = q.parts().get(id) {
            return Some(c.item.part_number.as_str().to_owned());
        }
        match self.libraries().resolve(r) {
            Ok(LibDocument::Part(c)) => Some(c.item.part_number.as_str().to_owned()),
            _ => None,
        }
    }

    /// The part numbered `number` (case aside): a held copy (no documents), else a library's part with the documents
    /// that embed it; with its kind. `None` when no part has the number.
    pub(crate) fn part_by_number(&self, q: &Q, number: &str) -> Option<(LibRef, PartKind, Vec<Value>)> {
        let not_offered = self.not_offered();
        if let Some((_, c)) = q.parts().iter().find(|(id, c)| {
            !not_offered.copy(id.as_uuid()) && c.item.part_number.as_str().eq_ignore_ascii_case(number)
        }) {
            let c: &LibCopy<Part> = c;
            return Some((
                lib_ref(c.library_id.as_uuid(), c.item_id.as_uuid()),
                c.item.kind.clone(),
                Vec::new(),
            ));
        }
        // By its part number, as the lists find a part (a part's key need not be its number), a new part of his too.
        let hit = self
            .part_search(&PartQuery {
                text: number.to_owned(),
                include_inactive: true,
                ..PartQuery::default()
            })
            .into_iter()
            .find(|h| h.part_number.eq_ignore_ascii_case(number))?;
        let r = lib_ref(hit.library, hit.item);
        let docs = self.closure_docs(std::slice::from_ref(&r)).ok()?;
        let kind = docs.iter().find_map(|d| match d {
            LibDocument::Part(c) if c.item_id.as_uuid() == hit.item => Some(c.item.kind.clone()),
            _ => None,
        })?;
        Some((
            r,
            kind,
            docs.iter()
                .map(|d| serde_json::to_value(d).expect("a library document has a JSON form"))
                .collect(),
        ))
    }

    /// The value of a declared row's or cell's text: `Value::Null` for an empty text; the check's message for a text
    /// that does not parse, for a length not above zero where the row asks it, for a part number no part has and
    /// for an accessory that a wire consumes. `docs` gets the documents a library part named by number needs.
    fn row_value(
        &self,
        q: &Q,
        field: &str,
        ty: &FieldType,
        positive: bool,
        text: &str,
        docs: &mut Vec<Value>,
    ) -> Result<Value, DraftMessage> {
        let t = text.trim();
        if t.is_empty() {
            return Ok(Value::Null);
        }
        if is_part_ref(ty) {
            let (r, kind, more) = self
                .part_by_number(q, t)
                .ok_or_else(|| message(field, key!("error-part-accessory"), &[t]))?;
            if kind.is_consumable() {
                return Err(message(field, key!("error-part-accessory-kind"), &[t]));
            }
            docs.extend(more);
            return Ok(serde_json::to_value(r).expect("a reference has a JSON form"));
        }
        // A length is millimetres as the layout's fields take them (`6,2`, `22.5`), at least zero, above zero where the
        // row asks it; anything else is no length.
        if matches!(ty.inner(), FieldType::Nm) {
            return crate::output::parse_mm(t)
                .and_then(|n| i64::try_from(n).ok())
                .filter(|n| *n > 0 || (!positive && *n == 0))
                .map(Value::from)
                .ok_or_else(|| message(field, key!("error-part-length"), &[t]));
        }
        let member = field.rsplit('.').next().unwrap_or(field);
        let value = self.parse_typed_text(ty, member, t).map_err(|e| {
            let why = match e {
                // The grid's own words, in the window's language.
                InputError::Invalid(m) => {
                    let args: Vec<String> = m.args().iter().map(ToString::to_string).collect();
                    let args: Vec<&str> = args.iter().map(String::as_str).collect();
                    return message(field, m.key(), &args);
                }
                InputError::UnknownPart(m) => m,
                e @ InputError::NotOneOf { .. } => e.text().unwrap_or_default(),
                InputError::Ambiguous { choices } => choices
                    .iter()
                    .map(|(t, _)| t.as_str())
                    .collect::<Vec<_>>()
                    .join(" or "),
            };
            message(field, key!("error-part-value"), &[&why])
        })?;
        Ok(value)
    }

    /// Sets a declared row's or table cell's text in `draft` and checks it; `None` when `field` is neither.
    pub(crate) fn set_row_text(
        &self,
        draft: &mut PartSheet,
        field: &str,
        text: &str,
    ) -> Option<Result<(), PartDraftError>> {
        let group = group_of(field)?;
        if !draft.section_editable(group) {
            return Some(Err(PartDraftError::Refused(key!("error-part-field"))));
        }
        let q = self.query();
        let mut pair = None;
        let (ty, positive) = if let Some(r) = draft.rows.iter_mut().find(|r| r.path == field) {
            r.text = text.to_owned();
            (r.ty.clone(), above_zero(field))
        } else {
            let (path, row, member) = cell(field)?;
            let t = draft.tables.iter_mut().find(|t| t.path == path)?;
            let c = t.columns.iter().position(|(m, _)| *m == member)?;
            let slot = t.rows.get_mut(row)?.get_mut(c)?;
            *slot = text.to_owned();
            if matches!(member, HOLE_KIND | HOLE_SIZE) {
                pair = hole_texts(t, row).map(|(k, s)| (format!("{path}[{row}].{HOLE_SIZE}"), k, s));
            }
            (t.columns[c].1.clone(), above_zero(&format!("{path}[].{member}")))
        };
        // A hole's kind is a choice of its words; its size is read with its kind, its message under the size.
        let result = self
            .row_value(&q, field, &ty, positive, text, &mut Vec::new())
            .and_then(|v| match &pair {
                Some((at, kind, size)) => hole_of(at, kind, size).map(|_| v),
                None => Ok(v),
            });
        draft.messages.retain(|m| m.field != field);
        if let Some((at, _, _)) = &pair {
            draft.messages.retain(|m| m.field != *at);
        }
        Some(match result {
            Ok(_) => Ok(()),
            Err(m) => {
                draft.messages.push(m.clone());
                Err(PartDraftError::Field(m))
            }
        })
    }

    /// The draft's declared rows and tables written into `p` (checked again): a row's value replaces the member, an
    /// empty text leaves it out; a table's rows replace the list, each over the item it was opened from so that the
    /// members the editor does not show stay. Returns the documents the named library parts need.
    pub(crate) fn apply_rows(&self, draft: &PartSheet, p: &mut Part) -> Result<Vec<Value>, PartDraftError> {
        let q = self.query();
        let original = serde_json::to_value(&draft.original).unwrap_or(Value::Null);
        let mut json = serde_json::to_value(&*p).unwrap_or(Value::Null);
        let mut docs = Vec::new();
        for d in draft
            .rows
            .iter()
            .filter(|d| d.text != d.opened && draft.section_editable(d.group))
        {
            let value = self
                .row_value(&q, d.path, &d.ty, above_zero(d.path), &d.text, &mut docs)
                .map_err(PartDraftError::Field)?;
            set_at(&mut json, d.path, value);
        }
        for d in draft.tables.iter().filter(|d| {
            draft.section_editable(d.group)
                && (d.rows != d.opened || d.origin.iter().enumerate().any(|(i, o)| *o != Some(i)))
        }) {
            let held = value_at(&original, d.path)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut items = Vec::with_capacity(d.rows.len());
            for (i, texts) in d.rows.iter().enumerate() {
                let origin = d.origin.get(i).copied().flatten();
                let mut item = origin
                    .and_then(|o| held.get(o).cloned())
                    .unwrap_or_else(|| Value::Object(Map::new()));
                // A new pin's row is that pin: its index and its name, which the pins give.
                if origin.is_none()
                    && let Some(key) = d.key
                    && let Some(pin) = draft.pins.iter().find(|p| d.keys.get(i) == Some(&p.name))
                {
                    set_at(&mut item, "idx", Value::from(pin.idx));
                    set_at(&mut item, key, Value::String(pin.name.clone()));
                }
                let opened = origin.and_then(|o| d.opened.get(o));
                // A hole's row, new or changed, gives every cell; its kind and size are written together.
                if d.hole && opened != Some(texts) {
                    if let Some(c) = texts.iter().position(|x| x.trim().is_empty()) {
                        return Err(PartDraftError::Field(message(
                            &format!("{}[{i}].{}", d.path, d.columns[c].0),
                            key!("error-part-hole-incomplete"),
                            &[&(i + 1).to_string()],
                        )));
                    }
                    let (kind, size) = hole_texts(d, i).unwrap_or_default();
                    let hole = hole_of(&format!("{}[{i}].{HOLE_SIZE}", d.path), &kind, &size)
                        .map_err(PartDraftError::Field)?;
                    set_at(
                        &mut item,
                        HOLE,
                        serde_json::to_value(hole).expect("a hole has a JSON form"),
                    );
                }
                for (c, ((m, ty), text)) in d.columns.iter().zip(texts).enumerate() {
                    // A cell as it was opened keeps its item's value; the hole's cells are written above.
                    if opened.and_then(|o| o.get(c)) == Some(text) || matches!(*m, HOLE_KIND | HOLE_SIZE) {
                        continue;
                    }
                    let field = format!("{}[{i}].{m}", d.path);
                    let positive = above_zero(&format!("{}[].{m}", d.path));
                    let value = self
                        .row_value(&q, &field, ty, positive, text, &mut docs)
                        .map_err(PartDraftError::Field)?;
                    set_at(&mut item, m, value);
                }
                // A row that lacks a value its item needs (an accessory's quantity, a variant's name) is refused at
                // that cell, which names it.
                if let Some((m, _)) = d.columns.iter().find(|(m, ty)| {
                    !ty.is_optional()
                        && !matches!(ty.inner(), FieldType::Bool)
                        && !matches!(*m, HOLE_KIND | HOLE_SIZE)
                        && value_at(&item, m).is_none_or(Value::is_null)
                }) {
                    return Err(PartDraftError::Field(message(
                        &format!("{}[{i}].{m}", d.path),
                        key!("error-part-cell-needed"),
                        &[&(i + 1).to_string()],
                    )));
                }
                items.push(item);
            }
            set_at(&mut json, d.path, Value::Array(items));
        }
        if draft.section_editable(Group::Cabinet) {
            apply_front(draft, &mut json)?;
        }
        if draft.section_editable(Group::Gland) {
            check_gland(&json)?;
        }
        *p = serde_json::from_value(json).map_err(|e| {
            PartDraftError::Field(message("rows", key!("error-part-value"), &[&e.to_string()]))
        })?;
        Ok(docs)
    }
}

/// Whether the declared field `path` (a list's item member as `[]`) is a length that must be above zero.
fn above_zero(path: &str) -> bool {
    ew_derive::part_fields::field(path).is_some_and(|f| f.limit == Limit::AboveZero)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every member of a part's schema has its one row in the declaration ([`PART_FIELDS`]), a reference to another
    /// part one row for its three members, and every row names a member or a named technical value: a member added to
    /// the schema fails here until it says where a person enters it and what reads it.
    #[test]
    fn every_member_of_a_part_has_its_row() {
        let catalogue = crate::output::library_fields(Kind::Part);
        // The part's own members, not the copy's (its IDs, version and hash).
        let schema = serde_json::to_value(schemars::schema_for!(Part)).expect("a schema");
        let own = schema["properties"].as_object().expect("the part's members");
        let member = |path: &str| own.contains_key(path.split(['.', '[']).next().unwrap_or(path));
        let part_ref = |path: &str| catalogue.iter().any(|f| f.path == path && is_part_ref(&f.ty));
        for f in catalogue
            .iter()
            .filter(|f| member(&f.path) && !f.ty.is_container())
        {
            let parent = f.path.rsplit_once('.').map(|(p, _)| p).filter(|p| part_ref(p));
            let rows = PART_FIELDS
                .iter()
                .filter(|r| r.path == f.path || Some(r.path) == parent)
                .count();
            assert_eq!(rows, 1, "{} has {rows} rows in the declaration", f.path);
        }
        for r in PART_FIELDS {
            let named = r.path.starts_with("tech_data.");
            assert!(
                named || catalogue.iter().any(|f| f.path == r.path),
                "{} is no member of a part",
                r.path
            );
        }
    }

    #[test]
    fn a_cell_field_reads_its_list_row_and_member() {
        assert_eq!(cell("accessories[2].qty"), Some(("accessories", 2, "qty")));
        assert_eq!(cell("mech.width"), None);
    }

    #[test]
    fn setting_null_removes_the_member_and_an_emptied_object() {
        let mut v = serde_json::json!({"mech": {"width": 5}});
        set_at(&mut v, "mech.width", Value::Null);
        assert_eq!(v, serde_json::json!({}));
        set_at(&mut v, "sections.wire.csa", serde_json::json!("1.5"));
        assert_eq!(v, serde_json::json!({"sections": {"wire": {"csa": "1.5"}}}));
    }
}
