// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Toolkit-agnostic panel models: navigator trees, table rows with edit-to-command mapping, property-grid
//! rows with provenance and message lists, each with a revision stamp; and, since WP-S2.01 part 1, the model of
//! the application shell.
//!
//! The shell ([`shell::ShellModel`]) holds all application state and has one mutating entry point,
//! [`shell::ShellModel::dispatch`], over the action registry of [`actions`]. It reaches the project only through
//! `ew_api::Session`. The egui frame (`ew-ui`) and the headless runner ([`script`]) drive the same `dispatch`.
//! Since WP-S2.02 part 1 the shell holds the canvas model ([`canvas`]): zoom, pan, the pointer and the hover pick
//! of the page on show. There is no egui, wgpu or winit here.

#![forbid(unsafe_code)]

/// The session facade and its types, re-exported so that `ew-ui` names them without a workspace edge to `ew-api`
/// (architecture section 3, rule 7).
pub use ew_api;

pub mod about;
pub mod actions;
pub mod canvas;
pub mod describe;
pub mod first_run;
pub mod help;
pub mod layout;
pub mod page_list;
pub mod script;
pub mod settings;
pub mod shell;
pub mod status;
pub mod strings;
pub mod wizard;
mod wizard_data;
