// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Table mode's pure rule (§8.2, §8.3; moved down from the view model by WP-S3.08 D10, ID-1174
//! ruling 3), so that the grid and the table round trip produce the same command for the same cells.
//!
//! **Editable cells** (§8.2): a column with a route of §8.2's table (`NavColumn::route`), on a row with records of the
//! route's kind ([`row_targets`]). Every other cell is read-only with the first reason of: "derived", "generated", "no
//! record of this kind", "edit in the property grid", "no command yet" ([`cell_reason_key`]).
//!
//! **Compile** (§8.3, D11, D13): the gesture's cells in (row display order, column visible order); each valid cell
//! gives a (target, path, value) triple per target. A cell one of whose (target, path) pairs an earlier cell of the
//! gesture sets to a different value is invalid (`"same record as row <n>"`); with the same value it is merged. A leaf
//! below a settable member (`props.colour`) is composed into the member per target from the target's stored member
//! value, as the property grid composes it ([`compose`]). The result is `fields.set` when every target gets the same
//! (kind, member, value), else one `fields.set_many` with one entry per (kind, member, value) in the order of first
//! appearance ([`compile`]).
//!
//! **Texts**: every text an item here returns is a text key, checked by `text_key!`
//! ([`cell_reason_key`], [`GestureCell::reason_text`], [`GestureKind::word_key`], [`rows_word_key`]); the view model
//! gives the texts under the old names.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::navigators::{NavKind, NavNode, NavRole, NavTable, Uuid};
use super::{EditRoute, FieldType, StripCell, field_catalogue};

/// The pseudo-route kind of a strip-editor cell: its cells compile through `Session::strip_edit`, not
/// through `parse_field_input`. The path is `group`, `bridge_part:<strip>:<bridge>` (one key per bridge group, so that
/// two rows of one group setting different parts are a conflict, "same bridge as row `<n>`") or `accessory_part:<index>`.
pub const STRIP_ROUTE: &str = "@strip";

/// The route of a strip-editor cell of `node` in `column`: Group and Bridge part on a terminal row, the
/// accessory's Part on an accessory row; `None` for every other cell.
pub fn strip_route(table: &NavTable, node: &NavNode, column: usize) -> Option<(String, String)> {
    if table.kind != NavKind::Strips {
        return None;
    }
    let c = table.columns.get(column)?;
    let path = match (c.strip_cell?, node.role) {
        (StripCell::Group, NavRole::Terminal) => "group".to_owned(),
        (StripCell::BridgePart, NavRole::Terminal) => {
            let bridge = cell_of(table, node, "bridge");
            if bridge.is_empty() {
                return None;
            }
            format!(
                "bridge_part:{}:{bridge}",
                node.cells.first().map_or("", String::as_str)
            )
        }
        (StripCell::AccessoryPart, NavRole::Accessory) => {
            format!("accessory_part:{}", accessory_index(node)?)
        }
        _ => return None,
    };
    Some((STRIP_ROUTE.to_owned(), path))
}

/// The index in `Strip.accessories` of an accessory row (its key's last segment `accessory:<index>`).
pub fn accessory_index(node: &NavNode) -> Option<u32> {
    node.key
        .rsplit('/')
        .next()?
        .strip_prefix("accessory:")?
        .parse()
        .ok()
}

/// The text of `node`'s cell in the column `key` (empty when the table has no such column).
fn cell_of<'n>(table: &NavTable, node: &'n NavNode, key: &str) -> &'n str {
    table
        .columns
        .iter()
        .position(|c| c.key == key)
        .and_then(|i| node.cells.get(i))
        .map_or("", String::as_str)
}

/// The strip editor's read-only reasons, as text keys: `Some(None)` for an editable strip-editor cell,
/// `Some(reason)` for an accessory row's other cells, the Pos column and a Bridge part cell of a terminal without a
/// bridge; `None` for every other cell (the rules of §8.2 apply).
fn strip_reason(table: &NavTable, node: &NavNode, column: usize) -> Option<Option<&'static str>> {
    if table.kind != NavKind::Strips {
        return None;
    }
    let c = table.columns.get(column)?;
    let editable = strip_route(table, node, column).is_some();
    if node.role == NavRole::Accessory {
        return Some((!editable).then_some(ew_commands::text_key!("table-accessory-row")));
    }
    if c.key == "pos" {
        return Some(Some(ew_commands::text_key!("table-pos-move")));
    }
    if node.role == NavRole::Terminal {
        return match c.strip_cell {
            Some(StripCell::BridgePart) if !editable => Some(Some(ew_commands::text_key!("table-no-bridge"))),
            Some(StripCell::Group | StripCell::BridgePart) => Some(None),
            _ => None,
        };
    }
    None
}

