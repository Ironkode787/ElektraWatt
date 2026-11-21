// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The menu model built from the action table (doc 16a §9.2): [`menu_model`], the app menu's submenus with their items
//! ([`ActionItem`], [`MenuEntry`]), each item enabled or greyed with its reason; the submenus whose items the
//! project gives (recent projects, workspaces, the station view's module commands); the shortcut text a menu shows.
//! The egui frame and the `ui.describe` text both read it, so they cannot disagree.
//!
//! Not here: the table itself is `actions/table/`; the enable checks in `actions/enabled.rs`; the command palette's
//! rows in `actions.rs`.

use serde_json::{Value, json};

use super::enabled::on_valve_position;
use super::{ACTIONS, ActionMeta, Menu, Submenu, find, guides, tutorials};
use crate::command_bar::Group;
use crate::key;
use crate::settings::{DENSITIES, MIN_FONT_CHOICES, THEMES, UI_SCALES, scale_label};
use crate::shell::ShellModel;
use crate::strings;
use crate::strings::Key;
use crate::tools::{Ghost, GraphicShape};

/// The commands on the module under a station view's cursor, in registry order (the view's own actions
/// `plc.module_*`): each
/// with its short label for the view's own bar (`bar-plc-module-…`, else its registry label) and as a menu entry.
/// A command that takes a part (Replace) is a submenu of the parts that fit, one that takes a folio (Draw on a page)
/// of the schematic pages. The view's bar and a right click on a card show them.
pub fn station_tools(model: &ShellModel) -> Vec<(String, MenuEntry)> {
    // On a valve terminal's position the entries speak of the valve (`valve-plc-module-…`).
    let on_valve = on_valve_position(model);
    ACTIONS
        .iter()
        // The view's own controls; a module's settings (Safety data…) are its row's menu.
        .filter(|a| a.hidden && a.id.starts_with("plc.module_"))
        .map(|meta| {
            let mut item = item_of(meta, model);
            let words = meta.id.replace(['.', '_'], "-");
            let short =
                strings::lookup(&format!("bar-{words}")).map_or_else(|| item.label.clone(), str::to_owned);
            if let Some(valve) = strings::lookup(&format!("valve-{words}")).filter(|_| on_valve) {
                valve.clone_into(&mut item.label);
            }
            let choices = match meta.params.split(':').next() {
                Some("part") => Some(Submenu::ModuleParts),
                Some("folio") => Some(Submenu::ModulePages),
                _ => None,
            };
            // A list of choices opens beside its entry, which says so by its arrow, not by an ellipsis; one that cannot
            // run is a greyed entry with its reason, not a list of greyed choices.
            if choices.is_some() {
                item.label = item.label.trim_end_matches('…').to_owned();
            }
            let entry = match choices {
                Some(submenu) if item.enabled => MenuEntry::Submenu {
                    submenu,
                    label: item.label.clone(),
                    items: submenu_items(submenu, &[meta], model),
                },
                _ => MenuEntry::Item(item),
            };
            (short, entry)
        })
        .collect()
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
    /// The documentation topic of the action ([`crate::help`]): its tooltip ends with "F1: help".
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
    let state = meta.enabled_now(model);
    // Paste names the page kind that cannot take it (window check, item 3): the reason is a text with the kind, not a
    // plain key, so the item formats it here.
    let paste_reason = match meta.id {
        _ if state.is_err() => None,
        "edit.paste_clip" => model.paste_page_reason(),
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
        "project.wire_numbers" => model.wire_numbers_on(),
        "project.wire_numbering" => model.wire_numbering_per_potential(),
        "project.plc_addresses_at_terminals" => model.plc_addresses_at_terminals_on(),
        "rep.keep_in_place" => model.selection_kept_in_place(),
        // The command bar's collapse.
        "view.command_bar" => model.command_bar_collapsed(),
        "props.show_advanced" => model.properties_show_advanced(),
        // The PLC and Fluid tabs' workspaces and panels.
        "plc.workspace" => model.workspace() == "PLC",
        "fluid.workspace" => model.workspace() == "Fluid",
        "plc.grid" => model.navigator_open(crate::navigators::NavKind::PlcGrid),
        "bus.panel" => model.navigator_open(crate::navigators::NavKind::Bus),
        "fluid.tubes" => model.navigator_open(crate::navigators::NavKind::Tubes),
        "fluid.media" => model.navigator_open(crate::navigators::NavKind::Media),
        _ => return None,
    })
}

