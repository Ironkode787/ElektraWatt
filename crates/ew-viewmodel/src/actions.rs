// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The action registry (spec section 5.2; UX/UI doc 16 section 2.4) in toolkit-neutral form.
//!
//! Every menu item, palette entry, keyboard shortcut and dialog result of the shell is an action of
//! [`ACTIONS`], dispatched with [`ShellModel::dispatch`]; the headless mode drives the same registry. An action
//! has a stable ID, a label key (the English text lives in [`crate::strings`]), default shortcuts, and an
//! `enabled` predicate that names the reason key of a refusal. [`menu_model`] builds the app menu from the
//! registry, so the egui frame and the `ui.describe` text cannot disagree.
//!
//! The menus (doc 16a §9.2) are the app menu's submenus, File, Edit, then one per command-bar
//! tab (Project, Page, Draw, PLC, Fluid, Check, Output, View), then Window and Help; Insert became Draw and Tools dissolved. An
//! action with a [`Placement`] is a command of the bar ([`crate::command_bar`]) and sits in its tab's menu under its
//! group; a group of more than three commands is a submenu, a smaller one a separated block.
//!
//! This file holds the registry's types, the builders the table is written with, the signatures' grammar and the
//! palette's rows. Not here: the table's entries are in `actions/table/`, the enable predicates in
//! `actions/enabled.rs`, the menu model in `actions/menu.rs`.

use serde_json::{Map, Value};

use crate::alias::ParamForm;
use crate::command_bar::{CommandSize, Group, Placement, Tab, place};
use crate::key;
use crate::shell::{Outcome, ShellModel};
use crate::strings;
use crate::strings::Key;

mod enabled;
mod menu;
mod table;

pub use enabled::enabled_with;
pub(crate) use enabled::{editable_page, page_type_reason};
pub use menu::{
    ActionItem, MenuEntry, MenuModel, group_metas, menu_model, shortcut_text, station_tools, workspace_label,
};
pub(crate) use menu::{heading, item_of, submenu_entries, switch_on};

/// An action ID, such as `file.open` (the `&ActionId` of `dispatch` is a `&str`).
pub type ActionId = str;

/// Whether an action changes what is shown or the project.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActionKind {
    /// Changes only what is shown (page, popup, scale, dialog).
    View,
    /// Changes the project, its files or the application's lifetime.
    Command,
}

/// The submenus of the app menu, in order (doc 16a §9.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Menu {
    /// File.
    File,
    /// Edit.
    Edit,
    /// Project: the Project tab's groups.
    Project,
    /// Page: the Page tab's groups.
    Page,
    /// Draw: the Draw tab's groups (the former Insert menu and the tools).
    Draw,
    /// PLC: the PLC tab's groups.
    Plc,
    /// Fluid: the Fluid tab's groups.
    Fluid,
    /// Check: the Check tab's groups.
    Check,
    /// Output: the Output tab's groups.
    Output,
    /// View: the View tab's groups.
    View,
    /// Window: the canvas tabs.
    Window,
    /// Help.
    Help,
}

impl Menu {
    /// The menus in the app menu's order.
    pub const ALL: [Menu; 12] = [
        Menu::File,
        Menu::Edit,
        Menu::Project,
        Menu::Page,
        Menu::Draw,
        Menu::Plc,
        Menu::Fluid,
        Menu::Check,
        Menu::Output,
        Menu::View,
        Menu::Window,
        Menu::Help,
    ];

    /// The key of the menu's label.
    pub fn label_key(self) -> Key<'static> {
        match self {
            Menu::File => key!("menu-file"),
            Menu::Edit => key!("menu-edit"),
            Menu::Project => key!("menu-project"),
            Menu::Page => key!("menu-page"),
            Menu::Draw => key!("menu-draw"),
            Menu::Plc => key!("menu-plc"),
            Menu::Fluid => key!("menu-fluid"),
            Menu::Check => key!("menu-check"),
            Menu::Output => key!("menu-output"),
            Menu::View => key!("menu-view"),
            Menu::Window => key!("menu-window"),
            Menu::Help => key!("menu-help"),
        }
    }

    /// The command-bar tab whose groups the menu shows, if any.
    pub fn tab(self) -> Option<Tab> {
        Tab::ALL.into_iter().find(|t| t.menu() == self)
    }
}

