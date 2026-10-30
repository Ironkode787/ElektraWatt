// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `ui.describe` text (WP-S2.01 part 1, spec section 6.2): the visible user interface as text, one line per
//! element, in paint order. It is what an agent reads instead of a screenshot.
//!
//! Paint order: the window title, the app menu with every entry and its enabled state (and reason), the command bar
//! (WP-S2.17 part 2), the banner,
//! the first-run flow, the wizard, or the project view (the page navigator, the canvas tabs and the focus, the page
//! view and its canvas lines, the workspace and its layout), or the start page, the status bar, then the palette or a dialog, then the last documentation request.
//! Directory names appear, paths never. The structural words are fixed English machine text (decision log ID-474):
//! [`VOCABULARY`] lists every one of them, [`FREE_TEXT_MARKERS`] the markers after which the rest of a line is user
//! text, and a test checks that no line the describer writes uses a word that is in neither list. Quoted text is
//! user text (a label, a name) and not part of the vocabulary. A record ID is printed as its alias `@n`, a value token
//! like a number (decision log ID-630, FT-422; [`crate::alias`]): the hovered entity and the selection unquoted, an
//! ID inside a quoted text inside the quotes; no UUID reaches the text.

use ew_api::output::{PageTables, TableCp, TableEnd, TableRep};

use crate::actions::{self, ActionItem, MenuEntry, PaletteRun};
use crate::canvas::PageId;
use crate::canvas::{CanvasPage, mm3, round_tenth};
use crate::first_run::FirstRunStep;
use crate::navigator::RowKind;
use crate::properties::{Provenance, RowValue};
use crate::settings::{KEYMAPS, LANGUAGES, THEMES};
use crate::shell::{Dialog, Focus, ShellModel};
use crate::strings;
use crate::wizard::{FieldKind, Step, fields_of};

/// The structural words of the describe text: the tokens of a line once the quoted texts, the numbers, the ID aliases
/// `@n` (value tokens like numbers, ID-630), the action IDs (they contain a dot), the shortcuts and the punctuation
/// `( ) [ ] , | %` are taken out. A token with a colon
/// keeps it. A change here is a reviewed transcript change (decision log ID-474).
pub const VOCABULARY: &[&str] = &[
    "title:",
    "menu",
    "submenu",
    "item",
    "enabled",
    "disabled",
    "disabled:",
    "checked",
    "banner:",
    "page",
    "selected",
    "view:",
    "workspace",
    "layout:",
    "split",
    "tabs",
    "horizontal",
    "vertical",
    "shown",
    "status",
    "bar:",
    "start",
    "button",
    "label",
    "recent",
    "quick",
    "options:",
    "first",
    "run",
    "step",
    "of",
    "choice",
    "on",
    "off",
    "wizard",
    "field",
    "review",
    "written",
    "kept",
    "error:",
    "preview",
    "ready",
    "unavailable",
    "palette",
    "query",
    "action",
    "command",
    "dialog",
    "pending",
    "notices:",
    "lines",
    "help:",
    // The canvas lines (WP-S2.02 part 1, decision log ID-511): a reviewed vocabulary change.
    "canvas",
    "zoom",
    "view",
    "grid",
    "pointer",
    "hover",
    "box",
    "mm",
    "none",
    "backend",
    "statistics",
    // The canvas renderer setting (WP-S2.02 part 2): a reviewed vocabulary change.
    "renderer",
    "auto",
    "cpu",
    // The viewer (WP-S2.03 part 1, decision log ID-537): 27 words, a reviewed vocabulary change.
    "navigator",
    "mode",
    "flat",
    "section",
    "structure",
    "sort",
    "folio",
    "type",
    "filter",
    "row",
    "level",
    "expanded",
    "collapsed",
    "current",
    "generated",
    "badge",
    "cursor",
    "tab",
    "group",
    "focused",
    "history",
    "back",
    "forward",
    "focus",
    "cell",
    "link",
    "external",
    // The tool framework (WP-S2.03 part 2, decision log ID-537): 18 words, a reviewed vocabulary change (§7.9's 17
    // and `junction`).
    "tool",
    "state",
    "idle",
    "pressed",
    "rubber-band",
    "armed",
    "measuring",
    "hint",
    "snap",
    "cp",
    "at",
    "selection",
    "stale",
    "failed",
    "tags",
    "xrefs",
    "overlay",
    // A snap marker's kind (`canvas snap junction …`), printed by `MarkerKind::word` (the review of part 2).
    "junction",
    // Place and arrange (WP-S2.04 part 1, §7.7): 28 words, a reviewed vocabulary change (ID-474 extended).
    "picker",
    "rows",
    "symbol",
    "part",
    "function",
    "main",
    "embedded",
    "starter",
    "ghost",
    "place",
    "variant",
    "rep",
    "target",
    "new",
    "join",
    "existing",
    "terminal",
    "strip",
    "letter",
    "tag",
    "move",
    "steps",
    "exchange",
    "unmapped",
    "floating",
    "dragging",
    "moving",
    "readout",
    // The property grid (WP-S2.05 part 1, §6.11; ID-474 extended): 53 words, a reviewed vocabulary change
    // (`function` is WP-S2.04 part 1's word above).
    "properties",
    "items",
    "kinds",
    "subtitle",
    "location",
    "finding",
    "info",
    "warning",
    "error",
    "missing",
    "targets",
    "category",
    "derived",
    "authored",
    "inherited",
    "overridden",
    "frozen",
    "library",
    "varies",
    "read-only",
    "actions",
    "edit",
    "clear",
    "override",
    "reset",
    "freeze",
    "unfreeze",
    "editing",
    "buffer",
    "choices",
    "popup",
    "mltext",
    "card",
    "changes",
    "toast",
    "undo",
    "device",
    "representation",
    "connection",
    "project",
    "plc_config_project",
    "import_lineage",
    "potential_def",
    "structure_node",
    "external_ref",
    "conductor",
    "channel",
    "bond",
    "device_link",
    "report_block",
    "revision",
    "page_item",
    "structure_box",
    // WP-S1.16 part 5: the typed waiver kind has a property sheet (read-only members); a reviewed vocabulary change.
    "waiver",
    // WP-S1.21 part 4 (D11): the typed bus nets have property sheets; a reviewed vocabulary change.
    "logical_net",
    "physical_net",
    // Find and replace (WP-S2.05 part 2, §7.6; ID-474 extended): 13 words and `text`, which no earlier part added,
    // so 14, a reviewed vocabulary change.
    "find",
    "replace",
    "regex",
    "case",
    "word",
    "scope",
    "function_text",
    "property",
    "part_number",
    "folios",
    "results",
    "result",
    "truncated",
    "text",
    // Wiring (WP-S2.04 part 2, §8.7; ID-474 extended): 21 words, a reviewed vocabulary change (`junction` is WP-S2.03
    // part 2's word above, ID-632).
    "route",
    "from",
    "end",
    "bends",
    "points",
    "proposals",
    "->",
    "style",
    "wiring",
    "sequence",
    "wire",
    "segment",
    "routing",
    "crossings",
    "plain",
    "hops",
    "dot",
    "bevel",
    "star",
    "chain",
    "connect",
    // The page text of `page.describe` (WP-S6.13 part 1, §5.5.3; ID-474 extended): 11 words, a reviewed vocabulary
    // change (`wire`, `->`, `route`, `bends`, `style`, `dot`, `bevel`, `wiring`, `star`, `chain` and `sequence` are
    // WP-S2.04 part 2's words above).
    "reps",
    "connections",
    "junctions",
    "wires",
    "stored",
    "provisional",
    "ip",
    "direction",
    "source",
    "undirected",
    "potential",
    // G-PP triage row 21: a terminal CP's side, `internal` (`field` is older), a reviewed vocabulary change.
    "internal",
    // FT-621 (WP-S2.07 part 2): the page text's line of a strip's bridge, a reviewed vocabulary change.
    "bridge",
    // The clipboard (WP-S2.05 part 3, §8.8; ID-474 extended): the 6 words of `CLIP_WORDS`, a reviewed vocabulary change
    // (`lines`, `none`, `at`, `mm`, `text` and `ghost` are older).
    "clipboard",
    "fragment",
    "records",
    "copies",
    "paste",
    "dropped",
    // Interruption points and black boxes (WP-S2.04 part 3, §9.6; ID-474 extended): 5 words, a reviewed vocabulary
    // change (`ip`, `direction`, `source`, `undirected` and `potential` are WP-S6.13 part 1's words above).
    "name",
    "outline",
    "cps",
    "arrange",
    "by",
    // The parts table import's card (FT-410): 3 words, a reviewed vocabulary change.
    "created",
    "skipped",
    "errors",
    // The parts import's column mapping step (FT-649): 5 words, a reviewed vocabulary change.
    "mapping",
    "saved",
    "column",
    "sample",
    "member",
    // Page items and structure boxes (WP-S2.04 part 4b, §10.7; ID-474 extended): 14 words, a reviewed vocabulary
    // change.
    "graphic",
    "line",
    "polyline",
    "rect",
    "circle",
    "arc",
    "polygon",
    "marker",
    "typing",
    "drawing",
    "crossing",
    "anticlockwise",
    "boxes",
    "unknown",
    // The Image tool (WP-S2.04 part 5, §11; ID-474 extended): 1 word, a reviewed vocabulary change.
    "image",
    // The navigators (WP-S2.06 part 1, §6.10; ID-474 extended, ID-688): the 18 words of `NAV_WORDS`, a reviewed
    // vocabulary change (`connections` and `structure` are older).
    "kind",
    "pages",
    "devices",
    "strips",
    "cables",
    "plc",
    "potentials",
    "ips",
    "parts",
    "blocks",
    "tree",
    "ascending",
    "descending",
    "showing",
    "to",
    "elsewhere",
    "aux",
    "reorder",
    // The further navigators and highlighting (WP-S2.06 part 2, §7.4; ID-474 extended, ID-688): the 6 words of
    // `NAV2_WORDS`, a reviewed vocabulary change.
    "cable",
    "module",
    "station",
    "manufacturer",
    "highlight",
    "slot",
    // The jobs, the notifications log and the recovery dialog (WP-S2.09 part 1, §6.8; ID-474 extended, ID-698): the 18
    // words of `JOB_WORDS`, a reviewed vocabulary change (`pages` is WP-S2.06 part 1's, `failed`, `undo` and `toast` are
    // older).
    "jobs",
    "job",
    "running",
    "pdf",
    "done",
    "kib",
    "rev",
    "now",
    "cancelled",
    "notifications",
    "notification",
    "open",
    "expired",
    "recovery",
    "transactions",
    "base",
    "matches",
    "moved",
    // The message panel (WP-S2.09 part 2, §7.6; ID-474 extended, ID-698): the 16 words of `MSG_WORDS`, a reviewed
    // vocabulary change (`finding`, `error`, `warning`, `info`, `errors` and `pending` are older).
    "messages",
    "severity",
    "rule",
    "scheme",
    "warnings",
    "infos",
    "waived",
    "detail",
    "title",
    "message",
    "fix",
    "completeness",
    "safety",
    "conflict",
    "hygiene",
    "integrity",
    // The Preview tab and the history panel (WP-S2.09 part 3, §8.7; ID-474 extended): the 13 words of `PREVIEW_WORDS`,
    // a reviewed vocabulary change (the specification's 14 less `saved`: a marker's text is quoted).
    "summary",
    "entity",
    "added",
    "changed",
    "removed",
    "fields",
    "xref",
    "more",
    "entries",
    "entry",
    "origin",
    "merged",
    "redo",
    // Table mode (WP-S2.06 part 3, §8.8; ID-474 extended): the 6 words of `TABLE_WORDS`, a reviewed vocabulary change.
    "table",
    "columns",
    "invalid",
    "ambiguous",
    "pattern",
    "values",
    // The command bar and the HUD (WP-S2.17 part 2, D15; ID-474 extended): the 4 words of `COMMAND_BAR_WORDS`, a
    // reviewed vocabulary change (`tab`, `collapsed`, `group`, `step`, `tool` and `target` are older).
    "commandbar",
    "next",
    "hud",
    "prompt",
    // The rails and the Advanced rows (WP-S2.17 part 3, D15; ID-474 extended): the 6 words of `RAIL_WORDS`, a reviewed
    // vocabulary change (`shown`, `collapsed` and `history` are older).
    "rail",
    "left",
    "right",
    "foot",
    "advanced",
    "hidden",
    // WP-S2.07 part 1 (§6.10): the strip editor's accessory rows.
    "accessory",
    // WP-S1.19 part 1: the kind `macro_instance` is implemented, so the property grid has a catalogue for it and a
    // section of it names the kind.
    "macro_instance",
    // WP-S2.18 part 1 (D2): the Join chooser's line (doc 16a §6.22), a reviewed vocabulary change.
    "chooser",
    // FT-660: a generated page's table in `page.describe` (`table`, `columns`, `rows`, `row` and `group` are older).
    "note",
    // WP-S1.21 part 2 (D7): the printed PLC address of the hovered connection point, a reviewed vocabulary change.
    "address",
    // WP-S1.19 part 6 (§11): the 6 words of `LIBRARY_WORDS`, a reviewed vocabulary change (`library` and `conflict` are
    // older).
    "macros",
    "libraries",
    "tier",
    "project_copies",
    "library_copy",
    "macro",
    // WP-S2.07 part 4a (§9.6): the 12 words of `PART_WORDS`, a reviewed vocabulary change.
    "fits",
    "all",
    "inactive",
    "compatible",
    "same-fndef",
    "same-category",
    "other",
    "favourite",
    "favourites",
    "tech",
    "version",
    "fork",
];

// WP-S2.07 part 4a
/// The 12 words WP-S2.07 part 4a adds to [`VOCABULARY`] (§9.6): the part dialog's `query` line (`fits`, `all`,
/// `inactive`, the views `favourites` and `all`), its `result` lines (`compatible`, the fits `same-fndef`,
/// `same-category` and `other`, the mark `favourite`) and the part editor's `tech`, `source … version … fork` lines.
pub const PART_WORDS: &[&str] = &[
    "fits",
    "all",
    "inactive",
    "compatible",
    "same-fndef",
    "same-category",
    "other",
    "favourite",
    "favourites",
    "tech",
    "version",
    "fork",
];