/// The records of a row of the route's kind (D10): a conductor route takes a core row's conductor (the records after
/// its cable), a potential definition route a potential row's record when it is a `potential_def` (not the bond row's,
/// nor a connection point naming the potential), every other route the row's records; deduplicated, in record order.
pub fn row_targets(node: &NavNode, route_kind: &str) -> Vec<Uuid> {
    let picked: Vec<Uuid> = match route_kind {
        "conductor" if node.role == NavRole::Conductor => node.records.iter().skip(1).copied().collect(),
        "conductor" => Vec::new(),
        // A cable's length is edited on its cable row, not on a core row.
        "device" if node.role == NavRole::Conductor => Vec::new(),
        // A PLC channel row: the channel, then its module device (the Slot column's record).
        "channel" if node.role == NavRole::Channel => node.records.iter().take(1).copied().collect(),
        "device" if node.role == NavRole::Channel => node.records.iter().skip(1).take(1).copied().collect(),
        "potential_def"
            if node.role == NavRole::Potential
                && node.facets.iter().any(|(f, _)| f == "potential")
                && node.kinds.first().is_some_and(|k| k == "potential_def") =>
        {
            node.records.clone()
        }
        "potential_def" => Vec::new(),
        _ => node.records.clone(),
    };
    let mut seen = BTreeSet::new();
    picked.into_iter().filter(|u| seen.insert(*u)).collect()
}

/// Where a cell of a potential known only at connection points defines it (the G-PP triage's row 8, amending):
/// the CP (`NavNode::define_at`, the row's minimum member CP). The definition's name is the one typed in the row's Name
/// cell, never the name on show.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DefineAt {
    /// The function of the CP.
    pub function: Uuid,
    /// The CP index.
    pub idx: u16,
}

/// The paths of a potential definition that a potential without one can be given in table mode: Name, Type and
/// Voltage (the Name cell defines the row; Type and Voltage only with a name in the same gesture).
pub const DEFINING_PATHS: [&str; 3] = ["name", "ptype", "voltage"];

/// Where a cell of route (`kind`, `path`) on `node` defines the row's potential: a Potentials row of a potential
/// known only at connection points (record kind `cp`, with its CP), the `potential_def` route of Name, Type or
/// Voltage. `None` for every other cell (the other definition cells stay read-only with "no record of this kind",
/// ID-752).
pub fn define_point(node: &NavNode, kind: &str, path: &str) -> Option<DefineAt> {
    if kind != "potential_def"
        || !DEFINING_PATHS.contains(&path)
        || node.role != NavRole::Potential
        || !node.kinds.first().is_some_and(|k| k == "cp")
    {
        return None;
    }
    let (function, idx) = node.define_at?;
    Some(DefineAt { function, idx })
}

/// The typed text of a potential definition cell as the field parser takes it, and the further leaf it sets (the G-PP
/// triage's row 8): Type takes the printed `L+` and `L-` (`l_plus`, `l_minus`); Voltage takes the printed form with
/// its unit and current, `230 V AC`, `24 V DC`, `24VDC`, `24 V` or `24`, whose `AC` or `DC` also sets `env.ac_dc`.
/// Every other route's text is returned as it is.
pub fn potential_input(kind: &str, path: &str, text: &str) -> (String, Option<(String, Value)>) {
    if kind != "potential_def" {
        return (text.to_owned(), None);
    }
    let t = text.trim();
    match path {
        "ptype" => {
            let word = match t {
                "L+" | "l+" => "l_plus",
                "L-" | "l-" | "L\u{2212}" | "l\u{2212}" => "l_minus",
                other => other,
            };
            (word.to_owned(), None)
        }
        "voltage" => {
            let compact: String = t.chars().filter(|c| !c.is_whitespace()).collect();
            let upper = compact.to_ascii_uppercase();
            for (suffix, ac_dc) in [
                ("VAC", Some("ac")),
                ("VDC", Some("dc")),
                ("AC", Some("ac")),
                ("DC", Some("dc")),
                ("V", None),
            ] {
                if upper.len() > suffix.len() && upper.ends_with(suffix) {
                    let number = compact[..compact.len() - suffix.len()].to_owned();
                    let extra = ac_dc.map(|w| ("env.ac_dc".to_owned(), Value::String(w.to_owned())));
                    return (number, extra);
                }
            }
            (t.to_owned(), None)
        }
        _ => (text.to_owned(), None),
    }
}

