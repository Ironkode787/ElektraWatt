// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Documentation anchors and F1 (UX/UI doc 16 section 13, FR-UI-031).
//!
//! Every dialog, screen, registry action, kind of thing on a page and row of Properties with a place in the user guide
//! has a **topic**: a stable name that maps to a page of the book (`docs/book/src`) and a heading or anchor of that
//! page ([`anchor`]). F1 never does nothing and answers for the most specific thing under the hand, in this order:
//!
//! 1. what is under the pointer ([`pointer_topic`]): a widget the window names (a button of the command bar, a menu
//!    item, a row of a list or of Properties, a part of a screen; [`HelpUnder`]), else the thing on the page under the
//!    pointer, by what it is ([`Thing`]);
//! 2. else what has the keyboard focus: the cursor row of a list or of Properties, a finding's rule ([`context_topic`]);
//! 3. else the selection's first thing on the page;
//! 4. else the active tool;
//! 5. else the screen: the symbol editor, a station's view, the page's kind, the project view or the start page. An
//!    open dialog, wizard or palette comes first: then only what the window names inside it, else its own topic.
//!
//! A topic with no section of its own falls back to the nearest one (the kind's chapter, a property's kind), never to
//! nothing. A row of Properties opens the properties reference at the row's anchor: the anchor is the key of the text
//! that explains the row ([`property_topic`]), and the reference page is generated from the same texts
//! ([`property_sections`]), so no list is kept by hand.
//!
//! The anchors are checked against the book by the tests of this module: the page exists, it is in `SUMMARY.md`
//! and has a heading whose id is the fragment (NFR-DOC-01), read from the sources alone, so that a stale local build
//! of the book never fails them. The reference pages are built, not committed, so an anchor into them is checked
//! against the source the page is built from (the command index, the rule catalogue, [`property_sections`]).

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::sync::OnceLock;

use crate::canvas::CanvasPage;
use crate::key;
use crate::layout::PaneKind;
use crate::navigators::NavKind;
use crate::properties::LabelPath as _;
use crate::shell::{Dialog, Focus, ShellModel};
use crate::strings;
use crate::tools::EntityKind;

/// One documentation anchor.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Anchor {
    /// The stable topic name.
    pub topic: Cow<'static, str>,
    /// The page of the book as a path below `docs/book/src`.
    pub page: Cow<'static, str>,
    /// The heading id on the page (mdBook lower-cases a heading and joins its words with `-`); empty for the
    /// top of the page.
    pub fragment: Cow<'static, str>,
}

const WINDOW: &str = "how-to/application-window.md";
/// The guide to starting a project and to templates.
const PROJECTS: &str = "how-to/start-a-project.md";
const PLC_GUIDE: &str = "how-to/plc-workspace.md";

/// The anchors, in no particular order; `topic` is unique.
pub static ANCHORS: &[Anchor] = &[
    on(
        "reference/symbol-editor.md",
        "symbol-editor",
        "drawing-a-symbol-and-automated-authoring",
    ),
    on("introduction.md", "home", ""),
    on(WINDOW, "start-page", "start-page"),
    on(WINDOW, "project-view", "project-view"),
    on(PROJECTS, "new-project", "start-a-new-project"),
    on(PROJECTS, "templates", "your-templates"),
    on(PROJECTS, "check-level", "the-check-level"),
    on(PROJECTS, "text-styles", "text-styles"),
    on(WINDOW, "sample-project", "sample-project"),
    on(WINDOW, "import-qet", "import-a-qelectrotech-project"),
    on(WINDOW, "recovery", "unsaved-changes-and-recovery"),
    on(WINDOW, "unsaved-changes", "unsaved-changes-and-recovery"),
    on(WINDOW, "command-palette", "command-palette"),
    on(WINDOW, "keyboard-shortcuts", "keyboard-shortcuts"),
    on(WINDOW, "workspaces", "workspaces"),
    on(WINDOW, "first-run", "first-run-setup"),
    on(
        PROJECTS,
        "company-standard-wizard",
        "make-a-template-for-your-companys-drawings",
    ),
    on("tutorials/index.md", "tutorials", ""),
    // The how-to guides of the disciplines: the PLC, Fluid and Workspace buttons' Guide, F1 on their
    // tabs, navigators and pages, Parts and a panel layout page.
    on(PLC_GUIDE, "guide-plc", ""),
    on(PLC_GUIDE, "guide-bus", "bus-ports-and-nets"),
    on("how-to/fluid-workspace.md", "guide-fluid", ""),
    on("how-to/own-parts.md", "guide-own-parts", ""),
    on("how-to/panel-layout.md", "guide-panel-layout", ""),
    on("reference/commands/index.md", "commands", ""),
    on(WINDOW, "about", "about"),
    // A topic for every tool action, every dialog without one above, every pane and navigator kind.
    // Route and Arrange and route have their sections on the tools page.
    on(TOOLS, "layout-route", "route"),
    on(TOOLS, "layout-arrange", "arrange-and-route"),
    on(TOOLS, "edit-frame", "edit-frame"),
    on(DIALOGS, "dialog-goto-page", "go-to-page-or-cell"),
    on(DIALOGS, "dialog-form", "form-dialogs"),
    on(DIALOGS, "dialog-purge", "purge-unused-images"),
    on(DIALOGS, "dialog-part-choose", "choose-part"),
    on(DIALOGS, "dialog-print", "print"),
    on(DIALOGS, "dialog-external-change", "changes-on-disk"),
    on(DIALOGS, "dialog-upgrade", "upgrade-the-format"),
    on(DIALOGS, "dialog-reimport-match", "re-import-a-matched-file"),
    on(DIALOGS, "dialog-reimport", "re-import-qelectrotech-project"),
    on(DIALOGS, "dialog-open-ewz", "open-ewz"),
    on(DIALOGS, "dialog-import-drawing", "import-a-drawing"),
    on(DIALOGS, "dialog-import-eplan", "import-eplan-data"),
    on(PLC_GUIDE, "dialog-rack", "read-and-change-a-rack"),
    on(DIALOGS, "dialog-plc-import", "plc-import"),
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
    on(PANELS, "nav-tubes", "tube-table"),
    on(PANELS, "nav-media", "medium-table"),
    on(PANELS, "nav-potentials", "potentials"),
    on(PANELS, "nav-ips", "interruption-points"),
    on(PANELS, "nav-structure", "structure-identifiers"),
    on(PANELS, "nav-parts", "parts-in-use"),
    on(PANELS, "nav-blocks", "report-blocks"),
    on(PANELS, "nav-unplaced", "unplaced-devices"),
    on(PANELS, "nav-macros", "macros"),
    on(PANELS, "nav-libraries", "libraries"),
    on(PANELS, "nav-messages", "messages"),
    // The kinds of thing on a page (F1 everywhere): their sections of the page about them.
    on(THINGS, "thing-device", "devices-and-symbols"),
    on(THINGS, "thing-wire", "wires"),
    on(THINGS, "thing-cable", "cables-and-their-marks"),
    on(THINGS, "thing-terminal", "terminals-and-terminal-strips"),
    on(THINGS, "thing-ip", "interruption-points"),
    on(THINGS, "thing-supply", "supply-points"),
    on(THINGS, "thing-plc", "plc-cards-and-stations"),
    on(THINGS, "thing-valve", "valves-and-cylinders"),
    on(THINGS, "thing-tube", "tubes"),
    on(THINGS, "thing-text", "texts"),
    on(THINGS, "thing-graphic", "graphics-and-images"),
    on(THINGS, "thing-box", "boxes"),
    on(THINGS, "thing-frame", "the-frame-and-title-block"),
    on(THINGS, "thing-list", "lists-on-a-page"),
    // A selected wire's handles, a valve terminal, a template made of a project.
    on(TOOLS, "route-by-hand", "change-a-wires-route-by-hand"),
    on(FLUID_GUIDE, "valve-terminals", "valve-terminals"),
    on(PROJECTS, "save-as-template", "make-a-template-of-a-project"),
    // F1 itself, the window's appearance and its notifications, the navigators, the parts guide.
    on(WINDOW, "help-f1", "help-for-what-is-under-the-pointer"),
    on(WINDOW, "appearance", "appearance"),
    on(WINDOW, "notifications", "notifications"),
    on(PANELS, "navigators", "navigators"),
    on("how-to/choose-parts.md", "guide-parts", ""),
    on("how-to/index.md", "guides", ""),
    on(DIALOGS, "revisions", "revisions"),
    // The parts of the symbol editor's screen.
    on(SYMBOL_EDITOR, "symbol-editor-screen", "the-screen"),
    on(SYMBOL_EDITOR, "symbol-editor-selecting", "selecting-and-moving"),
    on(SYMBOL_EDITOR, "symbol-editor-drawing", "drawing"),
    on(SYMBOL_EDITOR, "symbol-editor-device-box", "the-box-of-a-part"),
    // Their own places: the drawing's outline, a shape, a connection point and a pin of a box; a valve terminal's
    // positions.
    on(SYMBOL_EDITOR, "symbol-editor-outline", "the-outline"),
    on(SYMBOL_EDITOR, "symbol-editor-shape", "a-shape"),
    on(SYMBOL_EDITOR, "symbol-editor-cp", "a-connection-point"),
    on(SYMBOL_EDITOR, "symbol-editor-pin", "a-pin-of-the-box"),
    on(
        FLUID_GUIDE,
        "valve-terminal-positions",
        "the-positions-of-a-valve-terminal",
    ),
];

