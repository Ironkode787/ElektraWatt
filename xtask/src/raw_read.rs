// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-raw-read`: the source-level guard of ADR-0001 amendment 2 (decision log ID-50, task FT-01), driven by
//! the `[raw_read]` section of `ci/dep-rules.toml`.
//!
//! `check-features` stops a crate from *requesting* the `raw-read` feature of `ew-model`, but cargo features
//! are additive: a crate that depends on `ew-derive` is compiled with the raw readers present. This check
//! closes the remainder at the source. The non-test Rust source of every workspace package outside the
//! policy's `readers` (and `exempt_packages`) must not
//!
//! - invoke the `raw_read!` macro,
//! - contain a string literal equal to the feature name (`cfg(feature = "raw-read")`, `cfg_attr`, `cfg!`),
//! - call an item that the policy crate declares only under that feature, or
//! - enable the feature in its manifest (a non-dev dependency edge with the feature, or a `[features]`
//!   entry that forwards it; this duplicates part of `check-features` on purpose, as the ADR words it).
//!
//! # Gated items
//!
//! The gated items are read from the policy crate's own non-test source, so a newly gated reader is covered
//! without a list to maintain: every `fn <ident>` declared inside an invocation `raw_read! { ... }`, and every
//! `fn <ident>` of an item carrying the attribute `#[cfg(feature = "raw-read")]`. A `macro_rules!` definition
//! is skipped there: the token after `fn` in its body is `$`, not an identifier. Each gated item records its
//! name, the owner type of the enclosing `impl` block (the last path segment before the generics, for
//! `impl Trait for Type` the `Type`) and whether its declaration has generic parameters.
//!
//! # Distinctive and generic names
//!
//! The scanner is lexical: it has no type information, so it cannot tell `Vec::len` from `Table::len`. It
//! therefore classifies each gated name. A name is **generic** when it is declared as a method or associated
//! function (a `fn` inside an `impl` or `trait` block) in the non-test source of any workspace package,
//! outside the gated set itself (the policy crate's non-gated methods included), or when it is one of
//! [`STD_METHOD_NAMES`]. Free functions and test code never make a name generic: a test helper or a local
//! function that shares a reader's name cannot be the receiver of a method call in production code. Every
//! other gated name is **distinctive**.
//!
//! - A distinctive name is flagged in method form (`.name(`, `.name::<`) and in path form (`Owner::name`,
//!   `path::Owner::name`, `use path::name`; the owner is not compared, as no other item has the name).
//! - A generic name is flagged only where the receiver is known: in path form when the segment before it is
//!   the recorded owner type of a gated item of that name (`Table::len(&t)`, `Model::table::<T>(&m)`), and in
//!   method form with a turbofish (`.table::<T>()`) when a gated declaration of that name is generic.
//!
//! Misses (a generic name called as a plain method through a variable) are accepted: the type system and
//! `check-features` still stand behind this guard, which exists to catch the careless use of a distinctive
//! reader. A false positive gets an `[[raw_read.allow]]` entry (path, item, reason); inline suppressions are
//! not honoured and an entry that suppresses nothing fails.
//!
//! # Test code
//!
//! Test code is exempt, as the feature rule's dev exemption is. That means files under a package's top-level
//! `tests/`, `benches/`, `examples/` and `target/` directories; any item (fn, mod, impl, use, statement block)
//! preceded by an attribute `#[cfg(...)]` whose tokens contain the identifier `test` and no `not`
//! (`#[cfg(test)]` and `#[cfg(any(test, feature = "x"))]` are exempt, `#[cfg(not(test))]` is scanned), skipped
//! through its matching `}` or its `;`; a leading inner `#![cfg(test)]`; and out-of-line test modules:
//! `#[cfg(test)] mod foo;` in `dir/lib.rs`, `dir/main.rs` or `dir/mod.rs` exempts `dir/foo.rs` and
//! `dir/foo/**`, in `dir/bar.rs` it exempts `dir/bar/foo.rs` and `dir/bar/foo/**`. An attribute on a struct
//! field or match arm ends its item at the next top-level `,`. `#[path]` and the nesting of the declaring
//! module are not followed.
//!
//! # Lexing
//!
//! A small hand-written lexer yields identifiers, string literal contents and punctuation. Comments (line,
//! block with nesting, doc), char literals, lifetimes, raw strings (`r"..."`, `r#"..."#`), byte and C strings
//! never produce identifiers; the contents of a string count only for the feature-name rule.
//!
//! Scanning is pure and works on `(path, text)` pairs, so the unit tests feed synthetic sources; only [`run`]
//! and [`check`] touch the file system and `cargo metadata`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::deps::Kind;
use crate::features;

/// The policy file the section lives in (named by a stale allowlist entry).
const POLICY_FILE: &str = "ci/dep-rules.toml";

/// The authority each message cites.
const REFERENCE: &str = "ADR-0001 amendment 2";

/// Method names of the standard library that are common on collections. A gated reader that shares one of
/// them (`Table::len`, `Table::get`, `BaseIndexes::iter`) is always treated as a generic name: `.len()` and
/// `.get(` on a `Vec` or a map are everywhere in the workspace, and a lexical scan cannot tell them from the
/// gated reader even when no other `fn` of that name is declared in the workspace itself.
const STD_METHOD_NAMES: [&str; 5] = ["len", "is_empty", "get", "iter", "contains"];

/// Top-level directories of a package whose files are test code (or build output).
const TEST_DIRS: [&str; 4] = ["tests", "benches", "examples", "target"];

/// The `[raw_read]` section of `ci/dep-rules.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawReadPolicy {
    /// The crate that declares the gated readers (`ew-model`).
    #[serde(rename = "crate")]
    pub krate: String,
    /// The cargo feature that gates them (`raw-read`).
    pub feature: String,
    /// The macro that declares a gated reader (`raw_read`).
    #[serde(rename = "macro")]
    pub macro_name: String,
    /// The packages that may use the readers.
    pub readers: Vec<String>,
    /// Packages whose source is not scanned (the guard's own source names the feature and the macro).
    #[serde(default)]
    pub exempt_packages: Vec<String>,
    /// Reviewed false positives.
    #[serde(default)]
    pub allow: Vec<Allow>,
}

/// One reviewed false positive.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Allow {
    /// The file, relative to the workspace root, with forward slashes.
    pub path: String,
    /// A gated item name, `raw_read!` (the macro) or the feature name.
    pub item: String,
    /// Why the match is not a use of the raw reader.
    pub reason: String,
}

/// One declaration of a gated reader in the policy crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatedItem {
    /// The function name.
    pub name: String,
    /// The type of the enclosing `impl` block, if any.
    pub owner: Option<String>,
    /// Whether the declaration has generic parameters (`fn table<T: Entity>`).
    pub generic: bool,
}

/// The gated readers with the classification of their names.
#[derive(Debug, Clone)]
pub struct Gated {
    /// Every gated declaration.
    pub items: Vec<GatedItem>,
    generic_names: BTreeSet<String>,
}

impl Gated {
    /// Classifies the names of `items`; `declared` counts every `fn` declaration of the workspace by name
    /// (gated ones included, see [`declared_methods`]).
    pub fn new(items: Vec<GatedItem>, declared: &BTreeMap<String, usize>) -> Self {
        let mut gated_count: BTreeMap<&str, usize> = BTreeMap::new();
        for item in &items {
            *gated_count.entry(&item.name).or_default() += 1;
        }
        let generic_names = gated_count
            .iter()
            .filter(|(name, n)| {
                STD_METHOD_NAMES.contains(name) || declared.get(**name).copied().unwrap_or(0) > **n
            })
            .map(|(name, _)| (*name).to_owned())
            .collect();
        Self { items, generic_names }
    }

    /// Whether `name` is the name of a gated item.
    pub fn is_gated(&self, name: &str) -> bool {
        self.items.iter().any(|i| i.name == name)
    }

    /// Whether `name` is gated and not generic (declared nowhere else, not a std method name).
    pub fn is_distinctive(&self, name: &str) -> bool {
        self.is_gated(name) && !self.generic_names.contains(name)
    }

    fn has_owner(&self, name: &str, owner: &str) -> bool {
        self.items
            .iter()
            .any(|i| i.name == name && i.owner.as_deref() == Some(owner))
    }

    fn has_generic_decl(&self, name: &str) -> bool {
        self.items.iter().any(|i| i.name == name && i.generic)
    }
}

/// One finding: a place in a file (or manifest) and what is wrong with it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Violation {
    /// The file, relative to the workspace root, with forward slashes.
    pub path: String,
    /// The 1-based line.
    pub line: usize,
    /// What the match is about: a gated item name, `raw_read!` or the feature name (what an allow entry names).
    pub item: String,
    /// The description.
    pub message: String,
}

impl Violation {
    fn new(path: &str, line: usize, item: &str, message: String) -> Self {
        Self {
            path: path.to_owned(),
            line,
            item: item.to_owned(),
            message,
        }
    }

