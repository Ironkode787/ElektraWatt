// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `ui.describe` text (WP-S2.01 part 1, spec section 6.2): the visible user interface as text, one line per
//! element, in paint order. It is what an agent reads instead of a screenshot.
//!
//! Paint order: the window title, the menu bar with every entry and its enabled state (and reason), the banner,
//! the start page or the page list with the page view, the status bar, then the palette or a dialog. Directory
//! names appear, paths never.

use crate::actions::{self, ActionItem, MenuEntry, PaletteRun};
use crate::page_list::PageList;
use crate::shell::{Dialog, ShellModel};
use crate::strings;

fn item_line(indent: &str, item: &ActionItem) -> String {
    let mut line = format!(
        "{indent}{}",
        strings::format("describe-item", &[item.id, &item.label])
    );
    if !item.shortcut.is_empty() {
        line.push_str(&format!(" [{}]", item.shortcut));
    }
    if item.checked {
        line.push_str(" (checked)");
    }
    match (&item.enabled, &item.reason) {
        (true, _) => line.push_str(" enabled"),
        (false, Some(reason)) => line.push_str(&format!(" disabled: {reason}")),
        (false, None) => line.push_str(" disabled"),
    }
    line
}

/// The text of the visible user interface of `model`, lines separated by `\n`, without a final newline.
pub fn describe(model: &ShellModel) -> String {
    let mut lines: Vec<String> = vec![format!("title: {}", model.title())];
    for menu in actions::menu_model(model) {
        lines.push(strings::format("describe-menu", &[&menu.label]));
        for entry in &menu.entries {
            match entry {
                MenuEntry::Separator => {}
                MenuEntry::Item(item) => lines.push(item_line("  ", item)),
                MenuEntry::Submenu { label, items, .. } => {
                    lines.push(format!("  submenu \"{label}\""));
                    lines.extend(items.iter().map(|i| item_line("    ", i)));
                }
            }
        }
    }
    if let Some(banner) = model.banner() {
        lines.push(format!("banner: {banner}"));
    }
    if model.has_project() {
        let list = PageList::of(model);
        lines.push(format!("page list \"{}\"", strings::text("pages-title")));
        for (i, label) in list.labels.iter().enumerate() {
            let selected = if list.current == Some(i) { " selected" } else { "" };
            lines.push(format!("  page \"{label}\"{selected}"));
        }
        match model.current_folio() {
            Some(folio) => lines.push(format!("page view: {folio}")),
            None => lines.push(format!("page view: {}", strings::text("page-no-pages"))),
        }
    } else {
        lines.push(format!("start page \"{}\"", strings::text("start-title")));
        lines.push(format!("  button \"{}\"", strings::text("start-open")));
        lines.push(format!("  label \"{}\"", strings::text("start-recent")));
        for (i, name) in model.recent_names().iter().enumerate() {
            lines.push(format!("  recent {} \"{name}\"", i + 1));
        }
        lines.push(format!("  label \"{}\"", strings::text("start-update-check")));
    }
    lines.push(format!("status bar: {}", model.status_fields(false).join(" | ")));
    if let Some(palette) = model.palette() {
        lines.push(format!(
            "palette \"{}\": query \"{}\"",
            strings::text("palette-title"),
            palette.query
        ));
        let entries = model.palette_entries();
        if entries.is_empty() {
            lines.push(format!("  {}", strings::text("palette-no-match")));
        }
        for (i, entry) in entries.iter().enumerate() {
            let mark = if i == palette.selected { ">" } else { " " };
            let kind = match &entry.run {
                PaletteRun::Action(_) => "action",
                PaletteRun::Command(_) => "command",
            };
            let mut line = format!("{mark} {kind} {} \"{}\"", entry.id, entry.label);
            if !entry.shortcut.is_empty() {
                line.push_str(&format!(" [{}]", entry.shortcut));
            }
            match (&entry.enabled, &entry.reason) {
                (true, _) => line.push_str(" enabled"),
                (false, Some(reason)) => line.push_str(&format!(" disabled: {reason}")),
                (false, None) => line.push_str(" disabled"),
            }
            lines.push(line);
        }
    }
    match model.dialog() {
        None => {}
        Some(Dialog::About) => {
            lines.push(format!("dialog \"{}\"", strings::text("about-title")));
            lines.push(format!(
                "  {}",
                strings::format("about-version", &[&model.config().app_version])
            ));
            lines.push(format!("  {}", strings::text("about-page-view")));
            lines.push(format!("  {}", strings::text("about-privacy")));
            lines.push(format!("  {}", strings::text("about-licence")));
            lines.push(format!(
                "  {}",
                strings::format("about-repository", &[env!("CARGO_PKG_REPOSITORY")])
            ));
            lines.push(format!("  button \"{}\"", strings::text("dialog-close")));
        }
        Some(Dialog::Shortcuts) => {
            lines.push(format!("dialog \"{}\"", strings::text("shortcuts-title")));
            for meta in actions::ACTIONS
                .iter()
                .filter(|a| !a.headless_only && !a.shortcut.is_empty())
            {
                lines.push(format!(
                    "  {} \"{}\" {}",
                    meta.id,
                    strings::text(meta.label_key),
                    meta.shortcut.join(", ")
                ));
            }
            lines.push(format!("  button \"{}\"", strings::text("dialog-close")));
        }
        Some(Dialog::ConfirmDiscard { pending }) => {
            lines.push(format!("dialog \"{}\"", strings::text("confirm-title")));
            lines.push(format!("  {}", strings::text("confirm-text")));
            lines.push(format!("  pending {}", pending.action));
            for key in ["dialog-save", "dialog-discard", "dialog-cancel"] {
                lines.push(format!("  button \"{}\"", strings::text(key)));
            }
        }
        Some(Dialog::GotoPage { text }) => {
            lines.push(format!("dialog \"{}\"", strings::text("goto-title")));
            lines.push(format!("  field \"{}\": \"{text}\"", strings::text("goto-hint")));
            lines.push(format!("  button \"{}\"", strings::text("dialog-go")));
            lines.push(format!("  button \"{}\"", strings::text("dialog-cancel")));
        }
    }
    lines.join("\n")
}

impl ShellModel {
    /// The visible user interface as text, one line per element in paint order (`ui.describe`).
    pub fn describe(&self) -> String {
        describe(self)
    }
}
