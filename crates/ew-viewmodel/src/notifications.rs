// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The notifications (FR-UI-042, NFR-A11Y-04): the toasts on screen and the log of the
//! session's last notifications, with the actions that stay usable while they are valid.
//!
//! Every toast enters the log (`NotificationLog::push`): at most [`MAX_VISIBLE`] toasts are visible (a toast of a
//! key that is visible replaces that one; a fourth pushes out the oldest), and the log keeps the last [`MAX_LOG`]
//! (the oldest dropped). A toast's timeout (8 s) is the window's clock, never the model's: headless, a toast stays
//! until it is dismissed, replaced or pushed out. An entry's action is valid while what it acts on still holds: Undo
//! while the step it undoes is the next undo step of the same project session, Open while the file the job wrote in
//! this session is known. The shell judges the validity ([`crate::shell::ShellModel::notification_valid`]); this
//! module keeps the entries. Nothing here reads a clock, a path or a `Session`.

use crate::key;
use crate::strings::{self, Key};
use std::borrow::Cow;
use std::collections::VecDeque;
use std::path::PathBuf;

/// The most toasts on screen at once.
pub const MAX_VISIBLE: usize = 3;

/// The most entries the log keeps.
pub const MAX_LOG: usize = 100;

/// Declares [`NoteAction`] from one line per action: its variant, its word (in the describe text and in
/// `notifications.run`'s `action`) and the key of its button's label.
macro_rules! note_actions {
    ($($(#[$doc:meta])* $name:ident = $word:literal, $label:literal;)*) => {
        /// The action a notification offers.
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum NoteAction {
            $($(#[$doc])* $name,)*
        }

        impl NoteAction {
            /// Every action, in the order of their declaration.
            pub const ALL: &[NoteAction] = &[$(NoteAction::$name),*];

            /// The word in the describe text and in `notifications.run`'s `action`.
            pub fn word(self) -> &'static str {
                match self {
                    $(NoteAction::$name => $word,)*
                }
            }

            /// The key of the button's label in [`crate::strings`].
            pub fn label_key(self) -> Key<'static> {
                match self {
                    $(NoteAction::$name => key!($label),)*
                }
            }
        }
    };
}

note_actions! {
    /// Undo the step the notification reports.
    Undo = "undo", "toast-undo";
    /// Open the file the job wrote (Open PDF).
    Open = "open", "toast-open";
    // The actions of the one-time tips.
    /// Tip 1's Convert to connection: undo the line the tip reports and arm the Wire tool.
    Convert = "convert", "toast-convert";
    /// Tip 2's Renumber devices….
    Renumber = "renumber", "action-devices-renumber";
    /// Tip 3's Show report block: the block selected, in Properties.
    ShowBlock = "show_block", "toast-show-block";
    /// Open the history at the step the notification reports, with its changes (an external change's Show changes).
    ShowChanges = "show_changes", "toast-show-changes";
    /// Back to the symbol editor and its kept draft, after a test placement.
    BackToEditor = "edit", "toast-back-to-editor";
    /// The rack dialog of the station a placed module joined.
    Rack = "rack", "action-plc-rack-editor";
    /// New station…, for a module placed while the project has no station.
    NewStation = "new_station", "action-plc-new-station";
    /// Show the folder a practice project was made in, in the system's file manager.
    ShowFolder = "show_folder", "toast-show-folder";
    /// Swap two valves' positions on their valve terminal, after a typed position another valve holds.
    Swap = "swap", "action-fluid-swap";
    /// The layout page's devices labelled with item numbers, after a legend was added beside their tags.
    ItemNumbers = "item_numbers", "layout-show-item-numbers";
    /// Select the devices the parts list leaves out because they have no part, for Choose part….
    SelectUnlisted = "select_unlisted", "note-action-select-unlisted";
    /// Type not known yet… for the devices the parts list leaves out, so they are listed by a description.
    ListUnlisted = "list_unlisted", "action-part-placeholder";
    /// Draw the wire a placement offered for a far autoconnect hit instead of drawing it.
    Connect = "connect", "note-action-connect";
    /// Redraw the boxes of a part whose device box was changed: each as the part lays it out now.
    RedrawBoxes = "redraw_boxes", "box-redraw";
    /// The Cable form of a bus line just drawn (no cable was asked for).
    ChooseCable = "choose_cable", "note-action-choose-cable";
    /// Go to the page an Undo or a Redo changed, when it is not the page on show.
    GoTo = "go_to", "note-action-go-to-page";
    /// Route these too…: the preview of Route over the wires a page-wide Route left as the person's.
    RouteHis = "route_his", "note-action-route-his";
    /// The Messages list, after a check that found something (the check's toast had no way to it).
    ShowMessages = "show_messages", "note-action-show-messages";
    /// The mouse wheel scrolls the page (View ▸ Mouse wheel scrolls on), after its first zoom.
    WheelScrolls = "wheel_scrolls", "note-action-wheel-scrolls";
    /// Fit the layout to the sheet, after a scale at which the layout runs over the frame's drawing area.
    FitSheet = "fit_sheet", "action-layout-fit-sheet";
    /// Remove from the project the terminals a delete left drawn nowhere (beside the delete's Undo).
    RemoveUndrawn = "remove_undrawn", "note-action-remove-undrawn";
    /// Keep the tags a paste changed (a moved strip's tag follows the page): each frozen as it read before (beside the
    /// paste's Undo).
    KeepTag = "keep_tag", "note-action-keep-tags";
}

impl NoteAction {
    /// The action of a word.
    pub fn from_word(word: &str) -> Option<NoteAction> {
        NoteAction::ALL.iter().copied().find(|a| a.word() == word)
    }
}

/// What a toast's icon says (doc 16a §6.10 with the severity icons of §6.24): a finished outcome (a check in
/// `color.success`), plain information, or the worst severity a result reports.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoteTone {
    /// Done, or a result with nothing to report: `circle-check`.
    Success,
    /// Information: `info-circle`.
    Info,
    /// The result has warnings and no errors: `alert-triangle`.
    Warning,
    /// The result has errors: `circle-x`.
    Error,
}

/// What keeps an action valid (the shell's facts at the time of the notification; never printed).
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum Validity {
    /// Undo: while project session `epoch` is open, its history cursor is `cursor` and the entry below it has `rev`.
    Undo {
        /// The project session.
        epoch: u64,
        /// The history cursor right after the commit.
        cursor: usize,
        /// The revision of the entry at `cursor − 1`.
        rev: u64,
    },
    /// Open: the file the job wrote in this session (Show in folder: the folder).
    Open {
        /// The written file or folder (never printed).
        path: PathBuf,
    },
    /// A tip's action that needs no step: while project session `epoch` is open; `select` the records it selects
    /// (Show report block's block; never printed).
    Session {
        /// The project session.
        epoch: u64,
        /// The records the action selects.
        select: Vec<ew_api::output::Uuid>,
    },
    /// Show changes: while project session `epoch` is open and its history entry `entry` has `rev`.
    History {
        /// The project session.
        epoch: u64,
        /// The history entry's index.
        entry: usize,
        /// Its revision.
        rev: u64,
    },
    /// Back to the editor: while the symbol editor keeps a draft.
    SymbolDraft,
}

/// An action a notification offers: the action, what keeps it valid, and the parameters of the command it runs where
/// the offer fixed them (Connect's wire, Keep's edits, Remove's terminals), spent by its first run.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Offer {
    /// The action.
    pub(crate) action: NoteAction,
    /// What keeps it valid.
    pub(crate) validity: Validity,
    /// The parameters it runs with; `None` for an action that reads what it acts on when it runs.
    pub(crate) params: Option<serde_json::Value>,
}

