// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `outline` and `where`: the items of a Rust file with their line spans, and a name's definitions and uses, read
//! from the files of the worktree on every run (no index kept), so that a reader opens only the lines it needs.
//!
//! The scanner leans on the layout rustfmt gives the code: an item starts on a line at its parent's indentation
//! plus four, and ends where its brackets close or at its `;`. Strings, characters and comments are blanked before
//! the brackets are counted. What it does not read: an item a macro writes, and a file rustfmt has not formatted.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::{crate_map, repo};

/// A directory's outline starts no file after this many lines; the files left out are named.
const OUTLINE_LINES: usize = 300;
/// `where` prints at most this many uses of a name, then says how many more there are.
const WHERE_USES: usize = 100;
/// A doc sentence or a signature longer than this is cut, with `…`.
const SENTENCE_CHARS: usize = 80;
const SIGNATURE_CHARS: usize = 140;
/// The action map, whose row for a name `where` prints.
const ACTION_MAP: &str = "docs/map/actions.md";

/// A source text line by line: the text, the same text with string, character and comment contents blanked
/// (as many characters), and whether the line starts in code (not inside a string or a block comment).
struct Source {
    raw: Vec<String>,
    code: Vec<String>,
    in_code: Vec<bool>,
}

/// One item: its span (0-based lines, its doc and attributes included), the line of its keyword, its signature,
/// the first sentence of its doc and, for an `impl`, a `trait` or an inline `mod`, the items inside.
#[derive(Debug)]
struct Item {
    start: usize,
    line: usize,
    end: usize,
    name: Option<String>,
    signature: String,
    doc: String,
    /// `#[cfg(test)]` on a `mod`: the number of `#[test]` lines in it.
    tests: Option<usize>,
    children: Vec<Item>,
}

/// The lexer's state between characters.
enum State {
    Code,
    LineComment,
    BlockComment(u32),
    Str,
    RawStr(usize),
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

impl Source {
    /// Splits `text` into lines and blanks the contents of its strings, characters and comments.
    fn new(text: &str) -> Self {
        let chars: Vec<char> = text.chars().collect();
        let mut src = Source {
            raw: Vec::new(),
            code: Vec::new(),
            in_code: vec![true],
        };
        let (mut raw, mut code) = (String::new(), String::new());
        let mut state = State::Code;
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            let next = chars.get(i + 1).copied();
            if c == '\n' {
                if raw.ends_with('\r') {
                    raw.pop();
                    code.pop();
                }
                src.raw.push(std::mem::take(&mut raw));
                src.code.push(std::mem::take(&mut code));
                if matches!(state, State::LineComment) {
                    state = State::Code;
                }
                src.in_code.push(matches!(state, State::Code));
                i += 1;
                continue;
            }
            // The number of characters this step takes, and how many of them stay visible in the code.
            let (take, keep) = match state {
                State::Code => match c {
                    '/' if next == Some('/') => {
                        state = State::LineComment;
                        (2, 0)
                    }
                    '/' if next == Some('*') => {
                        state = State::BlockComment(1);
                        (2, 0)
                    }
                    '"' => {
                        state = State::Str;
                        (1, 1)
                    }
                    'r' | 'b' | 'c' if i == 0 || !is_ident(chars[i - 1]) => {
                        let prefix = if c != 'r' && next == Some('r') { 2 } else { usize::from(c == 'r') };
                        let hashes = chars
                            .get(i + prefix..)
                            .unwrap_or_default()
                            .iter()
                            .take_while(|&&h| h == '#')
                            .count();
                        if prefix > 0 && chars.get(i + prefix + hashes) == Some(&'"') {
                            state = State::RawStr(hashes);
                            (prefix + hashes + 1, prefix + hashes + 1)
                        } else {
                            (1, 1)
                        }
                    }
                    '\'' if next == Some('\\') => {
                        // An escaped character: up to the closing quote.
                        let mut j = i + 3;
                        while j < chars.len() && chars[j] != '\'' && chars[j] != '\n' {
                            j += 1;
                        }
                        let len = (j + 1).min(chars.len()) - i;
                        raw.extend(&chars[i..i + len]);
                        code.push('\'');
                        code.extend(std::iter::repeat_n(' ', len.saturating_sub(2)));
                        if len >= 2 {
                            code.push('\'');
                        }
                        i += len;
                        continue;
                    }
                    '\'' if chars.get(i + 2) == Some(&'\'') && next != Some('\n') => {
                        raw.extend(&chars[i..i + 3]);
                        code.push_str("' '");
                        i += 3;
                        continue;
                    }
                    _ => (1, 1),
                },
                State::LineComment => (1, 0),
                State::BlockComment(depth) => {
                    if c == '/' && next == Some('*') {
                        state = State::BlockComment(depth + 1);
                        (2, 0)
                    } else if c == '*' && next == Some('/') {
                        state = if depth == 1 { State::Code } else { State::BlockComment(depth - 1) };
                        (2, 0)
                    } else {
                        (1, 0)
                    }
                }
                State::Str => match c {
                    '\\' if next != Some('\n') => (2, 0),
                    '"' => {
                        state = State::Code;
                        (1, 1)
                    }
                    _ => (1, 0),
                },
                State::RawStr(hashes) => {
                    if c == '"' && chars[i + 1..].iter().take(hashes).filter(|&&h| h == '#').count() == hashes {
                        state = State::Code;
                        (hashes + 1, hashes + 1)
                    } else {
                        (1, 0)
                    }
                }
            };
            let take = take.min(chars.len() - i);
            for (k, &ch) in chars[i..i + take].iter().enumerate() {
                raw.push(ch);
                code.push(if k < keep { ch } else { ' ' });
            }
            i += take;
        }
        // A text that ends with a line break has no line after it.
        if raw.is_empty() && !src.raw.is_empty() {
            src.in_code.pop();
        } else {
            src.raw.push(raw);
            src.code.push(code);
        }
        src
    }