    /// The one-line form: `path:line: description (ADR-0001 amendment 2)`.
    pub fn render(&self) -> String {
        format!("{}:{}: {} ({REFERENCE})", self.path, self.line, self.message)
    }
}

/// A workspace package as the check sees it.
#[derive(Debug, Clone)]
pub struct SourcePackage {
    /// The package name.
    pub name: String,
    /// The manifest directory relative to the workspace root, with forward slashes (empty for the root).
    pub dir: String,
    /// Every `.rs` file of the package, test code included: workspace-relative path and text.
    pub files: Vec<(String, String)>,
    /// The package's `Cargo.toml` text (to locate manifest findings).
    pub manifest_text: String,
    /// Declared dependencies and `[features]`, as `check-features` reads them.
    pub manifest: features::Package,
}

/// The result of one run of the check.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// The rendered violations, sorted by path then line; empty means the check passed.
    pub violations: Vec<String>,
    /// How many gated declarations were found in the policy crate.
    pub gated_items: usize,
    /// How many packages were scanned (those outside `readers` and `exempt_packages`).
    pub packages_scanned: usize,
}

// ---------------------------------------------------------------------------------------------------------
// Lexer

#[derive(Debug, Clone, PartialEq)]
enum Tk {
    Ident(String),
    /// A string literal's content between the quotes (escapes untouched; prefixes `b`, `c`, `r#` dropped).
    Str(String),
    Punct(char),
    /// A char literal, lifetime or number.
    Other,
}

#[derive(Debug, Clone)]
struct Token {
    kind: Tk,
    line: usize,
}

