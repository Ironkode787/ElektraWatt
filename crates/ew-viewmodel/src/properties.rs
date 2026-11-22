// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The property grid's panel model: the sheet the shell fetched, the focused
//! row, the edit buffer, the choices and popups, the inline preview card and the toast, and the pure rules over them —
//! the edit policy of FR-UI-044 ([`policy`]), the composition of a leaf into its settable member ([`compose`]), the
//! read-only reasons ([`read_only_reason`]) and the rows on show ([`visible_rows`]).
//!
//! Only the shell (`ShellModel`'s `props.*` actions) calls the `Session`: once per sheet key for the sheet
//! (`Session::property_sheet`), once per edit for its preview (`Session::preview_with`), then the commit. The card is
//! the stand-in of the Preview tab; the toast of a commit with several targets is a notification of the
//! shell (`crate::notifications`).

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_api::Cost;
use ew_dl::EntityRef;

use crate::canvas::PageId;
use crate::strings::Key;
use crate::{count_key, key};
pub use ew_api::output::{
    Category, EditRoute, FieldReadOnly, FieldType, PropRow, PropertySheet, Provenance, RowAction, RowValue,
    SheetSection, SheetWhat, SourceLink,
};
use serde_json::Value;

use crate::strings;
use crate::tools::PreviewState;

/// Above this many targets an edit always goes through the card (FR-UI-044 rule 1, `edit.rs`'s `BULK_ABOVE`).
pub const BULK_TARGETS: usize = 500;

/// What the edit policy decides on a preview.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Decision {
    /// Commit the kept preview at once.
    Direct,
    /// Show the card; the key names why.
    Card(Key<'static>),
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
        Decision::Card(key!("reason-bulk"))
    } else if (identifier && targets > 1) || tags > 1 {
        Decision::Card(key!("reason-identifiers"))
    } else if cost == Cost::Bulk {
        Decision::Card(key!("reason-bulk-command"))
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
    /// The options of a choice editor (doc 16 §6.3): an enum's words, the colour codes, the part copies, the structure nodes
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
    /// Each item's secondary line, in the items' order (an address profile's reference under its name); empty
    /// when the items have none.
    pub details: Vec<String>,
    /// What they are for.
    pub kind: ChoiceKind,
    /// The highlighted item (Up, Down), if any.
    pub index: Option<usize>,
}

/// Whether a value type has a choice editor with options (doc 16 §6.3: a combo for enums and colours, a picker for part
/// copies, a page's or a report block's form and structure nodes).
pub fn has_options(ty: &FieldType) -> bool {
    matches!(
        ty.inner(),
        FieldType::Enum { .. } | FieldType::Colour | FieldType::Slot
    ) || matches!(ty.inner(), FieldType::Link { kind } if kind == "part" || kind == "form")
        || ty.is_part_uses()
}

/// The page's connection grid row, whose editor lists the grids in words.
pub(crate) const GRID_ROW: &str = "page/grid";

/// A free text's style row, whose editor lists the project's text styles by their names (the Text styles page's).
pub(crate) const STYLE_ROW: &str = "page_item/kind.free_text.style";

/// Whether editing `row` opens a choice list ([`has_options`], the connection grid's words, a text's styles).
pub fn opens_choices(row: &PropRow) -> bool {
    has_options(&row.ty) || row.key == GRID_ROW || row.key == STYLE_ROW
}

/// How an editable row's value is chosen, which its cell shows (so that a drop-down row does not look like
/// any other): from a list (a chevron, a combo box), or in a picker of parts ("…").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValueChooser {
    /// A closed list: the words of an enumeration, the colours, the structure nodes, the grids.
    List,
    /// The part picker: a Parts row, a part link.
    Picker,
}

/// The way `row`'s value is chosen; `None` for a row typed as text or one that cannot be edited.
pub fn value_chooser(row: &PropRow) -> Option<ValueChooser> {
    if !row.actions.contains(&RowAction::Edit) || !opens_choices(row) {
        return None;
    }
    let part = row.ty.is_part_uses() || matches!(row.ty.inner(), FieldType::Link { kind } if kind == "part");
    Some(if part {
        ValueChooser::Picker
    } else {
        ValueChooser::List
    })
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

/// An edit compiled into a command, kept with the card so that a stale card can be checked and rebuilt.
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

/// Where a card was opened: the property grid, the find pane's replace, or the parts table import (the transcript's
/// `origin`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CardOrigin {
    /// An edit of the property grid.
    Properties,
    /// A replace of the find pane: the query whose hits it writes, so that a stale card runs it again.
    Find {
        /// The query.
        query: crate::find::FindQuery,
        /// The number of hits the replace writes.
        hits: usize,
    },
    /// File, Import, Parts table: the card of `parts.import`, a window of its own that no selection change
    /// and no pane of the layout ends; it goes with the project.
    Import {
        /// The base name of the chosen table (`parts.csv`).
        source: String,
        /// The copies the command creates.
        created: usize,
        /// The copies it skips: their key exists already.
        skipped: usize,
    },
    /// The Pages navigator's reorder (`pages.reorder`): the card in the Pages pane, with the preview's folio
    /// changes `"<old> → <new>"` in position order.
    Reorder {
        /// The notices of `page.reorder`'s preview.
        notices: Vec<String>,
    },
    /// A finding's quick fix (`messages.fix`): the card in the Messages pane, always previewed.
    QuickFix {
        /// The rule ID of the finding.
        rule: String,
    },
    /// The card of a `Bulk` command a dialog built: Page, Renumber pages… (`page.renumber`) and Tools, Renumber
    /// devices… (`numbering.renumber`); its targets and changes are the number of entity changes. Draw ▸ Bond's
    /// `bond.create` (FT row 7 of the G-PP triage) uses the same card with its own reason.
    Bulk,
    /// A gesture of a navigator's table: the card in the navigator's pane, with the gesture's cells, so that
    /// `table.resolve` and `table.drop` rebuild the command and a stale card retargets its rows.
    Table {
        /// The gesture, with its title.
        gesture: crate::navigators::table::TableGesture,
    },
    /// A macro's insertion from the Macros navigator (`macro.insert`, FR-MAC-005): the card in the Preview tab
    /// with the conflicts the user decides on before Apply; a refused preview (a prevent rule) still shows them.
    Macro(Box<MacroInsert>),
    /// The part editor's Save (`part.edit_copy`, D20): the card in the Preview tab with the field diff, the devices
    /// that use the copy, the BOM rows that change and, for a fork, the fork note.
    Part(Box<PartCard>),
    /// FT-396's paste choice (D22): a paste whose fragment carries another version of a held library copy; the card in
    /// the Preview tab lists the copies and offers Use the project's copy and Update the project's copy (`paste.resolve`).
    /// It previews nothing: its result is the refusal, so Apply stays blocked.
    PasteVersions(Box<PasteChoice>),
    /// The `library.update` card that "Update the project's copy" opens from the pasted fragment's documents (D22):
    /// after its Apply commits, the shell arms the pending paste of `text` again (two commands, two undo steps).
    PasteUpdate {
        /// The clipboard text of the pending paste.
        text: String,
    },
    /// A table round trip's file read against a navigator's table (`navigator.import_table`): the card in the Preview tab with the counts and the per-cell list; Apply commits the import's one
    /// `fields.set_many`.
    TableImport(Box<TableImportCard>),
    /// Arrange and route (`layout.arrange`, docs/design/06a-drafting.md §7.3): the card in the Preview tab with the plan's
    /// rows; Apply commits the plan's `layout.apply`.
    Arrange(Box<ArrangeCard>),
    /// Route (`layout.route`) over routes a person drew: the card in the Preview tab with the plan's
    /// lead line and one row per connection left unrouted; Apply commits the plan's `layout.apply`.
    Route {
        /// "Route 3 wires on folio 101: 2 routed, 1 not routed."
        lead: String,
        /// One row per connection left unrouted, with its reason.
        rows: Vec<String>,
        /// The page in words (`folio 101`), for the toast.
        page: String,
        /// How many wires the plan routes.
        routed: usize,
    },
    /// Page ▸ Copy pages… (`page.copy`, doc 06 §3.6): the card in the Preview tab with its lead line and what the copy
    /// leaves out or continues, one line each in the tab's notices; Apply commits the one `page.copy`.
    PageCopy {
        /// "Copy 3 pages."
        lead: String,
        /// The devices drawn also on pages not copied, the frozen tags cleared, the names whose points continue.
        rows: Vec<String>,
    },
}

/// What an Arrange and route card shows (docs/design/06a-drafting.md §7.3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ArrangeCard {
    /// The folio of the page it arranges.
    pub folio: String,
    /// The page in words (`folio 102`), for the toast.
    pub page: String,
    /// The selection it arranges (symbols and wires by ID); empty: the page.
    pub scope: Vec<ew_api::output::Uuid>,
    /// "Arrange and route: folio 102. 14 symbols move, 3 of them across the sheet; 9 wires are routed. Nothing has
    /// changed yet."
    pub lead: String,
    /// The page's figures now, in words.
    pub now: String,
    /// The page's figures as the plan would leave it, in words.
    pub then: String,
    /// The symbols that move (the far moves first), then those unticked.
    pub parts: Vec<ArrangeRow>,
    /// The wires drawn by the person that get a new route, then those unticked.
    pub wires: Vec<ArrangeRow>,
    /// What is left as it is: the symbols kept in place and those without room.
    pub left: Vec<ArrangeRow>,
    /// One sentence per wire left without a way, with what is in it, and per net wired against its symbols' order.
    pub ways: Vec<String>,
    /// Each moved symbol's body where it stands and where it goes, in page nm.
    pub shifts: Vec<([i64; 4], [i64; 4])>,
    /// How many symbols move.
    pub moved: usize,
    /// How many wires the plan routes.
    pub routed: usize,
}

/// One row of an Arrange and route card.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ArrangeRow {
    /// The symbol or wire.
    pub id: ew_api::output::Uuid,
    /// Its name: a symbol's tag, a wire by its ends.
    pub name: String,
    /// What happens to it, in words ("6 steps right, 31 steps up", "kept in place").
    pub words: String,
    /// Whether the row is ticked (a row of what is left as it is is never).
    pub ticked: bool,
    /// A symbol kept in place: the row shows the pin.
    pub kept: bool,
}

/// What a table import card shows and commits.
#[derive(Clone, PartialEq, Debug)]
pub struct TableImportCard {
    /// The file's base name (`strips.xlsx`).
    pub source: String,
    /// The file read against the project: the cells, the findings, the missing rows and the command.
    pub import: ew_api::output::TableImport,
    /// The file's bytes, which the switch "Take the file's values" reads again.
    pub bytes: std::sync::Arc<Vec<u8>>,
    /// Whether a cell changed in both takes the file's value (the switch).
    pub take_file: bool,
}

// The import holds its command's parameters as JSON, which has no NaN: equality is total.
impl Eq for TableImportCard {}

