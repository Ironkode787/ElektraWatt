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
        "action-view-zoom-in" => "Zoom in",
        "action-view-zoom-out" => "Zoom out",
        "action-view-zoom-actual" => "Actual size",
        "action-view-pan-left" => "Pan left",
        "action-view-pan-right" => "Pan right",
        "action-view-pan-up" => "Pan up",
        "action-view-pan-down" => "Pan down",
        "action-view-grid" => "Grid",
        "action-view-frame-stats" => "Frame statistics",
        "action-view-cpu-canvas" => "Use the CPU renderer for the canvas",
        "banner-gpu-init" => "The GPU canvas could not start ({0}); the canvas draws on the CPU.",
        "banner-gpu-lost" => {
            "The GPU canvas stopped ({0}); the canvas draws on the CPU for the rest of this session."
        }
        "banner-gpu-no-software-adapter" => {
            "--no-gpu: no software adapter was found, so the window uses {0}; the canvas draws on the CPU."
        }
        "action-view-zoom-to" => "Zoom to",
        "action-view-pan" => "Pan the canvas",
        "action-view-canvas-resize" => "Resize the canvas",
        "action-canvas-pointer" => "Point at the canvas",
        "action-canvas-snapshot" => "Write canvas snapshot",
        "submenu-pan" => "Pan",
        "reason-zoom-max" => "The zoom is at its largest (5000 %)",
        "reason-zoom-min" => "The zoom is at its smallest (5 %)",
        "status-cursor" => "x {0} y {1} mm",
        "status-zoom" => "{0} %",
        "status-grid-qet" => "QET 10 px",
        "status-grid-mm" => "{0} mm",
        "status-grid-off" => "Grid off",
        "canvas-name" => "Page {0}, {1}",
        "canvas-description" => "{0}, zoom {1}",
        "canvas-stats-fps" => "{0} fps",
        "canvas-stats-frame" => "frame p99 {0} ms",
        "canvas-stats-phase" => "{0} p99 {1} ms",
        "canvas-stats-tiles" => "tiles pending {0}",
        "canvas-stats-backend" => "backend {0}",
        "message-no-canvas" => "The page cannot be shown: {0}",
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
            "This project has unsaved changes from a session that ended unexpectedly. It is open read-only. \
             File, Discard unsaved changes of the earlier session deletes them and opens the project for \
             editing; recovering them comes with a later version."
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
        "prompt-save-as-title" => "Choose a new, empty directory",
        "prompt-export-title" => "Export PDF",
        "prompt-pdf-name" => "project.pdf",

        // The text of the describe output.
        "describe-menu" => "menu {0}",
        "describe-item" => "item {0} \"{1}\"",

        // Part 2: menus, actions and reasons.
        "submenu-workspace" => "Workspace",
        "action-file-new" => "New project...",
        "action-file-open-sample" => "Open sample project",
        "action-file-import-qet" => "Import QElectroTech project...",
        "action-file-discard-journal" => "Discard unsaved changes of the earlier session...",
        "action-view-workspace-load" => "Load workspace",
        "action-view-workspace-save" => "Save layout as...",
        "action-view-workspace-reset" => "Reset layout",
        "action-tools-company-standard" => "Company standard wizard...",
        "action-help-documentation" => "Documentation",
        "action-help-tutorials" => "Tutorials",
        "action-help-first-run" => "First-run setup...",
        "action-help-notices" => "Third-party notices",
        "action-settings-set" => "Change a setting",
        "action-first-run-set" => "Answer a first-run question",
        "action-first-run-next" => "First run: next step",
        "action-first-run-back" => "First run: previous step",
        "action-first-run-finish" => "First run: finish",
        "action-wizard-set" => "Wizard: set a field",
        "action-wizard-next" => "Wizard: next step",
        "action-wizard-back" => "Wizard: previous step",
        "action-wizard-goto" => "Wizard: go to a step",
        "action-wizard-reset" => "Wizard: start over",
        "action-wizard-cancel" => "Wizard: close",
        "action-wizard-finish" => "Wizard: create the basic project",
        "action-wizard-apply-check-scheme" => "Wizard: apply the check scheme",
        "reason-wizard-finished" => "The basic project is written; close the wizard to start another one",
        "reason-wizard-not-finished" => "Create the basic project first",
        "reason-check-scheme-written" => "The check scheme is applied already",
        "reason-human-only" => {
            "Only a person applies this, in the window: it decides what the checks report (ADR-0008)"
        }
        "banner-check-scheme-applied" => "The check scheme {0} was applied to {1}.",
        "wizard-apply-check-scheme" => "Apply the check scheme",
        "wizard-kept-apply" => "kept: apply after finishing",
        "wizard-close" => "Close",
        "confirm-check-scheme-title" => "Apply the check scheme",
        "confirm-check-scheme-text" => {
            "Apply the check scheme {0} to the basic project? The check scheme decides which rules the release gate runs and what the checks report."
        }
        "dialog-apply" => "Apply",
        "reason-no-journal" => "The project has no journal of an earlier session",
        "reason-no-wizard" => "The company-standard wizard is not open",
        "reason-no-first-run" => "The first-run setup is not open",
        "readonly-sample" => "the sample project",
        "tooltip-learn-more" => "F1: learn more",

        // Part 2: banners and errors.
        "banner-sample" => {
            "This is the sample project. It is open read-only; Save as makes a copy of your own."
        }
        "banner-tutorial-copy" => "A fresh copy of the sample project was made for the tutorials.",
        "banner-imported" => "Imported {0}: {1} pages, {2} devices, {3} findings in the migration report.",
        "banner-import-no-content" => {
            "The starter library was not found; symbols the project refers to may be missing."
        }
        "banner-journal-discarded" => {
            "The unsaved changes of the earlier session were discarded. The project is open for editing."
        }
        "banner-workspace-bad" => {
            "The workspace file {0} could not be read; the built-in layout is used and the file was left as it is."
        }
        "banner-workspace-write" => "The workspace file could not be written.",
        "banner-wizard-done" => "The company standard was written to {0}. New projects start from it.",
        "error-new" => "Cannot create the project: {0}.",
        "error-no-sample" => "The sample project is not installed.",
        "error-import" => "Cannot import {0}: {1}.",
        "error-import-read" => "Cannot read {0}.",
        "error-import-write" => "Cannot write the imported project: {0}.",
        "error-confirm-needed" => "The step discards unsaved changes and needs \"confirm\": true.",
        "error-workspace-missing" => "There is no workspace named {0}.",
        "error-workspace-bad" => "The workspace {0} cannot be used: {1}.",
        "error-workspace-name" => "A workspace name has 1 to 40 letters, digits, spaces, - or _.",
        "error-workspace-write" => "The workspace file could not be written.",
        "error-wizard-write" => "The basic project could not be written: {0}.",

        // Part 2: the start page, the panes and the dialogs.
        "start-new" => "New project",
        "start-template-builtin" => "Template: the built-in basic project",
        "start-template" => "Template: {0}",
        "start-sample" => "Open sample project",
        "start-import-qet" => "Import QElectroTech project",
        "start-company-standard" => "Create company standard",
        "start-edit-standard" => "Edit company standard",
        "start-tutorials" => "Tutorials",
        "start-quick-language" => "Language",
        "start-quick-keymap" => "Keymap",
        "start-privacy-change" => "change",
        "start-update-check-on" => "Update check: on (this version has none)",
        "language-en" => "English",
        "keymap-ew" => "EW",
        "keymap-kicad" => "KiCad-like",
        "theme-system" => "Follow the system",
        "theme-light" => "Light",
        "theme-dark" => "Dark",
        "origin-eplan" => "EPLAN",
        "origin-qet" => "QElectroTech",
        "origin-kicad" => "KiCad",
        "origin-other" => "Another tool",
        "pane-pages" => "Pages",
        "pane-page-view" => "Page view",
        "dialog-save-workspace" => "Save layout",
        "save-workspace-title" => "Save layout as",
        "save-workspace-hint" => "Workspace name",
        "confirm-journal-title" => "Discard unsaved changes",
        "confirm-journal-text" => {
            "An earlier session of this project ended with unsaved changes. Discarding them deletes that work."
        }
        "notices-title" => "Third-party notices",
        "notices-generated-missing" => {
            "The notices of the Rust crates are generated for every release (THIRD-PARTY-NOTICES.txt next to the \
             application); this build has none."
        }
        "about-privacy-on" => {
            "Update check: on (this version has none; nothing is sent). Crash reports: none"
        }
        "about-privacy-file" => {
            "PRIVACY.md does not exist yet; it comes with the first release. Until then this version makes no network \
             connection and sends nothing."
        }
        "about-disclaimer" => {
            "ElektraWatt's checks do not replace engineering verification by a competent person."
        }
        "about-support-prerelease" => "Support: pre-release build; only the latest 0.x release is supported.",
        "about-support-until" => "Support: security and data-loss fixes until {0}.",
        "about-support-ended" => "Support: this release line no longer receives security fixes (ended {0}).",
        "about-notices-button" => "Third-party notices",
        "about-docs-button" => "Documentation",
        "doc-missing" => {
            "The user guide is not installed with this build; build it with mdbook (docs/book) and install it next to the application."
        }
        "prompt-open-qet-title" => "Open a QElectroTech project",
        "prompt-new-project-title" => "Create the project in a new directory",
        "prompt-basic-project-title" => "Write the basic project to a new directory",

        // Part 2: the first-run flow.
        "firstrun-title" => "Welcome to ElektraWatt",
        "firstrun-step-language" => "Language",
        "firstrun-step-origin" => "I come from",
        "firstrun-step-privacy" => "Privacy",
        "firstrun-step-appearance" => "Appearance",
        "firstrun-step-start" => "Where to start",
        "firstrun-text-language" => "Choose the language of the application.",
        "firstrun-text-origin" => {
            "Which tool do you come from? This sets the keymap preset and the vocabulary of the palette."
        }
        "firstrun-text-privacy" => {
            "ElektraWatt makes no network connection by itself. An optional update check would send the product \
             version, the operating system and the architecture, at most once a day. Crash reports stay on \
             this computer."
        }
        "firstrun-update-check" => "Check for updates (off by default)",
        "firstrun-text-appearance" => "Choose the theme and the size of the user interface.",
        "firstrun-theme" => "Theme",
        "firstrun-scale" => "Scale",
        "firstrun-text-start" => "What do you want to do first?",
        "firstrun-start-wizard" => "Create the company standard",
        "firstrun-start-sample" => "Open the sample project",
        "firstrun-start-qet" => "Import a QElectroTech project",
        "firstrun-start-new" => "New project from the built-in basic project",
        "firstrun-start-none" => "Start with the start page",
        "firstrun-skip" => "Skip",
        "firstrun-next" => "Next",
        "firstrun-back" => "Back",
        "firstrun-finish" => "Finish",

        // Part 2: the company-standard wizard.
        "wizard-title" => "Company standard",
        "wizard-step-company" => "Company",
        "wizard-step-frame" => "Frame and title block",
        "wizard-step-colour" => "Colour profile",
        "wizard-step-size" => "Size profile",
        "wizard-step-numbering" => "Identifiers and numbering",
        "wizard-step-structure" => "Structure and sections",
        "wizard-step-revisions" => "Revisions",
        "wizard-step-languages" => "Languages and formats",
        "wizard-step-reports" => "Reports and checks",
        "wizard-step-review" => "Review and create",
        "wizard-hint-company" => "The panel builder's name and the notice for the title block.",
        "wizard-hint-frame" => {
            "The frame and title block of new pages; the cover and the section dividers keep their own frames."
        }
        "wizard-hint-colour" => "The wire colours by voltage class and function, with a stated basis.",
        "wizard-hint-size" => "The cross section of each circuit class.",
        "wizard-hint-numbering" => {
            "How devices and terminal strips are numbered, and how cross references print."
        }
        "wizard-hint-structure" => {
            "The structure nodes of new projects and their sections: one section per line, number, first-last folio, title."
        }
        "wizard-hint-revisions" => "The label of the first revision.",
        "wizard-hint-languages" => "The source language and the formats of printed dates and numbers.",
        "wizard-hint-reports" => {
            "The check scheme of new projects. Kept in the draft: it decides what the release gate checks, so a person sets it in the project, not the wizard's script."
        }
        "wizard-hint-review" => "Check the choices, then write the basic project to a new directory.",
        "wizard-field-company-name" => "Company name",
        "wizard-field-company-notice" => "Notice text for the title block",
        "wizard-field-frame" => "Frame and title block",
        "wizard-field-colour" => "Colour profile",
        "wizard-field-size" => "Size profile",
        "wizard-field-csa-control" => "Control circuits (mm2)",
        "wizard-field-csa-mains" => "Mains circuits (mm2)",
        "wizard-field-device-format" => "Device numbering format",
        "wizard-field-strip-format" => "Terminal strip numbering format",
        "wizard-field-xref" => "Cross-reference style",
        "wizard-field-function-node" => "Function node (=)",
        "wizard-field-location-node" => "Location node (+)",
        "wizard-field-sections" => "Sections",
        "wizard-field-first-revision" => "First revision label",
        "wizard-field-language" => "Source language",
        "wizard-field-date-format" => "Date format",
        "wizard-field-decimal" => "Decimal separator",
        "wizard-field-check-scheme" => "Check scheme",
        "choice-frame-qet-grid" => "QET-compatible A4 landscape, with grid",
        "choice-frame-qet-nogrid" => "QET-compatible A4 landscape, without grid",
        "choice-colour-sample" => "DBU/WH 24 V (the sample project's convention)",
        "choice-colour-default" => "The profile's default",
        "choice-size-sample" => "0.75/1.5 (the sample project's profiles, with the cross sections below)",
        "choice-size-default" => "The profile's default",
        "choice-xref-qet" => "QET style (102-A1)",
        "choice-xref-eplan" => "EPLAN style (/102.A1)",
        "choice-language-en" => "English",
        "choice-date_format-dd_mm_yyyy" => "dd-mm-yyyy",
        "choice-date_format-iso8601" => "yyyy-mm-dd",
        "choice-decimal-point" => "0.75",
        "choice-decimal-comma" => "0,75",
        "choice-check_scheme-drafting" => "drafting",
        "choice-check_scheme-pre-delivery" => "pre-delivery",
        "choice-check_scheme-full" => "full",
        "wizard-error-field" => "That is not a field of the wizard.",
        "wizard-error-choice" => "That is not one of the choices.",
        "wizard-error-csa" => "A cross section is a number above zero, such as 0.75.",
        "wizard-error-format" => {
            "A numbering format has {page}, {letter} and {counter}, in any order and with other text between them; braces must pair."
        }
        "wizard-error-node" => "A node code has up to 20 letters, digits, - or _.",
        "wizard-error-sections" => {
            "One section per line: number, first-last folio, title. Numbers and folio ranges must not repeat or overlap."
        }
        "wizard-error-revision" => "A revision label has no spaces and up to 20 characters.",
        "wizard-error-text" => "That text is too long.",
        "wizard-error-command" => "The command {0} was refused: {1}",
        "wizard-written" => "written to the basic project",
        "wizard-kept" => "kept in the draft",
        "wizard-next" => "Next",
        "wizard-back" => "Back",
        "wizard-cancel" => "Close (the draft is kept)",
        "wizard-reset" => "Start over",
        "wizard-create" => "Create the basic project...",
        "wizard-preview-title" => "Preview",
        "wizard-preview-cover" => "Cover",
        "wizard-preview-page" => "Folio 102",
        "wizard-error-draft-write" => "The draft could not be saved in the configuration directory.",
        "error-dir-not-empty" => "The directory is not empty; choose a new, empty directory.",
        "error-workspace-case" => "A workspace whose name differs only in letter case already exists.",
        "layout-not-json" => "not JSON: {0}",
        "layout-not-object" => "a workspace file is a JSON object",
        "layout-schema" => "`$schema` is not ew.workspace/1",
        "layout-name-missing" => "`name` is missing",
        "layout-root-missing" => "`root` is missing",
        "layout-no-page-view" => "the page view is missing",
        "layout-too-deep" => "the layout is nested too deeply",
        "layout-too-many" => "the layout has too many nodes",
        "layout-pane-twice" => "the pane `{0}` appears twice",
        "layout-node-object" => "a node is a JSON object",
        "layout-unknown-member" => "unknown member `{0}`",
        "layout-pane-name" => "`pane` is a name",
        "layout-unknown-pane" => "unknown pane `{0}`",
        "layout-tabs-list" => "`tabs` is a list",
        "layout-tabs-empty" => "`tabs` is empty",
        "layout-split-dir" => "`split` is horizontal or vertical",
        "layout-children-list" => "`children` is a list",
        "layout-children-empty" => "`children` is empty",
        "layout-child-object" => "a child is a JSON object",
        "layout-share" => "`share` is a positive integer",
        "layout-node-missing" => "`node` is missing",
        "layout-node-kind" => "a node has `pane`, `tabs` or `split`",
        "wizard-preview-no-sample" => "The sample project is not installed; the preview needs it.",
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
