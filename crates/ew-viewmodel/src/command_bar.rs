// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The command surface (doc 16a §5.2, §5.3, §5.5) in toolkit-neutral form: the command bar's
//! tabs and groups, the placement of an action in the bar, the bar's model for one tab, each tab's next step, and the
//! tool HUD's model.
//!
//! **The bar is generated from the registry.** An action of [`crate::actions::ACTIONS`] with a [`Placement`] is a
//! command button of its group, in `(tab, group, order)` order; the app menu ([`crate::actions::menu_model`]) shows the
//! same groups under the tab's menu, so the bar, the menu and the palette cannot disagree. A command of doc 16a §5.3
//! that has no action in this build is drawn nowhere and is written down in that document alone.
//!
//! **The HUD** ([`hud`]) names the active tool, three quick tools and the prompt: the selection and its keys with
//! Select, the pick stack under the pointer when it holds more than one object, the tool's target and keys
//! otherwise.

use crate::actions::{self, ACTIONS, ActionItem, ActionMeta, Menu};
use crate::key;
use crate::shell::ShellModel;
use crate::strings;
use crate::strings::Key;
use crate::tools::{self, Ghost};
use ew_api::output::LabelShow;

/// The tabs of the command bar, in the order of work (doc 16a §9.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Tab {
    /// Set up once: properties, parts, standards, imports.
    Project,
    /// Create, order and go to pages.
    Page,
    /// The core loop: select, place, connect, annotate, arrange (the tab that opens with a project).
    Draw,
    /// The PLC and the bus: stations, addresses, bus pages, the PLC lists, the exchange with PLC software.
    Plc,
    /// Fluid power: fluid pages, tubes, fluid symbols, media and the fluid lists.
    Fluid,
    /// Make it right before output.
    Check,
    /// Everything that leaves the project.
    Output,
    /// Zoom, panels, layout and appearance.
    View,
}

impl Tab {
    /// The tabs, left to right.
    pub const ALL: [Tab; 8] = [
        Tab::Project,
        Tab::Page,
        Tab::Draw,
        Tab::Plc,
        Tab::Fluid,
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
            Tab::Plc => Menu::Plc,
            Tab::Fluid => Menu::Fluid,
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
            Tab::Plc => "plc",
            Tab::Fluid => "fluid",
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
    pub fn label_key(self) -> Key<'static> {
        self.menu().label_key()
    }

    /// The tab's own groups, left to right (its app menu's groups).
    pub fn groups(self) -> impl Iterator<Item = Group> {
        Group::ALL.into_iter().filter(move |g| g.tab() == self)
    }

    /// The groups the tab's command row shows, left to right: the groups it shares with another tab first, at the
    /// place they have on their own tab (its first), then its own (doc 16a §5.2: a tool on several tabs
    /// keeps its place).
    pub fn bar_groups(self) -> impl Iterator<Item = Group> {
        Group::ALL
            .into_iter()
            .filter(move |g| g.tab() != self && g.shared_with().contains(&self))
            .chain(self.groups())
    }

    /// Whether the tab works on a project: without one it is greyed (the start page showed the Draw tab as if
    /// one could draw). Project holds the start's Templates, View the window's appearance.
    pub fn needs_project(self) -> bool {
        !matches!(self, Tab::Project | Tab::View)
    }
}

/// Why `tab` is greyed in the state of `model` (no project is open for a tab that needs one; over the symbol editor,
/// whose bar has its one tab, none is greyed); `None` while it can be chosen.
pub fn tab_reason(model: &ShellModel, tab: Tab) -> Option<&'static str> {
    (tab.needs_project() && !model.has_project() && model.symbol_editor().is_none())
        .then(|| strings::text(key!("reason-tab-needs-project")))
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
    /// Page: the frame and its title block (Edit frame, Title block fields…).
    Frame,
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
    /// Draw: a panel layout page's set-up: rail, duct, auto-fill rail.
    PanelLayout,
    /// Draw: text and measure.
    Annotate,
    /// Draw: rotate, mirror, align, renumber.
    Arrange,
    /// PLC: the stations and their racks.
    Station,
    /// PLC: the channel addresses, the process tags and the symbol names.
    Addresses,
    /// PLC: the bus pages, ports and the Bus panel.
    Bus,
    /// PLC: the PLC panel, the PLC grid and the instrument list.
    PlcLists,
    /// PLC: the exchange with PLC software.
    Exchange,
    /// PLC: the PLC workspace and the PLC guide.
    PlcWorkspace,
    /// Fluid: a fluid page and the Tube tool.
    Tubes,
    /// Fluid: the fluid symbols, valve terminals and the air supply.
    FluidPlace,
    /// Fluid: the media and the fluid settings.
    Media,
    /// Fluid: the Fluid panel, the tube and medium tables, a valve terminal's overview.
    FluidLists,
    /// Fluid: the Fluid workspace and the fluid guide.
    FluidWorkspace,
    /// Check: run the check scheme.
    Run,
    /// Check: the findings.
    Findings,
    /// Check: freeze and unfreeze tags.
    Numbering,
    /// Check: highlights.
    Trace,
    /// Check: the revision: its author, its sign-off, its release, the revisions, the rework list.
    Revision,
    /// Output: the PDF.
    Pdf,
    /// Output: report blocks and generated pages.
    Reports,
    /// Output: the lists.
    Lists,
    /// Output: the workshop's lists (doc 13 §12).
    Workshop,
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
    pub const ALL: [Group; 40] = [
        Group::Project,
        Group::Parts,
        Group::Standards,
        Group::Import,
        Group::Pages,
        Group::Frame,
        Group::Order,
        Group::Go,
        Group::Select,
        Group::Place,
        Group::Connect,
        Group::PanelLayout,
        Group::Annotate,
        Group::Arrange,
        Group::Station,
        Group::Addresses,
        Group::Bus,
        Group::PlcLists,
        Group::Exchange,
        Group::PlcWorkspace,
        Group::Tubes,
        Group::FluidPlace,
        Group::Media,
        Group::FluidLists,
        Group::FluidWorkspace,
        Group::Run,
        Group::Findings,
        Group::Numbering,
        Group::Trace,
        Group::Revision,
        Group::Pdf,
        Group::Reports,
        Group::Lists,
        Group::Workshop,
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
            Group::Pages | Group::Frame | Group::Order | Group::Go => Tab::Page,
            Group::Select
            | Group::Place
            | Group::Connect
            | Group::PanelLayout
            | Group::Annotate
            | Group::Arrange => Tab::Draw,
            Group::Station
            | Group::Addresses
            | Group::Bus
            | Group::PlcLists
            | Group::Exchange
            | Group::PlcWorkspace => Tab::Plc,
            Group::Tubes | Group::FluidPlace | Group::Media | Group::FluidLists | Group::FluidWorkspace => {
                Tab::Fluid
            }
            Group::Run | Group::Findings | Group::Numbering | Group::Trace | Group::Revision => Tab::Check,
            Group::Pdf | Group::Reports | Group::Lists | Group::Workshop | Group::Jobs => Tab::Output,
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
            Group::Frame => "frame",
            Group::Order => "order",
            Group::Go => "go",
            Group::Select => "select",
            Group::Place => "place",
            Group::Connect => "connect",
            Group::PanelLayout => "panel-layout",
            Group::Annotate => "annotate",
            Group::Arrange => "arrange",
            Group::Station => "station",
            Group::Addresses => "addresses",
            Group::Bus => "bus",
            Group::PlcLists => "plc-lists",
            Group::PlcWorkspace => "plc-workspace",
            Group::Tubes => "tubes",
            Group::FluidPlace => "fluid-place",
            Group::Media => "media",
            Group::FluidLists => "fluid-lists",
            Group::FluidWorkspace => "fluid-workspace",
            Group::Run => "run",
            Group::Findings => "findings",
            Group::Numbering => "numbering",
            Group::Trace => "trace",
            Group::Revision => "revision",
            Group::Pdf => "pdf",
            Group::Reports => "reports",
            Group::Lists => "lists",
            Group::Workshop => "workshop",
            Group::Exchange => "exchange",
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

    /// The other tabs whose command row the group leads as well: Draw's Select group, the tools every
    /// drawn page is worked with (Select and Wire; Wire draws bus lines on a bus page and lines on a fluid page),
    /// leads PLC and Fluid too, so a person who changes tab finds them where they were. A shared group is the first
    /// group of its own tab (a test holds it), so it stands at the same place on every tab.
    pub fn shared_with(self) -> &'static [Tab] {
        match self {
            Group::Select => &[Tab::Plc, Tab::Fluid],
            _ => &[],
        }
    }

    /// Whether the group stays in the bar when the window is too narrow (doc 16a §5.2: Draw's Select and Place
    /// never collapse into an overflow button).
    pub fn never_collapses(self) -> bool {
        matches!(self, Group::Select | Group::Place)
    }

    /// Whether the group collapses only after every other one of its tab has (doc 16a §5.2: Draw's Connect, which
    /// holds Wire; it gives way at the smallest window in a language of long words, and nowhere else). Draw's
    /// Annotate, which holds Text and the Wire marker, collapses late too, before Connect: with Cable on Connect
    /// the Layout group of the panel pages gives way first below 1920, not the schematic's Marker.
    pub fn collapses_last(self) -> bool {
        matches!(self, Group::Connect | Group::Annotate)
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

/// Where an action sits in the command bar.
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
    pub bar_label: Option<Key<'static>>,
}