impl TableImportCard {
    /// Whether the file changes nothing it could apply (no command): its Apply is disabled.
    pub fn nothing_to_import(&self) -> bool {
        self.import.command.is_none()
    }

    /// The card's counts line: "5 cells changed, 0 cells kept, 1 conflict: the project's value is kept, …", each count
    /// with its noun; a file that matches the project says so in one sentence.
    pub fn counts_line(&self) -> String {
        let c = self.import.counts();
        if self.nothing_to_import() && c.kept + c.conflicts + c.refused + c.unknown + c.missing == 0 {
            return strings::text(key!("table-import-nothing")).to_owned();
        }
        let conflicts = match (c.conflicts, self.take_file) {
            (0, _) => strings::format(key!("table-import-conflicts"), &["0"]),
            (1, false) => strings::text(key!("table-import-conflict-kept")).to_owned(),
            (n, false) => strings::format(key!("table-import-conflicts-kept"), &[&n.to_string()]),
            (1, true) => strings::text(key!("table-import-conflict-taken")).to_owned(),
            (n, true) => strings::format(key!("table-import-conflicts-taken"), &[&n.to_string()]),
        };
        strings::format(
            key!("table-import-counts"),
            &[
                &strings::format_count(count_key!("table-import-changed"), c.changed),
                &strings::format_count(count_key!("table-import-kept"), c.kept),
                &conflicts,
                &strings::format_count(count_key!("table-import-refused"), c.refused),
                &strings::format_count(count_key!("table-import-unknown"), c.unknown),
                &strings::format_count(count_key!("table-import-missing"), c.missing),
            ],
        )
    }

    /// One cell's line of the per-cell list: `-X1031:1 · Function text · "Valve" → "Valve 1" · Apply takes the
    /// file's value` (what Apply will do), with the reason of a read-only or refused cell.
    pub fn cell_line(cell: &ew_api::output::ImportCell) -> String {
        use ew_api::output::CellOutcome;
        let outcome = match cell.outcome {
            CellOutcome::Applied => key!("table-import-cell-applied"),
            CellOutcome::Conflict { taken: true } => key!("table-import-cell-conflict-taken"),
            CellOutcome::Kept => key!("table-import-cell-kept"),
            CellOutcome::Conflict { taken: false } => key!("table-import-cell-conflict"),
            CellOutcome::ReadOnly => key!("table-import-cell-read-only"),
            CellOutcome::Refused => key!("table-import-cell-refused"),
        };
        let outcome = match &cell.reason {
            Some(reason) => format!("{}: {reason}", strings::text(outcome)),
            None => strings::text(outcome).to_owned(),
        };
        strings::format(
            key!("table-import-cell"),
            &[
                &cell.row_label,
                &cell.column_label,
                &cell.ew,
                &cell.file,
                &outcome,
            ],
        )
    }
}

/// What the paste choice card shows and keeps.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PasteChoice {
    /// The clipboard text of the pending paste.
    pub text: String,
    /// The connections the copy dropped (the paste tool's label).
    pub dropped: usize,
    /// Each copy in another version: its title, the project's version, the pasted version and the class word
    /// (`patch`, `minor`, `major`) of the change from the pasted version to the project's.
    pub conflicts: Vec<PasteConflict>,
    /// Why Use the project's copy is disabled (`reason-paste-adapt-major`); `None`: enabled.
    pub adapt: Option<Key<'static>>,
    /// Why Update the project's copy is disabled (`reason-paste-update-older`); `None`: enabled.
    pub update: Option<Key<'static>>,
}

/// One copy of the paste choice card.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PasteConflict {
    /// The copy ID.
    pub copy: ew_api::output::Uuid,
    /// The copy's printed name (`FX-MCB-1PN (Fixture)`).
    pub title: String,
    /// The project's version.
    pub held: String,
    /// The pasted version.
    pub pasted: String,
    /// The class word: `patch`, `minor` or `major`.
    pub class: &'static str,
}

impl PasteChoice {
    /// The card's line of one choice (`adapt` or `update`): `choice <word> enabled`, or `choice <word> disabled:
    /// "<reason>"`.
    pub fn choice_line(word: &str, reason: Option<Key<'static>>) -> String {
        match reason {
            None => format!("choice {word} enabled"),
            Some(key) => format!("choice {word} disabled: \"{}\"", strings::text(key)),
        }
    }

    /// The card's text of one conflict (`FX-MCB-1PN (Fixture): this project 1.1.0, pasted 1.0.0, a data change`): the
    /// change size in words, the describe line keeps the class word.
    pub fn conflict_text(c: &PasteConflict) -> String {
        let size = match c.class {
            "major" => strings::text(key!("paste-class-major")),
            "minor" => strings::text(key!("paste-class-minor")),
            _ => strings::text(key!("paste-class-patch")),
        };
        strings::format(key!("paste-conflict"), &[&c.title, &c.held, &c.pasted, size])
    }
}

/// What a part card shows.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PartCard {
    /// The editor's title (`Edit part 5SY4204-7 (Siemens)`).
    pub title: String,
    /// The members that change: (member, old text, new text).
    pub fields: Vec<(String, String, String)>,
    /// The tags of the devices that use the copy.
    pub devices: Vec<String>,
    /// The component BOM's rows that go.
    pub removed: Vec<String>,
    /// The component BOM's rows that come.
    pub added: Vec<String>,
    /// The component BOM's rows that change: each one's columns that differ.
    pub changed: Vec<String>,
    /// The sockets a bus line ends at that the change takes away (`-3K2:P8`).
    pub lost: Vec<String>,
    /// Whether saving forks the copy.
    pub fork: bool,
}

/// What a macro card inserts and the conflicts it lists.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MacroInsert {
    /// The Macros navigator's row key of the macro.
    pub key: String,
    /// The macro's name.
    pub name: String,
    /// The target page (its page UUID).
    pub page: ew_api::output::Uuid,
    /// The anchor on the page, nm.
    pub at: (i64, i64),
    /// The instance ID the shell minted (`Session::next_macro_instance`).
    pub instance: ew_api::output::Uuid,
    /// The value set chosen, `None` for the defaults.
    pub value_set: Option<String>,
    /// The macro's value sets.
    pub value_sets: Vec<String>,
    /// The variables' ad-hoc value fields, in the macro's order.
    pub fields: Vec<MacroField>,
    /// The conflicts in words, the refusals first (`Session::macro_conflicts`).
    pub conflicts: Vec<String>,
}

/// One variable's ad-hoc value field on a macro card: a typed value is a literal of `macro.insert`, which wins
/// over the value set and the default.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MacroField {
    /// The variable's name.
    pub name: String,
    /// The variable type's word (`text`, `number`, `bool`, `tag_counter`, …).
    pub ty: String,
    /// What the variable takes without a typed value: the chosen value set's value, else the default; empty for none.
    pub placeholder: String,
    /// The typed value, empty for none.
    pub value: String,
    /// The command's refusal of the typed value, in words; `None` when it is taken.
    pub error: Option<String>,
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
            CardOrigin::Macro(_) => "macro",
            CardOrigin::Part(_) => "part",
            CardOrigin::PasteVersions(_) => "paste",
            CardOrigin::PasteUpdate { .. } => "update",
            CardOrigin::TableImport(_) => "table-import",
            CardOrigin::Route { .. } => "route",
            CardOrigin::Arrange(_) => "arrange",
            CardOrigin::PageCopy { .. } => "page-copy",
        }
    }
}

/// The inline preview card.
#[derive(Debug)]
pub struct Card {
    /// The preview.
    pub preview: PreviewState,
    /// The edit it shows.
    pub edit: PendingEdit,
    /// The number of targets.
    pub targets: usize,
    /// The number of changed records (a replace: the number of replaced hits).
    pub changes: usize,
    /// Why the card opened (`reason-identifiers`).
    pub reason: Key<'static>,
    /// Where it was opened.
    pub origin: CardOrigin,
}

impl Card {
    /// What the card's title names (`Preview: <it>`): the command's title, but for a route card the button that opened
    /// it, Route (the window check's C3).
    pub fn command(&self) -> &str {
        match &self.origin {
            CardOrigin::Route { .. } => strings::text(key!("action-layout-route")),
            CardOrigin::Arrange(_) => strings::text(key!("action-layout-arrange")),
            _ => strings::command_title(&self.preview.name),
        }
    }

    /// The lead line of a failed card (doc 16a P3, words not IDs) and the rule code that may follow it
    /// in secondary text; `None` while the preview stands. A macro card leads with a sentence that says what blocks
    /// the apply and how many conflicts its list shows (the list is the detail, so the lead never repeats a row); the
    /// other cards keep `card-failed` with the command's message.
    pub fn failed_lead(&self) -> Option<FailedLead> {
        let message = self.preview.result.as_ref().err()?;
        // The paste choice card leads with what it asks; its rows and choices are the detail.
        if matches!(self.origin, CardOrigin::PasteVersions(_)) {
            return Some(FailedLead {
                text: strings::text(key!("card-paste-versions-title")).to_owned(),
                rule: None,
            });
        }
        let CardOrigin::Macro(m) = &self.origin else {
            return Some(FailedLead {
                text: strings::format(key!("card-failed"), &[message]),
                rule: None,
            });
        };
        // A typed value the command did not take leads in words naming its variable; the reason is under the
        // field.
        if let Some(f) = m.fields.iter().find(|f| f.error.is_some()) {
            return Some(FailedLead {
                text: strings::format(key!("macro-failed-value"), &[&f.name]),
                rule: None,
            });
        }
        let (rule, rest) = split_rule_code(message);
        let text = match m.conflicts.len() {
            0 => strings::format(key!("macro-failed-other"), &[rest]),
            1 => strings::text(key!("macro-failed-one")).to_owned(),
            n => strings::format(key!("macro-failed"), &[&n.to_string()]),
        };
        Some(FailedLead {
            text,
            rule: rule.map(str::to_owned),
        })
    }
}

/// The lead line of a failed card ([`Card::failed_lead`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FailedLead {
    /// The sentence.
    pub text: String,
    /// The rule code of the refusal (`EW-CORE-052`), shown after the sentence in secondary text; `None` without one.
    pub rule: Option<String>,
}

