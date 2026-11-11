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
//! tab (Project, Page, Draw, Check, Output, View), then Window and Help; Insert became Draw and Tools dissolved. An
//! action with a [`Placement`] is a command of the bar ([`crate::command_bar`]) and sits in its tab's menu under its
//! group; a group of more than three commands is a submenu, a smaller one a separated block.

use serde_json::{Map, Value, json};

use crate::alias::ParamForm;
use crate::command_bar::{CommandSize, Group, Placement, Tab, place};
use crate::key;
use crate::settings::{DENSITIES, MIN_FONT_CHOICES, THEMES, UI_SCALES, scale_label};
use crate::shell::{Outcome, ShellModel};
use crate::strings;
use crate::strings::Key;
use crate::tools::{Ghost, GraphicShape};

mod table;

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
    pub const ALL: [Menu; 10] = [
        Menu::File,
        Menu::Edit,
        Menu::Project,
        Menu::Page,
        Menu::Draw,
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
    /// Help, Tutorials: one item per tutorial of [`TUTORIALS`], each with its own start.
    Tutorials,
}

/// The tutorials of the guide in its order: each its page's name, which `help.tutorials` takes as
/// `tutorial` and whose label is `tutorial-<name>`.
pub const TUTORIALS: [&str; 5] = [
    "first-page",
    "terminal-strip-and-cable",
    "reports-and-pdf",
    "import-qet",
    "headless-workflow",
];

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

fn always(_: &ShellModel) -> Result<(), Key<'static>> {
    Ok(())
}

fn project_open(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.has_project() {
        Ok(())
    } else {
        Err(key!("reason-no-project"))
    }
}

fn writable(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.is_writable() {
        Ok(())
    } else {
        Err(key!("reason-read-only"))
    }
}

/// The project can be written and has an open revision (Approve, Release, Set author).
fn open_revision(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.open_revision_label().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-open-revision"))
    }
}

/// The project can be written and has a released revision that is not withdrawn (Withdraw).
fn released_revision(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.released_labels().is_empty() {
        Err(key!("reason-no-released-revision"))
    } else {
        Ok(())
    }
}

/// An incomplete release was found when the project opened and is not settled yet.
fn incomplete_release(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.has_incomplete_release() {
        Ok(())
    } else {
        Err(key!("reason-no-incomplete-release"))
    }
}

/// FT-717: the project has a parts list to open (a `parts_summary` or `parts_list` report block with pages).
fn has_parts_list(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.parts_list_folio().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-parts-list"))
    }
}

fn has_pages(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.page_count() > 0 {
        Ok(())
    } else {
        Err(key!("reason-no-pages"))
    }
}

/// Export table… and Import table…: a project with a navigator shown as its table, the
/// one the actions read (`ShellModel::table_file_nav`); the Messages pane passes, and its action refuses as
/// `navigator.table` does.
fn table_shown(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    m.table_file_nav().map(|_| ())
}

/// The history panel's row moves and Show changes: a project with the panel open.
fn history_open(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.history_shown() {
        Ok(())
    } else {
        Err(key!("reason-history-closed"))
    }
}

/// Undo to here: a writable project with the panel open.
fn history_writable(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    history_open(m)
}

fn can_undo(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.symbol_editor().is_none() {
        writable(m)?;
    }
    if m.can_undo() {
        Ok(())
    } else {
        Err(key!("reason-nothing-to-undo"))
    }
}

fn can_redo(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.symbol_editor().is_none() {
        writable(m)?;
    }
    if m.can_redo() {
        Ok(())
    } else {
        Err(key!("reason-nothing-to-redo"))
    }
}

fn has_next(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    if m.page_index() + 1 < m.page_count() {
        Ok(())
    } else {
        Err(key!("reason-no-next-page"))
    }
}

fn has_previous(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    if m.page_index() > 0 {
        Ok(())
    } else {
        Err(key!("reason-no-previous-page"))
    }
}

fn has_recent(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.recent_count() > 0 {
        Ok(())
    } else {
        Err(key!("reason-no-recent"))
    }
}

fn page_shown(m: &ShellModel) -> Result<(), Key<'static>> {
    // The wizard's preview page is a page on show too (`view.snapshot` writes it).
    if m.page_count() > 0 || m.wizard().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-page"))
    }
}

fn can_zoom_in(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    match m.canvas() {
        Some(c) if c.zoom() >= crate::canvas::ZOOM_MAX => Err(key!("reason-zoom-max")),
        _ => Ok(()),
    }
}

fn can_zoom_out(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    match m.canvas() {
        Some(c) if c.zoom() <= crate::canvas::ZOOM_MIN => Err(key!("reason-zoom-min")),
        _ => Ok(()),
    }
}

fn can_add_tab(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.tab_count() >= crate::area::MAX_TABS => Err(key!("reason-tabs-full")),
        _ => Ok(()),
    }
}