/// The window reference pages: the tools, the dialogs, the panes and navigators, the things on a page.
const TOOLS: &str = "reference/window/tools.md";
const DIALOGS: &str = "reference/window/dialogs.md";
const PANELS: &str = "reference/window/panels.md";
const THINGS: &str = "reference/window/things.md";
/// The command bar's buttons, a section each ([`bar_anchor`]).
const COMMAND_BAR: &str = "reference/window/command-bar.md";
const FLUID_GUIDE: &str = "how-to/fluid-workspace.md";
const SYMBOL_EDITOR: &str = "reference/symbol-editor.md";
/// The properties reference, generated from [`property_sections`] (`cargo xtask docs-gen`).
pub const PROPERTIES: &str = "reference/properties.md";
/// The topic of a row of Properties: this prefix and the row's anchor on [`PROPERTIES`].
const PROPERTY: &str = "property:";
/// The topic of a rule's page: this prefix and the rule ID.
const RULE: &str = "rule:";

/// An anchor on `page`.
const fn on(page: &'static str, topic: &'static str, fragment: &'static str) -> Anchor {
    Anchor {
        topic: Cow::Borrowed(topic),
        page: Cow::Borrowed(page),
        fragment: Cow::Borrowed(fragment),
    }
}

/// The anchor of a topic: a row of [`ANCHORS`], a tool action's own section of the tools page ([`tool_anchor`]), a
/// row of Properties (`property:<anchor>`, [`property_topic`]) or a shipped rule's page (`rule:<ID>`, [`rule_topic`]).
pub fn anchor(topic: &str) -> Option<Anchor> {
    if let Some(fragment) = topic.strip_prefix(PROPERTY) {
        return property_anchors().contains(fragment).then(|| Anchor {
            topic: Cow::Owned(topic.to_owned()),
            page: Cow::Borrowed(PROPERTIES),
            fragment: Cow::Owned(fragment.to_owned()),
        });
    }
    if let Some(rule) = topic.strip_prefix(RULE) {
        return rule_page(rule).map(|page| Anchor {
            topic: Cow::Owned(topic.to_owned()),
            page: Cow::Owned(page),
            fragment: Cow::Borrowed(""),
        });
    }
    ANCHORS
        .iter()
        .find(|a| a.topic == topic)
        .cloned()
        .or_else(|| tool_anchor(topic))
        .or_else(|| bar_anchor(topic))
}

/// The topic of a shipped rule's page (F1 on a finding or a rule of the Messages pane).
pub fn rule_topic(rule: &str) -> String {
    format!("{RULE}{rule}")
}

/// The section of the tools page of the tool action `action` (`tool.place`): the heading that is the action's
/// English label ("Symbol…" is `## Symbol`), so a new tool needs its section in the guide and no row here.
pub fn tool_anchor(action: &str) -> Option<Anchor> {
    let meta = crate::actions::find(action).filter(|m| m.id.starts_with("tool."))?;
    let label = crate::strings::source_text(&meta.label_key())?;
    Some(Anchor {
        topic: Cow::Borrowed(meta.id),
        page: Cow::Borrowed(TOOLS),
        fragment: Cow::Owned(heading_id(label)),
    })
}

/// The mdBook id of a heading: lower case, words joined by `-`, punctuation dropped.
pub fn heading_id(text: &str) -> String {
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

/// The topic of a registry action, for its tooltip and for F1 over its menu item or its button; `None` for an action
/// that no menu and no bar shows (the canvas's, the cursor's, a dialog's own steps). An action with a section of its
/// own opens it; the others open the section of what they act on (a cable's command the cables', a view command the
/// page view's), never the window's pages as a whole where a closer section exists.
pub fn action_topic(action: &str) -> Option<&'static str> {
    // A button of the bar opens its own section of the bar page (Highlight opened the panels page, a PLC
    // button the whole PLC guide), unless it has a section of its own elsewhere already.
    own_topic(action)
        .filter(|t| !t.starts_with("guide-") && *t != "symbol-editor")
        .or_else(|| bar_anchor(action).and_then(|_| bar_topic(action)))
        .or_else(|| own_topic(action))
        .or_else(|| family_topic(action))
}

/// The topic of a button of the bar (an action the bar places, no tool): the action itself, whose anchor is
/// [`bar_anchor`]'s.
fn bar_topic(action: &str) -> Option<&'static str> {
    crate::actions::find(action)
        .filter(|m| m.placement.is_some() && !m.hidden && !m.id.starts_with("tool."))
        .map(|m| m.id)
}

/// The section of the command bar page of the bar's button `action` (`navigator.highlight`): the heading that is the
/// action's English label ("Highlight" is `### Highlight`), so a new button needs its section in the guide and no row
/// here, as a tool does ([`tool_anchor`]).
pub fn bar_anchor(action: &str) -> Option<Anchor> {
    let id = bar_topic(action)?;
    let meta = crate::actions::find(id)?;
    let label = crate::strings::source_text(&meta.label_key())?;
    Some(Anchor {
        topic: Cow::Borrowed(id),
        page: Cow::Borrowed(COMMAND_BAR),
        fragment: Cow::Owned(heading_id(label)),
    })
}

