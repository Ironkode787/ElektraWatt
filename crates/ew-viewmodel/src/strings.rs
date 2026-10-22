// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The English texts of the shell, by key (WP-S2.01 part 1, spec section 5.2).
//!
//! Every user-facing text of `ew-viewmodel` and `ew-ui` is a key of [`lookup`]; no other literal reaches a
//! screen, a menu, a banner or a transcript. WP-S2.08 replaces the `match` with Fluent bundles without touching a
//! call site. A text may hold the placeholders `{0}`, `{1}`, ... that [`format()`] fills in.

/// The text of `key`, or `None` for a key that has none.
pub fn lookup(key: &str) -> Option<&'static str> {
    Some(match key {
        // Menus and submenus.
        "menu-file" => "File",
        "menu-edit" => "Edit",
        "menu-view" => "View",
        "menu-page" => "Page",
        "menu-tools" => "Tools",
        "menu-help" => "Help",
        "submenu-open-recent" => "Open recent",
        "submenu-export" => "Export",
        "submenu-ui-scale" => "UI scale",
        "menu-no-recent" => "(none)",
        "scale-100" => "100 %",
        "scale-125" => "125 %",
        "scale-150" => "150 %",
        "scale-200" => "200 %",

        // Actions.
        "action-file-open" => "Open...",
        "action-file-open-recent" => "Open recent project",
        "action-file-save" => "Save",
        "action-file-save-as" => "Save as...",
        "action-file-export-pdf" => "Export PDF...",
        "action-file-close" => "Close project",
        "action-app-quit" => "Quit",
        "action-edit-undo" => "Undo",
        "action-edit-redo" => "Redo",
        "action-page-next" => "Next page",
        "action-page-previous" => "Previous page",
        "action-page-first" => "First page",
        "action-page-last" => "Last page",
        "action-page-goto" => "Go to page...",
        "action-view-fit-page" => "Fit page",
        "action-view-ui-scale" => "UI scale",
        "action-tools-palette" => "Command palette",
        "action-help-shortcuts" => "Keyboard shortcuts",
        "action-help-about" => "About ElektraWatt",
        "action-command-run" => "Run model command",
        "action-view-snapshot" => "Write page snapshot",
        "action-ui-describe" => "Describe the user interface",
        "action-ui-dismiss" => "Dismiss popup or dialog",

        // Why an action is disabled.
        "reason-read-only" => "The project is open read-only",
        "reason-nothing-to-undo" => "Nothing to undo",
        "reason-nothing-to-redo" => "Nothing to redo",
        "reason-no-project" => "No project is open",
        "reason-no-pages" => "The project has no pages",
        "reason-no-next-page" => "This is the last page",
        "reason-no-previous-page" => "This is the first page",
        "reason-no-recent" => "There are no recent projects",
        "reason-no-page" => "No page is shown",
        "reason-parameters-later" => "Parameter forms come with a later version; run it from a script",

        // Why a project is read-only.
        "readonly-requested" => "opened read-only on request",
        "readonly-locked" => "another session holds the project",
        "readonly-not-writable" => "the project cannot be written",
        "readonly-integrity" => "integrity findings",
        "readonly-format" => "a newer file format",
        "readonly-lock-lost" => "the lock was taken over",
        "readonly-recovery" => "unsaved changes from an earlier session",

        // Banners and messages.
        "banner-recovery" => {
            "This project has unsaved changes from a session that ended unexpectedly. It is open read-only; \
             recovery comes with a later version."
        }
        "banner-read-only" => "This project is open read-only: {0}.",
        "banner-findings" => "{0} integrity findings were found when the project was opened.",
        "banner-settings-bad" => {
            "The settings file could not be read; defaults are used and the old file was kept as {0}."
        }
        "banner-settings-write" => "The settings could not be written.",
        "banner-recent-removed" => "{0} could not be opened and was removed from the recent projects.",
        "banner-dismiss" => "Dismiss",
        "message-no-snapshot" => "The snapshot needs the page renderer, which this runner does not have.",
        "error-unknown-action" => "Unknown action `{0}`.",
        "error-missing-param" => "Missing parameter `{0}`.",
        "error-bad-param" => "Parameter `{0}` is not valid.",
        "error-params-object" => "The parameters are a JSON object.",
        "error-open" => "Cannot open {0}: {1}.",
        "error-save" => "Saving failed: {0}.",
        "error-save-as" => "Save as failed: {0}.",
        "error-export" => "Export failed: {0}.",
        "error-write" => "The file could not be written.",
        "error-unsaved" => "The project has unsaved changes; the step needs \"discard\": true.",
        "error-unsaved-at-end" => {
            "The script ended with unsaved changes; their journal was kept for recovery. End a script that edits with              \"app.quit\" and \"discard\": true, or save first."
        }
        "banner-no-file-dialog" => {
            "The file dialog is not available: it needs the xdg-desktop-portal service on the session bus. Start the              portal for your desktop (for example xdg-desktop-portal-gtk or -kde) or open the project as an argument."
        }
        "error-no-folio" => "No page has the folio {0}.",
        "error-command" => "{0}",
        "error-render" => "The page could not be drawn: {0}",
        "openerr-io" => "the files cannot be read",
        "openerr-not-dir" => "it is not a project directory",
        "openerr-format" => "the file format is not supported",
        "openerr-migration" => "it needs a migration",
        "openerr-busy" => "a save is in progress",
        "openerr-no-project" => "it holds no project",
        "openerr-model" => "the project data is not valid",
        "openerr-takeover" => "the lock could not be taken over",
        "openerr-exists" => "the directory already holds a project",
        "openerr-other" => "the project cannot be opened",
        "saveerr-read-only" => "the project is read-only",
        "saveerr-lock-lost" => "the lock was taken over",
        "saveerr-external" => "the project changed on disk",
        "saveerr-failed" => "a file could not be written; nothing changed on disk",
        "saveerr-incomplete" => "the save is incomplete and completes on the next open",
        "saveerr-pending" => "an earlier save is still incomplete",
        "saveerr-other" => "the project could not be written",

        // The window, the start page, the page view.
        "window-title" => "{0} - ElektraWatt",
        "window-title-start" => "ElektraWatt",
        "start-title" => "ElektraWatt",
        "start-open" => "Open...",
        "start-recent" => "Recent projects",
        "start-update-check" => "Update check: off",
        "pages-title" => "Pages",
        "page-rendering" => "Rendering page {0}...",
        "page-no-pages" => "The project has no pages.",

        // The status bar.
        "status-hint-view" => "Read-only view",
        "status-hint-start" => "No project open",
        "status-page" => "{0} ({1} of {2})",
        "status-saved" => "saved",
        "status-unsaved" => "*",
        "status-read-only" => "read-only: {0}",
        "status-backend" => "CPU",
        "status-language" => "EN",
        "status-exporting" => "Exporting PDF...",

        // The palette and the dialogs.
        "palette-title" => "Command palette",
        "palette-hint" => "Type a command...",
        "palette-no-match" => "No matching command",
        "dialog-close" => "Close",
        "dialog-save" => "Save",
        "dialog-discard" => "Discard",
        "dialog-cancel" => "Cancel",
        "dialog-go" => "Go",
        "confirm-title" => "Unsaved changes",
        "confirm-text" => "The project has unsaved changes.",
        "goto-title" => "Go to page",
        "goto-hint" => "Folio",
        "shortcuts-title" => "Keyboard shortcuts",
        "about-title" => "About ElektraWatt",
        "about-version" => "Version {0}",
        "about-page-view" => "Page view: CPU renderer",
        "about-privacy" => "Update check: off. Crash reports: none",
        "about-licence" => "Licence: GPL-3.0-or-later",
        "about-repository" => "Repository: {0}",
        "prompt-open-title" => "Open a project directory",
        "prompt-save-as-title" => "Save the project as a new directory",
        "prompt-export-title" => "Export PDF",
        "prompt-pdf-name" => "project.pdf",

        // The text of the describe output.
        "describe-menu" => "menu {0}",
        "describe-item" => "item {0} \"{1}\"",
        _ => return None,
    })
}

/// The text of `key`; the key itself for a key without a text (a missing text is a bug a test catches).
pub fn text(key: &str) -> &'static str {
    lookup(key).unwrap_or("?")
}

/// The text of `key` with `{0}`, `{1}`, ... replaced by `args`.
pub fn format(key: &str, args: &[&str]) -> String {
    let mut out = text(key).to_owned();
    for (i, arg) in args.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), arg);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_are_filled() {
        assert_eq!(format("status-page", &["103", "3", "36"]), "103 (3 of 36)");
        assert_eq!(
            format("window-title", &["J-E-2416.ew"]),
            "J-E-2416.ew - ElektraWatt"
        );
    }

    #[test]
    fn an_unknown_key_has_no_text() {
        assert_eq!(lookup("no-such-key"), None);
        assert_eq!(text("no-such-key"), "?");
    }
}
