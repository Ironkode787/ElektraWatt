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
// FT-422 (ID-630): the ID alias `@n`.
pub mod alias;
pub mod area;
pub mod canvas;
// WP-S2.17 part 2
pub mod command_bar;
// WP-S2.05 part 3
pub mod clipboard;
pub mod describe;
// WP-S2.17 part 4: the dialog specifications.
pub mod dialogs;
// WP-S2.05 part 2
pub mod find;
pub mod first_run;
pub mod help;
// WP-S2.06 part 2
pub mod highlight;
pub mod history;
// WP-S2.09 part 1: the jobs (the PDF export) and the notifications.
pub mod jobs;
pub mod layout;
pub mod navigator;
// WP-S2.06 part 1
pub mod navigators;
pub mod notifications;
// WP-S2.09 part 3
pub mod preview;
pub mod properties;
pub mod script;
pub mod settings;
pub mod shell;
pub mod status;
pub mod strings;
pub mod tools;
// WP-S2.09 part 3
pub mod undo_history;
pub mod wizard;
mod wizard_data;