struct Lexer {
    c: Vec<char>,
    i: usize,
    line: usize,
    out: Vec<Token>,
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn is_ident_char(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

impl Lexer {
    fn peek(&self, n: usize) -> char {
        self.c.get(self.i + n).copied().unwrap_or('\0')
    }

    fn push(&mut self, kind: Tk, line: usize) {
        self.out.push(Token { kind, line });
    }

    fn ident_chars(&mut self) -> String {
        let start = self.i;
        while self.i < self.c.len() && is_ident_char(self.c[self.i]) {
            self.i += 1;
        }
        self.c[start..self.i].iter().collect()
    }

    /// The body of a cooked string; `self.i` is just after the opening quote. Consumes the closing quote.
    fn string_body(&mut self) -> String {
        let mut s = String::new();
        while self.i < self.c.len() {
            let ch = self.c[self.i];
            self.i += 1;
            match ch {
                '"' => break,
                '\\' if self.i < self.c.len() => {
                    s.push(ch);
                    let escaped = self.c[self.i];
                    self.i += 1;
                    if escaped == '\n' {
                        self.line += 1;
                    }
                    s.push(escaped);
                }
                _ => {
                    if ch == '\n' {
                        self.line += 1;
                    }
                    s.push(ch);
                }
            }
        }
        s
    }

    /// If a raw string starts at `self.i` (`#`* then `"`), moves past the opening quote and returns the
    /// number of hashes.
    fn raw_string_start(&mut self) -> Option<usize> {
        let mut j = self.i;
        while self.c.get(j) == Some(&'#') {
            j += 1;
        }
        if self.c.get(j) == Some(&'"') {
            let hashes = j - self.i;
            self.i = j + 1;
            Some(hashes)
        } else {
            None
        }
    }

    /// The body of a raw string with `hashes` hashes; consumes the closing delimiter.
    fn raw_string_body(&mut self, hashes: usize) -> String {
        let mut s = String::new();
        while self.i < self.c.len() {
            let ch = self.c[self.i];
            if ch == '"' && (1..=hashes).all(|k| self.c.get(self.i + k) == Some(&'#')) {
                self.i += 1 + hashes;
                break;
            }
            if ch == '\n' {
                self.line += 1;
            }
            s.push(ch);
            self.i += 1;
        }
        s
    }

    /// A char literal or a lifetime; `self.i` is at the quote.
    fn quote(&mut self) {
        if self.peek(1) == '\\' {
            // '\n', '\'', '\u{..}': to the closing quote.
            self.i += 3;
            while self.i < self.c.len() && self.c[self.i] != '\'' {
                self.i += 1;
            }
            self.i += 1;
        } else if self.peek(2) == '\'' {
            self.i += 3;
        } else {
            self.i += 1;
            self.ident_chars();
        }
    }

    fn block_comment(&mut self) {
        let mut depth = 1;
        self.i += 2;
        while self.i < self.c.len() && depth > 0 {
            match (self.c[self.i], self.peek(1)) {
                ('/', '*') => {
                    depth += 1;
                    self.i += 2;
                }
                ('*', '/') => {
                    depth -= 1;
                    self.i += 2;
                }
                (ch, _) => {
                    if ch == '\n' {
                        self.line += 1;
                    }
                    self.i += 1;
                }
            }
        }
    }

    /// An identifier, or a string with a prefix (`r"`, `r#"`, `br"`, `b"`, `c"`), or a raw identifier.
    fn word(&mut self) {
        let line = self.line;
        let word = self.ident_chars();
        let next = self.peek(0);
        match (word.as_str(), next) {
            ("r" | "br" | "cr", '"' | '#') => {
                let save = self.i;
                if let Some(hashes) = self.raw_string_start() {
                    let body = self.raw_string_body(hashes);
                    self.push(Tk::Str(body), line);
                    return;
                }
                if word == "r" && next == '#' && is_ident_start(self.peek(1)) {
                    self.i = save + 1;
                    let ident = self.ident_chars();
                    self.push(Tk::Ident(ident), line);
                    return;
                }
            }
            ("b" | "c", '"') => {
                self.i += 1;
                let body = self.string_body();
                self.push(Tk::Str(body), line);
                return;
            }
            ("b", '\'') => {
                self.quote();
                self.push(Tk::Other, line);
                return;
            }
            _ => {}
        }
        self.push(Tk::Ident(word), line);
    }

    fn run(mut self) -> Vec<Token> {
        while self.i < self.c.len() {
            let ch = self.c[self.i];
            let line = self.line;
            match ch {
                '\n' => {
                    self.line += 1;
                    self.i += 1;
                }
                _ if ch.is_whitespace() => self.i += 1,
                '/' if self.peek(1) == '/' => {
                    while self.i < self.c.len() && self.c[self.i] != '\n' {
                        self.i += 1;
                    }
                }
                '/' if self.peek(1) == '*' => self.block_comment(),
                '"' => {
                    self.i += 1;
                    let body = self.string_body();
                    self.push(Tk::Str(body), line);
                }
                '\'' => {
                    self.quote();
                    self.push(Tk::Other, line);
                }
                _ if is_ident_start(ch) => self.word(),
                _ if ch.is_ascii_digit() => {
                    while self.i < self.c.len()
                        && (is_ident_char(self.c[self.i])
                            || (self.c[self.i] == '.' && self.peek(1).is_ascii_digit()))
                    {
                        self.i += 1;
                    }
                    self.push(Tk::Other, line);
                }
                _ => {
                    self.i += 1;
                    self.push(Tk::Punct(ch), line);
                }
            }
        }
        self.out
    }
}

fn lex(text: &str) -> Vec<Token> {
    Lexer {
        c: text.chars().collect(),
        i: 0,
        line: 1,
        out: Vec::new(),
    }
    .run()
}

// ---------------------------------------------------------------------------------------------------------
// Token structure

/// An attribute `#[...]` or `#![...]`.
struct Attr {
    inner: bool,
    open: usize,
    close: usize,
}

/// A lexed file with the matching delimiter of every bracket.
struct Parsed {
    toks: Vec<Token>,
    pair: Vec<Option<usize>>,
}

impl Parsed {
    fn new(text: &str) -> Self {
        let toks = lex(text);
        let mut pair = vec![None; toks.len()];
        let mut stack: Vec<usize> = Vec::new();
        for (i, t) in toks.iter().enumerate() {
            match t.kind {
                Tk::Punct('(' | '[' | '{') => stack.push(i),
                Tk::Punct(c @ (')' | ']' | '}')) => {
                    let want = match c {
                        ')' => '(',
                        ']' => '[',
                        _ => '{',
                    };
                    if let Some(&open) = stack.last()
                        && toks[open].kind == Tk::Punct(want)
                    {
                        stack.pop();
                        pair[open] = Some(i);
                        pair[i] = Some(open);
                    }
                }
                _ => {}
            }
        }
        Self { toks, pair }
    }

    fn is_p(&self, i: usize, ch: char) -> bool {
        self.toks.get(i).is_some_and(|t| t.kind == Tk::Punct(ch))
    }

    fn ident(&self, i: usize) -> Option<&str> {
        match self.toks.get(i) {
            Some(Token {
                kind: Tk::Ident(s), ..
            }) => Some(s),
            _ => None,
        }
    }

    fn is_ident(&self, i: usize, name: &str) -> bool {
        self.ident(i) == Some(name)
    }

    fn line(&self, i: usize) -> usize {
        self.toks[i].line
    }

    /// The attribute starting at token `i`, if one does.
    fn attr_at(&self, i: usize) -> Option<Attr> {
        if !self.is_p(i, '#') {
            return None;
        }
        let (inner, open) = if self.is_p(i + 1, '!') {
            (true, i + 2)
        } else {
            (false, i + 1)
        };
        if !self.is_p(open, '[') {
            return None;
        }
        Some(Attr {
            inner,
            open,
            close: self.pair[open]?,
        })
    }

    /// `cfg(...)` whose tokens contain `test` and no `not`.
    fn is_cfg_test(&self, a: &Attr) -> bool {
        let body = a.open + 1..a.close;
        self.is_ident(a.open + 1, "cfg")
            && body.clone().any(|k| self.is_ident(k, "test"))
            && !body.into_iter().any(|k| self.is_ident(k, "not"))
    }

    /// The index after the attributes that start at `i`.
    fn skip_attrs(&self, mut i: usize) -> usize {
        while let Some(a) = self.attr_at(i) {
            i = a.close + 1;
        }
        i
    }

    /// The index after `pub` and `pub(...)` at `i`.
    fn skip_visibility(&self, mut i: usize) -> usize {
        if self.is_ident(i, "pub") {
            i += 1;
            if self.is_p(i, '(')
                && let Some(close) = self.pair[i]
            {
                i = close + 1;
            }
        }
        i
    }

    /// Whether the item starting at `i` (after its attributes) begins with an item keyword; otherwise it is
    /// a field, match arm or expression, which also ends at a top-level `,`.
    fn starts_item(&self, i: usize) -> bool {
        const KEYWORDS: [&str; 17] = [
            "fn",
            "mod",
            "impl",
            "use",
            "struct",
            "enum",
            "trait",
            "const",
            "static",
            "type",
            "unsafe",
            "async",
            "extern",
            "macro_rules",
            "union",
            "let",
            "default",
        ];
        let i = self.skip_visibility(i);
        self.ident(i).is_some_and(|w| KEYWORDS.contains(&w))
    }

    /// The end (exclusive) of the item that starts at `start`: through its matching `}` or its `;`, at a
    /// top-level `,` for a field or arm, or before an unmatched closing delimiter.
    fn item_end(&self, start: usize) -> usize {
        let n = self.toks.len();
        let first = self.skip_attrs(start);
        let item_mode = self.starts_item(first);
        let mut j = first;
        while j < n {
            match self.toks[j].kind {
                Tk::Punct(';') => return j + 1,
                Tk::Punct(',') if !item_mode => return j + 1,
                Tk::Punct('{') => return self.pair[j].map_or(n, |m| m + 1),
                Tk::Punct('(' | '[') => j = self.pair[j].map_or(n, |m| m + 1),
                Tk::Punct(')' | ']' | '}') => return j,
                _ => j += 1,
            }
        }
        n
    }

    /// The out-of-line module `[pub] mod name;` that follows the attributes ending before `i`, if any.
    fn out_of_line_mod(&self, i: usize) -> Option<&str> {
        let j = self.skip_visibility(self.skip_attrs(i));
        if self.is_ident(j, "mod") && self.is_p(j + 2, ';') {
            self.ident(j + 1)
        } else {
            None
        }
    }
}

/// The tokens that are test code (`true`): see the module documentation.
fn exempt_mask(p: &Parsed) -> Vec<bool> {
    let n = p.toks.len();
    // A leading `#![cfg(test)]` makes the whole file test code.
    let mut i = 0;
    while let Some(a) = p.attr_at(i) {
        if !a.inner {
            break;
        }
        if p.is_cfg_test(&a) {
            return vec![true; n];
        }
        i = a.close + 1;
    }
    let mut mask = vec![false; n];
    let mut i = 0;
    while i < n {
        if let Some(a) = p.attr_at(i)
            && !a.inner
            && p.is_cfg_test(&a)
        {
            let end = p.item_end(a.close + 1).max(a.close + 1);
            mask[i..end.min(n)].fill(true);
            i = end;
        } else {
            i += 1;
        }
    }
    mask
}

/// The tokens inside `macro_rules! name { ... }` definitions.
fn macro_def_mask(p: &Parsed) -> Vec<bool> {
    let mut mask = vec![false; p.toks.len()];
    for i in 0..p.toks.len() {
        if p.is_ident(i, "macro_rules")
            && p.is_p(i + 1, '!')
            && p.ident(i + 2).is_some()
            && let Some(close) = p.pair.get(i + 3).copied().flatten()
        {
            mask[i..=close].fill(true);
        }
    }
    mask
}

/// The index of the `fn` keyword of each function declaration (`fn` followed by an identifier) in
/// `lo..hi`.
fn fn_decls(p: &Parsed, lo: usize, hi: usize) -> Vec<usize> {
    (lo..hi.min(p.toks.len()))
        .filter(|&k| p.is_ident(k, "fn") && p.ident(k + 1).is_some())
        .collect()
}

/// The `impl` blocks of a file: body open, body close, owner type.
fn impl_ranges(p: &Parsed) -> Vec<(usize, usize, String)> {
    let mut ranges = Vec::new();
    for i in 0..p.toks.len() {
        // Item position only: not `-> impl Trait` or `x: impl Trait`.
        let item_position = i == 0
            || p.is_p(i - 1, '{')
            || p.is_p(i - 1, '}')
            || p.is_p(i - 1, ';')
            || p.is_p(i - 1, ']')
            || p.is_ident(i - 1, "unsafe")
            // `$( impl ... )*` in a macro definition.
            || (i >= 2 && p.is_p(i - 1, '(') && p.is_p(i - 2, '$'));
        if !p.is_ident(i, "impl") || !item_position {
            continue;
        }
        let mut j = i + 1;
        if p.is_p(j, '<') {
            j = skip_angle(p, j);
        }
        let header_start = j;
        let mut body = None;
        while j < p.toks.len() {
            match p.toks[j].kind {
                Tk::Punct('{') => {
                    body = Some(j);
                    break;
                }
                Tk::Punct(';') => break,
                Tk::Punct('(' | '[') => j = p.pair[j].map_or(p.toks.len(), |m| m + 1),
                _ => j += 1,
            }
        }
        let Some(open) = body else { continue };
        let Some(close) = p.pair[open] else { continue };
        if let Some(owner) = impl_owner(p, header_start, open) {
            ranges.push((open, close, owner));
        }
    }
    ranges
}

/// The index after the balanced `<...>` that starts at `i` (a `>` of `->` does not close).
fn skip_angle(p: &Parsed, mut i: usize) -> usize {
    let mut depth = 0usize;
    while i < p.toks.len() {
        if p.is_p(i, '<') {
            depth += 1;
        } else if p.is_p(i, '>') && !(i > 0 && p.is_p(i - 1, '-')) {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return i + 1;
            }
        }
        i += 1;
    }
    i
}

/// The owner type of an `impl` header in `lo..hi` (after the impl's own generics): the last identifier at
/// angle depth 0 of the self type, which follows `for` when there is one.
fn impl_owner(p: &Parsed, lo: usize, hi: usize) -> Option<String> {
    let mut depth = 0usize;
    let mut ty_start = lo;
    let mut end = hi;
    for k in lo..hi {
        if p.is_p(k, '<') {
            depth += 1;
        } else if p.is_p(k, '>') && !(k > 0 && p.is_p(k - 1, '-')) {
            depth = depth.saturating_sub(1);
        } else if depth == 0 && p.is_ident(k, "for") {
            ty_start = k + 1;
        } else if depth == 0 && p.is_ident(k, "where") {
            end = k;
            break;
        }
    }
    depth = 0;
    let mut owner = None;
    for k in ty_start..end {
        if p.is_p(k, '<') {
            depth += 1;
        } else if p.is_p(k, '>') && !(k > 0 && p.is_p(k - 1, '-')) {
            depth = depth.saturating_sub(1);
        } else if depth == 0
            && let Some(name) = p.ident(k)
            && !matches!(name, "dyn" | "mut" | "const" | "unsafe")
        {
            owner = Some(name.to_owned());
        }
    }
    owner
}

fn owner_at(impls: &[(usize, usize, String)], idx: usize) -> Option<String> {
    impls
        .iter()
        .filter(|(open, close, _)| *open < idx && idx < *close)
        .max_by_key(|(open, _, _)| *open)
        .map(|(_, _, owner)| owner.clone())
}

// ---------------------------------------------------------------------------------------------------------
// Gated items and declarations

/// The gated declarations in the (non-test) files of the policy crate.
pub fn gated_items(files: &[(String, String)], policy: &RawReadPolicy) -> Vec<GatedItem> {
    let mut items = Vec::new();
    for (_, text) in files {
        let p = Parsed::new(text);
        let exempt = exempt_mask(&p);
        let in_macro_def = macro_def_mask(&p);
        let impls = impl_ranges(&p);
        let mut seen: BTreeSet<usize> = BTreeSet::new();
        let mut take = |p: &Parsed, ks: Vec<usize>| {
            for k in ks {
                if seen.insert(k) {
                    items.push(GatedItem {
                        name: p.ident(k + 1).unwrap_or_default().to_owned(),
                        owner: owner_at(&impls, k),
                        generic: p.is_p(k + 2, '<'),
                    });
                }
            }
        };
        for i in 0..p.toks.len() {
            if exempt[i] {
                continue;
            }
            // `raw_read! { fn name ... }`
            if p.is_ident(i, &policy.macro_name)
                && p.is_p(i + 1, '!')
                && matches!(
                    p.toks.get(i + 2).map(|t| &t.kind),
                    Some(Tk::Punct('{' | '(' | '['))
                )
                && let Some(close) = p.pair[i + 2]
            {
                take(&p, fn_decls(&p, i + 3, close));
            }
            // `#[cfg(feature = "raw-read")] item`
            if !in_macro_def[i]
                && let Some(a) = p.attr_at(i)
                && !a.inner
                && is_feature_cfg(&p, &a, &policy.feature)
            {
                let lo = p.skip_attrs(a.close + 1);
                let end = p.item_end(a.close + 1);
                let ks = fn_decls(&p, lo, end);
                // A plain `fn` item yields only itself, not the functions nested in its body.
                let plain_fn = ks.first().is_some_and(|&k| {
                    (lo..k).all(|m| {
                        matches!(
                            p.ident(m),
                            Some(
                                "pub"
                                    | "const"
                                    | "async"
                                    | "unsafe"
                                    | "extern"
                                    | "crate"
                                    | "super"
                                    | "self"
                                    | "in"
                            )
                        ) || matches!(p.toks[m].kind, Tk::Str(_) | Tk::Punct('(' | ')'))
                    })
                });
                take(
                    &p,
                    if plain_fn {
                        ks.into_iter().take(1).collect()
                    } else {
                        ks
                    },
                );
            }
        }
    }
    items
}

/// `cfg(feature = "<feature>")` exactly.
fn is_feature_cfg(p: &Parsed, a: &Attr, feature: &str) -> bool {
    let o = a.open;
    p.is_ident(o + 1, "cfg")
        && p.is_p(o + 2, '(')
        && p.is_ident(o + 3, "feature")
        && p.is_p(o + 4, '=')
        && matches!(p.toks.get(o + 5).map(|t| &t.kind), Some(Tk::Str(s)) if s == feature)
        && p.is_p(o + 6, ')')
        && o + 7 == a.close
}

/// The bodies of the `trait` blocks of a file: body open, body close.
fn trait_ranges(p: &Parsed) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    for i in 0..p.toks.len() {
        if !p.is_ident(i, "trait") {
            continue;
        }
        let mut j = i + 1;
        while j < p.toks.len() {
            match p.toks[j].kind {
                Tk::Punct('{') => {
                    if let Some(close) = p.pair[j] {
                        ranges.push((j, close));
                    }
                    break;
                }
                Tk::Punct(';') => break,
                Tk::Punct('(' | '[') => j = p.pair[j].map_or(p.toks.len(), |m| m + 1),
                _ => j += 1,
            }
        }
    }
    ranges
}