/// A placement in `group` (the registry's constructor).
pub const fn place(
    group: Group,
    order: u16,
    size: CommandSize,
    bar_label: Option<Key<'static>>,
) -> Placement {
    Placement {
        tab: group.tab(),
        group,
        order,
        size,
        bar_label,
    }
}

/// The split buttons: the main action runs the default, the chevron opens its variants (doc 16a §5.2).
pub const SPLITS: &[(&str, &[&str])] = &[
    ("tool.mirror_x", &["tool.mirror_x", "tool.mirror_y"]),
    // Enclosure…, the plate a layout page shows, the page of another face of the cabinet, and Fit the layout to the
    // sheet.
    (
        "layout.enclosure_new",
        &[
            "layout.enclosure_new",
            "layout.choose_plate",
            "layout.surface_page",
            "layout.fit_sheet",
        ],
    ),
    // New station… and New part…, for a card or a head station of one's own.
    ("plc.new_station", &["plc.new_station", "parts.new"]),
];

/// The tab's next step (doc 16a §5.2): the one Primary button at the right end of the command row.
pub fn next_step(tab: Tab) -> Option<&'static str> {
    match tab {
        Tab::Draw => Some("messages.check"),
        Tab::Check => Some("file.export_pdf"),
        Tab::Project | Tab::Page | Tab::Plc | Tab::Fluid | Tab::Output | Tab::View => None,
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
    /// The registry label (the tooltip, the menus and the palette carry it), then for some commands a line on what it
    /// does, which the tooltip alone shows ([`CommandButton::name`]).
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

impl CommandButton {
    /// Its name: the label's first line, which a menu that lists the button shows.
    pub fn name(&self) -> &str {
        self.label.lines().next().unwrap_or_default()
    }
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
    /// Whether a double click on a tab collapses or expands the bar (not over the symbol editor, whose bar is never
    /// collapsed).
    pub collapsible: bool,
    /// Every tab with its label, left to right.
    pub tabs: Vec<(Tab, String)>,
    /// The quick-access buttons of the tab row: Save, Undo, Redo (doc 16a §5.2).
    pub quick: Vec<CommandButton>,
    /// The tab row's workspace switch, left of the command search: Schematic, PLC and Fluid power, and a saved
    /// workspace by its name while it is the active one; empty without a project.
    pub workspaces: Vec<WorkspaceChoice>,
    /// The tab's groups that hold a command, left to right.
    pub groups: Vec<GroupModel>,
    /// The tab's next step.
    pub next: Option<CommandButton>,
    /// Over Parts, its two tabs in place of the project's (doc 07 §5.1): Part and Drawing, which is greyed with its
    /// reason while nothing is drawn.
    pub place_tabs: Option<PlaceTabs>,
}

/// The tabs of Parts: Part and Drawing.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlaceTabs {
    /// Whether the Drawing tab is the one shown.
    pub drawing: bool,
    /// Why the Drawing tab cannot be chosen now: nothing is drawn.
    pub drawing_reason: Option<String>,
}

/// One choice of the tab row's workspace switch: a click loads it (`view.workspace_load {name}`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WorkspaceChoice {
    /// The workspace's name, the `name` of `view.workspace_load`.
    pub name: String,
    /// Its label: a built-in workspace in the UI language's words, a saved one by its name.
    pub label: String,
    /// Whether it is the active workspace.
    pub active: bool,
}

/// The workspace switch of `model`: the built-in workspaces in their order, then the active workspace when it is a
/// saved one; none without a project (a workspace lays out the project view).
pub fn workspace_choices(model: &ShellModel) -> Vec<WorkspaceChoice> {
    if !model.has_project() {
        return Vec::new();
    }
    let active = model.workspace();
    let mut names: Vec<&str> = crate::settings::BUILT_IN_WORKSPACES.to_vec();
    if !names.contains(&active) {
        names.push(active);
    }
    names
        .into_iter()
        .map(|name| WorkspaceChoice {
            name: name.to_owned(),
            label: actions::workspace_label(name),
            active: name == active,
        })
        .collect()
}

/// The registry label without its ellipsis (the bar's short label, doc 16a §8 item 3).
pub fn short_label(label: &str) -> String {
    label
        .trim_end_matches('\u{2026}')
        .trim_end_matches("...")
        .trim_end()
        .to_owned()
}

