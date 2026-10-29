// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The property grid's panel model (WP-S2.05 part 1, spec §6.2 to §6.11): the sheet the shell fetched, the focused
//! row, the edit buffer, the choices and popups, the inline preview card and the toast, and the pure rules over them —
//! the edit policy of FR-UI-044 ([`policy`]), the composition of a leaf into its settable member ([`compose`]), the
//! read-only reasons ([`read_only_reason`]) and the rows on show ([`visible_rows`]).
//!
//! Only the shell (`ShellModel`'s `props.*` actions) calls the `Session`: once per sheet key for the sheet
//! (`Session::property_sheet`), once per edit for its preview (`Session::preview_with`), then the commit. The card is
//! the stand-in of WP-S2.09's preview tab (D5); the toast of a commit with several targets is a notification of the
//! shell since WP-S2.09 part 1 (`crate::notifications`).

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_api::Cost;
use ew_api::output::ew_dl::EntityRef;

use crate::canvas::PageId;
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
        || ty.is_part_uses()
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
/// Where a card was opened: the property grid, the find pane's replace, or the parts table import (the transcript's
/// `origin`).
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
    /// File, Import, Parts table (FT-410): the card of `parts.import`, a window of its own that no selection change
    /// and no pane of the layout ends; it goes with the project.
    Import {
        /// The base name of the chosen table (`parts.csv`).
        source: String,
        /// The copies the command creates.
        created: usize,
        /// The copies it skips: their key exists already.
        skipped: usize,
    },
    // WP-S2.06 part 1
    /// The Pages navigator's reorder (`pages.reorder`, §6.6): the card in the Pages pane, with the preview's folio
    /// changes `"<old> → <new>"` in position order.
    Reorder {
        /// The notices of `page.reorder`'s preview.
        notices: Vec<String>,
    },
    // WP-S2.09 part 2
    /// A finding's quick fix (`messages.fix`, §7.4): the card in the Messages pane, always previewed.
    QuickFix {
        /// The rule ID of the finding.
        rule: String,
    },
    // WP-S2.09 part 3
    /// The card of a `Bulk` command a dialog built (§8.2): Page, Renumber pages… (`page.renumber`) and Tools, Renumber
    /// devices… (`numbering.renumber`); its targets and changes are the number of entity changes. Draw ▸ Bond's
    /// `bond.create` (FT row 7 of the G-PP triage) uses the same card with its own reason.
    Bulk,
    // WP-S2.06 part 3
    /// A gesture of a navigator's table (§8.7): the card in the navigator's pane, with the gesture's cells, so that
    /// `table.resolve` and `table.drop` rebuild the command and a stale card retargets its rows.
    Table {
        /// The gesture.
        gesture: crate::navigators::table::Gesture,
    },
}

impl CardOrigin {
    /// The transcript's word (`properties`, `find`, `import`, `reorder`, `fix`, `bulk`).
    pub fn word(&self) -> &'static str {
        match self {
            CardOrigin::Properties => "properties",
            CardOrigin::Find { .. } => "find",
            CardOrigin::Import { .. } => "import",
            CardOrigin::Reorder { .. } => "reorder",
            CardOrigin::QuickFix { .. } => "fix",
            CardOrigin::Bulk => "bulk",
            CardOrigin::Table { .. } => "table",
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
    pub(crate) notes: Vec<String>,
    /// `props.project` shows the project record until the selection changes (its generation then).
    pub(crate) project_mode: Option<u64>,
    /// The selection generation and the page it was made on: the pane shows the selection only while that page is on
    /// show; a page change with nothing selected there shows the page's own sheet (window check, item 6).
    pub(crate) selection_page: Option<(u64, Option<PageId>)>,
    /// FT-541: the text of the filter field (`props.filter`); it applies only while the field is on show.
    pub(crate) filter: String,
    /// The proposal of a refused tag-letter edit (FR-NUM-029): accepted by `props.accept_tag_proposal`.
    pub(crate) tag_proposal: Option<TagProposal>,
}

/// The proposal that comes with a tag-letter edit refused for a colliding counter (FR-NUM-029, ID-53; cycle 33 G-PP
/// row 6): the same letter with the counter left to online numbering, which gives the next free one. Accepting it is
/// one `fields.set_many` (the counter cleared, then the letter set), so the tag is not frozen.
#[derive(Clone, PartialEq, Debug)]
pub struct TagProposal {
    /// The row the refusal stands under.
    pub row: String,
    /// The proposed tag as the page prints it (`-102F2`).
    pub tag: String,
    /// The counter online numbering gives (`2`), for the toast.
    pub counter: String,
    /// The `fields.set_many` parameters that accept it.
    pub(crate) params: Value,
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

