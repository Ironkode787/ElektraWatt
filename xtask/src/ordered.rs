// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-ordered-collections` and `check-disallowed-methods`: the review lists of the determinism lints
//! (FT-271 and FT-317, WP-S6.02, doc 18 §3.5). One machinery, two [`Rule`]s.
//!
//! The iteration order of a `HashMap` or `HashSet` is unspecified and differs between runs, so it must not
//! reach a persisted, exported or printed sequence (NFR on deterministic output, doc 18 §3). Clippy enforces
//! that for every workspace crate: the generated `clippy.toml` (`check-raw-read`, [`crate::raw_read`]) lists
//! the std hash containers under `disallowed-types`. A use site whose iteration order provably never reaches
//! output (a lookup-only map, a counter, a set used only for `contains`) is allowed with
//! `#[allow(clippy::disallowed_types)]` and a one-line justification comment (or a `reason = "..."` in the
//! attribute); anything else uses a `BTreeMap` or `BTreeSet`. That is the rule [`TYPES`].
//!
//! The same file lists the wall clock, the process environment and randomness under `disallowed-methods`
//! (`SystemTime::now`, `Instant::now`, `std::env::var` and its siblings, `getrandom::fill`, ...): model, derive,
//! format and export paths must not depend on them. A justified use site (a bench or test timer, a bless flag,
//! the one ID generator, the CLI's inputs) is allowed with `#[allow(clippy::disallowed_methods)]` and a
//! justification. That is the rule [`METHODS`]. It differs in one respect: the lint `disallowed_methods` is also
//! the guard of the raw readers (`check-raw-read`), and the reader crates opt out of it as a whole at their
//! target roots, which would switch the new list off exactly in `ew-model`, `ew-derive`, `ew-commands` and
//! `ew-store`. For the reader crates this check therefore does not look at allow attributes (they say nothing
//! there) but scans the source lexically for the paths of the list (`SystemTime::now`, `std::env::var_os`, also
//! inside a `use` group), and every such call is a site that needs a justification and a reviewed entry.
//!
//! These checks keep those allow sites reviewed. They scan the Rust source of every workspace package, tests
//! and benches included, for attributes `#[allow(...)]`, `#![allow(...)]`, `#[expect(...)]` (also inside
//! `cfg_attr`) that name the lint (with or without `clippy::`), derive a key for each site, print the sites per
//! crate and compare them with the committed list (`ci/ordered-collections-allow.toml` and
//! `ci/disallowed-methods-allow.toml`).
//!
//! A key is the crate and the item path of the site, never a file or a line: the names of the enclosing
//! `mod`, `impl`, `fn`, `struct`, `enum` and `trait` items followed by the item the attribute decorates
//! (`fn parse_denylist`, `use std::collections::HashSet`, `struct Store > field holds`, `impl Store > fn
//! new`; for a lexical site `call SystemTime::now`). The list holds the number of sites per key. The check
//! fails when
//!
//! - a site is not in the list, or the source has more sites of a key than the list (a new allow must be
//!   reviewed: add the entry in the same change; `--bless` rewrites the list);
//! - the list names a key the source has fewer sites of (a stale entry);
//! - a site has no justification: no `//` comment on the line above or on the attribute's (or call's) line, and
//!   no `reason = "..."` in the attribute;
//! - a `Cargo.toml` of the workspace sets the lint, `style` or `all` (`[lints.clippy]`) or `warnings`
//!   (`[lints.rust]`, also under `[workspace.lints]`) to `allow`, which would switch the ban off without a site
//!   to review.
//!
//! The group lints `clippy::style` (which contains both lints), `clippy::all` and `warnings` count as allow
//! sites like the lint itself, so a group allow cannot switch a ban off unseen.
//!
//! The scan is lexical (the lexer of [`crate::raw_read`]): comments and string contents never count. A site
//! that a macro expands is not seen; the lint itself still fires there, and the macro's definition site is
//! the place to allow it. The lexical scan of the reader crates matches the last two segments of a path
//! (`SystemTime::now`), so an alias (`use std::time::SystemTime as Clock`) or a re-export is not seen there;
//! the reader crates are few and reviewed, and everywhere else clippy resolves the path itself.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::raw_read::{self, Attr, Parsed, Tk};

/// The policy file that names the reader crates.
const POLICY_FILE: &str = "ci/dep-rules.toml";