    fn len(&self) -> usize {
        self.raw.len()
    }

    /// The text of a `///` line, if line `n` is one.
    fn doc(&self, n: usize) -> Option<&str> {
        let t = self.raw[n].trim_start();
        if !self.in_code[n] || t.starts_with("////") {
            return None;
        }
        t.strip_prefix("///").map(|d| d.strip_prefix(' ').unwrap_or(d))
    }

    /// The line where the item or statement starting at line `from` ends, and the line of its body's `{` if it has
    /// one: the first `;` outside brackets, or the `}` that closes its brackets.
    fn extent(&self, from: usize) -> (usize, Option<usize>) {
        let mut depth = 0_i32;
        let mut open = None;
        for (n, line) in self.code.iter().enumerate().skip(from) {
            for c in line.chars() {
                match c {
                    '(' | '[' => depth += 1,
                    '{' => {
                        if depth == 0 && open.is_none() {
                            open = Some(n);
                        }
                        depth += 1;
                    }
                    ')' | ']' => depth -= 1,
                    '}' => {
                        depth -= 1;
                        if depth <= 0 {
                            return (n, open);
                        }
                    }
                    ';' if depth <= 0 => return (n, open),
                    _ => {}
                }
            }
        }
        (self.len() - 1, open)
    }

    /// The last line of the attribute that starts at line `from`.
    fn attribute_end(&self, from: usize) -> usize {
        let mut depth = 0_i32;
        for (n, line) in self.code.iter().enumerate().skip(from) {
            for c in line.chars() {
                match c {
                    '(' | '[' | '{' => depth += 1,
                    ')' | ']' | '}' => {
                        depth -= 1;
                        if depth <= 0 {
                            return n;
                        }
                    }
                    _ => {}
                }
            }
        }
        self.len() - 1
    }

