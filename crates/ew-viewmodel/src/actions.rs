// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The action registry (WP-S2.01 part 1, spec section 5.2; UX/UI doc 16 section 2.4) in toolkit-neutral form.
//!
//! Every menu item, palette entry, keyboard shortcut and dialog result of the shell is an action of
//! [`ACTIONS`], dispatched with [`ShellModel::dispatch`]; the headless mode drives the same registry. An action
//! has a stable ID, a label key (the English text lives in [`crate::strings`]), default shortcuts, and an
//! `enabled` predicate that names the reason key of a refusal. [`menu_model`] builds the app menu from the
//! registry, so the egui frame and the `ui.describe` text cannot disagree.
//!
//! WP-S2.17 part 2 (D3, doc 16a §9.2): the menus are the app menu's submenus, File, Edit, then one per command-bar
//! tab (Project, Page, Draw, Check, Output, View), then Window and Help; Insert became Draw and Tools dissolved. An
//! action with a [`Placement`] is a command of the bar ([`crate::command_bar`]) and sits in its tab's menu under its
//! group; a group of more than three commands is a submenu, a smaller one a separated block.

use serde_json::{Map, Value, json};

use crate::command_bar::{CommandSize, Group, Placement, Tab, place};
use crate::key;
use crate::settings::{DENSITIES, MIN_FONT_CHOICES, THEMES, UI_SCALES, scale_label};
use crate::shell::{Outcome, ShellModel};
use crate::strings;
use crate::strings::Key;
use crate::tools::{Ghost, GraphicShape};

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
    /// Project: the Project tab's groups (WP-S2.17 part 2).
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
    /// Window: the canvas tabs (WP-S2.03).
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
    /// View, Theme: one item per theme (WP-S2.17 part 1).
    Theme,
    /// View, Density: Comfortable and Compact (WP-S2.17 part 1).
    Density,
    /// View, Workspace: one item per workspace, then save and reset.
    Workspace,
    /// View, Pan: left, right, up, down.
    Pan,
    /// View, Page navigator: its three views and two sort orders (WP-S2.03).
    PageNavigator,
    // WP-S2.06 part 1
    /// View, Navigators: one checked item per navigator kind (`navigator.show`).
    Navigators,
    /// Insert, Graphic: one item per shape of the Graphic tool (WP-S2.04 §10.5).
    Graphic,
    /// Draw, Connect, Wire markers: the marker mode's four settable modes and the as-built one (WP-S2.04 §10.6).
    WireMarkers,
    /// View, Layout, Command tab: one item per tab of the command bar (WP-S2.17 part 2).
    CommandTab,
    /// Draw, Connect, Bond: one item per bond kind of `bond.create` (G-PP triage row 7).
    Bond,
    /// Check, Revision: Approve, Release, Withdraw, Set author and the incomplete release (WP-S1.18 part 5).
    Revision,
    /// View, Language: one item per UI language, under its own name (WP-S2.08 part 3).
    Language,
    /// View, Minimum text size: off, then one item per size of `MIN_FONT_CHOICES` (WP-S2.08 D14).
    MinFont,
    /// Help, Tutorials: one item per tutorial of [`TUTORIALS`], each with its own start (FT-1458).
    Tutorials,
}

/// The tutorials of the guide in its order (FT-1458, ID-1397): each its page's name, which `help.tutorials` takes as
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

/// Where an action's keyboard shortcuts apply (WP-S2.03 §6.6): the window binds a context's chords only while the
/// shell's focus is in that context; a context's binding shadows a global one with the same chord.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Context {
    /// Everywhere.
    Global,
    /// While a canvas has the focus (the grid cursor, following links).
    Canvas,
    /// While the page navigator has the focus.
    Navigator,
    /// While the property grid has the focus (WP-S2.05).
    Properties,
    /// While the find pane has the focus (WP-S2.05 part 2).
    Find,
}

/// Where an action sits in the app menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MenuSlot {
    /// The menu.
    pub menu: Menu,
    /// The command-bar group inside a tab's menu, if any (WP-S2.17 part 2): the group's commands first, then the
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
    /// The parameter signature (WP-S6.13 §5.5.2): `""` for an action without parameters, else a comma-separated list of
    /// `name[?]:type` (see `validate_signature`). The blind drafter's harness types its `key=value` arguments by it.
    pub params: &'static str,
    /// The place in the command bar (WP-S2.17 part 2, D3); `None` for an action the bar does not show.
    pub placement: Option<Placement>,
    /// The icon key (`ew-ui` maps it to its icon table; doc 16a §4.11); required for a placed action.
    pub icon: Option<&'static str>,
    /// The action reads no parameter although it is hidden, headless-only or marked `needs_parameters` (WP-S6.13 BD2):
    /// its signature stays empty on purpose.
    pub parameterless: bool,
    /// Dispatching the action leaves the banner standing (popups, dialogs, help, the describe text, the canvas's view
    /// and tool input, the panes' own moves); every other action clears it.
    pub keeps_banner: bool,
    /// The action goes while the recovery dialog is open (WP-S2.09 §6.6): its answers, Escape, what only observes (the
    /// describe text, F1) or ends the run (Quit closes the read-only project; the journal stays), and the window's own
    /// canvas geometry (`view.canvas_resize`, which the frame sends until the shell has the pane's size: refused, it
    /// would be sent again every frame). Any other action refuses `reason-dialog-open`.
    pub answers_recovery: bool,
    /// Dispatching the action leaves the link hover preview standing: the rest that shows it, the describe text, the
    /// snapshots and Escape (which closes it as the topmost popup); every other action (a pointer move, a cursor move,
    /// a view action) clears it (WP-S2.03 §7.8).
    pub keeps_link_preview: bool,
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

/// WP-S1.18 part 5: the project can be written and has an open revision (Approve, Release, Set author).
fn open_revision(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.open_revision_label().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-open-revision"))
    }
}

/// WP-S1.18 part 5: the project can be written and has a released revision that is not withdrawn (Withdraw).
fn released_revision(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.released_labels().is_empty() {
        Err(key!("reason-no-released-revision"))
    } else {
        Ok(())
    }
}

/// WP-S1.18 part 5: an incomplete release was found when the project opened and is not settled yet.
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

// WP-S3.08 part 3
/// Export table… and Import table… (ruling 2 of ID-1224, ID-1231): a project with a navigator shown as its table, the
/// one the actions read (`ShellModel::table_file_nav`); the Messages pane passes, and its action refuses as
/// `navigator.table` does.
fn table_shown(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    m.table_file_nav().map(|_| ())
}

// WP-S2.09 part 3
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

// WP-S4.02 part 3
/// A panel-layout page of a project that can be changed is on show (D10's reason: "Open a panel layout page").
pub(crate) fn panel_page(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    writable(m)?;
    match m.page_rows().get(m.page_index()).map(|r| r.kind.as_str()) {
        Some("panel_layout") => Ok(()),
        _ => Err(key!("reason-no-panel-page")),
    }
}

// WP-S4.02 part 4
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
/// (FT-892) and the generic text otherwise; `None` for a page that takes them. One mapping for the refusal of the
/// editing tools and the canvas HUD's prompt (FT-1089, ID-1183).
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

/// A drawn page of a project that can be changed is on show (the editing tools of WP-S2.04 §7.6).
pub(crate) fn editable_page(m: &ShellModel) -> Result<(), Key<'static>> {
    has_pages(m)?;
    writable(m)?;
    // The page types without wire or symbol tools (doc 06 §2.2: title page, panel layout, model view, external
    // document; the generated table of contents, dividers and reports are not drawn pages at all).
    // FT-892: the refusal names the page's kind in words where one key exists for it, the generic text otherwise.
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

/// An editable page whose type has a connection kind the Wire tool draws (WP-S2.04 part 2, fix round item 1).
fn wire_page(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    m.wire_kind().map(|_| ())
}

/// An editable page with a connection or a representation selected (Route selection, WP-S2.19 D13; ID-1509 (8)).
fn route_selection(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.route_selection_ids().is_empty() {
        Err(key!("reason-route-selection"))
    } else {
        Ok(())
    }
}

/// An editable page with exactly one connection selected (Edit ▸ Edit route; WP-S2.04 part 2, window check).
fn one_connection(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.one_connection_selected() {
        Ok(())
    } else {
        Err(key!("reason-one-connection"))
    }
}

/// An editable page whose selection gives a bond its two ends: exactly one selected connection (Draw ▸ Bond, FT-578).
/// The headless form that names the two ends (`a` and `b`) needs only the editable page ([`enabled_with`]).
fn bond_ends(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.one_connection_selected() {
        Ok(())
    } else {
        Err(key!("reason-bond-selection"))
    }
}

/// An editable page whose selection names one point a potential is defined at (Draw ▸ Define potential…, FT-1202).
fn potential_point(m: &ShellModel) -> Result<(), Key<'static>> {
    // FT-1239: a Potentials row's form defines at the row's point, whatever page is on show.
    if m.potential_pinned() {
        return writable(m);
    }
    editable_page(m)?;
    m.potential_point().map(|_| ())
}

/// Whether the action `meta` can run now with the parameters `p`: its predicate, except where the parameters supply
/// what the predicate reads from the selection (`bond.draw` with `a` or `b` names its two ends itself, FT-578). The
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

/// An editable page with exactly one junction selected (Edit ▸ Junction sequence…; WP-S2.04 part 2, window check).
fn one_junction(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.one_junction_selected() {
        Ok(())
    } else {
        Err(key!("reason-one-junction"))
    }
}

/// An editable page with exactly one interruption point selected (Edit ▸ Continue on page…; WP-S2.04 §9.3).
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
/// terminals; FT-411, §10.7, cycle 33).
fn two_strip_terminals(m: &ShellModel) -> Result<(), Key<'static>> {
    // WP-S2.07 D9: a writable project, not an editable page (the selection may be records).
    writable(m)?;
    m.bridge_command().map(|_| ())
}

/// The strip editor's gestures (WP-S2.07 §6.9): a writable project with the Terminal strips navigator open and focused
/// (headless: open).
fn strips_editor(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    m.strip_editor_shown()
}

/// Swap cores (WP-S2.07 §7): a writable project with the Cables navigator open and focused (headless: open).
fn cables_editor(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    m.cables_shown()
}

/// The junction sequence editor is open on a project that can be changed (WP-S2.04 part 2, §8.5).
fn junction_editor_open(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.junction_editor().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-junction-editor"))
    }
}

/// A project with a page is open: `page.describe` reads a page (WP-S6.13 §5.5.3).
fn page_describable(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.page_count() > 0 {
        Ok(())
    } else {
        Err(key!("reason-no-page"))
    }
}

/// The Place tool is active and its symbol can be an auxiliary function (WP-S2.18 D2: J offers the Join chooser).
fn place_joinable(m: &ShellModel) -> Result<(), Key<'static>> {
    editable_page(m)?;
    if m.place_joinable() {
        Ok(())
    } else {
        Err(key!("reason-join-not-auxiliary"))
    }
}

/// The Join chooser is open (WP-S2.18 D2).
fn chooser_open(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.chooser().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-chooser"))
    }
}

/// The parts import's column mapping step is open on a writable project (FT-649).
fn parts_map_open(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.parts_map().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-parts-map"))
    }
}

/// The picker is open (WP-S2.04 §7.1).
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

/// Update from library needs the damaged-copy banner that offers it (FT-1499, ID-1486).
fn damaged_update_offered(m: &ShellModel) -> Result<(), Key<'static>> {
    if m.damaged_update_offered() {
        Ok(())
    } else {
        Err(key!("reason-no-damaged-copy-update"))
    }
}

// WP-S2.09 part 1
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

/// A menu-only action in a command-bar group of its tab's menu (WP-S2.17 part 2).
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
        parameterless: false,
        keeps_banner: false,
        answers_recovery: false,
        keeps_link_preview: false,
        handler,
    }
}

/// Gives an action its parameter signature (WP-S6.13 §5.5.2).
const fn with_params(params: &'static str, mut a: ActionMeta) -> ActionMeta {
    a.params = params;
    a
}

const fn needs_parameters(mut a: ActionMeta) -> ActionMeta {
    a.needs_parameters = true;
    a
}

/// Marks a hidden, headless-only or `needs_parameters` action that reads no parameter, so its empty signature is meant.
const fn parameterless(mut a: ActionMeta) -> ActionMeta {
    a.parameterless = true;
    a
}

// map: action-flags: a new banner, recovery or link-preview exception goes on its ACTIONS entry as a keeps_banner, answers_recovery or keeps_link_preview wrapper.
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

// WP-S2.05 part 2
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
/// (FT-410) is a window of its own and needs neither.
fn card_shown(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    // WP-S2.09 part 3 (§8.7): the Preview tab shows every card; WP-S2.06 part 3: a table card is in its navigator.
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

// WP-S2.06 part 1
/// `navigator.apply`: a writable project with the Pages pane's reorder card, (WP-S2.09 part 2) the Messages pane's
/// quick-fix card or (WP-S2.06 part 3) a navigator's table card on show.
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

// WP-S2.06 part 3
/// `table.resolve` and `table.drop`: a writable project with a table card on show.
fn table_card(m: &ShellModel) -> Result<(), Key<'static>> {
    writable(m)?;
    if m.table_gesture().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-card"))
    }
}

// WP-S2.06 part 2
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

// WP-S2.05 part 3
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
    // WP-S4.02 part 3 (D10): Delete on a panel page deletes the selected placements and carriers.
    if panel_page(m).is_ok() {
        return if m.selection().is_empty() {
            Err(key!("reason-nothing-selected"))
        } else {
            Ok(())
        };
    }
    editable_page(m)?;
    // C53-fx#1 (ID-1497 (8)): a selection Delete has nothing of on this page (a page chosen in the page list) is not
    // offered here.
    if m.selection().is_empty() || !m.delete_has_target() {
        Err(key!("reason-nothing-selected"))
    } else {
        Ok(())
    }
}

/// Save as macro… (WP-S1.19 §11, FT-754 (b)): a page on show with something selected (a window macro) or pages
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