/// The value type of a route's field, from WP-S2.05's catalogue.
pub fn route_type(kind: &str, path: &str) -> Option<FieldType> {
    field_catalogue(kind)?
        .iter()
        .find(|f| f.path == path)
        .map(|f| f.ty.clone())
}

/// The settable member of a route's field (the catalogue's `fields.set` path): `props` for `props.colour`.
pub fn route_member(kind: &str, path: &str) -> Option<String> {
    field_catalogue(kind)?
        .iter()
        .find(|f| f.path == path)
        .and_then(|f| match &f.route {
            EditRoute::FieldsSet { path, .. } => Some(path.clone()),
            _ => None,
        })
}

/// Whether a route is an outgoing identifier (FR-UI-044 rule 2) in the catalogue.
pub fn route_identifier(kind: &str, path: &str) -> bool {
    field_catalogue(kind)
        .and_then(|c| c.iter().find(|f| f.path == path).map(|f| f.identifier))
        .unwrap_or(false)
}

/// Whether a type is a scalar of D10 (Text, Ident, MlText, Bool, Int, Dec3, Quantity, Csa, Nm, Colour, Enum, or
/// Optional of one).
pub fn is_scalar(ty: &FieldType) -> bool {
    matches!(
        ty.inner(),
        FieldType::Text
            | FieldType::Ident
            | FieldType::MlText
            | FieldType::Bool
            | FieldType::Int { .. }
            | FieldType::Dec3
            | FieldType::Quantity
            | FieldType::Csa
            | FieldType::Nm
            | FieldType::Colour
            | FieldType::Enum { .. }
    )
}

/// The PLC grid's cells whose field is not a scalar but has a typed editor in `Session::parse_field_input` (WP-S2.07
/// §8): a channel's address and target, a module's slot.
const TYPED_PLC: [(&str, &str); 3] = [
    ("channel", "address"),
    ("channel", "target"),
    ("device", "plc_module.slot"),
];

/// Why a cell is read-only (§8.2's precedence), as a text key, `None` for an editable cell:
/// "derived", "generated", "no record of this kind", "edit in the property grid", "no command yet" (the view model's
/// `cell_reason` gives the text). A column without a route reads its [`NavColumn::read_only`](super::NavColumn): the
/// builder's "derived" is `table-reason-derived`, any other `table-reason-no-command`.
pub fn cell_reason_key(table: &NavTable, node: &NavNode, column: usize) -> Option<&'static str> {
    let c = table.columns.get(column)?;
    if let Some(reason) = strip_reason(table, node, column) {
        return reason;
    }
    if c.derived {
        return Some(ew_commands::text_key!("table-reason-derived"));
    }
    if table.kind == NavKind::Pages && node.badges.iter().any(|b| b == "generated") {
        return Some(ew_commands::text_key!("table-reason-generated"));
    }
    let Some((kind, path)) = &c.route else {
        return Some(match c.read_only.as_deref() {
            Some("derived") => ew_commands::text_key!("table-reason-derived"),
            _ => ew_commands::text_key!("table-reason-no-command"),
        });
    };
    if row_targets(node, kind).is_empty() && define_point(node, kind, path).is_none() {
        return Some(ew_commands::text_key!("table-no-record"));
    }
    match route_type(kind, path) {
        Some(ty) if is_scalar(&ty) => None,
        // WP-S2.07 part 2 (§7, FT-429): a cable's length source has its typed editor (`parse_field_input`).
        Some(_) if (kind.as_str(), path.as_str()) == ("device", "cable.length") => None,
        // WP-S2.07 §8 (FT-429): the PLC grid's typed editors.
        Some(_) if TYPED_PLC.contains(&(kind.as_str(), path.as_str())) => None,
        Some(_) => Some(ew_commands::text_key!("table-reason-property-grid")),
        None => Some(ew_commands::text_key!("table-reason-no-command")),
    }
}