// WP-S1.19 part 6
/// The 6 words WP-S1.19 part 6 adds to [`VOCABULARY`] (§11): the navigator kinds `macros` and `libraries`, the row
/// roles `tier`, `project_copies`, `library_copy` and `macro` (also the macro card's origin word).
pub const LIBRARY_WORDS: &[&str] = &[
    "macros",
    "libraries",
    "tier",
    "project_copies",
    "library_copy",
    "macro",
];

/// The word FT-660 adds to [`VOCABULARY`]: the `note` line of a generated page whose table prints a note instead of
/// rows (see [`page_table_lines`]).
pub const LIST_WORDS: &[&str] = &["note"];

// WP-S2.07 part 1
/// The word WP-S2.07 part 1 adds to [`VOCABULARY`] (§6.10): an accessory row of the strip editor prints as `accessory`
/// with its cells, where a terminal row prints `row`.
pub const STRIP_EDITOR_WORDS: &[&str] = &["accessory"];

// WP-S2.17 part 3
/// The 6 words WP-S2.17 part 3 adds to [`VOCABULARY`] (D15): the `rail left` and `rail right` lines with the left rail's
/// `foot`, and the property grid's `advanced <n> shown|hidden` line.
pub const RAIL_WORDS: &[&str] = &["rail", "left", "right", "foot", "advanced", "hidden"];

// WP-S2.17 part 2
/// The 4 words WP-S2.17 part 2 adds to [`VOCABULARY`] (D15): the `commandbar` section's head word and its `next step`
/// line, the `hud` line and its `prompt`.
pub const COMMAND_BAR_WORDS: &[&str] = &["commandbar", "next", "hud", "prompt"];

// WP-S2.09 part 2
/// The 16 words WP-S2.09 part 2 adds to [`VOCABULARY`] (§7.6): the kind word `messages`, the role words `severity` and
/// `rule`, the header line's `scheme`, `warnings`, `infos` and `waived`, the detail lines' `detail`, `title`, `message`
/// and `fix` (also the quick-fix card's origin word), and the five rule kinds.
pub const MSG_WORDS: &[&str] = &[
    "messages",
    "severity",
    "rule",
    "scheme",
    "warnings",
    "infos",
    "waived",
    "detail",
    "title",
    "message",
    "fix",
    "completeness",
    "safety",
    "conflict",
    "hygiene",
    "integrity",
];

// WP-S2.09 part 3
/// The 13 words WP-S2.09 part 3 adds to [`VOCABULARY`] (§8.7): the `preview` and `history` sections' words (the
/// specification's 14 less `saved`, which only a quoted marker text carries).
pub const PREVIEW_WORDS: &[&str] = &[
    "summary", "entity", "added", "changed", "removed", "fields", "xref", "more", "entries", "entry",
    "origin", "merged", "redo",
];

// WP-S2.06 part 3
/// The 6 words WP-S2.06 part 3 adds to [`VOCABULARY`] (§8.8): the header's `table`, the `columns` line, the table
/// card's `invalid` and `ambiguous` cells, and the pattern dialog's `pattern` and `values`.
pub const TABLE_WORDS: &[&str] = &["table", "columns", "invalid", "ambiguous", "pattern", "values"];

// WP-S2.09 part 1
/// The 18 words WP-S2.09 part 1 adds to [`VOCABULARY`] (§6.8): the `jobs` and `notifications` sections, the toast's
/// action words and the recovery dialog's line (the specification's 19 tokens less `pages`, WP-S2.06 part 1's).
pub const JOB_WORDS: &[&str] = &[
    "jobs",
    "job",
    "running",
    "pdf",
    "done",
    "kib",
    "rev",
    "now",
    "cancelled",
    "notifications",
    "notification",
    "open",
    "expired",
    "recovery",
    "transactions",
    "base",
    "matches",
    "moved",
];

// WP-S2.06 part 2
/// The 6 words WP-S2.06 part 2 adds to [`VOCABULARY`] (§7.4): the role words `cable`, `module`, `station` and
/// `manufacturer`, and the highlight lines' `highlight` and `slot`.
pub const NAV2_WORDS: &[&str] = &["cable", "module", "station", "manufacturer", "highlight", "slot"];

// WP-S2.06 part 1
/// The 18 words WP-S2.06 part 1 adds to [`VOCABULARY`] (§6.10): the navigator kinds, the header's `kind`, `tree`,
/// `ascending`, `descending`, `showing` and `to`, the canvas section's `elsewhere`, the badge word `aux` and the card's
/// origin word `reorder`.
pub const NAV_WORDS: &[&str] = &[
    "kind",
    "pages",
    "devices",
    "strips",
    "cables",
    "plc",
    "potentials",
    "ips",
    "parts",
    "blocks",
    "tree",
    "ascending",
    "descending",
    "showing",
    "to",
    "elsewhere",
    "aux",
    "reorder",
];

/// The most rows a navigator prints in the describe text (§6.10): a window around the cursor.
pub const NAV_WINDOW: usize = 50;

/// The 10 words of WP-S2.04 §9.6, of which part 3 adds the last 5 to [`VOCABULARY`] (WP-S6.13 part 1 added the
/// first 5): `arrange`, `by` and `potential` name the parameters of `ip.arrange_selected` (FT-314's harness help
/// lists them); the canvas lines print the others.
pub const IP_WORDS: &[&str] = &[
    "ip",
    "direction",
    "source",
    "undirected",
    "potential",
    "name",
    "outline",
    "cps",
    "arrange",
    "by",
];

// WP-S2.05 part 3
/// The 6 words WP-S2.05 part 3 adds to [`VOCABULARY`] (§8.8): the `clipboard` line and the paste ghost's line.
pub const CLIP_WORDS: &[&str] = &["clipboard", "fragment", "records", "copies", "paste", "dropped"];

/// The 3 words FT-410 adds to [`VOCABULARY`]: the import card's line `card ready "parts.import" created 29 skipped 0
/// errors 0`.
pub const IMPORT_WORDS: &[&str] = &["created", "skipped", "errors"];

/// The 5 words FT-649 adds to [`VOCABULARY`]: the column mapping step's lines `card mapping "list.csv" source "list"
/// saved` and `column "Artikel" sample "A9F74210" member "part_number"`.
pub const MAP_WORDS: &[&str] = &["mapping", "saved", "column", "sample", "member"];

/// The 14 words of WP-S2.04 part 4b (§10.7 as ruled in cycle 28): its 11 tool words (`text` is WP-S2.05 part 2's and
/// `connections` WP-S6.13 part 1's), `anticlockwise` (an arc drawn anticlockwise) and `page.describe`'s `boxes` and
/// `unknown` (WP-S6.13 §5.5.3).
pub const ITEM_WORDS: &[&str] = &[
    "graphic",
    "line",
    "polyline",
    "rect",
    "circle",
    "arc",
    "polygon",
    "marker",
    "typing",
    "drawing",
    "crossing",
    "anticlockwise",
    "boxes",
    "unknown",
];

/// The word WP-S2.04 part 5 adds to [`VOCABULARY`] (§11): the Image tool's ghost line and `page.describe`'s image item.
pub const IMAGE_WORDS: &[&str] = &["image"];

/// The 14 words WP-S2.05 part 2 adds to [`VOCABULARY`] (§7.6).
pub const FIND_WORDS: &[&str] = &[
    "find",
    "replace",
    "regex",
    "case",
    "word",
    "scope",
    "function_text",
    "property",
    "part_number",
    "folios",
    "results",
    "result",
    "truncated",
    "text",
];

/// The 21 words WP-S2.04 part 2 adds to [`VOCABULARY`] (§8.7, ID-632: `junction` is a word since WP-S2.03 part 2).
pub const WIRE_WORDS: &[&str] = &[
    "route",
    "from",
    "end",
    "bends",
    "points",
    "proposals",
    "->",
    "style",
    "wiring",
    "sequence",
    "wire",
    "segment",
    "routing",
    "crossings",
    "plain",
    "hops",
    "dot",
    "bevel",
    "star",
    "chain",
    "connect",
];

/// The 28 words WP-S2.04 part 1 adds to [`VOCABULARY`] (§7.7).
pub const PLACE_WORDS: &[&str] = &[
    "picker", "rows", "symbol", "part", "function", "main", "embedded", "starter", "ghost", "place",
    "variant", "rep", "target", "new", "join", "existing", "terminal", "strip", "letter", "tag", "move",
    "steps", "exchange", "unmapped", "floating", "dragging", "moving", "readout",
];

/// The 11 words `page.describe` adds to [`VOCABULARY`] (WP-S6.13 §5.5.3): its own `reps`, `junctions`, `wires`,
/// `stored` and `provisional`, the interruption point's `ip`, `direction`, `source`, `undirected` and `potential` (WP-S2.04
/// part 3's words, which Golden A prints) and `connections` (part 4b's). The other words of its lines (`page`, `rep`,
/// `cell`, `symbol`, `variant`, `at`, `mm`, `cp`, `none`, `wire`, `->`, `route`, `bends`, `junction`, `style`, `dot`,
/// `bevel`, `wiring`, `star`, `chain`, `sequence`) are older. The G-PP triage added a terminal CP's `internal` and
/// FT-621 a strip's `bridge` line.
pub const PAGE_WORDS: &[&str] = &[
    "reps",
    "connections",
    "junctions",
    "wires",
    "stored",
    "provisional",
    "ip",
    "direction",
    "source",
    "undirected",
    "potential",
    "internal",
    "bridge",
];

/// The markers after which the rest of the line is user text (a title, a banner, an error, a reason, a folio, a
/// status field, a menu's name), wherever they stand in the line.
pub const FREE_TEXT_MARKERS: &[&str] = &[
    "title:",
    "banner:",
    "error:",
    "help:",
    "view:",
    "bar:",
    "disabled:",
    "menu",
];

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

/// `"a" | "b"`: the options of a choice, each quoted as user text.
fn quoted_list(items: impl Iterator<Item = String>) -> String {
    items.map(|i| format!("\"{i}\"")).collect::<Vec<_>>().join(" | ")
}

fn quick_label(prefix: &str, value: &str) -> String {
    strings::text(&format!("{prefix}-{value}")).to_owned()
}

fn start_page(model: &ShellModel, lines: &mut Vec<String>) {
    let button = |key: &str| format!("  button \"{}\"", strings::text(key));
    lines.push(format!("start page \"{}\"", strings::text("start-title")));
    lines.push(button("start-new"));
    lines.push(format!(
        "  label \"{}\"",
        match model.default_template_name() {
            Some(name) => strings::format("start-template", &[&name]),
            None => strings::text("start-template-builtin").to_owned(),
        }
    ));
    lines.push(button("start-open"));
    lines.push(format!("  label \"{}\"", strings::text("start-recent")));
    for (i, name) in model.recent_names().iter().enumerate() {
        lines.push(format!("  recent {} \"{name}\"", i + 1));
    }
    lines.push(button("start-sample"));
    lines.push(button("start-import-qet"));
    lines.push(button("start-company-standard"));
    if model.default_template_name().is_some() {
        lines.push(button("start-edit-standard"));
    }
    lines.push(button("start-tutorials"));
    let settings = model.settings();
    lines.push(format!(
        "  quick \"{}\": \"{}\" (options: {})",
        strings::text("start-quick-language"),
        quick_label("language", &settings.language),
        quoted_list(LANGUAGES.iter().map(|l| quick_label("language", l)))
    ));
    lines.push(format!(
        "  quick \"{}\": \"{}\" (options: {})",
        strings::text("start-quick-keymap"),
        quick_label("keymap", &settings.keymap),
        quoted_list(KEYMAPS.iter().map(|k| quick_label("keymap", k)))
    ));
    let privacy = if settings.update_check {
        strings::text("start-update-check-on")
    } else {
        strings::text("start-update-check")
    };
    lines.push(format!("  label \"{privacy}\""));
    lines.push(button("start-privacy-change"));
}

fn first_run(model: &ShellModel, lines: &mut Vec<String>) {
    let Some(run) = model.first_run() else { return };
    let s = model.settings();
    lines.push(format!(
        "first run \"{}\": step {} of {} \"{}\"",
        strings::text("firstrun-title"),
        run.step.index() + 1,
        FirstRunStep::ALL.len(),
        strings::text(run.step.title_key())
    ));
    let text_key = match run.step {
        FirstRunStep::Language => "firstrun-text-language",
        FirstRunStep::Origin => "firstrun-text-origin",
        FirstRunStep::Privacy => "firstrun-text-privacy",
        FirstRunStep::Appearance => "firstrun-text-appearance",
        FirstRunStep::Start => "firstrun-text-start",
    };
    lines.push(format!("  label \"{}\"", strings::text(text_key)));
    match run.step {
        FirstRunStep::Language => lines.push(format!(
            "  choice \"{}\": \"{}\"",
            strings::text("firstrun-step-language"),
            quick_label("language", &s.language)
        )),
        FirstRunStep::Origin => lines.push(format!(
            "  choice \"{}\": \"{}\"",
            strings::text("firstrun-step-origin"),
            if s.comes_from.is_empty() {
                "-".to_owned()
            } else {
                quick_label("origin", &s.comes_from)
            }
        )),
        FirstRunStep::Privacy => {
            lines.push(format!(
                "  choice \"{}\": {}",
                strings::text("firstrun-update-check"),
                if s.update_check { "on" } else { "off" }
            ));
            lines.push(format!("  button \"{}\"", strings::text("about-privacy-button")));
        }
        FirstRunStep::Appearance => {
            lines.push(format!(
                "  choice \"{}\": \"{}\" (options: {})",
                strings::text("firstrun-theme"),
                quick_label("theme", &s.theme),
                quoted_list(THEMES.iter().map(|t| quick_label("theme", t)))
            ));
            lines.push(format!(
                "  choice \"{}\": {}%",
                strings::text("firstrun-scale"),
                (s.ui_scale * 100.0).round()
            ));
        }
        FirstRunStep::Start => {
            for key in [
                "firstrun-start-wizard",
                "firstrun-start-sample",
                "firstrun-start-qet",
                "firstrun-start-new",
                "firstrun-start-none",
            ] {
                lines.push(format!("  button \"{}\"", strings::text(key)));
            }
        }
    }
    if run.step != FirstRunStep::Start {
        lines.push(format!("  button \"{}\"", strings::text("firstrun-skip")));
        lines.push(format!("  button \"{}\"", strings::text("firstrun-next")));
    }
    if run.step != FirstRunStep::Language {
        lines.push(format!("  button \"{}\"", strings::text("firstrun-back")));
    }
}

