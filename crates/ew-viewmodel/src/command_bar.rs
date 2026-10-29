// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The command surface of WP-S2.17 part 2 (D3, doc 16a §5.2, §5.3, §5.5) in toolkit-neutral form: the command bar's
//! tabs and groups, the placement of an action in the bar, the bar's model for one tab, each tab's next step, and the
//! tool HUD's model.
//!
//! **The bar is generated from the registry.** An action of [`crate::actions::ACTIONS`] with a [`Placement`] is a
//! command button of its group, in `(tab, group, order)` order; the app menu ([`crate::actions::menu_model`]) shows the
//! same groups under the tab's menu, so the bar, the menu, the palette and the mockup cannot disagree. A command of doc
//! 16a §5.3 that has no action in this build is listed in [`PENDING`] and is drawn nowhere; it joins its group with
//! its package.
//!
//! **The HUD** ([`hud`]) names the active tool, three quick tools and the prompt: the selection and its keys with
//! Select, the pick stack under the pointer when it holds more than one object (FT-521), the tool's target and keys
//! otherwise.

use crate::actions::{self, ACTIONS, ActionItem, ActionMeta, Menu};
use crate::shell::ShellModel;
use crate::strings;
use crate::tools::{self, Ghost};

/// The tabs of the command bar, in the order of work (doc 16a §9.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Tab {
    /// Set up once: properties, parts, standards, imports.
    Project,
    /// Create, order and go to pages.
    Page,
    /// The core loop: select, place, connect, annotate, arrange (the tab that opens with a project).
    Draw,
    /// Make it right before output.
    Check,
    /// Everything that leaves the project.
    Output,
    /// Zoom, panels, layout and appearance.
    View,
}

impl Tab {
    /// The tabs, left to right.
    pub const ALL: [Tab; 6] = [
        Tab::Project,
        Tab::Page,
        Tab::Draw,
        Tab::Check,
        Tab::Output,
        Tab::View,
    ];

    /// The app menu's submenu of the tab (the same groups in the same order, doc 16a §9.2).
    pub const fn menu(self) -> Menu {
        match self {
            Tab::Project => Menu::Project,
            Tab::Page => Menu::Page,
            Tab::Draw => Menu::Draw,
            Tab::Check => Menu::Check,
            Tab::Output => Menu::Output,
            Tab::View => Menu::View,
        }
    }

    /// The tab's word in `view.command_tab`.
    pub fn word(self) -> &'static str {
        match self {
            Tab::Project => "project",
            Tab::Page => "page",
            Tab::Draw => "draw",
            Tab::Check => "check",
            Tab::Output => "output",
            Tab::View => "view",
        }
    }

    /// The tab of a word of `view.command_tab`.
    pub fn from_word(word: &str) -> Option<Tab> {
        Tab::ALL.into_iter().find(|t| t.word() == word)
    }

    /// The key of the tab's label (the app menu's submenu has the same label).
    pub fn label_key(self) -> &'static str {
        self.menu().label_key()
    }

    /// The tab's groups, left to right.
    pub fn groups(self) -> impl Iterator<Item = Group> {
        Group::ALL.into_iter().filter(move |g| g.tab() == self)
    }
}

/// The groups of the command bar (doc 16a §5.3), in bar order: a tab's groups follow each other in this order.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Group {
    /// Project: the project's properties and structure.
    Project,
    /// Project: the parts.
    Parts,
    /// Project: the schemes and the company standard.
    Standards,
    /// Project: the imports.
    Import,
    /// Page: new, properties, delete.
    Pages,
    /// Page: renumber and move.
    Order,
    /// Page: go to, back, forward, follow.
    Go,
    /// Draw: the Select tool.
    Select,
    /// Draw: symbols, devices, terminals, black boxes.
    Place,
    /// Draw: wires, potentials, interruption points.
    Connect,
    /// Draw: text and measure.
    Annotate,
    /// Draw: rotate, mirror, align, renumber.
    Arrange,
    /// Check: run the check scheme.
    Run,
    /// Check: the findings.
    Findings,
    /// Check: freeze and unfreeze tags.
    Numbering,
    /// Check: highlights.
    Trace,
    /// Output: the PDF.
    Pdf,
    /// Output: report blocks and generated pages.
    Reports,
    /// Output: the lists.
    Lists,
    /// Output: the jobs.
    Jobs,
    /// View: zoom.
    Zoom,
    /// View: grid, snap, statistics.
    Show,
    /// View: the panels.
    Panels,
    /// View: side by side, workspaces, the command bar.
    Layout,
    /// View: theme, density, UI scale.
    Appearance,
}

