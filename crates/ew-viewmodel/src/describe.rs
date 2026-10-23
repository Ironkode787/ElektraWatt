// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `ui.describe` text (WP-S2.01 part 1, spec section 6.2): the visible user interface as text, one line per
//! element, in paint order. It is what an agent reads instead of a screenshot.
//!
//! Paint order: the window title, the menu bar with every entry and its enabled state (and reason), the banner,
//! the first-run flow, the wizard, or the project view (the page navigator, the canvas tabs and the focus, the page
//! view and its canvas lines, the workspace and its layout), or the start page, the status bar, then the palette or a dialog, then the last documentation request.
//! Directory names appear, paths never. The structural words are fixed English machine text (decision log ID-474):
//! [`VOCABULARY`] lists every one of them, [`FREE_TEXT_MARKERS`] the markers after which the rest of a line is user
//! text, and a test checks that no line the describer writes uses a word that is in neither list. Quoted text is
//! user text (a label, a name) and not part of the vocabulary.

use crate::actions::{self, ActionItem, MenuEntry, PaletteRun};
use crate::canvas::PageId;
use crate::canvas::{mm3, round_tenth};
use crate::first_run::FirstRunStep;
use crate::navigator::RowKind;
use crate::properties::{Provenance, RowValue};
use crate::settings::{KEYMAPS, LANGUAGES, THEMES};
use crate::shell::{Dialog, Focus, ShellModel};
use crate::strings;
use crate::wizard::{FieldKind, Step, fields_of};

/// The structural words of the describe text: the tokens of a line once the quoted texts, the numbers, the action
/// IDs (they contain a dot), the shortcuts and the punctuation `( ) [ ] , | %` are taken out. A token with a colon
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
];

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

/// The 28 words WP-S2.04 part 1 adds to [`VOCABULARY`] (§7.7).
pub const PLACE_WORDS: &[&str] = &[
    "picker", "rows", "symbol", "part", "function", "main", "embedded", "starter", "ghost", "place",
    "variant", "rep", "target", "new", "join", "existing", "terminal", "strip", "letter", "tag", "move",
    "steps", "exchange", "unmapped", "floating", "dragging", "moving", "readout",
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

/// The navigator lines (WP-S2.03 §6.10): the header, then one line per row on show.
fn navigator_lines(model: &ShellModel, lines: &mut Vec<String>) {
    let nav = model.navigator();
    lines.push(format!(
        "navigator \"{}\" mode {} sort {} filter \"{}\"",
        strings::text("pages-title"),
        nav.mode.word(),
        nav.sort.word(),
        nav.filter
    ));
    let rows = model.page_rows();
    let current = model.current_page();
    let cursor = model.navigator_row_key();
    for row in model.navigator_rows() {
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
                if current == Some(PageId::of(r)) {
                    line.push_str(" current");
                }
            }
        }
        if cursor.as_deref() == Some(row.key.as_str()) {
            line.push_str(" cursor");
        }
        lines.push(line);
    }
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
        Some(Focus::Navigator) => lines.push("focus navigator".to_owned()),
        Some(Focus::Canvas(g)) => lines.push(format!("focus canvas group {}", g + 1)),
        Some(Focus::Properties) => lines.push("focus properties".to_owned()),
        // WP-S2.05 part 2
        Some(Focus::Find) => lines.push("focus find".to_owned()),
        None => {}
    }
}

/// A user text of the property grid's lines as the describe text quotes it: a double quote becomes a single one (the
/// quoting stays unambiguous) and a hyphenated UUID becomes `‹id›` (no UUID in the describe text, WP-S2.05 §9 item 5).
fn user(text: &str) -> String {
    crate::properties::mask_uuids(text).replace('"', "'")
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
        for (category, rows) in crate::properties::categories(section) {
            let key = crate::properties::collapse_key(&section.kind, category);
            let shut = collapsed.contains(&key);
            lines.push(format!(
                "  category \"{}\" {}",
                strings::text(&format!("category-{}", category.word())),
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
            Ok(_) => lines.push(format!(
                "card {} \"{}\" targets {} changes {} tags {} xrefs {}",
                card.preview.status(rev).word(),
                user(&card.preview.name),
                card.targets,
                card.changes,
                card.preview.derived.tags.len(),
                card.preview.derived.xrefs.len()
            )),
        }
    }
    if let Some(t) = props.toast() {
        lines.push(format!(
            "toast \"{}\"{}",
            user(&t.text),
            if t.undo { " undo" } else { "" }
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
    navigator_lines(model, lines);
    properties_lines(model, lines);
    // WP-S2.05 part 2
    find_lines(model, lines);
    card_and_toast_lines(model, lines);
    tab_lines(model, lines);
    match model.current_folio() {
        Some(folio) => lines.push(format!("page view: {folio}")),
        None => lines.push(format!("page view: {}", strings::text("page-no-pages"))),
    }
    canvas_lines(model, lines);
    lines.push(format!("workspace \"{}\"", model.workspace()));
    lines.push(format!("layout: {}", model.layout().describe()));
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
            "canvas hover \"{}\" box {} {} {} {} mm",
            entity.0,
            mm3(r.min().x.0),
            mm3(r.min().y.0),
            mm3(r.max().x.0),
            mm3(r.max().y.0)
        )),
        None => lines.push("canvas hover none".to_owned()),
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
    tool_lines(model, lines);
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
        let ids: Vec<String> = selection.items.iter().map(|e| format!("\"{}\"", e.0)).collect();
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
        }
        Some(crate::tools::Ghost::Move {
            count,
            steps,
            readout,
        }) => lines.push(format!(
            "canvas ghost move {count} steps {} {} readout \"{readout}\"",
            steps.0, steps.1
        )),
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
    }
}