impl Offer {
    /// The action `action`, valid while `validity` holds, with no parameters of its own.
    pub(crate) fn new(action: NoteAction, validity: Validity) -> Offer {
        Offer {
            action,
            validity,
            params: None,
        }
    }
}

/// A notification before it is shown: its sentences, which the log joins with a space in their order, its key, the
/// actions it offers (at most two), the tone a result sets, and what its toast shows of it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Note {
    /// The sentences, in their order; each ends with its own full stop (doc 16a §8 item 5).
    pub(crate) sentences: Vec<String>,
    /// The key: a toast of a key that is visible replaces that one.
    pub(crate) key: &'static str,
    /// The actions it offers, in their order.
    pub(crate) offers: Vec<Offer>,
    /// The tone a result set (the check's worst severity); `None` for every other notification.
    pub(crate) tone: Option<NoteTone>,
    /// The one tag a Keep keeps, which its label names ("Keep -X1021"); `None`: several, or no Keep.
    pub(crate) keep: Option<String>,
    /// What its toast shows where that is shorter than its text ([`Notification::shown`]).
    pub(crate) brief: Option<String>,
    /// Whether it stays on screen until it is dismissed, replaced or the next command commits ([`Notification::stays`]).
    pub(crate) stays: bool,
}

impl Note {
    /// The notification of key `key` that says `text`, with no action.
    pub(crate) fn new(key: &'static str, text: String) -> Note {
        Note::of(key, vec![text])
    }