impl Group {
    /// Every group, in bar order.
    pub const ALL: [Group; 25] = [
        Group::Project,
        Group::Parts,
        Group::Standards,
        Group::Import,
        Group::Pages,
        Group::Order,
        Group::Go,
        Group::Select,
        Group::Place,
        Group::Connect,
        Group::Annotate,
        Group::Arrange,
        Group::Run,
        Group::Findings,
        Group::Numbering,
        Group::Trace,
        Group::Pdf,
        Group::Reports,
        Group::Lists,
        Group::Jobs,
        Group::Zoom,
        Group::Show,
        Group::Panels,
        Group::Layout,
        Group::Appearance,
    ];

    /// The tab the group belongs to.
    pub const fn tab(self) -> Tab {
        match self {
            Group::Project | Group::Parts | Group::Standards | Group::Import => Tab::Project,
            Group::Pages | Group::Order | Group::Go => Tab::Page,
            Group::Select | Group::Place | Group::Connect | Group::Annotate | Group::Arrange => Tab::Draw,
            Group::Run | Group::Findings | Group::Numbering | Group::Trace => Tab::Check,
            Group::Pdf | Group::Reports | Group::Lists | Group::Jobs => Tab::Output,
            Group::Zoom | Group::Show | Group::Panels | Group::Layout | Group::Appearance => Tab::View,
        }
    }

    /// The group's word (its label key is `group-<word>`).
    pub fn word(self) -> &'static str {
        match self {
            Group::Project => "project",
            Group::Parts => "parts",
            Group::Standards => "standards",
            Group::Import => "import",
            Group::Pages => "pages",
            Group::Order => "order",
            Group::Go => "go",
            Group::Select => "select",
            Group::Place => "place",
            Group::Connect => "connect",
            Group::Annotate => "annotate",
            Group::Arrange => "arrange",
            Group::Run => "run",
            Group::Findings => "findings",
            Group::Numbering => "numbering",
            Group::Trace => "trace",
            Group::Pdf => "pdf",
            Group::Reports => "reports",
            Group::Lists => "lists",
            Group::Jobs => "jobs",
            Group::Zoom => "zoom",
            Group::Show => "show",
            Group::Panels => "panels",
            Group::Layout => "layout",
            Group::Appearance => "appearance",
        }
    }

    /// The key of the group's name: the divider's tooltip, the app menu's submenu, the palette's breadcrumb.
    pub fn label_key(self) -> String {
        format!("group-{}", self.word())
    }

    /// Whether the group stays in the bar when the window is too narrow (doc 16a §5.2: Draw's Select, Place and
    /// Connect never collapse into an overflow button).
    pub fn never_collapses(self) -> bool {
        matches!(self, Group::Select | Group::Place | Group::Connect)
    }
}

/// The size of a command button (doc 16a §5.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CommandSize {
    /// An icon over its label, 46 px high (Compact: beside it, 28 px); at most two per group.
    Large,
    /// An icon beside its label, 24 px high, stacked two to a column (Compact: in one row).
    Small,
}

/// Where an action sits in the command bar (D3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Placement {
    /// The tab (always the group's).
    pub tab: Tab,
    /// The group.
    pub group: Group,
    /// The position in the group, ascending.
    pub order: u16,
    /// Large or small.
    pub size: CommandSize,
    /// The key of the short label the bar shows; `None`: the registry label without its ellipsis.
    pub bar_label: Option<&'static str>,
}

