<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# The cargo-fuzz crate

FT-297, FT-272 and FT-304; [doc 18](../docs/design/18-quality-security-release.md) §3.6 (the command-sequence fuzzer)
and §3.12 (parser fuzzing); CI-N-02 and CI-N-03 of the job catalogue in §5.2; the nightly workflow is
[`deep-nightly.yml`](../.github/workflows/deep-nightly.yml), described in [`ci/README.md`](../ci/README.md#deep-nightly-deep-nightlyyml-ci-n-02-and-ci-n-03); CI-PR-08 (FT-335) runs 60 s of every target that a pull request can reach in the `fuzz-smoke` job of [`ci.yml`](../.github/workflows/ci.yml) ([`ci/README.md`](../ci/README.md#fuzz-smoke-ci-pr-08)).

The crate is **not a member of the cargo workspace** (the root `Cargo.toml` lists it under `exclude`, and
`fuzz/Cargo.toml` has its own `[workspace]` table). cargo-fuzz instruments the build with LLVM sanitizer coverage and
AddressSanitizer, which needs a nightly toolchain, and that build must never reach `cargo build --workspace`,
`cargo xtask ci` or `cargo deny` on the stable toolchain. The crate is GPL-3.0-or-later (it includes the test modules of
`ew-testkit`), `publish = false`, and a development tool: it is never distributed. Its `Cargo.lock` was seeded from the
workspace's, so the shared dependencies have the versions of the tested build; the only crates it adds are
`libfuzzer-sys` and `arbitrary`.

`unsafe`: none in this crate (`#![forbid(unsafe_code)]`); the `unsafe` of the `fuzz_target!` macro lives in
`libfuzzer-sys`, a dependency. No workspace crate changed for it, and the determinism lints (the generated
`clippy.toml`, the `disallowed-methods` rules) apply to the workspace crates only.

## The targets

| Target | What it feeds | Properties |
|---|---|---|
| `jsonl_loader` | Files of a project directory (the golden project's paths and a few more classes), each with arbitrary bytes, a manifest (arbitrary bytes, or computed from the files so that the hashes match) and, with a mode bit, tiny limits, through `ew_format::{Manifest::parse, plan, load}` | No panic; a parsed manifest survives its text form; a valid input (no finding) renders to files that load again with no finding, nothing to rewrite and the same records and library documents; the canonical rendering is a fixpoint of load and render |
| `migration_chain` | The golden project with a list of operations on its records (members in byte-wise order, a removed or foreign member, another `$k` or `$v`, duplicated, swapped, cut, moved and deleted lines, CRLF, a BOM, a missing file), the manifest recomputed (or left stale), opened by `ew_store::Store::open` | No panic; the model passes `integrity_check`; when the session can edit, a save without an edit succeeds, a second save writes nothing, a reader opens the result with an equal model (`content_eq`) and nothing left to rewrite |
| `command_sequence` | A seed, a step count (1 to 8) and a tape of bytes through the seeded command generator of the nightly command fuzzer (`crates/ew-testkit/tests/command_fuzz`, included as modules) over Golden A | The oracles of the nightly test with a cheap cadence: the structural check after every changing step, the validity oracle of every command, the save-and-reload round trip, the closing undo-all (the undo law). The memo oracle belongs to `fuzz_long` and to the full oracle run |
| `png_pdf` | PNG bytes through `ew-render-cpu`'s `decode_image` and, when it accepts them, a one-page PDF/A-2b through `ew-export`'s writer (krilla) | No panic; an accepted PNG is written and drawn (`Ok`, one image, none skipped) |
| `ewz_read` | Arbitrary bytes through `ew-store`'s strict `.ewz` reader (`ew_store::ewz::read`, WP-S1.17 part 2) under the default `ZipLimits` | No panic; when an accepted archive's entries pack again (`ewz::pack`), the new archive reads to the same entries and manifest |
| `merge_file` | Three sides (base, ours, theirs) and a path selector (a page, the root file, a device shard, a library document, `manifest.json`, a file of no project class) through `ew_format::merge_file`, the core of `ew merge-driver` (WP-S1.17 part 4b), under the default limits and the shipped chain | No panic; a merge of two sides that each reload editable (alone at their path through `ew_format::load`, the manifest through `Manifest::parse`) reloads editable, with or without conflicts |
| `sql_sandbox` | One SQL text (lossy UTF-8) through `ew-sqlview`'s sandboxed `Reader` with the report profile over the golden project's SQL projection, built once (WP-S6.04, FT-223) | No panic; rows within the row cap or an `SqlError`; the image unchanged; the same text twice gives the same result unless the deadline stopped a run |
| `svg_asset` | Bytes through the asset sniff (`ew_schema::sniff`, with `svg_hazard`) and, accepted as an SVG, `ew-scene`'s drawing path (`svg_items`, usvg inside `catch_unwind`) (WP-S6.04) | No panic |
| `image_decode` | A selector byte (PNG even, JPEG odd) and the bytes through `ew-render-cpu`'s `decode_image` (WP-S6.04) | No panic; a decoded image is at least 1 × 1, at most `MAX_IMAGE_PIXELS`, four bytes per pixel |
| `textconv` | A selector byte (no session even, the golden project's session odd) and one project file through `ew_api::output::textconv` (WP-S6.04) | No panic; a text results; no record named without a session, at most one per line with it |
| `table_round_trip` | A selector byte (bit 0: the table key given or not; the rest: the navigator) and a round-trip file (CSV or a workbook, by its own signature) through `Session::table_import` on the golden project's session (WP-S6.04) | No panic; changes or a refusal; the model unchanged |
| `command_log` | A `.ewcmd` (lossy UTF-8) through `CmdLog::parse` and `Session::run_script` on a new in-memory session of the golden project (WP-S6.04) | No panic; a log naming a `HumanOnly` command is refused before its first line; a log refused before its first line changes nothing; the model passes the structural check |
| `qet_project`, `qet_element`, `golden_tables`, `parts_table` | The four importers through the import front door (WP-S3.09 D15, src/importers.rs): the bytes as one `.qet` imported into an empty project by `import.apply`; as one `.elmt` under the element-definition limits; as up to eight named tables of the golden dialect (a count byte, then a selector byte, a 2-byte length and the bytes per table) through the bootstrap and `session_from_batch`; as a CSV or XLSX supplier table with a fixed column map | No panic (the front door's caught panic fails the target); every emitted record and library document reads again strictly from its own line with no malformed variant and validates; a batch keeps the importer contract; a model made from it passes the structural check |
| `ewlib_read` | A `.ewlib` directory (WP-S6.04, ID-1354, src/ewlib_read.rs): a mode byte, then sections of a selector, a 2-byte length and the bytes, for `library.json`, `library.lock` and records files (a records section starts with its shard and generation); mode bit 0 computes the manifest from the files. The files are read by `ew-library`'s in-memory entry, `Registry::in_memory`, as a company library with the index in memory (FT-1382); only an input with a lock is written to a fresh folder below the system's temporary folder, for the lock, and removed | No panic; a parsed manifest and a parsed lock record survive their own text; every document handed out has a JSON form that reads again; another session's lock is never taken and its file is left as it was |
| `ewui_script` | A `.ewui` script (FT-1452, src/ewui_script.rs): the bytes as UTF-8 through the headless runner's parser, `ew_viewmodel::script::parse`; the replay test also runs the tutorial scripts of `docs/book/tutorial-scripts/` as seeds, read where they are | No panic; the steps stand on lines of their own, in order, with object parameters; a parsed script written back parses to the same actions and parameters |
| `symbol_authoring` | The symbol editor's readers (WP-S2.10, M25-p1#8, src/symbols.rs): the bytes as a portable `ew.symbol/1` draft (`SymbolDraft::parse`), and as an EPLAN input named `.edz`, `.ems` and `.xml` through `ew_api::symbols::import_eplan`, the front door under `Limits::element_definition()` | No panic (the front door's caught panic, `symbol-import-stopped`, fails the target); a draft validates and previews (every variant of a parsed one), and a draft without validation issues yields its library documents |
| `symbol_slk` | An EPLAN SLK 1.2 library (M25-p2#6, src/symbols.rs): nine sections of a 2-byte little-endian length and the bytes, the descriptor and then the eight `.sdb` tables of `ew_import_eplan::slk::FILES` in order, through `slk::directory` and `slk::read` | No panic; a read holds the read budget: at most 4,096 graphics and 4,096 connection points per symbol, 65,536 of both together, 1 to 512 symbols |

**The migration chain.** The format's upgrade chain on `serde_json::Value` (WP-S1.17) is not built. What exists is the
open path that takes a project written by an older build within the same format (the typed revisions, ADR-0011
amendment 4: a reserved kind typed in place) and rewrites it at the next save; `migration_chain` exercises that path with
every old shape an operation can make. When WP-S1.17 lands, the chain joins the target.

**The command sequence.** Every draw of the generator is its seeded SplitMix64 output XOR the next eight bytes of the
tape, so changing one tape word changes one draw and libFuzzer's coverage feedback steers the sequence; a tape of zeros is
the plain seeded run of `fuzz_long` with that seed (`EW_FUZZ_SEEDS=<seed>`). Because a step costs about 75 ms in a
sanitized build and a run about 0.15 s more, the target runs about 2 inputs a second: it is a coverage-guided complement
to the nightly test, not a replacement.

## Running a target locally

```sh
cargo install cargo-fuzz --version 0.13.2 --locked      # once
cd fuzz                                                  # the nightly of rust-toolchain.toml applies inside this directory
mkdir -p target/corpus/jsonl_loader
cargo fuzz run -a jsonl_loader target/corpus/jsonl_loader corpus/jsonl_loader -- -max_total_time=60 -max_len=65536
```

The first corpus directory is the working one (libFuzzer writes the inputs that add coverage there; `target/` is ignored);
the committed seeds under `corpus/<target>/` stay as they are. Useful `max_len` values: `jsonl_loader` 65536,
`migration_chain` 65, `command_sequence` 512, `png_pdf` 16384, `ewz_read` 16384, `merge_file` 65536, `sql_sandbox` 4096, and 16384 for `svg_asset`,
`image_decode`, `textconv`, `table_round_trip` and `command_log`; `qet_project` 65536, `qet_element` 16384, `golden_tables` 65536, `parts_table` 65536, `ewlib_read` 65536, `ewui_script` 16384, `symbol_authoring` 65536, `symbol_slk` 16384. Run it from `fuzz/`: from the repository root rustup would choose the stable
toolchain of the workspace and cargo-fuzz would refuse. AddressSanitizer is cargo-fuzz's default. A sanitized binary that
links the bundled SQLite reports an ODR violation for `aiOff` at start-up unless `ASAN_OPTIONS=detect_odr_violation=0`
(cargo-fuzz sets it for `cargo fuzz run`; export it to run a built binary by hand). This is a false positive of the C code,
not a second copy of SQLite: both lock files hold one `libsqlite3-sys` 0.38.2 and a binary has one `sqlite3_open`, but
`sqlite3.c` has two function-local tables `static const unsigned short aiOff[]` with identical contents (the FTS3 and FTS5
case folders), the linker merges them into one address and the check sees two globals registered there.

`-a` switches on debug assertions and overflow checks, which cargo-fuzz's release build lacks otherwise (the oracles of
the targets rely on them; CI sets them through `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS` and `..._OVERFLOW_CHECKS`).

**A crash carries no generator log.** libFuzzer's panic hook aborts the process at the first panic, before the unwind that
the command generator's `catch_unwind` relies on to print the seed, the step and the command log (what `fuzz_long`
reports): the crash output is the panic message and the stack. `tests/replay.rs` recovers the context: in an ordinary test
process the panic is caught, so replaying the crash input there prints the generator's failure text with the command log.
Copy the crash input to `regressions/<target>/` and run `cargo test --release --test replay -- --nocapture` in `fuzz/`
(the test replays every file of `regressions/` and reports the outcome without failing, since a finding fails by
definition).

Reproduce a crash: `cargo fuzz run <target> artifacts/<target>/crash-<hash>` (or a file of `regressions/<target>/`). The
inputs are byte formats, decoded by `ew_fuzz::loader::decode`, `ew_fuzz::open_path::decode` and `ew_fuzz::command::decode`
(their module documentation describes them); `cargo test --release` in `fuzz/` replays every seed of `corpus/` through the
targets' checks without the fuzzer (`tests/replay.rs`) and checks that the seeds get as far as the properties they were made
for.

## Seed corpora and regressions

`corpus/<target>/` holds small seeds made from the golden project (anonymised, CC0) by `src/bin/seed_corpus.rs`; `cargo xtask settle`
regenerates the folders of `jsonl_loader`, `migration_chain`, `command_sequence`, the four importer targets and `ewlib_read`
(from the sample parts library of `content/libraries/`; the step `fuzz-seeds`,
triggered by Golden A's files, `fuzz/src/`, the golden tables, the QET element fixtures and the synthetic QET project; it runs `cargo run --manifest-path fuzz/Cargo.toml --locked --bin seed_corpus -- <dir>` from the repository
root, which is deterministic, and in `--check` names each seed that differs, is missing or is no longer made). A loader seed names
its files by their index in Golden A's sorted file list, so a file added to or removed from Golden A makes the seeds stale. The seeds of `png_pdf` are four small PNGs
written by hand (an 8-bit RGBA image with translucent pixels, an 8-bit grey-and-alpha image, a palette image with `tRNS`, a
16-bit RGB image), not made by `seed_corpus`. The seeds of `ewz_read` are two `.ewz` files that `ew export ewz` wrote
from `golden/fixtures/release-min`, one with a `.gitattributes` and a `.gitignore` beside it, not made by `seed_corpus`.
The seeds of `merge_file` are AC-41's data, made by hand from Golden A: page 102 with W20's `colour` BU added on ours and
`csa` "2.5" on theirs (`w20-clean`), with `colour` WH on theirs too (`w20-conflict`), and the manifest changed on both
sides (`manifest`); not made by `seed_corpus`.
The seed of `ewui_script` is one small script written by hand (`small`, CC0 like the folder); the tutorial scripts
(CC-BY-4.0, under `docs/`) are not copied here: the replay test reads them where they are (FT-1452).
The seeds of `symbol_authoring` are made by hand, not by `seed_corpus`: `ew symbol template fuzz.seed Seed`'s draft, a
parts XML, a PXF macro, a stored ZIP EDZ of the parts XML and a copy-coded 7z EDZ of the macro and the parts XML; the
seed of `symbol_slk` is the SLK fixture of `crates/ew-import-eplan/src/slk.rs`'s unit tests (one symbol, one line, one
connection point) as nine sections.
The seeds of WP-S6.04's targets are not made by `seed_corpus` either: `sql_sandbox`'s are the hostile SQL of
`crates/ew-testkit/hostile/sql/` (the same bytes, which the replay test checks) and four queries of the golden project's
views (`query-*.sql`); `svg_asset`'s three small SVGs written by hand (one with a script, refused by the sniff);
`image_decode`'s the four PNGs of `png_pdf` behind the PNG selector and the hand-written 16 × 8 JPEG of `ew-render-cpu`'s
tests behind the JPEG one; `textconv`'s two golden files and a few lines that are no records; `table_round_trip`'s the
golden project's pages and devices as `ew export table --preset excel-en --line-end lf` writes them and its pages as a
workbook; `command_log`'s Golden B's variants B1 and B2 and a log that names `revision.approve`.
`artifacts/` (what libFuzzer writes for a crash, an out-of-memory, a timeout or a slow input) and `coverage/` are ignored.

**A finding is reported, not fixed in the change that finds it.** Copy the crash input to `regressions/<target>/` (a
descriptive name, no private data), add a failing-case description (what the input makes the target do, the panic message,
the expected behaviour) to the report of the finding and to the decision log, and give the defect a follow-up task. The
fuzz job's verdict goes green again when the defect is fixed; the regression input then becomes a plain test of the crate
that was fixed.

## Maintenance

- The nightly is pinned by date in `rust-toolchain.toml` (and `NIGHTLY_TOOLCHAIN` of `deep-nightly.yml` and of `ci.yml`, for
  the `fuzz-smoke` job; a test of xtask keeps them equal) and advanced by the upgrade train (doc 18 §6.3).
- A new target is added to the table of `ci/fuzz-smoke-targets.toml` (the path filter of CI-PR-08; a test of xtask recomputes its dependency closures) and to the matrix of
  `deep-nightly.yml`; the filter's self-test fails when the table, `Cargo.toml` and the matrix disagree.
- A command added to the registry is fuzzed by the schema-driven default arm
  (`crates/ew-testkit/tests/command_fuzz/schema_arm.rs`) without a change here; a hand arm in `arms.rs` adds
  well-formed steps and deep states, and must name a registered command (`every_hand_arm_names_a_registered_command`).
- The dependencies of the crate are outside the workspace's `cargo deny` and `cargo vet` runs; they are `libfuzzer-sys`
  (and its `arbitrary` and `cc`) and the workspace's own path crates. The job `fuzz-deny` of the deep nightly runs
  `cargo deny check licenses advisories` over the crate's own graph with `fuzz/deny.toml` (the workspace's `deny.toml` policy plus an NCSA exception for libfuzzer-sys, which carries LLVM's libFuzzer runtime), report-only.
- On every pull request the `lint` job formats, checks and lints the crate on the stable toolchain
  (`cargo fmt|check|clippy --manifest-path fuzz/Cargo.toml`; from the repository root rustup takes the stable toolchain of
  the root `rust-toolchain.toml`, because it looks at the working directory, not at the manifest's), and `cargo xtask ci`
  has the same three steps; only running a target needs the nightly.
