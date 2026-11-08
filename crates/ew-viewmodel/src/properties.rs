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
use crate::strings::Key;
use crate::{count_key, key};
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
    /// Each item's secondary line, in the items' order (FT-1478: an address profile's reference under its name); empty
    /// when the items have none.
    pub details: Vec<String>,
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
        /// The gesture, with its title (WP-S3.08 D10).
        gesture: crate::navigators::table::TableGesture,
    },
    // WP-S1.19 part 6
    /// A macro's insertion from the Macros navigator (`macro.insert`, §11, FR-MAC-005): the card in the Preview tab
    /// with the conflicts the user decides on before Apply; a refused preview (a prevent rule) still shows them.
    Macro(Box<MacroInsert>),
    // WP-S2.07 part 4a
    /// The part editor's Save (`part.edit_copy`, D20): the card in the Preview tab with the field diff, the devices
    /// that use the copy, the BOM rows that change and, for a fork, the fork note.
    Part(Box<PartCard>),
    // WP-S2.07 part 4b
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
    // WP-S3.08 part 3
    /// A table round trip's file read against a navigator's table (`navigator.import_table`, D14, ruling 6 of
    /// ID-1224): the card in the Preview tab with the counts and the per-cell list; Apply commits the import's one
    /// `fields.set_many`.
    TableImport(Box<TableImportCard>),
    // WP-S2.19 D13
    /// Route page and Route selection (`route.page`, `route.selection`): the card in the Preview tab with the plan's
    /// lead line and one row per connection left unrouted; Apply commits the plan's `connection.route_many`.
    Route {
        /// "Route 3 wires on folio 101: 2 routed, 1 not routed." (§12a).
        lead: String,
        /// One row per connection left unrouted, with its reason.
        rows: Vec<String>,
        /// The page in words (`folio 101`), for the toast.
        page: String,
        /// How many wires the plan routes.
        routed: usize,
    },
}

/// What a table import card shows and commits (WP-S3.08 D14).
#[derive(Clone, PartialEq, Debug)]
pub struct TableImportCard {
    /// The file's base name (`strips.xlsx`).
    pub source: String,
    /// The file read against the project: the cells, the findings, the missing rows and the command.
    pub import: ew_api::output::TableImport,
    /// The file's bytes, which the switch "Take the file's values" reads again (FT-1149).
    pub bytes: std::sync::Arc<Vec<u8>>,
    /// Whether a cell changed in both takes the file's value (the switch, WP-S3.08 D14).
    pub take_file: bool,
}

// The import holds its command's parameters as JSON, which has no NaN: equality is total.
impl Eq for TableImportCard {}

impl TableImportCard {
    /// Whether the file changes nothing it could apply (no command): its Apply is disabled (FT-1193, ID-1391).
    pub fn nothing_to_import(&self) -> bool {
        self.import.command.is_none()
    }

    /// The card's counts line: "5 cells changed, 0 cells kept, 1 conflict: the project's value is kept, …", each count
    /// with its noun (FT-1194); a file that matches the project says so in one sentence (FT-1193, ID-1391).
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
    /// file's value` (what Apply will do, FT-1194), with the reason of a read-only or refused cell.
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

/// What the paste choice card shows and keeps (WP-S2.07 D22).
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

/// One copy of the paste choice card (WP-S2.07 D22).
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
    /// change size in words (cycle 39, FIX39b-1), the describe line keeps the class word.
    pub fn conflict_text(c: &PasteConflict) -> String {
        let size = match c.class {
            "major" => strings::text(key!("paste-class-major")),
            "minor" => strings::text(key!("paste-class-minor")),
            _ => strings::text(key!("paste-class-patch")),
        };
        strings::format(key!("paste-conflict"), &[&c.title, &c.held, &c.pasted, size])
    }
}

/// What a part card shows (WP-S2.07 D20).
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
    /// Whether saving forks the copy.
    pub fork: bool,
}

