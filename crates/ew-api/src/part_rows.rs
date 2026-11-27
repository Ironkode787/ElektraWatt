// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The part editor's sections and their declared rows (doc 07 §5, picture A of the parts design): the editor lists
//! [`PART_SECTIONS`] on its left and shows one at a time. A section of declared rows names the part's members by
//! their paths; each row's value type is the part schema's ([`crate::output::library_fields`]), so a member is one
//! line of this table and its text is read and checked by the property grid's rules. A table section names a list
//! member and the members of its item that are its columns; an item's [`Hole`] is two cells, its Kind and its Size.
//! Identity, Technical data, Pins, Bus, PLC and Cable keep forms of their own; Pins adds the clamp columns of the pin
//! pattern as its table. Front view and holes adds the front's imported outline and its preview.

use std::sync::OnceLock;

use ew_derive::Q;
use ew_ids::Id;
use ew_schema::{Hole, HoleKind, Kind, LibCopy, LibDocument, LibRef, Part, PartKind};
use serde_json::{Map, Value};

use crate::Session;
use crate::library::ItemQuery;
use crate::output::{FieldType, InputError, format_field_value};
use crate::part_editor::{DraftMessage, PartDraft, PartDraftError, message};
use crate::part_offers::lib_ref;

/// A declared row: the part member's path, and whether a length must be above zero (a size; a clearance may be
/// zero).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PartRow {
    /// The member's path (`mech.width`, `sections.ferrule.csa_min`).
    pub path: &'static str,
    /// Whether the value, a length, must be above zero.
    pub positive: bool,
}

/// A declared table: a list member of the part and the members of its item that are its columns.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PartTable {
    /// The list member (`accessories`).
    pub path: &'static str,
    /// The item's member each row shows first and the editor does not change here (a pin's designation, which the
    /// Pins form edits); `None`: rows are added and removed here.
    pub key: Option<&'static str>,
    /// The item's members the columns edit, in their order.
    pub columns: &'static [&'static str],
    /// Whether its items hold a [`Hole`] in `hole`, edited in two cells after the columns: its Kind ([`HOLE_KIND`],
    /// a choice of [`HoleKind`]'s words) and its Size ([`HOLE_SIZE`], the text [`Hole::parse`] reads).
    pub hole: bool,
}

/// The cell of a table's hole that holds its kind ([`PartTable::hole`]).
pub const HOLE_KIND: &str = "hole.kind";

/// The cell of a table's hole that holds its size as text ("22 mm", "M5", "Ø 4.5", "92 × 92 R 2").
pub const HOLE_SIZE: &str = "hole.size";

/// A section of the part editor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PartSection {
    /// Its word (`part.section`'s `section`, the `part-section-<word>` text).
    pub word: &'static str,
    /// Its declared rows; empty for a section with a form of its own.
    pub rows: &'static [PartRow],
    /// Its declared table, if any.
    pub table: Option<PartTable>,
    /// Which parts it is listed for.
    pub shown: Shown,
}

/// Which parts the editor lists a section for ([`PartDraft::sections`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shown {
    /// Every part.
    Always,
    /// A part that is mounted: every kind but a cable and one a wire consumes ([`PartKind::is_consumable`]).
    Mounted,
    /// The kind's own section ([`PartKind::own_section`]): the section whose rows are that member's.
    Own,
    /// A cable.
    Cable,
    /// A part of one's own, a new part, or one that has bus interfaces.
    Bus,
    /// A device of one's own or a new one.
    Plc,
}

const fn row(path: &'static str) -> PartRow {
    PartRow {
        path,
        positive: false,
    }
}

const fn size(path: &'static str) -> PartRow {
    PartRow { path, positive: true }
}

const fn form(word: &'static str, shown: Shown) -> PartSection {
    PartSection {
        word,
        rows: &[],
        table: None,
        shown,
    }
}