/// One lint of the family that this module reviews.
#[derive(Debug)]
pub struct Rule {
    /// The subcommand that runs the rule.
    pub command: &'static str,
    /// The lint as named in messages (`disallowed_types`).
    pub lint: &'static str,
    /// The spellings of the lint in an `allow` or `expect` list (the group lints are added).
    pub spellings: &'static [&'static str],
    /// The keys of `[lints.clippy]` whose `allow` switches the ban off (a group is among them).
    pub manifest_keys: &'static [&'static str],
    /// The committed list of reviewed sites, relative to the workspace root.
    pub list_file: &'static str,
    /// The authority each message cites.
    pub reference: &'static str,
    /// What the ban is, in the words of a message.
    pub ban: &'static str,
    /// What a justification has to say.
    pub justification: &'static str,
    /// The comment block that the generated list starts with, after the licence lines.
    pub header: &'static str,
    /// Whether the reader crates are scanned for calls of the banned paths instead of for allow attributes.
    pub lexical_in_readers: bool,
}

/// The ordered-collection rule (FT-271): the hash containers are `disallowed-types`.
pub const TYPES: Rule = Rule {
    command: "check-ordered-collections",
    lint: "disallowed_types",
    spellings: &["disallowed_types", "clippy::disallowed_types"],
    manifest_keys: &["disallowed_types", "style", "all"],
    list_file: "ci/ordered-collections-allow.toml",
    reference: "FT-271, WP-S6.02",
    ban: "hash-container ban",
    justification: "says why the iteration order never reaches output",
    header: "# The reviewed allow sites of the ordered-collection lint (FT-271, WP-S6.02): every place that allows\n\
             # `clippy::disallowed_types` for `std::collections::HashMap` or `HashSet` because the iteration order\n\
             # never reaches a persisted, exported or printed sequence. Keyed by crate and item path (no file, no\n\
             # line) with the number of sites. `cargo xtask check-ordered-collections` fails when the source and\n\
             # this list differ, so a new allow is reviewed in the change that adds it; `--bless` rewrites the file.\n",
    lexical_in_readers: false,
};

/// The wall-clock, environment and randomness rule (FT-317): the functions are `disallowed-methods`.
pub const METHODS: Rule = Rule {
    command: "check-disallowed-methods",
    lint: "disallowed_methods",
    spellings: &[
        "disallowed_methods",
        "clippy::disallowed_methods",
        "disallowed_method",
        "clippy::disallowed_method",
    ],
    manifest_keys: &["disallowed_methods", "disallowed_method", "style", "all"],
    list_file: "ci/disallowed-methods-allow.toml",
    reference: "FT-317, WP-S6.02, doc 18 §3.5",
    ban: "wall-clock, environment and randomness ban",
    justification: "says why the value never reaches a persisted, exported or printed result (or why this is the sanctioned source)",
    header: "# The reviewed allow sites of the wall-clock, environment and randomness lint (FT-317, WP-S6.02, doc 18\n\
             # §3.5): every place that allows `clippy::disallowed_methods` for `SystemTime::now`, `Instant::now`,\n\
             # `std::env::var` and its siblings, `RandomState::new` or `getrandom` because the value never reaches a\n\
             # persisted, exported or printed result, or because the place is a sanctioned source (the ID generator,\n\
             # the program boundary, a timer of a test or bench). In the reader crates, which opt out of the whole\n\
             # lint for the raw readers, a site is the call itself, found lexically. Keyed by crate and item path (no\n\
             # file, no line) with the number of sites. `cargo xtask check-disallowed-methods` fails when the source\n\
             # and this list differ, so a new allow is reviewed in the change that adds it; `--bless` rewrites the file.\n",
    lexical_in_readers: true,
};

/// The functions that the lexical scan of the reader crates looks for: the path segment before the call and
/// the functions that are banned on it (the list of `raw_read::DISALLOWED_METHODS`, plus the generators that
/// the workspace's feature sets do not compile today).
const CALLS: [(&str, &[&str]); 8] = [
    ("SystemTime", &["now"]),
    ("Instant", &["now"]),
    ("env", &["var", "var_os", "vars", "vars_os"]),
    ("RandomState", &["new"]),
    ("getrandom", &["fill", "fill_uninit", "u32", "u64"]),
    (
        "rand",
        &["random", "rng", "thread_rng", "random_range", "random_bool"],
    ),
    ("Uuid", &["new_v4", "now_v7"]),
    // krilla's GIF and WebP image constructors (FT-406): their decoders are audited as unreachable.
    ("Image", &["from_gif", "from_webp"]),
];

/// Whether `lint` (a lint path as written in an attribute) switches the ban of `rule` off: the lint itself and
/// the groups that contain it (`clippy::style`, `clippy::all`) or every warning (`warnings`).
fn is_switch(rule: &Rule, lint: &str) -> bool {
    rule.spellings.contains(&lint) || matches!(lint, "clippy::style" | "clippy::all" | "warnings")
}