/// A placement in `group` (the registry's constructor).
pub const fn place(
    group: Group,
    order: u16,
    size: CommandSize,
    bar_label: Option<&'static str>,
) -> Placement {
    Placement {
        tab: group.tab(),
        group,
        order,
        size,
        bar_label,
    }
}

/// The commands of doc 16a §5.3 that have no action in this build, by group, position and the mockup's name. They
/// are drawn nowhere and join their group with their package (the conformance test CB8 skips them by name).
pub const PENDING: &[(Group, u16, &str)] = &[
    (Group::Parts, 2, "Project parts"),
    (Group::Standards, 1, "Schemes"),
    (Group::Pages, 2, "Page properties"),
    (Group::Pages, 3, "Delete page"),
    (Group::Place, 3, "Terminal"),
    (Group::Connect, 2, "Potential"),
    (Group::Arrange, 3, "Align"),
    (Group::Run, 2, "Check scheme"),
    (Group::Numbering, 1, "Freeze tag"),
    (Group::Numbering, 2, "Unfreeze tag"),
    (Group::Pdf, 2, "Print"),
    (Group::Reports, 1, "Report blocks"),
    (Group::Reports, 2, "Open generated page"),
    (Group::Lists, 1, "Parts list"),
    (Group::Lists, 2, "Wire list"),
    (Group::Lists, 3, "Terminal diagram"),
    (Group::Show, 2, "Snap"),
];

/// The split buttons: the main action runs the default, the chevron opens its variants (doc 16a §5.2).
pub const SPLITS: &[(&str, &[&str])] = &[("tool.mirror_x", &["tool.mirror_x", "tool.mirror_y"])];

/// The tab's next step (doc 16a §5.2): the one Primary button at the right end of the command row.
pub fn next_step(tab: Tab) -> Option<&'static str> {
    match tab {
        Tab::Draw => Some("messages.check"),
        Tab::Check => Some("file.export_pdf"),
        Tab::Project | Tab::Page | Tab::Output | Tab::View => None,
    }
}

/// What a command button does besides running its action.
#[derive(Clone, PartialEq, Debug)]
pub enum ButtonKind {
    /// Runs its action.
    Plain,
    /// Runs its action; its chevron opens these variants (Mirror).
    Split(Vec<ActionItem>),
    /// Opens these items as a menu (Theme, Density, the navigators): an action that needs a parameter.
    Menu(Vec<ActionItem>),
}

/// One command button of the bar.
#[derive(Clone, PartialEq, Debug)]
pub struct CommandButton {
    /// The action.
    pub id: &'static str,
    /// The registry label (the tooltip, the menus and the palette carry it).
    pub label: String,
    /// The short label the bar shows.
    pub bar_label: String,
    /// The icon key ([`crate::actions::ActionMeta::icon`]).
    pub icon: &'static str,
    /// Large or small.
    pub size: CommandSize,
    /// The first shortcut, empty when there is none.
    pub shortcut: String,
    /// Whether it can run now.
    pub enabled: bool,
    /// The reason text of a disabled button.
    pub reason: Option<String>,
    /// Whether it shows the active state (the running tool, Grid).
    pub checked: bool,
    /// Its documentation topic.
    pub topic: Option<&'static str>,
    /// Plain, split or menu.
    pub kind: ButtonKind,
}

/// One group of the command row.
#[derive(Clone, PartialEq, Debug)]
pub struct GroupModel {
    /// The group.
    pub group: Group,
    /// Its name (the divider's tooltip and the accessible name of its node).
    pub label: String,
    /// Its buttons in order.
    pub buttons: Vec<CommandButton>,
}