fn dialog(model: &ShellModel, lines: &mut Vec<String>) {
    match model.dialog() {
        None => {}
        Some(Dialog::About) => {
            lines.push(format!("dialog \"{}\"", strings::text("about-title")));
            for line in crate::about::about_lines(model) {
                lines.push(format!("  label \"{line}\""));
            }
            lines.push(format!("  button \"{}\"", strings::text("about-privacy-button")));
            lines.push(format!("  button \"{}\"", strings::text("about-notices-button")));
            lines.push(format!("  button \"{}\"", strings::text("about-docs-button")));
            lines.push(format!("  button \"{}\"", strings::text("dialog-close")));
        }
        Some(Dialog::Notices) => {
            lines.push(format!("dialog \"{}\"", strings::text("notices-title")));
            let text = crate::about::notices_text(model);
            lines.push(format!("  notices: {} lines", text.lines().count()));
            lines.push(format!("  button \"{}\"", strings::text("dialog-close")));
        }
        Some(Dialog::Shortcuts) => {
            lines.push(format!("dialog \"{}\"", strings::text("shortcuts-title")));
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
            lines.push(format!("  button \"{}\"", strings::text("dialog-close")));
        }
        Some(Dialog::ConfirmDiscard { pending }) => {
            lines.push(format!("dialog \"{}\"", strings::text("confirm-title")));
            lines.push(format!("  label \"{}\"", strings::text("confirm-text")));
            lines.push(format!("  pending {}", pending.action));
            for key in ["dialog-save", "dialog-discard", "dialog-cancel"] {
                lines.push(format!("  button \"{}\"", strings::text(key)));
            }
        }
        Some(Dialog::ConfirmCheckScheme { scheme }) => {
            lines.push(format!(
                "dialog \"{}\"",
                strings::text("confirm-check-scheme-title")
            ));
            lines.push(format!(
                "  label \"{}\"",
                strings::format("confirm-check-scheme-text", &[scheme])
            ));
            for key in ["dialog-apply", "dialog-cancel"] {
                lines.push(format!("  button \"{}\"", strings::text(key)));
            }
        }
        Some(Dialog::ConfirmJournalDiscard) => {
            lines.push(format!("dialog \"{}\"", strings::text("confirm-journal-title")));
            lines.push(format!("  label \"{}\"", strings::text("confirm-journal-text")));
            for key in ["dialog-discard", "dialog-cancel"] {
                lines.push(format!("  button \"{}\"", strings::text(key)));
            }
        }
        Some(Dialog::GotoPage { text }) => {
            lines.push(format!("dialog \"{}\"", strings::text("goto-title")));
            lines.push(format!("  field \"{}\": \"{text}\"", strings::text("goto-hint")));
            lines.push(format!("  button \"{}\"", strings::text("dialog-go")));
            lines.push(format!("  button \"{}\"", strings::text("dialog-cancel")));
        }
        Some(Dialog::SaveWorkspace { text }) => {
            lines.push(format!("dialog \"{}\"", strings::text("save-workspace-title")));
            lines.push(format!(
                "  field \"{}\": \"{text}\"",
                strings::text("save-workspace-hint")
            ));
            lines.push(format!("  button \"{}\"", strings::text("dialog-save-workspace")));
            lines.push(format!("  button \"{}\"", strings::text("dialog-cancel")));
        }
    }
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
    if model.first_run().is_some() {
        first_run(model, &mut lines);
    } else if model.wizard().is_some() {
        wizard(model, &mut lines);
    } else if model.has_project() {
        project_view(model, &mut lines);
    } else {
        start_page(model, &mut lines);
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
    /// The visible user interface as text, one line per element in paint order (`ui.describe`).
    pub fn describe(&self) -> String {
        describe(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(sheet_kinds.len(), 19, "{sheet_kinds:?}");
        for k in sheet_kinds.iter().chain(&["page_item", "structure_box"]) {
            assert!(VOCABULARY.contains(k), "`{k}` is not in VOCABULARY");
        }
    }
}
