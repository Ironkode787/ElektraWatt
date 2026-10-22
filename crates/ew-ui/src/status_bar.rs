// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The status bar (WP-S2.01 part 1, spec section 5.3; UX/UI doc 16 section 3.6): the fields of
//! `ShellModel::status_fields`, left to right.

use egui::Ui;
use ew_viewmodel::shell::ShellModel;

/// Paints the status bar of `shell` into `ui`; `exporting` adds the export label.
pub fn show(ui: &mut Ui, shell: &ShellModel, exporting: bool) {
    ui.horizontal(|ui| {
        for (i, field) in shell.status_fields(exporting).iter().enumerate() {
            if i > 0 {
                ui.separator();
            }
            ui.label(field.as_str());
        }
    });
}