/// The command bar of one tab in the state of a shell.
#[derive(Clone, PartialEq, Debug)]
pub struct CommandBarModel {
    /// The tab whose command row is shown.
    pub tab: Tab,
    /// Whether the bar is collapsed to its tab row (Ctrl+F1).
    pub collapsed: bool,
    /// Every tab with its label, left to right.
    pub tabs: Vec<(Tab, String)>,
    /// The quick-access buttons of the tab row: Save, Undo, Redo (doc 16a §5.2).
    pub quick: Vec<CommandButton>,
    /// The tab's groups that hold a command, left to right.
    pub groups: Vec<GroupModel>,
    /// The tab's next step.
    pub next: Option<CommandButton>,
}

/// The registry label without its ellipsis (the bar's short label, doc 16a §8 item 3).
pub fn short_label(label: &str) -> String {
    label
        .trim_end_matches('\u{2026}')
        .trim_end_matches("...")
        .trim_end()
        .to_owned()
}

/// The actions placed in `group`, by their order.
pub fn placed_in(group: Group) -> Vec<&'static ActionMeta> {
    let mut metas: Vec<&'static ActionMeta> = ACTIONS
        .iter()
        .filter(|a| a.placement.is_some_and(|p| p.group == group))
        .collect();
    metas.sort_by_key(|a| a.placement.map_or(0, |p| p.order));
    metas
}

fn button_of(meta: &'static ActionMeta, model: &ShellModel, size: CommandSize) -> CommandButton {
    let item = actions::item_of(meta, model);
    let bar_label = match meta.placement.and_then(|p| p.bar_label) {
        Some(key) => strings::text(key).to_owned(),
        None => short_label(&item.label),
    };
    let kind = if let Some((_, variants)) = SPLITS.iter().find(|(id, _)| *id == meta.id) {
        ButtonKind::Split(
            variants
                .iter()
                .filter_map(|id| actions::find(id))
                .map(|m| actions::item_of(m, model))
                .collect(),
        )
    } else if let Some(submenu) = meta.menu.and_then(|s| s.submenu) {
        ButtonKind::Menu(actions::submenu_entries(submenu, model))
    } else {
        ButtonKind::Plain
    };
    CommandButton {
        id: meta.id,
        label: item.label,
        bar_label,
        icon: meta.icon.unwrap_or(""),
        size,
        shortcut: item.shortcut,
        enabled: item.enabled,
        reason: item.reason,
        checked: item.checked,
        topic: item.topic,
        kind,
    }
}

/// The command bar of `tab` in the state of `model`: its groups with a command, each command's state, the next step.
pub fn command_bar_of(model: &ShellModel, tab: Tab) -> CommandBarModel {
    let groups = tab
        .groups()
        .filter_map(|group| {
            let buttons: Vec<CommandButton> = placed_in(group)
                .into_iter()
                .map(|meta| {
                    let size = meta.placement.map_or(CommandSize::Small, |p| p.size);
                    button_of(meta, model, size)
                })
                .collect();
            (!buttons.is_empty()).then(|| GroupModel {
                group,
                label: strings::text(&group.label_key()).to_owned(),
                buttons,
            })
        })
        .collect();
    let next = next_step(tab).and_then(actions::find).map(|meta| {
        let mut b = button_of(meta, model, CommandSize::Small);
        b.kind = ButtonKind::Plain;
        b.bar_label = strings::text(&format!("next-step-{}", tab.word())).to_owned();
        b
    });
    CommandBarModel {
        tab,
        collapsed: model.command_bar_collapsed(),
        tabs: Tab::ALL
            .into_iter()
            .map(|t| (t, strings::text(t.label_key()).to_owned()))
            .collect(),
        quick: QUICK_ACCESS
            .iter()
            .filter_map(|id| action_button(model, id))
            .collect(),
        groups,
        next,
    }
}

/// The command bar of the shell's active tab.
pub fn command_bar(model: &ShellModel) -> CommandBarModel {
    command_bar_of(model, model.command_tab())
}

/// The quick-access buttons of the tab row (doc 16a §5.2).
pub const QUICK_ACCESS: [&str; 3] = ["file.save", "edit.undo", "edit.redo"];