fn wizard(model: &ShellModel, lines: &mut Vec<String>) {
    let Some(w) = model.wizard() else { return };
    let step = w.step();
    lines.push(format!(
        "wizard \"{}\": step {} of {} \"{}\" (#{})",
        strings::text("wizard-title"),
        step.index() + 1,
        Step::ALL.len(),
        strings::text(step.title_key()),
        step.number()
    ));
    lines.push(format!("  label \"{}\"", strings::text(step.hint_key())));
    if step == Step::Review {
        for line in w.review() {
            lines.push(format!(
                "  review \"{}\": \"{}\" {}",
                line.label,
                line.value,
                if line.written { "written" } else { "kept" }
            ));
        }
    } else {
        for f in fields_of(step) {
            let label = strings::text(f.label_key);
            match f.kind {
                FieldKind::Choice(choices) => {
                    let options = choices
                        .iter()
                        .map(|c| strings::text(&format!("choice-{}-{c}", f.key)).to_owned());
                    lines.push(format!(
                        "  choice \"{label}\": \"{}\" (options: {})",
                        w.draft().display_value(f),
                        quoted_list(options)
                    ));
                }
                FieldKind::Text | FieldKind::Multiline => {
                    lines.push(format!("  field \"{label}\": \"{}\"", w.draft().display_value(f)));
                }
            }
        }
    }
    if let Some(error) = w.error() {
        lines.push(format!("  error: {error}"));
    }
    let preview = match w.preview_content() {
        Ok(_) => "ready".to_owned(),
        Err(why) => format!("unavailable \"{why}\""),
    };
    lines.push(format!(
        "  preview \"{}\": {preview}",
        strings::text(if w.preview_is_cover() {
            "wizard-preview-cover"
        } else {
            "wizard-preview-page"
        })
    ));
    if w.finished_dir().is_some() {
        // FT-336: the basic project is written; what is left is the confirmed check scheme, then Close.
        if w.can_apply_check_scheme() {
            lines.push(format!(
                "  button \"{}\"",
                strings::text("wizard-apply-check-scheme")
            ));
        }
        lines.push(format!("  button \"{}\"", strings::text("wizard-close")));
        return;
    }
    if step != Step::Company {
        lines.push(format!("  button \"{}\"", strings::text("wizard-back")));
    }
    if step == Step::Review {
        lines.push(format!("  button \"{}\"", strings::text("wizard-create")));
    } else {
        lines.push(format!("  button \"{}\"", strings::text("wizard-next")));
    }
    lines.push(format!("  button \"{}\"", strings::text("wizard-reset")));
    lines.push(format!("  button \"{}\"", strings::text("wizard-cancel")));
}

/// The first row of the describe window (§6.10): `clamp(cursor − 10, 0, max(0, n − 50))`.
fn window_start(n: usize, cursor: Option<usize>) -> usize {
    cursor
        .unwrap_or(0)
        .saturating_sub(10)
        .min(n.saturating_sub(NAV_WINDOW))
}

/// The navigator lines (WP-S2.03 §6.10): the header, then one line per row on show; with more than 50 rows (WP-S2.06
/// §6.10) the header ends with `showing <a> to <b>` and only the window around the cursor is printed.
fn navigator_lines(model: &ShellModel, lines: &mut Vec<String>) {
    let nav = model.navigator();
    let all = model.navigator_rows();
    let cursor = model.navigator_row_key();
    let at = cursor.as_ref().and_then(|c| all.iter().position(|r| &r.key == c));
    let start = window_start(all.len(), at);
    let end = (start + NAV_WINDOW).min(all.len());
    let showing = if all.len() > NAV_WINDOW {
        format!(" showing {} to {end}", start + 1)
    } else {
        String::new()
    };
    lines.push(format!(
        "navigator \"{}\" mode {} sort {} filter \"{}\"{showing}",
        strings::text("pages-title"),
        nav.mode.word(),
        nav.sort.word(),
        nav.filter
    ));
    let rows = model.page_rows();
    let current = model.current_page();
    for row in all.into_iter().skip(start).take(NAV_WINDOW) {
        let mut line = format!("  row level {}", row.level);
        match &row.kind {
            RowKind::Node { label, .. } => {
                line.push_str(&format!(
                    " \"{label}\" {}",
                    if row.expanded { "expanded" } else { "collapsed" }
                ));
            }
            RowKind::Page(i) => {
                let r = &rows[*i];
                line.push_str(&format!(" page \"{}\" \"{}\"", r.folio, r.title));
                if r.generated {
                    line.push_str(" generated");
                }
                if r.qet_grid {
                    line.push_str(&format!(" badge \"{}\"", strings::text("badge-qet-grid")));
                }
                // WP-S1.18 part 2 (D7): the "Changed in" column, the revision that last changed the page.
                if let Some(label) = &r.changed_in {
                    line.push_str(&format!(" changed \"{label}\""));
                }
                if current == Some(PageId::of(r)) {
                    line.push_str(" current");
                }
            }
        }
        // WP-S2.06 part 1: a marked row (D5: no Pages line changes while nothing is marked).
        if nav.is_marked(&row.key) {
            line.push_str(" selected");
        }
        if cursor.as_deref() == Some(row.key.as_str()) {
            line.push_str(" cursor");
        }
        lines.push(line);
    }
}

// WP-S2.06 part 1
/// The lines of every open navigator other than Pages (§6.10): the header with the kind, the view, the sort, the
/// filter, the row count and the window, then one line per row of the window around the cursor.
fn other_navigator_lines(model: &ShellModel, lines: &mut Vec<String>) {
    let pages = crate::navigators::NavKind::Pages;
    let kinds = model
        .nav(pages)
        .map(|_| pages)
        .into_iter()
        .chain(model.open_navigators());
    for kind in kinds {
        let Some(nav) = model.nav(kind) else { continue };
        let rows = model.shown_rows(kind);
        let cursor = model.nav_cursor(kind);
        let at = cursor
            .as_ref()
            .and_then(|c| rows.iter().position(|r| &r.key == c));
        let start = window_start(rows.len(), at);
        let end = (start + NAV_WINDOW).min(rows.len());
        lines.push(format!(
            "navigator \"{}\" kind {} view {}{} sort \"{}\" {} filter \"{}\" rows {} showing {} to {end}",
            strings::text(crate::layout::nav_title_key(kind)),
            kind.word(),
            nav.view.word(),
            if nav.table_mode().is_some() { " table" } else { "" },
            user(&nav.sort_label()),
            if nav.descending { "descending" } else { "ascending" },
            user(&nav.filter),
            rows.len(),
            if rows.is_empty() { 0 } else { start + 1 },
        ));
        // WP-S2.09 part 2: the Messages pane's header line (§7.6).
        if kind == crate::navigators::NavKind::Messages
            && let Some(c) = model.messages_counts()
        {
            lines.push(format!(
                "  messages scheme \"{}\" errors {} warnings {} infos {} waived {} pending {}",
                user(model.messages_scheme().unwrap_or_default()),
                c.errors,
                c.warnings,
                c.infos,
                c.waived,
                c.pending
            ));
        }
        let table = nav.table();
        // WP-S2.06 part 3: the table's lines (§8.8).
        if let (Some(mode), Some(t)) = (nav.table_mode(), table) {
            table_lines(
                model,
                kind,
                nav,
                mode,
                t,
                &rows[start..end],
                cursor.as_deref(),
                lines,
            );
            continue;
        }
        for r in rows.iter().skip(start).take(NAV_WINDOW) {
            let mut line = format!(
                "  row level {} {} \"{}\"",
                r.level,
                r.role,
                user(&label_of(table, &r.key))
            );
            if let Some(n) = table.and_then(|t| t.nodes.iter().find(|n| n.key == r.key)) {
                for b in &n.badges {
                    line.push_str(&format!(" badge \"{}\"", user(b)));
                }
            }
            // WP-S2.09 part 2: a pending finding's badge follows its others.
            if r.finding.is_some_and(|f| f.pending) {
                line.push_str(" badge \"pending\"");
            }
            match r.expanded {
                Some(true) => line.push_str(" expanded"),
                Some(false) => line.push_str(" collapsed"),
                None => {}
            }
            if r.marked {
                line.push_str(" selected");
            }
            if cursor.as_deref() == Some(r.key.as_str()) {
                line.push_str(" cursor");
            }
            // WP-S2.06 part 2: a highlighted row ends with its slot.
            if let Some(slot) = model.row_highlight_slot(kind, &r.key) {
                line.push_str(&format!(" highlight {slot}"));
            }
            lines.push(line);
        }
        // WP-S2.09 part 2: the detail lines of the cursor's finding (§7.6).
        if kind == crate::navigators::NavKind::Messages
            && let Some(d) = model.messages_detail()
        {
            lines.push(format!(
                "  detail rule \"{}\" title \"{}\" kind {} severity {}",
                user(&d.rule),
                user(&d.title),
                d.kind,
                d.severity
            ));
            lines.push(format!("  detail message \"{}\"", user(&d.message)));
            if let Some(fix) = &d.fix {
                lines.push(format!("  detail fix \"{}\"", user(fix)));
            }
            // FT-674: a waived finding (Unwaive in the window).
            if d.waived {
                lines.push("  detail waived".to_owned());
            }
        }
    }
}

// WP-S2.06 part 3
/// A navigator's table (§8.8): the `columns` line of the column labels on show, one `row` line per row of the window
/// with its cells on show quoted (` selected`, ` cursor "<column label>"`, the highlight slot), then the cell edit
/// (`editing "<column label>" buffer "<text>"`), its error, and the pattern dialog (`pattern "<pattern>" values
/// "<text>" …` with the first five texts, or `invalid "<message>"`).
#[allow(clippy::too_many_arguments)]
fn table_lines(
    model: &ShellModel,
    kind: crate::navigators::NavKind,
    nav: &crate::navigators::Navigator,
    mode: &crate::navigators::table::TableMode,
    table: &crate::navigators::NavTable,
    rows: &[crate::shell::ShownRow],
    cursor: Option<&str>,
    lines: &mut Vec<String>,
) {
    let shown = mode.shown_columns(table);
    let quoted = |cols: &mut dyn Iterator<Item = &str>| -> String {
        cols.map(|c| format!(" \"{}\"", user(c))).collect::<String>()
    };
    lines.push(format!(
        "  columns{}",
        quoted(&mut shown.iter().map(|i| table.columns[*i].label.as_str()))
    ));
    let cursor_col = mode.cursor_column(table).map(|c| table.columns[c].label.clone());
    for r in rows {
        let Some(node) = table.nodes.iter().find(|n| n.key == r.key) else {
            continue;
        };
        // WP-S2.07 §6.10: an accessory row of the strip editor has its own row word.
        let word = if node.role == crate::navigators::NavRole::Accessory {
            "accessory"
        } else {
            "row"
        };
        let mut line = format!(
            "  {word}{}",
            quoted(
                &mut shown
                    .iter()
                    .map(|i| node.cells.get(*i).map_or("", String::as_str))
            )
        );
        if nav.marked().contains(&r.key) {
            line.push_str(" selected");
        }
        if cursor == Some(r.key.as_str())
            && let Some(c) = &cursor_col
        {
            line.push_str(&format!(" cursor \"{}\"", user(c)));
        }
        if let Some(slot) = model.row_highlight_slot(kind, &r.key) {
            line.push_str(&format!(" highlight {slot}"));
        }
        lines.push(line);
    }
    if let Some(e) = mode.edit() {
        let label = table
            .columns
            .iter()
            .find(|c| c.key == e.column)
            .map_or(e.column.as_str(), |c| c.label.as_str());
        lines.push(format!(
            "  editing \"{}\" buffer \"{}\"",
            user(label),
            user(&e.buffer)
        ));
    }
    if let Some((pattern, values)) = model.pattern_preview(kind) {
        match values {
            Ok(v) => lines.push(format!(
                "  pattern \"{}\" values{}",
                user(&pattern),
                quoted(&mut v.iter().map(String::as_str))
            )),
            Err(message) => lines.push(format!(
                "  pattern \"{}\" invalid \"{}\"",
                user(&pattern),
                user(&message)
            )),
        }
    }
    if let Some(message) = mode.error() {
        lines.push(format!("  error \"{}\"", user(message)));
    }
}

/// The label of a row of a navigator table.
fn label_of(table: Option<&std::sync::Arc<crate::navigators::NavTable>>, key: &str) -> String {
    table
        .and_then(|t| t.nodes.iter().find(|n| n.key == key))
        .map(|n| n.label.clone())
        .unwrap_or_default()
}

/// The tab lines (WP-S2.03 §6.10): per group its header, then one line per tab; the selected tab with its history.
fn tab_lines(model: &ShellModel, lines: &mut Vec<String>) {
    let Some(area) = model.area() else { return };
    let n = area.groups.len();
    for (g, group) in area.groups.iter().enumerate() {
        let focused = if g == area.focused { " focused" } else { "" };
        lines.push(format!("tabs group {} of {n}{focused}", g + 1));
        for (i, tab) in group.tabs.iter().enumerate() {
            let (folio, title) = model.tab_row(tab).map_or((String::new(), String::new()), |r| {
                (r.folio.clone(), r.title.clone())
            });
            let mut line = format!("  tab \"{folio}\" \"{title}\"");
            if i == group.active {
                line.push_str(&format!(
                    " selected history back {} forward {}",
                    tab.history.back_len(),
                    tab.history.forward_len()
                ));
            }
            lines.push(line);
        }
    }
    match model.focus() {
        Some(Focus::Navigator(crate::navigators::NavKind::Pages)) => {
            lines.push("focus navigator".to_owned());
        }
        // WP-S2.06 part 1
        Some(Focus::Navigator(k)) => lines.push(format!("focus navigator kind {}", k.word())),
        Some(Focus::Canvas(g)) => lines.push(format!("focus canvas group {}", g + 1)),
        Some(Focus::Properties) => lines.push("focus properties".to_owned()),
        // WP-S2.05 part 2
        Some(Focus::Find) => lines.push("focus find".to_owned()),
        // WP-S2.17 part 2
        Some(Focus::CommandBar) => lines.push("focus commandbar".to_owned()),
        // WP-S2.17 part 3 (FT-519)
        Some(Focus::History) => lines.push("focus history".to_owned()),
        None => {}
    }
}