fn can_close_tab(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.tab_count() <= 1 => Err(key!("reason-last-tab")),
        _ => Ok(()),
    }
}

fn can_cycle_tabs(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.focused_group().tabs.len() <= 1 => Err(key!("reason-one-tab")),
        _ => Ok(()),
    }
}

fn two_groups(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.side_by_side() => Ok(()),
        _ => Err(key!("reason-one-group")),
    }
}

fn can_go_back(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.current().history.back_len() > 0 => Ok(()),
        _ => Err(key!("reason-history-empty")),
    }
}

fn can_go_forward(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.current().history.forward_len() > 0 => Ok(()),
        _ => Err(key!("reason-history-empty")),
    }
}

/// A panel-layout page of a project that can be changed is on show (D10's reason: "Open a panel layout page").
pub(crate) fn panel_page(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    writable(m)?;
    match m.page_rows().get(m.page_index()).map(|r| r.kind.as_str()) {
        Some("panel_layout") => Ok(()),
        _ => Err(key!("reason-no-panel-page")),
    }
}

/// A panel-layout page with a surface view is on show in a project that can be changed (D10: Draw ▸ Layout is
/// disabled elsewhere with "Open a panel layout page").
pub(crate) fn panel_view(m: &ShellModel) -> Result<(), Key<'static>> {
    panel_page(m)?;
    if m.panel_tables().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-panel-page"))
    }
}

/// A placement is selected on a panel page that can be changed.
fn panel_selection(m: &ShellModel) -> Result<(), Key<'static>> {
    panel_page(m)?;
    if m.selected_placements() == 0 {
        Err(key!("reason-no-placement"))
    } else {
        Ok(())
    }
}

/// Why the page on show takes no symbols or wires, by its type, in the words of its kind where one key exists for it
///  and the generic text otherwise; `None` for a page that takes them. One mapping for the refusal of the
/// editing tools and the canvas HUD's prompt.
pub(crate) fn page_type_reason(m: &ShellModel) -> Option<Key<'static>> {
    match m.page_rows().get(m.page_index()).map(|r| r.kind.as_str()) {
        Some("section_divider") => Some(key!("reason-page-type-no-symbols-divider")),
        Some("title_page") => Some(key!("reason-page-type-no-symbols-title")),
        Some("table_of_contents") => Some(key!("reason-page-type-no-symbols-contents")),
        Some("report") => Some(key!("reason-page-type-no-symbols-report")),
        Some("panel_layout") => Some(key!("reason-page-type-no-symbols-panel")),
        Some("model_view" | "external_document") => Some(key!("reason-page-type-no-symbols")),
        _ => None,
    }
}

/// A drawn page of a project that can be changed is on show (the editing tools).
pub(crate) fn editable_page(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    writable(m)?;
    // The page types without wire or symbol tools (doc 06 §2.2: title page, panel layout, model view, external
    // document; the generated table of contents, dividers and reports are not drawn pages at all).
    // The refusal names the page's kind in words where one key exists for it, the generic text otherwise.
    if let Some(reason) = page_type_reason(m) {
        return Err(reason);
    }
    match m.canvas() {
        Some(c) if !c.page().tables.page.is_nil() => Ok(()),
        _ => Err(key!("reason-not-editable")),
    }
}

/// A drawn page of a writable project for Paste (window check, item 3): the page's kind is checked by the action, which
/// refuses it with `reason-paste-page-kind` and the kind's name in the status bar; the menu disables it with that text.
fn paste_page(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    writable(m)?;
    // A page kind that takes no paste may have no canvas at all (a generated report): the action still runs, so it can
    // refuse with the kind's reason rather than the general one.
    if m.paste_page_kind().is_some() {
        return Ok(());
    }
    match m.canvas() {
        Some(c) if !c.page().tables.page.is_nil() => Ok(()),
        _ => Err(key!("reason-not-editable")),
    }
}

/// An editable page whose type has a connection kind the Wire tool draws (fix round item 1).
fn wire_page(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    m.wire_kind().map(|_| ())
}

/// An editable page with a connection or a representation selected (Route selection).
fn route_selection(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.route_selection_ids().is_empty() {
        Err(key!("reason-route-selection"))
    } else {
        Ok(())
    }
}

/// An editable page with exactly one connection selected (Edit ▸ Edit route; window check).
fn one_connection(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.one_connection_selected() {
        Ok(())
    } else {
        Err(key!("reason-one-connection"))
    }
}

/// An editable page whose selection gives a bond its two ends: exactly one selected connection (Draw ▸ Bond).
/// The headless form that names the two ends (`a` and `b`) needs only the editable page ([`enabled_with`]).
fn bond_ends(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.one_connection_selected() {
        Ok(())
    } else {
        Err(key!("reason-bond-selection"))
    }
}

