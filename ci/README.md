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
| CI-PR-01 | `ci.yml` / `lint` (ubuntu-24.04) | `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`, and again with `--all-features` (the generated `clippy.toml` also bans the std hash containers: FT-271, below); `cargo xtask check-ordered-collections` | `cargo fmt --all --check`, both clippy runs and `cargo xtask check-ordered-collections` |
| CI-PR-02 | `ci.yml` / `test-linux` (ubuntu-24.04) | `cargo build --workspace --locked`; `cargo nextest run --workspace --locked --profile ci -E 'not (package(ew-api) and binary(acceptance)) and not (package(ew-testkit) and binary(command_fuzz))'`, and again with `--all-features` and no filter (feature-gated tests such as `ew-ids/testkit`; the headless acceptance tests, about 2 min, and the command fuzzer's `fuzz_smoke`, 15 to 19 s, run once per job, in this second run: FT-268, FT-306); `cargo test --doc --workspace --locked`, and again with `--all-features` (nextest does not run doctests) | `cargo build --workspace --locked`, `cargo nextest run --workspace --locked -E 'not (package(ew-api) and binary(acceptance)) and not (package(ew-testkit) and binary(command_fuzz))'`, `cargo nextest run --workspace --all-features --locked` and `cargo test --doc --workspace [--all-features] --locked` |
| CI-PR-03 | `ci.yml` / `test-windows` (windows-2025) | as CI-PR-02 | as CI-PR-02, on Windows |
| CI-PR-04 | `ci.yml` / `test-macos` (macos-15, arm64) | as CI-PR-02 | as CI-PR-02, on macOS |
| CI-PR-05 | `ci.yml` / `headless` (ubuntu-24.04) | `cargo build -p ew --locked`; `ew export pdf golden/J-E-2416.ew` inside a bare `ubuntu:24.04` container (image pinned by digest; no network, no display, no GPU, no font file, empty user directories), on one thread and on four; both PDFs must be the committed digest of `golden/J-E-2416/expected/pdf/sha256.txt` (FT-295, G-M1-01); the PDFs uploaded as the artifact `headless-pdf`. Details: [Headless container job](#headless-container-job-ci-pr-05) | `cargo build -p ew --locked` and `docker run` as in the job (Linux and docker), or `cargo nextest run -p ew --test cli -E 'test(export_pdf_of_golden_a)'` for the same check without the container |
| CI-PR-09 | `ci.yml` / `supply-chain` (ubuntu-24.04) | `cargo deny check`; `cargo audit`; `cargo vet`; `cargo xtask check-deps`, `check-features`, `check-build-scripts`, `check-workflows`; third-party notices with cargo-about 0.9.2 (NFR-LIC-08); MSRV check of the permissive crates with Rust 1.85.0 | `cargo deny --locked check`, `cargo audit`, `cargo vet --locked`, the four `cargo xtask` checks, `cargo about generate -o target/THIRD-PARTY-NOTICES.txt ci/third-party-notices.hbs`, and the MSRV commands below |
| CI-PR-10 | `docs.yml` / `legal-docs` (ubuntu-24.04) | `docs/plan/tools/{totals,schedule,trace}.py --check` and `doccheck.py`; `reuse lint` (reuse-tool 6.2.0; NFR-LIC-01, NFR-LIC-02); `cargo xtask check-licence-map` (licence map, collective notice, crate licences and the licence boundary of the permissive layer: 19 §5.1, NFR-LIC-10, NFR-MAINT-02); `cargo xtask check-anonymisation` (NFR-PRIV-09); `cargo xtask check-governance-files` (NFR-DOC-08); rustdoc build with `-D warnings` (NFR-MAINT-08). On pull requests only: `cargo xtask check-dco` over the PR's commits (NFR-LIC-03); the LG-2 block, which fails a PR from anyone who is not an owner, member or collaborator while the plugin and design exception texts are missing from `LICENSES/` (NFR-LIC-12); and the RFC reference, which fails a PR labelled `format-major`, `api-break` or `licence` whose description does not mention `rfcs/` (NFR-DOC-09) | `python3 -I docs/plan/tools/totals.py --check` (and the other three); `python -m reuse lint`; `cargo xtask check-licence-map`, `check-anonymisation`, `check-governance-files`, `check-dco origin/main..HEAD`; `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` |
| CI-PR-15 | `docs.yml` / `docs-diagrams` (ubuntu-24.04) | Renders every Mermaid block under `docs/` and in the root Markdown with mermaid-cli 12.0.0 (container image pinned by digest) and fails on any error | `python3 -I ci/render-mermaid.py --image <image@sha256:…>` (needs docker); `--extract-only` lists the blocks without docker |
| CI-PR-10 (book) | `docs.yml` / `docs-book` (ubuntu-24.04) | The user-documentation book of `docs/book` (WP-S0.06, NFR-DOC-01, NFR-DOC-06): `cargo xtask docs-gen --check` (the generated command and rule reference pages are current); mdBook 0.5.4, installed from its pinned release archive (SHA-256 checked); `mdbook build docs/book` in an empty network namespace (the build is offline); `ci/check-book-links.py` over the built pages (relative links, anchors, and links into the repository); the built book uploaded as the artifact `user-guide`. A separate job, so that `legal-docs` stays fast | `cargo xtask docs-gen --check`; `mdbook build docs/book` (mdBook 0.5.4); `python3 -I ci/check-book-links.py target/book` |
| CI-N-09 | `ci.yml` / `verapdf` (ubuntu-24.04) | Builds `ew`, exports the golden PDF in three variants (all pages; dated by `SOURCE_DATE_EPOCH`; `--pages 101-106,901`) and validates each against PDF/A-2b with veraPDF 1.30.3 (the project's `verapdf/cli` image pinned by digest, run with no network); `ci/verapdf-check.py` turns the XML report into the verdict (FT-294, G-M1-02); the report uploaded as the artifact `verapdf-report`. Details: [veraPDF job](#verapdf-job-ci-n-09) | `python3 -I ci/verapdf-check.py --selftest`; with docker: `docker run --rm -v "$PWD/out:/data:ro" verapdf/cli@sha256:… --flavour 2b --format xml /data/golden-all.pdf > out/verapdf-report.xml`, then `python3 -I ci/verapdf-check.py out/verapdf-report.xml` |
| CI-PR-13 | `coverage.yml` / `coverage` (ubuntu-24.04; weekly and `workflow_dispatch`, not per PR) | cargo-llvm-cov 0.9.1 over the workspace tests (all features, nextest), the line coverage of `ew-model`, `ew-commands`, `ew-memo`, `ew-derive`, `ew-format` and `ew-store` against 80 % (NFR-MAINT-06); **report-only** (FT-296, G-M1-15). Details: [Coverage job](#coverage-job-ci-pr-13) | `cargo llvm-cov nextest --workspace --all-features --locked --no-report`, `cargo llvm-cov report --json --summary-only --output-path summary.json`, `python3 -I ci/coverage-summary.py summary.json` |

`cargo xtask ci` runs fmt, clippy, nextest and the doctests (each with the default features and with `--all-features`;
the default-features nextest step is labelled `nextest (no acceptance, no fuzzer)` in the summary, and the acceptance
tests and the command fuzzer run only in the `nextest-all-features` step, as in the jobs) and the ten repository checks (`check-deps`,
`check-features`, `check-raw-read`, `check-ordered-collections`, `check-build-scripts`, `check-workflows`, `check-licence-map`,
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
  -p ew-interop-api -p ew-dl -p ew-step-proto -p ew-memo -p ew-plcaddr -p ew-caex -p ew-devdesc -p ew-ewpp
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
   process it watches is the bench itself;
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
reload, and the closing undo-all with its own save and reload. The nightly body is this seeded test; the
cargo-fuzz target that doc 18 §3.6 and FT-272 list is deferred (FT-272, decision log ID-435): the repository has no
`fuzz/` crate yet (FT-297 creates it for the parser targets, doc 18 §3.12, and the command target joins it).

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
   `allow`, `expect` and `cfg_attr` attributes that name `disallowed_types`, prints the sites per crate and compares
   them with [`ordered-collections-allow.toml`](ordered-collections-allow.toml). A site is keyed by its crate and
   item path (`fn parse > let seen`, `struct State > field holds`, `use std::collections::HashSet`), never by file
   or line, with a count, so moving code does not touch the list and a new allow always does. It fails for a site
   that is not in the list, for a stale entry, for a site without a justification, and for a manifest that sets
   the lint (`[lints.clippy] disallowed_types = "allow"`). `--bless` rewrites the list after the review; a test of
   `xtask` (`ordered::tests::the_committed_list_is_current`) keeps it current in `cargo nextest run -p xtask`.

At the introduction (cycle 18) nine sites in four crates allow the lint, all lookup-only: `ew-import-tables` 2
(the duplicate-column check of the table reader), `ew-schema` 2 (the pattern cache of a test), `ew-store` 3 (the
fault-injection holds of the in-memory file system) and `xtask` 2 (the denylist of the anonymisation guard). No map
or set whose order reached output was found.

## Anonymisation guard (`cargo xtask check-anonymisation [--require-entries]`)

Every committed file that is not binary is scanned: files with a UTF-16 byte-order mark are decoded as UTF-16,
all other files as UTF-8 with invalid sequences replaced (so Latin-1 text is still scanned); a file with a NUL
byte in its first 8 KiB is binary and skipped. While `denylist.sha256` has no entries the guard cannot match
anything: the check prints a `WARNING` line and still exits 0, because supplying the entries is an owner action
and must not block CI. `--require-entries` turns that warning into a failure; no workflow uses it yet, the
maintainers switch it on once the list is committed (gate G-M0-06).

## Policy files

| File | Checked by | Content |
|---|---|---|
| [`dep-rules.toml`](dep-rules.toml) | `cargo xtask check-deps`, `check-features` | Layers, the permitted crate-to-crate edges (the ARCH §3 graph), external-crate owners (rusqlite and `libsqlite3-sys`, egui, wgpu; glob patterns cover the hyphenated sub-crates such as `egui-winit` and `wgpu-core`), the importer rule and the `ew-import-qet` confinement (NFR-MAINT-01, ADR-0024); the banned features and crates of `check-features` (below) |
| [`build-scripts.allowlist`](build-scripts.allowlist) | `cargo xtask check-build-scripts` | Every package with a build script in the all-features dependency graph, after review (ADR-0023, NFR-SEC-10) |
| [`../deny.toml`](../deny.toml) | `cargo deny check` | Licence allowlist (NFR-LIC-04), advisories, bans, sources |
| [`../supply-chain/`](../supply-chain) | `cargo vet` | Audits and the exemption list frozen at the first commit (ADR-0023). No public audit sets are imported yet, because `imports.lock` would record the auditors' names and e-mail addresses (decision log ID-4; ADR-0023 amendment 3: audits name the GitHub handle; audit sets are not imported while the public-repository rule stands) |
| [`licence-map.toml`](licence-map.toml) | `cargo xtask check-licence-map` | The path-to-licence map of 19 §2.1 (longest glob wins; unmapped paths fail), the files allowed to keep third-party copyright notices, where GPL-2.0 may appear, and the licence allowlist of the permissive layer's dependency closure (NFR-MAINT-02) |
| [`../REUSE.toml`](../REUSE.toml), [`../LICENSES/`](../LICENSES) | `reuse lint`, `cargo xtask check-licence-map` | Licence annotations for the files that cannot carry an SPDX header; the licence texts |
| [`denylist.sha256`](denylist.sha256) | `cargo xtask check-anonymisation` | SHA-256 digests of confidential word n-grams (n ≤ 3), never the tokens themselves (NFR-PRIV-09, 20 §3.5). Committed without entries: the maintainers add them (below); until then the check prints a warning and passes |
| [`../about.toml`](../about.toml), [`third-party-notices.hbs`](third-party-notices.hbs) | `cargo about generate` (CI-PR-09) | The cargo-about licence allowlist (the deny.toml list; the workspace's own `publish = false` crates are ignored) and the plain-text template of the third-party notice file (NFR-LIC-08). CI only generates the file, so a crate without a usable licence fails; the release pipeline (CI-R-07, WP-S6.07) ships it in every artefact |
| [`render-mermaid.py`](render-mermaid.py) | CI-PR-15 | Mermaid extraction and rendering |
| [`headless-export.sh`](headless-export.sh) | CI-PR-05 (`ci.yml` / `headless`) | Runs inside the headless container: proves that there is no display, GPU, font file or user directory content, then runs `ew export pdf` on one and four threads (FT-295) |
| [`verapdf-check.py`](verapdf-check.py) | CI-N-09 (`ci.yml` / `verapdf`) | The verdict over a veraPDF XML report: PDF/A-2B, compliant, no failed parse, the job count; `--selftest` (FT-294) |
| [`coverage-summary.py`](coverage-summary.py) | CI-PR-13 (`coverage.yml`) | The line coverage of the six core crates from a cargo-llvm-cov summary, against 80 %, report-only; `--selftest` (FT-296) |
| [`check-book-links.py`](check-book-links.py) | CI-PR-10 (book) | Link check of the built user-documentation book: no network, no plugin to pin |
| [`ordered-collections-allow.toml`](ordered-collections-allow.toml) | `cargo xtask check-ordered-collections` | The reviewed sites that allow the hash-container ban of the generated `clippy.toml`, by crate and item path with a count (FT-271, WP-S6.02) |
| [`perf-baseline.toml`](perf-baseline.toml) | `cargo xtask perf-gate` (nightly `perf.yml`) | The baseline medians and memory estimates per size class, the 15 % threshold, the noise floor and the runner they were taken on (WP-S6.03) |
| [`ci-times.py`](ci-times.py) | nightly `perf.yml` | The CI-time trend of `ci.yml` against NFR-MAINT-05 (FT-269) |
| [`../.github/workflows/fuzz.yml`](../.github/workflows/fuzz.yml) | nightly command fuzzer | The seeded `fuzz_long` run, with the findings file as artifact and a verdict step (red for a finding, a failed build or a hang) (WP-S6.02, FT-272) |

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
  `test-windows`, `test-macos`, `supply-chain`, `legal-docs`, `docs-diagrams`, `docs-book`. Require branches to be up to
  date before merging. `headless` and `verapdf` (cycle 18) are not on the list until their first runs on `main` are
  green; then add them (the owner decides, because they pull images from Docker Hub). `coverage` is a weekly
  report and is never a required check.
- Block force pushes and deletions; no direct pushes (changes land through pull requests only, administrators
  included).
- Require one approving review by someone other than the author; dismiss stale approvals on new commits.
- Require review from Code Owners ([`.github/CODEOWNERS`](../.github/CODEOWNERS); a second named owner per
  critical path is due by M1, gate G-M0-13).
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
- Create the labels `format-major`, `api-break`, `licence` (RFC reference check) and `blocked:LG-2` (label for
  non-maintainer pull requests while ADR-0025 is pending).
- **Denylist hashes** (NFR-PRIV-09): the maintainers who hold the confidential-token list run
  `cargo xtask denylist-hash "<phrase>"` locally for each customer, end-user and person name and each project or
  drawing number, keep the digests of the distinctive n-grams and append them to `denylist.sha256`. The tokens
  themselves never go into the repository, an issue or a CI log; the guard's messages name file and line only.
  Until entries exist the guard runs but cannot match anything and prints a warning (gate G-M0-06 needs the list
  committed); then enable `--require-entries` in `docs.yml`.

Both workflows run on every pull request, so all eight jobs can be required checks. `docs-diagrams`
renders every diagram each time (about 90 blocks, a few minutes) rather than filtering on changed paths, because
a required check whose workflow did not run blocks the merge.