/// What a macro card inserts and the conflicts it lists (WP-S1.19 §11).
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
    /// The variables' ad-hoc value fields, in the macro's order (FT-785).
    pub fields: Vec<MacroField>,
    /// The conflicts in words, the refusals first (`Session::macro_conflicts`).
    pub conflicts: Vec<String>,
}

/// One variable's ad-hoc value field on a macro card (FT-785): a typed value is a literal of `macro.insert`, which wins
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
    pub reason: Key<'static>,
    /// Where it was opened (WP-S2.05 part 2).
    pub origin: CardOrigin,
}

impl Card {
    /// The lead line of a failed card (cycle 38, FX38d; doc 16a P3, words not IDs) and the rule code that may follow it
    /// in secondary text; `None` while the preview stands. A macro card leads with a sentence that says what blocks
    /// the apply and how many conflicts its list shows (the list is the detail, so the lead never repeats a row); the
    /// other cards keep `card-failed` with the command's message.
    pub fn failed_lead(&self) -> Option<FailedLead> {
        let message = self.preview.result.as_ref().err()?;
        // WP-S2.07 D22: the paste choice card leads with what it asks; its rows and choices are the detail.
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
        // FT-785: a typed value the command did not take leads in words naming its variable; the reason is under the
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
    /// The revision the proposal was computed on: accepting it on another is refused.
    pub(crate) base_rev: u64,
    /// WP-S2.18 D3 (FT-628): a join instead (an existing device's tag typed into the Tag field), whose `params` are
    /// `rep.join_device`'s; `tag` is then that device's tag and `counter` empty.
    pub(crate) join: bool,
}

impl TagProposal {
    /// Whether it is a join (WP-S2.18 D3) rather than a tag.
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
    /// row, and the commands' texts are lowercase (FT-1075, ID-1182; [`crate::strings::sentence_start`]).
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
            RowGroup::Main => strings::text(key!("category-main")),
            RowGroup::Category(c) => strings::text(Key::run_time(&format!("category-{}", c.word()))),
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
            // FT-554: whether the wire prints its marker, beside the cross-section and the colour it prints.
            "overrides.show_marker",
        ],
    ),
];

/// The internal members shown only under Advanced, by the row's path: the record version, the ID, the source reference
/// with its key and lineage, the tag counter, the form's UUID and (FT-1019, ID-1107) the members of a page's connection
/// grid, a fraction of nanometres (`num`, `den`) with no meaning to the user, whose row itself is a choice (ID-1477
/// (1)), and (ID-1389) a connection's route, drawn on the canvas.
const ADVANCED: &[&str] = &[
    "$v",
    "id",
    "source_ref",
    "tag.counter",
    "form",
    "grid.num",
    "grid.den",
    "route",
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
    let main = marker_beside_colour(main);
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

/// The Main rows with a connection's `overrides.show_marker` moved right after the colour it prints (FT-554); the rows
/// as they are when either is absent.
fn marker_beside_colour(mut rows: Vec<&PropRow>) -> Vec<&PropRow> {
    let Some(from) = rows.iter().position(|r| r.path == "overrides.show_marker") else {
        return rows;
    };
    let marker = rows.remove(from);
    let at = rows
        .iter()
        .position(|r| r.key.ends_with("/derived.colour"))
        .map_or(from, |i| i + 1);
    rows.insert(at, marker);
    rows
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
/// `advanced` is on; with a non-empty `filter`, only the rows it matches and their parents ([`filtered`]).
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
                filtered(s, rows, filter)
                    .into_iter()
                    .map(|row| VisibleRow { section: s, row }),
            );
        }
    }
    out
}

/// The rows of one group that the Properties filter `filter` keeps: those it matches ([`row_matches`]) and, among
/// the group's rows, their parents, so that two rows on show never share a label without their parents (FT-1358, doc
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
        return Some(strings::format(key!("props-ro-generated"), &[block]));
    }
    if is_derived(row) || is_header(section, row) {
        return None;
    }
    match &row.route {
        EditRoute::FieldsSet { .. } | EditRoute::SettingsMember { .. } | EditRoute::Command { .. } => None,
        EditRoute::ReadOnly(reason) => Some(reason_text(reason)),
    }
}