/// An editable page whose selection names one point a potential is defined at (Draw ▸ Define potential…).
fn potential_point(m: &ShellModel) -> Result<(), Key<'static>> {
    // A Potentials row's form defines at the row's point, whatever page is on show.
    if m.potential_pinned() {
        return writable(m);
    }
    editable_page(m)?;
    m.potential_point().map(|_| ())
}

/// Whether the action `meta` can run now with the parameters `p`: its predicate, except where the parameters supply
/// what the predicate reads from the selection (`bond.draw` with `a` or `b` names its two ends itself). The
/// menus and the palette, which send no such parameters, read the predicate alone.
///
/// # Errors
///
/// The reason key of the refusal.
pub fn enabled_with(
    meta: &ActionMeta,
    m: &ShellModel,
    p: &serde_json::Map<String, Value>,
) -> Result<(), Key<'static>> {
    if m.symbol_editor().is_some() {
        if meta.id == "file.save" {
            return Ok(());
        }
        if meta.id.starts_with("tool.")
            || (meta.kind == ActionKind::Command
                && !meta.id.starts_with("symbol.")
                && !meta.id.starts_with("file.")
                && !matches!(meta.id, "edit.undo" | "edit.redo" | "page.new" | "app.quit"))
        {
            return Err(key!("symbol-editor-active"));
        }
    }
    if meta.id == "bond.draw" && (p.contains_key("a") || p.contains_key("b")) {
        return editable_page(m);
    }
    (meta.enabled)(m)
}

/// An editable page with exactly one junction selected (Edit ▸ Junction sequence…; window check).
fn one_junction(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.one_junction_selected() {
        Ok(())
    } else {
        Err(key!("reason-one-junction"))
    }
}

/// An editable page with exactly one interruption point selected (Edit ▸ Continue on page…).
fn one_ip(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.selected_ip_count() == 1 {
        Ok(())
    } else {
        Err(key!("reason-select-one-ip"))
    }
}

/// An editable page with at least two interruption points selected (Edit ▸ Arrange interruption points; §9.3).
fn two_ips(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.selected_ip_count() >= 2 {
        Ok(())
    } else {
        Err(key!("reason-select-ips"))
    }
}

/// An editable page with exactly one black box selected (Edit ▸ Add connection point, the grips; §9.4).
fn one_box(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.one_black_box_selected() {
        Ok(())
    } else {
        Err(key!("reason-one-black-box"))
    }
}

/// An editable page with exactly one unbound free text selected (Edit ▸ Edit text; §10.5).
fn one_free_text(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    m.text_edit_target().map(|_| ())
}

/// An editable page whose selection is two or more terminals of one strip not all bridged yet (Edit ▸ Bridge
/// terminals).
fn two_strip_terminals(m: &ShellModel) -> Result<(), Key<'static>> {
    // A writable project, not an editable page (the selection may be records).
    writable(m)?;
    m.bridge_command().map(|_| ())
}

/// The strip editor's gestures: a writable project with the Terminal strips navigator open and focused
/// (headless: open).
fn strips_editor(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    m.strip_editor_shown()
}

/// Swap cores: a writable project with the Cables navigator open and focused (headless: open).
fn cables_editor(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    m.cables_shown()
}

/// The junction sequence editor is open on a project that can be changed.
fn junction_editor_open(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.junction_editor().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-junction-editor"))
    }
}

/// A project with a page is open: `page.describe` reads a page.
fn page_describable(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.page_count() > 0 {
        Ok(())
    } else {
        Err(key!("reason-no-page"))
    }
}

/// The Place tool is active and its symbol can be an auxiliary function (J offers the Join chooser).
fn place_joinable(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.place_joinable() {
        Ok(())
    } else {
        Err(key!("reason-join-not-auxiliary"))
    }
}

/// The Join chooser is open.
fn chooser_open(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.chooser().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-chooser"))
    }
}

/// The parts import's column mapping step is open on a writable project.
fn parts_map_open(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.parts_map().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-parts-map"))
    }
}

/// The picker is open.
fn picker_open(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.picker().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-picker"))
    }
}

fn has_link_preview(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    if m.link_preview().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-preview"))
    }
}

fn can_discard_journal(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.can_discard_journal() {
        Ok(())
    } else {
        Err(key!("reason-no-journal"))
    }
}

/// Update from library needs the damaged-copy banner that offers it (FT-1499).
fn damaged_update_offered(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.damaged_update_offered() {
        Ok(())
    } else {
        Err(key!("reason-no-damaged-copy-update"))
    }
}

/// The recovery dialog's answers need the dialog (§6.6).
fn recovery_open(m: &ShellModel) -> Result<(), Key<'static>> {
    if matches!(m.dialog(), Some(crate::shell::Dialog::Recovery { .. })) {
        Ok(())
    } else {
        Err(key!("reason-no-recovery"))
    }
}

fn wizard_open(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.wizard().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-wizard"))
    }
}