/// A refusal's leading rule code and the rest: `EW-CORE-052: interruption point …` gives `(Some("EW-CORE-052"),
/// "interruption point …")`; a message without one is `(None, message)`. A rule code is `EW-`, upper-case letters,
/// `-` and digits.
fn split_rule_code(message: &str) -> (Option<&str>, &str) {
    let Some((code, rest)) = message.split_once(": ") else {
        return (None, message);
    };
    let mut parts = code.split('-');
    let shaped = parts.next() == Some("EW")
        && parts
            .next()
            .is_some_and(|a| !a.is_empty() && a.chars().all(|c| c.is_ascii_uppercase()))
        && parts
            .next()
            .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        && parts.next().is_none();
    if shaped {
        (Some(code), rest)
    } else {
        (None, message)
    }
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
    /// `props.project` shows the project record until the selection changes or another page shows (the selection's
    /// generation and the page on show then).
    pub(crate) project_mode: Option<(u64, Option<PageId>)>,
    /// The selection generation and the page it was made on: the pane shows the selection only while that page is on
    /// show; a page change with nothing selected there shows the page's own sheet (window check, item 6).
    pub(crate) selection_page: Option<(u64, Option<PageId>)>,
    /// The text of the filter field (`props.filter`); it applies only while the field is on show.
    pub(crate) filter: String,
    /// The proposal of a refused tag-letter edit (FR-NUM-029): accepted by `props.accept_tag_proposal`.
    pub(crate) tag_proposal: Option<TagProposal>,
    /// The Medium row whose New medium… opened the form: the medium it makes is set there (the persona re-test).
    pub(crate) medium_for: Option<String>,
}

/// The proposal that comes with a tag-letter edit refused for a colliding counter (FR-NUM-029): the same letter with the counter left to online numbering, which gives the next free one. Accepting it is
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
    /// The revision the proposal was computed on: accepting it on another is refused.
    pub(crate) base_rev: u64,
    /// A join instead (an existing device's tag typed into the Tag field), whose `params` are
    /// `rep.join_device`'s; `tag` is then that device's tag and `counter` empty.
    pub(crate) join: bool,
}

impl TagProposal {
    /// Whether it is a join rather than a tag.
    pub fn join(&self) -> bool {
        self.join
    }

    /// The button that accepts it: "Use -102F2", or "Join -K1" for a join.
    pub fn button_text(&self) -> String {
        strings::format(
            if self.join {
                key!("props-join-tag")
            } else {
                key!("props-use-tag")
            },
            &[&self.tag],
        )
    }
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

    /// Shows `message` as the error under the row `row`, starting with a capital: the line stands on its own under the
    /// row, and the commands' texts are lowercase ([`crate::strings::sentence_start`]).
    pub(crate) fn set_error(&mut self, row: impl Into<String>, message: impl AsRef<str>) {
        self.error = Some((
            row.into(),
            crate::strings::sentence_start(message.as_ref()).into_owned(),
        ));
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

    /// The text of the filter field; `ShellModel::properties_filter` is the filter in force.
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

// Main and Advanced (doc 16a §6.8).
/// A group of rows in the grid: **Main** (the kind's primary fields, first and open), one of the sheet's categories, or
/// **More** (the rows only a rare case needs, last and closed).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RowGroup {
    /// The primary fields of the kind ([`is_primary`]).
    Main,
    /// A category of the field catalogue, less the Main and More rows.
    Category(Category),
    /// The rows only a rare case needs ([`is_more`]).
    More,
}

impl RowGroup {
    /// The word of the group (`main`, a category's word): the `category` parameter of `props.category`.
    pub fn word(self) -> &'static str {
        match self {
            RowGroup::Main => "main",
            RowGroup::Category(c) => c.word(),
            RowGroup::More => "more",
        }
    }

    /// The group of `word`.
    pub fn from_word(word: &str) -> Option<RowGroup> {
        if word == "main" {
            return Some(RowGroup::Main);
        }
        if word == "more" {
            return Some(RowGroup::More);
        }
        Category::ALL
            .iter()
            .find(|c| c.word() == word)
            .map(|c| RowGroup::Category(*c))
    }

    /// The group's title (`Main`, `Identity`).
    pub fn title(self) -> &'static str {
        match self {
            RowGroup::Main => strings::text(key!("category-main")),
            RowGroup::Category(c) => strings::text(Key::run_time(&format!("category-{}", c.word()))),
            RowGroup::More => strings::text(key!("category-more")),
        }
    }
}

/// The collapse key of a section's group (`device/main`, `device/identity`).
pub fn group_key(kind: &str, group: RowGroup) -> String {
    format!("{kind}/{}", group.word())
}

/// The primary fields of a kind, by the row key after `<kind>/`: a row is primary when its key is one of them or a
/// member of one (`parts[0].part` of `parts`), and Main shows them in this order. A kind without a list has no Main
/// group.
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
            // The PLC a bus device belongs to, or Add to PLC.
            "derived.plc",
            "derived.structure.location",
            // A fluid source's medium (the persona re-test: a person starts at the source).
            "derived.medium",
            // Any other fluid device's ports with the medium each carries.
            "derived.port",
            // A valve's solenoids and its position on its valve terminal (not to be found only
            // through the filter).
            "derived.solenoid",
            "derived.position",
        ],
    ),
    (
        "connection",
        &[
            "derived.number",
            "derived.potential",
            "derived.csa",
            "derived.colour",
            // Whether the wire prints its marker, beside the cross-section and the colour it prints.
            "overrides.show_marker",
            // A bus line's cable and plugs.
            "derived.bus_cable",
            "derived.bus_plugs",
        ],
    ),
    // A bus port's address: the device name and IP, the station address.
    // A marked supply point's place and device ("the comes from cannot be changed in the properties").
    (
        "function",
        &[
            "bus_port.address",
            "derived.supply_place",
            "derived.supply_device",
        ],
    ),
    // A wire's and an interruption point's potential: its name, the kind of signal and the voltage (FT-1651).
    ("potential_def", &["name", "ptype", "voltage"]),
    // What a drafter fills first and the title block prints (a long flat list, the drawing number
    // ten rows down): the title, the project and drawing numbers, the description, the parties (customer, designer)
    // and the dates.
    (
        "project",
        &[
            "master.document_title",
            "master.project_ref",
            "master.drawing_id",
            "master.description",
            // The people the title block prints, one plain row each (`ShellModel::people_in_rows`).
            "people",
            "master.created",
            // Print wire numbers, for the whole project (`ShellModel::wire_numbers_in_row`).
            "settings.drawing.wire_numbers",
        ],
    ),
];

/// The rows only a rare case needs, under the closed group More at the end of their section, by the row
/// key after `<kind>/` as [`PRIMARY`]: the device's kind (its state is a drafter's row: Spare), its tag's numbering, its parent and the stored
/// structure members a derived row does not show; a function's kind, definition, device, role, part function and
/// connection-point overrides; a symbol's own members besides its page, place, turn and symbol. A raw value (shown as
/// data, not words) is one too.
const MORE: &[(&str, &[&str])] = &[
    ("device", &["kind", "tag", "parent", "position", "structure"]),
    // The parties with their addresses and personal data; their names are the people rows of Main.
    ("project", &["parties"]),
    (
        "function",
        &["kind", "def", "device", "role", "template_entry", "cp_overrides"],
    ),
    (
        "representation",
        &[
            "text_slots",
            "arrangement",
            "anchor",
            "box",
            "functions",
            "pin_map",
            "rep_type",
            "variant",
        ],
    ),
];

/// Whether `row` is one of the rows only a rare case needs (the group More): a [`MORE`] row of its kind or a member the
/// schema does not list (a raw value).
pub fn is_more(kind: &str, row: &PropRow) -> bool {
    let rest = row.key.split_once('/').map_or(row.key.as_str(), |(_, r)| r);
    // An interruption point's own rows and a device's tag letter are a drafter's, though their parents wait in More.
    if under(rest, "kind.ip") || under(rest, "tag.letter") {
        return false;
    }
    row.category == Category::Extra
        || MORE
            .iter()
            .find(|(k, _)| *k == kind)
            .is_some_and(|(_, paths)| paths.iter().any(|p| under(rest, p)))
}

/// Whether a group's collapse key is a More group's (`device/more`), which the settings store while it is open.
pub fn is_more_key(key: &str) -> bool {
    key.ends_with("/more")
}

/// The internal members shown only under Advanced, by the row's path: the ID, the source reference
/// with its key and lineage, the tag counter, a page's form (empty unless the page has one of its own; a report block's
/// form, which it always has, is a choice of the project's forms in the open, doc 09 §3.8), the members of a page's connection
/// grid, a fraction of nanometres (`num`, `den`) with no meaning to the user, whose row itself is a choice, a connection's route, drawn on the canvas, and its mark that the program routed it.
const ADVANCED: &[&str] = &[
    "id",
    "source_ref",
    "tag.counter",
    "form",
    "grid.num",
    "grid.den",
    "route",
    "computed",
];

/// Whether `rest` is `base` or one of its members (`parts[0].qty` of `parts`).
fn under(rest: &str, base: &str) -> bool {
    rest == base
        || rest
            .strip_prefix(base)
            .is_some_and(|tail| tail.starts_with('.') || tail.starts_with('['))
}

/// Whether `row` is one of its kind's primary fields (the Main group).
pub fn is_primary(kind: &str, row: &PropRow) -> bool {
    primary_rank(kind, row).is_some() && !is_more(kind, row)
}

/// The place of `row`'s primary field in its kind's [`PRIMARY`] list; `None` for another row.
fn primary_rank(kind: &str, row: &PropRow) -> Option<usize> {
    let rest = row.key.split_once('/').map_or(row.key.as_str(), |(_, r)| r);
    PRIMARY
        .iter()
        .find(|(k, _)| *k == kind)
        .and_then(|(_, paths)| paths.iter().position(|p| under(rest, p)))
}

/// Whether `row` is an internal member, shown only while Advanced is on.
pub fn is_advanced(row: &PropRow) -> bool {
    !is_derived(row) && row.key != "report_block/form" && ADVANCED.iter().any(|p| under(&row.path, p))
}

/// The groups of a section in display order with their rows: Main first when the kind has primary rows, then each
/// category with its other rows, then More; the Advanced rows are left out unless `advanced`. A group without rows is
/// left out.
pub fn groups(section: &SheetSection, advanced: bool) -> Vec<(RowGroup, Vec<&PropRow>)> {
    let shown = |r: &&PropRow| advanced || !is_advanced(r);
    let mut main: Vec<&PropRow> = section
        .rows
        .iter()
        .filter(shown)
        .filter(|r| is_primary(&section.kind, r))
        .collect();
    // In the order of the kind's primary fields (a connection's marker beside the colour it prints), a field's own
    // rows in sheet order.
    main.sort_by_key(|r| primary_rank(&section.kind, r));
    let mut out = Vec::new();
    if !main.is_empty() {
        out.push((RowGroup::Main, main));
    }
    for (c, rows) in categories(section) {
        let rows: Vec<&PropRow> = rows
            .into_iter()
            .filter(shown)
            .filter(|r| group_of(section, r) == RowGroup::Category(c))
            .collect();
        if !rows.is_empty() {
            out.push((RowGroup::Category(c), rows));
        }
    }
    let more: Vec<&PropRow> = section
        .rows
        .iter()
        .filter(shown)
        .filter(|r| group_of(section, r) == RowGroup::More)
        .collect();
    if !more.is_empty() {
        out.push((RowGroup::More, more));
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
    } else if is_more(&section.kind, row) {
        RowGroup::More
    } else {
        RowGroup::Category(row.category)
    }
}