/// What one cell of a gesture holds.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CellValue {
    /// The parsed value of the route's field (the leaf's value for a path below its member).
    Valid(Value),
    /// The cell cannot be applied: the reason.
    Invalid(String),
    /// An ambiguous number: its readings (text, value).
    Ambiguous(Vec<(String, Value)>),
}

/// One cell of a gesture.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GestureCell {
    /// The row's key.
    pub row: String,
    /// The row's label.
    pub row_label: String,
    /// The row's 1-based position in display order.
    pub row_number: usize,
    /// The column's key.
    pub column: String,
    /// The column's label.
    pub column_label: String,
    /// The text that was typed, filled, produced or pasted.
    pub text: String,
    /// The route (kind, path), when the cell is editable.
    pub route: Option<(String, String)>,
    /// The records it sets.
    pub targets: Vec<Uuid>,
    /// The parsed value or why not.
    pub value: CellValue,
    /// The row number of an earlier cell that sets one of its records to a different value (D13).
    pub conflict: Option<usize>,
    /// Dropped from the gesture (`table.drop`).
    pub dropped: bool,
    /// Where the cell defines a potential known only at connection points (its targets are empty).
    pub define: Option<DefineAt>,
    /// Further leaves of the same records that the text sets beside the route's (Voltage's `env.ac_dc`).
    pub extra: Vec<(String, Value)>,
    /// A defining cell in a gesture with cells of another row or of a defined record (one `potential.define` per
    /// gesture).
    pub define_clash: bool,
    /// A defining Type or Voltage cell whose row gets no name in the gesture (FT-577: "Name the
    /// potential first").
    pub define_unnamed: bool,
}

impl GestureCell {
    /// Whether the cell keeps Apply from going (invalid, ambiguous or a conflict) and is not dropped.
    pub fn blocking(&self) -> bool {
        !self.dropped
            && (self.conflict.is_some()
                || self.define_clash
                || self.define_unnamed
                || !matches!(self.value, CellValue::Valid(_)))
    }

    /// The reason a blocking cell is invalid (`None` for an ambiguous one and an applicable one): a text key with its
    /// arguments, or the reason an invalid value carries (the view model's `reason` gives the text).
    pub fn reason_text(&self) -> Option<CellText> {
        if let Some(n) = self.conflict {
            // Two rows of one bridge group with different parts.
            let key = if self
                .route
                .as_ref()
                .is_some_and(|(k, p)| k == STRIP_ROUTE && p.starts_with("bridge_part:"))
            {
                ew_commands::text_key!("table-same-bridge")
            } else {
                ew_commands::text_key!("table-same-record")
            };
            return Some(CellText::Key(key, vec![n.to_string()]));
        }
        if self.define_clash {
            return Some(CellText::Key(
                ew_commands::text_key!("table-define-alone"),
                Vec::new(),
            ));
        }
        if self.define_unnamed {
            return Some(CellText::Key(
                ew_commands::text_key!("table-define-name-first"),
                Vec::new(),
            ));
        }
        match &self.value {
            CellValue::Invalid(r) => Some(CellText::Given(r.clone())),
            _ => None,
        }
    }
}

/// A text a moved item hands a front-end (ruling 3: text keys, never English).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CellText {
    /// A text key and its arguments, in its placeholders' order (`{0}`, …).
    Key(&'static str, Vec<String>),
    /// A text given with the cell (a parser's reason), shown as it is.
    Given(String),
}

/// What a gesture is (its word in the card's title).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GestureKind {
    /// One cell or a column over the marked rows, typed (`table.commit`, `table.set`).
    Edit,
    /// Fill down (Ctrl+D).
    FillDown,
    /// Pattern fill (Ctrl+Shift+D).
    PatternFill,
    /// Paste from Excel (Ctrl+V).
    Paste,
    /// Clear (Delete).
    Clear,
}