    /// The proposal of a refused tag-letter edit, if any.
    pub fn tag_proposal(&self) -> Option<&TagProposal> {
        self.tag_proposal.as_ref()
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

    /// The notes of the last commit (confusable code points, an alias that resolved a node).
    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    /// The text of the filter field (FT-541); `ShellModel::properties_filter` is the filter in force.
    pub fn filter(&self) -> &str {
        &self.filter
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

// WP-S2.17 part 3 (D11; doc 16a §6.8): Main and Advanced.
/// A group of rows in the grid: **Main** (the kind's primary fields, first and open) or one of the sheet's categories.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RowGroup {
    /// The primary fields of the kind ([`is_primary`]).
    Main,
    /// A category of the field catalogue, less the Main rows.
    Category(Category),
}

impl RowGroup {
    /// The word of the group (`main`, a category's word): the `category` parameter of `props.category`.
    pub fn word(self) -> &'static str {
        match self {
            RowGroup::Main => "main",
            RowGroup::Category(c) => c.word(),
        }
    }

    /// The group of `word`.
    pub fn from_word(word: &str) -> Option<RowGroup> {
        if word == "main" {
            return Some(RowGroup::Main);
        }
        Category::ALL
            .iter()
            .find(|c| c.word() == word)
            .map(|c| RowGroup::Category(*c))
    }

    /// The group's title (`Main`, `Identity`).
    pub fn title(self) -> &'static str {
        match self {
            RowGroup::Main => strings::text("category-main"),
            RowGroup::Category(c) => strings::text(&format!("category-{}", c.word())),
        }
    }
}

/// The collapse key of a section's group (`device/main`, `device/identity`).
pub fn group_key(kind: &str, group: RowGroup) -> String {
    format!("{kind}/{}", group.word())
}

/// The primary fields of a kind, by the row key after `<kind>/`: a row is primary when its key is one of them or a
/// member of one (`parts[0].part` of `parts`). A kind without a list has no Main group.
const PRIMARY: &[(&str, &[&str])] = &[
    (
        "page",
        &["folio.number", "folio.section", "title", "derived.description"],
    ),
    (
        "device",
        &[
            "derived.tag",
            "function_text",
            "parts",
            "derived.structure.location",
        ],
    ),
    (
        "connection",
        &[
            "derived.number",
            "derived.potential",
            "derived.csa",
            "derived.colour",
        ],
    ),
];

/// The internal members shown only under Advanced, by the row's path: the record version, the ID, the source reference
/// with its key and lineage, the tag counter and the form's UUID.
const ADVANCED: &[&str] = &["$v", "id", "source_ref", "tag.counter", "form"];

/// Whether `rest` is `base` or one of its members (`parts[0].qty` of `parts`).
fn under(rest: &str, base: &str) -> bool {
    rest == base
        || rest
            .strip_prefix(base)
            .is_some_and(|tail| tail.starts_with('.') || tail.starts_with('['))
}

/// Whether `row` is one of its kind's primary fields (the Main group).
pub fn is_primary(kind: &str, row: &PropRow) -> bool {
    let rest = row.key.split_once('/').map_or(row.key.as_str(), |(_, r)| r);
    PRIMARY
        .iter()
        .find(|(k, _)| *k == kind)
        .is_some_and(|(_, paths)| paths.iter().any(|p| under(rest, p)))
}

/// Whether `row` is an internal member, shown only while Advanced is on.
pub fn is_advanced(row: &PropRow) -> bool {
    !is_derived(row) && ADVANCED.iter().any(|p| under(&row.path, p))
}

/// The groups of a section in display order with their rows: Main first when the kind has primary rows, then each
/// category with its other rows; the Advanced rows are left out unless `advanced`. A group without rows is left out.
pub fn groups(section: &SheetSection, advanced: bool) -> Vec<(RowGroup, Vec<&PropRow>)> {
    let shown = |r: &&PropRow| advanced || !is_advanced(r);
    let main: Vec<&PropRow> = section
        .rows
        .iter()
        .filter(shown)
        .filter(|r| is_primary(&section.kind, r))
        .collect();
    let mut out = Vec::new();
    if !main.is_empty() {
        out.push((RowGroup::Main, main));
    }
    for (c, rows) in categories(section) {
        let rows: Vec<&PropRow> = rows
            .into_iter()
            .filter(shown)
            .filter(|r| !is_primary(&section.kind, r))
            .collect();
        if !rows.is_empty() {
            out.push((RowGroup::Category(c), rows));
        }
    }
    out
}

/// The number of Advanced rows of `sheet` (the "Show advanced (n)" link).
pub fn advanced_count(sheet: &PropertySheet) -> usize {
    sheet
        .sections
        .iter()
        .flat_map(|s| &s.rows)
        .filter(|r| is_advanced(r))
        .count()
}

/// The group `row` is shown in.
pub fn group_of(section: &SheetSection, row: &PropRow) -> RowGroup {
    if is_primary(&section.kind, row) {
        RowGroup::Main
    } else {
        RowGroup::Category(row.category)
    }
}

/// The rows on show: every row of every expanded group, in display order ([`groups`]); the Advanced rows only while
/// `advanced` is on; with a non-empty `filter`, only the rows it matches ([`row_matches`]).
pub fn visible_rows<'s>(
    sheet: &'s PropertySheet,
    collapsed: &BTreeSet<String>,
    advanced: bool,
    filter: &str,
) -> Vec<VisibleRow<'s>> {
    let mut out = Vec::new();
    for s in &sheet.sections {
        for (g, rows) in groups(s, advanced) {
            if collapsed.contains(&group_key(&s.kind, g)) {
                continue;
            }
            out.extend(
                rows.into_iter()
                    .filter(|row| row_matches(s, row, filter))
                    .map(|row| VisibleRow { section: s, row }),
            );
        }
    }
    out
}