/// The submenus of the menu bar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Submenu {
    /// File, Open recent: one item per recent project.
    OpenRecent,
    /// View, UI scale: one item per scale.
    UiScale,
    /// View, Theme: one item per theme.
    Theme,
    /// View, Density: Comfortable and Compact.
    Density,
    /// View, Workspace: one item per workspace, then save and reset.
    Workspace,
    /// View, Pan: left, right, up, down.
    Pan,
    /// View, Page navigator: its three views and two sort orders.
    PageNavigator,
    /// View, Navigators: one checked item per navigator kind (`navigator.show`).
    Navigators,
    /// Insert, Graphic: one item per shape of the Graphic tool.
    Graphic,
    /// Draw, Connect, Wire markers: the marker mode's four settable modes and the as-built one.
    WireMarkers,
    /// View, Layout, Command tab: one item per tab of the command bar.
    CommandTab,
    /// Draw, Connect, Bond: one item per bond kind of `bond.create` (G-PP triage row 7).
    Bond,
    /// Check, Revision: Approve, Release, Withdraw, Set author and the incomplete release.
    Revision,
    /// View, Language: one item per UI language, under its own name.
    Language,
    /// View, Minimum text size: off, then one item per size of `MIN_FONT_CHOICES`.
    MinFont,
    /// Help, Tutorials: one item per tutorial of [`tutorials`], each with its own start.
    Tutorials,
    /// Help, Guides: one item per how-to guide of [`guides`], each opening its page.
    Guides,
    /// PLC, Station, Rack and slots: one item per PLC station.
    RackEditor,
    /// PLC, Station, Attach cards: one item per station that takes cards.
    AttachCards,
    /// A station's view, Replace: one item per part of the view's catalogue that fits where the cursor stands.
    ModuleParts,
    /// A station's view, Draw on a page: one item per schematic page, then a new page.
    ModulePages,
    /// PLC, Addresses: the addressing commands of each configuration project
    /// ([`ShellModel::plc_addressing_items`]).
    PlcAddressing,
    /// PLC, Addresses, Symbol policy: one item per configuration project.
    SymbolPolicy,
    /// PLC, Lists, Import instrument list: one item per configuration project.
    InstrumentList,
    /// PLC, Bus, Place port: Put a station on this page with one item per station or bus device the bus page does not
    /// draw yet, then one item per port the Bus panel lists as not drawn yet.
    BusPorts,
}

/// The guide's table of contents, the one place its tutorials and how-to guides are listed (FT-1715).
const BOOK_SUMMARY: &str = include_str!("../../../docs/book/src/SUMMARY.md");

/// The pages of the guide's directory `dir` (`tutorials`) in its order, read from its table of contents: each its
/// page's name and English title, the directory's index left out.
fn book_pages(dir: &'static str) -> impl Iterator<Item = (&'static str, &'static str)> {
    BOOK_SUMMARY.lines().filter_map(move |line| {
        let (title, rest) = line.trim().strip_prefix("- [")?.split_once("](")?;
        let name = rest.strip_prefix(dir)?.strip_prefix('/')?.strip_suffix(".md)")?;
        (name != "index").then_some((name, title))
    })
}

/// The tutorials of the guide in its order: each its page's name, which `help.tutorials` takes as `tutorial` and whose
/// label is `tutorial-<name>`, and the page's English title.
pub fn tutorials() -> impl Iterator<Item = (&'static str, &'static str)> {
    book_pages("tutorials")
}

/// The how-to guides of the guide in its order (F5): each its page's name, which `help.guide` takes as `guide` and
/// whose label is `guide-<name>`, and the page's English title.
pub fn guides() -> impl Iterator<Item = (&'static str, &'static str)> {
    book_pages("how-to")
}