/// A user text of the property grid's lines as the describe text quotes it: a double quote becomes a single one (the
/// quoting stays unambiguous). A UUID in it is printed as its alias `@n` with the rest of the text (FT-422).
fn user(text: &str) -> String {
    text.replace('"', "'")
}

/// The provenance of a row in the describe text: its word and quoted chip text (UUIDs left out, §9 item 5).
fn provenance_words(p: &Provenance) -> String {
    match p {
        Provenance::Authored => "authored".to_owned(),
        Provenance::Inherited { from } => format!("inherited \"{}\"", user(from)),
        Provenance::Derived { via } => format!("derived \"{}\"", user(via)),
        Provenance::Overridden { note } => format!("overridden \"{}\"", user(note)),
        Provenance::Frozen { note } => format!("frozen \"{}\"", user(note)),
        // `<library>/<item> v<version>`: the version only, the IDs are UUIDs.
        Provenance::Library { item } => format!(
            "library \"{}\"",
            user(item.rsplit_once(' ').map_or(item.as_str(), |(_, v)| v))
        ),
        Provenance::Generated { block } => format!("generated \"{}\"", user(block)),
        Provenance::Varies { .. } => String::new(),
    }
}

/// The property grid's lines (WP-S2.05 §6.11), after the navigator's; none while the pane is closed.
fn properties_lines(model: &ShellModel, lines: &mut Vec<String>) {
    let props = model.properties();
    let (true, Some(sheet)) = (model.properties_shown(), props.sheet()) else {
        return;
    };
    let kinds: Vec<String> = sheet
        .header
        .kinds
        .iter()
        .map(|(k, n)| format!("{k} {n}"))
        .collect();
    let kinds = if kinds.is_empty() {
        String::new()
    } else {
        format!(" kinds {}", kinds.join(" "))
    };
    lines.push(format!(
        "properties \"{}\" items {}{kinds}",
        user(&sheet.header.title),
        sheet.header.items
    ));
    match (&sheet.header.subtitle, &sheet.header.location) {
        (Some(s), Some(l)) => lines.push(format!(
            "properties subtitle \"{}\" location \"{}\"",
            user(s),
            user(l)
        )),
        (Some(s), None) => lines.push(format!("properties subtitle \"{}\"", user(s))),
        (None, Some(l)) => lines.push(format!("properties location \"{}\"", user(l))),
        (None, None) => {}
    }
    for f in &sheet.header.findings {
        lines.push(format!(
            "properties finding {} \"{}\" \"{}\"",
            f.severity,
            user(&f.rule),
            user(&f.message)
        ));
    }
    if let Some(r) = &sheet.read_only {
        lines.push(format!("properties read-only \"{}\"", user(r)));
    }
    if sheet.missing > 0 {
        lines.push(format!("properties missing {}", sheet.missing));
    }
    if sheet.sections.is_empty() {
        let reason = if sheet.missing > 0 && sheet.header.items == 0 {
            strings::text("props-gone-title").to_owned()
        } else if sheet.header.items == 0 {
            strings::text("props-empty").to_owned()
        } else {
            let kinds: Vec<String> = sheet.header.kinds.iter().map(|(k, _)| k.clone()).collect();
            strings::format("props-none", &[&kinds.join(", ")])
        };
        lines.push(format!("properties none \"{}\"", user(&reason)));
    }
    let collapsed = model.properties_collapsed();
    for section in &sheet.sections {
        lines.push(format!(
            "  section {} targets {}",
            section.kind,
            section.targets.len()
        ));
        // WP-S2.17 part 3 (D11): Main first, the Advanced rows only while they are shown.
        for (group, rows) in crate::properties::groups(section, model.properties_show_advanced()) {
            let key = crate::properties::group_key(&section.kind, group);
            let shut = collapsed.contains(&key);
            lines.push(format!(
                "  category \"{}\" {}",
                group.title(),
                if shut { "collapsed" } else { "expanded" }
            ));
            if shut {
                continue;
            }
            for row in rows {
                lines.push(row_line(sheet, section, row));
            }
        }
    }
    let advanced = crate::properties::advanced_count(sheet);
    if advanced > 0 {
        let state = if model.properties_show_advanced() {
            "shown"
        } else {
            "hidden"
        };
        lines.push(format!("  advanced {advanced} {state}"));
    }
    if let Some(row) = model.properties_row() {
        lines.push(format!("  focus row \"{}\"", user(&row)));
    }
    if let Some(e) = props.edit() {
        lines.push(format!(
            "  editing \"{}\" buffer \"{}\"",
            user(&e.row),
            user(&e.buffer)
        ));
    }
    if let Some((row, message)) = props.error() {
        lines.push(format!("  error \"{}\" \"{}\"", user(row), user(message)));
    }
    if let Some(c) = props.choices() {
        let texts: Vec<String> = c.items.iter().map(|(t, _)| format!("\"{}\"", user(t))).collect();
        // The highlighted choice (Up, Down) by its index: `selected <i>`.
        let highlight = c.index.map_or_else(String::new, |i| format!(" selected {i}"));
        lines.push(format!(
            "  choices \"{}\" {} {}{highlight}",
            user(&c.row),
            c.items.len(),
            texts.join(" ")
        ));
    }
    if let Some(p) = props.popup() {
        lines.push(format!("  popup {} \"{}\"", p.word(), user(p.row())));
    }
}

// WP-S2.05 part 2
/// The find pane's lines (§7.6), after the property grid's; none while the pane is closed.
fn find_lines(model: &ShellModel, lines: &mut Vec<String>) {
    if !model.find_shown() {
        return;
    }
    let f = model.find_state();
    let onoff = |b: bool| if b { "on" } else { "off" };
    let (regex, case, word, selection) = f.options();
    let mut line = format!("find \"{}\"", user(f.text()));
    if f.replace_mode() {
        line.push_str(&format!(" replace \"{}\"", user(f.replacement())));
    }
    line.push_str(&format!(
        " regex {} case {} word {}",
        onoff(regex),
        onoff(case),
        onoff(word)
    ));
    if selection {
        line.push_str(" selection on");
    }
    let scope: Vec<&str> = f.fields().iter().map(|x| crate::find::field_word(*x)).collect();
    line.push_str(&format!(" scope {}", scope.join(" ")));
    if !f.folios().is_empty() {
        let folios: Vec<String> = f.folios().iter().map(|x| format!("\"{}\"", user(x))).collect();
        line.push_str(&format!(" folios {}", folios.join(" ")));
    }
    let results = f.results();
    if let Some(r) = results {
        line.push_str(&format!(" results {}", r.hits.len()));
        if f.stale(model.revision()) {
            line.push_str(" stale");
        }
    }
    lines.push(line);
    if let Some(r) = results {
        if let Some(e) = &r.error {
            lines.push(format!("find error \"{}\"", user(e)));
        }
        if r.truncated {
            lines.push("find truncated".to_owned());
        }
        for (i, hit) in r.hits.iter().enumerate() {
            let mut l = format!("  result {}", i + 1);
            if let Some(folio) = &hit.folio {
                l.push_str(&format!(" folio \"{}\"", user(folio)));
            }
            if let Some(cell) = &hit.cell {
                l.push_str(&format!(" cell \"{}\"", user(cell)));
            }
            l.push_str(&format!(
                " \"{}\" \"{}\" \"{}\"",
                user(&hit.subject),
                user(&crate::find::hit_label(hit)),
                user(&hit.before)
            ));
            match (&hit.after, &hit.replaceable) {
                (Some(after), _) => l.push_str(&format!(" -> \"{}\"", user(after))),
                (None, Err(why)) => l.push_str(&format!(" read-only \"{}\"", user(why))),
                (None, Ok(())) => {}
            }
            lines.push(l);
        }
    }
    if let Some(row) = f.row() {
        lines.push(format!("  focus result {}", row + 1));
    }
    if let Some(note) = f.note() {
        lines.push(format!("find status \"{}\"", user(note)));
    }
}

/// FT-649: the column mapping step while open: `card mapping "list.csv" source "list" saved`, one `column` line per
/// column with its sample and member (`none`: not imported), and the importer's refusal of the map as `error`.
fn parts_map_lines(model: &ShellModel, lines: &mut Vec<String>) {
    let Some(map) = model.parts_map() else {
        return;
    };
    let mut head = format!(
        "card mapping \"{}\" source \"{}\"",
        user(&map.file),
        user(&map.source)
    );
    if map.saved {
        head.push_str(" saved");
    }
    lines.push(head);
    for c in &map.columns {
        let member = c
            .member
            .as_deref()
            .map_or_else(|| "none".to_owned(), |m| format!("\"{}\"", user(m)));
        lines.push(format!(
            "  column \"{}\" sample \"{}\" member {member}",
            user(&c.header),
            user(&c.sample)
        ));
    }
    if let Some(e) = &map.error {
        lines.push(format!("  error \"{}\"", user(e)));
    }
}

/// The inline preview card and the toast (WP-S2.05 §6.11), after the panes' lines: the card of the property grid or of
/// the find pane's replace.
fn card_and_toast_lines(model: &ShellModel, lines: &mut Vec<String>) {
    let props = model.properties();
    if let (Some(card), Some(rev)) = (props.card(), model.revision()) {
        match &card.preview.result {
            Err(message) => lines.push(format!(
                "card failed \"{}\" \"{}\"",
                user(&card.preview.name),
                user(message)
            )),
            Ok(_) => match &card.origin {
                // FT-410: the parts table import's card counts the part copies, not records, tags or texts.
                crate::properties::CardOrigin::Import { created, skipped, .. } => lines.push(format!(
                    "card {} \"{}\" created {created} skipped {skipped} errors 0",
                    card.preview.status(rev).word(),
                    user(&card.preview.name),
                )),
                _ => {
                    let mut line = format!(
                        "card {} \"{}\" targets {} changes {} tags {} xrefs {}",
                        card.preview.status(rev).word(),
                        user(&card.preview.name),
                        card.targets,
                        card.changes,
                        card.preview.derived.tags.len(),
                        card.preview.derived.xrefs.len()
                    );
                    // WP-S2.06 part 1: the reorder card names its origin and its folio changes.
                    if let crate::properties::CardOrigin::Reorder { notices } = &card.origin {
                        line.push_str(" reorder");
                        for n in notices {
                            line.push_str(&format!(" \"{}\"", user(n)));
                        }
                    }
                    // WP-S2.09 part 2: the quick-fix card names its origin and the finding's rule.
                    if let crate::properties::CardOrigin::QuickFix { rule } = &card.origin {
                        line.push_str(&format!(" fix \"{}\"", user(rule)));
                    }
                    // WP-S2.06 part 3: the table card names its origin.
                    if matches!(card.origin, crate::properties::CardOrigin::Table { .. }) {
                        line.push_str(" table");
                    }
                    lines.push(line);
                    // WP-S2.07 part 4a (§9.6): the part card names its origin, the field diff, the devices, the BOM
                    // rows and a fork's note.
                    if let crate::properties::CardOrigin::Part(p) = &card.origin {
                        if let Some(last) = lines.last_mut() {
                            last.push_str(" part");
                        }
                        for (member, old, new) in &p.fields {
                            lines.push(format!(
                                "  field \"{}\" \"{}\" \"{}\"",
                                user(member),
                                user(old),
                                user(new)
                            ));
                        }
                        for d in &p.devices {
                            lines.push(format!("  device \"{}\"", user(d)));
                        }
                        for r in &p.removed {
                            lines.push(format!("  removed \"{}\"", user(r)));
                        }
                        for r in &p.added {
                            lines.push(format!("  added \"{}\"", user(r)));
                        }
                        if p.fork {
                            lines.push(format!("  note \"{}\"", strings::text("part-edit-fork-note")));
                        }
                    }
                }
            },
        }
        // WP-S2.06 part 3: the table card's invalid and ambiguous cells (§8.8), numbered by their place in the gesture.
        if let crate::properties::CardOrigin::Table { gesture } = &card.origin {
            for (i, c) in gesture.cells.iter().enumerate() {
                if !c.blocking() {
                    continue;
                }
                let head = format!(
                    "  cell {} \"{}\" \"{}\"",
                    i + 1,
                    user(&c.row_label),
                    user(&c.column_label)
                );
                match (&c.value, c.reason()) {
                    (_, Some(reason)) => lines.push(format!("{head} invalid \"{}\"", user(&reason))),
                    (crate::navigators::table::CellValue::Ambiguous(readings), None) => {
                        let r: String = readings
                            .iter()
                            .map(|(t, _)| format!(" \"{}\"", user(t)))
                            .collect();
                        lines.push(format!("{head} ambiguous{r}"));
                    }
                    _ => {}
                }
            }
        }
    }
    toast_lines(model, lines);
}

// WP-S2.09 part 1
/// The visible toasts (§6.8), oldest first and the newest last: `toast "<text>"`, then the action's word and
/// `expired` when it is no longer valid.
fn toast_lines(model: &ShellModel, lines: &mut Vec<String>) {
    for n in model.notifications().visible() {
        lines.push(format!("toast \"{}\"{}", user(&n.text), action_words(model, n)));
    }
}

/// ` undo`, ` open expired`, or nothing: a notification's action and whether it is still valid.
fn action_words(model: &ShellModel, n: &crate::notifications::Notification) -> String {
    match n.action() {
        None => String::new(),
        Some(a) if model.notification_valid(n) => format!(" {}", a.word()),
        Some(a) => format!(" {} expired", a.word()),
    }
}

/// The Jobs pane while it is open (§6.8): `jobs running <k>`, then one line per entry, oldest first. A running job
/// prints no number (they depend on timing).
fn jobs_lines(model: &ShellModel, lines: &mut Vec<String>) {
    if !model.jobs_shown() {
        return;
    }
    let jobs = model.jobs();
    lines.push(format!("jobs running {}", jobs.running()));
    for e in jobs.entries() {
        let head = format!("  job {} {} \"{}\"", e.number, e.kind.word(), user(&e.name));
        lines.push(match &e.status {
            crate::jobs::JobStatus::Running => format!("{head} running"),
            crate::jobs::JobStatus::Done { pages, kib, now } => {
                let mut line = format!("{head} done pages {pages} kib {kib} rev {}", e.rev);
                if let Some(m) = now {
                    line.push_str(&format!(" now {m}"));
                }
                line
            }
            crate::jobs::JobStatus::Cancelled => format!("{head} cancelled"),
            crate::jobs::JobStatus::Failed(cause) => format!("{head} failed \"{}\"", user(cause)),
        });
    }
}

