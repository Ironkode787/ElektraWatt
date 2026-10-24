// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The action registry (WP-S2.01 part 1, spec section 5.2; UX/UI doc 16 section 2.4) in toolkit-neutral form.
//!
//! Every menu item, palette entry, keyboard shortcut and dialog result of the shell is an action of
//! [`ACTIONS`], dispatched with [`ShellModel::dispatch`]; the headless mode drives the same registry. An action
//! has a stable ID, a label key (the English text lives in [`crate::strings`]), default shortcuts, and an
//! `enabled` predicate that names the reason key of a refusal. [`menu_model`] builds the menu bar from the
//! registry, so the egui frame and the `ui.describe` text cannot disagree.

use serde_json::{Value, json};

use crate::settings::UI_SCALES;
use crate::shell::ShellModel;
use crate::strings;

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

/// The menus of the menu bar, in order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Menu {
    /// File.
    File,
    /// Edit.
    Edit,
    /// View.
    View,
    /// Insert: the symbols, parts and strips to place (WP-S2.04).
    Insert,
    /// Page.
    Page,
    /// Tools.
    Tools,
    /// Window: the canvas tabs (WP-S2.03).
    Window,
    /// Help.
    Help,
}

impl Menu {
    /// The menus in menu-bar order.
    pub const ALL: [Menu; 8] = [
        Menu::File,
        Menu::Edit,
        Menu::View,
        Menu::Insert,
        Menu::Page,
        Menu::Tools,
        Menu::Window,
        Menu::Help,
    ];

    /// The key of the menu's label.
    pub fn label_key(self) -> &'static str {
        match self {
            Menu::File => "menu-file",
            Menu::Edit => "menu-edit",
            Menu::View => "menu-view",
            Menu::Insert => "menu-insert",
            Menu::Page => "menu-page",
            Menu::Tools => "menu-tools",
            Menu::Window => "menu-window",
            Menu::Help => "menu-help",
        }
    }
}

/// The submenus of the menu bar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Submenu {
    /// File, Open recent: one item per recent project.
    OpenRecent,
    /// File, Export.
    Export,
    /// View, UI scale: one item per scale.
    UiScale,
    /// View, Workspace: one item per workspace, then save and reset.
    Workspace,
    /// View, Pan: left, right, up, down.
    Pan,
    /// View, Page navigator: its three views and two sort orders (WP-S2.03).
    PageNavigator,
}

impl Submenu {
    /// The key of the submenu's label.
    pub fn label_key(self) -> &'static str {
        match self {
            Submenu::OpenRecent => "submenu-open-recent",
            Submenu::Export => "submenu-export",
            Submenu::UiScale => "submenu-ui-scale",
            Submenu::Workspace => "submenu-workspace",
            Submenu::Pan => "submenu-pan",
            Submenu::PageNavigator => "submenu-page-navigator",
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

/// Where an action sits in the menu bar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MenuSlot {
    /// The menu.
    pub menu: Menu,
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
fn editable_page(m: &ShellModel) -> Result<(), &'static str> {
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
        submenu: None,
        separator_before: false,
    })
}

const fn slot_sep(menu: Menu) -> Option<MenuSlot> {
    Some(MenuSlot {
        menu,
        submenu: None,
        separator_before: true,
    })
}

const fn sub(menu: Menu, submenu: Submenu, separator_before: bool) -> Option<MenuSlot> {
    Some(MenuSlot {
        menu,
        submenu: Some(submenu),
        separator_before,
    })
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

/// The card's buttons: the card sits in the property grid or the find pane, so one of them is shown.
fn card_shown(m: &ShellModel) -> Result<(), &'static str> {
    project_open(m)?;
    if m.properties_shown() || m.find_shown() {
        Ok(())
    } else {
        Err("reason-no-sheet")
    }
}

