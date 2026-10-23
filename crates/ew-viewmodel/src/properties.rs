// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The property grid's panel model (WP-S2.05 part 1, spec §6.2 to §6.11): the sheet the shell fetched, the focused
//! row, the edit buffer, the choices and popups, the inline preview card and the toast, and the pure rules over them —
//! the edit policy of FR-UI-044 ([`policy`]), the composition of a leaf into its settable member ([`compose`]), the
//! read-only reasons ([`read_only_reason`]) and the rows on show ([`visible_rows`]).
//!
//! Only the shell (`ShellModel`'s `props.*` actions) calls the `Session`: once per sheet key for the sheet
//! (`Session::property_sheet`), once per edit for its preview (`Session::preview_with`), then the commit. The card and
//! the toast are the stand-ins of WP-S2.09's preview tab and notifications log (D5).

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_api::Cost;
use ew_api::output::ew_dl::EntityRef;
pub use ew_api::output::{
    Category, EditRoute, FieldReadOnly, FieldType, PropRow, PropertySheet, Provenance, RowAction, RowValue,
    SheetSection,
};
use serde_json::Value;

use crate::strings;
use crate::tools::PreviewState;

/// Above this many targets an edit always goes through the card (FR-UI-044 rule 1, `edit.rs`'s `BULK_ABOVE`).
pub const BULK_TARGETS: usize = 500;

/// What the edit policy decides on a preview (§6.6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Decision {
    /// Commit the kept preview at once.
    Direct,
    /// Show the card; the key names why.
    Card(&'static str),
    /// The preview failed: the row's error state, no card.
    Error,
}

/// The edit policy (FR-UI-044, §6.6): `targets` the command's targets, `identifier` whether the row is an outgoing
/// identifier, `tags` how many tags the preview's derived diff changes, `cost` the command's cost, `ok` whether the
/// preview succeeded.
pub fn policy(targets: usize, identifier: bool, tags: usize, cost: Cost, ok: bool) -> Decision {
    if !ok {
        Decision::Error
    } else if targets > BULK_TARGETS {
        Decision::Card("reason-bulk")
    } else if (identifier && targets > 1) || tags > 1 {
        Decision::Card("reason-identifiers")
    } else if cost == Cost::Bulk {
        Decision::Card("reason-bulk-command")
    } else {
        Decision::Direct
    }
}

/// What the sheet was fetched for: the project session, its revision and the records.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct SheetKey {
    pub epoch: u64,
    pub rev: u64,
    pub ids: Vec<EntityRef>,
}

/// An open edit: the row and its buffer.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EditBuffer {
    /// The row's key.
    pub row: String,
    /// The text as typed.
    pub buffer: String,
    /// The text the editor opened with when it showed the stored value: committing it unchanged writes nothing (the
    /// display form of a value need not parse back, a part's `<part number> (<manufacturer>)`).
    pub initial: Option<String>,
}

/// What the choices under a field are for.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ChoiceKind {
    /// Two readings of an ambiguous number: each value is the leaf's.
    Ambiguous,
    /// The switch between a neutral text and languages (FR-I18N-001): each value is the member's whole new value.
    MlTextSwitch,
    /// The options of a choice editor (§6.5): an enum's words, the colour codes, the part copies, the structure nodes
    /// of a slot's aspect, filtered by the buffer (`Session::field_choices`); each value is the leaf's.
    Options,
}

/// The choices under a field.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Choices {
    /// The row's key.
    pub row: String,
    /// The texts and values, in order.
    pub items: Vec<(String, Value)>,
    /// What they are for.
    pub kind: ChoiceKind,
    /// The highlighted item (Up, Down), if any.
    pub index: Option<usize>,
}

