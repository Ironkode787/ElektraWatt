// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The rule of `cargo xtask rules` that a greyed control says why (docs/design/04-architecture.md §13.3). An action of
//! the registry cannot be greyed without a reason: its `enabled` check returns the key of its reason, which is checked
//! at build. What the window greys on its own is checked here: every `add_enabled(` and `add_enabled_ui(` in
//! `crates/ew-ui/src` is followed, before the function it stands in ends and within [`REACH`] lines, by the reason of
//! its greyed state (`disabled_tip`, `disabled_tip_beside` or `on_disabled_hover_text`). The components whose caller
//! gives the reason, and the controls greyed without a reason in words, are named in [`WITHOUT`] with why.

use std::path::Path;

use anyhow::{Context, Result};

/// The scanned directory.
const SCANNED: &str = "crates/ew-ui/src";

/// The calls that grey a control.
const GREYING: &[&str] = &[".add_enabled(", ".add_enabled_ui("];

/// The calls that give a greyed control its reason.
const REASONS: &[&str] = &["disabled_tip", "on_disabled_hover_text"];

/// How many lines after the greying call its reason may stand.
const REACH: usize = 16;

/// The functions (file, function) that grey a control without a reason of their own, each with why.
const WITHOUT: &[(&str, &str, &str)] = &[
    (
        "components.rs",
        "button_with_icon",
        "a component: its caller gives the reason",
    ),
    (
        "components.rs",
        "icon_toggle_tip",
        "a component: its caller gives the reason",
    ),
    // Greyed without a reason in words today; a reason is a new text of the window, an item of the owner's list.
    (
        "nav_table.rs",
        "chooser",
        "the key column's check box, which cannot be unticked",
    ),
    (
        "notifications.rs",
        "one_link",
        "a toast's action that no longer applies",
    ),
    (
        "print_dialog.rs",
        "sections",
        "the section boxes while the range is not Sections",
    ),
    (
        "print_dialog.rs",
        "pages_field",
        "the Pages field while the range is not Pages",
    ),
    (
        "symbol_editor/panels.rs",
        "sheet_row",
        "a locked row of the symbol's sheet",
    ),
    ("symbol_editor.rs", "toolbar", "Make a part without a project"),
    (
        "wizard.rs",
        "step_form",
        "the wizard's choice after its project's folder exists",
    ),
];

/// The violations: each greying call without a reason, by file, line and function.
pub(crate) fn check(root: &Path) -> Result<Vec<String>> {
    let mut files = Vec::new();
    rust_files(&root.join(SCANNED), &mut files)?;
    let mut out = Vec::new();
    for path in files {
        let rel = path
            .strip_prefix(root.join(SCANNED))
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let src = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        out.extend(findings(&rel, &src));
    }
    Ok(out)
}

/// The greying calls of the file `rel` (its text `src`) without a reason, outside its `#[cfg(test)]` module.
fn findings(rel: &str, src: &str) -> Vec<String> {
    let lines: Vec<&str> = src.lines().collect();
    let end = lines
        .iter()
        .position(|l| l.trim() == "#[cfg(test)]")
        .unwrap_or(lines.len());
    let mut out = Vec::new();
    for (i, line) in lines[..end].iter().enumerate() {
        if line.trim_start().starts_with("//") || !GREYING.iter().any(|g| line.contains(g)) {
            continue;
        }
        let function = lines[..=i].iter().rev().find_map(|l| fn_name(l)).unwrap_or("");
        if WITHOUT.iter().any(|(f, n, _)| *f == rel && *n == function) {
            continue;
        }
        let reach = lines[i..end.min(i + REACH)]
            .iter()
            .take_while(|l| !l.starts_with('}'));
        if reach.clone().any(|l| REASONS.iter().any(|r| l.contains(r))) {
            continue;
        }
        out.push(format!(
            "crates/ew-ui/src/{rel}:{}: `{function}` greys a control without saying why: give it its reason \
             (`disabled_tip`), or name the function in xtask/src/greyed.rs with why it has none",
            i + 1
        ));
    }
    out
}

/// The name of the function a line declares, if it declares one.
fn fn_name(line: &str) -> Option<&str> {
    let at = line.find("fn ")?;
    let before = line[..at].trim();
    if !(before.is_empty()
        || before
            .split_whitespace()
            .all(|w| w.starts_with("pub") || w == "const"))
    {
        return None;
    }
    let rest = &line[at + 3..];
    let name = rest.split(|c: char| !(c.is_alphanumeric() || c == '_')).next()?;
    (!name.is_empty()).then_some(name)
}

/// The `.rs` files under `dir`, sorted.
fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            rust_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out.sort();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::findings;

    /// A greying call with its reason passes, one without is named by its line and function, and a comment is no
    /// call.
    #[test]
    fn a_greyed_control_without_its_reason_is_named() {
        let src = "fn shown(ui: &mut Ui) {\n    let b = ui.add_enabled(on, Button::new(text));\n    \
                   b.disabled_tip(reason);\n}\n\nfn silent(ui: &mut Ui) {\n    // ui.add_enabled(on, x)\n    \
                   ui.add_enabled(on, Button::new(text));\n}\n";
        assert_eq!(
            findings("panel.rs", src),
            [
                "crates/ew-ui/src/panel.rs:8: `silent` greys a control without saying why: give it its reason \
              (`disabled_tip`), or name the function in xtask/src/greyed.rs with why it has none"
            ]
        );
    }
}
