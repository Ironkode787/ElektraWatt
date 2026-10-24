// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The shell's clipboard (WP-S2.05 part 3, spec §8.5 to §8.8): one text value, what it holds, and the text the window
//! puts on the system clipboard.
//!
//! **Headless** runs have no system clipboard: the shell keeps the text (§8.7). `clipboard.text` sets it as an
//! external copy would; `edit.copy`, `edit.cut` and `edit.copy_tags` write it; `edit.paste_clip` reads it. **In the
//! window** the same actions run: a copy also leaves its text in [`ClipboardState::take_outbox`], which the window
//! hands to egui's `Context::copy_text`, and a paste event's text arrives as `edit.paste_clip`'s `text`.
//!
//! **Record IDs.** A fragment is data, not machine text: the clipboard keeps it with its real UUIDs (another instance
//! or a later session pastes it), `clipboard.text` takes it as given (its `text` is not an alias parameter), and
//! neither the transcript's `clipboard` member nor the describe line prints it, only its kind and counts.

use crate::strings;

/// What the clipboard holds (the transcript's `clipboard` member).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ClipKind {
    /// A project fragment (`#ew-fragment v1`), with its record and library-copy line counts.
    Fragment {
        /// The record lines.
        records: usize,
        /// The library-copy lines.
        copies: usize,
    },
    /// Any other text, with its line count.
    Text {
        /// The lines.
        lines: usize,
    },
}

/// The shell's clipboard.
#[derive(Clone, Debug, Default)]
pub struct ClipboardState {
    /// The text, `None` while empty.
    text: Option<String>,
    /// What the text holds.
    kind: Option<ClipKind>,
    /// The connections the copy that wrote the fragment dropped (0 for a text from elsewhere).
    dropped: usize,
    /// The text the window puts on the system clipboard at the end of the frame.
    outbox: Option<String>,
}

/// What `text` holds, without parsing it (the transcript reads this every step, so it costs no `parse_fragment` call):
/// a fragment when its first line is the fragment header, its records the lines after the header line that are not
/// `$lib` or `$node` lines; else plain text with its line count.
pub fn classify(text: &str) -> ClipKind {
    let mut lines = text.lines();
    if lines.next() == Some(ew_api::output::FRAGMENT_HEADER) {
        let body: Vec<&str> = lines.skip(1).filter(|l| !l.trim().is_empty()).collect();
        let copies = body.iter().filter(|l| l.starts_with("{\"$lib\"")).count();
        let nodes = body.iter().filter(|l| l.starts_with("{\"$node\"")).count();
        ClipKind::Fragment {
            records: body.len() - copies - nodes,
            copies,
        }
    } else {
        ClipKind::Text {
            lines: text.lines().count(),
        }
    }
}

impl ClipboardState {
    /// The text, `None` while empty.
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    /// What the clipboard holds, `None` while empty.
    pub fn kind(&self) -> Option<&ClipKind> {
        self.kind.as_ref()
    }

    /// The connections the copy of the fragment dropped.
    pub fn dropped(&self) -> usize {
        self.dropped
    }

    /// Sets the text: `kind` when the caller knows it (a copy), else classified; `to_system` when the window should put
    /// it on the system clipboard.
    pub(crate) fn set(&mut self, text: String, kind: Option<ClipKind>, dropped: usize, to_system: bool) {
        self.kind = Some(kind.unwrap_or_else(|| classify(&text)));
        self.dropped = dropped;
        if to_system {
            self.outbox = Some(text.clone());
        }
        self.text = Some(text);
    }

    /// The text the window puts on the system clipboard, once.
    pub fn take_outbox(&mut self) -> Option<String> {
        self.outbox.take()
    }

    /// The describe line (§8.8): `clipboard fragment records <n> copies <n>`, `clipboard text lines <n>` or
    /// `clipboard none`.
    pub fn describe_line(&self) -> String {
        match &self.kind {
            None => "clipboard none".to_owned(),
            Some(ClipKind::Fragment { records, copies }) => {
                format!("clipboard fragment records {records} copies {copies}")
            }
            Some(ClipKind::Text { lines }) => format!("clipboard text lines {lines}"),
        }
    }
}

/// The reason key of a copy refusal.
pub fn copy_refusal(e: ew_api::output::CopyError) -> &'static str {
    match e {
        ew_api::output::CopyError::Empty => "reason-nothing-selected",
        ew_api::output::CopyError::NotOnePage => "reason-copy-pages",
        ew_api::output::CopyError::Generated => "reason-copy-generated",
    }
}

/// The banner text of a fragment that does not read (other than plain text, which is refused quietly).
pub fn fragment_banner(e: &ew_api::output::FragmentError) -> String {
    strings::format("banner-paste-refused", &[&e.to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_counts_records_and_copies_and_text_lines() {
        let f = "#ew-fragment v1\n{\"$fragment\":{}}\n{\"$k\":\"device\"}\n{\"$k\":\"function\"}\n{\"$lib\":{}}\n{\"$node\":{}}\n";
        assert_eq!(
            classify(f),
            ClipKind::Fragment {
                records: 2,
                copies: 1
            }
        );
        assert_eq!(classify("-101F1\nF55\nS51\n"), ClipKind::Text { lines: 3 });
        assert_eq!(classify(""), ClipKind::Text { lines: 0 });
    }

    #[test]
    fn the_outbox_is_taken_once() {
        let mut c = ClipboardState::default();
        c.set("a".into(), None, 0, true);
        assert_eq!(c.take_outbox().as_deref(), Some("a"));
        assert_eq!(c.take_outbox(), None);
        assert_eq!(c.text(), Some("a"));
        assert_eq!(c.describe_line(), "clipboard text lines 1");
    }
}
