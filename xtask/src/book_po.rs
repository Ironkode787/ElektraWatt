// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `book-po`: the user guide's translation catalogue, made from the pages and never kept by hand.
//!
//! The guide's pages (`docs/book/src`) are the one source. From them this step writes
//! - `docs/book/po/messages.pot`, the template: every message of the hand-written pages and the table of contents, as
//!   `mdbook-xgettext` reads them (the same reader `mdbook-gettext` uses when the Dutch book is built, so a message
//!   of the template is a message the build looks up);
//! - every `docs/book/po/<language>.po`, merged with the template: a message whose English is unchanged keeps its
//!   translation; a message whose English changed takes the nearest old translation marked `fuzzy` with the old
//!   English beside it (`#| msgid`), which the build treats as untranslated until a translator has looked; a
//!   translation whose message is gone stays at the end as an obsolete entry (`#~`), so no Dutch text is lost.
//!
//! `cargo xtask gen` writes both; `cargo xtask rules` (and so every `cargo xtask check`) fails when either is stale.
//!
//! **The tools.** mdBook and mdbook-i18n-helpers at the versions `packaging/components.toml` pins. An `mdbook` of
//! that version on the PATH is used; otherwise, and for mdbook-i18n-helpers always, the step installs the pinned
//! crate once with `cargo install --locked` under `target/tools` of the main checkout (a worktree uses the same
//! folder), which takes a few minutes the first time on a machine.
//!
//! The generated pages (the root `.gitignore`'s lines under `docs/book/src/reference/`) are left out with their
//! lines of `SUMMARY.md`, as `ci/build-book.sh` leaves them out: their text is built, not translated here.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

const BOOK: &str = "docs/book";
const PO_DIR: &str = "docs/book/po";
const TEMPLATE: &str = "docs/book/po/messages.pot";
const PINS: &str = "packaging/components.toml";
/// The line of a template's header that changes with every run and says nothing about the messages.
const DATE: &str = "\"POT-Creation-Date: ";
/// The widest line of a wrapped string, in characters between the quotes.
const WRAP: usize = 77;
/// How alike an old English text must be to the new one for its translation to be offered as fuzzy.
const ALIKE: f64 = 0.6;

/// One entry of a PO file.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
struct Entry {
    /// Comment lines other than the ones below, as they stand.
    comments: Vec<String>,
    /// `#:` references.
    refs: Vec<String>,
    /// `#,` flags.
    flags: Vec<String>,
    /// `#| msgid`: the English the translation was made for, of a fuzzy entry.
    previous: Option<String>,
    msgid: String,
    msgstr: String,
    /// `#~`: a translation whose message the template no longer has.
    obsolete: bool,
}

impl Entry {
    fn fuzzy(&self) -> bool {
        self.flags.iter().any(|f| f == "fuzzy")
    }
}

/// `book-po [--check]`: `true` when the template and every translation are up to date (after writing them, unless
/// `check`).
pub(crate) fn run(root: &Path, check: bool) -> Result<bool> {
    let fresh = template(root)?;
    let mut ok = true;
    let template_path = root.join(TEMPLATE);
    let committed = std::fs::read_to_string(&template_path).unwrap_or_default();
    if messages_of(&fresh) != messages_of(&committed.replace("\r\n", "\n")) {
        if check {
            eprintln!("book-po: {TEMPLATE} does not hold the guide's messages as its pages give them");
            ok = false;
        } else {
            crate::repo::write_atomic(&template_path, &fresh)?;
            println!("book-po: wrote {TEMPLATE}");
        }
    }
    let template = parse(&fresh).context("reading the fresh template")?;
    let mut translations: Vec<PathBuf> = std::fs::read_dir(root.join(PO_DIR))
        .with_context(|| format!("reading {PO_DIR}"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "po"))
        .collect();
    translations.sort();
    for path in translations {
        let name = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string()
            .replace('\\', "/");
        let old_text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {name}"))?
            .replace("\r\n", "\n");
        let old = parse(&old_text).with_context(|| format!("reading {name}"))?;
        let merged = merge(&template, &old);
        let text = write(&merged);
        if text != old_text {
            if check {
                eprintln!("book-po: {name} is not the template merged with its translations");
                ok = false;
            } else {
                crate::repo::write_atomic(&path, &text)?;
                println!("book-po: wrote {name}");
            }
        }
        if !check {
            let live: Vec<&Entry> = merged.iter().skip(1).filter(|e| !e.obsolete).collect();
            let fuzzy = live.iter().filter(|e| e.fuzzy()).count();
            let done = live.iter().filter(|e| !e.msgstr.is_empty() && !e.fuzzy()).count();
            println!(
                "book-po: {name}: {done} of {} messages translated, {fuzzy} to look at again (fuzzy), {} not translated",
                live.len(),
                live.len() - done - fuzzy
            );
        }
    }
    Ok(ok)
}