impl Submenu {
    /// The key of the submenu's label.
    pub fn label_key(self) -> Key<'static> {
        match self {
            Submenu::OpenRecent => key!("submenu-open-recent"),
            Submenu::UiScale => key!("submenu-ui-scale"),
            Submenu::Theme => key!("submenu-theme"),
            Submenu::Density => key!("submenu-density"),
            Submenu::Workspace => key!("submenu-workspace"),
            Submenu::Pan => key!("submenu-pan"),
            Submenu::PageNavigator => key!("submenu-page-navigator"),
            Submenu::Navigators => key!("submenu-navigators"),
            Submenu::Graphic => key!("submenu-graphic"),
            Submenu::WireMarkers => key!("submenu-wire-markers"),
            Submenu::Bond => key!("submenu-bond"),
            Submenu::CommandTab => key!("submenu-command-tab"),
            Submenu::Revision => key!("submenu-revision"),
            Submenu::Language => key!("submenu-language"),
            Submenu::MinFont => key!("submenu-min-font"),
            Submenu::Tutorials => key!("submenu-tutorials"),
            Submenu::Guides => key!("submenu-guides"),
            Submenu::RackEditor => key!("action-plc-rack-editor"),
            Submenu::AttachCards => key!("action-plc-attach-cards"),
            Submenu::ModuleParts => key!("action-plc-module-replace"),
            Submenu::ModulePages => key!("action-plc-module-draw"),
            Submenu::PlcAddressing => key!("submenu-plc-addressing"),
            Submenu::SymbolPolicy => key!("action-plc-symbol-policy"),
            Submenu::InstrumentList => key!("action-plc-instrument-list"),
            Submenu::BusPorts => key!("action-bus-place-port"),
        }
    }
}

/// Where an action's keyboard shortcuts apply: the window binds a context's chords only while the
/// shell's focus is in that context; a context's binding shadows a global one with the same chord.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Context {
    /// Everywhere.
    Global,
    /// While a canvas has the focus (the grid cursor, following links).
    Canvas,
    /// While the page navigator has the focus.
    Navigator,
    /// While the property grid has the focus.
    Properties,
    /// While the find pane has the focus.
    Find,
}

/// Where an action sits in the app menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MenuSlot {
    /// The menu.
    pub menu: Menu,
    /// The command-bar group inside a tab's menu, if any: the group's commands first, then the
    /// group's menu-only actions in registry order.
    pub group: Option<Group>,
    /// The submenu inside it, if any.
    pub submenu: Option<Submenu>,
    /// Whether a separator is drawn before the item (or the submenu, at its first item).
    pub separator_before: bool,
}

/// What an action does when it runs: one call into the shell with the action's parameters, after the dispatch checked
/// that it is enabled and resolved its aliases.
pub(crate) type Handler = fn(&mut ShellModel, &Map<String, Value>) -> Outcome;

/// One action of the registry.
///
/// A dependant reads an action's facts but cannot reach its handler: an action runs only through
/// [`ShellModel::dispatch`], after the enabled check and the alias resolution. The intent is checked at compile time:
///
/// ```compile_fail
/// let _handler = ew_viewmodel::actions::ACTIONS[0].handler;
/// ```
#[derive(Clone, Copy)]
pub struct ActionMeta {
    /// The stable ID (`file.open`).
    pub id: &'static str,
    /// View or command.
    pub kind: ActionKind,
    /// The menu the palette groups the action under (also the menu it lives in, when it has a slot).
    pub category: Menu,
    /// The default shortcuts (`Ctrl+Shift+Z`); `Ctrl` reads `Cmd` on macOS. The first is shown in menus.
    pub shortcut: &'static [&'static str],
    /// Whether the action can run now: `Err` carries the reason key of its refusal.
    pub enabled: fn(&ShellModel) -> Result<(), Key<'static>>,
    /// The place in the menu bar; `None` for palette-only and headless-only actions.
    pub menu: Option<MenuSlot>,
    /// The action exists only in the headless mode (no menu entry, not in the palette).
    pub headless_only: bool,
    /// The palette cannot supply this action's parameters, so its entry is disabled there.
    pub needs_parameters: bool,
    /// The action is part of a screen's own controls (the wizard, the first-run flow, a setting): no menu entry, no
    /// palette entry, no line in the shortcut list; the window and the headless mode dispatch it.
    pub hidden: bool,
    /// Where its shortcuts apply.
    pub context: Context,
    /// The parameter signature: `""` for an action without parameters, else a comma-separated list of
    /// `name[?]:type` (see `validate_signature`). The blind drafter's harness types its `key=value` arguments by it.
    pub params: &'static str,
    /// The place in the command bar; `None` for an action the bar does not show.
    pub placement: Option<Placement>,
    /// A second place in the command bar, on another tab, where people look for it too (Report on Page and Output);
    /// the app menu and the palette keep the first.
    pub also: Option<Placement>,
    /// The icon key (`ew-ui` maps it to its icon table; doc 16a §4.11); required for a placed action.
    pub icon: Option<&'static str>,
    /// Dispatching the action leaves the banner standing (popups, dialogs, help, the describe text, the canvas's view
    /// and tool input, the panes' own moves); every other action clears it.
    pub keeps_banner: bool,
    /// The action goes while the recovery dialog is open: its answers, Escape, what only observes (the
    /// describe text, F1) or ends the run (Quit closes the read-only project; the journal stays), and the window's own
    /// canvas geometry (`view.canvas_resize`, which the frame sends until the shell has the pane's size: refused, it
    /// would be sent again every frame). Any other action refuses `reason-dialog-open`.
    pub answers_recovery: bool,
    /// Dispatching the action leaves the link hover preview standing: the rest that shows it, the describe text, the
    /// snapshots and Escape (which closes it as the topmost popup); every other action (a pointer move, a cursor move,
    /// a view action) clears it.
    pub keeps_link_preview: bool,
    /// The parameters that take a UUID or a reference, each with the form it takes it in: an alias `@n` in one of
    /// them resolves before the action reads it ([`crate::alias::resolve_params`]).
    pub aliases: &'static [(&'static str, ParamForm)],
    /// What the action does (run by [`ShellModel::dispatch`] once the action is enabled).
    pub(crate) handler: Handler,
}

