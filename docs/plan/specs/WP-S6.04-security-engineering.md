# WP-S6.04: Security engineering for the pilot release (closed specification)

**Status:** specified in cycle 50 (documents only), an M2 package at the size of ID-1187's re-estimate (PERT 1.1 pw,
which stays), built whole by one lane (ID-1281) in the six steps of §6 · **Date:** 2026-11-04 · **Work package:**
[WP-S6.04](../02-work-packages.md#wp-s604-security-engineering-for-the-pilot-release) · **Decision log:** ID-1187,
ID-1188, ID-1203, ID-1321 · **Part of:** ElektraWatt plan, specifications

**Purpose.** 0.1.0 must treat every received file as hostile (ID-1187), but the evidence is scattered: the threat
model exists only as a table in doc 18 §11.1 (`docs/security/threat-model.md`, which doc 18 names, does not exist), six
cargo-fuzz targets cover the loader, the open path, the `.ewz` reader, the merge and two decoders while no importer,
the SQL sandbox (FT-223), the table round trip, textconv, the command logs or the SVG assets has one, no hostile corpus
is committed as a set, the `unsafe` allowlist names one crate against doc 18 §11.2's longer list, `SECURITY.md` is
"a stub for milestone M0", and the approve and release dialogs do not carry the engineering-verification disclaimer.
This document closes the package's product items to decisions, a build order and acceptance, against the code at
`9d111131` (§2.1). The importers' targets and the importer half of NFR-REL-12 are WP-S3.09's (its D4, D15); what
ID-1187 and ID-1188 moved to WP-S6.10 stays there. Its size is an estimate, not a commitment (ID-1210); every time is
not measured (owner's rule). Where this document and a design document differ, this document is the narrower and wins
for the lane; the design documents stay the target and are updated at the design pass (§13). A choice the lane meets
that this document does not settle is escalated, never guessed.

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [The texts](#5-the-texts)
6. [Build order](#6-build-order)
7. [Acceptance](#7-acceptance)
8. [Neighbours, files owned and what a lane may not decide](#8-neighbours-files-owned-and-what-a-lane-may-not-decide)
9. [Risks](#9-risks)
10. [Size and routing](#10-size-and-routing)
11. [Follow-ups, proposed](#11-follow-ups-proposed)
12. [Open questions](#12-open-questions)
13. [Documents to update](#13-documents-to-update)
14. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package `docs/security/threat-model.md` holds TM-01 to TM-19 with their 0.1.0 state,
controls and evidence, an entry per parser of external bytes and per extension point, and a review log that RC-01
reads; every parser of NFR-SEC-02 that 0.1.0 ships, the merge driver and the SQL sandbox has a cargo-fuzz target,
registered in CI-PR-08 and the deep nightly, with seeds and, for the SQL sandbox, FT-223's hostile corpus; a committed
hostile corpus is refused file by file in a per-PR test that also bounds each refusal's extra allocation; an importer
reads no file but its source, which the network sandbox's trace proves; the `unsafe` allowlist is what the code uses,
checked over the fuzz crate too, with the test-only crates kept out of the shipped graph; `SECURITY.md` is the full
policy with the supported-versions table, the support period and the pointer to "Verify a download"; the
engineering-verification disclaimer stands in the About dialog, the check panel, the approve and release dialogs, the
guide, the licence text and the release report in one wording; and a panic in the SQL query of `ew sql` or in the table
round trip's read leaves the session running and the project unchanged.

**Non-goals** (each with its package):
- No tabletop exercise, no vulnerability-handling runbook (24 h / 72 h / 14 days), no single-reporting-platform account,
  no CRA notification procedure (WP-S6.10, from 1.0; ID-1187, ID-1188).
- No importer fuzz target, no change to the importers' limits or their front door, no importer panic isolation
  (WP-S3.09 D2, D4, D15).
- No rhai limits: 0.1.0 evaluates no rhai (the crate `ew-rhai` is a six-line stub without the `rhai` dependency); the
  limits of NFR-SEC-06 and the `rhai_formula` target are WP-S1.15's remainder, which adds the engine (D16, §12 O5).
- No trust gate (NFR-SEC-05, WP-S1.15): 0.1.0 runs no project-embedded automation (D2's TM-04 entry).
- No `ew-step-worker` restart: the worker is a stub that does no work at 0.1.0 (D15).
- No OS sandbox for workers (NFR-SEC-21, 1.x), no OSS-Fuzz application (1.0), no signing (ID-1187).
- No new Rust dependency in the workspace; no change to `deny.toml`, `supply-chain/` or `Cargo.lock` (the fuzz crate's
  own lock gains only workspace crates already in its graph).

## 2. Inputs and the code as it is

| Input | What the lane takes from it |
|---|---|
| [Quality](../../design/18-quality-security-release.md) §3.12, §5.4, §7.5, §11.1 to §11.5, §18 (Security row) | The targets and properties, CI hardening (TM-12), the support policy, the threat table, the `unsafe` rules, vulnerability handling |
| [Interop](../../design/14-interop.md) §11, §12 | The per-parser limits, the target list, the hostile set |
| [Non-functional requirements](../../design/02c-nonfunctional-requirements.md) NFR-SEC-01 to -06, -14, -18, NFR-REL-12, NFR-COMP-03, -04 | §3 |
| ADR-0020 (decision 7's threat table), ADR-0023 (decisions 3 and 4, amendment 7) | The controls, the disclosure items, 0.1.0 unsigned |
| Decisions ID-1187, ID-1188, ID-1203; ID-1030 (the cycle's reviewer is the second maintainer of NFR-SEC-04) | The 0.1.0 scope; the review of `unsafe` |
| Follow-ups FT-223, FT-1182, FT-332 | D9, D12; TM-17 |
| Specifications [WP-S3.09](WP-S3.09-importer-contracts-and-lineage.md) D2 to D5, D15; [WP-S6.05](WP-S6.05-privacy-and-crash-log.md) D4, D5; [WP-S6.07](WP-S6.07-release-pipeline.md) D14 (the checklist) | The importer half; the sandbox and its `strace` scan; RC-01, RC-02, RC-04, RC-09 |

### 2.1 What exists at `9d111131` (the base contract)

The lane relies on exactly these items. If one differs, the lane stops and reports it.

| Item | Where | Used for |
|---|---|---|
| No `docs/security/` folder; doc 18 names `docs/security/threat-model.md`; TM-01 to TM-19 | `docs/design/18-quality-security-release.md:72`, `:1696-1716` | D1, D2 |
| The release checklist's RC-01 (threat-model review), RC-02 (no open fuzz crash), RC-04 (`SECURITY.md`'s table names 0.1.0 and its support end), RC-09 (the disclaimer in About, check panel, guide, release report) | `docs/release/checklist.md:18-26` | D1, D11, D13 |
| Six cargo-fuzz targets in the crate outside the workspace; the CI-PR-08 table with an `xtask` test of each target's closure; the deep nightly's matrix (30 minutes per target, `-rss_limit_mb=4096`, `-timeout=120`); the replay test of seeds and regressions | `fuzz/Cargo.toml:56-100`; `ci/fuzz-smoke-targets.toml`; `.github/workflows/deep-nightly.yml:282-370`; `fuzz/tests/replay.rs` | D9 |
| `merge_file` fuzzes `ew_format::merge_file`, the core of `ew merge-driver`, whose own call runs under `catch_unwind` | `fuzz/src/merge_file.rs:1-12`; `bin/ew/src/cli/merge_driver.rs:66` | D3, D9 |
| textconv: `ew_api::output::textconv(session, bytes)`, called by `ew diff --textconv` | `crates/ew-api/src/output/project_diff.rs:232`; `bin/ew/src/cli/diff.rs:191-211` | D9 |
| Command logs (`.ewcmd`): parsed and replayed by `Session::run_script` | `crates/ew-api/src/cmdlog.rs:1-25` | D9 |
| The table round trip's read (`Session::table_import`), pure over the bytes and the session | `crates/ew-api/src/output/table_io.rs:1-25` | D9, D15 |
| The SQL sandbox: read-only deserialised image, statement gate, authorizer, `SQLITE_LIMIT_*`, progress-handler deadline, result caps; its security tests on a small image; `ew sql`'s profile | `crates/ew-sqlview/src/sandbox.rs:1-60`, `:114`; `crates/ew-sqlview/src/sandbox_tests.rs:1-12`; `bin/ew/src/cli.rs:802` | D9, D10, D15 |
| SVG assets: `sniff` and `svg_hazard` before any parse; `usvg` in `ew-scene` inside `catch_unwind` | `crates/ew-schema/src/asset.rs:250`, `:614`; `crates/ew-scene/src/images.rs:92` | D9 |
| Raster assets: `decode_image` (PNG and JPEG) inside `catch_unwind`; `png_pdf` covers PNG only | `crates/ew-render-cpu/src/image.rs:39-45`; `fuzz/src/png_pdf.rs` | D9 |
| The allowlist: `ew-print = ["src/windows.rs"]` only; `check-unsafe` scans the workspace's packages (not `fuzz/`), each listed file's `unsafe` blocks carry `SAFETY:` comments | `ci/unsafe-allowlist.toml:12-15`; `xtask/src/unsafe_code.rs:4-24`, `:138-190` | D7 |
| `banned_crates` (dev edges only, absent from the shipped binaries' graph): `egui_mcp`, `ew-testids` and WP-S6.05's network clients | `ci/dep-rules.toml:29-37` | D8 |
| The only build script in the shipped graph that compiles C is `libsqlite3-sys`'s | `ci/build-scripts.allowlist:60-64` | D7 |
| rhai: `ew-rhai` is a doc line and `#![forbid(unsafe_code)]`, no `rhai` dependency, no limit setter anywhere | `crates/ew-rhai/src/lib.rs:1-6`; `crates/ew-rhai/Cargo.toml` | D16 |
| `ew-step-worker` prints its version and exits | `bin/ew-step-worker/src/main.rs:8-10` | D15 |
| WP-S6.05's crash hook and the outer `catch_unwind` of `elektrawatt` (exit 101); the network sandbox `network-sandbox` with `ci/sandbox/run.sh` (the flow `ew-import-qet` on `ci/sandbox/fixture.qet`) and `ci/sandbox-check.py` (its `openat` pattern) | `bin/elektrawatt/src/main.rs:671-681`; `ci/sandbox/run.sh:30`, `:60`; `ci/sandbox-check.py:11-39`, `:65` | D14 |
| Panics caught in place: job threads, rule runs, printing, decoders, tessellation, the merge driver | `crates/ew-jobs/src/lib.rs:301`; `crates/ew-checks/src/rules/system.rs:241`; `crates/ew-print/src/lib.rs:261`; `crates/ew-render-cpu/src/image.rs:45`; `crates/ew-render2d/src/gpu/tess.rs:206` | D15 |
| `SECURITY.md`: reporting through GitHub's private vulnerability reporting, the 7- and 30-day targets, a CVE per fix, the table "No releases yet", the support rule, "Scope of this stub"; the issue templates link the advisory form | `SECURITY.md:6-47`; `.github/ISSUE_TEMPLATE/config.yml:7` | D11 |
| The install guide's section "Verify a download" | `docs/book/src/how-to/install.md:14` | D12 |
| The disclaimer: About (`about-disclaimer`, "ElektraWatt's checks do not replace engineering verification by a competent person."), the check panel (`messages-footer`, "Checks support, but do not replace, engineering verification."), the check report (`DISCLAIMER`, "Checks do not replace engineering verification (NFR-COMP-04).") written into the release archive's `check/report.json` with the packs; `LICENSE.md`; no line in the approve or release dialog's texts | `crates/ew-viewmodel/locales/en-GB/shell.ftl:1603`, `:2020`, `:2527-2547`; `crates/ew-checks/src/report.rs:22`, `:131`; `crates/ew-api/src/release_run.rs:1225-1240`; `LICENSE.md:51` | D13 |

## 3. Requirements traced

| Requirement | State and what this package delivers | Acceptance |
|---|---|---|
| NFR-SEC-01 (Should) | `docs/security/threat-model.md` with the parser and extension-point entries and the review log RC-01 reads (D1 to D3) | SE-1 |
| NFR-SEC-02 (0.1.0 part) | The committed hostile corpus refused per file, with each refusal's extra allocation bounded (D10); the limits themselves are WP-S3.09's and the loaders' | SE-7 |
| NFR-SEC-03 | A target for every parser of D3's table, the merge driver and the SQL sandbox (D9); RC-02's release rule unchanged | SE-5, SE-6 |
| NFR-SEC-04 (Should) | The allowlist as used, the fuzz crate scanned, the test-only crates banned from shipped graphs, C limited to bundled SQLite (D7, D8) | SE-4 |
| NFR-SEC-14 | `SECURITY.md` in full (D11); the tabletop exercise is WP-S6.10's | SE-2 |
| NFR-SEC-18 | The trace half: an import opens no file but its source and the content folder (D14); the code half is WP-S3.09's D5 | SE-8 |
| NFR-REL-12 (0.1.0 part) | The SQL query and the table round trip's read under `catch_unwind` (D15); the importers by WP-S3.09 D4; the STEP worker and rhai halves with their packages (§12 O4) | SE-9 |
| NFR-COMP-03 | The supported-versions table and the support period (D11; the commitment is the owner's, §12 O2) | SE-2 |
| NFR-COMP-04 (0.1.0 part) | One wording in About, check panel, approve and release dialogs, guide, licence text and release report, with packs and waivers (D13); thermal reports and checklists are 1.0's | SE-3 |
| FT-223 | `sql_sandbox` and the hostile-SQL corpus (D9, D10) | SE-6 |
| FT-1182 | `SECURITY.md` points to "Verify a download" (D12) | SE-2 |

## 4. Decisions

Each is a lead ruling that the owner may reverse.

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **The threat model's file and form.** `docs/security/threat-model.md` (CC-BY-4.0, as `SECURITY.md`), in four parts: the assets and the trust boundaries (doc 18 §11.1's text and diagram); the threat table TM-01 to TM-19 with the columns ID, threat, 0.1.0 state (`in force`, `1.0`, `1.x`, `not shipped`), controls as built with `path:line`, verified by (CI job IDs, tests, the hostile corpus files); the parser table and the extension-point table of D3; the review log, one row per review (date, release, reviewer, the parsers and extension points added since the last, result). New threats take TM-20 onward. Doc 18 §11.1 keeps the design form and points to this file as the maintained copy | NFR-SEC-01 asks for a maintained file and RC-01 for a review entry per release; the design table stays the target, the file states what 0.1.0 does |
| D2 | **The 0.1.0 states** (the lane fills the controls and evidence from the code): TM-01, TM-02, TM-10, TM-11, TM-14, TM-18, TM-19 *in force*; TM-03 *not shipped* (no foreign SQLite reader at 0.1.0; QETZ is 1.x); TM-04 *not shipped* (no rhai, no project SQL reports run, no rule packs: nothing in a received project executes; the trust gate arrives with WP-S1.15's remainder); TM-05 *in force* for SQL (`ew sql`'s deadline and limits), rhai *not shipped*; TM-06 *1.x*; TM-07 *not shipped* (`ew mcp` is not built; `egui_mcp` is a banned crate, test builds only); TM-08 *not shipped* (the worker is a stub); TM-09 *in force* as documentation (`ew diff`, the archive sums); TM-12 *in force* without the two-person publish (1.0, ID-1187); TM-13 and TM-15 *1.0* (no update check, no signing key); TM-16 *1.0*; TM-17 *in force* with FT-332 open | One honest line per threat for the release that ships, so RC-01's reviewer reads what is true rather than the 1.0 design |
| D3 | **The parser and extension-point tables.** Parsers of external bytes at 0.1.0, each with crate, entry point, limits (where held), target and owner of the target: the JSONL loader and migration chain (`jsonl_loader`, `migration_chain`), `.ewz` (`ewz_read`), the three-way merge of the merge driver (`merge_file`), textconv (`textconv`, new), the QET project and element importers, the golden tables and the parts table (WP-S3.09's `qet_project`, `qet_element`, `golden_tables`, `parts_table`), the table round trip's read (`table_round_trip`, new), SVG assets (`svg_asset`, new), PNG and JPEG assets (`png_pdf`; `image_decode`, new), SQL text (`sql_sandbox`, new), command logs (`command_log`, new), and library documents of a `.ewlib` folder (the `LibDocument` reader that `jsonl_loader` already drives through a project's `lib/` copies; the lane confirms that `ew-library` parses through it alone). Extension points: SQL through `ew sql`; command logs through `ew apply` (with `HumanOnly`'s refusal); the headless script runner (`--headless --script`, actions only); git's merge driver and textconv (run by git on fetched content, TM-01); rhai, the trust gate, plugins, MCP and the external converter, each marked with the release that brings it | NFR-SEC-01's "every parser or extension point adds an entry"; each row names the target that proves it, so a parser without a target is visible |
| D4 | **Review rule.** The file's first review is the 0.1.0 entry written by this package; every minor release adds a row (RC-01, built); a pull request that adds a parser or an extension point adds its table row in the same change (CONTRIBUTING's "Pull request checklist" gains the line) | The rule NFR-SEC-01 sets, with the step where it is checked |
| D5 | **Who owns which target.** WP-S3.09: `qet_project`, `qet_element`, `golden_tables`, `parts_table` (its D15). This package: `sql_sandbox`, `svg_asset`, `image_decode`, `textconv`, `table_round_trip`, `command_log`. The six built targets keep the packages that built them | The importers' targets come with their contract (WP-S3.09); the rest are this package's |
| D6 | **No counting allocator in the fuzz crate.** The fuzz targets keep libFuzzer's `-rss_limit_mb` and `-timeout`; the allocation bound of NFR-SEC-02 is measured where it is deterministic, in D10's per-PR corpus test | The fuzz crate then needs no `unsafe`, and the bound is checked per pull request instead of nightly |
| D7 | **The allowlist is what the code uses.** `ci/unsafe-allowlist.toml` keeps `ew-print` and gains `ew-testkit` with the one file of D10's counting allocator (`unsafe impl GlobalAlloc`, each block with its `SAFETY:` comment, reviewed by the cycle's reviewer, ID-1030); its manifest copies the workspace's lints with `unsafe_code = "deny"`. Doc 18 §11.2's other names (the renderers, the step worker, the planned `ew-sysprobe`, the crash handler) enter only when a package needs `unsafe` there, by the file's map marker. `check-unsafe` also scans every `.rs` file of `fuzz/` lexically and fails on the keyword outside a listed file (no crate-root `forbid` is required there: `libfuzzer-sys`'s macro expands its own exported symbol). C stays limited to the bundled SQLite: `check-build-scripts`' reviewed list already holds the one C-compiling script of the shipped graph, which the threat model records | A list of candidates that use no `unsafe` is no control; the scan closes the one tree the check did not read |
| D8 | **Test-only crates stay out of shipped binaries:** `banned_crates` gains `ew-testkit`, so `check-features` fails when it reaches the shipped binaries' normal or build graph (CI-R-09's per-PR half); the fuzz crate is outside the workspace and cannot be reached | Doc 18 §11.2's condition for allowlisting a test-only crate, checked per pull request |
| D9 | **The six new targets** (properties: no panic, no hang beyond libFuzzer's timeout, and per target): `sql_sandbox`: the input as one SQL text through a `Reader` with the report profile over a projection image built once from Golden A; the result is rows within the caps or an `SqlError`; the image's bytes are unchanged; the same text twice gives the same result unless the deadline stopped it (no clock leak). `svg_asset`: bytes through `sniff`; what it accepts parses through `ew-scene`'s path without a panic. `image_decode`: a selector byte picks PNG or JPEG; `decode_image` returns an image within `MAX_IMAGE_PIXELS` or none. `textconv`: bytes as one project file, with and without Golden A's session; a text results. `table_round_trip`: a selector byte picks CSV or XLSX and one table key of Golden A; `Session::table_import` returns its changes or a refusal and the model is unchanged. `command_log`: bytes as a `.ewcmd` run by `Session::run_script` on a copy of Golden A in memory; every line commits or is refused, the model passes the structural check, and a `HumanOnly` command is refused. Each is a `[[bin]]` of `fuzz/Cargo.toml`, a harness in `fuzz/src/`, seeds from `seed_corpus.rs`, a row of `ci/fuzz-smoke-targets.toml` (`cargo xtask fuzz-smoke-table`) and an entry of the deep nightly's matrix (`max_len` 16,384; 4,096 for `sql_sandbox`) | NFR-SEC-03's list for what 0.1.0 ships; doc 18 §3.12 names textconv and the SQL sandbox explicitly |
| D10 | **The hostile corpus** (CC0, own files): `crates/ew-testkit/hostile/<class>/`, with an index `README.md` naming each file, its class, the parser and the expected verdict, and a generator for the files that are large when expanded (committed small). Classes at 0.1.0: an XLSX ZIP bomb and one over the 200 : 1 ratio; XLSX entries named `../../x`, `/x`, `C:x`; billion-laughs and external-entity XML as a `.qet`, an `.elmt` and an XLSX part; a `.qet` with a NaN and one with an infinite coordinate; a `.qet` at the depth and attribute limits plus one; an `.ewz` with a `..` entry and one with 65,536 entries; a JSONL line over 16 MiB and one nested past depth 64; an SVG with an internal subset, a `<script>` and an external `href`; a PNG declaring 2^31 pixels and a JPEG without its end marker; FT-223's hostile SQL (recursive-CTE bombs, deep expressions, `ATTACH`, a `PRAGMA` read, two statements in one call, `load_extension`, a clock function given a BLOB, a CTE named after a view); a project copy whose page file is a symbolic link (Unix only, NFR-SEC-16). A test binary of `ew-testkit` runs each file through the parser's public entry point (for importers, WP-S3.09's front door) and asserts the verdict whole (a refusal's text, or the report codes and counts) and, through the counting allocator, at most 256 MiB of extra peak allocation per file (§12 O3). No time is asserted (owner's rule). The SQL files are also `sql_sandbox`'s seeds | Doc 18 §3.12's set and NFR-SEC-02's PR test, deterministic; one place a reviewer reads the hostile inputs |
| D11 | **`SECURITY.md` in full** (in the file's style, English): *Reporting a vulnerability* (GitHub's private vulnerability reporting, never public issues; what to include; no customer files; the conduct line kept); *What happens next* (acknowledgement within 7 days; triage with a CVSS 4.0 score within 14 days; a fix or mitigation for critical issues within 30 days; coordinated disclosure with an embargo of at most 90 days; the GitHub advisory published with the fix; a CVE requested through GitHub for each fixed vulnerability; credit if wished); *Supported versions* (the table: `0.1.x` supported until the next minor release; earlier development builds not supported; below it, the rule: before 1.0 only the latest 0.x release is supported, and from 1.0 the latest minor gets all fixes and each minor security and data-loss fixes for 6 months after its successor ships; no long-term-support line); *Verifying a download* (D12); *Scope* (in: `elektrawatt`, `ew`, the format crates, the release artefacts and the release workflow; out: vulnerabilities of third-party crates, which go upstream with a note to the project, and the content of users' own projects); *The threat model* (a link to D1's file); *What follows at 1.0* (the CRA notification process and the disclosure exercise, WP-S6.10). The stub section goes. `check-governance-files` keeps the file required | NFR-SEC-14, NFR-COMP-03 and ADR-0023 decision 4 in one page; the commitments in it are the owner's (§12 O1, O2) |
| D12 | **"Verify a download"** (FT-1182): one paragraph that 0.1.0 is unsigned, that each release carries `SHA256SUMS` and GitHub's build-provenance attestations, and a link to the install guide's section (`docs/book/src/how-to/install.md#verify-a-download`) for the commands | The guide holds the steps; `SECURITY.md` says where they are, as WP-S6.07 FP-3 asks |
| D13 | **The disclaimer, one wording:** the About dialog's sentence (`about-disclaimer`) is the statement everywhere. `DISCLAIMER` takes it (the requirement ID leaves a user text), so `ew check --format md`, the release report's `check/report.json` and every other writer of the constant change with it; the approve and release dialogs show it, through the same key, as their last line before the buttons; the guide's introduction gains a paragraph with it; `LICENSE.md` keeps its own fuller text. The check panel's footer keeps its shorter line. The release report already lists the packs; the lane checks that it lists the waivers in force too and adds them if not | NFR-COMP-04 names the release dialog, and doc 15 the approve dialog; one sentence keeps the places in step; an ID in a user text is the rule's exception nobody needs |
| D14 | **NFR-SEC-18's trace.** `ci/sandbox/run.sh`'s `ew-import-qet` flow gets a sibling file beside the fixture and a second fixture with an `xi:include` and an external entity that name it; `ci/sandbox-check.py` gains the rule `foreign-file`: in an import flow, an `openat` of a path under the job's work folder other than the flow's source, or under the repository other than `content/`, fails with the flow, the line and the rule; `--test` covers it | The sandbox already traces `openat` of every flow; one rule turns it into the file-access trace NFR-SEC-18 asks for |
| D15 | **NFR-REL-12 where it is missing at 0.1.0.** `ew sql`'s statement and the table round trip's read (`Session::table_import`, in the window and in `ew import table`) run inside `catch_unwind`: a panic gives S1 or S2, exit 2 in the CLI, a banner in the window, and the model and the files unchanged. The importers are WP-S3.09's (D4); the rule runs, jobs, printing, decoders, tessellation and the merge driver are built (§2.1). `ew-step-worker` does no work and plugins and rhai do not exist at 0.1.0, so "a worker is restarted" has no subject; the threat model and §13 say so | The two surfaces that run hostile input on the caller's thread without a catch; the rest is either caught or not shipped |
| D16 | **The rhai limits are WP-S1.15's.** NFR-SEC-06's operation, call-depth and string-size limits are set where the engine is created, and the engine does not exist: WP-S1.15's remainder adds `rhai` with `set_max_operations`, `set_max_call_levels`, `set_max_string_size`, no I/O registered and `unchecked` banned, with the `rhai_formula` target and a non-terminating-script test (NFR-REL-12's acceptance). This package records the entry as *not shipped* (D2) | A limit setter without an engine is no control; the package that adds the engine owns its limits (§12 O5) |
| D17 | **Every item closed;** no decision changes an ADR | — |

## 5. The texts

Every text is asserted whole (en-GB); the nl-NL texts are the orchestrator's (ID-1320), written by the lane and listed
for the orchestrator's read.

| # | Where | Text |
|---|---|---|
| S1 | `ew sql`, a panic in the statement (exit 2) | `error: the query stopped on an internal error; nothing was changed` |
| S2 | Table round trip, a panic in the read (window banner; `ew import table` exit 2) | `The table import stopped on an internal error while reading <file>; nothing was changed.` |
| S3 | Approve and release dialogs, last line; `DISCLAIMER` | `ElektraWatt's checks do not replace engineering verification by a competent person.` (the key `about-disclaimer`) |
| S4 | `ci/sandbox-check.py`, the new rule | `<flow>: foreign-file: <syscall line>` (the existing rules' form) |
| S5 | `check-unsafe`, a keyword in the fuzz crate | `<path>:<line>: \`unsafe\` in the fuzz crate, which ci/unsafe-allowlist.toml does not list (doc 18 §11.2, NFR-SEC-04)` |

## 6. Build order

One lane builds the six steps in this order and commits at the end of each. Steps 1 to 4 and step 5's targets need
nothing of WP-S3.09; step 6 and the threat model's importer rows need WP-S3.09 merged (§12 O6).

1. **The threat model** (D1 to D4): the file with every TM row, the tables (WP-S3.09's rows marked "with WP-S3.09"
   until step 6), the 0.1.0 review entry, CONTRIBUTING's checklist line, the link from `docs/README.md`.
2. **`SECURITY.md`** (D11, D12): the full file; FT-1182.
3. **The disclaimers** (D13): `DISCLAIMER`, the two dialogs (frames of SE-3), the guide's paragraph, the release
   report's waivers if missing.
4. **The allowlist and the graph** (D7, D8): `check-unsafe` over `fuzz/` with its unit test; `ew-testkit` in
   `banned_crates` with an `xtask` unit test.
5. **The targets** (D5, D9): six harnesses, seeds (FT-223's SQL among them), the `[[bin]]` entries, the smoke table
   regenerated, the nightly's matrix; the replay test passes.
6. **The corpus and the isolation** (D10, D14, D15): the hostile files and their index, the counting allocator (its
   allowlist entry), the corpus test; the sandbox rule and fixtures; the two `catch_unwind` sites with S1 and S2; the
   threat model's importer rows completed from WP-S3.09's code, and the review entry dated.

## 7. Acceptance

- **SE-1** (NFR-SEC-01) `docs/security/threat-model.md` has TM-01 to TM-19 with D2's states, each control with a
  `path:line` that exists at the lane's tip, and a row of D3's tables for every parser and extension point with its
  target; `python -I docs/plan/tools/doccheck.py docs/security/threat-model.md` passes; RC-01's evidence names the
  review entry.
- **SE-2** (NFR-SEC-14, NFR-COMP-03, FT-1182) `SECURITY.md` carries D11's sections and D12's paragraph, its links
  resolve (`doccheck.py SECURITY.md`), the stub section is gone, `cargo xtask check-governance-files` passes, and the
  table names 0.1.x and its support end (RC-04).
- **SE-3** (NFR-COMP-04) Kittest: the approve and the release dialog end with S3 (whole); the About dialog and the check
  panel tests stay green; `ew check --format md` ends with S3; the release archive's `check/report.json` has S3, the
  packs and the waivers in force; the guide's introduction holds S3 (a text test over `docs/book/src/introduction.md`);
  frames of both dialogs at 1280 × 800 and 1920 × 1080.
- **SE-4** (NFR-SEC-04) A fixture fuzz folder with the keyword fails `check-unsafe` with S5 and the repository passes; a
  fixture graph with `ew-testkit` on a normal edge of a shipped binary fails `check-features` and on a dev edge passes;
  the counting allocator's file is listed and its blocks carry `SAFETY:` comments.
- **SE-5** (NFR-SEC-03) The six targets build (lane-check's `fuzz` step), their seeds replay clean (`fuzz/tests/replay.rs`
  at the merge point), the `xtask` table test passes, and the delivery's CI run has a 60-second `fuzz-smoke` per new
  target and a complete deep-nightly run of each without a finding.
- **SE-6** (FT-223, NFR-SEC-06's PR test) Each hostile SQL file is refused by the gate or the authorizer, or stopped by
  a limit or the deadline, with its `SqlError` text whole, in `ew-sqlview`'s tests; the same files are `sql_sandbox`'s
  seeds.
- **SE-7** (NFR-SEC-02) The corpus test refuses every file of D10 with its verdict whole and at most 256 MiB extra peak
  allocation each (§12 O3); the symbolic-link case runs on Unix only.
- **SE-8** (NFR-SEC-18) `python -I ci/sandbox-check.py --test` passes with the `foreign-file` cases; the delivery's
  `network-sandbox` run is green with both import fixtures.
- **SE-9** (NFR-REL-12) The two wrappers, given a panicking closure, give S1 and S2 and leave the project's files and
  the session's model digest unchanged; a recursive-CTE bomb in `ew sql` on a copy of Golden A stops at its deadline
  with the copy's files unchanged; WP-S3.09's IC-4 is the threat model's importer evidence.
- **SE-10** The hand-back checks of the lane's package set pass (clippy, fmt, doc, nextest, settle, policy, fuzz).

## 8. Neighbours, files owned and what a lane may not decide

### 8.1 Neighbours

| Neighbour | It owns | This package |
|---|---|---|
| WP-S3.09 | The importers' limits, front door, purity check, panic isolation and four targets | Reads them into the threat model; feeds the importer corpus files through the front door |
| WP-S6.05 | The network sandbox (built) | Adds one rule and two fixtures to its import flow |
| WP-S6.07 | The release checklist (RC-01, RC-02, RC-04, RC-09) | Gives each item its evidence |
| WP-S1.15 | rhai, the trust gate, NFR-SEC-05 and NFR-SEC-06's rhai half | The *not shipped* entries; D16 |
| WP-S6.10 | The CRA processes, the runbook, the tabletop exercise | `SECURITY.md`'s "What follows at 1.0" |

### 8.2 Files owned

`docs/security/threat-model.md` (new); `SECURITY.md`; `docs/book/src/introduction.md` (one paragraph);
`docs/README.md` (one link) and `CONTRIBUTING.md` (one checklist line); `crates/ew-checks/src/report.rs` (`DISCLAIMER`)
and the tests that assert it; `crates/ew-viewmodel/src/shell/revision.rs`, `strings/`, `locales/` and `crates/ew-ui/`
for the dialogs' line; `crates/ew-api/src/release_run.rs` (waivers, if missing), `crates/ew-api/src/output/table_io.rs`
and `bin/ew/src/cli.rs` (the two catches); `crates/ew-testkit/` (the corpus, its index and generator, the counting
allocator, the corpus test; its `Cargo.toml` lints); `crates/ew-sqlview/src/sandbox_tests.rs` (the hostile SQL);
`ci/unsafe-allowlist.toml`, `ci/dep-rules.toml` (`banned_crates`), `xtask/src/{unsafe_code,features}.rs` (their tests);
`fuzz/` (six targets, seeds, `Cargo.toml`, `Cargo.lock`), `ci/fuzz-smoke-targets.toml`,
`.github/workflows/deep-nightly.yml` (the matrix); `ci/sandbox/` and `ci/sandbox-check.py`. Read-only: `deny.toml`,
`supply-chain/`, `Cargo.lock`, `golden/**`, the importer crates, every other document.

### 8.3 What a lane may not decide

A lane stops and reports instead of choosing when:
1. a target or the corpus finds a crash, hang, out-of-memory or allocation above the bound: it is committed under
   `fuzz/regressions/<target>/` or reported with its file, never fixed in this package's change (doc 18 §3.12);
2. a parser of external bytes is found that D3 does not list;
3. `unsafe` seems needed beyond the counting allocator, or a crate's `unsafe` must be allowed;
4. a sentence of `SECURITY.md` would commit the project to something beyond D11 (a reward, a safe harbour, a
   response time), or a GitHub setting must change;
5. a text would differ from §5, or a new dependency seems needed.

The lane decides freely: the threat model's wording within D1 and D2, the harnesses' input encodings, the seeds, the
corpus file names and its generator, the job's step names, and the nl-NL wording (listed for the orchestrator).

## 9. Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 A new target finds a crash before 0.1.0 | Medium · medium | It is a finding (§8.3 item 1); RC-02 holds the release until it is fixed by its owner's fix round |
| R2 `sql_sandbox`'s projection image makes each run slow, so the 60-second smoke covers little | Medium · low | The image is built once per process; the nightly's 30 minutes is the coverage run |
| R3 The allocation bound of D10 is too tight for a legitimate refusal path | Low · low | The bound is a stop with the measured figure (§8.3 item 1), not a tuned constant |
| R4 Two lanes edit the fuzz crate's shared files at once | Medium · low | §12 O6: this lane after WP-S3.09, or its step 5 rebased on WP-S3.09's merge |

## 10. Size and routing

**Size.** About 1.1 pw: step 1 0.2, step 2 0.05, step 3 0.1, step 4 0.05, step 5 0.35, step 6 0.3. The PERT of 1.1 pw
(O 0.5 · L 1 · P 2) stays. No hand-back point is needed at this size; a lane that nears its window commits and hands
back with a note as ID-1282 says.

**Routing** (`.claude/orchestration.md` rule 0): one lane, **Opus at `medium`** (decided work with tests to run; one
`unsafe` file, reviewed by the cycle's reviewer, ID-1030), after WP-S3.09 (§12 O6), with WP-S1.09 and WP-S0.05a's
built parts.

## 11. Follow-ups, proposed

The design pass opens the rows. **FT-223** and **FT-1182** close with steps 5 and 2.
- **FP-1** The `rhai_formula` target and the non-terminating-script test with WP-S1.15's engine (D16).
- **FP-2** A fuzz target for the headless script runner's `.ewui` files.
- **FP-3** The worker-restart test of NFR-REL-12 with the first package that gives `ew-step-worker` work.
- **FP-4** The counting allocator in the fuzz crate, if a nightly allocation bound is wanted beside D10's.
- **FP-5** A role mailbox as a second reporting channel (doc 18 §11.3), once the project has a domain.

## 12. Open questions

- **O1** (the owner's) Private vulnerability reporting must be switched on in the repository's GitHub settings for
  `SECURITY.md`'s channel to work, and the response targets (7, 14, 30 and 90 days) become public commitments. Options:
  (1) the owner switches it on and accepts the targets of NFR-SEC-14 and doc 18 §11.3; (2) keep only NFR-SEC-14's 7
  and 30 days. Recommendation: (1).
- **O2** (the owner's) The support period of 0.1.0. Options: (1) "until the next minor release", by doc 18 §7.5's
  pre-1.0 rule; (2) a fixed date. Recommendation: (1); RC-04 then reads the row as the support end.
- **O3** The allocation half of NFR-SEC-02's 1 s / 256 MB. Options: (1) assert the extra peak allocation through a
  counting allocator (deterministic, not a timing assertion), time not measured; (2) neither, both "not measured"
  (owner's rule). Recommendation: (1); it needs D7's one `unsafe` file.
- **O4** NFR-REL-12's acceptance names a hostile STEP input and a non-terminating rhai script, neither of which exists
  at 0.1.0. Options: (1) 0.1.0 is proven by SE-9 and WP-S3.09's IC-4, the STEP and rhai halves stay with their
  packages (FP-1, FP-3) and 02c's row gains a 0.1.0 note; (2) keep the row's acceptance as a G-M2 item.
  Recommendation: (1).
- **O5** The rhai limits. Options: (1) WP-S1.15's remainder, with the engine (D16); (2) this package adds a limited
  engine profile in `ew-rhai` now, with no caller. Recommendation: (1).
- **O6** The build order with WP-S3.09. Options: (1) this lane after WP-S3.09 is merged; (2) in the same wave, steps 1
  to 5 first and step 6 after WP-S3.09's merge, with `fuzz/Cargo.toml`, `fuzz/Cargo.lock`, `fuzz/src/lib.rs`, the smoke
  table and the nightly's matrix merged by hand at the merge point. Recommendation: (1) unless the wave has room, then
  (2).
- **O7** The size: 1.1 pw, the PERT unchanged. Recommendation: keep it.

## 13. Documents to update

At the design pass after the lane: doc 18 §3.12 (the targets as built, the corpus and its allocation bound, no counting
allocator in `fuzz/`), §11.1 (the maintained copy is `docs/security/threat-model.md`; TM states), §11.2 (the allowlist
as built; the candidates enter by need; the fuzz crate scanned), §11.3 (as `SECURITY.md` states it), §18's Security row;
doc 14 §11 and §12 (the hostile set's place and names, the target names); 02c NFR-REL-12 and NFR-SEC-03 (0.1.0 notes,
§12 O4), NFR-SEC-06 (the rhai half with WP-S1.15); doc 15's approve and release dialog rows (the line as built);
ADR-0023 decision 4 (the threat model's file); doc 02's WP-S1.15 status (D16's limits named in its open items).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S6.04, WP-S3.09, WP-S6.05, WP-S6.07, WP-S6.10, WP-S1.15; FT-223,
  FT-332.
- [Decisions](../decisions.md): ID-1187, ID-1188, ID-1203, ID-1210, ID-1281, ID-1282, ID-1320, ID-1321;
  [decision log](../06-decision-log.md): ID-1030; [follow-ups](../follow-ups.md): FT-1182.
- [Quality](../../design/18-quality-security-release.md) §3.12, §5.4, §7.5, §11, §18;
  [interop](../../design/14-interop.md) §11, §12;
  [non-functional requirements](../../design/02c-nonfunctional-requirements.md) §5 (SEC), NFR-REL-12, NFR-COMP-03,
  NFR-COMP-04.
- [ADR-0020](../../adr/0020-extensibility-tiers-and-security-model.md),
  [ADR-0023](../../adr/0023-supply-chain-and-release-integrity.md).
- `SECURITY.md`; `docs/release/checklist.md`; the specifications of [WP-S3.09](WP-S3.09-importer-contracts-and-lineage.md),
  [WP-S6.05](WP-S6.05-privacy-and-crash-log.md) and [WP-S6.07](WP-S6.07-release-pipeline.md).