/// The buttons of `group`: each action placed there, first or second ([`ActionMeta::also`]), with that place, by its
/// order.
pub fn buttons_in(group: Group) -> Vec<(&'static ActionMeta, Placement)> {
    let mut out: Vec<(&'static ActionMeta, Placement)> = ACTIONS
        .iter()
        .flat_map(|a| [a.placement, a.also].into_iter().flatten().map(move |p| (a, p)))
        .filter(|(_, p)| p.group == group)
        .collect();
    out.sort_by_key(|(_, p)| p.order);
    out
}

/// The actions placed first in `group`, by their order (the app menu's group).
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
    let kind = if meta.id == "parts.new_symbol" {
        // New symbol's menu: its starts, each the same action with its `like`.
        let mut starts = vec![item.clone()];
        starts.extend(
            model
                .new_symbol_starts()
                .into_iter()
                .map(|(like, label, reason)| actions::ActionItem {
                    params: serde_json::json!({ "like": like }),
                    label,
                    shortcut: String::new(),
                    enabled: reason.is_none(),
                    reason,
                    ..item.clone()
                }),
        );
        ButtonKind::Split(starts)
    } else if let Some((_, variants)) = SPLITS.iter().find(|(id, _)| *id == meta.id) {
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
    // A menu button whose every entry is greyed (no station yet, no port to place) is greyed with the first one's
    // reason, so the bar says why before the menu opens.
    let (mut enabled, mut reason) = (item.enabled, item.reason);
    if let ButtonKind::Menu(items) = &kind
        && enabled
        && !items.is_empty()
        && items.iter().all(|i| !i.enabled)
    {
        enabled = false;
        reason = items[0].reason.clone();
    }
    // A command whose name does not tell it from its neighbour's has a line in its tooltip on what it does: the
    // `tooltip-` key of its ID.
    let label = match strings::lookup(&format!("tooltip-{}", meta.id.replace(['.', '_'], "-"))) {
        Some(how) => format!("{}\n{how}", item.label),
        None => item.label,
    };
    CommandButton {
        id: meta.id,
        label,
        bar_label,
        icon: meta.icon.unwrap_or(""),
        size,
        shortcut: item.shortcut,
        enabled,
        reason,
        checked: item.checked,
        topic: item.topic,
        kind,
    }
}

/// The command bar of `tab` in the state of `model`: its groups with a command, each command's state, the next step.
pub fn command_bar_of(model: &ShellModel, tab: Tab) -> CommandBarModel {
    let groups = tab
        .bar_groups()
        .filter_map(|group| {
            let buttons: Vec<CommandButton> = buttons_in(group)
                .into_iter()
                .map(|(meta, p)| button_of(meta, model, p.size))
                .collect();
            (!buttons.is_empty()).then(|| GroupModel {
                group,
                label: strings::text(Key::run_time(&group.label_key())).to_owned(),
                buttons,
            })
        })
        .collect();
    let next = next_step(tab).and_then(actions::find).map(|meta| {
        let mut b = button_of(meta, model, CommandSize::Small);
        b.kind = ButtonKind::Plain;
        b.bar_label = strings::text(Key::run_time(&format!("next-step-{}", tab.word()))).to_owned();
        b
    });
    CommandBarModel {
        tab,
        collapsed: model.command_bar_collapsed(),
        collapsible: true,
        tabs: Tab::ALL
            .into_iter()
            .map(|t| (t, strings::text(t.label_key()).to_owned()))
            .collect(),
        quick: QUICK_ACCESS
            .iter()
            .filter_map(|id| action_button(model, id))
            .collect(),
        workspaces: workspace_choices(model),
        groups,
        next,
        place_tabs: None,
    }
}

/// The command bar of the shell's active tab.
pub fn command_bar(model: &ShellModel) -> CommandBarModel {
    command_bar_of(model, model.command_tab())
}

/// The command bar the window shows for the screen ([`ShellModel::screen`]): [`command_bar`], but over Parts, which has
/// its own row of commands, a bar never collapsed with the place's two tabs, Part and Drawing, and no workspaces.
pub fn shown_command_bar(model: &ShellModel) -> CommandBarModel {
    let mut bar = command_bar(model);
    bar.collapsed = shown_collapsed(model);
    if model.screen() == crate::shell::Screen::Parts {
        bar.collapsible = false;
        bar.tabs.clear();
        bar.workspaces.clear();
        bar.place_tabs = Some(PlaceTabs {
            drawing: model.symbol_editor().is_some(),
            drawing_reason: (model.kept_symbol_editor().is_none())
                .then(|| strings::text(key!("reason-no-drawing-in-hand")).to_owned()),
        });
    }
    bar
}

/// Whether the command bar the window shows is collapsed to its tab row: the project's bar's collapse, never over
/// Parts ([`shown_command_bar`]).
pub fn shown_collapsed(model: &ShellModel) -> bool {
    model.command_bar_collapsed() && model.parts_place().is_none()
}

/// The Draw tab's groups whose commands act on the selection: the canvas context menu offers them.
pub const SELECTION_GROUPS: [Group; 1] = [Group::Arrange];

/// Where a right-click on the canvas fell, as the window saw it when the menu opened.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MenuSpot {
    /// An object, which the click selected.
    Object,
    /// The frame or the title block: the sheet outside the frame grid's drawing area.
    Frame,
    /// An empty spot of the page.
    #[default]
    Sheet,
}

/// The spot of a right-click on `canvas` at its pointer over the entity `over`: an object when the page holds it as
/// a record, else the frame when the pointer is on one of the page's frame bands (the title block and its texts among
/// them), else the sheet.
pub fn menu_spot(canvas: &crate::canvas::CanvasModel, over: Option<ew_dl::EntityRef>) -> MenuSpot {
    let page = canvas.page();
    if over.is_some_and(|e| tools::page_entity_kind(page, e).is_some()) {
        return MenuSpot::Object;
    }
    let on_frame = canvas.pointer().is_some_and(|p| {
        let at = ew_api::routing::Ticks::new(page.tables.grid).pt(page.snapper.sheet_to_page(p));
        page.draft
            .keep_out
            .iter()
            .any(|r| (r.x0..=r.x1).contains(&at.x) && (r.y0..=r.y1).contains(&at.y))
    });
    if on_frame {
        MenuSpot::Frame
    } else {
        MenuSpot::Sheet
    }
}

