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
| CI-PR-01 | `ci.yml` / `lint` (ubuntu-24.04) | `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`, and again with `--all-features` | `cargo fmt --all --check` and both clippy runs |
| CI-PR-02 | `ci.yml` / `test-linux` (ubuntu-24.04) | `cargo build --workspace --locked`; `cargo nextest run --workspace --locked --profile ci -E 'not (package(ew-api) and binary(acceptance))'`, and again with `--all-features` and no filter (feature-gated tests such as `ew-ids/testkit`; the headless acceptance tests, about 2 min, run once per job, in this second run: FT-268); `cargo test --doc --workspace --locked`, and again with `--all-features` (nextest does not run doctests) | `cargo build --workspace --locked`, `cargo nextest run --workspace --locked -E 'not (package(ew-api) and binary(acceptance))'`, `cargo nextest run --workspace --all-features --locked` and `cargo test --doc --workspace [--all-features] --locked` |
| CI-PR-03 | `ci.yml` / `test-windows` (windows-2025) | as CI-PR-02 | as CI-PR-02, on Windows |
| CI-PR-04 | `ci.yml` / `test-macos` (macos-15, arm64) | as CI-PR-02 | as CI-PR-02, on macOS |
| CI-PR-09 | `ci.yml` / `supply-chain` (ubuntu-24.04) | `cargo deny check`; `cargo audit`; `cargo vet`; `cargo xtask check-deps`, `check-features`, `check-build-scripts`, `check-workflows`; third-party notices with cargo-about 0.9.2 (NFR-LIC-08); MSRV check of the permissive crates with Rust 1.85.0 | `cargo deny --locked check`, `cargo audit`, `cargo vet --locked`, the four `cargo xtask` checks, `cargo about generate -o target/THIRD-PARTY-NOTICES.txt ci/third-party-notices.hbs`, and the MSRV commands below |
| CI-PR-10 | `docs.yml` / `legal-docs` (ubuntu-24.04) | `docs/plan/tools/{totals,schedule,trace}.py --check` and `doccheck.py`; `reuse lint` (reuse-tool 6.2.0; NFR-LIC-01, NFR-LIC-02); `cargo xtask check-licence-map` (licence map, collective notice, crate licences and the licence boundary of the permissive layer: 19 §5.1, NFR-LIC-10, NFR-MAINT-02); `cargo xtask check-anonymisation` (NFR-PRIV-09); `cargo xtask check-governance-files` (NFR-DOC-08); rustdoc build with `-D warnings` (NFR-MAINT-08). On pull requests only: `cargo xtask check-dco` over the PR's commits (NFR-LIC-03); the LG-2 block, which fails a PR from anyone who is not an owner, member or collaborator while the plugin and design exception texts are missing from `LICENSES/` (NFR-LIC-12); and the RFC reference, which fails a PR labelled `format-major`, `api-break` or `licence` whose description does not mention `rfcs/` (NFR-DOC-09) | `python3 -I docs/plan/tools/totals.py --check` (and the other three); `python -m reuse lint`; `cargo xtask check-licence-map`, `check-anonymisation`, `check-governance-files`, `check-dco origin/main..HEAD`; `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` |
| CI-PR-15 | `docs.yml` / `docs-diagrams` (ubuntu-24.04) | Renders every Mermaid block under `docs/` and in the root Markdown with mermaid-cli 12.0.0 (container image pinned by digest) and fails on any error | `python3 -I ci/render-mermaid.py --image <image@sha256:…>` (needs docker); `--extract-only` lists the blocks without docker |
| CI-PR-10 (book) | `docs.yml` / `docs-book` (ubuntu-24.04) | The user-documentation book of `docs/book` (WP-S0.06, NFR-DOC-01, NFR-DOC-06): `cargo xtask docs-gen --check` (the generated command and rule reference pages are current); mdBook 0.5.4, installed from its pinned release archive (SHA-256 checked); `mdbook build docs/book` in an empty network namespace (the build is offline); `ci/check-book-links.py` over the built pages (relative links, anchors, and links into the repository); the built book uploaded as the artifact `user-guide`. A separate job, so that `legal-docs` stays fast | `cargo xtask docs-gen --check`; `mdbook build docs/book` (mdBook 0.5.4); `python3 -I ci/check-book-links.py target/book` |

`cargo xtask ci` runs fmt, clippy, nextest and the doctests (each with the default features and with `--all-features`;
the default-features nextest step is labelled `nextest (no acceptance)` in the summary, and the acceptance tests run
only in the `nextest-all-features` step, as in the jobs) and the nine repository checks (`check-deps`,
`check-features`, `check-raw-read`, `check-build-scripts`, `check-workflows`, `check-licence-map`,
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
cargo-fuzz target that doc 18 §3.6 and FT-272 list is deferred (FT-272; design pass): the repository has no
`fuzz/` crate yet (that arrives with the parser-fuzzing package, doc 18 §3.12).

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
`EW_FUZZ_BUDGET_SECS`. `fuzz_smoke` (2 seeds of 30 steps) stays in the targeted tests and in `cargo xtask ci`.

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
| [`check-book-links.py`](check-book-links.py) | CI-PR-10 (book) | Link check of the built user-documentation book: no network, no plugin to pin |
| [`perf-baseline.toml`](perf-baseline.toml) | `cargo xtask perf-gate` (nightly `perf.yml`) | The baseline medians and memory estimates per size class, the 15 % threshold, the noise floor and the runner they were taken on (WP-S6.03) |
| [`ci-times.py`](ci-times.py) | nightly `perf.yml` | The CI-time trend of `ci.yml` against NFR-MAINT-05 (FT-269) |
| [`../.github/workflows/fuzz.yml`](../.github/workflows/fuzz.yml) | nightly command fuzzer | The seeded `fuzz_long` run, with the findings file as artifact and a verdict step (red for a finding, a failed build or a hang) (WP-S6.02, FT-272) |

## Rules for the workflows

- Third-party actions are pinned by full commit SHA with a `# vX.Y.Z` comment; Renovate
  ([`.github/renovate.json`](../.github/renovate.json)) proposes updates weekly, grouped. Container images are
  pinned by digest.
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
  date before merging.
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