/// The rows on show: every row of every expanded group, in display order ([`groups`]); the Advanced rows only while
/// `advanced` is on; with a non-empty `filter`, only the rows it matches and their parents ([`filtered`]), in every group,
/// a closed one too (the filter finds a row that waits in the closed More).
pub fn visible_rows<'s>(
    sheet: &'s PropertySheet,
    collapsed: &BTreeSet<String>,
    advanced: bool,
    filter: &str,
) -> Vec<VisibleRow<'s>> {
    let mut out = Vec::new();
    for s in &sheet.sections {
        for (g, rows) in groups(s, advanced) {
            if group_shut(collapsed, &s.kind, g, filter) {
                continue;
            }
            out.extend(
                filtered(s, rows, filter)
                    .into_iter()
                    .map(|row| VisibleRow { section: s, row }),
            );
        }
    }
    out
}

/// Whether the group `group` of a section of `kind` shows no rows: it is collapsed and no filter is typed (a filter
/// searches a closed group too).
pub fn group_shut(collapsed: &BTreeSet<String>, kind: &str, group: RowGroup, filter: &str) -> bool {
    filter.trim().is_empty() && collapsed.contains(&group_key(kind, group))
}

/// The rows of one group that the Properties filter `filter` keeps: those it matches ([`row_matches`]) and, among
/// the group's rows, their parents, so that two rows on show never share a label without their parents (doc
/// 16a §6.8: "Party 2" above its "Personal data").
pub fn filtered<'r>(section: &SheetSection, rows: Vec<&'r PropRow>, filter: &str) -> Vec<&'r PropRow> {
    let matched: Vec<&str> = rows
        .iter()
        .filter(|r| row_matches(section, r, filter))
        .map(|r| r.path.as_str())
        .collect();
    let parent_of = |parent: &str, child: &str| {
        child.len() > parent.len()
            && child.starts_with(parent)
            && matches!(child.as_bytes()[parent.len()], b'.' | b'[')
    };
    rows.into_iter()
        .filter(|r| row_matches(section, r, filter) || matched.iter().any(|m| parent_of(&r.path, m)))
        .collect()
}

/// Doc 16a §6.8: the filter field "Filter properties" appears only when the open categories hold more than
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

/// Whether a row is a derived row (doc 16 §6.2).
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
    // A person of the project by the role's word (Designer, Checker).
    if let Some(label) = crate::shell::ShellModel::person_label(&row.key) {
        return label.to_owned();
    }
    // A valve's solenoid row is named by its solenoid ("Solenoid 14"), not by its position in the list.
    if row.key.starts_with("device/derived.solenoid[")
        && let RowValue::One { json, .. } = &row.value
        && let Some(name) = json.get("name").and_then(Value::as_str)
    {
        return strings::format(key!("props-solenoid"), &[name]);
    }
    // A fluid device's port row is named by its port ("Port 2").
    if row.key.starts_with("device/derived.port[")
        && let RowValue::One { json, .. } = &row.value
        && let Some(name) = json.get("designation").and_then(Value::as_str)
    {
        return strings::format(key!("props-port"), &[name]);
    }
    // A bus port's address is named by what it is on its bus: the device name and IP on PROFINET, the station address
    // on PROFIBUS.
    if section.kind == "function" && row.path == "bus_port.address" {
        let bus = section
            .rows
            .iter()
            .find(|r| r.path == "bus_port.bus")
            .and_then(|r| match &r.value {
                RowValue::One { json, .. } => json.as_str(),
                _ => None,
            });
        match bus {
            Some("profinet") => return strings::text(key!("props-address-profinet")).to_owned(),
            Some("profibus_dp") => return strings::text(key!("props-address-profibus")).to_owned(),
            _ => {}
        }
    }
    strings::field_label(&section.kind, &row.path_for_label())
}

/// Writes a connection's Kind in words (never as `{"bus":{}}`): its variant, as a word or the one member of
/// a variant with data, by its `choice-connection-kind-<word>` text (Wire, Bus line, Tube, Bridge) where the text is
/// raw data or the bare word. The JSON value is unchanged.
pub fn connection_kind_in_words(sheet: &mut PropertySheet) {
    for row in sheet.sections.iter_mut().flat_map(|s| s.rows.iter_mut()) {
        if row.key != "connection/kind" {
            continue;
        }
        if let RowValue::One { json, text } = &mut row.value {
            let word = match json {
                Value::String(w) => Some(w.clone()),
                Value::Object(m) if m.len() == 1 => m.keys().next().cloned(),
                _ => None,
            };
            // A text already in words (a core by its cable's tag and identification, `-1W1 BN`) stays.
            if let Some(w) = word.filter(|w| text.starts_with('{') || text == w) {
                *text = strings::field_choice_text("connection", "kind", &w);
            }
        }
    }
}

/// Writes a fluid port that carries no medium as what to do: set one at the air supply; and a bus device's
/// PLC row while it is in none. The JSON value is unchanged.
pub fn ports_in_words(sheet: &mut PropertySheet) {
    for row in sheet.sections.iter_mut().flat_map(|s| s.rows.iter_mut()) {
        if let RowValue::One { json, text } = &mut row.value
            && row.key.starts_with("device/derived.port[")
            && json.get("medium").is_some_and(Value::is_null)
        {
            *text = strings::text(key!("props-port-no-medium")).to_owned();
        }
        // A bus device in no PLC: what to do, as its choices do it.
        if let RowValue::One { json, text } = &mut row.value
            && row.key == "device/derived.plc"
            && json.is_null()
        {
            *text = strings::text(key!("props-plc-none")).to_owned();
        }
    }
}

/// Writes a valve's solenoid rows in words: where the coil is drawn, else "not drawn"; a multipin terminal's
/// plug row likewise; and the coil symbol's valve as "Solenoid 14 of -201Q1". The JSON value is unchanged.
pub fn solenoids_in_words(sheet: &mut PropertySheet) {
    for row in sheet.sections.iter_mut().flat_map(|s| s.rows.iter_mut()) {
        let RowValue::One { json, text } = &mut row.value else {
            continue;
        };
        if row.key.starts_with("device/derived.solenoid[")
            && let Some(words) = solenoid_seat_words(&json["seat"])
        {
            *text = words;
        } else if row.key.starts_with("device/derived.solenoid[")
            && json.get("drawn").is_some_and(Value::is_null)
        {
            *text = strings::text(key!("props-solenoid-not-drawn")).to_owned();
        } else if row.key == "device/derived.plug" && json.get("drawn").is_some_and(Value::is_null) {
            *text = strings::text(key!("props-plug-not-drawn")).to_owned();
        } else if row.key == "representation/derived.solenoid_of" {
            let word = |k: &str| json.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
            *text = strings::format(key!("props-solenoid-of"), &[&word("name"), &word("tag")]);
        }
    }
}

/// Where a terminal valve's solenoid is driven from, in words, from its row's `seat`: `driven by output
/// Q0.0 of -201K1 (inside the terminal: nothing to wire)`, `pin 3 of -201K1's plug at 101-C2`, or that the plug is not
/// drawn yet; `None` for a stand-alone valve's solenoid.
pub fn solenoid_seat_words(seat: &Value) -> Option<String> {
    let word = |k: &str| seat.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    Some(match seat.get("kind").and_then(Value::as_str)? {
        "terminal" => match seat.get("output").and_then(Value::as_str) {
            Some(output) => strings::format(key!("props-solenoid-terminal"), &[output, &word("terminal")]),
            None => strings::format(key!("props-solenoid-terminal-unbound"), &[&word("terminal")]),
        },
        _ => match seat.get("plug").and_then(Value::as_str) {
            Some(plug) => strings::format(
                key!("props-solenoid-pin"),
                &[&word("pin"), &word("terminal"), plug],
            ),
            None => strings::format(
                key!("props-solenoid-pin-undrawn"),
                &[&word("pin"), &word("terminal")],
            ),
        },
    })
}

/// Whether a row is one of a device's connection points (its value names the point: `function`, `idx`, `unused`).
pub fn is_cp_row(row: &PropRow) -> bool {
    row.key.starts_with("device/derived.cp[")
}

/// Writes a device's connection point marked not used as "<label> · not used" (FT-1649). The JSON value is unchanged.
pub fn cps_in_words(sheet: &mut PropertySheet) {
    for row in sheet
        .sections
        .iter_mut()
        .flat_map(|s| s.rows.iter_mut())
        .filter(|r| is_cp_row(r))
    {
        if let RowValue::One { json, text } = &mut row.value
            && json.get("unused") == Some(&Value::Bool(true))
        {
            *text = strings::format(key!("props-cp-unused"), &[text]);
        }
    }
}

/// The explanation of a row in the UI language (FT-1648): what it is, what changing it does and, for a derived
/// value, where it comes from (the tooltip of its label).
pub fn row_help(section: &SheetSection, row: &PropRow) -> Option<&'static str> {
    strings::field_help(&section.kind, &row.path_for_label())
}