/// The canvas context menu, the quick way to what fits the thing under the pointer (doc 16a §6.13). One rule:
/// first what fits the thing by what it is, then the general entries.
///
/// - Over an object: the entries that fit it (`ShellModel::menu_fits`: a wire's, an interruption point's, a
///   terminal's, a device's), then, where the selection holds a symbol, the commands the command bar offers for it
///   (the buttons of [`SELECTION_GROUPS`], in the bar's order and as a collapsed group lists them: a split button, then
///   its other variants), Move to page… and Keep in place, over a legend List columns…, then Cut, Copy, Paste, Paste
///   in place, Delete and Properties of selection. Over a placement of a panel page: Turn, Flip, Lock or Unlock, Reset label position and Delete.
/// - Over the frame or the title block: Title block fields…, then what an empty spot offers.
/// - Over an empty spot: Paste, Paste in place, Page properties, New page…, on a list's page Export list… and, on a page
///   that is not a panel layout, Print wire numbers; on a panel layout, Add legend and Reset label positions.
///
/// An entry that cannot run now is disabled and carries its reason, as the bar's button does. `at` is the sheet point
/// the menu was opened at (a right-click's; `None` from the keyboard): a valve terminal's cell there offers its own
/// entries. No action of its own.
pub fn canvas_menu(model: &ShellModel, spot: MenuSpot, at: Option<(i64, i64)>) -> Vec<ActionItem> {
    let item = |id: &str| model.menu_item(id);
    if spot != MenuSpot::Object {
        let mut items = Vec::new();
        if spot == MenuSpot::Frame {
            items.extend(item("page.title_block"));
            items.extend(item("frame.edit"));
        }
        items.extend(PASTE.iter().filter_map(|id| item(id)));
        items.extend(item("props.focus").map(|mut i| {
            i.label = strings::text(key!("canvas-page-properties")).to_owned();
            i
        }));
        items.extend(item("page.new"));
        // A list's page exports its list as a file (doc 13-workshop-lists).
        if model.shows_list() {
            items.extend(item("lists.export"));
        }
        if model.canvas().is_some_and(|c| c.page().panel.is_none()) {
            items.extend(item("project.wire_numbers"));
        } else if model.legend_offer() != ew_api::output::LegendOffer::NoLayout {
            // A panel page offers its legend beside the layout (F21), what its devices are labelled with and
            // puts its moved labels back.
            items.extend(item("page.add_legend"));
            let shown = model.panel_tables().map(|p| p.show);
            for (show, label, labels) in [
                ("tags", key!("canvas-labels-tags"), LabelShow::Tags),
                (
                    "item_numbers",
                    key!("canvas-labels-item-numbers"),
                    LabelShow::ItemNumbers,
                ),
                ("both", key!("canvas-labels-both"), LabelShow::Both),
                (
                    "tags_or_numbers",
                    key!("canvas-labels-tags-or-numbers"),
                    LabelShow::TagsOrNumbers,
                ),
            ] {
                let checked = shown == Some(labels);
                items.extend(model.menu_item("layout.labels").map(|mut i| {
                    i.params = serde_json::json!({ "show": show });
                    i.label = strings::text(label).to_owned();
                    i.checked = checked;
                    i
                }));
            }
            items.extend(item("layout.reset_labels"));
            items.extend(item("layout.fit_sheet"));
            // A hole by hand where the menu opened (doc 12-3d §10.6).
            items.extend(model.menu_item("layout.feature_add").map(|mut i| {
                if let Some((x, y)) = at {
                    i.params = serde_json::json!({ "x_mm": x as f64 / 1e6, "y_mm": y as f64 / 1e6 });
                }
                i
            }));
            // The cabinet's other faces, each its page (doc 12 §3.8).
            for (label, params) in model.surface_menu() {
                items.extend(model.menu_item("layout.surface_page").map(|mut i| {
                    i.params = params.clone();
                    i.label = label.clone();
                    i
                }));
            }
        }
        // A schematic page offers New station… (the rack comes first).
        let kind = model.page_rows().get(model.page_index()).map(|r| &r.kind);
        if kind.is_some_and(ew_api::PageType::is_schematic) {
            items.extend(item("plc.new_station"));
        }
        // A bus-topology page offers Put a station on this page… (the CPU drawn with its cards had no
        // way onto it), greyed with its reason when every station is on it.
        if kind == Some(&ew_api::PageType::BusTopology) {
            items.extend(item("bus.put_station"));
        }
        return items;
    }
    // Over a placement on a panel page, Turn, Flip, Lock or Unlock, Reset label position (disabled with its reason
    // while the label stands where the rules put it), and Delete.
    if model.selected_placements() > 0 {
        let lock = if model.selected_placement_locked() {
            "layout.unlock_selection"
        } else {
            "layout.lock_selection"
        };
        return [
            "layout.rotate",
            "layout.flip",
            lock,
            "layout.reset_label",
            "edit.delete_selection",
        ]
        .into_iter()
        .collect::<Vec<_>>()
        .iter()
        .filter_map(|id| actions::find(id))
        .map(|meta| actions::item_of(meta, model))
        .collect();
    }
    // Over a gland hole: Keep it as a hole (the hole stays, made by hand, its cable forgotten: EW-LAY-018's other
    // offer beside its quick fix Remove the hole) and Delete.
    if let Some(feature) = model.selected_gland_hole() {
        let keep = serde_json::json!({"cmd": "layout.feature.edit",
            "params": {"feature": feature.to_string(), "for_cable": false}});
        let mut items: Vec<ActionItem> = model
            .aimed(
                "command.run",
                keep,
                strings::text(key!("canvas-keep-as-hole")).to_owned(),
            )
            .into_iter()
            .collect();
        items.extend(actions::find("edit.delete_selection").map(|m| actions::item_of(m, model)));
        return items;
    }
    let bar = command_bar_of(model, Tab::Draw);
    let mut items = model.menu_fits(at);
    // Rotate, Mirror and Renumber devices… act on symbols: a selection without one (wires, a junction, a text) is
    // not offered them.
    let symbols = model.canvas().is_some_and(|c| {
        tools::selection_kinds(model.selection(), &c.page().tables)
            .iter()
            .any(|(_, k)| *k == tools::EntityKind::Rep)
    });
    for g in bar
        .groups
        .iter()
        .filter(|g| symbols && SELECTION_GROUPS.contains(&g.group))
    {
        // Add pin stands among a box's own entries, aimed at the click, not here.
        for b in g.buttons.iter().filter(|b| b.id != "box.add_cp") {
            let variants = match &b.kind {
                ButtonKind::Menu(_) => continue,
                ButtonKind::Plain => &[][..],
                ButtonKind::Split(variants) => variants.as_slice(),
            };
            items.push(ActionItem {
                id: b.id,
                params: serde_json::Value::Null,
                label: b.name().to_owned(),
                shortcut: b.shortcut.clone(),
                enabled: b.enabled,
                reason: b.reason.clone(),
                checked: b.checked,
                topic: b.topic,
            });
            items.extend(variants.iter().filter(|v| v.id != b.id).cloned());
        }
    }
    // Move to page…, which asks for the page, and Keep in place, checked while every selected symbol is kept (doc 06a
    // §2.4).
    if symbols {
        items.extend(item("edit.move_to_page"));
        items.extend(item("rep.keep_in_place"));
        // A text bound to the one selected symbol (doc 06 §3.8): its function text over its path, a strip's name.
        if let [one] = model.selected_rep_rows().as_slice() {
            if one.terminal {
                items.extend(item("text.strip_name"));
            } else if one.device.is_some() {
                items.extend(item("text.function_over_path"));
            }
        }
    }
    // Over a text of the one selected symbol, Hide text and Put the text back (greyed with its reason while the texts
    // stand where the symbol puts them); over a symbol with a moved text, Put
    // the text back.
    let over_text = model.text_under_pointer();
    if let Some(slot) = over_text
        && let Some(meta) = actions::find("rep.hide_text")
    {
        let mut hide = actions::item_of(meta, model);
        hide.params = serde_json::json!({ "slot": slot });
        items.push(hide);
    }
    if over_text.is_some() || !model.selected_reps_with_moved_texts().is_empty() {
        items.extend(item("rep.reset_texts"));
    }
    // A legend chooses its printed columns as a list does (doc 09 §3.8).
    if model.selected_legend().is_some() {
        items.extend(item("report_block.columns"));
        // An embedded legend becomes its layout's legend page (doc 12 §5.4).
        items.extend(item("report_block.legend_to_page"));
    }
    for id in ["edit.cut", "edit.copy"]
        .into_iter()
        .chain(PASTE)
        .chain(["edit.delete_selection", "props.focus"])
    {
        items.extend(item(id));
    }
    items
}

