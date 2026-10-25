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
`migration_chain` 65, `command_sequence` 512, `png_pdf` 16384. Run it from `fuzz/`: from the repository root rustup would choose the stable
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

`corpus/<target>/` holds small seeds made from the golden project (anonymised, CC0) by `src/bin/seed_corpus.rs`; regenerate
with `cargo run --release --bin seed_corpus` from `fuzz/` (deterministic). The seeds of `png_pdf` are four small PNGs
written by hand (an 8-bit RGBA image with translucent pixels, an 8-bit grey-and-alpha image, a palette image with `tRNS`, a
16-bit RGB image), not made by `seed_corpus`. `artifacts/` (what libFuzzer writes for a crash,
an out-of-memory, a timeout or a slow input) and `coverage/` are ignored.

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
- A command added to the registry needs a generator arm in `crates/ew-testkit/tests/command_fuzz/arms.rs`
  (`every_registered_command_has_a_generator_arm` fails otherwise); the target picks it up without a change here.
- The dependencies of the crate are outside the workspace's `cargo deny` and `cargo vet` runs; they are `libfuzzer-sys`
  (and its `arbitrary` and `cc`) and the workspace's own path crates. The job `fuzz-deny` of the deep nightly runs
  `cargo deny check licenses advisories` over the crate's own graph with `fuzz/deny.toml` (the workspace's `deny.toml` policy plus an NCSA exception for libfuzzer-sys, which carries LLVM's libFuzzer runtime), report-only.
- On every pull request the `lint` job formats, checks and lints the crate on the stable toolchain
  (`cargo fmt|check|clippy --manifest-path fuzz/Cargo.toml`; from the repository root rustup takes the stable toolchain of
  the root `rust-toolchain.toml`, because it looks at the working directory, not at the manifest's), and `cargo xtask ci`
  has the same three steps; only running a target needs the nightly.
