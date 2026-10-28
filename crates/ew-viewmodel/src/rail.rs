// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rails (WP-S2.17 part 3, D10, D15; doc 16a §5.4 and §5.6): the left rail's navigator entries and History at its
//! foot, the right rail's palettes, as a toolkit-neutral model.
//!
//! The rail is a view of the layout, not a second state: the left rail's active entry is the pane shown in the place
//! of the Pages pane (the navigator group, whose tab bar the window does not draw), so the workspace file stays the one
//! source (`ew.workspace/1` unchanged). Only the left column's collapse is the shell's own view state. `rail.show
//! {side, pane}` shows an entry (opening its navigator beside the Pages pane when it is not open); on the entry already
//! shown it collapses the column, as a click on the active rail button does (doc 16a §5.4).

use crate::layout::PaneKind;
use crate::navigators::NavKind;
use crate::shell::ShellModel;
use crate::strings;

/// Which rail.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    /// The navigator rail at the window's left.
    Left,
    /// The palettes rail at the window's right.
    Right,
}

impl Side {
    /// `left` or `right`.
    pub fn word(self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
        }
    }

    /// The side of `word`.
    pub fn from_word(word: &str) -> Option<Side> {
        [Side::Left, Side::Right].into_iter().find(|s| s.word() == word)
    }
}

/// What a rail button does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RailTarget {
    /// Shows a pane of the navigator group (`rail.show`).
    Pane(PaneKind),
    /// Runs a registry action (History's `history.show`, the palettes' tools).
    Action(&'static str),
}

/// One button of a rail.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RailButton {
    /// The entry's word (`project`, `strips`, `history`, `symbols`): the `pane` parameter of `rail.show`.
    pub word: &'static str,
    /// The entry's name (its accessible name and its tooltip's first line).
    pub label: String,
    /// The icon key.
    pub icon: &'static str,
    /// What a click does.
    pub target: RailTarget,
    /// Its key, `Ctrl+1` to `Ctrl+9` by position, empty after the ninth.
    pub shortcut: String,
    /// Whether it is the active entry.
    pub active: bool,
}

/// The rails of the window.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RailModel {
    /// The left rail's navigator entries in doc 16a §5.4's order.
    pub left: Vec<RailButton>,
    /// History at the left rail's foot.
    pub foot: RailButton,
    /// Whether the left column is collapsed to its rail.
    pub collapsed: bool,
    /// The right rail's palettes.
    pub right: Vec<RailButton>,
}

/// The left rail's entries (doc 16a §5.4): the word, the title key, the icon and the entry's pane. Project shows the
/// Pages pane and, through its segmented control, the Devices navigator.
pub const LEFT: [(&str, &str, &str, PaneKind); 10] = [
    ("project", "rail-project", "files", PaneKind::Pages),
    (
        "strips",
        "nav-title-strips",
        "ew-terminal-strip",
        PaneKind::Navigator(NavKind::Strips),
    ),
    (
        "cables",
        "nav-title-cables",
        "ew-cable",
        PaneKind::Navigator(NavKind::Cables),
    ),
    (
        "potentials",
        "nav-title-potentials",
        "ew-potential",
        PaneKind::Navigator(NavKind::Potentials),
    ),
    (
        "structure",
        "nav-title-structure",
        "sitemap",
        PaneKind::Navigator(NavKind::Structure),
    ),
    (
        "connections",
        "nav-title-connections",
        "plug-connected",
        PaneKind::Navigator(NavKind::Connections),
    ),
    ("plc", "nav-title-plc", "cpu", PaneKind::Navigator(NavKind::Plc)),
    (
        "ips",
        "nav-title-ips",
        "ew-interruption-point",
        PaneKind::Navigator(NavKind::Ips),
    ),
    (
        "parts",
        "nav-title-parts",
        "packages",
        PaneKind::Navigator(NavKind::Parts),
    ),
    (
        "blocks",
        "nav-title-blocks",
        "report",
        PaneKind::Navigator(NavKind::Blocks),
    ),
];