/// The button of action `id` in the state of `model` (a quick-access or HUD button; its bar size is Small unless the
/// action is placed); `None` for an unknown action.
pub fn action_button(model: &ShellModel, id: &str) -> Option<CommandButton> {
    let meta = actions::find(id)?;
    let size = meta.placement.map_or(CommandSize::Small, |p| p.size);
    Some(button_of(meta, model, size))
}

/// The HUD's quick tools (the mockup's Wire, Symbol and Text).
pub const QUICK_TOOLS: [&str; 3] = ["tool.wire", "tool.place", "tool.text"];

/// The action that arms the tool named `name` (`select` → `tool.select`); `tool.select` for a tool without one.
pub fn tool_action(name: &str) -> &'static str {
    match name {
        "place" => "tool.place",
        "exchange" => "rep.exchange",
        "text_edit" => "text.edit",
        "paste" => "edit.paste_clip",
        other => ACTIONS
            .iter()
            .find(|a| a.id.strip_prefix("tool.") == Some(other))
            .map_or("tool.select", |a| a.id),
    }
}

/// The tool HUD (doc 16a §5.5): always shown over the canvas.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HudModel {
    /// The action of the active tool.
    pub tool: &'static str,
    /// The tool's label.
    pub tool_label: String,
    /// The tool's icon key.
    pub icon: &'static str,
    /// The three quick tools.
    pub quick: [&'static str; 3],
    /// What the tool works on (the predicted tag or terminal of Place), when it says.
    pub target: Option<String>,
    /// The prompt: the selection and its keys, the pick stack, or the tool's target and keys.
    pub prompt: String,
    /// How many objects a click under the pointer can pick (FT-500's stack; Select only, else 0).
    pub stack: usize,
}

/// The number of objects in the pick stack under the focused canvas's pointer (FT-500), 0 without a pointer.
pub fn pointer_stack(model: &ShellModel) -> usize {
    let Some(c) = model.canvas() else { return 0 };
    let Some(at) = c.pointer() else { return 0 };
    let k = c.points_per_nm();
    let reach = if k.is_finite() && k > 0.0 {
        crate::canvas::HOVER_SLOP_PT / k
    } else {
        0.0
    };
    let page = c.page();
    tools::pick_stack(page, &page.snapper, at, reach).len()
}

/// The first shortcut of action `id`, or an empty text.
fn key_of(id: &str) -> String {
    actions::find(id).map(actions::shortcut_text).unwrap_or_default()
}