impl GestureKind {
    /// The text key of the word of the card's title (`paste`, `fill down`).
    pub fn word_key(self) -> &'static str {
        match self {
            GestureKind::Edit => ew_commands::text_key!("table-gesture-edit"),
            GestureKind::FillDown => ew_commands::text_key!("table-gesture-fill-down"),
            GestureKind::PatternFill => ew_commands::text_key!("table-gesture-pattern-fill"),
            GestureKind::Paste => ew_commands::text_key!("table-gesture-paste"),
            GestureKind::Clear => ew_commands::text_key!("table-gesture-clear"),
        }
    }
}

/// One gesture of table mode: its cells, in (row display order, column visible order).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Gesture {
    /// What it is.
    pub kind: GestureKind,
    /// The navigator.
    pub nav: NavKind,
    /// The cells.
    pub cells: Vec<GestureCell>,
    /// Pasted header labels that matched no column on show (listed on the card).
    pub ignored: Vec<String>,
}

impl Gesture {
    /// The cells that keep Apply from going.
    pub fn blocking(&self) -> usize {
        self.cells.iter().filter(|c| c.blocking()).count()
    }

    /// The labels of the columns it sets, each once, in order, joined by `, `.
    pub fn column_labels(&self) -> String {
        let mut out: Vec<&str> = Vec::new();
        for c in self.cells.iter().filter(|c| !c.dropped) {
            if !out.contains(&c.column_label.as_str()) {
                out.push(&c.column_label);
            }
        }
        out.join(", ")
    }

    /// The number of rows it touches.
    pub fn rows(&self) -> usize {
        let rows: BTreeSet<&str> = self
            .cells
            .iter()
            .filter(|c| !c.dropped)
            .map(|c| c.row.as_str())
            .collect();
        rows.len()
    }

    /// The number of cells it sets (the dropped ones left out).
    pub fn live_cells(&self) -> usize {
        self.cells.iter().filter(|c| !c.dropped).count()
    }
}

/// The command a gesture compiles to.
#[derive(Clone, PartialEq, Debug)]
pub struct Compiled {
    /// `fields.set` or `fields.set_many`.
    pub name: &'static str,
    /// Its parameters.
    pub params: Value,
    /// The number of targets over every entry.
    pub targets: usize,
    /// Whether an identifier route sets more than one target (FR-UI-044 rule 2).
    pub identifier: bool,
}

/// What composing a leaf needs about one target: the stored value of its settable member, and the project's source
/// language (a language object's text is replaced in the source language).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct MemberBase {
    /// The member's stored value (`None`: absent).
    pub value: Option<Value>,
    /// The source language (`en` when the project names none).
    pub language: String,
}

/// The value a target's member gets from a leaf (`path` below `member`) or a whole value (`path == member`), composed
/// onto `base` (WP-S2.05 §6.3's leaf rule): a text for a language object replaces the source language's text, and an
/// empty text removes it (the last one, the value).
pub fn compose_leaf(
    member: &str,
    path: &str,
    base: &MemberBase,
    leaf: &Value,
    mltext: bool,
) -> Option<Value> {
    if path == member {
        if mltext && let Some(Value::Object(mut object)) = base.value.clone() {
            match leaf {
                Value::String(t) if !t.is_empty() => {
                    object.insert(base.language.clone(), Value::String(t.clone()));
                }
                Value::String(_) | Value::Null => {
                    object.remove(&base.language);
                }
                other => return Some(other.clone()),
            }
            return Some(if object.is_empty() {
                Value::Null
            } else {
                Value::Object(object)
            });
        }
        return Some(leaf.clone());
    }
    compose(member, path, base.value.as_ref(), leaf)
}

