// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The notifications (FR-UI-042, NFR-A11Y-04): the toasts on screen and the log of the
//! session's last notifications, with the actions that stay usable while they are valid.
//!
//! Every toast enters the log (`NotificationLog::push`): at most [`MAX_VISIBLE`] toasts are visible (a toast of a
//! key that is visible replaces that one; a fourth pushes out the oldest), and the log keeps the last [`MAX_LOG`]
//! (the oldest dropped). A toast's timeout (8 s) is the window's clock, never the model's: headless, a toast stays
//! until it is dismissed, replaced or pushed out. Every toast goes after its time, whatever its tone or offer: its
//! tone says what happened, and nothing else; its offer stays usable in the log. An entry's offer is valid while what
//! it acts on still holds: Undo while the step it undoes is the next undo step of the same project session, Open while
//! the file the job wrote in this session is known. The shell judges the validity ([`crate::shell::ShellModel::notification_valid`]) and runs an
//! offer; this module keeps the entries. Nothing here reads a clock, a path or a `Session`.
//!
//! **An offer** is Undo, Open, Swap, Route these too…, or the one generic offer: actions of the table with their
//! parameters, run in their order (the records it selects are its `select.set`). Each has its word (in the describe
//! text and in `notifications.run`'s `action`) and its short label.

use crate::key;
use crate::strings::{self, Key};
use std::borrow::Cow;
use std::collections::VecDeque;
use std::path::PathBuf;

/// The most toasts on screen at once.
pub const MAX_VISIBLE: usize = 3;

/// The most entries the log keeps.
pub const MAX_LOG: usize = 100;

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

/// What keeps an offer valid (the shell's facts at the time of the notification; never printed).
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
    /// Always (Open: the file the job wrote in this session; Show in folder: the folder).
    Always,
    /// While project session `epoch` is open.
    Session {
        /// The project session.
        epoch: u64,
    },
    /// While project session `epoch` is open and page `page` is in it; with `shown`, while it is the page on show.
    Page {
        /// The project session.
        epoch: u64,
        /// The page.
        page: ew_api::output::Uuid,
        /// Whether the page must be on show.
        shown: bool,
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
    /// Back to the drawing: while a drawing is in hand.
    SymbolDraft,
}

/// What an offer runs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum Run {
    /// Undo of the step the notification reports (the toast goes, and the Undo says nothing of its own).
    Undo,
    /// The written file or folder handed to the system's opener (never printed).
    Open(PathBuf),
    /// Two valves change their positions on their valve terminal: `device` takes `holder`'s.
    Swap {
        /// The valve that moves.
        device: ew_api::output::Uuid,
        /// The valve that holds the position it takes.
        holder: ew_api::output::Uuid,
    },
    /// Route these too…: the preview of Route over the wires a page-wide Route left as the person's, on the page on
    /// show (a card of its own, which no action of the table opens).
    RouteHis,
    /// Actions of the table, each with its parameters, run in their order until one is not done; the toast goes,
    /// unless it `keeps` (Show changes leaves the external change's toast up).
    Steps {
        /// The actions and their parameters.
        steps: Vec<(&'static str, serde_json::Value)>,
        /// Whether its first run spends it (it fixed what it commits: Connect's wire, Keep's edits, Remove's
        /// terminals).
        once: bool,
        /// Whether the toast stays when it runs.
        keeps: bool,
    },
}

/// An action a notification offers: its word, its short label, what keeps it valid and what it runs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Offer {
    /// Its word, in the describe text and in `notifications.run`'s `action`.
    pub(crate) word: &'static str,
    /// Its short label.
    pub(crate) label: String,
    /// What keeps it valid.
    pub(crate) validity: Validity,
    /// What it runs.
    pub(crate) run: Run,
    /// Whether a run spent it (a `once` offer).
    pub(crate) spent: bool,
}

impl Offer {
    /// The offer `word`, labelled `label`, valid while `validity` holds, running `run`.
    pub(crate) fn new(word: &'static str, label: String, validity: Validity, run: Run) -> Offer {
        Offer {
            word,
            label,
            validity,
            run,
            spent: false,
        }
    }

    /// Undo of the step the notification reports, valid while `validity` holds.
    pub(crate) fn undo(validity: Validity) -> Offer {
        Offer::new(
            "undo",
            strings::text(key!("toast-undo")).to_owned(),
            validity,
            Run::Undo,
        )
    }

