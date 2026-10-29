// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The English texts of the shell, by key (WP-S2.01 part 1, spec section 5.2).
//!
//! Every user-facing text of `ew-viewmodel` and `ew-ui` is a key of [`lookup`]; no other literal reaches a
//! screen, a menu, a banner or a transcript. WP-S2.08 replaces the `match` with Fluent bundles without touching a
//! call site. A text may hold the placeholders `{0}`, `{1}`, … that [`format()`] fills in.

/// The text of `key`, or `None` for a key that has none.
pub fn lookup(key: &str) -> Option<&'static str> {
    Some(match key {
        // Menus and submenus.
        "menu-file" => "File",
        "menu-edit" => "Edit",
        "menu-view" => "View",
        "menu-page" => "Page",
        // WP-S2.17 part 2: the app menu's tab menus (doc 16a §9.2); Insert became Draw, Tools dissolved.
        "menu-project" => "Project",
        "menu-draw" => "Draw",
        "menu-check" => "Check",
        "menu-output" => "Output",
        "menu-help" => "Help",
        "submenu-open-recent" => "Open recent",
        "submenu-ui-scale" => "UI scale",
        "submenu-theme" => "Theme",
        "submenu-density" => "Density",
        "menu-no-recent" => "(none)",
        "scale-100" => "100 %",
        "scale-125" => "125 %",
        "scale-150" => "150 %",
        "scale-200" => "200 %",

        // Actions.
        "action-file-open" => "Open…",
        // The canvas tools (WP-S2.03 part 2).
        "action-tool-select" => "Select",
        "action-tool-measure" => "Measure",
        "action-cp-next" => "Next connection point",
        "action-cp-previous" => "Previous connection point",
        "action-cp-next-unconnected" => "Next unconnected connection point",
        "action-canvas-activate" => "Click at the grid cursor",
        "action-tool-back" => "Tool step back",
        "action-canvas-press" => "Press on the canvas",
        "action-canvas-release" => "Release on the canvas",
        "action-canvas-dwell" => "Pointer at rest",
        "action-canvas-preview-snapshot" => "Link preview snapshot",
        // The place-and-arrange tools (WP-S2.04 part 1).
        "action-tool-place" => "Symbol…",
        "action-tool-place-part" => "Device from part…",
        "action-place-pick" => "Pick a symbol to place",
        "action-picker-filter" => "Filter the picker",
        "action-picker-row" => "Move the picker's row",
        "action-picker-choose" => "Choose the picker's row",
        "action-strip-new" => "Terminal strip…",
        "action-tool-rotate-cw" => "Rotate clockwise",
        "action-tool-rotate-ccw" => "Rotate counter-clockwise",
        "action-tool-mirror-x" => "Mirror left to right",
        "action-tool-mirror-y" => "Mirror top to bottom",
        "action-tool-next-variant" => "Next variant",
        "action-tool-next-rep-type" => "Next representation type",
        "action-tool-move" => "Move",
        "action-tool-finish" => "Finish",
        "action-text-input" => "Type into the tool",
        "action-edit-delete-selection" => "Delete",
        "action-rep-exchange" => "Change symbol…",
        "tool-place" => "Place",
        "tool-exchange" => "Change symbol",
        "hint-place" => "Click or Enter to place; R, X, Y and V turn it; Escape ends",
        "hint-place-proposals" => {
            "Click or Enter to place and connect the proposed wires; Alt+Click or Alt+Enter places without them; Tab tries the next target; Escape ends"
        }
        "hint-select-move" => "Release, click or Enter to move; Escape cancels",
        "hint-exchange" => "Click or Enter to change the symbol; Escape cancels",
        "move-readout" => "dx {0} dy {1} mm",
        "place-target-new" => "new {0}",
        "place-target-join" => "join {0}",
        "place-target-function" => "function",
        "place-target-part" => "part {0}",
        "place-target-terminal" => "terminal {0}:{1}",
        "place-target-box" => "box",
        "announce-place" => "Place {0}: {1}, variant {2}, at {3}",
        "announce-place-no-cell" => "Place {0}: {1}, variant {2}",
        "picker-title" => "Insert symbol",
        "picker-title-exchange" => "Change symbol",
        "picker-filter-hint" => "Filter by key, part number or function",
        "picker-badge-embedded" => "embedded",
        "picker-badge-starter" => "starter",
        "picker-part-function" => "{0} of {1} {2}",
        "picker-main" => "main",
        "picker-empty" => "Nothing matches the filter",
        "reason-letter-needed" => "Give a letter code: the symbol's category maps to none",
        "reason-one-rep" => "Select exactly one representation",
        "reason-nothing-to-move" => "Nothing movable is selected",
        "reason-place-only" => "Only while placing a symbol",
        "reason-nothing-selected" => "Nothing is selected",
        "error-wire-reference" => {
            "No wire of the page shown runs \"{0}\"; name one as the page text does: wire:<end> -> <end>"
        }
        "error-wire-ambiguous" => "Several wires of the page shown run \"{0}\"; click the one you mean",
        "reason-not-on-page" => "The selection is drawn on another page; go to that page to delete it",
        "reason-no-picker" => "The picker is not open",
        "reason-rep-type-not-allowed" => "The page's type does not take this symbol",
        "reason-black-box-tool" => "Use the black box tool (B)",
        "reason-no-symbol-for-function" => "No symbol for this function on this page",
        "reason-no-catalogue" => "Nothing can be placed on this page",
        "reason-unknown-symbol" => "No such symbol, part or function for this page",
        "reason-no-strip" => "No terminal strip to add to: create one (Insert, Terminal strip)",
        "reason-designation-needed" => "Give the terminal's designation",
        "reason-ip-exchange" => "An interruption point keeps its symbol",
        "reason-no-next-rep-type" => "No other symbol draws this function on this page",
        "reason-no-picker-row" => "The picker shows no row",
        "reason-not-editable" => "Symbols are placed on drawn pages of a project that can be changed",
        "banner-host-commit" => "{0} failed: {1}",
        "tool-strip-new" => "New strip",
        "reason-page-type-no-symbols" => "This page type takes no symbols or wires",
        // WP-S2.04 part 2 (wiring; the strip chooser of FT-421). Begin.
        "picker-title-strip" => "Choose a terminal strip",
        "picker-title-black-box" => "Insert black box",
        "picker-strip-row" => "{0}  next terminal {1}",
        "picker-strip-next" => "next terminal {0}",
        "action-tool-wire" => "Connection",
        "action-canvas-activate-alt" => "Click with Alt at the grid cursor",
        "action-tool-next-proposal" => "Next autoconnect proposal",
        "action-tool-connect-on-move" => "Connect while moving",
        "action-tool-route-edit" => "Edit route",
        "action-junction-open" => "Junction sequence…",
        "action-junction-member" => "Move a junction member",
        "action-junction-style" => "Junction style",
        "action-junction-wiring" => "Junction wiring",
        "action-project-crossings" => "Show crossings as hops",
        "tool-wire" => "Wire",
        "tool-route_edit" => "Edit route",
        "hint-wire-armed" => "Click or Enter on a connection point, a junction or a wire to start",
        "hint-wire-routing" => "Click a connection point; Backspace removes the last bend; Escape cancels",
        "hint-route-edit" => "Tab picks the segment, the arrow keys move it, Enter commits; Escape cancels",
        "hint-place-needs-route" => "Give this wire a route first (Edit route)",
        "announce-wire-routing" => {
            "Wire: routing from {0}. Click a connection point; Backspace removes the last bend; Escape cancels"
        }
        "wire-readout" => "{0} mm",
        "reason-connect-on-place" => {
            "Proposals connect when a symbol is placed; a move never adds a connection"
        }
        "reason-one-connection" => "Select exactly one connection",
        "reason-one-junction" => "Select exactly one junction",
        "reason-no-wire-kind" => "The Wire tool does not draw this page type's connections yet",
        "reason-no-junction-editor" => "The junction sequence editor is not open",
        "reason-junction-member" => "No such member position",
        "junction-title" => "Junction {0}",
        "junction-style" => "Style",
        "junction-style-dot" => "Dot",
        "junction-style-bevel" => "Bevel",
        "junction-wiring" => "Wiring",
        "junction-wiring-star" => "Star",
        "junction-wiring-chain" => "Chain",
        "junction-wiring-default" => "Style default",
        "junction-sequence" => "Sequence (Alt+Up, Alt+Down)",
        "junction-wires" => "Physical wires",
        "junction-wire-row" => "{0}: {1} to {2}",
        "junction-orient" => "Orientation {0} (R)",
        // WP-S2.04 part 2. End.
        // WP-S2.04 part 3 (interruption points and black boxes). Begin.
        "action-tool-ip" => "Interruption point",
        "action-tool-black-box" => "Black box…",
        "action-tool-ip-direction" => "Interruption point direction",
        "action-tool-star-source" => "Star source",
        "action-ip-name" => "Interruption point name",
        "action-ip-external" => "External reference",
        "action-ip-continue-on" => "Continue on page…",
        "action-ip-arrange-selected" => "Arrange interruption points",
        "action-box-grip" => "Move a black box grip",
        "action-box-add-cp" => "Add connection point",
        "tool-ip" => "Interruption point",
        "tool-black_box" => "Black box",
        "hint-ip" => "Click or Enter to place; D changes the direction; type a name; Escape ends",
        "hint-ip-external" => {
            "Click or Enter to place the external connection point; type a name; Escape ends"
        }
        "hint-box-armed" => "Click or Enter at the first corner; Escape ends",
        "hint-box-outline" => "Click or Enter at the opposite corner; Escape starts again",
        "reason-no-ip-symbol" => "This page's library has no interruption-point symbol",
        "reason-ip-tool-only" => "Only while placing an interruption point",
        "reason-external-never-pairs" => "An external connection point never pairs, so it has no star source",
        "error-no-external" => {
            "No external reference is \"{0}\"; name one as device:connection, optionally after its location and \" / \" (+J-E-206 / JE-N1000-03:L). The project's: {1}"
        }
        "external-none" => "none (create one with Insert ▸ External reference…, external_ref.new)",
        "reason-external-ambiguous" => "Several external references print this text: name one by its ID",
        "reason-select-one-ip" => "Select exactly one interruption point",
        "reason-select-ips" => "Select at least two interruption points",
        "reason-not-black-box" => "This is no black box: place it with Insert, Symbol (I)",
        "reason-one-black-box" => "Select exactly one black box",
        "reason-grip-not-axis" => "Only a horizontal or vertical edge moves",
        "reason-box-too-small" => "A black box keeps a width and a height",
        "reason-box-inside-out" => "A grip cannot pass the opposite side of the black box",
        "reason-no-grip" => "The black box has no such grip",
        "reason-cp-off-outline" => "Put the grid cursor on the black box's outline",
        "reason-ip-end-grid" => "End the wire on an empty grid point to end it in an interruption point",
        "reason-ip-end-segment" => "Draw a segment before ending the wire in an interruption point",
        "reason-ip-symbol" => "The interruption-point symbol has not exactly one connection point",
        // WP-S2.04 part 3. End.
        // WP-S2.04 part 4b (text, text edit and graphic tools). Begin.
        "submenu-graphic" => "Graphic",
        "action-tool-text" => "Text",
        "action-tool-graphic" => "Graphic",
        "action-text-bind" => "Text binding",
        "action-text-edit" => "Edit text",
        "action-strip-bridge" => "Bridge terminals",
        "graphic-line" => "Line",
        "graphic-polyline" => "Polyline",
        "graphic-rect" => "Rectangle",
        "graphic-circle" => "Circle",
        "graphic-arc" => "Arc",
        "graphic-polygon" => "Polygon",
        "graphic-anticlockwise" => "anticlockwise",
        "graphic-clockwise" => "clockwise",
        "tool-text" => "Text",
        "tool-text_edit" => "Edit text",
        "tool-graphic" => "Graphic",
        "hint-text-armed" => "Click or Enter where the text goes; Escape ends",
        "hint-text-typing" => "Type the text; Enter places it; Escape cancels",
        "hint-text-edit" => "Type the new text; Enter changes it; Escape cancels",
        "hint-graphic-armed" => "Click or Enter at the first point; Escape ends",
        "hint-graphic-drawing" => {
            "Click or Enter at the next point; Shift+Enter or a double click finishes a polyline or a polygon; Backspace removes the last point"
        }
        "reason-text-tool-only" => "Choose the Text tool first",
        "reason-text-bound" => "This text shows a project or page field; change that field instead",
        "reason-select-one-text" => "Select exactly one free text",
        "reason-bridge-selection" => "Select two or more terminals of one strip",
        "reason-bridge-strips" => {
            "The selected terminals belong to more than one strip; select terminals of one strip"
        }
        "reason-bridge-no-shaft" => {
            "Each terminal needs a bridge shaft on its part or a side-1 connection point"
        }
        "reason-bridge-exists" => "These terminals are bridged already",
        // G-PP triage row 7: Draw ▸ Bond.
        "action-bond-draw" => "Bond",
        "submenu-bond" => "Bond",
        "bond-kind-pelv_0v_pe" => "0 V–PE bond (PELV)",
        "bond-kind-fe_pe" => "FE–PE bond",
        "bond-kind-equipotential" => "Equipotential bond",
        "bond-kind-shield" => "Shield bond",
        "reason-bond-selection" => {
            "Select the one wire that bonds the two potentials, or name its two connection points"
        }
        "reason-bond" => "A bond is previewed first",
        "error-bond-end" => {
            "No connection point of the page shown is printed \"{0}\"; name it as the page text does"
        }
        "error-bond-end-ambiguous" => "Several connection points of the page shown are printed \"{0}\"",
        "error-bond-same-end" => "Both ends of the bond are \"{0}\"; name two different connection points",
        // WP-S2.04 part 5 (images, §11).
        "action-tool-image" => "Image…",
        "action-image-pick" => "Image asset",
        "action-image-file" => "Image file",
        "prompt-open-image-title" => "Insert an image",
        "error-image-file" => "Cannot read the image file {0}: {1}.",
        "error-image-too-large" => {
            "The image {0} is larger than {1} KiB (the limit of an image the project stores)."
        }
        "error-image-refused" => "The image {0} is not accepted: {1}.",
        "tool-image" => "Image",
        "hint-image-picking" => "Choose an image asset of the project first",
        "hint-image-armed" => "Click or Enter at the first corner; Escape ends",
        "hint-image-placing" => {
            "Click or Enter at the opposite corner; Backspace or Escape drops the first corner"
        }
        "reason-image-tool-only" => "Choose the Image tool first",
        "error-image-asset-missing" => "The project holds no image asset {0}.",
        // WP-S2.04 part 4b: the Wire marker tool, the Structure box tool and the marker mode.
        "submenu-wire-markers" => "Wire markers",
        "action-tool-marker" => "Wire marker",
        "action-marker-fields" => "Marker fields",
        "action-marker-csa" => "Cross-section",
        "action-marker-colours" => "Colours",
        "action-tool-structure-box" => "Structure box",
        "action-structure-box-fields" => "Structure box fields",
        "action-structure-box-style" => "Style",
        "action-structure-box-caption" => "Caption",
        "action-structure-box-location" => "Location",
        "action-project-marker-mode" => "Wire markers",
        "marker-mode-per_wire" => "As set per wire",
        "marker-mode-all" => "All wires",
        "marker-mode-deviations" => "Deviations",
        "marker-mode-explicit" => "Explicit overrides",
        "marker-mode-none" => "None",
        "box-style-dotted" => "Dotted",
        "box-style-dash_dot" => "Dash-dot",
        "tool-marker" => "Wire marker",
        "tool-structure_box" => "Structure box",
        "marker-crossed" => "{0} crossed",
        "hint-marker-armed" => "Press and drag (or Enter, move, Enter) across the wires to mark; Escape ends",
        "hint-marker-crossing" => "Release (or Enter) to take the crossed wires",
        "hint-marker-typing" => {
            "Set the cross-section and colours if wanted; Enter places the marker; Escape cancels"
        }
        "hint-structure-box-armed" => "Click or Enter at the first corner; Escape ends",
        "hint-structure-box-outline" => {
            "Click or Enter at the next corner; Shift+Enter or a double click closes the box (two corners: a rectangle)"
        }
        "hint-structure-box-typing" => {
            "Set the style, caption, location and external point; Enter places the box; Escape cancels"
        }
        "reason-marker-tool-only" => "Choose the Wire marker tool first",
        "reason-structure-box-tool-only" => "Choose the Structure box tool first",
        // WP-S2.04 part 4b. End.
        "tool-select" => "Select",
        "tool-measure" => "Measure",
        "hint-select" => "Click to select; drag for a rubber band",
        "hint-measure-armed" => "Click or Enter at the first point",
        "hint-measure-measuring" => "Click or Enter at the second point; Escape starts again",
        "measure-label" => "dx {0} dy {1} mm, {2} mm",
        "announce-tool" => "{0}: {1}",
        "announce-selected" => "{0} selected",
        "preview-chip" => "Preview of {0}",
        "preview-chip-stale" => "Preview of {0} (stale)",
        "preview-chip-failed" => "Preview of {0} failed: {1}",
        "preview-chip-change" => "{0} → {1}",
        "link-preview-title" => "{0}",
        // WP-S2.17 part 5 (FT-390): the link preview's accessible node.
        "link-preview-node" => "Link preview: page {0}, cell {1}",
        "link-preview-node-page" => "Link preview: page {0}",
        "canvas-cursor-live-cp" => ", connection point {0}",
        "reason-no-cp" => "The page has no connection point",
        "reason-no-unconnected-cp" => "The page has no unconnected connection point",
        "reason-no-cursor" => "No grid cursor is shown (the arrow keys show it)",
        "reason-no-preview" => "No link preview is shown",
        "action-file-open-recent" => "Open recent project",
        "action-file-save" => "Save",
        "action-file-save-as" => "Save as…",
        "action-file-export-pdf" => "Export PDF…",
        "action-file-close" => "Close project",
        "action-app-quit" => "Quit",
        "action-edit-undo" => "Undo",
        "action-edit-redo" => "Redo",
        "action-page-next" => "Next page",
        "action-page-previous" => "Previous page",
        "action-page-first" => "First page",
        "action-page-last" => "Last page",
        "action-page-goto" => "Go to page…",
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
        "action-view-theme" => "Theme",
        "action-view-density" => "Density",
        // WP-S2.17 part 2: the command bar (doc 16a §5).
        "action-view-command-tab" => "Command tab",
        "action-view-command-bar" => "Collapse command bar",
        "submenu-command-tab" => "Command tab",
        "group-project" => "Project",
        "group-parts" => "Parts",
        "group-standards" => "Standards",
        "group-import" => "Import",
        "group-pages" => "Pages",
        "group-order" => "Order",
        "group-go" => "Go",
        "group-select" => "Select",
        "group-place" => "Place",
        "group-connect" => "Connect",
        "group-annotate" => "Annotate",
        "group-arrange" => "Arrange",
        "group-run" => "Run",
        "group-findings" => "Findings",
        "group-numbering" => "Numbering",
        "group-trace" => "Trace",
        "group-pdf" => "PDF",
        "group-reports" => "Reports",
        "group-lists" => "Lists",
        "group-jobs" => "Jobs",
        "group-zoom" => "Zoom",
        "group-show" => "Show",
        "group-panels" => "Panels",
        "group-layout" => "Layout",
        "group-appearance" => "Appearance",
        "bar-properties" => "Properties",
        "bar-structure" => "Structure",
        "bar-import-parts" => "Import parts",
        "bar-company-standard" => "Company standard",
        "bar-import-qet" => "Import QET",
        "bar-move-up" => "Move up",
        "bar-move-down" => "Move down",
        "bar-go-to" => "Go to",
        "bar-follow-reference" => "Follow reference",
        "bar-device" => "Device",
        "bar-wire" => "Wire",
        "bar-interruption" => "Interruption",
        "bar-rotate" => "Rotate",
        "bar-mirror" => "Mirror",
        "bar-renumber" => "Renumber",
        "bar-check-project" => "Check project",
        "bar-navigators" => "Navigators",
        "bar-workspaces" => "Workspaces",
        "next-step-draw" => "Check project",
        "next-step-check" => "Export PDF",
        "commandbar-name" => "Command bar",
        "commandbar-tabs" => "Command tabs",
        "commandbar-app" => "ElektraWatt",
        "commandbar-menu" => "Menu",
        "commandbar-quick-access" => "Quick access",
        "commandbar-search" => "Search commands and devices",
        "commandbar-search-key" => "Ctrl K",
        "commandbar-more" => "More",
        "commandbar-next" => "Next step",
        "commandbar-variants" => "Variants",
        "hud-name" => "Tool",
        "hud-stack" => "{0} here \u{b7} click again for the next",
        "hud-selected" => "{0} selected \u{b7} Del deletes \u{b7} {1} rotates",
        "hud-keys-place" => "{0} rotates \u{b7} {1} variant \u{b7} Esc done",
        "hud-tool-options" => "Tool options",
        "zoom-control" => "Zoom",
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
        "reason-no-tag-proposal" => "No refused tag edit has a proposal to accept",
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
             editing; to recover them, reopen the project and choose Recover in the recovery dialog."
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
        "error-unknown-alias" => "No record has the alias `{0}` in this session.",
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
        "error-part-fndef-missing" => {
            "The part needs the function definitions {0}, which neither the project nor the starter library holds."
        }
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
        "start-open" => "Open…",
        "start-recent" => "Recent projects",
        "start-update-check" => "Update check: off",
        "pages-title" => "Pages",
        "page-rendering" => "Rendering page {0}…",
        "page-no-pages" => "The project has no pages.",

        // The status bar.
        "status-hint-start" => "No project open",
        "status-page" => "{0} ({1} of {2})",
        "status-saved" => "saved",
        "status-unsaved" => "*",
        "status-read-only" => "read-only: {0}",
        "status-language" => "EN",

        // The palette and the dialogs.
        "palette-title" => "Command palette",
        "palette-hint" => "Type a command…",
        "palette-no-match" => "No matching command",
        "dialog-close" => "Close",
        "dialog-save" => "Save",
        "dialog-discard" => "Discard",
        "dialog-cancel" => "Cancel",
        "dialog-go" => "Go",
        "confirm-title" => "Save changes to {0} before closing?",
        "dialog-dont-save" => "Don't save",
        "confirm-text" => "The project has unsaved changes.",
        "goto-title" => "Go to page or cell",
        "goto-hint" => "Folio or cell",
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
        "action-file-new" => "New project…",
        "action-file-open-sample" => "Open sample project",
        "action-file-import-qet" => "Import QElectroTech project…",
        "action-parts-import" => "Parts table…",
        "action-file-discard-journal" => "Discard unsaved changes of the earlier session…",
        "action-view-workspace-load" => "Load workspace",
        "action-view-workspace-save" => "Save layout as…",
        "action-view-workspace-reset" => "Reset layout",
        "action-tools-company-standard" => "Company standard wizard…",
        "action-help-documentation" => "Documentation",
        "action-help-tutorials" => "Tutorials",
        "action-help-first-run" => "First-run setup…",
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
        "reason-human-only-history" => {
            "Only a person undoes or redoes this step, in the window: it holds a human-only command that decides what the checks report (ADR-0008 amendment 3)"
        }
        "banner-check-scheme-applied" => "The check scheme {0} was applied to {1}.",
        "wizard-apply-check-scheme" => "Apply the check scheme",
        "wizard-kept-apply" => "kept: apply after finishing",
        "wizard-close" => "Close",
        "confirm-check-scheme-title" => "Apply the check scheme?",
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
        "theme-hc-dark" => "High contrast dark",
        "theme-hc-light" => "High contrast light",
        "density-comfortable" => "Comfortable",
        "density-compact" => "Compact",
        "origin-eplan" => "EPLAN",
        "origin-qet" => "QElectroTech",
        "origin-kicad" => "KiCad",
        "origin-other" => "Another tool",
        "pane-pages" => "Pages",
        "pane-page-view" => "Page view",
        // The property grid (WP-S2.05 part 1).
        "pane-properties" => "Properties",
        "action-props-show" => "Properties",
        "action-props-focus" => "Properties of selection",
        "action-props-project" => "Project properties",
        "action-select-set" => "Select records",
        "action-props-row" => "Move the property cursor",
        "action-props-category" => "Collapse or expand a property category",
        "action-props-edit" => "Edit the property",
        "action-props-input" => "Type into the property field",
        "action-props-commit" => "Commit the property",
        "action-props-cancel" => "Revert the property",
        "action-props-set" => "Set a property",
        "action-props-accept-tag-proposal" => "Accept the proposed tag",
        "action-props-choose" => "Choose",
        "action-props-row-menu" => "Property actions",
        "action-props-filter" => "Filter the properties",
        "props-filter-hint" => "Filter properties",
        "props-finding-chip" => "{0}, {1}: {2}",
        "props-finding-tip" => "{0}: {1}",
        "props-action-edit" => "Edit",
        "props-action-clear" => "Clear",
        "props-action-override" => "Override",
        "props-action-reset" => "Reset to derived",
        "props-action-freeze" => "Freeze",
        "props-action-unfreeze" => "Unfreeze",
        "action-props-action" => "Run a property action",
        "action-props-list" => "Change a list property",
        "action-props-mltext" => "Texts per language",
        "action-preview-apply" => "Apply the preview",
        "action-preview-cancel" => "Cancel the preview",
        "action-toast-undo" => "Undo the last edit",
        "action-toast-dismiss" => "Close the notification",
        "reason-no-sheet" => "The property grid shows nothing",
        "reason-no-row" => "No property row",
        "reason-not-editing" => "No property is being edited",
        "reason-no-choices" => "No choices are open",
        "reason-no-actions" => "The row has no actions",
        "reason-not-mltext" => "The row is not a multi-language text",
        "reason-no-card" => "No preview is open",
        "reason-no-toast" => "No notification is shown",
        "reason-changed-meanwhile" => "Changed meanwhile: edit again",
        "reason-bulk" => "More than 500 records",
        "reason-identifiers" => "Identifiers change",
        "reason-bulk-command" => "The command is previewed first",
        "reason-rebuilt" => "Rebuilt on the current values",
        "error-no-row" => "There is no property row {0}",
        "error-no-part" => {
            "The project has no part with the part number {0}: choose one of the project's parts from the list"
        }
        "props-ro-generated" => "generated by block '{0}'",
        "props-ro-identity" => "identity",
        "props-ro-action" => "changed with {0}",
        "props-ro-command" => "changed by {0}",
        "props-ro-human-only" => "human only: decides check outcomes; change it in {0}",
        "props-ro-no-command" => "no command edits this member yet",
        "props-refused-on" => "on {0}: {1}",
        "props-target-n" => "target {0}",
        "props-gone" => "The record no longer exists",
        "props-a-record" => "a record",
        "props-options" => "Options of {0}",
        "props-gone-title" => "Object no longer exists",
        "props-none" => "No common fields: the selection holds {0}",
        "props-empty" => "Nothing to show",
        "props-member-varies" => "The member differs between the selected records; set the whole value first",
        "props-cannot-compose" => "This value cannot be set here",
        "props-cannot-freeze" => "There is no number to freeze",
        "props-changed-meanwhile" => "changed meanwhile: edit again",
        "props-varies" => "‹varies› ({0})",
        "props-notes" => "Note: {0}",
        "props-items" => "{0} items",
        "props-row-name" => "{0}: {1}",
        "props-neutral" => "Neutral",
        "toast-set" => "Set {0} on {1} {2}",
        "toast-tag-proposal" => "Tag {0}: online numbering gave counter {1}",
        "props-tag-proposal" => "{0}. props.accept_tag_proposal takes {1}",
        "props-use-tag" => "Use {0}",
        "count-wire" => "{0} wire",
        "count-wires" => "{0} wires",
        "toast-strip-new" => {
            "Created terminal strip {0}; it has no terminals yet. Place them with Insert ▸ Symbol… and the terminal symbol, whose strip picker lists it"
        }
        "toast-export-pdf" => "Exported {0}: {1} pages, {2} KiB",
        "toast-structure-new" => "Created the {0} node {1}",
        "toast-external-ref-new" => {
            "Created the external reference {0}; the Interruption point tool links an external point to it by this label"
        }
        "toast-autoconnect" => {
            "Placed and connected {0}: {1}. Alt+Click or Alt+Enter places without connecting"
        }
        "toast-undo" => "Undo",
        "toast-close" => "Close",
        // WP-S2.09 part 1: the jobs, the notifications and the recovery dialog.
        "toast-open" => "Open",
        "toast-export-cancelled" => "Export cancelled; no file written",
        "toast-export-failed" => {
            "Export failed: {0}; check that the folder is writable and the file is not open in another program"
        }
        "toast-export-internal" => "Export failed: an internal error",
        "toast-recovered" => "Recovered {0} changes; save to keep them",
        "toast-recovered-one" => "Recovered 1 change; save to keep it",
        "marker-export" => "Exported PDF to {0}",
        "marker-recovered" => "Recovered {0} changes from the journal",
        "marker-recovered-one" => "Recovered 1 change from the journal",
        "action-jobs-show" => "Jobs",
        "action-jobs-cancel" => "Cancel job",
        "action-jobs-wait" => "Wait for job",
        "action-jobs-open-result" => "Open result",
        "action-notifications-show" => "Notifications",
        "action-notifications-run" => "Run notification action",
        "action-journal-recover" => "Recover",
        "action-journal-keep" => "Open read-only",
        "pane-jobs" => "Jobs",
        "reason-dialog-open" => "A dialog is open: answer it first",
        "reason-file-exists" => "The file exists; pass replace to overwrite it",
        "reason-export-running" => "An export is running",
        "reason-export-into-project" => {
            "The file would be inside the open project's folder; choose another folder"
        }
        "reason-no-running-job" => "No job is running",
        "reason-job-finished" => "The job has finished",
        "reason-no-result" => "No export has written a file",
        "reason-no-opener" => "The headless mode opens no file",
        "reason-action-expired" => "The action is no longer valid",
        "reason-no-toast-undo" => "The notification offers no Undo",
        "reason-no-recovery" => "No recovery is pending",
        "reason-recovery-failed" => "The recovery failed: open read-only or discard the changes",
        "error-unknown-notification" => "There is no notification {0}; the log holds {1}.",
        "error-no-such-action" => "The notification offers no {0} action.",
        "error-unknown-job" => "There is no job {0}; the jobs are {1}.",
        "error-no-thread" => "the job could not be started",
        "list-none" => "none",
        "list-and" => "and",
        "job-kind-pdf" => "PDF export",
        "job-cause-in-use" => "the file is in use",
        "job-cause-no-folder" => "the folder does not exist",
        "job-cause-denied" => "access to the folder or the file was denied",
        "job-cause-full" => "the disk is full",
        "job-cause-internal" => "an internal error",
        "job-phase-building" => "building pages",
        "job-phase-drawing" => "drawing pages",
        "job-phase-writing" => "writing the file",
        "job-as-of" => "as of rev {0}",
        "job-progress" => "page {0} of {1}",
        "job-running" => "running",
        "job-elapsed" => "{0} s",
        "job-cancel" => "Cancel",
        "job-done" => "{0} pages, {1} KiB",
        "job-now" => "exported rev {0}; the model is now at rev {1}",
        "job-cancelled" => "cancelled; no file written",
        "job-failed" => "failed: {0}",
        "jobs-empty" => "No jobs",
        "status-notifications" => "Notifications {0}",
        "status-job" => "{0} {1} %",
        "status-job-phase" => "{0}: {1}",
        "notifications-title" => "Notifications",
        "notifications-empty" => "No notifications",
        "recovery-title" => "Recover unsaved changes?",
        "recovery-text" => "{0} has unsaved changes from a session that ended unexpectedly.",
        "recovery-count" => "Unsaved changes: {0}",
        "recovery-last-change" => "Last change: {0}",
        "recovery-last-change-utc" => "{0} UTC",
        "elapsed-just-now" => "just now",
        "elapsed-minutes" => "{0} minutes ago",
        "elapsed-minutes-one" => "1 minute ago",
        "elapsed-hours" => "{0} hours ago",
        "elapsed-hours-one" => "1 hour ago",
        "elapsed-days" => "{0} days ago",
        "elapsed-days-one" => "1 day ago",
        "recovery-moved" => "The project changed on disk since.",
        "recovery-discard" => "Discard…",
        "recovery-failed-moved" => {
            "the project changed on disk in the same places; recovery needs the merge panel"
        }
        "recovery-failed-replay" => "the journal could not be replayed",
        "recovery-failed" => "Recover failed: {0}",
        "open-failed" => "The system could not open the file.",
        "card-line" => "{0} {1} · {2} changes · {3} tags, {4} cross-reference texts",
        "card-apply" => "Apply",
        "card-cancel" => "Cancel",
        "card-stale" => "stale",
        "card-failed" => "failed: {0}",
        // WP-S2.17 part 4: cards, badges, empty states and outcome badges (doc 16a §6.16, §6.17, §6.23).
        "card-strip-failed" => "{0} failed: {1}",
        "card-as-of" => "as of rev {0}",
        "badge-stale" => "Stale",
        "badge-failed" => "Failed",
        "badge-done" => "Done",
        "badge-cancelled" => "Cancelled",
        "card-title-reorder" => "Reorder pages",
        "card-title-fix" => "Quick fix {0}",
        "card-title-find" => "Replace",
        "card-title-properties" => "Preview of {0}",
        "card-title-pattern" => "Pattern fill",
        "empty-jobs" => "No jobs yet",
        "empty-jobs-text" => "Exports you start appear here with their progress.",
        "empty-preview-text" => {
            "Renumbering, a quick fix or an import shows its changes here before you apply them."
        }
        "empty-history-text" => "Your changes appear here; Undo steps back through them.",
        "empty-find-text" => "Nothing in the project matches what you searched for.",
        "empty-find-start" => "Find in the project",
        "empty-find-start-text" => "Type what to find; the matches appear here.",
        "empty-notifications-text" => "Exports, recoveries and edits of many objects report here.",
        "empty-messages" => "No findings",
        "empty-messages-text" => "Check project lists the errors and warnings of the project here.",
        // WP-S2.17 part 3/4 (lane K32): the navigators' empty states (doc 16a §6.17), the Messages severity words and
        // the row badges of Messages and History (doc 16a §6.24).
        "empty-nav-text" => "What you draw on the pages is listed here.",
        "empty-nav-pages-text" => "New page adds the first page of the project.",
        "empty-nav-filter" => "Nothing matches \"{0}\"",
        "empty-nav-filter-text" => "Change the filter or clear it to see every row.",
        "empty-nav-clear" => "Clear filter",
        "empty-nav-pages" => "No pages yet",
        "empty-nav-devices" => "No devices yet",
        "empty-nav-connections" => "No connections yet",
        "empty-nav-strips" => "No terminal strips yet",
        "empty-nav-cables" => "No cables yet",
        "empty-nav-plc" => "No PLC cards yet",
        "empty-nav-potentials" => "No potentials yet",
        "empty-nav-ips" => "No interruption points yet",
        "empty-nav-structure" => "No structure identifiers yet",
        "empty-nav-parts" => "No parts in use yet",
        "empty-nav-blocks" => "No report blocks yet",
        "severity-error" => "Error",
        "severity-warning" => "Warning",
        "severity-info" => "Info",
        "badge-pending" => "Pending",
        "badge-waived" => "Waived",
        "badge-fix" => "Fix",
        "badge-not-undoable" => "Not undoable",
        "toast-dismiss" => "Dismiss",
        "toast-name" => "Notification",
        // WP-S2.05 part 2: find and replace.
        "pane-find" => "Find and replace",
        "action-find-open" => "Find…",
        "action-find-open-replace" => "Find and replace…",
        "action-find-close" => "Close the find pane",
        "action-find-text" => "Type the search text",
        "action-find-replacement" => "Type the replacement",
        "action-find-option" => "Toggle a find option",
        "action-find-fields" => "Choose the fields to search",
        "action-find-folios" => "Filter the search by folio",
        "action-find-run" => "Find",
        "action-find-row" => "Move the result cursor",
        "action-find-jump" => "Show the result",
        // WP-S6.13 part 1
        "action-page-describe" => "Describe the page content",
        "action-find-replace-all" => "Replace all",
        "find-field-function-text" => "Function text",
        "find-field-tag" => "Tag",
        "find-field-property" => "Property",
        "find-field-text" => "Text",
        "find-field-title" => "Title",
        "find-field-description" => "Description",
        "find-field-process-tag" => "Process tag",
        "find-field-part-number" => "Part number",
        "find-text" => "Find",
        "find-replacement" => "Replace with",
        "find-regex" => "Regular expression",
        "find-match-case" => "Match case",
        "find-whole-word" => "Whole word",
        "find-in-selection" => "In selection",
        "find-folios" => "Folios (comma-separated)",
        "find-run" => "Find",
        "find-replace-all" => "Replace all",
        "find-close" => "Close",
        "find-results" => "{0} results",
        "find-no-results" => "No results",
        "find-truncated" => "More than 10,000 results: refine the search",
        "find-stale" => "stale",
        "find-replaced" => "Replaced {0}; run again to refresh",
        "find-updated-fewer" => "Preview updated: {0} fewer results",
        "find-updated-more" => "Preview updated: {0} more results",
        "find-updated-same" => "Preview updated: the same results",
        "find-not-replaced" => "not replaced: {0}",
        "find-card-line" => "{0} results on {1} records · {2} tags, {3} cross-reference texts",
        "reason-no-find" => "The find pane is closed",
        "reason-find-no-replace" => "Open Find and replace to replace",
        "reason-find-truncated" => "More than 10,000 results: refine the search before replacing",
        "reason-find-nothing" => "No result is replaced",
        "reason-find-no-results" => "No result is listed",
        "reason-find-no-selection" => "Nothing is selected",
        "reason-find-replace" => "A replace is previewed first",
        // FT-410: the parts table import.
        "reason-parts-import" => "An import is previewed first",
        "error-parts-import" => "Parts table refused: {0}",
        "banner-part-imported" => "Imported 1 part from {2} ({1} skipped).",
        "banner-parts-imported" => "Imported {0} parts from {2} ({1} skipped).",
        "parts-card-title" => "Import parts",
        "parts-card-line" => "{0}: {1} created, {2} skipped, {3} errors",
        "error-find-field" => "There is no find field {0}",
        "error-find-option" => "There is no find option {0}",
        // WP-S2.05 part 3: the clipboard.
        "action-edit-cut" => "Cut",
        "action-edit-copy" => "Copy",
        "action-edit-paste-clip" => "Paste",
        "action-edit-duplicate" => "Duplicate",
        "action-edit-copy-tags" => "Copy tag list",
        "action-clipboard-text" => "Set the clipboard text",
        "reason-cut-incomplete-items" => {
            "Cut would lose page items or marker connections; include every marker wire or use Copy"
        }
        "reason-copy-pages" => "The selection spans several pages: copy from one page",
        "reason-copy-generated" => "Generated content is not copied",
        "reason-clipboard-empty" => "The clipboard is empty",
        "reason-not-a-fragment" => "The clipboard holds no ElektraWatt fragment",
        "reason-paste-page-kind" => "Nothing can be pasted on a {0} page",
        "reason-no-tags" => "No device is selected",
        "tool-paste" => "Paste",
        "hint-paste" => "Click or Enter to paste; Escape ends",
        "label-paste" => "paste {0} records",
        "label-paste-dropped" => ", {0} connections dropped",
        "announce-copied" => "Copied {0} records",
        "banner-paste-refused" => "Not pasted: {0}",
        "mltext-replace" => "Replace the neutral text",
        "mltext-keep" => "Keep it",
        "chip-authored" => "= authored",
        "chip-inherited" => "^ inherited · {0}",
        "chip-derived" => "f derived · {0}",
        "chip-overridden" => "! overridden · {0}",
        "chip-frozen" => "* frozen · {0}",
        "chip-library" => "# library · {0}",
        "chip-generated" => "~ generated by block '{0}'",
        "chip-varies" => "≠ varies ({0})",
        "chip-word-page" => "page",
        "chip-word-inherited" => "inherited",
        "chip-word-derived" => "derived",
        "chip-word-override" => "override",
        "chip-word-frozen" => "frozen",
        "chip-word-library" => "library",
        "chip-word-generated" => "generated",
        "chip-word-varies" => "varies",
        "category-identity" => "Identity",
        "category-part" => "Part",
        "category-electrical" => "Electrical",
        "category-connection_points" => "Connection points",
        "category-texts" => "Texts",
        "category-structure" => "Structure",
        "category-placement" => "Placement",
        "category-terminal" => "Terminal",
        "category-cable" => "Cable",
        "category-plc" => "PLC",
        "category-assembly" => "Assembly",
        "category-other" => "Other",
        "category-extra" => "Extra",
        "dialog-save-workspace" => "Save layout",
        "save-workspace-title" => "Save layout as",
        "save-workspace-hint" => "Workspace name",
        "confirm-journal-title" => "Discard unsaved changes?",
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
            "PRIVACY.md lists every data flow and what is stored on this computer. This version makes no network \
             connection and sends nothing."
        }
        "about-privacy-button" => "Privacy (PRIVACY.md)",
        "privacy-open-failed" => "PRIVACY.md could not be opened in a browser; it is at {0}.",
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
        "prompt-open-parts-title" => "Import parts table",
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
        "wizard-hint-company" => "The panel builder's name, the notice and the logo for the title block.",
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
            "The report blocks of new projects, and their check scheme. The check scheme is kept in the draft: it decides what the release gate checks, so a person applies it after finishing, not the wizard's script."
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
        "wizard-field-company-logo" => "Logo file (SVG; PNG or JPEG print rasterised)",
        "wizard-field-report-blocks" => "Report blocks",
        "choice-colour-en60204-machine" => "EN 60204-1 machine (shipped, not yet verified)",
        "choice-colour-iec60445-installation" => "IEC 60445 installation (shipped, not yet verified)",
        "choice-report_blocks-golden" => {
            "The standard layout: TOC 2, parts 901, terminals 903, cables 906, wires 907, PLC 908, potentials 909, tags 910, revisions 911"
        }
        "choice-report_blocks-none" => "None",
        "wizard-error-logo" => "The logo must be a readable SVG, PNG or JPEG file of at most 512 KiB.",
        "wizard-error-logo-size" => {
            "The logo file is larger than 512 KiB; choose a smaller file (an SVG logo is usually far smaller)."
        }
        "wizard-error-logo-svg-editing" => {
            "The logo SVG holds editing data (entities or foreignObject). Save it again as a plain SVG: in \
             Illustrator with \"Preserve Illustrator Editing Capabilities\" off, in Inkscape as \"Plain SVG\", in \
             draw.io with \"Formatted Text\" and \"Word Wrap\" off."
        }
        "wizard-error-logo-svg-active" => {
            "The logo SVG holds a script or an event handler, which a drawing never runs. Remove it and export the \
             logo again as a plain SVG."
        }
        "wizard-error-logo-svg-external" => {
            "The logo SVG links or embeds other files (images, fonts, web addresses). Convert text to outlines, \
             remove linked or embedded images and export it again as a plain SVG; a raster logo can be chosen as its \
             own PNG or JPEG file."
        }
        "wizard-error-logo-svg-complex" => {
            "The logo SVG is too complex to draw safely (deep nesting, long chains of clones or markers, huge \
             numbers, transform-origin or convolution filters). Simplify it and export it again as a plain SVG with \
             presentation attributes rather than a style sheet."
        }
        "wizard-notice-logo-gone" => {
            "The logo file of the saved draft could not be read; choose the logo again."
        }
        "wizard-logo-raster" => "{0} (a raster image: it prints rasterised)",
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
        "wizard-create" => "Create the basic project…",
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

        // The viewer (WP-S2.03 part 1).
        "menu-window" => "Window",
        "submenu-page-navigator" => "Page navigator",
        "action-tab-new" => "New tab",
        "action-tab-close" => "Close tab",
        "action-tab-next" => "Next tab",
        "action-tab-previous" => "Previous tab",
        "action-tab-focus-other" => "Other tab group",
        "action-tab-activate" => "Show tab",
        "action-tab-move" => "Move tab",
        "action-tab-open" => "Open page in a new tab",
        "action-view-side-by-side" => "Side by side",
        "action-nav-goto" => "Go to page or cell…",
        "action-nav-back" => "Back",
        "action-nav-forward" => "Forward",
        "action-nav-follow" => "Follow cross-reference",
        "action-nav-next-link" => "Next cross-reference",
        "action-nav-previous-link" => "Previous cross-reference",
        "action-cursor-left" => "Cursor left",
        "action-cursor-right" => "Cursor right",
        "action-cursor-up" => "Cursor up",
        "action-cursor-down" => "Cursor down",
        "action-cursor-cell-left" => "Cursor one cell left",
        "action-cursor-cell-right" => "Cursor one cell right",
        "action-cursor-cell-up" => "Cursor one cell up",
        "action-cursor-cell-down" => "Cursor one cell down",
        "action-cursor-set" => "Set cursor",
        "action-pages-mode" => "Page navigator view",
        "action-pages-sort" => "Page navigator order",
        "action-pages-filter" => "Filter pages",
        "action-pages-toggle" => "Expand or collapse",
        "action-pages-row" => "Move in the page navigator",
        "action-pages-open" => "Open page",
        // WP-S2.06 part 1: the navigators.
        "action-navigator-show" => "Navigator",
        "action-navigator-view" => "Navigator view",
        "action-navigator-sort" => "Navigator order",
        "action-navigator-filter" => "Filter navigator",
        "action-navigator-toggle" => "Expand or collapse",
        "action-navigator-row" => "Move in the navigator",
        "action-navigator-open" => "Open row",
        "action-navigator-select" => "Mark rows",
        "action-navigator-mark" => "Toggle mark",
        "action-navigator-jump" => "Jump to row",
        "action-navigator-apply" => "Apply the preview",
        "action-pages-reorder" => "Reorder pages",
        "action-pages-move-up" => "Move page up",
        "action-pages-move-down" => "Move page down",
        // WP-S2.06 part 2: highlighting.
        "action-navigator-highlight" => "Highlight",
        "action-view-clear-highlights" => "Clear highlights",
        "reason-no-highlights" => "No potential or net is highlighted",
        "error-highlight-row" => "Only potentials and connections can be highlighted.",
        // WP-S2.06 part 3: table mode.
        "action-navigator-table" => "Table",
        "action-table-columns" => "Columns",
        "action-table-cell" => "Move the cell cursor",
        "action-table-edit" => "Edit cell",
        "action-table-input" => "Type into the cell",
        "action-table-commit" => "Commit the cell",
        "action-table-set" => "Set cells",
        "action-table-fill-down" => "Fill down",
        "action-table-pattern-fill" => "Pattern fill…",
        "action-table-paste" => "Paste cells",
        "action-table-copy" => "Copy cells",
        "action-table-clear" => "Clear cells",
        "action-table-resolve" => "Choose a reading",
        "action-table-drop" => "Drop a cell",
        "reason-table-off" => "Switch to the table first",
        "reason-table-not-editing" => "No cell is being edited",
        "reason-table-mark-fill" => "Mark the rows to fill",
        "reason-table-invalid" => "Some cells are invalid or ambiguous",
        "error-table-blocking" => "{0} cells are invalid or ambiguous; resolve or drop them first.",
        "error-table-blocking-one" => "1 cell is invalid or ambiguous; resolve or drop it first.",
        "error-table-read-only" => "{0} is read-only here: {1}.",
        "error-table-key-column" => "The key column {0} cannot be hidden.",
        "error-table-not-ambiguous" => "Cell {0} has no readings to choose from.",
        "table-nothing" => "No cell can be applied.",
        "table-no-row" => "no row",
        "table-no-column" => "no column",
        // FT-515/FT-535: a cell whose row has no record of the column's kind (a CP-only potential's definition cells).
        "table-no-record" => "no record of this kind",
        "table-define-alone" => "a potential without a definition is defined on its own row alone",
        // The other read-only reasons of a table cell (`cell_reason`, §8.2's precedence; FT-535 follow-up).
        "table-reason-derived" => "derived",
        "table-reason-generated" => "generated",
        "table-reason-no-command" => "no command yet",
        "table-reason-property-grid" => "edit in the property grid",
        "table-not-clearable" => "cannot be cleared",
        "card-table-drop" => "Drop",
        // WP-S2.06 part 3: the table widget (§8.1).
        "table-toggle" => "Table",
        "table-name" => "{0} table",
        "table-columns" => "Columns",
        "table-move-left-name" => "Move {0} left",
        "table-move-right-name" => "Move {0} right",
        "table-sort-by" => "Sort by {0}",
        // Doc 16a §6.21: the tooltip of a derived column's header icon.
        "table-derived" => "Derived; read-only",
        "table-cell-row" => ", row {0} of {1}",
        "table-cell-read-only" => ", read-only",
        "table-pattern-title" => "Pattern fill",
        "table-pattern-hint" => "Pattern",
        "table-pattern-values" => "Values: {0}",
        "table-resize" => "Resize {0} column",
        // FT-509: the tab group's menu of every tab.
        "dock-all-tabs" => "All tabs",
        // Doc 16a §5.7: the bottom dock's close button.
        "dock-close" => "Close the bottom dock",
        "submenu-navigators" => "Navigators",
        "nav-title-devices" => "Devices",
        "nav-title-connections" => "Connections",
        "nav-title-strips" => "Terminal strips",
        "nav-title-cables" => "Cables",
        "nav-title-plc" => "PLC",
        "nav-title-potentials" => "Potentials",
        "nav-title-ips" => "Interruption points",
        "nav-title-structure" => "Structure identifiers",
        "nav-title-parts" => "Parts in use",
        "nav-title-blocks" => "Report blocks",
        "nav-title-messages" => "Messages",
        // WP-S2.09 part 2: the message panel (§7.3, §7.6).
        "action-messages-show" => "Messages",
        "action-messages-next" => "Next finding",
        "action-messages-previous" => "Previous finding",
        "action-messages-fix" => "Apply quick fix…",
        "action-messages-check" => "Run checks",
        "reason-no-fix" => "The finding has no quick fix",
        "reason-no-findings" => "There are no findings to show",
        "reason-quick-fix" => "A quick fix is previewed first",
        "messages-header" => "{0} · E {1} · W {2} · I {3} · waived {4} · pending {5}",
        "messages-header-waived" => "waived {0}",
        "messages-header-pending" => "pending {0}",
        "messages-footer" => "Checks support, but do not replace, engineering verification.",
        "messages-detail-rule" => "{0} {1}",
        "messages-detail-kind" => "Kind: {0} · severity: {1}",
        "messages-detail-fix" => "Quick fix: {0}",
        "messages-fix-button" => "Fix…",
        "messages-rule-page" => "Rule page",
        "card-fix" => "{0} records, {1} tags, {2} cross-reference texts",
        "nav-tree-name" => "{0} navigator",
        "chips-label" => "Not on this page:",
        "chip-name" => "Open {0}",
        "card-reorder" => "{0} · {1} tags, {2} cross-reference texts",
        "error-nav-kind" => "Unknown navigator {0}; use one of {1}.",
        "error-nav-closed" => "The {0} navigator is not open; show it first (navigator.show).",
        "error-nav-key" => "No row {0} is on show in the {1} navigator.",
        "error-unknown-strip" => {
            "No terminal strip {0} in the project; its strips are {1}. A new strip's tag (-X??) takes its counter with its first terminal"
        }
        "error-nav-no-table" => "The {0} pane has no table mode; its rows are findings.",
        "error-nav-value" => "Unknown {0} {1}; use one of {2}.",
        "error-generated-page-order" => "A generated page has no folio of its own.",
        "error-pages-sections" => {
            "The marked pages are in different sections; move pages between sections by changing their section first."
        }
        "reason-no-pages-marked" => "No page record is marked",
        "reason-page-at-start" => "The page is the first of its section",
        "reason-page-at-end" => "The page is the last of its section",
        "reason-no-nav-row" => "No navigator row",
        "reason-no-navigator" => "No navigator has the focus",
        "action-ui-focus" => "Focus an area",
        "action-ui-focus-next" => "Next area",
        "action-ui-focus-previous" => "Previous area",
        "nav-mode-flat" => "Flat list",
        "nav-mode-section" => "By section",
        "nav-mode-structure" => "By structure",
        "nav-sort-folio" => "Sort by folio",
        "nav-sort-type" => "Sort by type",
        "nav-front-matter" => "Front matter",
        "nav-section" => "Section {0} - {1}",
        "nav-none" => "(none)",
        "nav-filter-hint" => "Filter",
        "badge-generated" => "generated",
        "badge-qet-grid" => "QET grid",
        "badge-current" => "current",
        "nav-page-name" => "{0} {1}, page{2}",
        "nav-node-name" => "{0}, {1}",
        "tab-title" => "{0} {1}",
        "tab-close-name" => "Close {0}",
        "reason-tabs-full" => "32 tabs are open",
        "reason-last-tab" => "The last tab stays open",
        "reason-one-tab" => "The tab group has one tab",
        "reason-one-group" => "Only one tab group is shown",
        "reason-history-empty" => "No page to return to",
        "reason-no-link" => "No cross-reference here",
        "reason-link-external" => "The cross-reference leads to another project",
        "error-goto-empty" => "Type a folio or a cell.",
        "error-goto-not-found" => "No page or cell matches \"{0}\".",
        "error-goto-no-cell" => "Page {0} has no cell {1}.",
        "status-cursor-cell" => "{0} · x {1} y {2} mm",
        "status-hint-link" => "Click or F: go to {0}",
        "status-hint-link-external" => "Link to another project",
        "canvas-cursor-description" => ", cursor {0} x {1} y {2} mm",
        "canvas-cursor-description-no-cell" => ", cursor x {0} y {1} mm",
        "canvas-cursor-name" => "Canvas cursor",
        "canvas-cursor-live" => "{0}, x {1} y {2} mm",
        "canvas-cursor-live-link" => ", link to {0}",
        // Creating pages, structure nodes and external references (FT-408, FT-409).
        "action-page-new" => "New page…",
        "action-structure-new" => "Structure node…",
        "action-external-ref-new" => "External reference…",
        "new-page-title" => "New page",
        "new-structure-title" => "New structure node",
        "new-external-title" => "New external reference",
        "nav-new-page" => "New page",
        "dialog-create" => "Create",
        "form-folio" => "Folio",
        "form-page-type" => "Type",
        "form-title" => "Title",
        "form-section" => "Section number",
        "form-kind" => "Kind",
        "form-designation" => "Designation",
        "form-parent" => "Parent (designation)",
        "form-location" => "Location (designation)",
        "form-device" => "Device",
        "form-connection" => "Connection",
        "form-project" => "Other project",
        "error-folio-number" => "A folio starts with its number: \"{0}\" does not.",
        "error-page-type" => "\"{0}\" is not a page type; use one of: {1}.",
        "error-structure-kind" => "\"{0}\" is not a structure kind; use one of: {1}.",
        "error-no-node" => "The project has no {0} node \"{1}\".",
        "error-ambiguous-node" => {
            "{0} nodes share the designation \"{1}\": {2}. Give the alias of one of them instead."
        }
        "node-candidate-under" => "{0} (under {1})",
        "node-candidate-top" => "{0} (top level)",
        "error-second-sign" => "\"{0}\" contains a second aspect sign {1}; give one designation",
        "error-wrong-sign" => {
            "\"{0}\" starts with {1}, the sign of the {2} aspect, but this node is in the {3} aspect."
        }
        // WP-S2.09 part 3: the Preview tab, the renumber dialogs, the history panel and the purge step.
        "pane-preview" => "Preview",
        "pane-history" => "History",
        "action-preview-show" => "Preview",
        "action-history-show" => "History",
        "action-history-row" => "Move in the history",
        "action-history-undo-to" => "Undo to here",
        "action-history-changes" => "Show changes",
        "action-pages-renumber" => "Renumber pages…",
        "action-devices-renumber" => "Renumber devices…",
        "action-project-purge-assets" => "Purge unused images…",
        "preview-none" => "No preview is open",
        "preview-header" => "Preview: {0} · as of rev {1}",
        "preview-title" => "Preview: {0}",
        "preview-stale" => "stale",
        "preview-summary" => "{0} records · {1} tags · {2} cross-reference texts · {3} pages",
        "preview-summary-import" => "{0} part copies created · {1} skipped",
        "preview-entities" => "Entities",
        "preview-tags" => "Tags",
        "preview-xrefs" => "Xrefs",
        "preview-pages" => "Pages",
        "preview-more" => "and {0} more",
        "preview-open-chip" => "Preview open: {0}",
        "preview-show" => "Show",
        "preview-apply" => "Apply",
        "preview-cancel" => "Cancel",
        "history-header" => "{0} steps · cursor at {1}",
        "history-empty" => "Nothing to undo yet",
        "history-not-undoable" => "not undoable",
        "history-merged" => "merged {0}",
        "history-changes-title" => "Changes of step {0}",
        "history-redo" => "undone",
        "history-redo-to" => "Redo to here",
        "marker-saved" => "Saved",
        "renumber-pages-title" => "Renumber pages",
        "renumber-devices-title" => "Renumber devices",
        "dialog-renumber" => "Renumber",
        "form-renumber-section" => "Section (number, or front)",
        "form-start" => "Start",
        "form-step" => "Step",
        "form-keep-gaps" => "Keep gaps",
        "form-scope" => "Scope",
        "form-scheme" => "Scheme",
        "form-include-frozen" => "Include frozen tags",
        "form-only-nonconforming" => "Only non-conforming",
        "error-no-section" => "The project has no pages in section {0}.",
        "error-no-front-matter" => "The project has no pages outside a section.",
        "error-unknown-entry" => "There is no history entry {0}; the history holds {1}.",
        "reason-no-devices-selected" => "No device is selected",
        "reason-history-closed" => "The history panel is closed",
        "reason-undo-history-empty" => "The history is empty",
        "reason-nothing-to-purge" => "No image is unused",
        "reason-no-purge-dialog" => "Open Purge unused images… first",
        "confirm-purge-title" => "Purge unused images?",
        "confirm-purge-text" => "These files are deleted and do not come back by undo.",
        "confirm-purge-more" => "and {0} more",
        "dialog-purge" => "Purge",
        "error-purge" => "The purge was refused: {0}",
        "purge-unreadable" => "an image file could not be read",
        // WP-S2.17 part 3: the rails, Properties' Main and Advanced, the status bar (doc 16a §5.4, §5.8, §6.8).
        "rail-project" => "Project",
        "rail-symbols" => "Symbols",
        "rail-parts" => "Parts",
        "rail-pages" => "Pages",
        "rail-devices" => "Devices",
        "rail-views" => "Pages or devices",
        "action-rail-show" => "Show navigator",
        "reason-rail-no-palettes" => "The palettes are not built yet; Symbols and Parts open their pickers",
        "category-main" => "Main",
        "action-props-show-advanced" => "Show advanced properties",
        "props-show-advanced" => "Show advanced ({0})",
        "props-hide-advanced" => "Hide advanced ({0})",
        "props-row-menu" => "Actions of {0}",
        "status-ready" => "Ready",
        "status-selected" => "{0} selected",
        "status-findings-errors" => "{0} errors",
        "status-findings-error" => "1 error",
        "status-findings-warnings" => "{0} warnings",
        "status-findings-warning" => "1 warning",
        "status-findings-infos" => "{0} infos",
        "status-findings-info" => "1 info",
        "status-renderer-why" => "The canvas draws on the CPU",
        "status-bar-saved" => "Saved",
        "table-sorted-ascending" => "{0}, sorted ascending",
        "table-sorted-descending" => "{0}, sorted descending",
        "props-empty-value" => "—",
        "status-bar-unsaved" => "Unsaved changes",
        "status-bar-read-only" => "Read-only",
        "splitter-name" => "Resize {0} and {1}",
        "history-header-kept" => "oldest kept: step {0} · {1} MB of {2} MB",
        _ => return None,
    })
}

