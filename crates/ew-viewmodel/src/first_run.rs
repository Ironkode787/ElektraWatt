// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The first-run flow (WP-S2.01 part 2, FT-308; UX/UI doc 16 section 9.1, FR-UI-030): language, what the user
//! comes from, the privacy choice, the appearance, and where to start. Every step can be skipped; Help, First-run
//! setup runs it again. The flow works offline and opens no connection (NFR-PRIV-01).
//!
//! The answers are settings (`language`, `comes_from`, `update_check`, `theme`, `ui_scale`) written as they are
//! given; the flow itself holds only the step. The keymap preset follows the second answer: KiCad users get the
//! KiCad-like preset, everyone else the EW preset (doc 16 section 9.1, step 2).

/// A step of the first-run flow.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum FirstRunStep {
    /// 1 Language.
    Language,
    /// 2 What the user comes from.
    Origin,
    /// 3 Privacy: the update check.
    Privacy,
    /// 4 Appearance: theme and scale.
    Appearance,
    /// 5 Where to start.
    Start,
}

impl FirstRunStep {
    /// Every step in order.
    pub const ALL: [FirstRunStep; 5] = [
        FirstRunStep::Language,
        FirstRunStep::Origin,
        FirstRunStep::Privacy,
        FirstRunStep::Appearance,
        FirstRunStep::Start,
    ];

    /// The 0-based position.
    pub fn index(self) -> usize {
        FirstRunStep::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    /// The step at a 0-based position.
    pub fn from_index(index: usize) -> Option<FirstRunStep> {
        FirstRunStep::ALL.get(index).copied()
    }

    /// The key of the step's title.
    pub fn title_key(self) -> &'static str {
        match self {
            FirstRunStep::Language => "firstrun-step-language",
            FirstRunStep::Origin => "firstrun-step-origin",
            FirstRunStep::Privacy => "firstrun-step-privacy",
            FirstRunStep::Appearance => "firstrun-step-appearance",
            FirstRunStep::Start => "firstrun-step-start",
        }
    }
}

/// The state of the flow while it is open.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FirstRun {
    /// The step on show.
    pub step: FirstRunStep,
}

/// Where the first-run flow can lead (the `start` of `first_run.finish`).
pub const START_CHOICES: [&str; 5] = ["wizard", "sample", "qet", "new", "none"];

/// The action each start choice runs, with its parameters; `None` for "none".
pub fn start_action(choice: &str) -> Option<&'static str> {
    match choice {
        "wizard" => Some("tools.company_standard"),
        "sample" => Some("file.open_sample"),
        "qet" => Some("file.import_qet"),
        "new" => Some("file.new"),
        _ => None,
    }
}

/// The keymap preset that goes with an answer to "I come from".
pub fn keymap_for_origin(origin: &str) -> &'static str {
    if origin == "kicad" { "kicad" } else { "ew" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strings;

    #[test]
    fn steps_have_titles_and_round_trip_their_index() {
        for (i, s) in FirstRunStep::ALL.iter().enumerate() {
            assert_eq!(s.index(), i);
            assert_eq!(FirstRunStep::from_index(i), Some(*s));
            assert!(strings::lookup(s.title_key()).is_some());
        }
        assert_eq!(FirstRunStep::from_index(5), None);
    }

    #[test]
    fn kicad_users_get_the_kicad_preset_and_others_the_ew_preset() {
        assert_eq!(keymap_for_origin("kicad"), "kicad");
        for o in ["eplan", "qet", "other", ""] {
            assert_eq!(keymap_for_origin(o), "ew");
        }
    }

    #[test]
    fn every_start_choice_but_none_runs_an_action() {
        for c in START_CHOICES {
            assert_eq!(start_action(c).is_some(), c != "none", "{c}");
            if let Some(a) = start_action(c) {
                assert!(crate::actions::find(a).is_some(), "{a}");
            }
        }
    }
}
