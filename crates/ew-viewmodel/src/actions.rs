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

use serde_json::{Value, json};

use crate::command_bar::{CommandSize, Group, Placement, Tab, place};
use crate::settings::{DENSITIES, THEMES, UI_SCALES};
use crate::shell::ShellModel;
use crate::strings;
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
    pub fn label_key(self) -> &'static str {
        match self {
            Menu::File => "menu-file",
            Menu::Edit => "menu-edit",
            Menu::Project => "menu-project",
            Menu::Page => "menu-page",
            Menu::Draw => "menu-draw",
            Menu::Check => "menu-check",
            Menu::Output => "menu-output",
            Menu::View => "menu-view",
            Menu::Window => "menu-window",
            Menu::Help => "menu-help",
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
}

impl Submenu {
    /// The key of the submenu's label.
    pub fn label_key(self) -> &'static str {
        match self {
            Submenu::OpenRecent => "submenu-open-recent",
            Submenu::UiScale => "submenu-ui-scale",
            Submenu::Theme => "submenu-theme",
            Submenu::Density => "submenu-density",
            Submenu::Workspace => "submenu-workspace",
            Submenu::Pan => "submenu-pan",
            Submenu::PageNavigator => "submenu-page-navigator",
            Submenu::Navigators => "submenu-navigators",
            Submenu::Graphic => "submenu-graphic",
            Submenu::WireMarkers => "submenu-wire-markers",
            Submenu::CommandTab => "submenu-command-tab",
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

/// One action of the registry.
#[derive(Clone, Copy)]
pub struct ActionMeta {
    /// The stable ID (`file.open`).
    pub id: &'static str,
    /// View or command.
    pub kind: ActionKind,
    /// The key of the label in [`crate::strings`].
    pub label_key: &'static str,
    /// The menu the palette groups the action under (also the menu it lives in, when it has a slot).
    pub category: Menu,
    /// The default shortcuts (`Ctrl+Shift+Z`); `Ctrl` reads `Cmd` on macOS. The first is shown in menus.
    pub shortcut: &'static [&'static str],
    /// Whether the action can run now: `Err` carries the reason key of its refusal.
    pub enabled: fn(&ShellModel) -> Result<(), &'static str>,
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
}

impl std::fmt::Debug for ActionMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActionMeta")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

fn always(_: &ShellModel) -> Result<(), &'static str> {
    Ok(())
}

fn project_open(m: &ShellModel) -> Result<(), &'static str> {
    if m.has_project() {
        Ok(())
    } else {
        Err("reason-no-project")
    }
}

fn writable(m: &ShellModel) -> Result<(), &'static str> {
    project_open(m)?;
    if m.is_writable() {
        Ok(())
    } else {
        Err("reason-read-only")
    }
}

fn has_pages(m: &ShellModel) -> Result<(), &'static str> {
    project_open(m)?;
    if m.page_count() > 0 {
        Ok(())
    } else {
        Err("reason-no-pages")
    }
}

// WP-S2.09 part 3
/// The history panel's row moves and Show changes: a project with the panel open.
fn history_open(m: &ShellModel) -> Result<(), &'static str> {
    project_open(m)?;
    if m.history_shown() {
        Ok(())
    } else {
        Err("reason-history-closed")
    }
}

/// Undo to here: a writable project with the panel open.
fn history_writable(m: &ShellModel) -> Result<(), &'static str> {
    writable(m)?;
    history_open(m)
}

fn can_undo(m: &ShellModel) -> Result<(), &'static str> {
    writable(m)?;
    if m.can_undo() {
        Ok(())
    } else {
        Err("reason-nothing-to-undo")
    }
}

fn can_redo(m: &ShellModel) -> Result<(), &'static str> {
    writable(m)?;
    if m.can_redo() {
        Ok(())
    } else {
        Err("reason-nothing-to-redo")
    }
}

fn has_next(m: &ShellModel) -> Result<(), &'static str> {
    has_pages(m)?;
    if m.page_index() + 1 < m.page_count() {
        Ok(())
    } else {
        Err("reason-no-next-page")
    }
}

fn has_previous(m: &ShellModel) -> Result<(), &'static str> {
    has_pages(m)?;
    if m.page_index() > 0 {
        Ok(())
    } else {
        Err("reason-no-previous-page")
    }
}

fn has_recent(m: &ShellModel) -> Result<(), &'static str> {
    if m.recent_count() > 0 {
        Ok(())
    } else {
        Err("reason-no-recent")
    }
}

fn page_shown(m: &ShellModel) -> Result<(), &'static str> {
    // The wizard's preview page is a page on show too (`view.snapshot` writes it).
    if m.page_count() > 0 || m.wizard().is_some() {
        Ok(())
    } else {
        Err("reason-no-page")
    }
}

fn can_zoom_in(m: &ShellModel) -> Result<(), &'static str> {
    has_pages(m)?;
    match m.canvas() {
        Some(c) if c.zoom() >= crate::canvas::ZOOM_MAX => Err("reason-zoom-max"),
        _ => Ok(()),
    }
}

fn can_zoom_out(m: &ShellModel) -> Result<(), &'static str> {
    has_pages(m)?;
    match m.canvas() {
        Some(c) if c.zoom() <= crate::canvas::ZOOM_MIN => Err("reason-zoom-min"),
        _ => Ok(()),
    }
}

fn can_add_tab(m: &ShellModel) -> Result<(), &'static str> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.tab_count() >= crate::area::MAX_TABS => Err("reason-tabs-full"),
        _ => Ok(()),
    }
}

fn can_close_tab(m: &ShellModel) -> Result<(), &'static str> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.tab_count() <= 1 => Err("reason-last-tab"),
        _ => Ok(()),
    }
}

fn can_cycle_tabs(m: &ShellModel) -> Result<(), &'static str> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.focused_group().tabs.len() <= 1 => Err("reason-one-tab"),
        _ => Ok(()),
    }
}

fn two_groups(m: &ShellModel) -> Result<(), &'static str> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.side_by_side() => Ok(()),
        _ => Err("reason-one-group"),
    }
}

fn can_go_back(m: &ShellModel) -> Result<(), &'static str> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.current().history.back_len() > 0 => Ok(()),
        _ => Err("reason-history-empty"),
    }
}

fn can_go_forward(m: &ShellModel) -> Result<(), &'static str> {
    has_pages(m)?;
    match m.area() {
        Some(a) if a.current().history.forward_len() > 0 => Ok(()),
        _ => Err("reason-history-empty"),
    }
}

/// A drawn page of a project that can be changed is on show (the editing tools of WP-S2.04 §7.6).
pub(crate) fn editable_page(m: &ShellModel) -> Result<(), &'static str> {
    has_pages(m)?;
    writable(m)?;
    // The page types without wire or symbol tools (doc 06 §2.2: title page, panel layout, model view, external
    // document; the generated table of contents, dividers and reports are not drawn pages at all).
    let kind = m.page_rows().get(m.page_index()).map(|r| r.kind.as_str());
    if matches!(
        kind,
        Some(
            "title_page"
                | "panel_layout"
                | "model_view"
                | "external_document"
                | "table_of_contents"
                | "section_divider"
                | "report"
        )
    ) {
        return Err("reason-page-type-no-symbols");
    }
    match m.canvas() {
        Some(c) if !c.page().tables.page.is_nil() => Ok(()),
        _ => Err("reason-not-editable"),
    }
}

/// A drawn page of a writable project for Paste (window check, item 3): the page's kind is checked by the action, which
/// refuses it with `reason-paste-page-kind` and the kind's name in the status bar; the menu disables it with that text.
fn paste_page(m: &ShellModel) -> Result<(), &'static str> {
    has_pages(m)?;
    writable(m)?;
    // A page kind that takes no paste may have no canvas at all (a generated report): the action still runs, so it can
    // refuse with the kind's reason rather than the general one.
    if m.paste_page_kind().is_some() {
        return Ok(());
    }
    match m.canvas() {
        Some(c) if !c.page().tables.page.is_nil() => Ok(()),
        _ => Err("reason-not-editable"),
    }
}

/// An editable page whose type has a connection kind the Wire tool draws (WP-S2.04 part 2, fix round item 1).
fn wire_page(m: &ShellModel) -> Result<(), &'static str> {
    editable_page(m)?;
    m.wire_kind().map(|_| ())
}

/// An editable page with exactly one connection selected (Edit ▸ Edit route; WP-S2.04 part 2, window check).
fn one_connection(m: &ShellModel) -> Result<(), &'static str> {
    editable_page(m)?;
    if m.one_connection_selected() {
        Ok(())
    } else {
        Err("reason-one-connection")
    }
}

/// An editable page with exactly one junction selected (Edit ▸ Junction sequence…; WP-S2.04 part 2, window check).
fn one_junction(m: &ShellModel) -> Result<(), &'static str> {
    editable_page(m)?;
    if m.one_junction_selected() {
        Ok(())
    } else {
        Err("reason-one-junction")
    }
}

/// An editable page with exactly one interruption point selected (Edit ▸ Continue on page…; WP-S2.04 §9.3).
fn one_ip(m: &ShellModel) -> Result<(), &'static str> {
    editable_page(m)?;
    if m.selected_ip_count() == 1 {
        Ok(())
    } else {
        Err("reason-select-one-ip")
    }
}

/// An editable page with at least two interruption points selected (Edit ▸ Arrange interruption points; §9.3).
fn two_ips(m: &ShellModel) -> Result<(), &'static str> {
    editable_page(m)?;
    if m.selected_ip_count() >= 2 {
        Ok(())
    } else {
        Err("reason-select-ips")
    }
}

/// An editable page with exactly one black box selected (Edit ▸ Add connection point, the grips; §9.4).
fn one_box(m: &ShellModel) -> Result<(), &'static str> {
    editable_page(m)?;
    if m.one_black_box_selected() {
        Ok(())
    } else {
        Err("reason-one-black-box")
    }
}

/// An editable page with exactly one unbound free text selected (Edit ▸ Edit text; §10.5).
fn one_free_text(m: &ShellModel) -> Result<(), &'static str> {
    editable_page(m)?;
    m.text_edit_target().map(|_| ())
}

/// An editable page whose selection is exactly two terminals of one strip not yet bridged (Edit ▸ Bridge
/// terminals; FT-411, §10.7).
fn two_strip_terminals(m: &ShellModel) -> Result<(), &'static str> {
    editable_page(m)?;
    m.bridge_params().map(|_| ())
}