// map: actions: a new action goes in ACTIONS as one entry naming its handler; its label key is action-<id, . and _ as ->.
/// The registry: the part-1 actions of the spec, section 5.2, in menu order, then the headless-only ones.
pub static ACTIONS: &[ActionMeta] = &[
    with_params(
        "dir?:path, template?:path, discard?:bool",
        act(
            "file.new",
            Command,
            Menu::File,
            &["Ctrl+N"],
            always,
            slot(Menu::File),
            |s, p| s.act_new(p),
        ),
    ),
    with_params(
        "dir?:path, read_only?:bool, discard?:bool",
        act(
            "file.open",
            Command,
            Menu::File,
            &["Ctrl+O"],
            always,
            slot(Menu::File),
            |s, p| s.act_open(p),
        ),
    ),
    // WP-S1.17 part 7 (D24): File ▸ Open .ewz…, asking Extract and open or Open read-only.
    with_params(
        "path?:path, how?:enum(extract|read_only), discard?:bool",
        act(
            "file.open_ewz",
            Command,
            Menu::File,
            &[],
            always,
            slot(Menu::File),
            |s, p| s.act_open_ewz(p),
        ),
    ),
    with_params(
        "index:uint, discard?:bool",
        needs_parameters(act(
            "file.open_recent",
            Command,
            Menu::File,
            &[],
            has_recent,
            sub(Menu::File, Submenu::OpenRecent, false),
            |s, p| s.act_open_recent(p),
        )),
    ),
    with_params(
        "discard?:bool",
        act(
            "file.open_sample",
            Command,
            Menu::File,
            &[],
            always,
            slot(Menu::File),
            |s, p| s.act_open_sample(p),
        ),
    ),
    with_params(
        "file?:path, dir?:path, discard?:bool",
        placed(
            place(Group::Import, 2, CommandSize::Small, Some(key!("bar-import-qet"))),
            "file-import",
            act(
                "file.import_qet",
                Command,
                Menu::Project,
                &[],
                always,
                // FT-466 moved it to File, Import; WP-S2.17 part 2 places it in Project, Import (its ID unchanged).
                None,
                |s, p| s.act_import_qet(p),
            ),
        ),
    ),
    // WP-S3.09 D8: the import card's Apply and the checkpoint confirmation's Create project.
    with_params(
        "confirm?:bool",
        hidden(act(
            "file.import_qet_apply",
            Command,
            Menu::Project,
            &[],
            always,
            None,
            |s, p| s.act_import_qet_apply(p),
        )),
    ),
    // WP-S3.09 D10 to D12: Project, Import, Re-import QElectroTech project (the open project's earlier import), its
    // proposal's Re-import (`accept`) and the re-import card's Apply. FT-1388 (ID-1392): a command-bar button beside
    // the import's.
    with_params(
        "file?:path, accept?:bool",
        placed(
            place(
                Group::Import,
                3,
                CommandSize::Small,
                Some(key!("bar-reimport-qet")),
            ),
            "file-import",
            act(
                "file.reimport_qet",
                Command,
                Menu::Project,
                &[],
                project_open,
                slot(Menu::Project),
                |s, p| s.act_reimport_qet(p),
            ),
        ),
    ),
    hidden(parameterless(act(
        "file.reimport_qet_apply",
        Command,
        Menu::Project,
        &[],
        project_open,
        None,
        |s, _| s.act_reimport_qet_apply(),
    ))),
    // FT-410: File, Import, Parts table…
    with_params(
        "path?:path",
        placed(
            // Cycle 32: both imports in one group, Import (the window check), the parts table first.
            place(
                Group::Import,
                1,
                CommandSize::Large,
                Some(key!("bar-import-parts")),
            ),
            "database-import",
            act(
                "parts.import",
                Command,
                Menu::Project,
                &[],
                writable,
                None,
                |s, p| s.act_parts_import(p),
            ),
        ),
    ),
    // FT-649: the parts import's column mapping step (the card's fields and its Import button).
    with_params(
        "source?:text, column?:text, member?:text",
        hidden(act(
            "parts.map",
            Command,
            Menu::Project,
            &[],
            parts_map_open,
            None,
            |s, p| s.act_parts_map(p),
        )),
    ),
    hidden(parameterless(act(
        "parts.map_import",
        Command,
        Menu::Project,
        &[],
        parts_map_open,
        None,
        |s, _| s.act_parts_map_import(),
    ))),
    with_icon(
        "device-floppy",
        act(
            "file.save",
            Command,
            Menu::File,
            &["Ctrl+S"],
            writable,
            slot_sep(Menu::File),
            |s, _| s.act_save(),
        ),
    ),
    // ID-1463: while the symbol editor shows, Save is the symbol's and the File menu keeps Save project.
    hidden(parameterless(act(
        "file.save_project",
        Command,
        Menu::File,
        &[],
        writable,
        None,
        |s, _| s.act_save_project(),
    ))),
    with_params(
        "dir?:path",
        act(
            "file.save_as",
            Command,
            Menu::File,
            &["Ctrl+Shift+S"],
            project_open,
            slot(Menu::File),
            |s, p| s.act_save_as(p),
        ),
    ),
    answers_recovery(keeps_banner(with_params(
        "confirm?:bool",
        act(
            "file.discard_journal",
            Command,
            Menu::File,
            &[],
            can_discard_journal,
            slot(Menu::File),
            |s, p| s.act_discard_journal(p),
        ),
    ))),
    // WP-S2.09 part 1: the export runs as a job (§6.3); `replace` confirms an existing file, `wait` (headless) returns at
    // once.
    with_params(
        "path?:path, replace?:bool, wait?:bool",
        placed(
            place(Group::Pdf, 1, CommandSize::Large, None),
            "file-type-pdf",
            act(
                "file.export_pdf",
                Command,
                Menu::Output,
                &["Ctrl+Shift+E"],
                has_pages,
                None,
                |s, p| s.act_export_pdf(p),
            ),
        ),
    ),
    // WP-S1.17 part 7 (D24): File ▸ Export .ewz…, the saved generation as one file.
    with_params(
        "path?:path, replace?:bool",
        act(
            "file.export_ewz",
            Command,
            Menu::File,
            &[],
            project_open,
            slot(Menu::File),
            |s, p| s.act_export_ewz(p),
        ),
    ),
    // WP-S1.17 part 7 (D20, D24): an external change now, as the window's poll finds it; the upgrade dialog's Upgrade
    // and save; the collision dialog's buttons and choices; Set up merge driver.
    keeps_banner(parameterless(act(
        "project.check_disk",
        Command,
        Menu::File,
        &[],
        project_open,
        slot(Menu::File),
        |s, _| s.act_check_disk(),
    ))),
    hidden(parameterless(act(
        "file.upgrade",
        Command,
        Menu::File,
        &[],
        project_open,
        None,
        |s, _| s.act_upgrade(),
    ))),
    keeps_banner(with_params(
        "take?:enum(mine|theirs)",
        hidden(act(
            "external.apply",
            Command,
            Menu::File,
            &[],
            project_open,
            None,
            |s, p| s.act_external_apply(p),
        )),
    )),
    keeps_banner(hidden(parameterless(act(
        "external.review",
        View,
        Menu::File,
        &[],
        project_open,
        None,
        |s, _| s.act_external_review(),
    )))),
    // FT-1499 (ID-1486): the damaged-copy banner's Update from library.
    hidden(parameterless(act(
        "library.update_damaged",
        Command,
        Menu::File,
        &[],
        damaged_update_offered,
        None,
        |s, _| s.act_update_damaged(),
    ))),
    keeps_banner(with_params(
        "row:uint, take:enum(mine|theirs)",
        hidden(act(
            "external.choose",
            View,
            Menu::File,
            &[],
            project_open,
            None,
            |s, p| s.act_external_choose(p),
        )),
    )),
    with_params(
        "confirm?:bool",
        act(
            "git.setup",
            Command,
            Menu::Project,
            &[],
            project_open,
            slot(Menu::Project),
            |s, p| s.act_git_setup(p),
        ),
    ),
    // WP-S3.07 part 3 (D10, ruling 5 of ID-1180): the print pre-dialog, its options settable at once; it builds its
    // print set as a job (`wait`, headless: return at once).
    with_params(
        "range?:enum(all|current|sections|pages), sections?:text, pages?:text, paper?:text, fit?:bool, mono?:bool, wait?:bool",
        placed(
            place(Group::Pdf, 2, CommandSize::Small, None),
            "printer",
            act(
                "file.print",
                View,
                Menu::Output,
                &["Ctrl+P"],
                has_pages,
                None,
                |s, p| s.act_file_print(p),
            ),
        ),
    ),
    with_params(
        "range?:enum(all|current|sections|pages), sections?:text, pages?:text, paper?:text, fit?:bool, mono?:bool, wait?:bool",
        hidden(act(
            "print.set",
            View,
            Menu::Output,
            &[],
            print_dialog_open,
            None,
            |s, p| s.act_print_set(p),
        )),
    ),
    with_params(
        "to?:enum(next|previous), page?:uint",
        hidden(act(
            "print.page",
            View,
            Menu::Output,
            &[],
            print_dialog_open,
            None,
            |s, p| s.act_print_page(p),
        )),
    ),
    with_params(
        "via?:enum(printer|viewer)",
        hidden(act(
            "print.send",
            View,
            Menu::Output,
            &[],
            print_dialog_open,
            None,
            |s, p| s.act_print_send(p),
        )),
    ),
    with_params(
        "discard?:bool",
        act(
            "file.close",
            Command,
            Menu::File,
            &["Ctrl+Shift+W"],
            project_open,
            slot_sep(Menu::File),
            |s, p| s.act_close(p),
        ),
    ),
    answers_recovery(with_params(
        "discard?:bool",
        act(
            "app.quit",
            Command,
            Menu::File,
            &["Ctrl+Q"],
            always,
            slot(Menu::File),
            |s, p| s.act_quit(p),
        ),
    )),
    with_icon(
        "arrow-back-up",
        act(
            "edit.undo",
            Command,
            Menu::Edit,
            &["Ctrl+Z"],
            can_undo,
            slot(Menu::Edit),
            |s, _| s.act_undo(),
        ),
    ),
    with_icon(
        "arrow-forward-up",
        act(
            "edit.redo",
            Command,
            Menu::Edit,
            &["Ctrl+Shift+Z", "Ctrl+Y"],
            can_redo,
            slot(Menu::Edit),
            |s, _| s.act_redo(),
        ),
    ),
    // WP-S2.09 part 3 (§8.4): Edit, History.
    keeps_banner(with_params(
        "on?:bool",
        placed(
            place(Group::Panels, 4, CommandSize::Small, None),
            "history",
            act(
                "history.show",
                View,
                Menu::View,
                &[],
                project_open,
                None,
                |s, p| s.act_history_show(p),
            ),
        ),
    )),
    // Doc 16a §5.7: View ▸ Bottom dock (Ctrl+J) closes the bottom dock and opens it again with the panes it had.
    keeps_banner(with_params(
        "on?:bool",
        act(
            "view.bottom_dock",
            View,
            Menu::View,
            &["Ctrl+J"],
            project_open,
            grp(Group::Panels),
            |s, p| s.act_bottom_dock(p),
        ),
    )),
    // Edit, Properties of selection (E and Alt+Enter on the canvas) and Project properties: the shell has no Project
    // menu yet, so the project's properties sit beside the selection's (WP-S2.05 §6.10, lane ruling).
    keeps_banner(in_canvas(act(
        "props.focus",
        View,
        Menu::Edit,
        &["E", "Alt+Enter"],
        project_open,
        slot_sep(Menu::Edit),
        |s, _| s.act_props_focus(),
    ))),
    keeps_banner(placed(
        place(
            Group::Project,
            1,
            CommandSize::Large,
            Some(key!("bar-properties")),
        ),
        "adjustments",
        act(
            "props.project",
            View,
            Menu::Project,
            &[],
            project_open,
            None,
            |s, _| s.act_props_project(),
        ),
    )),
    // WP-S2.05 part 2: Edit, Find…, Find and replace… and Replace all (§7.6). macOS's ⌥⌘F (doc 16 §4.4) needs a
    // per-platform binding: `Ctrl+Alt+F` would be a Ctrl+Alt+letter chord, which NFR-I18N-06 forbids.
    keeps_banner(act(
        "find.open",
        View,
        Menu::Edit,
        &["Ctrl+F"],
        project_open,
        slot_sep(Menu::Edit),
        |s, _| s.act_find_open(false),
    )),
    keeps_banner(act(
        "find.open_replace",
        View,
        Menu::Edit,
        &["Ctrl+H"],
        project_open,
        slot(Menu::Edit),
        |s, _| s.act_find_open(true),
    )),
    keeps_banner(in_find(act(
        "find.replace_all",
        Command,
        Menu::Edit,
        &["Ctrl+Enter"],
        find_writable,
        slot(Menu::Edit),
        |s, _| s.act_find_replace_all(),
    ))),
    // FT-408: Page, New page.
    with_params(
        "folio:text, type?:enum(title_page|table_of_contents|section_divider|graphic|single_line|multi_line|overview|bus_topology|fluid|assembly_overview|panel_layout|model_view|external_document), title?:text, section?:text, grid?:enum(mm4|qet_10px)",
        placed(
            place(Group::Pages, 1, CommandSize::Large, None),
            "file-plus",
            act("page.new", Command, Menu::Page, &[], writable, None, |s, p| {
                s.act_page_new(p)
            }),
        ),
    ),
    // WP-S2.09 part 3 (§8.3): Page, Renumber pages…
    with_params(
        "section?:text, start?:uint, step?:uint, keep_gaps?:bool",
        placed(
            place(Group::Order, 1, CommandSize::Small, None),
            "list-numbers",
            act(
                "pages.renumber",
                Command,
                Menu::Page,
                &[],
                writable,
                None,
                |s, p| s.act_pages_renumber(p),
            ),
        ),
    ),
    act(
        "page.next",
        View,
        Menu::Page,
        &["Page Down"],
        has_next,
        grp(Group::Go),
        |s, _| s.walk(s.page_index() + 1),
    ),
    act(
        "page.previous",
        View,
        Menu::Page,
        &["Page Up"],
        has_previous,
        grp(Group::Go),
        |s, _| s.walk(s.page_index().saturating_sub(1)),
    ),
    act(
        "page.first",
        View,
        Menu::Page,
        &[],
        has_pages,
        grp(Group::Go),
        |s, _| s.walk(0),
    ),
    act(
        "page.last",
        View,
        Menu::Page,
        &[],
        has_pages,
        grp(Group::Go),
        |s, _| s.walk(s.page_count().saturating_sub(1)),
    ),
    with_params(
        "text?:text",
        placed(
            place(Group::Go, 1, CommandSize::Large, Some(key!("bar-go-to"))),
            "current-location",
            act(
                "nav.goto",
                View,
                Menu::Page,
                &["Ctrl+G"],
                has_pages,
                None,
                |s, p| s.act_nav_goto(p),
            ),
        ),
    ),
    keeps_banner(placed(
        place(Group::Go, 2, CommandSize::Small, None),
        "arrow-left",
        act(
            "nav.back",
            View,
            Menu::Page,
            &["Alt+Arrow Left"],
            can_go_back,
            None,
            |s, _| s.act_history(true),
        ),
    )),
    keeps_banner(placed(
        place(Group::Go, 3, CommandSize::Small, None),
        "arrow-right",
        act(
            "nav.forward",
            View,
            Menu::Page,
            &["Alt+Arrow Right"],
            can_go_forward,
            None,
            |s, _| s.act_history(false),
        ),
    )),
    keeps_banner(with_params(
        "x_mm?:mm, y_mm?:mm, group?:uint",
        in_canvas(placed(
            place(
                Group::Go,
                4,
                CommandSize::Small,
                Some(key!("bar-follow-reference")),
            ),
            "link",
            act("nav.follow", View, Menu::Page, &["F"], has_pages, None, |s, p| {
                s.act_follow(p)
            }),
        )),
    )),
    keeps_banner(with_params(
        "group?:uint",
        in_canvas(act(
            "nav.next_link",
            View,
            Menu::Page,
            &["Shift+F"],
            has_pages,
            grp(Group::Go),
            |s, p| s.act_link_step(p, true),
        )),
    )),
    keeps_banner(with_params(
        "group?:uint",
        in_canvas(act(
            "nav.previous_link",
            View,
            Menu::Page,
            &["Ctrl+Shift+F"],
            has_pages,
            grp(Group::Go),
            |s, p| s.act_link_step(p, false),
        )),
    )),
    // Keyboard tool input (WP-S2.03 §7.7): the connection points in reading order; `[` and `]` are matched as the
    // physical keys by the window, so they need no AltGr on QWERTZ and AZERTY (doc 16 §4.4).
    keeps_banner(with_params(
        "group?:uint",
        in_canvas(act(
            "cp.next",
            View,
            Menu::Page,
            &["]", "Alt+N"],
            has_pages,
            grp(Group::Go),
            |s, p| s.act_cp_step(p, crate::shell::tools::CpStep::Next),
        )),
    )),
    keeps_banner(with_params(
        "group?:uint",
        in_canvas(act(
            "cp.previous",
            View,
            Menu::Page,
            &["[", "Alt+Shift+N"],
            has_pages,
            grp(Group::Go),
            |s, p| s.act_cp_step(p, crate::shell::tools::CpStep::Previous),
        )),
    )),
    keeps_banner(with_params(
        "group?:uint",
        in_canvas(act(
            "cp.next_unconnected",
            View,
            Menu::Page,
            &["N"],
            has_pages,
            grp(Group::Go),
            |s, p| s.act_cp_step(p, crate::shell::tools::CpStep::NextUnconnected),
        )),
    )),
    keeps_banner(with_params(
        "alt?:bool",
        in_canvas(act(
            "canvas.activate",
            View,
            Menu::Page,
            &["Enter"],
            has_pages,
            None,
            |s, p| s.act_activate_with(p),
        )),
    )),
    keeps_banner(in_canvas(act(
        "tool.back",
        View,
        Menu::Page,
        &["Backspace"],
        has_pages,
        None,
        |s, _| s.act_tool_back(),
    ))),
    keeps_banner(with_params(
        "group?:uint",
        placed(
            place(Group::Zoom, 2, CommandSize::Small, None),
            "zoom-in",
            act(
                "view.zoom_in",
                View,
                Menu::View,
                &["Ctrl+Plus", "Ctrl+Equals"],
                can_zoom_in,
                None,
                |s, p| s.act_zoom_in(p),
            ),
        ),
    )),
    keeps_banner(with_params(
        "group?:uint",
        placed(
            place(Group::Zoom, 3, CommandSize::Small, None),
            "zoom-out",
            act(
                "view.zoom_out",
                View,
                Menu::View,
                &["Ctrl+Minus"],
                can_zoom_out,
                None,
                |s, p| s.act_zoom_out(p),
            ),
        ),
    )),
    keeps_banner(with_params(
        "group?:uint",
        placed(
            place(Group::Zoom, 4, CommandSize::Small, None),
            "zoom-reset",
            act(
                "view.zoom_actual",
                View,
                Menu::View,
                &["Ctrl+phys:Digit0"],
                has_pages,
                None,
                |s, p| s.act_zoom_actual(p),
            ),
        ),
    )),
    keeps_banner(with_params(
        "group?:uint",
        placed(
            place(Group::Zoom, 1, CommandSize::Large, None),
            "maximize",
            act(
                "view.fit_page",
                View,
                Menu::View,
                &["Home"],
                has_pages,
                None,
                |s, p| s.act_fit_page(p),
            ),
        ),
    )),
    keeps_banner(with_params(
        "group?:uint",
        act(
            "view.pan_left",
            View,
            Menu::View,
            &["Ctrl+Arrow Left"],
            has_pages,
            grp_sub(Group::Zoom, Submenu::Pan),
            |s, p| s.act_pan_step(p, -1.0, 0.0),
        ),
    )),
    keeps_banner(with_params(
        "group?:uint",
        act(
            "view.pan_right",
            View,
            Menu::View,
            &["Ctrl+Arrow Right"],
            has_pages,
            grp_sub(Group::Zoom, Submenu::Pan),
            |s, p| s.act_pan_step(p, 1.0, 0.0),
        ),
    )),
    keeps_banner(with_params(
        "group?:uint",
        act(
            "view.pan_up",
            View,
            Menu::View,
            &["Ctrl+Arrow Up"],
            has_pages,
            grp_sub(Group::Zoom, Submenu::Pan),
            |s, p| s.act_pan_step(p, 0.0, -1.0),
        ),
    )),
    keeps_banner(with_params(
        "group?:uint",
        act(
            "view.pan_down",
            View,
            Menu::View,
            &["Ctrl+Arrow Down"],
            has_pages,
            grp_sub(Group::Zoom, Submenu::Pan),
            |s, p| s.act_pan_step(p, 0.0, 1.0),
        ),
    )),
    keeps_banner(placed(
        place(Group::Layout, 1, CommandSize::Small, None),
        "layout-columns",
        act(
            "view.side_by_side",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, _| s.act_side_by_side(),
        ),
    )),
    keeps_banner(with_params(
        "mode:enum(flat|section|structure)",
        needs_parameters(act(
            "pages.mode",
            View,
            Menu::View,
            &[],
            has_pages,
            grp_sub(Group::Panels, Submenu::PageNavigator),
            |s, p| s.act_pages_mode(p),
        )),
    )),
    keeps_banner(with_params(
        "by:enum(folio|type)",
        // FT-1174: the palette row's icon (doc 16a §6.14).
        needs_parameters(with_icon(
            "arrows-sort",
            act(
                "pages.sort",
                View,
                Menu::View,
                &[],
                has_pages,
                grp_sub(Group::Panels, Submenu::PageNavigator),
                |s, p| s.act_pages_sort(p),
            ),
        )),
    )),
    keeps_banner(placed(
        place(Group::Show, 1, CommandSize::Small, None),
        "grid-dots",
        act("view.grid", View, Menu::View, &[], has_pages, None, |s, _| {
            s.act_grid()
        }),
    )),
    keeps_banner(placed(
        place(Group::Show, 3, CommandSize::Small, None),
        "chart-line",
        act("view.frame_stats", View, Menu::View, &[], always, None, |s, _| {
            s.act_frame_stats()
        }),
    )),
    act(
        "view.cpu_canvas",
        View,
        Menu::View,
        &[],
        always,
        grp(Group::Show),
        |s, _| s.act_cpu_canvas(),
    ),
    // WP-S2.08 part 6 (D18): View ▸ Wire colours, a view toggle of the session.
    keeps_banner(with_params(
        "on?:bool",
        act(
            "view.wire_colours",
            View,
            Menu::View,
            &[],
            has_pages,
            slot(Menu::View),
            |s, p| s.act_wire_colours(p),
        ),
    )),
    // The property grid (WP-S2.05 part 1, spec §6.10).
    keeps_banner(with_params(
        "on?:bool",
        placed(
            place(Group::Panels, 2, CommandSize::Small, None),
            "layout-sidebar-right",
            act("props.show", View, Menu::View, &[], project_open, None, |s, p| {
                s.act_props_show(p)
            }),
        ),
    )),
    // WP-S2.17 part 3 (§4.3, D11, D15): the Advanced rows of the property grid, and the rails' own control.
    with_params(
        "on?:bool",
        act(
            "props.show_advanced",
            View,
            Menu::View,
            &[],
            always,
            grp(Group::Panels),
            |s, p| s.act_props_show_advanced(p),
        ),
    ),
    with_params(
        "side?:enum(left|right), pane:text",
        hidden(act(
            "rail.show",
            View,
            Menu::View,
            &[],
            project_open,
            None,
            |s, p| s.act_rail_show(p),
        )),
    ),
    // WP-S2.09 part 1 (§6.8): View ▸ Jobs and View ▸ Notifications.
    keeps_banner(with_params(
        "on?:bool",
        placed(
            place(Group::Jobs, 1, CommandSize::Small, None),
            "activity",
            act(
                "jobs.show",
                View,
                Menu::Output,
                &[],
                project_open,
                None,
                |s, p| s.act_jobs_show(p),
            ),
        ),
    )),
    // FT-1372 (ID-1362): Output ▸ Reports ▸ Report block…, `report_block.create` through its form.
    with_params(
        "kind:text, folio:text, title?:text",
        act(
            "report_block.new",
            Command,
            Menu::Output,
            &[],
            writable,
            grp(Group::Reports),
            |s, p| s.act_report_block_new(p),
        ),
    ),
    // FT-717: Output ▸ Parts list opens the first page of the project's parts list (the first `parts_summary` block,
    // else the first `parts_list` block, in page order).
    placed(
        place(Group::Lists, 1, CommandSize::Large, Some(key!("bar-lists-parts"))),
        "list-details",
        act(
            "lists.parts",
            View,
            Menu::Output,
            &[],
            has_parts_list,
            None,
            |s, _| s.act_lists_parts(),
        ),
    ),
    keeps_banner(with_params(
        "on?:bool",
        placed(
            place(Group::Panels, 5, CommandSize::Small, None),
            "bell",
            act(
                "notifications.show",
                View,
                Menu::View,
                &[],
                always,
                None,
                |s, p| s.act_notifications_show(p),
            ),
        ),
    )),
    // WP-S3.09 D9 d (T11): the Messages pane's choice of the migration report beside the project's scheme.
    with_params(
        "on?:bool",
        hidden(act(
            "messages.migration",
            View,
            Menu::Check,
            &[],
            project_open,
            None,
            |s, p| s.act_messages_migration(p),
        )),
    ),
    // WP-S2.09 part 2 (§7.6): View ▸ Messages, F8 and Shift+F8, the quick fix of a finding row.
    with_params(
        "on?:bool",
        placed(
            place(Group::Findings, 1, CommandSize::Large, None),
            "messages",
            act(
                "messages.show",
                View,
                Menu::Check,
                &[],
                project_open,
                None,
                |s, p| s.act_messages_show(p),
            ),
        ),
    ),
    placed(
        place(Group::Findings, 2, CommandSize::Small, None),
        "arrow-narrow-down",
        act(
            "messages.next",
            View,
            Menu::Check,
            &["F8"],
            project_open,
            None,
            |s, _| s.act_messages_walk(true),
        ),
    ),
    placed(
        place(Group::Findings, 3, CommandSize::Small, None),
        "arrow-narrow-up",
        act(
            "messages.previous",
            View,
            Menu::Check,
            &["Shift+F8"],
            project_open,
            None,
            |s, _| s.act_messages_walk(false),
        ),
    ),
    with_params(
        "key?:text",
        hidden(in_navigator(act(
            "messages.fix",
            Command,
            Menu::Check,
            &[],
            writable,
            None,
            |s, p| s.act_messages_fix(p),
        ))),
    ),
    // FT-674: Waive… and Unwaive in the Messages detail (`check.waive`, `check.unwaive`; window only).
    with_params(
        "key?:text, reason?:text, author?:text, date?:text",
        hidden(in_navigator(act(
            "messages.waive",
            Command,
            Menu::Check,
            &[],
            writable,
            None,
            |s, p| s.act_messages_waive(p),
        ))),
    ),
    with_params(
        "key?:text",
        hidden(in_navigator(act(
            "messages.unwaive",
            Command,
            Menu::Check,
            &[],
            writable,
            None,
            |s, p| s.act_messages_unwaive(p),
        ))),
    ),
    with_params(
        "scale:json",
        needs_parameters(placed(
            place(Group::Appearance, 3, CommandSize::Small, None),
            "text-size",
            act(
                "view.ui_scale",
                View,
                Menu::View,
                &[],
                always,
                sub(Menu::View, Submenu::UiScale, false),
                |s, p| s.act_ui_scale(p),
            ),
        )),
    ),
    // WP-S2.08 D14 (ID-1272 ruling 5): View ▸ Minimum text size (`pt` 0 is off, else 8 to 16) and View ▸ Reduce motion,
    // the settings `min_font_pt` and `reduce_motion`.
    with_params(
        "pt:uint",
        needs_parameters(act(
            "view.min_font",
            View,
            Menu::View,
            &[],
            always,
            sub(Menu::View, Submenu::MinFont, true),
            |s, p| s.act_min_font(p),
        )),
    ),
    with_params(
        "on?:bool",
        act(
            "view.reduce_motion",
            View,
            Menu::View,
            &[],
            always,
            slot(Menu::View),
            |s, p| s.act_reduce_motion(p),
        ),
    ),
    // FT-1439 (ID-1397): View ▸ Mouse wheel scrolls, the setting `wheel_scrolls`; off, the wheel zooms.
    with_params(
        "on?:bool",
        act(
            "view.wheel_scrolls",
            View,
            Menu::View,
            &[],
            always,
            slot(Menu::View),
            |s, p| s.act_wheel_scrolls(p),
        ),
    ),
    // FT-1280 (ID-1315 ruling 3): View ▸ Show tips, the setting `tips` (FR-UI-048), beside Reduce motion. FT-1367
    // (ID-1392): a switch of the View tab's Show group too, so its menu item is View ▸ Show ▸ Show tips.
    with_params(
        "on?:bool",
        placed(
            place(Group::Show, 4, CommandSize::Small, None),
            "info-circle",
            act(
                "view.tips",
                View,
                Menu::View,
                &[],
                always,
                slot(Menu::View),
                |s, p| s.act_tips(p),
            ),
        ),
    ),
    // WP-S2.17 part 1: View ▸ Theme and View ▸ Density (doc 16a §4.13, D5, D6). The theme words of the spec
    // (`hc-dark`, `hc-light`) are not words of the signature grammar, so `value` is text, checked against `THEMES`.
    with_params(
        "value:text",
        needs_parameters(placed(
            place(Group::Appearance, 1, CommandSize::Small, None),
            "sun-moon",
            act(
                "view.theme",
                View,
                Menu::View,
                &[],
                always,
                sub(Menu::View, Submenu::Theme, false),
                |s, p| s.act_view_setting("theme", p),
            ),
        )),
    ),
    with_params(
        "value:enum(comfortable|compact)",
        needs_parameters(placed(
            place(Group::Appearance, 2, CommandSize::Small, None),
            "baseline-density-medium",
            act(
                "view.density",
                View,
                Menu::View,
                &[],
                always,
                sub(Menu::View, Submenu::Density, false),
                |s, p| s.act_view_setting("density", p),
            ),
        )),
    ),
    // WP-S2.08 part 3 (D10, ID-1219 ruling 5): View ▸ Language switches the UI language at run time; `value` is a
    // value of the `language` setting.
    with_params(
        "value:text",
        // FT-1174: the palette row's icon (doc 16a §6.14).
        needs_parameters(with_icon(
            "language",
            act(
                "view.language",
                View,
                Menu::View,
                &[],
                always,
                sub(Menu::View, Submenu::Language, true),
                |s, p| s.act_view_setting("language", p),
            ),
        )),
    ),
    // WP-S2.17 part 2 (§4.3): the command bar's tab and its collapse (Ctrl+F1, a tab's double click), view actions.
    keeps_banner(with_params(
        "tab:enum(project|page|draw|check|output|view)",
        needs_parameters(act(
            "view.command_tab",
            View,
            Menu::View,
            &[],
            always,
            grp_sub(Group::Layout, Submenu::CommandTab),
            |s, p| s.act_command_tab(p),
        )),
    )),
    keeps_banner(with_params(
        "collapsed?:bool",
        act(
            "view.command_bar",
            View,
            Menu::View,
            &["Ctrl+F1"],
            always,
            grp(Group::Layout),
            |s, p| s.act_command_bar(p),
        ),
    )),
    with_params(
        "name:text",
        needs_parameters(placed(
            place(Group::Layout, 2, CommandSize::Small, Some(key!("bar-workspaces"))),
            "layout-board",
            act(
                "view.workspace_load",
                View,
                Menu::View,
                &[],
                always,
                sub(Menu::View, Submenu::Workspace, false),
                |s, p| s.act_workspace_load(p),
            ),
        )),
    ),
    with_params(
        "name:text",
        act(
            "view.workspace_save",
            View,
            Menu::View,
            &[],
            always,
            grp_sub(Group::Layout, Submenu::Workspace),
            |s, p| s.act_workspace_save(p),
        ),
    ),
    placed(
        place(Group::Layout, 3, CommandSize::Small, None),
        "restore",
        act(
            "view.workspace_reset",
            View,
            Menu::View,
            &[],
            always,
            None,
            |s, _| s.act_workspace_reset(),
        ),
    ),
    keeps_banner(with_params(
        "query?:text",
        with_icon(
            "search",
            act(
                "tools.palette",
                View,
                Menu::View,
                &["Ctrl+K", "Ctrl+Shift+P"],
                always,
                grp(Group::Panels),
                |s, p| s.act_palette(p),
            ),
        ),
    )),
    keeps_banner(placed(
        place(Group::Select, 1, CommandSize::Large, None),
        "pointer",
        act("tool.select", View, Menu::Draw, &[], has_pages, None, |s, _| {
            s.act_tool("select")
        }),
    )),
    keeps_banner(placed(
        place(Group::Annotate, 2, CommandSize::Small, None),
        "ruler-measure",
        act("tool.measure", View, Menu::Draw, &[], has_pages, None, |s, _| {
            s.act_tool("measure")
        }),
    )),
    // Place and arrange (WP-S2.04 part 1, §7.6): the Insert menu, the transform keys, Move, Delete, Change symbol.
    keeps_banner(in_canvas(placed(
        place(Group::Place, 1, CommandSize::Large, None),
        "circuit-switch-open",
        act(
            "tool.place",
            Command,
            Menu::Draw,
            &["I"],
            editable_page,
            None,
            |s, _| s.act_open_picker(crate::tools::PickerMode::Place),
        ),
    ))),
    keeps_banner(in_canvas(placed(
        place(Group::Place, 2, CommandSize::Large, Some(key!("bar-device"))),
        "package",
        act(
            "tool.place_part",
            Command,
            Menu::Draw,
            &["Shift+I"],
            editable_page,
            None,
            |s, _| s.act_open_picker(crate::tools::PickerMode::Parts),
        ),
    ))),
    keeps_banner(with_params(
        "letter?:text, tag?:text, dialog?:bool",
        act(
            "strip.new",
            Command,
            Menu::Draw,
            &[],
            editable_page,
            grp(Group::Place),
            |s, p| s.act_strip_new(p),
        ),
    )),
    // FT-409: the structure node and the external reference, one previewed command each.
    with_params(
        "kind:enum(functional_assignment|function|installation_site|location|document_kind|user_defined|type), designation:text, parent?:text",
        placed(
            place(Group::Project, 2, CommandSize::Small, Some(key!("bar-structure"))),
            "sitemap",
            act(
                "structure.new",
                Command,
                Menu::Project,
                &[],
                writable,
                None,
                |s, p| s.act_structure_new(p),
            ),
        ),
    ),
    with_params(
        "location:text, device:text, connection?:text, project?:text",
        act(
            "external_ref.new",
            Command,
            Menu::Project,
            &[],
            writable,
            grp(Group::Project),
            |s, p| s.act_external_ref_new(p),
        ),
    ),
    keeps_banner(with_params(
        "symbol?:text, part?:text, function?:ref, device?:ref, template?:uint, letter?:text, strip?:text, terminal_designation?:text, designation?:text",
        hidden(in_canvas(act(
            "place.pick",
            Command,
            Menu::Draw,
            &[],
            editable_page,
            None,
            |s, p| s.act_place_pick(p),
        ))),
    )),
    keeps_banner(with_params(
        "text:text",
        hidden(act(
            "picker.filter",
            View,
            Menu::Draw,
            &[],
            picker_open,
            None,
            |s, p| s.act_picker_filter(p),
        )),
    )),
    keeps_banner(with_params(
        "step?:int, to?:enum(first|last)",
        hidden(act(
            "picker.row",
            View,
            Menu::Draw,
            &[],
            picker_open,
            None,
            |s, p| s.act_picker_row(p),
        )),
    )),
    keeps_banner(hidden(with_params(
        // `arm` reads the optional device letter and, for a terminal, the strip and its designation
        // (`terminal_designation`; FT-543 keeps `designation` as its old name).
        "letter?:text, strip?:text, terminal_designation?:text, designation?:text",
        act(
            "picker.choose",
            Command,
            Menu::Draw,
            &[],
            picker_open,
            None,
            |s, p| s.act_picker_choose(p),
        ),
    ))),
    // ID-1477 (1): the current row, a symbol on another connection grid, placed on a new page with its grid.
    keeps_banner(hidden(parameterless(act(
        "picker.new_page",
        Command,
        Menu::Draw,
        &[],
        picker_open,
        None,
        |s, _| s.act_picker_new_page(),
    )))),
    // WP-S2.18 part 1 (D2): J opens the Place tool's Join chooser (doc 16a §6.22) while the armed symbol can be an
    // auxiliary function; the chooser's own keys are its actions.
    keeps_banner(hidden(in_canvas(parameterless(act(
        "place.join",
        Command,
        Menu::Draw,
        &["J"],
        place_joinable,
        None,
        |s, _| s.act_place_join(),
    ))))),
    keeps_banner(with_params(
        "step?:int, to?:enum(first|last)",
        hidden(act(
            "chooser.row",
            View,
            Menu::Draw,
            &[],
            chooser_open,
            None,
            |s, p| s.act_chooser_row(p),
        )),
    )),
    keeps_banner(with_params(
        "text:text",
        hidden(act(
            "chooser.type",
            View,
            Menu::Draw,
            &[],
            chooser_open,
            None,
            |s, p| s.act_chooser_type(p),
        )),
    )),
    keeps_banner(with_params(
        "row?:uint",
        hidden(act(
            "chooser.choose",
            Command,
            Menu::Draw,
            &[],
            chooser_open,
            None,
            |s, p| s.act_chooser_choose(p),
        )),
    )),
    keeps_banner(in_canvas(placed(
        place(Group::Arrange, 1, CommandSize::Small, Some(key!("bar-rotate"))),
        "rotate-clockwise",
        act(
            "tool.rotate_cw",
            Command,
            Menu::Draw,
            &["R"],
            editable_page,
            None,
            |s, _| s.act_tool_key(crate::tools::ToolKey::RotateCw),
        ),
    ))),
    keeps_banner(in_canvas(act(
        "tool.rotate_ccw",
        Command,
        Menu::Draw,
        &["Shift+R"],
        editable_page,
        grp(Group::Arrange),
        |s, _| s.act_tool_key(crate::tools::ToolKey::RotateCcw),
    ))),
    keeps_banner(in_canvas(placed(
        place(Group::Arrange, 2, CommandSize::Small, Some(key!("bar-mirror"))),
        "flip-vertical",
        act(
            "tool.mirror_x",
            Command,
            Menu::Draw,
            &["X"],
            editable_page,
            None,
            |s, _| s.act_tool_key(crate::tools::ToolKey::MirrorX),
        ),
    ))),
    keeps_banner(in_canvas(act(
        "tool.mirror_y",
        Command,
        Menu::Draw,
        &["Y"],
        editable_page,
        grp(Group::Arrange),
        |s, _| s.act_tool_key(crate::tools::ToolKey::MirrorY),
    ))),
    keeps_banner(in_canvas(act(
        "tool.next_variant",
        Command,
        Menu::Draw,
        &["V"],
        editable_page,
        grp(Group::Arrange),
        |s, _| s.act_tool_key(crate::tools::ToolKey::NextVariant),
    ))),
    keeps_banner(in_canvas(act(
        "tool.next_rep_type",
        View,
        Menu::Draw,
        &["Shift+V"],
        editable_page,
        grp(Group::Arrange),
        |s, _| s.act_tool_key(crate::tools::ToolKey::NextRepType),
    ))),
    keeps_banner(in_canvas(act(
        "tool.move",
        Command,
        Menu::Draw,
        &["M"],
        editable_page,
        grp(Group::Arrange),
        |s, _| s.act_tool_move(),
    ))),
    keeps_banner(in_canvas(act(
        "edit.delete_selection",
        Command,
        Menu::Edit,
        &["Delete"],
        editable_selection,
        slot_sep(Menu::Edit),
        |s, _| s.act_delete_selection(),
    ))),
    // WP-S2.05 part 3: the clipboard (§8.8). The chords are the canvas's: a text field of the property grid or the find
    // pane keeps egui's own copy and paste, and the window routes no chord while a text field has the keyboard.
    in_canvas(act(
        "edit.cut",
        Command,
        Menu::Edit,
        &["Ctrl+X"],
        selection_editable,
        slot(Menu::Edit),
        |s, _| s.act_cut(),
    )),
    in_canvas(act(
        "edit.copy",
        View,
        Menu::Edit,
        &["Ctrl+C"],
        selection_shown,
        slot(Menu::Edit),
        |s, _| s.act_copy(),
    )),
    with_params(
        "text?:text",
        in_canvas(with_icon(
            "clipboard-list",
            act(
                "edit.paste_clip",
                Command,
                Menu::Edit,
                &["Ctrl+V"],
                paste_page,
                slot(Menu::Edit),
                |s, p| s.act_paste_clip(p),
            ),
        )),
    ),
    in_canvas(act(
        "edit.duplicate",
        Command,
        Menu::Edit,
        &["Ctrl+D"],
        selection_editable,
        slot(Menu::Edit),
        |s, _| s.act_duplicate(),
    )),
    in_canvas(act(
        "edit.copy_tags",
        View,
        Menu::Edit,
        &[],
        selection_shown,
        slot(Menu::Edit),
        |s, _| s.act_copy_tags(),
    )),
    with_params(
        "text:text",
        headless(act(
            "clipboard.text",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_clipboard_text(p),
        )),
    ),
    keeps_banner(with_params(
        "symbol?:text",
        in_canvas(with_icon(
            "circuit-switch-open",
            act(
                "rep.exchange",
                Command,
                Menu::Draw,
                &[],
                editable_page,
                grp(Group::Arrange),
                |s, p| s.act_rep_exchange(p),
            ),
        )),
    )),
    keeps_banner(hidden(in_canvas(parameterless(act(
        "tool.finish",
        View,
        Menu::Edit,
        &["Shift+Enter"],
        has_pages,
        None,
        |s, p| s.act_tool_finish(p),
    ))))),
    keeps_banner(with_params(
        "text:text",
        hidden(in_canvas(act(
            "text.input",
            View,
            Menu::Edit,
            &[],
            has_pages,
            None,
            |s, p| s.act_text_input(p),
        ))),
    )),
    // WP-S2.04 part 2 (wiring, §8.7): the Wire tool, the proposals' Tab, route edit, the junction sequence editor and
    // the crossings setting. Begin.
    keeps_banner(in_canvas(placed(
        place(Group::Connect, 1, CommandSize::Large, Some(key!("bar-wire"))),
        "route-2",
        act(
            "tool.wire",
            Command,
            Menu::Draw,
            &["W"],
            wire_page,
            None,
            |s, _| s.act_tool_wire(),
        ),
    ))),
    // Ctrl+Enter: a click with Alt held at the grid cursor (a placement without its autoconnect proposals, a bend
    // without object snaps), the keyboard's twin of `canvas.activate {alt}` (fix round; per-context chords, ID-628).
    keeps_banner(hidden(in_canvas(parameterless(act(
        "canvas.activate_alt",
        View,
        Menu::Page,
        &["Ctrl+Enter"],
        has_pages,
        None,
        |s, p| s.act_canvas_activate_alt(p),
    ))))),
    keeps_banner(hidden(in_canvas(parameterless(act(
        "tool.next_proposal",
        View,
        Menu::Edit,
        &["Tab"],
        editable_page,
        None,
        |s, _| s.send_proposal_key(),
    ))))),
    // WP-S4.02 part 3 (D4, D10; doc 12 §12.4): placing on a panel page. Tab, R, X and the arrows keep their bindings:
    // on a panel page the shell maps them to these hidden actions (`ShellModel::shortcut_action`, ID-628); Shift+Tab is
    // `layout.previous`'s own canvas binding (ruling 5).
    with_params(
        "device:ref",
        hidden(act(
            "tool.place_layout",
            Command,
            Menu::Draw,
            &[],
            panel_page,
            None,
            |s, p| s.act_place_layout(p),
        )),
    ),
    hidden(in_canvas(parameterless(act(
        "layout.next",
        View,
        Menu::Edit,
        &[],
        panel_page,
        None,
        |s, _| s.act_layout_step(1),
    )))),
    hidden(in_canvas(parameterless(act(
        "layout.previous",
        View,
        Menu::Edit,
        &["Shift+Tab"],
        panel_page,
        None,
        |s, _| s.act_layout_step(-1),
    )))),
    with_params(
        "dx?:int, dy?:int",
        hidden(in_canvas(act(
            "layout.nudge",
            Command,
            Menu::Edit,
            &[],
            // A selected placement, or a floating layout ghost (the shell refuses without either).
            panel_page,
            None,
            |s, p| s.act_layout_nudge(p),
        ))),
    ),
    hidden(in_canvas(parameterless(act(
        "layout.rotate",
        Command,
        Menu::Edit,
        &[],
        panel_page,
        None,
        |s, _| s.act_layout_turn(crate::tools::ToolKey::RotateCw),
    )))),
    hidden(in_canvas(parameterless(act(
        "layout.flip",
        Command,
        Menu::Edit,
        &[],
        panel_page,
        None,
        |s, _| s.act_layout_turn(crate::tools::ToolKey::MirrorX),
    )))),
    hidden(in_canvas(parameterless(act(
        "layout.lock_selection",
        Command,
        Menu::Edit,
        &[],
        panel_selection,
        None,
        |s, _| s.act_layout_lock(true),
    )))),
    hidden(in_canvas(parameterless(act(
        "layout.unlock_selection",
        Command,
        Menu::Edit,
        &[],
        panel_selection,
        None,
        |s, _| s.act_layout_lock(false),
    )))),
    with_params(
        "mode?:enum(free|push|swap), grid?:uint",
        hidden(in_canvas(act(
            "layout.options",
            View,
            Menu::Edit,
            &[],
            panel_page,
            None,
            |s, p| s.act_layout_options(p),
        ))),
    ),
    // WP-S4.02 part 4 (D10, D11, ruling 1): Draw ▸ Layout (Rail, Duct, Auto-fill rail…) on a panel layout page with a
    // surface view, the draw tools' options card, and Project ▸ Enclosure….
    keeps_banner(in_canvas(placed(
        place(Group::PanelLayout, 1, CommandSize::Small, Some(key!("bar-rail"))),
        "ew-din-rail",
        act(
            "tool.rail_draw",
            Command,
            Menu::Draw,
            &[],
            panel_view,
            None,
            |s, _| s.act_tool_rail_draw(),
        ),
    ))),
    keeps_banner(in_canvas(placed(
        place(Group::PanelLayout, 2, CommandSize::Small, Some(key!("bar-duct"))),
        "ew-duct",
        act(
            "tool.duct_draw",
            Command,
            Menu::Draw,
            &[],
            panel_view,
            None,
            |s, _| s.act_tool_duct_draw(),
        ),
    ))),
    with_params(
        "rail?:text, order?:enum(schematic|dt)",
        placed(
            place(
                Group::PanelLayout,
                3,
                CommandSize::Small,
                Some(key!("bar-auto-fill")),
            ),
            "wand",
            act(
                "layout.auto_fill_rail",
                Command,
                Menu::Draw,
                &[],
                panel_view,
                None,
                |s, p| s.act_layout_auto_fill_rail(p),
            ),
        ),
    ),
    with_params(
        "part?:text, width?:text, height?:text",
        hidden(act(
            "layout.draw_options",
            View,
            Menu::Draw,
            &[],
            panel_view,
            None,
            |s, p| s.act_layout_draw_options(p),
        )),
    ),
    with_params(
        "location?:text, part?:text",
        placed(
            place(Group::Project, 3, CommandSize::Small, Some(key!("bar-enclosure"))),
            "door",
            act(
                "layout.enclosure_new",
                Command,
                Menu::Project,
                &[],
                writable,
                None,
                |s, p| s.act_layout_enclosure_new(p),
            ),
        ),
    ),
    keeps_banner(hidden(in_canvas(parameterless(act(
        "tool.connect_on_move",
        Command,
        Menu::Edit,
        &["C"],
        editable_page,
        None,
        |s, p| s.act_tool_connect_on_move(p),
    ))))),
    // ID-1497 (10): Edit route on the Draw tab's Connect group, beside Wire (the polyline: Wire has the route icon).
    keeps_banner(in_canvas(placed(
        place(Group::Connect, 2, CommandSize::Small, None),
        "ew-shape-polyline",
        act(
            "tool.route_edit",
            Command,
            Menu::Draw,
            &[],
            one_connection,
            grp(Group::Connect),
            |s, _| s.act_route_edit(),
        ),
    ))),
    // WP-S2.19 D13: Route page and Route selection after Edit route, previewed (no default key); after Interruption,
    // so the two stack in one column of their own (ID-1506 (6)). Each its own icon, neither the generated badge's
    // (ID-1509 (9), C1): the page as a whole, and a pull on the selected wires.
    keeps_banner(placed(
        place(Group::Connect, 7, CommandSize::Small, None),
        "file-settings",
        act(
            "route.page",
            Command,
            Menu::Draw,
            &[],
            editable_page,
            grp(Group::Connect),
            |s, _| s.act_route_page(),
        ),
    )),
    keeps_banner(placed(
        place(Group::Connect, 8, CommandSize::Small, None),
        "magnet",
        act(
            "route.selection",
            Command,
            Menu::Draw,
            &[],
            route_selection,
            grp(Group::Connect),
            |s, _| s.act_route_selection(),
        ),
    )),
    keeps_banner(in_canvas(act(
        "junction.open",
        View,
        Menu::Draw,
        &[],
        one_junction,
        grp(Group::Connect),
        |s, _| s.act_junction_open(),
    ))),
    keeps_banner(with_params(
        "from:uint, to:uint",
        hidden(act(
            "junction.member",
            Command,
            Menu::Edit,
            &[],
            junction_editor_open,
            None,
            |s, p| s.act_junction_member(p),
        )),
    )),
    keeps_banner(with_params(
        "style:enum(dot|bevel), orient?:uint",
        hidden(act(
            "junction.style",
            Command,
            Menu::Edit,
            &[],
            junction_editor_open,
            None,
            |s, p| s.act_junction_style(p),
        )),
    )),
    keeps_banner(with_params(
        "wiring:enum(star|chain|style_default)",
        hidden(act(
            "junction.wiring",
            Command,
            Menu::Edit,
            &[],
            junction_editor_open,
            None,
            |s, p| s.act_junction_wiring(p),
        )),
    )),
    keeps_banner(with_params(
        "on?:bool",
        act(
            "project.crossings",
            Command,
            Menu::Draw,
            &[],
            writable,
            grp(Group::Connect),
            |s, p| s.act_crossings(p),
        ),
    )),
    // WP-S2.04 part 2. End.
    // WP-S2.04 part 3 (interruption points and black boxes, §9.6). Begin.
    keeps_banner(in_canvas(placed(
        place(
            Group::Connect,
            6,
            CommandSize::Small,
            Some(key!("bar-interruption")),
        ),
        "ew-interruption-point",
        act(
            "tool.ip",
            Command,
            Menu::Draw,
            &["P"],
            editable_page,
            None,
            |s, _| s.act_tool_ip(),
        ),
    ))),
    keeps_banner(with_params(
        "symbol?:text, part?:text, template?:uint, letter?:text",
        in_canvas(placed(
            place(Group::Place, 4, CommandSize::Small, None),
            "ew-black-box",
            act(
                "tool.black_box",
                Command,
                Menu::Draw,
                &["B"],
                editable_page,
                None,
                |s, p| s.act_tool_black_box(p),
            ),
        )),
    )),
    keeps_banner(with_params(
        "direction?:enum(source|target|undirected)",
        in_canvas(act(
            "tool.ip_direction",
            View,
            Menu::Draw,
            &["D"],
            editable_page,
            grp(Group::Connect),
            |s, p| s.act_ip_direction(p),
        )),
    )),
    keeps_banner(in_canvas(act(
        "tool.star_source",
        View,
        Menu::Draw,
        &[],
        editable_page,
        grp(Group::Connect),
        |s, _| s.act_ip_key(crate::tools::ToolKey::ToggleStar),
    ))),
    keeps_banner(with_params(
        "text:text",
        hidden(in_canvas(act(
            "ip.name",
            View,
            Menu::Edit,
            &[],
            editable_page,
            None,
            |s, p| s.act_ip_name(p),
        ))),
    )),
    // `external`: an external reference's ID, alias or printed text (`JE-N1000-03:L`); empty clears it.
    keeps_banner(with_params(
        "external:text",
        hidden(in_canvas(act(
            "ip.external",
            View,
            Menu::Edit,
            &[],
            editable_page,
            None,
            |s, p| s.act_ip_external(p),
        ))),
    )),
    keeps_banner(with_params(
        "folio?:text, x_mm?:mm, y_mm?:mm",
        in_canvas(act(
            "ip.continue_on",
            Command,
            Menu::Draw,
            &[],
            one_ip,
            grp(Group::Connect),
            |s, p| s.act_ip_continue_on(p),
        )),
    )),
    keeps_banner(with_params(
        "by?:enum(name|potential), chain_order?:bool",
        in_canvas(act(
            "ip.arrange_selected",
            Command,
            Menu::Draw,
            &[],
            two_ips,
            grp(Group::Connect),
            |s, p| s.act_ip_arrange(p),
        )),
    )),
    keeps_banner(with_params(
        "grip:uint",
        hidden(in_canvas(act(
            "box.grip",
            Command,
            Menu::Edit,
            &[],
            one_box,
            None,
            |s, p| s.act_box_grip(p),
        ))),
    )),
    keeps_banner(in_canvas(act(
        "box.add_cp",
        Command,
        Menu::Draw,
        &[],
        one_box,
        grp(Group::Place),
        |s, _| s.act_box_add_cp(),
    ))),
    // WP-S2.04 part 3. End.
    // WP-S2.04 part 4b (text and graphic tools, §10.7). Begin.
    keeps_banner(in_canvas(placed(
        place(Group::Annotate, 1, CommandSize::Large, None),
        "typography",
        act(
            "tool.text",
            Command,
            Menu::Draw,
            &["T"],
            editable_page,
            None,
            |s, _| s.act_tool_text(),
        ),
    ))),
    keeps_banner(with_params(
        "shape?:enum(line|polyline|rect|circle|arc|polygon)",
        in_canvas(with_icon(
            "pencil",
            act(
                "tool.graphic",
                Command,
                Menu::Draw,
                &["L"],
                editable_page,
                grp_sub(Group::Annotate, Submenu::Graphic),
                |s, p| s.act_tool_graphic(p),
            ),
        )),
    )),
    keeps_banner(with_params(
        "bind:text",
        hidden(in_canvas(act(
            "text.bind",
            View,
            Menu::Edit,
            &[],
            editable_page,
            None,
            |s, p| s.act_text_bind(p),
        ))),
    )),
    keeps_banner(in_canvas(with_icon(
        "pencil",
        act(
            "text.edit",
            Command,
            Menu::Draw,
            &["F2"],
            one_free_text,
            grp(Group::Annotate),
            |s, _| s.act_text_edit(),
        ),
    ))),
    // FT-411: Edit ▸ Bridge terminals.
    keeps_banner(in_canvas(act(
        "strip.bridge",
        Command,
        Menu::Draw,
        &[],
        two_strip_terminals,
        grp(Group::Connect),
        |s, _| s.act_strip_bridge(),
    ))),
    // WP-S2.07 part 1: the strip editor (§6.9; 262 → 268).
    with_params(
        "strip?:text",
        act(
            "strip.edit",
            View,
            Menu::Edit,
            &[],
            project_open,
            slot(Menu::Edit),
            |s, p| s.act_strip_edit(p),
        ),
    ),
    with_params(
        "count?:uint, designation?:text",
        in_navigator(act(
            "strip.insert",
            Command,
            Menu::Edit,
            &["Insert"],
            strips_editor,
            None,
            |s, p| s.act_strip_insert(p),
        )),
    ),
    in_navigator(act(
        "strip.delete_rows",
        Command,
        Menu::Edit,
        &["Ctrl+Delete"],
        strips_editor,
        None,
        |s, _| s.act_strip_delete_rows(),
    )),
    // Alt+Arrow Up and Alt+Arrow Down stay bound to `pages.move_up` and `pages.move_down` (the keymap test forbids a
    // second binding); while the Terminal strips navigator has the focus they run this action with `step` -1 or 1
    // (`ShellModel::shortcut_action`, WP-S2.07 part 1 ruling).
    with_params(
        "step?:int, before?:text",
        in_navigator(act(
            "strip.move",
            Command,
            Menu::Edit,
            &[],
            strips_editor,
            None,
            |s, p| s.act_strip_move(p),
        )),
    ),
    with_params(
        "kind?:enum(end_plate|end_clamp|partition|marker|cover), part?:text, before?:bool",
        in_navigator(act(
            "strip.add_accessory",
            Command,
            Menu::Edit,
            &[],
            strips_editor,
            slot(Menu::Edit),
            |s, p| s.act_strip_add_accessory(p),
        )),
    ),
    in_navigator(act(
        "strip.unbridge",
        Command,
        Menu::Edit,
        &[],
        strips_editor,
        None,
        |s, _| s.act_strip_unbridge(),
    )),
    // WP-S2.07 part 2 (§7, FR-CBL-016): swap two cores of one cable. The navigators have no row menu yet, so it sits
    // in the Edit menu with the strip editor's gestures.
    in_navigator(act(
        "cable.swap_cores",
        Command,
        Menu::Edit,
        &[],
        cables_editor,
        slot(Menu::Edit),
        |s, _| s.act_cable_swap_cores(),
    )),
    // FT-490's generic half: Delete and F2 in a navigator tree (`table.clear` and `table.edit` in tree mode run these,
    // `ShellModel::shortcut_action`).
    with_params(
        "kind?:text",
        hidden(in_navigator(act(
            "navigator.delete",
            Command,
            Menu::Edit,
            &[],
            writable,
            None,
            |s, p| s.act_navigator_delete(p),
        ))),
    ),
    with_params(
        "kind?:text, text?:text, cancel?:bool",
        hidden(in_navigator(act(
            "navigator.rename",
            Command,
            Menu::Edit,
            &[],
            writable,
            None,
            |s, p| s.act_navigator_rename(p),
        ))),
    ),
    // G-PP triage row 7: Draw ▸ Bond, `bond.create` on the selected connection or two named CPs, previewed.
    keeps_banner(with_params(
        "kind?:enum(pelv_0v_pe|fe_pe|equipotential|shield), a?:text, b?:text",
        in_canvas(act(
            "bond.draw",
            Command,
            Menu::Draw,
            &[],
            bond_ends,
            grp_sub(Group::Connect, Submenu::Bond),
            |s, p| s.act_bond_draw(p),
        )),
    )),
    // FT-1202 (ID-1270): Draw ▸ Define potential…, beside Bond: `potential.define` at the selection's point, through
    // its parameter form.
    keeps_banner(with_params(
        "name?:text, type?:text, voltage?:text",
        in_canvas(act(
            "potential.draw",
            Command,
            Menu::Draw,
            &[],
            potential_point,
            grp(Group::Connect),
            |s, p| s.act_potential_draw(p),
        )),
    )),
    // FT-1371 (ID-1362): Draw ▸ Connect ▸ New cable… (`cable.create` from a cable part, with its cores) and Connect
    // cores… (the selected wires become the cable's cores, by order), each through its form.
    with_params(
        "part:text",
        act(
            "cable.new",
            Command,
            Menu::Draw,
            &[],
            writable,
            grp(Group::Connect),
            |s, p| s.act_cable_new(p),
        ),
    ),
    with_params(
        "cable:text",
        in_canvas(act(
            "cable.connect_cores",
            Command,
            Menu::Draw,
            &[],
            selection_editable,
            grp(Group::Connect),
            |s, p| s.act_cable_connect_cores(p),
        )),
    ),
    // FT-1239: Define potential… from a Potentials row's context menu, at the row's point.
    keeps_banner(with_params(
        "row:text",
        hidden(in_navigator(act(
            "potential.draw_row",
            Command,
            Menu::Draw,
            &[],
            writable,
            None,
            |s, p| s.act_potential_draw_row(p),
        ))),
    )),
    // WP-S2.04 part 4b: the Wire marker and Structure box tools, their fields and the marker mode.
    keeps_banner(in_canvas(with_icon(
        "ew-cable",
        act(
            "tool.marker",
            Command,
            Menu::Draw,
            &["Shift+W"],
            editable_page,
            grp(Group::Connect),
            |s, _| s.act_tool_marker(),
        ),
    ))),
    keeps_banner(with_params(
        "csa?:text, colours?:text",
        hidden(in_canvas(act(
            "marker.fields",
            View,
            Menu::Draw,
            &[],
            editable_page,
            None,
            |s, p| s.act_marker_fields(p),
        ))),
    )),
    keeps_banner(in_canvas(with_icon(
        "ew-structure-box",
        act(
            "tool.structure_box",
            Command,
            Menu::Draw,
            &["Shift+B"],
            editable_page,
            grp(Group::Annotate),
            |s, _| s.act_tool_structure_box(),
        ),
    ))),
    keeps_banner(with_params(
        "style?:enum(dotted|dash_dot), caption?:text, location?:text, external?:text",
        hidden(in_canvas(act(
            "structure_box.fields",
            View,
            Menu::Draw,
            &[],
            editable_page,
            None,
            |s, p| s.act_structure_box_fields(p),
        ))),
    )),
    keeps_banner(with_params(
        "mode:enum(per_wire|all|deviations|explicit|none)",
        act(
            "project.marker_mode",
            Command,
            Menu::Draw,
            &[],
            writable,
            grp_sub(Group::Connect, Submenu::WireMarkers),
            |s, p| s.act_marker_mode(p),
        ),
    )),
    // WP-S2.04 part 4b. End.
    // WP-S2.04 part 5 (images, §11).
    keeps_banner(in_canvas(with_icon(
        "file",
        act(
            "tool.image",
            Command,
            Menu::Draw,
            &[],
            editable_page,
            grp(Group::Annotate),
            |s, _| s.act_tool_image(),
        ),
    ))),
    keeps_banner(with_params(
        "asset:text",
        hidden(in_canvas(act(
            "image.pick",
            View,
            Menu::Draw,
            &[],
            editable_page,
            None,
            |s, p| s.act_image_pick(p),
        ))),
    )),
    keeps_banner(with_params(
        "path:path",
        hidden(in_canvas(act(
            "image.file",
            View,
            Menu::Draw,
            &[],
            editable_page,
            None,
            |s, p| s.act_image_file(p),
        ))),
    )),
    placed(
        place(
            Group::Standards,
            2,
            CommandSize::Small,
            Some(key!("bar-company-standard")),
        ),
        "building-factory-2",
        act(
            "tools.company_standard",
            Command,
            Menu::Project,
            &[],
            always,
            None,
            |s, _| s.act_wizard_open(),
        ),
    ),
    // WP-S2.09 part 2 (§7.6): Tools ▸ Run checks.
    placed(
        place(Group::Run, 1, CommandSize::Large, Some(key!("bar-check-project"))),
        "list-check",
        act(
            "messages.check",
            Command,
            Menu::Check,
            &["F7"],
            project_open,
            None,
            |s, _| s.act_messages_check(),
        ),
    ),
    // WP-S2.09 part 3 (§8.3, §8.6): Tools, Renumber devices… and Purge unused images…
    // WP-S2.08 D7: Ctrl+Shift+N, doc 16 §4.4's shell binding.
    with_params(
        "scope?:enum(project|page|selection), scheme?:text, include_frozen?:bool, only_nonconforming?:bool",
        placed(
            place(Group::Arrange, 4, CommandSize::Small, Some(key!("bar-renumber"))),
            "sort-ascending-numbers",
            act(
                "devices.renumber",
                Command,
                Menu::Draw,
                &["Ctrl+Shift+N"],
                writable,
                None,
                |s, p| s.act_devices_renumber(p),
            ),
        ),
    ),
    with_params(
        "confirm?:bool",
        act(
            "project.purge_assets",
            Command,
            Menu::Project,
            &[],
            writable,
            slot_sep(Menu::Project),
            |s, p| s.act_purge_assets(p),
        ),
    ),
    // WP-S1.18 part 5 (§9): Check ▸ Revision, the revision dialogs; each answered by its own action with parameters
    // (`confirm` submits the `HumanOnly` command, only from the window a person drives: cycle 41, ruling 1).
    // In the order of the workflow (FIX41b ruling 7): the author, the sign-off, the release, a withdrawal.
    with_params(
        "role?:text, confirm?:bool, cancel?:bool",
        act(
            "revision.set_author",
            Command,
            Menu::Check,
            &[],
            open_revision,
            sub(Menu::Check, Submenu::Revision, true),
            |s, p| s.act_revision_set_author(p),
        ),
    ),
    with_params(
        "role?:text, typed?:text, date?:text, confirm?:bool, cancel?:bool",
        act(
            "revision.approve",
            Command,
            Menu::Check,
            &[],
            open_revision,
            sub(Menu::Check, Submenu::Revision, true),
            |s, p| s.act_revision_approve(p),
        ),
    ),
    with_params(
        "typed?:text, git?:bool, date?:text, confirm?:bool, cancel?:bool",
        act(
            "revision.release",
            Command,
            Menu::Check,
            &[],
            open_revision,
            sub(Menu::Check, Submenu::Revision, true),
            |s, p| s.act_revision_release(p),
        ),
    ),
    with_params(
        "revision?:text, reason?:text, confirm?:bool, cancel?:bool",
        act(
            "revision.withdraw",
            Command,
            Menu::Check,
            &[],
            released_revision,
            sub(Menu::Check, Submenu::Revision, true),
            |s, p| s.act_revision_withdraw(p),
        ),
    ),
    with_params(
        "how?:enum(complete|discard), git?:bool, confirm?:bool, cancel?:bool",
        act(
            "revision.incomplete",
            Command,
            Menu::Check,
            &[],
            incomplete_release,
            sub(Menu::Check, Submenu::Revision, true),
            |s, p| s.act_revision_incomplete(p),
        ),
    ),
    keeps_banner(act(
        "tab.new",
        View,
        Menu::Window,
        &["Ctrl+T"],
        can_add_tab,
        slot(Menu::Window),
        |s, _| s.act_tab_new(),
    )),
    keeps_banner(act(
        "tab.close",
        View,
        Menu::Window,
        &["Ctrl+W"],
        can_close_tab,
        slot(Menu::Window),
        |s, _| s.act_tab_close(),
    )),
    keeps_banner(act(
        "tab.next",
        View,
        Menu::Window,
        &["Ctrl+Tab"],
        can_cycle_tabs,
        slot(Menu::Window),
        |s, _| s.act_tab_cycle(1),
    )),
    keeps_banner(act(
        "tab.previous",
        View,
        Menu::Window,
        &["Ctrl+Shift+Tab"],
        can_cycle_tabs,
        slot(Menu::Window),
        |s, _| s.act_tab_cycle(-1),
    )),
    keeps_banner(act(
        "tab.focus_other",
        View,
        Menu::Window,
        &["Ctrl+F6"],
        two_groups,
        slot(Menu::Window),
        |s, _| s.act_focus_other(),
    )),
    answers_recovery(keeps_banner(with_params(
        "topic?:text",
        act(
            "help.documentation",
            View,
            Menu::Help,
            &["F1"],
            always,
            slot(Menu::Help),
            |s, p| s.act_documentation(p),
        ),
    ))),
    with_params(
        "dir?:path, discard?:bool, tutorial?:text",
        act(
            "help.tutorials",
            Command,
            Menu::Help,
            &[],
            always,
            sub(Menu::Help, Submenu::Tutorials, false),
            |s, p| s.act_tutorials(p),
        ),
    ),
    with_params(
        "step?:enum(language|origin|privacy|appearance|start)",
        act(
            "help.first_run",
            View,
            Menu::Help,
            &[],
            always,
            slot_sep(Menu::Help),
            |s, p| s.act_first_run_open(p),
        ),
    ),
    keeps_banner(act(
        "help.shortcuts",
        View,
        Menu::Help,
        &[],
        always,
        slot(Menu::Help),
        |s, p| s.act_help_shortcuts(p),
    )),
    // WP-S2.08 part 6 (D16): Help ▸ Glossary, the glossary pane in the bottom dock, and its search text.
    keeps_banner(with_params(
        "on?:bool",
        act(
            "help.glossary",
            View,
            Menu::Help,
            &[],
            project_open,
            slot(Menu::Help),
            |s, p| s.act_help_glossary(p),
        ),
    )),
    keeps_banner(with_params(
        "text:text",
        hidden(act(
            "glossary.filter",
            View,
            Menu::Help,
            &[],
            project_open,
            None,
            |s, p| s.act_glossary_filter(p),
        )),
    )),
    // FT-1281: the glossary pane's row cursor (Up and Down in its field).
    keeps_banner(with_params(
        "step?:int, to?:uint",
        hidden(act(
            "glossary.row",
            View,
            Menu::Help,
            &[],
            project_open,
            None,
            |s, p| s.act_glossary_row(p),
        )),
    )),
    keeps_banner(act(
        "help.notices",
        View,
        Menu::Help,
        &[],
        always,
        slot(Menu::Help),
        |s, _| s.open_dialog(crate::shell::Dialog::Notices),
    )),
    keeps_banner(act(
        "help.about",
        View,
        Menu::Help,
        &[],
        always,
        slot(Menu::Help),
        |s, _| s.open_dialog(crate::shell::Dialog::About),
    )),
    with_params(
        "cmd:text, params?:json",
        needs_parameters(act(
            "command.run",
            Command,
            Menu::View,
            &[],
            writable,
            None,
            |s, p| s.act_command_run(p),
        )),
    ),
    keeps_link_preview(keeps_banner(with_params(
        "path:path, dpi?:uint",
        headless(act(
            "view.snapshot",
            View,
            Menu::View,
            &[],
            page_shown,
            None,
            |s, p| s.act_view_snapshot(p),
        )),
    ))),
    keeps_link_preview(keeps_banner(with_params(
        "path:path, group?:uint",
        headless(act(
            "canvas.snapshot",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_canvas_snapshot(p),
        )),
    ))),
    keeps_banner(with_params(
        "percent:json, x_mm?:mm, y_mm?:mm, group?:uint",
        hidden(act(
            "view.zoom_to",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_zoom_to(p),
        )),
    )),
    keeps_banner(with_params(
        "dx:json, dy:json, group?:uint",
        hidden(act("view.pan", View, Menu::View, &[], has_pages, None, |s, p| {
            s.act_pan(p)
        })),
    )),
    answers_recovery(keeps_banner(with_params(
        "width:json, height:json, ppp?:json, group?:uint",
        hidden(act(
            "view.canvas_resize",
            View,
            Menu::View,
            &[],
            always,
            None,
            |s, p| s.act_canvas_resize(p),
        )),
    ))),
    keeps_banner(with_params(
        "x_mm?:mm, y_mm?:mm, group?:uint, shift?:bool, ctrl?:bool, alt?:bool",
        hidden(act(
            "canvas.pointer",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_pointer(p),
        )),
    )),
    keeps_link_preview(keeps_banner(with_params(
        "path:path",
        headless(act(
            "canvas.preview_snapshot",
            View,
            Menu::View,
            &[],
            has_link_preview,
            None,
            |s, p| s.act_canvas_snapshot(p),
        )),
    ))),
    keeps_banner(with_params(
        "x_mm:mm, y_mm:mm, button?:enum(primary|middle), shift?:bool, ctrl?:bool, alt?:bool, group?:uint",
        hidden(in_canvas(act(
            "canvas.press",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_press(p, false),
        ))),
    )),
    keeps_banner(with_params(
        "x_mm:mm, y_mm:mm, button?:enum(primary|middle), shift?:bool, ctrl?:bool, alt?:bool, group?:uint",
        hidden(in_canvas(act(
            "canvas.release",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_press(p, true),
        ))),
    )),
    keeps_link_preview(keeps_banner(with_params(
        "ms:uint",
        hidden(in_canvas(act(
            "canvas.dwell",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_dwell(p),
        ))),
    ))),
    // FT-720: a script step that expects its inner step to be refused, so a refusal's window state can be framed.
    with_params(
        "action:text, params?:json",
        headless(act(
            "script.expect_refused",
            View,
            Menu::View,
            &[],
            always,
            None,
            |s, p| s.act_script_expect_refused(p),
        )),
    ),
    // FT-1409: a script step that expects its inner step to fail, so a failure's window state (a form's error) can be
    // framed.
    with_params(
        "action:text, params?:json",
        headless(act(
            "script.expect_failed",
            View,
            Menu::View,
            &[],
            always,
            None,
            |s, p| s.act_script_expect_failed(p),
        )),
    ),
    keeps_link_preview(answers_recovery(keeps_banner(headless(parameterless(act(
        "ui.describe",
        View,
        Menu::View,
        &[],
        always,
        None,
        |s, p| s.act_ui_describe(p),
    )))))),
    with_params(
        "folio:text",
        hidden(act(
            "page.goto",
            View,
            Menu::Page,
            &[],
            has_pages,
            None,
            |s, p| s.act_page_goto(p),
        )),
    ),
    keeps_banner(with_params(
        "group:uint, index:uint",
        hidden(act(
            "tab.activate",
            View,
            Menu::Window,
            &[],
            has_pages,
            None,
            |s, p| s.act_tab_activate(p),
        )),
    )),
    keeps_banner(with_params(
        "group:uint, index:uint, to_group:uint, to_index:uint",
        hidden(act(
            "tab.move",
            View,
            Menu::Window,
            &[],
            has_pages,
            None,
            |s, p| s.act_tab_move(p),
        )),
    )),
    with_params(
        "folio:text, group?:uint",
        hidden(act(
            "tab.open",
            View,
            Menu::Window,
            &[],
            has_pages,
            None,
            |s, p| s.act_tab_open(p),
        )),
    ),
    keeps_banner(with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.left",
            View,
            Menu::Page,
            &["Arrow Left"],
            has_pages,
            None,
            |s, p| s.act_cursor(p, -1, 0, false),
        ))),
    )),
    keeps_banner(with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.right",
            View,
            Menu::Page,
            &["Arrow Right"],
            has_pages,
            None,
            |s, p| s.act_cursor(p, 1, 0, false),
        ))),
    )),
    keeps_banner(with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.up",
            View,
            Menu::Page,
            &["Arrow Up"],
            has_pages,
            None,
            |s, p| s.act_cursor(p, 0, -1, false),
        ))),
    )),
    keeps_banner(with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.down",
            View,
            Menu::Page,
            &["Arrow Down"],
            has_pages,
            None,
            |s, p| s.act_cursor(p, 0, 1, false),
        ))),
    )),
    keeps_banner(with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.cell_left",
            View,
            Menu::Page,
            &["Shift+Arrow Left"],
            has_pages,
            None,
            |s, p| s.act_cursor(p, -1, 0, true),
        ))),
    )),
    keeps_banner(with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.cell_right",
            View,
            Menu::Page,
            &["Shift+Arrow Right"],
            has_pages,
            None,
            |s, p| s.act_cursor(p, 1, 0, true),
        ))),
    )),
    keeps_banner(with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.cell_up",
            View,
            Menu::Page,
            &["Shift+Arrow Up"],
            has_pages,
            None,
            |s, p| s.act_cursor(p, 0, -1, true),
        ))),
    )),
    keeps_banner(with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.cell_down",
            View,
            Menu::Page,
            &["Shift+Arrow Down"],
            has_pages,
            None,
            |s, p| s.act_cursor(p, 0, 1, true),
        ))),
    )),
    keeps_banner(with_params(
        "x_mm?:mm, y_mm?:mm, group?:uint",
        hidden(in_canvas(act(
            "cursor.set",
            View,
            Menu::Page,
            &[],
            has_pages,
            None,
            |s, p| s.act_cursor_set(p),
        ))),
    )),
    keeps_banner(with_params(
        "text:text",
        hidden(act(
            "pages.filter",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_pages_filter(p),
        )),
    )),
    keeps_banner(with_params(
        "key:text",
        hidden(in_navigator(act(
            "pages.toggle",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_pages_toggle(p),
        ))),
    )),
    keeps_banner(with_params(
        "key?:text, step?:int, to?:enum(first|last)",
        hidden(in_navigator(act(
            "pages.row",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_pages_row(p),
        ))),
    )),
    keeps_banner(with_params(
        "key?:text, folio?:text, new_tab?:bool",
        hidden(in_navigator(act(
            "pages.open",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_pages_open(p),
        ))),
    )),
    // WP-S2.06 part 1: the navigator framework (§6.9).
    keeps_banner(with_params(
        "kind:enum(pages|devices|connections|strips|cables|plc|potentials|ips|structure|parts|blocks|unplaced|macros|libraries|messages), on?:bool",
        needs_parameters(placed(
            place(Group::Panels, 1, CommandSize::Small, Some(key!("bar-navigators"))),
            "layout-sidebar",
            act(
                "navigator.show",
                View,
                Menu::View,
                &[],
                has_pages,
                sub(Menu::View, Submenu::Navigators, false),
                |s, p| s.act_navigator_show(p),
            ),
        )),
    )),
    keeps_banner(with_params(
        "kind:text, view:text",
        hidden(act(
            "navigator.view",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_navigator_view(p),
        )),
    )),
    keeps_banner(with_params(
        "kind:text, by:text, descending?:bool",
        hidden(act(
            "navigator.sort",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_navigator_sort(p),
        )),
    )),
    keeps_banner(with_params(
        "kind:text, text:text",
        hidden(act(
            "navigator.filter",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_navigator_filter(p),
        )),
    )),
    keeps_banner(with_params(
        "kind:text, key:text",
        hidden(in_navigator(act(
            "navigator.toggle",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_navigator_toggle(p),
        ))),
    )),
    keeps_banner(with_params(
        "kind:text, key?:text, step?:int, to?:enum(first|last), extend?:bool",
        hidden(in_navigator(act(
            "navigator.row",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_navigator_row(p),
        ))),
    )),
    keeps_banner(with_params(
        "kind:text, key?:text, new_tab?:bool",
        hidden(in_navigator(act(
            "navigator.open",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_navigator_open(p),
        ))),
    )),
    // WP-S1.19 part 6 (§11): Draw ▸ Insert macro… with the place tools; a macro row dropped on the canvas; the macro
    // card's value set; Edit ▸ Save as macro…; the Libraries navigator's Update…; a property row's Make variable.
    with_params(
        "key?:text",
        in_canvas(act(
            "macros.insert",
            Command,
            Menu::Draw,
            &[],
            editable_page,
            grp(Group::Place),
            |s, p| s.act_macros_insert(p),
        )),
    ),
    with_params(
        "key:text, x_mm:mm, y_mm:mm, value_set?:text, group?:uint",
        hidden(in_canvas(act(
            "macros.place",
            Command,
            Menu::Draw,
            &[],
            editable_page,
            None,
            |s, p| s.act_macros_place(p),
        ))),
    ),
    with_params(
        "name?:text",
        hidden(act(
            "macros.value_set",
            Command,
            Menu::Draw,
            &[],
            writable,
            None,
            |s, p| s.act_macros_value_set(p),
        )),
    ),
    // FT-785: an ad-hoc value of the macro card's variable `name` (empty or absent `value`: none).
    with_params(
        "name:text, value?:text",
        hidden(act(
            "macros.value",
            Command,
            Menu::Draw,
            &[],
            writable,
            None,
            |s, p| s.act_macros_value(p),
        )),
    ),
    with_params(
        "name?:text, key?:text, description?:text, library?:text, kind?:text, pages?:list(text), x_mm?:mm, \
         y_mm?:mm, value_sets?:json, confirm?:bool",
        in_canvas(act(
            "macros.save",
            Command,
            Menu::Edit,
            &[],
            macro_scope_shown,
            slot(Menu::Edit),
            |s, p| s.act_macros_save(p),
        )),
    ),
    with_params(
        "keys?:list(text)",
        in_navigator(act(
            "libraries.update",
            Command,
            Menu::Project,
            &[],
            writable,
            slot(Menu::Project),
            |s, p| s.act_libraries_update(p),
        )),
    ),
    with_params(
        "row?:text, name?:text",
        hidden(in_properties(act(
            "props.make_variable",
            View,
            Menu::Edit,
            &[],
            grid_shown,
            None,
            |s, p| s.act_props_make_variable(p),
        ))),
    ),
    // WP-S2.07 §8 (FR-PLC-020): a channel row dropped on the canvas binds the channel to the connection point under
    // the drop point with `plc.bind_channel`.
    with_params(
        "channel:ref, x_mm:mm, y_mm:mm, group?:uint",
        hidden(in_canvas(act(
            "navigator.bind",
            Command,
            Menu::Edit,
            &[],
            editable_page,
            None,
            |s, p| s.act_navigator_bind(p),
        ))),
    ),
    keeps_banner(with_params(
        "kind:text, keys:list(text), add?:bool",
        hidden(act(
            "navigator.select",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_navigator_select(p),
        )),
    )),
    keeps_banner(with_params(
        "kind?:text, key?:text",
        hidden(in_navigator(act(
            "navigator.mark",
            View,
            Menu::View,
            &["Space"],
            has_pages,
            None,
            |s, p| s.act_navigator_mark(p),
        ))),
    )),
    keeps_banner(with_params(
        "kind:text, prefix:text",
        hidden(in_navigator(act(
            "navigator.jump",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_navigator_jump(p),
        ))),
    )),
    hidden(in_navigator(parameterless(act(
        "navigator.apply",
        Command,
        Menu::Edit,
        &[],
        reorder_card,
        None,
        |s, _| s.act_navigator_apply(),
    )))),
    with_params(
        "pages?:list(text), before?:text, end?:bool",
        hidden(in_navigator(act(
            "pages.reorder",
            Command,
            Menu::Page,
            &[],
            pages_writable,
            None,
            |s, p| s.act_pages_reorder(p),
        ))),
    ),
    in_navigator(placed(
        place(Group::Order, 2, CommandSize::Small, Some(key!("bar-move-up"))),
        "arrow-up",
        parameterless(act(
            "pages.move_up",
            Command,
            Menu::Page,
            &["Alt+Arrow Up"],
            pages_writable,
            None,
            |s, _| s.act_pages_move(true),
        )),
    )),
    in_navigator(placed(
        place(Group::Order, 3, CommandSize::Small, Some(key!("bar-move-down"))),
        "arrow-down",
        parameterless(act(
            "pages.move_down",
            Command,
            Menu::Page,
            &["Alt+Arrow Down"],
            pages_writable,
            None,
            |s, _| s.act_pages_move(false),
        )),
    )),
    // WP-S2.06 part 2: potential and net highlighting (§7.4).
    keeps_banner(with_params(
        "kind?:text, key?:text, off?:bool",
        in_navigator(placed(
            place(Group::Trace, 1, CommandSize::Small, None),
            "highlight",
            act(
                "navigator.highlight",
                View,
                Menu::Check,
                &["Ctrl+Shift+H"],
                has_pages,
                None,
                |s, p| s.act_navigator_highlight(p),
            ),
        )),
    )),
    keeps_banner(placed(
        place(Group::Trace, 2, CommandSize::Small, None),
        "highlight-off",
        act(
            "view.clear_highlights",
            View,
            Menu::Check,
            &[],
            has_highlights,
            None,
            |s, _| s.act_clear_highlights(),
        ),
    )),
    // WP-S2.06 part 3: table mode (§8.8).
    keeps_banner(with_params(
        "kind:text, on?:bool",
        hidden(act(
            "navigator.table",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_navigator_table(p),
        )),
    )),
    // WP-S3.08 part 3 (D14, ruling 2 of ID-1224): the table on show to a file as a job, and a round trip's file back
    // through the Preview tab; in the table's Columns menu and the Output menu.
    with_params(
        "path?:path, format?:enum(xlsx|csv), preset?:text, replace?:bool, wait?:bool",
        act(
            "navigator.export_table",
            Command,
            Menu::Output,
            &[],
            table_shown,
            grp(Group::Lists),
            |s, p| s.act_export_table(p),
        ),
    ),
    with_params(
        "path?:path, take_file?:bool",
        act(
            "navigator.import_table",
            Command,
            Menu::Output,
            &[],
            table_shown,
            grp(Group::Lists),
            |s, p| s.act_import_table(p),
        ),
    ),
    keeps_banner(with_params(
        "kind:text, show?:list(text), hide?:list(text), order?:list(text)",
        hidden(act(
            "table.columns",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_table_columns(p),
        )),
    )),
    keeps_banner(with_params(
        "kind?:text, row?:text, column?:text, step?:int, across?:int",
        hidden(in_navigator(act(
            "table.cell",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_table_cell(p),
        ))),
    )),
    keeps_banner(with_params(
        "kind?:text, row?:text, column?:text",
        hidden(in_navigator(act(
            "table.edit",
            View,
            Menu::Edit,
            &["F2"],
            pages_writable,
            None,
            |s, p| s.act_table_edit(p),
        ))),
    )),
    keeps_banner(with_params(
        "kind?:text, text:text",
        hidden(in_navigator(act(
            "table.input",
            View,
            Menu::Edit,
            &[],
            pages_writable,
            None,
            |s, p| s.act_table_input(p),
        ))),
    )),
    with_params(
        "kind?:text, all?:bool",
        hidden(in_navigator(act(
            "table.commit",
            Command,
            Menu::Edit,
            &[],
            pages_writable,
            None,
            |s, p| s.act_table_commit(p),
        ))),
    ),
    with_params(
        "kind:text, column:text, value:text, rows?:list(text), all?:bool",
        hidden(act(
            "table.set",
            Command,
            Menu::Edit,
            &[],
            pages_writable,
            None,
            |s, p| s.act_table_set(p),
        )),
    ),
    with_params(
        "kind?:text",
        in_navigator(act(
            "table.fill_down",
            Command,
            Menu::Edit,
            &["Ctrl+D"],
            pages_writable,
            None,
            |s, p| s.act_table_fill_down(p),
        )),
    ),
    with_params(
        "kind?:text, pattern?:text",
        in_navigator(act(
            "table.pattern_fill",
            Command,
            Menu::Edit,
            &["Ctrl+Shift+D"],
            pages_writable,
            None,
            |s, p| s.act_table_pattern_fill(p),
        )),
    ),
    with_params(
        "kind?:text, text?:text",
        in_navigator(act(
            "table.paste",
            Command,
            Menu::Edit,
            &["Ctrl+V"],
            pages_writable,
            None,
            |s, p| s.act_table_paste(p),
        )),
    ),
    keeps_banner(with_params(
        "kind?:text",
        in_navigator(act(
            "table.copy",
            View,
            Menu::Edit,
            &["Ctrl+C"],
            has_pages,
            None,
            |s, p| s.act_table_copy(p),
        )),
    )),
    with_params(
        "kind?:text",
        in_navigator(act(
            "table.clear",
            Command,
            Menu::Edit,
            &["Delete"],
            pages_writable,
            None,
            |s, p| s.act_table_clear(p),
        )),
    ),
    with_params(
        "cell:uint, choice:uint",
        hidden(in_navigator(act(
            "table.resolve",
            Command,
            Menu::Edit,
            &[],
            table_card,
            None,
            |s, p| s.act_table_resolve(p),
        ))),
    ),
    with_params(
        "cell:uint",
        hidden(in_navigator(act(
            "table.drop",
            Command,
            Menu::Edit,
            &[],
            table_card,
            None,
            |s, p| s.act_table_drop(p),
        ))),
    ),
    keeps_banner(with_params(
        "area:enum(navigator|canvas|properties|find|commandbar|history), group?:uint, kind?:text",
        hidden(act(
            "ui.focus",
            View,
            Menu::Window,
            &[],
            project_open,
            None,
            |s, p| s.act_focus(p),
        )),
    )),
    keeps_banner(hidden(parameterless(act(
        "ui.focus_next",
        View,
        Menu::Window,
        &["F6"],
        project_open,
        None,
        |s, _| s.act_focus_cycle(1),
    )))),
    keeps_banner(hidden(parameterless(act(
        "ui.focus_previous",
        View,
        Menu::Window,
        &["Shift+F6"],
        project_open,
        None,
        |s, _| s.act_focus_cycle(-1),
    )))),
    keeps_link_preview(answers_recovery(keeps_banner(hidden(parameterless(act(
        "ui.dismiss",
        View,
        Menu::View,
        &["Escape"],
        always,
        None,
        |s, _| s.act_dismiss(),
    )))))),
    with_params(
        "key:text, value:json",
        hidden(act(
            "settings.set",
            Command,
            Menu::View,
            &[],
            always,
            None,
            |s, p| s.act_settings_set(p),
        )),
    ),
    with_params(
        "field:text, value:json",
        hidden(act(
            "first_run.set",
            View,
            Menu::Help,
            &[],
            first_run_open,
            None,
            |s, p| s.act_first_run_set(p),
        )),
    ),
    hidden(parameterless(act(
        "first_run.next",
        View,
        Menu::Help,
        &[],
        first_run_open,
        None,
        |s, _| s.act_first_run_step(1),
    ))),
    hidden(parameterless(act(
        "first_run.back",
        View,
        Menu::Help,
        &[],
        first_run_open,
        None,
        |s, _| s.act_first_run_step(-1),
    ))),
    with_params(
        "start?:enum(wizard|sample|qet|new|none)",
        hidden(act(
            "first_run.finish",
            Command,
            Menu::Help,
            &[],
            first_run_open,
            None,
            |s, p| s.act_first_run_finish(p),
        )),
    ),
    with_params(
        "field:text, value:text",
        hidden(act(
            "wizard.set",
            View,
            Menu::Project,
            &[],
            wizard_open,
            None,
            |s, p| s.act_wizard_set(p),
        )),
    ),
    hidden(parameterless(act(
        "wizard.next",
        View,
        Menu::Project,
        &[],
        wizard_open,
        None,
        |s, _| s.act_wizard_step(1),
    ))),
    hidden(parameterless(act(
        "wizard.back",
        View,
        Menu::Project,
        &[],
        wizard_open,
        None,
        |s, _| s.act_wizard_step(-1),
    ))),
    with_params(
        "step:uint",
        hidden(act(
            "wizard.goto",
            View,
            Menu::Project,
            &[],
            wizard_open,
            None,
            |s, p| s.act_wizard_goto(p),
        )),
    ),
    hidden(parameterless(act(
        "wizard.reset",
        View,
        Menu::Project,
        &[],
        wizard_open,
        None,
        |s, _| s.act_wizard_reset(),
    ))),
    hidden(parameterless(act(
        "wizard.cancel",
        View,
        Menu::Project,
        &[],
        wizard_open,
        None,
        |s, _| s.act_wizard_cancel(),
    ))),
    with_params(
        "dir?:path",
        hidden(act(
            "wizard.finish",
            Command,
            Menu::Project,
            &[],
            wizard_open,
            None,
            |s, p| s.act_wizard_finish(p),
        )),
    ),
    with_params(
        "confirm?:bool",
        hidden(act(
            "wizard.apply_check_scheme",
            Command,
            Menu::Project,
            &[],
            wizard_open,
            None,
            |s, p| s.act_wizard_apply_check_scheme(p),
        )),
    ),
    // The property grid's own controls (WP-S2.05 part 1, spec §6.10): the window and the headless mode dispatch them.
    keeps_banner(with_params(
        "ids?:list(ref), refs?:list(ref)",
        hidden(act(
            "select.set",
            View,
            Menu::Edit,
            &[],
            project_open,
            None,
            |s, p| s.act_select_set(p),
        )),
    )),
    keeps_banner(with_params(
        "step?:int, to?:enum(first|last)",
        hidden(in_properties(act(
            "props.row",
            View,
            Menu::Edit,
            &[],
            grid_shown,
            None,
            |s, p| s.act_props_row(p),
        ))),
    )),
    // FT-541 (doc 16a §6.8): the filter field of a grid whose open categories hold more than twelve rows.
    keeps_banner(with_params(
        "text:text",
        hidden(in_properties(act(
            "props.filter",
            View,
            Menu::Edit,
            &[],
            grid_shown,
            None,
            |s, p| s.act_props_filter(p),
        ))),
    )),
    keeps_banner(with_params(
        "category:text, expanded?:bool",
        hidden(in_properties(act(
            "props.category",
            View,
            Menu::Edit,
            &[],
            grid_shown,
            None,
            |s, p| s.act_props_category(p),
        ))),
    )),
    keeps_banner(with_params(
        "row?:text",
        hidden(in_properties(act(
            "props.edit",
            View,
            Menu::Edit,
            // Enter is the canvas's `canvas.activate`: the grid's own Enter is the pane's key (§6.9).
            &["F2"],
            grid_shown,
            None,
            |s, p| s.act_props_edit(p),
        ))),
    )),
    keeps_banner(with_params(
        "text:text",
        hidden(in_properties(act(
            "props.input",
            View,
            Menu::Edit,
            &[],
            grid_shown,
            None,
            |s, p| s.act_props_input(p),
        ))),
    )),
    keeps_banner(hidden(in_properties(parameterless(act(
        "props.commit",
        Command,
        Menu::Edit,
        &[],
        grid_writable,
        None,
        |s, _| s.act_props_commit(),
    ))))),
    keeps_banner(hidden(in_properties(parameterless(act(
        "props.cancel",
        View,
        Menu::Edit,
        &[],
        grid_shown,
        None,
        |s, _| s.act_props_cancel(),
    ))))),
    keeps_banner(with_params(
        "row?:text, value?:text, choice?:uint",
        hidden(act(
            "props.set",
            Command,
            Menu::Edit,
            &[],
            grid_writable,
            None,
            |s, p| s.act_props_set(p),
        )),
    )),
    keeps_banner(hidden(parameterless(act(
        "props.accept_tag_proposal",
        Command,
        Menu::Edit,
        &[],
        grid_writable,
        None,
        |s, _| s.act_props_accept_tag_proposal(),
    )))),
    keeps_banner(with_params(
        "index?:uint, step?:int",
        hidden(in_properties(act(
            "props.choose",
            Command,
            Menu::Edit,
            &[],
            grid_shown,
            None,
            |s, p| s.act_props_choose(p),
        ))),
    )),
    keeps_banner(with_params(
        "step?:int",
        hidden(in_properties(act(
            "props.row_menu",
            View,
            Menu::Edit,
            &["Shift+F10"],
            grid_shown,
            None,
            |s, p| s.act_props_row_menu(p),
        ))),
    )),
    // FT-1030 (ID-1107): F (or a click) on a link row of the grid follows it, as F follows a link on the canvas.
    keeps_banner(with_params(
        "row?:text",
        hidden(in_properties(act(
            "props.follow",
            View,
            Menu::Edit,
            &["F"],
            grid_shown,
            None,
            |s, p| s.act_props_follow(p),
        ))),
    )),
    keeps_banner(with_params(
        "row?:text, action:enum(edit|clear|override|reset|freeze|unfreeze)",
        hidden(in_properties(act(
            "props.action",
            Command,
            Menu::Edit,
            &[],
            grid_shown,
            None,
            |s, p| s.act_props_action(p),
        ))),
    )),
    keeps_banner(with_params(
        "row?:text, op:enum(add|remove|up|down)",
        hidden(in_properties(act(
            "props.list",
            Command,
            Menu::Edit,
            &[],
            grid_writable,
            None,
            |s, p| s.act_props_list(p),
        ))),
    )),
    keeps_banner(with_params(
        "row?:text, lang?:text, text?:text",
        hidden(in_properties(act(
            "props.mltext",
            Command,
            Menu::Edit,
            &["Shift+Enter"],
            grid_shown,
            None,
            |s, p| s.act_props_mltext(p),
        ))),
    )),
    keeps_banner(hidden(in_properties(parameterless(act(
        "preview.apply",
        Command,
        Menu::Edit,
        &["Ctrl+Enter"],
        card_writable,
        None,
        |s, _| s.act_preview_apply(),
    ))))),
    keeps_banner(hidden(in_properties(parameterless(act(
        "preview.cancel",
        View,
        Menu::Edit,
        &[],
        card_shown,
        None,
        |s, _| s.act_preview_cancel(),
    ))))),
    // FT-1149 (WP-S3.08 D14): the table import card's switch "Take the file's values".
    keeps_banner(with_params(
        "on?:bool",
        hidden(act(
            "preview.take_file",
            View,
            Menu::Edit,
            &[],
            card_shown,
            None,
            |s, p| s.act_preview_take_file(p),
        )),
    )),
    // WP-S2.09 part 3 (§8.1, §8.4): the Preview tab and the history panel's own moves.
    keeps_banner(with_params(
        "on?:bool",
        hidden(act(
            "preview.show",
            View,
            Menu::View,
            &[],
            project_open,
            None,
            |s, p| s.act_preview_show(p),
        )),
    )),
    keeps_banner(with_params(
        "step?:int, to?:enum(first|last)",
        hidden(act(
            "history.row",
            View,
            Menu::Edit,
            &[],
            history_open,
            None,
            |s, p| s.act_history_row(p),
        )),
    )),
    with_params(
        "entry?:uint",
        hidden(act(
            "history.undo_to",
            Command,
            Menu::Edit,
            &[],
            history_writable,
            None,
            |s, p| s.act_history_undo_to(p),
        )),
    ),
    keeps_banner(with_params(
        "entry?:uint",
        hidden(act(
            "history.changes",
            View,
            Menu::Edit,
            &[],
            history_open,
            None,
            |s, p| s.act_history_changes(p),
        )),
    )),
    keeps_banner(hidden(parameterless(act(
        "toast.undo",
        Command,
        Menu::Edit,
        &[],
        writable,
        None,
        |s, _| s.act_toast_undo(),
    )))),
    keeps_banner(hidden(parameterless(act(
        "toast.dismiss",
        View,
        Menu::Edit,
        &[],
        project_open,
        None,
        |s, _| s.act_toast_dismiss(),
    )))),
    // WP-S2.09 part 1 (§6.8): the jobs' and the notifications' own controls, and the recovery dialog's answers.
    keeps_banner(with_params(
        "job?:uint",
        hidden(act(
            "jobs.cancel",
            Command,
            Menu::View,
            &[],
            always,
            None,
            |s, p| s.act_jobs_cancel(p),
        )),
    )),
    keeps_banner(with_params(
        "job?:uint",
        headless(hidden(act(
            "jobs.wait",
            View,
            Menu::View,
            &[],
            always,
            None,
            |s, p| s.act_jobs_wait(p),
        ))),
    )),
    keeps_banner(with_params(
        "job?:uint",
        hidden(act(
            "jobs.open_result",
            View,
            Menu::View,
            &[],
            always,
            None,
            |s, p| s.act_jobs_open_result(p),
        )),
    )),
    keeps_banner(with_params(
        "entry:uint, action:text",
        hidden(act(
            "notifications.run",
            Command,
            Menu::View,
            &[],
            always,
            None,
            |s, p| s.act_notifications_run(p),
        )),
    )),
    answers_recovery(hidden(parameterless(act(
        "journal.recover",
        Command,
        Menu::File,
        &[],
        recovery_open,
        None,
        |s, _| s.act_journal_recover(),
    )))),
    answers_recovery(keeps_banner(hidden(parameterless(act(
        "journal.keep",
        View,
        Menu::File,
        &[],
        recovery_open,
        None,
        |s, _| s.act_journal_keep(),
    ))))),
    // WP-S6.05 D11: the start-up offer's answers.
    with_params(
        "project:uint",
        hidden(act(
            "startup.recover",
            Command,
            Menu::File,
            &[],
            always,
            None,
            |s, p| s.act_startup_recover(p),
        )),
    ),
    keeps_banner(hidden(parameterless(act(
        "startup.open_log_folder",
        View,
        Menu::File,
        &[],
        always,
        None,
        |s, _| s.act_startup_open_log_folder(),
    )))),
    keeps_banner(hidden(parameterless(act(
        "startup.delete_log",
        Command,
        Menu::File,
        &[],
        always,
        None,
        |s, _| s.act_startup_delete_log(),
    )))),
    keeps_banner(hidden(parameterless(act(
        "startup.dismiss",
        View,
        Menu::File,
        &[],
        always,
        None,
        |s, _| s.act_startup_dismiss(),
    )))),
    // WP-S2.05 part 2: the find pane's own controls (§7.6); the pane handles Enter, Up and Down itself.
    keeps_banner(hidden(in_find(parameterless(act(
        "find.close",
        View,
        Menu::Edit,
        &[],
        find_shown,
        None,
        |s, _| s.act_find_close(),
    ))))),
    keeps_banner(with_params(
        "text:text",
        hidden(in_find(act(
            "find.text",
            View,
            Menu::Edit,
            &[],
            find_shown,
            None,
            |s, p| s.act_find_text(p, false),
        ))),
    )),
    keeps_banner(with_params(
        "text:text",
        hidden(in_find(act(
            "find.replacement",
            View,
            Menu::Edit,
            &[],
            find_shown,
            None,
            |s, p| s.act_find_text(p, true),
        ))),
    )),
    keeps_banner(with_params(
        "option:enum(regex|match_case|whole_word|in_selection), on?:bool",
        hidden(in_find(act(
            "find.option",
            View,
            Menu::Edit,
            &[],
            find_shown,
            None,
            |s, p| s.act_find_option(p),
        ))),
    )),
    keeps_banner(with_params(
        "fields:list(text)",
        hidden(in_find(act(
            "find.fields",
            View,
            Menu::Edit,
            &[],
            find_shown,
            None,
            |s, p| s.act_find_fields(p),
        ))),
    )),
    keeps_banner(with_params(
        "folios:list(text)",
        hidden(in_find(act(
            "find.folios",
            View,
            Menu::Edit,
            &[],
            find_shown,
            None,
            |s, p| s.act_find_folios(p),
        ))),
    )),
    keeps_banner(hidden(in_find(parameterless(act(
        "find.run",
        View,
        Menu::Edit,
        &[],
        find_shown,
        None,
        |s, _| s.act_find_run(),
    ))))),
    keeps_banner(with_params(
        "step?:int, to?:uint",
        hidden(in_find(act(
            "find.row",
            View,
            Menu::Edit,
            &[],
            find_shown,
            None,
            |s, p| s.act_find_row(p),
        ))),
    )),
    keeps_banner(hidden(in_find(parameterless(act(
        "find.jump",
        View,
        Menu::Edit,
        &[],
        find_shown,
        None,
        |s, _| s.act_find_jump(),
    ))))),
    // WP-S6.13 part 1: the page's content as text (FR-UI-046's headless v0).
    with_params(
        "folio?:text",
        headless(act(
            "page.describe",
            View,
            Menu::Page,
            &[],
            page_describable,
            None,
            |s, p| s.act_page_describe(p),
        )),
    ),
    // WP-S2.07 part 4a (§9.4): the part selection dialog and the part editor.
    with_params(
        "target?:enum(parts|bridge_part|accessory_part), use?:uint, add?:bool",
        act(
            "part.choose",
            View,
            Menu::Edit,
            &[],
            writable,
            slot(Menu::Edit),
            |s, p| s.act_part_choose(p),
        ),
    ),
    with_params(
        "text?:text, kind?:text, category?:text, fits?:bool, view?:enum(all|project|favourites|recent), inactive?:bool",
        hidden(act(
            "part.search",
            View,
            Menu::Edit,
            &[],
            part_dialog_open,
            None,
            |s, p| s.act_part_search(p),
        )),
    ),
    with_params(
        "index?:uint, part?:text",
        hidden(act(
            "part.pick",
            Command,
            Menu::Edit,
            &[],
            part_dialog_open,
            None,
            |s, p| s.act_part_pick(p),
        )),
    ),
    with_params(
        "index?:uint",
        hidden(act(
            "part.favourite",
            View,
            Menu::Edit,
            &[],
            part_dialog_open,
            None,
            |s, p| s.act_part_favourite(p),
        )),
    ),
    with_params(
        "part?:text",
        act(
            "part.edit",
            View,
            Menu::Edit,
            &[],
            writable,
            slot(Menu::Edit),
            |s, p| s.act_part_edit(p),
        ),
    ),
    with_params(
        "field:text, value:text",
        hidden(act(
            "part.field",
            View,
            Menu::Edit,
            &[],
            part_editor_open,
            None,
            |s, p| s.act_part_field(p),
        )),
    ),
    hidden(parameterless(act(
        "part.save",
        Command,
        Menu::Edit,
        &[],
        part_editor_open,
        None,
        |s, _| s.act_part_save(),
    ))),
    // WP-S2.07 part 4b (§9.4)
    with_params(
        "choice:enum(adapt|update)",
        hidden(act(
            "paste.resolve",
            Command,
            Menu::Edit,
            &[],
            paste_choice_open,
            None,
            |s, p| s.act_paste_resolve(p),
        )),
    ),
    // WP-S2.08 part 2 (D8): the keymap editor's own controls (Help ▸ Keyboard shortcuts); the window dispatches them.
    with_params(
        "text:text",
        hidden(act(
            "keymap.search",
            View,
            Menu::Help,
            &[],
            keymap_editor_open,
            None,
            |s, p| s.act_keymap_search(p),
        )),
    ),
    with_params(
        "action?:text",
        hidden(act(
            "keymap.record_start",
            View,
            Menu::Help,
            &[],
            keymap_editor_open,
            None,
            |s, p| s.act_keymap_record_start(p),
        )),
    ),
    with_params(
        "action:text, keys:text",
        hidden(act(
            "keymap.record",
            View,
            Menu::Help,
            &[],
            keymap_editor_open,
            None,
            |s, p| s.act_keymap_record(p),
        )),
    ),
    with_params(
        "action:text",
        hidden(act(
            "keymap.reset",
            View,
            Menu::Help,
            &[],
            keymap_editor_open,
            None,
            |s, p| s.act_keymap_reset(p),
        )),
    ),
    with_params(
        "path?:path",
        hidden(act(
            "keymap.import",
            View,
            Menu::Help,
            &[],
            keymap_editor_open,
            None,
            |s, p| s.act_keymap_import(p),
        )),
    ),
    with_params(
        "path?:path",
        hidden(act(
            "keymap.export",
            View,
            Menu::Help,
            &[],
            keymap_editor_open,
            None,
            |s, p| s.act_keymap_export(p),
        )),
    ),
    // ID-1477 (7): the editor has a button on the Draw tab, whose Place group holds its menu entry and palette row.
    with_params(
        "key?:text, name?:text, discard?:bool",
        placed(
            place(
                Group::Place,
                5,
                CommandSize::Small,
                Some(key!("bar-symbol-editor")),
            ),
            "pencil",
            act(
                "symbol.new",
                View,
                Menu::Edit,
                &[],
                always,
                slot(Menu::Edit),
                |s, p| s.act_symbol("new", p),
            ),
        ),
    ),
    with_params(
        "file?:path, key?:text, discard?:bool",
        hidden(act("symbol.open", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("open", p)
        })),
    ),
    // ID-1463: the Library folder, typed or from the folder picker.
    with_params(
        "directory?:path",
        hidden(act(
            "symbol.directory",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("directory", p),
        )),
    ),
    with_params(
        "index:uint, discard?:bool",
        hidden(act(
            "symbol.candidate",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("candidate", p),
        )),
    ),
    with_params(
        "key:text",
        hidden(act(
            "symbol.function",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("function", p),
        )),
    ),
    with_params(
        "cp:uint, fn_cp:uint",
        hidden(act(
            "symbol.map_cp",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("map_cp", p),
        )),
    ),
    // Part 3's nit (D6): a connection point's potential role, for scripts and AI tools as for the role list.
    with_params(
        "cp:uint, role:enum(none|l1|l2|l3|l|n|pe|pen|fe|l_plus|l_minus|m|signal|undefined)",
        hidden(act("symbol.role", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("role", p)
        })),
    ),
    with_params(
        "name:text, x:mm, y:mm",
        hidden(act("symbol.link", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("link", p)
        })),
    ),
    with_params(
        "points:json, closed?:bool",
        hidden(act("symbol.path", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("path", p)
        })),
    ),
    with_params(
        "source:text, key:text, discard?:bool",
        hidden(act("symbol.fork", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("fork", p)
        })),
    ),
    with_params(
        "discard?:bool",
        hidden(act(
            "symbol.close",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("close", p),
        )),
    ),
    with_params(
        "document:json",
        hidden(act(
            "symbol.replace",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("replace", p),
        )),
    ),
    hidden(parameterless(act(
        "symbol.undo",
        View,
        Menu::Edit,
        &[],
        always,
        None,
        |s, p| s.act_symbol("undo", p),
    ))),
    hidden(parameterless(act(
        "symbol.redo",
        View,
        Menu::Edit,
        &[],
        always,
        None,
        |s, p| s.act_symbol("redo", p),
    ))),
    // ID-1463 (doc 16a P8): the editor's tool, also chosen by a key of the keymap.
    with_params(
        "tool:enum(select|line|rectangle|circle|ellipse|arc|text|cp|slot|anchor|link|polyline|polygon)",
        hidden(act("symbol.tool", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("tool", p)
        })),
    ),
    with_params(
        "variant:uint",
        hidden(act(
            "symbol.variant",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("variant", p),
        )),
    ),
    hidden(parameterless(act(
        "symbol.validate",
        View,
        Menu::Edit,
        &[],
        always,
        None,
        |s, p| s.act_symbol("validate", p),
    ))),
    with_params(
        "tool:enum(line|rectangle|circle|ellipse|arc|text|anchor), x:mm, y:mm, x2?:mm, y2?:mm, text?:text, start?:int, sweep?:int",
        hidden(act("symbol.draw", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("draw", p)
        })),
    ),
    with_params(
        "x:mm, y:mm, idx?:uint, fn_cp?:uint, designation?:text, direction?:enum(up|right|down|left)",
        hidden(act("symbol.cp", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("cp", p)
        })),
    ),
    with_params(
        "kind?:json, x:mm, y:mm",
        hidden(act("symbol.slot", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("slot", p)
        })),
    ),
    with_params(
        "file?:path",
        hidden(act(
            "symbol.export",
            Command,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("export", p),
        )),
    ),
    with_params(
        "directory?:path",
        hidden(act(
            "symbol.save",
            Command,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("save", p),
        )),
    ),
    with_params(
        "x:mm, y:mm",
        hidden(act(
            "symbol.place",
            Command,
            Menu::Edit,
            &[],
            writable,
            None,
            |s, p| s.act_symbol("place", p),
        )),
    ),
    hidden(parameterless(act(
        "symbol.scratch",
        Command,
        Menu::Edit,
        &[],
        writable,
        None,
        |s, p| s.act_symbol("scratch", p),
    ))),
    // M25-p7#2 (ID-1477 (1)): the editor's offer for a draft saved to a library and unchanged since.
    hidden(parameterless(act(
        "symbol.new_page",
        Command,
        Menu::Edit,
        &[],
        writable,
        None,
        |s, p| s.act_symbol("new_page", p),
    ))),
];

