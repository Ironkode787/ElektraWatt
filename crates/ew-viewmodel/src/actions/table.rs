// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The action table in its sections. Each section is a slice of entries; [`SECTIONS`] lists them in registry order.

use super::ActionMeta;

mod controls;
mod draw;
mod edit;
mod file;
mod page;
mod panes;
mod view;
mod window;

/// The sections in registry order.
pub(super) const SECTIONS: &[&[ActionMeta]] = &[
    file::ACTIONS,
    edit::ACTIONS,
    page::ACTIONS,
    view::ACTIONS,
    draw::ACTIONS,
    window::ACTIONS,
    panes::ACTIONS,
    controls::ACTIONS,
];
