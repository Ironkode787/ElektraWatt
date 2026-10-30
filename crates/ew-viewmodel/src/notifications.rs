// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The notifications (WP-S2.09 part 1, D6, §6.5; FR-UI-042, NFR-A11Y-04): the toasts on screen and the log of the
//! session's last notifications, with the actions that stay usable while they are valid.
//!
//! Every toast enters the log (`NotificationLog::push`): at most [`MAX_VISIBLE`] toasts are visible (a toast of a
//! key that is visible replaces that one; a fourth pushes out the oldest), and the log keeps the last [`MAX_LOG`]
//! (the oldest dropped). A toast's timeout (8 s) is the window's clock, never the model's: headless, a toast stays
//! until it is dismissed, replaced or pushed out. An entry's action is valid while what it acts on still holds: Undo
//! while the step it undoes is the next undo step of the same project session, Open while the file the job wrote in
//! this session is known. The shell judges the validity ([`crate::shell::ShellModel::notification_valid`]); this
//! module keeps the entries. Nothing here reads a clock, a path or a `Session`.

use std::collections::VecDeque;
use std::path::PathBuf;

/// The most toasts on screen at once.
pub const MAX_VISIBLE: usize = 3;

/// The most entries the log keeps.
pub const MAX_LOG: usize = 100;

/// The action a notification offers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoteAction {
    /// Undo the step the notification reports.
    Undo,
    /// Open the file the job wrote (Open PDF).
    Open,
}

impl NoteAction {
    /// The word in the describe text and in `notifications.run`'s `action`.
    pub fn word(self) -> &'static str {
        match self {
            NoteAction::Undo => "undo",
            NoteAction::Open => "open",
        }
    }

    /// The action of a word.
    pub fn from_word(word: &str) -> Option<NoteAction> {
        match word {
            "undo" => Some(NoteAction::Undo),
            "open" => Some(NoteAction::Open),
            _ => None,
        }
    }

    /// The key of the button's label in [`crate::strings`].
    pub fn label_key(self) -> &'static str {
        match self {
            NoteAction::Undo => "toast-undo",
            NoteAction::Open => "toast-open",
        }
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
    /// Open: the file the job wrote in this session.
    Open {
        /// The written file (never printed).
        path: PathBuf,
    },
}

/// One notification: a toast while visible, an entry of the log always.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Notification {
    /// Counted from 1 per shell.
    pub serial: u64,
    /// The text (no path, no time, no UUID).
    pub text: String,
    /// The key: a toast of a key that is visible replaces that one (`edit`, `export`, `recovery`).
    pub key: &'static str,
    pub(crate) action: Option<(NoteAction, Validity)>,
    /// The tone a result set (the check's worst severity); `None` for every other notification.
    pub(crate) tone: Option<NoteTone>,
}

impl Notification {
    /// The action it offers, valid or not.
    pub fn action(&self) -> Option<NoteAction> {
        self.action.as_ref().map(|(a, _)| *a)
    }

    /// The tone of its icon: the tone the result set, else success for a toast with an action, else info.
    pub fn tone(&self) -> NoteTone {
        match (self.tone, &self.action) {
            (Some(tone), _) => tone,
            (None, Some(_)) => NoteTone::Success,
            (None, None) => NoteTone::Info,
        }
    }

    /// The title and the optional line of the toast (doc 16a §6.10): a toned result's text splits at its first `": "`
    /// into the title ("Check") and the line of counts ("4 errors, 25 warnings"); any other text is the title alone.
    pub fn lines(&self) -> (&str, Option<&str>) {
        if self.tone.is_some()
            && let Some((title, line)) = self.text.split_once(": ")
        {
            return (title, Some(line));
        }
        (&self.text, None)
    }

    /// The file an Open action opens.
    pub(crate) fn open_path(&self) -> Option<&PathBuf> {
        match &self.action {
            Some((_, Validity::Open { path })) => Some(path),
            _ => None,
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
    /// Adds a notification as the newest toast and log entry; returns its serial.
    pub(crate) fn push(
        &mut self,
        text: String,
        key: &'static str,
        action: Option<(NoteAction, Validity)>,
    ) -> u64 {
        self.push_toned(text, key, action, None)
    }

    /// [`Self::push`] with the tone a result sets (the check's worst severity).
    pub(crate) fn push_toned(
        &mut self,
        text: String,
        key: &'static str,
        action: Option<(NoteAction, Validity)>,
        tone: Option<NoteTone>,
    ) -> u64 {
        self.serials += 1;
        let serial = self.serials;
        let entries = &self.entries;
        self.visible.retain(|s| {
            entries
                .iter()
                .find(|n| n.serial == *s)
                .is_none_or(|n| n.key != key)
        });
        self.visible.push(serial);
        if self.visible.len() > MAX_VISIBLE {
            self.visible.remove(0);
        }
        self.entries.push_back(Notification {
            serial,
            text,
            key,
            action,
            tone,
        });
        while self.entries.len() > MAX_LOG {
            if let Some(gone) = self.entries.pop_front() {
                self.visible.retain(|s| *s != gone.serial);
            }
        }
        serial
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
        log.push("a".into(), "edit", None);
        log.push("b".into(), "export", None);
        log.push("c".into(), "recovery", None);
        let texts =
            |log: &NotificationLog| -> Vec<String> { log.visible().iter().map(|n| n.text.clone()).collect() };
        assert_eq!(texts(&log), ["a", "b", "c"]);
        // A toast of a visible key replaces that one; the others keep their order.
        log.push("d".into(), "export", None);
        assert_eq!(texts(&log), ["a", "c", "d"]);
        // A fourth key pushes out the oldest.
        log.push("e".into(), "other", None);
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
            log.push(format!("n{i}"), "edit", None);
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
        let plain = log.push("Placed -K1".into(), "edit", None);
        let open = log.push(
            "Exported PDF".into(),
            "export",
            Some((NoteAction::Open, Validity::Open { path: PathBuf::new() })),
        );
        let checked = log.push_toned(
            "Check: 1 error, 0 warnings".into(),
            "edit",
            None,
            Some(NoteTone::Error),
        );
        let tone = |s| log.get(s).map(Notification::tone);
        assert_eq!(tone(plain), Some(NoteTone::Info));
        assert_eq!(tone(open), Some(NoteTone::Success));
        assert_eq!(tone(checked), Some(NoteTone::Error));
        // A toned result splits into the title and the line of counts; any other text is the title alone.
        assert_eq!(
            log.get(checked).map(Notification::lines),
            Some(("Check", Some("1 error, 0 warnings")))
        );
        assert_eq!(
            log.get(plain).map(Notification::lines),
            Some(("Placed -K1", None))
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