/// Paste and Paste in place, which stand together wherever Paste stands in the canvas menu (Paste in place was in the
/// command search alone).
const PASTE: [&str; 2] = ["edit.paste_clip", "edit.paste_in_place"];

/// The context menu of the Pages tree's row `row`: on a page, what the Page tab offers for it
/// (`ShellModel::page_row_fits`); elsewhere what the project can gain there, New page… (its folio proposed after the
/// row's section's last page) and New section…. Each
/// in the state of `model` (disabled with its reason in a read-only project).
pub fn pages_tree_menu(model: &ShellModel, row: &crate::shell::ShownRow) -> Vec<ActionItem> {
    if row.role == crate::navigators::NavRole::Page.word() {
        let mut items = Vec::new();
        model.page_row_fits(&row.key, &mut items);
        return items;
    }
    let mut items: Vec<ActionItem> = ["page.new", "section.new"]
        .into_iter()
        .filter_map(actions::find)
        .map(|meta| actions::item_of(meta, model))
        .collect();
    // New page… after the row's last page (a section's), else as the menu offers it.
    if let Some(new_page) = items.iter_mut().find(|i| i.id == "page.new") {
        new_page.params = serde_json::json!({ "at": row.key });
    }
    // A section's row (`s:<n>`, the last part of its key in every mode) also edits that section: its title and range.
    if let Some(number) = row
        .key
        .rsplit('/')
        .next()
        .and_then(|k| k.strip_prefix("s:"))
        .and_then(|n| n.parse::<u16>().ok())
        && let Some(meta) = actions::find("section.edit")
    {
        let mut item = actions::item_of(meta, model);
        item.params = serde_json::json!({ "number": number });
        items.push(item);
    }
    items
}

/// The canvas context menu's entry for the connection point `cp` (its ID) under the pointer: "Mark -102F2:13
/// not used", or "Mark -102F2:13 used" for a point marked not used (`cp.unused`), disabled with its reason in a
/// read-only project; `None` for a point the page on show does not draw.
pub fn cp_menu(model: &ShellModel, cp: ew_api::output::Uuid) -> Option<ActionItem> {
    let point = model
        .canvas()?
        .page()
        .tables
        .cps
        .iter()
        .find(|c| c.id == cp)?
        .clone();
    let unused = model.cp_unused(point.function, point.idx);
    let mut item = actions::item_of(actions::find("cp.unused")?, model);
    item.params = serde_json::json!({
        "function": point.function.to_string(),
        "idx": point.idx,
        "on": !unused,
    });
    let label = if unused {
        key!("canvas-cp-used")
    } else {
        key!("canvas-cp-unused")
    };
    item.label = strings::format(label, &[&point.label]);
    Some(item)
}

/// A navigator row's context menu, what the same thing offers on the page: on a Potentials row with a definition
/// point (`NavNode::define_at`), Define potential… at it (`potential.draw_row`); on a Terminal strips row of a strip,
/// Add terminal, Choose part for its terminals without one and Edit strip; on a device's, a PLC channel's, a
/// cable's and a page's row their entries (`ShellModel::row_fits`). Each disabled with its reason where it
/// cannot run. Empty when the row offers nothing: the window then opens no menu.
pub fn navigator_row_menu(
    model: &ShellModel,
    kind: crate::navigators::NavKind,
    node: &crate::navigators::NavNode,
) -> Vec<ActionItem> {
    use crate::navigators::{NavKind, NavRole};
    // Each aimed at the row and enabled as the dispatch with its parameters is (the one gate).
    let item = |id: &'static str, params: serde_json::Value, label: Key<'static>| {
        model
            .aimed(id, params, strings::text(label).to_owned())
            .map(|mut item| {
                item.shortcut = String::new();
                item.topic = None;
                item
            })
    };
    match (kind, node.role, node.records.first()) {
        (NavKind::Potentials, _, _) if node.define_at.is_some() => item(
            "potential.draw_row",
            serde_json::json!({ "row": node.key }),
            key!("action-potential-draw-row"),
        )
        .into_iter()
        .collect(),
        (NavKind::Strips, NavRole::Strip, Some(strip)) => {
            let bare = model.strip_terminals_without_part(*strip).len();
            let strip = serde_json::json!({ "strip": strip.to_string() });
            let choose = item("part.choose", strip.clone(), key!("action-part-choose")).map(|mut choose| {
                choose.label = strings::format_count(crate::count_key!("canvas-choose-part-strip"), bare);
                choose
            });
            [
                item("tool.place_terminal", strip.clone(), key!("canvas-add-terminal")),
                choose,
                item("strip.edit", strip, key!("canvas-edit-strip")),
            ]
            .into_iter()
            .flatten()
            .collect()
        }
        // A device the layout cannot place to size: Give it a size… opens its part in Parts (F23).
        (NavKind::Unplaced, NavRole::Device, Some(device))
            if node.badges.iter().any(|b| b == "unknown size") =>
        {
            match model.device_part(*device) {
                Some(copy) => item(
                    "parts.open",
                    serde_json::json!({ "part": format!("id:{copy}") }),
                    key!("action-part-give-size"),
                )
                .into_iter()
                .collect(),
                None => model.row_fits(kind, node),
            }
        }
        _ => model.row_fits(kind, node),
    }
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
        "box_pin" => "box.add_cp",
        "paste" => "edit.paste_clip",
        "layout_place" => "tool.place_layout",
        "frame" => "frame.edit",
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
    /// The quick tools: the three, and the Tube tool's Line kind while it draws tubes.
    pub quick: Vec<&'static str>,
    /// What the tool works on (the predicted tag or terminal of Place), when it says.
    pub target: Option<String>,
    /// The prompt: the selection and its keys, the pick stack, or the tool's target and keys.
    pub prompt: String,
    /// How many objects a click under the pointer can pick (the pick stack; Select only, else 0).
    pub stack: usize,
    /// The prompt while the IP tool's name field holds the keys: its key part says that Enter ends the name first,
    /// since D is then a letter of the name, not the tool's key. `None` for
    /// another tool, or while a refusal stands in the prompt.
    pub field_prompt: Option<String>,
    /// The ways on the prompt names, as buttons after it:
    /// a layout page's Give location…, Unplaced devices and Auto-fill rail…, a list's pages beside.
    pub buttons: Vec<HudButton>,
}