impl ActionMeta {
    /// The key of the label in [`crate::strings`]: `action-` and the ID with `.` and `_` as `-`
    /// (`file.open_recent` reads `action-file-open-recent`).
    pub fn label_key(&self) -> String {
        format!("action-{}", self.id.replace(['.', '_'], "-"))
    }
}

impl std::fmt::Debug for ActionMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActionMeta")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

const fn slot(menu: Menu) -> Option<MenuSlot> {
    Some(MenuSlot {
        menu,
        group: None,
        submenu: None,
        separator_before: false,
    })
}

const fn slot_sep(menu: Menu) -> Option<MenuSlot> {
    Some(MenuSlot {
        menu,
        group: None,
        submenu: None,
        separator_before: true,
    })
}

const fn sub(menu: Menu, submenu: Submenu, separator_before: bool) -> Option<MenuSlot> {
    Some(MenuSlot {
        menu,
        group: None,
        submenu: Some(submenu),
        separator_before,
    })
}

/// A menu-only action in a command-bar group of its tab's menu.
const fn grp(group: Group) -> Option<MenuSlot> {
    Some(MenuSlot {
        menu: group.tab().menu(),
        group: Some(group),
        submenu: None,
        separator_before: false,
    })
}

/// A menu-only action in a submenu of a command-bar group.
const fn grp_sub(group: Group, submenu: Submenu) -> Option<MenuSlot> {
    Some(MenuSlot {
        menu: group.tab().menu(),
        group: Some(group),
        submenu: Some(submenu),
        separator_before: false,
    })
}

/// Places an action in the command bar with its icon; its app-menu slot becomes its group in its tab's menu (a
/// submenu it had stays: the bar's button opens it as a menu).
const fn placed(placement: Placement, icon: &'static str, mut a: ActionMeta) -> ActionMeta {
    let submenu = match a.menu {
        Some(s) => s.submenu,
        None => None,
    };
    a.menu = Some(MenuSlot {
        menu: placement.tab.menu(),
        group: Some(placement.group),
        submenu,
        separator_before: false,
    });
    a.placement = Some(placement);
    a.icon = Some(icon);
    a
}

/// Gives a placed action a second place in the bar, on another tab ([`ActionMeta::also`]).
const fn placed_also(placement: Placement, mut a: ActionMeta) -> ActionMeta {
    a.also = Some(placement);
    a
}

/// Gives an action that the bar does not place an icon (quick access, the HUD's tools).
const fn with_icon(icon: &'static str, mut a: ActionMeta) -> ActionMeta {
    a.icon = Some(icon);
    a
}

const fn act(
    id: &'static str,
    kind: ActionKind,
    category: Menu,
    shortcut: &'static [&'static str],
    enabled: fn(&ShellModel) -> Result<(), Key<'static>>,
    menu: Option<MenuSlot>,
    handler: Handler,
) -> ActionMeta {
    ActionMeta {
        id,
        kind,
        category,
        shortcut,
        enabled,
        menu,
        headless_only: false,
        needs_parameters: false,
        hidden: false,
        context: Context::Global,
        params: "",
        placement: None,
        also: None,
        icon: None,
        keeps_banner: false,
        answers_recovery: false,
        keeps_link_preview: false,
        aliases: &[],
        handler,
    }
}