/// How many times each name is declared as a method or associated function (a `fn` inside an `impl` or
/// `trait` block) in the non-test code of `files`. The caller leaves out the files that are test code by
/// path; `#[cfg(test)]` items are skipped here.
pub fn declared_methods(files: &[(String, String)]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for (_, text) in files {
        let p = Parsed::new(text);
        let exempt = exempt_mask(&p);
        let mut containers: Vec<(usize, usize)> = impl_ranges(&p)
            .into_iter()
            .map(|(open, close, _)| (open, close))
            .collect();
        containers.extend(trait_ranges(&p));
        for k in fn_decls(&p, 0, p.toks.len()) {
            if exempt[k] || !containers.iter().any(|&(open, close)| open < k && k < close) {
                continue;
            }
            if let Some(name) = p.ident(k + 1) {
                *counts.entry(name.to_owned()).or_default() += 1;
            }
        }
    }
    counts
}

// ---------------------------------------------------------------------------------------------------------
// Test code by path

/// Files that are test code because a `#[cfg(test)] mod name;` declares them.
#[derive(Debug, Default)]
struct TestModules {
    files: BTreeSet<String>,
    dirs: Vec<String>,
}

impl TestModules {
    fn contains(&self, path: &str) -> bool {
        self.files.contains(path) || self.dirs.iter().any(|d| path.starts_with(d.as_str()))
    }
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_owned()
    } else {
        format!("{dir}/{name}")
    }
}

fn test_modules(files: &[(String, String)]) -> TestModules {
    let mut out = TestModules::default();
    for (path, text) in files {
        let p = Parsed::new(text);
        let (dir, file) = path.rsplit_once('/').unwrap_or(("", path));
        let base = if matches!(file, "lib.rs" | "main.rs" | "mod.rs") {
            dir.to_owned()
        } else {
            join(dir, file.strip_suffix(".rs").unwrap_or(file))
        };
        for i in 0..p.toks.len() {
            if let Some(a) = p.attr_at(i)
                && !a.inner
                && p.is_cfg_test(&a)
                && let Some(name) = p.out_of_line_mod(a.close + 1)
            {
                out.files.insert(join(&base, &format!("{name}.rs")));
                out.dirs.push(format!("{}/", join(&base, name)));
            }
        }
    }
    out
}

/// Whether `path` lies in a test directory of the package at `package_dir`.
fn is_test_path(package_dir: &str, path: &str) -> bool {
    let rel = if package_dir.is_empty() {
        path
    } else {
        path.strip_prefix(package_dir)
            .and_then(|r| r.strip_prefix('/'))
            .unwrap_or(path)
    };
    rel.split('/')
        .next()
        .is_some_and(|first| rel.contains('/') && TEST_DIRS.contains(&first))
}

// ---------------------------------------------------------------------------------------------------------
// Scanning

/// The owner named by the path segment before the `::` that precedes token `i`: `Owner::name`,
/// `Owner::<G>::name` and `<Owner as Trait>::name`.
fn path_owner(p: &Parsed, i: usize) -> Option<String> {
    let k = i.checked_sub(3)?;
    if let Some(name) = p.ident(k) {
        return Some(name.to_owned());
    }
    if !p.is_p(k, '>') {
        return None;
    }
    // Walk back to the matching `<`.
    let mut depth = 0usize;
    let mut m = k;
    loop {
        if p.is_p(m, '>') && !(m > 0 && p.is_p(m - 1, '-')) {
            depth += 1;
        } else if p.is_p(m, '<') {
            depth -= 1;
            if depth == 0 {
                break;
            }
        }
        m = m.checked_sub(1)?;
    }
    if m >= 3 && p.is_p(m - 1, ':') && p.is_p(m - 2, ':') {
        p.ident(m - 3).map(str::to_owned)
    } else {
        p.ident(m + 1).map(str::to_owned)
    }
}

/// The violations in one source file of a package that may not use the raw readers.
pub fn scan_source(path: &str, text: &str, gated: &Gated, policy: &RawReadPolicy) -> Vec<Violation> {
    let p = Parsed::new(text);
    let exempt = exempt_mask(&p);
    let mut v = Vec::new();
    let call = |v: &mut Vec<Violation>, i: usize, name: &str| {
        v.push(Violation::new(
            path,
            p.line(i),
            name,
            format!(
                "calls `{name}`, an item {} declares only under feature `{}`",
                policy.krate, policy.feature
            ),
        ));
    };
    for (i, &skip) in exempt.iter().enumerate() {
        if skip {
            continue;
        }
        match &p.toks[i].kind {
            Tk::Ident(name) => {
                if *name == policy.macro_name && p.is_p(i + 1, '!') && !p.is_p(i + 2, '=') {
                    v.push(Violation::new(
                        path,
                        p.line(i),
                        &format!("{name}!"),
                        format!(
                            "invokes `{name}!`, the macro that declares {}'s readers behind feature `{}`",
                            policy.krate, policy.feature
                        ),
                    ));
                }
                if gated.is_gated(name) && p.is_p(i.wrapping_sub(1), ':') && p.is_p(i.wrapping_sub(2), ':') {
                    let flagged = if gated.is_distinctive(name) {
                        // `a::name::<T>` is a call; `a::name::b` is a module segment.
                        !(p.is_p(i + 1, ':') && p.is_p(i + 2, ':') && !p.is_p(i + 3, '<'))
                    } else {
                        path_owner(&p, i).is_some_and(|owner| gated.has_owner(name, &owner))
                    };
                    if flagged {
                        call(&mut v, i, name);
                    }
                }
            }
            Tk::Str(s) if *s == policy.feature => v.push(Violation::new(
                path,
                p.line(i),
                &policy.feature,
                format!("names the feature `{}` of {}", policy.feature, policy.krate),
            )),
            // Method form; `..name(` is a range, not a call.
            Tk::Punct('.') if !(i > 0 && p.is_p(i - 1, '.')) => {
                if let Some(name) = p.ident(i + 1)
                    && gated.is_gated(name)
                {
                    let paren = p.is_p(i + 2, '(');
                    let turbofish = p.is_p(i + 2, ':') && p.is_p(i + 3, ':') && p.is_p(i + 4, '<');
                    let flagged = if gated.is_distinctive(name) {
                        paren || turbofish
                    } else {
                        turbofish && gated.has_generic_decl(name)
                    };
                    if flagged {
                        call(&mut v, i + 1, name);
                    }
                }
            }
            _ => {}
        }
    }
    v
}