fn first_run_open(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.first_run().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-first-run"))
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

/// The property grid's actions need a project; the grid's own controls also need the pane.
fn grid_shown(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.properties_shown() {
        Ok(())
    } else {
        Err(key!("reason-no-sheet"))
    }
}

/// A grid action that may commit: a writable project with the pane shown.
fn grid_writable(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    grid_shown(m)
}

const fn in_find(mut a: ActionMeta) -> ActionMeta {
    a.context = Context::Find;
    a
}

/// The find pane's own controls need the pane.
fn find_shown(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.find_shown() {
        Ok(())
    } else {
        Err(key!("reason-no-find"))
    }
}

/// A replace: a writable project with the find pane shown.
fn find_writable(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    find_shown(m)
}

/// The card's buttons: the card sits in the property grid or the find pane, so one of them is shown; an import card
///  is a window of its own and needs neither.
fn card_shown(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    // The Preview tab shows every card: a table card is in its navigator.
    if m.properties_shown()
        || m.find_shown()
        || m.import_card_open()
        || m.preview_shown()
        || m.table_gesture().is_some()
    {
        Ok(())
    } else {
        Err(key!("reason-no-sheet"))
    }
}

/// Apply on the card: a writable project with the property grid or the find pane shown, or an import card open.
fn card_writable(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    card_shown(m)
}

/// `navigator.apply`: a writable project with the Pages pane's reorder card, the Messages pane's
/// quick-fix card or a navigator's table card on show.
fn reorder_card(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.properties().card().is_some_and(|c| {
        matches!(
            c.origin,
            crate::properties::CardOrigin::Reorder { .. }
                | crate::properties::CardOrigin::QuickFix { .. }
                | crate::properties::CardOrigin::Table { .. }
        )
    }) {
        Ok(())
    } else {
        Err(key!("reason-no-card"))
    }
}

/// `table.resolve` and `table.drop`: a writable project with a table card on show.
fn table_card(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.table_gesture().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-card"))
    }
}

/// View ▸ Clear highlights: a highlight is shown.
fn has_highlights(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.highlights().is_empty() {
        Err(key!("reason-no-highlights"))
    } else {
        Ok(())
    }
}

/// The page reorder actions: a writable project with pages.
fn pages_writable(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    has_pages(m)
}

/// Copy and Copy tag list: a page is shown and something is selected (a read-only project copies too).
fn selection_shown(m: &ShellModel) -> Result<(), Key<'static>> {
    page_shown(m)?;
    if m.selection().is_empty() {
        Err(key!("reason-nothing-selected"))
    } else {
        Ok(())
    }
}

/// Edit ▸ Delete (M39): an editable page with something selected; it deletes the canvas selection only (a navigator's
/// rows have their own Delete, `table.clear`), so with nothing selected it is disabled for that reason, as Cut is.
fn editable_selection(m: &ShellModel) -> Result<(), Key<'static>> {
    // Delete on a panel page deletes the selected placements and carriers.
    if panel_page(m).is_ok() {
        return if m.selection().is_empty() {
            Err(key!("reason-nothing-selected"))
        } else {
            Ok(())
        };
    }
    editable_page(m)?;
    // A selection Delete has nothing of on this page (a page chosen in the page list) is not
    // offered here.
    if m.selection().is_empty() || !m.delete_has_target() {
        Err(key!("reason-nothing-selected"))
    } else {
        Ok(())
    }
}

/// Save as macro…: a page on show with something selected (a window macro) or pages
/// marked in the Pages navigator (a page macro).
fn macro_scope_shown(m: &ShellModel) -> Result<(), Key<'static>> {
    page_shown(m)?;
    if m.selection().is_empty() && m.navigator().marked().is_empty() {
        Err(key!("reason-nothing-selected"))
    } else {
        Ok(())
    }
}

/// Cut and Duplicate: an editable page with something selected.
fn selection_editable(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    selection_shown(m)
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

/// The keymap editor's controls need the keyboard shortcuts dialog.
fn keymap_editor_open(m: &ShellModel) -> Result<(), Key<'static>> {
    if matches!(m.dialog(), Some(crate::shell::Dialog::Shortcuts)) {
        Ok(())
    } else {
        Err(key!("reason-no-keymap-editor"))
    }
}

/// The paste choice card's choices need the card and a writable project.
fn paste_choice_open(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if matches!(
        m.properties().card().map(|c| &c.origin),
        Some(crate::properties::CardOrigin::PasteVersions(_))
    ) {
        Ok(())
    } else {
        Err(key!("reason-no-paste-choice"))
    }
}

/// The print pre-dialog's own steps.
fn print_dialog_open(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.print_dialog().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-print-dialog"))
    }
}

/// The part selection dialog's own actions need the dialog.
fn part_dialog_open(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if matches!(m.dialog(), Some(crate::shell::Dialog::PartChoose(_))) {
        Ok(())
    } else {
        Err(key!("reason-no-part-dialog"))
    }
}