    /// The offer `word` labelled with the text of `label`: the actions `steps` with their parameters, in their order.
    pub(crate) fn steps(
        word: &'static str,
        label: Key<'static>,
        validity: Validity,
        steps: Vec<(&'static str, serde_json::Value)>,
    ) -> Offer {
        Offer::new(
            word,
            strings::text(label).to_owned(),
            validity,
            Run::Steps {
                steps,
                once: false,
                keeps: false,
            },
        )
    }

    /// The same offer, spent by its first run.
    #[must_use]
    pub(crate) fn once(mut self) -> Offer {
        if let Run::Steps { once, .. } = &mut self.run {
            *once = true;
        }
        self
    }

    /// The same offer, leaving its toast up when it runs.
    #[must_use]
    pub(crate) fn keeping(mut self) -> Offer {
        if let Run::Steps { keeps, .. } = &mut self.run {
            *keeps = true;
        }
        self
    }

    /// The same offer labelled `label` (a label that counts or names what it acts on).
    #[must_use]
    pub(crate) fn labelled(mut self, label: String) -> Offer {
        self.label = label;
        self
    }
}

/// Back to the drawing in hand in Parts, after a test placement on a page, while it is in hand.
pub(crate) fn back_to_editor() -> Offer {
    Offer::steps(
        "edit",
        key!("toast-back-to-editor"),
        Validity::SymbolDraft,
        vec![("parts.new_symbol", serde_json::json!({ "like": "hand" }))],
    )
}

/// The step of a generic offer that selects `records`, the records it acts on, as a person's selection is set.
pub(crate) fn select_step(records: &[ew_api::output::Uuid]) -> (&'static str, serde_json::Value) {
    let ids: Vec<String> = records.iter().map(ToString::to_string).collect();
    ("select.set", serde_json::json!({ "ids": ids }))
}

/// The step of a generic offer that commits the command `cmd` with `params` (`command.run`).
pub(crate) fn command_step(cmd: &str, params: serde_json::Value) -> (&'static str, serde_json::Value) {
    ("command.run", serde_json::json!({ "cmd": cmd, "params": params }))
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
    /// What its toast shows where that is shorter than its text ([`Notification::shown`]); the log may add the times
    /// in a row ([`NotificationLog::push`]).
    pub(crate) brief: Option<String>,
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
            brief: None,
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
    /// What its toast shows where that differs from its text: the first of several reasons and how many more, and the
    /// times in a row its key's toast said it ([`NotificationLog::push`]). The toast's tooltip and the log show the
    /// whole text.
    pub(crate) brief: Option<String>,
    /// How many times in a row its key's toast said its text, it included (1: the first).
    pub(crate) repeats: usize,
}

impl Notification {
    /// The short label of its offer `word` (empty for a word it does not offer).
    pub fn label(&self, word: &str) -> &str {
        self.offer(word).map_or("", |o| o.label.as_str())
    }

    /// The word of the offer it offers first, valid or not.
    pub fn action(&self) -> Option<&'static str> {
        self.offers.first().map(|o| o.word)
    }