/// Whether a value type has a choice editor with options (§6.5: a combo for enums and colours, a picker for part
/// copies and structure nodes).
pub fn has_options(ty: &FieldType) -> bool {
    matches!(
        ty.inner(),
        FieldType::Enum { .. } | FieldType::Colour | FieldType::Slot
    ) || matches!(ty.inner(), FieldType::Link { kind } if kind == "part")
}

/// `text` with every hyphenated UUID replaced by `‹id›` (no UUID in the transcript or the describe text, WP-S2.05 §9
/// item 5).
pub fn mask_uuids(text: &str) -> String {
    let b = text.as_bytes();
    let is_uuid = |w: &[u8]| {
        w.iter().enumerate().all(|(k, c)| match k {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_hexdigit(),
        })
    };
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < b.len() {
        if i + 36 <= b.len() && is_uuid(&b[i..i + 36]) {
            out.push_str("‹id›");
            i += 36;
            continue;
        }
        let ch = text[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// A popup of the grid.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Popup {
    /// The multi-language popup of a row.
    MlText {
        /// The row's key.
        row: String,
    },
    /// The row's actions as a list.
    Actions {
        /// The row's key.
        row: String,
        /// The highlighted action.
        index: usize,
    },
}

impl Popup {
    /// The transcript's word.
    pub fn word(&self) -> &'static str {
        match self {
            Popup::MlText { .. } => "mltext",
            Popup::Actions { .. } => "actions",
        }
    }

    /// The row the popup belongs to.
    pub fn row(&self) -> &str {
        match self {
            Popup::MlText { row } | Popup::Actions { row, .. } => row,
        }
    }
}

/// An edit compiled into a command, kept with the card so that a stale card can be checked and rebuilt (§6.6).
#[derive(Clone, PartialEq, Debug)]
pub struct PendingEdit {
    /// The row's key.
    pub row: String,
    /// The section's kind.
    pub kind: String,
    /// The targets (UUID text), ascending.
    pub targets: Vec<String>,
    /// The route the command takes.
    pub route: EditRoute,
    /// The row's path below the settable member (`parts[0].qty`), or the member itself.
    pub path: String,
    /// The new value of the edited leaf (`None` for a whole-member value).
    pub leaf: Option<Value>,
    /// The settable member's value on every target when the edit began (`None`: absent or varies).
    pub base: Option<Value>,
    /// The command.
    pub name: String,
    /// Its parameters.
    pub params: Value,
    /// The text as typed (kept on a refusal).
    pub typed: String,
    /// The row's label, for the toast.
    pub label: String,
}

// WP-S2.05 part 2
/// Where a card was opened: the property grid, or the find pane's replace (the transcript's `origin`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CardOrigin {
    /// An edit of the property grid.
    Properties,
    /// A replace of the find pane: the query whose hits it writes, so that a stale card runs it again (§7.4).
    Find {
        /// The query.
        query: crate::find::FindQuery,
        /// The number of hits the replace writes.
        hits: usize,
    },
}

impl CardOrigin {
    /// The transcript's word (`properties`, `find`).
    pub fn word(&self) -> &'static str {
        match self {
            CardOrigin::Properties => "properties",
            CardOrigin::Find { .. } => "find",
        }
    }
}

/// The inline preview card (§6.6).
#[derive(Debug)]
pub struct Card {
    /// The preview.
    pub preview: PreviewState,
    /// The edit it shows.
    pub edit: PendingEdit,
    /// The number of targets.
    pub targets: usize,
    /// The number of changed records (a replace: the number of replaced hits, §7.2).
    pub changes: usize,
    /// Why the card opened (`reason-identifiers`).
    pub reason: &'static str,
    /// Where it was opened (WP-S2.05 part 2).
    pub origin: CardOrigin,
}

/// The toast after a direct commit with several targets (§6.6).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Toast {
    /// `Set <label> on <n> <kind plural>`.
    pub text: String,
    /// Whether Undo is offered.
    pub undo: bool,
    /// Counted up per toast (the window's timer keys on it).
    pub serial: u64,
}