/// The manifest keys (`[lints.clippy]`, `[lints.rust]`, also under `[workspace.lints]`) that, set to `allow`,
/// switch the ban of `rule` off, with the table they live in.
fn manifest_tables(rule: &Rule) -> [(&'static str, &'static [&'static str]); 2] {
    [("clippy", rule.manifest_keys), ("rust", &["warnings"])]
}

/// The manifest keys of `text` that set a switch of the rule to `allow` (as `key = "allow"` or
/// `key = { level = "allow", .. }`), with the 1-based line of the key.
fn manifest_switches(rule: &Rule, text: &str) -> Vec<(String, usize)> {
    let Ok(doc) = toml::from_str::<toml::Table>(text) else {
        return Vec::new();
    };
    let mut tables = Vec::new();
    if let Some(l) = doc.get("lints").and_then(toml::Value::as_table) {
        tables.push(l);
    }
    if let Some(l) = doc
        .get("workspace")
        .and_then(|w| w.get("lints"))
        .and_then(toml::Value::as_table)
    {
        tables.push(l);
    }
    let mut out = Vec::new();
    for lints in tables {
        for (group, keys) in manifest_tables(rule) {
            let Some(table) = lints.get(group).and_then(toml::Value::as_table) else {
                continue;
            };
            for key in keys {
                // Cargo accepts `disallowed-types` and `disallowed_types` alike: normalise the dash.
                let value = table
                    .iter()
                    .find(|(k, _)| k.replace('-', "_") == *key)
                    .map(|(_, v)| v);
                let allowed = match value {
                    Some(toml::Value::String(level)) => level == "allow",
                    Some(toml::Value::Table(t)) => {
                        t.get("level").and_then(toml::Value::as_str) == Some("allow")
                    }
                    _ => false,
                };
                if allowed {
                    let line = text
                        .lines()
                        .position(|l| l.trim_start().replace('-', "_").starts_with(key))
                        .map_or(1, |n| n + 1);
                    out.push((format!("{group}::{key}"), line));
                }
            }
        }
    }
    out
}

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
    /// The 1-based line of the attribute or call (for messages only).
    pub line: usize,
    /// Whether a comment or a `reason` justifies the site.
    pub justified: bool,
    /// Whether the site is a call found lexically (a reader crate) and not an allow attribute.
    pub call: bool,
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
    /// Whether the package is a reader crate of the raw-read policy (it opts out of `disallowed_methods`).
    pub reader: bool,
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
    /// Parses the list file of `rule`.
    ///
    /// # Errors
    ///
    /// A TOML or schema error, or a key listed twice.
    pub fn parse(rule: &Rule, text: &str) -> Result<List> {
        let list: List = toml::from_str(text).with_context(|| format!("parsing {}", rule.list_file))?;
        let mut seen = BTreeMap::new();
        for e in &list.allow {
            if seen.insert((e.krate.as_str(), e.item.as_str()), ()).is_some() {
                anyhow::bail!(
                    "{} lists `{}` of {} twice; merge the counts",
                    rule.list_file,
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

/// Whether the attribute names the lint of `rule` in an `allow(...)` or `expect(...)` list, and whether it
/// carries a `reason = "..."`.
fn names_lint(rule: &Rule, p: &Parsed, a: &Attr) -> (bool, bool) {
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
            } else if is_switch(rule, &lint) {
                listed = true;
            }
        }
        hit |= listed;
    }
    (hit, hit && reason)
}

/// The item path of the site at token `i`: the labels of the enclosing items, then `target`.
fn item_path(scopes: &[Scope], i: usize, target: &str) -> String {
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
    item.push_str(target);
    item
}

/// The calls of the banned paths of [`CALLS`] in the file: the token index of the first segment and the path as
/// `Segment::function`. A group of a `use` (`use std::env::{args, var}`) counts as a call of each name in it.
fn call_hits(p: &Parsed) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for i in 0..p.toks.len() {
        let Some(head) = p.ident(i) else { continue };
        let Some((_, fns)) = CALLS.iter().find(|(h, _)| *h == head) else {
            continue;
        };
        if !(p.is_p(i + 1, ':') && p.is_p(i + 2, ':')) {
            continue;
        }
        let j = i + 3;
        if let Some(f) = p.ident(j).filter(|f| fns.contains(f)) {
            out.push((i, format!("{head}::{f}")));
        } else if p.is_p(j, '{')
            && let Some(close) = p.pair[j]
        {
            // The names at the top level of the group.
            let mut k = j + 1;
            while k < close {
                if p.is_p(k, '{') {
                    k = p.pair[k].map_or(close, |m| m + 1);
                    continue;
                }
                if let Some(f) = p.ident(k).filter(|f| fns.contains(f))
                    && (p.is_p(k - 1, '{') || p.is_p(k - 1, ','))
                {
                    out.push((i, format!("{head}::{f}")));
                }
                k += 1;
            }
        }
    }
    out
}

/// Whether a `//` comment justifies the site on `line` (1-based): one on the line above, or one in `own_tail`,
/// the text of the site's line that follows the site.
fn has_comment(lines: &[&str], line: usize, own_tail: &str) -> bool {
    let above = line
        .checked_sub(2)
        .and_then(|n| lines.get(n))
        .copied()
        .unwrap_or("");
    above.trim_start().starts_with("//") || has_line_comment(own_tail)
}

/// Whether `text` holds a `//` comment: a `//` outside string literals (the `//` of `"http://"` is not a
/// comment). The char literals `'"'` and `'\"'` are skipped so that they do not open a string.
fn has_line_comment(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let mut in_string = false;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' if in_string => i += 1,
            '"' => in_string = !in_string,
            '\'' if !in_string && chars.get(i + 1) == Some(&'"') && chars.get(i + 2) == Some(&'\'') => i += 2,
            '\'' if !in_string
                && chars.get(i + 1) == Some(&'\\')
                && chars.get(i + 2) == Some(&'"')
                && chars.get(i + 3) == Some(&'\'') =>
            {
                i += 3;
            }
            '/' if !in_string && chars.get(i + 1) == Some(&'/') => return true,
            _ => {}
        }
        i += 1;
    }
    false
}

