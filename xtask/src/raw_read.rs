// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The raw-read rule of `cargo xtask rules`: the guard of ADR-0001 amendments 2, 4 and 5 for the raw readers of
//! `ew-model`, driven by the `[raw_read]` section of `ci/dep-rules.toml`.
//!
//! The feature rule (`features.rs`) stops a crate from *requesting* the `raw-read` feature of `ew-model`, but cargo
//! features are additive: a crate that depends on `ew-derive` is compiled with the raw readers present. Clippy closes
//! the remainder with a type-aware list that this rule generates and keeps current, and the rule keeps the opt-out of
//! that list narrow. It holds no other lexical rule: `raw_read!` is not exported, so no other crate can invoke it, and a
//! `cfg(feature = "raw-read")` in a crate without that feature is an unexpected cfg, which `-D warnings` fails.
//!
//! # The clippy list
//!
//! Method calls cannot be matched by name without type information (`Vec::len` is not `Table::len`), so the
//! calls are left to clippy's `disallowed-methods`. The workspace-root `clippy.toml` is **generated** by
//! `cargo xtask gen` from the readers `ew-model` declares under its
//! feature: every `fn <ident>` inside an invocation `raw_read! { ... }`, and every `fn <ident>` of an item
//! carrying `#[cfg(feature = "raw-read")]`, each with the owner type of its enclosing `impl` block (the last
//! path segment before the generics, for `impl Trait for Type` the `Type`). A `macro_rules!` definition is
//! skipped: the token after `fn` in its body is `$`, not an identifier. Test code of `ew-model` is skipped.
//!
//! The path of an entry is `ew_model::<Owner>::<method>` (`ew_model::Model::row`, `ew_model::Table::get`,
//! `ew_model::BaseIndexes::rebuild`) or `ew_model::<function>` for a free function. Clippy resolves such a
//! path through the type, so it flags the method call, the UFCS call, a `use ... as` rename, a type alias,
//! `<ew_model::Model>::row`, calls built by a macro, a turbofish and a function path (`iter.map(Table::len)`),
//! and it leaves same-named std methods alone. The generic `Table<T>` is listed as `ew_model::Table`.
//!
//! Clippy reports a path that does not resolve only as a plain warning, which `-D warnings` does not turn into
//! an error, and a path through a private module is not the supported form. The generator therefore refuses
//! to write an entry whose owner (or free function) is not re-exported at the crate root of the gated crate
//! (an identifier inside a `pub use` of its `src/lib.rs`, see [`root_exports`]) and fails with a message that
//! names the item. Re-export the type, or the guard cannot cover it.
//!
//! The same file carries a `disallowed-types` list of the hash containers (`std::collections::HashMap` and
//! `HashSet`, and `imbl`'s, [`DISALLOWED_TYPES`]): their iteration order must not reach a persisted,
//! exported or printed sequence. It is static (not derived from `ew-model`).
//!
//! The `disallowed-methods` list also ends with the wall-clock, environment and randomness functions
//! ([`DISALLOWED_METHODS`]: doc 18 §3.5), which share the lint `disallowed_methods` with the raw
//! readers. A helper that wraps one such call allows the lint on itself and gives the reason in the attribute
//! (`#[allow(clippy::disallowed_methods, reason = "...")]`; see below).
//!
//! The reader crates (`policy.readers`) opt out with `#![allow(clippy::disallowed_methods)]` at each target
//! root; no other crate may. The check fails when the committed file (policy key `clippy_config`, default
//! `clippy.toml`) is missing or differs from the generated text (line endings are normalised first): run
//! `cargo xtask gen`.
//!
//! # Every allow gives a reason
//!
//! In the non-test Rust source of every workspace package outside the policy's `readers` whose normal dependencies
//! reach the gated crate, an attribute `#[...]` or `#![...]` (also inside `cfg_attr(...)`) whose `allow(...)` or
//! `expect(...)` list contains `disallowed_methods` or its old name `disallowed_method` (with or without `clippy::`),
//! `renamed_and_removed_lints` (which would silence the rename warning), `clippy::style`, `clippy::all` or
//! `warnings` suppresses the guard and is a finding, but for `#[allow(clippy::disallowed_methods, reason = "...")]`
//! on one item: its reason stands where the code is. Packages whose normal dependencies cannot reach the gated crate
//! are not scanned (they cannot name its readers).
//!
//! # Asset presence
//!
//! `[raw_read.asset_presence]` (ARCH §3 rule 1): the bytes under an asset hash never change, so
//! `Model::asset`'s bytes sit outside the read gate; but whether the model holds an asset changes without a revision,
//! so no memoised query may read it. In the non-test source of the listed packages (`ew-derive`, whose `Q` holds the
//! model; reader crates included), a method call `.<accessor>(` or a path `::<accessor>` of the listed accessors
//! (`asset`, `assets`, `has_asset`) is a finding unless the innermost function around it has an
//! `[[raw_read.asset_presence.allow]]` entry (path, function, reason). `Q::logo_asset` and `Q::typeface_bytes` are the
//! entries, each recording a read of the asset store (`DepKey::Assets`), so the memo engine tracks the presence it
//! answers (ADR-0002); an entry that matches nothing fails.
//!
//! # Test code
//!
//! Test code is exempt from the suppression and asset-presence rules, as the feature rule's dev exemption is. That
//! means files under a package's top-level `tests/`, `benches/`, `examples/` and `target/` directories; any item (fn, mod, impl,
//! use, statement block) preceded by an attribute `#[cfg(test)]` or `#[cfg(all(test, ...))]` (a `test`
//! argument at the top level of an `all`), skipped through its matching `}` or its `;`; a leading inner
//! `#![cfg(test)]`; and out-of-line test modules: `#[cfg(test)] mod foo;` in `dir/lib.rs`, `dir/main.rs` or
//! `dir/mod.rs` exempts `dir/foo.rs` and `dir/foo/**`, in `dir/bar.rs` it exempts `dir/bar/foo.rs` and
//! `dir/bar/foo/**`. A `cfg` that merely mentions `test` is production code: `#[cfg(any(test, feature = "x"))]`
//! and `#[cfg(not(test))]` are scanned. An attribute on a struct field or match arm ends its item at the next
//! top-level `,`. `#[path]` and the nesting of the declaring module are not followed.
//!
//! # Lexing
//!
//! A small hand-written lexer yields identifiers, string literal contents and punctuation. Comments (line,
//! block with nesting, doc), char literals, lifetimes, raw strings (`r"..."`, `r#"..."#`), byte and C strings
//! never produce identifiers or attributes; the contents of a string count only as `cfg(feature = "...")` values.
//!
//! Scanning and generation are pure and work on `(path, text)` pairs, so the unit tests feed synthetic
//! sources; only [`run`], [`check`] and the loading code touch the file system and `cargo metadata`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

/// The policy file the section lives in (named by a stale allowlist entry).
const POLICY_FILE: &str = "ci/dep-rules.toml";

/// The authority each message cites.
const REFERENCE: &str = "ADR-0001 amendment 2";

/// Top-level directories of a package whose files are test code (or build output).
const TEST_DIRS: [&str; 4] = ["tests", "benches", "examples", "target"];

/// The hash containers of the `disallowed-types` list of the generated clippy configuration: their
/// iteration order is unspecified, so it must not reach a persisted, exported or printed sequence.
/// The persistent hash map and set of `imbl` (a dependency of three crates, doc 18 §3.5) are listed too: clippy
/// ignores a path whose crate a package does not load, and warns about a path that does not resolve in one
/// that does (which `cargo xtask check` turns into a failure). No workspace crate names
/// `hashbrown` directly; an entry for it belongs here when one does.
pub const DISALLOWED_TYPES: [&str; 4] = [
    "imbl::HashMap",
    "imbl::HashSet",
    "std::collections::HashMap",
    "std::collections::HashSet",
];

/// The reason clippy prints with a use of one of [`DISALLOWED_TYPES`].
pub const DISALLOWED_TYPES_REASON: &str = "iteration order must not reach persisted, exported or printed sequences (NFR); use BTreeMap/BTreeSet/IndexMap, or allow with a justification";

/// Where the list of wall-clock, environment and randomness functions is reasoned.
const METHODS_REFERENCE: &str = "doc 18 §3.5";

/// The reason clippy prints with a call of a wall-clock function of [`DISALLOWED_METHODS`].
const CLOCK_REASON: &str = "no wall clock in model, derive, format or export paths (doc 18 §3.5, determinism): inject a clock, or allow with a one-line justification that reviewed the site";