/// The topic of an action with a section of its own: what a form of the action opens (Check level…, Save as
/// template…) rather than the forms' section.
fn own_topic(action: &str) -> Option<&'static str> {
    Some(match action {
        "file.new" | "new_project.set" | "new_project.browse" | "new_project.other" => "new-project",
        a if a.starts_with("templates.") => "templates",
        "project.save_as_template" => "save-as-template",
        "project.check_level" => "check-level",
        a if a.starts_with("text_styles.") => "text-styles",
        "file.open_sample" => "sample-project",
        "file.import_qet" | "file.import" | "file.import_qet_apply" => "import-qet",
        "file.discard_journal" => "recovery",
        "tools.palette" => "command-palette",
        "tools.company_standard" => "company-standard-wizard",
        "help.shortcuts" => "keyboard-shortcuts",
        "help.first_run" => "first-run",
        "help.tutorials" => "tutorials",
        "help.about" | "help.notices" => "about",
        "help.documentation" => "help-f1",
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
        "layout.route" => "layout-route",
        "layout.arrange" | "layout.arrange_tick" | "layout.arrange_as_is" => "layout-arrange",
        "route.handle" | "route.handle_move" => "route-by-hand",
        a if a.starts_with("symbol.") => "symbol-editor",
        a if a.starts_with("tool.") => return tool_topic(a),
        a if a.starts_with("view.workspace") => "workspaces",
        // The PLC and Fluid tabs' commands: their discipline's guide.
        a if a.starts_with("bus.") => "guide-bus",
        a if a.starts_with("plc.") || a == "project.plc_addresses_at_terminals" => "guide-plc",
        a if a.starts_with("fluid.") => "guide-fluid",
        "file.print" | "file.export_pdf" => "dialog-print",
        "file.reimport_qet" | "file.reimport_qet_apply" => "dialog-reimport",
        "file.import_drawing" => "dialog-import-drawing",
        "file.import_eplan" => "dialog-import-eplan",
        "file.open_ewz" | "file.export_ewz" => "dialog-open-ewz",
        "file.upgrade" => "dialog-upgrade",
        "git.setup" => "dialog-git-setup",
        "project.purge_assets" => "dialog-purge",
        "nav.goto" | "page.goto" => "dialog-goto-page",
        "part.choose" | "part.pick" | "part.search" | "part.favourite" => "dialog-part-choose",
        a if a.starts_with("parts.") || a == "part.pin" => "guide-own-parts",
        "page.title_block" | "field.new" | "field.rename" | "field.remove" => "dialog-form",
        a if a.starts_with("frame.") => "edit-frame",
        "page.add_legend" => Thing::PanelLayout.topic(),
        "ip.supply_in" => Thing::SupplyPoint.topic(),
        "navigator.show" => "navigators",
        "notifications.show" | "notifications.run" | "toast.undo" => "notifications",
        _ => return None,
    })
}

/// The topic of an action by what it acts on: its family of actions (`cable.`, `view.`, `page.`, …).
fn family_topic(action: &str) -> Option<&'static str> {
    let (family, _) = action.split_once('.')?;
    Some(match family {
        "cable" => Thing::Cable.topic(),
        "devices" | "place" | "rep" | "cp" => Thing::Device.topic(),
        "ip" => Thing::InterruptionPoint.topic(),
        "strip" | "terminals" => Thing::Terminal.topic(),
        "junction" => Thing::Wire.topic(),
        "marker" => Thing::WireMarker.topic(),
        "text" => Thing::Text.topic(),
        "image" => Thing::Graphic.topic(),
        "box" => Thing::Box.topic(),
        "lists" | "report_block" => Thing::List.topic(),
        "layout" | "rail" => Thing::PanelLayout.topic(),
        "part" => "symbol-editor-device-box",
        "parts" | "workshop" => "guide-parts",
        "page" | "pages" | "section" => nav_topic(NavKind::Pages),
        "structure" | "structure_box" => nav_topic(NavKind::Structure),
        "potential" => nav_topic(NavKind::Potentials),
        "macros" => nav_topic(NavKind::Macros),
        "libraries" | "library" => nav_topic(NavKind::Libraries),
        "messages" => nav_topic(NavKind::Messages),
        "navigator" => "navigators",
        "props" => pane_topic(PaneKind::Properties),
        "find" => pane_topic(PaneKind::Find),
        "history" => pane_topic(PaneKind::History),
        "glossary" => pane_topic(PaneKind::Glossary),
        "jobs" => pane_topic(PaneKind::Jobs),
        "preview" => pane_topic(PaneKind::Preview),
        "bond" => nav_topic(NavKind::Potentials),
        "revision" => "revisions",
        "external_ref" => "dialog-form",
        "help" => "guides",
        "keymap" => "keyboard-shortcuts",
        "wizard" => "company-standard-wizard",
        "first_run" => "first-run",
        "journal" | "startup" => "recovery",
        "print" => "dialog-print",
        // The page view: moving about the pages, zoom, grid, tabs and highlights.
        "nav" | "tab" => pane_topic(PaneKind::PageView),
        "view" => match action {
            "view.theme" | "view.density" | "view.ui_scale" | "view.min_font" | "view.reduce_motion"
            | "view.tips" | "view.language" | "view.wire_colours" | "view.wheel_scrolls"
            | "view.cpu_canvas" | "view.frame_stats" | "view.command_bar" | "view.bottom_dock" => {
                "appearance"
            }
            _ => pane_topic(PaneKind::PageView),
        },
        "project" => match action {
            "project.wire_numbers" | "project.wire_numbering" | "project.marker_mode" => {
                Thing::WireMarker.topic()
            }
            "project.crossings" => Thing::Wire.topic(),
            _ => "project-view",
        },
        // Opening, saving and closing, undo and the clipboard: the project view's section.
        "file" | "edit" | "app" => "project-view",
        _ => return None,
    })
}

/// What the window names under the pointer in this frame (a button of the command bar, a menu item, a row of a list or
/// of Properties, a part of a screen): the topic F1 opens there and the words the status bar says it with ("F1:
/// Wires").
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HelpUnder {
    /// The topic ([`anchor`]).
    pub topic: String,
    /// What it is, in the UI language.
    pub subject: String,
}

/// The kinds of thing a page holds, each with the section of the guide that explains it: what F1 opens for a thing
/// under the pointer, selected, or a row of a list of such things.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Thing {
    /// A device or one of its symbols (a contactor's coil, a contact, a lamp).
    Device,
    /// A wire or a junction.
    Wire,
    /// The selected wire under the pointer: its handles change its route.
    WireRoute,
    /// A cable, one of its cores, or its mark.
    Cable,
    /// A terminal or a terminal strip, or a bridge between terminals.
    Terminal,
    /// An interruption point.
    InterruptionPoint,
    /// A supply point: an interruption point where the supply comes in from another drawing set.
    SupplyPoint,
    /// A PLC card, or a station's box.
    Plc,
    /// A device on a bus that is no PLC station (a switch, a drive's port).
    BusDevice,
    /// A valve, a cylinder or another fluid part.
    Valve,
    /// A valve terminal.
    ValveTerminal,
    /// A tube.
    Tube,
    /// A free text.
    Text,
    /// A wire marker that is no cable's mark.
    WireMarker,
    /// A graphic or an image.
    Graphic,
    /// A structure box or a black box.
    Box,
    /// The frame and its title block.
    Frame,
    /// A list: a generated page or a report block.
    List,
    /// A device placed on a panel layout page, a rail or a duct.
    PanelLayout,
}

impl Thing {
    /// Every kind, for the tests.
    pub const ALL: [Thing; 19] = [
        Thing::Device,
        Thing::Wire,
        Thing::WireRoute,
        Thing::Cable,
        Thing::Terminal,
        Thing::InterruptionPoint,
        Thing::SupplyPoint,
        Thing::Plc,
        Thing::BusDevice,
        Thing::Valve,
        Thing::ValveTerminal,
        Thing::Tube,
        Thing::Text,
        Thing::WireMarker,
        Thing::Graphic,
        Thing::Box,
        Thing::Frame,
        Thing::List,
        Thing::PanelLayout,
    ];

