// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Table mode (WP-S2.06 part 3, §8; D10 to D13): the state of a navigator's table, its rows, the cell cursor, the
//! closed table of editable cells, and the compile rule that turns one gesture (a cell, a column over several rows, a
//! fill-down, a pattern fill, a paste or a clear) into exactly one command.
//!
//! **Rows** are the table rows of the navigator's [`NavTable`] that pass the filter, sorted by the sort column (natural,
//! ascending or descending, ties by node order); [`table_rows`]. **Columns** on show are the chosen ones in the chosen
//! order; the key column cannot be hidden and stays first.
//!
//! **Editable cells** (§8.2): a column with a route of §8.2's table (`NavColumn::route`), on a row with records of the
//! route's kind. Every other cell is read-only with the first reason of: "derived", "generated", "no record of this
//! kind", "edit in the property grid", "no command yet" ([`cell_reason`]).
//!
//! **Compile** (§8.3, D11, D13): the gesture's cells in (row display order, column visible order); each valid cell
//! gives a (target, path, value) triple per target. A cell one of whose (target, path) pairs an earlier cell of the
//! gesture sets to a different value is invalid (`"same record as row <n>"`); with the same value it is merged. A leaf
//! below a settable member (`props.colour`) is composed into the member per target from the target's stored member
//! value, as the property grid composes it. The result is `fields.set` when every target gets the same (kind, member,
//! value), else one `fields.set_many` with one entry per (kind, member, value) in the order of first appearance.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::pattern::{RowError, Template};
use super::{NavKind, NavNode, NavRole, NavTable, Navigator, Query, SortBy, VisRow};
use ew_api::output::{EditRoute, FieldType, StripCell, Uuid, field_catalogue};

/// The pseudo-route kind of a strip-editor cell (WP-S2.07 §6.3): its cells compile through `Session::strip_edit`, not
/// through `parse_field_input`. The path is `group`, `bridge_part:<strip>:<bridge>` (one key per bridge group, so that
/// two rows of one group setting different parts are a conflict, "same bridge as row `<n>`") or `accessory_part:<index>`.
pub const STRIP_ROUTE: &str = "@strip";

/// The route of a strip-editor cell of `node` in `column` (WP-S2.07 §6.2): Group and Bridge part on a terminal row, the
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

/// The records a strip-editor cell of `node` on `path` is keyed by: a Group cell's terminal device; a Bridge part
/// cell's and an accessory row's strip device (so that the conflict rule reads one bridge group, or one accessory, as
/// one record).
pub fn strip_targets(table: &NavTable, node: &NavNode, path: &str) -> Vec<Uuid> {
    if path == "group" {
        return node.records.clone();
    }
    node.parent
        .and_then(|p| table.nodes.get(p as usize))
        .map(|s| s.records.clone())
        .unwrap_or_default()
}

/// The terminal device of a terminal row (its key's last segment `terminal:<uuid>`).
pub fn row_terminal(row_key: &str) -> Option<Uuid> {
    row_key
        .rsplit('/')
        .next()?
        .strip_prefix("terminal:")?
        .parse()
        .ok()
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

/// The strip editor's read-only reasons (WP-S2.07 §6.2): `Some(None)` for an editable strip-editor cell, `Some(reason)`
/// for an accessory row's other cells, the Pos column and a Bridge part cell of a terminal without a bridge; `None` for
/// every other cell (the rules of §8.2 apply).
fn strip_reason(table: &NavTable, node: &NavNode, column: usize) -> Option<Option<String>> {
    if table.kind != NavKind::Strips {
        return None;
    }
    let c = table.columns.get(column)?;
    let editable = strip_route(table, node, column).is_some();
    if node.role == NavRole::Accessory {
        return Some((!editable).then(|| crate::strings::text("table-accessory-row").to_owned()));
    }
    if c.key == "pos" {
        return Some(Some(crate::strings::text("table-pos-move").to_owned()));
    }
    if node.role == NavRole::Terminal {
        return match c.strip_cell {
            Some(StripCell::BridgePart) if !editable => {
                Some(Some(crate::strings::text("table-no-bridge").to_owned()))
            }
            Some(StripCell::Group | StripCell::BridgePart) => Some(None),
            _ => None,
        };
    }
    None
}

/// The state of a navigator in table mode.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct TableMode {
    /// The column keys on show in their order; `None`: every column in the table's order.
    pub(crate) columns: Option<Vec<String>>,
    /// The cell cursor's column key (the row is the navigator's row cursor).
    pub(crate) column: Option<String>,
    /// The cell being edited.
    pub(crate) edit: Option<CellEdit>,
    /// The parse error or refusal under the edited cell.
    pub(crate) error: Option<String>,
    /// The pattern fill dialog's pattern, while the dialog is open.
    pub(crate) pattern: Option<String>,
    /// WP-S2.07 §6.5: the chooser list of `strip.add_accessory` while several parts fit (doc 16a §6.22).
    pub(crate) chooser: Option<Box<Chooser>>,
}