/// The exact English labels of the property grid's rows (WP-S2.05 §6.1.2): the rows the pilot edits on folios 101 to
/// 103 (§6.12) whose humanised member name would not read well; every other row's label is its humanised last member.
const FIELD_LABELS: &[(&str, &str, &str)] = &[
    ("device", "tag.letter", "Tag letter"),
    ("device", "tag.counter", "Tag counter"),
    ("device", "tag.frozen", "Frozen tag"),
    ("device", "tech_data", "Technical data"),
    ("device", "derived.tag", "Tag"),
    ("device", "derived.main_location", "Main location"),
    ("connection", "derived.number", "Wire number"),
    // WP-S2.17 part 3 (D11): no two rows of a section share a label; an override is named as one.
    ("connection", "overrides.flags.shielded", "Shielded override"),
    ("connection", "overrides.flags.twisted", "Twisted override"),
    ("connection", "overrides.colour", "Colour override"),
    ("connection", "overrides.csa", "Cross-section override"),
    ("connection", "overrides.function", "Function override"),
    ("connection", "overrides.insulation", "Insulation override"),
    ("connection", "overrides.wire_part", "Wire part override"),
    ("connection", "overrides.length", "Length override"),
    ("connection", "derived.shielded", "Shielded"),
    ("connection", "derived.twisted", "Twisted"),
    ("device", "tag", "Tag numbering"),
    ("device", "parts[]", "Part use"),
    ("device", "derived.structure.function", "Effective function"),
    ("device", "derived.structure.location", "Effective location"),
    ("device", "terminal.strip", "Terminal strip"),
    ("page", "description_override", "Description override"),
    ("page", "grid", "Connection grid"),
    ("page", "derived.description", "Description"),
    ("representation", "derived.xref", "Cross-reference"),
    ("representation", "text_slots", "Text slots"),
];