/// A button of the HUD's prompt: its words and the action it runs with its parameters.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HudButton {
    /// The button's words.
    pub label: String,
    /// The action.
    pub action: &'static str,
    /// Its parameters.
    pub params: serde_json::Value,
}

/// The number of objects in the pick stack under the focused canvas's pointer, 0 without a pointer.
pub fn pointer_stack(model: &ShellModel) -> usize {
    let Some(c) = model.canvas() else { return 0 };
    let Some(at) = c.pointer() else { return 0 };
    let k = c.points_per_nm();
    let reach = if k.is_finite() && k > 0.0 {
        crate::pointer::HOVER_SLOP_PT / k
    } else {
        0.0
    };
    let page = c.page();
    tools::pick_stack(page, &page.snapper, at, reach).len()
}

/// The first shortcut of action `id` in its keycap form ("Esc", "Del", "Up", "Ctrl+Z"; [`crate::keymap::short_display`]),
/// or an empty text: the one form every text of the window names a key by (doc 16a §8 item 7).
pub(crate) fn key_of(model: &ShellModel, id: &str) -> String {
    actions::find(id)
        .map(|m| crate::keymap::short_display(&model.shortcut_text(m)))
        .unwrap_or_default()
}

/// The key a hint, a HUD prompt, a toast or a reason names for action `id`: its
/// first shortcut in the keycap form of [`key_of`], or "(no key)" when the keymap binds it to none.
pub(crate) fn key_name(model: &ShellModel, id: &str) -> String {
    or_no_key(key_of(model, id))
}

/// `key`, or "(no key)" when it is empty.
fn or_no_key(key: String) -> String {
    if key.is_empty() {
        strings::text(key!("key-none")).to_owned()
    } else {
        key
    }
}

/// The key of the HUD's text for an interruption point's role.
fn ip_role_key(d: crate::tools::IpDir) -> Key<'static> {
    match d {
        crate::tools::IpDir::Source => key!("hud-ip-source"),
        crate::tools::IpDir::Target => key!("hud-ip-target"),
        crate::tools::IpDir::Undirected => key!("hud-ip-undirected"),
    }
}

/// The HUD of `model` (doc 16a §5.5).
pub fn hud(model: &ShellModel) -> HudModel {
    let tool = model.tool();
    let name = tool.name();
    let action = tool_action(name);
    let mut target = None;
    let mut stack = 0;
    let mut field_prompt = None;
    let mut buttons = Vec::new();
    let hint = || crate::status::hint(model);
    // The Tube tool (the Wire tool on a fluid page) says what it draws first: its line kind and tube.
    let tube = name == "wire" && model.wire_kind() == Ok(Some("fluid"));
    let prompt = match model.tool_ghost() {
        None if name == "select" && tool.state() == "idle" => select_prompt(model, &mut stack, &mut buttons),
        None => hint(),
        // Doc 16a §5.5: "Place -X1031:21 · R rotates · V variant · Esc done": the predicted tag or terminal (the symbol
        // until the pointer gives it one), then the keys; the ghost's line in `ui.describe` keeps the rest.
        // While autoconnect proposes wires, the count and the way to place without them come before the keys,
        // so the strip says it before the first placement does.
        Some(Ghost::Place {
            symbol,
            strip,
            tag,
            hints,
            joinable,
            ..
        }) => place_prompt(model, &mut target, symbol, strip, tag, hints, joinable),
        // The move's HUD names connect on move's key and the session's state, which the key turns at once
        // (C2).
        Some(Ghost::Move { readout, .. }) => {
            let state = strings::format(
                if model.connect_on_move() {
                    key!("hud-connect-on-move-on")
                } else {
                    key!("hud-connect-on-move-off")
                },
                &[&key_name(model, "tool.connect_on_move")],
            );
            format!("{readout} \u{b7} {state}")
        }
        Some(Ghost::Paste { words, .. }) => format!("{words} \u{b7} {}", hint()),
        // The marks a click joins to pins, each with its pin, before the keys ("L connects to -Q1:1 · Click …").
        Some(Ghost::Supply { joins, .. }) => joins
            .iter()
            .map(|(mark, pin)| strings::format(key!("hud-supply-connects"), &[mark, pin]))
            .chain([hint()])
            .collect::<Vec<_>>()
            .join(" \u{b7} "),
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
            connects,
            ..
        }) => ip_prompt(
            model,
            &mut target,
            &mut field_prompt,
            name,
            direction,
            star,
            external,
            connects,
        ),
        // The count says what it counts ("7 connection points"), never a bare "· 7".
        Some(Ghost::Box { cps, .. }) => format!(
            "{} \u{b7} {}",
            hint(),
            strings::format_count(crate::count_key!("count-box-pins"), cps)
        ),
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
                    key!("graphic-anticlockwise")
                } else {
                    key!("graphic-clockwise")
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
        // The placing prompt (doc 16a §5.5); a refused drop's text stands in its chip.
        Some(Ghost::Layout { device, .. }) => strings::format(
            key!("hud-layout-place"),
            &[&device, &key_name(model, crate::tools::ESCAPE)],
        ),
        // The draw tools' line; the length stands at the line.
        Some(Ghost::Line { .. }) => hint(),
    };
    // A tool's refusal stands in the HUD in place of its prompt, also over a ghost (the stacked twin's
    // "An identical symbol already stands here"), until the next canvas input ends it.
    // An open paste choice card's prompt stands in place of the tool's, following the card.
    let prompt = match model.tool_notice() {
        Some(notice) => {
            field_prompt = None;
            notice.to_owned()
        }
        None => model.card_prompt().unwrap_or(prompt),
    };
    let prompt = if tube {
        format!("{} \u{b7} {prompt}", model.tube_words())
    } else {
        prompt
    };
    // The Tube tool's mode reads Tube, as its button (a re-test read "Wire").
    let (action, tool_label) = match actions::find("tool.fluid_line").filter(|_| tube) {
        Some(a) => (
            a.id,
            strings::lookup(&a.label_key()).unwrap_or_default().to_owned(),
        ),
        None => (action, tools::tool_label(name)),
    };
    // Edit frame: the card's sentence as its target, and its HUD's Split, Join, Picture… and Done.
    if name == "frame" {
        target = model.frame_facts().map(|f| model.frame_card(&f));
        buttons.extend(model.frame_hud_buttons());
    }
    HudModel {
        tool: action,
        tool_label,
        icon: actions::find(action).and_then(|a| a.icon).unwrap_or("pointer"),
        quick: if tube {
            QUICK_TOOLS.iter().copied().chain(["fluid.tube_kind"]).collect()
        } else {
            QUICK_TOOLS.to_vec()
        },
        target,
        prompt,
        stack,
        field_prompt,
        buttons,
    }
}

