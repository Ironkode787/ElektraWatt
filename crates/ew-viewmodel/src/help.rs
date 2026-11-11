// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Documentation anchors and the context of F1 (UX/UI doc 16 section 13, FR-UI-031).
//!
//! Every dialog, screen and registry action with a page in the user guide has a **topic**: a stable name that
//! maps to a page of the book (`docs/book/src`) and a heading of that page. F1 resolves the context in the order
//! of doc 16 section 13 as far as the shell has the contexts: an open wizard, first-run flow, palette or dialog, then
//! the focused pane (a navigator, the strip editor, a table, the property grid, the find pane, the history, the
//! command bar, or the canvas's active tool), then the project view or the start page, otherwise the documentation
//! home. Every tool action, dialog, pane and navigator has a topic on the window reference pages. A tooltip of a menu item that has a
//! topic ends with "F1: learn more"; F1 over a hovered item opens that item's topic (the `topic` parameter of
//! `help.documentation`).
//!
//! The anchors are checked against the book by the tests of this module: the page exists, it is in `SUMMARY.md`
//! and has a heading whose id is the fragment (NFR-DOC-01). The reference pages are built, not committed, so an
//! anchor into them is checked against the contract file the page is built from (the command index, the rule
//! catalogue), with the page and anchor rule of `cargo xtask docs-gen`.

use crate::layout::PaneKind;
use crate::navigators::NavKind;
use crate::shell::{Dialog, Focus, ShellModel};

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
    on(
        "reference/symbol-editor.md",
        "symbol-editor",
        "symbol-editor-and-automated-authoring",
    ),
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
    // A topic for every tool action, every dialog without one above, every pane and navigator kind.
    on(TOOLS, "tool-back", "tool-step-back"),
    on(TOOLS, "tool-select", "select"),
    on(TOOLS, "tool-measure", "measure"),
    on(TOOLS, "tool-place", "symbol"),
    on(TOOLS, "tool-place-part", "device-from-part"),
    on(TOOLS, "tool-rotate-cw", "rotate-clockwise"),
    on(TOOLS, "tool-rotate-ccw", "rotate-counter-clockwise"),
    on(TOOLS, "tool-mirror-x", "mirror-left-to-right"),
    on(TOOLS, "tool-mirror-y", "mirror-top-to-bottom"),
    on(TOOLS, "tool-next-variant", "next-variant"),
    on(TOOLS, "tool-next-rep-type", "next-representation-type"),
    on(TOOLS, "tool-move", "move"),
    on(TOOLS, "tool-finish", "finish"),
    on(TOOLS, "tool-wire", "wire"),
    on(TOOLS, "tool-next-proposal", "next-autoconnect-proposal"),
    on(TOOLS, "tool-place-layout", "place-device"),
    on(TOOLS, "tool-rail-draw", "rail"),
    on(TOOLS, "tool-duct-draw", "duct"),
    on(TOOLS, "tool-connect-on-move", "connect-while-moving"),
    on(TOOLS, "tool-route-edit", "edit-route"),
    // The two route actions have their sections on the tools page.
    on(TOOLS, "route-page", "route-page"),
    on(TOOLS, "route-selection", "route-selection"),
    on(TOOLS, "tool-ip", "interruption-point"),
    on(TOOLS, "tool-black-box", "black-box"),
    on(TOOLS, "tool-ip-direction", "interruption-point-direction"),
    on(TOOLS, "tool-star-source", "star-source"),
    on(TOOLS, "tool-text", "text"),
    on(TOOLS, "tool-graphic", "graphic"),
    on(TOOLS, "tool-marker", "wire-marker"),
    on(TOOLS, "tool-structure-box", "structure-box"),
    on(TOOLS, "tool-image", "image"),
    on(DIALOGS, "dialog-goto-page", "go-to-page-or-cell"),
    on(DIALOGS, "dialog-form", "form-dialogs"),
    on(DIALOGS, "dialog-purge", "purge-unused-images"),
    on(DIALOGS, "dialog-part-choose", "choose-part"),
    on(DIALOGS, "dialog-part-edit", "edit-part"),
    on(DIALOGS, "dialog-print", "print"),
    on(DIALOGS, "dialog-external-change", "changes-on-disk"),
    on(DIALOGS, "dialog-upgrade", "upgrade-the-format"),
    on(DIALOGS, "dialog-reimport-match", "re-import-a-matched-file"),
    on(DIALOGS, "dialog-reimport", "re-import-qelectrotech-project"),
    on(DIALOGS, "dialog-open-ewz", "open-ewz"),
    on(DIALOGS, "dialog-git-setup", "set-up-merge-driver"),
    on(PANELS, "pane-page-view", "page-view"),
    on(PANELS, "pane-properties", "properties"),
    on(PANELS, "pane-find", "find"),
    on(PANELS, "pane-jobs", "jobs"),
    on(PANELS, "pane-preview", "preview"),
    on(PANELS, "pane-history", "history"),
    on(PANELS, "pane-glossary", "glossary"),
    on(PANELS, "command-bar", "command-bar"),
    on(PANELS, "strip-editor", "strip-editor"),
    on(PANELS, "table-mode", "table-mode"),
    on(PANELS, "nav-pages", "pages"),
    on(PANELS, "nav-devices", "devices"),
    on(PANELS, "nav-connections", "connections"),
    on(PANELS, "nav-strips", "terminal-strips"),
    on(PANELS, "nav-cables", "cables"),
    on(PANELS, "nav-plc", "plc"),
    on(PANELS, "nav-potentials", "potentials"),
    on(PANELS, "nav-ips", "interruption-points"),
    on(PANELS, "nav-structure", "structure-identifiers"),
    on(PANELS, "nav-parts", "parts-in-use"),
    on(PANELS, "nav-blocks", "report-blocks"),
    on(PANELS, "nav-unplaced", "unplaced-devices"),
    on(PANELS, "nav-macros", "macros"),
    on(PANELS, "nav-libraries", "libraries"),
    on(PANELS, "nav-messages", "messages"),
];

