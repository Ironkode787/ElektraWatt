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
    /// Page.
    Page,
    /// Tools.
    Tools,
    /// Help.
    Help,
}

impl Menu {
    /// The menus in menu-bar order.
    pub const ALL: [Menu; 6] = [
        Menu::File,
        Menu::Edit,
        Menu::View,
        Menu::Page,
        Menu::Tools,
        Menu::Help,
    ];

    /// The key of the menu's label.
    pub fn label_key(self) -> &'static str {
        match self {
            Menu::File => "menu-file",
            Menu::Edit => "menu-edit",
            Menu::View => "menu-view",
            Menu::Page => "menu-page",
            Menu::Tools => "menu-tools",
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
}

impl Submenu {
    /// The key of the submenu's label.
    pub fn label_key(self) -> &'static str {
        match self {
            Submenu::OpenRecent => "submenu-open-recent",
            Submenu::Export => "submenu-export",
            Submenu::UiScale => "submenu-ui-scale",
        }
    }
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
    if m.page_count() > 0 {
        Ok(())
    } else {
        Err("reason-no-page")
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
    }
}

const fn needs_parameters(mut a: ActionMeta) -> ActionMeta {
    a.needs_parameters = true;
    a
}

const fn headless(mut a: ActionMeta) -> ActionMeta {
    a.headless_only = true;
    a
}

use ActionKind::{Command, View};

/// The registry: the part-1 actions of the spec, section 5.2, in menu order, then the headless-only ones.
pub static ACTIONS: &[ActionMeta] = &[
    act(
        "file.open",
        Command,
        "action-file-open",
        Menu::File,
        &["Ctrl+O"],
        always,
        slot(Menu::File),
    ),
    needs_parameters(act(
        "file.open_recent",
        Command,
        "action-file-open-recent",
        Menu::File,
        &[],
        has_recent,
        sub(Menu::File, Submenu::OpenRecent, false),
    )),
    act(
        "file.save",
        Command,
        "action-file-save",
        Menu::File,
        &["Ctrl+S"],
        writable,
        slot_sep(Menu::File),
    ),
    act(
        "file.save_as",
        Command,
        "action-file-save-as",
        Menu::File,
        &["Ctrl+Shift+S"],
        project_open,
        slot(Menu::File),
    ),
    act(
        "file.export_pdf",
        Command,
        "action-file-export-pdf",
        Menu::File,
        &["Ctrl+Shift+E"],
        has_pages,
        sub(Menu::File, Submenu::Export, true),
    ),
    act(
        "file.close",
        Command,
        "action-file-close",
        Menu::File,
        &["Ctrl+W"],
        project_open,
        slot_sep(Menu::File),
    ),
    act(
        "app.quit",
        Command,
        "action-app-quit",
        Menu::File,
        &["Ctrl+Q"],
        always,
        slot(Menu::File),
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
    act(
        "page.next",
        View,
        "action-page-next",
        Menu::Page,
        &["Page Down"],
        has_next,
        slot(Menu::Page),
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
    act(
        "page.goto",
        View,
        "action-page-goto",
        Menu::Page,
        &["Ctrl+G"],
        has_pages,
        slot_sep(Menu::Page),
    ),
    act(
        "view.fit_page",
        View,
        "action-view-fit-page",
        Menu::View,
        &["Home"],
        page_shown,
        slot(Menu::View),
    ),
    needs_parameters(act(
        "view.ui_scale",
        View,
        "action-view-ui-scale",
        Menu::View,
        &[],
        always,
        sub(Menu::View, Submenu::UiScale, true),
    )),
    act(
        "tools.palette",
        View,
        "action-tools-palette",
        Menu::Tools,
        &["Ctrl+K", "Ctrl+Shift+P"],
        always,
        slot(Menu::Tools),
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
        "help.about",
        View,
        "action-help-about",
        Menu::Help,
        &[],
        always,
        slot(Menu::Help),
    ),
    needs_parameters(act(
        "command.run",
        Command,
        "action-command-run",
        Menu::Tools,
        &[],
        writable,
        None,
    )),
    headless(act(
        "view.snapshot",
        View,
        "action-view-snapshot",
        Menu::View,
        &[],
        page_shown,
        None,
    )),
    headless(act(
        "ui.describe",
        View,
        "action-ui-describe",
        Menu::Tools,
        &[],
        always,
        None,
    )),
    act(
        "ui.dismiss",
        View,
        "action-ui-dismiss",
        Menu::Tools,
        &["Escape"],
        always,
        None,
    ),
];

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
    /// Whether the item shows a check mark (the current UI scale).
    pub checked: bool,
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
        checked: false,
    }
}

fn submenu_items(submenu: Submenu, meta: &ActionMeta, model: &ShellModel) -> Vec<ActionItem> {
    match submenu {
        Submenu::Export => vec![item_of(meta, model)],
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
                if s.separator_before && !entries.is_empty() {
                    entries.push(MenuEntry::Separator);
                }
                match s.submenu {
                    None => entries.push(MenuEntry::Item(item_of(meta, model))),
                    Some(submenu) => {
                        if done_submenus.contains(&submenu) {
                            continue;
                        }
                        done_submenus.push(submenu);
                        entries.push(MenuEntry::Submenu {
                            submenu,
                            label: strings::text(submenu.label_key()).to_owned(),
                            items: submenu_items(submenu, meta, model),
                        });
                    }
                }
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
        for s in [Submenu::OpenRecent, Submenu::Export, Submenu::UiScale] {
            assert!(strings::lookup(s.label_key()).is_some());
        }
    }

    #[test]
    fn shortcuts_are_unique_and_never_ctrl_alt_letter() {
        let mut seen: Vec<&str> = Vec::new();
        for a in ACTIONS {
            for s in a.shortcut {
                assert!(!seen.contains(s), "shortcut {s} is bound twice");
                seen.push(s);
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
    fn headless_only_actions_have_no_menu_slot() {
        for a in ACTIONS.iter().filter(|a| a.headless_only) {
            assert!(a.menu.is_none(), "{}", a.id);
        }
    }
}