/// The part editor's own actions need the editor and a writable project.
fn part_editor_open(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if matches!(m.dialog(), Some(crate::shell::Dialog::PartEdit(_))) {
        Ok(())
    } else {
        Err(key!("reason-no-part-editor"))
    }
}

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

/// The shortcut text of an action as a menu shows it: its first binding, or an empty string.
pub fn shortcut_text(meta: &ActionMeta) -> String {
    meta.shortcut.first().copied().unwrap_or("").to_owned()
}

/// One item of a menu.
#[derive(Clone, PartialEq, Debug)]
pub struct ActionItem {
    /// The action it dispatches.
    pub id: &'static str,
    /// The parameters it dispatches with (`{"index": 0}` for a recent project).
    pub params: Value,
    /// The label.
    pub label: String,
    /// The accelerator text, empty when there is none.
    pub shortcut: String,
    /// Whether the item can be used now.
    pub enabled: bool,
    /// The reason text of a disabled item.
    pub reason: Option<String>,
    /// Whether the item shows a check mark (the current UI scale or workspace).
    pub checked: bool,
    /// The documentation topic of the action ([`crate::help`]): its tooltip ends with "F1: learn more".
    pub topic: Option<&'static str>,
}

/// An entry of a menu: an item, a submenu or a command-bar group shown as a submenu.
#[derive(Clone, PartialEq, Debug)]
pub enum MenuEntry {
    /// A separator line.
    Separator,
    /// An item.
    Item(ActionItem),
    /// A submenu with its items.
    Submenu {
        /// The submenu.
        submenu: Submenu,
        /// Its label.
        label: String,
        /// Its items.
        items: Vec<ActionItem>,
    },
    /// A command-bar group of more than three commands, as a submenu of its tab's menu (doc 16a
    /// §9.2): its entries are items and submenus.
    Group {
        /// The group.
        group: Group,
        /// Its label (the group's name).
        label: String,
        /// Its entries.
        entries: Vec<MenuEntry>,
    },
}

impl MenuEntry {
    /// Every item of the entry, depth first (a separator has none).
    pub fn items(&self) -> Vec<&ActionItem> {
        match self {
            MenuEntry::Separator => Vec::new(),
            MenuEntry::Item(i) => vec![i],
            MenuEntry::Submenu { items, .. } => items.iter().collect(),
            MenuEntry::Group { entries, .. } => entries.iter().flat_map(MenuEntry::items).collect(),
        }
    }
}

impl MenuEntry {
    /// The items of `submenu` if this entry is it or holds it.
    pub fn submenu(&self, submenu: Submenu) -> Option<&[ActionItem]> {
        match self {
            MenuEntry::Submenu {
                submenu: s, items, ..
            } if *s == submenu => Some(items),
            MenuEntry::Group { entries, .. } => entries.iter().find_map(|e| e.submenu(submenu)),
            _ => None,
        }
    }
}

impl MenuModel {
    /// Every item of the menu, depth first.
    pub fn items(&self) -> Vec<&ActionItem> {
        self.entries.iter().flat_map(MenuEntry::items).collect()
    }

    /// The items of `submenu`, wherever in the menu it sits (a command-bar group may hold it).
    pub fn submenu(&self, submenu: Submenu) -> Option<&[ActionItem]> {
        self.entries.iter().find_map(|e| e.submenu(submenu))
    }
}

/// One menu of the menu bar.
#[derive(Clone, PartialEq, Debug)]
pub struct MenuModel {
    /// The menu.
    pub menu: Menu,
    /// Its label.
    pub label: String,
    /// Its entries, in order.
    pub entries: Vec<MenuEntry>,
}

/// The menu item of `meta` in the state of `model`.
pub(crate) fn item_of(meta: &ActionMeta, model: &ShellModel) -> ActionItem {
    let state = enabled_with(meta, model, &serde_json::Map::new());
    // Paste names the page kind that cannot take it (window check, item 3): the reason is a text with the kind, not a
    // plain key, so the item formats it here.
    // Enclosure… likewise names Structure node… in its reason when the project has no location (W44.2 F1).
    let paste_reason = match meta.id {
        _ if state.is_err() => None,
        "edit.paste_clip" => model.paste_page_reason(),
        "layout.enclosure_new" => model.enclosure_new_reason(),
        _ => None,
    };
    let enabled = state.is_ok() && paste_reason.is_none();
    ActionItem {
        id: meta.id,
        params: Value::Object(serde_json::Map::new()),
        label: strings::text(Key::run_time(&meta.label_key())).to_owned(),
        shortcut: model.shortcut_text(meta),
        enabled,
        reason: paste_reason.or_else(|| state.err().map(|k| strings::text(k).to_owned())),
        checked: switch_on(model, meta.id).unwrap_or_else(|| match meta.id {
            "tool.select" | "tool.measure" => meta.id.strip_prefix("tool.") == Some(model.tool().name()),
            "tool.wire" => model.tool().name() == "wire",
            "tool.ip" => model.tool().name() == "ip",
            "tool.black_box" => model.tool().name() == "black_box",
            "tool.text" => model.tool().name() == "text",
            "tool.marker" => model.tool().name() == "marker",
            "tool.structure_box" => model.tool().name() == "structure_box",
            "tool.rail_draw" => model.tool().name() == "rail_draw",
            "tool.duct_draw" => model.tool().name() == "duct_draw",
            // The Place tool lights Symbol in the bar.
            "tool.place" => model.tool().name() == "place",
            _ => false,
        }),
        topic: crate::help::action_topic(meta.id),
    }
}