/// A template's text without the line that carries the date of the run.
fn messages_of(template: &str) -> String {
    template
        .lines()
        .filter(|l| !l.starts_with(DATE))
        .collect::<Vec<_>>()
        .join("\n")
}

// --------------------------------------------------------------------------------------------------------------------
// The template, by the pinned tools

/// The template of the guide as it stands in the tree: `mdbook-xgettext` over a copy of the book without its
/// generated pages. Line ends are `\n` and references use `/`, whatever the platform.
fn template(root: &Path) -> Result<String> {
    let tools = tools(root)?;
    let work = root.join("target").join("book-po");
    if work.exists() {
        std::fs::remove_dir_all(&work).with_context(|| format!("removing {}", work.display()))?;
    }
    let copy = work.join("book");
    copy_dir(&root.join(BOOK), &copy)?;
    // The generated pages and their lines of the table of contents.
    let ignore = std::fs::read_to_string(root.join(".gitignore")).context("reading .gitignore")?;
    let generated: Vec<&str> = ignore
        .lines()
        .filter_map(|l| l.trim().strip_prefix("/docs/book/src/"))
        .filter(|l| l.starts_with("reference/"))
        .collect();
    if generated.is_empty() {
        bail!(".gitignore names no generated page under /docs/book/src/reference/");
    }
    for path in &generated {
        let at = copy.join("src").join(path);
        if at.is_dir() {
            std::fs::remove_dir_all(&at).with_context(|| format!("removing {}", at.display()))?;
        } else if at.exists() {
            std::fs::remove_file(&at).with_context(|| format!("removing {}", at.display()))?;
        }
    }
    let summary_path = copy.join("src").join("SUMMARY.md");
    let summary = std::fs::read_to_string(&summary_path).context("reading SUMMARY.md")?;
    let kept: String = summary
        .lines()
        .filter(|l| !generated.iter().any(|g| l.contains(&format!("]({g}"))))
        .map(|l| format!("{l}\n"))
        .collect();
    std::fs::write(&summary_path, kept).context("writing the copy's SUMMARY.md")?;
    let out = work.join("out");
    let renderer = format!(
        "{{\"xgettext\": {{\"command\": \"{}\"}}}}",
        tools.xgettext.display().to_string().replace('\\', "/")
    );
    let status = Command::new(&tools.mdbook)
        .arg("build")
        .arg(&copy)
        .arg("--dest-dir")
        .arg(&out)
        .env("MDBOOK_OUTPUT", renderer)
        .output()
        .with_context(|| format!("running {}", tools.mdbook.display()))?;
    if !status.status.success() {
        bail!(
            "mdbook build with mdbook-xgettext failed:\n{}",
            String::from_utf8_lossy(&status.stderr)
        );
    }
    let made = out.join("messages.pot");
    let text = std::fs::read_to_string(&made)
        .with_context(|| format!("mdbook-xgettext wrote no {}", made.display()))?
        .replace("\r\n", "\n");
    // References with the platform's separator would differ between Windows and Linux.
    Ok(text
        .lines()
        .map(|l| {
            if l.starts_with("#:") {
                l.replace('\\', "/")
            } else {
                l.to_owned()
            }
        })
        .map(|l| l + "\n")
        .collect())
}