/// A workspace's label: a built-in workspace in the UI language's words, a saved one by its name.
pub fn workspace_label(name: &str) -> String {
    crate::settings::BUILT_IN_WORKSPACES
        .contains(&name)
        .then(|| strings::lookup(&format!("workspace-{}", name.to_lowercase())))
        .flatten()
        .map_or_else(|| name.to_owned(), str::to_owned)
}

/// The greyed first line of a submenu that says what its entries are ("How to do one job"): no action runs from it.
fn heading_item(meta: &ActionMeta, model: &ShellModel, what: Key<'static>) -> ActionItem {
    heading(meta, model, strings::text(what).to_owned())
}

/// A greyed line `label` of a menu that only says something (a heading): no action runs from it, it has no key.
pub(crate) fn heading(meta: &ActionMeta, model: &ShellModel, label: String) -> ActionItem {
    let mut item = item_of(meta, model);
    item.label = label;
    item.params = Value::Null;
    item.shortcut = String::new();
    item.enabled = false;
    item.reason = None;
    item
}

/// The one greyed item of a submenu that has nothing to offer now, its label and reason the text of `why` (a
/// command-bar menu button is greyed with it).
fn nothing_item(meta: &ActionMeta, model: &ShellModel, why: Key<'static>) -> Vec<ActionItem> {
    let mut item = item_of(meta, model);
    let text = strings::text(why).to_owned();
    item.label.clone_from(&text);
    if item.enabled {
        item.enabled = false;
        item.reason = Some(text);
    }
    vec![item]
}