/// The chooser list of `strip.add_accessory` (WP-S2.07 §6.5; doc 16a §6.22, its first use): open at a row while
/// several part copies fit the accessory; picking an item dispatches `strip.add_accessory` again with `kind` and the
/// item's `part`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Chooser {
    /// The row it opened at.
    pub row: String,
    /// The accessory kind's word (`end_clamp`).
    pub kind: String,
    /// The items: (`<part number> (<manufacturer>) · <description>`, the `part` text that picks it).
    pub items: Vec<(String, String)>,
}

impl TableMode {
    /// The cell being edited.
    pub fn edit(&self) -> Option<&CellEdit> {
        self.edit.as_ref()
    }

    /// The chooser list, while it is open (WP-S2.07 §6.5).
    pub fn chooser(&self) -> Option<&Chooser> {
        self.chooser.as_deref()
    }

    /// The error under the edited cell.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The pattern fill dialog's pattern, while it is open.
    pub fn pattern(&self) -> Option<&str> {
        self.pattern.as_deref()
    }

    /// The cell cursor's column key, as chosen (see [`TableMode::cursor_column`] for the one on show).
    pub fn column(&self) -> Option<&str> {
        self.column.as_deref()
    }

    /// The column indices on show, in order: the key column first, then the chosen ones (every one without a choice).
    pub fn shown_columns(&self, table: &NavTable) -> Vec<usize> {
        let key = table.columns.iter().position(|c| c.key_column);
        let mut out: Vec<usize> = key.into_iter().collect();
        match &self.columns {
            None => out.extend((0..table.columns.len()).filter(|i| Some(*i) != key)),
            Some(keys) => {
                for k in keys {
                    if let Some(i) = table.columns.iter().position(|c| c.key == *k)
                        && !out.contains(&i)
                    {
                        out.push(i);
                    }
                }
            }
        }
        out
    }

    /// The cursor's column on show: the chosen one while it is on show, else the first column on show.
    pub fn cursor_column(&self, table: &NavTable) -> Option<usize> {
        let shown = self.shown_columns(table);
        self.column
            .as_ref()
            .and_then(|c| table.columns.iter().position(|x| x.key == *c))
            .filter(|i| shown.contains(i))
            .or_else(|| shown.first().copied())
    }
}

/// The cell being edited and its buffer.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CellEdit {
    /// The row's key.
    pub row: String,
    /// The column's key.
    pub column: String,
    /// The text as typed.
    pub buffer: String,
}

/// The rows of the table on show (§8.1): the table rows that pass the filter, sorted by the sort column (or the
/// label), natural, ascending or descending, ties by node order. One level, a pure function.
pub fn table_rows(table: &NavTable, sort: &SortBy, descending: bool, query: &Query) -> Vec<VisRow> {
    let col = match sort {
        SortBy::Label | SortBy::Order => None,
        SortBy::Column(c) => table.columns.iter().position(|x| x.key == *c),
    };
    let by_order = *sort == SortBy::Order;
    let key = |i: usize| {
        let n = &table.nodes[i];
        if by_order {
            // `Order` (WP-S2.07 D2): the node order alone, through the tie-break.
            return None;
        }
        match col {
            Some(c) => n.natural.get(c + 1),
            None => n.natural.first(),
        }
    };
    let filtering = !query.is_empty();
    let mut rows: Vec<usize> = (0..table.nodes.len())
        .filter(|&i| {
            let n = &table.nodes[i];
            n.table_row && (!filtering || query.matches(&n.tokens, &n.facets, &n.label))
        })
        .collect();
    rows.sort_by(|&a, &b| {
        if by_order && descending {
            // `Order` descending: the node order reversed.
            return b.cmp(&a);
        }
        let k = key(a).cmp(&key(b));
        let k = if descending { k.reverse() } else { k };
        k.then(a.cmp(&b))
    });
    let size = rows.len();
    rows.into_iter()
        .enumerate()
        .map(|(k, i)| VisRow {
            node: u32::try_from(i).unwrap_or(u32::MAX),
            level: 1,
            expanded: None,
            position: k + 1,
            set_size: size,
        })
        .collect()
}