/// The right rail's palettes (doc 16a §5.6): until the palette panes are built (§7.1 of the WP-S2.17 specification),
/// each opens its picker, the keyboard path the palettes keep.
pub const RIGHT: [(&str, &str, &str, &str); 2] = [
    ("symbols", "rail-symbols", "category", "tool.place"),
    ("parts", "rail-parts", "package", "tool.place_part"),
];

/// The panes of the Project entry: the Pages pane and the Devices navigator (the segmented control's two views).
pub const PROJECT_PANES: [PaneKind; 2] = [PaneKind::Pages, PaneKind::Navigator(NavKind::Devices)];

/// The pane of `word` on the left rail (`project` and `pages` are the Pages pane, `devices` the Devices navigator).
pub fn left_pane(word: &str) -> Option<PaneKind> {
    match word {
        "devices" => return Some(PaneKind::Navigator(NavKind::Devices)),
        "pages" => return Some(PaneKind::Pages),
        _ => {}
    }
    LEFT.iter().find(|(w, ..)| *w == word).map(|(.., p)| *p)
}

/// The pane shown in the navigator group: the active tab of the group that holds the Pages pane, else of the group
/// that holds the first open rail navigator; `None` when no rail pane is in the layout.
pub fn shown_pane(model: &ShellModel) -> Option<PaneKind> {
    let layout = model.layout();
    std::iter::once(PaneKind::Pages)
        .chain(PROJECT_PANES.into_iter().skip(1))
        .chain(LEFT.iter().skip(1).map(|(.., p)| *p))
        .find_map(|p| layout.shown_in_place_of(p))
}

/// The rails of `model`.
pub fn rail(model: &ShellModel) -> RailModel {
    let shown = shown_pane(model);
    let left = LEFT
        .iter()
        .enumerate()
        .map(|(i, (word, key, icon, pane))| {
            let active = match shown {
                Some(s) if *word == "project" => PROJECT_PANES.contains(&s),
                Some(s) => s == *pane,
                None => false,
            };
            RailButton {
                word,
                label: strings::text(key).to_owned(),
                icon,
                target: RailTarget::Pane(*pane),
                shortcut: if i < 9 {
                    format!("Ctrl+{}", i + 1)
                } else {
                    String::new()
                },
                active,
            }
        })
        .collect();
    let foot = RailButton {
        word: "history",
        label: strings::text("pane-history").to_owned(),
        icon: "history",
        target: RailTarget::Action("history.show"),
        shortcut: String::new(),
        active: model.history_shown(),
    };
    let right = RIGHT
        .iter()
        .map(|(word, key, icon, action)| RailButton {
            word,
            label: strings::text(key).to_owned(),
            icon,
            target: RailTarget::Action(action),
            shortcut: String::new(),
            active: false,
        })
        .collect();
    RailModel {
        left,
        foot,
        collapsed: model.rail_collapsed(),
        right,
    }
}

/// The describe lines of the rails (D15): `rail left "Project" (shown), "Terminal strips", ... foot "History"`, with
/// ` collapsed` when the column is; `rail right "Symbols", "Parts"`.
pub fn describe_lines(model: &ShellModel) -> Vec<String> {
    let r = rail(model);
    let item = |b: &RailButton| {
        if b.active {
            format!("\"{}\" (shown)", b.label)
        } else {
            format!("\"{}\"", b.label)
        }
    };
    let mut left = format!(
        "rail left {} foot {}",
        r.left.iter().map(item).collect::<Vec<_>>().join(", "),
        item(&r.foot)
    );
    if r.collapsed {
        left.push_str(" collapsed");
    }
    let right = format!(
        "rail right {}",
        r.right.iter().map(item).collect::<Vec<_>>().join(", ")
    );
    vec![left, right]
}