    /// The items between lines `from` and `to` (exclusive) at indentation `indent`.
    fn items(&self, from: usize, to: usize, indent: usize) -> Vec<Item> {
        let mut items = Vec::new();
        let (mut start, mut doc, mut test) = (None, Vec::new(), false);
        let mut n = from;
        while n < to {
            let code = &self.code[n];
            let trimmed = code.trim_start();
            if trimmed.is_empty() {
                if let Some(d) = self.doc(n) {
                    start.get_or_insert(n);
                    doc.push(d);
                } else if self.raw[n].trim().is_empty() {
                    (start, test) = (None, false);
                    doc.clear();
                }
                n += 1;
                continue;
            }
            if !self.in_code[n] || code.len() - trimmed.len() != indent {
                n += 1;
                continue;
            }
            if trimmed.starts_with('#') {
                let end = self.attribute_end(n).min(to - 1);
                if trimmed.starts_with("#[") {
                    start.get_or_insert(n);
                    let text = self.code[n..=end].concat();
                    test |= text.contains("cfg(test)") || text.contains("cfg(all(test");
                }
                n = end + 1;
                continue;
            }
            let (end, open) = self.extent(n);
            let end = end.min(to - 1);
            if let Some((kind, name)) = header(trimmed) {
                let children = match open {
                    Some(open) if open < end && matches!(kind, "impl" | "trait" | "mod") => {
                        self.items(open + 1, end, indent + 4)
                    }
                    _ => Vec::new(),
                };
                let tests = (test && kind == "mod" && open.is_some()).then(|| {
                    self.raw[n..=end]
                        .iter()
                        .filter(|l| l.trim_start().starts_with("#[test]"))
                        .count()
                });
                items.push(Item {
                    start: start.unwrap_or(n),
                    line: n,
                    end,
                    name,
                    signature: self.signature(n, end, kind),
                    doc: first_sentence(&doc),
                    tests,
                    children,
                });
            }
            (start, test) = (None, false);
            doc.clear();
            n = end + 1;
        }
        items
    }

    /// The signature of the item whose keyword stands on line `line`: its lines joined, up to its body, its `;`,
    /// its `where` clause or (a `const` or `static`) its value.
    fn signature(&self, line: usize, end: usize, kind: &str) -> String {
        let (mut raw, mut code) = (Vec::new(), Vec::new());
        for n in line..=end {
            let lead = self.code[n].chars().take_while(|c| c.is_whitespace()).count();
            if n > line {
                raw.push(' ');
                code.push(' ');
            }
            raw.extend(self.raw[n].chars().skip(lead));
            code.extend(self.code[n].chars().skip(lead));
        }
        let mut depth = 0_i32;
        let mut cut = code.len();
        for (i, &c) in code.iter().enumerate() {
            let at_word = |w: &str| {
                let w: Vec<char> = w.chars().collect();
                code[i..].starts_with(&w)
                    && (i == 0 || !is_ident(code[i - 1]))
                    && code.get(i + w.len()).is_none_or(|&c| !is_ident(c))
            };
            match c {
                '(' | '[' => depth += 1,
                ')' | ']' => depth -= 1,
                '{' | ';' if depth <= 0 => {
                    cut = i;
                    break;
                }
                '=' if depth <= 0
                    && matches!(kind, "const" | "static")
                    && code.get(i + 1) == Some(&' ')
                    && i > 0
                    && code[i - 1] == ' ' =>
                {
                    cut = i;
                    break;
                }
                'w' if depth <= 0 && at_word("where") => {
                    cut = i;
                    break;
                }
                _ => {}
            }
        }
        let text: String = raw[..cut].iter().collect();
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let text = text
            .replace("( ", "(")
            .replace(", )", ")")
            .replace(" )", ")")
            .replace("< ", "<")
            .replace(", >", ">");
        cut_to(text.trim_end(), SIGNATURE_CHARS)
    }