/// The HUD's prompt of the IP tool: the point's name (or the proposal's reason), its role and the key that changes it,
/// its external reference and the pin a click connects it to; `target` the name, `field_prompt` the line while the
/// name field holds the keys.
#[allow(clippy::too_many_arguments)]
fn ip_prompt(
    model: &ShellModel,
    target: &mut Option<String>,
    field_prompt: &mut Option<String>,
    name: String,
    direction: &'static str,
    star: bool,
    external: Option<String>,
    connects: Option<String>,
) -> String {
    *target = Some(name.clone());
    // A proposal other than its base's name says so in the name's place.
    let mut parts = vec![model.tool().proposal_note().unwrap_or(name)];
    // The role and the key that changes it ("0V · source: where the potential comes from … · D: target"), so the
    // next role is on screen; the role says what it does, the key's part keeps the short next word.
    // A star source is a source point (the tool shows a starred point as a source), so the
    // star's hint stands alone in the role's place.
    let mut key_part = None;
    if let Some(d) = crate::tools::IpDir::from_word(direction) {
        // The other half of a name that has one says so: "next: a target, where 24V goes on".
        let role = if star {
            strings::text(key!("hud-ip-star")).to_owned()
        } else {
            model
                .tool()
                .role_note()
                .unwrap_or_else(|| strings::text(ip_role_key(d)).to_owned())
        };
        parts.push(role);
        let (key, next) = (key_name(model, "tool.ip_direction"), d.next().word());
        key_part = Some((
            parts.len(),
            strings::format(key!("hud-ip-field-keys"), &[&key, next]),
        ));
        parts.push(format!("{key}: {next}"));
    } else {
        parts.push(direction.to_owned());
    }
    parts.extend(external);
    // A click on a pin connects the point; the HUD says it before the click.
    if let Some(cp) = connects {
        parts.push(strings::format(key!("hud-ip-connects"), &[&cp]));
    }
    // While the name field holds the keys, the key part says how to give them back.
    if let Some((i, words)) = key_part {
        let mut typing = parts.clone();
        typing[i] = words;
        typing.retain(|p| !p.is_empty());
        *field_prompt = Some(typing.join(" \u{b7} "));
    }
    // An empty part (a name not yet computed) never joins the HUD.
    parts.retain(|p| !p.is_empty());
    parts.join(" \u{b7} ")
}

/// The HUD's prompt of the Place tool (doc 16a §5.5): the predicted tag or terminal, else the symbol in words, the
/// wires its autoconnect hints draw, then the keys; `target` the tag or terminal it names.
fn place_prompt(
    model: &ShellModel,
    target: &mut Option<String>,
    symbol: String,
    strip: Option<String>,
    tag: Option<String>,
    hints: usize,
    joinable: bool,
) -> String {
    // A terminal of a strip without a tag yet reads in words until the preview predicts its tag, never as
    // "-?X?:1".
    *target = tag.or_else(|| {
        strip.map(|text| match text.rsplit_once(':') {
            Some((strip, designation)) if strip.contains('?') => strings::format(
                key!("place-target-terminal"),
                &[strings::text(key!("place-target-new-strip")), designation],
            ),
            _ => text,
        })
    });
    // The symbol by its name in words (`Contact, normally open`); without a cached catalogue its key's last
    // segment in sentence case (`Contact no`), never the key.
    let mut what = target.clone().unwrap_or_else(|| {
        model
            .symbol_title(&symbol)
            .unwrap_or_else(|| strings::choice_words(&crate::tools::symbol_name(&symbol).replace('-', "_")))
    });
    if hints > 0 {
        let count = strings::format_count(crate::count_key!("count-wires"), hints);
        what = format!(
            "{what} \u{b7} {}",
            strings::format(
                key!("hud-place-wires"),
                &[&count, &key_name(model, "canvas.activate_alt")]
            )
        );
    }
    // "J Join device…" while the symbol can be an auxiliary function, before "Esc done".
    let keys = if joinable {
        strings::format(
            key!("hud-keys-place-join"),
            &[
                &key_name(model, "tool.rotate_cw"),
                &key_name(model, "tool.next_variant"),
                &key_name(model, "place.join"),
                &key_name(model, crate::tools::ESCAPE),
            ],
        )
    } else {
        strings::format(
            key!("hud-keys-place"),
            &[
                &key_name(model, "tool.rotate_cw"),
                &key_name(model, "tool.next_variant"),
                &key_name(model, crate::tools::ESCAPE),
            ],
        )
    };
    format!("{what} \u{b7} {keys}")
}

