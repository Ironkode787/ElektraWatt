// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rails (doc 16a §5.4 and §5.6): the left rail's navigator entries and History at its
//! foot, the right rail's palettes, as a toolkit-neutral model.
//!
//! The rail is a view of the layout, not a second state: the left rail's active entry is the pane shown in the place
//! of the Pages pane (the navigator group, whose tab bar the window does not draw), so the workspace file stays the one
//! source (`ew.workspace/1` unchanged). Only the left column's collapse is the shell's own view state. `rail.show
//! {side, pane}` shows an entry (opening its navigator beside the Pages pane when it is not open); on the entry already
//! shown it collapses the column, as a click on the active rail button does (doc 16a §5.4).

use crate::key;
use crate::layout::PaneKind;
use crate::navigators::NavKind;
use crate::shell::ShellModel;
use crate::strings;
use crate::strings::Key;

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

/// The left rail's entries (doc 16a §5.4, with Unplaced devices, Macros and Libraries after Report blocks): the word, the
/// icon and the entry's pane, whose [`PaneKind::title_key`] is the entry's label ([`left_label`]). Project shows the
/// Pages pane and, through its segmented control, the Devices navigator, so its label is its own.
pub const LEFT: [(&str, &str, PaneKind); 13] = [
    ("project", "files", PaneKind::Pages),
    (
        "strips",
        "ew-terminal-strip",
        PaneKind::Navigator(NavKind::Strips),
    ),
    ("cables", "ew-cable", PaneKind::Navigator(NavKind::Cables)),
    (
        "potentials",
        "ew-potential",
        PaneKind::Navigator(NavKind::Potentials),
    ),
    ("structure", "sitemap", PaneKind::Navigator(NavKind::Structure)),
    (
        "connections",
        "plug-connected",
        PaneKind::Navigator(NavKind::Connections),
    ),
    ("plc", "cpu", PaneKind::Navigator(NavKind::Plc)),
    ("ips", "ew-interruption-point", PaneKind::Navigator(NavKind::Ips)),
    ("parts", "packages", PaneKind::Navigator(NavKind::Parts)),
    ("blocks", "report", PaneKind::Navigator(NavKind::Blocks)),
    // The devices a layout space has not placed yet (doc 16 §5.2's row), the panel tools'
    // source, after Report blocks and without a Ctrl+<n> key.
    (
        "unplaced",
        "layout-dashboard",
        PaneKind::Navigator(NavKind::Unplaced),
    ),
    // After Report blocks, without a Ctrl+<n> key (the ninth is the last).
    ("macros", "template", PaneKind::Navigator(NavKind::Macros)),
    ("libraries", "books", PaneKind::Navigator(NavKind::Libraries)),
];

/// The right rail's palettes (doc 16a §5.6): until the palette panes are built,
/// each opens its picker, the keyboard path the palettes keep.
pub const RIGHT: [(&str, Key<'static>, &str, &str); 2] = [
    ("symbols", key!("rail-symbols"), "category", "tool.place"),
    ("parts", key!("rail-parts"), "package", "tool.place_part"),
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

/// The label key of the left rail's entry `word` for `pane`: the pane's title, `rail-project` for Project (which fronts
/// both [`PROJECT_PANES`]).
pub fn left_label(word: &str, pane: PaneKind) -> Key<'static> {
    if word == "project" {
        key!("rail-project")
    } else {
        pane.title_key()
    }
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
        .map(|(i, (word, icon, pane))| {
            let active = match shown {
                Some(s) if *word == "project" => PROJECT_PANES.contains(&s),
                Some(s) => s == *pane,
                None => false,
            };
            RailButton {
                word,
                label: strings::text(left_label(word, *pane)).to_owned(),
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
        label: strings::text(key!("pane-history")).to_owned(),
        icon: "history",
        target: RailTarget::Action("history.show"),
        shortcut: String::new(),
        active: model.history_shown(),
    };
    let right = RIGHT
        .iter()
        .map(|(word, key, icon, action)| RailButton {
            word,
            label: strings::text(*key).to_owned(),
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