    /// The topic of its section: the page about things on a page, else the guide that explains it.
    pub fn topic(self) -> &'static str {
        match self {
            Thing::Device => "thing-device",
            Thing::Wire => "thing-wire",
            Thing::WireRoute => "route-by-hand",
            Thing::Cable => "thing-cable",
            Thing::Terminal => "thing-terminal",
            Thing::InterruptionPoint => "thing-ip",
            Thing::SupplyPoint => "thing-supply",
            Thing::Plc => "thing-plc",
            Thing::BusDevice => "guide-bus",
            Thing::Valve => "thing-valve",
            Thing::ValveTerminal => "valve-terminals",
            Thing::Tube => "thing-tube",
            Thing::Text => "thing-text",
            Thing::WireMarker => "tool.marker",
            Thing::Graphic => "thing-graphic",
            Thing::Box => "thing-box",
            Thing::Frame => "thing-frame",
            Thing::List => "thing-list",
            Thing::PanelLayout => "guide-panel-layout",
        }
    }

    /// What its section is about, in the UI language: the status bar's "F1: Wires".
    pub fn words(self) -> &'static str {
        strings::text(match self {
            Thing::Device => key!("help-thing-device"),
            Thing::Wire => key!("help-thing-wire"),
            Thing::WireRoute => key!("help-thing-wire-route"),
            Thing::Cable => key!("help-thing-cable"),
            Thing::Terminal => key!("help-thing-terminal"),
            Thing::InterruptionPoint => key!("help-thing-ip"),
            Thing::SupplyPoint => key!("help-thing-supply"),
            Thing::Plc => key!("help-thing-plc"),
            Thing::BusDevice => key!("help-thing-bus"),
            Thing::Valve => key!("help-thing-valve"),
            Thing::ValveTerminal => key!("help-thing-valve-terminal"),
            Thing::Tube => key!("help-thing-tube"),
            Thing::Text => key!("help-thing-text"),
            Thing::WireMarker => key!("help-thing-wire-marker"),
            Thing::Graphic => key!("help-thing-graphic"),
            Thing::Box => key!("help-thing-box"),
            Thing::Frame => key!("help-thing-frame"),
            Thing::List => key!("help-thing-list"),
            Thing::PanelLayout => key!("help-thing-panel-layout"),
        })
    }
}

/// What `e` on `page` is ([`Thing`]); `pointer` when it is under the pointer, where the one selected wire is its route
/// (its handles stand on it). A device by what it is (a terminal, a PLC card, a valve, …), a connection by its kind,
/// a page item by its kind; an entity the page tables do not hold is what the program draws itself: on a generated
/// page the list, else a device (an assembly overview's).
pub fn thing_at(model: &ShellModel, page: &CanvasPage, e: ew_dl::EntityRef, pointer: bool) -> Thing {
    let tables = &page.tables;
    match crate::tools::page_entity_kind(page, e) {
        Some(EntityKind::Rep) => {
            let Some(rep) = tables.reps.iter().find(|r| r.id == e.0) else {
                return Thing::Device;
            };
            if let Some(ip) = &rep.ip {
                return if ip.external {
                    Thing::SupplyPoint
                } else {
                    Thing::InterruptionPoint
                };
            }
            match rep.device.and_then(|d| device_thing(model, d)) {
                Some(Thing::Device) | None if rep.black_box.is_some() => Thing::Box,
                Some(thing) => thing,
                None => Thing::Device,
            }
        }
        Some(EntityKind::Connection) => {
            let Some(c) = tables.connections.iter().find(|c| c.id == e.0) else {
                return Thing::Wire;
            };
            if c.line_kind.is_some() {
                Thing::Tube
            } else if pointer && model.selection().items == [e] {
                Thing::WireRoute
            } else {
                match c.kind.as_str() {
                    "core" => Thing::Cable,
                    "bridge" => Thing::Terminal,
                    _ => Thing::Wire,
                }
            }
        }
        Some(EntityKind::Junction) => Thing::Wire,
        Some(EntityKind::Item) => {
            let Some(item) = tables.items.iter().find(|i| i.id == e.0) else {
                return Thing::Graphic;
            };
            match item.kind.as_str() {
                "free_text" => Thing::Text,
                // A marker whose wires are all cores of a cable is the cable's mark.
                "wire_marker" => {
                    let core = |id: &ew_api::output::Uuid| {
                        tables.connections.iter().any(|c| c.id == *id && c.kind == "core")
                    };
                    if !item.connections.is_empty() && item.connections.iter().all(core) {
                        Thing::Cable
                    } else {
                        Thing::WireMarker
                    }
                }
                "surface_view" => Thing::PanelLayout,
                _ => Thing::Graphic,
            }
        }
        Some(EntityKind::StructureBox) => Thing::Box,
        Some(EntityKind::Placement | EntityKind::Carrier | EntityKind::Feature) => Thing::PanelLayout,
        Some(EntityKind::Block) => Thing::List,
        // What the program draws itself: a generated page's list, an overview's devices.
        None if generated(model, page) => Thing::List,
        None => Thing::Device,
    }
}

/// What the pointer at the sheet point `p` of `page` is where it is on no thing: on a generated page the list, on the
/// sheet outside the frame's cells (its bands and the title block) the frame; `None` on the empty drawing area.
fn off_things(model: &ShellModel, page: &CanvasPage, p: (i64, i64)) -> Option<Thing> {
    if generated(model, page) {
        return Some(Thing::List);
    }
    let on_sheet = (0..=page.sheet.0).contains(&p.0) && (0..=page.sheet.1).contains(&p.1);
    let outside_cells = page.cells.as_ref().is_some_and(|c| c.cell_at(p).is_none());
    (on_sheet && outside_cells).then_some(Thing::Frame)
}

/// Whether `page` is a generated page (a list the program writes).
fn generated(model: &ShellModel, page: &CanvasPage) -> bool {
    model
        .page_rows()
        .iter()
        .any(|r| r.folio == page.folio && r.generated)
}

/// What the device `device` is, by its natures; `None` for an ID that names no device.
fn device_thing(model: &ShellModel, device: ew_api::output::Uuid) -> Option<Thing> {
    let session = model.project_session()?;
    let n = session.device_natures(device)?;
    Some(if session.terminal_kind(device).is_some() {
        Thing::ValveTerminal
    } else if n.station.is_some() || n.module {
        Thing::Plc
    } else if n.terminal || n.strip {
        Thing::Terminal
    } else if n.cable {
        Thing::Cable
    } else if n.fluid() {
        Thing::Valve
    } else if n.bus {
        Thing::BusDevice
    } else {
        Thing::Device
    })
}

/// The kind of thing a list's rows are, where its rows are things on a page.
fn list_thing(kind: NavKind) -> Option<Thing> {
    match kind {
        NavKind::Devices | NavKind::Unplaced => Some(Thing::Device),
        NavKind::Connections => Some(Thing::Wire),
        NavKind::Strips => Some(Thing::Terminal),
        NavKind::Cables => Some(Thing::Cable),
        NavKind::Tubes => Some(Thing::Tube),
        NavKind::Ips => Some(Thing::InterruptionPoint),
        NavKind::Blocks => Some(Thing::List),
        _ => None,
    }
}

/// What F1 opens for the row `key` of the list `kind` (`label` its words): a finding's or a rule's row its rule's
/// page, a device's row the device by what it is ([`Thing`]), any other row the kind of thing its list holds, else the
/// list's section.
pub fn row_help(model: &ShellModel, kind: NavKind, key: &str, label: &str) -> HelpUnder {
    let node = model.nav(kind).and_then(|n| n.node_of(key)).map(|(_, n)| n);
    let rule = (kind == NavKind::Messages)
        .then(|| node.and_then(crate::navigators::messages::rule_of))
        .flatten()
        .filter(|rule| rule_page(rule).is_some());
    if let Some(rule) = rule {
        return HelpUnder {
            topic: rule_topic(&rule),
            subject: rule,
        };
    }
    let device = node
        .filter(|n| n.kinds.first().is_some_and(|k| k == "device"))
        .and_then(|n| n.records.first())
        .and_then(|id| device_thing(model, *id));
    match device.or_else(|| list_thing(kind)) {
        Some(thing) => HelpUnder {
            topic: thing.topic().to_owned(),
            subject: thing.words().to_owned(),
        },
        None => HelpUnder {
            topic: nav_topic(kind).to_owned(),
            subject: label.to_owned(),
        },
    }
}

/// An open dialog, wizard, first-run flow or palette: its topic. F1 then answers only for what the window names inside
/// it.
fn modal_topic(model: &ShellModel) -> Option<&'static str> {
    if model.wizard().is_some() {
        return Some("company-standard-wizard");
    }
    if model.first_run().is_some() {
        return Some("first-run");
    }
    if model.palette().is_some() {
        return Some("command-palette");
    }
    model.dialog().map(dialog_topic)
}