/// The first 1-based line of `text` that contains one of `needles` (tried in order), or 1.
fn line_of(text: &str, needles: &[&str]) -> usize {
    needles
        .iter()
        .find_map(|n| text.lines().position(|l| l.contains(n)))
        .map_or(1, |l| l + 1)
}

/// The manifest findings of one package: a non-dev dependency on the policy crate that enables the feature,
/// and `[features]` entries that forward it (`crate/feature`, `crate?/feature`, a renamed dependency
/// resolved).
pub fn manifest_violations(
    path: &str,
    manifest_text: &str,
    package: &features::Package,
    policy: &RawReadPolicy,
) -> Vec<Violation> {
    let mut v = Vec::new();
    for d in &package.deps {
        if d.name == policy.krate && d.kind != Kind::Dev && d.features.contains(&policy.feature) {
            v.push(Violation::new(
                path,
                line_of(manifest_text, &[policy.krate.as_str(), policy.feature.as_str()]),
                &policy.feature,
                format!(
                    "enables `{}/{}` on a {}",
                    policy.krate,
                    policy.feature,
                    d.kind.label()
                ),
            ));
        }
    }
    for (feature, items) in &package.features {
        for item in items {
            let bare = item.strip_prefix("dep:").unwrap_or(item);
            let Some((dep, f)) = bare.split_once('/') else {
                continue;
            };
            let dep = dep.strip_suffix('?').unwrap_or(dep);
            let krate = package
                .deps
                .iter()
                .find(|d| d.alias == dep)
                .map_or(dep, |d| d.name.as_str());
            if krate == policy.krate && f == policy.feature {
                v.push(Violation::new(
                    path,
                    line_of(manifest_text, &[item.as_str(), policy.feature.as_str()]),
                    &policy.feature,
                    format!(
                        "its feature `{feature}` forwards `{}/{}`",
                        policy.krate, policy.feature
                    ),
                ));
            }
        }
    }
    v
}

/// Applies the allowlist: drops the violations an entry names, and reports entries without a reason and
/// entries that suppressed nothing.
fn apply_allow(violations: Vec<Violation>, allow: &[Allow]) -> Vec<Violation> {
    let mut used = vec![false; allow.len()];
    let mut out: Vec<Violation> = violations
        .into_iter()
        .filter(|v| {
            let mut suppressed = false;
            for (k, a) in allow.iter().enumerate() {
                if a.path == v.path && a.item == v.item {
                    used[k] = true;
                    suppressed = true;
                }
            }
            !suppressed
        })
        .collect();
    for (a, used) in allow.iter().zip(used) {
        if a.reason.trim().is_empty() {
            out.push(Violation::new(
                POLICY_FILE,
                1,
                &a.item,
                format!("allow entry for `{}` in `{}` has no reason", a.item, a.path),
            ));
        }
        if !used {
            out.push(Violation::new(
                POLICY_FILE,
                1,
                &a.item,
                format!(
                    "stale allow entry: `{}` in `{}` matches no violation; remove it",
                    a.item, a.path
                ),
            ));
        }
    }
    out
}

/// The gated declarations of the policy crate within `packages`, with their names classified against every
/// declaration of the workspace.
fn gated_of(policy: &RawReadPolicy, packages: &[SourcePackage], modules: &TestModules) -> Option<Gated> {
    let model = packages.iter().find(|p| p.name == policy.krate)?;
    let model_files: Vec<(String, String)> = model
        .files
        .iter()
        .filter(|(path, _)| !is_test_path(&model.dir, path) && !modules.contains(path))
        .cloned()
        .collect();
    let non_test: Vec<(String, String)> = packages
        .iter()
        .flat_map(|p| {
            p.files
                .iter()
                .filter(|(path, _)| !is_test_path(&p.dir, path) && !modules.contains(path))
                .cloned()
        })
        .collect();
    Some(Gated::new(
        gated_items(&model_files, policy),
        &declared_methods(&non_test),
    ))
}

/// Runs the check over `packages`: pure, so the unit tests can seed violations.
pub fn check_packages(policy: &RawReadPolicy, packages: &[SourcePackage]) -> Outcome {
    let all: Vec<(String, String)> = packages.iter().flat_map(|p| p.files.iter().cloned()).collect();
    let modules = test_modules(&all);
    let Some(gated) = gated_of(policy, packages, &modules) else {
        return Outcome {
            violations: vec![format!(
                "{POLICY_FILE}:1: [raw_read] crate `{}` is not a workspace package ({REFERENCE})",
                policy.krate
            )],
            gated_items: 0,
            packages_scanned: 0,
        };
    };
    let mut found = Vec::new();
    let mut scanned = 0;
    for pkg in packages {
        if policy.readers.contains(&pkg.name) || policy.exempt_packages.contains(&pkg.name) {
            continue;
        }
        scanned += 1;
        for (path, text) in &pkg.files {
            if is_test_path(&pkg.dir, path) || modules.contains(path) {
                continue;
            }
            found.extend(scan_source(path, text, &gated, policy));
        }
        found.extend(manifest_violations(
            &join(&pkg.dir, "Cargo.toml"),
            &pkg.manifest_text,
            &pkg.manifest,
            policy,
        ));
    }
    let mut violations = apply_allow(found, &policy.allow);
    violations.sort();
    violations.dedup();
    Outcome {
        violations: violations.iter().map(Violation::render).collect(),
        gated_items: gated.items.len(),
        packages_scanned: scanned,
    }
}

// ---------------------------------------------------------------------------------------------------------
// The workspace

/// Collects the `.rs` files below `dir`: not hidden directories, not `target`, not directories that hold
/// another package's manifest.
fn collect_rs(dir: &Path, root: &Path, is_package_root: bool, out: &mut Vec<(String, String)>) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .collect::<std::io::Result<_>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_dir() {
            if name.starts_with('.')
                || name == "target"
                || (!is_package_root && path.join("Cargo.toml").exists())
            {
                continue;
            }
            collect_rs(&path, root, false, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            let rel = path
                .strip_prefix(root)
                .with_context(|| format!("{} is outside the workspace", path.display()))?
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            out.push((rel, String::from_utf8_lossy(&bytes).into_owned()));
        }
    }
    Ok(())
}

/// Reads the policy and the packages of the workspace at `root`.
fn load(root: &Path) -> Result<(RawReadPolicy, Vec<SourcePackage>)> {
    let policy = crate::deps::Policy::load(&root.join(POLICY_FILE))?
        .raw_read
        .with_context(|| format!("{POLICY_FILE} has no [raw_read] section (ADR-0001 amendment 2)"))?;
    let metadata = crate::metadata()?;
    let manifests = features::from_metadata(&metadata);
    let mut packages = Vec::new();
    for (meta, manifest) in metadata.workspace_packages().iter().zip(manifests) {
        let manifest_dir = meta
            .manifest_path
            .parent()
            .context("a manifest path has a parent directory")?;
        let dir = manifest_dir
            .strip_prefix(&metadata.workspace_root)
            .map(|d| d.as_str().replace('\\', "/"))
            .with_context(|| format!("{manifest_dir} is outside the workspace"))?;
        let mut files = Vec::new();
        collect_rs(
            manifest_dir.as_std_path(),
            metadata.workspace_root.as_std_path(),
            true,
            &mut files,
        )?;
        let manifest_text = std::fs::read_to_string(&meta.manifest_path)
            .with_context(|| format!("reading {}", meta.manifest_path))?;
        packages.push(SourcePackage {
            name: meta.name.to_string(),
            dir,
            files,
            manifest_text,
            manifest,
        });
    }
    Ok((policy, packages))
}

/// Runs the check on the workspace at `root`.
pub fn check(root: &Path) -> Result<Outcome> {
    let (policy, packages) = load(root)?;
    Ok(check_packages(&policy, &packages))
}