/// The grid's state in the shell.
#[derive(Default, Debug)]
pub struct PropertiesState {
    pub(crate) sheet: Option<Arc<PropertySheet>>,
    pub(crate) key: Option<SheetKey>,
    pub(crate) row: Option<String>,
    pub(crate) edit: Option<EditBuffer>,
    pub(crate) error: Option<(String, String)>,
    pub(crate) choices: Option<Choices>,
    pub(crate) popup: Option<Popup>,
    pub(crate) card: Option<Card>,
    pub(crate) toast: Option<Toast>,
    pub(crate) notes: Vec<String>,
    /// `props.project` shows the project record until the selection changes (its generation then).
    pub(crate) project_mode: Option<u64>,
    pub(crate) toasts: u64,
}

impl PropertiesState {
    /// The sheet on show.
    pub fn sheet(&self) -> Option<&PropertySheet> {
        self.sheet.as_deref()
    }

    /// The open edit.
    pub fn edit(&self) -> Option<&EditBuffer> {
        self.edit.as_ref()
    }

    /// The error under a field: (row key, message).
    pub fn error(&self) -> Option<(&str, &str)> {
        self.error.as_ref().map(|(r, m)| (r.as_str(), m.as_str()))
    }

    /// The choices under a field.
    pub fn choices(&self) -> Option<&Choices> {
        self.choices.as_ref()
    }

    /// The open popup.
    pub fn popup(&self) -> Option<&Popup> {
        self.popup.as_ref()
    }

    /// The preview card.
    pub fn card(&self) -> Option<&Card> {
        self.card.as_ref()
    }

    /// The toast.
    pub fn toast(&self) -> Option<&Toast> {
        self.toast.as_ref()
    }

    /// The notes of the last commit (confusable code points, an alias that resolved a node).
    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    /// The row the cursor was moved to, if it still exists.
    pub fn moved_row(&self) -> Option<&str> {
        self.row.as_deref()
    }
}

/// One row on show with its section.
#[derive(Clone, Copy, Debug)]
pub struct VisibleRow<'s> {
    /// The section.
    pub section: &'s SheetSection,
    /// The row.
    pub row: &'s PropRow,
}

/// The categories of a section in display order with their rows (each category once, rows in sheet order).
pub fn categories(section: &SheetSection) -> Vec<(Category, Vec<&PropRow>)> {
    let mut out: Vec<(Category, Vec<&PropRow>)> = Vec::new();
    for r in &section.rows {
        match out.iter_mut().find(|(c, _)| *c == r.category) {
            Some((_, rows)) => rows.push(r),
            None => out.push((r.category, vec![r])),
        }
    }
    out
}

/// The collapse key of a section's category (`device/identity`).
pub fn collapse_key(kind: &str, category: Category) -> String {
    format!("{kind}/{}", category.word())
}

/// The rows on show: every row of every expanded category, in sheet order.
pub fn visible_rows<'s>(sheet: &'s PropertySheet, collapsed: &BTreeSet<String>) -> Vec<VisibleRow<'s>> {
    let mut out = Vec::new();
    for s in &sheet.sections {
        for (c, rows) in categories(s) {
            if collapsed.contains(&collapse_key(&s.kind, c)) {
                continue;
            }
            out.extend(rows.into_iter().map(|row| VisibleRow { section: s, row }));
        }
    }
    out
}

/// The section and row of `key`.
pub fn find_row<'s>(sheet: &'s PropertySheet, key: &str) -> Option<(&'s SheetSection, &'s PropRow)> {
    sheet
        .sections
        .iter()
        .find_map(|s| s.rows.iter().find(|r| r.key == key).map(|r| (s, r)))
}

/// Whether a row is a derived row (§6.4).
pub fn is_derived(row: &PropRow) -> bool {
    row.key.contains("/derived.")
}