/// What F1 opens for what is under the pointer, and the words the status bar says it with: what the window names
/// there ([`HelpUnder`]), else the thing on the page on show under the pointer ([`thing_at`]), the frame where the
/// pointer is on its bands or its title block; `None` over the empty drawing area, and over a page behind an open
/// dialog, the symbol editor or a station's view.
pub fn pointer_topic(model: &ShellModel) -> Option<HelpUnder> {
    if let Some(under) = model.help_under() {
        return Some(under.clone());
    }
    if modal_topic(model).is_some() || model.symbol_editor().is_some() || model.station_tab().is_some() {
        return None;
    }
    let canvas = (0..2).find_map(|group| model.canvas_of(group).filter(|c| c.pointer().is_some()))?;
    let page = canvas.page();
    let thing = match canvas.pointer_entity() {
        Some((e, _)) => thing_at(model, page, e, true),
        None => off_things(model, page, canvas.pointer()?)?,
    };
    Some(HelpUnder {
        topic: thing.topic().to_owned(),
        subject: thing.words().to_owned(),
    })
}

/// The topic F1 opens when the pointer is on nothing ([`pointer_topic`]), and Help ▸ What is this?: an open dialog's,
/// else what has the keyboard focus (a list's cursor row, the cursor row of Properties), else the selection's first
/// thing, else the active tool, else the screen (the symbol editor, a station's view, the page's kind, the project
/// view or the start page).
pub fn context_topic(model: &ShellModel) -> String {
    if let Some(topic) = modal_topic(model) {
        return topic.to_owned();
    }
    if let Some(editor) = model.symbol_editor() {
        return symbol_editor_topic(editor).to_owned();
    }
    // Parts makes and changes your own parts: the guide that walks through it.
    if model.parts_place().is_some() {
        return "guide-own-parts".to_owned();
    }
    if let Some(topic) = focus_topic(model) {
        return topic;
    }
    if let Some(tab) = model.station_tab() {
        return station_topic(model, tab).to_owned();
    }
    if let Some(topic) = selection_topic(model) {
        return topic;
    }
    match crate::command_bar::tool_action(model.tool().name()) {
        "tool.select" => {}
        action => {
            if let Some(topic) = tool_topic(action) {
                return topic.to_owned();
            }
        }
    }
    if model.has_project() {
        page_topic(model).unwrap_or("project-view").to_owned()
    } else {
        "start-page".to_owned()
    }
}

/// The part of the drawing in Parts on show: a box (a selected pin's own place), drawing with a tool, the selection's
/// first connection point or shape, else the drawing's page.
fn symbol_editor_topic(editor: &crate::shell::symbol_editor::SymbolEditor) -> &'static str {
    if let Some(b) = editor.device_box() {
        if b.selection.is_empty() {
            "symbol-editor-device-box"
        } else {
            "symbol-editor-pin"
        }
    } else if editor.tool != "select" {
        "symbol-editor-drawing"
    } else {
        selected_part(editor).unwrap_or("symbol-editor")
    }
}

/// The section of what is selected in the symbol editor's drawing: a connection point's, else a shape's (or a text's).
fn selected_part(editor: &crate::shell::symbol_editor::SymbolEditor) -> Option<&'static str> {
    use crate::shell::symbol_editor::Item;
    if editor.selection.iter().any(|i| matches!(i, Item::Cp(_))) {
        Some("symbol-editor-cp")
    } else if editor.selection.iter().any(|i| matches!(i, Item::Graphic(_))) {
        Some("symbol-editor-shape")
    } else {
        None
    }
}

/// A part of the symbol editor's screen under the pointer ([`symbol_editor_part`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditorPart {
    /// The drawing in the middle.
    Drawing,
    /// The outline on the left.
    Outline,
    /// Anything else: Properties, the command bar's room.
    Other,
}

/// The topic of a part of the symbol editor's screen under the pointer (F1 on any part opened the editor's
/// general page): a box (a selected pin's own place); the outline and the drawing by the
/// selection (a connection point, a shape: F1 on a point selected in the outline opened the outline's section),
/// else the outline its own section and the drawing by the tool in hand (Select its selecting and moving,
/// another its drawing); Properties the screen's section.
pub fn symbol_editor_part(
    editor: &crate::shell::symbol_editor::SymbolEditor,
    part: EditorPart,
) -> &'static str {
    if editor.device_box().is_some() {
        return symbol_editor_topic(editor);
    }
    match part {
        EditorPart::Outline => selected_part(editor).unwrap_or("symbol-editor-outline"),
        EditorPart::Other => "symbol-editor-screen",
        EditorPart::Drawing if editor.tool != "select" => "symbol-editor-drawing",
        EditorPart::Drawing => selected_part(editor).unwrap_or("symbol-editor-selecting"),
    }
}

/// A station's view: a valve terminal's the guide's valve terminals, any other the rack's section.
fn station_topic(model: &ShellModel, tab: &crate::area::StationTab) -> &'static str {
    match station_thing(model, tab) {
        Thing::ValveTerminal => Thing::ValveTerminal.topic(),
        _ => "dialog-rack",
    }
}

/// What F1 opens over a card or a cell of a station's view, and its words (a rack's card opened the page about
/// things on a page): a valve terminal's position its own place in the valve terminals' section, a PLC card the rack's
/// section.
pub fn station_card_help(model: &ShellModel, tab: &crate::area::StationTab) -> (&'static str, &'static str) {
    match station_thing(model, tab) {
        Thing::ValveTerminal => (
            "valve-terminal-positions",
            strings::text(key!("help-valve-terminal-positions")),
        ),
        _ => ("dialog-rack", strings::text(key!("help-rack"))),
    }
}

/// What the cards and cells of a station's view are: a valve terminal's cells, else PLC cards.
pub fn station_thing(model: &ShellModel, tab: &crate::area::StationTab) -> Thing {
    let valve_terminal = tab
        .station
        .zip(model.project_session())
        .is_some_and(|(station, session)| session.terminal_kind(station).is_some());
    if valve_terminal {
        Thing::ValveTerminal
    } else {
        Thing::Plc
    }
}

/// The selection's first thing on the page on show.
fn selection_topic(model: &ShellModel) -> Option<String> {
    let canvas = model.canvas()?;
    let e = *model.selection().items.first()?;
    Some(thing_at(model, canvas.page(), e, false).topic().to_owned())
}

/// The guide of the page on show where its type has one: a bus-topology page the bus part of the PLC guide, a fluid
/// page the fluid guide, a panel layout page the panel layout guide.
fn page_topic(model: &ShellModel) -> Option<&'static str> {
    match model.page_rows().get(model.page_index())?.kind {
        ew_api::PageType::BusTopology => Some("guide-bus"),
        ew_api::PageType::Fluid => Some("guide-fluid"),
        ew_api::PageType::PanelLayout => Some("guide-panel-layout"),
        _ => None,
    }
}

/// The topic of a tool action: the action itself (`tool.place`), whose anchor is [`tool_anchor`]'s.
fn tool_topic(action: &str) -> Option<&'static str> {
    crate::actions::find(action)
        .filter(|_| tool_anchor(action).is_some())
        .map(|m| m.id)
}