/// The words the humanised labels replace (`csa` → "cross-section").
const LABEL_WORDS: &[(&str, &str)] = &[
    ("csa", "cross-section"),
    ("seq", "sequence"),
    ("qty", "quantity"),
    ("id", "ID"),
    ("plc", "PLC"),
    ("ip", "IP"),
    ("cp", "connection point"),
];

/// The English label of the field `path` of `kind` (list indices allowed): an exact entry of the table, else the last
/// member humanised (underscores to spaces, the word list applied, the first letter capitalised).
pub fn field_label(kind: &str, path: &str) -> String {
    let mut generic = String::with_capacity(path.len());
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '[' && chars.peek().is_some_and(char::is_ascii_digit) {
            for d in chars.by_ref() {
                if d == ']' {
                    break;
                }
            }
            generic.push_str("[]");
        } else {
            generic.push(c);
        }
    }
    // WP-S2.17 part 3: a list item carries its position (`Connection point 2`), so the items of a list differ.
    let item = path
        .strip_suffix(']')
        .and_then(|p| p.rsplit_once('['))
        .and_then(|(_, n)| n.parse::<usize>().ok())
        .map(|n| format!(" {}", n + 1))
        .unwrap_or_default();
    if let Some((_, _, label)) = FIELD_LABELS.iter().find(|(k, p, _)| *k == kind && *p == generic) {
        return format!("{label}{item}");
    }
    let last = generic
        .trim_end_matches("[]")
        .rsplit(['.', '['])
        .find(|s| !s.is_empty() && *s != "]")
        .unwrap_or(&generic)
        .trim_end_matches(']');
    let words: Vec<String> = last
        .split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            LABEL_WORDS
                .iter()
                .find(|(k, _)| *k == w)
                .map_or_else(|| w.to_owned(), |(_, v)| (*v).to_owned())
        })
        .collect();
    let text = words.join(" ");
    let mut out = String::with_capacity(text.len());
    let mut first = true;
    for c in text.chars() {
        if first {
            out.extend(c.to_uppercase());
            first = false;
        } else {
            out.push(c);
        }
    }
    out.push_str(&item);
    out
}