/// The on or off state of a switch (a view toggle, a pane shown or hidden): the menu item's check, the command-bar
/// button's lit state and the palette entry's state; `None` for an action that is no switch.
pub(crate) fn switch_on(model: &ShellModel, id: &str) -> Option<bool> {
    Some(match id {
        "view.grid" => model.grid_on(),
        "view.frame_stats" => model.frame_stats_shown(),
        "view.cpu_canvas" => model.cpu_canvas_setting(),
        "view.reduce_motion" => model.reduce_motion(),
        "view.wheel_scrolls" => model.settings().wheel_scrolls,
        "view.tips" => model.tips_on(),
        "view.side_by_side" => model.area().is_some_and(crate::area::CanvasArea::side_by_side),
        "props.show" => model
            .layout()
            .panes()
            .contains(&crate::layout::PaneKind::Properties),
        "jobs.show" => model.jobs_shown(),
        "notifications.show" => model.notifications().popup_open(),
        "messages.show" => model.navigator_open(crate::navigators::NavKind::Messages),
        "messages.migration" => model.messages_shows_migration(),
        "history.show" => model.history_shown(),
        "preview.show" => model.preview_shown(),
        "view.bottom_dock" => model.bottom_dock_shown(),
        "help.glossary" => model.glossary_shown(),
        "view.wire_colours" => model.wire_colours_on(),
        "project.crossings" => model.crossings_hops(),
        // The command bar's collapse.
        "view.command_bar" => model.command_bar_collapsed(),
        "props.show_advanced" => model.properties_show_advanced(),
        _ => return None,
    })
}