/// The allow sites of `krate`'s file `path` with text `text` (for a reader crate under a lexical rule: the
/// calls of the banned paths).
pub fn scan_source(rule: &Rule, krate: &str, path: &str, text: &str, reader: bool) -> Vec<Found> {
    let p = Parsed::new(text);
    let scopes = scopes(&p);
    let lines: Vec<&str> = text.lines().collect();
    let mut found = Vec::new();
    if rule.lexical_in_readers && reader {
        for (i, call) in call_hits(&p) {
            let line = p.line(i);
            let own = lines.get(line - 1).copied().unwrap_or("");
            found.push(Found {
                site: Site {
                    krate: krate.to_owned(),
                    item: item_path(&scopes, i, &format!("call {call}")),
                },
                path: path.to_owned(),
                line,
                justified: has_comment(&lines, line, own),
                call: true,
            });
        }
        return found;
    }
    for i in 0..p.toks.len() {
        let Some(a) = p.attr_at(i) else { continue };
        let (hit, reason) = names_lint(rule, &p, &a);
        if !hit {
            continue;
        }
        let target = if a.inner {
            "inner attribute".to_owned()
        } else {
            describe(&p, p.skip_attrs(a.close + 1))
        };
        let line = p.line(i);
        let own = lines.get(line - 1).copied().unwrap_or("");
        let after_attr = own.split_once(']').map_or("", |(_, rest)| rest);
        found.push(Found {
            site: Site {
                krate: krate.to_owned(),
                item: item_path(&scopes, i, &target),
            },
            path: path.to_owned(),
            line,
            justified: reason || has_comment(&lines, line, after_attr),
            call: false,
        });
    }
    found
}