/// The editor's sections in the order its list shows them.
pub const PART_SECTIONS: &[PartSection] = &[
    form("identity", Shown::Always),
    form("technical", Shown::Always),
    PartSection {
        word: "pins",
        rows: &[],
        table: Some(PartTable {
            path: "cp_pattern",
            key: Some("designation"),
            columns: &[
                "clamp_type",
                "csa_min",
                "csa_max",
                "max_conductors",
                "twin_ferrule",
            ],
            hole: false,
        }),
        shown: Shown::Mounted,
    },
    PartSection {
        word: "size",
        rows: &[
            size("mech.width"),
            size("mech.height"),
            size("mech.depth"),
            row("mech.mount_kind"),
            row("mech.mass"),
            row("mech.clearance.top"),
            row("mech.clearance.bottom"),
            row("mech.clearance.left"),
            row("mech.clearance.right"),
        ],
        table: None,
        shown: Shown::Mounted,
    },
    PartSection {
        word: "front",
        rows: &[row("mech.front_shape")],
        table: Some(PartTable {
            path: "mech.holes",
            key: None,
            columns: &["x", "y"],
            hole: true,
        }),
        shown: Shown::Mounted,
    },
    PartSection {
        word: "accessories",
        rows: &[],
        table: Some(PartTable {
            path: "accessories",
            key: None,
            columns: &["part", "kind", "qty", "per", "optional"],
            hole: false,
        }),
        shown: Shown::Mounted,
    },
    PartSection {
        word: "variants",
        rows: &[],
        table: Some(PartTable {
            path: "variants",
            key: None,
            columns: &["key", "name", "part_number", "order_number", "type_designation"],
            hole: false,
        }),
        shown: Shown::Mounted,
    },
    PartSection {
        word: "wire",
        rows: &[
            row("sections.wire.csa"),
            row("sections.wire.colour"),
            row("sections.wire.class"),
        ],
        table: None,
        shown: Shown::Own,
    },
    PartSection {
        word: "ferrule",
        rows: &[
            row("sections.ferrule.csa_min"),
            row("sections.ferrule.csa_max"),
            row("sections.ferrule.twin"),
            row("sections.ferrule.insulated"),
            row("sections.ferrule.sleeve"),
            row("sections.ferrule.colour"),
            row("sections.ferrule.stud"),
        ],
        table: None,
        shown: Shown::Own,
    },
    PartSection {
        word: "gland",
        rows: &[
            row("sections.gland.thread"),
            size("sections.gland.clamp_min"),
            size("sections.gland.clamp_max"),
            size("sections.gland.nut"),
        ],
        table: None,
        shown: Shown::Own,
    },
    PartSection {
        word: "marker",
        rows: &[
            row("sections.marker.marks"),
            row("sections.marker.lines"),
            row("sections.marker.chars"),
        ],
        table: None,
        shown: Shown::Own,
    },
    form("bus", Shown::Bus),
    form("plc", Shown::Plc),
    form("cable", Shown::Cable),
];

/// The section named `word`.
pub fn part_section(word: &str) -> Option<&'static PartSection> {
    PART_SECTIONS.iter().find(|s| s.word == word)
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

/// A value's text in a row: a length in millimetres as the layout writes them (`6.2`), anything else as the grid
/// shows it.
fn display(ty: &FieldType, v: &Value) -> String {
    match (ty.inner(), v.as_i64()) {
        (FieldType::Nm, Some(n)) => ew_derive::mm_exact(i128::from(n)),
        _ => format_field_value(ty, v),
    }
}