/// Marks the same-record conflicts of a gesture (D13), in cell order, over the cells that are not dropped and parse.
pub fn mark_conflicts(gesture: &mut Gesture) {
    // A defining cell compiles to its own `potential.define`: it may share the gesture only with defining cells of
    // its row.
    let live = |c: &GestureCell| !c.dropped && c.route.is_some();
    let define_rows: BTreeSet<String> = gesture
        .cells
        .iter()
        .filter(|c| live(c) && c.define.is_some())
        .map(|c| c.row.clone())
        .collect();
    let others = gesture.cells.iter().any(|c| live(c) && c.define.is_none());
    for cell in &mut gesture.cells {
        cell.define_clash = live(cell) && cell.define.is_some() && (define_rows.len() > 1 || others);
    }
    // A potential is defined under a name the user types; Type, Voltage or `env.ac_dc` typed on
    // a row known only at its CPs, without a Name in the same gesture, refuses ("Name the potential first").
    let sets = |c: &GestureCell| {
        live(c) && c.define.is_some() && matches!(&c.value, CellValue::Valid(v) if !v.is_null())
    };
    let named: BTreeSet<String> = gesture
        .cells
        .iter()
        .filter(|c| sets(c) && c.route.as_ref().is_some_and(|(_, p)| p == "name"))
        .map(|c| c.row.clone())
        .collect();
    for cell in &mut gesture.cells {
        cell.define_unnamed =
            sets(cell) && cell.route.as_ref().is_some_and(|(_, p)| p != "name") && !named.contains(&cell.row);
    }
    // (target, kind, path) -> the first value set on it and that cell's row number.
    let mut seen: BTreeMap<(Uuid, String, String), (Value, usize)> = BTreeMap::new();
    for cell in &mut gesture.cells {
        cell.conflict = None;
        let (CellValue::Valid(v), Some((kind, path)), false) = (&cell.value, &cell.route, cell.dropped)
        else {
            continue;
        };
        let clash = cell.targets.iter().find_map(|t| {
            seen.get(&(*t, kind.clone(), path.clone()))
                .filter(|(val, _)| val != v)
                .map(|(_, row)| *row)
        });
        if clash.is_some() {
            cell.conflict = clash;
            continue;
        }
        for t in &cell.targets {
            seen.entry((*t, kind.clone(), path.clone()))
                .or_insert_with(|| (v.clone(), cell.row_number));
        }
    }
}

/// Compiles the applicable cells of a gesture (after [`mark_conflicts`]) into one command; `base(kind, target,
/// member)` reads a target's stored member for a composed leaf or a language object. `None` when no cell applies.
pub fn compile(gesture: &Gesture, base: &mut dyn FnMut(&str, Uuid, &str) -> MemberBase) -> Option<Compiled> {
    let applicable = |c: &&GestureCell| {
        matches!(c.value, CellValue::Valid(_))
            && c.route.is_some()
            && !c.dropped
            && c.conflict.is_none()
            && !c.define_clash
            && !c.define_unnamed
    };
    let defining: Vec<&GestureCell> = gesture
        .cells
        .iter()
        .filter(applicable)
        .filter(|c| c.define.is_some())
        .collect();
    if !defining.is_empty() {
        return compile_define(&defining);
    }
    // (kind, member, target) -> composed value, in the order of first appearance; `at_of` indexes it.
    let mut values: Vec<((String, String, Uuid), Value)> = Vec::new();
    let mut at_of: BTreeMap<(String, String, Uuid), usize> = BTreeMap::new();
    let mut identifier_routes: BTreeMap<(String, String), BTreeSet<Uuid>> = BTreeMap::new();
    let mut bases: BTreeMap<(String, String, Uuid), MemberBase> = BTreeMap::new();
    for cell in &gesture.cells {
        let (CellValue::Valid(v), Some((kind, path)), false, None) =
            (&cell.value, &cell.route, cell.dropped, cell.conflict)
        else {
            continue;
        };
        // A strip-editor cell compiles through `Session::strip_edit`, in the shell.
        if kind == STRIP_ROUTE {
            continue;
        }
        for (path, v) in std::iter::once((path, v)).chain(cell.extra.iter().map(|(p, x)| (p, x))) {
            let member = route_member(kind, path).unwrap_or_else(|| path.clone());
            let mltext = route_type(kind, path).is_some_and(|t| matches!(t.inner(), FieldType::MlText));
            let leaf = path != &member || mltext;
            if route_identifier(kind, path) {
                identifier_routes
                    .entry((kind.clone(), path.clone()))
                    .or_default()
                    .extend(cell.targets.iter().copied());
            }
            for t in &cell.targets {
                let key = (kind.clone(), member.clone(), *t);
                let at = at_of.get(&key).copied();
                let new = if leaf {
                    // The stored member, read once per target; a second leaf composes onto the first one's result.
                    let stored = match bases.get(&key) {
                        Some(b) => b.clone(),
                        None => {
                            let b = base(kind, *t, &member);
                            bases.insert(key.clone(), b.clone());
                            b
                        }
                    };
                    let b = match at {
                        Some(i) => MemberBase {
                            value: Some(values[i].1.clone()),
                            language: stored.language,
                        },
                        None => stored,
                    };
                    match compose_leaf(&member, path, &b, v, mltext) {
                        Some(x) => x,
                        None => continue,
                    }
                } else {
                    v.clone()
                };
                match at {
                    Some(i) => values[i].1 = new,
                    None => {
                        at_of.insert(key.clone(), values.len());
                        values.push((key, new));
                    }
                }
            }
        }
    }
    if values.is_empty() {
        return None;
    }
    let identifier = identifier_routes.values().any(|s| s.len() > 1);
    let targets = values.len();
    let first = &values[0];
    let same = values
        .iter()
        .all(|((k, m, _), v)| *k == first.0.0 && *m == first.0.1 && *v == first.1);
    if same {
        let ids: Vec<String> = values.iter().map(|((_, _, t), _)| t.to_string()).collect();
        return Some(Compiled {
            name: "fields.set",
            params: json!({"kind": first.0.0, "targets": ids, "path": first.0.1, "value": first.1}),
            targets,
            identifier,
        });
    }
    // One entry per (kind, member, value), its targets in order of appearance; a value is indexed by its JSON text
    // and compared with `==` inside that bucket.
    let mut entries: Vec<(String, String, Value, Vec<String>)> = Vec::new();
    let mut entry_of: BTreeMap<(String, String, String), Vec<usize>> = BTreeMap::new();
    for ((k, m, t), v) in &values {
        let bucket = entry_of.entry((k.clone(), m.clone(), v.to_string())).or_default();
        match bucket.iter().copied().find(|i| entries[*i].2 == *v) {
            Some(i) => entries[i].3.push(t.to_string()),
            None => {
                bucket.push(entries.len());
                entries.push((k.clone(), m.clone(), v.clone(), vec![t.to_string()]));
            }
        }
    }
    let edits: Vec<Value> = entries
        .into_iter()
        .map(|(k, m, v, ts)| json!({"kind": k, "targets": ts, "path": m, "value": v}))
        .collect();
    Some(Compiled {
        name: "fields.set_many",
        params: json!({ "edits": edits }),
        targets,
        identifier,
    })
}

