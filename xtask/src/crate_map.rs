// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gen-maps`, its second half: the crate map `docs/map/crates.md`, every workspace crate with the first sentences
//! of its crate comment, its licence, its tier, the workspace crates it depends on and is used by, and under it each
//! source file with the first sentence of its head comment, a file of more than 1,000 lines marked large. At its head
//! stands the order of the crates, a table of the tiers.
//!
//! It is read from the manifests (`cargo metadata --no-deps`), the tiers of `ci/dep-rules.toml` and the `//!` heads of
//! the files under each crate's `src/`; nothing is built. A file whose head is missing or too short to say what it
//! holds is listed under "No head comment" and the map is written all the same. No size is written but the mark, so
//! that the map changes when a file crosses that line, not with every edit. Not here: the action map
//! (`action_map.rs`); the check of the order and of the other rules between crates (`deps.rs`).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result};
use cargo_metadata::DependencyKind;

/// The generated map, relative to the workspace root.
pub(crate) const OUTPUT: &str = "docs/map/crates.md";
/// The workspace-hack crate, which every crate depends on for its features alone: left out of the edges.
const HACK: &str = "ew-hack";
/// A head of fewer words says too little to tell what the file holds.
const MIN_HEAD_WORDS: usize = 4;

/// The first paragraph of a file's `//!` head, its lines joined: the head is the first run of `//!` lines, after the
/// licence header and any blank line or attribute.
fn head_paragraph(text: &str) -> Option<String> {
    let mut lines = text
        .lines()
        .map(str::trim_start)
        .skip_while(|l| !l.starts_with("//!"))
        .map_while(|l| l.strip_prefix("//!"))
        .map(str::trim);
    let mut out = Vec::new();
    for line in lines.by_ref() {
        if line.is_empty() {
            if out.is_empty() {
                continue;
            }
            break;
        }
        out.push(line);
    }
    (!out.is_empty()).then(|| out.join(" "))
}

/// The first `n` sentences of `text`: a sentence ends at a full stop, a question or an exclamation mark followed by
/// a space, or at the end.
pub(crate) fn sentences(text: &str, n: usize) -> String {
    let mut ends = 0;
    for (i, c) in text.char_indices() {
        if matches!(c, '.' | '?' | '!') && text[i + 1..].starts_with(' ') {
            ends += 1;
            if ends == n {
                return text[..=i].to_owned();
            }
        }
    }
    text.to_owned()
}

/// A table cell: no pipe breaks the row.
fn cell(text: &str) -> String {
    text.replace('|', "\\|")
}

/// A file of more lines than this is marked large in the map: read it by range.
const LARGE: usize = 1000;

/// Whether a head says too little to tell what its file holds.
fn thin(head: Option<&str>) -> bool {
    head.is_none_or(|h| h.split_whitespace().count() < MIN_HEAD_WORDS)
}

/// One workspace crate as the map shows it.
struct Crate {
    name: String,
    /// Its folder, relative to the root.
    dir: String,
    licence: String,
    /// The path of its root file (`src/lib.rs`, else `src/main.rs`), relative to its folder.
    root_file: String,
    depends_on: BTreeSet<String>,
    /// The workspace crates its tests depend on.
    dev_depends_on: BTreeSet<String>,
}

fn crates(root: &Path) -> Result<Vec<Crate>> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .cargo_path(crate::cargo())
        .manifest_path(root.join("Cargo.toml"))
        .no_deps()
        .exec()
        .context("running `cargo metadata --no-deps`")?;
    let members: BTreeSet<&str> = metadata.packages.iter().map(|p| p.name.as_str()).collect();
    let edges = |p: &cargo_metadata::Package, kind: fn(DependencyKind) -> bool| -> BTreeSet<String> {
        p.dependencies
            .iter()
            .filter(|d| kind(d.kind) && d.name != HACK && members.contains(d.name.as_str()))
            .map(|d| d.name.clone())
            .collect()
    };
    let mut out = Vec::new();
    for p in &metadata.packages {
        if p.name.as_str() == HACK {
            continue;
        }
        let folder = p
            .manifest_path
            .parent()
            .context("a manifest has a folder")?
            .as_std_path();
        let dir = folder
            .strip_prefix(root)
            .with_context(|| format!("{} is outside the workspace", folder.display()))?
            .to_string_lossy()
            .replace('\\', "/");
        let root_file = if root.join(&dir).join("src/lib.rs").exists() {
            "src/lib.rs"
        } else {
            "src/main.rs"
        };
        out.push(Crate {
            name: p.name.to_string(),
            dir,
            licence: p.license.clone().unwrap_or_else(|| "none".to_owned()),
            root_file: root_file.to_owned(),
            depends_on: edges(p, |k| k != DependencyKind::Development),
            dev_depends_on: edges(p, |k| k == DependencyKind::Development),
        });
    }
    out.sort_by(|a, b| a.dir.cmp(&b.dir));
    Ok(out)
}