    /// The first two lines of the file's `//!` head, ` …` added when it goes on.
    fn head(&self) -> Vec<String> {
        let lines: Vec<&str> = self
            .raw
            .iter()
            .map(|l| l.trim_start())
            .skip_while(|l| !l.starts_with("//!"))
            .map_while(|l| l.strip_prefix("//!"))
            .map(str::trim)
            .collect();
        let mut head: Vec<String> = lines.iter().take(2).map(|l| (*l).to_owned()).collect();
        if lines.len() > 2
            && let Some(last) = head.last_mut()
        {
            last.push_str(" …");
        }
        head
    }
}

/// The kind and the name of the item a line starts, if it starts one: `fn`, `struct`, `enum`, `union`, `trait`,
/// `impl` (no name), `mod`, `const`, `static`, `type` or `macro_rules`, after its visibility and qualifiers.
fn header(line: &str) -> Option<(&'static str, Option<String>)> {
    fn word(s: &str) -> (&str, &str) {
        let end = s.find(|c: char| !is_ident(c)).unwrap_or(s.len());
        (&s[..end], s[end..].trim_start())
    }
    let mut rest = line;
    loop {
        let (w, after) = word(rest);
        rest = after;
        let kind = match w {
            "pub" => {
                if rest.starts_with('(') {
                    rest = rest[rest.find(')')? + 1..].trim_start();
                }
                continue;
            }
            "default" | "async" | "unsafe" | "safe" | "auto" => continue,
            "extern" => {
                if let Some(abi) = rest.strip_prefix('"') {
                    rest = abi[abi.find('"')? + 1..].trim_start();
                }
                if word(rest).0 == "fn" {
                    continue;
                }
                return None;
            }
            "const" if matches!(word(rest).0, "fn" | "unsafe" | "async" | "extern") => continue,
            "impl" => return Some(("impl", None)),
            "macro_rules" => {
                rest = rest.strip_prefix('!')?.trim_start();
                "macro_rules"
            }
            "fn" => "fn",
            "struct" => "struct",
            "enum" => "enum",
            "union" => "union",
            "trait" => "trait",
            "mod" => "mod",
            "const" => "const",
            "static" => "static",
            "type" => "type",
            _ => return None,
        };
        let (mut name, after) = word(rest);
        if kind == "static" && name == "mut" {
            name = word(after).0;
        }
        return (!name.is_empty()).then(|| (kind, Some(name.to_owned())));
    }
}

/// The first sentence of a doc comment, up to its first blank line or code fence, cut to a line's length.
fn first_sentence(doc: &[&str]) -> String {
    let text = doc
        .iter()
        .map(|l| l.trim())
        .take_while(|l| !l.is_empty() && !l.starts_with("```"))
        .collect::<Vec<_>>()
        .join(" ");
    cut_to(&crate_map::sentences(&text, 1), SENTENCE_CHARS)
}

fn cut_to(text: &str, chars: usize) -> String {
    match text.char_indices().nth(chars) {
        Some((i, _)) => format!("{}…", &text[..i]),
        None => text.to_owned(),
    }
}

/// Whether a file says that it is generated, in its first lines.
fn generated(text: &str) -> bool {
    text.lines()
        .take(12)
        .any(|l| l.contains("@generated") || l.contains("do not edit") || l.contains("Do not edit"))
}