// WP-S2.09 part 3
/// One labelled record of a preview or of Show changes: `entity <change> "<kind>" "<label>" [fields "<f>"...]` (the
/// kind quoted: the record kinds stay out of the vocabulary).
fn entity_line(indent: &str, e: &ew_api::output::PreviewEntity) -> String {
    let mut line = format!(
        "{indent}entity {} \"{}\" \"{}\"",
        e.change,
        user(&e.kind),
        user(&e.label)
    );
    if !e.fields.is_empty() {
        line.push_str(" fields");
        for f in &e.fields {
            line.push_str(&format!(" \"{}\"", user(f)));
        }
    }
    line
}

/// At most [`crate::preview::MAX_LIST`] lines of `items`, then `  more <word> <n>`.
fn listed<T>(lines: &mut Vec<String>, items: &[T], word: &str, line: impl Fn(&T) -> String) {
    lines.extend(items.iter().take(crate::preview::MAX_LIST).map(line));
    if items.len() > crate::preview::MAX_LIST {
        lines.push(format!(
            "  more {word} {}",
            items.len() - crate::preview::MAX_LIST
        ));
    }
}

/// The Preview tab while it is open (§8.7): `preview none` without a card; else the header, the summary and the lists
/// of entities, tags, cross-reference texts and pages.
fn preview_lines(model: &ShellModel, lines: &mut Vec<String>) {
    if !model.preview_shown() {
        return;
    }
    let (Some(card), Some(rev)) = (model.properties().card(), model.revision()) else {
        lines.push("preview none".to_owned());
        return;
    };
    let name = user(&card.preview.name);
    let preview = match &card.preview.result {
        Ok(p) => p,
        Err(message) => {
            lines.push(format!("preview \"{name}\" failed \"{}\"", user(message)));
            return;
        }
    };
    lines.push(format!(
        "preview \"{name}\" {} rev {}",
        card.preview.status(rev).word(),
        card.preview.base_rev
    ));
    let pane = model.preview_pane();
    let mut summary = format!(
        "  summary records {} tags {} xrefs {} pages {}",
        preview.diff().len(),
        pane.tags().len(),
        pane.xrefs().len(),
        pane.pages().len()
    );
    if let crate::properties::CardOrigin::Import { created, skipped, .. } = &card.origin {
        summary.push_str(&format!(" created {created} skipped {skipped}"));
    }
    lines.push(summary);
    listed(lines, pane.entities(), "entity", |e| entity_line("  ", e));
    listed(lines, pane.tags(), "tag", |(a, b)| {
        format!("  tag \"{}\" \"{}\"", user(a), user(b))
    });
    listed(lines, pane.xrefs(), "xref", |x| {
        format!(
            "  xref \"{}\" \"{}\" \"{}\"",
            user(&x.folio),
            user(&x.before),
            user(&x.after)
        )
    });
    listed(lines, pane.pages(), "page", |f| format!("  page \"{}\"", user(f)));
}

/// The history panel while it is open (§8.7): `history entries <n> cursor <c>`, then its lines newest first (entries
/// numbered from 1, the markers after their revision's entry); the row cursor's entry ends with `cursor`. Show changes
/// follows as `  changes entry <n> records <k>` and at most [`crate::undo_history::MAX_CHANGES`] entity lines.
fn history_lines(model: &ShellModel, lines: &mut Vec<String>) {
    use crate::undo_history::HistoryLine;
    if !model.history_shown() {
        return;
    }
    let pane = model.history_pane();
    let Some(view) = pane.view() else { return };
    lines.push(format!(
        "history entries {} cursor {}",
        view.rows.len(),
        view.cursor
    ));
    for line in pane.lines(model.markers()) {
        match line {
            HistoryLine::Marker(m) => lines.push(format!("  marker \"{}\"", user(&m.text))),
            HistoryLine::Entry { row, state, origin } => {
                let mut text = format!(
                    "  entry {} {} \"{}\" origin \"{}\"",
                    row.index + 1,
                    state.word(),
                    user(&row.label),
                    user(origin)
                );
                if !row.pages.is_empty() {
                    text.push_str(" pages");
                    for f in &row.pages {
                        text.push_str(&format!(" \"{}\"", user(f)));
                    }
                }
                if row.merged > 1 {
                    text.push_str(&format!(" merged {}", row.merged));
                }
                if !row.step {
                    text.push_str(" marker");
                }
                if pane.row_cursor() == Some(row.index) {
                    text.push_str(" cursor");
                }
                lines.push(text);
            }
        }
    }
    if let Some((index, records)) = pane.changes() {
        lines.push(format!("  changes entry {} records {}", index + 1, records.len()));
        for e in records.iter().take(crate::undo_history::MAX_CHANGES) {
            lines.push(entity_line("    ", e));
        }
    }
}

/// The notifications log while its popup is open (§6.8): `notifications <n>`, then the entries, newest first.
fn notification_lines(model: &ShellModel, lines: &mut Vec<String>) {
    let log = model.notifications();
    if !log.popup_open() {
        return;
    }
    lines.push(format!("notifications {}", log.len()));
    for n in log.entries() {
        lines.push(format!(
            "  notification {} \"{}\"{}",
            n.serial,
            user(&n.text),
            action_words(model, n)
        ));
    }
}

/// One row's line: key, label, value, provenance, read-only reason, actions.
fn row_line(
    sheet: &crate::properties::PropertySheet,
    section: &crate::properties::SheetSection,
    row: &crate::properties::PropRow,
) -> String {
    let label = crate::properties::row_label(section, row);
    let mut line = format!("    row \"{}\" \"{}\"", user(&row.key), user(&label));
    if crate::properties::is_header(section, row) {
        line.push_str(" group");
        return line;
    }
    match &row.value {
        RowValue::One { text, .. } => line.push_str(&format!(" \"{}\"", user(text))),
        RowValue::Varies { count, texts } => {
            line.push_str(&format!(" varies {count}"));
            for t in texts {
                line.push_str(&format!(" \"{}\"", user(t)));
            }
        }
        RowValue::Absent => line.push_str(" none"),
    }
    let prov = provenance_words(&row.provenance);
    if !prov.is_empty() {
        line.push(' ');
        line.push_str(&prov);
    }
    if let Some(reason) = crate::properties::read_only_reason(sheet, section, row) {
        line.push_str(&format!(" read-only \"{}\"", user(&reason)));
    }
    if !row.actions.is_empty() {
        let words: Vec<&str> = row.actions.iter().map(|a| a.word()).collect();
        line.push_str(&format!(" actions {}", words.join(" ")));
    }
    line
}

fn project_view(model: &ShellModel, lines: &mut Vec<String>) {
    // WP-S2.06 part 3: the Pages navigator in table mode prints as the other navigators do.
    if model.nav(crate::navigators::NavKind::Pages).is_none() {
        navigator_lines(model, lines);
    }
    // WP-S2.06 part 1
    other_navigator_lines(model, lines);
    properties_lines(model, lines);
    // WP-S2.05 part 2
    find_lines(model, lines);
    // WP-S2.09 part 1
    jobs_lines(model, lines);
    // WP-S2.09 part 3
    preview_lines(model, lines);
    history_lines(model, lines);
    // FT-649: the parts import's column mapping step.
    parts_map_lines(model, lines);
    card_and_toast_lines(model, lines);
    // WP-S2.05 part 3: what the clipboard holds, never its text.
    lines.push(model.clipboard().describe_line());
    tab_lines(model, lines);
    match model.current_folio() {
        Some(folio) => lines.push(format!("page view: {folio}")),
        None => lines.push(format!("page view: {}", strings::text("page-no-pages"))),
    }
    canvas_lines(model, lines);
    lines.push(format!("workspace \"{}\"", model.workspace()));
    lines.push(format!("layout: {}", model.layout().describe()));
    // WP-S2.17 part 3 (D15): the rails.
    lines.extend(crate::rail::describe_lines(model));
}

/// The menu lines of `entry` at `depth` (WP-S2.17 part 2): an item, a submenu with its items, or a command-bar group
/// with its entries one level deeper.
fn menu_entry_lines(entry: &MenuEntry, depth: usize, lines: &mut Vec<String>) {
    let indent = "  ".repeat(depth);
    match entry {
        MenuEntry::Separator => {}
        MenuEntry::Item(item) => lines.push(item_line(&indent, item)),
        MenuEntry::Submenu { label, items, .. } => {
            lines.push(format!("{indent}submenu \"{label}\""));
            let inner = "  ".repeat(depth + 1);
            lines.extend(items.iter().map(|i| item_line(&inner, i)));
        }
        MenuEntry::Group { label, entries, .. } => {
            lines.push(format!("{indent}group \"{label}\""));
            for e in entries {
                menu_entry_lines(e, depth + 1, lines);
            }
        }
    }
}

/// A command of the command bar as `id "bar label" [shortcut] (checked) enabled|disabled`; the reason of a disabled
/// command is on its menu line.
fn bar_button(b: &crate::command_bar::CommandButton) -> String {
    let mut text = format!("{} \"{}\"", b.id, b.bar_label);
    if !b.shortcut.is_empty() {
        text.push_str(&format!(" [{}]", b.shortcut));
    }
    if b.checked {
        text.push_str(" (checked)");
    }
    text.push_str(if b.enabled { " enabled" } else { " disabled" });
    text
}

/// The command bar's lines (WP-S2.17 part 2, D15): the active tab, then one line per group with its commands in bar
/// order, then the next step; a collapsed bar prints its tab line only.
fn command_bar_lines(model: &ShellModel, lines: &mut Vec<String>) {
    let bar = crate::command_bar::command_bar(model);
    let tab = bar
        .tabs
        .iter()
        .find(|(t, _)| *t == bar.tab)
        .map_or("", |(_, l)| l.as_str());
    let mut head = format!("commandbar tab \"{tab}\"");
    if bar.collapsed {
        head.push_str(" collapsed");
        lines.push(head);
        return;
    }
    lines.push(head);
    for g in &bar.groups {
        let buttons: Vec<String> = g.buttons.iter().map(bar_button).collect();
        lines.push(format!("  group \"{}\" {}", g.label, buttons.join(", ")));
    }
    if let Some(next) = &bar.next {
        lines.push(format!("  next step {}", bar_button(next)));
    }
}

/// The HUD's line over the canvas (WP-S2.17 part 2, D15): the tool, what it works on and the prompt (quoted user text).
fn hud_line(model: &ShellModel, lines: &mut Vec<String>) {
    let hud = crate::command_bar::hud(model);
    let mut line = format!("hud tool {} \"{}\"", hud.tool, hud.tool_label);
    if let Some(target) = &hud.target {
        line.push_str(&format!(" target \"{target}\""));
    }
    line.push_str(&format!(" prompt \"{}\"", hud.prompt));
    lines.push(line);
}

/// The canvas lines after the page-view line (WP-S2.02 section 6.6): millimetres with three decimals from whole
/// nanometres, the zoom with one decimal; the grid and backend labels are quoted user text.
fn canvas_lines(model: &ShellModel, lines: &mut Vec<String>) {
    let Some(c) = model.canvas() else { return };
    let [x0, y0, x1, y1] = c.view_nm();
    lines.push(format!(
        "canvas zoom {:.1} % view {} {} {} {} mm",
        round_tenth(c.zoom() * 100.0),
        mm3(x0),
        mm3(y0),
        mm3(x1),
        mm3(y1)
    ));
    if model.grid_on() && c.page().grid.is_some() {
        lines.push(format!("canvas grid on \"{}\"", crate::status::grid_text(model)));
    } else {
        lines.push("canvas grid off".to_owned());
    }
    let cell = |p: (i64, i64)| {
        c.page()
            .cell_text(p)
            .map_or_else(String::new, |t| format!(" cell \"{t}\""))
    };
    match c.pointer() {
        Some((x, y)) => lines.push(format!("canvas pointer {} {} mm{}", mm3(x), mm3(y), cell((x, y)))),
        None => lines.push("canvas pointer none".to_owned()),
    }
    match c.hover() {
        Some((entity, r)) => lines.push(format!(
            "canvas hover {} box {} {} {} {} mm",
            entity.0,
            mm3(r.min().x.0),
            mm3(r.min().y.0),
            mm3(r.max().x.0),
            mm3(r.max().y.0)
        )),
        None => lines.push("canvas hover none".to_owned()),
    }
    // WP-S1.21 D7: the printed PLC address of the connection point under the pointer, when it has one.
    if let Some(cp) = c.hovered_cp()
        && let Some(address) = c
            .page()
            .tables
            .cps
            .iter()
            .find(|t| t.id == cp.id)
            .and_then(|t| t.plc_address.as_deref())
    {
        lines.push(format!(
            "canvas hover cp \"{}\" address \"{}\"",
            user(&cp.label),
            user(address)
        ));
    }
    match c.cursor() {
        Some((x, y)) => {
            let cp = c
                .cursor_cp()
                .map_or_else(String::new, |cp| format!(" cp \"{}\"", cp.label));
            lines.push(format!(
                "canvas cursor {} {} mm{}{cp}",
                mm3(x),
                mm3(y),
                cell((x, y))
            ));
        }
        None => lines.push("canvas cursor none".to_owned()),
    }
    match c.hovered_link() {
        None => lines.push("canvas link none".to_owned()),
        Some(l) => match &l.target {
            None => lines.push("canvas link external".to_owned()),
            Some(t) => match &t.cell {
                Some(cell) => lines.push(format!("canvas link \"{cell}\" folio \"{}\"", t.folio)),
                None => lines.push(format!("canvas link folio \"{}\"", t.folio)),
            },
        },
    }
    lines.push(format!("canvas backend \"{}\"", model.canvas_backend().label()));
    if model.frame_stats_shown() {
        lines.push("canvas statistics shown".to_owned());
    }
    lines.push(format!("canvas renderer {}", crate::script::renderer_word(model)));
    // WP-S2.06 part 1: the chip line "Not on this page" (§6.5).
    if !model.chips().is_empty() {
        let chips: Vec<String> = model
            .chips()
            .iter()
            .map(|c| format!("\"{}\"", user(&c.text())))
            .collect();
        lines.push(format!("  elsewhere {}", chips.join(" ")));
    }
    // WP-S2.06 part 2: the highlights drawn on the shown page (§7.4), oldest first.
    if let Some(group) = model.area().map(|a| a.focused) {
        let shown = model.highlight_overlay(group);
        for h in model.highlights().items() {
            let drawn = shown
                .iter()
                .find(|(item, _)| matches!(item, crate::tools::OverlayItem::Highlight { slot, .. } if *slot == h.slot))
                .map(|(_, n)| *n);
            if let Some(n) = drawn {
                lines.push(format!(
                    "  highlight slot {} {} \"{}\" connections {n}",
                    h.slot,
                    h.word(),
                    user(&h.name)
                ));
            }
        }
    }
    tool_lines(model, lines);
    // WP-S2.17 part 2: the HUD over the canvas.
    hud_line(model, lines);
}