// WP-S2.08 part 2
/// The keymap editor's controls need the keyboard shortcuts dialog.
fn keymap_editor_open(m: &ShellModel) -> Result<(), Key<'static>> {
    if matches!(m.dialog(), Some(crate::shell::Dialog::Shortcuts)) {
        Ok(())
    } else {
        Err(key!("reason-no-keymap-editor"))
    }
}

// WP-S2.07 part 4b
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

/// The print pre-dialog's own steps (WP-S3.07 part 3).
fn print_dialog_open(m: &ShellModel) -> Result<(), Key<'static>> {
    project_open(m)?;
    if m.print_dialog().is_some() {
        Ok(())
    } else {
        Err(key!("reason-no-print-dialog"))
    }
}

// WP-S2.07 part 4a
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

/// One parameter of an action's signature (WP-S6.13 §5.5.2).
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

/// Parses a parameter signature by the grammar of WP-S6.13 §5.5.2:
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
    /// A command-bar group of more than three commands, as a submenu of its tab's menu (WP-S2.17 part 2, doc 16a
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
            // WP-S2.04 part 2.
            "tool.wire" => model.tool().name() == "wire",
            // WP-S2.04 part 3.
            "tool.ip" => model.tool().name() == "ip",
            "tool.black_box" => model.tool().name() == "black_box",
            // WP-S2.04 part 4b.
            "tool.text" => model.tool().name() == "text",
            "tool.marker" => model.tool().name() == "marker",
            "tool.structure_box" => model.tool().name() == "structure_box",
            // WP-S4.02 part 4.
            "tool.rail_draw" => model.tool().name() == "rail_draw",
            "tool.duct_draw" => model.tool().name() == "duct_draw",
            // WP-S2.17 part 2: the Place tool lights Symbol in the bar.
            "tool.place" => model.tool().name() == "place",
            _ => false,
        }),
        topic: crate::help::action_topic(meta.id),
    }
}