/// `cargo xtask check-raw-read`: prints the outcome and returns whether the check passed.
pub fn run(root: &Path) -> Result<bool> {
    let outcome = check(root)?;
    if outcome.violations.is_empty() {
        println!(
            "check-raw-read: ok ({} gated items, {} packages scanned)",
            outcome.gated_items, outcome.packages_scanned
        );
        Ok(true)
    } else {
        eprintln!("check-raw-read: {} violation(s)", outcome.violations.len());
        for v in &outcome.violations {
            eprintln!("  - {v}");
        }
        Ok(false)
    }
}

/// The violations of the check, for the in-process `ci` summary.
pub fn violations(root: &Path) -> Result<Vec<String>> {
    Ok(check(root)?.violations)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> RawReadPolicy {
        toml::from_str(
            r#"
            crate = "ew-model"
            feature = "raw-read"
            macro = "raw_read"
            readers = ["ew-model", "ew-derive", "ew-commands", "ew-store"]
            exempt_packages = ["xtask"]
            "#,
        )
        .expect("test policy parses")
    }

    const MODEL: &str = r#"
        /// Declares an untracked read accessor.
        macro_rules! raw_read {
            ($(#[$meta:meta])* fn $($item:tt)*) => {
                #[cfg(feature = "raw-read")]
                $(#[$meta])*
                pub fn $($item)*

                #[cfg(not(feature = "raw-read"))]
                $(#[$meta])*
                pub(crate) fn $($item)*
            };
        }

        impl<T: Entity> Table<T> {
            raw_read! {
                /// Number of rows.
                fn len(&self) -> usize { self.map.len() }
            }
            raw_read! {
                fn get(&self, id: &RowId<T>) -> Option<&Arc<T>> { self.map.get(id) }
            }
        }

        impl Model {
            raw_read! {
                fn table<T: Entity>(&self) -> &Table<T> { T::table(self) }
            }
            raw_read! {
                fn delete_plan(&self, k: &AnyKey) -> Result<Vec<Op>, ModelError> { Ok(vec![]) }
            }
            pub fn apply(&self, ops: &[Op]) -> Result<Model, ModelError> { todo!() }
            pub fn from_records() -> Model { todo!() }
        }

        impl sealed::Sealed for Device {
            fn table(model: &Model) -> &Table<Self> { &model.devices }
        }

        impl BaseIndexes {
            raw_read! {
                fn by_page(&self) -> usize { 0 }
            }
        }

        #[cfg(feature = "raw-read")]
        pub fn snapshot_rows() {}

        #[cfg(test)]
        mod tests {
            raw_read! { fn only_in_tests() {} }
        }
    "#;

    const MODEL_PATH: &str = "crates/ew-model/src/lib.rs";

    fn files(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(p, t)| ((*p).to_owned(), (*t).to_owned()))
            .collect()
    }

    fn dep(name: &str, kind: Kind, features: &[&str]) -> features::Dep {
        features::Dep {
            alias: name.into(),
            name: name.into(),
            kind,
            features: features.iter().map(|f| (*f).to_owned()).collect(),
        }
    }

    fn manifest(name: &str, deps: Vec<features::Dep>, feats: &[(&str, &[&str])]) -> features::Package {
        features::Package {
            name: name.into(),
            deps,
            features: feats
                .iter()
                .map(|(k, vs)| ((*k).to_owned(), vs.iter().map(|s| (*s).to_owned()).collect()))
                .collect(),
        }
    }

    fn package(
        name: &str,
        sources: &[(&str, &str)],
        manifest: features::Package,
        manifest_text: &str,
    ) -> SourcePackage {
        SourcePackage {
            name: name.into(),
            dir: format!("crates/{name}"),
            files: files(sources),
            manifest_text: manifest_text.into(),
            manifest,
        }
    }

    fn model_package() -> SourcePackage {
        package(
            "ew-model",
            &[(MODEL_PATH, MODEL)],
            manifest("ew-model", vec![], &[]),
            "",
        )
    }

    /// The outcome for a synthetic workspace: the fixture ew-model plus one package `app`.
    fn run_app(sources: &[(&str, &str)], app_manifest: features::Package, manifest_text: &str) -> Outcome {
        let app = package("app", sources, app_manifest, manifest_text);
        check_packages(&policy(), &[model_package(), app])
    }

    fn scan(src: &str) -> Vec<String> {
        run_app(
            &[("crates/app/src/lib.rs", src)],
            manifest("app", vec![dep("ew-model", Kind::Normal, &[])], &[]),
            "",
        )
        .violations
    }

    fn gated_fixture() -> Gated {
        let model = files(&[(MODEL_PATH, MODEL)]);
        Gated::new(gated_items(&model, &policy()), &declared_methods(&model))
    }

    #[test]
    fn gated_extraction_reads_macro_invocations_and_cfg_items() {
        let items = gated_items(&files(&[(MODEL_PATH, MODEL)]), &policy());
        let find = |name: &str| items.iter().find(|i| i.name == name);
        let item = |name: &str, owner: Option<&str>, generic: bool| GatedItem {
            name: name.into(),
            owner: owner.map(str::to_owned),
            generic,
        };
        assert_eq!(find("len"), Some(&item("len", Some("Table"), false)));
        assert_eq!(find("get"), Some(&item("get", Some("Table"), false)));
        assert_eq!(find("table"), Some(&item("table", Some("Model"), true)));
        assert_eq!(
            find("delete_plan"),
            Some(&item("delete_plan", Some("Model"), false))
        );
        assert_eq!(
            find("by_page"),
            Some(&item("by_page", Some("BaseIndexes"), false))
        );
        assert_eq!(find("snapshot_rows"), Some(&item("snapshot_rows", None, false)));
        // Not gated: the macro definition, plain items, the trait impl's `table`, test code.
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(
            names,
            ["len", "get", "table", "delete_plan", "by_page", "snapshot_rows"],
            "{items:#?}"
        );
    }

    #[test]
    fn cfg_gated_impl_block_yields_all_its_functions_with_the_owner() {
        let src = r#"
            #[cfg(feature = "raw-read")]
            impl Model {
                pub fn a(&self) {}
                pub fn b<T>(&self) {}
            }
            #[cfg(feature = "raw-read")]
            pub fn outer() { fn nested() {} }
            #[cfg(not(feature = "raw-read"))]
            pub fn not_gated() {}
        "#;
        let items = gated_items(&files(&[("x.rs", src)]), &policy());
        let got: Vec<(&str, Option<&str>, bool)> = items
            .iter()
            .map(|i| (i.name.as_str(), i.owner.as_deref(), i.generic))
            .collect();
        assert_eq!(
            got,
            [
                ("a", Some("Model"), false),
                ("b", Some("Model"), true),
                ("outer", None, false)
            ]
        );
    }

    #[test]
    fn impl_owner_forms() {
        let src = r#"
            impl<'a, T: Fn() -> u8> crate::index::Wrapper<'a, T> { raw_read! { fn one(&self) {} } }
            impl<T> Trait<T> for path::Holder<T> where T: Clone { raw_read! { fn two(&self) {} } }
            impl Plain { raw_read! { fn three(&self) {} } }
            fn make() -> impl Iterator<Item = u8> { raw_read! { fn four() {} } 0..1 }
        "#;
        let items = gated_items(&files(&[("x.rs", src)]), &policy());
        let got: Vec<(&str, Option<&str>)> = items
            .iter()
            .map(|i| (i.name.as_str(), i.owner.as_deref()))
            .collect();
        assert_eq!(
            got,
            [
                ("one", Some("Wrapper")),
                ("two", Some("Holder")),
                ("three", Some("Plain")),
                ("four", None)
            ]
        );
    }

    #[test]
    fn name_classification() {
        let g = gated_fixture();
        // Declared nowhere else.
        assert!(g.is_distinctive("delete_plan"));
        assert!(g.is_distinctive("snapshot_rows"));
        assert!(g.is_distinctive("by_page"));
        // Another `fn table` (the trait impl) and the std method names.
        assert!(g.is_gated("table") && !g.is_distinctive("table"));
        assert!(g.is_gated("len") && !g.is_distinctive("len"));
        assert!(g.is_gated("get") && !g.is_distinctive("get"));
        assert!(!g.is_gated("apply") && !g.is_gated("only_in_tests"));
    }

    /// Only a method or associated function in non-test source makes a gated name generic.
    #[test]
    fn generic_names_come_from_non_test_methods() {
        let classify = |extra: &str| {
            let mut all = files(&[(MODEL_PATH, MODEL)]);
            all.push(("crates/app/src/lib.rs".into(), extra.into()));
            Gated::new(
                gated_items(&files(&[(MODEL_PATH, MODEL)]), &policy()),
                &declared_methods(&all),
            )
        };
        // Free functions, test helpers and test-only methods keep the name distinctive.
        assert!(classify("fn by_page() {}").is_distinctive("by_page"));
        assert!(classify("pub fn helper() { fn by_page() {} }").is_distinctive("by_page"));
        assert!(
            classify(
                "#[cfg(test)]
mod t { impl Foo { fn by_page(&self) {} } }"
            )
            .is_distinctive("by_page")
        );
        assert!(
            classify(
                "#[cfg(test)]
impl Foo { fn by_page(&self) {} }"
            )
            .is_distinctive("by_page")
        );
        // A non-test method or trait function makes it generic.
        assert!(!classify("impl Foo { fn by_page(&self) {} }").is_distinctive("by_page"));
        assert!(!classify("impl Tr for Foo { fn by_page(&self) {} }").is_distinctive("by_page"));
        assert!(!classify("trait Tr { fn by_page(&self); }").is_distinctive("by_page"));
        assert!(
            !classify(
                "#[cfg(not(test))]
impl Foo { fn by_page(&self) {} }"
            )
            .is_distinctive("by_page")
        );
        // An impl generated in a macro definition counts.
        assert!(
            !classify(
                "macro_rules! m { ($($t:ty),*) => { $( impl Foo for $t { fn by_page(&self) {} } )* }; }"
            )
            .is_distinctive("by_page")
        );

        // End to end: a test-directory method does not stop `.by_page()` from being flagged; a non-test
        // one does.
        let call = "fn f(m: M) { m.by_page(); }";
        let out = run_app(
            &[
                ("crates/app/src/lib.rs", call),
                ("crates/app/tests/t.rs", "impl X { fn by_page(&self) {} }"),
            ],
            manifest("app", vec![], &[]),
            "",
        );
        assert_eq!(out.violations.len(), 1, "{:#?}", out.violations);
        let both = format!(
            "{call}
impl Foo {{ fn by_page(&self) {{}} }}"
        );
        let out = run_app(
            &[("crates/app/src/lib.rs", both.as_str())],
            manifest("app", vec![], &[]),
            "",
        );
        assert_eq!(out.violations, Vec::<String>::new());
    }

    #[test]
    fn passing_crate_is_clean() {
        let src = r#"
            use ew_model::{Model, Rev};
            use std::collections::HashMap;
            fn f(m: Model, v: Vec<u8>, map: HashMap<u8, u8>, ops: Vec<Op>) {
                let m0 = Model::from_records(project, [], []).unwrap();
                let m1 = m.apply(&ops, Rev(1)).unwrap();
                let n = v.len() + Vec::len(&v);
                let g = map.get(&1);
                for i in 0..v.len() {}
                let r = 0..delete_plan_len;
            }
        "#;
        assert_eq!(scan(src), Vec::<String>::new());
    }

    #[test]
    fn calls_of_gated_items_fail() {
        let src = "fn f(m: Model, t: Table<Device>) {\n\
            let a = m.delete_plan(&k);\n\
            let b = Model::table::<Device>(&m);\n\
            let c = m.table::<Device>();\n\
            let d = Table::len(&t);\n\
            let e = ew_model::Model::delete_plan(&m, &k);\n\
            let f = t.len();\n\
            let g = m.table();\n\
            let h = Table::<Device>::len(&t);\n\
            let i = <Table<Device>>::len(&t);\n\
            let j = ew_model::snapshot_rows();\n\
            let k = m.indexes().by_page();\n\
        }\n";
        let v = scan(src);
        let at = |line: usize, name: &str| {
            format!(
                "crates/app/src/lib.rs:{line}: calls `{name}`, an item ew-model declares only under feature `raw-read` (ADR-0001 amendment 2)"
            )
        };
        assert_eq!(
            v,
            [
                at(2, "delete_plan"),
                at(3, "table"),
                at(4, "table"),
                at(5, "len"),
                at(6, "delete_plan"),
                at(9, "len"),
                at(10, "len"),
                at(11, "snapshot_rows"),
                at(12, "by_page"),
            ]
        );
    }

    #[test]
    fn macro_invocation_and_feature_name_fail() {
        let src = "raw_read! { fn x() {} }\n\
            #[cfg(feature = \"raw-read\")]\n\
            fn y() {}\n\
            #[cfg_attr(feature = \"raw-read\", derive(Debug))]\n\
            struct S;\n\
            fn z() -> bool { cfg!(feature = \"raw-read\") }\n\
            macro_rules! raw_read { () => {}; }\n\
            fn w(raw_read: u8) -> bool { raw_read != 1 }\n";
        let v = scan(src);
        assert_eq!(v.len(), 4, "{v:#?}");
        assert!(
            v[0].starts_with("crates/app/src/lib.rs:1: invokes `raw_read!`"),
            "{v:#?}"
        );
        assert!(
            v[1].starts_with("crates/app/src/lib.rs:2: names the feature `raw-read` of ew-model"),
            "{v:#?}"
        );
        assert!(v[2].contains(":4: names the feature"), "{v:#?}");
        assert!(v[3].contains(":6: names the feature"), "{v:#?}");
    }

    #[test]
    fn manifest_rules() {
        let text = "[dependencies]\new-model = { workspace = true, features = [\"raw-read\"] }\n";
        let out = run_app(
            &[],
            manifest("app", vec![dep("ew-model", Kind::Normal, &["raw-read"])], &[]),
            text,
        );
        assert_eq!(
            out.violations,
            ["crates/app/Cargo.toml:2: enables `ew-model/raw-read` on a dependency (ADR-0001 amendment 2)"]
        );
        let out = run_app(
            &[],
            manifest("app", vec![dep("ew-model", Kind::Build, &["raw-read"])], &[]),
            text,
        );
        assert!(
            out.violations[0].contains("on a build-dependency"),
            "{:#?}",
            out.violations
        );

        let text = "[features]\ndebug = [\"ew-model/raw-read\"]\n";
        for item in ["ew-model/raw-read", "ew-model?/raw-read", "dep:ew-model/raw-read"] {
            let out = run_app(
                &[],
                manifest(
                    "app",
                    vec![dep("ew-model", Kind::Normal, &[])],
                    &[("debug", &[item])],
                ),
                text,
            );
            assert_eq!(
                out.violations,
                [
                    "crates/app/Cargo.toml:2: its feature `debug` forwards `ew-model/raw-read` (ADR-0001 amendment 2)"
                ],
                "{item}"
            );
        }
        // A renamed dependency is resolved.
        let mut renamed = dep("ew-model", Kind::Normal, &[]);
        renamed.alias = "model".into();
        let out = run_app(
            &[],
            manifest("app", vec![renamed], &[("debug", &["model/raw-read"])]),
            text,
        );
        assert_eq!(out.violations.len(), 1, "{:#?}", out.violations);
        // Other features and other crates pass; so does a dev edge with the feature.
        let out = run_app(
            &[],
            manifest(
                "app",
                vec![
                    dep("ew-model", Kind::Dev, &["raw-read"]),
                    dep("other", Kind::Normal, &["raw-read"]),
                ],
                &[("x", &["other/raw-read", "ew-model/other"])],
            ),
            "",
        );
        assert_eq!(out.violations, Vec::<String>::new());
    }

    #[test]
    fn readers_and_exempt_packages_are_not_scanned() {
        let src = "fn f(m: Model) { let _ = m.delete_plan(&k); }\n#[cfg(feature = \"raw-read\")] fn g() {}";
        let readers = ["ew-derive", "ew-commands", "ew-store", "xtask"].map(|n| {
            package(
                n,
                &[(&format!("crates/{n}/src/lib.rs"), src)],
                manifest(n, vec![dep("ew-model", Kind::Normal, &["raw-read"])], &[]),
                "",
            )
        });
        let mut packages = vec![model_package()];
        packages.extend(readers);
        let out = check_packages(&policy(), &packages);
        assert_eq!(out.violations, Vec::<String>::new());
        assert_eq!(out.packages_scanned, 0);
        assert_eq!(out.gated_items, 6);
    }

    #[test]
    fn test_code_is_exempt() {
        let bad = "m.delete_plan(&k); let _ = \"raw-read\"; raw_read! { fn x() {} }";
        let src = format!(
            "#[cfg(test)]\nmod tests {{ fn t() {{ {bad} }} }}\n\
             #[cfg(test)]\nfn helper() {{ {bad} }}\n\
             #[cfg(any(test, feature = \"x\"))]\nfn either() {{ {bad} }}\n\
             #[cfg(test)]\nuse ew_model::snapshot_rows;\n\
             fn run() {{\n    #[cfg(test)]\n    {{ {bad} }}\n}}\n\
             struct S {{\n    #[cfg(test)]\n    a: Vec<A, B>,\n}}\n"
        );
        assert_eq!(scan(&src), Vec::<String>::new());
        // `not(test)` is scanned, and so is code after an exempt item.
        let src =
            format!("#[cfg(not(test))]\nfn f() {{ {bad} }}\n#[cfg(test)]\nfn g() {{}}\nfn h() {{ {bad} }}\n");
        let v = scan(&src);
        assert_eq!(v.len(), 6, "{v:#?}");
        assert!(v.iter().all(|l| l.contains(":2:") || l.contains(":5:")), "{v:#?}");
        // A leading inner attribute exempts the file.
        assert_eq!(
            scan(&format!("#![cfg(test)]\nfn h() {{ {bad} }}")),
            Vec::<String>::new()
        );
    }

    #[test]
    fn test_directories_and_out_of_line_test_modules_are_exempt() {
        let bad = "fn t(m: Model) { m.delete_plan(&k); }";
        let sources = [
            (
                "crates/app/src/lib.rs",
                "#[cfg(test)]\nmod tests;\n#[cfg(test)]\nmod support;\nmod live;\n",
            ),
            ("crates/app/src/tests.rs", bad),
            ("crates/app/src/support/mod.rs", bad),
            ("crates/app/src/live.rs", "#[cfg(test)]\nmod checks;\n"),
            ("crates/app/src/live/checks.rs", bad),
            ("crates/app/src/live/checks/deep.rs", bad),
            ("crates/app/src/live/other.rs", bad),
            ("crates/app/src/mod.rs", "#[cfg(test)] mod fixtures;"),
            ("crates/app/src/fixtures.rs", bad),
            ("crates/app/tests/it.rs", bad),
            ("crates/app/benches/b.rs", bad),
            ("crates/app/examples/e.rs", bad),
            ("crates/app/src/bin/tests/x.rs", bad),
        ];
        let out = run_app(&sources, manifest("app", vec![], &[]), "");
        assert_eq!(
            out.violations,
            [
                "crates/app/src/bin/tests/x.rs:1: calls `delete_plan`, an item ew-model declares only under feature `raw-read` (ADR-0001 amendment 2)",
                "crates/app/src/live/other.rs:1: calls `delete_plan`, an item ew-model declares only under feature `raw-read` (ADR-0001 amendment 2)",
            ]
        );
        assert!(is_test_path("crates/app", "crates/app/tests/it.rs"));
        assert!(is_test_path("", "tests/it.rs"));
        assert!(!is_test_path("crates/app", "crates/app/src/tests.rs"));
        assert!(!is_test_path("crates/app", "crates/app/tests.rs"));
    }

    #[test]
    fn comments_strings_and_literals_do_not_count() {
        let src = r###"
            // m.delete_plan(&k) raw_read! { fn x() {} } "raw-read"
            /* m.delete_plan(&k) /* nested raw_read! {} */ still comment: delete_plan( */
            /// Docs: Model::delete_plan(&m, &k), raw_read!
            //! m.delete_plan(
            fn f() {
                let a = "delete_plan(";
                let b = "m.delete_plan(&k)";
                let c = '"';
                let d = '\'';
                let e = '\\';
                let lt: &'static str = "x";
                fn g<'a>(x: &'a u8) -> &'a u8 { x }
                let r1 = r"m.delete_plan(&k)";
                let r2 = r#"he said "m.delete_plan(&k)" and "raw-read""#;
                let r3 = r##"a "# b .delete_plan( "##;
                let b1 = b"delete_plan(";
                let b2 = br#"raw_read!"#;
                let b3 = b'x';
                let multi = "line one
                    m.delete_plan(&k)";
                let q = '"'; let s = "x"; // m.delete_plan(&k)
            }
        "###;
        assert_eq!(scan(src), Vec::<String>::new());
        // The raw string with the feature name is the only string match, and the line count survives the
        // multi-line literals and comments before it.
        let src = "/* a\nb */\nlet _ = \"multi\nline\";\nlet r = r#\"x\"y\"#;\nm.delete_plan(&k);\nlet _ = r#\"raw-read\"#;\n";
        let v = scan(src);
        assert_eq!(v.len(), 2, "{v:#?}");
        assert!(v[0].contains(":6: calls `delete_plan`"), "{v:#?}");
        assert!(v[1].contains(":7: names the feature"), "{v:#?}");
    }

    #[test]
    fn allowlist_suppresses_and_stale_entries_fail() {
        let src = "fn f(m: Model) { m.delete_plan(&k); }";
        let sources = [("crates/app/src/lib.rs", src)];
        let allow = |path: &str, item: &str, reason: &str| Allow {
            path: path.into(),
            item: item.into(),
            reason: reason.into(),
        };
        let app = package("app", &sources, manifest("app", vec![], &[]), "");
        let run = |entries: Vec<Allow>| {
            let mut p = policy();
            p.allow = entries;
            check_packages(&p, &[model_package(), app.clone()]).violations
        };
        assert_eq!(run(vec![]).len(), 1);
        assert_eq!(
            run(vec![allow(
                "crates/app/src/lib.rs",
                "delete_plan",
                "not ew-model's"
            )]),
            Vec::<String>::new()
        );
        // Wrong item or wrong path: the violation stays and the entry is stale.
        let v = run(vec![allow("crates/app/src/lib.rs", "table", "x")]);
        assert_eq!(v.len(), 2, "{v:#?}");
        assert!(
            v.iter()
                .any(|m| m.contains("stale allow entry: `table` in `crates/app/src/lib.rs`")),
            "{v:#?}"
        );
        let v = run(vec![allow("crates/other/src/lib.rs", "delete_plan", "x")]);
        assert_eq!(v.len(), 2, "{v:#?}");
        // An entry that matches nothing at all fails on its own.
        let v = run(vec![
            allow("crates/app/src/lib.rs", "delete_plan", "ok"),
            allow("crates/app/src/gone.rs", "raw_read!", "old"),
        ]);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(
            v[0].starts_with("ci/dep-rules.toml:1: stale allow entry"),
            "{v:#?}"
        );
        // An empty reason fails even when the entry is used.
        let v = run(vec![allow("crates/app/src/lib.rs", "delete_plan", "  ")]);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(v[0].contains("has no reason"), "{v:#?}");
    }

    #[test]
    fn violations_are_sorted_by_path_then_line() {
        let a = "\n\nfn f(m: Model) { m.delete_plan(&k); }\nfn g(m: Model) { m.delete_plan(&k); }";
        let b = "fn f(m: Model) { m.delete_plan(&k); }";
        let out = run_app(
            &[("crates/app/src/z.rs", b), ("crates/app/src/a.rs", a)],
            manifest("app", vec![], &[]),
            "",
        );
        let heads: Vec<&str> = out
            .violations
            .iter()
            .map(|v| &v[..v.find(": ").unwrap()])
            .collect();
        assert_eq!(
            heads,
            [
                "crates/app/src/a.rs:3",
                "crates/app/src/a.rs:4",
                "crates/app/src/z.rs:1"
            ]
        );
    }

    #[test]
    fn missing_policy_crate_is_reported() {
        let out = check_packages(
            &policy(),
            &[package("app", &[], manifest("app", vec![], &[]), "")],
        );
        assert_eq!(out.violations.len(), 1);
        assert!(out.violations[0].contains("crate `ew-model` is not a workspace package"));
    }

    #[test]
    fn policy_rejects_unknown_keys() {
        let err = toml::from_str::<RawReadPolicy>(
            "crate = \"a\"\nfeature = \"f\"\nmacro = \"m\"\nreaders = []\nextra = 1\n",
        );
        assert!(err.is_err());
    }

    /// The real workspace passes, the real ew-model yields the documented gated readers, and a seeded call in
    /// an otherwise clean package fails.
    #[test]
    fn real_workspace_passes_and_seeded_violation_fails() {
        let root = crate::workspace_root();
        let (policy, mut packages) = load(&root).expect("workspace loads");
        let out = check_packages(&policy, &packages);
        assert_eq!(out.violations, Vec::<String>::new());
        assert!(out.packages_scanned > 0 && out.gated_items > 0);

        let all: Vec<(String, String)> = packages.iter().flat_map(|p| p.files.iter().cloned()).collect();
        let modules = test_modules(&all);
        let gated = gated_of(&policy, &packages, &modules).expect("ew-model is a workspace package");
        for name in [
            "table",
            "row",
            "delete_plan",
            "delete_plan_many",
            "apply_unchecked",
            "reps_by_page",
        ] {
            assert!(gated.is_gated(name), "`{name}` is gated: {:#?}", gated.items);
        }
        let table = gated
            .items
            .iter()
            .find(|i| i.name == "table")
            .expect("table is gated");
        assert_eq!(table.owner.as_deref(), Some("Model"));
        assert!(table.generic);
        assert!(gated.is_distinctive("delete_plan"));
        assert!(gated.is_distinctive("unknown_records"));
        assert!(!gated.is_gated("apply") && !gated.is_gated("from_records"));

        packages.push(package(
            "seeded",
            &[(
                "crates/seeded/src/lib.rs",
                "fn f(m: ew_model::Model) { m.delete_plan(&k); }",
            )],
            manifest("seeded", vec![], &[]),
            "",
        ));
        let out = check_packages(&policy, &packages);
        assert_eq!(out.violations.len(), 1, "{:#?}", out.violations);
        assert!(out.violations[0].starts_with("crates/seeded/src/lib.rs:1: calls `delete_plan`"));
    }
}