/// The provenance every value row of `sheet` shares when it is not authored (FT-1761: a report page's Properties
/// showed a "generated" chip on each of its rows): the header says it once and the rows show none (doc 16a §6.8).
/// `None` when a row is authored or two rows differ.
pub fn shared_provenance(sheet: &PropertySheet) -> Option<&Provenance> {
    let mut rows = sheet
        .sections
        .iter()
        .flat_map(|s| s.rows.iter().filter(move |r| !is_header(s, r)))
        .map(|r| &r.provenance);
    let first = rows.next()?;
    (!matches!(first, Provenance::Authored) && rows.all(|p| p == first)).then_some(first)
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

/// Why a row is read-only, as the grid shows it (doc 16 §6.3); `None` for an editable row, a derived row and a group header.
pub fn read_only_reason(sheet: &PropertySheet, section: &SheetSection, row: &PropRow) -> Option<String> {
    if let Some(r) = &sheet.read_only {
        return Some(r.clone());
    }
    if let Provenance::Generated { block } = &row.provenance {
        return Some(strings::format(key!("props-ro-generated"), &[block]));
    }
    if is_derived(row) || is_header(section, row) {
        return None;
    }
    match &row.route {
        EditRoute::FieldsSet { .. }
        | EditRoute::SettingsMember { .. }
        | EditRoute::SupplyMember { .. }
        | EditRoute::Command { .. } => None,
        EditRoute::ReadOnly(reason) => Some(reason_text(reason)),
    }
}

/// The refusal of an edit of read-only row `row` as a sentence: the identity, a command and no command each
/// have their own; any other reason is wrapped (`props-refused-read-only`). `None` where [`read_only_reason`] is.
pub fn refusal_text(sheet: &PropertySheet, section: &SheetSection, row: &PropRow) -> Option<String> {
    let reason = read_only_reason(sheet, section, row)?;
    let generated = matches!(row.provenance, Provenance::Generated { .. });
    Some(match &row.route {
        EditRoute::ReadOnly(r) if sheet.read_only.is_none() && !generated => match r {
            FieldReadOnly::Identity => strings::text(key!("props-refused-identity")).to_owned(),
            FieldReadOnly::Command {
                action: Some(action), ..
            } => strings::format(key!("props-refused-action"), &[action]),
            FieldReadOnly::Command { command, .. } => {
                strings::format(key!("props-refused-command"), &[strings::command_title(command)])
            }
            FieldReadOnly::NoCommand => strings::text(key!("props-refused-no-command")).to_owned(),
            _ => strings::format(key!("props-refused-read-only"), &[&reason]),
        },
        _ => strings::format(key!("props-refused-read-only"), &[&reason]),
    })
}

/// The text of a read-only reason.
pub fn reason_text(reason: &FieldReadOnly) -> String {
    match reason {
        FieldReadOnly::Identity => strings::text(key!("props-ro-identity")).to_owned(),
        FieldReadOnly::Narrowed { why } => why.clone(),
        FieldReadOnly::Command {
            action: Some(action), ..
        } => strings::format(key!("props-ro-action"), &[action]),
        // The command by its title ("Renumber pages"), never its ID.
        FieldReadOnly::Command { command, .. } => {
            strings::format(key!("props-ro-command"), &[strings::command_title(command)])
        }
        FieldReadOnly::HumanOnly { step, .. } => strings::format(key!("props-ro-human-only"), &[step]),
        FieldReadOnly::NoCommand => strings::text(key!("props-ro-no-command")).to_owned(),
        FieldReadOnly::OnSave => strings::text(key!("props-ro-on-save")).to_owned(),
    }
}

/// The settable member of a route: the `fields.set` path, or the settings or supply member's row path.
pub fn member_path(route: &EditRoute) -> Option<String> {
    match route {
        EditRoute::FieldsSet { path, .. } => Some(path.clone()),
        EditRoute::SettingsMember { member } => Some(format!("settings.{member}")),
        EditRoute::SupplyMember { path } => Some(format!("supply.{path}")),
        // A layout row has no settable member; `Session::layout_edit` compiles its text.
        EditRoute::Command { .. } | EditRoute::ReadOnly(_) => None,
    }
}

/// The settable member's new value with a leaf replaced, moved down to `ew-api` with table mode's pure rule,
/// re-exported under its old path.
pub use ew_api::output::table_edit::compose;

/// The kind's plural for the card and the toast (`devices`), in the UI language inside a sentence
/// (`apparaten`).
pub fn kind_plural(kind: &str, n: usize) -> String {
    if n == 1 {
        return strings::in_sentence(&strings::kind_heading(kind));
    }
    strings::in_sentence(&strings::kinds_heading(kind))
}

/// The header of a sheet's section of `n` records of `kind`: the kind in sentence case, singular for one,
/// with the count ("Device (1)", "Devices (2)"), in the UI language ("Apparaat (1)", "Apparaten
/// (2)").
pub fn section_header(kind: &str, n: usize) -> String {
    if n == 1 {
        return format!("{} ({n})", strings::kind_heading(kind));
    }
    format!("{} ({n})", strings::kinds_heading(kind))
}

/// The default value of a new list item of type `ty` (`props.list` `add`).
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

/// Writes a free text's Style row in words: the style's name as the Text styles page lists it ("Device tag"), the
/// default style's name for a text without one. Its JSON value is unchanged.
pub fn style_in_words(sheet: &mut PropertySheet) {
    for row in sheet
        .sections
        .iter_mut()
        .flat_map(|s| s.rows.iter_mut())
        .filter(|r| r.key == STYLE_ROW)
    {
        if let RowValue::One { json, text } = &mut row.value {
            let key = json.as_str().unwrap_or(ew_api::output::DEFAULT_STYLE_KEY);
            *text = ew_api::checks::messages::text_style_name(key, strings::rule_language());
        }
    }
}

/// Writes a page's Connection grid rows in words: the group `{ num, den }` has no text of its own, so its
/// shown value is the grid as the status bar writes it ("QET 10 px", "4 mm"). Its JSON value is unchanged.
pub fn grid_in_words(sheet: &mut PropertySheet) {
    for row in sheet
        .sections
        .iter_mut()
        .filter(|s| s.kind == "page")
        .flat_map(|s| s.rows.iter_mut())
        .filter(|r| r.path == "grid")
    {
        if let RowValue::One { json, text } = &mut row.value
            && let (Some(num), Some(den)) = (
                json.get("num").and_then(Value::as_i64),
                json.get("den").and_then(Value::as_i64),
            )
            && den > 0
        {
            *text = crate::status::grid_words(num, den);
        }
    }
    // The grid factor (`num`, nanometres) as a length in millimetres, never the count.
    for row in sheet
        .sections
        .iter_mut()
        .filter(|s| s.kind == "page")
        .flat_map(|s| s.rows.iter_mut())
        .filter(|r| r.path == "grid.num")
    {
        if let RowValue::One { json, text } = &mut row.value
            && let Some(nm) = json.as_i64()
        {
            let (whole, part) = (nm / 1_000_000, (nm % 1_000_000).abs());
            let mm = if part == 0 {
                whole.to_string()
            } else {
                let sign = if nm < 0 && whole == 0 { "-" } else { "" };
                format!("{sign}{whole}.{}", format!("{part:06}").trim_end_matches('0'))
            };
            *text = strings::format(key!("status-grid-mm"), &[&strings::decimal(&mm)]);
        }
    }
}

/// Writes a report block's printed columns (their headings, the block's own else the report's), a printed column's
/// column and each of its own rows' texts by the report's column headings in the window's language (doc 09 §3.8), never
/// a column's key; `words` gives the headings of a block's report by key. The JSON values are unchanged.
pub fn report_columns_in_words(
    sheet: &mut PropertySheet,
    words: impl Fn(ew_api::output::Uuid) -> Vec<(String, String)>,
) {
    for section in sheet.sections.iter_mut().filter(|s| s.kind == "report_block") {
        let [id] = section.targets.as_slice() else {
            continue;
        };
        let labels = words(*id);
        if labels.is_empty() {
            continue;
        }
        let label = |key: &str| {
            labels
                .iter()
                .find(|(k, _)| k == key)
                .map_or_else(|| key.to_owned(), |(_, l)| l.clone())
        };
        let text_of = |v: &Value| match v {
            Value::String(s) => s.clone(),
            Value::Object(m) => {
                let lang = strings::rule_language();
                m.get(lang)
                    .or_else(|| m.values().next())
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            }
            _ => String::new(),
        };
        for row in &mut section.rows {
            let RowValue::One { json, text } = &mut row.value else {
                continue;
            };
            let path = row.path.as_str();
            if path == "columns" {
                // The printed columns by their headings: a column's own, else the report's.
                let heads: Vec<String> = json
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(
                        |c| match c.get("heading").map(text_of).filter(|h| !h.is_empty()) {
                            Some(h) => h,
                            None => label(c.get("key").and_then(Value::as_str).unwrap_or_default()),
                        },
                    )
                    .collect();
                *text = heads.join(", ");
            } else if path.starts_with("columns[") && path.ends_with("].key") {
                *text = label(json.as_str().unwrap_or_default());
            } else if path.starts_with("added_rows[") && path.ends_with("].cells") {
                let cells: Vec<String> = json
                    .as_object()
                    .into_iter()
                    .flatten()
                    .map(|(k, v)| format!("{} {}", label(k), text_of(v)))
                    .collect();
                *text = cells.join("; ");
            }
        }
    }
}

/// Writes a page's and a report block's Form row by the form's name in the window's language (doc 09 §3.8), never its
/// ID; `forms` gives the project's forms by name, asked only when a Form row names one. A page that names no frame of
/// its own reads the frame it draws with, which `default_frame` gives (FT-1915: the row was empty). The JSON value of
/// a named form is unchanged.
pub fn forms_in_words(
    sheet: &mut PropertySheet,
    forms: impl FnOnce() -> Vec<(ew_api::output::Uuid, String)>,
    default_frame: impl Fn(ew_api::output::Uuid) -> Option<ew_api::output::Uuid>,
) {
    let (mut forms, mut names) = (Some(forms), None);
    for section in sheet
        .sections
        .iter_mut()
        .filter(|s| s.kind == "page" || s.kind == "report_block")
    {
        let page = match section.targets.as_slice() {
            [one] if section.kind == "page" => Some(*one),
            _ => None,
        };
        for row in section.rows.iter_mut().filter(|r| r.path == "form") {
            if let (RowValue::Absent, Some(frame)) = (&row.value, page.and_then(&default_frame)) {
                row.value = RowValue::One {
                    json: Value::String(frame.to_string()),
                    text: String::new(),
                };
            }
            let RowValue::One {
                json: Value::String(id),
                text,
            } = &mut row.value
            else {
                continue;
            };
            let names: &Vec<_> = names.get_or_insert_with(|| forms.take().map_or_else(Vec::new, |f| f()));
            if let Some((_, name)) = names.iter().find(|(form, _)| form.to_string() == *id) {
                *text = name.clone();
            }
        }
    }
    // The project's Frames row: its default and each page type's frame by name, never by ID (FT-1916).
    for row in sheet
        .sections
        .iter_mut()
        .filter(|s| s.kind == "project")
        .flat_map(|s| s.rows.iter_mut())
        .filter(|r| r.path == "settings.frames")
    {
        let RowValue::One { json, text } = &mut row.value else {
            continue;
        };
        let names: &Vec<_> = names.get_or_insert_with(|| forms.take().map_or_else(Vec::new, |f| f()));
        let name = |id: &Value| {
            let id = id.as_str().unwrap_or_default();
            names
                .iter()
                .find(|(form, _)| form.to_string() == id)
                .map_or_else(|| id.to_owned(), |(_, n)| n.clone())
        };
        let default = json
            .get("default")
            .map(|id| strings::format(key!("props-frames-default"), &[&name(id)]));
        let types = json
            .get("per_page_type")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|e| {
                let kind =
                    strings::choice_text(e.get("page_type").and_then(Value::as_str).unwrap_or_default());
                strings::format(key!("props-frames-type"), &[&kind, &name(&e["form"])])
            });
        let words: Vec<String> = default.into_iter().chain(types).collect();
        if !words.is_empty() {
            *text = words.join("; ");
        }
    }
}