fn names(set: &BTreeSet<String>) -> String {
    if set.is_empty() {
        "none".to_owned()
    } else {
        set.iter()
            .map(|n| format!("`{n}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The map's text.
fn generate(root: &Path) -> Result<String> {
    let crates = crates(root)?;
    let tiers = crate::deps::Policy::load(&root.join("ci/dep-rules.toml"))?.tiers;
    let tier_of = |name: &str| {
        tiers
            .iter()
            .find(|t| t.crates.iter().any(|c| c == name))
            .map_or("none", |t| t.name.as_str())
    };
    let mut used_by: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let mut tested_by: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for c in &crates {
        for d in &c.depends_on {
            used_by.entry(d.as_str()).or_default().insert(c.name.clone());
        }
        for d in c.dev_depends_on.difference(&c.depends_on) {
            tested_by.entry(d.as_str()).or_default().insert(c.name.clone());
        }
    }
    let none = BTreeSet::new();
    let mut out = String::from(
        "# The crate map\n\n\
         Generated by `cargo xtask gen-maps` (`xtask/src/crate_map.rs`) from the manifests, the tiers of \
         `ci/dep-rules.toml` and the head comments; do \
         not edit it. Every workspace crate, by folder: the first sentences of its crate comment, its licence, its \
         tier, the workspace crates it depends on, the ones that depend on it and the ones whose tests alone do; \
         under it each file of its `src/` with the first sentence of its head comment, marked large above 1,000 \
         lines (read it by range). The rules on these edges are in `ci/dep-rules.toml` and \
         [the architecture](../design/04-architecture.md) §3.\n\n\
         ## The order\n\n\
         A crate depends (normal and build dependencies) only on crates before it: in an earlier tier, or earlier \
         in its own tier's list.\n\n\
         | Tier | Crates |\n|---|---|\n",
    );
    for tier in &tiers {
        let names: Vec<String> = tier.crates.iter().map(|c| format!("`{c}`")).collect();
        writeln!(out, "| {} | {} |", tier.name, names.join(", "))?;
    }
    let mut missing = Vec::new();
    for c in &crates {
        let files = crate::action_map::rust_files(root, &format!("{}/src", c.dir))?;
        let root_head = files
            .iter()
            .find(|(f, _)| *f == format!("{}/{}", c.dir, c.root_file))
            .and_then(|(_, t)| head_paragraph(t));
        writeln!(out, "\n## {}\n", c.name)?;
        writeln!(
            out,
            "`{}` · {} · tier {} · depends on {} · used by {} · in tests by {}\n",
            c.dir,
            c.licence,
            tier_of(&c.name),
            names(&c.depends_on),
            names(used_by.get(c.name.as_str()).unwrap_or(&none)),
            names(tested_by.get(c.name.as_str()).unwrap_or(&none))
        )?;
        if let Some(head) = &root_head {
            writeln!(out, "{}\n", sentences(head, 2))?;
        }
        out.push_str("| File | Size | What it holds |\n|---|---|---|\n");
        for (file, text) in &files {
            let head = head_paragraph(text);
            let rel = file.strip_prefix(&format!("{}/", c.dir)).unwrap_or(file);
            let what = head
                .as_deref()
                .map(|h| cell(&sentences(h, 1)))
                .unwrap_or_default();
            let large = if text.lines().count() > LARGE { "large" } else { "" };
            writeln!(out, "| `{rel}` | {large} | {what} |")?;
            if thin(head.as_deref()) {
                missing.push(file.clone());
            }
        }
    }
    out.push_str("\n## No head comment\n\nThe files whose head is missing or says too little to tell what they hold.\n\n");
    if missing.is_empty() {
        out.push_str("None.\n");
    }
    for file in &missing {
        writeln!(out, "- `{file}`")?;
    }
    Ok(out)
}

/// The problems of `gen-maps --check` for this map: empty when the committed map is the one the tree gives.
pub(crate) fn violations(root: &Path) -> Result<Vec<String>> {
    let wanted = generate(root)?;
    let have = std::fs::read_to_string(root.join(OUTPUT))
        .ok()
        .map(|t| t.replace("\r\n", "\n"));
    Ok(if have.as_deref() == Some(wanted.as_str()) {
        Vec::new()
    } else {
        vec![format!("{OUTPUT} differs; run `cargo xtask gen-maps`")]
    })
}

/// Writes the map.
pub(crate) fn write(root: &Path) -> Result<()> {
    let path = root.join(OUTPUT);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    crate::repo::write_atomic(&path, &generate(root)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_head_is_the_first_paragraph_after_the_licence() {
        let text = "// SPDX-License-Identifier: MIT\n\n//! The registry. It holds\n//! every action.\n//!\n//! More.\n";
        assert_eq!(
            head_paragraph(text).as_deref(),
            Some("The registry. It holds every action.")
        );
        assert_eq!(
            sentences("The registry. It holds every action.", 1),
            "The registry."
        );
        assert_eq!(head_paragraph("// SPDX\nfn main() {}\n"), None);
        assert!(thin(Some("Tests.")) && thin(None) && !thin(Some("The action table in its sections.")));
    }
}