    /// The notification of key `key` that says `sentences`, with no action.
    pub(crate) fn of(key: &'static str, sentences: Vec<String>) -> Note {
        Note {
            sentences,
            key,
            offers: Vec::new(),
            tone: None,
            keep: None,
            brief: None,
            stays: false,
        }
    }

    /// The same notification offering `offers` (none where an offer is `None`), in their order.
    #[must_use]
    pub(crate) fn offering(mut self, offers: impl IntoIterator<Item = Option<Offer>>) -> Note {
        self.offers.extend(offers.into_iter().flatten());
        self
    }

    /// The same notification in the tone `tone`.
    #[must_use]
    pub(crate) fn toned(mut self, tone: NoteTone) -> Note {
        self.tone = Some(tone);
        self
    }
}

/// One notification: a toast while visible, an entry of the log always.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Notification {
    /// Counted from 1 per shell.
    pub serial: u64,
    /// The text (no path, no time, no UUID): its sentences joined with a space.
    pub text: String,
    /// The key: a toast of a key that is visible replaces that one (`edit`, `export`, `recovery`).
    pub key: &'static str,
    /// The sentences its text joins.
    pub(crate) sentences: Vec<String>,
    /// The actions it offers, in their order.
    pub(crate) offers: Vec<Offer>,
    /// The tone a result set (the check's worst severity); `None` for every other notification.
    pub(crate) tone: Option<NoteTone>,
    /// The one tag a Keep keeps, which its label names ("Keep -X1021"); `None`: several, or no Keep.
    pub(crate) keep: Option<String>,
    /// What its toast shows where that is shorter than its text: the first of several reasons and how many more. The
    /// toast's tooltip and the log show the whole text.
    pub(crate) brief: Option<String>,
    /// Whether it reports a consequence the person may not want (terminals that stay in the project, wires a cut
    /// removed): it stays on screen until it is dismissed, replaced or the next command commits, no timeout.
    pub(crate) stays: bool,
}

impl Notification {
    /// Whether it stays on screen without a timeout, until dismissed, replaced or the next commit.
    pub fn stays(&self) -> bool {
        self.stays
    }

    /// The label of its action `action`: Keep names the one tag it keeps, any other its fixed words.
    pub fn label(&self, action: NoteAction) -> String {
        match (action, &self.keep) {
            (NoteAction::KeepTag, Some(tag)) => strings::format(key!("note-action-keep-tag"), &[tag]),
            _ => strings::text(action.label_key()).to_owned(),
        }
    }

    /// The action it offers, valid or not.
    pub fn action(&self) -> Option<NoteAction> {
        self.offers.first().map(|o| o.action)
    }

    /// Every action it offers, in their order (the first is [`Notification::action`]).
    pub fn actions(&self) -> Vec<NoteAction> {
        self.offers.iter().map(|o| o.action).collect()
    }