/// Writes a connection's ends A and B in words (W51 finding G): each end as the live description says it
/// (the device's tag and its connection point, else "a junction"), never its JSON;
/// `words` gives a record's description values. The JSON values are unchanged.
pub fn ends_in_words(
    sheet: &mut PropertySheet,
    words: impl Fn(ew_api::output::Uuid) -> Option<ew_api::output::SelectionWords>,
) {
    for section in sheet.sections.iter_mut().filter(|s| s.kind == "connection") {
        let [id] = section.targets.as_slice() else {
            continue;
        };
        let Some(ew_api::output::SelectionWords::Wire { from, to, .. }) = words(*id) else {
            continue;
        };
        for row in &mut section.rows {
            let end = match row.path.as_str() {
                "a" => &from,
                "b" => &to,
                _ => continue,
            };
            if let RowValue::One { text, .. } = &mut row.value {
                *text = crate::status::end_words(end);
            }
        }
    }
    // A potential definition's point: `ew-api` gives a connection point's label; a junction's in words, never its ID.
    for row in sheet
        .sections
        .iter_mut()
        .filter(|s| s.kind == "potential_def")
        .flat_map(|s| s.rows.iter_mut())
        .filter(|r| r.path == "at")
    {
        if let RowValue::One { json, text } = &mut row.value
            && json.get("cp").is_none()
        {
            let end = if json.get("junction").is_some() {
                ew_api::output::WireEnd::Junction
            } else {
                ew_api::output::WireEnd::Unknown
            };
            *text = crate::status::end_words(&end);
        }
    }
}

/// A choice of a row its own command commits in the UI language's words (W51 finding J): a panel page's Label
/// mode as its `choice-label_mode-<mode>` text, a Surface as its enclosure's tag and the keyed word for the plate
/// ("-UC1 montageplaat", with its ordinal " (2)" kept); any other choice as it is. The option itself is what the
/// command reads.
pub fn command_choice_text(path: &str, option: &str) -> String {
    match path.rsplit('.').next().unwrap_or(path) {
        "label_mode" => strings::lookup(&format!("choice-label_mode-{}", option.to_lowercase()))
            .map_or_else(|| option.to_owned(), str::to_owned),
        "labels" => strings::lookup(&format!(
            "choice-labels-{}",
            option.to_lowercase().replace(' ', "_")
        ))
        .map_or_else(|| option.to_owned(), str::to_owned),
        "surface" => {
            let (words, ordinal) = match option.rsplit_once(" (") {
                Some((w, n)) if n.ends_with(')') && n[..n.len() - 1].bytes().all(|b| b.is_ascii_digit()) => {
                    (w, &option[w.len()..])
                }
                _ => (option, ""),
            };
            let plate = if words == "plate" {
                strings::text(key!("surface-plate")).to_owned()
            } else if let Some(tag) = words.strip_suffix(" plate") {
                strings::format(key!("surface-plate-of"), &[tag])
            } else {
                return option.to_owned();
            };
            format!("{plate}{ordinal}")
        }
        _ => option.to_owned(),
    }
}

/// Writes the choices of the rows their own command commits in the UI language's words ([`command_choice_text`]).
/// The JSON values are unchanged.
pub fn command_choices_in_words(sheet: &mut PropertySheet) {
    for row in sheet.sections.iter_mut().flat_map(|s| s.rows.iter_mut()) {
        if !matches!(row.route, EditRoute::Command { .. })
            || !matches!(row.ty.inner(), FieldType::Enum { .. })
        {
            continue;
        }
        match &mut row.value {
            RowValue::One { text, .. } => *text = command_choice_text(&row.path, text),
            RowValue::Varies { texts, .. } => {
                for text in texts {
                    *text = command_choice_text(&row.path, text);
                }
            }
            RowValue::Absent => {}
        }
    }
}

/// Writes the values of the enumerated rows in the UI language's words: each stored word as
/// [`strings::choice_text`] gives it ("Multi-line", "Meerlijnschema"), as the row's choices show them. The JSON values
/// are unchanged, and a word typed in the editor is read back ([`enum_value_of`]). A layout row keeps
/// the texts its sheet built.
pub fn enums_in_words(sheet: &mut PropertySheet) {
    for section in &mut sheet.sections {
        for row in &mut section.rows {
            if !matches!(row.ty.inner(), FieldType::Enum { .. })
                || matches!(row.route, EditRoute::Command { .. })
            {
                continue;
            }
            let words = |w: &str| strings::field_choice_text(&section.kind, &row.path, w);
            match &mut row.value {
                RowValue::One {
                    json: Value::String(word),
                    text,
                } => *text = words(word),
                RowValue::Varies { texts, .. } => {
                    for text in texts {
                        *text = words(text);
                    }
                }
                _ => {}
            }
        }
    }
}

/// The stored word of an enumerated row that `text` names in the UI language's words, else `text` as typed:
/// the parser reads the stored words.
pub fn enum_value_of<'t>(kind: &str, row: &PropRow, text: &'t str) -> std::borrow::Cow<'t, str> {
    if matches!(row.route, EditRoute::Command { .. }) {
        // A Surface view choice typed in the UI language's words is its option.
        let FieldType::Enum { options } = row.ty.inner() else {
            return text.into();
        };
        return options
            .iter()
            .find(|o| command_choice_text(&row.path, o).eq_ignore_ascii_case(text.trim()))
            .map_or(text.into(), |o| o.clone().into());
    }
    enum_word_of(kind, &row.path, &row.ty, text)
}

/// The stored word of a value of the field `path` of `kind`, of type `ty`, that `text` names in the UI language's
/// words, else `text` as typed: what [`enum_value_of`] reads for a row and a table cell for its column's field.
pub fn enum_word_of<'t>(kind: &str, path: &str, ty: &FieldType, text: &'t str) -> std::borrow::Cow<'t, str> {
    let typed = text.trim();
    // A yes or no row takes the words it shows ("ja", "nee"; FT-1555).
    if matches!(ty.inner(), FieldType::Bool) {
        return if typed.eq_ignore_ascii_case(strings::text(key!("props-yes"))) {
            "yes".into()
        } else if typed.eq_ignore_ascii_case(strings::text(key!("props-no"))) {
            "no".into()
        } else {
            text.into()
        };
    }
    let FieldType::Enum { options } = ty.inner() else {
        return text.into();
    };
    options
        .iter()
        .find(|o| strings::field_choice_text(kind, path, o).eq_ignore_ascii_case(typed))
        .map_or(text.into(), |o| o.clone().into())
}

/// A Properties card's summary line (`card-line`): its targets, then its changes, tags and cross-reference texts each
/// counted with its own singular (as the quick fix's), a count of none left out; the window and the describe text
/// show it.
pub fn card_line(card: &Card) -> String {
    strings::format(
        key!("card-line"),
        &[
            &card.targets.to_string(),
            &kind_plural(&card.edit.kind, card.targets),
            &change_counts(card, Some(card.changes)),
        ],
    )
}

/// A reorder card's summary line (`card-reorder`): its moves (`notices`), then its tags and cross-reference texts each
/// counted with its own singular, a count of none left out.
pub fn reorder_line(card: &Card, notices: &[String]) -> String {
    let counts = change_counts(card, None);
    if counts.is_empty() {
        return notices.join(", ");
    }
    strings::format(key!("card-reorder"), &[&notices.join(", "), &counts])
}

/// A quick fix card's summary line: its changes, tags and cross-reference texts each counted with its own singular,
/// a count of none left out; the window and the describe text show it.
pub fn fix_line(card: &Card) -> String {
    change_counts(card, Some(card.changes))
}