/// The reason clippy prints with a read of the process environment.
const ENV_REASON: &str = "no environment access in model, derive, format or export paths (doc 18 §3.5, determinism): read it at the program boundary and pass the value in, or allow with a one-line justification that reviewed the site";

/// The reason clippy prints with a source of randomness.
const RANDOM_REASON: &str = "no randomness in model, derive, format or export paths (doc 18 §3.5, determinism): ew-ids is the one ID generator, tests seed a generator; allow with a one-line justification that reviewed the site";

/// The reason clippy prints with a call of krilla's GIF or WebP image constructors: the audits
/// of gif 0.14.2, image-webp 0.2.4 and imagesize 0.14.0 rest on these paths never being reached.
const KRILLA_DECODER_REASON: &str = "krilla's GIF and WebP decoders were audited as unreachable for 0.1.0: only PNG and JPEG reach krilla, after ew_render_cpu::decode_image; a new format needs a decision and a new audit";

/// The randomly keyed hasher of the std hash containers, the generator of their iteration order: a `disallowed-types`
/// entry after [`DISALLOWED_TYPES`]. A use is a justified site.
pub const RANDOM_STATE: &str = "std::hash::RandomState";

/// The reason clippy prints with a use of [`RANDOM_STATE`].
const RANDOM_STATE_REASON: &str = "a randomly keyed hasher is a source of randomness and of hash iteration order (doc 18 §3.5, determinism): use a seeded hasher or a BTree container, or allow with a justification";

/// The generators that the workspace's feature sets do not compile today: entries with `allow-invalid = true`, so
/// that clippy accepts the path while it does not resolve and bans it once a crate enables the generator (the
/// feature bans of the feature rule keep that from happening unseen).
pub const OPTIONAL_METHODS: [(&str, &str); 5] = [
    ("uuid::Uuid::new_v4", RANDOM_REASON),
    ("uuid::Uuid::now_v7", CLOCK_REASON),
    ("rand::random", RANDOM_REASON),
    ("rand::rng", RANDOM_REASON),
    ("rand::random_range", RANDOM_REASON),
];

/// The wall-clock, environment and randomness functions of the `disallowed-methods` list of the generated clippy
/// configuration (doc 18 §3.5), after the raw readers: the path and the reason clippy prints.
///
/// Only paths that resolve in every package that loads their crate are listed (clippy warns about one that does
/// not, which `cargo xtask check` turns into a failure, and ignores one whose crate a package does not load).
/// Not listed, because they do not resolve under the workspace's feature sets: `Uuid::new_v4` and
/// `Uuid::now_v7` (`uuid` has the `v5` feature only) and `rand::random`, `rand::rng` and `rand::random_range`
/// (`rand` 0.10 without its `thread_rng` feature, which only `proptest` loads). A crate that enables a generator
/// feature adds its path here. The last two entries are not about determinism: krilla's GIF and WebP image constructors, whose decoders the audit
/// of the decoders assumes unreachable.
pub const DISALLOWED_METHODS: [(&str, &str); 13] = [
    ("std::time::SystemTime::now", CLOCK_REASON),
    ("std::time::Instant::now", CLOCK_REASON),
    ("std::env::var", ENV_REASON),
    ("std::env::var_os", ENV_REASON),
    ("std::env::vars", ENV_REASON),
    ("std::env::vars_os", ENV_REASON),
    ("std::hash::RandomState::new", RANDOM_REASON),
    ("getrandom::fill", RANDOM_REASON),
    ("getrandom::fill_uninit", RANDOM_REASON),
    ("getrandom::u32", RANDOM_REASON),
    ("getrandom::u64", RANDOM_REASON),
    ("krilla::image::Image::from_gif", KRILLA_DECODER_REASON),
    ("krilla::image::Image::from_webp", KRILLA_DECODER_REASON),
];

/// The lint the guard rests on (the item of every suppression finding).
const GUARD_LINT: &str = "disallowed_methods";

/// Lint names whose `allow` or `expect` switches the guard off in source (`clippy::` prefix included where
/// the lint has no rustc namesake).
const SUPPRESSING: [&str; 8] = [
    "disallowed_methods",
    "clippy::disallowed_methods",
    // The lint's old name (clippy still accepts it as a rename) and the rustc lint that reports the rename.
    "disallowed_method",
    "clippy::disallowed_method",
    "renamed_and_removed_lints",
    "clippy::style",
    "clippy::all",
    "warnings",
];

fn default_clippy_config() -> String {
    "clippy.toml".to_owned()
}

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
    /// The generated clippy configuration, relative to the workspace root.
    #[serde(default = "default_clippy_config")]
    pub clippy_config: String,
    /// The asset-presence rule (`[raw_read.asset_presence]`).
    #[serde(default)]
    pub asset_presence: Option<AssetPresencePolicy>,
}

/// The asset-presence rule (ARCH §3 rule 1): the bytes of an asset are outside the read
/// gate (they never change under a hash), but **whether** the model holds an asset can change without a revision, so
/// no memoised query may read it. The rule is lexical: in the non-test source of `packages` (the crate whose
/// memoised queries can reach the model, `ew-derive`), a method call `.<accessor>(` or a path `::<accessor>` of one of
/// `accessors` is a finding, unless the innermost function around it is an allowlisted site. The listed packages are
/// scanned even when they are reader crates (those opt out of clippy's list, not of this rule).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetPresencePolicy {
    /// The packages whose source is scanned.
    pub packages: Vec<String>,
    /// The model's asset accessors (`asset`, `assets`, `has_asset`).
    pub accessors: Vec<String>,
    /// The named exceptions: one function each.
    #[serde(default)]
    pub allow: Vec<AssetAllow>,
}

/// One allowlisted reader of asset presence: a function of a file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetAllow {
    /// The file, relative to the workspace root, with forward slashes.
    pub path: String,
    /// The function whose body may call the accessors.
    pub function: String,
    /// Why the read cannot go stale.
    pub reason: String,
}

/// One declaration of a gated reader in the policy crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatedItem {
    /// The function name.
    pub name: String,
    /// The type of the enclosing `impl` block, if any.
    pub owner: Option<String>,
}

/// One finding: a place in a file (or manifest) and what is wrong with it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Violation {
    /// The file, relative to the workspace root, with forward slashes.
    pub path: String,
    /// The 1-based line.
    pub line: usize,
    /// What the finding is about: `disallowed_methods`, `clippy_config` or `asset_presence:<function>` (what an
    /// asset-presence allow entry names).
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
    /// Whether the package's normal-dependency closure inside the workspace contains the gated crate. A
    /// package that cannot reach it cannot name its readers either, so it is not scanned.
    pub reaches_gated_crate: bool,
}

/// The file outside the packages that the check reads.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceFiles {
    /// The committed clippy configuration, if the file exists.
    pub clippy_config: Option<String>,
}

/// The result of one run of the check.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// The rendered violations, sorted by path then line; empty means the check passed.
    pub violations: Vec<String>,
    /// How many gated declarations were found in the policy crate.
    pub gated_items: usize,
    /// How many packages were scanned (those outside `readers` that reach the gated crate).
    pub packages_scanned: usize,
}

// ---------------------------------------------------------------------------------------------------------
// Lexer

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Tk {
    Ident(String),
    /// A string literal's content between the quotes (escapes untouched; prefixes `b`, `c`, `r#` dropped).
    Str(String),
    Punct(char),
    /// A char literal, lifetime or number.
    Other,
}