    /// The validity of its action `action`.
    pub(crate) fn validity(&self, action: NoteAction) -> Option<&Validity> {
        self.offers
            .iter()
            .find(|o| o.action == action)
            .map(|o| &o.validity)
    }

    /// The tone of its icon: the tone the result set, else success for a toast with an action, else info.
    pub fn tone(&self) -> NoteTone {
        match (self.tone, self.offers.first()) {
            (Some(tone), _) => tone,
            (None, Some(_)) => NoteTone::Success,
            (None, None) => NoteTone::Info,
        }
    }

    /// What its toast shows: its brief where it has one, else its text.
    pub fn shown(&self) -> &str {
        self.brief.as_deref().unwrap_or(&self.text)
    }

    /// The title and the optional line of the toast (doc 16a §6.10): a toned result's text ([`Self::shown`]) splits at
    /// its first `": "` into the title ("Check") and the line of counts ("4 errors, 25 warnings"); any other text is the
    /// title alone. The line stands on its own, so it starts with a capital ([`strings::sentence_start`]).
    pub fn lines(&self) -> (&str, Option<Cow<'_, str>>) {
        let shown = self.shown();
        if self.tone.is_some()
            && let Some((title, line)) = shown.split_once(": ")
        {
            return (title, Some(strings::sentence_start(line)));
        }
        (shown, None)
    }

    /// The file an Open action opens.
    pub(crate) fn open_path(&self, action: NoteAction) -> Option<&PathBuf> {
        match self.validity(action) {
            Some(Validity::Open { path }) => Some(path),
            _ => None,
        }
    }

    /// The records its action `action` selects (Show report block's block); empty for the others.
    pub(crate) fn selects(&self, action: NoteAction) -> &[ew_api::output::Uuid] {
        match self.validity(action) {
            Some(Validity::Session { select, .. }) => select,
            _ => &[],
        }
    }
}

/// The visible toasts and the log.
#[derive(Debug, Default)]
pub struct NotificationLog {
    /// Oldest first.
    entries: VecDeque<Notification>,
    /// The serials of the visible toasts, oldest first.
    visible: Vec<u64>,
    serials: u64,
    /// Whether the log popup is open.
    pub(crate) popup: bool,
}

impl NotificationLog {
    /// Adds `note` as the newest toast and log entry; returns its serial.
    pub(crate) fn push(&mut self, note: Note) -> u64 {
        self.serials += 1;
        let serial = self.serials;
        let entries = &self.entries;
        self.visible.retain(|s| {
            entries
                .iter()
                .find(|n| n.serial == *s)
                .is_none_or(|n| n.key != note.key)
        });
        self.visible.push(serial);
        if self.visible.len() > MAX_VISIBLE {
            self.visible.remove(0);
        }
        self.entries.push_back(Notification {
            serial,
            text: note.sentences.join(" "),
            key: note.key,
            sentences: note.sentences,
            offers: note.offers,
            tone: note.tone,
            keep: note.keep,
            brief: note.brief,
            stays: note.stays,
        });
        while self.entries.len() > MAX_LOG {
            if let Some(gone) = self.entries.pop_front() {
                self.visible.retain(|s| *s != gone.serial);
            }
        }
        serial
    }

    /// Takes the parameters of the offer of `action` that notification `serial` holds: an offer's parameters are
    /// spent by its first run.
    pub(crate) fn take_params(&mut self, serial: u64, action: NoteAction) -> Option<serde_json::Value> {
        self.entries
            .iter_mut()
            .rev()
            .find(|n| n.serial == serial)?
            .offers
            .iter_mut()
            .find(|o| o.action == action)?
            .params
            .take()
    }