/// The HUD of `model` (doc 16a §5.5, FT-521).
pub fn hud(model: &ShellModel) -> HudModel {
    let tool = model.tool();
    let name = tool.name();
    let action = tool_action(name);
    let mut target = None;
    let mut stack = 0;
    let hint = || crate::status::hint(model);
    let prompt = match model.tool_ghost() {
        None if name == "select" && tool.state() == "idle" => {
            stack = pointer_stack(model);
            let selected = model.selection().items.len();
            if stack > 1 {
                strings::format("hud-stack", &[&stack.to_string()])
            } else if selected > 0 {
                strings::format(
                    "hud-selected",
                    &[&selected.to_string(), &key_of("tool.rotate_cw")],
                )
            } else {
                hint()
            }
        }
        None => hint(),
        // Doc 16a §5.5: "Place -X1031:21 · R rotates · V variant · Esc done": the predicted tag or terminal (the symbol
        // until the pointer gives it one), then the keys; the ghost's line in `ui.describe` keeps the rest.
        // FT-545: while autoconnect proposes wires, the count and the way to place without them come before the keys,
        // so the strip says it before the first placement does.
        Some(Ghost::Place {
            symbol,
            strip,
            tag,
            hints,
            joinable,
            ..
        }) => {
            target = tag.or(strip);
            let mut what = target.clone().unwrap_or(symbol);
            if hints > 0 {
                let count = strings::format(
                    if hints == 1 { "count-wire" } else { "count-wires" },
                    &[&hints.to_string()],
                );
                what = format!(
                    "{what} \u{b7} {}",
                    strings::format("hud-place-wires", &[&count, &key_of("canvas.activate_alt")])
                );
            }
            // WP-S2.18 D2: "J Join device…" while the symbol can be an auxiliary function, before "Esc done".
            let keys = if joinable {
                strings::format(
                    "hud-keys-place-join",
                    &[
                        &key_of("tool.rotate_cw"),
                        &key_of("tool.next_variant"),
                        &key_of("place.join"),
                    ],
                )
            } else {
                strings::format(
                    "hud-keys-place",
                    &[&key_of("tool.rotate_cw"), &key_of("tool.next_variant")],
                )
            };
            format!("{what} \u{b7} {keys}")
        }
        Some(Ghost::Move { readout, .. }) => readout,
        Some(Ghost::Paste { records, .. }) => format!(
            "{} \u{b7} {}",
            strings::format("label-paste", &[&records.to_string()]),
            hint()
        ),
        Some(Ghost::Exchange { symbol, unmapped }) => {
            let mut parts = vec![symbol];
            parts.extend(unmapped);
            parts.join(" \u{b7} ")
        }
        Some(Ghost::Ip {
            name,
            direction,
            star,
            external,
            ..
        }) => {
            target = Some(name.clone());
            let mut parts = vec![name, direction.to_owned()];
            if star {
                parts.push(strings::text("action-tool-star-source").to_owned());
            }
            parts.extend(external);
            parts.join(" \u{b7} ")
        }
        Some(Ghost::Box { cps, .. }) => format!("{} \u{b7} {cps}", hint()),
        Some(Ghost::Text { text, bind, .. }) => {
            let shown = bind.map_or(text, |b| format!("{{{b}}}"));
            format!("{shown} \u{b7} {}", hint())
        }
        Some(Ghost::Graphic {
            shape,
            points,
            anticlockwise,
        }) => {
            let mut parts = vec![shape.to_owned(), points.to_string()];
            if shape == "arc" {
                let key = if anticlockwise {
                    "graphic-anticlockwise"
                } else {
                    "graphic-clockwise"
                };
                parts.push(strings::text(key).to_owned());
            }
            parts.push(hint());
            parts.join(" \u{b7} ")
        }
        Some(Ghost::Marker { connections, text }) => format!(
            "{} \u{b7} {} \u{b7} {}",
            connections,
            text.unwrap_or_else(|| "-".to_owned()),
            hint()
        ),
        Some(Ghost::StructureBox { outline }) => format!("{outline} \u{b7} {}", hint()),
        Some(Ghost::Image { asset }) => {
            format!("{} \u{b7} {}", asset.unwrap_or_else(|| "-".to_owned()), hint())
        }
    };
    HudModel {
        tool: action,
        tool_label: tools::tool_label(name),
        icon: actions::find(action).and_then(|a| a.icon).unwrap_or("pointer"),
        quick: QUICK_TOOLS,
        target,
        prompt,
        stack,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_group_belongs_to_one_tab_in_order() {
        let mut last = Tab::Project;
        for g in Group::ALL {
            assert!(g.tab() >= last, "{g:?} out of tab order");
            last = g.tab();
        }
        for t in Tab::ALL {
            assert!(t.groups().count() >= 3, "{t:?}");
            assert_eq!(Tab::from_word(t.word()), Some(t));
        }
        assert_eq!(Tab::from_word("nope"), None);
    }

    #[test]
    fn short_labels_drop_the_ellipsis() {
        assert_eq!(short_label("Symbol\u{2026}"), "Symbol");
        assert_eq!(short_label("Export PDF..."), "Export PDF");
        assert_eq!(short_label("Wire"), "Wire");
    }

    #[test]
    fn tools_map_to_their_actions() {
        assert_eq!(tool_action("select"), "tool.select");
        assert_eq!(tool_action("wire"), "tool.wire");
        assert_eq!(tool_action("place"), "tool.place");
        assert_eq!(tool_action("black_box"), "tool.black_box");
        assert_eq!(tool_action("paste"), "edit.paste_clip");
        assert_eq!(tool_action("no-such-tool"), "tool.select");
    }
}