/// The topic of an open dialog. The match names every variant, so a new dialog does not compile until it
/// has its topic.
pub fn dialog_topic(dialog: &Dialog) -> &'static str {
    match dialog {
        Dialog::About | Dialog::Notices => "about",
        Dialog::Shortcuts => "keyboard-shortcuts",
        Dialog::ConfirmDiscard { .. } => "unsaved-changes",
        Dialog::ConfirmJournalDiscard | Dialog::Recovery { .. } => "recovery",
        Dialog::NewProject(_) => "new-project",
        Dialog::Templates(_) => "templates",
        Dialog::TextStyles(_) => "text-styles",
        Dialog::WorkshopStock(_) => "guide-parts",
        // Rework list…: its button's section of the command bar page.
        Dialog::Rework(_) => action_topic("rework.show").unwrap_or("dialog-form"),
        // Labels…: its button's section of the command bar page.
        Dialog::Labels(_) => action_topic("labels.show").unwrap_or("dialog-form"),
        // Package…: its button's section of the command bar page.
        Dialog::Package(_) => action_topic("package.export").unwrap_or("dialog-form"),
        Dialog::SaveWorkspace { .. } => "workspaces",
        Dialog::GotoPage { .. } => "dialog-goto-page",
        // A form opens the section of its action (Check level…, Cable…), else the forms' section.
        Dialog::Form(f) => match action_topic(f.action) {
            Some("project-view") | None => "dialog-form",
            Some(topic) => topic,
        },
        Dialog::ConfirmPurge { .. } => "dialog-purge",
        // Replacing an earlier DXF set: the export's own section.
        Dialog::ConfirmReplaceDxf { .. } => action_topic("file.export_dxf").unwrap_or("dialog-form"),
        Dialog::PartChoose(_) => "dialog-part-choose",
        // Remove… of Parts takes a part out of My parts: the guide that walks through your own parts.
        Dialog::ConfirmRemovePart { .. } | Dialog::ConfirmLeavePart { .. } => "guide-own-parts",
        Dialog::Print(_) => "dialog-print",
        Dialog::ExternalChange { .. } => "dialog-external-change",
        Dialog::ConfirmUpgrade { .. } => "dialog-upgrade",
        Dialog::ImportQet { .. } => "import-qet",
        Dialog::ReimportMatch { .. } => "dialog-reimport-match",
        Dialog::Reimport { .. } => "dialog-reimport",
        Dialog::OpenEwz { .. } => "dialog-open-ewz",
        Dialog::ImportDrawing { .. } => "dialog-import-drawing",
        Dialog::ImportEplan { .. } => "dialog-import-eplan",
        Dialog::DiscardStation => "dialog-rack",
        Dialog::PlcImport(_) => "dialog-plc-import",
        Dialog::ConfirmGitSetup { .. } => "dialog-git-setup",
        Dialog::ChooseStation { .. } => "dialog-rack",
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
        // The PLC, Bus and Fluid panels lead to their discipline's guide.
        NavKind::Plc | NavKind::PlcGrid => "guide-plc",
        NavKind::Bus => "guide-bus",
        NavKind::Fluid => "guide-fluid",
        NavKind::Tubes => "nav-tubes",
        NavKind::Media => "nav-media",
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

/// The topic of what has the keyboard focus: a list's cursor row ([`row_help`]; a panel with a guide its guide, the
/// strip editor or a list's table mode their sections), the cursor row of Properties ([`property_topic`]), the find
/// pane, the history, the command bar's tab; `None` for the canvas, whose selection and tool come next.
fn focus_topic(model: &ShellModel) -> Option<String> {
    Some(match model.focus()? {
        // A panel with a guide leads to it, in its table mode too (the PLC grid is a table).
        // A panel with a guide leads to it, but a row of a device (a module, a valve) to the device's section (a
        // valve's row opened the whole Fluid guide).
        Focus::Navigator(kind) if nav_topic(kind).starts_with("guide-") => {
            match model.nav(kind).and_then(|n| n.cursor_key()) {
                Some(key) if model.nav(kind).is_some_and(|n| n.table_mode().is_none()) => {
                    row_help(model, kind, &key, "").topic
                }
                _ => nav_topic(kind).to_owned(),
            }
        }
        Focus::Navigator(kind) if model.nav(kind).is_some_and(|n| n.table_mode().is_some()) => {
            if kind == NavKind::Strips {
                "strip-editor".to_owned()
            } else {
                "table-mode".to_owned()
            }
        }
        Focus::Navigator(kind) => match model.nav(kind).and_then(|n| n.cursor_key()) {
            Some(key) => row_help(model, kind, &key, "").topic,
            None => nav_topic(kind).to_owned(),
        },
        Focus::Properties => {
            property_cursor_topic(model).unwrap_or_else(|| pane_topic(PaneKind::Properties).to_owned())
        }
        Focus::Find => pane_topic(PaneKind::Find).to_owned(),
        Focus::History => pane_topic(PaneKind::History).to_owned(),
        Focus::CommandBar => match model.command_tab() {
            crate::command_bar::Tab::Plc => "guide-plc",
            crate::command_bar::Tab::Fluid => "guide-fluid",
            _ => "command-bar",
        }
        .to_owned(),
        Focus::Canvas(_) => return None,
    })
}

/// The topic of the cursor row of Properties.
fn property_cursor_topic(model: &ShellModel) -> Option<String> {
    let props = model.properties();
    let key = props.row.as_deref()?;
    props.sheet()?.sections.iter().find_map(|s| {
        s.rows
            .iter()
            .find(|r| r.key == key)
            .map(|r| property_topic(&s.kind, &r.path_for_label()))
    })
}

// ---------------------------------------------------------------------------------------------------------------
// The properties reference

/// The prefix of the keys of the texts that explain the rows of Properties (`field-help-device-tag`): the rest of the
/// key is the row's anchor on the properties reference (`device-tag`).
const HELP_KEY: &str = "field-help-";
/// The anchor of the section of the explanations that rows of several kinds share (`field-help-word-colour`).
const SHARED: &str = "word";

/// One section of the properties reference: the rows of one kind of record, or the rows several kinds share.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PropertySection {
    /// The record kind (`device`); empty for the rows several kinds share.
    pub kind: String,
    /// The section's anchor: the kind with `-` for `_` (`potential-def`), `word` for the shared rows.
    pub anchor: String,
    /// The kind as a heading in the UI language (`Potential definition`); empty for the shared rows.
    pub title: String,
    /// The rows, in key order.
    pub rows: Vec<PropertyEntry>,
}

/// One row of the properties reference: what a row of Properties means.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PropertyEntry {
    /// The anchor (`device-tag`).
    pub anchor: String,
    /// The row's label in the UI language (`Tag`).
    pub label: String,
    /// The explanation in en-GB: the row's tooltip.
    pub help: String,
}

/// The topic of the row of Properties for the field `path` of `kind` (its path as labelled,
/// `PropRow::path_for_label`): the anchor of the text that explains it (the row's tooltip, `strings::field_help`) on
/// the properties reference, else the section of its kind there, else the Properties pane's section.
pub fn property_topic(kind: &str, path: &str) -> String {
    let anchor = strings::field_help_key(kind, path)
        .map(|key| key[HELP_KEY.len()..].to_owned())
        .unwrap_or_else(|| kind.replace('_', "-"));
    if property_anchors().contains(anchor.as_str()) {
        format!("{PROPERTY}{anchor}")
    } else {
        pane_topic(PaneKind::Properties).to_owned()
    }
}

/// Every anchor of the properties reference: its sections' and its rows'.
fn property_anchors() -> &'static BTreeSet<String> {
    static ANCHORS: OnceLock<BTreeSet<String>> = OnceLock::new();
    ANCHORS.get_or_init(|| {
        property_sections()
            .into_iter()
            .flat_map(|s| std::iter::once(s.anchor).chain(s.rows.into_iter().map(|r| r.anchor)))
            .collect()
    })
}

