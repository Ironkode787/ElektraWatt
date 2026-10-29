// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The chooser list (doc 16a §6.22): a popup list of candidates the user picks one from, for a choice too small for
//! a dialog. Its first use is the Place tool's Join chooser (WP-S2.18 D2, J): "New device", then the devices the
//! armed symbol can join. Up and Down move the active row (wrapping), Home and End go to the ends, type-ahead moves it
//! to the first row whose name matches, Enter picks, Escape closes it and leaves everything as it was. Every change
//! is a registry action (`chooser.row`, `chooser.type`, `chooser.choose`, `ui.dismiss`), so the headless mode drives
//! the same model.

/// What a chooser row stands for.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ChooserValue {
    /// The Join chooser's first row: a new device (the target before the join comes back).
    NewDevice,
    /// An existing device (WP-S2.18 D1).
    Join {
        /// The device's ID text.
        device: String,
        /// Its tag as printed.
        tag: String,
        /// The unplaced function the row fills (a slot), `None` for a new auxiliary function.
        function: Option<String>,
    },
}

/// One row of a chooser list.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ChooserRow {
    /// The printed name (`-101K1`, "New device").
    pub name: String,
    /// The kind word, right-aligned (`slot 13-14`, `auxiliary`); may be empty.
    pub kind: String,
    /// What choosing it does.
    pub value: ChooserValue,
}

/// An open chooser list.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Chooser {
    /// The question that names the list (AccessKit `ListBox` name; "Join device").
    pub question: String,
    /// The rows, in order.
    pub rows: Vec<ChooserRow>,
    /// The active row.
    pub row: usize,
    /// The type-ahead text the active row was last moved by.
    pub typed: String,
}

impl Chooser {
    /// A chooser with `rows`, the row `row` active (clamped).
    pub fn new(question: String, rows: Vec<ChooserRow>, row: usize) -> Chooser {
        let row = row.min(rows.len().saturating_sub(1));
        Chooser {
            question,
            rows,
            row,
            typed: String::new(),
        }
    }

    /// Moves the active row by `step` (wrapping) or to `to` (clamped; a negative `to` is the last row).
    pub fn move_row(&mut self, step: Option<i64>, to: Option<i64>) {
        let n = self.rows.len();
        if n == 0 {
            return;
        }
        if let Some(to) = to {
            self.row = if to < 0 {
                n - 1
            } else {
                usize::try_from(to).unwrap_or(usize::MAX).min(n - 1)
            };
        } else if let Some(step) = step {
            let n = i64::try_from(n).unwrap_or(i64::MAX);
            let at = i64::try_from(self.row).unwrap_or(0);
            self.row = usize::try_from((at + step).rem_euclid(n)).unwrap_or(0);
        }
    }

    /// Type-ahead (§6.22): the active row becomes the first row whose name starts with `text`, ignoring case and a
    /// leading `-` (so `k1` and `-101k1` both reach `-101K1`); else the first whose name contains it. No match keeps
    /// the active row. Returns whether a row matched.
    pub fn type_ahead(&mut self, text: &str) -> bool {
        self.typed = text.to_owned();
        let needle = text.trim().to_lowercase();
        if needle.is_empty() {
            return false;
        }
        let bare = needle.trim_start_matches('-').to_owned();
        let names: Vec<String> = self.rows.iter().map(|r| r.name.to_lowercase()).collect();
        let found = names
            .iter()
            .position(|n| n.starts_with(&needle) || n.trim_start_matches('-').starts_with(&bare))
            .or_else(|| names.iter().position(|n| n.contains(&bare)));
        if let Some(i) = found {
            self.row = i;
        }
        found.is_some()
    }

    /// The active row.
    pub fn current(&self) -> Option<&ChooserRow> {
        self.rows.get(self.row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str) -> ChooserRow {
        ChooserRow {
            name: name.to_owned(),
            kind: String::new(),
            value: ChooserValue::NewDevice,
        }
    }

    #[test]
    fn rows_wrap_and_clamp() {
        let mut c = Chooser::new("q".into(), vec![row("a"), row("b"), row("c")], 9);
        assert_eq!(c.row, 2);
        c.move_row(Some(1), None);
        assert_eq!(c.row, 0, "Down on the last row wraps");
        c.move_row(Some(-1), None);
        assert_eq!(c.row, 2, "Up on the first row wraps");
        c.move_row(None, Some(0));
        assert_eq!(c.row, 0);
        c.move_row(None, Some(-1));
        assert_eq!(c.row, 2);
    }

    #[test]
    fn type_ahead_matches_the_tag_with_or_without_its_sign() {
        let mut c = Chooser::new(
            "q".into(),
            vec![row("New device"), row("-101K1"), row("-101K2"), row("-102K1")],
            0,
        );
        assert!(c.type_ahead("-101K2"));
        assert_eq!(c.row, 2);
        assert!(c.type_ahead("102"));
        assert_eq!(c.row, 3);
        assert!(c.type_ahead("k1"), "a substring of the tag");
        assert_eq!(c.row, 1);
        assert!(!c.type_ahead("zz"));
        assert_eq!(c.row, 1, "no match keeps the active row");
    }
}