/// The tool lines after the canvas lines (WP-S2.03 §7.9): the active tool, the snap indicator, the selection, the
/// kept preview, the overlay with its labels and the link preview while shown. The tool's label and hint, a failed
/// preview's message and the labels are quoted user text.
fn tool_lines(model: &ShellModel, lines: &mut Vec<String>) {
    let tool = model.tool();
    lines.push(format!(
        "canvas tool \"{}\" state {} hint \"{}\"",
        crate::tools::tool_label(tool.name()),
        tool.state(),
        model.tool_hint()
    ));
    match model.snap_indicator() {
        Some(s) => {
            let (x, y) = s.at_nm;
            match &s.label {
                Some(label) => lines.push(format!(
                    "canvas snap {} \"{label}\" at {} {} mm",
                    s.kind.word(),
                    mm3(x),
                    mm3(y)
                )),
                None => lines.push(format!(
                    "canvas snap {} at {} {} mm",
                    s.kind.word(),
                    mm3(x),
                    mm3(y)
                )),
            }
        }
        None => lines.push("canvas snap none".to_owned()),
    }
    let selection = model.selection();
    if selection.is_empty() {
        lines.push("canvas selection none".to_owned());
    } else {
        // The IDs as value tokens, which the printer turns into aliases `@n` (FT-422).
        let ids: Vec<String> = selection.items.iter().map(|e| e.0.to_string()).collect();
        lines.push(format!(
            "canvas selection {} {}",
            selection.items.len(),
            ids.join(" ")
        ));
    }
    match (model.preview_state(), model.preview_status()) {
        (Some(p), Some(status)) => match &p.result {
            Err(message) => lines.push(format!("canvas preview failed \"{}\" \"{message}\"", p.name)),
            Ok(_) => lines.push(format!(
                "canvas preview {} \"{}\" tags {} xrefs {}",
                status.word(),
                p.name,
                p.derived.tags.len(),
                p.derived.xrefs.len()
            )),
        },
        _ => lines.push("canvas preview none".to_owned()),
    }
    let overlay = model.tool_overlay();
    let labels: Vec<&str> = overlay.labels().collect();
    if labels.is_empty() {
        lines.push(format!("canvas overlay {}", overlay.len()));
    } else {
        for label in labels {
            lines.push(format!("canvas overlay {} label \"{label}\"", overlay.len()));
        }
    }
    if let Some(lp) = model.link_preview() {
        lines.push(format!("canvas link preview \"{}\"", lp.target.folio));
    }
    ghost_lines(model, lines);
}

/// The lines of WP-S2.04 §7.7 after the tool lines: the picker while open (one line, then one line per row; the
/// current row ends with `current`) and what the active tool floats or drags.
fn ghost_lines(model: &ShellModel, lines: &mut Vec<String>) {
    if let Some(p) = model.picker() {
        lines.push(format!(
            "canvas picker filter \"{}\" rows {}",
            p.filter,
            p.rows.len()
        ));
        for (i, row) in p.rows.iter().enumerate() {
            let mut line = format!("  {}", p.row_line(*row));
            if i == p.row {
                line.push_str(" current");
            }
            lines.push(line);
        }
    }
    // WP-S2.18 D2: the Join chooser while open (one line, then one line per row with its kind word; the active row
    // ends with `current`).
    if let Some(c) = model.chooser() {
        lines.push(format!("canvas chooser \"{}\" rows {}", c.question, c.rows.len()));
        for (i, row) in c.rows.iter().enumerate() {
            let mut line = format!("  \"{}\"", row.name);
            if !row.kind.is_empty() {
                line.push_str(&format!(" \"{}\"", row.kind));
            }
            if i == c.row {
                line.push_str(" current");
            }
            lines.push(line);
        }
    }
    match model.tool_ghost() {
        None => {}
        Some(crate::tools::Ghost::Place {
            symbol,
            variant,
            rep_type,
            at_nm,
            target,
            letter,
            strip,
            tag,
            proposals,
            ..
        }) => {
            let mut line =
                format!("canvas ghost place symbol \"{symbol}\" variant \"{variant}\" rep \"{rep_type}\"");
            if let Some((x, y)) = at_nm {
                line.push_str(&format!(" at {} {} mm", mm3(x), mm3(y)));
            }
            // Another representation of an existing function reads `target existing`.
            let target = if target == "function" { "existing" } else { target };
            line.push_str(&format!(" target {target}"));
            if let Some(l) = letter {
                line.push_str(&format!(" letter \"{l}\""));
            }
            if let Some(st) = strip {
                line.push_str(&format!(" strip \"{st}\""));
            }
            if let Some(t) = tag {
                line.push_str(&format!(" tag \"{t}\""));
            }
            lines.push(line);
            // WP-S2.04 part 2 (§8.7): the autoconnect proposals while the ghost has any.
            if !proposals.is_empty() {
                lines.push(proposals_line(&proposals));
            }
        }
        Some(crate::tools::Ghost::Move {
            count,
            steps,
            readout,
            proposals,
        }) => {
            lines.push(format!(
                "canvas ghost move {count} steps {} {} readout \"{readout}\"",
                steps.0, steps.1
            ));
            // WP-S2.04 part 2 (§8.3): a move shows its proposals and connects none.
            if !proposals.is_empty() {
                lines.push(format!("{} connect none", proposals_line(&proposals)));
            }
        }
        // WP-S2.05 part 3 (§8.8)
        Some(crate::tools::Ghost::Paste {
            records,
            at_nm,
            dropped,
        }) => {
            let mut line = format!("canvas ghost paste records {records}");
            if let Some((x, y)) = at_nm {
                line.push_str(&format!(" at {} {} mm", mm3(x), mm3(y)));
            }
            line.push_str(&format!(" dropped {dropped}"));
            lines.push(line);
        }
        Some(crate::tools::Ghost::Exchange { symbol, unmapped }) => {
            let list = if unmapped.is_empty() {
                "none".to_owned()
            } else {
                unmapped
                    .iter()
                    .map(|u| format!("\"{u}\""))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            lines.push(format!(
                "canvas ghost exchange symbol \"{symbol}\" unmapped {list}"
            ));
        }
        // WP-S2.04 part 3 (§9.6): the IP tool's ghost (an external connection point names its reference) and the black
        // box tool's rectangle.
        Some(crate::tools::Ghost::Ip {
            name,
            direction,
            star,
            at_nm,
            external,
        }) => {
            let mut line = format!(
                "canvas ghost ip name \"{name}\" direction {direction} star {}",
                if star { "on" } else { "off" }
            );
            if let Some((x, y)) = at_nm {
                line.push_str(&format!(" at {} {} mm", mm3(x), mm3(y)));
            }
            if let Some(e) = external {
                line.push_str(&format!(" external \"{e}\""));
            }
            lines.push(line);
        }
        Some(crate::tools::Ghost::Box { outline, cps }) => {
            lines.push(format!("canvas ghost box outline {outline} cps {cps}"));
        }
        // WP-S2.04 part 4b (§10.7): the text tools' free text and the Graphic tool's shape.
        Some(crate::tools::Ghost::Text { at_nm, text, bind }) => {
            let mut line = "canvas ghost item text".to_owned();
            if let Some((x, y)) = at_nm {
                line.push_str(&format!(" at {} {} mm", mm3(x), mm3(y)));
            }
            let shown = bind.map_or(text, |b| format!("{{{b}}}"));
            line.push_str(&format!(" text \"{shown}\""));
            lines.push(line);
        }
        Some(crate::tools::Ghost::Graphic {
            shape,
            points,
            anticlockwise,
        }) => {
            let mut line = format!("canvas ghost item graphic {shape} points {points}");
            if anticlockwise {
                line.push_str(" anticlockwise");
            }
            lines.push(line);
        }
        // WP-S2.04 part 4b: the Wire marker tool's group and the Structure box tool's outline.
        Some(crate::tools::Ghost::Marker { connections, text }) => {
            let mut line = format!("canvas ghost marker connections {connections}");
            if let Some(text) = text {
                line.push_str(&format!(" text \"{text}\""));
            }
            lines.push(line);
        }
        Some(crate::tools::Ghost::StructureBox { outline }) => {
            lines.push(format!("canvas ghost box outline {outline}"));
        }
        // WP-S2.04 part 5 (§11): the Image tool's asset.
        Some(crate::tools::Ghost::Image { asset }) => {
            let mut line = "canvas ghost item image".to_owned();
            if let Some(asset) = asset {
                line.push_str(&format!(" \"{asset}\""));
            }
            lines.push(line);
        }
    }
    wiring_lines(model, lines);
}

/// `canvas proposals <n> "<cp>" -> "<target>" …` (a split: `"<cp>" "<cp>" -> segment`).
fn proposals_line(proposals: &[(String, Option<String>)]) -> String {
    let mut line = format!("canvas proposals {}", proposals.len());
    for (cp, target) in proposals {
        match target {
            Some(t) => line.push_str(&format!(" {cp} -> \"{t}\"")),
            None => line.push_str(&format!(" {cp} -> segment")),
        }
    }
    line
}

/// The lines of WP-S2.04 part 2 (§8.7): the route the Wire tool draws, the junction sequence editor while open (one
/// line, then one line per physical wire) and the crossings setting when it draws hops.
fn wiring_lines(model: &ShellModel, lines: &mut Vec<String>) {
    if let Some(r) = model.tool().route_info() {
        let mut line = format!(
            "canvas route from \"{}\" bends {} points {}",
            r.from, r.bends, r.points
        );
        if let Some(end) = &r.end {
            line.push_str(&format!(" end \"{end}\""));
        }
        lines.push(line);
    }
    if let Some(e) = model.junction_editor() {
        let sequence: Vec<String> = e.sequence_labels().iter().map(|l| format!("\"{l}\"")).collect();
        lines.push(format!(
            "canvas junction \"{}\" style {} wiring {} sequence {}",
            e.cell,
            e.style,
            e.wiring,
            sequence.join(" ")
        ));
        for w in &e.wires {
            lines.push(format!(
                "  wire \"{}\" from \"{}\" end \"{}\"",
                w.label, w.from, w.to
            ));
        }
    }
    if let Some(word) = model.crossings_word() {
        lines.push(format!("canvas crossings {word}"));
    }
}

fn on_off(b: bool) -> &'static str {
    if b { "on" } else { "off" }
}

/// The part dialog's lines (WP-S2.07 §9.6).
fn part_choose_lines(
    model: &ShellModel,
    c: &crate::shell::part_dialog::PartChooser,
    lines: &mut Vec<String>,
) {
    let q = &c.query;
    lines.push(format!(
        "  query \"{}\" kind {} category {} fits {} view {} inactive {}",
        user(&q.text),
        c.kind_word(),
        c.category_word(),
        on_off(q.fits_only),
        q.view.word(),
        on_off(q.include_inactive)
    ));
    let shown = c.offers.len().min(crate::shell::part_dialog::PART_ROWS);
    lines.push(format!("  results showing {} to {shown}", usize::from(shown > 0)));
    for (i, o) in c.offers.iter().take(shown).enumerate() {
        let source = if o.held {
            "project".to_owned()
        } else {
            format!(
                "library \"{}\"",
                user(&o.library.as_ref().map_or_else(String::new, |l| l.1.clone()))
            )
        };
        let mut line = format!(
            "  result {} \"{}\" \"{}\" \"{}\" {source} compatible {}",
            i + 1,
            user(&o.part_number),
            user(&o.manufacturer),
            user(&o.description),
            o.fit.word()
        );
        if o.favourite {
            line.push_str(" favourite");
        }
        if i == c.cursor {
            line.push_str(" cursor");
        }
        lines.push(line);
    }
    if let Some(detail) = model.part_offer_detail(c.cursor) {
        lines.push(format!("  detail \"{}\"", user(&detail)));
    }
}

/// The part editor's lines (WP-S2.07 §9.6).
fn part_edit_lines(e: &crate::shell::part_dialog::PartEditor, lines: &mut Vec<String>) {
    let d = &e.draft;
    for f in ew_api::DRAFT_FIELDS {
        lines.push(format!(
            "  field \"{}\" \"{}\"",
            strings::text(&format!("part-field-{f}")),
            user(d.text(f))
        ));
    }
    for (k, v) in &d.tech {
        lines.push(format!("  tech \"{}\" \"{}\"", user(k), user(v)));
    }
    let none = || strings::text("part-none").to_owned();
    let functions = if d.functions.is_empty() {
        none()
    } else {
        d.functions.join(", ")
    };
    let cps = if d.cps.is_empty() {
        none()
    } else {
        d.cps.join(", ")
    };
    lines.push(format!(
        "  read-only \"{}\" \"{}\"",
        strings::text("part-field-functions"),
        user(&functions)
    ));
    lines.push(format!(
        "  read-only \"{}\" \"{}\"",
        strings::text("part-field-cps"),
        user(&cps)
    ));
    let source = match &d.library {
        Some(name) => format!("library \"{}\"", user(name)),
        None => "project".to_owned(),
    };
    lines.push(format!(
        "  source {source} version \"{}\" fork {}",
        user(&d.version),
        on_off(d.fork)
    ));
    for m in &d.messages {
        let label = match m.field.strip_prefix("tech:") {
            Some(key) => key.to_owned(),
            None => strings::text(&format!("part-field-{}", m.field)).to_owned(),
        };
        lines.push(format!(
            "  invalid \"{}\" \"{}\"",
            user(&label),
            user(&crate::shell::part_dialog::message_text(m.key, &m.args))
        ));
    }
    if let Some(err) = &e.error {
        lines.push(format!("  error: {err}"));
    }
}