/// Gives an action its parameter signature.
const fn with_params(params: &'static str, mut a: ActionMeta) -> ActionMeta {
    a.params = params;
    a
}

/// Names the parameters of an action that take a UUID or a reference.
const fn with_aliases(aliases: &'static [(&'static str, ParamForm)], mut a: ActionMeta) -> ActionMeta {
    a.aliases = aliases;
    a
}

const fn needs_parameters(mut a: ActionMeta) -> ActionMeta {
    a.needs_parameters = true;
    a
}

/// Marks an action whose dispatch leaves the banner standing.
const fn keeps_banner(mut a: ActionMeta) -> ActionMeta {
    a.keeps_banner = true;
    a
}

/// Marks an action that goes while the recovery dialog is open.
const fn answers_recovery(mut a: ActionMeta) -> ActionMeta {
    a.answers_recovery = true;
    a
}

/// Marks an action whose dispatch leaves the link hover preview standing.
const fn keeps_link_preview(mut a: ActionMeta) -> ActionMeta {
    a.keeps_link_preview = true;
    a
}

const fn headless(mut a: ActionMeta) -> ActionMeta {
    a.headless_only = true;
    a
}

const fn hidden(mut a: ActionMeta) -> ActionMeta {
    a.hidden = true;
    a
}

const fn in_canvas(mut a: ActionMeta) -> ActionMeta {
    a.context = Context::Canvas;
    a
}

const fn in_navigator(mut a: ActionMeta) -> ActionMeta {
    a.context = Context::Navigator;
    a
}

const fn in_properties(mut a: ActionMeta) -> ActionMeta {
    a.context = Context::Properties;
    a
}

const fn in_find(mut a: ActionMeta) -> ActionMeta {
    a.context = Context::Find;
    a
}

use ActionKind::{Command, View};

/// The number of registered actions.
const COUNT: usize = {
    let mut n = 0;
    let mut i = 0;
    while i < table::SECTIONS.len() {
        n += table::SECTIONS[i].len();
        i += 1;
    }
    n
};

/// The sections of the table joined in their order.
static JOINED: [ActionMeta; COUNT] = {
    let mut out = [table::SECTIONS[0][0]; COUNT];
    let (mut n, mut s) = (0, 0);
    while s < table::SECTIONS.len() {
        let mut i = 0;
        while i < table::SECTIONS[s].len() {
            out[n] = table::SECTIONS[s][i];
            n += 1;
            i += 1;
        }
        s += 1;
    }
    out
};

/// Every action, in registry order: the order of menu items, palette rows and the shortcut list. An action is
/// declared once, as an entry of a section under `actions/table/`, naming its handler.
pub static ACTIONS: &[ActionMeta] = &JOINED;

/// One parameter of an action's signature.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParamSpec {
    /// The parameter's name, `[a-z][a-z0-9_]*`.
    pub name: String,
    /// Whether the parameter may be left out (`?`).
    pub optional: bool,
    /// The type, as the signature spells it (`mm`, `enum(a|b)`, `list(text)`, ...).
    pub ty: String,
}

/// The scalar types of the signature grammar; `enum(…)` and `list(…)` are the two constructors.
pub const PARAM_TYPES: [&str; 8] = ["mm", "int", "uint", "bool", "text", "path", "json", "ref"];