/// The on or off state of a switch (a view toggle, a pane shown or hidden): the menu item's check, the command-bar
/// button's lit state and the palette entry's state (FT-1367, ID-1392); `None` for an action that is no switch.
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
        // WP-S2.09 part 1.
        "jobs.show" => model.jobs_shown(),
        "notifications.show" => model.notifications().popup_open(),
        // WP-S2.09 part 2.
        "messages.show" => model.navigator_open(crate::navigators::NavKind::Messages),
        // WP-S3.09 D9 d.
        "messages.migration" => model.messages_shows_migration(),
        // WP-S2.09 part 3.
        "history.show" => model.history_shown(),
        "preview.show" => model.preview_shown(),
        "view.bottom_dock" => model.bottom_dock_shown(),
        // WP-S2.08 part 6.
        "help.glossary" => model.glossary_shown(),
        "view.wire_colours" => model.wire_colours_on(),
        "project.crossings" => model.crossings_hops(),
        // WP-S2.17 part 2: the command bar's collapse.
        "view.command_bar" => model.command_bar_collapsed(),
        // WP-S2.17 part 3.
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
        // WP-S2.17 part 2: one item per command-bar tab.
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
        // WP-S2.06 part 1: one checked item per navigator; the message panel has its own item, View ▸ Messages
        // (WP-S2.09 part 2).
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
        // WP-S2.08 part 3: one item per UI language, under its own name.
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
        // FT-1458: one item per tutorial, each with its own start.
        Submenu::Tutorials => TUTORIALS
            .iter()
            .map(|name| {
                let mut item = item_of(meta, model);
                item.label = strings::lookup(&format!("tutorial-{name}"))
                    .unwrap_or(name)
                    .to_owned();
                // ID-1427 (W52 F14, doc 16a §8.3): an entry that asks for a folder carries "…" (`act_tutorials`).
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

/// The Window menu's list of open tabs: one checked item per tab of the focused group, dispatching `tab.activate`
/// (WP-S2.03 §6.7).
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
            // ID-1463: while the symbol editor shows, Save names the symbol and Save project follows it.
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

/// [`breadcrumb`] of a palette row labelled `label`, which it never repeats (FT-1174, doc 16a §6.14): "View" on the
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
    /// A model command of `Session::registry()`; part 1 only lists it (disabled).
    Command(String),
    /// A page of the `#` prefix (WP-S2.08 D4): its folio; Enter shows it (`page.goto`).
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
        // Unique within a shell context (WP-S2.01 A1; a context's binding may shadow a global one, WP-S2.03
        // §6.6): Shift+Enter finishes a tool in the canvas and opens the multilingual popup in the property grid.
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
        // WP-S2.17 part 2: the imports and the company standard sit in the Project tab's menu.
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

    /// WP-S6.13 BD2: a hidden, headless-only or `needs_parameters` action has a signature or is marked
    /// `parameterless`, and a parameterless action has no signature.
    #[test]
    fn every_action_that_reads_parameters_has_a_signature() {
        for a in ACTIONS
            .iter()
            .filter(|a| a.needs_parameters || a.headless_only || a.hidden)
        {
            assert!(
                !a.params.is_empty() || a.parameterless,
                "{} reads parameters but has no signature and is not marked parameterless",
                a.id
            );
        }
        for a in ACTIONS.iter().filter(|a| a.parameterless) {
            assert!(
                a.params.is_empty(),
                "{} is marked parameterless but has the signature {}",
                a.id,
                a.params
            );
        }
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