/// The refusal of an edit of read-only row `row` as a sentence (FT-1051): the identity, a command and no command each
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
        // FT-1353: the command by its title ("Renumber pages"), never its ID.
        FieldReadOnly::Command { command, .. } => {
            strings::format(key!("props-ro-command"), &[strings::command_title(command)])
        }
        FieldReadOnly::HumanOnly { step, .. } => strings::format(key!("props-ro-human-only"), &[step]),
        FieldReadOnly::NoCommand => strings::text(key!("props-ro-no-command")).to_owned(),
    }
}

/// The settable member of a route: the `fields.set` path, or the settings member's row path.
pub fn member_path(route: &EditRoute) -> Option<String> {
    match route {
        EditRoute::FieldsSet { path, .. } => Some(path.clone()),
        EditRoute::SettingsMember { member } => Some(format!("settings.{member}")),
        // WP-S4.02 part 4 (ruling 3): a layout row has no settable member; `Session::layout_edit` compiles its text.
        EditRoute::Command { .. } | EditRoute::ReadOnly(_) => None,
    }
}

/// The settable member's new value with a leaf replaced (§6.6), moved down to `ew-api` with table mode's pure rule
/// (WP-S3.08 D10; ID-1174 ruling 3), re-exported under its old path.
pub use ew_api::output::table_edit::compose;

/// The kind's plural for the card and the toast (`devices`), in the UI language inside a sentence (FT-1479:
/// `apparaten`).
pub fn kind_plural(kind: &str, n: usize) -> String {
    if n == 1 {
        return strings::in_sentence(&strings::kind_heading(kind));
    }
    strings::in_sentence(&strings::kinds_heading(kind))
}

/// The header of a sheet's section of `n` records of `kind` (ID-1139): the kind in sentence case, singular for one,
/// with the count ("Device (1)", "Devices (2)"), in the UI language (FT-1356: "Apparaat (1)"; FT-1411: "Apparaten
/// (2)").
pub fn section_header(kind: &str, n: usize) -> String {
    if n == 1 {
        return format!("{} ({n})", strings::kind_heading(kind));
    }
    format!("{} ({n})", strings::kinds_heading(kind))
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

/// Writes a page's Connection grid rows in words (ID-1139): the group `{ num, den }` has no text of its own, so its
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
    // ID-1389 (W51 finding I): the grid factor (`num`, nanometres) as a length in millimetres, never the count.
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

/// Writes a connection's ends A and B in words (ID-1389, W51 finding G): each end as the live description says it
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
}

/// A choice of a row its own command commits in the UI language's words (ID-1389, W51 finding J): a panel page's Label
/// mode as its `choice-label_mode-<mode>` text, a Surface as its enclosure's tag and the keyed word for the plate
/// ("-UC1 montageplaat", with its ordinal " (2)" kept); any other choice as it is. The option itself is what the
/// command reads.
pub fn command_choice_text(path: &str, option: &str) -> String {
    match path.rsplit('.').next().unwrap_or(path) {
        "label_mode" => strings::lookup(&format!("choice-label_mode-{}", option.to_lowercase()))
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

/// Writes the values of the enumerated rows in the UI language's words (FT-1258): each stored word as
/// [`strings::choice_text`] gives it ("Multi-line", "Meerlijnschema"), as the row's choices show them. The JSON values
/// are unchanged, and a word typed in the editor is read back ([`enum_value_of`]). A layout row (WP-S4.02 D12) keeps
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

/// The stored word of an enumerated row that `text` names in the UI language's words (FT-1258), else `text` as typed:
/// the parser reads the stored words.
pub fn enum_value_of<'t>(kind: &str, row: &PropRow, text: &'t str) -> std::borrow::Cow<'t, str> {
    if matches!(row.route, EditRoute::Command { .. }) {
        // ID-1389: a Surface view choice typed in the UI language's words is its option.
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
/// words, else `text` as typed: what [`enum_value_of`] reads for a row and a table cell for its column's field
/// (C50-p5#3).
pub fn enum_word_of<'t>(kind: &str, path: &str, ty: &FieldType, text: &'t str) -> std::borrow::Cow<'t, str> {
    let FieldType::Enum { options } = ty.inner() else {
        return text.into();
    };
    let typed = text.trim();
    options
        .iter()
        .find(|o| strings::field_choice_text(kind, path, o).eq_ignore_ascii_case(typed))
        .map_or(text.into(), |o| o.clone().into())
}

