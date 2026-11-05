// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-unsafe`: the `unsafe` allowlist of doc 18 §11.2 and NFR-SEC-04, as WP-S3.07 ruling 6 of ID-1218 reads it.
//!
//! A workspace package complies when its `[lints]` table is `workspace = true` and the workspace forbids
//! `unsafe_code` (the workspace's lint counts as the crate's `forbid`), or when each of its crate roots (every target's
//! root file) carries `#![forbid(unsafe_code)]`. Any other package must be listed in [`ALLOWLIST`]. The `unsafe`
//! keyword (a block, fn, impl, trait or extern block) may stand only in a file the allowlist lists for its package,
//! and there exactly as many times as the allowlist gives for that file (FT-1151: an added block fails until the
//! allowlist counts it); comments and strings do not count (the source is lexed). `fuzz/` is its own workspace, outside `cargo metadata`:
//! every `.rs` file under it is lexed too, and the keyword fails there unless the allowlist lists the file for the
//! fuzz crate `ew-fuzz`, relative to `fuzz/` (WP-S6.04 D7; no crate root's `forbid` is required there, since
//! `libfuzzer-sys`'s macro expands its own exported symbol).
//!
//! The `SAFETY:` comment of each block is clippy's `undocumented_unsafe_blocks` (an error under `-D warnings`), which
//! this check does not repeat; the second maintainer's review is the cycle reviewer's (ID-1030).

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::raw_read::{Parsed, Tk, collect_rs};

/// The allowlist, relative to the workspace root.
pub const ALLOWLIST: &str = "ci/unsafe-allowlist.toml";

/// The allowlist file: per crate, the files (relative to the crate's directory) where `unsafe` may stand, each with
/// the number of its `unsafe` keywords.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Allowlist {
    crates: BTreeMap<String, Files>,
}

/// A crate's files where `unsafe` stands and how many times in each.
type Files = BTreeMap<String, usize>;

/// The violation of a listed file whose `unsafe` keywords are not the allowlist's count; `None` when they are.
fn miscount(path: &str, text: &str, allowed: usize, package: &str) -> Option<String> {
    let found = unsafe_count(text);
    (found != allowed).then(|| {
        format!(
            "{path}: `unsafe` stands {found} times, and {ALLOWLIST} gives {allowed} for `{package}` (doc 18 §11.2, \
             NFR-SEC-04)"
        )
    })
}

/// One workspace package as the check reads it.
struct Package {
    name: String,
    /// The package's directory, relative to the workspace root and `/`-separated (empty for the root).
    dir: String,
    /// Whether its manifest has `[lints] workspace = true`.
    workspace_lints: bool,
    /// Its targets' root files, relative to the workspace root.
    roots: Vec<String>,
    /// Its `.rs` files, relative to the workspace root, with their text.
    files: Vec<(String, String)>,
}

/// The violations of the workspace at `root`; empty when it complies.
pub fn violations(root: &Path) -> Result<Vec<String>> {
    let path = root.join(ALLOWLIST);
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let allowlist: Allowlist =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).context("reading the root Cargo.toml")?;
    let packages = packages(root)?;
    let mut v = check(&manifest, &allowlist.crates, &packages);
    v.extend(fuzz_violations(root, &allowlist.crates)?);
    Ok(v)
}

/// The fuzz crate's package name, its key in the allowlist.
const FUZZ_CRATE: &str = "ew-fuzz";

/// The `unsafe` keywords of the `.rs` files under `<root>/fuzz` (not `target`, not hidden directories) that stand
/// outside a file the allowlist lists for [`FUZZ_CRATE`] (WP-S6.04 D7, S5); none without a `fuzz/` folder.
fn fuzz_violations(root: &Path, allowed: &BTreeMap<String, Files>) -> Result<Vec<String>> {
    let dir = root.join("fuzz");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    collect_rs(&dir, root, true, &mut files)?;
    let listed = allowed.get(FUZZ_CRATE);
    let mut v = Vec::new();
    for (path, text) in &files {
        if let Some(count) = listed.and_then(|files| files.iter().find(|(f, _)| format!("fuzz/{f}") == *path)) {
            v.extend(miscount(path, text, *count.1, FUZZ_CRATE));
            continue;
        }
        for line in unsafe_lines(text) {
            v.push(format!(
                "{path}:{line}: `unsafe` in the fuzz crate, which {ALLOWLIST} does not list (doc 18 §11.2, NFR-SEC-04)"
            ));
        }
    }
    Ok(v)
}