/// The executables of the template's making.
struct Tools {
    mdbook: PathBuf,
    xgettext: PathBuf,
}

/// The pinned version of the component `name` of `packaging/components.toml`.
fn pin(root: &Path, name: &str) -> Result<String> {
    let text = std::fs::read_to_string(root.join(PINS)).with_context(|| format!("reading {PINS}"))?;
    let table: toml::Table = text.parse().with_context(|| format!("parsing {PINS}"))?;
    table
        .get("component")
        .and_then(|c| c.as_array())
        .into_iter()
        .flatten()
        .find(|c| c.get("name").and_then(|n| n.as_str()) == Some(name))
        .and_then(|c| c.get("version").and_then(|v| v.as_str()))
        .map(str::to_owned)
        .with_context(|| format!("{PINS} pins no component `{name}`"))
}

/// The tools at their pinned versions, installed under the main checkout's `target/tools` where they are not there.
fn tools(root: &Path) -> Result<Tools> {
    let dir = tools_dir(root)?;
    let exe = |name: &str| {
        dir.join("bin")
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
    };
    let mdbook_pin = pin(root, "mdbook")?;
    let wanted = format!("mdbook v{mdbook_pin}");
    let prints = |program: &Path| {
        Command::new(program)
            .arg("--version")
            .output()
            .ok()
            .is_some_and(|o| String::from_utf8_lossy(&o.stdout).trim() == wanted)
    };
    let mdbook = if prints(Path::new("mdbook")) {
        PathBuf::from("mdbook")
    } else {
        if !prints(&exe("mdbook")) {
            install(root, &dir, "mdbook", &mdbook_pin)?;
        }
        exe("mdbook")
    };
    let helpers_pin = pin(root, "mdbook-i18n-helpers")?;
    let installed = std::fs::read_to_string(dir.join(".crates.toml")).unwrap_or_default();
    if !exe("mdbook-xgettext").exists() || !installed.contains(&format!("mdbook-i18n-helpers {helpers_pin} "))
    {
        install(root, &dir, "mdbook-i18n-helpers", &helpers_pin)?;
    }
    Ok(Tools {
        mdbook,
        xgettext: exe("mdbook-xgettext"),
    })
}

/// `target/tools` of the main checkout: a worktree shares it, so the tools are built once per machine.
fn tools_dir(root: &Path) -> Result<PathBuf> {
    let common = crate::repo::git(root, &["rev-parse", "--path-format=absolute", "--git-common-dir"])?;
    let main = Path::new(common.trim())
        .parent()
        .context("the git directory has no parent")?
        .to_path_buf();
    Ok(main.join("target").join("tools"))
}

fn install(root: &Path, dir: &Path, name: &str, version: &str) -> Result<()> {
    println!(
        "book-po: installing {name} {version} under {} (once per machine)",
        dir.display()
    );
    let status = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .current_dir(root)
        .args(["install", "--locked", "--root"])
        .arg(dir)
        .arg(name)
        .arg("--version")
        .arg(format!("={version}"))
        .status()
        .context("running cargo install")?;
    if !status.success() {
        bail!("cargo install of {name} {version} failed");
    }
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to).with_context(|| format!("creating {}", to.display()))?;
    for entry in std::fs::read_dir(from).with_context(|| format!("reading {}", from.display()))? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)
                .with_context(|| format!("copying {}", entry.path().display()))?;
        }
    }
    Ok(())
}

// --------------------------------------------------------------------------------------------------------------------
// PO files: read, merge, write

/// The text between the quotes of a PO string line, unescaped.
fn unquote(line: &str) -> Result<String> {
    let inner = line
        .trim()
        .strip_prefix('"')
        .and_then(|l| l.strip_suffix('"'))
        .with_context(|| format!("not a quoted string: {line}"))?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(other) => out.push(other),
            None => bail!("a string ends in a backslash: {line}"),
        }
    }
    Ok(out)
}