/// Apply on the card: a writable project with the property grid or the find pane shown.
fn card_writable(m: &ShellModel) -> Result<(), &'static str> {
    writable(m)?;
    card_shown(m)
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
        act(
            "file.import_qet",
            Command,
            "action-file-import-qet",
            Menu::File,
            &[],
            always,
            slot(Menu::File),
        ),
    ),
    act(
        "file.save",
        Command,
        "action-file-save",
        Menu::File,
        &["Ctrl+S"],
        writable,
        slot_sep(Menu::File),
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
    with_params(
        "path?:path",
        act(
            "file.export_pdf",
            Command,
            "action-file-export-pdf",
            Menu::File,
            &["Ctrl+Shift+E"],
            has_pages,
            sub(Menu::File, Submenu::Export, true),
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
    act(
        "edit.undo",
        Command,
        "action-edit-undo",
        Menu::Edit,
        &["Ctrl+Z"],
        can_undo,
        slot(Menu::Edit),
    ),
    act(
        "edit.redo",
        Command,
        "action-edit-redo",
        Menu::Edit,
        &["Ctrl+Shift+Z", "Ctrl+Y"],
        can_redo,
        slot(Menu::Edit),
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
    act(
        "props.project",
        View,
        "action-props-project",
        Menu::Edit,
        &[],
        project_open,
        slot(Menu::Edit),
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
        act(
            "page.new",
            Command,
            "action-page-new",
            Menu::Page,
            &[],
            writable,
            slot(Menu::Page),
        ),
    ),
    act(
        "page.next",
        View,
        "action-page-next",
        Menu::Page,
        &["Page Down"],
        has_next,
        slot_sep(Menu::Page),
    ),
    act(
        "page.previous",
        View,
        "action-page-previous",
        Menu::Page,
        &["Page Up"],
        has_previous,
        slot(Menu::Page),
    ),
    act(
        "page.first",
        View,
        "action-page-first",
        Menu::Page,
        &[],
        has_pages,
        slot(Menu::Page),
    ),
    act(
        "page.last",
        View,
        "action-page-last",
        Menu::Page,
        &[],
        has_pages,
        slot(Menu::Page),
    ),
    with_params(
        "text?:text",
        act(
            "nav.goto",
            View,
            "action-nav-goto",
            Menu::Page,
            &["Ctrl+G"],
            has_pages,
            slot_sep(Menu::Page),
        ),
    ),
    act(
        "nav.back",
        View,
        "action-nav-back",
        Menu::Page,
        &["Alt+Arrow Left"],
        can_go_back,
        slot(Menu::Page),
    ),
    act(
        "nav.forward",
        View,
        "action-nav-forward",
        Menu::Page,
        &["Alt+Arrow Right"],
        can_go_forward,
        slot(Menu::Page),
    ),
    with_params(
        "x_mm?:mm, y_mm?:mm, group?:uint",
        in_canvas(act(
            "nav.follow",
            View,
            "action-nav-follow",
            Menu::Page,
            &["F"],
            has_pages,
            slot_sep(Menu::Page),
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
            slot(Menu::Page),
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
            slot(Menu::Page),
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
            slot_sep(Menu::Page),
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
            slot(Menu::Page),
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
            slot(Menu::Page),
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
        act(
            "view.zoom_in",
            View,
            "action-view-zoom-in",
            Menu::View,
            &["Ctrl+Plus", "Ctrl+Equals"],
            can_zoom_in,
            slot(Menu::View),
        ),
    ),
    with_params(
        "group?:uint",
        act(
            "view.zoom_out",
            View,
            "action-view-zoom-out",
            Menu::View,
            &["Ctrl+Minus"],
            can_zoom_out,
            slot(Menu::View),
        ),
    ),
    with_params(
        "group?:uint",
        act(
            "view.zoom_actual",
            View,
            "action-view-zoom-actual",
            Menu::View,
            &["Ctrl+0"],
            has_pages,
            slot(Menu::View),
        ),
    ),
    with_params(
        "group?:uint",
        act(
            "view.fit_page",
            View,
            "action-view-fit-page",
            Menu::View,
            &["Home"],
            has_pages,
            slot(Menu::View),
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
            sub(Menu::View, Submenu::Pan, false),
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
            sub(Menu::View, Submenu::Pan, false),
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
            sub(Menu::View, Submenu::Pan, false),
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
            sub(Menu::View, Submenu::Pan, false),
        ),
    ),
    act(
        "view.side_by_side",
        View,
        "action-view-side-by-side",
        Menu::View,
        &[],
        has_pages,
        slot_sep(Menu::View),
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
            sub(Menu::View, Submenu::PageNavigator, false),
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
            sub(Menu::View, Submenu::PageNavigator, false),
        )),
    ),
    act(
        "view.grid",
        View,
        "action-view-grid",
        Menu::View,
        &[],
        has_pages,
        slot_sep(Menu::View),
    ),
    act(
        "view.frame_stats",
        View,
        "action-view-frame-stats",
        Menu::View,
        &[],
        always,
        slot(Menu::View),
    ),
    act(
        "view.cpu_canvas",
        View,
        "action-view-cpu-canvas",
        Menu::View,
        &[],
        always,
        slot(Menu::View),
    ),
    // The property grid (WP-S2.05 part 1, spec §6.10).
    with_params(
        "on?:bool",
        act(
            "props.show",
            View,
            "action-props-show",
            Menu::View,
            &[],
            project_open,
            slot_sep(Menu::View),
        ),
    ),
    with_params(
        "scale:json",
        needs_parameters(act(
            "view.ui_scale",
            View,
            "action-view-ui-scale",
            Menu::View,
            &[],
            always,
            sub(Menu::View, Submenu::UiScale, true),
        )),
    ),
    with_params(
        "name:text",
        needs_parameters(act(
            "view.workspace_load",
            View,
            "action-view-workspace-load",
            Menu::View,
            &[],
            always,
            sub(Menu::View, Submenu::Workspace, false),
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
            sub(Menu::View, Submenu::Workspace, false),
        ),
    ),
    act(
        "view.workspace_reset",
        View,
        "action-view-workspace-reset",
        Menu::View,
        &[],
        always,
        sub(Menu::View, Submenu::Workspace, false),
    ),
    with_params(
        "query?:text",
        act(
            "tools.palette",
            View,
            "action-tools-palette",
            Menu::Tools,
            &["Ctrl+K", "Ctrl+Shift+P"],
            always,
            slot(Menu::Tools),
        ),
    ),
    act(
        "tool.select",
        View,
        "action-tool-select",
        Menu::Tools,
        &[],
        has_pages,
        slot_sep(Menu::Tools),
    ),
    act(
        "tool.measure",
        View,
        "action-tool-measure",
        Menu::Tools,
        &[],
        has_pages,
        slot(Menu::Tools),
    ),
    // Place and arrange (WP-S2.04 part 1, §7.6): the Insert menu, the transform keys, Move, Delete, Change symbol.
    in_canvas(act(
        "tool.place",
        Command,
        "action-tool-place",
        Menu::Insert,
        &["I"],
        editable_page,
        slot(Menu::Insert),
    )),
    in_canvas(act(
        "tool.place_part",
        Command,
        "action-tool-place-part",
        Menu::Insert,
        &["Shift+I"],
        editable_page,
        slot(Menu::Insert),
    )),
    with_params(
        "letter?:text",
        act(
            "strip.new",
            Command,
            "action-strip-new",
            Menu::Insert,
            &[],
            editable_page,
            slot_sep(Menu::Insert),
        ),
    ),
    // FT-409: the structure node and the external reference, one previewed command each.
    with_params(
        "kind:enum(functional_assignment|function|installation_site|location|document_kind|user_defined|type), designation:text, parent?:text",
        act(
            "structure.new",
            Command,
            "action-structure-new",
            Menu::Insert,
            &[],
            writable,
            slot_sep(Menu::Insert),
        ),
    ),
    with_params(
        "location:text, device:text, connection?:text, project?:text",
        act(
            "external_ref.new",
            Command,
            "action-external-ref-new",
            Menu::Insert,
            &[],
            writable,
            slot(Menu::Insert),
        ),
    ),
    with_params(
        "symbol?:text, part?:text, function?:ref, template?:uint, letter?:text, strip?:text, designation?:text",
        hidden(in_canvas(act(
            "place.pick",
            Command,
            "action-place-pick",
            Menu::Insert,
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
            Menu::Insert,
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
            Menu::Insert,
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
            Menu::Insert,
            &[],
            picker_open,
            None,
        ),
    )),
    in_canvas(act(
        "tool.rotate_cw",
        Command,
        "action-tool-rotate-cw",
        Menu::Edit,
        &["R"],
        editable_page,
        slot_sep(Menu::Edit),
    )),
    in_canvas(act(
        "tool.rotate_ccw",
        Command,
        "action-tool-rotate-ccw",
        Menu::Edit,
        &["Shift+R"],
        editable_page,
        slot(Menu::Edit),
    )),
    in_canvas(act(
        "tool.mirror_x",
        Command,
        "action-tool-mirror-x",
        Menu::Edit,
        &["X"],
        editable_page,
        slot(Menu::Edit),
    )),
    in_canvas(act(
        "tool.mirror_y",
        Command,
        "action-tool-mirror-y",
        Menu::Edit,
        &["Y"],
        editable_page,
        slot(Menu::Edit),
    )),
    in_canvas(act(
        "tool.next_variant",
        Command,
        "action-tool-next-variant",
        Menu::Edit,
        &["V"],
        editable_page,
        slot(Menu::Edit),
    )),
    in_canvas(act(
        "tool.next_rep_type",
        View,
        "action-tool-next-rep-type",
        Menu::Edit,
        &["Shift+V"],
        editable_page,
        slot(Menu::Edit),
    )),
    in_canvas(act(
        "tool.move",
        Command,
        "action-tool-move",
        Menu::Edit,
        &["M"],
        editable_page,
        slot(Menu::Edit),
    )),
    in_canvas(act(
        "edit.delete_selection",
        Command,
        "action-edit-delete-selection",
        Menu::Edit,
        &["Delete"],
        editable_page,
        slot(Menu::Edit),
    )),
    with_params(
        "symbol?:text",
        in_canvas(act(
            "rep.exchange",
            Command,
            "action-rep-exchange",
            Menu::Edit,
            &[],
            editable_page,
            slot(Menu::Edit),
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
    in_canvas(act(
        "tool.wire",
        Command,
        "action-tool-wire",
        Menu::Insert,
        &["W"],
        wire_page,
        slot_sep(Menu::Insert),
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
    in_canvas(act(
        "tool.route_edit",
        Command,
        "action-tool-route-edit",
        Menu::Edit,
        &[],
        one_connection,
        slot_sep(Menu::Edit),
    )),
    in_canvas(act(
        "junction.open",
        View,
        "action-junction-open",
        Menu::Edit,
        &[],
        one_junction,
        slot(Menu::Edit),
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
            Menu::Tools,
            &[],
            writable,
            slot_sep(Menu::Tools),
        ),
    ),
    // WP-S2.04 part 2. End.
    act(
        "tools.company_standard",
        Command,
        "action-tools-company-standard",
        Menu::Tools,
        &[],
        always,
        slot(Menu::Tools),
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
            Menu::Tools,
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
        Menu::Tools,
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
    with_params(
        "area:enum(navigator|canvas|properties|find), group?:uint",
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
        Menu::Tools,
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
            Menu::Tools,
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
            Menu::Tools,
            &[],
            wizard_open,
            None,
        )),
    ),
    hidden(act(
        "wizard.next",
        View,
        "action-wizard-next",
        Menu::Tools,
        &[],
        wizard_open,
        None,
    )),
    hidden(act(
        "wizard.back",
        View,
        "action-wizard-back",
        Menu::Tools,
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
            Menu::Tools,
            &[],
            wizard_open,
            None,
        )),
    ),
    hidden(act(
        "wizard.reset",
        View,
        "action-wizard-reset",
        Menu::Tools,
        &[],
        wizard_open,
        None,
    )),
    hidden(act(
        "wizard.cancel",
        View,
        "action-wizard-cancel",
        Menu::Tools,
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
            Menu::Tools,
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
            Menu::Tools,
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
            Menu::Tools,
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

/// An entry of a menu: an item or a submenu.
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

fn item_of(meta: &ActionMeta, model: &ShellModel) -> ActionItem {
    let state = (meta.enabled)(model);
    ActionItem {
        id: meta.id,
        params: Value::Object(serde_json::Map::new()),
        label: strings::text(meta.label_key).to_owned(),
        shortcut: shortcut_text(meta),
        enabled: state.is_ok(),
        reason: state.err().map(|k| strings::text(k).to_owned()),
        checked: match meta.id {
            "view.grid" => model.grid_on(),
            "view.frame_stats" => model.frame_stats_shown(),
            "view.cpu_canvas" => model.cpu_canvas_setting(),
            "view.side_by_side" => model.area().is_some_and(crate::area::CanvasArea::side_by_side),
            "props.show" => model
                .layout()
                .panes()
                .contains(&crate::layout::PaneKind::Properties),
            "tool.select" | "tool.measure" => meta.id.strip_prefix("tool.") == Some(model.tool().name()),
            // WP-S2.04 part 2.
            "tool.wire" => model.tool().name() == "wire",
            "project.crossings" => model.crossings_hops(),
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
        Submenu::Export => vec![item_of(meta, model)],
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

/// The menu bar generated from the registry, in the state of `model`: File, Edit, View, Page, Tools, Help.
pub fn menu_model(model: &ShellModel) -> Vec<MenuModel> {
    Menu::ALL
        .iter()
        .map(|&menu| {
            let mut entries = Vec::new();
            let mut done_submenus: Vec<Submenu> = Vec::new();
            for meta in ACTIONS {
                let Some(s) = meta.menu.filter(|s| s.menu == menu) else {
                    continue;
                };
                if let Some(submenu) = s.submenu
                    && done_submenus.contains(&submenu)
                {
                    continue;
                }
                if s.separator_before && !entries.is_empty() {
                    entries.push(MenuEntry::Separator);
                }
                match s.submenu {
                    None => entries.push(MenuEntry::Item(item_of(meta, model))),
                    Some(submenu) => {
                        done_submenus.push(submenu);
                        let metas: Vec<&ActionMeta> = ACTIONS
                            .iter()
                            .filter(|a| {
                                a.menu
                                    .is_some_and(|m| m.menu == menu && m.submenu == Some(submenu))
                            })
                            .collect();
                        entries.push(MenuEntry::Submenu {
                            submenu,
                            label: strings::text(submenu.label_key()).to_owned(),
                            items: submenu_items(submenu, &metas, model),
                        });
                    }
                }
            }
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
            Submenu::Export,
            Submenu::UiScale,
            Submenu::Workspace,
            Submenu::Pan,
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
            for entry in &menu.entries {
                let items: Vec<&ActionItem> = match entry {
                    MenuEntry::Separator => Vec::new(),
                    MenuEntry::Item(i) => vec![i],
                    MenuEntry::Submenu { items, .. } => items.iter().collect(),
                };
                for item in items {
                    assert!(find(item.id).is_some(), "{} is not in the registry", item.id);
                }
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
            m.entries
                .iter()
                .flat_map(|e| match e {
                    MenuEntry::Separator => Vec::new(),
                    MenuEntry::Item(i) => vec![i.id],
                    MenuEntry::Submenu { items, .. } => items.iter().map(|i| i.id).collect(),
                })
                .collect()
        };
        let file = ids(Menu::File);
        for id in [
            "file.new",
            "file.open_sample",
            "file.import_qet",
            "file.discard_journal",
        ] {
            assert!(file.contains(&id), "{id} in File");
        }
        assert!(ids(Menu::Tools).contains(&"tools.company_standard"));
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