    /// The words of every offer, in their order (the first is [`Notification::action`]).
    pub fn actions(&self) -> Vec<&'static str> {
        self.offers.iter().map(|o| o.word).collect()
    }

    /// Its offer `word`.
    pub(crate) fn offer(&self, word: &str) -> Option<&Offer> {
        self.offers.iter().find(|o| o.word == word)
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

/// The ways to the places `text` names: each navigator of the left rail whose title of more than one word ("Unplaced
/// devices", "Terminal strips") the text holds, as the offer that shows it, after `offers` and up to two in all. A
/// sentence that names a pane is a way to it; a title of one word ("Cables", "PLC") also reads as an ordinary word,
/// so it is no way.
fn place_offers(text: &str, offers: &[Offer]) -> Vec<Offer> {
    let room = 2usize.saturating_sub(offers.len());
    crate::rail::LEFT
        .iter()
        .filter_map(|(word, _, pane)| match pane {
            crate::layout::PaneKind::Navigator(kind) => Some((*word, *kind, crate::rail::left_label(word, *pane))),
            _ => None,
        })
        .filter(|(word, _, title)| {
            let title = strings::text(*title);
            title.contains(' ') && text.contains(title) && !offers.iter().any(|o| o.word == *word)
        })
        .take(room)
        .map(|(word, kind, title)| {
            Offer::steps(
                word,
                title,
                Validity::Always,
                vec![("navigator.show", serde_json::json!({ "kind": kind.word() }))],
            )
        })
        .collect()
}

impl NotificationLog {
    /// Adds `note` as the newest toast and log entry; returns its serial. A toast that says what the visible toast of
    /// its key says counts the times in a row on its brief ("… (2×)"), so that a second run that changes nothing is
    /// seen to have run (Arrange and route with nothing to arrange); the log keeps the text. A toast that names a pane
    /// of the left rail offers it ([`place_offers`]).
    pub(crate) fn push(&mut self, mut note: Note) -> u64 {
        self.serials += 1;
        let serial = self.serials;
        let text = note.sentences.join(" ");
        let ways = place_offers(&text, &note.offers);
        note.offers.extend(ways);
        let repeats = self
            .visible()
            .iter()
            .find(|n| n.key == note.key && n.text == text)
            .map_or(1, |n| n.repeats + 1);
        if repeats > 1 {
            let shown = note.brief.take().unwrap_or_else(|| text.clone());
            note.brief = Some(strings::format(
                key!("note-said-again"),
                &[&shown, &repeats.to_string()],
            ));
        }
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
            text,
            repeats,
            key: note.key,
            sentences: note.sentences,
            offers: note.offers,
            tone: note.tone,
            brief: note.brief,
        });
        while self.entries.len() > MAX_LOG {
            if let Some(gone) = self.entries.pop_front() {
                self.visible.retain(|s| *s != gone.serial);
            }
        }
        serial
    }

    /// Marks the offer `word` of notification `serial` spent: a `once` offer runs once.
    pub(crate) fn spend(&mut self, serial: u64, word: &str) {
        if let Some(o) = self
            .entries
            .iter_mut()
            .rev()
            .find(|n| n.serial == serial)
            .and_then(|n| n.offers.iter_mut().find(|o| o.word == word))
        {
            o.spent = true;
        }
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
                "open",
                "Open".into(),
                Validity::Always,
                Run::Open(PathBuf::new()),
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

    /// A persona run: a second Arrange and route with nothing to arrange showed the first toast as it stood, so
    /// nothing said it ran. A toast that says again what the visible toast of its key says counts the times in a row;
    /// another text, or the same text after the toast left, starts again, and the log keeps the text.
    #[test]
    fn a_toast_said_again_counts_the_times_in_a_row() {
        let mut log = NotificationLog::default();
        let nothing =
            || Note::new("edit", "Nothing to arrange: the page is arranged.".into()).toned(NoteTone::Info);
        let first = log.push(nothing());
        let second = log.push(nothing());
        let third = log.push(nothing());
        let lines = |log: &NotificationLog, s| log.get(s).map(|n| n.lines().1.map(Cow::into_owned));
        assert_eq!(lines(&log, first), Some(Some("The page is arranged.".into())));
        assert_eq!(
            lines(&log, second),
            Some(Some("The page is arranged. (2×)".into()))
        );
        assert_eq!(
            lines(&log, third),
            Some(Some("The page is arranged. (3×)".into()))
        );
        assert_eq!(
            log.get(third).map(|n| n.text.as_str()),
            Some("Nothing to arrange: the page is arranged.")
        );
        assert!(log.dismiss(third));
        let after = log.push(nothing());
        assert_eq!(lines(&log, after), Some(Some("The page is arranged.".into())));
        let other = log.push(Note::new("edit", "Placed -K1.".into()));
        assert_eq!(log.get(other).map(Notification::shown), Some("Placed -K1."));
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

    /// Each offer runs its own steps: of two toasts that offer Connect, the earlier one spent is spent alone, and the
    /// later toast keeps its own; an offer is found by its word, with its label.
    #[test]
    fn an_offer_is_its_word_its_label_and_its_own_steps() {
        let mut log = NotificationLog::default();
        let connect = |wire: &str| {
            Some(
                Offer::steps(
                    "connect",
                    key!("note-action-connect"),
                    Validity::SymbolDraft,
                    vec![command_step(
                        "connection.draw",
                        serde_json::json!({ "wire": wire }),
                    )],
                )
                .once(),
            )
        };
        let first = log.push(Note::new("edit", "Placed -K1.".into()).offering([connect("a")]));
        let second = log.push(Note::new("edit", "Placed -K2.".into()).offering([connect("b")]));
        let steps = |log: &NotificationLog, s: u64| match log.get(s).and_then(|n| n.offer("connect")) {
            Some(Offer {
                run: Run::Steps {
                    steps, once: true, ..
                },
                spent,
                ..
            }) => Some((steps[0].1["params"]["wire"].clone(), *spent)),
            _ => None,
        };
        log.spend(first, "connect");
        assert_eq!(steps(&log, first), Some((serde_json::json!("a"), true)));
        assert_eq!(steps(&log, second), Some((serde_json::json!("b"), false)));
        let n = log.get(second).expect("the toast");
        assert_eq!((n.action(), n.actions()), (Some("connect"), vec!["connect"]));
        assert_eq!(n.label("connect"), strings::text(key!("note-action-connect")));
        assert_eq!(n.label("undo"), "");
    }
}
