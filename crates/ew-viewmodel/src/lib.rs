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

/// The value of an `Ok`, or the enclosing function returns the `Err`'s value as it is: for an action that reads a
/// parameter with a reader whose error is already the action's outcome.
macro_rules! or_return {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(e) => return e,
        }
    };
}

pub mod about;
pub mod actions;
// The ID alias `@n`.
pub mod alias;
pub mod area;
pub mod canvas;
pub mod clipboard;
pub mod command_bar;
pub mod describe;
// The dialog specifications.
pub mod dialogs;
pub mod find;
pub mod first_run;
// WP-S2.08 part 1 (D5 (b)): the glossary file, the palette's aliases.
pub mod glossary;
pub mod help;
pub mod highlight;
pub mod history;
// The jobs (the PDF export) and the notifications.
pub mod jobs;
// The keymap files, the presets, the conflict rules and the editor's rows.
pub mod keymap;
pub mod layout;
pub mod navigator;
pub mod navigators;
pub mod notifications;
// The palette's ranking, recents, prefixes and parameter forms.
pub mod palette;
pub mod preview;
pub mod properties;
pub mod rail;
pub mod script;
pub mod settings;
pub mod shell;
pub mod status;
pub mod strings;
pub mod tools;
pub mod undo_history;
pub mod window;
pub mod wire_colours;
pub mod wizard;
mod wizard_data;