    /// The log, newest first.
    pub fn entries(&self) -> impl Iterator<Item = &Notification> {
        self.entries.iter().rev()
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entry with `serial`, if the log still holds it.
    pub fn get(&self, serial: u64) -> Option<&Notification> {
        self.entries.iter().find(|n| n.serial == serial)
    }

    /// The visible toasts, oldest first (the newest last).
    pub fn visible(&self) -> Vec<&Notification> {
        self.visible.iter().filter_map(|s| self.get(*s)).collect()
    }

    /// The newest visible toast.
    pub fn newest_visible(&self) -> Option<&Notification> {
        self.visible.last().and_then(|s| self.get(*s))
    }

    /// The serial of the newest notification (0 before the first).
    pub(crate) fn newest_serial(&self) -> u64 {
        self.serials
    }

    /// Takes the toast `serial` off the screen (the log keeps it); `false` when it was not visible.
    pub(crate) fn dismiss(&mut self, serial: u64) -> bool {
        let before = self.visible.len();
        self.visible.retain(|s| *s != serial);
        self.visible.len() != before
    }

    /// Takes every toast off the screen.
    pub(crate) fn clear_visible(&mut self) {
        self.visible.clear();
    }

    /// Takes the toasts up to `serial` off the screen (the log keeps them).
    pub(crate) fn clear_visible_to(&mut self, serial: u64) {
        self.visible.retain(|s| *s > serial);
    }

    /// Whether the log popup is open.
    pub fn popup_open(&self) -> bool {
        self.popup
    }

    /// The serials the log holds, oldest first (the valid numbers of a refusal).
    pub(crate) fn serials(&self) -> Vec<u64> {
        self.entries.iter().map(|n| n.serial).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_toasts_at_most_and_a_visible_key_is_replaced() {
        let mut log = NotificationLog::default();
        log.push(Note::new("edit", "a".into()));
        log.push(Note::new("export", "b".into()));
        log.push(Note::new("recovery", "c".into()));
        let texts =
            |log: &NotificationLog| -> Vec<String> { log.visible().iter().map(|n| n.text.clone()).collect() };
        assert_eq!(texts(&log), ["a", "b", "c"]);
        // A toast of a visible key replaces that one; the others keep their order.
        log.push(Note::new("export", "d".into()));
        assert_eq!(texts(&log), ["a", "c", "d"]);
        // A fourth key pushes out the oldest.
        log.push(Note::new("other", "e".into()));
        assert_eq!(texts(&log), ["c", "d", "e"]);
        assert_eq!(log.newest_visible().map(|n| n.text.as_str()), Some("e"));
        assert_eq!(log.len(), 5, "the log keeps them all");
        let newest_first: Vec<u64> = log.entries().map(|n| n.serial).collect();
        assert_eq!(newest_first, [5, 4, 3, 2, 1]);
        assert!(log.dismiss(4));
        assert!(!log.dismiss(4));
        assert_eq!(texts(&log), ["c", "e"]);
    }

    #[test]
    fn the_log_keeps_the_last_hundred() {
        let mut log = NotificationLog::default();
        for i in 1..=101 {
            log.push(Note::new("edit", format!("n{i}")));
        }
        assert_eq!(log.len(), MAX_LOG);
        assert!(log.get(1).is_none(), "the 101st drops the first");
        assert_eq!(log.entries().last().map(|n| n.serial), Some(2));
        assert_eq!(log.entries().next().map(|n| n.serial), Some(101));
        assert_eq!(log.visible().len(), 1, "one key, one toast");
    }

    #[test]
    fn the_tone_is_the_result_s_else_success_with_an_action_else_info() {
        let mut log = NotificationLog::default();
        let plain = log.push(Note::new("edit", "Placed -K1".into()));
        let open = log.push(
            Note::new("export", "Exported PDF".into()).offering([Some(Offer::new(
                NoteAction::Open,
                Validity::Open { path: PathBuf::new() },
            ))]),
        );
        let checked = log.push(Note::new("edit", "Check: 1 error, 0 warnings".into()).toned(NoteTone::Error));
        let tone = |s| log.get(s).map(Notification::tone);
        assert_eq!(tone(plain), Some(NoteTone::Info));
        assert_eq!(tone(open), Some(NoteTone::Success));
        assert_eq!(tone(checked), Some(NoteTone::Error));
        // A toned result splits into the title and the line of counts; any other text is the title alone.
        assert_eq!(
            log.get(checked).map(Notification::lines),
            Some(("Check", Some("1 error, 0 warnings".into())))
        );
        assert_eq!(
            log.get(plain).map(Notification::lines),
            Some(("Placed -K1", None))
        );
    }

    /// A toast with a brief shows the brief, split as a result's text is, while the log keeps the whole text (Arrange
    /// and route's first reason and how many more, doc 16a §6.10).
    #[test]
    fn a_toast_shows_its_brief_and_the_log_the_whole_text() {
        let mut log = NotificationLog::default();
        let whole = "Nothing to arrange: -K1: no room. -K2: no room. 1 fault stays on the page.";
        let s = log.push(Note {
            brief: Some("Nothing to arrange: -K1: no room. And 2 more.".into()),
            ..Note::new("edit", whole.into()).toned(NoteTone::Info)
        });
        let n = log.get(s).expect("the toast");
        assert_eq!(
            n.lines(),
            ("Nothing to arrange", Some("-K1: no room. And 2 more.".into()))
        );
        assert_eq!(log.entries().next().map(|n| n.text.as_str()), Some(whole));
    }

    /// The line under a toast's title starts with a capital where it started lowercase; the text
    /// itself, a digit or a designation first, stays as it is.
    #[test]
    fn a_toasts_line_starts_with_a_capital() {
        let mut log = NotificationLog::default();
        let mut line = |text: &str| {
            let s = log.push(Note::new("edit", text.into()).toned(NoteTone::Warning));
            log.get(s)
                .map(|n| (n.lines().0.to_owned(), n.lines().1.map(Cow::into_owned)))
        };
        assert_eq!(
            line("Not placed: the interruption point W464 is already here; place it on a pin"),
            Some((
                "Not placed".to_owned(),
                Some("The interruption point W464 is already here; place it on a pin".to_owned())
            ))
        );
        assert_eq!(
            line("Place symbol: draw on a schematic page"),
            Some((
                "Place symbol".to_owned(),
                Some("Draw on a schematic page".to_owned())
            ))
        );
        assert_eq!(
            line("Check: 4 errors, 25 warnings"),
            Some(("Check".to_owned(), Some("4 errors, 25 warnings".to_owned())))
        );
        assert_eq!(
            line("Bound: -K1 is on the card"),
            Some(("Bound".to_owned(), Some("-K1 is on the card".to_owned())))
        );
        assert_eq!(
            log.get(1).map(|n| n.text.as_str()),
            Some("Not placed: the interruption point W464 is already here; place it on a pin")
        );
    }

    /// Each offer carries its own parameters: of two toasts that offer Connect, running the earlier one takes the
    /// earlier wire, which is spent, and the later toast keeps its own.
    #[test]
    fn an_offer_runs_its_own_parameters_once() {
        let mut log = NotificationLog::default();
        let connect = |wire: &str| {
            Some(Offer {
                params: Some(serde_json::json!({ "wire": wire })),
                ..Offer::new(NoteAction::Connect, Validity::SymbolDraft)
            })
        };
        let first = log.push(Note::new("edit", "Placed -K1.".into()).offering([connect("a")]));
        let second = log.push(Note::new("edit", "Placed -K2.".into()).offering([connect("b")]));
        assert_eq!(
            log.take_params(first, NoteAction::Connect),
            Some(serde_json::json!({ "wire": "a" }))
        );
        assert_eq!(
            log.take_params(first, NoteAction::Connect),
            None,
            "spent by its first run"
        );
        assert_eq!(
            log.take_params(second, NoteAction::Connect),
            Some(serde_json::json!({ "wire": "b" }))
        );
    }

    #[test]
    fn action_words_round_trip() {
        for a in [NoteAction::Undo, NoteAction::Open] {
            assert_eq!(NoteAction::from_word(a.word()), Some(a));
        }
        assert_eq!(NoteAction::from_word("redo"), None);
    }
}