/// The workspace's packages at `root` (`cargo metadata --no-deps`), with their crate roots and `.rs` files.
fn packages(root: &Path) -> Result<Vec<Package>> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .cargo_path(crate::cargo())
        .manifest_path(root.join("Cargo.toml"))
        .no_deps()
        .exec()
        .context("running `cargo metadata --no-deps`")?;
    let ws = metadata.workspace_root.as_std_path();
    let relative = |p: &Path| -> Result<String> {
        Ok(p.strip_prefix(ws)
            .with_context(|| format!("{} is outside the workspace", p.display()))?
            .to_string_lossy()
            .replace('\\', "/"))
    };
    let mut out = Vec::new();
    for meta in metadata.workspace_packages() {
        let dir = meta
            .manifest_path
            .parent()
            .context("a manifest path has a parent directory")?
            .as_std_path();
        let manifest: toml::Table = std::fs::read_to_string(&meta.manifest_path)
            .with_context(|| format!("reading {}", meta.manifest_path))?
            .parse()
            .with_context(|| format!("parsing {}", meta.manifest_path))?;
        let workspace_lints = manifest
            .get("lints")
            .and_then(|l| l.get("workspace"))
            .and_then(toml::Value::as_bool)
            == Some(true);
        let mut roots = meta
            .targets
            .iter()
            .map(|t| relative(t.src_path.as_std_path()))
            .collect::<Result<Vec<String>>>()?;
        roots.sort();
        roots.dedup();
        let mut files = Vec::new();
        collect_rs(dir, ws, true, &mut files)?;
        out.push(Package {
            name: meta.name.to_string(),
            dir: relative(dir)?,
            workspace_lints,
            roots,
            files,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Whether the workspace manifest sets `unsafe_code` to `forbid` in `[workspace.lints.rust]`.
fn workspace_forbids(manifest: &str) -> bool {
    let Ok(doc) = manifest.parse::<toml::Table>() else {
        return false;
    };
    let level = doc
        .get("workspace")
        .and_then(|w| w.get("lints"))
        .and_then(|l| l.get("rust"))
        .and_then(|r| r.get("unsafe_code"));
    match level {
        Some(toml::Value::String(s)) => s == "forbid",
        Some(toml::Value::Table(t)) => t.get("level").and_then(toml::Value::as_str) == Some("forbid"),
        _ => false,
    }
}

/// Whether `text` carries the inner attribute `#![forbid(unsafe_code)]` (among other lints or alone).
fn forbids_unsafe(text: &str) -> bool {
    let p = Parsed::new(text);
    (0..p.toks.len()).any(|i| {
        p.attr_at(i).is_some_and(|a| {
            a.inner
                && p.is_ident(a.open + 1, "forbid")
                && (a.open + 2..a.close).any(|j| p.is_ident(j, "unsafe_code"))
        })
    })
}

/// The lines of `text` where the `unsafe` keyword stands, outside comments and strings.
fn unsafe_lines(text: &str) -> Vec<usize> {
    let p = Parsed::new(text);
    let mut lines: Vec<usize> = p
        .toks
        .iter()
        .filter(|t| matches!(&t.kind, Tk::Ident(word) if word == "unsafe"))
        .map(|t| t.line)
        .collect();
    lines.dedup();
    lines
}

/// The number of `unsafe` keywords in `text`, outside comments and strings.
fn unsafe_count(text: &str) -> usize {
    Parsed::new(text)
        .toks
        .iter()
        .filter(|t| matches!(&t.kind, Tk::Ident(word) if word == "unsafe"))
        .count()
}

/// The check over `packages`, with the workspace manifest's text and the allowlist's entries (crate name to the files
/// relative to the crate's directory).
fn check(manifest: &str, allowed: &BTreeMap<String, Files>, packages: &[Package]) -> Vec<String> {
    let inherited = workspace_forbids(manifest);
    let mut v = Vec::new();
    for package in packages {
        let listed = allowed.get(&package.name);
        if listed.is_none() && !(inherited && package.workspace_lints) {
            for root in &package.roots {
                let forbids = package
                    .files
                    .iter()
                    .find(|(path, _)| path == root)
                    .is_some_and(|(_, text)| forbids_unsafe(text));
                if !forbids {
                    v.push(format!(
                        "{root}: this crate root of `{}` does not carry `#![forbid(unsafe_code)]`, and the crate \
                         neither takes the workspace's lints (`[lints] workspace = true`) nor stands in {ALLOWLIST} \
                         (doc 18 §11.2, NFR-SEC-04)",
                        package.name
                    ));
                }
            }
        }
        let prefix = if package.dir.is_empty() {
            String::new()
        } else {
            format!("{}/", package.dir)
        };
        for (path, text) in &package.files {
            if let Some(count) = listed.and_then(|files| files.iter().find(|(f, _)| format!("{prefix}{f}") == *path)) {
                v.extend(miscount(path, text, *count.1, &package.name));
                continue;
            }
            for line in unsafe_lines(text) {
                v.push(format!(
                    "{path}:{line}: `unsafe` in a file that {ALLOWLIST} does not list for `{}` (doc 18 §11.2, \
                     NFR-SEC-04)",
                    package.name
                ));
            }
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory under the system temporary folder, removed by the caller.
    fn fresh(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ew-xtask-unsafe-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// A workspace at `dir` whose lints forbid `unsafe_code`, with the allowlist `allowlist` and two crates: `good`,
    /// which takes the workspace's lints, and `own`, whose root forbids `unsafe_code` itself (in a comment and a
    /// string `unsafe` does not count).
    fn workspace(dir: &Path, allowlist: &str) {
        write(
            dir,
            "Cargo.toml",
            "[workspace]\nresolver = \"2\"\nmembers = [\"crates/*\"]\n\n[workspace.lints.rust]\nunsafe_code = \
             \"forbid\"\n",
        );
        write(dir, ALLOWLIST, allowlist);
        write(
            dir,
            "crates/good/Cargo.toml",
            "[package]\nname = \"good\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[lints]\nworkspace = true\n",
        );
        write(
            dir,
            "crates/good/src/lib.rs",
            "//! No `unsafe` here: a comment.\npub const WORD: &str = \"unsafe\";\n",
        );
        write(
            dir,
            "crates/own/Cargo.toml",
            "[package]\nname = \"own\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
        );
        write(
            dir,
            "crates/own/src/lib.rs",
            "#![forbid(missing_docs, unsafe_code)]\n//! Own.\n",
        );
    }

    #[test]
    fn the_repository_passes() {
        assert_eq!(
            violations(&crate::workspace_root()).unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_workspace_of_complying_crates_and_a_listed_file_passes() {
        let dir = fresh("pass");
        workspace(&dir, "[crates]\nffi = { \"src/sys.rs\" = 1 }\n");
        write(
            &dir,
            "crates/ffi/Cargo.toml",
            "[package]\nname = \"ffi\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
        );
        write(&dir, "crates/ffi/src/lib.rs", "#![deny(unsafe_code)]\nmod sys;\n");
        write(
            &dir,
            "crates/ffi/src/sys.rs",
            "#![allow(unsafe_code)]\n// SAFETY: nothing.\npub fn f() { unsafe {} }\n",
        );
        let v = violations(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(v.unwrap(), Vec::<String>::new());
    }

    /// WN-1: `unsafe` in a file the allowlist does not list fails with the file, its line and the crate.
    #[test]
    fn unsafe_in_an_unlisted_file_fails() {
        let dir = fresh("unlisted");
        workspace(&dir, "[crates]\nffi = { \"src/sys.rs\" = 1 }\n");
        write(
            &dir,
            "crates/ffi/Cargo.toml",
            "[package]\nname = \"ffi\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
        );
        write(&dir, "crates/ffi/src/lib.rs", "mod sys;\nmod other;\n");
        write(&dir, "crates/ffi/src/sys.rs", "pub fn f() { unsafe {} }\n");
        write(
            &dir,
            "crates/ffi/src/other.rs",
            "/// A comment with unsafe.\npub unsafe fn g() {}\n",
        );
        write(
            &dir,
            "crates/good/src/sneaky.rs",
            "\n\nunsafe impl Send for X {}\n",
        );
        let v = violations(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            v.unwrap(),
            [
                "crates/ffi/src/other.rs:2: `unsafe` in a file that ci/unsafe-allowlist.toml does not list for `ffi` \
                 (doc 18 §11.2, NFR-SEC-04)",
                "crates/good/src/sneaky.rs:3: `unsafe` in a file that ci/unsafe-allowlist.toml does not list for \
                 `good` (doc 18 §11.2, NFR-SEC-04)",
            ]
        );
    }

    /// FT-1151: a listed file counts its `unsafe` keywords, so a block added beyond the allowlist's count fails, as
    /// does a count the file no longer reaches; comments and strings do not count.
    #[test]
    fn a_listed_file_holds_exactly_its_counted_unsafe() {
        let dir = fresh("count");
        workspace(&dir, "[crates]\nffi = { \"src/sys.rs\" = 2 }\n");
        write(
            &dir,
            "crates/ffi/Cargo.toml",
            "[package]\nname = \"ffi\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
        );
        write(&dir, "crates/ffi/src/lib.rs", "#![deny(unsafe_code)]\nmod sys;\n");
        let sys = |blocks: usize| {
            format!(
                "#![allow(unsafe_code)]\n// unsafe in a comment\npub fn f() {{ {} }}\n",
                "unsafe {} ".repeat(blocks)
            )
        };
        let found = |n: usize| {
            vec![format!(
                "crates/ffi/src/sys.rs: `unsafe` stands {n} times, and ci/unsafe-allowlist.toml gives 2 for `ffi` (doc \
                 18 §11.2, NFR-SEC-04)"
            )]
        };
        write(&dir, "crates/ffi/src/sys.rs", &sys(2));
        assert_eq!(violations(&dir).unwrap(), Vec::<String>::new());
        write(&dir, "crates/ffi/src/sys.rs", &sys(3));
        assert_eq!(violations(&dir).unwrap(), found(3));
        write(&dir, "crates/ffi/src/sys.rs", &sys(1));
        let v = violations(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(v.unwrap(), found(1));
    }

    /// SE-4 (WP-S6.04 D7, S5): `unsafe` in a file of the fuzz folder fails with the file and its line, unless the
    /// allowlist lists the file for `ew-fuzz`; comments, strings and `target/` do not count.
    #[test]
    fn unsafe_in_the_fuzz_crate_fails_unless_listed() {
        let dir = fresh("fuzz");
        workspace(&dir, "[crates]\n");
        write(
            &dir,
            "fuzz/fuzz_targets/t.rs",
            "#![no_main]\n// unsafe in a comment\nfuzz_target!(|d: &[u8]| { let _ = \"unsafe\"; });\n",
        );
        write(&dir, "fuzz/src/alloc.rs", "\nunsafe impl Sync for X {}\n");
        write(&dir, "fuzz/target/debug/build/out.rs", "unsafe fn f() {}\n");
        let v = violations(&dir);
        assert_eq!(
            v.unwrap(),
            [
                "fuzz/src/alloc.rs:2: `unsafe` in the fuzz crate, which ci/unsafe-allowlist.toml does not list (doc 18 \
              §11.2, NFR-SEC-04)"
            ]
        );
        write(&dir, ALLOWLIST, "[crates]\new-fuzz = { \"src/alloc.rs\" = 1 }\n");
        let v = violations(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(v.unwrap(), Vec::<String>::new());
    }

    /// WN-1: a crate with neither `forbid(unsafe_code)` in its root nor the workspace's lints, and no entry, fails
    /// once per crate root; the workspace's lints count only while the workspace forbids `unsafe_code`.
    #[test]
    fn a_crate_without_forbid_or_the_workspace_lints_fails_unless_listed() {
        let dir = fresh("unforbidden");
        workspace(&dir, "[crates]\n");
        write(
            &dir,
            "crates/loose/Cargo.toml",
            "[package]\nname = \"loose\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[lints.rust]\nunsafe_code = \
             \"deny\"\n",
        );
        write(&dir, "crates/loose/src/lib.rs", "#![deny(unsafe_code)]\n");
        write(
            &dir,
            "crates/loose/src/main.rs",
            "#![forbid(unsafe_code)]\nfn main() {}\n",
        );
        let v = violations(&dir);
        let refusal = |root: &str, name: &str| {
            format!(
                "{root}: this crate root of `{name}` does not carry `#![forbid(unsafe_code)]`, and the crate neither \
                 takes the workspace's lints (`[lints] workspace = true`) nor stands in ci/unsafe-allowlist.toml (doc \
                 18 §11.2, NFR-SEC-04)"
            )
        };
        assert_eq!(v.unwrap(), [refusal("crates/loose/src/lib.rs", "loose")]);
        // Listed, it passes; a workspace that does not forbid `unsafe_code` covers no crate.
        write(&dir, ALLOWLIST, "[crates]\nloose = {}\n");
        assert_eq!(violations(&dir).unwrap(), Vec::<String>::new());
        write(
            &dir,
            "Cargo.toml",
            "[workspace]\nresolver = \"2\"\nmembers = [\"crates/*\"]\n\n[workspace.lints.rust]\nunsafe_code = \"deny\"\n",
        );
        let v = violations(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(v.unwrap(), [refusal("crates/good/src/lib.rs", "good")]);
    }
}