/// `outline [--tests] <path>...`: prints the outline of each Rust file named, or of each one under a directory.
pub(crate) fn outline(args: &[String], root: &Path) -> Result<bool> {
    let tests = args.iter().any(|a| a == "--tests");
    let paths: Vec<&String> = args.iter().filter(|a| *a != "--tests").collect();
    if paths.is_empty() {
        bail!("outline needs a file or a directory: cargo xtask outline [--tests] <path>...");
    }
    let cwd = std::env::current_dir().context("reading the current directory")?;
    let mut files: Option<Vec<String>> = None;
    let mut out = String::new();
    for arg in paths {
        let rel = relative(root, &cwd.join(arg)).with_context(|| arg.clone())?;
        if root.join(&rel).is_dir() {
            let all = match &mut files {
                Some(files) => files,
                None => files.insert(repo::list_files(root)?),
            };
            let prefix = if rel.is_empty() { String::new() } else { format!("{rel}/") };
            let under: Vec<&String> =
                all.iter().filter(|f| f.starts_with(&prefix) && f.ends_with(".rs")).collect();
            if under.is_empty() {
                writeln!(out, "{rel}: no Rust file")?;
            }
            let (mut lines, mut left, mut skipped) = (0, Vec::new(), Vec::new());
            for path in under {
                let text = read(root, path)?;
                if generated(&text) {
                    skipped.push(&path[prefix.len()..]);
                } else if lines >= OUTLINE_LINES {
                    left.push(&path[prefix.len()..]);
                } else {
                    let before = out.len();
                    write_outline(&mut out, path, &text, tests)?;
                    lines += out[before..].lines().count();
                }
            }
            if !left.is_empty() {
                writeln!(out, "… {} more files left out, outline them one by one: {}", left.len(), left.join(", "))?;
            }
            if !skipped.is_empty() {
                writeln!(out, "generated, left out: {}", skipped.join(", "))?;
            }
        } else if rel.ends_with(".rs") {
            write_outline(&mut out, &rel, &read(root, &rel)?, tests)?;
        } else {
            bail!("{arg} is neither a directory nor a Rust file");
        }
    }
    print!("{out}");
    Ok(true)
}

/// A path as `/`-separated text relative to the workspace root; an error outside it.
fn relative(root: &Path, path: &Path) -> Result<String> {
    let path = path.canonicalize().context("no such file or directory")?;
    let root = root.canonicalize().context("reading the workspace root")?;
    let rel = path.strip_prefix(&root).context("not inside this workspace")?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

fn read(root: &Path, path: &str) -> Result<String> {
    let bytes = std::fs::read(root.join(path)).with_context(|| format!("reading {path}"))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The outline of one file: its path and length, its head, then one line an item.
fn write_outline(out: &mut String, path: &str, text: &str, tests: bool) -> Result<()> {
    let src = Source::new(text);
    let items = src.items(0, src.len(), 0);
    writeln!(out, "{path} ({} lines)", src.len())?;
    for line in src.head() {
        writeln!(out, "  //! {line}")?;
    }
    write_items(out, &items, 0, span_width(&items), tests)
}

/// The widest span of `items` and their children.
fn span_width(items: &[Item]) -> usize {
    items.iter().map(|i| span(i).len().max(span_width(&i.children))).max().unwrap_or(0)
}

/// One line an item, its children indented below it; a tests module as one line unless `tests`.
fn write_items(out: &mut String, items: &[Item], depth: usize, width: usize, tests: bool) -> Result<()> {
    for item in items {
        let indent = "  ".repeat(depth);
        let span = span(item);
        write!(out, "  {span:>width$}  {indent}{}", item.signature)?;
        if let Some(count) = item.tests.filter(|_| !tests) {
            writeln!(out, " — {count} tests")?;
            continue;
        }
        if !item.doc.is_empty() {
            write!(out, " — {}", item.doc)?;
        }
        out.push('\n');
        write_items(out, &item.children, depth + 1, width, tests)?;
    }
    Ok(())
}

/// An item's span as 1-based lines: `12-40`, or `12` for one line.
fn span(item: &Item) -> String {
    if item.start == item.end {
        format!("{}", item.start + 1)
    } else {
        format!("{}-{}", item.start + 1, item.end + 1)
    }
}

/// The 0-based lines of `text` on which `name` stands as a whole word.
fn word_lines(text: &str, name: &str) -> Vec<usize> {
    let mut lines = Vec::new();
    let (mut pos, mut line) = (0, 0);
    for (i, _) in text.match_indices(name) {
        let before = text[..i].chars().next_back();
        let after = text[i + name.len()..].chars().next();
        if before.is_some_and(is_ident) || after.is_some_and(is_ident) {
            continue;
        }
        line += text[pos..i].matches('\n').count();
        pos = i;
        if lines.last() != Some(&line) {
            lines.push(line);
        }
    }
    lines
}

/// `where [--in <crate>] <name>...`: prints each name's definitions, its row of the action map, then its uses.
pub(crate) fn find(args: &[String], root: &Path) -> Result<bool> {
    let mut names = Vec::new();
    let mut within = String::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--in" {
            let name = args.next().context("--in needs a crate name")?;
            within = crate_dir(root, name)?;
        } else if arg.is_empty() || arg.chars().any(char::is_whitespace) || arg.starts_with('-') {
            bail!("`{arg}` is not a name: a name is one word, such as act_pages_copy, CopyPages or page.copy");
        } else {
            names.push(arg.as_str());
        }
    }
    if names.is_empty() {
        bail!("where needs a name: cargo xtask where [--in <crate>] <name>...");
    }
    let mut files = Vec::new();
    for path in repo::list_files(root)? {
        if path.ends_with(".rs") && path.starts_with(&within) {
            let text = read(root, &path)?;
            if !generated(&text) {
                files.push((path, text));
            }
        }
    }
    let map = std::fs::read_to_string(root.join(ACTION_MAP)).unwrap_or_default();
    print!("{}", where_text(&files, &map, &names)?);
    Ok(true)
}

