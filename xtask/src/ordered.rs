// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-ordered-collections`: the review list of the ordered-collection lint (FT-271, WP-S6.02).
//!
//! The iteration order of a `HashMap` or `HashSet` is unspecified and differs between runs, so it must not
//! reach a persisted, exported or printed sequence (NFR on deterministic output, doc 18 §3). Clippy enforces
//! that for every workspace crate: the generated `clippy.toml` (`check-raw-read`, [`crate::raw_read`]) lists
//! the std hash containers under `disallowed-types`. A use site whose iteration order provably never reaches
//! output (a lookup-only map, a counter, a set used only for `contains`) is allowed with
//! `#[allow(clippy::disallowed_types)]` and a one-line justification comment (or a `reason = "..."` in the
//! attribute); anything else uses a `BTreeMap` or `BTreeSet`.
//!
//! This check keeps those allow sites reviewed. It scans the Rust source of every workspace package, tests
//! and benches included, for attributes `#[allow(...)]`, `#![allow(...)]`, `#[expect(...)]` (also inside
//! `cfg_attr`) that name `disallowed_types` (with or without `clippy::`), derives a key for each site, prints
//! the sites per crate and compares them with the committed list `ci/ordered-collections-allow.toml`.
//!
//! A key is the crate and the item path of the site, never a file or a line: the names of the enclosing
//! `mod`, `impl`, `fn`, `struct`, `enum` and `trait` items followed by the item the attribute decorates
//! (`fn parse_denylist`, `use std::collections::HashSet`, `struct Store > field holds`, `impl Store > fn
//! new`). The list holds the number of sites per key. The check fails when
//!
//! - a site is not in the list, or the source has more sites of a key than the list (a new allow must be
//!   reviewed: add the entry in the same change; `--bless` rewrites the list);
//! - the list names a key the source has fewer sites of (a stale entry);
//! - a site has no justification: no `//` comment on the line above or on the attribute's line, and no
//!   `reason = "..."` in the attribute;
//! - a `Cargo.toml` of the workspace sets the lint (`[lints.clippy] disallowed_types = "allow"`), which
//!   would switch the list off without a site to review.
//!
//! The scan is lexical (the lexer of [`crate::raw_read`]): comments and string contents never count. A site
//! that a macro expands is not seen; the lint itself still fires there, and the macro's definition site is
//! the place to allow it.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::raw_read::{self, Attr, Parsed, Tk};

/// The committed list of reviewed allow sites, relative to the workspace root.
pub const LIST_FILE: &str = "ci/ordered-collections-allow.toml";

/// The lint of the hash-container ban.
const LINT: &str = "disallowed_types";

/// The authority each message cites.
const REFERENCE: &str = "FT-271, WP-S6.02";

/// The key of an allow site: crate and item path (no file, no line).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Site {
    /// The workspace package.
    pub krate: String,
    /// The item path: enclosing items, then the decorated item.
    pub item: String,
}

/// One allow site found in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The key.
    pub site: Site,
    /// The workspace-relative file (for messages only).
    pub path: String,
    /// The 1-based line of the attribute (for messages only).
    pub line: usize,
    /// Whether a comment or a `reason` justifies the site.
    pub justified: bool,
}

/// A workspace package as the check sees it.
#[derive(Debug, Clone)]
pub struct Package {
    /// The package name.
    pub name: String,
    /// The manifest path relative to the workspace root, forward slashes.
    pub manifest: String,
    /// The manifest text.
    pub manifest_text: String,
    /// Every `.rs` file of the package: workspace-relative path and text.
    pub files: Vec<(String, String)>,
}

/// One entry of the committed list.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// The package.
    #[serde(rename = "crate")]
    pub krate: String,
    /// The item path.
    pub item: String,
    /// How many sites of this key the source has.
    pub count: usize,
}

/// The committed list.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct List {
    /// The reviewed sites.
    #[serde(default)]
    pub allow: Vec<Entry>,
}

impl List {
    /// Parses the list file.
    ///
    /// # Errors
    ///
    /// A TOML or schema error, or a key listed twice.
    pub fn parse(text: &str) -> Result<List> {
        let list: List = toml::from_str(text).with_context(|| format!("parsing {LIST_FILE}"))?;
        let mut seen = BTreeMap::new();
        for e in &list.allow {
            if seen.insert((e.krate.as_str(), e.item.as_str()), ()).is_some() {
                anyhow::bail!(
                    "{LIST_FILE} lists `{}` of {} twice; merge the counts",
                    e.item,
                    e.krate
                );
            }
        }
        Ok(list)
    }