/// The window reference pages: the tools, the dialogs, the panes and navigators.
const TOOLS: &str = "reference/window/tools.md";
const DIALOGS: &str = "reference/window/dialogs.md";
const PANELS: &str = "reference/window/panels.md";

/// An anchor on `page`.
const fn on(page: &'static str, topic: &'static str, fragment: &'static str) -> Anchor {
    Anchor {
        topic,
        page,
        fragment,
    }
}

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
        // The panes, the strip editor and the table mode open from these.
        "props.show" | "props.focus" => "pane-properties",
        "find.open" | "find.open_replace" => "pane-find",
        "jobs.show" => "pane-jobs",
        "preview.show" => "pane-preview",
        "history.show" => "pane-history",
        "help.glossary" => "pane-glossary",
        "messages.show" => "nav-messages",
        "strip.edit" => "strip-editor",
        "navigator.table" => "table-mode",
        "route.page" => "route-page",
        "route.selection" => "route-selection",
        a if a.starts_with("symbol.") => "symbol-editor",
        a if a.starts_with("tool.") => return tool_topic(a),
        a if a.starts_with("view.workspace") => "workspaces",
        // Every command of the command bar has a page; until the book has a command-bar
        // section the project view's page explains the window it sits in.
        a if crate::actions::find(a).is_some_and(|m| m.placement.is_some()) => "project-view",
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
    if let Some(dialog) = model.dialog() {
        return dialog_topic(dialog);
    }
    if model.symbol_editor().is_some() {
        return "symbol-editor";
    }
    if let Some(topic) = focus_topic(model) {
        return topic;
    }
    if model.has_project() {
        "project-view"
    } else {
        "start-page"
    }
}

/// The topic of a tool action (`tool.place` → `tool-place`); `None` for one without an anchor.
fn tool_topic(action: &str) -> Option<&'static str> {
    let name = action.strip_prefix("tool.")?.replace('_', "-");
    anchor(&format!("tool-{name}")).map(|a| a.topic)
}

/// The topic of an open dialog. The match names every variant, so a new dialog does not compile until it
/// has its topic.
pub fn dialog_topic(dialog: &Dialog) -> &'static str {
    match dialog {
        Dialog::About | Dialog::Notices => "about",
        Dialog::Shortcuts => "keyboard-shortcuts",
        Dialog::ConfirmDiscard { .. } => "unsaved-changes",
        Dialog::ConfirmJournalDiscard | Dialog::Recovery { .. } => "recovery",
        Dialog::ConfirmCheckScheme { .. } => "company-standard-wizard",
        Dialog::SaveWorkspace { .. } => "workspaces",
        Dialog::GotoPage { .. } => "dialog-goto-page",
        Dialog::Form(_) => "dialog-form",
        Dialog::ConfirmPurge { .. } => "dialog-purge",
        Dialog::PartChoose(_) => "dialog-part-choose",
        Dialog::PartEdit(_) => "dialog-part-edit",
        Dialog::Print(_) => "dialog-print",
        Dialog::ExternalChange { .. } => "dialog-external-change",
        Dialog::ConfirmUpgrade { .. } => "dialog-upgrade",
        Dialog::ImportQet { .. } => "import-qet",
        Dialog::ReimportMatch { .. } => "dialog-reimport-match",
        Dialog::Reimport { .. } => "dialog-reimport",
        Dialog::OpenEwz { .. } => "dialog-open-ewz",
        Dialog::ConfirmGitSetup { .. } => "dialog-git-setup",
    }
}

