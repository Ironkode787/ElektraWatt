// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! egui widgets, canvas host, tools (state machines), command palette, keymaps and the action registry
//! behind every menu.
//!
//! WP-S2.01 is the application shell: [`app::App`] paints the shell model of `ew-viewmodel` (menu bar, start page,
//! the first-run screen, the company-standard wizard, the docked project view with the page list and the read-only
//! page view, status bar, command palette, dialogs) and sends every input to `ShellModel::dispatch`; [`dock`]
//! turns the toolkit-neutral layout of the workspaces into an egui_tiles tree and back; [`headless`] runs the same
//! dispatch from a UI script with no window. Only this crate and the application binary name egui;
//! `ew-viewmodel` compiles without it.

#![forbid(unsafe_code)]

pub use ew_viewmodel;

mod about;
// FT-571: the recovery dialog's test clock (the module is private).
#[doc(hidden)]
pub use about::set_now_for_tests;
pub mod app;
pub mod canvas;
pub mod canvas_area;
pub mod canvas_chips;
// WP-S2.17 part 2: the command bar and the tool HUD.
mod command_bar;
// WP-S2.17 part 4: the small components (buttons, badges, chips, empty states, cards) and the dialog frame.
mod components;
mod dialog;
pub mod dock;
// WP-S2.05 part 2
pub mod find;
mod first_run;
mod focus;
pub mod fonts;
pub mod gpu;
pub mod headless;
pub mod icons;
// WP-S2.09 part 3
mod history;
mod hud;
// WP-S2.09 part 1: the Jobs pane, the toasts and the notifications log.
mod jobs;
pub mod keys;
mod latest;
mod menu;
// WP-S2.09 part 2: the message panel's header, card, detail block and icons.
mod messages;
mod nav_table;
pub mod navigator;
mod notifications;
pub mod offscreen;
pub mod page_view;
// WP-S2.09 part 3
mod palette;
mod preview;
mod rail;
// FT-410: the parts table import card.
mod parts_card;
// WP-S2.04 part 2: the junction sequence editor popup.
mod junction;
mod picker;
pub mod properties;
mod splitter;
mod start_page;
mod status_bar;
// WP-S2.04 part 4b: the text tool's popup.
mod text_popup;
// WP-S2.17 part 1: the design tokens, themes and their mapping onto egui.
pub mod theme;
pub mod tool_view;
// WP-S2.17 part 3 (DK6, FT-348): window memory.
pub mod window_memory;
mod wizard;