/// The items of a submenu, from the registry actions that have a slot in it (in registry order).
fn submenu_items(submenu: Submenu, metas: &[&ActionMeta], model: &ShellModel) -> Vec<ActionItem> {
    let Some(&meta) = metas.first() else {
        return Vec::new();
    };
    match submenu {
        // One item per command-bar tab.
        Submenu::CommandTab => crate::command_bar::Tab::ALL
            .into_iter()
            .map(|tab| {
                let mut item = item_of(meta, model);
                item.label = strings::text(tab.label_key()).to_owned();
                item.params = json!({ "tab": tab.word() });
                item.shortcut = String::new();
                item.checked = model.command_tab() == tab;
                item
            })
            .collect(),
        Submenu::PageNavigator => {
            let nav = model.navigator();
            let mut items = Vec::new();
            for meta in metas {
                if meta.id == "pages.mode" {
                    for mode in crate::navigator::NavMode::ALL {
                        let mut item = item_of(meta, model);
                        item.label =
                            strings::text(Key::run_time(&format!("nav-mode-{}", mode.word()))).to_owned();
                        item.params = json!({ "mode": mode.word() });
                        item.checked = nav.mode == mode;
                        items.push(item);
                    }
                } else if meta.id == "pages.sort" {
                    for sort in crate::navigator::NavSort::ALL {
                        let mut item = item_of(meta, model);
                        item.label =
                            strings::text(Key::run_time(&format!("nav-sort-{}", sort.word()))).to_owned();
                        item.params = json!({ "by": sort.word() });
                        item.checked = nav.sort == sort;
                        items.push(item);
                    }
                }
            }
            items
        }
        // One checked item per navigator; the message panel has its own item, View ▸ Messages.
        Submenu::Navigators => {
            let mut items = Vec::new();
            for kind in crate::navigators::NavKind::ALL
                .into_iter()
                .filter(|k| *k != crate::navigators::NavKind::Messages)
            {
                let mut item = item_of(meta, model);
                item.label = strings::text(crate::layout::nav_title_key(kind)).to_owned();
                let open = model.navigator_open(kind);
                item.params = if open {
                    json!({ "kind": kind.word(), "on": false })
                } else {
                    json!({ "kind": kind.word() })
                };
                item.checked = open;
                items.push(item);
            }
            items
        }
        Submenu::Pan | Submenu::Revision => metas.iter().map(|m| item_of(m, model)).collect(),
        // One item per UI language, under its own name.
        Submenu::Language => strings::languages()
            .into_iter()
            .map(|(value, name)| {
                let mut item = item_of(meta, model);
                item.label = name.to_owned();
                item.params = json!({ "value": value });
                item.checked = model.settings().language == value;
                item
            })
            .collect(),
        Submenu::Workspace => {
            let active = model.workspace();
            let mut items = Vec::new();
            for meta in metas {
                if meta.id == "view.workspace_load" {
                    for name in model.workspace_names() {
                        let mut item = item_of(meta, model);
                        item.checked = name == active;
                        item.params = json!({ "name": name });
                        item.label = name;
                        item.shortcut = String::new();
                        items.push(item);
                    }
                } else {
                    items.push(item_of(meta, model));
                }
            }
            items
        }
        Submenu::OpenRecent => {
            let names = model.recent_names();
            if names.is_empty() {
                let mut item = item_of(meta, model);
                item.label = strings::text(key!("menu-no-recent")).to_owned();
                item.enabled = false;
                return vec![item];
            }
            names
                .into_iter()
                .enumerate()
                .map(|(i, name)| {
                    let mut item = item_of(meta, model);
                    item.label = name;
                    item.params = json!({ "index": i });
                    item
                })
                .collect()
        }
        Submenu::UiScale => {
            let current = model.settings().ui_scale;
            UI_SCALES
                .iter()
                .map(|&scale| {
                    let mut item = item_of(meta, model);
                    item.label = scale_label(scale).to_owned();
                    item.params = json!({ "scale": scale });
                    item.shortcut = String::new();
                    item.checked = (current - scale).abs() < 1e-3;
                    item
                })
                .collect()
        }
        // One item per tutorial, each with its own start.
        Submenu::Tutorials => TUTORIALS
            .iter()
            .map(|name| {
                let mut item = item_of(meta, model);
                item.label = strings::lookup(&format!("tutorial-{name}"))
                    .unwrap_or(name)
                    .to_owned();
                // Doc 16a §8.3: an entry that asks for a folder carries "…" (`act_tutorials`).
                if matches!(*name, "first-page" | "reports-and-pdf") {
                    item.label.push('…');
                }
                item.params = json!({ "tutorial": name });
                item
            })
            .collect(),
        Submenu::MinFont => {
            let current = model.settings().min_font_pt;
            MIN_FONT_CHOICES
                .iter()
                .map(|&pt| {
                    let mut item = item_of(meta, model);
                    item.label = match pt {
                        0 => strings::text(key!("min-font-off")).to_owned(),
                        pt => strings::format(key!("min-font-pt"), &[&pt.to_string()]),
                    };
                    item.params = json!({ "pt": pt });
                    item.shortcut = String::new();
                    item.checked = pt == current;
                    item
                })
                .collect()
        }
        Submenu::Theme | Submenu::Density => {
            let (prefix, choices, current): (&str, &[&str], &str) = if submenu == Submenu::Theme {
                ("theme", &THEMES, &model.settings().theme)
            } else {
                ("density", &DENSITIES, &model.settings().density)
            };
            choices
                .iter()
                .map(|value| {
                    let mut item = item_of(meta, model);
                    item.label = strings::text(Key::run_time(&format!("{prefix}-{value}"))).to_owned();
                    item.params = json!({ "value": value });
                    item.shortcut = String::new();
                    item.checked = *value == current;
                    item
                })
                .collect()
        }
        Submenu::Graphic => {
            let active = match model.tool_ghost() {
                Some(Ghost::Graphic { shape, .. }) => Some(shape),
                _ => None,
            };
            GraphicShape::ALL
                .iter()
                .map(|shape| {
                    let mut item = item_of(meta, model);
                    item.label =
                        strings::text(Key::run_time(&format!("graphic-{}", shape.word()))).to_owned();
                    item.params = json!({ "shape": shape.word() });
                    item.shortcut = String::new();
                    item.checked = active == Some(shape.word());
                    item
                })
                .collect()
        }
        Submenu::Bond => crate::shell::BOND_KINDS
            .iter()
            .map(|kind| {
                let mut item = item_of(meta, model);
                item.label = strings::text(Key::run_time(&format!("bond-kind-{kind}"))).to_owned();
                item.params = json!({ "kind": kind });
                item
            })
            .collect(),
        Submenu::WireMarkers => {
            let current = model.marker_mode_word();
            ["per_wire", "all", "deviations", "explicit", "none"]
                .iter()
                .map(|mode| {
                    let mut item = item_of(meta, model);
                    item.label = strings::text(Key::run_time(&format!("marker-mode-{mode}"))).to_owned();
                    item.params = json!({ "mode": mode });
                    item.checked = *mode == current;
                    item
                })
                .collect()
        }
    }
}

/// The Window menu's list of open tabs: one checked item per tab of the focused group, dispatching `tab.activate`.
fn window_tabs(model: &ShellModel, entries: &mut Vec<MenuEntry>) {
    let Some(area) = model.area() else { return };
    let Some(meta) = find("tab.activate") else { return };
    entries.push(MenuEntry::Separator);
    let g = area.focused;
    for (i, tab) in area.focused_group().tabs.iter().enumerate() {
        let mut item = item_of(meta, model);
        item.label = model.tab_title(tab);
        item.params = json!({ "group": g, "index": i });
        item.checked = i == area.focused_group().active;
        entries.push(MenuEntry::Item(item));
    }
}