fn is_word(w: &str) -> bool {
    let mut chars = w.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn valid_type(ty: &str) -> bool {
    if PARAM_TYPES.contains(&ty) {
        return true;
    }
    if let Some(inner) = ty.strip_prefix("enum(").and_then(|t| t.strip_suffix(')')) {
        return !inner.is_empty() && inner.split('|').all(is_word);
    }
    if let Some(inner) = ty.strip_prefix("list(").and_then(|t| t.strip_suffix(')')) {
        return valid_type(inner);
    }
    false
}

/// Parses a parameter signature by this grammar:
///
/// ```text
/// signature := "" | param (", " param)*
/// param     := name ["?"] ":" type
/// type      := "mm" | "int" | "uint" | "bool" | "text" | "path" | "json" | "ref"
///            | "enum(" word ("|" word)* ")" | "list(" type ")"
/// ```
///
/// # Errors
///
/// A message naming what is wrong: an empty parameter, a name that is not `[a-z][a-z0-9_]*`, a repeated name or a
/// type outside the grammar.
pub fn parse_signature(sig: &str) -> Result<Vec<ParamSpec>, String> {
    if sig.is_empty() {
        return Ok(Vec::new());
    }
    let mut out: Vec<ParamSpec> = Vec::new();
    for part in sig.split(", ") {
        let Some((name, ty)) = part.split_once(':') else {
            return Err(format!("`{part}` has no `:type`"));
        };
        let (name, optional) = match name.strip_suffix('?') {
            Some(n) => (n, true),
            None => (name, false),
        };
        if !is_word(name) {
            return Err(format!("`{name}` is not a parameter name"));
        }
        if !valid_type(ty) {
            return Err(format!("`{ty}` is not a type of the grammar"));
        }
        if out.iter().any(|p| p.name == name) {
            return Err(format!("`{name}` is repeated"));
        }
        out.push(ParamSpec {
            name: name.to_owned(),
            optional,
            ty: ty.to_owned(),
        });
    }
    Ok(out)
}

/// The action with this ID.
pub fn find(id: &ActionId) -> Option<&'static ActionMeta> {
    ACTIONS.iter().find(|a| a.id == id)
}

/// An entry of the command palette.
#[derive(Clone, PartialEq, Debug)]
pub struct PaletteEntry {
    /// What Enter runs.
    pub run: PaletteRun,
    /// The label (the action's, or the model command's name).
    pub label: String,
    /// The ID (the action's, or the model command's name).
    pub id: String,
    /// The accelerator text, empty when there is none.
    pub shortcut: String,
    /// Whether Enter runs it.
    pub enabled: bool,
    /// The reason text of a disabled entry.
    pub reason: Option<String>,
    /// Where the action lives in the command bar or the app menu ("Draw · Place", "File"; doc 16a §9.3).
    pub breadcrumb: String,
}

/// Where `meta` lives, as the palette shows it (doc 16a §6.14, §9.3): the tab and the group of its command-bar place
/// or of its menu group ("Draw · Place"), else its menu and submenu ("View · Theme"), else its category's menu.
pub fn breadcrumb(meta: &ActionMeta) -> String {
    let group = meta
        .placement
        .map(|p| p.group)
        .or_else(|| meta.menu.and_then(|m| m.group));
    if let Some(g) = group {
        return format!(
            "{} · {}",
            strings::text(g.tab().label_key()),
            strings::text(Key::run_time(&g.label_key()))
        );
    }
    match meta.menu {
        Some(MenuSlot {
            menu,
            submenu: Some(s),
            ..
        }) => format!(
            "{} · {}",
            strings::text(menu.label_key()),
            strings::text(s.label_key())
        ),
        Some(slot) => strings::text(slot.menu.label_key()).to_owned(),
        None => strings::text(meta.category.label_key()).to_owned(),
    }
}

/// [`breadcrumb`] of a palette row labelled `label`, which it never repeats (doc 16a §6.14): "View" on the
/// row "Language", not "View · Language".
pub fn breadcrumb_of(meta: &ActionMeta, label: &str) -> String {
    let crumb = breadcrumb(meta);
    match crumb.rsplit_once(" · ") {
        Some((head, last)) if last == label => head.to_owned(),
        _ => crumb,
    }
}