/// The folder of a workspace crate, with its trailing `/`.
fn crate_dir(root: &Path, name: &str) -> Result<String> {
    for dir in [format!("crates/{name}"), format!("bin/{name}"), name.to_owned()] {
        if root.join(&dir).join("Cargo.toml").is_file() {
            return Ok(format!("{dir}/"));
        }
    }
    bail!("no crate {name} in this workspace (a folder of crates/ or bin/, or xtask)")
}

/// What `where` prints for `names` over `files` (path and text, in path order) and the action map's text.
fn where_text(files: &[(String, String)], map: &str, names: &[&str]) -> Result<String> {
    let mut out = String::new();
    for name in names {
        writeln!(out, "{name}")?;
        let mut defs = Vec::new();
        let mut uses: Vec<(&str, Vec<usize>, bool)> = Vec::new();
        for (path, text) in files {
            if !text.contains(name) {
                continue;
            }
            let mut lines = word_lines(text, name);
            if lines.is_empty() {
                continue;
            }
            let src = Source::new(text);
            let items = src.items(0, src.len(), 0);
            let mut found = Vec::new();
            collect(&items, name, &mut found);
            lines.retain(|l| !found.iter().any(|d: &&Item| d.line == *l));
            let defined = !found.is_empty();
            for item in found {
                defs.push(format!(
                    "  {path}:{}  {}{}",
                    span(item),
                    item.signature,
                    if item.doc.is_empty() { String::new() } else { format!(" — {}", item.doc) }
                ));
            }
            if !lines.is_empty() {
                uses.push((path, lines, defined));
            }
        }
        if defs.is_empty() {
            writeln!(out, "  no definition")?;
        }
        for def in defs {
            writeln!(out, "{def}")?;
        }
        let row = format!("`{name}`");
        for line in map.lines().filter(|l| l.starts_with('|') && l.contains(&row)) {
            writeln!(out, "  action map: {line}")?;
        }
        let is_test = |p: &str| p.starts_with("tests/") || p.contains("/tests/");
        uses.sort_by_key(|(path, _, defined)| (!defined, is_test(path), *path));
        let total: usize = uses.iter().map(|(_, l, _)| l.len()).sum();
        if total == 0 {
            writeln!(out, "  no other use")?;
            continue;
        }
        writeln!(out, "  uses: {total} in {} files", uses.len())?;
        let mut shown = 0;
        for (n, (path, lines, _)) in uses.iter().enumerate() {
            if shown > 0 && shown + lines.len() > WHERE_USES {
                writeln!(out, "  … {} more in {} files", total - shown, uses.len() - n)?;
                break;
            }
            shown += lines.len();
            let numbers: Vec<String> = lines.iter().map(|l| (l + 1).to_string()).collect();
            writeln!(out, "  {path} ({}): {}", lines.len(), numbers.join(", "))?;
        }
    }
    Ok(out)
}