/// The properties reference (`reference/properties.md`, written by `cargo xtask docs-gen`): one row for every text
/// that explains a row of Properties (`field-help-<kind>-<path>`), under the kind of record it belongs to, and the
/// texts every field of one name shares (`field-help-word-<member>`) in a section of their own; each with the label
/// the row shows (in the UI language: en-GB where the guide is written). A text names its kind by the longest start of its key that is a kind of the property grid
/// (`potential-def-name` is `potential_def`'s `name`); its label is that of the field of the kind's catalogue it
/// explains, else of the derived row it names (`device-derived-cp-item` is `derived.cp[]`).
pub fn property_sections() -> Vec<PropertySection> {
    let mut kinds: Vec<PropertySection> = Vec::new();
    let mut shared = PropertySection {
        kind: String::new(),
        anchor: SHARED.to_owned(),
        title: String::new(),
        rows: Vec::new(),
    };
    for rest in strings::source_keys(HELP_KEY) {
        let help = strings::source_text(&format!("{HELP_KEY}{rest}"))
            .unwrap_or_default()
            .to_owned();
        let named = if rest.starts_with("word-") {
            None
        } else {
            help_kind(rest)
        };
        let Some((kind, path)) = named else {
            let member = rest.strip_prefix("word-").unwrap_or(rest).replace('-', "_");
            shared.rows.push(PropertyEntry {
                anchor: rest.to_owned(),
                label: strings::field_label("", &member),
                help,
            });
            continue;
        };
        let entry = PropertyEntry {
            anchor: rest.to_owned(),
            label: strings::field_label(&kind, &path),
            help,
        };
        match kinds.iter_mut().find(|s| s.kind == kind) {
            Some(section) => section.rows.push(entry),
            None => kinds.push(PropertySection {
                anchor: kind.replace('_', "-"),
                title: strings::kind_heading(&kind),
                kind,
                rows: vec![entry],
            }),
        }
    }
    kinds.push(shared);
    kinds
}

/// The kind of the property grid and the field path an explanation's key (without its prefix) names: the longest
/// start of `rest` that is a kind with a field catalogue, and the catalogue's field whose explanation it is, else the
/// derived row it names.
fn help_kind(rest: &str) -> Option<(String, String)> {
    let words: Vec<&str> = rest.split('-').collect();
    (1..words.len()).rev().find_map(|n| {
        let kind = words[..n].join("_");
        let catalogue = ew_api::output::field_catalogue(&kind)?;
        let key = format!("{HELP_KEY}{rest}");
        let path = catalogue
            .iter()
            .find(|f| strings::field_help_key(&kind, &f.path).as_deref() == Some(key.as_str()))
            .map(|f| f.path.clone())
            .unwrap_or_else(|| derived_path(&words[n..]));
        Some((kind, path))
    })
}

/// The path of a derived row from the words of its key: `derived-cp-item` is `derived.cp[]`.
fn derived_path(words: &[&str]) -> String {
    let mut out = String::new();
    for (i, w) in words.iter().enumerate() {
        match *w {
            "item" => out.push_str("[]"),
            "derived" if i == 0 => out.push_str("derived."),
            w => {
                if !out.is_empty() && !out.ends_with('.') && !out.ends_with("[]") {
                    out.push('_');
                }
                out.push_str(w);
            }
        }
    }
    out
}

/// The page of a shipped rule in the book: the rule's declared `docs` reference as a page,
/// `reference/rules/<id in lower case>.md` (`reference/rules/ew-core-033.md`); `None` for a rule this build does not
/// ship.
pub fn rule_page(rule: &str) -> Option<String> {
    Some(format!("{}.md", rule_meta(rule)?.docs()))
}