/// The junction sequence editor is open on a project that can be changed (WP-S2.04 part 2, §8.5).
fn junction_editor_open(m: &ShellModel) -> Result<(), &'static str> {
    writable(m)?;
    if m.junction_editor().is_some() {
        Ok(())
    } else {
        Err("reason-no-junction-editor")
    }
}

/// A project with a page is open: `page.describe` reads a page (WP-S6.13 §5.5.3).
fn page_describable(m: &ShellModel) -> Result<(), &'static str> {
    project_open(m)?;
    if m.page_count() > 0 {
        Ok(())
    } else {
        Err("reason-no-page")
    }
}

/// The picker is open (WP-S2.04 §7.1).
fn picker_open(m: &ShellModel) -> Result<(), &'static str> {
    if m.picker().is_some() {
        Ok(())
    } else {
        Err("reason-no-picker")
    }
}

fn has_link_preview(m: &ShellModel) -> Result<(), &'static str> {
    has_pages(m)?;
    if m.link_preview().is_some() {
        Ok(())
    } else {
        Err("reason-no-preview")
    }
}

fn can_discard_journal(m: &ShellModel) -> Result<(), &'static str> {
    if m.can_discard_journal() {
        Ok(())
    } else {
        Err("reason-no-journal")
    }
}

// WP-S2.09 part 1
/// The recovery dialog's answers need the dialog (§6.6).
fn recovery_open(m: &ShellModel) -> Result<(), &'static str> {
    if matches!(m.dialog(), Some(crate::shell::Dialog::Recovery { .. })) {
        Ok(())
    } else {
        Err("reason-no-recovery")
    }
}

fn wizard_open(m: &ShellModel) -> Result<(), &'static str> {
    if m.wizard().is_some() {
        Ok(())
    } else {
        Err("reason-no-wizard")
    }
}

fn first_run_open(m: &ShellModel) -> Result<(), &'static str> {
    if m.first_run().is_some() {
        Ok(())
    } else {
        Err("reason-no-first-run")
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
    label_key: &'static str,
    category: Menu,
    shortcut: &'static [&'static str],
    enabled: fn(&ShellModel) -> Result<(), &'static str>,
    menu: Option<MenuSlot>,
) -> ActionMeta {
    ActionMeta {
        id,
        kind,
        label_key,
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
fn grid_shown(m: &ShellModel) -> Result<(), &'static str> {
    project_open(m)?;
    if m.properties_shown() {
        Ok(())
    } else {
        Err("reason-no-sheet")
    }
}

/// A grid action that may commit: a writable project with the pane shown.
fn grid_writable(m: &ShellModel) -> Result<(), &'static str> {
    writable(m)?;
    grid_shown(m)
}

// WP-S2.05 part 2
const fn in_find(mut a: ActionMeta) -> ActionMeta {
    a.context = Context::Find;
    a
}

/// The find pane's own controls need the pane.
fn find_shown(m: &ShellModel) -> Result<(), &'static str> {
    project_open(m)?;
    if m.find_shown() {
        Ok(())
    } else {
        Err("reason-no-find")
    }
}

/// A replace: a writable project with the find pane shown.
fn find_writable(m: &ShellModel) -> Result<(), &'static str> {
    writable(m)?;
    find_shown(m)
}

/// The card's buttons: the card sits in the property grid or the find pane, so one of them is shown; an import card
/// (FT-410) is a window of its own and needs neither.
fn card_shown(m: &ShellModel) -> Result<(), &'static str> {
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
        Err("reason-no-sheet")
    }
}

/// Apply on the card: a writable project with the property grid or the find pane shown, or an import card open.
fn card_writable(m: &ShellModel) -> Result<(), &'static str> {
    writable(m)?;
    card_shown(m)
}

// WP-S2.06 part 1
/// `navigator.apply`: a writable project with the Pages pane's reorder card, (WP-S2.09 part 2) the Messages pane's
/// quick-fix card or (WP-S2.06 part 3) a navigator's table card on show.
fn reorder_card(m: &ShellModel) -> Result<(), &'static str> {
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
        Err("reason-no-card")
    }
}

// WP-S2.06 part 3
/// `table.resolve` and `table.drop`: a writable project with a table card on show.
fn table_card(m: &ShellModel) -> Result<(), &'static str> {
    writable(m)?;
    if m.table_gesture().is_some() {
        Ok(())
    } else {
        Err("reason-no-card")
    }
}

// WP-S2.06 part 2
/// View ▸ Clear highlights: a highlight is shown.
fn has_highlights(m: &ShellModel) -> Result<(), &'static str> {
    project_open(m)?;
    if m.highlights().is_empty() {
        Err("reason-no-highlights")
    } else {
        Ok(())
    }
}

/// The page reorder actions: a writable project with pages.
fn pages_writable(m: &ShellModel) -> Result<(), &'static str> {
    writable(m)?;
    has_pages(m)
}

// WP-S2.05 part 3
/// Copy and Copy tag list: a page is shown and something is selected (a read-only project copies too).
fn selection_shown(m: &ShellModel) -> Result<(), &'static str> {
    page_shown(m)?;
    if m.selection().is_empty() {
        Err("reason-nothing-selected")
    } else {
        Ok(())
    }
}

/// Cut and Duplicate: an editable page with something selected.
fn selection_editable(m: &ShellModel) -> Result<(), &'static str> {
    editable_page(m)?;
    selection_shown(m)
}

use ActionKind::{Command, View};