/// The items named `name`, at any depth.
fn collect<'a>(items: &'a [Item], name: &str, found: &mut Vec<&'a Item>) {
    for item in items {
        if item.name.as_deref() == Some(name) {
            found.push(item);
        }
        collect(&item.children, name, found);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The item lines of an outline: span and signature, the doc left out.
    fn outline_of(text: &str, tests: bool) -> String {
        let mut out = String::new();
        write_outline(&mut out, "a.rs", text, tests).unwrap();
        out
    }

    #[test]
    fn items_get_their_spans_through_the_awkward_shapes() {
        let text = r##"//! The head.
//! Second line.
//! Third line.

/// Doc of `f`. More text.
///
/// ```
/// fn not_an_item() { }
/// ```
#[inline]
#[cfg_attr(
    feature = "x",
    allow(dead_code)
)]
pub(crate) fn f<T>(
    a: T,
) -> u8
where
    T: Clone,
{
    let s = r#"} { fn fake() {"#;
    let c = '{';
    let e = '\'';
    0
}

my_macro! {
    fn inside_macro() {}
}

impl<'a, T> Trait for Wrap<'a, T>
where
    T: Copy,
{
    /// The constant.
    const N: usize = 3;

    fn method(&self) -> &'a str {
        "}"
    }
}

pub mod inner {
    pub struct S {
        a: u8,
    }

    impl S {
        pub fn get(&self) -> u8 {
            self.a
        }
    }
}

macro_rules! m {
    () => {};
}