/// The entries of a PO file, the header first.
fn parse(text: &str) -> Result<Vec<Entry>> {
    #[derive(Clone, Copy, PartialEq)]
    enum Field {
        None,
        Previous,
        Id,
        Str,
    }
    let mut entries = Vec::new();
    let mut entry = Entry::default();
    let mut field = Field::None;
    let mut seen = false;
    let mut flush = |entry: &mut Entry, seen: &mut bool, field: &mut Field| {
        if *seen {
            entries.push(std::mem::take(entry));
        }
        *entry = Entry::default();
        *seen = false;
        *field = Field::None;
    };
    for raw in text.lines() {
        let line = raw.trim_end();
        if line.is_empty() {
            flush(&mut entry, &mut seen, &mut field);
            continue;
        }
        seen = true;
        let (line, obsolete) = match line.strip_prefix("#~") {
            Some(rest) => (rest.trim_start(), true),
            None => (line, false),
        };
        entry.obsolete |= obsolete;
        if let Some(rest) = line.strip_prefix("#:") {
            entry.refs.extend(rest.split_whitespace().map(str::to_owned));
        } else if let Some(rest) = line.strip_prefix("#,") {
            entry.flags.extend(
                rest.split(',')
                    .map(|f| f.trim().to_owned())
                    .filter(|f| !f.is_empty()),
            );
        } else if let Some(rest) = line.strip_prefix("#|") {
            let rest = rest.trim_start();
            if let Some(s) = rest.strip_prefix("msgid ") {
                entry.previous = Some(unquote(s)?);
                field = Field::Previous;
            } else if rest.starts_with('"') {
                entry
                    .previous
                    .get_or_insert_with(String::new)
                    .push_str(&unquote(rest)?);
            }
        } else if line.starts_with('#') {
            entry.comments.push(line.to_owned());
        } else if let Some(s) = line.strip_prefix("msgid ") {
            entry.msgid = unquote(s)?;
            field = Field::Id;
        } else if let Some(s) = line.strip_prefix("msgstr ") {
            entry.msgstr = unquote(s)?;
            field = Field::Str;
        } else if line.starts_with('"') {
            let s = unquote(line)?;
            match field {
                Field::Id => entry.msgid.push_str(&s),
                Field::Str => entry.msgstr.push_str(&s),
                Field::Previous => entry.previous.get_or_insert_with(String::new).push_str(&s),
                Field::None => bail!("a string outside an entry: {raw}"),
            }
        } else {
            // msgctxt and plural forms: the guide's catalogue has none, and merging them is not written.
            bail!("a line this step does not read: {raw}");
        }
    }
    flush(&mut entry, &mut seen, &mut field);
    if entries.first().is_none_or(|e| !e.msgid.is_empty()) {
        bail!("the file does not start with its header entry");
    }
    Ok(entries)
}

/// The template merged with the translations of `old` (see the module's text).
fn merge(template: &[Entry], old: &[Entry]) -> Vec<Entry> {
    let wanted: BTreeSet<&str> = template.iter().skip(1).map(|e| e.msgid.as_str()).collect();
    let by_id: BTreeMap<&str, &Entry> = old
        .iter()
        .skip(1)
        .filter(|e| !e.obsolete)
        .map(|e| (e.msgid.as_str(), e))
        .collect();
    // Translations without a message of their own: the ones to offer as fuzzy, then to keep as obsolete.
    let mut spare: Vec<&Entry> = old
        .iter()
        .skip(1)
        .filter(|e| !e.msgstr.is_empty() && (e.obsolete || !wanted.contains(e.msgid.as_str())))
        .collect();
    // The header is the translation's own.
    let mut out = vec![old[0].clone()];
    for t in template.iter().skip(1) {
        let mut entry = Entry {
            refs: t.refs.clone(),
            msgid: t.msgid.clone(),
            ..Entry::default()
        };
        if let Some(e) = by_id.get(t.msgid.as_str()) {
            entry.comments.clone_from(&e.comments);
            entry.msgstr.clone_from(&e.msgstr);
            if e.fuzzy() && !e.msgstr.is_empty() {
                entry.flags.push("fuzzy".to_owned());
                entry.previous.clone_from(&e.previous);
            }
        } else {
            let best = spare
                .iter()
                .enumerate()
                .map(|(i, e)| (alike(&e.msgid, &t.msgid), i))
                .filter(|(score, _)| *score >= ALIKE)
                .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)));
            if let Some((_, i)) = best {
                let e = spare.remove(i);
                entry.msgstr.clone_from(&e.msgstr);
                entry.flags.push("fuzzy".to_owned());
                entry.previous = Some(e.msgid.clone());
            }
        }
        out.push(entry);
    }
    for e in spare {
        out.push(Entry {
            comments: e.comments.clone(),
            msgid: e.msgid.clone(),
            msgstr: e.msgstr.clone(),
            obsolete: true,
            ..Entry::default()
        });
    }
    out
}