#[derive(Debug, Clone)]
pub(crate) struct Token {
    pub(crate) kind: Tk,
    pub(crate) line: usize,
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
pub(crate) struct Attr {
    pub(crate) inner: bool,
    pub(crate) open: usize,
    pub(crate) close: usize,
}

/// A lexed file with the matching delimiter of every bracket.
pub(crate) struct Parsed {
    pub(crate) toks: Vec<Token>,
    pub(crate) pair: Vec<Option<usize>>,
}

impl Parsed {
    pub(crate) fn new(text: &str) -> Self {
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

    pub(crate) fn is_p(&self, i: usize, ch: char) -> bool {
        self.toks.get(i).is_some_and(|t| t.kind == Tk::Punct(ch))
    }

    pub(crate) fn ident(&self, i: usize) -> Option<&str> {
        match self.toks.get(i) {
            Some(Token {
                kind: Tk::Ident(s), ..
            }) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn is_ident(&self, i: usize, name: &str) -> bool {
        self.ident(i) == Some(name)
    }

    pub(crate) fn line(&self, i: usize) -> usize {
        self.toks[i].line
    }

    /// The attribute starting at token `i`, if one does.
    pub(crate) fn attr_at(&self, i: usize) -> Option<Attr> {
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

    /// `cfg(test)` or `cfg(all(..))` with `test` among its top-level arguments (recursively): see the module
    /// documentation. `any(..)`, `not(..)` and a `cfg` that merely mentions `test` are production code.
    fn is_cfg_test(&self, a: &Attr) -> bool {
        self.is_ident(a.open + 1, "cfg")
            && self.is_p(a.open + 2, '(')
            && self.pair[a.open + 2] == Some(a.close - 1)
            && self.pred_is_test(a.open + 3)
    }

    /// Whether the cfg predicate that starts at token `i` holds in a test build and only there: `test`, or
    /// `all(..)` with such an argument.
    fn pred_is_test(&self, i: usize) -> bool {
        if self.is_ident(i, "test") && !self.is_p(i + 1, '(') {
            return true;
        }
        if !(self.is_ident(i, "all") && self.is_p(i + 1, '(')) {
            return false;
        }
        let Some(close) = self.pair[i + 1] else {
            return false;
        };
        let mut j = i + 2;
        while j < close {
            if self.pred_is_test(j) {
                return true;
            }
            // To the next top-level `,`.
            while j < close && !self.is_p(j, ',') {
                j = match self.toks[j].kind {
                    Tk::Punct('(' | '[' | '{') => self.pair[j].map_or(close, |m| m + 1),
                    _ => j + 1,
                };
            }
            j += 1;
        }
        false
    }

    /// The index after the attributes that start at `i`.
    pub(crate) fn skip_attrs(&self, mut i: usize) -> usize {
        while let Some(a) = self.attr_at(i) {
            i = a.close + 1;
        }
        i
    }

    /// The index after `pub` and `pub(...)` at `i`.
    pub(crate) fn skip_visibility(&self, mut i: usize) -> usize {
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
pub(crate) fn skip_angle(p: &Parsed, mut i: usize) -> usize {
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
pub(crate) fn impl_owner(p: &Parsed, lo: usize, hi: usize) -> Option<String> {
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

/// Whether `path` lies in a test directory of the package at `package_dir`, or is its build script, which runs
/// before the package is built and links none of its dependencies.
fn is_test_path(package_dir: &str, path: &str) -> bool {
    let rel = if package_dir.is_empty() {
        path
    } else {
        path.strip_prefix(package_dir)
            .and_then(|r| r.strip_prefix('/'))
            .unwrap_or(path)
    };
    if rel == "build.rs" {
        return true;
    }
    rel.split('/')
        .next()
        .is_some_and(|first| rel.contains('/') && TEST_DIRS.contains(&first))
}

// ---------------------------------------------------------------------------------------------------------
// Suppressions

/// The suppressions of the guard in one source file of a package that may not use the raw readers. Calls of the
/// readers are clippy's job (see the module documentation).
pub fn scan_source(path: &str, text: &str) -> Vec<Violation> {
    let p = Parsed::new(text);
    let exempt = exempt_mask(&p);
    let mut v = Vec::new();
    for (i, &skip) in exempt.iter().enumerate() {
        if !skip && let Some(a) = p.attr_at(i) {
            suppressions(&p, &a, path, &mut v);
        }
    }
    v
}

/// The item of an asset-presence finding: `asset_presence:<function>` (`<top level>` outside a function), so that
/// an allow entry names its function.
fn asset_item(function: Option<&str>) -> String {
    format!("asset_presence:{}", function.unwrap_or("<top level>"))
}

/// The violations of the asset-presence rule in one source file: every `.<accessor>(` and `::<accessor>`
/// outside test code, named by the innermost function around it ([`asset_item`]).
pub fn asset_presence_violations(path: &str, text: &str, rule: &AssetPresencePolicy) -> Vec<Violation> {
    let p = Parsed::new(text);
    let exempt = exempt_mask(&p);
    // The function bodies: `fn <name>` to the first `{` after it, and on to its matching `}`.
    let mut bodies: Vec<(usize, usize, String)> = Vec::new();
    for k in fn_decls(&p, 0, p.toks.len()) {
        let Some(name) = p.ident(k + 1) else { continue };
        let mut j = k + 2;
        while j < p.toks.len() && !p.is_p(j, '{') && !p.is_p(j, ';') {
            j += 1;
        }
        if p.is_p(j, '{') {
            if let Some(close) = p.pair[j] {
                bodies.push((j, close, name.to_owned()));
            }
        }
    }
    let innermost = |i: usize| -> Option<&str> {
        bodies
            .iter()
            .filter(|(open, close, _)| *open < i && i < *close)
            .min_by_key(|(open, close, _)| close - open)
            .map(|(_, _, name)| name.as_str())
    };
    let mut v = Vec::new();
    for (i, &skip) in exempt.iter().enumerate().skip(1) {
        if skip {
            continue;
        }
        let Some(name) = p.ident(i) else { continue };
        if !rule.accessors.iter().any(|a| a == name) {
            continue;
        }
        let call = p.is_p(i - 1, '.') && p.is_p(i + 1, '(');
        let path_ref = i >= 2 && p.is_p(i - 1, ':') && p.is_p(i - 2, ':');
        if !(call || path_ref) {
            continue;
        }
        let function = innermost(i);
        v.push(Violation::new(
            path,
            p.line(i),
            &asset_item(function),
            format!(
                "reads asset presence (`{name}`) in {}: whether the model holds an asset changes without a \
                 revision, so no memoised query may read it (ARCH §3 rule 1); read the \
                 bytes outside the queries, or make asset presence a tracked input of the memo engine (ADR-0002) \
                 instead of a second exception",
                function.map_or_else(|| "top-level code".to_owned(), |f| format!("`{f}`"))
            ),
        ));
    }
    v
}

/// Applies the asset-presence allowlist: drops the findings of an allowed function, and reports entries without a
/// reason and entries that suppressed nothing.
fn apply_asset_allow(violations: Vec<Violation>, allow: &[AssetAllow]) -> Vec<Violation> {
    let mut used = vec![0usize; allow.len()];
    let mut out: Vec<Violation> = violations
        .into_iter()
        .filter(|v| {
            let hit = allow
                .iter()
                .position(|a| a.path == v.path && asset_item(Some(&a.function)) == v.item);
            if let Some(k) = hit {
                used[k] += 1;
            }
            hit.is_none()
        })
        .collect();
    for (a, used) in allow.iter().zip(used) {
        let item = asset_item(Some(&a.function));
        if a.reason.trim().is_empty() {
            out.push(Violation::new(
                POLICY_FILE,
                1,
                &item,
                format!(
                    "asset-presence allow entry for `{}` in `{}` has no reason",
                    a.function, a.path
                ),
            ));
        }
        if used == 0 {
            out.push(Violation::new(
                POLICY_FILE,
                1,
                &item,
                format!(
                    "stale asset-presence allow entry: `{}` in `{}` reads no asset accessor; remove it",
                    a.function, a.path
                ),
            ));
        }
    }
    out
}

/// The `allow(...)` and `expect(...)` lists inside the attribute `a` (the attribute itself, or a `cfg_attr`
/// that wraps one) that name a lint of [`SUPPRESSING`].
fn suppressions(p: &Parsed, a: &Attr, path: &str, v: &mut Vec<Violation>) {
    for k in a.open + 1..a.close {
        let Some(kind) = p.ident(k).filter(|n| matches!(*n, "allow" | "expect")) else {
            continue;
        };
        if !p.is_p(k + 1, '(') {
            continue;
        }
        let Some(close) = p.pair[k + 1] else { continue };
        // An `allow` of the lint itself on one item that gives its reason is the reviewed exception: the reason
        // stands where the code is. A crate-wide `#![allow]`, an `expect` and the wider lint groups get no such pass.
        let reasoned = !a.inner
            && kind == "allow"
            && (k + 2..close).any(|j| p.ident(j) == Some("reason") && p.is_p(j + 1, '='));
        let mut j = k + 2;
        while j < close {
            let Some(first) = p.ident(j) else {
                j += 1;
                continue;
            };
            // A lint path `a::b::c`.
            let mut lint = first.to_owned();
            let line = p.line(j);
            j += 1;
            while p.is_p(j, ':')
                && p.is_p(j + 1, ':')
                && let Some(segment) = p.ident(j + 2)
            {
                lint.push_str("::");
                lint.push_str(segment);
                j += 3;
            }
            if SUPPRESSING.contains(&lint.as_str()) && !(reasoned && lint == "clippy::disallowed_methods") {
                v.push(Violation::new(
                    path,
                    line,
                    GUARD_LINT,
                    format!(
                        "`{kind}({lint})` suppresses the raw-read guard; that is reserved to the reader crates' roots"
                    ),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// The generated clippy configuration

/// The identifiers a `pub use` statement of `lib_rs` brings to the crate root: `pub use a::b::{C, D as E};`
/// gives `C` and `E` (the name after `as` when there is one; path segments, `self`, `crate`, `super`, `_`
/// and glob imports count for nothing). Lexical: a `cfg` on the statement is not looked at.
pub fn root_exports(lib_rs: &str) -> BTreeSet<String> {
    let p = Parsed::new(lib_rs);
    let mut out = BTreeSet::new();
    let mut i = 0;
    while i < p.toks.len() {
        if !(p.is_ident(i, "pub") && p.is_ident(i + 1, "use")) {
            i += 1;
            continue;
        }
        let mut j = i + 2;
        while j < p.toks.len() && !p.is_p(j, ';') {
            if let Some(name) = p.ident(j) {
                if name == "as" {
                    if let Some(alias) = p.ident(j + 1).filter(|a| *a != "_") {
                        out.insert(alias.to_owned());
                    }
                    j += 1;
                } else {
                    let path_segment = p.is_p(j + 1, ':') && p.is_p(j + 2, ':');
                    let renamed = p.is_ident(j + 1, "as");
                    if !path_segment && !renamed && !matches!(name, "self" | "crate" | "super") {
                        out.insert(name.to_owned());
                    }
                }
            }
            j += 1;
        }
        i = j;
    }
    out
}

/// The text of the generated clippy configuration for `gated` (see the module documentation), or the
/// messages for the gated items whose owner (or free function) is not in `exports`, the root re-exports of
/// the gated crate: such a path would not resolve, and clippy reports that only as a warning.
pub fn clippy_config(
    policy: &RawReadPolicy,
    gated: &[GatedItem],
    exports: &BTreeSet<String>,
) -> Result<String, Vec<String>> {
    let krate = policy.krate.replace('-', "_");
    let mut paths = BTreeSet::new();
    let mut errors = BTreeSet::new();
    for item in gated {
        let (head, shown, path) = match &item.owner {
            Some(owner) => (
                owner.as_str(),
                format!("{owner}::{}", item.name),
                format!("{krate}::{owner}::{}", item.name),
            ),
            None => (
                item.name.as_str(),
                item.name.clone(),
                format!("{krate}::{}", item.name),
            ),
        };
        if exports.contains(head) {
            paths.insert(path);
        } else {
            errors.insert(format!(
                "gated item `{shown}` of {} cannot be listed: `{head}` is not re-exported at the crate root of {} (a `pub use` in its src/lib.rs), so clippy could not resolve its path; re-export it",
                policy.krate, policy.krate
            ));
        }
    }
    if !errors.is_empty() {
        return Err(errors.into_iter().collect());
    }
    let readers = policy
        .readers
        .iter()
        .map(|r| format!("`{r}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let reason = format!(
        "raw reader of {} ({} feature, {REFERENCE}): read through Q in queries or TxBuilder in commands",
        policy.krate, policy.feature
    );
    // The licence tag of the generated file is spelled with an escaped hyphen so that REUSE does not read this
    // source line as a (malformed) tag of raw_read.rs itself.
    let mut out = format!(
        "# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors\n\
         # SPDX-License\u{2d}Identifier: CC0-1.0\n\
         \n\
         # Generated by `cargo xtask gen` from the readers that `{krate_name}` declares under its\n\
         # `{feature}` feature ({REFERENCE}). A stale or hand-edited file fails `cargo xtask rules`.\n\
         #\n\
         # The reader crates ({readers}) opt out with\n\
         # `#![allow(clippy::disallowed_methods)]` at each target root; no other crate may opt out of the raw\n\
         # readers. The same lint also carries the wall-clock, environment and randomness functions at the end\n\
         # of the list ({METHODS_REFERENCE}); a justified use site is allowed with\n\
         # `#[allow(clippy::disallowed_methods)]` and a one-line justification.\n\
         #\n\
         # The hash containers are listed under `disallowed-types`; a use site whose iteration\n\
         # order never reaches output is allowed with `#[allow(clippy::disallowed_types)]` and a justification.\n\
         #\n\
         # The whole file is generated: any other clippy setting would need a change of the generator\n\
         # (xtask/src/raw_read.rs).\n\
         disallowed-methods = [\n",
        krate_name = policy.krate,
        feature = policy.feature,
    );
    for path in &paths {
        out.push_str(&format!("    {{ path = \"{path}\", reason = \"{reason}\" }},\n"));
    }
    for (path, why) in DISALLOWED_METHODS {
        out.push_str(&format!("    {{ path = \"{path}\", reason = \"{why}\" }},\n"));
    }
    for (path, why) in OPTIONAL_METHODS {
        out.push_str(&format!(
            "    {{ path = \"{path}\", reason = \"{why}\", allow-invalid = true }},\n"
        ));
    }
    out.push_str("]\n\ndisallowed-types = [\n");
    for path in DISALLOWED_TYPES {
        out.push_str(&format!(
            "    {{ path = \"{path}\", reason = \"{DISALLOWED_TYPES_REASON}\" }},\n"
        ));
    }
    out.push_str(&format!(
        "    {{ path = \"{RANDOM_STATE}\", reason = \"{RANDOM_STATE_REASON}\" }},\n]\n"
    ));
    Ok(out)
}

/// The non-test files of the gated crate's package and its `src/lib.rs`.
fn model_lib<'a>(policy: &RawReadPolicy, packages: &'a [SourcePackage]) -> Option<&'a (String, String)> {
    let model = packages.iter().find(|p| p.name == policy.krate)?;
    let lib = join(&model.dir, "src/lib.rs");
    model.files.iter().find(|(path, _)| *path == lib)
}

/// The generated configuration for `gated`, with the exports read from the gated crate's `src/lib.rs`.
fn generate_with(
    policy: &RawReadPolicy,
    packages: &[SourcePackage],
    gated: &[GatedItem],
) -> Result<String, Vec<String>> {
    let Some((_, lib)) = model_lib(policy, packages) else {
        return Err(vec![format!(
            "{}'s src/lib.rs is not among the workspace sources",
            policy.krate
        )]);
    };
    clippy_config(policy, gated, &root_exports(lib))
}

/// The generated configuration of the workspace (what `cargo xtask gen` writes).
fn generate(policy: &RawReadPolicy, packages: &[SourcePackage]) -> Result<String, Vec<String>> {
    let all: Vec<(String, String)> = packages.iter().flat_map(|p| p.files.iter().cloned()).collect();
    let modules = test_modules(&all);
    match gated_of(policy, packages, &modules) {
        Some(gated) => generate_with(policy, packages, &gated),
        None => Err(vec![format!(
            "crate `{}` is not a workspace package",
            policy.krate
        )]),
    }
}

/// The first 1-based line at which `a` and `b` differ.
fn first_difference(a: &str, b: &str) -> usize {
    let mut x = a.lines();
    let mut y = b.lines();
    let mut line = 1;
    loop {
        match (x.next(), y.next()) {
            (Some(l), Some(r)) if l == r => line += 1,
            _ => return line,
        }
    }
}

/// The findings of the clippy half: the committed configuration against the generated one.
fn config_violations(
    policy: &RawReadPolicy,
    packages: &[SourcePackage],
    gated: &[GatedItem],
    ws: &WorkspaceFiles,
) -> Vec<Violation> {
    let config = policy.clippy_config.as_str();
    let mut v = Vec::new();
    match generate_with(policy, packages, gated) {
        Err(errors) => {
            let at = model_lib(policy, packages).map_or(config, |(path, _)| path.as_str());
            v.extend(
                errors
                    .into_iter()
                    .map(|e| Violation::new(at, 1, "clippy_config", e)),
            );
        }
        Ok(expected) => match &ws.clippy_config {
            None => v.push(Violation::new(
                config,
                1,
                "clippy_config",
                format!("the generated clippy configuration `{config}` is missing; run `cargo xtask gen`"),
            )),
            Some(text) => {
                let committed = text.replace("\r\n", "\n");
                if committed != expected {
                    v.push(Violation::new(
                        config,
                        first_difference(&committed, &expected),
                        "clippy_config",
                        format!(
                            "`{config}` is stale or was edited by hand (it differs from the generated text from this line on); run `cargo xtask gen`"
                        ),
                    ));
                }
            }
        },
    }
    v
}

// ---------------------------------------------------------------------------------------------------------
// Running the check

/// The gated declarations of the policy crate within `packages`.
fn gated_of(
    policy: &RawReadPolicy,
    packages: &[SourcePackage],
    modules: &TestModules,
) -> Option<Vec<GatedItem>> {
    let model = packages.iter().find(|p| p.name == policy.krate)?;
    let model_files: Vec<(String, String)> = model
        .files
        .iter()
        .filter(|(path, _)| !is_test_path(&model.dir, path) && !modules.contains(path))
        .cloned()
        .collect();
    Some(gated_items(&model_files, policy))
}

fn collect(policy: &RawReadPolicy, packages: &[SourcePackage], ws: Option<&WorkspaceFiles>) -> Outcome {
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
    let mut violations = Vec::new();
    let mut scanned = 0;
    for pkg in packages {
        if policy.readers.contains(&pkg.name) || !pkg.reaches_gated_crate {
            continue;
        }
        scanned += 1;
        for (path, text) in &pkg.files {
            if is_test_path(&pkg.dir, path) || modules.contains(path) {
                continue;
            }
            violations.extend(scan_source(path, text));
        }
    }
    // The asset-presence rule over its packages, the reader crates included.
    if let Some(rule) = &policy.asset_presence {
        let mut presence = Vec::new();
        for pkg in packages.iter().filter(|p| rule.packages.contains(&p.name)) {
            for (path, text) in &pkg.files {
                if is_test_path(&pkg.dir, path) || modules.contains(path) {
                    continue;
                }
                presence.extend(asset_presence_violations(path, text, rule));
            }
        }
        for name in &rule.packages {
            if !packages.iter().any(|p| &p.name == name) {
                presence.push(Violation::new(
                    POLICY_FILE,
                    1,
                    "asset_presence",
                    format!("[raw_read.asset_presence] names `{name}`, which is not a workspace package"),
                ));
            }
        }
        violations.extend(apply_asset_allow(presence, &rule.allow));
    }
    if let Some(ws) = ws {
        violations.extend(config_violations(policy, packages, &gated, ws));
    }
    violations.sort();
    violations.dedup();
    Outcome {
        violations: violations.iter().map(Violation::render).collect(),
        gated_items: gated.len(),
        packages_scanned: scanned,
    }
}

/// Runs the source rules over `packages` without the clippy half: pure, so the unit tests can seed violations.
#[cfg(test)]
fn check_packages(policy: &RawReadPolicy, packages: &[SourcePackage]) -> Outcome {
    collect(policy, packages, None)
}

/// Runs the whole check over `packages` and the files around them: pure, like the private `check_packages`.
pub fn check_workspace(policy: &RawReadPolicy, packages: &[SourcePackage], ws: &WorkspaceFiles) -> Outcome {
    collect(policy, packages, Some(ws))
}

// ---------------------------------------------------------------------------------------------------------
// The workspace

/// Collects the `.rs` files below `dir`: not hidden directories, not `target`, not directories that hold
/// another package's manifest.
pub(crate) fn collect_rs(
    dir: &Path,
    root: &Path,
    is_package_root: bool,
    out: &mut Vec<(String, String)>,
) -> Result<()> {
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

/// The workspace packages whose normal-dependency closure inside the workspace contains `krate`, plus
/// `krate` itself. Dev and build dependencies do not count: they cannot enable a feature on the normal edge
/// of the package's library.
fn reaches(metadata: &cargo_metadata::Metadata, krate: &str) -> BTreeSet<String> {
    let workspace: BTreeSet<String> = metadata
        .workspace_packages()
        .iter()
        .map(|p| p.name.to_string())
        .collect();
    let mut deps: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in metadata.workspace_packages() {
        let direct = p
            .dependencies
            .iter()
            .filter(|d| matches!(d.kind, cargo_metadata::DependencyKind::Normal))
            .map(|d| d.name.clone())
            .filter(|n| workspace.contains(n))
            .collect();
        deps.insert(p.name.to_string(), direct);
    }
    let mut out = BTreeSet::new();
    for name in &workspace {
        let mut stack = vec![name.clone()];
        let mut seen = BTreeSet::new();
        while let Some(n) = stack.pop() {
            if n == krate {
                out.insert(name.clone());
                break;
            }
            if seen.insert(n.clone()) {
                stack.extend(deps.get(&n).into_iter().flatten().cloned());
            }
        }
    }
    out
}

/// The text of `path` below `root`, if the file exists.
fn read_optional(root: &Path, path: &str) -> Result<Option<String>> {
    let full = root.join(path);
    if full.is_file() {
        let bytes = std::fs::read(&full).with_context(|| format!("reading {}", full.display()))?;
        Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
    } else {
        Ok(None)
    }
}

/// The `[raw_read]` section of the policy.
fn section(policy: &crate::deps::Policy) -> Result<RawReadPolicy> {
    policy
        .raw_read
        .clone()
        .with_context(|| format!("{POLICY_FILE} has no [raw_read] section (ADR-0001 amendment 2)"))
}

/// Reads the packages of the workspace at `root` (`metadata` is its `cargo metadata`) for the section `policy`.
fn load(
    root: &Path,
    policy: RawReadPolicy,
    metadata: &cargo_metadata::Metadata,
) -> Result<(RawReadPolicy, Vec<SourcePackage>, WorkspaceFiles)> {
    let reach = reaches(metadata, &policy.krate);
    let mut packages = Vec::new();
    for meta in metadata.workspace_packages() {
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
        packages.push(SourcePackage {
            name: meta.name.to_string(),
            dir,
            files,
            reaches_gated_crate: reach.contains(meta.name.as_str()),
        });
    }
    let ws = WorkspaceFiles {
        clippy_config: read_optional(root, &policy.clippy_config)?,
    };
    Ok((policy, packages, ws))
}

/// The rule `raw-read`, or with `bless` the `lint-list` step of `cargo xtask gen`: prints the outcome and returns whether the check passed. With
/// `bless` the generated clippy configuration is written first.
pub fn run(root: &Path, bless: bool) -> Result<bool> {
    let metadata = crate::metadata()?;
    let policy = section(&crate::deps::Policy::load(&root.join(POLICY_FILE))?)?;
    let (policy, packages, mut ws) = load(root, policy, &metadata)?;
    if bless {
        match generate(&policy, &packages) {
            Ok(text) => {
                let entries = text.lines().filter(|l| l.contains("{ path = ")).count();
                std::fs::write(root.join(&policy.clippy_config), &text)
                    .with_context(|| format!("writing {}", policy.clippy_config))?;
                println!("raw-read: wrote {} ({entries} entries)", policy.clippy_config);
                ws.clippy_config = Some(text);
            }
            Err(errors) => {
                eprintln!("raw-read: cannot generate {}", policy.clippy_config);
                for e in errors {
                    eprintln!("  - {e}");
                }
                return Ok(false);
            }
        }
    }
    let outcome = check_workspace(&policy, &packages, &ws);
    if outcome.violations.is_empty() {
        println!(
            "raw-read: ok ({} gated items, {} packages scanned)",
            outcome.gated_items, outcome.packages_scanned
        );
        Ok(true)
    } else {
        eprintln!("raw-read: {} violation(s)", outcome.violations.len());
        for v in &outcome.violations {
            eprintln!("  - {v}");
        }
        Ok(false)
    }
}

/// The rule `raw-read` of `cargo xtask rules` over the workspace `ws` read at `root`: its violations.
pub(crate) fn violations(root: &Path, ws: &crate::Workspace) -> Result<Vec<String>> {
    let (policy, packages, files) = load(root, section(&ws.policy)?, ws.metadata())?;
    Ok(check_workspace(&policy, &packages, &files).violations)
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
            "#,
        )
        .expect("test policy parses")
    }

    const MODEL: &str = r#"
        pub use index::BaseIndexes;
        pub use model::Model;
        pub use rev::{Row as Line, Table};
        pub use util::snapshot_rows;

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

    /// The generated text for the fixture: methods on two owners, one generic owner, one free function.
    const MODEL_CLIPPY_TEMPLATE: &str = r##"# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: CC0-1.0

# Generated by `cargo xtask gen` from the readers that `ew-model` declares under its
# `raw-read` feature (ADR-0001 amendment 2). A stale or hand-edited file fails `cargo xtask rules`.
#
# The reader crates (`ew-model`, `ew-derive`, `ew-commands`, `ew-store`) opt out with
# `#![allow(clippy::disallowed_methods)]` at each target root; no other crate may opt out of the raw
# readers. The same lint also carries the wall-clock, environment and randomness functions at the end
# of the list (doc 18 §3.5); a justified use site is allowed with
# `#[allow(clippy::disallowed_methods)]` and a one-line justification.
#
# The hash containers are listed under `disallowed-types`; a use site whose iteration
# order never reaches output is allowed with `#[allow(clippy::disallowed_types)]` and a justification.
#
# The whole file is generated: any other clippy setting would need a change of the generator
# (xtask/src/raw_read.rs).
disallowed-methods = [
    { path = "ew_model::BaseIndexes::by_page", reason = "raw reader of ew-model (raw-read feature, ADR-0001 amendment 2): read through Q in queries or TxBuilder in commands" },
    { path = "ew_model::Model::delete_plan", reason = "raw reader of ew-model (raw-read feature, ADR-0001 amendment 2): read through Q in queries or TxBuilder in commands" },
    { path = "ew_model::Model::table", reason = "raw reader of ew-model (raw-read feature, ADR-0001 amendment 2): read through Q in queries or TxBuilder in commands" },
    { path = "ew_model::Table::get", reason = "raw reader of ew-model (raw-read feature, ADR-0001 amendment 2): read through Q in queries or TxBuilder in commands" },
    { path = "ew_model::Table::len", reason = "raw reader of ew-model (raw-read feature, ADR-0001 amendment 2): read through Q in queries or TxBuilder in commands" },
    { path = "ew_model::snapshot_rows", reason = "raw reader of ew-model (raw-read feature, ADR-0001 amendment 2): read through Q in queries or TxBuilder in commands" },
@@METHODS@@
]

disallowed-types = [
    { path = "imbl::HashMap", reason = "iteration order must not reach persisted, exported or printed sequences (NFR); use BTreeMap/BTreeSet/IndexMap, or allow with a justification" },
    { path = "imbl::HashSet", reason = "iteration order must not reach persisted, exported or printed sequences (NFR); use BTreeMap/BTreeSet/IndexMap, or allow with a justification" },
    { path = "std::collections::HashMap", reason = "iteration order must not reach persisted, exported or printed sequences (NFR); use BTreeMap/BTreeSet/IndexMap, or allow with a justification" },
    { path = "std::collections::HashSet", reason = "iteration order must not reach persisted, exported or printed sequences (NFR); use BTreeMap/BTreeSet/IndexMap, or allow with a justification" },
@@RANDOM_STATE@@
]
"##;

    /// The generated text for the fixture: the template with the static wall-clock, environment and randomness
    /// entries after the raw readers.
    fn model_clippy() -> String {
        let methods: String = DISALLOWED_METHODS
            .iter()
            .map(|(path, why)| format!("    {{ path = \"{path}\", reason = \"{why}\" }},\n"))
            .collect();
        let optional: String = OPTIONAL_METHODS
            .iter()
            .map(|(path, why)| {
                format!("    {{ path = \"{path}\", reason = \"{why}\", allow-invalid = true }},\n")
            })
            .collect();
        let state = format!("    {{ path = \"{RANDOM_STATE}\", reason = \"{RANDOM_STATE_REASON}\" }},\n");
        MODEL_CLIPPY_TEMPLATE
            .replace("@@METHODS@@\n", &(methods + &optional))
            .replace("@@RANDOM_STATE@@\n", &state)
    }

    const MODEL_PATH: &str = "crates/ew-model/src/lib.rs";
    const APP_LIB: &str = "crates/app/src/lib.rs";

    fn files(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(p, t)| ((*p).to_owned(), (*t).to_owned()))
            .collect()
    }

    fn package(name: &str, sources: &[(&str, &str)]) -> SourcePackage {
        SourcePackage {
            name: name.into(),
            dir: format!("crates/{name}"),
            files: files(sources),
            reaches_gated_crate: true,
        }
    }

    fn model_package() -> SourcePackage {
        package("ew-model", &[(MODEL_PATH, MODEL)])
    }

    /// The outcome for a synthetic workspace: the fixture ew-model plus one package `app`.
    fn run_app(sources: &[(&str, &str)]) -> Outcome {
        check_packages(&policy(), &[model_package(), package("app", sources)])
    }

    fn scan(src: &str) -> Vec<String> {
        run_app(&[(APP_LIB, src)]).violations
    }

    /// The finding of the suppression `attr` at `line` of `path`.
    fn suppresses_in(path: &str, line: usize, attr: &str) -> String {
        format!(
            "{path}:{line}: `{attr}` suppresses the raw-read guard; that is reserved to the reader crates' roots (ADR-0001 amendment 2)"
        )
    }

    fn suppresses(line: usize, attr: &str) -> String {
        suppresses_in(APP_LIB, line, attr)
    }

    /// A finding the tests seed: a suppression of the guard on one item, `allow(clippy::all)`.
    const BAD: &str = "#[allow(clippy::all)] fn bad() {}";
    const BAD_ATTR: &str = "allow(clippy::all)";

    // -----------------------------------------------------------------------------------------------------
    // Gated items

    #[test]
    fn a_package_that_cannot_reach_the_gated_crate_is_not_scanned() {
        let src = "#![allow(clippy::disallowed_methods)]";
        let mut app = package("csv-tool", &[("crates/csv-tool/src/lib.rs", src)]);
        app.reaches_gated_crate = false;
        let out = check_packages(&policy(), &[model_package(), app]);
        assert!(out.violations.is_empty(), "{:?}", out.violations);
        assert_eq!(out.packages_scanned, 0);
    }

    #[test]
    fn gated_extraction_reads_macro_invocations_and_cfg_items() {
        let items = gated_items(&files(&[(MODEL_PATH, MODEL)]), &policy());
        let item = |name: &str, owner: Option<&str>| GatedItem {
            name: name.into(),
            owner: owner.map(str::to_owned),
        };
        // Not gated: the macro definition, plain items, the trait impl's `table`, test code.
        assert_eq!(
            items,
            [
                item("len", Some("Table")),
                item("get", Some("Table")),
                item("table", Some("Model")),
                item("delete_plan", Some("Model")),
                item("by_page", Some("BaseIndexes")),
                item("snapshot_rows", None),
            ]
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
        let got: Vec<(&str, Option<&str>)> = items
            .iter()
            .map(|i| (i.name.as_str(), i.owner.as_deref()))
            .collect();
        assert_eq!(got, [("a", Some("Model")), ("b", Some("Model")), ("outer", None)]);
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

    // -----------------------------------------------------------------------------------------------------
    // The generated clippy configuration

    #[test]
    fn root_exports_reads_brace_lists_and_renames() {
        let src = r#"
            pub use entity::{AnyKey, AnyRow, Entity, LibraryCopies};
            pub use model::Model;
            pub use rev::{Row as Line, Table, nested::{Deep, Other as _}};
            pub use glob::*;
            pub(crate) use hidden::Hidden;
            use private::Private;
            // pub use commented::Out;
            pub use self::inner::Inner as Renamed;
        "#;
        let got: Vec<String> = root_exports(src).into_iter().collect();
        assert_eq!(
            got,
            [
                "AnyKey",
                "AnyRow",
                "Deep",
                "Entity",
                "LibraryCopies",
                "Line",
                "Model",
                "Renamed",
                "Table"
            ]
        );
    }

    #[test]
    fn generator_output_is_exact_and_sorted() {
        let model = files(&[(MODEL_PATH, MODEL)]);
        let items = gated_items(&model, &policy());
        let exports = root_exports(MODEL);
        let text = clippy_config(&policy(), &items, &exports).expect("every owner is re-exported");
        assert_eq!(text, model_clippy());
        assert!(text.ends_with("]\n") && !text.contains('\r'));
        // The order of the declarations does not matter, and duplicates collapse.
        let mut shuffled = items.clone();
        shuffled.reverse();
        shuffled.extend(items.iter().take(2).cloned());
        assert_eq!(
            clippy_config(&policy(), &shuffled, &exports).expect("generates"),
            model_clippy()
        );
    }

    #[test]
    fn a_missing_export_is_an_error_naming_the_item() {
        let model = files(&[(MODEL_PATH, MODEL)]);
        let items = gated_items(&model, &policy());
        let exports: BTreeSet<String> = ["Model", "Table"].map(String::from).into();
        let errors = clippy_config(&policy(), &items, &exports).expect_err("owner and function missing");
        assert_eq!(errors.len(), 2, "{errors:#?}");
        assert!(
            errors[0].contains("`BaseIndexes::by_page`")
                && errors[0].contains("`BaseIndexes` is not re-exported")
                && errors[0].contains("re-export it"),
            "{errors:#?}"
        );
        assert!(
            errors[1].contains("`snapshot_rows`") && errors[1].contains("crate root of ew-model"),
            "{errors:#?}"
        );
    }

    /// The fixture workspace with the generated file committed.
    fn good_workspace() -> (Vec<SourcePackage>, WorkspaceFiles) {
        let app = package("app", &[(APP_LIB, "pub fn f() {}")]);
        let ws = WorkspaceFiles {
            clippy_config: Some(model_clippy()),
        };
        (vec![model_package(), app], ws)
    }

    fn workspace_violations(packages: &[SourcePackage], ws: &WorkspaceFiles) -> Vec<String> {
        check_workspace(&policy(), packages, ws).violations
    }

    #[test]
    fn a_committed_file_equal_to_the_generated_one_passes_even_with_crlf() {
        let (packages, mut ws) = good_workspace();
        assert_eq!(workspace_violations(&packages, &ws), Vec::<String>::new());
        ws.clippy_config = Some(model_clippy().replace('\n', "\r\n"));
        assert_eq!(workspace_violations(&packages, &ws), Vec::<String>::new());
        assert_eq!(generate(&policy(), &packages), Ok(model_clippy()));
    }

    #[test]
    fn a_missing_stale_or_edited_clippy_toml_fails() {
        let (packages, mut ws) = good_workspace();
        ws.clippy_config = None;
        let v = workspace_violations(&packages, &ws);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(
            v[0].starts_with("clippy.toml:1: the generated clippy configuration `clippy.toml` is missing;")
                && v[0].contains("cargo xtask gen"),
            "{v:#?}"
        );

        // A reader that the committed file does not list (stale), at the first differing line.
        let stale = model_clippy().replace(
            "    { path = \"ew_model::Table::len\"",
            "    # removed\n    { path = \"x\"",
        );
        ws.clippy_config = Some(stale);
        let v = workspace_violations(&packages, &ws);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(
            v[0].starts_with("clippy.toml:23: `clippy.toml` is stale or was edited by hand")
                && v[0].contains("cargo xtask gen"),
            "{v:#?}"
        );

        // A hand-added setting.
        ws.clippy_config = Some(format!("{}msrv = \"1.80\"\n", model_clippy()));
        let v = workspace_violations(&packages, &ws);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(v[0].contains("stale or was edited by hand"), "{v:#?}");
    }

    #[test]
    fn a_reader_without_a_root_export_fails_the_check() {
        let (mut packages, ws) = good_workspace();
        packages[0].files = files(&[(MODEL_PATH, &MODEL.replace("pub use index::BaseIndexes;", ""))]);
        let v = workspace_violations(&packages, &ws);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(
            v[0].starts_with(
                "crates/ew-model/src/lib.rs:1: gated item `BaseIndexes::by_page` of ew-model cannot be listed"
            ),
            "{v:#?}"
        );
    }

    // -----------------------------------------------------------------------------------------------------
    // Suppressions

    #[test]
    fn passing_crate_is_clean() {
        let src = r#"
            use ew_model::{Model, Rev};
            #![allow(clippy::too_many_arguments)]
            #[allow(dead_code)]
            #[expect(clippy::needless_return)]
            #[derive(Debug)]
            fn f(m: Model, v: Vec<u8>, ops: Vec<Op>) {
                let m1 = m.apply(&ops, Rev(1)).unwrap();
                let n = v.len();
                let r = 0..raw_read_len;
                let raw_read = 1; let _ = raw_read != 2;
            }
        "#;
        assert_eq!(scan(src), Vec::<String>::new());
    }

    #[test]
    fn method_calls_are_left_to_clippy() {
        // Without type information these are indistinguishable from std calls; clippy flags them.
        let src = "fn f(m: Model, t: Table<Device>) { let _ = (m.delete_plan(&k), Table::len(&t), t.len(), ew_model::snapshot_rows()); }";
        assert_eq!(scan(src), Vec::<String>::new());
    }

    #[test]
    fn suppressions_of_the_guard_fail() {
        let src = "#![allow(clippy::disallowed_methods)]\n\
            #[allow(clippy::all)]\n\
            fn a() {}\n\
            #[cfg_attr(feature = \"x\", allow(clippy::disallowed_methods))]\n\
            fn b() {}\n\
            #![allow(warnings)]\n\
            #[expect(disallowed_methods, reason = \"x\")]\n\
            fn c() {}\n\
            #[allow(dead_code, clippy::style)]\n\
            fn d() {}\n\
            #[allow(clippy::disallowed_methods, reason = \"the environment, read once\")]\n\
            fn e() {}\n\
            #[allow(clippy::all, reason = \"x\")]\n\
            fn f() {}\n";
        assert_eq!(
            scan(src),
            [
                suppresses(1, "allow(clippy::disallowed_methods)"),
                suppresses(2, "allow(clippy::all)"),
                suppresses(4, "allow(clippy::disallowed_methods)"),
                suppresses(6, "allow(warnings)"),
                suppresses(7, "expect(disallowed_methods)"),
                suppresses(9, "allow(clippy::style)"),
                suppresses(13, "allow(clippy::all)"),
            ]
        );
    }

    #[test]
    fn old_lint_name_and_rename_warning_suppressions_fail() {
        let src = "#![allow(clippy::disallowed_method)]\n\
            #[expect(disallowed_method, reason = \"x\")]\n\
            fn a() {}\n\
            #[cfg_attr(feature = \"x\", allow(renamed_and_removed_lints))]\n\
            fn b() {}\n";
        assert_eq!(
            scan(src),
            [
                suppresses(1, "allow(clippy::disallowed_method)"),
                suppresses(2, "expect(disallowed_method)"),
                suppresses(4, "allow(renamed_and_removed_lints)"),
            ]
        );
    }

    #[test]
    fn harmless_attributes_and_exempt_suppressions_do_not_fail() {
        // Other lints, a `cfg(all(..))` mentioning `all`, and the lint name outside an allow/expect list.
        let src = "#[allow(clippy::too_many_arguments)]\n\
            #[cfg(all(unix, not(windows)))]\n\
            #[warn(clippy::disallowed_methods)]\n\
            #[deny(warnings)]\n\
            fn f() {}\n";
        assert_eq!(scan(src), Vec::<String>::new());

        let allow = "#[allow(clippy::disallowed_methods)]";
        // Test code (items and files) may carry the allow.
        let src = format!("#[cfg(test)]\n{allow}\nmod tests {{ {allow} fn t() {{}} }}\n");
        assert_eq!(scan(&src), Vec::<String>::new());
        let out = run_app(&[
            (APP_LIB, "pub fn f() {}"),
            ("crates/app/tests/it.rs", "#![allow(clippy::disallowed_methods)]"),
            ("crates/app/benches/b.rs", "#![allow(warnings)]"),
            ("crates/app/examples/e.rs", "#![allow(clippy::all)]"),
        ]);
        assert_eq!(out.violations, Vec::<String>::new());
    }

    #[test]
    fn readers_may_suppress() {
        let src = "#![allow(clippy::disallowed_methods)]\n#[allow(warnings)] fn h() {}\n";
        let readers = ["ew-derive", "ew-commands", "ew-store"]
            .map(|n| package(n, &[(&format!("crates/{n}/src/lib.rs"), src)]));
        let mut packages = vec![model_package()];
        packages.extend(readers);
        let out = check_packages(&policy(), &packages);
        assert_eq!(out.violations, Vec::<String>::new());
        assert_eq!(out.packages_scanned, 0);
        assert_eq!(out.gated_items, 6);
    }

    // -----------------------------------------------------------------------------------------------------
    // Test code

    #[test]
    fn test_code_is_exempt() {
        let bad = BAD;
        let src = format!(
            "#[cfg(test)]\nmod tests {{ fn t() {{ {bad} }} }}\n\
             #[cfg(test)]\nfn helper() {{ {bad} }}\n\
             #[cfg(all(test, feature = \"x\"))]\nfn both() {{ {bad} }}\n\
             #[cfg(all(feature = \"x\", test))]\nfn both_reversed() {{ {bad} }}\n\
             #[cfg(test)]\nuse ew_model::snapshot_rows;\n\
             fn run() {{\n    #[cfg(test)]\n    {{ {bad} }}\n}}\n\
             struct S {{\n    #[cfg(test)]\n    a: Vec<A, B>,\n}}\n"
        );
        assert_eq!(scan(&src), Vec::<String>::new());
        // `not(test)` is scanned, and so is code after an exempt item.
        let src =
            format!("#[cfg(not(test))]\nfn f() {{ {bad} }}\n#[cfg(test)]\nfn g() {{}}\nfn h() {{ {bad} }}\n");
        assert_eq!(scan(&src), [suppresses(2, BAD_ATTR), suppresses(5, BAD_ATTR)]);
        // A leading inner attribute exempts the file.
        assert_eq!(
            scan(&format!("#![cfg(test)]\nfn h() {{ {bad} }}")),
            Vec::<String>::new()
        );
        assert_eq!(
            scan(&format!("#![cfg(all(test, unix))]\nfn h() {{ {bad} }}")),
            Vec::<String>::new()
        );
    }

    /// Only `cfg(test)` and `cfg(all(.., test, ..))` are test code.
    #[test]
    fn a_cfg_that_merely_mentions_test_is_production_code() {
        let bad = BAD;
        for cfg in [
            "any(test, feature = \"x\")",
            "any(feature = \"x\", test)",
            "not(test)",
            "all(any(test, x), y)",
            "all(not(test), y)",
            "feature = \"test\"",
            "all(unix, windows)",
        ] {
            let v = scan(&format!("#[cfg({cfg})]\nfn f() {{ {bad} }}\n"));
            assert_eq!(v, [suppresses(2, BAD_ATTR)], "cfg({cfg})");
            // The inner attribute does not exempt the file either.
            let v = scan(&format!("#![cfg({cfg})]\nfn f() {{ {bad} }}\n"));
            assert_eq!(v, [suppresses(2, BAD_ATTR)], "#![cfg({cfg})]");
        }
        for cfg in ["test", "all(test, feature = \"x\")", "all(x, all(test, y))"] {
            let v = scan(&format!("#[cfg({cfg})]\nfn f() {{ {bad} }}\n"));
            assert_eq!(v, Vec::<String>::new(), "cfg({cfg})");
        }
        // Out-of-line modules follow the same rule.
        let out = run_app(&[
            (
                APP_LIB,
                "#[cfg(any(test, feature = \"x\"))]\nmod either;\n#[cfg(all(test, feature = \"x\"))]\nmod both;\n",
            ),
            ("crates/app/src/either.rs", bad),
            ("crates/app/src/both.rs", bad),
        ]);
        assert_eq!(
            out.violations,
            [suppresses_in("crates/app/src/either.rs", 1, BAD_ATTR)]
        );
    }

    #[test]
    fn test_directories_and_out_of_line_test_modules_are_exempt() {
        let bad = BAD;
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
        let out = run_app(&sources);
        assert_eq!(
            out.violations,
            [
                suppresses_in("crates/app/src/bin/tests/x.rs", 1, BAD_ATTR),
                suppresses_in("crates/app/src/live/other.rs", 1, BAD_ATTR),
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
            // raw_read! { fn x() {} } "raw-read" #[allow(warnings)]
            /* /* nested raw_read! {} */ still comment: #![allow(clippy::all)] */
            /// Docs: raw_read!, #[allow(clippy::disallowed_methods)]
            //! raw_read!
            fn f() {
                let a = "raw_read!";
                let b = "#[allow(warnings)]";
                let c = '"';
                let d = '\'';
                let e = '\\';
                let lt: &'static str = "x";
                fn g<'a>(x: &'a u8) -> &'a u8 { x }
                let r1 = r"raw_read! {}";
                let r2 = r#"he said "raw_read!" and "raw-read""#;
                let r3 = r##"a "# b raw_read! "##;
                let b1 = b"raw_read!";
                let b2 = br#"#[allow(warnings)]"#;
                let b3 = b'x';
                let multi = "line one
                    raw_read!";
                let q = '"'; let s = "x"; // raw_read! {}
            }
        "###;
        assert_eq!(scan(src), Vec::<String>::new());
        // The line count survives the multi-line literals and comments before a finding.
        let src = "/* a\nb */\nlet _ = \"multi\nline\";\nlet r = r#\"x\"y\"#;\n#[allow(warnings)] fn f() {}\nlet _ = r#\"#[allow(warnings)]\"#;\n";
        assert_eq!(scan(src), [suppresses(6, "allow(warnings)")]);
    }

    #[test]
    fn violations_are_sorted_by_path_then_line() {
        let a = format!("\n\n{BAD}\n{BAD}");
        let out = run_app(&[("crates/app/src/z.rs", BAD), ("crates/app/src/a.rs", a.as_str())]);
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

    // -----------------------------------------------------------------------------------------------------
    // Policy

    #[test]
    fn missing_policy_crate_is_reported() {
        let out = check_packages(&policy(), &[package("app", &[])]);
        assert_eq!(out.violations.len(), 1);
        assert!(out.violations[0].contains("crate `ew-model` is not a workspace package"));
    }

    #[test]
    fn policy_rejects_unknown_keys_and_defaults_the_clippy_path() {
        let err = toml::from_str::<RawReadPolicy>(
            "crate = \"a\"\nfeature = \"f\"\nmacro = \"m\"\nreaders = []\nextra = 1\n",
        );
        assert!(err.is_err());
        assert_eq!(policy().clippy_config, "clippy.toml");
        let p = toml::from_str::<RawReadPolicy>(
            "crate = \"a\"\nfeature = \"f\"\nmacro = \"m\"\nreaders = []\nclippy_config = \"cfg/clippy.toml\"\n",
        )
        .expect("parses");
        assert_eq!(p.clippy_config, "cfg/clippy.toml");
    }

    // -----------------------------------------------------------------------------------------------------
    // Asset presence

    const DERIVE_Q: &str = "crates/ew-derive/src/q.rs";

    fn asset_policy(allow: &str) -> RawReadPolicy {
        toml::from_str(&format!(
            r#"
            crate = "ew-model"
            feature = "raw-read"
            macro = "raw_read"
            readers = ["ew-model", "ew-derive", "ew-commands", "ew-store"]

            [asset_presence]
            packages = ["ew-derive"]
            accessors = ["asset", "assets", "has_asset"]
            {allow}
            "#
        ))
        .expect("test policy parses")
    }

    const LOGO_ALLOW: &str = r#"
            [[asset_presence.allow]]
            path = "crates/ew-derive/src/q.rs"
            function = "logo_asset"
            reason = "the named exception"
    "#;

    fn derive_run(policy: &RawReadPolicy, src: &str) -> Vec<String> {
        let derive = package("ew-derive", &[(DERIVE_Q, src)]);
        check_packages(policy, &[model_package(), derive]).violations
    }

    #[test]
    fn the_allowlisted_logo_asset_site_passes() {
        let src = "impl Q {\n    pub fn logo_asset(&self, h: &Sha256) -> Option<AssetRef> {\n        self.model.asset(h).map(|a| *a.reference())\n    }\n}\n";
        assert_eq!(derive_run(&asset_policy(LOGO_ALLOW), src), Vec::<String>::new());
    }

    #[test]
    fn a_seeded_read_of_asset_presence_in_a_query_fails() {
        // A memoised query that asks whether the logo is held: the answer can change without a revision.
        let src = "impl Q {\n    pub fn logo_asset(&self, h: &Sha256) -> Option<AssetRef> {\n        self.model.asset(h).map(|a| *a.reference())\n    }\n}\n\nfn title_block(q: &Q) -> bool {\n    q.model.assets().count() > 0 || ew_model::Model::has_asset(&q.model, &H)\n}\n";
        let out = derive_run(&asset_policy(LOGO_ALLOW), src);
        assert_eq!(out.len(), 2, "{out:#?}");
        assert!(
            out[0]
                .starts_with("crates/ew-derive/src/q.rs:8: reads asset presence (`assets`) in `title_block`"),
            "{out:#?}"
        );
        assert!(out[1].contains("(`has_asset`) in `title_block`"), "{out:#?}");
        assert!(
            out.iter().all(|v| v.contains("no memoised query may read it")),
            "{out:#?}"
        );
    }

    #[test]
    fn test_code_fields_and_other_names_are_not_asset_reads() {
        let src = "struct Store { assets: Vec<u8> }\nfn count(s: &Store) -> usize { s.assets.len() + asset_count() }\n#[cfg(test)]\nmod tests {\n    fn t(m: &Model) { m.asset(&H); }\n}\n";
        assert_eq!(derive_run(&asset_policy(""), src), Vec::<String>::new());
    }

    #[test]
    fn a_stale_asset_presence_entry_fails() {
        let out = derive_run(&asset_policy(LOGO_ALLOW), "fn nothing() {}\n");
        assert_eq!(out.len(), 1, "{out:#?}");
        assert!(
            out[0].contains("stale asset-presence allow entry: `logo_asset`"),
            "{out:#?}"
        );
    }
}