static mut COUNT: [u8; 2] = [0; 2];
type Alias = Vec<u8>;
"##;
        let src = Source::new(text);
        let items = src.items(0, src.len(), 0);
        let line = |needle: &str| text.lines().position(|l| l.contains(needle)).unwrap();
        let by_name = |items: &[Item], name: &str| {
            let mut found = Vec::new();
            collect(items, name, &mut found);
            assert_eq!(found.len(), 1, "{name}: {items:#?}");
            (found[0].start, found[0].line, found[0].end)
        };
        assert_eq!(by_name(&items, "f"), (line("/// Doc of"), line("pub(crate) fn f"), line("    0") + 1));
        let f = items.iter().find(|i| i.name.as_deref() == Some("f")).unwrap();
        assert_eq!(f.signature, "pub(crate) fn f<T>(a: T) -> u8");
        assert_eq!(f.doc, "Doc of `f`.");
        let none = |name: &str| {
            let mut found = Vec::new();
            collect(&items, name, &mut found);
            assert!(found.is_empty(), "{name} is no item");
        };
        none("not_an_item");
        none("fake");
        none("inside_macro");
        let imp = items.iter().find(|i| i.signature.starts_with("impl")).unwrap();
        assert_eq!(imp.signature, "impl<'a, T> Trait for Wrap<'a, T>");
        assert_eq!((imp.line, imp.end), (line("impl<'a"), line("        \"}\"") + 2));
        assert_eq!(by_name(&items, "N"), (line("/// The constant"), line("const N"), line("const N")));
        assert_eq!(by_name(&items, "method").2, line("        \"}\"") + 1);
        assert_eq!(by_name(&items, "get").1, line("pub fn get"));
        assert_eq!(by_name(&items, "S").2, line("        a: u8") + 1);
        assert_eq!(by_name(&items, "inner").2, line("macro_rules!") - 2);
        assert_eq!(by_name(&items, "m"), (line("macro_rules!"), line("macro_rules!"), line("() => {};") + 1));
        assert_eq!(by_name(&items, "COUNT").1, line("static mut"));
        let count = items.iter().find(|i| i.name.as_deref() == Some("COUNT")).unwrap();
        assert_eq!(count.signature, "static mut COUNT: [u8; 2]");
        assert_eq!(src.head(), ["The head.", "Second line. …"]);
    }

    #[test]
    fn a_tests_module_is_one_line_unless_asked() {
        let text = "fn a() {}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn one() {}\n\n    #[test]\n    fn two() {}\n}\n";
        let short = outline_of(text, false);
        assert!(short.contains("3-10  mod tests — 2 tests"), "{short}");
        assert!(!short.contains("fn one"), "{short}");
        let long = outline_of(text, true);
        assert!(long.contains("fn one") && long.contains("fn two"), "{long}");
    }

    #[test]
    fn where_finds_whole_words_in_code_and_strings() {
        let files = vec![
            (
                "crates/a/src/lib.rs".to_owned(),
                "/// Copies.\npub fn copy_pages() {}\nfn other() { copy_pages(); copy_pages_more(); }\nconst A: &str = \"page.copy\";\n".to_owned(),
            ),
            ("crates/a/tests/it.rs".to_owned(), "fn t() { copy_pages() }\n".to_owned()),
            ("crates/b/src/x.rs".to_owned(), "use a::copy_pages;\nfn p() { \"pages.copy\"; }\n".to_owned()),
        ];
        let map = "| `pages.copy` | e | `copy_pages` `crates/a/src/lib.rs` | k |\n";
        let out = where_text(&files, map, &["copy_pages", "page.copy"]).unwrap();
        let expected = "copy_pages
  crates/a/src/lib.rs:1-2  pub fn copy_pages() — Copies.
  action map: | `pages.copy` | e | `copy_pages` `crates/a/src/lib.rs` | k |
  uses: 3 in 3 files
  crates/a/src/lib.rs (1): 3
  crates/b/src/x.rs (1): 1
  crates/a/tests/it.rs (1): 1
page.copy
  no definition
  uses: 1 in 1 files
  crates/a/src/lib.rs (1): 4
";
        assert_eq!(out, expected);
    }

    #[test]
    fn both_commands_read_real_files_of_the_workspace() {
        let root = crate::workspace_root();
        let repo_rs = read(&root, "xtask/src/repo.rs").unwrap();
        let mut out = String::new();
        write_outline(&mut out, "xtask/src/repo.rs", &repo_rs, false).unwrap();
        let lines: Vec<&str> = repo_rs.lines().collect();
        for name in ["git", "list_files", "nul_paths", "write_atomic"] {
            let row = out
                .lines()
                .find(|l| l.contains(&format!("fn {name}(")))
                .unwrap_or_else(|| panic!("{name} in {out}"));
            let span = row.split_whitespace().next().unwrap();
            let (a, b) = span.split_once('-').unwrap();
            let (a, b): (usize, usize) = (a.parse().unwrap(), b.parse().unwrap());
            assert!(lines[a - 1].starts_with("///"), "{name} starts at its doc");
            assert!(lines[a - 1..b].iter().any(|l| l.contains(&format!("fn {name}("))));
            assert_eq!(lines[b - 1], "}", "{name} ends at its brace");
        }
        let main_rs = read(&root, "xtask/src/main.rs").unwrap();
        let files = vec![
            ("xtask/src/main.rs".to_owned(), main_rs.clone()),
            ("xtask/src/repo.rs".to_owned(), repo_rs),
        ];
        let found = where_text(&files, "", &["list_files"]).unwrap();
        let def = found.lines().nth(1).unwrap();
        assert!(def.starts_with("  xtask/src/repo.rs:") && def.contains("pub fn list_files(root: &Path)"), "{found}");
        let main_lines = word_lines(&main_rs, "list_files");
        assert!(!main_lines.is_empty());
        let numbers: Vec<String> = main_lines.iter().map(|l| (l + 1).to_string()).collect();
        assert!(found.contains(&format!("xtask/src/main.rs ({}): {}", numbers.len(), numbers.join(", "))), "{found}");
    }

}