/// The path a row's label is made from: a derived row's own name (`derived.csa`), else its path.
pub trait LabelPath {
    /// The path.
    fn path_for_label(&self) -> String;
}

impl LabelPath for PropRow {
    fn path_for_label(&self) -> String {
        match self.key.split_once('/') {
            Some((_, rest)) if rest.starts_with("derived.") => rest.to_owned(),
            _ => self.path.clone(),
        }
    }
}

/// The English label of a row (`Function text`, `Cross-section`).
pub fn row_label(section: &SheetSection, row: &PropRow) -> String {
    strings::field_label(&section.kind, &row.path_for_label())
}

/// Whether a row is a group header: a group or list without a route of its own whose members are editable.
pub fn is_header(section: &SheetSection, row: &PropRow) -> bool {
    if !row.ty.is_container() || row.route.is_editable() || is_derived(row) {
        return false;
    }
    section.rows.iter().any(|r| {
        r.key != row.key
            && r.path.len() > row.path.len()
            && r.path.starts_with(&row.path)
            && matches!(r.path.as_bytes()[row.path.len()], b'.' | b'[')
            && r.route.is_editable()
    })
}

/// Why a row is read-only, as the grid shows it (§6.8); `None` for an editable row, a derived row and a group header.
pub fn read_only_reason(sheet: &PropertySheet, section: &SheetSection, row: &PropRow) -> Option<String> {
    if let Some(r) = &sheet.read_only {
        return Some(r.clone());
    }
    if let Provenance::Generated { block } = &row.provenance {
        return Some(strings::format("props-ro-generated", &[block]));
    }
    if is_derived(row) || is_header(section, row) {
        return None;
    }
    match &row.route {
        EditRoute::FieldsSet { .. } | EditRoute::SettingsMember { .. } => None,
        EditRoute::ReadOnly(reason) => Some(reason_text(reason)),
    }
}

/// The text of a read-only reason.
pub fn reason_text(reason: &FieldReadOnly) -> String {
    match reason {
        FieldReadOnly::Identity => strings::text("props-ro-identity").to_owned(),
        FieldReadOnly::Narrowed { why } => why.clone(),
        FieldReadOnly::Command {
            action: Some(action), ..
        } => strings::format("props-ro-action", &[action]),
        FieldReadOnly::Command { command, .. } => strings::format("props-ro-command", &[command]),
        FieldReadOnly::HumanOnly { step, .. } => strings::format("props-ro-human-only", &[step]),
        FieldReadOnly::NoCommand => strings::text("props-ro-no-command").to_owned(),
    }
}

/// The settable member of a route: the `fields.set` path, or the settings member's row path.
pub fn member_path(route: &EditRoute) -> Option<String> {
    match route {
        EditRoute::FieldsSet { path, .. } => Some(path.clone()),
        EditRoute::SettingsMember { member } => Some(format!("settings.{member}")),
        EditRoute::ReadOnly(_) => None,
    }
}

/// One segment of a row path below its member.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Seg {
    Member(String),
    Index(usize),
}