/// A Properties card's summary line (`card-line`): its targets, then its changes, tags and cross-reference texts each
/// counted with its own singular (FIX50b, as the quick fix's, FT-1259); the window and the describe text show it.
pub fn card_line(card: &Card) -> String {
    let (tags, xrefs) = card_counts(card);
    strings::format(
        key!("card-line"),
        &[
            &card.targets.to_string(),
            &kind_plural(&card.edit.kind, card.targets),
            &strings::format_count(crate::count_key!("preview-changes-count"), card.changes),
            &tags,
            &xrefs,
        ],
    )
}

/// A reorder card's summary line (`card-reorder`): its moves (`notices`), then its tags and cross-reference texts each
/// counted with its own singular (FIX50b).
pub fn reorder_line(card: &Card, notices: &[String]) -> String {
    let (tags, xrefs) = card_counts(card);
    strings::format(key!("card-reorder"), &[&notices.join(", "), &tags, &xrefs])
}

/// A quick fix card's summary line (`card-fix`): its changes (FT-1366), tags and cross-reference texts each counted
/// with its own singular (FT-1259); the window and the describe text show it (C50-p6#3).
pub fn fix_line(card: &Card) -> String {
    let (tags, xrefs) = card_counts(card);
    let records = strings::format_count(crate::count_key!("preview-changes-count"), card.changes);
    strings::format(key!("card-fix"), &[&records, &tags, &xrefs])
}

/// A card's tags and cross-reference texts, counted.
fn card_counts(card: &Card) -> (String, String) {
    (
        strings::format_count(
            crate::count_key!("preview-tags-count"),
            card.preview.derived.tags.len(),
        ),
        strings::format_count(
            crate::count_key!("preview-xrefs-count"),
            card.preview.derived.xrefs.len(),
        ),
    )
}

/// The refusal of a word that is not one of an enumerated field's (FT-1240), in the UI language: a potential's Type
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

/// Writes a report block's row source in words on a generated page's sheet (FT-1287, ID-1315 ruling 3): a built-in
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
                // C50-p5#7: a kind of a newer format reads as its words, never as JSON.
                *text = strings::lookup(&format!("report-kind-{}", kind.replace('_', "-")))
                    .map_or_else(|| strings::choice_words(kind), str::to_owned);
            } else if let Some(file) = json.pointer("/sql_query/file").and_then(Value::as_str) {
                *text = strings::format(key!("report-source-sql"), &[file]);
            }
        }
    }
}

/// Writes the values `ew-api` gives in English words in the UI language's (FT-1357): a yes or no ("ja", "nee"), a
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

/// A structured value in words (FT-1357): its name (a profile's "DBU/WH 24 V", in the UI language where it has one),
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