/// The `potential.define` of the defining cells of one row (the G-PP triage's row 8): at the row's CP,
/// with the name typed in the row's Name cell and the Type, Voltage and `env.ac_dc` the cells set; `None` when no
/// cell names it (a clear, or Type and Voltage alone, which [`mark_conflicts`] refuses).
fn compile_define(cells: &[&GestureCell]) -> Option<Compiled> {
    let at = cells[0].define.as_ref()?;
    let mut params = json!({
        "at": {"cp": {"function": at.function.to_string(), "idx": at.idx}},
    });
    let mut named = false;
    for cell in cells {
        let (Some((_, path)), CellValue::Valid(v)) = (&cell.route, &cell.value) else {
            continue;
        };
        for (path, v) in std::iter::once((path, v)).chain(cell.extra.iter().map(|(p, x)| (p, x))) {
            if v.is_null() {
                continue;
            }
            match path.as_str() {
                "name" => {
                    params["name"] = v.clone();
                    named = true;
                }
                "ptype" | "voltage" => params[path.as_str()] = v.clone(),
                "env.ac_dc" => params["env"] = json!({ "ac_dc": v }),
                _ => continue,
            }
        }
    }
    named.then_some(Compiled {
        name: "potential.define",
        params,
        targets: 1,
        identifier: false,
    })
}
/// The text key of the word of a navigator's table rows, singular for one (`terminal`, `structure nodes`).
pub fn rows_word_key(kind: NavKind, n: usize) -> &'static str {
    let (one, many) = match kind {
        NavKind::Pages => (
            ew_commands::text_key!("table-row-pages"),
            ew_commands::text_key!("table-rows-pages"),
        ),
        NavKind::Devices => (
            ew_commands::text_key!("table-row-devices"),
            ew_commands::text_key!("table-rows-devices"),
        ),
        NavKind::Connections => (
            ew_commands::text_key!("table-row-connections"),
            ew_commands::text_key!("table-rows-connections"),
        ),
        NavKind::Strips => (
            ew_commands::text_key!("table-row-strips"),
            ew_commands::text_key!("table-rows-strips"),
        ),
        NavKind::Cables => (
            ew_commands::text_key!("table-row-cables"),
            ew_commands::text_key!("table-rows-cables"),
        ),
        NavKind::Plc => (
            ew_commands::text_key!("table-row-plc"),
            ew_commands::text_key!("table-rows-plc"),
        ),
        NavKind::Potentials => (
            ew_commands::text_key!("table-row-potentials"),
            ew_commands::text_key!("table-rows-potentials"),
        ),
        NavKind::Ips => (
            ew_commands::text_key!("table-row-ips"),
            ew_commands::text_key!("table-rows-ips"),
        ),
        NavKind::Structure => (
            ew_commands::text_key!("table-row-structure"),
            ew_commands::text_key!("table-rows-structure"),
        ),
        NavKind::Parts => (
            ew_commands::text_key!("table-row-parts"),
            ew_commands::text_key!("table-rows-parts"),
        ),
        NavKind::Blocks => (
            ew_commands::text_key!("table-row-blocks"),
            ew_commands::text_key!("table-rows-blocks"),
        ),
        NavKind::Unplaced => (
            ew_commands::text_key!("table-row-unplaced"),
            ew_commands::text_key!("table-rows-unplaced"),
        ),
        NavKind::Macros => (
            ew_commands::text_key!("table-row-macros"),
            ew_commands::text_key!("table-rows-macros"),
        ),
        NavKind::Libraries => (
            ew_commands::text_key!("table-row-libraries"),
            ew_commands::text_key!("table-rows-libraries"),
        ),
        // The Messages pane has no table mode (`navigator.table` fails); its rows are findings.
        NavKind::Messages => (
            ew_commands::text_key!("table-row-messages"),
            ew_commands::text_key!("table-rows-messages"),
        ),
    };
    if n == 1 { one } else { many }
}

