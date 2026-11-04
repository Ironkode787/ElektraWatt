// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `fuzz-seeds`: the seed corpora of the loader, migration-chain and command targets (`fuzz/corpus/`; FT-1102).
//!
//! A loader seed names its files by their index in Golden A's sorted file list, so it goes stale when that
//! project gains or loses a file. `fuzz/src/bin/seed_corpus.rs` makes the seeds from the project, deterministically;
//! this runs it into a temporary folder under `target/` (stable toolchain, dev profile, from the repository root,
//! as `fuzz-build` builds `fuzz/`) and makes the generated folders (with the four importer targets' of WP-S3.09
//! D15 and the `.ewlib` reader's of WP-S6.04) equal to what it wrote. In check mode it
//! changes nothing in the corpus and names each seed that differs, is missing or is no longer made; it still builds
//! `seed_corpus` into `fuzz/target/`, the fuzz workspace's own target folder. The hand-made `png_pdf`, `ewz_read`
//! and `merge_file` seeds are not touched.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

/// The corpus folders `seed_corpus` writes, below [`CORPUS`].
const TARGETS: [&str; 8] = [
    "jsonl_loader",
    "migration_chain",
    "command_sequence",
    "qet_project",
    "qet_element",
    "golden_tables",
    "parts_table",
    "ewlib_read",
];
/// The corpus folder, relative to the workspace root.
const CORPUS: &str = "fuzz/corpus";
/// The temporary folder `seed_corpus` writes into, relative to the workspace root (removed afterwards).
const SCRATCH: &str = "target/settle-fuzz-seeds";

/// The files directly in `dir` by name; none when the folder is missing.
fn files_of(dir: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut out = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(out);
    };
    for entry in entries {
        let path = entry
            .with_context(|| format!("reading {}", dir.display()))?
            .path();
        if path.is_file() {
            let name = path
                .file_name()
                .context("a file name")?
                .to_string_lossy()
                .into_owned();
            let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            out.insert(name, bytes);
        }
    }
    Ok(out)
}

/// What differs between the seeds on disk (`have`) and the seeds just made (`made`): the names that differ, that
/// are missing and that are no longer made, as one line each.
fn differences(
    target: &str,
    made: &BTreeMap<String, Vec<u8>>,
    have: &BTreeMap<String, Vec<u8>>,
) -> Vec<String> {
    let mut out = Vec::new();
    for (name, bytes) in made {
        match have.get(name) {
            None => out.push(format!("{CORPUS}/{target}/{name} is missing")),
            Some(old) if old != bytes => out.push(format!("{CORPUS}/{target}/{name} differs")),
            Some(_) => {}
        }
    }
    for name in have.keys().filter(|n| !made.contains_key(*n)) {
        out.push(format!("{CORPUS}/{target}/{name} is no longer made"));
    }
    out
}

/// Compares the generated folders of `made_dir` with those of `corpus` and, unless `check`, makes `corpus` equal to
/// them. Returns the differences found.
fn sync(made_dir: &Path, corpus: &Path, check: bool) -> Result<Vec<String>> {
    let mut problems = Vec::new();
    for target in TARGETS {
        let made = files_of(&made_dir.join(target))?;
        let have = files_of(&corpus.join(target))?;
        problems.extend(differences(target, &made, &have));
        if check {
            continue;
        }
        let dir = corpus.join(target);
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        for (name, bytes) in &made {
            if have.get(name) != Some(bytes) {
                std::fs::write(dir.join(name), bytes).with_context(|| format!("writing {name}"))?;
            }
        }
        for name in have.keys().filter(|n| !made.contains_key(*n)) {
            std::fs::remove_file(dir.join(name)).with_context(|| format!("removing {name}"))?;
        }
    }
    Ok(problems)
}

/// `fuzz-seeds [--check]` for the repository at `root`.
pub(crate) fn run(root: &Path, check: bool) -> Result<bool> {
    let scratch = root.join(SCRATCH);
    if scratch.exists() {
        std::fs::remove_dir_all(&scratch).with_context(|| format!("removing {}", scratch.display()))?;
    }
    let output = Command::new(crate::cargo())
        .args([
            "run",
            "--manifest-path",
            "fuzz/Cargo.toml",
            "--locked",
            "--bin",
            "seed_corpus",
            "--",
        ])
        .arg(&scratch)
        .current_dir(root)
        .output()
        .context("running seed_corpus")?;
    if !output.status.success() {
        eprintln!(
            "fuzz-seeds: seed_corpus failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(false);
    }
    let result = sync(&scratch, &root.join(CORPUS), check);
    let _ = std::fs::remove_dir_all(&scratch);
    let problems = result?;
    if check && !problems.is_empty() {
        eprintln!(
            "fuzz-seeds: {} problem(s); run `cargo xtask settle`",
            problems.len()
        );
        for line in &problems {
            eprintln!("  - {line}");
        }
        return Ok(false);
    }
    println!("fuzz-seeds: {}", if check { "ok" } else { "written" });
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ew-xtask-fuzz-seeds-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn put(base: &Path, rel: &str, bytes: &[u8]) {
        let path = base.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    /// Check mode names each file that differs, is missing or is no longer made and writes nothing; write mode
    /// makes the folder equal to what was made and leaves the other targets' folders alone.
    #[test]
    fn sync_names_and_repairs_each_difference() {
        let base = dir("sync");
        let (made, corpus) = (base.join("made"), base.join("corpus"));
        put(&made, "jsonl_loader/same", b"1");
        put(&made, "jsonl_loader/changed", b"new");
        put(&made, "jsonl_loader/added", b"2");
        put(&corpus, "jsonl_loader/same", b"1");
        put(&corpus, "jsonl_loader/changed", b"old");
        put(&corpus, "jsonl_loader/gone", b"3");
        put(&corpus, "png_pdf/hand-made", b"4");
        let expected = vec![
            "fuzz/corpus/jsonl_loader/added is missing".to_owned(),
            "fuzz/corpus/jsonl_loader/changed differs".to_owned(),
            "fuzz/corpus/jsonl_loader/gone is no longer made".to_owned(),
        ];
        assert_eq!(sync(&made, &corpus, true).unwrap(), expected);
        assert_eq!(
            std::fs::read(corpus.join("jsonl_loader/changed")).unwrap(),
            b"old"
        );
        assert!(corpus.join("jsonl_loader/gone").exists());
        assert_eq!(sync(&made, &corpus, false).unwrap(), expected);
        assert_eq!(
            files_of(&corpus.join("jsonl_loader")).unwrap(),
            files_of(&made.join("jsonl_loader")).unwrap()
        );
        assert!(corpus.join("png_pdf/hand-made").exists());
        assert!(sync(&made, &corpus, true).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }
}