fn dialog(model: &ShellModel, lines: &mut Vec<String>) {
    let Some(d) = model.dialog() else {
        return;
    };
    // WP-S2.17 part 4: the title and the buttons are the dialog's specification (`dialogs::spec`), the window's too;
    // the buttons in the order Primary, the others, the destructive one.
    let spec = crate::dialogs::spec(d, model);
    lines.push(format!("dialog \"{}\"", user(&spec.title)));
    match d {
        Dialog::About => {
            for line in &spec.body {
                lines.push(format!("  label \"{line}\""));
            }
        }
        Dialog::Notices => {
            let text = crate::about::notices_text(model);
            lines.push(format!("  notices: {} lines", text.lines().count()));
        }
        Dialog::Shortcuts => {
            for meta in actions::ACTIONS
                .iter()
                .filter(|a| !a.headless_only && !a.hidden && !a.shortcut.is_empty())
            {
                lines.push(format!(
                    "  {} \"{}\" {}",
                    meta.id,
                    strings::text(meta.label_key),
                    meta.shortcut.join(", ")
                ));
            }
        }
        Dialog::ConfirmDiscard { pending } => {
            for line in &spec.body {
                lines.push(format!("  label \"{line}\""));
            }
            lines.push(format!("  pending {}", pending.action));
        }
        Dialog::ConfirmCheckScheme { .. } | Dialog::ConfirmJournalDiscard => {
            for line in &spec.body {
                lines.push(format!("  label \"{line}\""));
            }
        }
        // WP-S2.09 part 1 (§6.6): the time of the last change is the window's only (never in the transcript).
        Dialog::Recovery {
            name,
            transactions,
            base_matches,
            failed,
            ..
        } => {
            lines.push(format!(
                "  label \"{}\"",
                user(&strings::format("recovery-text", &[name]))
            ));
            let base = if *base_matches { "matches" } else { "moved" };
            let mut line = format!("  recovery transactions {transactions} base {base}");
            if let Some(reason) = failed {
                line.push_str(&format!(" failed \"{}\"", user(reason)));
            }
            lines.push(line);
        }
        Dialog::GotoPage { text } => {
            lines.push(format!("  field \"{}\": \"{text}\"", strings::text("goto-hint")));
        }
        Dialog::SaveWorkspace { text } => {
            lines.push(format!(
                "  field \"{}\": \"{text}\"",
                strings::text("save-workspace-hint")
            ));
        }
        // WP-S2.09 part 3 (§8.6): at most 20 file names, then "and <n> more".
        Dialog::ConfirmPurge { files, .. } => {
            lines.push(format!("  label \"{}\"", strings::text("confirm-purge-text")));
            for f in files.iter().take(crate::shell::PURGE_LISTED) {
                lines.push(format!("  item \"{}\"", user(f)));
            }
            if files.len() > crate::shell::PURGE_LISTED {
                let more = (files.len() - crate::shell::PURGE_LISTED).to_string();
                lines.push(format!(
                    "  label \"{}\"",
                    strings::format("confirm-purge-more", &[&more])
                ));
            }
        }
        // WP-S2.07 part 4a (§9.6): the query, the window of results (50 rows, never the total) and the active row's
        // detail.
        Dialog::PartChoose(c) => part_choose_lines(model, c, lines),
        Dialog::PartEdit(e) => part_edit_lines(e, lines),
        // FT-408 and FT-409: a creation dialog, one `field` line per field in order; the refusal of the last attempt
        // follows the fields as an `error:` line.
        Dialog::Form(form) => {
            for f in &form.fields {
                lines.push(format!(
                    "  field \"{}\": \"{}\"",
                    strings::text(f.label_key),
                    f.value
                ));
            }
            if let Some(e) = &form.error {
                lines.push(format!("  error: {e}"));
            }
        }
    }
    for b in spec.buttons() {
        lines.push(format!("  button \"{}\"", b.label));
    }
}

/// A label or title of the page text as it is quoted: a double quote becomes a single one and a line break a space.
fn page_quoted(text: &str) -> String {
    let t: String = text
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    format!("\"{}\"", t.replace('"', "'"))
}

/// `at X Y mm` of a page point in page nm, printed as the sheet point a pointer or the grid cursor reaches it at
/// (`canvas.press`, `canvas.pointer`, `cursor.set`; FT-485 item 5): the page's own coordinates are offset from the
/// sheet's by the frame margin, so a page coordinate printed as is missed every click by that margin.
fn page_at(page: &CanvasPage, p: (i64, i64)) -> String {
    let s = page.snapper.page_to_sheet(p);
    format!("at {} {} mm", mm3(s.0), mm3(s.1))
}

/// The candidate points of [`wire_point`] along a segment, in 32nds: the middle first, then outward (the nearer to the
/// middle first, the lower first).
const FRACTIONS: [i64; 31] = [
    16, 15, 17, 14, 18, 13, 19, 12, 20, 11, 21, 10, 22, 9, 23, 8, 24, 7, 25, 6, 26, 5, 27, 4, 28, 3, 29, 2,
    30, 1, 31,
];