/// One declared row of a draft: its path, its value type and its text.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DraftRow {
    /// The member's path.
    pub path: &'static str,
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
    /// The key column's member, if any ([`PartTable::key`]).
    pub key: Option<&'static str>,
    /// The key column's text per row (empty without a key column).
    pub keys: Vec<String>,
    /// The columns: the item's member and its value type.
    pub columns: Vec<(&'static str, FieldType)>,
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

impl PartDraft {
    /// The sections the editor lists for the draft, in [`PART_SECTIONS`]' order, each as its [`Shown`] declares: the
    /// mounted part's sections for every kind but a cable and one a wire consumes, the kind's own section (a wire's, a
    /// ferrule's, a marker's) for that kind alone, the bus interfaces and the PLC section of a part that is not one's
    /// own only where it has them.
    pub fn sections(&self) -> Vec<&'static PartSection> {
        let kind = serde_json::from_value::<PartKind>(Value::String(self.text("kind").to_owned()))
            .unwrap_or(PartKind::Unknown(Value::Null));
        let own = kind.own_section().map(|m| format!("sections.{m}."));
        PART_SECTIONS
            .iter()
            .filter(|s| match s.shown {
                Shown::Always => true,
                Shown::Mounted => !kind.is_consumable() && kind != PartKind::Cable,
                Shown::Own => own
                    .as_ref()
                    .is_some_and(|own| s.rows.iter().all(|r| r.path.starts_with(own.as_str()))),
                Shown::Cable => kind == PartKind::Cable,
                Shown::Bus => self.new || self.own || !self.bus.is_empty(),
                Shown::Plc => (self.new || self.own) && kind == PartKind::Device,
            })
            .collect()
    }

    /// Whether the rows of section `word` may be changed: a kind's own section (a wire's, a ferrule's, a marker's)
    /// only on a project-local part, which `part.edit_copy` lets change them; a gland's, the size and the front view
    /// and holes on a part of one's own or a new one (a library part's fork keeps its mechanical data and its gland
    /// data); the others on any part the editor saves.
    pub fn section_editable(&self, word: &str) -> bool {
        match part_section(word) {
            Some(s) if s.word == "gland" => self.own || self.new,
            Some(s) if s.shown == Shown::Own => self.own && !self.fork,
            Some(s) if s.word == "size" || s.word == "front" => self.own || self.new,
            _ => true,
        }
    }

    /// Remove of Front view and holes: the imported outline goes, and the front is drawn by its shape again; `false`
    /// where the section is not the person's to change.
    pub fn remove_outline(&mut self) -> bool {
        if !self.section_editable("front") {
            return false;
        }
        self.outline.clear();
        true
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
        let mech = ew_schema::MechData {
            width: mm("mech.width"),
            height: mm("mech.height"),
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
fn apply_front(draft: &PartDraft, json: &mut Value) -> Result<(), PartDraftError> {
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
    /// The declared rows and tables of part `p`, each with its texts.
    pub(crate) fn part_rows(&self, q: &Q, p: &Part) -> (Vec<DraftRow>, Vec<DraftTable>) {
        let json = serde_json::to_value(p).unwrap_or(Value::Null);
        let mut rows = Vec::new();
        let mut tables = Vec::new();
        for s in PART_SECTIONS {
            for r in s.rows {
                let Some(ty) = field_type(r.path) else {
                    continue;
                };
                let text = value_at(&json, r.path)
                    .filter(|v| !v.is_null())
                    .map(|v| display(&ty, v))
                    .unwrap_or_default();
                rows.push(DraftRow {
                    path: r.path,
                    ty,
                    opened: text.clone(),
                    text,
                });
            }
            let Some(t) = s.table else {
                continue;
            };
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
            let keys = t
                .key
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
                            Some(v) if is_part_ref(ty) => serde_json::from_value::<LibRef>(v.clone())
                                .ok()
                                .and_then(|r| self.ref_number(q, &r))
                                .unwrap_or_default(),
                            Some(v) => display(ty, v),
                            None => String::new(),
                        })
                        .collect()
                })
                .collect();
            tables.push(DraftTable {
                path: t.path,
                key: t.key,
                keys,
                columns,
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
    /// In words: the section is not the person's to change, the file cannot be read, the front door's refusal, a
    /// drawing without a line.
    pub fn draft_outline_import(
        &self,
        draft: &mut PartDraft,
        path: &std::path::Path,
    ) -> Result<usize, String> {
        if !draft.section_editable("front") {
            return Err(text!("error-part-outline-locked").to_string());
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
                return Err(crate::dxf::DxfError::from(refusal).to_string());
            }
            Err(e) => {
                return Err(text!("error-part-outline-unreadable", name.as_str(), e.to_string()).to_string());
            }
        };
        let lines = crate::dxf::read_outline(&name, &bytes, Some(ew_import_dxf::Unit::Millimetre))?;
        let n = lines.len();
        draft.outline = lines;
        Ok(n)
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
        if let Some((_, c)) = q
            .parts()
            .iter()
            .find(|(_, c)| c.item.part_number.as_str().eq_ignore_ascii_case(number))
        {
            let c: &LibCopy<Part> = c;
            return Some((
                lib_ref(c.library_id.as_uuid(), c.item_id.as_uuid()),
                c.item.kind.clone(),
                Vec::new(),
            ));
        }
        let hit = self
            .libraries()
            .search_items(&ItemQuery {
                text: number.to_owned(),
                kinds: vec![Kind::Part],
                ..ItemQuery::default()
            })
            .into_iter()
            .find(|h| h.key.eq_ignore_ascii_case(number) || h.name.eq_ignore_ascii_case(number))?;
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
        draft: &mut PartDraft,
        field: &str,
        text: &str,
    ) -> Option<Result<(), PartDraftError>> {
        let section = PART_SECTIONS.iter().find(|s| {
            s.rows.iter().any(|r| r.path == field)
                || s.table
                    .is_some_and(|t| cell(field).is_some_and(|(p, _, _)| p == t.path))
        })?;
        if !draft.section_editable(section.word) {
            return Some(Err(PartDraftError::Refused(key!("error-part-field"))));
        }
        let q = self.query();
        let mut pair = None;
        let (ty, positive) = if let Some(r) = draft.rows.iter_mut().find(|r| r.path == field) {
            r.text = text.to_owned();
            let positive = section.rows.iter().any(|x| x.path == field && x.positive);
            (r.ty.clone(), positive)
        } else {
            let (path, row, member) = cell(field)?;
            let t = draft.tables.iter_mut().find(|t| t.path == path)?;
            let c = t.columns.iter().position(|(m, _)| *m == member)?;
            let slot = t.rows.get_mut(row)?.get_mut(c)?;
            *slot = text.to_owned();
            if matches!(member, HOLE_KIND | HOLE_SIZE) {
                pair = hole_texts(t, row).map(|(k, s)| (format!("{path}[{row}].{HOLE_SIZE}"), k, s));
            }
            (t.columns[c].1.clone(), false)
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
    pub(crate) fn apply_rows(&self, draft: &PartDraft, p: &mut Part) -> Result<Vec<Value>, PartDraftError> {
        let q = self.query();
        let original = serde_json::to_value(&draft.original).unwrap_or(Value::Null);
        let mut json = serde_json::to_value(&*p).unwrap_or(Value::Null);
        let mut docs = Vec::new();
        for s in PART_SECTIONS {
            if !draft.section_editable(s.word) {
                continue;
            }
            for r in s.rows {
                let Some(d) = draft.row(r.path).filter(|d| d.text != d.opened) else {
                    continue;
                };
                let value = self
                    .row_value(&q, r.path, &d.ty, r.positive, &d.text, &mut docs)
                    .map_err(PartDraftError::Field)?;
                set_at(&mut json, r.path, value);
            }
            let Some(t) = s.table else { continue };
            let Some(d) = draft
                .table(t.path)
                .filter(|d| d.rows != d.opened || d.origin.iter().enumerate().any(|(i, o)| *o != Some(i)))
            else {
                continue;
            };
            let held = value_at(&original, t.path)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut items = Vec::with_capacity(d.rows.len());
            for (i, texts) in d.rows.iter().enumerate() {
                let origin = d.origin.get(i).copied().flatten();
                let mut item = origin
                    .and_then(|o| held.get(o).cloned())
                    .unwrap_or_else(|| Value::Object(Map::new()));
                let opened = origin.and_then(|o| d.opened.get(o));
                // A hole's row, new or changed, gives every cell; its kind and size are written together.
                if t.hole && opened != Some(texts) {
                    if let Some(c) = texts.iter().position(|x| x.trim().is_empty()) {
                        return Err(PartDraftError::Field(message(
                            &format!("{}[{i}].{}", t.path, d.columns[c].0),
                            key!("error-part-hole-incomplete"),
                            &[&(i + 1).to_string()],
                        )));
                    }
                    let (kind, size) = hole_texts(d, i).unwrap_or_default();
                    let hole = hole_of(&format!("{}[{i}].{HOLE_SIZE}", t.path), &kind, &size)
                        .map_err(PartDraftError::Field)?;
                    set_at(
                        &mut item,
                        "hole",
                        serde_json::to_value(hole).expect("a hole has a JSON form"),
                    );
                }
                for (c, ((m, ty), text)) in d.columns.iter().zip(texts).enumerate() {
                    // A cell as it was opened keeps its item's value; the hole's cells are written above.
                    if opened.and_then(|o| o.get(c)) == Some(text) || matches!(*m, HOLE_KIND | HOLE_SIZE) {
                        continue;
                    }
                    let field = format!("{}[{i}].{m}", t.path);
                    let value = self
                        .row_value(&q, &field, ty, false, text, &mut docs)
                        .map_err(PartDraftError::Field)?;
                    set_at(&mut item, m, value);
                }
                items.push(item);
            }
            set_at(&mut json, t.path, Value::Array(items));
        }
        if draft.section_editable("front") {
            apply_front(draft, &mut json)?;
        }
        if draft.section_editable("gland") {
            check_gland(&json)?;
        }
        *p = serde_json::from_value(json).map_err(|e| {
            PartDraftError::Field(message("rows", key!("error-part-value"), &[&e.to_string()]))
        })?;
        Ok(docs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every declared row and column names a member of the part schema: a renamed member fails here, not in the
    /// editor.
    #[test]
    fn every_declared_path_is_a_member_of_the_part() {
        for s in PART_SECTIONS {
            for r in s.rows {
                assert!(field_type(r.path).is_some(), "{}", r.path);
            }
            if let Some(t) = s.table {
                for c in t.columns.iter().chain(t.key.as_ref()) {
                    assert!(
                        field_type(&format!("{}[].{c}", t.path)).is_some(),
                        "{}[].{c}",
                        t.path
                    );
                }
            }
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
