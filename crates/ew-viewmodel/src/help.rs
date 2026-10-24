// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Documentation anchors and the context of F1 (WP-S2.01 part 2, FT-311; UX/UI doc 16 section 13, FR-UI-031).
//!
//! Every dialog, screen and registry action with a page in the user guide has a **topic**: a stable name that
//! maps to a page of the book (`docs/book/src`) and a heading of that page. F1 resolves the context in the order
//! of doc 16 section 13 as far as the shell has the contexts: an open wizard, first-run flow, palette or dialog,
//! then the project view or the start page, otherwise the documentation home. A tooltip of a menu item that has a
//! topic ends with "F1: learn more"; F1 over a hovered item opens that item's topic (the `topic` parameter of
//! `help.documentation`).
//!
//! The anchors are checked against the book by the tests of this module: the page exists, it is in `SUMMARY.md`
//! and has a heading whose id is the fragment (NFR-DOC-01).

use crate::shell::{Dialog, ShellModel};

/// One documentation anchor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Anchor {
    /// The stable topic name.
    pub topic: &'static str,
    /// The page of the book as a path below `docs/book/src`.
    pub page: &'static str,
    /// The heading id on the page (mdBook lower-cases a heading and joins its words with `-`); empty for the
    /// top of the page.
    pub fragment: &'static str,
}

const WINDOW: &str = "how-to/application-window.md";

/// The anchors, in no particular order; `topic` is unique.
pub static ANCHORS: &[Anchor] = &[
    Anchor {
        topic: "home",
        page: "introduction.md",
        fragment: "",
    },
    Anchor {
        topic: "start-page",
        page: WINDOW,
        fragment: "start-page",
    },
    Anchor {
        topic: "project-view",
        page: WINDOW,
        fragment: "project-view",
    },
    Anchor {
        topic: "new-project",
        page: WINDOW,
        fragment: "new-project",
    },
    Anchor {
        topic: "sample-project",
        page: WINDOW,
        fragment: "sample-project",
    },
    Anchor {
        topic: "import-qet",
        page: WINDOW,
        fragment: "import-a-qelectrotech-project",
    },
    Anchor {
        topic: "recovery",
        page: WINDOW,
        fragment: "unsaved-changes-and-recovery",
    },
    Anchor {
        topic: "unsaved-changes",
        page: WINDOW,
        fragment: "unsaved-changes-and-recovery",
    },
    Anchor {
        topic: "command-palette",
        page: WINDOW,
        fragment: "command-palette",
    },
    Anchor {
        topic: "keyboard-shortcuts",
        page: WINDOW,
        fragment: "keyboard-shortcuts",
    },
    Anchor {
        topic: "workspaces",
        page: WINDOW,
        fragment: "workspaces",
    },
    Anchor {
        topic: "first-run",
        page: WINDOW,
        fragment: "first-run-setup",
    },
    Anchor {
        topic: "company-standard-wizard",
        page: WINDOW,
        fragment: "company-standard-wizard",
    },
    Anchor {
        topic: "tutorials",
        page: "tutorials/index.md",
        fragment: "",
    },
    Anchor {
        topic: "commands",
        page: "reference/commands/index.md",
        fragment: "",
    },
    Anchor {
        topic: "about",
        page: WINDOW,
        fragment: "about",
    },
];

/// The anchor of a topic.
pub fn anchor(topic: &str) -> Option<&'static Anchor> {
    ANCHORS.iter().find(|a| a.topic == topic)
}

/// The topic of a registry action, for its tooltip and for F1 over its menu item; `None` for an action without a
/// page of its own.
pub fn action_topic(action: &str) -> Option<&'static str> {
    Some(match action {
        "file.new" => "new-project",
        "file.open_sample" => "sample-project",
        "file.import_qet" => "import-qet",
        "file.discard_journal" => "recovery",
        "tools.palette" => "command-palette",
        "tools.company_standard" => "company-standard-wizard",
        "help.shortcuts" => "keyboard-shortcuts",
        "help.first_run" => "first-run",
        "help.tutorials" => "tutorials",
        "help.about" => "about",
        "command.run" => "commands",
        a if a.starts_with("view.workspace") => "workspaces",
        _ => return None,
    })
}

/// The topic of the context the shell is in (F1 without a hovered item).
pub fn context_topic(model: &ShellModel) -> &'static str {
    if model.wizard().is_some() {
        return "company-standard-wizard";
    }
    if model.first_run().is_some() {
        return "first-run";
    }
    if model.palette().is_some() {
        return "command-palette";
    }
    match model.dialog() {
        Some(Dialog::About | Dialog::Notices) => return "about",
        Some(Dialog::Shortcuts) => return "keyboard-shortcuts",
        Some(Dialog::ConfirmDiscard { .. }) => return "unsaved-changes",
        Some(Dialog::ConfirmJournalDiscard) => return "recovery",
        Some(Dialog::ConfirmCheckScheme { .. }) => return "company-standard-wizard",
        Some(Dialog::SaveWorkspace { .. }) => return "workspaces",
        Some(Dialog::GotoPage { .. } | Dialog::Form(_)) | None => {}
    }
    if model.has_project() {
        "project-view"
    } else {
        "start-page"
    }
}

/// The page of the built book for an anchor, relative to the book's output directory: `how-to/x.html#fragment`.
pub fn html_target(anchor: &Anchor) -> String {
    let page = anchor.page.strip_suffix(".md").unwrap_or(anchor.page);
    if anchor.fragment.is_empty() {
        format!("{page}.html")
    } else {
        format!("{page}.html#{}", anchor.fragment)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    const BOOK_SRC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/book/src");

    /// The mdBook id of a heading line: lower case, words joined by `-`, punctuation dropped.
    fn heading_id(text: &str) -> String {
        let mut out = String::new();
        for c in text.trim().chars() {
            if c.is_alphanumeric() {
                out.extend(c.to_lowercase());
            } else if (c == ' ' || c == '-') && !out.ends_with('-') {
                out.push('-');
            }
        }
        out.trim_end_matches('-').to_owned()
    }

    #[test]
    fn topics_are_unique() {
        for (i, a) in ANCHORS.iter().enumerate() {
            assert!(ANCHORS[i + 1..].iter().all(|b| b.topic != a.topic), "{}", a.topic);
        }
    }

    #[test]
    fn every_anchor_resolves_in_the_book() {
        let summary = std::fs::read_to_string(Path::new(BOOK_SRC).join("SUMMARY.md")).expect("SUMMARY.md");
        for a in ANCHORS {
            let path = Path::new(BOOK_SRC).join(a.page);
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", a.page));
            if a.page != "introduction.md" {
                assert!(summary.contains(a.page), "{} is not in SUMMARY.md", a.page);
            }
            if !a.fragment.is_empty() {
                let found = text
                    .lines()
                    .filter_map(|l| l.strip_prefix('#'))
                    .map(|l| heading_id(l.trim_start_matches('#')))
                    .any(|id| id == a.fragment);
                assert!(found, "{}: no heading with the id `{}`", a.page, a.fragment);
            }
        }
    }

    #[test]
    fn action_topics_name_anchors() {
        for meta in crate::actions::ACTIONS {
            if let Some(topic) = action_topic(meta.id) {
                assert!(anchor(topic).is_some(), "{} -> {topic}", meta.id);
            }
        }
    }

    #[test]
    fn html_targets_swap_the_extension() {
        let a = anchor("start-page").unwrap();
        assert_eq!(html_target(a), "how-to/application-window.html#start-page");
        assert_eq!(html_target(anchor("home").unwrap()), "introduction.html");
    }
}