/// The registry actions in `submenu`, in registry order.
fn submenu_metas(submenu: Submenu) -> Vec<&'static ActionMeta> {
    ACTIONS
        .iter()
        .filter(|a| a.menu.is_some_and(|m| m.submenu == Some(submenu)))
        .collect()
}

/// The items of `submenu` in the state of `model` (the command bar's menu buttons open them).
pub(crate) fn submenu_entries(submenu: Submenu, model: &ShellModel) -> Vec<ActionItem> {
    submenu_items(submenu, &submenu_metas(submenu), model)
}

/// The entries of `metas` in order: an item each, a submenu once for the first action in it; with separators where
/// a slot asks for one (`separate`).
fn entries_of(metas: &[&'static ActionMeta], model: &ShellModel, separate: bool) -> Vec<MenuEntry> {
    let mut entries = Vec::new();
    let mut done: Vec<Submenu> = Vec::new();
    for meta in metas {
        let Some(s) = meta.menu else { continue };
        if s.submenu.is_some_and(|sub| done.contains(&sub)) {
            continue;
        }
        if separate && s.separator_before && !entries.is_empty() {
            entries.push(MenuEntry::Separator);
        }
        match s.submenu {
            // While the symbol editor shows, Save names the symbol and Save project follows it.
            None if meta.id == "file.save" && model.symbol_editor().is_some() => {
                let mut item = item_of(meta, model);
                item.label = strings::text(key!("symbol-save-symbol")).to_owned();
                entries.push(MenuEntry::Item(item));
                if let Some(project) = find("file.save_project") {
                    entries.push(MenuEntry::Item(item_of(project, model)));
                }
            }
            None => entries.push(MenuEntry::Item(item_of(meta, model))),
            Some(submenu) => {
                done.push(submenu);
                entries.push(MenuEntry::Submenu {
                    submenu,
                    label: strings::text(submenu.label_key()).to_owned(),
                    items: submenu_entries(submenu, model),
                });
            }
        }
    }
    entries
}

/// The registry actions of `group` in the app menu: its placed commands by their order, then its menu-only actions in
/// registry order.
pub fn group_metas(group: Group) -> Vec<&'static ActionMeta> {
    let mut metas = crate::command_bar::placed_in(group);
    metas.extend(
        ACTIONS
            .iter()
            .filter(|a| a.placement.is_none() && a.menu.is_some_and(|m| m.group == Some(group))),
    );
    metas
}

/// The app menu generated from the registry, in the state of `model` (doc 16a §9.2): File, Edit, the six tabs' menus
/// with their groups (a group of more than three entries is a submenu named by the group, a smaller one a separated
/// block), then Window and Help. Every action with a slot appears exactly once.
pub fn menu_model(model: &ShellModel) -> Vec<MenuModel> {
    Menu::ALL
        .iter()
        .map(|&menu| {
            let mut entries = Vec::new();
            if let Some(tab) = menu.tab() {
                for group in tab.groups() {
                    let block = entries_of(&group_metas(group), model, false);
                    if block.is_empty() {
                        continue;
                    }
                    if !entries.is_empty() {
                        entries.push(MenuEntry::Separator);
                    }
                    if block.len() > 3 {
                        entries.push(MenuEntry::Group {
                            group,
                            label: strings::text(Key::run_time(&group.label_key())).to_owned(),
                            entries: block,
                        });
                    } else {
                        entries.extend(block);
                    }
                }
            }
            let rest: Vec<&'static ActionMeta> = ACTIONS
                .iter()
                .filter(|a| a.menu.is_some_and(|s| s.menu == menu && s.group.is_none()))
                .collect();
            let mut rest = entries_of(&rest, model, true);
            if menu.tab().is_some() && !entries.is_empty() && !rest.is_empty() {
                if rest.first() != Some(&MenuEntry::Separator) {
                    entries.push(MenuEntry::Separator);
                }
            } else if rest.first() == Some(&MenuEntry::Separator) {
                rest.remove(0);
            }
            entries.extend(rest);
            if menu == Menu::Window {
                window_tabs(model, &mut entries);
            }
            MenuModel {
                menu,
                label: strings::text(menu.label_key()).to_owned(),
                entries,
            }
        })
        .collect()
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
        for id in ["file.new", "file.open_sample", "file.discard_journal"] {
            assert!(file.contains(&id), "{id} in File");
        }
        // The imports and the company standard sit in the Project tab's menu.
        let project = ids(Menu::Project);
        for id in ["tools.company_standard", "file.import_qet", "parts.import"] {
            assert!(project.contains(&id), "{id} in Project");
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
        // The picker's Enter arms the pick with the same optional parameters as `place.pick` (fix round).
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