/// The HUD's prompt of Select at rest: the stack under the pointer, what the selection allows, the page's ways on as
/// buttons, or why the page takes no symbols.
fn select_prompt(model: &ShellModel, stack: &mut usize, buttons: &mut Vec<HudButton>) -> String {
    let hint = || crate::status::hint(model);
    // Where a drop left the pointer, the stack is the dropped item and what it sits on; the HUD names
    // it again once the pointer moves on.
    if !model.stack_quiet() {
        *stack = pointer_stack(model);
    }
    let selected = model.canvas_selected();
    if *stack > 1 {
        strings::format(key!("hud-stack"), &[&stack.to_string()])
    } else if selected > 0 && model.selected_placements() > 0 {
        // A placement selected on a panel page (doc 16a §5.5), its keys from the keymap.
        // Only the keys the selected placement takes: R on the plate, X on a rail, Delete always.
        let delete = key_name(model, "edit.delete_selection");
        let on_rail = model
            .selected_placement()
            .map(|(panel, row)| row.carrier.and_then(|c| panel.carrier(c)).is_some());
        match on_rail {
            Some(true) => strings::format(
                key!("hud-layout-selected-rail"),
                &[&selected.to_string(), &key_name(model, "tool.mirror_x"), &delete],
            ),
            Some(false) => strings::format(
                key!("hud-layout-selected-plate"),
                &[&selected.to_string(), &key_name(model, "tool.rotate_cw"), &delete],
            ),
            None => strings::format(key!("hud-layout-selected"), &[&selected.to_string(), &delete]),
        }
    } else if selected > 0
        && let Some((wire, _)) = model.wire_handles()
    {
        // One wire selected: how its handles are reached by keyboard, and with one in the focus, how
        // it moves (Select's hint); a wire does not rotate.
        if model.handle_focus().is_some_and(|w| w == wire) {
            hint()
        } else {
            strings::format(
                key!("hud-wire-selected"),
                &[
                    &selected.to_string(),
                    &key_name(model, "tool.next_proposal"),
                    &key_name(model, "edit.delete_selection"),
                ],
            )
        }
    } else if selected > 0 {
        let line = strings::format(
            key!("hud-selected"),
            &[
                &selected.to_string(),
                &key_name(model, "tool.rotate_cw"),
                &key_name(model, "edit.delete_selection"),
            ],
        );
        // A terminal says which of its clamps is the cabinet's side.
        match model.terminal_sides_hint() {
            Some(sides) => format!("{line} · {sides}"),
            None => line,
        }
    } else if model
        .page_rows()
        .get(model.page_index())
        .is_some_and(|r| r.kind == ew_api::PageType::BusTopology)
    {
        // A bus-topology page says its three steps where the user looks.
        strings::text(key!("hud-bus-page-steps")).to_owned()
    } else if model.page_rows().get(model.page_index()).map(|r| &r.kind)
        == Some(&ew_api::PageType::PanelLayout)
    {
        // A layout page names its first step while it shows no plate,
        // and the Select hint once it does.
        // Its ways on are buttons beside the words (the rail's icons have no words).
        let button = |label: String, action: &'static str, params: serde_json::Value| HudButton {
            label,
            action,
            params,
        };
        let unplaced = || {
            button(
                strings::text(key!("nav-title-unplaced")).to_owned(),
                "navigator.show",
                serde_json::json!({ "kind": "unplaced" }),
            )
        };
        // The cables' glands that wait (doc 12 §3.9): placed on a side, the top or the bottom's own page, else that
        // page first.
        let glands = |t: &ew_api::output::PanelTables, buttons: &mut Vec<HudButton>| {
            let (n, wall) = model.glands_waiting(t.surface);
            if n == 0 {
                return None;
            }
            buttons.push(if wall {
                button(
                    strings::text(key!("action-layout-glands-place")).to_owned(),
                    "layout.glands_place",
                    serde_json::Value::Null,
                )
            } else {
                button(
                    strings::text(key!("hud-layout-bottom")).to_owned(),
                    "layout.surface_page",
                    serde_json::json!({ "surface": "bottom" }),
                )
            });
            Some(strings::format_count(crate::count_key!("hud-layout-glands"), n))
        };
        match model.panel_tables() {
            None => strings::text(key!("hud-layout-start")).to_owned(),
            // A door, a side, the top or the bottom: the glands that wait, else Select's hint. The first steps below
            // are the plate's.
            Some(t) if !t.plate => glands(&t, buttons).unwrap_or_else(hint),
            // A plate with no device on it says the next step.
            Some(t) if t.placements.is_empty() && !t.unplaced.is_empty() => {
                buttons.push(unplaced());
                buttons.push(button(
                    strings::text(key!("action-layout-auto-fill-rail")).to_owned(),
                    "layout.auto_fill_rail",
                    serde_json::Value::Null,
                ));
                strings::text(key!("hud-layout-place-next")).to_owned()
            }
            // No device of the cabinet's location yet: the location comes first (Unplaced devices lists only
            // the devices of the location), in one step for the devices drawn without one.
            Some(t) if t.placements.is_empty() && model.has_devices_to_locate() => {
                buttons.push(button(
                    strings::format(key!("hud-layout-give-location"), &[&t.location]),
                    "layout.locate_devices",
                    serde_json::Value::Null,
                ));
                buttons.push(unplaced());
                strings::format(key!("hud-layout-locate-first"), &[&t.location])
            }
            Some(t) if t.placements.is_empty() => {
                strings::format(key!("auto-fill-cause-none"), &[&t.location])
            }
            // Devices of the cabinet that go in a cut-out wait for the door's page (doc 12 §3.8).
            Some(t)
                if t.plate
                    && t.unplaced
                        .iter()
                        .any(|u| u.mount_kind == Some(ew_api::symbols::schema::MountKind::DoorCutout)) =>
            {
                let n = t
                    .unplaced
                    .iter()
                    .filter(|u| u.mount_kind == Some(ew_api::symbols::schema::MountKind::DoorCutout))
                    .count();
                buttons.push(button(
                    strings::text(key!("hud-layout-door")).to_owned(),
                    "layout.surface_page",
                    serde_json::json!({ "surface": "door" }),
                ));
                strings::format_count(crate::count_key!("hud-layout-cutout"), n)
            }
            // The glands that wait first; then, every device placed, Route is the next step while wires have no way
            // or ways no longer hold (doc 12 §12.5).
            Some(t) => match glands(&t, buttons) {
                Some(waiting) => waiting,
                None if t.unplaced.is_empty() => {
                    let due = model
                        .current_folio()
                        .and_then(|f| model.project_session().and_then(|s| s.cabinet_due(f)));
                    let route = || {
                        button(
                            strings::text(key!("action-layout-route")).to_owned(),
                            "layout.route",
                            serde_json::Value::Null,
                        )
                    };
                    match due {
                        Some((_, stale)) if stale > 0 => {
                            buttons.push(route());
                            strings::format_count(crate::count_key!("hud-layout-ways-stale"), stale)
                        }
                        Some((missing, _)) if missing > 0 => {
                            buttons.push(route());
                            strings::text(key!("hud-layout-route-next")).to_owned()
                        }
                        _ => hint(),
                    }
                }
                None => hint(),
            },
        }
    } else if let Some(list) = model.list_pages(model.page_index()) {
        // A list of more than one page says which page this is, with its way to the pages beside it.
        let ways = [
            (list.previous, key!("hud-list-previous")),
            (list.next, key!("hud-list-next")),
        ];
        for (folio, label) in ways {
            if let Some(folio) = folio {
                buttons.push(HudButton {
                    label: strings::format(label, &[&folio]),
                    action: "page.goto",
                    params: serde_json::json!({ "folio": folio }),
                });
            }
        }
        strings::format(
            key!("hud-list-page"),
            &[&list.n.to_string(), &list.of.to_string()],
        )
    } else if let Some(reason) = crate::actions::page_type_reason(model) {
        // Doc 16a P3: on a page that takes no symbols, with no tool armed, the reason stands
        // where the user looks: the page type's, as a greyed drawing tool says it.
        strings::render(&reason)
    } else {
        hint()
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

    /// An action the keymap binds to no key (`layout.nudge` ships with none) is named
    /// "(no key)" in a hint and in the HUD (one form), and a text formatted with it reads whole.
    #[test]
    fn an_action_with_no_key_is_named_no_key() {
        let model = ShellModel::for_tests();
        assert_eq!(key_of(&model, "layout.nudge"), "", "layout.nudge has no shortcut");
        let full = key_name(&model, "layout.nudge");
        assert_eq!(full, "(no key)");
        assert_eq!(
            strings::format(key!("hint-layout-place"), &[&full]),
            "Click or release to place; (no key) ends"
        );
        assert_eq!(
            strings::format(
                key!("hud-layout-selected"),
                &["1", &key_name(&model, "layout.nudge")]
            ),
            "1 selected \u{b7} arrows nudge \u{b7} (no key) deletes"
        );
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