/// The segments of a row path (`[0].qty` → index 0, member `qty`).
fn segments(path: &str) -> Vec<Seg> {
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

/// The kind's plural for the card and the toast (`devices`).
pub fn kind_plural(kind: &str, n: usize) -> String {
    if n == 1 {
        return strings::kind_label(kind);
    }
    strings::kind_plural(kind)
}

/// The default value of a new list item of type `ty` (`props.list` `add`, §6.5).
pub fn item_default(ty: &FieldType) -> Value {
    match ty.inner() {
        FieldType::Text | FieldType::Ident | FieldType::MlText => Value::String(String::new()),
        FieldType::Int { min, .. } => Value::from(min.unwrap_or(0).max(0)),
        FieldType::Dec3 => Value::String("0".into()),
        FieldType::Bool => Value::Bool(false),
        FieldType::Group(children) => Value::Object(
            children
                .iter()
                .filter(|c| !c.ty.is_optional() && !matches!(c.ty.inner(), FieldType::Link { .. }))
                .map(|c| (c.name.clone(), item_default(&c.ty)))
                .collect(),
        ),
        FieldType::List(_) => Value::Array(Vec::new()),
        _ => Value::Null,
    }
}

/// The display text of a row's value in the grid and the describe text: one value's text, `‹varies›` with its count,
/// or empty when absent.
pub fn value_text(row: &PropRow) -> String {
    match &row.value {
        RowValue::One { text, .. } => text.clone(),
        RowValue::Varies { count, .. } => strings::format("props-varies", &[&count.to_string()]),
        RowValue::Absent => String::new(),
    }
}

/// The provenance chip's text (an icon glyph and words, never colour alone; FR-UI-025).
pub fn chip_text(p: &Provenance) -> String {
    match p {
        Provenance::Authored => strings::text("chip-authored").to_owned(),
        Provenance::Inherited { from } => strings::format("chip-inherited", &[from]),
        Provenance::Derived { via } => strings::format("chip-derived", &[via]),
        Provenance::Overridden { note } => strings::format("chip-overridden", &[note]),
        Provenance::Frozen { note } => strings::format("chip-frozen", &[note]),
        Provenance::Library { item } => strings::format("chip-library", &[item]),
        Provenance::Generated { block } => strings::format("chip-generated", &[block]),
        Provenance::Varies { count } => strings::format("chip-varies", &[&count.to_string()]),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_policy_follows_fr_ui_044() {
        assert_eq!(policy(1, false, 0, Cost::Interactive, true), Decision::Direct);
        assert_eq!(policy(12, false, 0, Cost::Interactive, true), Decision::Direct);
        assert_eq!(
            policy(501, false, 0, Cost::Interactive, true),
            Decision::Card("reason-bulk")
        );
        assert_eq!(
            policy(2, true, 0, Cost::Interactive, true),
            Decision::Card("reason-identifiers")
        );
        assert_eq!(policy(1, true, 1, Cost::Interactive, true), Decision::Direct);
        assert_eq!(
            policy(1, false, 2, Cost::Interactive, true),
            Decision::Card("reason-identifiers")
        );
        assert_eq!(
            policy(1, false, 0, Cost::Bulk, true),
            Decision::Card("reason-bulk-command")
        );
        assert_eq!(policy(1, false, 0, Cost::Interactive, false), Decision::Error);
    }

    #[test]
    fn a_leaf_is_composed_into_its_member() {
        let parts = json!([{"part": {"copy": "a"}, "qty": "1"}, {"part": {"copy": "b"}, "qty": "2"}]);
        assert_eq!(
            compose("parts", "parts[1].qty", Some(&parts), &json!("3")).unwrap(),
            json!([{"part": {"copy": "a"}, "qty": "1"}, {"part": {"copy": "b"}, "qty": "3"}])
        );
        assert_eq!(
            compose("structure", "structure.location", None, &json!("n1")).unwrap(),
            json!({"location": "n1"})
        );
        assert_eq!(
            compose(
                "structure",
                "structure.location",
                Some(&json!({"location": "n1", "function": "f"})),
                &Value::Null
            )
            .unwrap(),
            json!({"function": "f"})
        );
        assert_eq!(compose("title", "title", None, &json!("x")).unwrap(), json!("x"));
        assert!(compose("parts", "partsx", None, &json!(1)).is_none());
        assert!(compose("parts", "parts[5].qty", Some(&parts), &json!("1")).is_none());
    }

    #[test]
    fn list_item_defaults() {
        assert_eq!(item_default(&FieldType::Text), json!(""));
        assert_eq!(item_default(&FieldType::Bool), json!(false));
        assert_eq!(
            item_default(&FieldType::Int {
                min: Some(1),
                max: None
            }),
            json!(1)
        );
    }
}