/// A card's changes (when given), tags and cross-reference texts, counted and joined by commas; a count of none is
/// left out (FT-1761: a card read "0 tags, 0 cross-reference texts"), the changes always said.
fn change_counts(card: &Card, changes: Option<usize>) -> String {
    let counts = [
        (
            crate::count_key!("preview-tags-count"),
            card.preview.derived.tags.len(),
        ),
        (
            crate::count_key!("preview-xrefs-count"),
            card.preview.derived.xrefs.len(),
        ),
    ];
    match changes {
        Some(n) => crate::preview::counted(
            &[
                [(crate::count_key!("preview-changes-count"), n)].as_slice(),
                &counts,
            ]
            .concat(),
            ", ",
        ),
        None => counts
            .iter()
            .filter(|(_, n)| *n > 0)
            .map(|(k, n)| strings::format_count(*k, *n))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

/// The refusal of a word that is not one of an enumerated field's, in the UI language: a potential's Type
/// names its printed forms (`error-potential-type`), any other field its words as its choices show them.
pub fn not_one_of_text(kind: &str, path: &str, typed: &str, options: &[String]) -> String {
    if kind == "potential_def" && path == "ptype" {
        return strings::format(key!("error-potential-type"), &[typed]);
    }
    let words: Vec<String> = options
        .iter()
        .map(|o| strings::field_choice_text(kind, path, o))
        .collect();
    strings::format(key!("error-not-one-of"), &[typed, &words.join(", ")])
}

/// Writes a report block's row source in words on a generated page's sheet: a built-in
/// kind as its `report-kind-<kind>` text ("Table of contents"), a user SQL report with its file, never the JSON. The
/// JSON value is unchanged; the generated page's rows are read-only.
pub fn report_source_in_words(sheet: &mut PropertySheet) {
    for row in sheet
        .sections
        .iter_mut()
        .filter(|s| s.kind == "report_block")
        .flat_map(|s| s.rows.iter_mut())
        .filter(|r| r.path == "source" && matches!(r.provenance, Provenance::Generated { .. }))
    {
        if let RowValue::One { json, text } = &mut row.value {
            if let Some(kind) = json.get("kind").and_then(Value::as_str) {
                // A kind of a newer format reads as its words, never as JSON.
                *text = strings::lookup(&format!("report-kind-{}", kind.replace('_', "-")))
                    .map_or_else(|| strings::choice_words(kind), str::to_owned);
            } else if let Some(file) = json.pointer("/sql_query/file").and_then(Value::as_str) {
                *text = strings::format(key!("report-source-sql"), &[file]);
            }
        }
    }
}

/// Writes the values `ew-api` gives in English words in the UI language's: a yes or no ("ja", "nee"), a
/// list's count ("4 elementen") and a project setting's structured value in words, never its JSON (`json_words`).
/// The JSON values are unchanged, and the CLI keeps the English words (FT-680).
pub fn values_in_words(sheet: &mut PropertySheet) {
    let yes_no = |text: &mut String| match text.as_str() {
        "yes" => *text = strings::text(key!("props-yes")).to_owned(),
        "no" => *text = strings::text(key!("props-no")).to_owned(),
        _ => {}
    };
    for section in &mut sheet.sections {
        let project = section.kind == "project";
        for row in &mut section.rows {
            value_in_words(row, project, &yes_no);
        }
    }
}

/// [`values_in_words`] for one row; a structured value is worded on the project's sheet only.
fn value_in_words(row: &mut PropRow, project: bool, yes_no: &dyn Fn(&mut String)) {
    match (row.ty.inner(), &mut row.value) {
        (FieldType::Bool, RowValue::One { text, .. }) => yes_no(text),
        (FieldType::Bool, RowValue::Varies { texts, .. }) => texts.iter_mut().for_each(yes_no),
        (
            FieldType::List(_),
            RowValue::One {
                json: Value::Array(items),
                text,
            },
        ) if *text
            == if items.len() == 1 {
                "1 item".to_owned()
            } else {
                format!("{} items", items.len())
            } =>
        {
            *text = strings::format_count(crate::count_key!("props-items"), items.len());
        }
        (FieldType::Json, RowValue::One { json, text }) if project && text.starts_with(['{', '[']) => {
            *text = json_words(json);
        }
        _ => {}
    }
}

/// A structured value in words: its name (a profile's "DBU/WH 24 V", in the UI language where it has one),
/// else its key or its default (a numbering's scheme), else the count of its items or members.
fn json_words(json: &Value) -> String {
    let named = |v: &Value| match v {
        Value::String(s) => Some(s.clone()),
        Value::Object(m) => m
            .get(strings::rule_language().split('-').next().unwrap_or("en"))
            .or_else(|| m.get("en"))
            .or_else(|| m.values().next())
            .and_then(Value::as_str)
            .map(str::to_owned),
        _ => None,
    };
    match json {
        Value::Object(m) => m
            .get("name")
            .and_then(named)
            .or_else(|| {
                ["key", "default"]
                    .iter()
                    .find_map(|k| m.get(*k)?.as_str())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| strings::format_count(crate::count_key!("props-items"), m.len())),
        Value::Array(items) => strings::format_count(crate::count_key!("props-items"), items.len()),
        other => other.to_string(),
    }
}

/// Writes the sheet's header in words: a record without a title of its own by its kind as a heading
/// in the UI language ("Project", not `project`), a wire without a number and a fluid line by its line kind ("Working
/// line", "Werkleiding") in words, its ID row likewise, and a selection of several records by its count and kind ("2
/// apparaten").
pub fn header_in_words(sheet: &mut PropertySheet) {
    // A selection of several records is titled by its first section's count and kind in the UI
    // language ("2 apparaten"), as the card lines are (FT52i), or by its count when it has no section.
    let picked: usize = sheet.header.kinds.iter().map(|(_, n)| n).sum();
    if picked > 1 {
        sheet.header.title = match sheet.sections.first() {
            Some(s) => strings::format(
                key!("props-header-many"),
                &[
                    &s.targets.len().to_string(),
                    &kind_plural(&s.kind, s.targets.len()),
                ],
            ),
            None => strings::format_count(crate::count_key!("props-items"), picked),
        };
        return;
    }
    let [(kind, 1)] = sheet.header.kinds.as_slice() else {
        return;
    };
    if sheet.header.title == *kind {
        sheet.header.title = strings::kind_heading(kind);
    } else if kind == "connection" && sheet.header.title == ew_api::output::UNNUMBERED_WIRE {
        // A wire without a number in words (FT-1799), its ID row likewise.
        let words = strings::text(key!("props-wire-unnumbered")).to_owned();
        let english = std::mem::replace(&mut sheet.header.title, words.clone());
        for row in sheet.sections.iter_mut().flat_map(|s| &mut s.rows) {
            if let RowValue::One { text, .. } = &mut row.value
                && row.path == "id"
                && *text == english
            {
                *text = words.clone();
            }
        }
    } else if kind == "connection"
        && let Some(RowValue::One {
            json: Value::String(word),
            ..
        }) = sheet
            .sections
            .iter()
            .flat_map(|s| &s.rows)
            .find(|r| r.path == "line_kind")
            .map(|r| &r.value)
    {
        let words = strings::field_choice_text("connection", "line_kind", word);
        let words = strings::format(key!("props-fluid-line"), &[&words]);
        let english = std::mem::replace(&mut sheet.header.title, words.clone());
        for row in sheet.sections.iter_mut().flat_map(|s| &mut s.rows) {
            if let RowValue::One { text, .. } = &mut row.value
                && row.path == "id"
                && *text == english
            {
                *text = words.clone();
            }
        }
    }
}

/// What the one selected device is, in the UI language, the header's line under the title: what it is
/// first — a PLC station or module, a terminal, a strip or a cable by that word, anything else by its symbol's name —
/// then its natures ("Compressed-air supply point · pneumatic", "5/2 valve, solenoid and spring · pneumatic, 1
/// solenoid", "PLC station · CPU", "Valve terminal · PLC station on PROFIBUS DP", "Valve terminal · multipin, wired",
/// "Terminal"). A valve's electrical points are its solenoids, so a fluid device with solenoids counts them instead of
/// saying electrical.
pub fn what_words(what: &SheetWhat) -> String {
    use ew_api::symbols::schema::{Discipline, StationRole};
    if let Some(kind) = &what.terminal {
        return terminal_kind_text(kind);
    }
    let n = &what.natures;
    let word = |k: Key<'static>| strings::text(k).to_owned();
    let role = |r: &StationRole| {
        serde_json::to_value(r)
            .ok()
            .and_then(|v| v.as_str().map(strings::choice_text))
            .unwrap_or_default()
    };
    let (lead, mut details): (Option<String>, Vec<String>) = match &n.station {
        Some(r) => (Some(word(key!("nature-plc-station"))), vec![role(r)]),
        None if n.module => (Some(word(key!("nature-plc-module"))), Vec::new()),
        None if n.terminal => (Some(word(key!("nature-terminal"))), Vec::new()),
        None if n.strip => (Some(word(key!("nature-strip"))), Vec::new()),
        // A cable's mark stands for its cable (it read as a raw wire marker).
        None if n.cable && what.mark => (Some(word(key!("nature-cable-mark"))), Vec::new()),
        None if n.cable => (Some(word(key!("nature-cable"))), Vec::new()),
        None => (
            what.symbol.as_ref().map(crate::shell::symbol_editor::name_words),
            Vec::new(),
        ),
    };
    if n.station.is_none() && !n.module && !n.terminal && !n.strip && !n.cable {
        for d in &n.disciplines {
            let k = match d {
                Discipline::Electrical if n.fluid() && what.solenoids > 0 => continue,
                Discipline::Electrical => key!("nature-electrical"),
                Discipline::Pneumatic => key!("nature-pneumatic"),
                Discipline::Vacuum => key!("nature-vacuum"),
                Discipline::Hydraulic => key!("nature-hydraulic"),
                Discipline::Bus | Discipline::Unknown(_) => continue,
            };
            details.push(word(k));
        }
        if n.fluid() && what.solenoids > 0 {
            details.push(strings::format_count(
                count_key!("nature-solenoids"),
                what.solenoids,
            ));
        }
        if n.bus_device() {
            details.push(word(key!("nature-bus-device")));
        }
        if n.mechanical_only() {
            details.push(word(key!("nature-mechanical")));
        }
    }
    let details = details.join(", ");
    match lead.filter(|l| !l.is_empty()) {
        Some(lead) if details.is_empty() => lead,
        Some(lead) => strings::format(key!("props-what"), &[&lead, &details]),
        None => strings::sentence_start(&details).into_owned(),
    }
}

/// What kind of valve terminal it is, in words: `Valve terminal · PLC station on PROFIBUS DP`, `Valve
/// terminal · PLC station with bus node`, `Valve terminal · multipin, wired`. Properties' header, the terminal's menu
/// and its row's hover in the Devices list say it.
pub fn terminal_kind_text(kind: &ew_api::output::TerminalKind) -> String {
    use ew_api::output::TerminalKind;
    match kind {
        TerminalKind::BusNode { bus: Some(bus) } => strings::format(key!("kind-terminal-bus"), &[bus]),
        TerminalKind::BusNode { bus: None } => strings::text(key!("kind-terminal-bus-node")).to_owned(),
        TerminalKind::Multipin => strings::text(key!("kind-terminal-multipin")).to_owned(),
    }
}

/// The display text of a row's value in the grid and the describe text: one value's text, `‹varies›` with its count,
/// or empty when absent.
pub fn value_text(row: &PropRow) -> String {
    match &row.value {
        RowValue::One { text, .. } => text.clone(),
        RowValue::Varies { count, .. } => strings::format(key!("props-varies"), &[&count.to_string()]),
        RowValue::Absent => String::new(),
    }
}

/// One entry of a row's actions menu: a row action of the grid, the shell's Make variable, which remembers the row's field as a variable of the next Save as macro…, or a Parts row's Choose
/// part…, Add part… and Edit part….
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RowMenuItem {
    /// A row action of the grid.
    Action(RowAction),
    /// Make variable (`props.make_variable`).
    MakeVariable,
    /// Choose part… on a Parts row: `part.choose`, replacing the first use when the row
    /// names a part (`use` 0).
    ChoosePart {
        /// Whether the row names a part (then the choice replaces its first use).
        first_use: bool,
    },
    /// Add part… on a Parts row: `part.choose add=true`.
    AddPart,
    /// Edit part… on a Parts row that names a part: `part.edit`.
    EditPart,
    /// Go to source on a derived or inherited row whose source is a row of a record or of the project (doc 16 §6.2,
    /// FT-1651): that row, with the cursor on it.
    GoToSource,
    /// Define potential… on a Potential row of a potential without a definition: `potential.draw`'s form at the
    /// wire's or the interruption point's point (FT-1651).
    DefinePotential,
    /// Mark not used (`on`) or Mark used on a device's connection point row: `cp.unused` (FT-1649).
    CpUnused {
        /// Whether the point is to be marked not used.
        on: bool,
    },
}

/// The label of an entry of the row's actions menu.
pub fn menu_label(item: RowMenuItem) -> &'static str {
    match item {
        RowMenuItem::Action(a) => action_label(a),
        RowMenuItem::MakeVariable => strings::text(key!("props-action-make-variable")),
        RowMenuItem::ChoosePart { .. } => strings::text(key!("action-part-choose")),
        RowMenuItem::AddPart => strings::text(key!("action-part-add")),
        RowMenuItem::EditPart => strings::text(key!("action-part-edit")),
        RowMenuItem::GoToSource => strings::text(key!("props-action-go-to-source")),
        RowMenuItem::DefinePotential => strings::text(key!("action-potential-draw")),
        RowMenuItem::CpUnused { on: true } => strings::text(key!("action-cp-unused")),
        RowMenuItem::CpUnused { on: false } => strings::text(key!("props-action-cp-used")),
    }
}

/// The label of a row action in the row's actions menu (doc 16a §6.8: "Reset to derived", "Freeze").
pub fn action_label(action: RowAction) -> &'static str {
    strings::text(match action {
        RowAction::Edit => key!("props-action-edit"),
        RowAction::Clear => key!("props-action-clear"),
        RowAction::Override => key!("props-action-override"),
        RowAction::Reset => key!("props-action-reset"),
        RowAction::Freeze => key!("props-action-freeze"),
        RowAction::Unfreeze => key!("props-action-unfreeze"),
    })
}

/// The phrases `ew-api` builds a provenance chip's text from, in English for the CLI and the JSON (FT-680), each with
/// its key: a fixed phrase (`None`), or the text before and after one argument.
const CHIP_PHRASES: &[(&str, Option<&str>, Key<'static>)] = &[
    ("frozen (sign hidden)", None, key!("chip-via-frozen-sign-hidden")),
    ("frozen", None, key!("chip-via-frozen")),
    ("numbering", None, key!("chip-via-numbering")),
    ("main representation", None, key!("chip-via-main-representation")),
    ("strip groups", None, key!("chip-via-strip-groups")),
    ("function definition", None, key!("chip-via-function-definition")),
    ("override (CDP)", None, key!("chip-via-override-cdp")),
    ("wire numbering", None, key!("chip-via-wire-numbering")),
    ("connectivity", None, key!("chip-via-connectivity")),
    ("project description", None, key!("chip-via-project-description")),
    (
        "cross-reference format",
        None,
        key!("chip-via-cross-reference-format"),
    ),
    ("project default", None, key!("chip-via-project-default")),
    ("override", None, key!("chip-via-override")),
    ("primary part", None, key!("chip-via-primary-part")),
    ("layout label", None, key!("chip-via-layout-label")),
    ("part", None, key!("chip-via-part")),
    (
        "rail length less placements and end brackets",
        None,
        key!("chip-via-rail-length"),
    ),
    ("path", None, key!("chip-via-path")),
    (
        "override on another connection",
        None,
        key!("chip-via-override-on-another"),
    ),
    ("child of ", Some(""), key!("chip-via-child-of")),
    ("terminal of strip ", Some(""), key!("chip-via-terminal-of-strip")),
    ("numbering scheme '", Some("'"), key!("chip-via-numbering-scheme")),
    (
        "structure box on page ",
        Some(""),
        key!("chip-via-structure-box-on-page"),
    ),
    ("page ", Some(""), key!("chip-via-page")),
    ("override on ", Some(""), key!("chip-via-override-on")),
    (
        "project description is '",
        Some("'"),
        key!("chip-via-project-description-is"),
    ),
    (
        "potential definition at ",
        Some(""),
        key!("chip-via-potential-definition-at"),
    ),
    (
        "potential definition ",
        Some(""),
        key!("chip-via-potential-definition"),
    ),
    ("potential ", Some(""), key!("chip-via-potential")),
    ("colour profile '", Some("'"), key!("chip-via-colour-profile")),
    ("size profile '", Some("'"), key!("chip-via-size-profile")),
    ("class ", Some(""), key!("chip-via-class")),
    (
        "no supply found upstream of ",
        Some(", so its rating is not used"),
        key!("chip-via-no-supply-upstream"),
    ),
    ("built-in default (", Some(")"), key!("chip-via-built-in-default")),
    // A tube's medium and tube.
    (
        "medium definition at ",
        Some(""),
        key!("chip-via-medium-definition-at"),
    ),
    (
        "carried through the valves",
        None,
        key!("chip-via-through-valves"),
    ),
    ("no medium definition", None, key!("chip-via-no-medium")),
    // A source without a medium says what to do.
    ("no medium yet", None, key!("chip-via-no-medium-yet")),
    ("no medium at its source", None, key!("chip-via-port-no-medium")),
    (
        "project default tube",
        None,
        key!("chip-via-project-default-tube"),
    ),
    ("medium '", Some("'"), key!("chip-via-medium")),
    ("tube part ", Some(""), key!("chip-via-tube-part")),
    ("no tube specification", None, key!("chip-via-no-tube-spec")),
    // A valve's position.
    ("on no valve terminal", None, key!("chip-via-no-terminal")),
    // A valve's solenoids.
    ("PLC output ", Some(""), key!("chip-via-plc-output")),
    ("no PLC output", None, key!("chip-via-no-plc-output")),
    ("the valve's part", None, key!("chip-via-valve-part")),
    ("its plug", None, key!("chip-via-plug")),
    // An earth symbol's tag row.
    (
        "an earth connection, no tag",
        None,
        key!("chip-via-earth-connection"),
    ),
];

/// A provenance chip's words in the UI language: each ` · `-separated part of `ew-api`'s English
/// text by its [`CHIP_PHRASES`] key, its argument kept; a part without a phrase stays as it is.
fn chip_words(text: &str) -> String {
    text.split(" · ")
        .map(|part| {
            // A cable's core names two things: the cable and the core.
            if let Some((cable, core)) = part.strip_prefix("cable ").and_then(|p| p.split_once(", core ")) {
                return strings::format(key!("chip-via-cable-core"), &[cable, core]);
            }
            CHIP_PHRASES
                .iter()
                .find_map(|(before, after, key)| match after {
                    None => (part == *before).then(|| strings::text(*key).to_owned()),
                    Some(after) => {
                        let arg = part.strip_prefix(before)?.strip_suffix(after)?;
                        Some(strings::format(*key, &[arg]))
                    }
                })
                .unwrap_or_else(|| part.to_owned())
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The provenance chip's text (an icon glyph and words, never colour alone; FR-UI-025).
pub fn chip_text(p: &Provenance) -> String {
    match p {
        Provenance::Authored => strings::text(key!("chip-authored")).to_owned(),
        Provenance::Inherited { from } => strings::format(key!("chip-inherited"), &[&chip_words(from)]),
        Provenance::Derived { via } => strings::format(key!("chip-derived"), &[&chip_words(via)]),
        Provenance::Overridden { note } => strings::format(key!("chip-overridden"), &[&chip_words(note)]),
        Provenance::Frozen { note } => strings::format(key!("chip-frozen"), &[&chip_words(note)]),
        Provenance::Library { item } => strings::format(key!("chip-library"), &[item]),
        Provenance::Generated { block } => strings::format(key!("chip-generated"), &[block]),
        Provenance::Varies { count } => strings::format(key!("chip-varies"), &[&count.to_string()]),
    }
}

/// The provenance chip's one word (doc 16a §6.8): the value's source, "page" for a value inherited from a page (or
/// from a structure box on a page), "inherited" for any other inheritance; a derived value from a potential definition,
/// a profile, a cable's core or a default by that source ("potential", "profile", "cable", "default"); else the kind
/// ("derived", "override", "frozen", "library", "generated", "varies"); an authored value has no chip (the word is
/// empty). [`chip_text`] is the whole text (the tooltip and the accessible description).
pub fn chip_word(p: &Provenance) -> &'static str {
    match p {
        Provenance::Authored => "",
        Provenance::Inherited { from } if from.starts_with("page ") || from.contains(" on page ") => {
            strings::text(key!("chip-word-page"))
        }
        Provenance::Inherited { .. } => strings::text(key!("chip-word-inherited")),
        // A size that leaves a protective device's rating unread for want of a supply says so where it shows.
        Provenance::Derived { via } if via.contains("no supply found upstream of ") => {
            strings::text(key!("chip-word-no-supply"))
        }
        // A wire property names the level of the chain it comes from (FT-1651).
        Provenance::Derived { via } if via.contains(" profile '") => strings::text(key!("chip-word-profile")),
        Provenance::Derived { via } if via.starts_with("potential definition ") => {
            strings::text(key!("chip-word-potential"))
        }
        Provenance::Derived { via } if via.starts_with("cable ") => strings::text(key!("chip-word-cable")),
        Provenance::Derived { via } if via == "project default" || via.starts_with("built-in default") => {
            strings::text(key!("chip-word-default"))
        }
        Provenance::Derived { .. } => strings::text(key!("chip-word-derived")),
        Provenance::Overridden { .. } => strings::text(key!("chip-word-override")),
        Provenance::Frozen { .. } => strings::text(key!("chip-word-frozen")),
        Provenance::Library { .. } => strings::text(key!("chip-word-library")),
        Provenance::Generated { .. } => strings::text(key!("chip-word-generated")),
        Provenance::Varies { .. } => strings::text(key!("chip-word-varies")),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// A section's header names its kind in sentence case, singular for one record.
    #[test]
    fn a_section_header_reads_in_sentence_case() {
        assert_eq!(
            [
                section_header("device", 1),
                section_header("device", 2),
                section_header("potential_def", 3)
            ],
            ["Device (1)", "Devices (2)", "Potential definitions (3)"]
        );
        // One record's kind in the UI language.
        assert!(strings::set_locale("nl"));
        assert_eq!(section_header("device", 1), "Apparaat (1)");
        // Several records' kind in the plural of the UI language.
        assert_eq!(
            [section_header("device", 2), section_header("potential_def", 3)],
            ["Apparaten (2)", "Potentiaaldefinities (3)"]
        );
        assert!(strings::set_locale("en"));
    }

    /// The card line's and the toast's kind reads in the UI language inside a sentence.
    #[test]
    fn a_card_kind_reads_in_the_ui_language() {
        assert_eq!(
            [
                kind_plural("device", 1),
                kind_plural("device", 2),
                kind_plural("medium", 2)
            ],
            ["device", "devices", "media"]
        );
        assert!(strings::set_locale("nl"));
        let dutch = [
            kind_plural("device", 1),
            kind_plural("device", 2),
            kind_plural("potential_def", 3),
        ];
        assert!(strings::set_locale("en"));
        assert_eq!(dutch, ["apparaat", "apparaten", "potentiaaldefinities"]);
    }

    /// A row another command edits names the command by its title, in the UI language, never its ID.
    #[test]
    fn a_read_only_reason_names_the_command_by_its_title() {
        let folio = FieldReadOnly::Command {
            command: "page.renumber".to_owned(),
            action: None,
        };
        assert_eq!(reason_text(&folio), "changed by Renumber pages");
        assert!(strings::set_locale("nl"));
        assert_eq!(reason_text(&folio), "gewijzigd door Pagina's hernummeren");
        assert!(strings::set_locale("en"));
    }

    /// A refusal's leading rule code is split off for the secondary text; any other message stays whole.
    #[test]
    fn a_rule_code_is_split_off_the_message() {
        assert_eq!(
            split_rule_code("EW-CORE-052: interruption point 0VDC: 2 source points"),
            (Some("EW-CORE-052"), "interruption point 0VDC: 2 source points")
        );
        assert_eq!(split_rule_code("EW-POT-9: x"), (Some("EW-POT-9"), "x"));
        for whole in [
            "interruption point 0VDC: 2 source points",
            "EW-core-052: x",
            "EW-CORE-: x",
            "EW-CORE-5a: x",
        ] {
            assert_eq!(split_rule_code(whole), (None, whole));
        }
    }

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
            Decision::Card(key!("reason-bulk"))
        );
        assert_eq!(
            policy(2, true, 0, Cost::Interactive, true),
            Decision::Card(key!("reason-identifiers"))
        );
        assert_eq!(policy(1, true, 1, Cost::Interactive, true), Decision::Direct);
        assert_eq!(
            policy(1, false, 2, Cost::Interactive, true),
            Decision::Card(key!("reason-identifiers"))
        );
        assert_eq!(
            policy(1, false, 0, Cost::Bulk, true),
            Decision::Card(key!("reason-bulk-command"))
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