/// Writes the sheet's header in words (FT-1357, FT-1319): a record without a title of its own by its kind as a heading
/// in the UI language ("Project", not `project`), a fluid line by its line kind ("Working line", "Werkleiding"), its
/// ID row likewise, and a selection of several records by its count and kind (ID-1427: "2 apparaten").
pub fn header_in_words(sheet: &mut PropertySheet) {
    // ID-1427 (W52 F4): a selection of several records is titled by its first section's count and kind in the UI
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

/// The display text of a row's value in the grid and the describe text: one value's text, `‹varies›` with its count,
/// or empty when absent.
pub fn value_text(row: &PropRow) -> String {
    match &row.value {
        RowValue::One { text, .. } => text.clone(),
        RowValue::Varies { count, .. } => strings::format(key!("props-varies"), &[&count.to_string()]),
        RowValue::Absent => String::new(),
    }
}

/// One entry of a row's actions menu (FT-530): a row action of the grid, the shell's Make variable (WP-S1.19 §11,
/// FT-754 (h)), which remembers the row's field as a variable of the next Save as macro…, or a Parts row's Choose
/// part…, Add part… and Edit part… (WP-S2.07 part 4a, §9.4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RowMenuItem {
    /// A row action of the grid.
    Action(RowAction),
    /// Make variable (`props.make_variable`).
    MakeVariable,
    /// Choose part… on a Parts row (WP-S2.07 part 4a, §9.4): `part.choose`, replacing the first use when the row
    /// names a part (`use` 0).
    ChoosePart {
        /// Whether the row names a part (then the choice replaces its first use).
        first_use: bool,
    },
    /// Add part… on a Parts row: `part.choose add=true`.
    AddPart,
    /// Edit part… on a Parts row that names a part: `part.edit`.
    EditPart,
}

/// The label of an entry of the row's actions menu.
pub fn menu_label(item: RowMenuItem) -> &'static str {
    match item {
        RowMenuItem::Action(a) => action_label(a),
        RowMenuItem::MakeVariable => strings::text(key!("props-action-make-variable")),
        RowMenuItem::ChoosePart { .. } => strings::text(key!("action-part-choose")),
        RowMenuItem::AddPart => strings::text(key!("action-part-add")),
        RowMenuItem::EditPart => strings::text(key!("action-part-edit")),
    }
}

/// The label of a row action in the row's actions menu (FT-530; doc 16a §6.8: "Reset to derived", "Freeze").
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
    ("conductor ", Some(""), key!("chip-via-conductor")),
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
    ("built-in default (", Some(")"), key!("chip-via-built-in-default")),
];

/// A provenance chip's words in the UI language (FT-1338, ID-1392): each ` · `-separated part of `ew-api`'s English
/// text by its [`CHIP_PHRASES`] key, its argument kept; a part without a phrase stays as it is.
fn chip_words(text: &str) -> String {
    text.split(" · ")
        .map(|part| {
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
/// from a structure box on a page), "inherited" for any other inheritance, else the kind ("derived", "override",
/// "frozen", "library", "generated", "varies"); an authored value has no chip (the word is empty). [`chip_text`] is
/// the whole text (the tooltip and the accessible description).
pub fn chip_word(p: &Provenance) -> &'static str {
    match p {
        Provenance::Authored => "",
        Provenance::Inherited { from } if from.starts_with("page ") || from.contains(" on page ") => {
            strings::text(key!("chip-word-page"))
        }
        Provenance::Inherited { .. } => strings::text(key!("chip-word-inherited")),
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

    /// ID-1139: a section's header names its kind in sentence case, singular for one record.
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
        // FT-1356: one record's kind in the UI language.
        assert!(strings::set_locale("nl"));
        assert_eq!(section_header("device", 1), "Apparaat (1)");
        // FT-1411: several records' kind in the plural of the UI language.
        assert_eq!(
            [section_header("device", 2), section_header("potential_def", 3)],
            ["Apparaten (2)", "Potentiaaldefinities (3)"]
        );
        assert!(strings::set_locale("en"));
    }

    /// FT-1479: the card line's and the toast's kind reads in the UI language inside a sentence.
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

    /// FT-1353: a row another command edits names the command by its title, in the UI language, never its ID.
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

    /// FX38d: a refusal's leading rule code is split off for the secondary text; any other message stays whole.
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