/// A record kind in words, singular (`potential definition`).
pub fn kind_label(kind: &str) -> String {
    match kind {
        "potential_def" => "potential definition".into(),
        "external_ref" => "external reference".into(),
        "plc_config_project" => "PLC configuration project".into(),
        other => other.replace('_', " "),
    }
}

/// A record kind in words, plural (`devices`, `potential definitions`).
pub fn kind_plural(kind: &str) -> String {
    let one = kind_label(kind);
    if one.ends_with("box") {
        format!("{one}es")
    } else {
        format!("{one}s")
    }
}

/// The text of `key`; the key itself for a key without a text (a missing text is a bug a test catches).
pub fn text(key: &str) -> &'static str {
    lookup(key).unwrap_or("?")
}

/// The text of `key` with `{0}`, `{1}`, … replaced by `args`.
pub fn format(key: &str, args: &[&str]) -> String {
    let mut out = text(key).to_owned();
    for (i, arg) in args.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), arg);
    }
    out
}

/// A count-dependent string: `<key>-one` for exactly one, `<key>` with the count as `{0}` otherwise.
pub fn format_count(key: &str, count: usize) -> String {
    if count == 1 {
        text(&format!("{key}-one")).to_owned()
    } else {
        format(key, &[&count.to_string()])
    }
}