    fn counts(&self) -> BTreeMap<Site, usize> {
        self.allow
            .iter()
            .map(|e| {
                (
                    Site {
                        krate: e.krate.clone(),
                        item: e.item.clone(),
                    },
                    e.count,
                )
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------------------
// Scanning

/// An item with a body: open and close brace tokens and the label shown in item paths.
struct Scope {
    open: usize,
    close: usize,
    label: String,
}

/// `kw name`, for a keyword followed by an identifier.
fn named(p: &Parsed, kw: usize) -> Option<String> {
    let mut name = kw + 1;
    // `static mut X`
    if p.is_ident(name, "mut") {
        name += 1;
    }
    Some(format!("{} {}", p.ident(kw)?, p.ident(name)?))
}

/// The label of the `impl` whose keyword is at `kw`: `impl Type` or `impl Trait for Type` (the last path
/// segment of each side, generics left out), and the index of the body's `{`.
fn impl_label(p: &Parsed, kw: usize) -> Option<(String, usize)> {
    let mut j = kw + 1;
    if p.is_p(j, '<') {
        j = raw_read::skip_angle(p, j);
    }
    let start = j;
    let mut body = None;
    let mut depth = 0usize;
    let mut for_at = None;
    while j < p.toks.len() {
        match p.toks[j].kind {
            Tk::Punct('{') if depth == 0 => {
                body = Some(j);
                break;
            }
            Tk::Punct(';') if depth == 0 => break,
            Tk::Punct('<') => depth += 1,
            Tk::Punct('>') if !(j > 0 && p.is_p(j - 1, '-')) => depth = depth.saturating_sub(1),
            Tk::Punct('(' | '[') => {
                j = p.pair[j].map_or(p.toks.len(), |m| m + 1);
                continue;
            }
            Tk::Ident(ref w) if depth == 0 && w == "for" => for_at = Some(j),
            _ => {}
        }
        j += 1;
    }
    let body = body?;
    let owner = raw_read::impl_owner(p, start, body)?;
    let label = match for_at.and_then(|f| raw_read::impl_owner(p, start, f)) {
        Some(tr) => format!("impl {tr} for {owner}"),
        None => format!("impl {owner}"),
    };
    Some((label, body))
}

/// Whether the token before `i` lets an item start at `i`.
fn item_position(p: &Parsed, i: usize) -> bool {
    i == 0
        || p.is_p(i - 1, '{')
        || p.is_p(i - 1, '}')
        || p.is_p(i - 1, ';')
        || p.is_p(i - 1, ']')
        || p.is_p(i - 1, ')')
        || ["pub", "unsafe", "const", "async", "extern"]
            .iter()
            .any(|w| p.is_ident(i - 1, w))
        || matches!(p.toks[i - 1].kind, Tk::Str(_))
}

/// The items of the file that have a body, with their labels.
fn scopes(p: &Parsed) -> Vec<Scope> {
    let mut out = Vec::new();
    for i in 0..p.toks.len() {
        let Some(kw) = p.ident(i) else { continue };
        if !matches!(kw, "fn" | "mod" | "impl" | "struct" | "enum" | "trait") || !item_position(p, i) {
            continue;
        }
        let (label, mut j) = if kw == "impl" {
            match impl_label(p, i) {
                Some((label, body)) => (label, body),
                None => continue,
            }
        } else {
            let Some(label) = named(p, i).filter(|_| p.ident(i + 1).is_some()) else {
                continue;
            };
            (label, i + 2)
        };
        // The body is the first `{` that is not inside parentheses or brackets; a `;` first means no body.
        let mut open = None;
        if kw == "impl" {
            open = Some(j);
        } else {
            while j < p.toks.len() {
                match p.toks[j].kind {
                    Tk::Punct('{') => {
                        open = Some(j);
                        break;
                    }
                    Tk::Punct(';') => break,
                    Tk::Punct('(' | '[') => j = p.pair[j].map_or(p.toks.len(), |m| m + 1),
                    _ => j += 1,
                }
            }
        }
        let Some(open) = open else { continue };
        let Some(close) = p.pair[open] else { continue };
        out.push(Scope { open, close, label });
    }
    out
}

/// The text of a `use` item that starts at the keyword `kw`, up to its `;`.
fn use_text(p: &Parsed, kw: usize) -> String {
    let mut out = String::from("use ");
    let mut j = kw + 1;
    while j < p.toks.len() && !p.is_p(j, ';') {
        match &p.toks[j].kind {
            Tk::Ident(s) => out.push_str(s),
            Tk::Punct(':') => out.push(':'),
            Tk::Punct('{') => out.push('{'),
            Tk::Punct('}') => out.push('}'),
            Tk::Punct(',') => out.push_str(", "),
            Tk::Punct('*') => out.push('*'),
            _ => {}
        }
        j += 1;
    }
    out
}

/// What the item at `j` (after its attributes) is, in one short phrase.
fn describe(p: &Parsed, j: usize) -> String {
    let mut j = p.skip_visibility(j);
    // Qualifiers before the keyword: `const fn`, `async fn`, `unsafe fn`, `extern "C" fn`, `default fn`.
    while let Some(w) = p.ident(j) {
        let qualifier = matches!(w, "async" | "unsafe" | "extern" | "default")
            || (w == "const"
                && p.ident(j + 1)
                    .is_some_and(|n| matches!(n, "fn" | "unsafe" | "async")));
        if !qualifier {
            break;
        }
        j += 1;
        if matches!(p.toks.get(j).map(|t| &t.kind), Some(Tk::Str(_))) {
            j += 1;
        }
    }
    match p.ident(j) {
        Some("fn" | "struct" | "enum" | "trait" | "mod" | "union" | "type" | "static" | "const") => {
            named(p, j).unwrap_or_else(|| "item".to_owned())
        }
        Some("impl") => impl_label(p, j).map_or_else(|| "impl".to_owned(), |(label, _)| label),
        Some("use") => use_text(p, j),
        Some("let") => {
            let name = if p.is_ident(j + 1, "mut") { j + 2 } else { j + 1 };
            format!("let {}", p.ident(name).unwrap_or("_"))
        }
        // A struct field or a pattern: `name: Type`.
        Some(name) if p.is_p(j + 1, ':') && !p.is_p(j + 2, ':') => format!("field {name}"),
        Some(name) => format!("statement {name}"),
        None => "statement".to_owned(),
    }
}

/// Whether the attribute names the lint in an `allow(...)` or `expect(...)` list, and whether it carries a
/// `reason = "..."`.
fn names_lint(p: &Parsed, a: &Attr) -> (bool, bool) {
    let (mut hit, mut reason) = (false, false);
    for k in a.open + 1..a.close {
        if !p.ident(k).is_some_and(|n| matches!(n, "allow" | "expect")) || !p.is_p(k + 1, '(') {
            continue;
        }
        let Some(close) = p.pair[k + 1] else { continue };
        let mut listed = false;
        let mut j = k + 2;
        while j < close {
            let Some(first) = p.ident(j) else {
                j += 1;
                continue;
            };
            let mut lint = first.to_owned();
            j += 1;
            while p.is_p(j, ':')
                && p.is_p(j + 1, ':')
                && let Some(segment) = p.ident(j + 2)
            {
                lint.push_str("::");
                lint.push_str(segment);
                j += 3;
            }
            if lint == "reason" && p.is_p(j, '=') {
                reason = true;
            } else if lint == LINT || lint == format!("clippy::{LINT}") {
                listed = true;
            }
        }
        hit |= listed;
    }
    (hit, hit && reason)
}

/// The allow sites of `krate`'s file `path` with text `text`.
pub fn scan_source(krate: &str, path: &str, text: &str) -> Vec<Found> {
    let p = Parsed::new(text);
    let scopes = scopes(&p);
    let lines: Vec<&str> = text.lines().collect();
    let mut found = Vec::new();
    for i in 0..p.toks.len() {
        let Some(a) = p.attr_at(i) else { continue };
        let (hit, reason) = names_lint(&p, &a);
        if !hit {
            continue;
        }
        let target = if a.inner {
            "inner attribute".to_owned()
        } else {
            describe(&p, p.skip_attrs(a.close + 1))
        };
        let mut chain: Vec<&Scope> = scopes.iter().filter(|s| s.open < i && i < s.close).collect();
        chain.sort_by_key(|s| s.open);
        let mut item = chain
            .iter()
            .map(|s| s.label.as_str())
            .collect::<Vec<_>>()
            .join(" > ");
        if !item.is_empty() {
            item.push_str(" > ");
        }
        item.push_str(&target);
        let line = p.line(i);
        let own = lines.get(line - 1).copied().unwrap_or("");
        let above = line
            .checked_sub(2)
            .and_then(|n| lines.get(n))
            .copied()
            .unwrap_or("");
        let after_attr = own.split_once(']').map_or("", |(_, rest)| rest);
        let justified = reason || above.trim_start().starts_with("//") || after_attr.contains("//");
        found.push(Found {
            site: Site {
                krate: krate.to_owned(),
                item,
            },
            path: path.to_owned(),
            line,
            justified,
        });
    }
    found
}

/// Every allow site of the workspace packages.
pub fn found_sites(packages: &[Package]) -> Vec<Found> {
    let mut out: Vec<Found> = packages
        .iter()
        .flat_map(|pkg| {
            pkg.files
                .iter()
                .flat_map(|(path, text)| scan_source(&pkg.name, path, text))
        })
        .collect();
    out.sort_by(|a, b| (&a.site, &a.path, a.line).cmp(&(&b.site, &b.path, b.line)));
    out
}

/// The number of sites per key.
pub fn tally(found: &[Found]) -> BTreeMap<Site, usize> {
    let mut counts = BTreeMap::new();
    for f in found {
        *counts.entry(f.site.clone()).or_default() += 1;
    }
    counts
}

// ---------------------------------------------------------------------------------------------------------
// Checking and printing

/// The violations of the check: `list` against the sites of `packages`.
pub fn check(packages: &[Package], list: &List) -> Vec<String> {
    let found = found_sites(packages);
    let have = tally(&found);
    let want = list.counts();
    let mut out = Vec::new();
    for f in found.iter().filter(|f| !f.justified) {
        out.push(format!(
            "{}:{}: `allow({LINT})` on `{}` has no justification: put a `//` comment on the line above (or a `reason = \"...\"` in the attribute) that says why the iteration order never reaches output ({REFERENCE})",
            f.path, f.line, f.site.item
        ));
    }
    for (site, n) in &have {
        match want.get(site) {
            None => out.push(format!(
                "{}: `{}` allows `{LINT}` ({n} site(s)) and is not in {LIST_FILE}: review it, then add the entry or run `cargo xtask check-ordered-collections --bless` ({REFERENCE})",
                site.krate, site.item
            )),
            Some(m) if m < n => out.push(format!(
                "{}: `{}` allows `{LINT}` at {n} site(s) but {LIST_FILE} lists {m}: review the new site, then update the count ({REFERENCE})",
                site.krate, site.item
            )),
            Some(m) if m > n => out.push(format!(
                "{}: `{}` allows `{LINT}` at {n} site(s) but {LIST_FILE} lists {m}: remove the stale count ({REFERENCE})",
                site.krate, site.item
            )),
            Some(_) => {}
        }
    }
    for (site, m) in &want {
        if !have.contains_key(site) {
            out.push(format!(
                "{}: {LIST_FILE} lists `{}` ({m} site(s)) but the source has no such allow: remove the stale entry ({REFERENCE})",
                site.krate, site.item
            ));
        }
    }
    for pkg in packages {
        for (n, line) in pkg.manifest_text.lines().enumerate() {
            let code = line.split('#').next().unwrap_or("");
            if code.contains(LINT) || code.contains("disallowed-types") {
                out.push(format!(
                    "{}:{}: a manifest sets the lint `{LINT}`; the hash-container ban is set in the generated clippy.toml only ({REFERENCE})",
                    pkg.manifest,
                    n + 1
                ));
            }
        }
    }
    out
}

/// The allow sites per crate as lines of text: `crate: n site(s)` and one `  item (xN)` line per key.
pub fn render(found: &[Found]) -> String {
    let counts = tally(found);
    let mut per_crate: BTreeMap<&str, Vec<(&str, usize)>> = BTreeMap::new();
    for (site, n) in &counts {
        per_crate
            .entry(site.krate.as_str())
            .or_default()
            .push((site.item.as_str(), *n));
    }
    let mut out = String::new();
    for (krate, items) in &per_crate {
        let total: usize = items.iter().map(|(_, n)| n).sum();
        out.push_str(&format!("{krate}: {total} allow site(s)\n"));
        for (item, n) in items {
            out.push_str(&format!(
                "  {item}{}\n",
                if *n > 1 { format!(" (x{n})") } else { String::new() }
            ));
        }
    }
    out.push_str(&format!(
        "total: {} allow site(s) in {} crate(s)\n",
        counts.values().sum::<usize>(),
        per_crate.len()
    ));
    out
}

fn toml_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The committed list for the sites `found` (what `--bless` writes).
pub fn generate(found: &[Found]) -> String {
    // The licence tag is spelled with an escaped hyphen so that REUSE does not read this source line as a tag.
    let mut out = String::from(
        "# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors\n\
         # SPDX-License\u{2d}Identifier: MIT OR Apache-2.0\n\
         \n\
         # The reviewed allow sites of the ordered-collection lint (FT-271, WP-S6.02): every place that allows\n\
         # `clippy::disallowed_types` for `std::collections::HashMap` or `HashSet` because the iteration order\n\
         # never reaches a persisted, exported or printed sequence. Keyed by crate and item path (no file, no\n\
         # line) with the number of sites. `cargo xtask check-ordered-collections` fails when the source and\n\
         # this list differ, so a new allow is reviewed in the change that adds it; `--bless` rewrites the file.\n",
    );
    for (site, n) in &tally(found) {
        out.push_str(&format!(
            "\n[[allow]]\ncrate = {}\nitem = {}\ncount = {n}\n",
            toml_string(&site.krate),
            toml_string(&site.item)
        ));
    }
    out
}

// ---------------------------------------------------------------------------------------------------------
// Loading

fn load_packages(root: &Path) -> Result<Vec<Package>> {
    let metadata = crate::metadata()?;
    let mut packages = Vec::new();
    for meta in metadata.workspace_packages() {
        let manifest_dir = meta
            .manifest_path
            .parent()
            .context("a manifest path has a parent directory")?;
        let mut files = Vec::new();
        raw_read::collect_rs(manifest_dir.as_std_path(), root, true, &mut files)?;
        let manifest = meta
            .manifest_path
            .strip_prefix(&metadata.workspace_root)
            .map(|m| m.as_str().replace('\\', "/"))
            .unwrap_or_else(|_| meta.manifest_path.to_string());
        packages.push(Package {
            name: meta.name.to_string(),
            manifest,
            manifest_text: std::fs::read_to_string(&meta.manifest_path)
                .with_context(|| format!("reading {}", meta.manifest_path))?,
            files,
        });
    }
    // The workspace manifest is checked like a package's (`[workspace.lints.clippy]`), without sources.
    let root_manifest = root.join("Cargo.toml");
    packages.push(Package {
        name: "(workspace)".to_owned(),
        manifest: "Cargo.toml".to_owned(),
        manifest_text: std::fs::read_to_string(&root_manifest)
            .with_context(|| format!("reading {}", root_manifest.display()))?,
        files: Vec::new(),
    });
    Ok(packages)
}

fn load_list(root: &Path) -> Result<List> {
    let path = root.join(LIST_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => List::parse(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(List::default()),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

/// The violations of the workspace at `root` (the step of `cargo xtask ci`).
///
/// # Errors
///
/// A package or the list that does not load.
pub fn violations(root: &Path) -> Result<Vec<String>> {
    Ok(check(&load_packages(root)?, &load_list(root)?))
}

/// `cargo xtask check-ordered-collections [--bless]`: prints the allow sites per crate, then the verdict; with
/// `bless` the list is rewritten first.
///
/// # Errors
///
/// A package or the list that does not load, or a list that cannot be written.
pub fn run(root: &Path, bless: bool) -> Result<bool> {
    let packages = load_packages(root)?;
    let found = found_sites(&packages);
    print!("{}", render(&found));
    if bless {
        std::fs::write(root.join(LIST_FILE), generate(&found))
            .with_context(|| format!("writing {LIST_FILE}"))?;
        println!("check-ordered-collections: wrote {LIST_FILE}");
    }
    let violations = check(&packages, &load_list(root)?);
    if violations.is_empty() {
        println!("check-ordered-collections: ok");
        Ok(true)
    } else {
        eprintln!("check-ordered-collections: {} violation(s)", violations.len());
        for v in &violations {
            eprintln!("  - {v}");
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str, files: &[(&str, &str)]) -> Package {
        Package {
            name: name.to_owned(),
            manifest: format!("crates/{name}/Cargo.toml"),
            manifest_text: "[package]\nname = \"x\"\n".to_owned(),
            files: files
                .iter()
                .map(|(p, t)| ((*p).to_owned(), (*t).to_owned()))
                .collect(),
        }
    }

    fn items(found: &[Found]) -> Vec<String> {
        found.iter().map(|f| f.site.item.clone()).collect()
    }

    const SRC: &str = r##"
        // Lookup only: never iterated.
        #[allow(clippy::disallowed_types)]
        use std::collections::{BTreeMap, HashMap};

        struct Store {
            // Counter by name, read with `get`.
            #[allow(clippy::disallowed_types)]
            holds: HashMap<String, u32>,
        }

        impl Store {
            // Membership only.
            #[allow(clippy::disallowed_types)]
            fn new() -> Self {
                Self { holds: HashMap::new() }
            }
        }

        impl<T: Clone> Default for Store {
            #[allow(clippy::disallowed_types, reason = "lookup only")]
            fn default() -> Self { Store::new() }
        }

        mod tests {
            fn helper() {
                // A set used with `contains` only.
                #[allow(clippy::disallowed_types)]
                let mut seen = std::collections::HashSet::new();
                seen.insert(1);
            }
        }

        // A lint that is not ours and a comment that mentions it:
        #[allow(clippy::too_many_arguments)]
        fn other() {} // #[allow(clippy::disallowed_types)]
        const TEXT: &str = "#[allow(clippy::disallowed_types)]";
    "##;

    #[test]
    fn sites_are_keyed_by_crate_and_item_path_not_by_line() {
        let found = scan_source("c", "crates/c/src/lib.rs", SRC);
        assert_eq!(
            items(&found),
            [
                "use std::collections::{BTreeMap, HashMap}",
                "struct Store > field holds",
                "impl Store > fn new",
                "impl Default for Store > fn default",
                "mod tests > fn helper > let seen",
            ]
        );
        assert!(found.iter().all(|f| f.justified), "{found:#?}");
        // Moving the code down changes the lines, not the keys.
        let shifted = scan_source("c", "crates/c/src/lib.rs", &format!("\n\n\n{SRC}"));
        assert_eq!(items(&shifted), items(&found));
        assert_ne!(
            shifted.iter().map(|f| f.line).collect::<Vec<_>>(),
            found.iter().map(|f| f.line).collect::<Vec<_>>()
        );
    }

    #[test]
    fn expect_cfg_attr_inner_and_bare_forms_are_sites() {
        let src = "
            #![allow(disallowed_types)] // the whole module is lookup-only
            #[expect(clippy::disallowed_types)] // counter
            static A: u8 = 0;
            #[cfg_attr(test, allow(clippy::disallowed_types))] // test helper
            pub(crate) const fn b() {}
            #[allow(clippy::all, clippy::disallowed_types)]
            fn c() {}
        ";
        let found = scan_source("c", "x.rs", src);
        assert_eq!(items(&found), ["inner attribute", "static A", "fn b", "fn c"]);
        assert_eq!(
            found.iter().map(|f| f.justified).collect::<Vec<_>>(),
            [true, true, true, false],
            "the last has no comment"
        );
    }

    #[test]
    fn a_site_without_a_justification_fails() {
        let src = "#[allow(clippy::disallowed_types)]\nfn f() {}\n";
        let packages = [pkg("c", &[("crates/c/src/lib.rs", src)])];
        let list = List::parse("[[allow]]\ncrate = \"c\"\nitem = \"fn f\"\ncount = 1\n").unwrap();
        let v = check(&packages, &list);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(
            v[0].starts_with(
                "crates/c/src/lib.rs:1: `allow(disallowed_types)` on `fn f` has no justification"
            ),
            "{v:#?}"
        );
    }

    fn one_site(count: usize) -> (Vec<Package>, String) {
        let one = "// lookup only\n#[allow(clippy::disallowed_types)]\nfn f() {}\n";
        let src = one.repeat(count);
        let list = format!("[[allow]]\ncrate = \"c\"\nitem = \"fn f\"\ncount = {count}\n");
        (vec![pkg("c", &[("crates/c/src/lib.rs", &src)])], list)
    }

    #[test]
    fn the_list_and_the_source_must_agree() {
        let (packages, list) = one_site(2);
        assert!(check(&packages, &List::parse(&list).unwrap()).is_empty());

        // A site the list does not have.
        let v = check(&packages, &List::default());
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(v[0].starts_with(
            "c: `fn f` allows `disallowed_types` (2 site(s)) and is not in ci/ordered-collections-allow.toml"
        ));

        // More sites than the list says: a new allow has to be reviewed.
        let v = check(
            &packages,
            &List::parse(&list.replace("count = 2", "count = 1")).unwrap(),
        );
        assert!(
            v.len() == 1 && v[0].contains("at 2 site(s) but ci/ordered-collections-allow.toml lists 1"),
            "{v:#?}"
        );

        // Fewer sites than the list says, and an entry with no site at all.
        let v = check(
            &packages,
            &List::parse(&list.replace("count = 2", "count = 3")).unwrap(),
        );
        assert!(v.len() == 1 && v[0].contains("remove the stale count"), "{v:#?}");
        let stale = format!("{list}\n[[allow]]\ncrate = \"c\"\nitem = \"fn gone\"\ncount = 1\n");
        let v = check(&packages, &List::parse(&stale).unwrap());
        assert!(
            v.len() == 1 && v[0].contains("lists `fn gone` (1 site(s)) but the source has no such allow"),
            "{v:#?}"
        );
    }

    #[test]
    fn a_manifest_that_sets_the_lint_fails() {
        let mut p = pkg("c", &[]);
        p.manifest_text =
            "[lints.clippy]\n# disallowed_types is generated\ndisallowed_types = \"allow\"\n".to_owned();
        let v = check(&[p], &List::default());
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(
            v[0].starts_with("crates/c/Cargo.toml:3: a manifest sets the lint `disallowed_types`"),
            "{v:#?}"
        );
    }

    #[test]
    fn the_list_round_trips_and_rejects_duplicates_and_unknown_keys() {
        let (packages, _) = one_site(3);
        let found = found_sites(&packages);
        let text = generate(&found);
        let list = List::parse(&text).expect("the generated list parses");
        assert_eq!(
            list.allow,
            [Entry {
                krate: "c".into(),
                item: "fn f".into(),
                count: 3
            }]
        );
        assert!(check(&packages, &list).is_empty());
        assert!(
            List::parse(&format!("{text}{text}")).is_err(),
            "a key listed twice"
        );
        assert!(List::parse("[[allow]]\ncrate = \"c\"\nitem = \"x\"\ncount = 1\nwhy = \"y\"\n").is_err());
    }

    #[test]
    fn the_report_counts_per_crate() {
        let src = "// a\n#[allow(clippy::disallowed_types)]\nfn f() {}\n// b\n#[allow(clippy::disallowed_types)]\nfn g() {}\n// c\n#[allow(clippy::disallowed_types)]\nfn f() {}\n";
        let found = found_sites(&[
            pkg("c", &[("a.rs", src)]),
            pkg(
                "d",
                &[("b.rs", "// x\n#[allow(clippy::disallowed_types)]\nfn h() {}\n")],
            ),
        ]);
        assert_eq!(
            render(&found),
            "c: 3 allow site(s)\n  fn f (x2)\n  fn g\nd: 1 allow site(s)\n  fn h\ntotal: 4 allow site(s) in 2 crate(s)\n"
        );
    }

    /// The real workspace: the committed list is current and every site is justified. `--bless` rewrites it.
    #[test]
    fn the_committed_list_is_current() {
        let root = crate::workspace_root();
        let packages = load_packages(&root).expect("the workspace loads");
        let v = check(&packages, &load_list(&root).expect("the list loads"));
        assert_eq!(
            v,
            Vec::<String>::new(),
            "run `cargo xtask check-ordered-collections --bless` after review"
        );
        let found = found_sites(&packages);
        let text = std::fs::read_to_string(root.join(LIST_FILE)).expect("the list exists");
        assert_eq!(
            text.replace("\r\n", "\n"),
            generate(&found),
            "the committed list is the generated one"
        );
    }
}