/// A sheet point on the drawn polyline of `wire` that the Select tool's press picks as this wire (WP-S2.04 §6.6 as
/// FT-500 amends it; FT-485 item 4), so a reader of the page text can hover, select or delete it: with the pick reach
/// at 50 % zoom (else at 90 %), the wire is first in the pick stack there ([`crate::tools::pick_stack`]) and no other
/// connection is in it, and no link is under it (a press on a link follows it). The candidates are the points at n/32 of each segment ([`FRACTIONS`]: the middle first, then
/// outward), longer segments first (ties: the segment's order); `None` when no candidate is unambiguous (a provisional
/// route that runs along other wires for its whole length; a repeated click there walks the stack).
fn wire_point(page: &CanvasPage, wire: &ew_api::output::TableConnection) -> Option<(i64, i64)> {
    let own: Vec<(i64, i64)> = wire
        .polyline
        .iter()
        .map(|p| page.snapper.page_to_sheet(*p))
        .collect();
    let mut segments: Vec<(i128, usize)> = own
        .windows(2)
        .enumerate()
        .map(|(i, w)| (crate::tools::dist2(w[0], w[1]), i))
        .filter(|(len, _)| *len > 0)
        .collect();
    segments.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let wires: std::collections::BTreeSet<_> = page.tables.connections.iter().map(|c| c.id).collect();
    let me = ew_api::output::ew_dl::EntityRef(wire.id);
    // The pick reach of 4 points at 50 % and at 90 % zoom, nm (1 pt = 25.4 mm / 72).
    for reach in [25_400_000 * 4 * 2 / 72, 25_400_000 * 4 * 10 / 72 / 9] {
        for &(_, i) in &segments {
            let (a, b) = (own[i], own[i + 1]);
            for n in FRACTIONS {
                let p = (a.0 + (b.0 - a.0) * n / 32, a.1 + (b.1 - a.1) * n / 32);
                let stack = crate::tools::pick_stack(page, &page.snapper, p, f64::from(reach));
                let alone = stack.iter().filter(|e| wires.contains(&e.0)).count() == 1;
                let link =
                    crate::tools::link_at(&page.links, p, crate::canvas::HOVER_SLOP_PT / f64::from(reach))
                        .is_some();
                if stack.first() == Some(&me) && alone && !link {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// A terminal's tag (`-X1011:3`) as a sentence names it (FT-716): "-X1011 terminal 3", so that the strip and the
/// number read apart. The designation `-X1011:3` stays in tables, Properties and printed labels; a tag without a colon
/// is returned as it is.
pub(crate) fn terminal_in_sentence(tag: &str) -> String {
    match tag.rsplit_once(':') {
        Some((strip, number)) if !strip.is_empty() && !number.is_empty() => {
            crate::strings::format("terminal-in-sentence", &[strip, number])
        }
        _ => tag.to_owned(),
    }
}

/// A connection point's label (`-1A4:-#2`) as a sentence names it (cycle 38, FX38d): a designation the device
/// shares between pins carries its position in words, "-1A4 pin - (2nd)", never the label's `#2`. Any other label
/// (an unshared pin `-1A4:L+`, a terminal's clamp `-X1:1/a#2`, which FT-716's form names) is returned as it is; tags,
/// tables and printed labels keep the label.
pub(crate) fn pin_in_sentence(label: &str) -> String {
    let Some((head, n)) = label.rsplit_once('#') else {
        return label.to_owned();
    };
    let Ok(n) = n.parse::<u32>() else {
        return label.to_owned();
    };
    match head.rsplit_once(':') {
        Some((tag, pin)) if !tag.is_empty() && !pin.is_empty() && !pin.contains('/') && n > 0 => {
            crate::strings::format("pin-in-sentence", &[tag, pin, &ordinal(n)])
        }
        _ => label.to_owned(),
    }
}

/// The English ordinal suffixes, for a number ending in 1, 2 or 3 and for every other number (the teens 11 to 13
/// take the last). English only for now: this table is what WP-S2.08 part 3 (the shell's Fluent tables) localises
/// (FT-752), where a language's ordinal rule replaces the choice in [`ordinal`].
const ORDINAL_SUFFIXES: [&str; 4] = ["st", "nd", "rd", "th"];

/// An English ordinal: `1st`, `2nd`, `3rd`, `4th`, `11th`, `12th`, `13th`, `21st` ([`ORDINAL_SUFFIXES`]).
fn ordinal(n: u32) -> String {
    let [st, nd, rd, th] = ORDINAL_SUFFIXES;
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => th,
        (1, _) => st,
        (2, _) => nd,
        (3, _) => rd,
        _ => th,
    };
    format!("{n}{suffix}")
}

/// The variant letter of an orientation `0..=7`.
fn variant_letter(orient: u8) -> char {
    char::from(b'A' + orient.min(7))
}

/// The tag a representation names: its device's short tag, read off the labels of its connection points (a CP's
/// label is `tag:designation`, a terminal's `tag/clamp` or, without clamps, its tag alone, an interruption point's
/// its name); `None` without one. A terminal's tag is the strip's and its own designation (`-X1011:2`), so that two
/// terminals of one strip never read alike (cycle 36, play-test rv-txt-2).
pub(crate) fn rep_tag(rep: &TableRep, cps: &[&TableCp]) -> Option<String> {
    let label = &cps.first()?.label;
    // A terminal's label is `-X1031:11/a` (its tag is `-X1031:11`), or `-X1011:2` when its function names no clamps
    // (a part's template); a device's is `-101F1:2` or `-101F1:2#2`.
    if let Some((tag, _clamp)) = label.rsplit_once('/') {
        return Some(tag.to_owned());
    }
    if rep.terminal {
        return Some(label.clone());
    }
    Some(label.split(':').next().unwrap_or(label).to_owned())
}

/// The reading key of a page point: its frame cell's row then column (a point outside every cell last), then `y`, `x`.
fn reading(page: &CanvasPage, p: (i64, i64)) -> (u32, u32, i64, i64) {
    let (row, column) = page
        .cells
        .as_ref()
        .and_then(|c| c.cell_at(page.snapper.page_to_sheet(p)))
        .map_or((u32::MAX, u32::MAX), |c| (c.row_index, c.column_index));
    (row, column, p.1, p.0)
}

/// The cell of a page point as the line prints it: the quoted cell text, or `none` outside the frame.
fn cell_word(page: &CanvasPage, p: (i64, i64)) -> String {
    page.cell_text(page.snapper.page_to_sheet(p))
        .map_or_else(|| "none".to_owned(), |c| page_quoted(&c))
}

/// The junctions of the page text in their numbered order (indices into the page tables' junctions): reading order of
/// the position, then style, wiring, the far-end CP labels of the sequence and the ID.
fn junction_order(page: &CanvasPage) -> Vec<usize> {
    let t: &PageTables = &page.tables;
    let cp_label = |function, idx| {
        t.cps
            .iter()
            .find(|c| c.function == function && c.idx == idx)
            .map_or_else(String::new, |c| c.label.clone())
    };
    let end_cp_only = |e: &TableEnd| match e {
        TableEnd::Cp { function, idx } => cp_label(*function, *idx),
        TableEnd::Junction { .. } => String::new(),
    };
    let far_cp_labels = |j: &ew_api::output::TableJunction| -> Vec<String> {
        j.sequence
            .iter()
            .filter_map(|id| t.connections.iter().find(|c| c.id == *id))
            .map(|c| match (c.a, c.b) {
                (TableEnd::Junction { id }, other) if id == j.id => end_cp_only(&other),
                (other, _) => end_cp_only(&other),
            })
            .collect()
    };
    let mut order: Vec<usize> = (0..t.junctions.len()).collect();
    order.sort_by_cached_key(|i| {
        let j = &t.junctions[*i];
        // The IDs one transaction mints are not in creation order, so an ID decides only between junctions that the
        // position, the style, the wiring and the sequence cannot tell apart.
        (
            reading(page, j.at),
            j.style.clone(),
            j.wiring.clone(),
            far_cp_labels(j),
            j.id,
        )
    });
    order
}

/// The drawn connections of `page` with their two ends as the page text names them, without quotes: a CP label or
/// `junction <n>` (FT-485 item 4: `select.set` takes `wire:<a> -> <b>`).
pub fn page_wires(page: &CanvasPage) -> Vec<(ew_api::output::ew_dl::EntityRef, String, String)> {
    let t: &PageTables = &page.tables;
    let order = junction_order(page);
    let end = |e: &TableEnd| match e {
        TableEnd::Cp { function, idx } => t
            .cps
            .iter()
            .find(|c| c.function == *function && c.idx == *idx)
            .map_or_else(String::new, |c| c.label.clone()),
        TableEnd::Junction { id } => format!(
            "junction {}",
            order
                .iter()
                .position(|i| t.junctions[*i].id == *id)
                .map_or(0, |n| n + 1)
        ),
    };
    t.connections
        .iter()
        .map(|c| (ew_api::output::ew_dl::EntityRef(c.id), end(&c.a), end(&c.b)))
        .collect()
}

/// The lines `page.describe` adds for a generated page (FT-660): its printed table, so a text-only reader can read a
/// parts list or any other list page.
///
/// ```text
/// table columns 3 rows 2
///   columns "Item" "Quantity" "Designation"
///   row "1" "2" "-101F1"
///   row group "Group heading"
/// note "No cables yet"                (instead of the rows, when the page prints a note)
/// ```
///
/// The cells are the folio's printed texts, quoted as user text; a heading row (a group header or footer) prints
/// `group` and its spanning text.
pub fn page_table_lines(table: &ew_api::output::PageListTable) -> Vec<String> {
    let mut lines = vec![format!(
        "table columns {} rows {}",
        table.columns.len(),
        table.rows.len()
    )];
    let quoted = |cells: &[String]| {
        cells
            .iter()
            .map(|c| format!("\"{}\"", user(c)))
            .collect::<Vec<_>>()
            .join(" ")
    };
    if !table.columns.is_empty() {
        lines.push(format!("  columns {}", quoted(&table.columns)));
    }
    for row in &table.rows {
        if row.heading {
            let text = row.cells.first().map_or("", String::as_str);
            lines.push(format!("  row group \"{}\"", user(text)));
        } else {
            lines.push(format!("  row {}", quoted(&row.cells)));
        }
    }
    if let Some(note) = &table.note {
        lines.push(format!("note \"{}\"", user(note)));
    }
    lines
}

/// The text of one page for `page.describe` (WP-S6.13 §5.5.3): the header with the exact counts, then the
/// representations with their connection points, the connections, the junctions, the page items and the structure
/// boxes, each in a named order (reading order of the anchor cell, then the position and the symbol key; connections
/// by their end labels; junctions in reading order; items and boxes in the page tables' order). The header's
/// `items <n> boxes <m>` follows `junctions <j>` only when the page has one of them, so a page without items prints
/// as before. No record ID appears.
pub fn page_describe(page: &CanvasPage) -> String {
    let t: &PageTables = &page.tables;
    let mut header = format!(
        "page {} {} reps {} connections {} junctions {}",
        page_quoted(&page.folio),
        page_quoted(&page.title),
        t.reps.len(),
        t.connections.len(),
        t.junctions.len()
    );
    if !t.items.is_empty() || !t.structure_boxes.is_empty() {
        header.push_str(&format!(
            " items {} boxes {}",
            t.items.len(),
            t.structure_boxes.len()
        ));
    }
    let mut lines = vec![header];

    // The junctions are numbered first: connections and sequences name them by number.
    let order = junction_order(page);
    let cp_label = |function, idx| {
        t.cps
            .iter()
            .find(|c| c.function == function && c.idx == idx)
            .map_or_else(String::new, |c| c.label.clone())
    };
    let number = |id| {
        order
            .iter()
            .position(|i| t.junctions[*i].id == id)
            .map_or(0, |n| n + 1)
    };
    let end_text = |e: &TableEnd| match e {
        TableEnd::Cp { function, idx } => page_quoted(&cp_label(*function, *idx)),
        TableEnd::Junction { id } => format!("junction {}", number(*id)),
    };

    // The representations.
    let mut reps: Vec<_> = t
        .reps
        .iter()
        .map(|r| {
            let mut cps: Vec<&TableCp> = t.cps.iter().filter(|c| c.rep == r.id).collect();
            cps.sort_by(|a, b| (a.idx, &a.label).cmp(&(b.idx, &b.label)));
            let tag = rep_tag(r, &cps);
            let key = (
                reading(page, r.pos),
                r.symbol_key.clone(),
                tag.clone().unwrap_or_default(),
                r.orient,
            );
            (key, r, cps, tag)
        })
        .collect();
    reps.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, r, cps, tag) in &reps {
        let who = match (&r.ip, tag) {
            (Some(ip), _) => format!("ip {} direction {}", page_quoted(&ip.name), ip.direction),
            (None, Some(tag)) => page_quoted(tag),
            (None, None) => "none".to_owned(),
        };
        lines.push(format!(
            "  rep {who} cell {} symbol {} variant {} {}",
            cell_word(page, r.pos),
            page_quoted(&r.symbol_key),
            page_quoted(&variant_letter(r.orient).to_string()),
            page_at(page, r.pos)
        ));
        for cp in cps {
            // G-PP triage row 21: a terminal CP names its side beside its clamp (`"1X1:1/a" internal`).
            let side = cp.side.map_or_else(String::new, |s| format!(" {s}"));
            let mut line = format!(
                "    cp {}{side} {} wires {}",
                page_quoted(&cp.label),
                page_at(page, cp.at),
                cp.connections.len()
            );
            if let Some(name) = &cp.potential {
                line.push_str(&format!(" potential {}", page_quoted(name)));
            }
            lines.push(line);
        }
    }

    // The connections, by their end labels.
    let mut wires: Vec<(String, String, bool, usize, String)> = t
        .connections
        .iter()
        .map(|c| {
            let (a, b) = (end_text(&c.a), end_text(&c.b));
            let bends = c.polyline.len().saturating_sub(2);
            let route = if c.stored_route { "stored" } else { "provisional" };
            // A point that picks this wire and nothing else (FT-485 item 4), where there is one.
            let point =
                wire_point(page, c).map_or_else(String::new, |p| format!(" at {} {} mm", mm3(p.0), mm3(p.1)));
            (
                a.clone(),
                b.clone(),
                c.stored_route,
                bends,
                format!("  wire {a} -> {b} route {route} bends {bends}{point}"),
            )
        })
        .collect();
    wires.sort();
    lines.extend(wires.into_iter().map(|w| w.4));

    // The junctions, numbered.
    for (n, i) in order.iter().enumerate() {
        let j = &t.junctions[*i];
        let members: Vec<String> = j
            .sequence
            .iter()
            .filter_map(|id| t.connections.iter().find(|c| c.id == *id))
            .map(|c| match (c.a, c.b) {
                (TableEnd::Junction { id }, other) if id == j.id => end_text(&other),
                (other, _) => end_text(&other),
            })
            .collect();
        let mut line = format!(
            "  junction {} cell {} {} style {} wiring {} sequence",
            n + 1,
            cell_word(page, j.at),
            page_at(page, j.at),
            j.style,
            j.wiring
        );
        for m in members {
            line.push(' ');
            line.push_str(&m);
        }
        lines.push(line);
    }

    // The page items, then the structure boxes, in the page tables' order (WP-S2.04 part 4b, §10.7).
    for item in &t.items {
        let word = match item.kind.as_str() {
            "graphic" => "graphic",
            "free_text" => "text",
            "wire_marker" => "marker",
            "image" => "image",
            _ => "unknown",
        };
        // G-PP triage row 12: the middle of the bounds, a point that picks the item, like a rep's or a wire's `at`.
        let [x0, y0, x1, y1] = item.bounds;
        let mid = (x0 + (x1 - x0) / 2, y0 + (y1 - y0) / 2);
        let mut line = format!(
            "  item {word} cell {} {}",
            cell_word(page, (x0, y0)),
            page_at(page, mid)
        );
        if matches!(word, "text" | "marker")
            && let Some(text) = &item.text
        {
            line.push(' ');
            line.push_str(&page_quoted(text));
        }
        lines.push(line);
    }
    for b in &t.structure_boxes {
        let top_left = b
            .outline
            .iter()
            .copied()
            .min_by_key(|(x, y)| (*y, *x))
            .unwrap_or_default();
        lines.push(format!(
            "  box structure cell {} {}",
            cell_word(page, top_left),
            page_quoted(&b.caption)
        ));
    }
    lines.join("\n")
}

/// The bridge lines `page.describe` appends (FT-621): one `  bridge "BR<n>" "<terminal>" …` per bridge group of which
/// the strip or a terminal is drawn on `page`, in `BR<n>` order, its terminals in (`seq`, ID) order. Bridges have no page (they are
/// defined in the strip editor), so the page lists the bridges of the strips it draws.
pub fn page_bridge_lines(page: &CanvasPage, bridges: &[ew_api::output::BridgeRow]) -> Vec<String> {
    bridges
        .iter()
        .filter(|b| {
            // A strip segment draws the strip's device; a terminal symbol draws the terminal's.
            page.tables.reps.iter().any(|r| {
                r.device
                    .is_some_and(|d| d == b.strip || b.terminals.iter().any(|(t, _)| *t == d))
            })
        })
        .map(|b| {
            let mut line = format!("  bridge {}", page_quoted(&b.label));
            for (_, name) in &b.terminals {
                line.push(' ');
                line.push_str(&page_quoted(name));
            }
            line
        })
        .collect()
}

/// The text of the visible user interface of `model`, lines separated by `\n`, without a final newline, with record IDs
/// as UUIDs: [`ShellModel::describe`] and the transcript print them as aliases `@n` (FT-422).
pub fn describe(model: &ShellModel) -> String {
    let mut lines: Vec<String> = vec![format!("title: {}", model.title())];
    for menu in actions::menu_model(model) {
        lines.push(strings::format("describe-menu", &[&menu.label]));
        for entry in &menu.entries {
            menu_entry_lines(entry, 1, &mut lines);
        }
    }
    // WP-S2.17 part 2: the command bar under the app menu.
    command_bar_lines(model, &mut lines);
    if let Some(banner) = model.banner() {
        lines.push(format!("banner: {banner}"));
    }
    if model.first_run().is_some() {
        first_run(model, &mut lines);
    } else if model.wizard().is_some() {
        wizard(model, &mut lines);
    } else if model.has_project() {
        project_view(model, &mut lines);
    } else {
        start_page(model, &mut lines);
        // WP-S2.09 part 1: a toast can outlive its project (an export's end, a recovery's failure).
        toast_lines(model, &mut lines);
    }
    // WP-S2.17 part 3 (D12): the status bar's fields in their order.
    let fields: Vec<String> = crate::status::bar(model, false)
        .into_iter()
        .map(|f| f.text)
        .collect();
    lines.push(format!("status bar: {}", fields.join(" | ")));
    // WP-S2.09 part 1: the notifications log is a popup.
    notification_lines(model, &mut lines);
    if let Some(palette) = model.palette() {
        lines.push(format!(
            "palette \"{}\": query \"{}\"",
            strings::text("palette-title"),
            palette.query
        ));
        let entries = model.palette_entries();
        if entries.is_empty() {
            lines.push(format!("  label \"{}\"", strings::text("palette-no-match")));
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
    dialog(model, &mut lines);
    if let Some(target) = model.last_help() {
        lines.push(format!("help: {target}"));
    }
    lines.join("\n")
}

impl ShellModel {
    /// The visible user interface as text, one line per element in paint order (`ui.describe`), every record ID
    /// printed as its alias `@n` (FT-422, ID-630).
    pub fn describe(&self) -> String {
        let text = describe(self);
        self.alias_text(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FX38d (play-test rv-vis-2 t0013): a pin whose designation the device shares is named by its position in words
    /// in a sentence, never by the label's `#n`; an unshared pin, a terminal's clamp and an interruption point keep
    /// their label.
    #[test]
    fn a_shared_pin_is_named_by_its_position_in_a_sentence() {
        assert_eq!(pin_in_sentence("-1A4:-#2"), "-1A4 pin - (2nd)");
        assert_eq!(pin_in_sentence("-1A4:+#1"), "-1A4 pin + (1st)");
        assert_eq!(pin_in_sentence("-101F1:2#3"), "-101F1 pin 2 (3rd)");
        assert_eq!(pin_in_sentence("-101K1:13#11"), "-101K1 pin 13 (11th)");
        assert_eq!(pin_in_sentence("-101K1:13#21"), "-101K1 pin 13 (21st)");
        assert_eq!(pin_in_sentence("-1A4:L+"), "-1A4:L+");
        assert_eq!(pin_in_sentence("-X1:1/a#2"), "-X1:1/a#2");
        assert_eq!(pin_in_sentence("0VDC"), "0VDC");
        assert!(!pin_in_sentence("-1A4:-#2").contains('#'));
    }

    /// FT-716: a sentence names a terminal by strip and number in words; a tag without a number stays as it is.
    #[test]
    fn a_terminal_in_a_sentence_reads_strip_terminal_number() {
        assert_eq!(terminal_in_sentence("-X1011:3"), "-X1011 terminal 3");
        assert_eq!(terminal_in_sentence("=A1-X2:PE"), "=A1-X2 terminal PE");
        assert_eq!(terminal_in_sentence("-X1011"), "-X1011");
        assert_eq!(terminal_in_sentence("-X1011:"), "-X1011:");
    }

    // WP-S2.09 part 3 (§8.7): a list prints at most 20 lines, then `more <word> <n>`.
    #[test]
    fn a_preview_list_prints_twenty_lines_then_more() {
        let items: Vec<u32> = (1..=23).collect();
        let mut lines = Vec::new();
        listed(&mut lines, &items, "tag", |n| format!("  tag \"{n}\" \"x\""));
        assert_eq!(lines.len(), 21);
        assert_eq!(lines[19], "  tag \"20\" \"x\"");
        assert_eq!(lines[20], "  more tag 3");
        let mut exact = Vec::new();
        listed(&mut exact, &items[..20], "tag", |n| n.to_string());
        assert_eq!(exact.len(), 20, "no `more` line at exactly 20");
    }

    /// The record kinds of the format (doc 05 §2.2), by their `$k` names: the 53 of `ew_schema::Kind::ALL`.
    const KINDS: [&str; 53] = [
        "project",
        "plc_config_project",
        "logical_net",
        "physical_net",
        "medium",
        "import_lineage",
        "structure_node",
        "section",
        "external_ref",
        "page",
        "device",
        "function",
        "representation",
        "connection",
        "junction",
        "conductor",
        "channel",
        "potential_def",
        "net_def",
        "medium_def",
        "bond",
        "device_link",
        "page_item",
        "structure_box",
        "report_block",
        "macro_instance",
        "placeholder_group",
        "layout_space",
        "enclosure",
        "mounting_surface",
        "carrier",
        "placement",
        "route_override",
        "committed_route",
        "routing_path",
        "surface_feature",
        "revision",
        "waiver",
        "comment",
        "transmittal",
        "exchange_record",
        "foreign_id",
        "import_run",
        "conflict",
        "part",
        "symbol",
        "fndef",
        "form",
        "macro",
        "manufacturer",
        "plc_profile",
        "machine_profile",
        "label_profile",
    ];

    /// WP-S2.05 §6.11: every implemented non-library kind (one the property grid has a catalogue for) names its
    /// section in the describe text, so its name is in the vocabulary; so are the two kinds of WP-S2.04 part 4a.
    #[test]
    fn every_kind_a_section_can_have_is_in_the_vocabulary() {
        let sheet_kinds: Vec<&str> = KINDS
            .iter()
            .copied()
            .filter(|k| ew_api::output::field_catalogue(k).is_some())
            .collect();
        assert!(!sheet_kinds.is_empty(), "{sheet_kinds:?}");
        for k in sheet_kinds.iter().chain(&["page_item", "structure_box"]) {
            assert!(VOCABULARY.contains(k), "`{k}` is not in VOCABULARY");
        }
    }
}
