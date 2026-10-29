<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# CI and repository policy

This directory holds the machine-readable policy files and scripts of the per-PR CI set
([18](../docs/design/18-quality-security-release.md) §5.2, gate G-M0 in [03-gates](../docs/plan/03-gates.md)).
The workflows live in [`.github/workflows/`](../.github/workflows); the policy checks are subcommands of the
[`xtask`](../xtask) package (`cargo xtask help`).

## Jobs of gate G-M0

The job names are the GitHub status-check names to mark as required.

| ID | Workflow / job | Content | Reproduce locally |
|---|---|---|---|
| CI-PR-01 | `ci.yml` / `lint` (ubuntu-24.04) | `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`, and again with `--all-features` (the generated `clippy.toml` also bans the std hash containers, FT-271, and the wall clock, environment and randomness, FT-317, below); `cargo xtask check-ordered-collections` and `cargo xtask check-disallowed-methods` | `cargo fmt --all --check`, both clippy runs, `cargo xtask check-ordered-collections` and `check-disallowed-methods` |
| CI-PR-02 | `ci.yml` / `test-linux` and `test-linux-default` (ubuntu-24.04), split by feature set since FT-353 (ID-527): `test-linux` runs the all-features part and both doctest runs, `test-linux-default` the filtered default-features run, in parallel; each builds first, after installing Mesa's lavapipe (`mesa-vulkan-drivers`; `apt-get` with `Acquire::Retries=3` and 20-second connection timeouts since FT-479) so that the GPU canvas's parity test runs instead of skipping (WP-S2.02 part 2) | `cargo build --workspace --locked`; `cargo nextest run --workspace --locked --profile ci -E 'not (package(ew-api) and binary(acceptance)) and not (package(ew-testkit) and binary(command_fuzz))'`, and again with `--all-features` and no filter (feature-gated tests such as `ew-ids/testkit`; the headless acceptance tests, about 2 min, and the command fuzzer's `fuzz_smoke`, 15 to 19 s, run once per job, in this second run: FT-268, FT-306); `cargo test --doc --workspace --locked`, and again with `--all-features` (nextest does not run doctests) | `cargo build --workspace --locked`, `cargo nextest run --workspace --locked -E 'not (package(ew-api) and binary(acceptance)) and not (package(ew-testkit) and binary(command_fuzz))'`, `cargo nextest run --workspace --all-features --locked` and `cargo test --doc --workspace [--all-features] --locked` |
| CI-PR-03 | `ci.yml` / `test-windows-build`, `test-windows-1`, `test-windows-2`, `test-windows-3` (windows-2025) | all-features run only (FT-330, ID-482), split since FT-353 (ID-527), in three run jobs since FT-387 (the cycle-24 delivery run's chain was 20:01 + 10:00 = 30:01, ID-582's reading; the cycle-25 run's 16:20 + 14:54 = 31:14, which balanced the parts by time instead of by test count): `test-windows-build` runs `cargo nextest archive --workspace --all-features --locked --archive-file "$RUNNER_TEMP/nextest-archive.tar.zst"` (the archive lives outside the checkout, which the licence-map walk would otherwise flag), saves the rust-cache and uploads the archive (`actions/upload-artifact`); `test-windows-1`, `test-windows-2` and `test-windows-3` download it (`actions/download-artifact`, no cache save) and run `cargo nextest run --archive-file "$RUNNER_TEMP/nextest-archive/nextest-archive.tar.zst" --workspace-remap .` with `--profile ci` and these filters: part 1 `-E 'binary_id(=elektrawatt::canvas) or binary_id(=ew-api::acceptance) or binary_id(=ew-derive::incremental_partition) or binary_id(=ew-import-qet::hostile_input) or binary_id(=ew-store::incremental) or binary_id(=ew-store::purge) or binary_id(=ew-testkit::command_fuzz) or binary_id(=ew::cli)'`, part 2 `-E 'binary_id(=elektrawatt::headless) or binary_id(=elektrawatt::part2) or binary_id(=ew-api::checks) or binary_id(=ew-api::memo_oracle) or binary_id(=ew-api::page_output) or binary_id(=ew-store::fault_injection) or binary_id(=ew-testkit::golden_dsl) or binary_id(=ew-testkit::spike_memo) or binary_id(=ew-ui::canvas) or binary_id(=ew-viewmodel::part2)'`, part 3 `-E 'not (` both lists `)'` (eight heavy binaries in part 1, ten in part 2, every other binary in part 3: a complete, disjoint cover; re-balanced in cycle 27, see the Windows chain section; the split was `count:1/2` and `count:2/2` of the complement of six binaries before). The acceptance tests and the command fuzzer are in part 1, so they run once in total. No default-features build or nextest run, no doctest run (doctests run in `test-linux`, `test-macos` and `cargo xtask ci`) | `cargo nextest run --workspace --all-features --locked` (the three parts together) |
| CI-PR-04 | `ci.yml` / `test-macos` and `test-macos-all-features` (macos-15, arm64; parallel jobs since FT-438) | as CI-PR-02, on macOS, in two jobs that each build and test on their own runner: `test-macos` is the default-features half (`cargo build --workspace --locked`, the filtered `cargo nextest run --workspace --locked --profile ci -E '<once-per-job filter>'`, `cargo test --doc --workspace --locked`), `test-macos-all-features` the all-features half (the unfiltered `cargo nextest run --workspace --all-features --locked --profile ci`, which runs the acceptance tests and the command fuzzer once, and `cargo test --doc --workspace --all-features --locked`); the union is the former single job, nothing is dropped | the two halves' commands, on macOS |
| CI-PR-05 | `ci.yml` / `headless` (ubuntu-24.04) | `cargo build -p ew --locked`; `ew export pdf golden/J-E-2416.ew` inside a bare `ubuntu:24.04` container (image pinned by digest; no network, no display, no GPU, no font file, empty user directories), on one thread and on four; both PDFs must be the committed digest of `golden/J-E-2416/expected/pdf/sha256.txt` (FT-295, G-M1-01); the PDFs uploaded as the artifact `headless-pdf`. Details: [Headless container job](#headless-container-job-ci-pr-05) | `cargo build -p ew --locked` and `docker run` as in the job (Linux and docker), or `cargo nextest run -p ew --test cli -E 'test(export_pdf_of_golden_a)'` for the same check without the container |
| CI-PR-08 | `ci.yml` / `fuzz-smoke` (ubuntu-24.04) | 60 s of each cargo-fuzz target of [`fuzz/`](../fuzz) that the change can reach, on the pinned nightly, the committed corpus as seeds; the path filter [`fuzz-smoke-targets.py`](fuzz-smoke-targets.py) maps the changed paths to targets through the dependency closure of each target ([`fuzz-smoke-targets.toml`](fuzz-smoke-targets.toml); FT-335, G-M1-13). Red for a finding (the `::error::` annotation names the target and the class: `crash`, `oom`, `timeout` or `leak`, FT-370), a failed build or a run without a verdict (a target that outlives its time bound included). Details: [Fuzz smoke](#fuzz-smoke-ci-pr-08) | `python3 -I ci/fuzz-smoke-targets.py --selftest`; `git diff --name-only origin/main HEAD \| python3 -I ci/fuzz-smoke-targets.py` names the targets; then, in `fuzz/`, `cargo fuzz run <target> target/corpus/<target> corpus/<target> -- -max_total_time=60 -max_len=<n>` |
| CI-PR-09 | `ci.yml` / `supply-chain` (ubuntu-24.04) | `cargo deny check`; `cargo audit`; `cargo vet`; `cargo xtask check-deps`, `check-features`, `check-build-scripts`, `check-vet-imports`, `check-workflows`; third-party notices with cargo-about 0.9.2 (NFR-LIC-08); MSRV check of the permissive crates with Rust 1.85.0 | `cargo deny --locked check`, `cargo audit`, `cargo vet --locked`, the five `cargo xtask` checks, `cargo about generate -o target/THIRD-PARTY-NOTICES.txt ci/third-party-notices.hbs`, and the MSRV commands below |
| CI-PR-10 | `docs.yml` / `legal-docs` (ubuntu-24.04) | `docs/plan/tools/{totals,schedule,trace}.py --check` and `doccheck.py`; `reuse lint` (reuse-tool 6.2.0; NFR-LIC-01, NFR-LIC-02); `cargo xtask check-licence-map` (licence map, collective notice, crate licences and the licence boundary of the permissive layer: 19 §5.1, NFR-LIC-10, NFR-MAINT-02); `cargo xtask check-anonymisation` (NFR-PRIV-09); `cargo xtask check-governance-files` (NFR-DOC-08); rustdoc build with `-D warnings` (NFR-MAINT-08). On pull requests only: `cargo xtask check-dco` over the PR's commits (NFR-LIC-03); and the RFC reference, which fails a PR labelled `format-major`, `api-break` or `licence` whose description does not mention `rfcs/` (NFR-DOC-09) | `python3 -I docs/plan/tools/totals.py --check` (and the other three); `python -m reuse lint`; `cargo xtask check-licence-map`, `check-anonymisation`, `check-governance-files`, `check-dco origin/main..HEAD`; `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` |
| CI-PR-15 | `docs.yml` / `docs-diagrams` (ubuntu-24.04) | Renders every Mermaid block under `docs/` and in the root Markdown with mermaid-cli 12.0.0 (container image pinned by digest) and fails on any error | `python3 -I ci/render-mermaid.py --image <image@sha256:…>` (needs docker); `--extract-only` lists the blocks without docker |
| CI-PR-10 (book) | `docs.yml` / `docs-book` (ubuntu-24.04) | The user-documentation book of `docs/book` (WP-S0.06, NFR-DOC-01, NFR-DOC-06): `cargo xtask docs-gen --check` (the generated command and rule reference pages are current); mdBook 0.5.4, installed from its pinned release archive (SHA-256 checked); `mdbook build docs/book` in an empty network namespace (the build is offline); `ci/check-book-links.py` over the built pages (relative links, anchors, and links into the repository); the built book uploaded as the artifact `user-guide`. A separate job, so that `legal-docs` stays fast | `cargo xtask docs-gen --check`; `mdbook build docs/book` (mdBook 0.5.4); `python3 -I ci/check-book-links.py target/book` |
| CI-N-09 | `ci.yml` / `verapdf` (ubuntu-24.04) | Builds `ew`, exports the golden PDF in three variants (all pages; dated by `SOURCE_DATE_EPOCH`; `--pages 101-106,901`) and validates each against PDF/A-2b with veraPDF 1.30.3 (the project's `verapdf/cli` image pinned by digest, run with no network); `ci/verapdf-check.py` turns the XML report into the verdict (FT-294, G-M1-02); the report uploaded as the artifact `verapdf-report`. Details: [veraPDF job](#verapdf-job-ci-n-09) | `python3 -I ci/verapdf-check.py --selftest`; with docker: `docker run --rm -v "$PWD/out:/data:ro" verapdf/cli@sha256:… --flavour 2b --format xml /data/golden-all.pdf > out/verapdf-report.xml`, then `python3 -I ci/verapdf-check.py out/verapdf-report.xml` |
| CI-N-12 | `cli-artefact.yml` / `cli-artefact` (ubuntu-24.04, windows-2025, macos-15; nightly and `workflow_dispatch`) | `cargo build -p ew --release --locked` and a smoke test (`ew --version`, `ew --help`) per platform; uploads `ew-<os>-<arch>-unsigned-dev-<short sha>` (the program, `LICENSE.md` and a README that says "unsigned development build, not a release") with 90-day retention; no signing, no release, no secret (FT-299, G-M1-22). Details: [CLI artefact](#nightly-cli-artefact-cli-artefactyml-ci-n-12) | `cargo build -p ew --release --locked`; `target/release/ew --version` |
| CI-R-03 (before WP-S6.07) | `cli-artefact.yml` / `release-check` (ubuntu-24.04; nightly and `workflow_dispatch`) | `cargo build -p elektrawatt --release --locked` with `PKG_CONFIG=false` and path remapping; asserts no pkg-config override, no `ICU4X_DATA_DIR`, no `CARGO_ENCODED_RUSTFLAGS` and no cargo `[env]` table in the build environment, then fails on any build-host path in the binary (outside a reviewed exception list), any hard link to libwayland, libxkbcommon, libX11, libxcb, libEGL, libGL or libvulkan, and any library directory in x11-dl's generated config; uploads the report only, never the binary (FT-333, ID-476); report-only. Details: [Release check](#release-check-cli-artefactyml--release-check-ci-r-03-before-wp-s607) | `bash ci/release-check.sh env`, the build and `bash ci/release-check.sh scan …` with `PKG_CONFIG=false` and the `RUSTFLAGS` remap, as in the section below (Linux) |
| CI-N-02, CI-N-03 | `deep-nightly.yml` / `full-oracle`, `proptest`, `fuzz (<target>)` (three jobs), `asan-command-fuzz`, `gpu-parity` (ubuntu-24.04; nightly at 06:07 UTC and `workflow_dispatch`) | The memo oracle on every read over Golden A and Golden B (release profile, debug assertions on); the tests of the seven core crates with `PROPTEST_CASES=20000`; the three cargo-fuzz targets of [`fuzz/`](../fuzz) (`jsonl_loader`, `migration_chain`, `command_sequence`), 30 minutes each, AddressSanitizer, on the nightly pinned by date; the seeded command fuzzer `fuzz_long` under `-Zsanitizer=address` (FT-297, FT-304, FT-272). Red only for a finding, a failed build or a hang. Details: [Deep nightly](#deep-nightly-deep-nightlyyml-ci-n-02-and-ci-n-03) | `cd fuzz && cargo fuzz run <target> -- -max_total_time=60`; `PROPTEST_CASES=2000 cargo nextest run --release -p ew-model -p ew-commands -p ew-store -p ew-format -p ew-memo -p ew-derive -p ew-schema`; `cargo nextest run --release -p ew-api --all-features -E 'binary(memo_oracle) or binary(golden_b)' --run-ignored all` |
| CI-PR-13 | `coverage.yml` / `coverage` (ubuntu-24.04; weekly and `workflow_dispatch`, not per PR) | cargo-llvm-cov 0.9.1 over the workspace tests (all features, nextest), the line coverage of `ew-model`, `ew-commands`, `ew-memo`, `ew-derive`, `ew-format` and `ew-store` against 80 % (NFR-MAINT-06); **report-only** (FT-296, G-M1-15). Details: [Coverage job](#coverage-job-ci-pr-13) | `cargo llvm-cov nextest --workspace --all-features --locked --no-report`, `cargo llvm-cov report --json --summary-only --output-path summary.json`, `python3 -I ci/coverage-summary.py summary.json` |

Step times of the test jobs in ci run 39797238020 (the cycle-20 delivery; GitHub jobs API, 2026-10-22; FT-353, ID-527):

| Step | test-linux | test-windows | test-macos |
|---|---|---|---|
| set-up, checkout, toolchain, cache restore, nextest install | 0:37 | 1:11 | 0:40 |
| `cargo build --workspace` | 1:09 | not run | 1:03 |
| `cargo nextest` without the acceptance tests and the fuzzer | 15:02 | not run | 8:54 |
| `cargo nextest --all-features` | 11:48 | 24:43 | 6:17 |
| `cargo test --doc` | 1:08 | not run | 0:59 |
| `cargo test --doc --all-features` | 0:15 | 2:26 | 0:13 |
| cache save (post step) | 0:21 | 4:55 | 0:29 |
| **Job total** | **30:23** | **33:20** | **18:40** |

Without the all-features doctest step (dropped from `test-windows` by FT-353) the Windows job is 30:54 on these
numbers, still over the 30-minute budget (NFR-MAINT-05), so FT-353 splits it (CI-PR-03) and splits the Linux job by
feature set (CI-PR-02: expected about 15 min for `test-linux`, about 17 min for `test-linux-default`; not yet measured).

Pinned action added by FT-353: `actions/download-artifact` v7.0.0 (`37930b1c2abaa49bbe596cd826c3c89aef350131`), the
same publisher and major as the `actions/upload-artifact` v7.0.1 of the other jobs.

The Windows chain (FT-387, decision log ID-582's reading of NFR-MAINT-05): the budget is the pull request's longest job
chain, here `test-windows-build` plus the longest run job, since the run jobs start when the build job ends. The cycle-23
delivery run had 17:34 + 9:46 = 27:20, the cycle-24 delivery run (39892594457, on `main`, so with the rust-cache save in
the build job) 20:01 + 10:00 = 30:01 and the run before 21:53 + 10:58 = 32:51. The lever taken is a third run job
(`test-windows-3`, `--partition count:3/3`): assuming about 2 minutes of fixed set-up in each run job (checkout, toolchain,
nextest install, download, `cargo fetch`; the step times are not recorded), each part drops from 8:25 and 10:00 to about
7:00 and the chain to about 27:00 on `main` and less on a pull request, which does not save the cache; the worst build time
seen (21:53) would still give 28:53. This is a projection, not a measurement: the next delivery run records the real
times. Nothing is skipped, filtered or run in release mode; the archive and the tests are the same. The next levers, if
the chain is over again, are a fourth part and, in the build job, the cache save moved out of the job's critical path.

The third run job did not balance the parts (FT-387, the next lever). The cycle-25 delivery run (39914507884, on `main`) had
`test-windows-build` 16:20 and the run jobs 12:22, 14:54 and 7:46, a chain of 16:20 + 14:54 = **31:14**: nextest's
`count:N/3` deals the tests out one by one in the order of the list, so it balances the number of tests (1113, 1114, 1113),
not the time, and the few binaries that hold most of the time (one test of `ew-api::memo_oracle` alone takes about
4 minutes of a full local run) landed in the second part. The test durations of a full local all-features run, summed per
binary (seconds, a relative weight only: that run used many cores at once, the runner has four), 3340 tests in 201
binaries, 3221 s in all:

| Test binary | tests | sum (s) | longest test (s) | part |
|---|---|---|---|---|
| `ew-api::memo_oracle` | 5 | 378 | 235 | 2 or 3 |
| `ew-testkit::command_fuzz` | 9 | 296 | 76 | 1 |
| `ew-store::fault_injection` | 20 | 249 | 69 | 1 |
| `ew-api::acceptance` | 13 | 205 | 203 | 1 |
| `ew-api::page_output` | 23 | 173 | 114 | 1 |
| `ew-store::purge` | 8 | 102 | 89 | 2 or 3 |
| `ew-testkit::spike_memo` | 1 | 97 | 97 | 2 or 3 |
| `ew-testkit::golden_dsl` | 7 | 96 | 38 | 1 |
| `ew-derive::incremental_partition` | 3 | 92 | 92 | 2 or 3 |
| `ew-api::checks` | 5 | 78 | 76 | 2 or 3 |
| `ew-viewmodel::part2` | 32 | 64 | 22 | 1 |

The split taken: part 1 is the six binaries marked 1 (104 tests, 1083 s), parts 2 and 3 the other 195 binaries by
`count:1/2` and `count:2/2` (1618 tests each, 1065 s and 1073 s); the six were chosen by trying every subset of the sixteen
slowest binaries. `--partition hash:N/3` was weighed and rejected: it deals tests out at random, so the heavy binaries'
tests land wherever the hash puts them (one emulated hash on these durations gives parts of about 9:00, 13:55 and
11:50), the result cannot be predicted offline and changes whenever a test is added; explicit binaries are stable and
leave only the rare re-balance by hand. Projection, **not a measurement**, from a fit of the cycle-25 run (a job takes
about 0:45 plus 2.4 times the list-scheduled time of the durations above on four threads, which reproduces 12:22, 14:54
and 7:46 within 4 %): parts of about 11:35, 11:27 and 11:29, so the chain is about 16:20 + 11:35 = **27:55** against
31:14. If the build job's 16:20 grows or the parts drift apart, re-run the per-binary sums on a fresh full run and move
binaries between part 1 and the complement; the guard test in `xtask` checks that the three filters are complementary
and that the acceptance tests and the fuzzer are in part 1. The next delivery run records the real times.

**Measured** (FT-387 closed, decision log ID-633): run 39985050322 on `main` had `test-windows-build` 10:49 and the
parts 8:13, 5:37 and 9:35, a chain of 10:49 + 9:35 = **20:24** (31:14 before), under the 30-minute budget. In the same
run `test-macos` took 30:04, four seconds over as a chain of one job; its levers are FT-438.

**`test-macos` split** (FT-438): the single job took 30:04 (39985050322), 23:12 (40041990698), 21:17 (40115388450), 33:21
(40131448239), 36:50 (40364837825) and 34:36 (40376419761, the cycle-28 delivery run). In the last run the steps were:
set-up and cache restore 0:46, `cargo build` 1:34, `cargo nextest` (default features, filtered) 17:47, `cargo nextest
--all-features` 13:01, the two doctest steps 0:25 and 0:17, cache save 0:38. The two nextest steps are 31 of the 34:36 and
they grow with the tests (the all-features step is 8:47 to 13:59 across the runs), so a smaller cut (one feature set
fewer) would have to take macOS coverage away. The cut taken is two parallel jobs on separate runners (CI-PR-04) with no
loss of coverage. Projection from the worst run (40364837825: build 1:49, default-features nextest 18:41, all-features
nextest 13:59, doctests 0:26 and 0:23, set-up about 0:45, cache save about 0:40): `test-macos` is about 0:45 + 1:49 +
18:41 + 0:26 + 0:40 = **22:20**; `test-macos-all-features` is about 0:45 + 13:59 + 0:23 + 0:40 plus the compile of the
test binaries that the default-features job no longer shares, about 4 to 6 minutes, = **about 20:30 to 22:30**: both
under the 25-minute aim and the 30-minute budget, with the worst observed run (36:50) cut by about 14 minutes. These are
projections; the first delivery run after the split records the real times. If either half grows past 25 minutes again,
the next lever is a nextest archive built once and run in two partitions, as the Windows jobs do (FT-387). Not part of
this FT: `test-linux-default` (28:40 and 26:35 in the two latest delivery-length runs) and the Windows chain (15:28 +
18:10 = 33:38 in run 40376419761, 17:55 + 22:42 in the failed run before it) are near or over the budget in those runs.

**Re-balance of cycle 27** (FT-387 follow-up): the cycle-26 delivery run (40041990698) measured `test-windows-build` 14:33
and the run jobs 15:01, 7:52 and 13:35, a chain of 14:33 + 15:01 = **29:34**: part 1 (six binaries) had become the long pole as
the wiring and find/replace tests grew `ew-viewmodel`, `ew-ui`, `elektrawatt` and `ew-api`, and the count halves of the
complement were 7:52 against 13:35 because a few lumpy binaries (`ew-api::memo_oracle`, five tests, 425 s; `spike_memo`, one
test) fall into one half. Per-binary sums of the all-features run of the local Windows gate (3472 tests, 211 binaries, 3917 s
in all; a relative weight, since that run used many cores): `ew-api::memo_oracle` 426 s (5 tests), `ew-testkit::command_fuzz`
306 (9), `ew-store::fault_injection` 248 (20), `ew-api::acceptance` 240 (13), `ew-api::page_output` 208 (23),
`ew-api::checks` 121 (5), `ew-store::purge` 111 (8), `ew-testkit::spike_memo` 99 (1), `ew-testkit::golden_dsl` 96 (7),
`ew-viewmodel::part2` 91 (32). The structure changed from "one slow list plus a count-halved complement" to "two explicit
lists plus the complement of both": the count halves cannot be steered, and the lumpy binaries have to be placed by hand to be
balanced. `--partition hash` was rejected again (random, unpredictable offline). Choice: a random search over the
assignment of the twenty heaviest binaries to part 1, part 2 or the remainder, scored with the cycle-25 fit (a job takes
0:45 plus 2.4 times the list-scheduled time of its tests on four threads, here per test instead of per binary), with the
single binaries `xtask` (208 fast tests) and the long tail left in part 3. Projection, **not a measurement**: parts of about
13:36 (138 tests), 13:12 (108 tests) and 14:37 (3226 tests), within 10 % of one another and, with the total work unchanged,
a chain of about 14:33 + 14:37 = **29:10** (the fit under-predicts the cycle-26 part 1 by about 20 %, so read the three figures as
relative; the gain is the removal of the 7:52 against 15:01 imbalance, not less total work). The next delivery run records the
real times; the guard test in `xtask` (`WINDOWS_PART1_BINARIES`, `WINDOWS_PART2_BINARIES`) keeps `ci.yml` equal to the lists.

**FT-479: wall time of `test-linux-default` and the Windows chain** (measured in runs 40611786232, 40607950237, 40376419761
and 40364837825 on `main`; the chain is the build job plus the slowest run job). Job totals (min:sec) and the steps that
vary:

| Run | `test-linux-default` (apt step, nextest) | `test-linux` | `test-windows-build` | parts 1, 2, 3 (download, nextest) | Windows chain |
|---|---|---|---|---|---|
| 40611786232 | 17:32 (-, 15:34) | 17:38 | 13:32 | 14:12 (2:58, 9:27), 6:30 (-, 4:37), 8:00 (1:33, 5:23) | 27:44 |
| 40607950237 | 17:48 (-, 16:01) | 17:03 | 16:17 | 12:08 (-, 10:14), 8:24 (-, 6:21), 10:37 (-, 8:27) | 28:25 |
| 40376419761 | 28:40 (7:20, 19:27) | 24:46 | 15:28 | 15:27 (5:12, 8:38), 10:03 (5:05, 3:53), 18:10 (7:38, 7:56) | 33:38 |
| 40364837825 | 26:35 (8:55, 15:46) | 16:04 | 17:55 | 18:06 (6:38, 9:59), 18:15 (11:26, 5:31), 22:42 (22:10, failed) | 40:37 |

`test-macos` and `test-macos-all-features` (the FT-438 halves) are 20:16 and 24:26 in run 40611786232 and 22:50 and 18:22
in run 40607950237 (red: a failing test, not the time). Findings: (1) the two slow `test-linux-default` runs lost 7:20 and
8:55 in the step that installs lavapipe, in the same runs where `test-linux` took 10 s and 11 s for the same command: a
stalled mirror connection, not test time; without it they would be 21:20 and 17:40. (2) The Windows chain varies by 13
minutes with the artifact transfer, not with the tests: the archive is 2.1 GB (2146855972 bytes in run 40611786232), the
download step takes 1:33 to 22:10 (it ran in all three parts at once) while the nextest steps stay at 3:53 to 10:14, and the
build job's cache save adds 1:50 to 4:29 to the critical path in a run on `main`. (3) Part 1 was the longest by test time
(8:38 to 10:14 against 3:53 to 6:21 for part 2 and 5:23 to 8:27 for part 3), and parts 1 and 2 ran `cargo fetch` (62 s and
64 s) only for the `xtask` tests, which are in part 3.

Taken: (a) both Linux jobs install lavapipe with `Acquire::Retries=3` and 20-second timeouts; (b) `ew-store::fault_injection`
(about 130 s of part 1, by the weights below) moves to part 2, so the means of the nextest steps become about 7:25, 7:15
and 7:15 (8:00, 5:05 and 7:15 before); (c) parts 1 and 2 no longer run `cargo fetch` (the `xtask` tests, the only ones that
start cargo, stay in part 3, which keeps it). No coverage changes: the partition test still proves a complete, disjoint
cover. Projected, not measured, from the steps of the two fast runs: run 40611786232 would have had part 1 at 14:12 - 2:10
- 1:02 = 11:00, part 2 at 6:30 + 2:10 = 8:40 and part 3 unchanged at 8:00, a chain of 13:32 + 11:00 = **24:32** (27:44
measured); run 40607950237 part 1 at 8:56, part 2 at 10:34, part 3 at 10:37, a chain of 16:17 + 10:37 = **26:54** (28:25
measured). `test-linux-default` without the stall is 17:30 to 21:20. Not taken: the download of the 2.1 GB archive
(the 22:10 outlier cannot be balanced away) and the build job's cache save are the next levers if a chain is over 30
minutes again: a slimmer archive and the cache save off the build job.

**Lesson: partition by time, not by count.** `--partition count:N/M` equalises the number of tests; when a handful of
binaries hold most of the run time, name them with `binary_id(=crate::binary)` in explicit lists, one explicit list per
part except the last, which is the complement, and keep every part either an explicit set or `not (those sets)` so that
the cover stays complete and disjoint.

Two lessons of the cycle-22 hotfixes to the split Windows jobs (FT-387), for every workflow that moves artefacts between jobs:

- **Artefacts go to `$RUNNER_TEMP`, not into the checkout.** The tree-walking checks (`check-licence-map`,
  `check-anonymisation`, `check-raw-read`) see every file under the working directory, so an archive or any other download
  placed in the checkout is scanned as if it were repository content. Download and extract into `$RUNNER_TEMP` (or an
  `actions/download-artifact` `path:` below it) and point the command at that path.
- **No Windows paths in `${{ }}` expressions of bash `run:` steps.** The runner substitutes the expression before bash
  parses the line, so `${{ github.workspace }}` or `${{ runner.temp }}` puts backslashes (`D:\...`) into the script, where
  bash reads them as escapes. Use the environment variables `RUNNER_TEMP`, `RUNNER_OS`, `GITHUB_WORKSPACE` and the other
  `RUNNER_*` and `GITHUB_*` ones (`"$RUNNER_TEMP/x"`), which bash expands as data.

A third lesson, from the cycle-25 delivery run (39914507884): `test-linux` builds the workspace twice (all features for
nextest, the default features for the doctests) in one `target`, and with full debug info the second build ran the runner's
disk dry ("No space left on device" while linking `ew-model`). The workflow env now sets `CARGO_PROFILE_DEV_DEBUG` to
`line-tables-only`, as every local build has since 2026-10-21; a job that adds a third feature set to one `target` must
check the free disk first.

`cargo xtask ci` runs fmt, clippy, nextest and the doctests (each with the default features and with `--all-features`;
the default-features nextest step is labelled `nextest (no acceptance, no fuzzer)` in the summary, and the acceptance
tests and the command fuzzer run only in the `nextest-all-features` step, as in the jobs) and the twelve repository checks (`check-deps`,
`check-features`, `check-raw-read`, `check-ordered-collections`, `check-disallowed-methods`, `check-build-scripts`, `check-vet-imports`, `check-workflows`, `check-licence-map`,
`check-anonymisation`, `check-governance-files`, `docs-gen --check`) in sequence and prints a summary: the local
mirror of the Rust part of the PR set. `check-dco` needs a revision range and is run on its own.

The licence-boundary check that 18 §5.2 lists under CI-PR-09 is part of `check-licence-map` and therefore runs in
`legal-docs`, next to the licence map it shares its policy file with; the dependency-rule part of the same
boundary (no permissive crate depends on a GPL workspace crate, `ew-import-qet` confinement) stays in `check-deps`
in `supply-chain`.

The MSRV step builds the permissive crates against a lockfile that the MSRV-aware resolver generates inside
the job (18 §6.2), so the committed application lockfile can move ahead of the MSRV:

```sh
CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback cargo +1.85.0 generate-lockfile
cargo +1.85.0 check --locked -p ew-units -p ew-ids -p ew-mltext -p ew-rds -p ew-schema -p ew-format \
  -p ew-interop-api -p ew-dl -p ew-step-proto -p ew-memo -p ew-plcaddr -p ew-caex -p ew-devdesc -p ew-ewpp -p ew-testdir
git checkout Cargo.lock   # locally: restore the committed lockfile afterwards
```

The MSRV 1.85 applies to the permissive crates only, which are the ones the MSRV step checks. The GPL application crates follow the pinned toolchain of `rust-toolchain.toml`: `ew-export` and `ew-render-cpu` (krilla 0.8 and vello_cpu 0.3 need Rust 1.92) and every workspace crate that depends on them (`ew-api`, `ew-print`, `ew-testkit`, `ew-ui`, `ew-viewmodel` and the binaries `ew` and `elektrawatt`) declare `rust-version = "1.92"` instead of the workspace value.

## Nightly performance and memory gate (`perf.yml`, `cargo xtask perf-gate`)

WP-S6.03 (NFR-PERF-20, 18 §4). The nightly workflow [`perf.yml`](../.github/workflows/perf.yml) (schedule and
`workflow_dispatch`, with a `quick` smoke input and a `classes` choice) runs `cargo xtask perf-gate`, which

1. builds and runs the `spike` and `cold_ops` benches of `ew-testkit` over the generated fixture classes
   (`--classes`, default `S,M` on the command line; **the nightly run measures class S only**, because the class M
   baseline does not exist yet and M takes over an hour in full mode; a manual dispatch can choose `S,M`)
   (full mode: 1,000 latency samples, median of 10 single runs), directly from their executables so that the
   process it watches is the bench itself; and the `canvas` bench of `ew-ui` (WP-S2.02, 18 §4.2) once per gate run,
   class GP, report-only (its families never fail the gate; a crash or its 120-s tile wait running out is a bench
   failure, item 4);
2. compares the **medians** and the deterministic memory estimates the benches print (model rows, undo history,
   memo values and dependency entries) with [`perf-baseline.toml`](perf-baseline.toml): a value more than 15 %
   above its baseline is a regression (WP-S6.03), unless the change is below a noise floor of the file:
   `noise_floor_ms` (0.05 ms) for the medians, `noise_floor_bytes` (16 MiB, 18 §4.4) for the peak RSS; the memory
   estimates and the entry counts are deterministic computations and have no floor, so a 15 % growth of a 2 MB
   estimate is a regression. The report's "Baseline:" line prints both floors. A baseline value
   the run did not measure (a removed family) is listed under "Missing from the run" and a value the baseline
   does not have under "New in the run"; under enforcement a missing value fails the gate (re-bless), a new one
   never does;
3. reads the peak RSS of the bench process (`VmHWM`, polled every 50 ms; Linux only, elsewhere the report names the
   gap until `ew-sysprobe` exists, FT-89);
4. writes `target/perf/report.md` (verdict, bench failures, regressions, missing and new values, medians against
   the baseline, memory, the engine statistics of the memo store: hit, ver, push_skip, re, incr, cut) and
   `target/perf/baseline.candidate.toml` (of what was measured). **A bench that does not run** (a build failure,
   a non-zero exit such as a panic, output that cannot be read) does not stop the others: it is recorded in a
   "Bench failures" section with its command and the last 20 lines of its output, the other benches are still
   compared, and the command exits non-zero whatever `EW_PERF_ASSERT` or `--enforce` say (an infrastructure
   failure, not a performance verdict); `--bless` refuses to write a baseline in that case;
5. [`ci-times.py`](ci-times.py) appends the CI-time trend (FT-269): the per-job durations of the last runs of
   `ci.yml` from the GitHub jobs API (at most 10 calls), set against the 30-minute budget of NFR-MAINT-05. The
   hosted runners share egress addresses, so the unauthenticated limit of 60 calls an hour is often used up: the
   step passes the automatic, read-only job token (`GITHUB_TOKEN: ${{ github.token }}` with `actions: read` on
   the job; no configured secret), which the script sends as `Authorization: Bearer <token>` when the variable
   is set and never prints.

The report is the job summary and the artifact `perf-report`. **The job is report-only on the hosted runners**:
they are not BM-class (DEC-034), so `EW_PERF_ASSERT` is not set and a regression does not fail the job; that
variable is the only switch that makes `perf-gate` exit non-zero (the timing rule of 18 §3.16), next to
`--enforce`. A baseline taken on another runner (`meta.runner`, `EW_PERF_RUNNER`, else `hosted:<os>-<arch>`) or in
the quick mode is reported as not comparable and never enforced. **Owner item:** the dedicated BM-class runner
(18 §4.3). When it exists, a job on it sets `EW_PERF_RUNNER=bm` and `EW_PERF_ASSERT=1`, and the baseline is
re-blessed there with `cargo xtask perf-gate --bless` (the generator version changes it as well, 18 §4.4). The committed baseline holds class S only, taken in full mode on a developer Windows machine (runner `dev-windows-x86_64`, so the hosted nightly reports "not compared"); the M class takes over an hour in full mode and has no baseline yet. Its `[meta]` records where the values come from: `generator_version` (the fixture generator version the benches report; a run with another version is "not compared"), `commit` (the short hash it was blessed at), `toolchain` (from `rust-toolchain.toml`), `date` (UTC) and `noise_floor_bytes`. `--bless` writes all of them; every field is required when the file is read.

Locally: `cargo xtask perf-gate --dry-run` prints the plan; `--quick --classes S` is the smallest real run (a
smoke run, no evidence); `--from <captured bench output>` re-reports one captured output without running anything (one
file per flag; repeat the flag for more files).
`python3 -I ci/ci-times.py --dry-run` lists the API calls; `--selftest` checks its arithmetic offline.

## Nightly command fuzzer (`fuzz.yml`, the test `fuzz_long`)

WP-S6.02, FT-272 and FT-279, decision log ID-388 (doc 18 §3.6, CI-N-02). The nightly workflow
[`fuzz.yml`](../.github/workflows/fuzz.yml) (schedule and `workflow_dispatch`, with `seeds`, `steps` and
`budget_minutes` inputs) runs the ignored test `fuzz_long` of
[`ew-testkit/tests/command_fuzz`](../crates/ew-testkit/tests/command_fuzz): `EW_FUZZ_SEEDS` seeds (a comma list or
ranges such as `1..=40`; the schedule uses 12 seeds from a range that starts at the day number, so every night
explores new streams) of `EW_FUZZ_STEPS` (default 200 in the workflow) random steps over Golden A. Every one of
the registered commands is generated (the test `every_registered_command_has_a_generator_arm` fails when a
command has no arm), next to undo and redo, save-and-reload round trips and strict and lenient CSV table imports.
Oracles: the structural check after every changing step, the memo oracle, the prevent rule, `verify_derived`, the
validity oracle of each command (a well-formed step must commit, a malformed one must be refused and change
nothing, the checks of what the command created), the equality of the model and of the derived families after a
reload, and the closing undo-all with its own save and reload. The nightly body is this seeded test, on the stable
toolchain without a sanitizer; the same test under AddressSanitizer and the cargo-fuzz form of the generator (the
target `command_sequence` of [`fuzz/`](../fuzz)) run in the
[deep nightly](#deep-nightly-deep-nightlyyml-ci-n-02-and-ci-n-03) (FT-297, FT-304, FT-272 closed).

The job builds and runs in the **release profile with debug assertions and overflow checks on**
(`CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS`, `CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS`): measured on a developer
machine, a step takes about 1.4 s there against 10 s in the dev profile, and a seed of 150 steps takes about
200 s. The timeout is sized from that: `budget_minutes` (default 75) stops new seeds from starting (the
test then ends with `status: budget`, which is not a failure), the run step is bounded by `timeout` at the budget
plus 45 minutes, and the job at 180 minutes.

**Red or green**: the job is red for a real finding (a panic, an oracle or invariant violation, a round-trip
mismatch) and for what would leave a night without coverage or hide a hang: a build that fails, a seed that was
running when the run step ended (a hang bounded by the step's `timeout`, a stack overflow, an out-of-memory
abort; a seed takes about four minutes, so a seed in flight at the timeout is a hang, and a hang is a finding),
and a run that ends without a verdict line. The test writes the findings file `target/fuzz/findings.md` first
(`status: running seed N` before each seed, then `running`, `complete` or `budget`, the line `FINDINGS: n`, each
finding with its seed, step, oracle and the command log in the `ew.cmdlog/1` format) and rewrites it after every
seed; a seed that fails does not stop the others. The workflow uploads the file, the parameters and the log as
the artifact `fuzz-report` (`if: always()`), copies the report into the job summary, and the verdict step turns
the cases above into a failure; it is green only for `complete` or `budget` with `FINDINGS: 0`. The run step has
its own `timeout` (budget plus 45 minutes), shorter than the job's, so that the verdict step always runs. Same
hardening as the other workflows: pinned actions, read-only token, no secrets.

Locally: `EW_FUZZ_SEEDS=17 EW_FUZZ_STEPS=200 EW_FUZZ_REPORT=target/fuzz/findings.md cargo test --release -p ew-testkit
--test command_fuzz fuzz_long -- --ignored --nocapture` (or in the dev profile for the debug checks, about seven times
slower); the cadences of the costly oracles are `EW_FUZZ_SAMPLE_EVERY`, `EW_FUZZ_CHECK_EVERY` and
`EW_FUZZ_VERIFY_EVERY`, the step mix `EW_FUZZ_SAVE_WEIGHT` and `EW_FUZZ_IMPORT_WEIGHT`, the wall-clock budget
`EW_FUZZ_BUDGET_SECS`. `fuzz_smoke` (2 seeds of 30 steps, 15 to 19 s in a debug build) stays in the targeted tests (`cargo nextest run -p ew-testkit`) and runs once per CI test job and in `cargo xtask ci`, in the all-features nextest run (the default-features run skips the `command_fuzz` binary: FT-306, decision log ID-437).

## Deep nightly (`deep-nightly.yml`, CI-N-02 and CI-N-03)

FT-297, FT-304 and FT-272, WP-S6.02, gate criterion G-M1-13 (doc 18 §3.6, §3.12, §5.2). The workflow
[`deep-nightly.yml`](../.github/workflows/deep-nightly.yml) (schedule 06:07 UTC, after `perf.yml` at 02:17, `fuzz.yml`
at 03:47 and `cli-artefact.yml` at 04:37, and `workflow_dispatch` with the inputs `fuzz_minutes`, `proptest_cases`,
`asan_seeds`, `asan_steps`, `asan_budget_minutes` and `asan_odr_level1`) has five kinds of job, which run in parallel on ubuntu-24.04, so
the workflow takes as long as the longest, below two hours:

| Job | What it runs | Budget |
|---|---|---|
| `full-oracle` | `cargo nextest run --release -p ew-api --all-features -E 'binary(memo_oracle) or binary(golden_b)' --run-ignored all`: the memo oracle over Golden A (`memo_oracle`: the `#[ignore]`d `oracle_finds_no_mismatch_over_every_state_on_every_read` re-runs **every** memoised read cold at every depth, where the pull-request test samples one in eight; the memoised-equals-cold sweep of every family and key) and over Golden B (`golden_b`: the script runs with the oracle on every read) | measured: build 6 minutes, run 3 minutes (24 cores, release); step bound 75 minutes |
| `proptest` | every test of `ew-model`, `ew-commands`, `ew-store`, `ew-format`, `ew-memo`, `ew-derive` and `ew-schema`, so every property test of them, with `PROPTEST_CASES=20000` (proptest's environment variable overrides the case count each test sets; the pull-request runs keep the 12 to 256 of the tests). The two hand-built runners of `ew-derive` (`memo_keys::tests::touched_covers_every_changed_stamp` and `incremental_partition_equals_cold`, the cold-versus-memo partition oracle, 12 cases per gate since the test clean-up of 2026-10-29) do not go through the `proptest!` macro, so they call `proptest::test_runner::contextualize_config` on their config and honour the variable too (checked when it ran 36 cases: 36 cases took 32 s in a debug build, `PROPTEST_CASES=300` 267 s). The `proptest-regressions` files a failing property writes are uploaded | measured: 2,000 cases take 108 s with four test threads on a 24-core developer machine (the slowest test, `ew-store::incremental incremental_save_equals_full_save`, 82 s); the cold-versus-memo partition oracle takes 198 s at 2,000 cases in release (the other tests of `ew-derive` are fast), so 20,000 cases are about 33 minutes on that machine and the job is estimated at 40 to 55 minutes on the hosted runner (that test and `incremental_save_equals_full_save`, about 14 minutes, run in parallel); step bound 95 minutes |
| `fuzz (jsonl_loader)`, `fuzz (migration_chain)`, `fuzz (command_sequence)` | `cargo fuzz run <target> target/corpus/<target> corpus/<target> -- -max_total_time=1800 -max_len=<n> -timeout=120 -rss_limit_mb=4096` in `fuzz/`, with cargo-fuzz 0.13.2 (`cargo install --locked`: taiki-e/install-action has no manifest for it) on the nightly of `fuzz/rust-toolchain.toml`, AddressSanitizer on (cargo-fuzz's default); the seeds are the committed corpus, the working corpus is `fuzz/target/corpus/<target>` | 30 minutes each (CI-N-03's "at least 30 minutes per parser target"); step bound `fuzz_minutes` + 20, job 90 minutes |
| `asan-command-fuzz` | the test `fuzz_long` of `ew-testkit` (the nightly test of `fuzz.yml`), built with `RUSTFLAGS=-Zsanitizer=address` for `x86_64-unknown-linux-gnu` on the same nightly (`cargo +nightly-… test --release --target x86_64-unknown-linux-gnu -p ew-testkit --test command_fuzz`; `--target` keeps build scripts and proc macros out of the sanitized flags), with the findings file and the seeds and steps of `fuzz.yml` (another seed range: offset 50 from the day number; 100 steps, because a seed of 200 steps would take about 30 minutes under ASan), in the release profile with debug assertions and overflow checks on (FT-304, CI-N-02's "command fuzzer 1 h with ASan") | build about 15 minutes, no new seed after 55 minutes (`EW_FUZZ_BUDGET_SECS`), a seed in flight finishes within about 16 (measured on WSL2: 2 seeds of 30 steps take 89 s plain and 684 s under ASan, 7.7 times as long; a seed of 100 steps is about 15 minutes), step bound 100 minutes, job 115 |
| `gpu-parity` | `cargo nextest run --locked --profile ci -p ew-ui -E 'binary(gpu)' --run-ignored only`: the `#[ignore]`d GPU canvas tests, the dense A3 parity of B2 (`b2_the_dense_a3_page_matches_the_cpu_raster`; the folio-103 parity stays per pull request), debug profile, on Mesa's lavapipe with `EW_REQUIRE_GPU_ADAPTER=1` (test audit, 2026-10-29) | build about 10 minutes (not measured), run under a minute; job 60 |

**The nightly toolchain.** FT-304 asks for the nightly toolchain "for that job only, pinned like the stable one":
`nightly-2026-10-08` (rustc 1.101.0-nightly), named in `fuzz/rust-toolchain.toml` (rustup applies it to every command
run inside `fuzz/`) and in `NIGHTLY_TOOLCHAIN` of the workflow, and installed by `dtolnay/rust-toolchain` at the commit
`ci.yml` pins. A test of `xtask` (`workflows::tests::the_nightly_workflows_pin_the_toolchains_of_the_repository`) keeps these dates equal, in `deep-nightly.yml` and in the `fuzz-smoke` job of `ci.yml`, and the stable toolchain of both workflows equal to `rust-toolchain.toml`'s. The date is advanced by the quarterly
upgrade train (doc 18 §6.3). The root `rust-toolchain.toml` names the stable toolchain, which the nightly jobs do not
install, so each of them sets `RUSTUP_TOOLCHAIN` (rustup takes it before the file; the test checks it too) and the ASan job
also names the nightly on the command line (`cargo +nightly-…`). `ASAN_OPTIONS=detect_odr_violation=0` is set for
the sanitized test binary (cargo-fuzz sets it itself). The ODR report for `aiOff` at start-up is a false positive of the
bundled SQLite's C code, not a second copy (one `libsqlite3-sys` 0.38.2 in both lock files, one `sqlite3_open` in the binary):
`sqlite3.c` has two identical function-local `aiOff` tables (FTS3 and FTS5 case folders, lines 212454 and 267267), the linker
merges them into one address and the check sees two globals there.

**The ODR level-1 trial (FT-343).** The dispatch input `asan_odr_level1` (boolean, default off) runs `asan-command-fuzz`
with `ASAN_OPTIONS=detect_odr_violation=1`: level 1 reports only size mismatches, so SQLite's two equal `aiOff` tables stay
silent while a real second SQLite copy of another version would still be caught (doc 18 §3.6). The input reaches the job as
an environment value (`ASAN_OPTIONS: ${{ inputs.asan_odr_level1 && 'detect_odr_violation=1' || 'detect_odr_violation=0' }}`,
one of two literals, nothing interpolated into a script) and only the test step of that job reads it; the nightly and a
dispatch without the input keep `=0`. The parameters file of the job's artifact records the value, and the job's verdict is
the trial's: an AddressSanitizer report (`ERROR: AddressSanitizer: odr-violation`) is red and printed in the job summary. If a
dispatch run is clean, adopting level 1 for the nightly is a change of the workflow-level `ASAN_OPTIONS` and of this text. A
test of `xtask` (`workflows::tests::the_odr_trial_is_a_dispatch_input_that_defaults_to_off`) keeps the default off.

**Red or green.** As `fuzz.yml`: red only for a real finding (a failed test or oracle, a crash, an out-of-memory, a
timeout of one input, a leak, an AddressSanitizer report), a failed build and a hang. Every job writes a status file
first (`running`) and rewrites it when the run ends; a status that is still `running` is a hang or an abort. The fuzz jobs
read the crash artifacts (`crash-*`, `oom-*`, `timeout-*`, `leak-*`; a `slow-unit-*` is not a finding) and the closing
`Done n runs` line of libFuzzer; `asan-command-fuzz` has `fuzz.yml`'s verdict (the `FINDINGS: n` line, `running seed N`,
a missing report) and reads an `ERROR: AddressSanitizer` in the log. Every job copies its report into the job summary and
uploads it as an artifact with `if: always()` (`deep-oracle-report`, `deep-proptest-report`, `deep-fuzz-<target>` with the
crash inputs, `deep-fuzz-corpus-<target>` with the grown corpus for minimisation, `deep-asan-fuzz-report`). Same hardening
as the other workflows: pinned actions, read-only token, no secrets, dispatch inputs through the environment.

**Triage of a finding.** Reproduce it (`cd fuzz && cargo fuzz run <target> <crash file>`; for the ASan test the command in
the findings file), commit the input under `fuzz/regressions/<target>/` with a failing-case description in the report, and
open a follow-up task; the fix is a separate change that also turns the input into a plain test of the crate. The change
that builds a target does not fix what it finds (`fuzz/README.md`).

**The fuzz crate.** [`fuzz/`](../fuzz) is not a workspace member (the root manifest lists it under `exclude`, and it has its
own `[workspace]` table): the nightly-only, sanitizer-instrumented build stays out of `cargo build --workspace`, `cargo xtask
ci` and the supply-chain jobs (cargo-deny, cargo-vet and the dependency checks of `xtask` read the workspace metadata, so they
do not see it; its dependencies are `libfuzzer-sys` with `arbitrary` and `cc`, and the workspace's own crates; the job
`fuzz-deny` of the deep nightly runs `cargo deny --manifest-path fuzz/Cargo.toml --config fuzz/deny.toml check licenses advisories`
over them, report-only: a finding is a warning in the job summary, not a red night). The pull-request `lint` job formats, checks and
lints the crate on the stable toolchain (`cargo fmt|check|clippy --manifest-path fuzz/Cargo.toml --locked`; rustup takes the
toolchain from the working directory, so the root's stable one applies, not `fuzz/rust-toolchain.toml`; libfuzzer-sys's build
script needs a C++ compiler, not a nightly), and `cargo xtask ci` has the steps `fuzz-fmt`, `fuzz-check` and `fuzz-clippy`. Its
`Cargo.lock` was seeded from the root one. The licence map has a rule for it (`fuzz/**` GPL-3.0-or-later; the lock file, the
toolchain file, the corpora and the regressions CC0-1.0; the README CC-BY-4.0). `cargo xtask check-workflows` covers the
workflow, `reuse lint` and `check-licence-map` the files. Locally, from `fuzz/`: `cargo test --release` replays the seed
corpora through the targets' checks (`tests/replay.rs`), `cargo run --release --bin seed_corpus` regenerates them, and
`cargo fuzz run <target> target/corpus/<target> corpus/<target> -- -max_total_time=60` runs one target for a minute (see
[`fuzz/README.md`](../fuzz/README.md) for the targets, their properties and their input formats). The local runs of FT-297,
all with AddressSanitizer on WSL2 and without a finding: `jsonl_loader` 65,544 inputs in 7 minutes (about 155 a second),
`migration_chain` 637 in 7 minutes (about 1.5 a second: every input opens the 842 KB golden project twice) and
`command_sequence` 151 in 60 seconds (about 2.5 a second).

## Fuzz smoke (CI-PR-08)

FT-335, WP-S6.02, gate criterion G-M1-13 (doc 18 §3.12, §5.2). The job `fuzz-smoke` of [`ci.yml`](../.github/workflows/ci.yml)
runs 60 s of each cargo-fuzz target of [`fuzz/`](../fuzz) that a change can reach. The same targets run 30 minutes each in the
[deep nightly](#deep-nightly-deep-nightlyyml-ci-n-02-and-ci-n-03), with the same corpus and limits.

**Path filter.** [`fuzz-smoke-targets.py`](fuzz-smoke-targets.py) reads the changed paths and prints the targets as
`<target>:<max_len>` words (stdlib Python; `--all`, `--selftest`). The job takes the changed paths of a pull request against
`github.event.pull_request.base.sha`, of a push against `github.event.before`, and runs every target for a dispatch and for a
push without a usable base (a new branch). The mapping is the **path-dependency closure** of each target:
[`fuzz-smoke-targets.toml`](fuzz-smoke-targets.toml) lists, for each target, its `-max_len`, its `roots` (the direct path
dependencies of `ew-fuzz` that its code uses) and the `closure` of those (every crate whose source the target is built from:
`jsonl_loader` six crates, `migration_chain` eight, `command_sequence` forty, because it runs through the `ew-api` facade). A
test of `xtask` (`workflows::tests::the_fuzz_smoke_table_is_the_path_dependency_closure_of_each_target`) recomputes each closure
from the `Cargo.toml` files of the crates and fails when the file differs, when a direct dependency of `fuzz/Cargo.toml` is
the root of no target, or when the targets differ from the `[[bin]]` targets of `fuzz/`, so a new dependency cannot leave the
filter behind. The filter job itself needs no cargo and no nightly (Python's `tomllib` reads the table).

| Changed path | Targets |
|---|---|
| `crates/<name>/` (the source, `Cargo.toml`, `build.rs`, data) | every target whose closure holds `<name>` |
| a crate's `tests/`, `benches/`, `examples/` and Markdown files | none (not built into a target); the exception is `crates/ew-testkit/tests/command_fuzz/`, the seeded command generator that `fuzz/` includes as modules: `command_sequence` |
| `fuzz/fuzz_targets/<t>.rs`, `fuzz/corpus/<t>/`, `fuzz/regressions/<t>/` | `<t>` |
| anything else in `fuzz/` (the harness in `src/`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `tests/`), `ci/fuzz-smoke-targets.py` and `ci/fuzz-smoke-targets.toml` | all three |
| `fuzz/README.md`, `fuzz/deny.toml`, `fuzz/target/`, `fuzz/artifacts/`, `fuzz/coverage/` | none |

A change that reaches no target runs the filter's self-test and the selection, then nothing: the job stays in the set of the
required checks and costs seconds. `ci.yml` itself is not an input (a CI change would otherwise cost a build on every
edit); exercise the job after changing it with `workflow_dispatch` of `ci.yml`, which runs every target. The job has a
timeout of 45 minutes; the first build of the command target is the long one (the cache is saved on `main`, key `ew-fuzz`,
shared with the deep nightly).

**Toolchain and flags.** The nightly of `fuzz/rust-toolchain.toml` (`nightly-2026-10-08`), for this job only: `NIGHTLY_TOOLCHAIN`
at the workflow level for `dtolnay/rust-toolchain`, `RUSTUP_TOOLCHAIN` at job level (rustup takes it before the repository's
stable `rust-toolchain.toml`), cargo-fuzz 0.13.2 built from source (`CARGO_FUZZ_VERSION`). The job sets `RUSTFLAGS` empty (the
workflow's `-D warnings` would turn a lint that a newer nightly adds into a build failure) and switches on the debug assertions
and overflow checks of the release profile, as the deep nightly does (the oracles rely on them).

**Red or green.** Red for a finding (a `crash-*`, `oom-*`, `leak-*` or `timeout-*` input in `fuzz/artifacts/<target>/`: libFuzzer
writes a `timeout-*` when one input runs over 120 s, which is a finding), for a build that fails (no evidence), and for a run
that ended without a verdict (a target's exit code that is not 0, or missing). Exit code 124, the step's own `timeout 8m`
killing a target that libFuzzer's per-input `-timeout=120` did not stop, is a **no verdict** and so red, not a warning: a target
that is still running after the 8 minutes is a hang the oracle missed or a broken setup, and the job has no evidence. The verdict reads the exit code of each
target from `target/smoke/results.txt`, which the run step writes, so it does not depend on libFuzzer's output text. The `::error::` annotation of a finding names the target and the class of the input, the prefix of its file name (`crash`, `oom`, `timeout`, `leak`; FT-370), so a contributor without repository admin rights (the job log and the artifact need them) can tell the classes apart. The crash
inputs and the logs are uploaded as the artifact `fuzz-smoke` with `if: always()`, and the first kilobyte of an input is in the job
summary as base64. Triage as for the deep nightly: commit the input under `fuzz/regressions/<target>/`, report it, and fix it in
a separate change. Same hardening as the other jobs: pinned actions, read-only token, no secrets, `persist-credentials: false`
(the checkout takes the whole history, `fetch-depth: 0`, so that the base revision is there), event data through the
environment. The job is not a required check yet (FT-325 and FT-284, owner).

## Nightly CLI artefact (`cli-artefact.yml`, CI-N-12)

FT-299, WP-S6.02, gate criterion G-M1-22 (doc 18 §5.2). The nightly workflow
[`cli-artefact.yml`](../.github/workflows/cli-artefact.yml) (schedule and `workflow_dispatch`) builds the `ew`
command-line program with `cargo build -p ew --release --locked` on ubuntu-24.04 (x86_64), windows-2025 (x86_64)
and macos-15 (aarch64), runs `ew --version` and `ew --help` as a smoke test and uploads one artefact per
platform, named `ew-<os>-<arch>-unsigned-dev-<short sha>`. An artefact is a distribution of the GPL program, so it holds the program, the licence texts
(`LICENSES/GPL-3.0-or-later.txt`, `CC-BY-SA-4.0.txt`, `OFL-1.1.txt`), `NOTICE`, the third-party notices
(`THIRD-PARTY-NOTICES.txt`, `cargo about generate` with the pinned cargo-about 0.9.2 as in the supply-chain job)
and a `README.md` copied from [`cli-artefact-README.md`](cli-artefact-README.md) (it states that the archive is an
`unsigned development build, not a release`) with the commit, runner, toolchain and the SHA-256 of the program
appended. Retention is 90 days.

What it deliberately does not do: sign or notarise anything, create a tag or a release, or use a secret; the
artefacts are kept as CI-run artefacts for 90 days and are not a release. Publishing and signing are WP-S6.07
release items (the signing identities are the owner's). The three platform jobs do not stop each other (`fail-fast: false`). The upload is not `if: always()`:
a failed build leaves no artefact to keep, and the job summary step, which is, says so. The first scheduled run is
the evidence for G-M1-22's nightly part; the `m1` release-notes skeleton is
[`docs/release-notes/m1-draft.md`](../docs/release-notes/m1-draft.md).

## Release check (`cli-artefact.yml` / `release-check`, CI-R-03 before WP-S6.07)

FT-333 (doc 18 §3.5, §5.2, §5.4; NFR-REL-07, NFR-SEC-20). The second job of
[`cli-artefact.yml`](../.github/workflows/cli-artefact.yml) runs the release-build assertions of CI-R-03 every night
before the release pipeline of WP-S6.07 exists, so that the release recipe is known to hold when the pipeline is
built. It is a job of the CLI-artefact workflow rather than a workflow of its own because that workflow is already
the nightly release-profile build on the same runner, with the same toolchain pin and hardening; the job runs in
parallel with the `ew` artefact jobs and does not hold them up.

What it does, on ubuntu-24.04 with `PKG_CONFIG=false` set for the job:

1. `bash ci/release-check.sh env`, in the same shell as the build: fails unless `PKG_CONFIG` is `false` and no
   `PKG_CONFIG_*`, `HOST_PKG_CONFIG*` or `TARGET_PKG_CONFIG*` variable is exported (the pkg-config crate prefers
   `PKG_CONFIG_<target>` and `HOST_PKG_CONFIG` to `PKG_CONFIG`), unless `ICU4X_DATA_DIR` and `CARGO_ENCODED_RUSTFLAGS`
   (which would replace `RUSTFLAGS` and drop the remap) are unset, and unless no `.cargo/config.toml` or `.cargo/config`
   of the checkout or the cargo home has an `[env]` table or `env.` key (cargo hands those to the build scripts; there
   is none today). The names come from the environment the shell passes on, so a name such as
   `PKG_CONFIG_x86_64-unknown-linux-gnu` is caught too.
2. `cargo build -p elektrawatt --release --locked` with `RUSTFLAGS` remapping the cargo home to `/cargo`, the rustup
   home to `/rustup` and the checkout to `/build`: without it the panic locations of registry crates carry
   `/home/runner/.cargo/registry/…`. Cargo's `trim-paths` would do the same but is not stable on Rust 1.99.0.
3. `bash ci/release-check.sh scan target/release/elektrawatt target release-check-report.md`: fails on any build-host
   path in the binary (`strings -a`: `/home/`, `/usr/lib/x86_64-linux-gnu`, `/opt/`, and the checkout, cargo home,
   rustup home and runner temp directory), on a `NEEDED` entry for `libwayland*`, `libxkbcommon*`, `libX11*`,
   `libxcb*`, `libEGL*`, `libGL*` or `libvulkan*` (`readelf -d`; all are loaded with `dlopen`, and a hard link to any
   of them breaks NFR-PLAT-03's fallback chain; the `required_features` guard of `check-features` keeps
   `wayland-sys/dlopen` on), and on any `Some(..)` library directory in x11-dl's generated `config.rs` (or no such
   file). A legitimate run-time search-path string that a crate hard-codes can only pass through the script's reviewed
   exception list `ALLOWED_STRINGS` (exact whole lines, each with a comment naming the crate, the reason and the FT or
   decision-log ID; empty today). The report lists each pattern's hit count with the first hits, the `NEEDED` entries,
   `ldd` for information, and the verdict; it goes to the job summary and the artifact `release-check-report`
   (30 days).

It uploads the report only, never the binary: no binary that links the UI graph is distributed before decision log
ID-476's hold lifts. Report-only: no branch rule requires it; it is red on any hit, a failed build or a missing
x11-dl config. No secret, actions pinned by SHA, `permissions: contents: read`, `persist-credentials: false`
(`check-workflows`).

What the first run must settle: whether the hosted image exports any `PKG_CONFIG_*` variable (the environment step
fails then, and the release build must unset it); whether the remap leaves any `/home/` string (for example from a
build script that embeds an absolute path through `include!` or `env!`); whether any crate hard-codes
`/usr/lib/x86_64-linux-gnu` or an `/opt/` path as a run-time search path (a legitimate hit would need a reviewed
exception in `ALLOWED_STRINGS`, not a weaker pattern); that no `NEEDED` entry names a library of the deny-list;
and the job's cold build time against its 90-minute limit.

Locally (Linux; the script is not executable, so it runs through `bash`; without the remap the scan fails on the
registry paths under `/home/`):

```sh
export PKG_CONFIG=false
export RUSTFLAGS="--remap-path-prefix=${CARGO_HOME:-$HOME/.cargo}=/cargo --remap-path-prefix=${RUSTUP_HOME:-$HOME/.rustup}=/rustup --remap-path-prefix=$PWD=/build"
bash ci/release-check.sh env
cargo build -p elektrawatt --release --locked
bash ci/release-check.sh scan target/release/elektrawatt target report.md
```

## Headless container job (CI-PR-05)

FT-295, evidence for G-M1-01 (NFR-PLAT-04, NFR-REL-11). The job `headless` of [`ci.yml`](../.github/workflows/ci.yml)
builds `ew` (`cargo build -p ew --locked`, a debug build) and runs `ew export pdf golden/J-E-2416.ew` in a Linux
container, once with `--threads 1` and once with `--threads 4`. The two PDFs must be byte for byte the digest that
[`golden/J-E-2416/expected/pdf/sha256.txt`](../golden/J-E-2416/expected/pdf/sha256.txt) commits; the job fails
otherwise and uploads the PDFs as the artifact `headless-pdf`.

**Why a container, and why this image.** "No system fonts" and "no display or GPU libraries" are properties of the file
system the process sees, and a hosted ubuntu-24.04 runner has fonts, fontconfig and X and GL libraries on it. An
empty `HOME` and `unshare --net` would hide the user's directories and the network, but not the system fonts, so a
digest that equals the committed one would not show that `ew` uses the bundled faces only. In a container whose image
has no font file at all, an export that reached for a system font would find none and the digest would differ. The
image is `ubuntu:24.04`, pinned by the digest of its image index (`HEADLESS_IMAGE` in `ci.yml`, resolved
2026-10-22; update the tag and the digest together). It has the same glibc (2.39) as the runner that builds `ew`, so the
dynamically linked binary, mounted read-only into the container, runs unchanged; a `debian:12-slim` or distroless
image has glibc 2.36, and a binary built on the runner may need newer symbols. A statically linked musl build would
remove the dependence, at the price of a second target and of a build that is not the shipped one; it is not decided.

**The container.** `docker run --network none --read-only --tmpfs /tmp --cap-drop ALL --security-opt
no-new-privileges`, with only three capabilities added back (`DAC_READ_SEARCH`, `SETUID`, `SETGID`: below), no
`--device` (so no `/dev/dri`) and no `--gpus`; the mounts are the `ew`
binary, the golden project (`golden/J-E-2416.ew`, read-only, which also shows that the export writes nothing into the
project) and [`ci/headless-export.sh`](headless-export.sh), all read-only, and the output directory. The script runs
inside and first proves that the container is bare, so that a change of image or flags cannot weaken the job unseen:
`DISPLAY` and `WAYLAND_DISPLAY` unset; no `/dev/dri`, `/dev/nvidiactl`, `/dev/nvidia0`, `/dev/kfd`; no font file
(`*.ttf`, `*.otf`, `*.ttc`, `*.otc`, `*.pfb`, `*.pfa`, `*.woff*`, `*.pcf*`) anywhere on the image, no
`/usr/share/fonts`, `/etc/fonts`, `fc-list` or `fc-cache`; the user directories (`HOME`, `APPDATA`,
`XDG_STATE_HOME`, `XDG_DATA_HOME`, `XDG_CONFIG_HOME`) point at empty directories. The export then runs under `env -i`,
so there is no `EW_CONTENT_DIR`, no `SOURCE_DATE_EPOCH` and no locale. The same property is held in process by
`bin/ew/tests/cli.rs` (`export_pdf_of_golden_a_is_the_committed_pdf`) and by AC-15 of the acceptance runner; the job
adds the file system the process sees.

The checks run as root with `DAC_READ_SEARCH`, so that the font search reads every directory of the image, and the
script fails when `find` writes anything to stderr (an unreadable directory skipped silently would leave a hole in
the proof); the export then drops to the runner's uid and gid with `setpriv` (`SETUID`, `SETGID`; `EW_UID`, `EW_GID`),
so that the PDFs are the runner's and the export itself holds no capability.

**What it does not cover.** `ew` is the debug build of the job's own toolchain, not a release artefact (the PDF is
independent of the profile; the release pipeline has its own checks, CI-R). The kernel and the CPU are the runner's.
`ew render --page 103 --png` (FT-254) is not part of the job because the command does not exist yet. The equality of
this PDF with the kittest GUI export of CI-PR-07 (FR-OUT-022) waits for CI-PR-07.

Locally (Linux with docker): build `ew`, then run the `docker run` of the job with `--volume` mounts for the three
files and a writable output directory, and compare `sha256sum` of the two PDFs with `sha256.txt`.

## veraPDF job (CI-N-09)

FT-294, evidence for G-M1-02 (AC-13). The doc 18 catalogue lists CI-N-09 as a nightly; it runs as the job `verapdf` of
[`ci.yml`](../.github/workflows/ci.yml) on every pull request and push to `main`, in parallel with the other jobs,
so that a PDF/A regression is caught where it is made. It is in `ci.yml` and not in `docs.yml`, because `docs.yml` also
fires on edited, labeled and unlabeled events with cancel-in-progress (a label change would cancel and rebuild the
job) and because the CI-time trend of `ci-times.py` reads the jobs of `ci.yml`. It costs one `ew` build and a 53 MB image pull (about 5 minutes
expected; not measured before the first runs) and does not lengthen the 30-minute wall time of the test jobs.

The job builds `ew`, exports the golden PDF in three variants (all 36 pages with the model's date; the same dated by
`SOURCE_DATE_EPOCH=1700000000`; `--pages 101-106,901`), checks that the first is the committed digest, and runs
veraPDF 1.30.3 over the three files with `--flavour 2b --format xml`. veraPDF is the project's own CLI image
`verapdf/cli`, **pinned by the digest of the image manifest** (`VERAPDF_IMAGE` in `ci.yml`, resolved 2026-10-22 from the
Docker Hub tag `v1.30.3`; update the version and the digest together), run with `--network none --cap-drop ALL` and the
PDFs mounted read-only. The Java runtime is inside the image, so the runner's JDK does not matter. The image rather
than the installer archive, because the project publishes a GPG signature for the archive and no SHA-256, and a digest
is the checksum of exactly the bytes that run.

The verdict is [`ci/verapdf-check.py`](verapdf-check.py) over the XML report, not veraPDF's exit status (which the
log prints): the job fails unless there are exactly three jobs, each validated against a PDF/A-2B profile and
`isCompliant="true"`, with no file that failed to parse, was encrypted, ran out of memory or hit a veraPDF exception.
The first failed rules of a non-compliant file are printed, and the verdict is the job summary. The report is the
artifact `verapdf-report` (14 days, with the three PDFs).

**AC-13 reads the result.** The acceptance runner's entry "veraPDF cross-check of PDF/A-2b"
(`crates/ew-api/tests/acceptance/checks.rs`) reads the report when `EW_VERAPDF_REPORT=<report.xml>` is set and applies
the same rules (pass, or fail with the reasons); the milestone run takes the artifact of the job:
`EW_VERAPDF_REPORT=verapdf-report.xml cargo xtask acceptance --milestone <report.md>`. Without the variable the entry
stays `not_applicable` and its text names the job, which is what the committed headless report
(`golden/J-E-2416/expected/acceptance_headless.md`) holds, so the job does not run the runner (with the variable set
the entry would pass and the committed report would differ). `python3 -I ci/verapdf-check.py --selftest` and the test
`the_verapdf_entry_reads_the_report_of_the_job` check the two readers on the same cases.

Since FT-436 the job also prints every decodable file of the real-exporter raster corpus
(`crates/ew-export/tests/fixtures/raster-exporters/`) on a page of its own through the test
`every_decodable_raster_file_prints_in_a_pdf_a` with `EW_RASTER_PDF_DIR`, validates the folder with the same image and
flags, and gives the second report its own verdict (`--expect-jobs 8`, the eight retained files); the reports are in
the same artifact. GIMP's public-domain ICC profile remains covered; real-exporter Adobe/HP ICC profiles, PNG iCCP
and grey JPEGs remain follow-up gaps after excluding files whose embedded-profile licences were not verified.

## Coverage job (CI-PR-13)

FT-296, G-M1-15 (S criterion: a dated plan). The workflow [`coverage.yml`](../.github/workflows/coverage.yml) runs
weekly (Sunday 03:43 UTC) and on `workflow_dispatch`, not per pull request: cargo-llvm-cov 0.9.1 (installed by
`taiki-e/install-action`, which verifies the checksum) runs the **workspace** tests with all features under nextest
(the acceptance and fuzz tests exercise `ew-commands` and `ew-derive` from outside, so they count) with
instrumentation, and [`ci/coverage-summary.py`](coverage-summary.py) reads `cargo llvm-cov report --json
--summary-only` and reports the line coverage of `crates/<name>/src/` for `ew-model`, `ew-commands`, `ew-memo`,
`ew-derive`, `ew-format` and `ew-store` against 80 % (NFR-MAINT-06), per crate and in total. The report is the job
summary and the artifact `coverage-report` (90 days).

**Report-only.** A crate below 80 % is listed and does not fail the job. The job is red only when the tool fails:
cargo-llvm-cov or the report generation fails, or the summary holds no instrumented line of a core crate (the run did
not measure it). A test that fails under instrumentation is a warning annotation and a note in the summary, not a
red job (the test jobs of `ci.yml` gate the tests). The owner sets the dated plan that turns the threshold into a gate;
the readiness report's row G-M1-15 stays open until then. The first run has no number to compare with, and the
duration is not known (the timeout is 150 minutes).

## Feature bans (`cargo xtask check-features`)

CI-PR-09 requires that test-only and unsafe features never reach a non-test target. `[[banned_features]]` in
[`dep-rules.toml`](dep-rules.toml) lists `{ crate, feature, allowed_kinds }` (`ew-ids/testkit` and
`ew/test-confirm` on dev edges only; rhai `unchecked` and egui `inspection` nowhere) and `banned_crates` lists
crates that may only be dev-dependencies (`egui_mcp`). The check reads the declared dependencies of every
workspace package from `cargo metadata` and fails when a banned feature is requested on an edge whose kind
(`normal`, `dev`, `build`) is not allowed, when a banned crate is a non-dev dependency, or when a package's own
`default` feature forwards a banned feature (`default = ["ew-ids/testkit"]`, also through other local
features). Forwarding is followed transitively through the workspace features (`dep/feature`, `dep?/feature`
and local features): the edge rule also applies to a feature that reaches a banned one (a normal edge that
requests `ew-derive/raw` fails when `raw = ["ew-model/raw-read"]`). The message names the package, dependency,
feature and kind, and prints the chain of features down to the banned one. Features that a dependency enables
by default are out of scope here; they would have to be named on the edge by the dependent crate.

A `[[banned_features]]` entry may also list `allowed_packages`: the named workspace packages may enable the
feature on a dependency edge of any kind and may forward it from any of their own features, while any other
package with a `[features]` entry (not only `default`) that reaches it, directly or through other workspace
features, is reported as a back door. The untracked read API of `ew-model` (`raw-read`, ARCH §3 rule 1) is
banned this way: dev edges anywhere, normal edges only in `ew-derive`, `ew-commands` and `ew-store`.

## Ordered collections (`cargo xtask check-ordered-collections [--bless]`)

FT-271, WP-S6.02, NFR-MAINT-12: the iteration order of a hash container must not reach a persisted, exported or
printed sequence. As built, the ban is workspace-wide (not only in the output crates) and has two halves:

1. **Clippy.** The generated [`clippy.toml`](../clippy.toml) (`cargo xtask check-raw-read --bless` writes it, so the
   list lives in `xtask/src/raw_read.rs`, `DISALLOWED_TYPES`) has a `disallowed-types` list: `std::collections::HashMap`
   and `HashSet`, and `imbl::HashMap` and `HashSet`. Both clippy runs of the `lint` job and of `cargo xtask ci` fail on
   a use. `hashbrown` is not listed: no workspace crate names it. A use whose order provably never reaches output (a
   lookup-only map, a counter, a set used for `contains`) carries `#[allow(clippy::disallowed_types)]` with a
   one-line `//` comment above it (or a `reason = "..."` in the attribute); anything else uses `BTreeMap` or
   `BTreeSet` (or `IndexMap` where the crate has it).
2. **The review list.** The check scans the Rust source of every workspace package (tests and benches too) for
   `allow`, `expect` and `cfg_attr` attributes that name `disallowed_types` or a group that contains it
   (`clippy::style`, `clippy::all`, `warnings`), prints the sites per crate and compares
   them with [`ordered-collections-allow.toml`](ordered-collections-allow.toml). A site is keyed by its crate and
   item path (`fn parse > let seen`, `struct State > field holds`, `use std::collections::HashSet`), never by file
   or line, with a count, so moving code does not touch the list and a new allow always does. It fails for a site
   that is not in the list, for a stale entry, for a site without a justification, and for a manifest that sets
   `disallowed_types`, `style` or `all` (`[lints.clippy]`) or `warnings` (`[lints.rust]`) to `allow`. `--bless` rewrites the list after the review; a test of
   `xtask` (`ordered::tests::the_committed_list_is_current`) keeps it current in `cargo nextest run -p xtask`.

At the introduction (cycle 18) nine sites in four crates allow the lint, all lookup-only: `ew-import-tables` 2
(the duplicate-column check of the table reader), `ew-schema` 2 (the pattern cache of a test), `ew-store` 3 (the
fault-injection holds of the in-memory file system) and `xtask` 2 (the denylist of the anonymisation guard). No map
or set whose order reached output was found. A use of a type alias is not linted, so an alias of a hash container
bypasses the ban at every use: the one alias (`Denylist` of `xtask/src/anonymise.rs`) is private to its module and
listed in the allow file like any other site; a public alias would need the same review.

## Wall clock, environment and randomness (`cargo xtask check-disallowed-methods [--bless]`)

FT-317, WP-S6.02, NFR-MAINT-12, doc 18 §3.5: model, derive, format and export paths must not depend on the wall
clock, the process environment or randomness. The check is the machinery of the ordered-collections check
(`xtask/src/ordered.rs`, one `Rule` each) with a second list, and has the same two halves:

1. **Clippy.** The generated [`clippy.toml`](../clippy.toml) lists, after the raw readers, `std::time::SystemTime::now`
   and `Instant::now`, `std::env::var`, `var_os`, `vars` and `vars_os`, `std::hash::RandomState::new` and
   `getrandom::fill`, `fill_uninit`, `u32` and `u64` (`DISALLOWED_METHODS` in `xtask/src/raw_read.rs`). `Uuid::new_v4`,
   `Uuid::now_v7` and the `rand` functions are listed with `allow-invalid = true` (clippy 1.99 accepts the key
   without a warning, in both clippy modes), because they do not resolve under the workspace's features: `uuid` has
   the `v5` feature only, and `rand` 0.10 without `thread_rng` is loaded by `proptest` alone. They bite once a crate
   enables a generator, and `check-features` bans `uuid/v4`, `uuid/v7`, `uuid/fast-rng` and `rand/thread_rng`
   outside dev edges (`dep-rules.toml`), so that does not happen unseen. `std::hash::RandomState` (the generator of
   hash iteration order) is also a `disallowed-types` entry; its one use, the lock's session ID, is a reviewed
   site of the ordered-collections list. A justified use carries
   `#[allow(clippy::disallowed_methods)]` with a one-line `//` comment above it (or a `reason = "..."`); an
   inner attribute at a test or bench target root is the usual form there. A new use that is not justified is
   replaced by an injected clock, an argument or a seeded generator.
2. **The review list.** The check compares the allow sites, keyed by crate and item path with a count, with
   [`disallowed-methods-allow.toml`](disallowed-methods-allow.toml), and fails for a site that is not in the list,
   for a stale entry, for a site without a justification, and for a manifest that sets `disallowed_methods`, `style`
   or `all` or `warnings` to `allow`. `--bless` rewrites the list after the review; `ordered::tests::the_committed_methods_list_is_current`
   keeps it current in `cargo nextest run -p xtask`.

The lint `disallowed_methods` is also the guard of the raw readers (`check-raw-read`, ADR-0001 amendment 2), and
the reader crates (`ew-model`, `ew-derive`, `ew-commands`, `ew-store`) opt out of it at their target roots, which
would switch this rule off in exactly those crates. For them the check ignores allow attributes and scans the source
for the calls (`SystemTime::now`, `env::var_os`, also inside a `use` group; the last two path segments, so an alias
is not seen) and lists each call as a site (`impl Clock for SystemClock > fn now > call SystemTime::now`), which
needs a `//` justification on the line above or on the line. In non-test source of a package that reaches `ew-model`,
an allow of the lint is also a suppression for `check-raw-read` and has an `[[raw_read.allow]]` entry in
[`dep-rules.toml`](dep-rules.toml) (`bin/ew`, `ew-sqlview`, `ew-testkit`) with a `count`: the allow is on a one-line
helper that wraps the one call (`source_date_epoch`, `content_dir`, `deadline_now`, `class_var`, `timer_start`),
never on a whole body, and a further allow in the file fails `check-raw-read`.

At the introduction (cycle 19) 43 sites in 18 crates are listed: `ew-ids` 1 (`SystemSource`, the one clock and entropy
source for identity), `ew-store` 8 (the `Clock`, the lock record's user, host and start time, the session ID, the
state directory, and the lock test's scratch names: none is project data, NFR-REL-07), `ew-sqlview` 2 (the monotonic
deadline of a query), `ew` 2 (`SOURCE_DATE_EPOCH` and `EW_CONTENT_DIR`; no wall-clock fallback for the creation
date), `ew-testkit` 4, `xtask` 1 (build tooling), and 26 in test and bench targets of other crates (timers asserted
only with `EW_PERF_ASSERT`, bless and diagnostic flags, scratch-directory names, the private-corpus path). No call
reaches project data or an export, except the sanctioned `SOURCE_DATE_EPOCH`; the lock record and the journal are
persisted by design. Not covered: `std::env::temp_dir` and
`current_dir`, and a call through an alias or re-export in a reader crate.

## Anonymisation guard (`cargo xtask check-anonymisation [--require-entries]`)

Every committed file that is not binary is scanned: files with a UTF-16 byte-order mark are decoded as UTF-16,
all other files as UTF-8 with invalid sequences replaced (so Latin-1 text is still scanned); a file with a NUL
byte in its first 8 KiB is binary and skipped. While `denylist.sha256` has no entries the guard cannot match
anything: the check prints a `WARNING` line and still exits 0, because supplying the entries is an owner action
and must not block CI. `--require-entries` turns that warning into a failure; no workflow uses it yet, the
maintainers switch it on once the list is committed (gate G-M0-06).

## Supply-chain files (`check-build-scripts`, `check-vet-imports`)

`ci/build-scripts.allowlist` lists the packages that may have a build script, one `name@version` per line with an
optional `#` comment that states what the script does and does not repeat the version. The version is the exact
string cargo reports. A new version of a listed crate, or a new crate with a build script, fails
`cargo xtask check-build-scripts` until a reviewer has read the new `build.rs` (no network access, no download,
writes only under `OUT_DIR`). For a version bump, run `cargo xtask check-build-scripts --bless` (alias `--write`):
it re-versions the line in place, keeps its comment, drops entries that no package in the graph has any more, prints
what it changed and then runs the check. The commit message names the re-reviewed crates (the `re-versioned` lines
of the output). Bless never adds a crate: a new build-script crate needs a reviewed line written by hand, and the
check that follows bless fails on it until then.

`supply-chain/imports.lock` is written by `cargo vet` from the imported audit sets, together with the auditors'
names, e-mail addresses and free-text notes, which the public-repository rule keeps out of the repository.
`cargo xtask check-vet-imports` fails when the file has a `who`, `notes` or `user-name` key, a name in a `user-id`
comment (the login alone is allowed), or an e-mail address. After any `cargo vet` run that fetches imports, run
`cargo xtask check-vet-imports --bless` (alias `--write`): it scrubs the file, runs `cargo vet fmt` and checks
again.

## Policy files

| File | Checked by | Content |
|---|---|---|
| [`dep-rules.toml`](dep-rules.toml) | `cargo xtask check-deps`, `check-features` | Layers, the permitted crate-to-crate edges (the ARCH §3 graph), external-crate owners (rusqlite and `libsqlite3-sys`, egui, wgpu; glob patterns cover the hyphenated sub-crates such as `egui-winit` and `wgpu-core`), the importer rule and the `ew-import-qet` confinement (NFR-MAINT-01, ADR-0024); the banned features and crates of `check-features` (below) |
| [`build-scripts.allowlist`](build-scripts.allowlist) | `cargo xtask check-build-scripts` | Every package with a build script in the all-features dependency graph, one `name@version` per line, after review (ADR-0023, NFR-SEC-10); a version bump of a listed crate fails until its build script is re-read and the line is blessed (below) |
| [`../deny.toml`](../deny.toml) | `cargo deny check` | Licence allowlist (NFR-LIC-04), advisories, bans, sources |
| [`../supply-chain/`](../supply-chain) | `cargo vet` | Audits and the exemption list frozen at the first commit (ADR-0023), and the imported audit sets in `imports.lock`, which are kept without auditor names and e-mail addresses (decision log ID-4; ADR-0023 amendments 3 and 5; `cargo xtask check-vet-imports`, below) |
| [`licence-map.toml`](licence-map.toml) | `cargo xtask check-licence-map` | The path-to-licence map of 19 §2.1 (longest glob wins; unmapped paths fail), the files allowed to keep third-party copyright notices, where GPL-2.0 may appear, and the licence allowlist of the permissive layer's dependency closure (NFR-MAINT-02) |
| [`../REUSE.toml`](../REUSE.toml), [`../LICENSES/`](../LICENSES) | `reuse lint`, `cargo xtask check-licence-map` | Licence annotations for the files that cannot carry an SPDX header; the licence texts |
| [`denylist.sha256`](denylist.sha256) | `cargo xtask check-anonymisation` | SHA-256 digests of confidential word n-grams (n ≤ 3), never the tokens themselves (NFR-PRIV-09, 20 §3.5). Committed without entries: the maintainers add them (below); until then the check prints a warning and passes |
| [`../about.toml`](../about.toml), [`third-party-notices.hbs`](third-party-notices.hbs) | `cargo about generate` (CI-PR-09) | The cargo-about licence allowlist (the deny.toml list; the workspace's own `publish = false` crates are ignored) and the plain-text template of the third-party notice file (NFR-LIC-08). CI only generates the file, so a crate without a usable licence fails; the release pipeline (CI-R-07, WP-S6.07) ships it in every artefact |
| [`render-mermaid.py`](render-mermaid.py) | CI-PR-15 | Mermaid extraction and rendering |
| [`headless-export.sh`](headless-export.sh) | CI-PR-05 (`ci.yml` / `headless`) | Runs inside the headless container: proves that there is no display, GPU, font file or user directory content, then runs `ew export pdf` on one and four threads (FT-295) |
| [`fuzz-smoke-targets.py`](fuzz-smoke-targets.py) | CI-PR-08 (`ci.yml` / `fuzz-smoke`) | The path filter of the per-pull-request fuzz run: the changed paths to the cargo-fuzz targets whose dependency closure holds the changed crate, from `fuzz-smoke-targets.toml`; `--all`, `--selftest` (cross-checks the targets of `fuzz/Cargo.toml` and the matrix of `deep-nightly.yml`) (FT-335) |
| [`fuzz-smoke-targets.toml`](fuzz-smoke-targets.toml) | CI-PR-08, `xtask` test | The table of the path filter: each target's `-max_len`, roots and path-dependency closure; a test of `xtask` recomputes the closures from the `Cargo.toml` files (FT-335) |
| [`verapdf-check.py`](verapdf-check.py) | CI-N-09 (`ci.yml` / `verapdf`) | The verdict over a veraPDF XML report: PDF/A-2B, compliant, no failed parse, the job count; `--selftest` (FT-294) |
| [`coverage-summary.py`](coverage-summary.py) | CI-PR-13 (`coverage.yml`) | The line coverage of the six core crates from a cargo-llvm-cov summary, against 80 %, report-only; `--selftest` (FT-296) |
| [`check-book-links.py`](check-book-links.py) | CI-PR-10 (book) | Link check of the built user-documentation book: no network, no plugin to pin |
| [`ordered-collections-allow.toml`](ordered-collections-allow.toml) | `cargo xtask check-ordered-collections` | The reviewed sites that allow the hash-container ban of the generated `clippy.toml`, by crate and item path with a count (FT-271, WP-S6.02) |
| [`disallowed-methods-allow.toml`](disallowed-methods-allow.toml) | `cargo xtask check-disallowed-methods` | The reviewed sites that allow the wall-clock, environment and randomness ban, and the calls in the reader crates, by crate and item path with a count (FT-317, WP-S6.02) |
| [`perf-baseline.toml`](perf-baseline.toml) | `cargo xtask perf-gate` (nightly `perf.yml`) | The baseline medians and memory estimates per size class, the 15 % threshold, the noise floor and the runner they were taken on (WP-S6.03) |
| [`ci-times.py`](ci-times.py) | nightly `perf.yml` | The CI-time trend of `ci.yml` against NFR-MAINT-05 (FT-269) |
| [`../.github/workflows/fuzz.yml`](../.github/workflows/fuzz.yml) | nightly command fuzzer | The seeded `fuzz_long` run, with the findings file as artifact and a verdict step (red for a finding, a failed build or a hang) (WP-S6.02, FT-272) |
| [`../.github/workflows/deep-nightly.yml`](../.github/workflows/deep-nightly.yml) | the deep nightly (CI-N-02, CI-N-03) | The full oracle, the 20,000-case property run, the three cargo-fuzz targets and `fuzz_long` under AddressSanitizer, each with a status file and a verdict step (FT-297, FT-304) |
| [`../fuzz/`](../fuzz) | `cargo fuzz`, `deep-nightly.yml`, `fuzz/tests/replay.rs` | The cargo-fuzz crate (outside the workspace): the targets, the seed corpora, the committed regressions and the pinned nightly `rust-toolchain.toml` (FT-297) |

## Rules for the workflows

- Third-party actions are pinned by full commit SHA with a `# vX.Y.Z` comment; Renovate
  ([`.github/renovate.json`](../.github/renovate.json)) proposes updates weekly, grouped. Container images are
  pinned by digest: `check-workflows` fails on an `image:` or `*_IMAGE` value, and on a `docker run`, `docker pull` or
  `docker create` command, that names no `@sha256:<64 hex>` digest or `$..._IMAGE` variable. A job that pulls an image
  tries the pull three times with a back-off, and a pull that still fails fails the job: an evidence job never
  passes without its evidence.
- Rust caches (`Swatinem/rust-cache`) of the evidence jobs and of `coverage.yml` are saved only on `main` (`save-if`),
  and `headless` and `verapdf` share one key (`ew-cli`), so that pull requests restore the cache and never write
  one, within the 10 GB cache limit of the repository.
- Every workflow declares a top-level `permissions:` block with `contents: read` and no write scope; a job that
  needs more widens it for that job only. `permissions: write-all` is never used. `cargo xtask check-workflows`
  enforces these rules (NFR-SEC-19).
- No workflow runs fork code with secrets; `pull_request_target` is not used, every `actions/checkout` sets
  `persist-credentials: false`, and `${{ secrets.… }}` appears only in a job named `release`; `check-workflows`
  enforces all three.
- CI tools are installed at pinned versions: through `taiki-e/install-action`, which verifies checksums, or with
  `cargo install --locked --version`.
- Tests are never retried (NFR-MAINT-13; `retries = 0` in [`.config/nextest.toml`](../.config/nextest.toml)).

## Branch protection (owner action on GitHub)

These settings cannot be committed; a repository owner applies them under Settings → Rules (or Branches) for
`main` and `release/*` (18 §5.4):

- Require status checks to pass before merging, with these required checks: `lint`, `test-linux`,
  `test-linux-default`, `test-windows-build`, `test-windows-1`, `test-windows-2`, `test-windows-3`, `test-macos`, `test-macos-all-features`, `supply-chain`, `legal-docs`, `docs-diagrams`, `docs-book`. Require branches to be up to
  date before merging. `headless` and `verapdf` (cycle 18) are not on the list until three runs on `main` are
  green; then add them (the owner decides, because they pull images from Docker Hub; decision log ID-460, FT-325). `coverage` is a weekly
  report and is never a required check.
- Block force pushes and deletions; no direct pushes (changes land through pull requests only, administrators
  included).
- Require one approving review by someone other than the author; dismiss stale approvals on new commits.
- Require review from Code Owners ([`.github/CODEOWNERS`](../.github/CODEOWNERS); a second named owner per
  critical path is no longer required, decision log ID-452).
- Settings → Code security: enable secret scanning with push protection.
- Settings → Actions → General: require actions to be pinned to a full-length commit SHA, and set the default
  workflow permissions to "Read repository contents".

## Other owner actions (WP-S0.02, WP-S0.03)

- Settings → Code security: enable **private vulnerability reporting**, the channel that `SECURITY.md` (and,
  until the conduct contacts exist, `CODE_OF_CONDUCT.md`) points to.
- Settings → General: enable **require contributors to sign off on web-based commits**, so that commits made in
  the GitHub web editor carry the DCO trailer (19 §3.2).
- **BM-class performance runner** (DEC-034, 18 §4.3): provide and own the dedicated, isolated machine, then add a
  job on it with `EW_PERF_RUNNER=bm` and `EW_PERF_ASSERT=1` and re-bless `perf-baseline.toml` there; until then
  `perf.yml` on the hosted runner is report-only.
- Create the labels `format-major`, `api-break` and `licence` (RFC reference check). (The `blocked:LG-2` label
  and the LG-2 merge block were removed on the owner's ruling of 2026-10-22, decision log ID-454, FT-315.)
- **Denylist hashes** (NFR-PRIV-09): the maintainers who hold the confidential-token list run
  `cargo xtask denylist-hash "<phrase>"` locally for each customer, end-user and person name and each project or
  drawing number, keep the digests of the distinctive n-grams and append them to `denylist.sha256`. The tokens
  themselves never go into the repository, an issue or a CI log; the guard's messages name file and line only.
  Until entries exist the guard runs but cannot match anything and prints a warning (gate G-M0-06 needs the list
  committed); then enable `--require-entries` in `docs.yml`.

Both workflows run on every pull request, so all eight jobs can be required checks. `docs-diagrams`
renders every diagram each time (about 90 blocks, a few minutes) rather than filtering on changed paths, because
a required check whose workflow did not run blocks the merge.