/// How alike two texts are, from 0 to 1: twice the longest common run of words in order over the words of both.
fn alike(a: &str, b: &str) -> f64 {
    let (a, b): (Vec<&str>, Vec<&str>) = (a.split_whitespace().collect(), b.split_whitespace().collect());
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let mut row = vec![0usize; b.len() + 1];
    for x in &a {
        let mut diagonal = 0;
        for (j, y) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = if x == y { diagonal + 1 } else { above.max(row[j]) };
            diagonal = above;
        }
    }
    2.0 * row[b.len()] as f64 / (a.len() + b.len()) as f64
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

/// `key "text"` on one line where it fits and holds no line break before its end; else `key ""` and the text on the
/// lines after it, a line per line of the text, each wrapped after a space at [`WRAP`] characters.
fn string(prefix: &str, key: &str, text: &str, out: &mut String) {
    let escaped = escape(text);
    let inner_break = text.trim_end_matches('\n').contains('\n');
    if !inner_break && key.chars().count() + escaped.chars().count() + 3 <= WRAP + 2 {
        out.push_str(&format!("{prefix}{key} \"{escaped}\"\n"));
        return;
    }
    out.push_str(&format!("{prefix}{key} \"\"\n"));
    for segment in text.split_inclusive('\n') {
        let mut line = String::new();
        let mut length = 0;
        // A word with the spaces after it, so that a line ends in its space.
        let mut rest = segment;
        while !rest.is_empty() {
            let word_end = rest.find(' ').map_or(rest.len(), |i| {
                i + rest[i..].chars().take_while(|c| *c == ' ').count()
            });
            let (word, tail) = rest.split_at(word_end);
            rest = tail;
            let piece = escape(word);
            let piece_length = piece.chars().count();
            if length > 0 && length + piece_length > WRAP {
                out.push_str(&format!("{prefix}\"{line}\"\n"));
                line.clear();
                length = 0;
            }
            line.push_str(&piece);
            length += piece_length;
        }
        if !line.is_empty() {
            out.push_str(&format!("{prefix}\"{line}\"\n"));
        }
    }
}

