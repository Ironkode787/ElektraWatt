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
pub mod app;
pub mod canvas;
pub mod canvas_area;
pub mod dock;
mod first_run;
pub mod fonts;
pub mod gpu;
pub mod headless;
pub mod keys;
mod latest;
mod menu;
pub mod navigator;
pub mod page_view;
mod palette;
mod picker;
pub mod properties;
mod start_page;
mod status_bar;
pub mod tool_view;
mod wizard;
