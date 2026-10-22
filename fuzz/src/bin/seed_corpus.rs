// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Writes the seed corpora of the three targets into `fuzz/corpus/<target>/` (FT-297): small inputs made from
//! the golden project (no private data: the golden project is anonymised), deterministic, so a rerun gives the
//! same files. Run it from the `fuzz/` directory: `cargo run --release --bin seed_corpus`; an argument names
//! another output directory.

use std::path::{Path, PathBuf};

use ew_fuzz::golden;
use ew_fuzz::loader::{self, MODE_COMPUTED_MANIFEST, MODE_TINY_LIMITS};
use ew_fuzz::open_path::{self, MODE_READ_ONLY, MODE_STALE_MANIFEST, OP_KINDS, Op};

/// The golden file of that name; the seeds name files by path.
fn file(path: &str) -> &'static [u8] {
    golden::files()
        .get(path)
        .unwrap_or_else(|| panic!("the golden project has no file {path}"))
}

/// The smallest golden file whose path starts with `prefix`.
fn smallest(prefix: &str) -> &'static str {
    golden::paths()
        .iter()
        .filter(|p| p.starts_with(prefix))
        .min_by_key(|p| file(p).len())
        .unwrap_or_else(|| panic!("the golden project has no file below {prefix}"))
}

fn write(dir: &Path, name: &str, bytes: &[u8]) {
    std::fs::create_dir_all(dir).expect("the corpus directory is created");
    std::fs::write(dir.join(name), bytes).unwrap_or_else(|e| panic!("writing {name}: {e}"));
}

fn loader_seeds(dir: &Path) {
    let computed = MODE_COMPUTED_MANIFEST;
    let roots = [
        "project.jsonl",
        "structure.jsonl",
        "revisions.jsonl",
        "report-blocks.jsonl",
    ];
    let pick = |paths: &[&str]| -> Vec<(String, &'static [u8])> {
        paths.iter().map(|p| ((*p).to_owned(), file(p))).collect()
    };
    let with = |mode: u8, paths: &[&str], manifest: Option<&[u8]>| -> Vec<u8> {
        let files = pick(paths);
        let refs: Vec<(&str, &[u8])> = files.iter().map(|(p, b)| (p.as_str(), *b)).collect();
        loader::encode(mode, &refs, manifest)
    };
    write(dir, "root-files", &with(computed, &roots, None));
    write(dir, "project-only", &with(computed, &roots[..1], None));
    let page = smallest("pages/");
    let shard = smallest("devices/");
    write(
        dir,
        "shard-and-page",
        &with(computed, &[shard, page, "structure.jsonl"], None),
    );
    write(
        dir,
        "library-documents",
        &with(
            computed,
            &[
                smallest("lib/fndefs/"),
                smallest("lib/forms/"),
                smallest("lib/parts/"),
                smallest("lib/symbols/"),
                "project.jsonl",
            ],
            None,
        ),
    );
    write(dir, "manifest-only", &with(0, &[], Some(file("manifest.json"))));
    write(
        dir,
        "stale-manifest",
        &with(
            0,
            &["revisions.jsonl", "structure.jsonl"],
            Some(file("manifest.json")),
        ),
    );
    write(
        dir,
        "tiny-limits",
        &with(computed | MODE_TINY_LIMITS, &roots[..2], None),
    );
    write(dir, "empty", &[0]);
    // The shape an older build wrote (the typed revisions, FT-165): the revision lines with their members in
    // byte-wise key order, next to the project.
    let old: Vec<u8> = file("revisions.jsonl")
        .split(|b| *b == b'\n')
        .filter(|l| !l.is_empty())
        .flat_map(|l| {
            let value: serde_json::Value = serde_json::from_slice(l).expect("a revision line");
            let mut line = serde_json::to_vec(&value).expect("a line");
            line.push(b'\n');
            line
        })
        .collect();
    write(
        dir,
        "old-shape-revisions",
        &loader::encode(
            computed,
            &[
                ("project.jsonl", file("project.jsonl")),
                ("revisions.jsonl", &old),
            ],
            None,
        ),
    );
}

fn open_path_seeds(dir: &Path) {
    let op = |file: &str, line: u8, op: u8, arg: u8| Op {
        file: golden::index_of(file),
        line,
        op,
        arg,
    };
    write(dir, "golden-as-it-is", &open_path::encode(0, &[]));
    write(
        dir,
        "golden-stale-manifest",
        &open_path::encode(MODE_STALE_MANIFEST, &[op("revisions.jsonl", 0, 0, 0)]),
    );
    write(dir, "golden-read-only", &open_path::encode(MODE_READ_ONLY, &[]));
    // The older build's shape: the revisions in byte-wise member order.
    write(
        dir,
        "old-shape-revisions",
        &open_path::encode(
            0,
            &[op("revisions.jsonl", 0, 0, 0), op("revisions.jsonl", 1, 0, 0)],
        ),
    );
    // The findings of the first hosted fuzz-smoke run (cycle 22), as libFuzzer wrote them: a BOM before a library
    // copy (a part copy, then the member order of a revision; edit mode), the same before a function definition
    // (stale manifest, read-only), and a swap of two lines inside a part copy (not a shape: it moves `sha256` into
    // `meta`).
    write(
        dir,
        "lib-document-bom",
        &[0x00, 0x75, 0x73, 0x65, 0x72, 0xa9, 0x01, 0x00, 0x00],
    );
    write(dir, "lib-document-bom-read-only", &[0xff, 0xff, 0xff, 0x65, 0x00]);
    write(
        dir,
        "lib-document-line-swap",
        &[0x2f, 0x73, 0x74, 0x61, 0x74, 0x65],
    );
    for kind in 0..OP_KINDS {
        // Revisions are leaf records: most changes to them still open (a change to the structure refuses to).
        let target = "revisions.jsonl";
        write(
            dir,
            &format!("op-{kind:02}"),
            &open_path::encode(0, &[op(target, 1, kind, 0x21)]),
        );
    }
}

/// A deterministic tape: an LCG stream.
fn tape(seed: u64, len: usize) -> Vec<u8> {
    let mut state = seed;
    (0..len)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 56) as u8
        })
        .collect()
}

fn command_seeds(dir: &Path) {
    // The seeds of `fuzz_smoke`, as plain seeded runs (a tape of zeros), and runs with a tape.
    write(dir, "seed-1", &ew_fuzz::command::encode(1, 8, &[]));
    write(dir, "seed-2", &ew_fuzz::command::encode(2, 8, &[]));
    write(dir, "seed-7-tape", &ew_fuzz::command::encode(7, 8, &tape(7, 192)));
    write(
        dir,
        "seed-13-tape",
        &ew_fuzz::command::encode(13, 8, &tape(13, 256)),
    );
    write(
        dir,
        "seed-21-short",
        &ew_fuzz::command::encode(21, 5, &tape(21, 48)),
    );
}

fn main() {
    let out: PathBuf = std::env::args_os()
        .nth(1)
        .map_or_else(|| golden::repo().join("fuzz").join("corpus"), PathBuf::from);
    loader_seeds(&out.join("jsonl_loader"));
    open_path_seeds(&out.join("migration_chain"));
    command_seeds(&out.join("command_sequence"));
    println!("seed corpora written to {}", out.display());
}