/// One segment of a row path below its member.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Seg {
    /// A member name.
    Member(String),
    /// A list index.
    Index(usize),
}

/// The segments of a row path (`[0].qty` → index 0, member `qty`).
pub fn segments(path: &str) -> Vec<Seg> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '.' => {
                if !cur.is_empty() {
                    out.push(Seg::Member(std::mem::take(&mut cur)));
                }
            }
            '[' => {
                if !cur.is_empty() {
                    out.push(Seg::Member(std::mem::take(&mut cur)));
                }
                let mut inner = String::new();
                for d in chars.by_ref() {
                    if d == ']' {
                        break;
                    }
                    inner.push(d);
                }
                match inner.parse::<usize>() {
                    Ok(i) => out.push(Seg::Index(i)),
                    Err(_) => out.push(Seg::Member(inner)),
                }
            }
            other => cur.push(other),
        }
    }
    if !cur.is_empty() {
        out.push(Seg::Member(cur));
    }
    out
}

/// The settable member's new value with the leaf at `row_path` (a path below `member`) replaced by `leaf` (§6.6):
/// `base` is the member's current value (`None`: absent, starts from an empty object). A `null` leaf removes the
/// member. `None` when the path does not lie below the member or crosses a value that is not an object or list.
pub fn compose(member: &str, row_path: &str, base: Option<&Value>, leaf: &Value) -> Option<Value> {
    if row_path == member {
        return Some(leaf.clone());
    }
    let rest = row_path.strip_prefix(member)?;
    if !(rest.starts_with('.') || rest.starts_with('[')) {
        return None;
    }
    let mut root = base.cloned().unwrap_or_else(|| Value::Object(Default::default()));
    let segs = segments(rest);
    let (last, parents) = segs.split_last()?;
    let mut cur = &mut root;
    for s in parents {
        cur = match s {
            Seg::Member(m) => {
                let obj = cur.as_object_mut()?;
                obj.entry(m.clone())
                    .or_insert_with(|| Value::Object(Default::default()))
            }
            Seg::Index(i) => cur.as_array_mut()?.get_mut(*i)?,
        };
    }
    match last {
        Seg::Member(m) => {
            let obj = cur.as_object_mut()?;
            if leaf.is_null() {
                obj.remove(m);
            } else {
                obj.insert(m.clone(), leaf.clone());
            }
        }
        Seg::Index(i) => *cur.as_array_mut()?.get_mut(*i)? = leaf.clone(),
    }
    Some(root)
}