/// The page of the built book for a shipped rule (F1 on a finding or rule row of the Messages pane):
/// `reference/rules/ew-core-033.html`.
pub fn rule_target(rule: &str) -> Option<String> {
    Some(format!("{}.html", rule_meta(rule)?.docs()))
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
    let page = anchor.page.strip_suffix(".md").unwrap_or(&anchor.page);
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

    /// The dialogs a test can build, each kind once.
    fn dialogs() -> Vec<Dialog> {
        let text = String::new;
        vec![
            Dialog::About,
            Dialog::Shortcuts,
            Dialog::GotoPage { text: text() },
            Dialog::Notices,
            Dialog::ConfirmJournalDiscard,
            Dialog::Templates(Box::new(crate::shell::TemplatesDialog {
                rows: Vec::new(),
                selected: 0,
                renaming: None,
                note: None,
            })),
            Dialog::SaveWorkspace { text: text() },
            Dialog::ConfirmPurge {
                files: Vec::new(),
                assets: Vec::new(),
            },
            Dialog::ConfirmRemovePart { number: text() },
            Dialog::ConfirmLeavePart {
                number: text(),
                next: None,
            },
            Dialog::ConfirmReplaceDxf {
                dir: text(),
                folder: text(),
                files: 2,
            },
            Dialog::ExternalChange { rows: Vec::new() },
            Dialog::ConfirmUpgrade { from: text() },
            Dialog::ImportQet {
                file: text(),
                summary: text(),
                verdict: text(),
                note: None,
                groups: Vec::new(),
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
            Dialog::ChooseStation {
                card: ew_api::output::Uuid::nil(),
                tag: text(),
                stations: Vec::new(),
            },
            Dialog::ImportDrawing {
                path: text(),
                name: text(),
                kind: crate::shell::DrawingKind::Dxf,
                folio: None,
                unitless: false,
                converter: false,
            },
            Dialog::ImportEplan {
                path: text(),
                name: text(),
                step: crate::shell::EplanStep::Terms,
            },
            Dialog::DiscardStation,
            Dialog::Rework(Box::new(crate::shell::rework::ReworkDialog {
                releases: Vec::new(),
                against: crate::shell::rework::Against::Nothing,
                list: Err(text()),
            })),
        ]
    }

    /// Every topic the window can open: the table's, every action's (a tool's section among them), every kind of
    /// thing on a page, every pane and list, every dialog a test can build and the forms', every row of the properties
    /// reference and every shipped rule's page.
    fn every_topic() -> Vec<String> {
        let mut topics: Vec<String> = ANCHORS.iter().map(|a| a.topic.to_string()).collect();
        topics.extend(
            crate::actions::ACTIONS
                .iter()
                .filter_map(|m| action_topic(m.id))
                .map(str::to_owned),
        );
        topics.extend(Thing::ALL.iter().map(|t| t.topic().to_owned()));
        topics.extend(PaneKind::ALL.into_iter().map(|p| pane_topic(p).to_owned()));
        topics.extend(NavKind::ALL.into_iter().map(|k| nav_topic(k).to_owned()));
        topics.extend(dialogs().iter().map(|d| dialog_topic(d).to_owned()));
        topics.extend(
            [
                "dialog-form",
                "dialog-part-choose",
                "dialog-print",
                "guide-own-parts",
            ]
            .map(str::to_owned),
        );
        topics.extend(property_anchors().iter().map(|a| format!("{PROPERTY}{a}")));
        let rules = ew_api::checks::rules();
        let integrity = ew_api::checks::rules::core::INTEGRITY;
        topics.extend(
            rules
                .iter()
                .map(|r| r.meta())
                .chain(integrity)
                .map(|m| rule_topic(m.id)),
        );
        topics.sort();
        topics.dedup();
        topics
    }

    /// The ids a page's source gives its headings and anchors: a heading's mdBook id or its `{#id}`, an `<a id="…">`.
    fn source_ids(text: &str) -> BTreeSet<String> {
        let mut ids = BTreeSet::new();
        for line in text.lines() {
            if let Some(heading) = line.strip_prefix('#') {
                let heading = heading.trim_start_matches('#');
                match heading.rsplit_once("{#") {
                    Some((_, id)) => ids.insert(id.trim_end_matches('}').to_owned()),
                    None => ids.insert(heading_id(heading)),
                };
            }
            for part in line.split("<a id=\"").skip(1) {
                if let Some((id, _)) = part.split_once('"') {
                    ids.insert(id.to_owned());
                }
            }
        }
        ids
    }

    #[test]
    fn topics_are_unique() {
        for (i, a) in ANCHORS.iter().enumerate() {
            assert!(ANCHORS[i + 1..].iter().all(|b| b.topic != a.topic), "{}", a.topic);
        }
    }

    /// Every target the window can open is in the guide: its page is in `SUMMARY.md` and holds the heading or anchor
    /// (a built reference page: the source it is built from). It reads the sources, never `target/book`: a local build
    /// of the guide older than the sources failed it.
    #[test]
    fn every_help_target_is_in_the_guide() {
        let summary = std::fs::read_to_string(Path::new(BOOK_SRC).join("SUMMARY.md")).expect("SUMMARY.md");
        let commands = command_names();
        assert!(!commands.is_empty());
        for topic in every_topic() {
            let a = anchor(&topic).unwrap_or_else(|| panic!("{topic}: no anchor"));
            if a.page != "introduction.md" {
                assert!(
                    summary.contains(a.page.as_ref()),
                    "{} is not in SUMMARY.md",
                    a.page
                );
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
            // A rule's built page is the rule's (`every_shipped_rule_has_its_page`); the properties reference's ids are
            // its sections' and rows' (`property_topic`'s check).
            if a.page.starts_with("reference/rules/") || a.page == PROPERTIES {
                continue;
            }
            let path = Path::new(BOOK_SRC).join(a.page.as_ref());
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", a.page));
            assert!(
                a.fragment.is_empty() || source_ids(&text).contains(a.fragment.as_ref()),
                "{}: no heading with the id `{}` (topic {topic}; a tool's section is headed by its English label)",
                a.page,
                a.fragment
            );
        }
    }

    /// F1 on Route opens its section of the tools page, not the project view's page.
    #[test]
    fn route_opens_its_section() {
        let a = action_topic("layout.route")
            .and_then(anchor)
            .expect("layout.route");
        assert_eq!((a.page.as_ref(), a.fragment.as_ref()), (TOOLS, "route"));
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
            html_target(&anchor(&rule_topic("EW-CORE-033")).unwrap()),
            "reference/rules/ew-core-033.html"
        );
        assert_eq!(
            rule_target("EW-CORE-005").as_deref(),
            Some("reference/rules/ew-core-005.html")
        );
        assert_eq!(rule_page("EW-NONE-001"), None);
        assert_eq!(anchor(&rule_topic("EW-NONE-001")), None);
    }

    /// Every tool action, every pane kind and every navigator kind has a topic whose anchor is in
    /// the book (`every_help_target_is_in_the_guide` checks the anchors), and so do the dialogs a test can build.
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
            assert_eq!(anchor(topic).map(|a| a.page), Some(Cow::Borrowed(TOOLS)), "{id}");
        }
        for kind in NavKind::ALL {
            // The PLC, Bus and Fluid panels open their discipline's guide, the others their panel's section.
            let page = match kind {
                NavKind::Plc | NavKind::PlcGrid | NavKind::Bus => PLC_GUIDE,
                NavKind::Fluid => FLUID_GUIDE,
                _ => PANELS,
            };
            assert_eq!(
                anchor(nav_topic(kind)).map(|a| a.page),
                Some(Cow::Borrowed(page)),
                "{kind:?}"
            );
            assert_eq!(pane_topic(PaneKind::Navigator(kind)), nav_topic(kind));
        }
        for pane in PaneKind::ALL {
            assert!(anchor(pane_topic(pane)).is_some(), "{pane:?}");
        }
        for topic in ["strip-editor", "table-mode", "command-bar"] {
            assert_eq!(
                anchor(topic).map(|a| a.page),
                Some(Cow::Borrowed(PANELS)),
                "{topic}"
            );
        }
        for dialog in dialogs() {
            assert!(anchor(dialog_topic(&dialog)).is_some(), "{dialog:?}");
        }
        for topic in ["dialog-form", "dialog-part-choose", "dialog-print"] {
            assert_eq!(
                anchor(topic).map(|a| a.page),
                Some(Cow::Borrowed(DIALOGS)),
                "{topic}"
            );
        }
    }

    /// Every action a menu or the command bar shows has a topic: F1 over its entry or its button opens a section.
    /// Only opening, saving, closing, undo and the clipboard open the project view's section; the others a closer one.
    #[test]
    fn every_entry_of_the_menus_and_the_bar_has_its_section() {
        let shown = crate::actions::ACTIONS
            .iter()
            .filter(|m| (m.menu.is_some() || m.placement.is_some()) && !m.hidden);
        for meta in shown {
            let topic = action_topic(meta.id).unwrap_or_else(|| panic!("{} has no topic", meta.id));
            assert!(anchor(topic).is_some(), "{} -> {topic}", meta.id);
            let family = meta.id.split('.').next().unwrap_or_default();
            assert!(
                topic != "project-view" || ["file", "edit", "app", "project"].contains(&family),
                "{}: the project view's section is not about it",
                meta.id
            );
        }
        // A button of the bar opens its own section; a menu-only action the section of what it acts on.
        assert_eq!(action_topic("cable.from_wires"), Some("cable.from_wires"));
        assert_eq!(action_topic("view.zoom_in"), Some("view.zoom_in"));
        assert_eq!(action_topic("cable.swap_cores"), Some(Thing::Cable.topic()));
        assert_eq!(action_topic("canvas.press"), None);
    }

    /// A row of Properties opens the properties reference at the text that explains it: its own (a device's tag), the
    /// text every field of its name shares (a wire's colour override reads the word's), else its kind's section; a
    /// kind with no row of the reference opens the Properties pane's section.
    #[test]
    fn a_property_row_opens_its_place_in_the_properties_reference() {
        let tag = anchor(&property_topic("device", "tag")).expect("the tag's row");
        assert_eq!(tag.page, PROPERTIES);
        assert_eq!(tag.fragment, "device-tag");
        assert_eq!(html_target(&tag), "reference/properties.html#device-tag");
        let qty = anchor(&property_topic("device", "parts[0].qty")).expect("a part's quantity");
        assert_eq!(qty.fragment, "device-parts-item-qty");
        // A field without a text of its own: the kind's section.
        assert_eq!(
            property_topic("device", "no_such_member"),
            format!("{PROPERTY}device")
        );
        assert_eq!(property_topic("no_such_kind", "x"), "pane-properties");
        // Every text that explains a row has its row in the reference, under one anchor.
        let anchors: Vec<String> = property_sections()
            .into_iter()
            .flat_map(|s| s.rows.into_iter().map(|r| r.anchor))
            .collect();
        for rest in strings::source_keys(HELP_KEY) {
            assert_eq!(anchors.iter().filter(|a| *a == rest).count(), 1, "{rest}");
        }
        // Labels are the rows' own words.
        let sections = property_sections();
        let device = sections
            .iter()
            .find(|s| s.kind == "device")
            .expect("the device's section");
        assert_eq!(device.title, "Device");
        let row = device
            .rows
            .iter()
            .find(|r| r.anchor == "device-tag")
            .expect("the tag's row");
        assert_eq!(row.label, strings::field_label("device", "tag"));
        assert_eq!(row.help, strings::field_help("device", "tag").unwrap());
    }

    #[test]
    fn html_targets_swap_the_extension() {
        let a = anchor("start-page").unwrap();
        assert_eq!(html_target(&a), "how-to/application-window.html#start-page");
        assert_eq!(html_target(&anchor("home").unwrap()), "introduction.html");
    }

    /// F1 on Highlight must not open the panels page. A button of the bar opens
    /// its action's own section: the bar page's (its English label), or an own section it had already.
    #[test]
    fn each_button_of_the_bar_opens_its_actions_own_section() {
        for m in crate::actions::ACTIONS
            .iter()
            .filter(|m| m.placement.is_some() && !m.hidden && !m.id.starts_with("tool."))
        {
            let topic = action_topic(m.id).expect("a topic");
            let a = anchor(topic).expect("an anchor");
            assert!(
                !a.fragment.is_empty(),
                "{} opens {}#{}, not a section of its own",
                m.id,
                a.page,
                a.fragment
            );
        }
        let a = action_topic("navigator.highlight")
            .and_then(anchor)
            .expect("Highlight");
        assert_eq!((a.page.as_ref(), a.fragment.as_ref()), (COMMAND_BAR, "highlight"));
    }
}
