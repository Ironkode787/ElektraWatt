// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! egui widgets, canvas host, tools (state machines), command palette, keymaps and the action registry
//! behind every menu.
//!
//! The application shell: [`app::App`] paints the shell model of `ew-viewmodel` (menu bar, start page,
//! the first-run screen, the company-standard wizard, the docked project view with the page list and the read-only
//! page view, status bar, command palette, dialogs) and sends every input to `ShellModel::dispatch`; [`dock`]
//! turns the toolkit-neutral layout of the workspaces into an egui_tiles tree and back; [`headless`] runs the same
//! dispatch from a UI script with no window. Only this crate and the application binary name egui;
//! `ew-viewmodel` compiles without it.

#![forbid(unsafe_code)]

pub use ew_viewmodel;

mod about;
// The recovery dialog's test clock (the module is private).
#[doc(hidden)]
pub use about::set_now_for_tests;
pub mod app;
pub mod canvas;
pub mod canvas_area;
pub mod canvas_chips;
pub mod clock;
// The command bar and the tool HUD.
// The Join chooser (doc 16a §6.22).
mod chooser;
mod command_bar;
// The small components (buttons, badges, chips, empty states, cards) and the dialog frame.
mod components;
mod dialog;
pub mod dock;
pub mod find;
pub mod first_run;
mod focus;
pub mod fonts;
pub mod glossary;
pub mod gpu;
pub mod headless;
mod history;
mod hud;
pub mod icons;
// The Jobs pane, the toasts and the notifications log.
mod jobs;
pub mod keys;
// The keymap editor (Help ▸ Keyboard shortcuts).
mod keymap_editor;
mod latest;
// The Macros navigator's detail and the Libraries navigator's Update….
mod library_panes;
mod menu;
// The message panel's header, card, detail block and icons.
mod messages;
mod nav_table;
pub mod navigator;
mod notifications;
pub mod offscreen;
pub mod page_view;
mod palette;
mod preview;
mod rail;
// The parts table import card.
mod part_dialog;
// The print pre-dialog and its preview.
mod parts_card;
mod parts_map;
mod print_dialog;
// The junction sequence editor popup.
mod junction;
mod picker;
pub mod properties;
mod splitter;
mod start_page;
mod symbol_editor;
// The start-up offer after a crash.
mod startup;
mod status_bar;
// The text tool's popup.
mod text_popup;
// The design tokens, themes and their mapping onto egui.
pub mod theme;
pub mod tool_view;
mod value_table;
pub mod visual;
// Window memory.
pub mod window_memory;
mod wizard;