/// The topic of a navigator kind.
pub fn nav_topic(kind: NavKind) -> &'static str {
    match kind {
        NavKind::Pages => "nav-pages",
        NavKind::Devices => "nav-devices",
        NavKind::Connections => "nav-connections",
        NavKind::Strips => "nav-strips",
        NavKind::Cables => "nav-cables",
        NavKind::Plc => "nav-plc",
        NavKind::Potentials => "nav-potentials",
        NavKind::Ips => "nav-ips",
        NavKind::Structure => "nav-structure",
        NavKind::Parts => "nav-parts",
        NavKind::Blocks => "nav-blocks",
        NavKind::Unplaced => "nav-unplaced",
        NavKind::Macros => "nav-macros",
        NavKind::Libraries => "nav-libraries",
        NavKind::Messages => "nav-messages",
    }
}

/// The topic of a pane kind.
pub fn pane_topic(pane: PaneKind) -> &'static str {
    match pane {
        PaneKind::Pages => nav_topic(NavKind::Pages),
        PaneKind::PageView => "pane-page-view",
        PaneKind::Properties => "pane-properties",
        PaneKind::Find => "pane-find",
        PaneKind::Navigator(kind) => nav_topic(kind),
        PaneKind::Jobs => "pane-jobs",
        PaneKind::Preview => "pane-preview",
        PaneKind::History => "pane-history",
        PaneKind::Glossary => "pane-glossary",
    }
}

/// The topic of the focused pane: a navigator, the strip editor or a navigator's table mode, the property
/// grid, the find pane, the history, the command bar, or the canvas's active tool other than Select.
fn focus_topic(model: &ShellModel) -> Option<&'static str> {
    Some(match model.focus()? {
        Focus::Navigator(kind) if model.nav(kind).is_some_and(|n| n.table_mode().is_some()) => {
            if kind == NavKind::Strips {
                "strip-editor"
            } else {
                "table-mode"
            }
        }
        Focus::Navigator(kind) => nav_topic(kind),
        Focus::Properties => pane_topic(PaneKind::Properties),
        Focus::Find => pane_topic(PaneKind::Find),
        Focus::History => pane_topic(PaneKind::History),
        Focus::CommandBar => "command-bar",
        Focus::Canvas(_) => match crate::command_bar::tool_action(model.tool().name()) {
            "tool.select" => return None,
            action => return tool_topic(action),
        },
    })
}

/// The page of a shipped rule in the book (§7.3): the rule's declared `docs` reference as a page,
/// `reference/rules/<id in lower case>.md` (`reference/rules/ew-core-033.md`); `None` for a rule this build does not
/// ship.
pub fn rule_page(rule: &str) -> Option<String> {
    Some(format!("{}.md", rule_meta(rule)?.docs))
}

/// The page of the built book for a shipped rule (F1 on a finding or rule row of the Messages pane):
/// `reference/rules/ew-core-033.html`.
pub fn rule_target(rule: &str) -> Option<String> {
    Some(format!("{}.html", rule_meta(rule)?.docs))
}