/// The records of a row of the route's kind (D10): a conductor route takes a core row's conductor (the records after
/// its cable), a potential definition route a potential row's record when it is a `potential_def` (not the bond row's,
/// nor a connection point naming the potential), every other route the row's records; deduplicated, in record order.
pub fn row_targets(node: &NavNode, route_kind: &str) -> Vec<Uuid> {
    let picked: Vec<Uuid> = match route_kind {
        "conductor" if node.role == NavRole::Conductor => node.records.iter().skip(1).copied().collect(),
        "conductor" => Vec::new(),
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

/// Where a cell of a potential known only at connection points defines it (the G-PP triage's row 8, amending ID-752):
/// the CP (`NavNode::define_at`, the row's minimum member CP). The definition's name is the one typed in the row's Name
/// cell (WP-S1.16 D4, FT-577), never the name on show.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DefineAt {
    /// The function of the CP.
    pub function: Uuid,
    /// The CP index.
    pub idx: u16,
}

/// The paths of a potential definition that a potential without one can be given in table mode: Name, Type and
/// Voltage (WP-S1.16 D4: the Name cell defines the row; Type and Voltage only with a name in the same gesture).
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

/// Why a cell is read-only (§8.2's precedence), `None` for an editable cell: "derived", "generated", "no record of this
/// kind", "edit in the property grid", "no command yet".
pub fn cell_reason(table: &NavTable, node: &NavNode, column: usize) -> Option<String> {
    let c = table.columns.get(column)?;
    if let Some(reason) = strip_reason(table, node, column) {
        return reason;
    }
    if c.derived {
        return Some(crate::strings::text("table-reason-derived").to_owned());
    }
    if table.kind == NavKind::Pages && node.badges.iter().any(|b| b == "generated") {
        return Some(crate::strings::text("table-reason-generated").to_owned());
    }
    let Some((kind, path)) = &c.route else {
        return Some(
            c.read_only
                .clone()
                .unwrap_or_else(|| crate::strings::text("table-reason-no-command").to_owned()),
        );
    };
    if row_targets(node, kind).is_empty() && define_point(node, kind, path).is_none() {
        return Some(crate::strings::text("table-no-record").to_owned());
    }
    match route_type(kind, path) {
        Some(ty) if is_scalar(&ty) => None,
        Some(_) => Some(crate::strings::text("table-reason-property-grid").to_owned()),
        None => Some(crate::strings::text("table-reason-no-command").to_owned()),
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
    /// A defining Type or Voltage cell whose row gets no name in the gesture (WP-S1.16 D4, FT-577: "Name the
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

    /// The reason a blocking cell is invalid (`None` for an ambiguous one and an applicable one).
    pub fn reason(&self) -> Option<String> {
        if let Some(n) = self.conflict {
            // WP-S2.07 §6.3: two rows of one bridge group with different parts.
            if self
                .route
                .as_ref()
                .is_some_and(|(k, p)| k == STRIP_ROUTE && p.starts_with("bridge_part:"))
            {
                return Some(format!("same bridge as row {n}"));
            }
            return Some(format!("same record as row {n}"));
        }
        if self.define_clash {
            return Some(crate::strings::text("table-define-alone").to_owned());
        }
        if self.define_unnamed {
            return Some(crate::strings::text("table-define-name-first").to_owned());
        }
        match &self.value {
            CellValue::Invalid(r) => Some(r.clone()),
            _ => None,
        }
    }
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
    /// The word of the card's title (`paste`, `fill down`).
    pub fn word(self) -> &'static str {
        match self {
            GestureKind::Edit => "edit",
            GestureKind::FillDown => "fill down",
            GestureKind::PatternFill => "pattern fill",
            GestureKind::Paste => "paste",
            GestureKind::Clear => "clear",
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

    /// The card's title: "Preview: paste 12 cells on 12 terminals · 2 invalid".
    pub fn title(&self) -> String {
        let cells = self.cells.iter().filter(|c| !c.dropped).count();
        let mut t = format!(
            "Preview: {} {cells} {} on {} {}",
            self.kind.word(),
            if cells == 1 { "cell" } else { "cells" },
            self.rows(),
            rows_word(self.nav, self.rows())
        );
        let blocking = self.blocking();
        if blocking > 0 {
            t.push_str(&format!(" · {blocking} invalid"));
        }
        t
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
/// language (a language object's text is replaced in the source language, WP-S2.05 §6.5).
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
    crate::properties::compose(member, path, base.value.as_ref(), leaf)
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
    // WP-S1.16 D4 (FT-577): a potential is defined under a name the user types; Type, Voltage or `env.ac_dc` typed on
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
        // A strip-editor cell compiles through `Session::strip_edit` (WP-S2.07 §6.3), in the shell.
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

/// The `potential.define` of the defining cells of one row (the G-PP triage's row 8; WP-S1.16 D4): at the row's CP,
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

/// The node indices a gesture acts on (§8.3 to §8.6): the marked rows on show, in display order, else the cursor row.
pub fn gesture_rows(nav: &Navigator) -> Vec<u32> {
    let Some(table) = nav.table() else {
        return Vec::new();
    };
    let rows = nav.visible();
    let marked: Vec<u32> = rows
        .iter()
        .map(|r| r.node)
        .filter(|i| nav.marked().contains(&table.nodes[*i as usize].key))
        .collect();
    if !marked.is_empty() {
        return marked;
    }
    nav.cursor_key()
        .and_then(|k| {
            rows.iter()
                .find(|r| table.nodes[r.node as usize].key == k)
                .map(|r| r.node)
        })
        .into_iter()
        .collect()
}

/// The cell text of node `node` in the column with key `key` (empty for an unknown key).
pub fn cell_text(table: &NavTable, node: u32, key: &str) -> String {
    table
        .columns
        .iter()
        .position(|c| c.key == key)
        .and_then(|c| table.nodes.get(node as usize)?.cells.get(c).cloned())
        .unwrap_or_default()
}

/// The texts a pattern fill produces (§8.5) over [`gesture_rows`] in the cursor column: `{n}` counts the rows from 1,
/// `{prev}` starts with the cursor column's text of the row on show directly above the first one (empty when there is
/// none). With `limit`, only the first `limit` rows are computed (the dialog's preview). A row in error passes the
/// empty text on as the next row's `{prev}`, so an error cannot grow later rows.
pub fn pattern_texts(
    nav: &Navigator,
    template: &Template,
    limit: Option<usize>,
) -> Vec<Result<String, RowError>> {
    let (Some(table), Some(mode)) = (nav.table(), nav.table_mode()) else {
        return Vec::new();
    };
    let Some(col) = mode.cursor_column(table) else {
        return Vec::new();
    };
    let rows = gesture_rows(nav);
    let vis = nav.visible();
    let mut prev = rows
        .first()
        .and_then(|first| vis.iter().position(|r| r.node == *first))
        .and_then(|at| at.checked_sub(1))
        .and_then(|at| table.nodes[vis[at].node as usize].cells.get(col).cloned())
        .unwrap_or_default();
    rows.iter()
        .take(limit.unwrap_or(usize::MAX))
        .enumerate()
        .map(|(k, i)| {
            let cell = |key: &str| cell_text(table, *i, key);
            let n = i64::try_from(k + 1).unwrap_or(i64::MAX);
            let out = template.render(n, &cell, &prev);
            prev = out.clone().unwrap_or_default();
            out
        })
        .collect()
}

/// The plural word of a navigator's table rows (`terminals`, `structure nodes`).
pub fn rows_word(kind: NavKind, n: usize) -> String {
    let (one, many) = match kind {
        NavKind::Pages => ("page", "pages"),
        NavKind::Devices => ("device", "devices"),
        NavKind::Connections => ("connection", "connections"),
        NavKind::Strips => ("terminal", "terminals"),
        NavKind::Cables => ("conductor", "conductors"),
        NavKind::Plc => ("channel", "channels"),
        NavKind::Potentials => ("potential", "potentials"),
        NavKind::Ips => ("interruption point", "interruption points"),
        NavKind::Structure => ("structure node", "structure nodes"),
        NavKind::Parts => ("part", "parts"),
        NavKind::Blocks => ("report block", "report blocks"),
        // The Messages pane has no table mode (`navigator.table` fails); its rows are findings.
        NavKind::Messages => ("finding", "findings"),
    };
    if n == 1 { one } else { many }.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid(n: u8) -> Uuid {
        Uuid::from_bytes([n; 16])
    }

    fn cell(row: usize, column: &str, path: &str, targets: &[u8], v: Value) -> GestureCell {
        GestureCell {
            row: format!("r{row}"),
            row_label: format!("R{row}"),
            row_number: row,
            column: column.to_owned(),
            column_label: column.to_owned(),
            text: v.to_string(),
            route: Some(("device".to_owned(), path.to_owned())),
            targets: targets.iter().map(|t| uuid(*t)).collect(),
            value: CellValue::Valid(v),
            conflict: None,
            dropped: false,
            define: None,
            extra: Vec::new(),
            define_clash: false,
            define_unnamed: false,
        }
    }

    fn gesture(cells: Vec<GestureCell>) -> Gesture {
        Gesture {
            kind: GestureKind::Paste,
            nav: NavKind::Strips,
            cells,
            ignored: Vec::new(),
        }
    }

    fn no_base(_: &str, _: Uuid, _: &str) -> MemberBase {
        MemberBase {
            value: None,
            language: "en".to_owned(),
        }
    }

    #[test]
    fn one_value_on_every_target_is_one_fields_set() {
        let g = gesture(vec![
            cell(1, "state", "state", &[1], json!("spare")),
            cell(2, "state", "state", &[2], json!("spare")),
        ]);
        let c = compile(&g, &mut no_base).unwrap();
        assert_eq!(c.name, "fields.set");
        assert_eq!(c.targets, 2);
        assert_eq!(c.params["path"], "state");
        assert_eq!(c.params["targets"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn different_values_are_one_fields_set_many_in_first_appearance_order() {
        let g = gesture(vec![
            cell(1, "state", "state", &[1], json!("spare")),
            cell(2, "state", "state", &[2], json!("used")),
            cell(3, "state", "state", &[3], json!("spare")),
        ]);
        let c = compile(&g, &mut no_base).unwrap();
        assert_eq!(c.name, "fields.set_many");
        let edits = c.params["edits"].as_array().unwrap();
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0]["value"], "spare");
        assert_eq!(edits[0]["targets"].as_array().unwrap().len(), 2);
        assert_eq!(edits[1]["value"], "used");
    }

    #[test]
    fn the_same_record_twice_with_another_value_is_invalid_with_the_same_value_merged() {
        let mut g = gesture(vec![
            cell(1, "state", "state", &[1], json!("spare")),
            cell(2, "state", "state", &[1], json!("used")),
            cell(3, "state", "state", &[1], json!("spare")),
        ]);
        mark_conflicts(&mut g);
        assert_eq!(g.cells[1].conflict, Some(1));
        assert_eq!(g.cells[1].reason().as_deref(), Some("same record as row 1"));
        assert_eq!(g.cells[2].conflict, None, "the same value is merged");
        assert_eq!(g.blocking(), 1);
        let c = compile(&g, &mut no_base).unwrap();
        assert_eq!((c.name, c.targets), ("fields.set", 1));
        // Dropping the conflicting cell clears it.
        g.cells[1].dropped = true;
        mark_conflicts(&mut g);
        assert_eq!(g.blocking(), 0);
    }

    #[test]
    fn a_leaf_is_composed_into_its_member_per_target() {
        let mut g = gesture(vec![
            cell(1, "colour", "props.colour", &[1], json!("BU")),
            cell(1, "cross_section", "props.csa", &[1], json!("1.5")),
        ]);
        for c in &mut g.cells {
            c.route = Some(("potential_def".to_owned(), c.route.clone().unwrap().1));
        }
        let mut base = |_: &str, _: Uuid, _: &str| MemberBase {
            value: Some(json!({"colour": "RD", "keep": 1})),
            language: "en".to_owned(),
        };
        let c = compile(&g, &mut base).unwrap();
        assert_eq!(c.name, "fields.set");
        assert_eq!(c.params["path"], "props");
        assert_eq!(
            c.params["value"],
            json!({"colour": "BU", "csa": "1.5", "keep": 1})
        );
    }

    #[test]
    fn a_language_object_keeps_its_other_languages() {
        let b = MemberBase {
            value: Some(json!({"en": "Old", "de": "Alt"})),
            language: "en".to_owned(),
        };
        assert_eq!(
            compose_leaf("function_text", "function_text", &b, &json!("New"), true),
            Some(json!({"en": "New", "de": "Alt"}))
        );
        assert_eq!(
            compose_leaf("function_text", "function_text", &b, &json!(""), true),
            Some(json!({"de": "Alt"}))
        );
        let plain = MemberBase {
            value: Some(json!("Old")),
            language: "en".to_owned(),
        };
        assert_eq!(
            compose_leaf("function_text", "function_text", &plain, &json!("New"), true),
            Some(json!("New"))
        );
    }

    /// The G-PP triage's row 8: the printed Type and Voltage forms parse; `AC`/`DC` also set `env.ac_dc`.
    #[test]
    fn potential_inputs_take_the_printed_forms() {
        let ac = Some(("env.ac_dc".to_owned(), json!("ac")));
        let dc = Some(("env.ac_dc".to_owned(), json!("dc")));
        for (text, want) in [
            ("230 V AC", ("230", ac.clone())),
            ("230VAC", ("230", ac.clone())),
            ("24 v dc", ("24", dc.clone())),
            ("24 DC", ("24", dc)),
            ("24 V", ("24", None)),
            ("24", ("24", None)),
            ("0.5 V AC", ("0.5", ac)),
        ] {
            assert_eq!(
                potential_input("potential_def", "voltage", text),
                (want.0.to_owned(), want.1),
                "{text}"
            );
        }
        assert_eq!(potential_input("potential_def", "ptype", "L+").0, "l_plus");
        assert_eq!(potential_input("potential_def", "ptype", " L- ").0, "l_minus");
        assert_eq!(potential_input("potential_def", "ptype", "M").0, "M");
        // Another route's text is untouched.
        assert_eq!(
            potential_input("device", "voltage", "24 V"),
            ("24 V".to_owned(), None)
        );
    }

    /// The G-PP triage's row 8 as amended by WP-S1.16 D4 (FT-577): defining cells of one row compile to one
    /// `potential.define` with the typed name and every value they set; Type or Voltage without a Name refuses with
    /// "Name the potential first"; with a cell of another row or a defined record each is a clash; a clear defines
    /// nothing.
    #[test]
    fn defining_cells_compile_to_one_potential_define() {
        let define = |row: usize, path: &str, v: Value| {
            let mut c = cell(row, path, path, &[], v);
            c.route = Some(("potential_def".to_owned(), path.to_owned()));
            c.define = Some(DefineAt {
                function: uuid(9),
                idx: 2,
            });
            c
        };
        let mut voltage = define(1, "voltage", json!("230"));
        voltage.extra = vec![("env.ac_dc".to_owned(), json!("ac"))];
        let mut g = gesture(vec![
            define(1, "name", json!("P1")),
            define(1, "ptype", json!("l")),
            voltage.clone(),
        ]);
        mark_conflicts(&mut g);
        assert_eq!(g.blocking(), 0);
        let c = compile(&g, &mut no_base).unwrap();
        assert_eq!(c.name, "potential.define");
        assert_eq!(
            c.params,
            json!({"at": {"cp": {"function": uuid(9).to_string(), "idx": 2}}, "name": "P1", "ptype": "l",
                   "voltage": "230", "env": {"ac_dc": "ac"}})
        );

        // A name alone defines the potential with that name.
        let mut name = gesture(vec![define(1, "name", json!("24V-AUX"))]);
        mark_conflicts(&mut name);
        assert_eq!(name.blocking(), 0);
        assert_eq!(
            compile(&name, &mut no_base).unwrap().params,
            json!({"at": {"cp": {"function": uuid(9).to_string(), "idx": 2}}, "name": "24V-AUX"})
        );

        // Type or Voltage without a name: refused, nothing compiles; the name typed with them lifts it.
        let mut unnamed = gesture(vec![define(1, "ptype", json!("l")), voltage]);
        mark_conflicts(&mut unnamed);
        assert_eq!(unnamed.blocking(), 2);
        assert_eq!(
            unnamed.cells[0].reason().as_deref(),
            Some("Name the potential first")
        );
        assert!(compile(&unnamed, &mut no_base).is_none());
        let mut cleared_name = gesture(vec![
            define(1, "name", Value::Null),
            define(1, "ptype", json!("l")),
        ]);
        mark_conflicts(&mut cleared_name);
        assert!(cleared_name.cells[1].define_unnamed, "a cleared name is no name");

        let mut two = gesture(vec![
            define(1, "name", json!("P1")),
            define(2, "name", json!("P2")),
        ]);
        mark_conflicts(&mut two);
        assert_eq!(two.blocking(), 2);
        assert!(compile(&two, &mut no_base).is_none());
        two.cells[1].dropped = true;
        mark_conflicts(&mut two);
        assert_eq!(two.blocking(), 0, "dropping the other row clears the clash");

        let mut mixed = gesture(vec![
            define(1, "name", json!("P1")),
            cell(2, "state", "state", &[2], json!("spare")),
        ]);
        mark_conflicts(&mut mixed);
        assert!(mixed.cells[0].define_clash);
        assert_eq!(
            mixed.cells[0].reason().as_deref(),
            Some("a potential without a definition is defined on its own row alone")
        );

        let mut cleared = gesture(vec![define(1, "ptype", Value::Null)]);
        mark_conflicts(&mut cleared);
        assert_eq!(cleared.blocking(), 0, "a clear is not refused");
        assert!(compile(&cleared, &mut no_base).is_none());
    }

    /// The G-PP triage's row 8: a defined row's Voltage with its current is two members of one record.
    #[test]
    fn a_voltage_with_its_current_sets_two_members() {
        let mut c = cell(1, "voltage", "voltage", &[1], json!("24"));
        c.route = Some(("potential_def".to_owned(), "voltage".to_owned()));
        c.extra = vec![("env.ac_dc".to_owned(), json!("dc"))];
        let g = gesture(vec![c]);
        let mut base = |_: &str, _: Uuid, _: &str| MemberBase {
            value: Some(json!({"earthing": "pelv"})),
            language: "en".to_owned(),
        };
        let out = compile(&g, &mut base).unwrap();
        assert_eq!(out.name, "fields.set_many");
        assert_eq!(
            out.params["edits"],
            json!([
                {"kind": "potential_def", "targets": [uuid(1).to_string()], "path": "voltage", "value": "24"},
                {"kind": "potential_def", "targets": [uuid(1).to_string()], "path": "env",
                 "value": {"earthing": "pelv", "ac_dc": "dc"}},
            ])
        );
    }

    #[test]
    fn identifiers_on_several_targets() {
        let g = gesture(vec![
            cell(1, "terminal", "terminal.designation", &[1], json!("1")),
            cell(2, "terminal", "terminal.designation", &[2], json!("2")),
        ]);
        assert!(compile(&g, &mut no_base).unwrap().identifier);
        let one = gesture(vec![cell(
            1,
            "terminal",
            "terminal.designation",
            &[1],
            json!("1"),
        )]);
        assert!(!compile(&one, &mut no_base).unwrap().identifier);
    }

    #[test]
    fn titles() {
        let mut g = gesture(vec![
            cell(1, "state", "state", &[1], json!("spare")),
            cell(2, "state", "state", &[2], json!("x")),
        ]);
        g.cells[1].value = CellValue::Invalid("bad".into());
        assert_eq!(g.title(), "Preview: paste 2 cells on 2 terminals · 1 invalid");
        assert_eq!(rows_word(NavKind::Structure, 2), "structure nodes");
        assert_eq!(rows_word(NavKind::Strips, 1), "terminal");
    }

    /// The straightforward linear-search reference of [`mark_conflicts`] (FT-525).
    fn reference_mark_conflicts(gesture: &mut Gesture) {
        let mut seen: Vec<((Uuid, String, String), Value, usize)> = Vec::new();
        for cell in &mut gesture.cells {
            cell.conflict = None;
            let (CellValue::Valid(v), Some((kind, path)), false) = (&cell.value, &cell.route, cell.dropped)
            else {
                continue;
            };
            let clash = cell.targets.iter().find_map(|t| {
                seen.iter()
                    .find(|(k, val, _)| k.0 == *t && k.1 == *kind && k.2 == *path && val != v)
                    .map(|(_, _, row)| *row)
            });
            if clash.is_some() {
                cell.conflict = clash;
                continue;
            }
            for t in &cell.targets {
                let key = (*t, kind.clone(), path.clone());
                if !seen.iter().any(|(k, _, _)| *k == key) {
                    seen.push((key, v.clone(), cell.row_number));
                }
            }
        }
    }

    /// The straightforward linear-search reference of [`compile`] (FT-525).
    fn reference_compile(
        gesture: &Gesture,
        base: &mut dyn FnMut(&str, Uuid, &str) -> MemberBase,
    ) -> Option<Compiled> {
        let mut values: Vec<((String, String, Uuid), Value)> = Vec::new();
        let mut identifier_routes: Vec<(String, String, BTreeSet<Uuid>)> = Vec::new();
        let mut bases: Vec<((String, String, Uuid), MemberBase)> = Vec::new();
        for cell in &gesture.cells {
            let (CellValue::Valid(v), Some((kind, path)), false, None) =
                (&cell.value, &cell.route, cell.dropped, cell.conflict)
            else {
                continue;
            };
            let member = route_member(kind, path).unwrap_or_else(|| path.clone());
            let mltext = route_type(kind, path).is_some_and(|t| matches!(t.inner(), FieldType::MlText));
            let leaf = path != &member || mltext;
            if route_identifier(kind, path) {
                match identifier_routes
                    .iter_mut()
                    .find(|(k, p, _)| k == kind && p == path)
                {
                    Some((_, _, set)) => set.extend(cell.targets.iter().copied()),
                    None => identifier_routes.push((
                        kind.clone(),
                        path.clone(),
                        cell.targets.iter().copied().collect(),
                    )),
                }
            }
            for t in &cell.targets {
                let key = (kind.clone(), member.clone(), *t);
                let at = values.iter().position(|(k, _)| *k == key);
                let new = if leaf {
                    let stored = match bases.iter().find(|(k, _)| *k == key) {
                        Some((_, b)) => b.clone(),
                        None => {
                            let b = base(kind, *t, &member);
                            bases.push((key.clone(), b.clone()));
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
                    None => values.push((key, new)),
                }
            }
        }
        if values.is_empty() {
            return None;
        }
        let identifier = identifier_routes.iter().any(|(_, _, s)| s.len() > 1);
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
        let mut entries: Vec<(String, String, Value, Vec<String>)> = Vec::new();
        for ((k, m, t), v) in &values {
            match entries
                .iter_mut()
                .find(|(ek, em, ev, _)| ek == k && em == m && ev == v)
            {
                Some((_, _, _, ts)) => ts.push(t.to_string()),
                None => entries.push((k.clone(), m.clone(), v.clone(), vec![t.to_string()])),
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

    /// A few hundred mixed cells (plain fields, composed leaves, identifiers, several targets per cell, repeated
    /// targets, clashing and agreeing values, dropped and invalid cells), from a fixed linear congruential sequence.
    fn mixed_gesture(seed: u64) -> Gesture {
        let mut x = seed;
        let mut next = |m: u64| {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (x >> 33) % m
        };
        let routes = [
            ("device", "state"),
            ("device", "label"),
            ("potential_def", "props.colour"),
            ("potential_def", "props.csa"),
            ("terminal", "terminal.designation"),
        ];
        let mut cells = Vec::new();
        for row in 1..=400usize {
            let (kind, path) = routes[next(routes.len() as u64) as usize];
            let n = 1 + next(3) as usize;
            let targets: Vec<u8> = (0..n).map(|_| next(60) as u8).collect();
            let mut c = cell(row, path, path, &targets, json!(format!("v{}", next(4))));
            c.route = Some((kind.to_owned(), path.to_owned()));
            match next(10) {
                0 => c.dropped = true,
                1 => c.value = CellValue::Invalid("bad".to_owned()),
                _ => {}
            }
            cells.push(c);
        }
        gesture(cells)
    }

    #[test]
    fn indexed_compile_and_conflicts_match_the_linear_reference() {
        for seed in [1, 7, 42, 1234] {
            let mut fast = mixed_gesture(seed);
            let mut slow = fast.clone();
            mark_conflicts(&mut fast);
            reference_mark_conflicts(&mut slow);
            assert_eq!(fast, slow, "conflict marks, seed {seed}");
            assert!(
                fast.cells.iter().any(|c| c.conflict.is_some()),
                "seed {seed} has conflicts"
            );
            let mut calls_fast = Vec::new();
            let mut calls_slow = Vec::new();
            let base = |calls: &mut Vec<(String, Uuid, String)>, k: &str, t: Uuid, m: &str| {
                calls.push((k.to_owned(), t, m.to_owned()));
                MemberBase {
                    value: Some(json!({"colour": "RD", "keep": t.as_bytes()[0]})),
                    language: "en".to_owned(),
                }
            };
            let a = compile(&fast, &mut |k, t, m| base(&mut calls_fast, k, t, m));
            let b = reference_compile(&slow, &mut |k, t, m| base(&mut calls_slow, k, t, m));
            assert_eq!(a, b, "compiled, seed {seed}");
            assert_eq!(a.unwrap().name, "fields.set_many");
            assert_eq!(calls_fast, calls_slow, "base reads, seed {seed}");
        }
    }
}