/// The PO text of `entries`, in the one form this step writes.
fn write(entries: &[Entry]) -> String {
    let mut out = String::new();
    for (i, e) in entries.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        for c in &e.comments {
            out.push_str(c);
            out.push('\n');
        }
        if !e.refs.is_empty() {
            out.push_str(&format!("#: {}\n", e.refs.join(" ")));
        }
        if !e.flags.is_empty() {
            out.push_str(&format!("#, {}\n", e.flags.join(", ")));
        }
        let prefix = if e.obsolete { "#~ " } else { "" };
        if let Some(previous) = &e.previous {
            string("#| ", "msgid", previous, &mut out);
        }
        string(prefix, "msgid", &e.msgid, &mut out);
        string(prefix, "msgstr", &e.msgstr, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str = "msgid \"\"\nmsgstr \"\"\n\"Language: en\\n\"\n\n#: src/a.md:1\nmsgid \"Kept as it was.\"\nmsgstr \"\"\n\n#: src/a.md:3\nmsgid \"\"\n\"The folio looks as it did in QElectroTech: the frame, the labels and the \"\n\"three conductors.\"\nmsgstr \"\"\n\n#: src/a.md:5\nmsgid \"A sentence nobody has translated.\"\nmsgstr \"\"\n";
    const OLD: &str = "msgid \"\"\nmsgstr \"\"\n\"Language: nl\\n\"\n\n#: src/a.md:9\nmsgid \"Kept as it was.\"\nmsgstr \"Gebleven zoals het was.\"\n\n#: src/a.md:3\nmsgid \"The folio looks as it did in QElectroTech: the three conductors.\"\nmsgstr \"Het folio ziet eruit zoals in QElectroTech: de drie geleiders.\"\n\n#: src/b.md:1\nmsgid \"A page that is gone, with \\\"quotes\\\".\"\nmsgstr \"Een pagina die weg is, met \\\"aanhalingstekens\\\".\"\n";

    /// A translation of unchanged English stays, with the template's reference; changed English takes its old
    /// translation as fuzzy with the old English beside it; a translation without a message is kept as obsolete;
    /// and merging the result again changes nothing.
    #[test]
    fn a_merge_keeps_offers_and_retires_translations_and_is_stable() {
        let template = parse(TEMPLATE).unwrap();
        let merged = merge(&template, &parse(OLD).unwrap());
        assert_eq!(
            merged[0].msgstr, "Language: nl\n",
            "the translation keeps its own header"
        );
        assert_eq!(merged[1].msgstr, "Gebleven zoals het was.");
        assert_eq!(merged[1].refs, ["src/a.md:1"]);
        assert!(merged[2].fuzzy(), "{:?}", merged[2]);
        assert_eq!(
            merged[2].msgstr,
            "Het folio ziet eruit zoals in QElectroTech: de drie geleiders."
        );
        assert_eq!(
            merged[2].previous.as_deref(),
            Some("The folio looks as it did in QElectroTech: the three conductors.")
        );
        assert!(merged[3].msgstr.is_empty() && !merged[3].fuzzy());
        let last = merged.last().unwrap();
        assert!(last.obsolete && last.msgid.starts_with("A page that is gone"));
        let text = write(&merged);
        assert!(
            text.contains("#~ msgid \"A page that is gone, with \\\"quotes\\\".\"\n"),
            "{text}"
        );
        assert!(
            text.contains(
                "#, fuzzy\n#| msgid \"The folio looks as it did in QElectroTech: the three conductors.\"\n"
            ),
            "{text}"
        );
        let again = parse(&text).unwrap();
        assert_eq!(again, merged, "what is written reads back the same");
        assert_eq!(
            write(&merge(&template, &again)),
            text,
            "a second merge changes nothing"
        );
    }

    /// A long text is wrapped after a space, a text of several lines is written a line per line, and both read
    /// back as they were.
    #[test]
    fn strings_wrap_and_read_back() {
        let long = "word ".repeat(40);
        let lines = "first line\nsecond \"quoted\" line\n";
        for text in [long.as_str(), lines, "", "short"] {
            let mut out = String::new();
            string("", "msgid", text, &mut out);
            string("", "msgstr", "", &mut out);
            assert!(out.lines().all(|l| l.chars().count() <= WRAP + 2), "{out}");
            let read = parse(&format!("msgid \"\"\nmsgstr \"\"\n\n{out}")).unwrap();
            assert_eq!(read[1].msgid, text);
        }
    }

    #[test]
    fn alike_is_one_for_equal_texts_and_low_for_strangers() {
        assert!((alike("a b c", "a b c") - 1.0).abs() < 1e-9);
        assert!(alike("the lamp has its part", "choose a file to import") < ALIKE);
        assert!(
            alike(
                "Choose Apply in the preview.",
                "Choose Create project in the card."
            ) < 1.0
        );
    }
}