/// The data of a shipped rule: a registered rule, or an integrity rule of opening a project, whose findings reach the
/// Messages pane too.
fn rule_meta(rule: &str) -> Option<&'static ew_api::checks::RuleMeta> {
    ew_api::checks::rule(rule).map(|r| r.meta()).or_else(|| {
        ew_api::checks::rules::core::INTEGRITY
            .iter()
            .find(|m| m.id == rule)
    })
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
    /// The built directory of the command pages.
    const COMMANDS_DIR: &str = "reference/commands/";

    /// The built page of a command, below `docs/book/src`.
    fn command_page(name: &str) -> String {
        format!("{COMMANDS_DIR}{name}.md")
    }

    /// The names of the registered commands, each of which gets a built page.
    fn command_names() -> Vec<String> {
        ew_api::CommandRegistry::builtin()
            .names()
            .map(str::to_owned)
            .collect()
    }

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
        let commands = command_names();
        assert!(!commands.is_empty());
        for a in ANCHORS {
            if a.page != "introduction.md" {
                assert!(summary.contains(a.page), "{} is not in SUMMARY.md", a.page);
            }
            // A built command page: the index always, a command's page when the index lists the command; neither
            // has a heading an anchor names.
            if a.page.starts_with(COMMANDS_DIR) {
                let index = format!("{COMMANDS_DIR}index.md");
                assert!(
                    a.page == index || commands.iter().any(|c| command_page(c) == a.page),
                    "{}: no command of the index has this page",
                    a.page
                );
                assert_eq!(a.fragment, "", "{}", a.page);
                continue;
            }
            let path = Path::new(BOOK_SRC).join(a.page);
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", a.page));
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

    /// F1 on Route page and Route selection opens their sections of the tools page, not the
    /// project view's page.
    #[test]
    fn route_page_and_route_selection_open_their_sections() {
        for (action, fragment) in [
            ("route.page", "route-page"),
            ("route.selection", "route-selection"),
        ] {
            let a = action_topic(action).and_then(anchor).expect(action);
            assert_eq!((a.page, a.fragment), (TOOLS, fragment), "{action}");
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

    /// Every shipped rule, the integrity rules of opening a project included, has a page under the built rule
    /// reference, named after its ID.
    #[test]
    fn every_shipped_rule_has_its_page() {
        let rules = ew_api::checks::rules();
        assert!(!rules.is_empty());
        let integrity = ew_api::checks::rules::core::INTEGRITY;
        for meta in rules.iter().map(|r| r.meta()).chain(integrity) {
            let page = rule_page(meta.id).expect("a shipped rule");
            assert_eq!(page, format!("reference/rules/{}.md", meta.id.to_lowercase()));
        }
        assert_eq!(
            rule_page("EW-CORE-033").as_deref(),
            Some("reference/rules/ew-core-033.md")
        );
        assert_eq!(
            rule_target("EW-CORE-033").as_deref(),
            Some("reference/rules/ew-core-033.html")
        );
        assert_eq!(
            rule_target("EW-CORE-005").as_deref(),
            Some("reference/rules/ew-core-005.html")
        );
        assert_eq!(rule_page("EW-NONE-001"), None);
    }

    /// Every tool action, every pane kind and every navigator kind has a topic whose anchor is in
    /// the book (`every_anchor_resolves_in_the_book` checks the anchors), and so do the dialogs a test can build.
    #[test]
    fn every_tool_pane_navigator_and_dialog_has_a_topic() {
        let tools: Vec<&str> = crate::actions::ACTIONS
            .iter()
            .map(|a| a.id)
            .filter(|id| id.starts_with("tool."))
            .collect();
        assert!(!tools.is_empty());
        for id in tools {
            let topic = action_topic(id).unwrap_or_else(|| panic!("{id} has no topic"));
            assert_eq!(anchor(topic).map(|a| a.page), Some(TOOLS), "{id}");
        }
        for kind in NavKind::ALL {
            assert_eq!(anchor(nav_topic(kind)).map(|a| a.page), Some(PANELS), "{kind:?}");
            assert_eq!(pane_topic(PaneKind::Navigator(kind)), nav_topic(kind));
        }
        for pane in PaneKind::ALL {
            assert!(anchor(pane_topic(pane)).is_some(), "{pane:?}");
        }
        for topic in ["strip-editor", "table-mode", "command-bar"] {
            assert_eq!(anchor(topic).map(|a| a.page), Some(PANELS), "{topic}");
        }
        let text = String::new;
        for dialog in [
            Dialog::About,
            Dialog::Shortcuts,
            Dialog::GotoPage { text: text() },
            Dialog::Notices,
            Dialog::ConfirmJournalDiscard,
            Dialog::ConfirmCheckScheme { scheme: text() },
            Dialog::SaveWorkspace { text: text() },
            Dialog::ConfirmPurge {
                files: Vec::new(),
                assets: Vec::new(),
            },
            Dialog::ExternalChange { rows: Vec::new() },
            Dialog::ConfirmUpgrade { from: text() },
            Dialog::ImportQet {
                file: text(),
                folder: text(),
                summary: text(),
                pages: Vec::new(),
                devices: 0,
                confirm: false,
            },
            Dialog::ReimportMatch {
                path: text(),
                file: text(),
                source: text(),
                percent: 0,
            },
            Dialog::Reimport {
                file: text(),
                summary: text(),
                rows: Vec::new(),
                nothing: false,
            },
            Dialog::OpenEwz {
                path: text(),
                name: text(),
            },
            Dialog::ConfirmGitSetup { folder: text() },
        ] {
            assert!(anchor(dialog_topic(&dialog)).is_some(), "{dialog:?}");
        }
        for topic in [
            "dialog-form",
            "dialog-part-choose",
            "dialog-part-edit",
            "dialog-print",
        ] {
            assert_eq!(anchor(topic).map(|a| a.page), Some(DIALOGS), "{topic}");
        }
    }

    #[test]
    fn html_targets_swap_the_extension() {
        let a = anchor("start-page").unwrap();
        assert_eq!(html_target(a), "how-to/application-window.html#start-page");
        assert_eq!(html_target(anchor("home").unwrap()), "introduction.html");
    }
}