/// The time from epoch milliseconds `last_ms` to `now_ms` in words (FT-503): `just now` under a minute (also for a
/// `last_ms` after `now_ms`, a clock skew, never a negative), then whole minutes, hours or days, rounded down.
pub fn elapsed_text(last_ms: u64, now_ms: u64) -> String {
    const MINUTE: u64 = 60_000;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    let span = now_ms.saturating_sub(last_ms);
    let count = |unit: u64| usize::try_from(span / unit).unwrap_or(usize::MAX);
    if span < MINUTE {
        text("elapsed-just-now").to_owned()
    } else if span < HOUR {
        format_count("elapsed-minutes", count(MINUTE))
    } else if span < DAY {
        format_count("elapsed-hours", count(HOUR))
    } else {
        format_count("elapsed-days", count(DAY))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_793_000_000_000;

    #[test]
    fn elapsed_text_is_just_now_under_a_minute() {
        assert_eq!(elapsed_text(NOW, NOW), "just now");
        assert_eq!(elapsed_text(NOW - 59_999, NOW), "just now");
    }

    #[test]
    fn elapsed_text_counts_minutes() {
        assert_eq!(elapsed_text(NOW - 60_000, NOW), "1 minute ago");
        assert_eq!(elapsed_text(NOW - 12 * 60_000 - 5, NOW), "12 minutes ago");
        assert_eq!(elapsed_text(NOW - 3_599_999, NOW), "59 minutes ago");
    }

    #[test]
    fn elapsed_text_counts_hours() {
        assert_eq!(elapsed_text(NOW - 3_600_000, NOW), "1 hour ago");
        assert_eq!(elapsed_text(NOW - 5 * 3_600_000 - 1, NOW), "5 hours ago");
    }

    #[test]
    fn elapsed_text_counts_days() {
        assert_eq!(elapsed_text(NOW - 86_400_000, NOW), "1 day ago");
        assert_eq!(elapsed_text(NOW - 40 * 86_400_000, NOW), "40 days ago");
        assert_eq!(elapsed_text(0, NOW), "20752 days ago");
    }

    #[test]
    fn elapsed_text_of_a_future_last_change_is_just_now() {
        assert_eq!(elapsed_text(NOW + 1, NOW), "just now");
        assert_eq!(elapsed_text(NOW + 86_400_000, NOW), "just now");
    }

    #[test]
    fn format_count_is_singular_for_one_and_plural_otherwise() {
        assert_eq!(
            format_count("toast-recovered", 1),
            "Recovered 1 change; save to keep it"
        );
        assert_eq!(
            format_count("toast-recovered", 3),
            "Recovered 3 changes; save to keep them"
        );
        assert_eq!(
            format_count("toast-recovered", 0),
            "Recovered 0 changes; save to keep them"
        );
        assert_eq!(
            format_count("marker-recovered", 1),
            "Recovered 1 change from the journal"
        );
        assert_eq!(
            format_count("marker-recovered", 2),
            "Recovered 2 changes from the journal"
        );
    }

    #[test]
    fn placeholders_are_filled() {
        assert_eq!(format("status-page", &["103", "3", "36"]), "103 (3 of 36)");
        assert_eq!(
            format("window-title", &["J-E-2416.ew"]),
            "J-E-2416.ew - ElektraWatt"
        );
    }

    #[test]
    fn field_labels_are_exact_or_humanised() {
        assert_eq!(field_label("device", "function_text"), "Function text");
        assert_eq!(field_label("device", "parts[0].part"), "Part");
        assert_eq!(field_label("device", "parts[1].qty"), "Quantity");
        assert_eq!(
            field_label("connection", "overrides.csa"),
            "Cross-section override"
        );
        assert_eq!(field_label("connection", "derived.csa"), "Cross-section");
        assert_eq!(field_label("device", "tag.counter"), "Tag counter");
        assert_eq!(field_label("project", "settings.checks"), "Checks");
        assert_eq!(field_label("device", "plc_module"), "PLC module");
        assert_eq!(
            field_label("device", "derived.structure.location"),
            "Effective location"
        );
        assert_eq!(field_label("device", "parts[1]"), "Part use 2");
        assert_eq!(field_label("device", "derived.cp[0]"), "Connection point 1");
        assert_eq!(kind_plural("device"), "devices");
        assert_eq!(kind_plural("potential_def"), "potential definitions");
        assert_eq!(kind_plural("structure_box"), "structure boxes");
    }

    #[test]
    fn an_unknown_key_has_no_text() {
        assert_eq!(lookup("no-such-key"), None);
        assert_eq!(text("no-such-key"), "?");
    }
}