/// FT-541 (doc 16a §6.8): the filter field "Filter properties" appears only when the open categories hold more than
/// this many rows.
pub const FILTER_ROWS: usize = 12;

/// Whether the Properties filter field is on show: the rows of the open categories, unfiltered, are more than
/// [`FILTER_ROWS`].
pub fn filter_shown(sheet: &PropertySheet, collapsed: &BTreeSet<String>, advanced: bool) -> bool {
    visible_rows(sheet, collapsed, advanced, "").len() > FILTER_ROWS
}

/// Whether `row` matches the Properties filter `filter`: an empty filter matches every row, else the row's label or
/// its value text contains the filter, case-folded.
pub fn row_matches(section: &SheetSection, row: &PropRow, filter: &str) -> bool {
    let filter = filter.trim();
    if filter.is_empty() {
        return true;
    }
    let needle = filter.to_lowercase();
    row_label(section, row).to_lowercase().contains(&needle)
        || value_text(row).to_lowercase().contains(&needle)
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

/// The label of a row action in the row's actions menu (FT-530; doc 16a §6.8: "Reset to derived", "Freeze").
pub fn action_label(action: RowAction) -> &'static str {
    strings::text(match action {
        RowAction::Edit => "props-action-edit",
        RowAction::Clear => "props-action-clear",
        RowAction::Override => "props-action-override",
        RowAction::Reset => "props-action-reset",
        RowAction::Freeze => "props-action-freeze",
        RowAction::Unfreeze => "props-action-unfreeze",
    })
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

/// The provenance chip's one word (doc 16a §6.8): the value's source, "page" for a value inherited from a page (or
/// from a structure box on a page), "inherited" for any other inheritance, else the kind ("derived", "override",
/// "frozen", "library", "generated", "varies"); an authored value has no chip (the word is empty). [`chip_text`] is
/// the whole text (the tooltip and the accessible description).
pub fn chip_word(p: &Provenance) -> &'static str {
    match p {
        Provenance::Authored => "",
        Provenance::Inherited { from } if from.starts_with("page ") || from.contains(" on page ") => {
            strings::text("chip-word-page")
        }
        Provenance::Inherited { .. } => strings::text("chip-word-inherited"),
        Provenance::Derived { .. } => strings::text("chip-word-derived"),
        Provenance::Overridden { .. } => strings::text("chip-word-override"),
        Provenance::Frozen { .. } => strings::text("chip-word-frozen"),
        Provenance::Library { .. } => strings::text("chip-word-library"),
        Provenance::Generated { .. } => strings::text("chip-word-generated"),
        Provenance::Varies { .. } => strings::text("chip-word-varies"),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Doc 16a §6.8: the chip names the source in one word; an authored value has none.
    #[test]
    fn the_chip_word_names_the_source() {
        let inherited = |from: &str| Provenance::Inherited { from: from.into() };
        assert_eq!(chip_word(&Provenance::Authored), "");
        assert_eq!(chip_word(&inherited("page 101")), "page");
        assert_eq!(chip_word(&inherited("structure box on page 103")), "page");
        assert_eq!(chip_word(&inherited("project description")), "inherited");
        assert_eq!(chip_word(&inherited("-X1")), "inherited");
        assert_eq!(
            chip_word(&Provenance::Derived {
                via: "numbering".into()
            }),
            "derived"
        );
        assert_eq!(
            chip_word(&Provenance::Overridden {
                note: "override".into()
            }),
            "override"
        );
        assert_eq!(
            chip_word(&Provenance::Frozen {
                note: "frozen".into()
            }),
            "frozen"
        );
        assert_eq!(
            chip_word(&Provenance::Library {
                item: "l/i v1".into()
            }),
            "library"
        );
        assert_eq!(
            chip_word(&Provenance::Generated { block: "b".into() }),
            "generated"
        );
        assert_eq!(chip_word(&Provenance::Varies { count: 2 }), "varies");
    }

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