/// Every allow site of the workspace packages.
pub fn found_sites(rule: &Rule, packages: &[Package]) -> Vec<Found> {
    let mut out: Vec<Found> = packages
        .iter()
        .flat_map(|pkg| {
            pkg.files
                .iter()
                .flat_map(|(path, text)| scan_source(rule, &pkg.name, path, text, pkg.reader))
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
pub fn check(rule: &Rule, packages: &[Package], list: &List) -> Vec<String> {
    let found = found_sites(rule, packages);
    let have = tally(&found);
    let want = list.counts();
    let (lint, list_file, reference) = (rule.lint, rule.list_file, rule.reference);
    let mut out = Vec::new();
    for f in found.iter().filter(|f| !f.justified) {
        if f.call {
            out.push(format!(
                "{}:{}: `{}` in a reader crate has no justification: put a `//` comment on the line above (or on the line) that {} ({reference})",
                f.path, f.line, f.site.item, rule.justification
            ));
        } else {
            out.push(format!(
                "{}:{}: `allow({lint})` on `{}` has no justification: put a `//` comment on the line above (or a `reason = \"...\"` in the attribute) that {} ({reference})",
                f.path, f.line, f.site.item, rule.justification
            ));
        }
    }
    for (site, n) in &have {
        match want.get(site) {
            None => out.push(format!(
                "{}: `{}` allows `{lint}` ({n} site(s)) and is not in {list_file}: review it, then add the entry or run `cargo xtask {} --bless` ({reference})",
                site.krate, site.item, rule.command
            )),
            Some(m) if m < n => out.push(format!(
                "{}: `{}` allows `{lint}` at {n} site(s) but {list_file} lists {m}: review the new site, then update the count ({reference})",
                site.krate, site.item
            )),
            Some(m) if m > n => out.push(format!(
                "{}: `{}` allows `{lint}` at {n} site(s) but {list_file} lists {m}: remove the stale count ({reference})",
                site.krate, site.item
            )),
            Some(_) => {}
        }
    }
    for (site, m) in &want {
        if !have.contains_key(site) {
            out.push(format!(
                "{}: {list_file} lists `{}` ({m} site(s)) but the source has no such allow: remove the stale entry ({reference})",
                site.krate, site.item
            ));
        }
    }
    for pkg in packages {
        for (key, line) in manifest_switches(rule, &pkg.manifest_text) {
            out.push(format!(
                "{}:{line}: a manifest sets the lint `{key}` to allow, which switches the {} off (`{lint}` is in `clippy::style`); the ban is set in the generated clippy.toml only ({reference})",
                pkg.manifest, rule.ban
            ));
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
pub fn generate(rule: &Rule, found: &[Found]) -> String {
    // The licence tag is spelled with an escaped hyphen so that REUSE does not read this source line as a tag.
    let mut out = format!(
        "# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors\n\
         # SPDX-License\u{2d}Identifier: MIT OR Apache-2.0\n\
         \n\
         {}",
        rule.header
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
    let readers = crate::deps::Policy::load(&root.join(POLICY_FILE))?
        .raw_read
        .map(|r| r.readers)
        .unwrap_or_default();
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
            reader: readers.iter().any(|r| r == meta.name.as_str()),
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
        reader: false,
    });
    Ok(packages)
}

fn load_list(root: &Path, rule: &Rule) -> Result<List> {
    let path = root.join(rule.list_file);
    match std::fs::read_to_string(&path) {
        Ok(text) => List::parse(rule, &text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(List::default()),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

/// The violations of the workspace at `root` under `rule` (the step of `cargo xtask ci`).
///
/// # Errors
///
/// A package or the list that does not load.
pub fn violations(root: &Path, rule: &Rule) -> Result<Vec<String>> {
    Ok(check(rule, &load_packages(root)?, &load_list(root, rule)?))
}

/// `cargo xtask check-ordered-collections|check-disallowed-methods [--bless]`: prints the allow sites per
/// crate, then the verdict; with `bless` the list is rewritten first.
///
/// # Errors
///
/// A package or the list that does not load, or a list that cannot be written.
pub fn run(root: &Path, rule: &Rule, bless: bool) -> Result<bool> {
    let packages = load_packages(root)?;
    let found = found_sites(rule, &packages);
    print!("{}", render(&found));
    if bless {
        std::fs::write(root.join(rule.list_file), generate(rule, &found))
            .with_context(|| format!("writing {}", rule.list_file))?;
        println!("{}: wrote {}", rule.command, rule.list_file);
    }
    let violations = check(rule, &packages, &load_list(root, rule)?);
    if violations.is_empty() {
        println!("{}: ok", rule.command);
        Ok(true)
    } else {
        eprintln!("{}: {} violation(s)", rule.command, violations.len());
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
            reader: false,
        }
    }

    fn reader(name: &str, files: &[(&str, &str)]) -> Package {
        Package {
            reader: true,
            ..pkg(name, files)
        }
    }

    fn items(found: &[Found]) -> Vec<String> {
        found.iter().map(|f| f.site.item.clone()).collect()
    }

    fn scan(krate: &str, path: &str, text: &str) -> Vec<Found> {
        scan_source(&TYPES, krate, path, text, false)
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
        let found = scan("c", "crates/c/src/lib.rs", SRC);
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
        let shifted = scan("c", "crates/c/src/lib.rs", &format!("\n\n\n{SRC}"));
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
        let found = scan("c", "x.rs", src);
        assert_eq!(items(&found), ["inner attribute", "static A", "fn b", "fn c"]);
        assert_eq!(
            found.iter().map(|f| f.justified).collect::<Vec<_>>(),
            [true, true, true, false],
            "the last has no comment"
        );
    }

    #[test]
    fn group_lints_are_sites_like_the_lint_itself() {
        let src = "
            // style contains the lint
            #[allow(clippy::style)]
            fn a() {}
            // all contains it too
            #![allow(clippy::all)]
            // every warning
            #[allow(warnings)]
            struct B;
            // not a switch of the ban
            #[allow(clippy::pedantic, clippy::too_many_arguments, dead_code)]
            fn d() {}
        ";
        let found = scan("c", "x.rs", src);
        assert_eq!(items(&found), ["fn a", "inner attribute", "struct B"]);
        assert!(found.iter().all(|f| f.justified));
    }

    #[test]
    fn manifests_that_allow_a_group_or_warnings_fail() {
        for (text, lint) in [
            ("[lints.clippy]\nstyle = \"allow\"\n", "clippy::style"),
            (
                "[workspace.lints.clippy]\nall = { level = \"allow\", priority = 1 }\n",
                "clippy::all",
            ),
            (
                "[lints.rust]\nmissing_docs = \"warn\"\nwarnings = \"allow\"\n",
                "rust::warnings",
            ),
        ] {
            for rule in [&TYPES, &METHODS] {
                let mut p = pkg("c", &[]);
                p.manifest_text = text.to_owned();
                let v = check(rule, &[p], &List::default());
                assert_eq!(v.len(), 1, "{text}: {v:#?}");
                assert!(
                    v[0].contains(&format!("sets the lint `{lint}` to allow")),
                    "{v:#?}"
                );
            }
        }
        // Warning about a group, or a priority, is the usual form and not a switch.
        let mut p = pkg("c", &[]);
        p.manifest_text =
            "[lints.clippy]\nall = { level = \"warn\", priority = -1 }\nstyle = \"warn\"\n".to_owned();
        assert!(check(&TYPES, &[p], &List::default()).is_empty());
    }

    #[test]
    fn a_site_without_a_justification_fails() {
        let src = "#[allow(clippy::disallowed_types)]\nfn f() {}\n";
        let packages = [pkg("c", &[("crates/c/src/lib.rs", src)])];
        let list = List::parse(&TYPES, "[[allow]]\ncrate = \"c\"\nitem = \"fn f\"\ncount = 1\n").unwrap();
        let v = check(&TYPES, &packages, &list);
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
        assert!(check(&TYPES, &packages, &List::parse(&TYPES, &list).unwrap()).is_empty());

        // A site the list does not have.
        let v = check(&TYPES, &packages, &List::default());
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(v[0].starts_with(
            "c: `fn f` allows `disallowed_types` (2 site(s)) and is not in ci/ordered-collections-allow.toml"
        ));

        // More sites than the list says: a new allow has to be reviewed.
        let v = check(
            &TYPES,
            &packages,
            &List::parse(&TYPES, &list.replace("count = 2", "count = 1")).unwrap(),
        );
        assert!(
            v.len() == 1 && v[0].contains("at 2 site(s) but ci/ordered-collections-allow.toml lists 1"),
            "{v:#?}"
        );

        // Fewer sites than the list says, and an entry with no site at all.
        let v = check(
            &TYPES,
            &packages,
            &List::parse(&TYPES, &list.replace("count = 2", "count = 3")).unwrap(),
        );
        assert!(v.len() == 1 && v[0].contains("remove the stale count"), "{v:#?}");
        let stale = format!("{list}\n[[allow]]\ncrate = \"c\"\nitem = \"fn gone\"\ncount = 1\n");
        let v = check(&TYPES, &packages, &List::parse(&TYPES, &stale).unwrap());
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
        let v = check(&TYPES, &[p], &List::default());
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(
            v[0].starts_with("crates/c/Cargo.toml:3: a manifest sets the lint `clippy::disallowed_types`"),
            "{v:#?}"
        );
        // Cargo accepts the dashed spelling of a lint key as well.
        let mut dashed = pkg("c", &[]);
        dashed.manifest_text = "[lints.clippy]
disallowed-types = \"allow\"
"
        .to_owned();
        let v = check(&TYPES, &[dashed], &List::default());
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(v[0].contains("`clippy::disallowed_types`"), "{v:#?}");
    }

    #[test]
    fn the_list_round_trips_and_rejects_duplicates_and_unknown_keys() {
        let (packages, _) = one_site(3);
        let found = found_sites(&TYPES, &packages);
        let text = generate(&TYPES, &found);
        let list = List::parse(&TYPES, &text).expect("the generated list parses");
        assert_eq!(
            list.allow,
            [Entry {
                krate: "c".into(),
                item: "fn f".into(),
                count: 3
            }]
        );
        assert!(check(&TYPES, &packages, &list).is_empty());
        assert!(
            List::parse(&TYPES, &format!("{text}{text}")).is_err(),
            "a key listed twice"
        );
        assert!(
            List::parse(
                &TYPES,
                "[[allow]]\ncrate = \"c\"\nitem = \"x\"\ncount = 1\nwhy = \"y\"\n"
            )
            .is_err()
        );
    }

    #[test]
    fn the_report_counts_per_crate() {
        let src = "// a\n#[allow(clippy::disallowed_types)]\nfn f() {}\n// b\n#[allow(clippy::disallowed_types)]\nfn g() {}\n// c\n#[allow(clippy::disallowed_types)]\nfn f() {}\n";
        let found = found_sites(
            &TYPES,
            &[
                pkg("c", &[("a.rs", src)]),
                pkg(
                    "d",
                    &[("b.rs", "// x\n#[allow(clippy::disallowed_types)]\nfn h() {}\n")],
                ),
            ],
        );
        assert_eq!(
            render(&found),
            "c: 3 allow site(s)\n  fn f (x2)\n  fn g\nd: 1 allow site(s)\n  fn h\ntotal: 4 allow site(s) in 2 crate(s)\n"
        );
    }

    // -----------------------------------------------------------------------------------------------------
    // The wall-clock, environment and randomness rule

    fn scan_m(krate: &str, text: &str, is_reader: bool) -> Vec<Found> {
        scan_source(&METHODS, krate, "x.rs", text, is_reader)
    }

    #[test]
    fn a_comment_marker_inside_a_string_is_not_a_justification() {
        assert!(has_line_comment("let x = 1; // why"));
        assert!(has_line_comment(r#"let x = "a\"b"; // after an escaped quote"#));
        assert!(!has_line_comment(r#"let url = "http://example.org";"#));
        assert!(!has_line_comment(r#"let q = '"'; let u = "//";"#));
        assert!(has_line_comment(
            r#"let q = '"'; // a char literal does not open a string"#
        ));
        assert!(has_line_comment(
            "fn f<'a>(x: &'a str) {} // a lifetime is not a char literal"
        ));
        // As sites: a call and an attribute whose line has a `//` only inside a string.
        let call = "fn f() {\n    let _ = (\"http://x\", Instant::now());\n}\n";
        let found = scan_m("ew-store", call, true);
        assert_eq!(found.len(), 1);
        assert!(!found[0].justified, "{found:#?}");
        let attr = "#[allow(clippy::disallowed_methods)] const URL: &str = \"http://x\";\nfn f() {}\n";
        let found = scan_m("c", attr, false);
        assert_eq!(found.len(), 1);
        assert!(!found[0].justified, "{found:#?}");
        let attr = "#[allow(clippy::disallowed_methods)] // the URL below\nconst URL: &str = \"http://x\";\n";
        assert!(scan_m("c", attr, false)[0].justified);
    }

    #[test]
    fn each_rule_sees_only_its_own_lint() {
        let src = "
            // order never reaches output
            #[allow(clippy::disallowed_types)]
            fn a() {}
            // a bless flag
            #[allow(clippy::disallowed_methods)]
            fn b() {}
            // the old name of the lint is accepted by clippy
            #[allow(clippy::disallowed_method)]
            fn c() {}
        ";
        assert_eq!(items(&scan("c", "x.rs", src)), ["fn a"]);
        assert_eq!(items(&scan_m("c", src, false)), ["fn b", "fn c"]);
        assert!(scan_m("c", src, false).iter().all(|f| f.justified && !f.call));
    }

    #[test]
    fn a_reader_crate_is_scanned_for_calls_and_not_for_attributes() {
        let src = r##"
            #![allow(clippy::disallowed_methods)]
            use std::time::SystemTime;

            struct SystemClock;

            impl SystemClock {
                fn now(&self) -> SystemTime {
                    // The production clock; tests inject a manual one.
                    SystemTime::now()
                }
            }

            fn boundary() -> Option<String> {
                std::env::var("HOME").ok() // the user's home for the config directory
            }

            fn unreviewed() -> std::time::Instant {
                std::time::Instant::now()
            }

            // Names in comments and strings are not calls: SystemTime::now(), "Instant::now()".
            const TEXT: &str = "std::env::var_os";
            fn clock() {} // a method called `now` on something else: self.clock.now()
        "##;
        let found = scan_m("ew-store", src, true);
        assert_eq!(
            items(&found),
            [
                "impl SystemClock > fn now > call SystemTime::now",
                "fn boundary > call env::var",
                "fn unreviewed > call Instant::now",
            ]
        );
        assert_eq!(
            found.iter().map(|f| f.justified).collect::<Vec<_>>(),
            [true, true, false]
        );
        assert!(found.iter().all(|f| f.call));
        // The same source in a crate that is not a reader: the inner allow is the one site.
        let found = scan_m("ew-other", src, false);
        assert_eq!(items(&found), ["inner attribute"]);
        assert!(!found[0].justified, "no comment on the attribute: {found:#?}");
    }

    #[test]
    fn use_groups_and_every_listed_function_are_calls() {
        let src = "
            // boundary
            use std::env::{args, var, vars_os};
            // a nested group is skipped, only the top level names count
            use std::{env::{var_os}, time::{Instant, SystemTime}};
            fn f() {
                // x
                let _ = (RandomState::new(), getrandom::fill(&mut []), getrandom::u64());
                // y
                let _ = (rand::thread_rng(), rand::random::<u8>(), Uuid::new_v4(), Uuid::now_v7());
                // z
                let _ = (std::env::vars(), std::env::var_os(\"A\"), SystemTime::now(), Instant::now());
                // not banned: arguments, the temp directory, a type, a method named `var`
                let _ = (std::env::args(), std::env::temp_dir(), SystemTime::UNIX_EPOCH, s.var());
            }
        ";
        let found = scan_m("ew-store", src, true);
        let mut got: Vec<String> = found
            .iter()
            .map(|f| f.site.item.rsplit(" > ").next().unwrap().to_owned())
            .collect();
        got.sort();
        let mut want: Vec<String> = [
            "env::var",
            "env::vars_os",
            "RandomState::new",
            "getrandom::fill",
            "getrandom::u64",
            "rand::thread_rng",
            "rand::random",
            "Uuid::new_v4",
            "Uuid::now_v7",
            "env::var_os",
            "env::vars",
            "env::var_os",
            "SystemTime::now",
            "Instant::now",
        ]
        .iter()
        .map(|c| format!("call {c}"))
        .collect();
        want.sort();
        assert_eq!(got, want, "{found:#?}");
    }

    #[test]
    fn a_call_in_a_reader_crate_needs_a_reviewed_entry_and_a_justification() {
        let src = "fn t() {\n    let started = Instant::now();\n}\n";
        let packages = [reader(
            "ew-commands",
            &[("crates/ew-commands/tests/perf.rs", src)],
        )];
        let list = "[[allow]]\ncrate = \"ew-commands\"\nitem = \"fn t > call Instant::now\"\ncount = 1\n";
        let v = check(&METHODS, &packages, &List::parse(&METHODS, list).unwrap());
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(
            v[0].starts_with("crates/ew-commands/tests/perf.rs:2: `fn t > call Instant::now` in a reader crate has no justification"),
            "{v:#?}"
        );
        // Justified, but not listed: the review is the list.
        let src =
            "fn t() {\n    // a timer of the test, never asserted\n    let started = Instant::now();\n}\n";
        let packages = [reader(
            "ew-commands",
            &[("crates/ew-commands/tests/perf.rs", src)],
        )];
        let v = check(&METHODS, &packages, &List::default());
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(
            v[0].contains("is not in ci/disallowed-methods-allow.toml")
                && v[0].contains("cargo xtask check-disallowed-methods --bless"),
            "{v:#?}"
        );
        assert!(check(&METHODS, &packages, &List::parse(&METHODS, list).unwrap()).is_empty());
    }

    #[test]
    fn the_methods_list_round_trips_with_its_own_header() {
        let src = "fn t() {\n    // timer\n    let _ = Instant::now();\n}\n";
        let packages = [reader("ew-commands", &[("a.rs", src)])];
        let text = generate(&METHODS, &found_sites(&METHODS, &packages));
        assert!(
            text.contains("wall-clock, environment and randomness lint"),
            "{text}"
        );
        let list = List::parse(&METHODS, &text).expect("the generated list parses");
        assert!(check(&METHODS, &packages, &list).is_empty());
    }

    fn committed_list_is_current(rule: &Rule) {
        let root = crate::workspace_root();
        let packages = load_packages(&root).expect("the workspace loads");
        let v = check(rule, &packages, &load_list(&root, rule).expect("the list loads"));
        assert_eq!(
            v,
            Vec::<String>::new(),
            "run `cargo xtask {} --bless` after review",
            rule.command
        );
        let found = found_sites(rule, &packages);
        let text = std::fs::read_to_string(root.join(rule.list_file)).expect("the list exists");
        assert_eq!(
            text.replace("\r\n", "\n"),
            generate(rule, &found),
            "the committed list is the generated one"
        );
    }

    /// The real workspace: the committed list is current and every site is justified. `--bless` rewrites it.
    #[test]
    fn the_committed_list_is_current() {
        committed_list_is_current(&TYPES);
    }

    /// The same for the wall-clock, environment and randomness rule.
    #[test]
    fn the_committed_methods_list_is_current() {
        committed_list_is_current(&METHODS);
    }

    /// The lexical table covers the functions of the generated clippy list, so that a reader crate (which opts
    /// out of the lint) is held to what the other crates are.
    #[test]
    fn the_lexical_table_covers_the_clippy_list() {
        for (path, _) in raw_read::DISALLOWED_METHODS {
            let mut segments = path.rsplit("::");
            let function = segments.next().unwrap();
            let head = segments.next().unwrap();
            assert!(
                CALLS.iter().any(|(h, fns)| *h == head && fns.contains(&function)),
                "`{path}` is in the clippy list but not in the lexical table"
            );
        }
    }
}