/// The registry: the part-1 actions of the spec, section 5.2, in menu order, then the headless-only ones.
pub static ACTIONS: &[ActionMeta] = &[
    with_params(
        "dir?:path, template?:path, discard?:bool",
        act(
            "file.new",
            Command,
            "action-file-new",
            Menu::File,
            &["Ctrl+N"],
            always,
            slot(Menu::File),
        ),
    ),
    with_params(
        "dir?:path, read_only?:bool, discard?:bool",
        act(
            "file.open",
            Command,
            "action-file-open",
            Menu::File,
            &["Ctrl+O"],
            always,
            slot(Menu::File),
        ),
    ),
    with_params(
        "index:uint, discard?:bool",
        needs_parameters(act(
            "file.open_recent",
            Command,
            "action-file-open-recent",
            Menu::File,
            &[],
            has_recent,
            sub(Menu::File, Submenu::OpenRecent, false),
        )),
    ),
    with_params(
        "discard?:bool",
        act(
            "file.open_sample",
            Command,
            "action-file-open-sample",
            Menu::File,
            &[],
            always,
            slot(Menu::File),
        ),
    ),
    with_params(
        "file?:path, dir?:path, discard?:bool",
        placed(
            place(Group::Import, 1, CommandSize::Small, Some("bar-import-qet")),
            "file-import",
            act(
                "file.import_qet",
                Command,
                "action-file-import-qet",
                Menu::Project,
                &[],
                always,
                // FT-466 moved it to File, Import; WP-S2.17 part 2 places it in Project, Import (its ID unchanged).
                None,
            ),
        ),
    ),
    // FT-410: File, Import, Parts table...
    with_params(
        "path?:path",
        placed(
            place(Group::Parts, 1, CommandSize::Large, Some("bar-import-parts")),
            "database-import",
            act(
                "parts.import",
                Command,
                "action-parts-import",
                Menu::Project,
                &[],
                writable,
                None,
            ),
        ),
    ),
    with_icon(
        "device-floppy",
        act(
            "file.save",
            Command,
            "action-file-save",
            Menu::File,
            &["Ctrl+S"],
            writable,
            slot_sep(Menu::File),
        ),
    ),
    with_params(
        "dir?:path",
        act(
            "file.save_as",
            Command,
            "action-file-save-as",
            Menu::File,
            &["Ctrl+Shift+S"],
            project_open,
            slot(Menu::File),
        ),
    ),
    with_params(
        "confirm?:bool",
        act(
            "file.discard_journal",
            Command,
            "action-file-discard-journal",
            Menu::File,
            &[],
            can_discard_journal,
            slot(Menu::File),
        ),
    ),
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
                "action-file-export-pdf",
                Menu::Output,
                &["Ctrl+Shift+E"],
                has_pages,
                None,
            ),
        ),
    ),
    with_params(
        "discard?:bool",
        act(
            "file.close",
            Command,
            "action-file-close",
            Menu::File,
            &["Ctrl+Shift+W"],
            project_open,
            slot_sep(Menu::File),
        ),
    ),
    with_params(
        "discard?:bool",
        act(
            "app.quit",
            Command,
            "action-app-quit",
            Menu::File,
            &["Ctrl+Q"],
            always,
            slot(Menu::File),
        ),
    ),
    with_icon(
        "arrow-back-up",
        act(
            "edit.undo",
            Command,
            "action-edit-undo",
            Menu::Edit,
            &["Ctrl+Z"],
            can_undo,
            slot(Menu::Edit),
        ),
    ),
    with_icon(
        "arrow-forward-up",
        act(
            "edit.redo",
            Command,
            "action-edit-redo",
            Menu::Edit,
            &["Ctrl+Shift+Z", "Ctrl+Y"],
            can_redo,
            slot(Menu::Edit),
        ),
    ),
    // WP-S2.09 part 3 (§8.4): Edit, History.
    with_params(
        "on?:bool",
        placed(
            place(Group::Panels, 4, CommandSize::Small, None),
            "history",
            act(
                "history.show",
                View,
                "action-history-show",
                Menu::View,
                &[],
                project_open,
                None,
            ),
        ),
    ),
    // Edit, Properties of selection (E and Alt+Enter on the canvas) and Project properties: the shell has no Project
    // menu yet, so the project's properties sit beside the selection's (WP-S2.05 §6.10, lane ruling).
    in_canvas(act(
        "props.focus",
        View,
        "action-props-focus",
        Menu::Edit,
        &["E", "Alt+Enter"],
        project_open,
        slot_sep(Menu::Edit),
    )),
    placed(
        place(Group::Project, 1, CommandSize::Large, Some("bar-properties")),
        "adjustments",
        act(
            "props.project",
            View,
            "action-props-project",
            Menu::Project,
            &[],
            project_open,
            None,
        ),
    ),
    // WP-S2.05 part 2: Edit, Find..., Find and replace... and Replace all (§7.6). macOS's ⌥⌘F (doc 16 §4.4) needs a
    // per-platform binding: `Ctrl+Alt+F` would be a Ctrl+Alt+letter chord, which NFR-I18N-06 forbids.
    act(
        "find.open",
        View,
        "action-find-open",
        Menu::Edit,
        &["Ctrl+F"],
        project_open,
        slot_sep(Menu::Edit),
    ),
    act(
        "find.open_replace",
        View,
        "action-find-open-replace",
        Menu::Edit,
        &["Ctrl+H"],
        project_open,
        slot(Menu::Edit),
    ),
    in_find(act(
        "find.replace_all",
        Command,
        "action-find-replace-all",
        Menu::Edit,
        &["Ctrl+Enter"],
        find_writable,
        slot(Menu::Edit),
    )),
    // FT-408: Page, New page.
    with_params(
        "folio:text, type?:enum(title_page|table_of_contents|section_divider|graphic|single_line|multi_line|overview|bus_topology|fluid|assembly_overview|panel_layout|model_view|external_document), title?:text, section?:text",
        placed(
            place(Group::Pages, 1, CommandSize::Large, None),
            "file-plus",
            act(
                "page.new",
                Command,
                "action-page-new",
                Menu::Page,
                &[],
                writable,
                None,
            ),
        ),
    ),
    // WP-S2.09 part 3 (§8.3): Page, Renumber pages...
    with_params(
        "section?:text, start?:uint, step?:uint, keep_gaps?:bool",
        placed(
            place(Group::Order, 1, CommandSize::Small, None),
            "list-numbers",
            act(
                "pages.renumber",
                Command,
                "action-pages-renumber",
                Menu::Page,
                &[],
                writable,
                None,
            ),
        ),
    ),
    act(
        "page.next",
        View,
        "action-page-next",
        Menu::Page,
        &["Page Down"],
        has_next,
        grp(Group::Go),
    ),
    act(
        "page.previous",
        View,
        "action-page-previous",
        Menu::Page,
        &["Page Up"],
        has_previous,
        grp(Group::Go),
    ),
    act(
        "page.first",
        View,
        "action-page-first",
        Menu::Page,
        &[],
        has_pages,
        grp(Group::Go),
    ),
    act(
        "page.last",
        View,
        "action-page-last",
        Menu::Page,
        &[],
        has_pages,
        grp(Group::Go),
    ),
    with_params(
        "text?:text",
        placed(
            place(Group::Go, 1, CommandSize::Large, Some("bar-go-to")),
            "current-location",
            act(
                "nav.goto",
                View,
                "action-nav-goto",
                Menu::Page,
                &["Ctrl+G"],
                has_pages,
                None,
            ),
        ),
    ),
    placed(
        place(Group::Go, 2, CommandSize::Small, None),
        "arrow-left",
        act(
            "nav.back",
            View,
            "action-nav-back",
            Menu::Page,
            &["Alt+Arrow Left"],
            can_go_back,
            None,
        ),
    ),
    placed(
        place(Group::Go, 3, CommandSize::Small, None),
        "arrow-right",
        act(
            "nav.forward",
            View,
            "action-nav-forward",
            Menu::Page,
            &["Alt+Arrow Right"],
            can_go_forward,
            None,
        ),
    ),
    with_params(
        "x_mm?:mm, y_mm?:mm, group?:uint",
        in_canvas(placed(
            place(Group::Go, 4, CommandSize::Small, Some("bar-follow-reference")),
            "link",
            act(
                "nav.follow",
                View,
                "action-nav-follow",
                Menu::Page,
                &["F"],
                has_pages,
                None,
            ),
        )),
    ),
    with_params(
        "group?:uint",
        in_canvas(act(
            "nav.next_link",
            View,
            "action-nav-next-link",
            Menu::Page,
            &["Shift+F"],
            has_pages,
            grp(Group::Go),
        )),
    ),
    with_params(
        "group?:uint",
        in_canvas(act(
            "nav.previous_link",
            View,
            "action-nav-previous-link",
            Menu::Page,
            &["Ctrl+Shift+F"],
            has_pages,
            grp(Group::Go),
        )),
    ),
    // Keyboard tool input (WP-S2.03 §7.7): the connection points in reading order; `[` and `]` are matched as the
    // physical keys by the window, so they need no AltGr on QWERTZ and AZERTY (doc 16 §4.4).
    with_params(
        "group?:uint",
        in_canvas(act(
            "cp.next",
            View,
            "action-cp-next",
            Menu::Page,
            &["]", "Alt+N"],
            has_pages,
            grp(Group::Go),
        )),
    ),
    with_params(
        "group?:uint",
        in_canvas(act(
            "cp.previous",
            View,
            "action-cp-previous",
            Menu::Page,
            &["[", "Alt+Shift+N"],
            has_pages,
            grp(Group::Go),
        )),
    ),
    with_params(
        "group?:uint",
        in_canvas(act(
            "cp.next_unconnected",
            View,
            "action-cp-next-unconnected",
            Menu::Page,
            &["N"],
            has_pages,
            grp(Group::Go),
        )),
    ),
    with_params(
        "alt?:bool",
        in_canvas(act(
            "canvas.activate",
            View,
            "action-canvas-activate",
            Menu::Page,
            &["Enter"],
            has_pages,
            None,
        )),
    ),
    in_canvas(act(
        "tool.back",
        View,
        "action-tool-back",
        Menu::Page,
        &["Backspace"],
        has_pages,
        None,
    )),
    with_params(
        "group?:uint",
        placed(
            place(Group::Zoom, 2, CommandSize::Small, None),
            "zoom-in",
            act(
                "view.zoom_in",
                View,
                "action-view-zoom-in",
                Menu::View,
                &["Ctrl+Plus", "Ctrl+Equals"],
                can_zoom_in,
                None,
            ),
        ),
    ),
    with_params(
        "group?:uint",
        placed(
            place(Group::Zoom, 3, CommandSize::Small, None),
            "zoom-out",
            act(
                "view.zoom_out",
                View,
                "action-view-zoom-out",
                Menu::View,
                &["Ctrl+Minus"],
                can_zoom_out,
                None,
            ),
        ),
    ),
    with_params(
        "group?:uint",
        placed(
            place(Group::Zoom, 4, CommandSize::Small, None),
            "zoom-reset",
            act(
                "view.zoom_actual",
                View,
                "action-view-zoom-actual",
                Menu::View,
                &["Ctrl+0"],
                has_pages,
                None,
            ),
        ),
    ),
    with_params(
        "group?:uint",
        placed(
            place(Group::Zoom, 1, CommandSize::Large, None),
            "maximize",
            act(
                "view.fit_page",
                View,
                "action-view-fit-page",
                Menu::View,
                &["Home"],
                has_pages,
                None,
            ),
        ),
    ),
    with_params(
        "group?:uint",
        act(
            "view.pan_left",
            View,
            "action-view-pan-left",
            Menu::View,
            &["Ctrl+Arrow Left"],
            has_pages,
            grp_sub(Group::Zoom, Submenu::Pan),
        ),
    ),
    with_params(
        "group?:uint",
        act(
            "view.pan_right",
            View,
            "action-view-pan-right",
            Menu::View,
            &["Ctrl+Arrow Right"],
            has_pages,
            grp_sub(Group::Zoom, Submenu::Pan),
        ),
    ),
    with_params(
        "group?:uint",
        act(
            "view.pan_up",
            View,
            "action-view-pan-up",
            Menu::View,
            &["Ctrl+Arrow Up"],
            has_pages,
            grp_sub(Group::Zoom, Submenu::Pan),
        ),
    ),
    with_params(
        "group?:uint",
        act(
            "view.pan_down",
            View,
            "action-view-pan-down",
            Menu::View,
            &["Ctrl+Arrow Down"],
            has_pages,
            grp_sub(Group::Zoom, Submenu::Pan),
        ),
    ),
    placed(
        place(Group::Layout, 1, CommandSize::Small, None),
        "layout-columns",
        act(
            "view.side_by_side",
            View,
            "action-view-side-by-side",
            Menu::View,
            &[],
            has_pages,
            None,
        ),
    ),
    with_params(
        "mode:enum(flat|section|structure)",
        needs_parameters(act(
            "pages.mode",
            View,
            "action-pages-mode",
            Menu::View,
            &[],
            has_pages,
            grp_sub(Group::Panels, Submenu::PageNavigator),
        )),
    ),
    with_params(
        "by:enum(folio|type)",
        needs_parameters(act(
            "pages.sort",
            View,
            "action-pages-sort",
            Menu::View,
            &[],
            has_pages,
            grp_sub(Group::Panels, Submenu::PageNavigator),
        )),
    ),
    placed(
        place(Group::Show, 1, CommandSize::Small, None),
        "grid-dots",
        act(
            "view.grid",
            View,
            "action-view-grid",
            Menu::View,
            &[],
            has_pages,
            None,
        ),
    ),
    placed(
        place(Group::Show, 3, CommandSize::Small, None),
        "chart-line",
        act(
            "view.frame_stats",
            View,
            "action-view-frame-stats",
            Menu::View,
            &[],
            always,
            None,
        ),
    ),
    act(
        "view.cpu_canvas",
        View,
        "action-view-cpu-canvas",
        Menu::View,
        &[],
        always,
        grp(Group::Show),
    ),
    // The property grid (WP-S2.05 part 1, spec §6.10).
    with_params(
        "on?:bool",
        placed(
            place(Group::Panels, 2, CommandSize::Small, None),
            "list-details",
            act(
                "props.show",
                View,
                "action-props-show",
                Menu::View,
                &[],
                project_open,
                None,
            ),
        ),
    ),
    // WP-S2.09 part 1 (§6.8): View ▸ Jobs and View ▸ Notifications.
    with_params(
        "on?:bool",
        placed(
            place(Group::Jobs, 1, CommandSize::Small, None),
            "activity",
            act(
                "jobs.show",
                View,
                "action-jobs-show",
                Menu::Output,
                &[],
                project_open,
                None,
            ),
        ),
    ),
    with_params(
        "on?:bool",
        placed(
            place(Group::Panels, 5, CommandSize::Small, None),
            "bell",
            act(
                "notifications.show",
                View,
                "action-notifications-show",
                Menu::View,
                &[],
                always,
                None,
            ),
        ),
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
                "action-messages-show",
                Menu::Check,
                &[],
                project_open,
                None,
            ),
        ),
    ),
    placed(
        place(Group::Findings, 2, CommandSize::Small, None),
        "arrow-narrow-down",
        act(
            "messages.next",
            View,
            "action-messages-next",
            Menu::Check,
            &["F8"],
            project_open,
            None,
        ),
    ),
    placed(
        place(Group::Findings, 3, CommandSize::Small, None),
        "arrow-narrow-up",
        act(
            "messages.previous",
            View,
            "action-messages-previous",
            Menu::Check,
            &["Shift+F8"],
            project_open,
            None,
        ),
    ),
    with_params(
        "key?:text",
        hidden(in_navigator(act(
            "messages.fix",
            Command,
            "action-messages-fix",
            Menu::Check,
            &[],
            writable,
            None,
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
                "action-view-ui-scale",
                Menu::View,
                &[],
                always,
                sub(Menu::View, Submenu::UiScale, false),
            ),
        )),
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
                "action-view-theme",
                Menu::View,
                &[],
                always,
                sub(Menu::View, Submenu::Theme, false),
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
                "action-view-density",
                Menu::View,
                &[],
                always,
                sub(Menu::View, Submenu::Density, false),
            ),
        )),
    ),
    // WP-S2.17 part 2 (§4.3): the command bar's tab and its collapse (Ctrl+F1, a tab's double click), view actions.
    with_params(
        "tab:enum(project|page|draw|check|output|view)",
        needs_parameters(act(
            "view.command_tab",
            View,
            "action-view-command-tab",
            Menu::View,
            &[],
            always,
            grp_sub(Group::Layout, Submenu::CommandTab),
        )),
    ),
    with_params(
        "collapsed?:bool",
        act(
            "view.command_bar",
            View,
            "action-view-command-bar",
            Menu::View,
            &["Ctrl+F1"],
            always,
            grp(Group::Layout),
        ),
    ),
    with_params(
        "name:text",
        needs_parameters(placed(
            place(Group::Layout, 2, CommandSize::Small, Some("bar-workspaces")),
            "layout-dashboard",
            act(
                "view.workspace_load",
                View,
                "action-view-workspace-load",
                Menu::View,
                &[],
                always,
                sub(Menu::View, Submenu::Workspace, false),
            ),
        )),
    ),
    with_params(
        "name:text",
        act(
            "view.workspace_save",
            View,
            "action-view-workspace-save",
            Menu::View,
            &[],
            always,
            grp_sub(Group::Layout, Submenu::Workspace),
        ),
    ),
    placed(
        place(Group::Layout, 3, CommandSize::Small, None),
        "restore",
        act(
            "view.workspace_reset",
            View,
            "action-view-workspace-reset",
            Menu::View,
            &[],
            always,
            None,
        ),
    ),
    with_params(
        "query?:text",
        with_icon(
            "search",
            act(
                "tools.palette",
                View,
                "action-tools-palette",
                Menu::View,
                &["Ctrl+K", "Ctrl+Shift+P"],
                always,
                grp(Group::Panels),
            ),
        ),
    ),
    placed(
        place(Group::Select, 1, CommandSize::Large, None),
        "pointer",
        act(
            "tool.select",
            View,
            "action-tool-select",
            Menu::Draw,
            &[],
            has_pages,
            None,
        ),
    ),
    placed(
        place(Group::Annotate, 2, CommandSize::Small, None),
        "ruler-measure",
        act(
            "tool.measure",
            View,
            "action-tool-measure",
            Menu::Draw,
            &[],
            has_pages,
            None,
        ),
    ),
    // Place and arrange (WP-S2.04 part 1, §7.6): the Insert menu, the transform keys, Move, Delete, Change symbol.
    in_canvas(placed(
        place(Group::Place, 1, CommandSize::Large, None),
        "circuit-switch-open",
        act(
            "tool.place",
            Command,
            "action-tool-place",
            Menu::Draw,
            &["I"],
            editable_page,
            None,
        ),
    )),
    in_canvas(placed(
        place(Group::Place, 2, CommandSize::Large, Some("bar-device")),
        "package",
        act(
            "tool.place_part",
            Command,
            "action-tool-place-part",
            Menu::Draw,
            &["Shift+I"],
            editable_page,
            None,
        ),
    )),
    with_params(
        "letter?:text",
        act(
            "strip.new",
            Command,
            "action-strip-new",
            Menu::Draw,
            &[],
            editable_page,
            grp(Group::Place),
        ),
    ),
    // FT-409: the structure node and the external reference, one previewed command each.
    with_params(
        "kind:enum(functional_assignment|function|installation_site|location|document_kind|user_defined|type), designation:text, parent?:text",
        placed(
            place(Group::Project, 2, CommandSize::Small, Some("bar-structure")),
            "sitemap",
            act(
                "structure.new",
                Command,
                "action-structure-new",
                Menu::Project,
                &[],
                writable,
                None,
            ),
        ),
    ),
    with_params(
        "location:text, device:text, connection?:text, project?:text",
        act(
            "external_ref.new",
            Command,
            "action-external-ref-new",
            Menu::Project,
            &[],
            writable,
            grp(Group::Project),
        ),
    ),
    with_params(
        "symbol?:text, part?:text, function?:ref, template?:uint, letter?:text, strip?:text, designation?:text",
        hidden(in_canvas(act(
            "place.pick",
            Command,
            "action-place-pick",
            Menu::Draw,
            &[],
            editable_page,
            None,
        ))),
    ),
    with_params(
        "text:text",
        hidden(act(
            "picker.filter",
            View,
            "action-picker-filter",
            Menu::Draw,
            &[],
            picker_open,
            None,
        )),
    ),
    with_params(
        "step?:int, to?:enum(first|last)",
        hidden(act(
            "picker.row",
            View,
            "action-picker-row",
            Menu::Draw,
            &[],
            picker_open,
            None,
        )),
    ),
    hidden(with_params(
        // `arm` reads the optional device letter and, for a terminal, the strip and its designation.
        "letter?:text, strip?:text, designation?:text",
        act(
            "picker.choose",
            Command,
            "action-picker-choose",
            Menu::Draw,
            &[],
            picker_open,
            None,
        ),
    )),
    in_canvas(placed(
        place(Group::Arrange, 1, CommandSize::Small, Some("bar-rotate")),
        "rotate-clockwise",
        act(
            "tool.rotate_cw",
            Command,
            "action-tool-rotate-cw",
            Menu::Draw,
            &["R"],
            editable_page,
            None,
        ),
    )),
    in_canvas(act(
        "tool.rotate_ccw",
        Command,
        "action-tool-rotate-ccw",
        Menu::Draw,
        &["Shift+R"],
        editable_page,
        grp(Group::Arrange),
    )),
    in_canvas(placed(
        place(Group::Arrange, 2, CommandSize::Small, Some("bar-mirror")),
        "flip-vertical",
        act(
            "tool.mirror_x",
            Command,
            "action-tool-mirror-x",
            Menu::Draw,
            &["X"],
            editable_page,
            None,
        ),
    )),
    in_canvas(act(
        "tool.mirror_y",
        Command,
        "action-tool-mirror-y",
        Menu::Draw,
        &["Y"],
        editable_page,
        grp(Group::Arrange),
    )),
    in_canvas(act(
        "tool.next_variant",
        Command,
        "action-tool-next-variant",
        Menu::Draw,
        &["V"],
        editable_page,
        grp(Group::Arrange),
    )),
    in_canvas(act(
        "tool.next_rep_type",
        View,
        "action-tool-next-rep-type",
        Menu::Draw,
        &["Shift+V"],
        editable_page,
        grp(Group::Arrange),
    )),
    in_canvas(act(
        "tool.move",
        Command,
        "action-tool-move",
        Menu::Draw,
        &["M"],
        editable_page,
        grp(Group::Arrange),
    )),
    in_canvas(act(
        "edit.delete_selection",
        Command,
        "action-edit-delete-selection",
        Menu::Edit,
        &["Delete"],
        editable_page,
        slot_sep(Menu::Edit),
    )),
    // WP-S2.05 part 3: the clipboard (§8.8). The chords are the canvas's: a text field of the property grid or the find
    // pane keeps egui's own copy and paste, and the window routes no chord while a text field has the keyboard.
    in_canvas(act(
        "edit.cut",
        Command,
        "action-edit-cut",
        Menu::Edit,
        &["Ctrl+X"],
        selection_editable,
        slot(Menu::Edit),
    )),
    in_canvas(act(
        "edit.copy",
        View,
        "action-edit-copy",
        Menu::Edit,
        &["Ctrl+C"],
        selection_shown,
        slot(Menu::Edit),
    )),
    with_params(
        "text?:text",
        in_canvas(with_icon(
            "clipboard-list",
            act(
                "edit.paste_clip",
                Command,
                "action-edit-paste-clip",
                Menu::Edit,
                &["Ctrl+V"],
                paste_page,
                slot(Menu::Edit),
            ),
        )),
    ),
    in_canvas(act(
        "edit.duplicate",
        Command,
        "action-edit-duplicate",
        Menu::Edit,
        &["Ctrl+D"],
        selection_editable,
        slot(Menu::Edit),
    )),
    in_canvas(act(
        "edit.copy_tags",
        View,
        "action-edit-copy-tags",
        Menu::Edit,
        &[],
        selection_shown,
        slot(Menu::Edit),
    )),
    with_params(
        "text:text",
        headless(act(
            "clipboard.text",
            View,
            "action-clipboard-text",
            Menu::Edit,
            &[],
            always,
            None,
        )),
    ),
    with_params(
        "symbol?:text",
        in_canvas(with_icon(
            "circuit-switch-open",
            act(
                "rep.exchange",
                Command,
                "action-rep-exchange",
                Menu::Draw,
                &[],
                editable_page,
                grp(Group::Arrange),
            ),
        )),
    ),
    hidden(in_canvas(act(
        "tool.finish",
        View,
        "action-tool-finish",
        Menu::Edit,
        &["Shift+Enter"],
        has_pages,
        None,
    ))),
    with_params(
        "text:text",
        hidden(in_canvas(act(
            "text.input",
            View,
            "action-text-input",
            Menu::Edit,
            &[],
            has_pages,
            None,
        ))),
    ),
    // WP-S2.04 part 2 (wiring, §8.7): the Wire tool, the proposals' Tab, route edit, the junction sequence editor and
    // the crossings setting. Begin.
    in_canvas(placed(
        place(Group::Connect, 1, CommandSize::Large, Some("bar-wire")),
        "route-2",
        act(
            "tool.wire",
            Command,
            "action-tool-wire",
            Menu::Draw,
            &["W"],
            wire_page,
            None,
        ),
    )),
    // Ctrl+Enter: a click with Alt held at the grid cursor (a placement without its autoconnect proposals, a bend
    // without object snaps), the keyboard's twin of `canvas.activate {alt}` (fix round; per-context chords, ID-628).
    hidden(in_canvas(act(
        "canvas.activate_alt",
        View,
        "action-canvas-activate-alt",
        Menu::Page,
        &["Ctrl+Enter"],
        has_pages,
        None,
    ))),
    hidden(in_canvas(act(
        "tool.next_proposal",
        View,
        "action-tool-next-proposal",
        Menu::Edit,
        &["Tab"],
        editable_page,
        None,
    ))),
    hidden(in_canvas(act(
        "tool.connect_on_move",
        Command,
        "action-tool-connect-on-move",
        Menu::Edit,
        &["C"],
        editable_page,
        None,
    ))),
    in_canvas(with_icon(
        "route-2",
        act(
            "tool.route_edit",
            Command,
            "action-tool-route-edit",
            Menu::Draw,
            &[],
            one_connection,
            grp(Group::Connect),
        ),
    )),
    in_canvas(act(
        "junction.open",
        View,
        "action-junction-open",
        Menu::Draw,
        &[],
        one_junction,
        grp(Group::Connect),
    )),
    with_params(
        "from:uint, to:uint",
        hidden(act(
            "junction.member",
            Command,
            "action-junction-member",
            Menu::Edit,
            &[],
            junction_editor_open,
            None,
        )),
    ),
    with_params(
        "style:enum(dot|bevel), orient?:uint",
        hidden(act(
            "junction.style",
            Command,
            "action-junction-style",
            Menu::Edit,
            &[],
            junction_editor_open,
            None,
        )),
    ),
    with_params(
        "wiring:enum(star|chain|style_default)",
        hidden(act(
            "junction.wiring",
            Command,
            "action-junction-wiring",
            Menu::Edit,
            &[],
            junction_editor_open,
            None,
        )),
    ),
    with_params(
        "on?:bool",
        act(
            "project.crossings",
            Command,
            "action-project-crossings",
            Menu::Draw,
            &[],
            writable,
            grp(Group::Connect),
        ),
    ),
    // WP-S2.04 part 2. End.
    // WP-S2.04 part 3 (interruption points and black boxes, §9.6). Begin.
    in_canvas(placed(
        place(Group::Connect, 3, CommandSize::Small, Some("bar-interruption")),
        "ew-interruption-point",
        act(
            "tool.ip",
            Command,
            "action-tool-ip",
            Menu::Draw,
            &["P"],
            editable_page,
            None,
        ),
    )),
    with_params(
        "symbol?:text, part?:text, template?:uint, letter?:text",
        in_canvas(placed(
            place(Group::Place, 4, CommandSize::Small, None),
            "ew-black-box",
            act(
                "tool.black_box",
                Command,
                "action-tool-black-box",
                Menu::Draw,
                &["B"],
                editable_page,
                None,
            ),
        )),
    ),
    with_params(
        "direction?:enum(source|target|undirected)",
        in_canvas(act(
            "tool.ip_direction",
            View,
            "action-tool-ip-direction",
            Menu::Draw,
            &["D"],
            editable_page,
            grp(Group::Connect),
        )),
    ),
    in_canvas(act(
        "tool.star_source",
        View,
        "action-tool-star-source",
        Menu::Draw,
        &[],
        editable_page,
        grp(Group::Connect),
    )),
    with_params(
        "text:text",
        hidden(in_canvas(act(
            "ip.name",
            View,
            "action-ip-name",
            Menu::Edit,
            &[],
            editable_page,
            None,
        ))),
    ),
    // `external`: an external reference's ID, alias or printed text (`JE-N1000-03:L`); empty clears it.
    with_params(
        "external:text",
        hidden(in_canvas(act(
            "ip.external",
            View,
            "action-ip-external",
            Menu::Edit,
            &[],
            editable_page,
            None,
        ))),
    ),
    with_params(
        "folio?:text, x_mm?:mm, y_mm?:mm",
        in_canvas(act(
            "ip.continue_on",
            Command,
            "action-ip-continue-on",
            Menu::Draw,
            &[],
            one_ip,
            grp(Group::Connect),
        )),
    ),
    with_params(
        "by?:enum(name|potential), chain_order?:bool",
        in_canvas(act(
            "ip.arrange_selected",
            Command,
            "action-ip-arrange-selected",
            Menu::Draw,
            &[],
            two_ips,
            grp(Group::Connect),
        )),
    ),
    with_params(
        "grip:uint",
        hidden(in_canvas(act(
            "box.grip",
            Command,
            "action-box-grip",
            Menu::Edit,
            &[],
            one_box,
            None,
        ))),
    ),
    in_canvas(act(
        "box.add_cp",
        Command,
        "action-box-add-cp",
        Menu::Draw,
        &[],
        one_box,
        grp(Group::Place),
    )),
    // WP-S2.04 part 3. End.
    // WP-S2.04 part 4b (text and graphic tools, §10.7). Begin.
    in_canvas(placed(
        place(Group::Annotate, 1, CommandSize::Large, None),
        "typography",
        act(
            "tool.text",
            Command,
            "action-tool-text",
            Menu::Draw,
            &["T"],
            editable_page,
            None,
        ),
    )),
    with_params(
        "shape?:enum(line|polyline|rect|circle|arc|polygon)",
        in_canvas(with_icon(
            "pencil",
            act(
                "tool.graphic",
                Command,
                "action-tool-graphic",
                Menu::Draw,
                &["L"],
                editable_page,
                grp_sub(Group::Annotate, Submenu::Graphic),
            ),
        )),
    ),
    with_params(
        "bind:text",
        hidden(in_canvas(act(
            "text.bind",
            View,
            "action-text-bind",
            Menu::Edit,
            &[],
            editable_page,
            None,
        ))),
    ),
    in_canvas(with_icon(
        "pencil",
        act(
            "text.edit",
            Command,
            "action-text-edit",
            Menu::Draw,
            &["F2"],
            one_free_text,
            grp(Group::Annotate),
        ),
    )),
    // FT-411: Edit ▸ Bridge terminals.
    in_canvas(act(
        "strip.bridge",
        Command,
        "action-strip-bridge",
        Menu::Draw,
        &[],
        two_strip_terminals,
        grp(Group::Connect),
    )),
    // WP-S2.04 part 4b: the Wire marker and Structure box tools, their fields and the marker mode.
    in_canvas(with_icon(
        "ew-cable",
        act(
            "tool.marker",
            Command,
            "action-tool-marker",
            Menu::Draw,
            &["Shift+W"],
            editable_page,
            grp(Group::Connect),
        ),
    )),
    with_params(
        "csa?:text, colours?:text",
        hidden(in_canvas(act(
            "marker.fields",
            View,
            "action-marker-fields",
            Menu::Draw,
            &[],
            editable_page,
            None,
        ))),
    ),
    in_canvas(with_icon(
        "ew-structure-box",
        act(
            "tool.structure_box",
            Command,
            "action-tool-structure-box",
            Menu::Draw,
            &["Shift+B"],
            editable_page,
            grp(Group::Annotate),
        ),
    )),
    with_params(
        "style?:enum(dotted|dash_dot), caption?:text, location?:text, external?:text",
        hidden(in_canvas(act(
            "structure_box.fields",
            View,
            "action-structure-box-fields",
            Menu::Draw,
            &[],
            editable_page,
            None,
        ))),
    ),
    with_params(
        "mode:enum(per_wire|all|deviations|explicit|none)",
        act(
            "project.marker_mode",
            Command,
            "action-project-marker-mode",
            Menu::Draw,
            &[],
            writable,
            grp_sub(Group::Connect, Submenu::WireMarkers),
        ),
    ),
    // WP-S2.04 part 4b. End.
    // WP-S2.04 part 5 (images, §11).
    in_canvas(with_icon(
        "file",
        act(
            "tool.image",
            Command,
            "action-tool-image",
            Menu::Draw,
            &[],
            editable_page,
            grp(Group::Annotate),
        ),
    )),
    with_params(
        "asset:text",
        hidden(in_canvas(act(
            "image.pick",
            View,
            "action-image-pick",
            Menu::Draw,
            &[],
            editable_page,
            None,
        ))),
    ),
    with_params(
        "path:path",
        hidden(in_canvas(act(
            "image.file",
            View,
            "action-image-file",
            Menu::Draw,
            &[],
            editable_page,
            None,
        ))),
    ),
    placed(
        place(
            Group::Standards,
            2,
            CommandSize::Small,
            Some("bar-company-standard"),
        ),
        "building-factory-2",
        act(
            "tools.company_standard",
            Command,
            "action-tools-company-standard",
            Menu::Project,
            &[],
            always,
            None,
        ),
    ),
    // WP-S2.09 part 2 (§7.6): Tools ▸ Run checks.
    placed(
        place(Group::Run, 1, CommandSize::Large, Some("bar-check-project")),
        "list-check",
        act(
            "messages.check",
            Command,
            "action-messages-check",
            Menu::Check,
            &["F7"],
            project_open,
            None,
        ),
    ),
    // WP-S2.09 part 3 (§8.3, §8.6): Tools, Renumber devices... and Purge unused images...
    with_params(
        "scope?:enum(project|page|selection), scheme?:text, include_frozen?:bool, only_nonconforming?:bool",
        placed(
            place(Group::Arrange, 4, CommandSize::Small, Some("bar-renumber")),
            "sort-ascending-numbers",
            act(
                "devices.renumber",
                Command,
                "action-devices-renumber",
                Menu::Draw,
                &[],
                writable,
                None,
            ),
        ),
    ),
    with_params(
        "confirm?:bool",
        act(
            "project.purge_assets",
            Command,
            "action-project-purge-assets",
            Menu::Project,
            &[],
            writable,
            slot_sep(Menu::Project),
        ),
    ),
    act(
        "tab.new",
        View,
        "action-tab-new",
        Menu::Window,
        &["Ctrl+T"],
        can_add_tab,
        slot(Menu::Window),
    ),
    act(
        "tab.close",
        View,
        "action-tab-close",
        Menu::Window,
        &["Ctrl+W"],
        can_close_tab,
        slot(Menu::Window),
    ),
    act(
        "tab.next",
        View,
        "action-tab-next",
        Menu::Window,
        &["Ctrl+Tab"],
        can_cycle_tabs,
        slot(Menu::Window),
    ),
    act(
        "tab.previous",
        View,
        "action-tab-previous",
        Menu::Window,
        &["Ctrl+Shift+Tab"],
        can_cycle_tabs,
        slot(Menu::Window),
    ),
    act(
        "tab.focus_other",
        View,
        "action-tab-focus-other",
        Menu::Window,
        &["Ctrl+F6"],
        two_groups,
        slot(Menu::Window),
    ),
    with_params(
        "topic?:text",
        act(
            "help.documentation",
            View,
            "action-help-documentation",
            Menu::Help,
            &["F1"],
            always,
            slot(Menu::Help),
        ),
    ),
    with_params(
        "dir?:path, discard?:bool",
        act(
            "help.tutorials",
            Command,
            "action-help-tutorials",
            Menu::Help,
            &[],
            always,
            slot(Menu::Help),
        ),
    ),
    with_params(
        "step?:enum(language|origin|privacy|appearance|start)",
        act(
            "help.first_run",
            View,
            "action-help-first-run",
            Menu::Help,
            &[],
            always,
            slot_sep(Menu::Help),
        ),
    ),
    act(
        "help.shortcuts",
        View,
        "action-help-shortcuts",
        Menu::Help,
        &[],
        always,
        slot(Menu::Help),
    ),
    act(
        "help.notices",
        View,
        "action-help-notices",
        Menu::Help,
        &[],
        always,
        slot(Menu::Help),
    ),
    act(
        "help.about",
        View,
        "action-help-about",
        Menu::Help,
        &[],
        always,
        slot(Menu::Help),
    ),
    with_params(
        "cmd:text, params?:json",
        needs_parameters(act(
            "command.run",
            Command,
            "action-command-run",
            Menu::View,
            &[],
            writable,
            None,
        )),
    ),
    with_params(
        "path:path, dpi?:uint",
        headless(act(
            "view.snapshot",
            View,
            "action-view-snapshot",
            Menu::View,
            &[],
            page_shown,
            None,
        )),
    ),
    with_params(
        "path:path, group?:uint",
        headless(act(
            "canvas.snapshot",
            View,
            "action-canvas-snapshot",
            Menu::View,
            &[],
            has_pages,
            None,
        )),
    ),
    with_params(
        "percent:json, x_mm?:mm, y_mm?:mm, group?:uint",
        hidden(act(
            "view.zoom_to",
            View,
            "action-view-zoom-to",
            Menu::View,
            &[],
            has_pages,
            None,
        )),
    ),
    with_params(
        "dx:json, dy:json, group?:uint",
        hidden(act(
            "view.pan",
            View,
            "action-view-pan",
            Menu::View,
            &[],
            has_pages,
            None,
        )),
    ),
    with_params(
        "width:json, height:json, ppp?:json, group?:uint",
        hidden(act(
            "view.canvas_resize",
            View,
            "action-view-canvas-resize",
            Menu::View,
            &[],
            always,
            None,
        )),
    ),
    with_params(
        "x_mm?:mm, y_mm?:mm, group?:uint, shift?:bool, ctrl?:bool, alt?:bool",
        hidden(act(
            "canvas.pointer",
            View,
            "action-canvas-pointer",
            Menu::View,
            &[],
            has_pages,
            None,
        )),
    ),
    with_params(
        "path:path",
        headless(act(
            "canvas.preview_snapshot",
            View,
            "action-canvas-preview-snapshot",
            Menu::View,
            &[],
            has_link_preview,
            None,
        )),
    ),
    with_params(
        "x_mm:mm, y_mm:mm, button?:enum(primary|middle), shift?:bool, ctrl?:bool, alt?:bool, group?:uint",
        hidden(in_canvas(act(
            "canvas.press",
            View,
            "action-canvas-press",
            Menu::View,
            &[],
            has_pages,
            None,
        ))),
    ),
    with_params(
        "x_mm:mm, y_mm:mm, button?:enum(primary|middle), shift?:bool, ctrl?:bool, alt?:bool, group?:uint",
        hidden(in_canvas(act(
            "canvas.release",
            View,
            "action-canvas-release",
            Menu::View,
            &[],
            has_pages,
            None,
        ))),
    ),
    with_params(
        "ms:uint",
        hidden(in_canvas(act(
            "canvas.dwell",
            View,
            "action-canvas-dwell",
            Menu::View,
            &[],
            has_pages,
            None,
        ))),
    ),
    headless(act(
        "ui.describe",
        View,
        "action-ui-describe",
        Menu::View,
        &[],
        always,
        None,
    )),
    with_params(
        "folio:text",
        hidden(act(
            "page.goto",
            View,
            "action-page-goto",
            Menu::Page,
            &[],
            has_pages,
            None,
        )),
    ),
    with_params(
        "group:uint, index:uint",
        hidden(act(
            "tab.activate",
            View,
            "action-tab-activate",
            Menu::Window,
            &[],
            has_pages,
            None,
        )),
    ),
    with_params(
        "group:uint, index:uint, to_group:uint, to_index:uint",
        hidden(act(
            "tab.move",
            View,
            "action-tab-move",
            Menu::Window,
            &[],
            has_pages,
            None,
        )),
    ),
    with_params(
        "folio:text, group?:uint",
        hidden(act(
            "tab.open",
            View,
            "action-tab-open",
            Menu::Window,
            &[],
            has_pages,
            None,
        )),
    ),
    with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.left",
            View,
            "action-cursor-left",
            Menu::Page,
            &["Arrow Left"],
            has_pages,
            None,
        ))),
    ),
    with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.right",
            View,
            "action-cursor-right",
            Menu::Page,
            &["Arrow Right"],
            has_pages,
            None,
        ))),
    ),
    with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.up",
            View,
            "action-cursor-up",
            Menu::Page,
            &["Arrow Up"],
            has_pages,
            None,
        ))),
    ),
    with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.down",
            View,
            "action-cursor-down",
            Menu::Page,
            &["Arrow Down"],
            has_pages,
            None,
        ))),
    ),
    with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.cell_left",
            View,
            "action-cursor-cell-left",
            Menu::Page,
            &["Shift+Arrow Left"],
            has_pages,
            None,
        ))),
    ),
    with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.cell_right",
            View,
            "action-cursor-cell-right",
            Menu::Page,
            &["Shift+Arrow Right"],
            has_pages,
            None,
        ))),
    ),
    with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.cell_up",
            View,
            "action-cursor-cell-up",
            Menu::Page,
            &["Shift+Arrow Up"],
            has_pages,
            None,
        ))),
    ),
    with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.cell_down",
            View,
            "action-cursor-cell-down",
            Menu::Page,
            &["Shift+Arrow Down"],
            has_pages,
            None,
        ))),
    ),
    with_params(
        "x_mm?:mm, y_mm?:mm, group?:uint",
        hidden(in_canvas(act(
            "cursor.set",
            View,
            "action-cursor-set",
            Menu::Page,
            &[],
            has_pages,
            None,
        ))),
    ),
    with_params(
        "text:text",
        hidden(act(
            "pages.filter",
            View,
            "action-pages-filter",
            Menu::View,
            &[],
            has_pages,
            None,
        )),
    ),
    with_params(
        "key:text",
        hidden(in_navigator(act(
            "pages.toggle",
            View,
            "action-pages-toggle",
            Menu::View,
            &[],
            has_pages,
            None,
        ))),
    ),
    with_params(
        "key?:text, step?:int, to?:enum(first|last)",
        hidden(in_navigator(act(
            "pages.row",
            View,
            "action-pages-row",
            Menu::View,
            &[],
            has_pages,
            None,
        ))),
    ),
    with_params(
        "key?:text, folio?:text, new_tab?:bool",
        hidden(in_navigator(act(
            "pages.open",
            View,
            "action-pages-open",
            Menu::View,
            &[],
            has_pages,
            None,
        ))),
    ),
    // WP-S2.06 part 1: the navigator framework (§6.9).
    with_params(
        "kind:enum(pages|devices|connections|strips|cables|plc|potentials|ips|structure|parts|blocks|messages), on?:bool",
        needs_parameters(placed(
            place(Group::Panels, 1, CommandSize::Small, Some("bar-navigators")),
            "layout-sidebar",
            act(
                "navigator.show",
                View,
                "action-navigator-show",
                Menu::View,
                &[],
                has_pages,
                sub(Menu::View, Submenu::Navigators, false),
            ),
        )),
    ),
    with_params(
        "kind:text, view:text",
        hidden(act(
            "navigator.view",
            View,
            "action-navigator-view",
            Menu::View,
            &[],
            has_pages,
            None,
        )),
    ),
    with_params(
        "kind:text, by:text, descending?:bool",
        hidden(act(
            "navigator.sort",
            View,
            "action-navigator-sort",
            Menu::View,
            &[],
            has_pages,
            None,
        )),
    ),
    with_params(
        "kind:text, text:text",
        hidden(act(
            "navigator.filter",
            View,
            "action-navigator-filter",
            Menu::View,
            &[],
            has_pages,
            None,
        )),
    ),
    with_params(
        "kind:text, key:text",
        hidden(in_navigator(act(
            "navigator.toggle",
            View,
            "action-navigator-toggle",
            Menu::View,
            &[],
            has_pages,
            None,
        ))),
    ),
    with_params(
        "kind:text, key?:text, step?:int, to?:enum(first|last), extend?:bool",
        hidden(in_navigator(act(
            "navigator.row",
            View,
            "action-navigator-row",
            Menu::View,
            &[],
            has_pages,
            None,
        ))),
    ),
    with_params(
        "kind:text, key?:text, new_tab?:bool",
        hidden(in_navigator(act(
            "navigator.open",
            View,
            "action-navigator-open",
            Menu::View,
            &[],
            has_pages,
            None,
        ))),
    ),
    with_params(
        "kind:text, keys:list(text), add?:bool",
        hidden(act(
            "navigator.select",
            View,
            "action-navigator-select",
            Menu::View,
            &[],
            has_pages,
            None,
        )),
    ),
    with_params(
        "kind?:text, key?:text",
        hidden(in_navigator(act(
            "navigator.mark",
            View,
            "action-navigator-mark",
            Menu::View,
            &["Space"],
            has_pages,
            None,
        ))),
    ),
    with_params(
        "kind:text, prefix:text",
        hidden(in_navigator(act(
            "navigator.jump",
            View,
            "action-navigator-jump",
            Menu::View,
            &[],
            has_pages,
            None,
        ))),
    ),
    hidden(in_navigator(act(
        "navigator.apply",
        Command,
        "action-navigator-apply",
        Menu::Edit,
        &[],
        reorder_card,
        None,
    ))),
    with_params(
        "pages?:list(text), before?:text, end?:bool",
        hidden(in_navigator(act(
            "pages.reorder",
            Command,
            "action-pages-reorder",
            Menu::Page,
            &[],
            pages_writable,
            None,
        ))),
    ),
    in_navigator(placed(
        place(Group::Order, 2, CommandSize::Small, Some("bar-move-up")),
        "arrow-up",
        act(
            "pages.move_up",
            Command,
            "action-pages-move-up",
            Menu::Page,
            &["Alt+Arrow Up"],
            pages_writable,
            None,
        ),
    )),
    in_navigator(placed(
        place(Group::Order, 3, CommandSize::Small, Some("bar-move-down")),
        "arrow-down",
        act(
            "pages.move_down",
            Command,
            "action-pages-move-down",
            Menu::Page,
            &["Alt+Arrow Down"],
            pages_writable,
            None,
        ),
    )),
    // WP-S2.06 part 2: potential and net highlighting (§7.4).
    with_params(
        "kind?:text, key?:text, off?:bool",
        in_navigator(placed(
            place(Group::Trace, 1, CommandSize::Small, None),
            "highlight",
            act(
                "navigator.highlight",
                View,
                "action-navigator-highlight",
                Menu::Check,
                &["Ctrl+Shift+H"],
                has_pages,
                None,
            ),
        )),
    ),
    placed(
        place(Group::Trace, 2, CommandSize::Small, None),
        "highlight-off",
        act(
            "view.clear_highlights",
            View,
            "action-view-clear-highlights",
            Menu::Check,
            &[],
            has_highlights,
            None,
        ),
    ),
    // WP-S2.06 part 3: table mode (§8.8).
    with_params(
        "kind:text, on?:bool",
        hidden(act(
            "navigator.table",
            View,
            "action-navigator-table",
            Menu::View,
            &[],
            has_pages,
            None,
        )),
    ),
    with_params(
        "kind:text, show?:list(text), hide?:list(text), order?:list(text)",
        hidden(act(
            "table.columns",
            View,
            "action-table-columns",
            Menu::View,
            &[],
            has_pages,
            None,
        )),
    ),
    with_params(
        "kind?:text, row?:text, column?:text, step?:int, across?:int",
        hidden(in_navigator(act(
            "table.cell",
            View,
            "action-table-cell",
            Menu::View,
            &[],
            has_pages,
            None,
        ))),
    ),
    with_params(
        "kind?:text, row?:text, column?:text",
        hidden(in_navigator(act(
            "table.edit",
            View,
            "action-table-edit",
            Menu::Edit,
            &["F2"],
            pages_writable,
            None,
        ))),
    ),
    with_params(
        "kind?:text, text:text",
        hidden(in_navigator(act(
            "table.input",
            View,
            "action-table-input",
            Menu::Edit,
            &[],
            pages_writable,
            None,
        ))),
    ),
    with_params(
        "kind?:text, all?:bool",
        hidden(in_navigator(act(
            "table.commit",
            Command,
            "action-table-commit",
            Menu::Edit,
            &[],
            pages_writable,
            None,
        ))),
    ),
    with_params(
        "kind:text, column:text, value:text, rows?:list(text), all?:bool",
        hidden(act(
            "table.set",
            Command,
            "action-table-set",
            Menu::Edit,
            &[],
            pages_writable,
            None,
        )),
    ),
    with_params(
        "kind?:text",
        in_navigator(act(
            "table.fill_down",
            Command,
            "action-table-fill-down",
            Menu::Edit,
            &["Ctrl+D"],
            pages_writable,
            None,
        )),
    ),
    with_params(
        "kind?:text, pattern?:text",
        in_navigator(act(
            "table.pattern_fill",
            Command,
            "action-table-pattern-fill",
            Menu::Edit,
            &["Ctrl+Shift+D"],
            pages_writable,
            None,
        )),
    ),
    with_params(
        "kind?:text, text?:text",
        in_navigator(act(
            "table.paste",
            Command,
            "action-table-paste",
            Menu::Edit,
            &["Ctrl+V"],
            pages_writable,
            None,
        )),
    ),
    with_params(
        "kind?:text",
        in_navigator(act(
            "table.copy",
            View,
            "action-table-copy",
            Menu::Edit,
            &["Ctrl+C"],
            has_pages,
            None,
        )),
    ),
    with_params(
        "kind?:text",
        in_navigator(act(
            "table.clear",
            Command,
            "action-table-clear",
            Menu::Edit,
            &["Delete"],
            pages_writable,
            None,
        )),
    ),
    with_params(
        "cell:uint, choice:uint",
        hidden(in_navigator(act(
            "table.resolve",
            Command,
            "action-table-resolve",
            Menu::Edit,
            &[],
            table_card,
            None,
        ))),
    ),
    with_params(
        "cell:uint",
        hidden(in_navigator(act(
            "table.drop",
            Command,
            "action-table-drop",
            Menu::Edit,
            &[],
            table_card,
            None,
        ))),
    ),
    with_params(
        "area:enum(navigator|canvas|properties|find), group?:uint, kind?:text",
        hidden(act(
            "ui.focus",
            View,
            "action-ui-focus",
            Menu::Window,
            &[],
            project_open,
            None,
        )),
    ),
    hidden(act(
        "ui.focus_next",
        View,
        "action-ui-focus-next",
        Menu::Window,
        &["F6"],
        project_open,
        None,
    )),
    hidden(act(
        "ui.focus_previous",
        View,
        "action-ui-focus-previous",
        Menu::Window,
        &["Shift+F6"],
        project_open,
        None,
    )),
    hidden(act(
        "ui.dismiss",
        View,
        "action-ui-dismiss",
        Menu::View,
        &["Escape"],
        always,
        None,
    )),
    with_params(
        "key:text, value:json",
        hidden(act(
            "settings.set",
            Command,
            "action-settings-set",
            Menu::View,
            &[],
            always,
            None,
        )),
    ),
    with_params(
        "field:text, value:json",
        hidden(act(
            "first_run.set",
            View,
            "action-first-run-set",
            Menu::Help,
            &[],
            first_run_open,
            None,
        )),
    ),
    hidden(act(
        "first_run.next",
        View,
        "action-first-run-next",
        Menu::Help,
        &[],
        first_run_open,
        None,
    )),
    hidden(act(
        "first_run.back",
        View,
        "action-first-run-back",
        Menu::Help,
        &[],
        first_run_open,
        None,
    )),
    with_params(
        "start?:enum(wizard|sample|qet|new|none)",
        hidden(act(
            "first_run.finish",
            Command,
            "action-first-run-finish",
            Menu::Help,
            &[],
            first_run_open,
            None,
        )),
    ),
    with_params(
        "field:text, value:text",
        hidden(act(
            "wizard.set",
            View,
            "action-wizard-set",
            Menu::Project,
            &[],
            wizard_open,
            None,
        )),
    ),
    hidden(act(
        "wizard.next",
        View,
        "action-wizard-next",
        Menu::Project,
        &[],
        wizard_open,
        None,
    )),
    hidden(act(
        "wizard.back",
        View,
        "action-wizard-back",
        Menu::Project,
        &[],
        wizard_open,
        None,
    )),
    with_params(
        "step:uint",
        hidden(act(
            "wizard.goto",
            View,
            "action-wizard-goto",
            Menu::Project,
            &[],
            wizard_open,
            None,
        )),
    ),
    hidden(act(
        "wizard.reset",
        View,
        "action-wizard-reset",
        Menu::Project,
        &[],
        wizard_open,
        None,
    )),
    hidden(act(
        "wizard.cancel",
        View,
        "action-wizard-cancel",
        Menu::Project,
        &[],
        wizard_open,
        None,
    )),
    with_params(
        "dir?:path",
        hidden(act(
            "wizard.finish",
            Command,
            "action-wizard-finish",
            Menu::Project,
            &[],
            wizard_open,
            None,
        )),
    ),
    with_params(
        "confirm?:bool",
        hidden(act(
            "wizard.apply_check_scheme",
            Command,
            "action-wizard-apply-check-scheme",
            Menu::Project,
            &[],
            wizard_open,
            None,
        )),
    ),
    // The property grid's own controls (WP-S2.05 part 1, spec §6.10): the window and the headless mode dispatch them.
    with_params(
        "ids?:list(ref), refs?:list(ref)",
        hidden(act(
            "select.set",
            View,
            "action-select-set",
            Menu::Edit,
            &[],
            project_open,
            None,
        )),
    ),
    with_params(
        "step?:int, to?:enum(first|last)",
        hidden(in_properties(act(
            "props.row",
            View,
            "action-props-row",
            Menu::Edit,
            &[],
            grid_shown,
            None,
        ))),
    ),
    with_params(
        "category:text, expanded?:bool",
        hidden(in_properties(act(
            "props.category",
            View,
            "action-props-category",
            Menu::Edit,
            &[],
            grid_shown,
            None,
        ))),
    ),
    with_params(
        "row?:text",
        hidden(in_properties(act(
            "props.edit",
            View,
            "action-props-edit",
            Menu::Edit,
            // Enter is the canvas's `canvas.activate`: the grid's own Enter is the pane's key (§6.9).
            &["F2"],
            grid_shown,
            None,
        ))),
    ),
    with_params(
        "text:text",
        hidden(in_properties(act(
            "props.input",
            View,
            "action-props-input",
            Menu::Edit,
            &[],
            grid_shown,
            None,
        ))),
    ),
    hidden(in_properties(act(
        "props.commit",
        Command,
        "action-props-commit",
        Menu::Edit,
        &[],
        grid_writable,
        None,
    ))),
    hidden(in_properties(act(
        "props.cancel",
        View,
        "action-props-cancel",
        Menu::Edit,
        &[],
        grid_shown,
        None,
    ))),
    with_params(
        "row?:text, value?:text, choice?:uint",
        hidden(act(
            "props.set",
            Command,
            "action-props-set",
            Menu::Edit,
            &[],
            grid_writable,
            None,
        )),
    ),
    with_params(
        "index?:uint, step?:int",
        hidden(in_properties(act(
            "props.choose",
            Command,
            "action-props-choose",
            Menu::Edit,
            &[],
            grid_shown,
            None,
        ))),
    ),
    with_params(
        "step?:int",
        hidden(in_properties(act(
            "props.row_menu",
            View,
            "action-props-row-menu",
            Menu::Edit,
            &["Shift+F10"],
            grid_shown,
            None,
        ))),
    ),
    with_params(
        "row?:text, action:enum(edit|clear|override|reset|freeze|unfreeze)",
        hidden(in_properties(act(
            "props.action",
            Command,
            "action-props-action",
            Menu::Edit,
            &[],
            grid_shown,
            None,
        ))),
    ),
    with_params(
        "row?:text, op:enum(add|remove|up|down)",
        hidden(in_properties(act(
            "props.list",
            Command,
            "action-props-list",
            Menu::Edit,
            &[],
            grid_writable,
            None,
        ))),
    ),
    with_params(
        "row?:text, lang?:text, text?:text",
        hidden(in_properties(act(
            "props.mltext",
            Command,
            "action-props-mltext",
            Menu::Edit,
            &["Shift+Enter"],
            grid_shown,
            None,
        ))),
    ),
    hidden(in_properties(act(
        "preview.apply",
        Command,
        "action-preview-apply",
        Menu::Edit,
        &["Ctrl+Enter"],
        card_writable,
        None,
    ))),
    hidden(in_properties(act(
        "preview.cancel",
        View,
        "action-preview-cancel",
        Menu::Edit,
        &[],
        card_shown,
        None,
    ))),
    // WP-S2.09 part 3 (§8.1, §8.4): the Preview tab and the history panel's own moves.
    with_params(
        "on?:bool",
        hidden(act(
            "preview.show",
            View,
            "action-preview-show",
            Menu::View,
            &[],
            project_open,
            None,
        )),
    ),
    with_params(
        "step?:int, to?:enum(first|last)",
        hidden(act(
            "history.row",
            View,
            "action-history-row",
            Menu::Edit,
            &[],
            history_open,
            None,
        )),
    ),
    with_params(
        "entry?:uint",
        hidden(act(
            "history.undo_to",
            Command,
            "action-history-undo-to",
            Menu::Edit,
            &[],
            history_writable,
            None,
        )),
    ),
    with_params(
        "entry?:uint",
        hidden(act(
            "history.changes",
            View,
            "action-history-changes",
            Menu::Edit,
            &[],
            history_open,
            None,
        )),
    ),
    hidden(act(
        "toast.undo",
        Command,
        "action-toast-undo",
        Menu::Edit,
        &[],
        writable,
        None,
    )),
    hidden(act(
        "toast.dismiss",
        View,
        "action-toast-dismiss",
        Menu::Edit,
        &[],
        project_open,
        None,
    )),
    // WP-S2.09 part 1 (§6.8): the jobs' and the notifications' own controls, and the recovery dialog's answers.
    with_params(
        "job?:uint",
        hidden(act(
            "jobs.cancel",
            Command,
            "action-jobs-cancel",
            Menu::View,
            &[],
            always,
            None,
        )),
    ),
    with_params(
        "job?:uint",
        headless(hidden(act(
            "jobs.wait",
            View,
            "action-jobs-wait",
            Menu::View,
            &[],
            always,
            None,
        ))),
    ),
    with_params(
        "job?:uint",
        hidden(act(
            "jobs.open_result",
            View,
            "action-jobs-open-result",
            Menu::View,
            &[],
            always,
            None,
        )),
    ),
    with_params(
        "entry:uint, action:text",
        hidden(act(
            "notifications.run",
            Command,
            "action-notifications-run",
            Menu::View,
            &[],
            always,
            None,
        )),
    ),
    hidden(act(
        "journal.recover",
        Command,
        "action-journal-recover",
        Menu::File,
        &[],
        recovery_open,
        None,
    )),
    hidden(act(
        "journal.keep",
        View,
        "action-journal-keep",
        Menu::File,
        &[],
        recovery_open,
        None,
    )),
    // WP-S2.05 part 2: the find pane's own controls (§7.6); the pane handles Enter, Up and Down itself.
    hidden(in_find(act(
        "find.close",
        View,
        "action-find-close",
        Menu::Edit,
        &[],
        find_shown,
        None,
    ))),
    with_params(
        "text:text",
        hidden(in_find(act(
            "find.text",
            View,
            "action-find-text",
            Menu::Edit,
            &[],
            find_shown,
            None,
        ))),
    ),
    with_params(
        "text:text",
        hidden(in_find(act(
            "find.replacement",
            View,
            "action-find-replacement",
            Menu::Edit,
            &[],
            find_shown,
            None,
        ))),
    ),
    with_params(
        "option:enum(regex|match_case|whole_word|in_selection), on?:bool",
        hidden(in_find(act(
            "find.option",
            View,
            "action-find-option",
            Menu::Edit,
            &[],
            find_shown,
            None,
        ))),
    ),
    with_params(
        "fields:list(text)",
        hidden(in_find(act(
            "find.fields",
            View,
            "action-find-fields",
            Menu::Edit,
            &[],
            find_shown,
            None,
        ))),
    ),
    with_params(
        "folios:list(text)",
        hidden(in_find(act(
            "find.folios",
            View,
            "action-find-folios",
            Menu::Edit,
            &[],
            find_shown,
            None,
        ))),
    ),
    hidden(in_find(act(
        "find.run",
        View,
        "action-find-run",
        Menu::Edit,
        &[],
        find_shown,
        None,
    ))),
    with_params(
        "step?:int, to?:uint",
        hidden(in_find(act(
            "find.row",
            View,
            "action-find-row",
            Menu::Edit,
            &[],
            find_shown,
            None,
        ))),
    ),
    hidden(in_find(act(
        "find.jump",
        View,
        "action-find-jump",
        Menu::Edit,
        &[],
        find_shown,
        None,
    ))),
    // WP-S6.13 part 1: the page's content as text (FR-UI-046's headless v0).
    with_params(
        "folio?:text",
        headless(act(
            "page.describe",
            View,
            "action-page-describe",
            Menu::Page,
            &[],
            page_describable,
            None,
        )),
    ),
];

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
    let state = (meta.enabled)(model);
    // Paste names the page kind that cannot take it (window check, item 3): the reason is a text with the kind, not a
    // plain key, so the item formats it here.
    let paste_reason = (meta.id == "edit.paste_clip" && state.is_ok())
        .then(|| model.paste_page_reason())
        .flatten();
    let enabled = state.is_ok() && paste_reason.is_none();
    ActionItem {
        id: meta.id,
        params: Value::Object(serde_json::Map::new()),
        label: strings::text(meta.label_key).to_owned(),
        shortcut: shortcut_text(meta),
        enabled,
        reason: paste_reason.or_else(|| state.err().map(|k| strings::text(k).to_owned())),
        checked: match meta.id {
            "view.grid" => model.grid_on(),
            "view.frame_stats" => model.frame_stats_shown(),
            "view.cpu_canvas" => model.cpu_canvas_setting(),
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
            // WP-S2.09 part 3.
            "history.show" => model.history_shown(),
            "preview.show" => model.preview_shown(),
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
            "project.crossings" => model.crossings_hops(),
            // WP-S2.17 part 2: the Place tool lights Symbol in the bar; the command bar's collapse.
            "tool.place" => model.tool().name() == "place",
            "view.command_bar" => model.command_bar_collapsed(),
            _ => false,
        },
        topic: crate::help::action_topic(meta.id),
    }
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
                        item.label = strings::text(&format!("nav-mode-{}", mode.word())).to_owned();
                        item.params = json!({ "mode": mode.word() });
                        item.checked = nav.mode == mode;
                        items.push(item);
                    }
                } else if meta.id == "pages.sort" {
                    for sort in crate::navigator::NavSort::ALL {
                        let mut item = item_of(meta, model);
                        item.label = strings::text(&format!("nav-sort-{}", sort.word())).to_owned();
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
        Submenu::Pan => metas.iter().map(|m| item_of(m, model)).collect(),
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
                item.label = strings::text("menu-no-recent").to_owned();
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
            ["scale-100", "scale-125", "scale-150", "scale-200"]
                .iter()
                .zip(UI_SCALES)
                .map(|(key, scale)| {
                    let mut item = item_of(meta, model);
                    item.label = strings::text(key).to_owned();
                    item.params = json!({ "scale": scale });
                    item.shortcut = String::new();
                    item.checked = (current - scale).abs() < 1e-3;
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
                    item.label = strings::text(&format!("{prefix}-{value}")).to_owned();
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
                    item.label = strings::text(&format!("graphic-{}", shape.word())).to_owned();
                    item.params = json!({ "shape": shape.word() });
                    item.shortcut = String::new();
                    item.checked = active == Some(shape.word());
                    item
                })
                .collect()
        }
        Submenu::WireMarkers => {
            let current = model.marker_mode_word();
            ["per_wire", "all", "deviations", "explicit", "none"]
                .iter()
                .map(|mode| {
                    let mut item = item_of(meta, model);
                    item.label = strings::text(&format!("marker-mode-{mode}")).to_owned();
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
                            label: strings::text(&group.label_key()).to_owned(),
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
}

/// What a palette entry runs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PaletteRun {
    /// An action of the registry.
    Action(&'static str),
    /// A model command of `Session::registry()`; part 1 only lists it (disabled).
    Command(String),
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
            assert!(strings::lookup(a.label_key).is_some(), "label of {}", a.id);
        }
        for m in Menu::ALL {
            assert!(strings::lookup(m.label_key()).is_some());
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
        ] {
            assert!(strings::lookup(s.label_key()).is_some());
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

    /// The actions that read no parameter although they are hidden, headless-only or marked `needs_parameters`
    /// (WP-S6.13 BD2): a new such action either has a signature or is added here with a reason.
    const PARAMETERLESS: &[&str] = &[
        // Screen controls and keys that read nothing.
        "first_run.next",
        "first_run.back",
        "wizard.next",
        "wizard.back",
        "wizard.reset",
        "wizard.cancel",
        "ui.focus_next",
        "ui.focus_previous",
        "ui.dismiss",
        "ui.describe",
        // WP-S2.06 part 1: the reorder card's Apply and the page moves read the navigator's state.
        "navigator.apply",
        "pages.move_up",
        "pages.move_down",
        // The picker, the tools and the proposals.
        "tool.finish",
        "tool.next_proposal",
        "tool.connect_on_move",
        // `canvas.activate` with `alt` forced.
        "canvas.activate_alt",
        // The property grid, the card, the toast and the find pane.
        "props.commit",
        "props.cancel",
        "preview.apply",
        "preview.cancel",
        "toast.undo",
        "toast.dismiss",
        "find.close",
        "find.run",
        "find.jump",
        // WP-S2.09 part 1: the recovery dialog's answers.
        "journal.recover",
        "journal.keep",
    ];

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
    fn every_action_that_reads_parameters_has_a_signature() {
        for a in ACTIONS
            .iter()
            .filter(|a| a.needs_parameters || a.headless_only || a.hidden)
        {
            assert!(
                !a.params.is_empty() || PARAMETERLESS.contains(&a.id),
                "{} has no signature and is not in PARAMETERLESS",
                a.id
            );
        }
        for id in PARAMETERLESS {
            let a = find(id).unwrap_or_else(|| panic!("{id} is not an action"));
            assert!(a.params.is_empty(), "{id} is listed as parameterless");
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
            "letter?:text, strip?:text, designation?:text"
        );
        let new = parse_signature(sig("file.new")).unwrap();
        let template = new.iter().find(|p| p.name == "template").unwrap();
        assert_eq!(template.ty, "path");
        // Every action that takes a path types it `path`, so the harness can check it.
        for (id, name) in [
            ("file.open", "dir"),
            ("file.save_as", "dir"),
            ("file.export_pdf", "path"),
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