/// What a palette entry runs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PaletteRun {
    /// An action of the registry.
    Action(&'static str),
    /// A page of the `#` prefix: its folio; Enter shows it (`page.goto`).
    Page(String),
    /// The cell of the `:` prefix: the Go to text; Enter goes there (`nav.goto`, FR-UI-039).
    Cell(String),
    /// An object of the `@` prefix: the record's ID text and the Go to text of its cell or page; Enter selects it
    /// (`select.set`) and shows its page.
    Object {
        /// The record's ID text.
        record: String,
        /// Its cell (`102-B3`) or folio, when it is on a page.
        target: Option<String>,
    },
    /// A rule of the `?` prefix: its ID; Enter shows Messages filtered to it (`messages.show`, `navigator.filter`).
    Rule(String),
    /// A panel of the left rail by its word (`plc`): Enter shows it (`rail.show`), and leaves it open when it is
    /// shown already.
    Panel(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_found() {
        for (i, a) in ACTIONS.iter().enumerate() {
            assert!(
                ACTIONS[i + 1..].iter().all(|b| b.id != a.id),
                "duplicate action id {}",
                a.id
            );
            assert_eq!(find(a.id).map(|m| m.id), Some(a.id));
        }
        assert!(find("no.such.action").is_none());
    }

    /// FT-1715: a tutorial added to the guide reaches Help ▸ Tutorials under its own name in both languages.
    #[test]
    fn every_tutorial_of_the_guide_has_a_label_in_both_languages() {
        let nl = include_str!("../locales/nl-NL/shell.ftl");
        for (name, title) in tutorials() {
            let key = format!("tutorial-{name}");
            assert_eq!(strings::lookup(&key), Some(title), "{key} is the page's title");
            assert!(
                nl.lines().any(|l| l.starts_with(&format!("{key} = "))),
                "{key} has no Dutch label"
            );
        }
        assert!(tutorials().any(|(name, _)| name == "symbol-editor"));
    }

    /// F5: a how-to guide added to the guide reaches Help ▸ Guides under its own name in both languages.
    #[test]
    fn every_guide_of_the_guide_has_a_label_in_both_languages() {
        let nl = include_str!("../locales/nl-NL/shell.ftl");
        for (name, title) in guides() {
            let key = format!("guide-{name}");
            assert_eq!(strings::lookup(&key), Some(title), "{key} is the page's title");
            assert!(
                nl.lines().any(|l| l.starts_with(&format!("{key} = "))),
                "{key} has no Dutch label"
            );
        }
        assert!(guides().any(|(name, _)| name == "plc-workspace"));
        assert!(guides().all(|(name, _)| !tutorials().any(|(t, _)| t == name)));
    }

    #[test]
    fn every_entry_has_an_english_label() {
        for a in ACTIONS {
            let key = a.label_key();
            assert!(
                strings::lookup(&key).is_some(),
                "{} has no English label: strings has no text for {key}",
                a.id
            );
        }
        for m in Menu::ALL {
            assert!(strings::lookup(m.label_key().as_str()).is_some());
        }
        for s in [
            Submenu::OpenRecent,
            Submenu::CommandTab,
            Submenu::UiScale,
            Submenu::Theme,
            Submenu::Density,
            Submenu::Workspace,
            Submenu::Pan,
            Submenu::Graphic,
            Submenu::WireMarkers,
            Submenu::Bond,
            Submenu::Revision,
            Submenu::Language,
            Submenu::MinFont,
            Submenu::Tutorials,
            Submenu::Guides,
        ] {
            assert!(strings::lookup(s.label_key().as_str()).is_some());
        }
    }

    #[test]
    fn shortcuts_are_unique_and_never_ctrl_alt_letter() {
        // Unique within a shell context (a context's binding may shadow a global one): Shift+Enter finishes a tool in the canvas and opens the multilingual popup in the property grid.
        let mut seen: Vec<(Context, &str)> = Vec::new();
        for a in ACTIONS {
            for s in a.shortcut {
                let clash = seen.iter().any(|(c, k)| k == s && *c == a.context);
                assert!(
                    !clash,
                    "shortcut {s} is bound twice in the {:?} context",
                    a.context
                );
                seen.push((a.context, s));
                let parts: Vec<&str> = s.split('+').collect();
                let key = parts.last().unwrap();
                let alt = parts.contains(&"Alt");
                let ctrl = parts.contains(&"Ctrl");
                assert!(
                    !(alt && ctrl && key.len() == 1 && key.chars().all(|c| c.is_ascii_alphabetic())),
                    "{s} is a Ctrl+Alt+letter binding (NFR-I18N-06)"
                );
            }
        }
    }

    #[test]
    fn every_menu_entry_is_a_registry_entry() {
        let model = ShellModel::for_tests();
        for menu in menu_model(&model) {
            for item in menu.items() {
                assert!(find(item.id).is_some(), "{} is not in the registry", item.id);
            }
        }
        let order: Vec<Menu> = menu_model(&model).iter().map(|m| m.menu).collect();
        assert_eq!(order, Menu::ALL);
    }

    #[test]
    fn headless_only_and_hidden_actions_have_no_menu_slot() {
        for a in ACTIONS.iter().filter(|a| a.headless_only || a.hidden) {
            assert!(a.menu.is_none(), "{}", a.id);
        }
    }

    #[test]
    fn the_new_actions_are_in_their_menus() {
        let model = ShellModel::for_tests();
        let menus = menu_model(&model);
        let ids = |menu: Menu| -> Vec<&'static str> {
            let m = menus.iter().find(|m| m.menu == menu).unwrap();
            m.items().iter().map(|i| i.id).collect()
        };
        let file = ids(Menu::File);
        for id in [
            "file.new",
            "file.open_sample",
            "file.discard_journal",
            "file.import",
        ] {
            assert!(file.contains(&id), "{id} in File");
        }
        // An `.ewz` opens through File ▸ Import…, not an entry of its own (FT-1708).
        assert!(!file.contains(&"file.open_ewz"), "no Open .ewz… in File");
        // The imports and the templates sit in the Project tab's menu: Templates…, Save as template…, New template….
        let project = ids(Menu::Project);
        for id in [
            "templates.open",
            "project.save_as_template",
            "tools.company_standard",
        ] {
            assert!(project.contains(&id), "{id} in Project");
        }
        // One File ▸ Import… takes every file; the separate entries are in no menu (FT-1741).
        for id in [
            "file.import_qet",
            "parts.import",
            "file.import_drawing",
            "file.import_eplan",
        ] {
            assert!(!project.contains(&id) && !file.contains(&id), "{id} in no menu");
        }
        let help = ids(Menu::Help);
        for id in [
            "help.documentation",
            "help.tutorials",
            "help.first_run",
            "help.notices",
        ] {
            assert!(help.contains(&id), "{id} in Help");
        }
        let view = ids(Menu::View);
        for id in [
            "view.workspace_load",
            "view.workspace_save",
            "view.workspace_reset",
        ] {
            assert!(view.contains(&id), "{id} in View");
        }
    }

    #[test]
    fn every_signature_parses_with_the_grammar() {
        for a in ACTIONS {
            let parsed = parse_signature(a.params).unwrap_or_else(|e| panic!("{}: {e}", a.id));
            // The canonical spelling: parsing and printing again gives the text.
            let again: Vec<String> = parsed
                .iter()
                .map(|p| format!("{}{}:{}", p.name, if p.optional { "?" } else { "" }, p.ty))
                .collect();
            assert_eq!(again.join(", "), a.params, "{}", a.id);
        }
        assert!(parse_signature("a:nope").is_err());
        assert!(parse_signature("A:int").is_err());
        assert!(
            parse_signature("a:int,b:int").is_err(),
            "the separator is a comma and a space"
        );
        assert!(parse_signature("a:int, a:uint").is_err());
        assert!(parse_signature("a:enum()").is_err());
        assert!(parse_signature("a:list(enum(x|y))").is_ok());
    }

    #[test]
    fn the_signatures_the_harness_relies_on() {
        let sig = |id: &str| find(id).unwrap().params;
        assert_eq!(
            sig("canvas.press"),
            "x_mm:mm, y_mm:mm, button?:enum(primary|middle), shift?:bool, ctrl?:bool, alt?:bool, group?:uint"
        );
        // The as-built parameter of Go to is `text` (the specification's example names `folio` and `cell`).
        assert_eq!(sig("nav.goto"), "text?:text");
        assert_eq!(sig("page.describe"), "folio?:text");
        // The picker's Enter arms the pick with the same optional parameters as `place.pick`.
        assert_eq!(
            sig("picker.choose"),
            "letter?:text, strip?:text, terminal_designation?:text, designation?:text"
        );
        let new = parse_signature(sig("file.new")).unwrap();
        let template = new.iter().find(|p| p.name == "template").unwrap();
        assert_eq!(template.ty, "path");
        // Every action that takes a path types it `path`, so the harness can check it.
        for (id, name) in [
            ("file.open", "dir"),
            ("file.save_as", "dir"),
            ("file.export_pdf", "path"),
            ("navigator.export_table", "path"),
            ("navigator.import_table", "path"),
            ("file.import_qet", "file"),
            ("file.import", "file"),
            ("help.tutorials", "dir"),
            ("wizard.finish", "dir"),
            ("image.file", "path"),
            ("view.snapshot", "path"),
            ("canvas.snapshot", "path"),
            ("canvas.preview_snapshot", "path"),
        ] {
            let params = parse_signature(sig(id)).unwrap();
            let p = params
                .iter()
                .find(|p| p.name == name)
                .unwrap_or_else(|| panic!("{id} {name}"));
            assert_eq!(p.ty, "path", "{id} {name}");
        }
    }
}