/// One item of `meta` per target of the PLC and Fluid tabs' menu buttons: `targets` as (label, parameters), else
/// [`nothing_item`] with `why`.
fn target_items(
    meta: &ActionMeta,
    model: &ShellModel,
    targets: Vec<(String, Value)>,
    why: Key<'static>,
) -> Vec<ActionItem> {
    if targets.is_empty() {
        return nothing_item(meta, model, why);
    }
    targets
        .into_iter()
        .map(|(label, params)| {
            let mut item = item_of(meta, model);
            item.label = label;
            item.params = params;
            item
        })
        .collect()
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
                        item.label = workspace_label(&name);
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
        // One item per tutorial, each with its own start, under a line that says what a tutorial is.
        Submenu::Tutorials => std::iter::once(heading_item(meta, model, key!("submenu-tutorials-what")))
            .chain(tutorials().map(|(name, title)| {
                let mut item = item_of(meta, model);
                item.label = strings::lookup(&format!("tutorial-{name}"))
                    .unwrap_or(title)
                    .to_owned();
                item.params = json!({ "tutorial": name });
                item
            }))
            .collect(),
        // One item per how-to guide, opening its page, under a line that says what a guide is.
        Submenu::Guides => std::iter::once(heading_item(meta, model, key!("submenu-guides-what")))
            .chain(guides().map(|(name, title)| {
                let mut item = item_of(meta, model);
                item.label = strings::lookup(&format!("guide-{name}"))
                    .unwrap_or(title)
                    .to_owned();
                item.params = json!({ "guide": name });
                item
            }))
            .collect(),
        // The stations by tag: Rack and slots… of each; Attach cards to each that takes cards, while cards wait.
        Submenu::RackEditor | Submenu::AttachCards => {
            let session = model.project_session();
            let stations = if submenu == Submenu::RackEditor {
                model.plc_stations()
            } else {
                session
                    .map(ew_api::Session::plc_stations_for_cards)
                    .unwrap_or_default()
            };
            let waiting = session.is_some_and(|s| !s.plc_cards_without_cpu().is_empty());
            let mut items = target_items(
                meta,
                model,
                stations
                    .into_iter()
                    .map(|(id, tag)| (tag, json!({ "station": id.to_string() })))
                    .collect(),
                key!("reason-no-station"),
            );
            if submenu == Submenu::AttachCards && !waiting {
                for item in items.iter_mut().filter(|i| i.enabled) {
                    item.enabled = false;
                    item.reason = Some(strings::text(key!("reason-no-cards-without-cpu")).to_owned());
                }
            }
            items
        }
        // The parts that fit where the station view's cursor stands, each by number, description and what it gives; the
        // part it has already is checked.
        Submenu::ModuleParts => {
            let current = model
                .station_device()
                .and_then(|d| model.device_part(d))
                .map(|c| json!(c.to_string()));
            // The view's catalogue as made for its state, read in place: the bar asks every frame.
            let shown = model.station_shown();
            let mut items = target_items(
                meta,
                model,
                shown
                    .iter()
                    .flat_map(|s| &s.catalogue)
                    .filter(|p| p.unfit.is_none())
                    .map(|p| (p.words(), json!({ "part": p.copy.to_string() })))
                    .collect(),
                key!("rack-no-part-fits"),
            );
            for item in &mut items {
                item.checked = current.is_some() && item.params.get("part") == current.as_ref();
            }
            items
        }
        // The schematic pages by folio and title, then New page….
        Submenu::ModulePages => target_items(
            meta,
            model,
            model
                .schematic_pages()
                .into_iter()
                .map(|(folio, title)| {
                    let label = strings::format(key!("rack-next-draw-on"), &[&folio, &title]);
                    (label.trim().to_owned(), json!({ "folio": folio }))
                })
                .chain([(
                    strings::text(key!("rack-draw-on-new-page")).to_owned(),
                    json!({ "folio": "new" }),
                )])
                .collect(),
            key!("reason-rack-draw-page"),
        ),
        Submenu::PlcAddressing => {
            let items = model.plc_addressing_items();
            if items.is_empty() {
                nothing_item(meta, model, key!("reason-no-config-project"))
            } else {
                items
            }
        }
        // The configuration projects, by name.
        Submenu::SymbolPolicy | Submenu::InstrumentList => {
            let configs = model
                .project_session()
                .map(ew_api::Session::plc_config_projects)
                .unwrap_or_default();
            target_items(
                meta,
                model,
                configs
                    .into_iter()
                    .map(|(id, name)| (name, json!({ "config": id.to_string() })))
                    .collect(),
                key!("reason-no-config-project"),
            )
        }
        // Put a station on this page: on a bus-topology page the stations and bus devices it does not draw yet, each put
        // there whole, else the entry greyed with its reason; then the ports the Bus panel lists as not drawn yet.
        Submenu::BusPorts => {
            let mut items = Vec::new();
            if let Some(put) = metas.iter().find(|m| m.id == "bus.put_station") {
                let stations = model.stations_off_page(None);
                if stations.is_empty() {
                    items.push(item_of(put, model));
                } else {
                    items.push(heading_item(put, model, key!("submenu-bus-stations")));
                    items.extend(stations.into_iter().map(|(id, tag)| {
                        let mut item = item_of(put, model);
                        item.label = tag;
                        item.params = json!({ "station": id.to_string() });
                        item
                    }));
                    items.push(heading_item(meta, model, key!("submenu-bus-one-port")));
                }
            }
            let bus_page =
                model.page_rows().get(model.page_index()).map(|r| r.kind.as_str()) == Some("bus_topology");
            if !bus_page {
                items.extend(nothing_item(meta, model, key!("reason-port-bus-page")));
                return items;
            }
            let table = model
                .project_session()
                .map(|s| s.navigator_table(crate::navigators::NavKind::Bus));
            let ports = table.iter().flat_map(|t| &t.nodes).filter_map(|n| {
                let (device, port) = crate::shell::undrawn_port(&n.key)?;
                Some((n.label.clone(), json!({ "device": device, "port": port })))
            });
            items.extend(target_items(
                meta,
                model,
                ports.collect(),
                key!("reason-no-undrawn-port"),
            ));
            items
        }
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

/// The app menu generated from the registry, in the state of `model` (doc 16a §9.2): File, Edit, the tabs' menus
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
