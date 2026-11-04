# WP-S6.07: Release pipeline: sums, provenance, SBOM (closed specification)

**Status:** complete in cycle 50, both parts built by one lane and its fix round (decision log ID-1336, ID-1337); RP-3's tag half and RP-4 to RP-6 are proven at WP-S6.08's first candidate, and the installers join the release files when ID-476's hold lifts (ID-1331); specified in cycle 48 (documents only), an M2 package at the size of the owner's 0.1.0 scope (ID-1187), in
two parts: part 1 the release workflow from a tag, with the build checks, `SHA256SUMS`, provenance, publishing and the
release checklist (0.6 pw, D2 to D8, D10, D13, D14), part 2 the SBOM with its notices, the advisory gate and the canary
assertions (0.4 pw, D9, D11, D12); 0.1 pw reserve; both after WP-S6.06 parts 1 and 2, part 1 before part 2 ·
**Date:** 2026-11-03 · **Work package:**
[WP-S6.07](../02-work-packages.md#wp-s607-release-pipeline-sums-provenance-sbom) · **Decision log:** ID-1187,
ID-1188, ID-1210 · **Part of:** ElektraWatt plan, specifications

**Purpose.** No workflow of ElektraWatt reacts to a tag, and none creates a release, attests a file, writes
`SHA256SUMS` or an SBOM, or builds with `cargo auditable`. 0.1.0 is built by one release workflow from a tag, in a
protected environment with short-lived OIDC credentials, with `SHA256SUMS`, GitHub's build-provenance attestations,
`cargo-auditable` metadata and a CycloneDX SBOM; it is unsigned (ID-1187, ADR-0023 amendment 7). This document closes
doc 18's CI-R-01 to CI-R-11 and RC-01 to RC-10 in their 0.1.0 form to items, decisions and acceptance, and settles the
boundaries with WP-S6.06 (the artefacts and their install tests), WP-S6.08 (the release itself) and WP-S6.10 (signing
and the second approval, from 1.0). It is written against the code at `af780b80` (§2.1). Its size is an estimate, not
a commitment (ID-1210); every build and job time is not measured (owner's rule). Where this document and a design
document differ, this document is the narrower and wins for the lanes; the design documents stay the target and are
updated at the design pass after each part (§13). A choice a lane meets that this document does not settle is
escalated, never guessed.

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [Part 1: the release workflow](#5-part-1-the-release-workflow)
6. [Part 2: SBOM, advisories and canaries](#6-part-2-sbom-advisories-and-canaries)
7. [The release jobs in 0.1.0's form](#7-the-release-jobs-in-010s-form)
8. [Neighbours and who owns what](#8-neighbours-and-who-owns-what)
9. [Files owned and what a lane may not decide](#9-files-owned-and-what-a-lane-may-not-decide)
10. [Risks](#10-risks)
11. [Size, order and routing](#11-size-order-and-routing)
12. [The gate items and what WP-S6.08 keeps](#12-the-gate-items-and-what-wp-s608-keeps)
13. [Open questions, follow-ups and the documents to update](#13-open-questions-follow-ups-and-the-documents-to-update)
14. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package a maintainer pushes the tag `v0.1.0-rc.1` and the release workflow checks the
version, the notes and the commit's green runs, builds every artefact of WP-S6.06 with `cargo auditable` (hermetic on
Linux), proves each binary's embedded dependency data, its advisories and its canaries, writes an SBOM per artefact
complete against `cargo metadata` with SQLite, the fonts and the packaging runtimes in it, writes `SHA256SUMS`,
attests every file with GitHub's build provenance, and creates a **draft** prerelease holding them all, verified by
`sha256sum -c` and `gh attestation verify` after upload. Nothing is public until the go: then a dispatch of the same
workflow, in the protected `release` environment, publishes the candidate's own files as `v0.1.0`. The release
checklist of RC-01 to RC-10 in its 0.1.0 form is a template that WP-S6.08 fills.

**Non-goals** (each with its package or follow-up):
- No signing of any kind: no signed tag (CI-R-01's signature), no Authenticode, no signed `SHA256SUMS`, no release
  key, no update manifest (WP-S6.10, from 1.0, or earlier when the owner holds a signing identity; ID-1187). No macOS
  notarisation (ID-880).
- No second maintainer's approval of release jobs (WP-S6.10, 1.0).
- No reproducible builds (NFR-SEC-20, WP-S6.10); no crates.io publishing, no Flathub (1.0).
- No `ew-step-worker` or OCCT (CI-R-04 and OCCT's SBOM entry are 1.0's); no Golden A-R archive check (CI-R-02's 1.0
  part).
- No SBOM vulnerability scanner beyond `cargo audit` (D11, FP-1).
- No installer or install test (WP-S6.06), no network sandbox (CI-N-06, WP-S6.05), no steward checklist or LG-4
  (1.0, ID-1188).
- No change to `deny.toml`, `supply-chain/` or any crate: the pipeline adds tools, not dependencies.

## 2. Inputs and the code as it is

| Input | What the lanes take from it |
|---|---|
| [Quality](../../design/18-quality-security-release.md) §5.1, §5.2 (CI-R rows), §5.4, §7.4 (CI-R-01 to CI-R-11, RC-01 to RC-11), §9.1, §9.2, §10, §16 item 1 | The jobs, the hardening rules, the checklist |
| [Non-functional requirements](../../design/02c-nonfunctional-requirements.md) NFR-SEC-10 to NFR-SEC-13, NFR-SEC-19 | §3 |
| ADR-0023 (decision 2, amendment 7: the pilot release), ADR-0030 (the signing identity, 1.0) | What 0.1.0 carries and what it leaves |
| [Open-source governance](../../design/19-open-source-governance.md) §3.3 (LG-1), §5.3 (notices) | RC-10, D9 |
| [Gates](../03-gates.md) G-M2-13, G-M2-14, G-M2-15, G-M2-17, GP-11 | §12 |
| Decision log ID-454, ID-476, ID-723, ID-1082, ID-1187, ID-1188, ID-1203, ID-1210 | D12, §13 |
| Follow-ups FT-02 (`ICU4X_DATA_DIR` in the release build), FT-04 (non-crate notices), FT-333 (closed: the release check) | D7, D9 |
| [WP-S6.06](WP-S6.06-pilot-installers-and-platform-matrix.md) D3, D5, D6, D8, D11, D18 | The artefacts, the builds, `package.yml` and its `upload` input, the components list |

### 2.1 What exists at `af780b80` (the base contract)

The lanes rely on exactly these items, and on WP-S6.06 parts 1 and 2 as built. If one differs, the lane stops and
reports it.

| Item | Where | Used here for |
|---|---|---|
| No tag or release trigger, no `actions/attest-build-provenance`, no `cargo auditable`, no CycloneDX, no `SHA256SUMS`, no `gh release` in any workflow | `.github/workflows/` | Part 1 |
| `check-workflows`: actions pinned by full SHA with a version comment; top-level `contents: read` and no top-level write scope; a job may widen its own permissions; secrets only in a job named `release`; checkout without persisted credentials; images by digest | `xtask/src/workflows.rs:4-17`, `:127-158`, `:267-298`, `:329-360`; test `job_level_write_scope_is_allowed` `:780` | D4's environment rule |
| No job of any workflow grants a write scope or names an environment | `.github/workflows/*.yml` | D4 |
| `ci/release-check.sh env` (no `PKG_CONFIG*` override, `ICU4X_DATA_DIR` and `CARGO_ENCODED_RUSTFLAGS` unset, no `[env]` in a cargo configuration) and `scan` (no build-host path, no hard link to the dlopened graphics libraries, no x11-dl library directory); WP-S6.06 D6 runs both in the Linux release build | `ci/release-check.sh:5-38` | D7 |
| The supply-chain job: cargo-deny 0.20.2, cargo-audit 0.22.2, cargo-about 0.9.2 and cargo-hakari through `taiki-e/install-action` (pinned); `cargo deny`, `cargo audit`, `cargo vet`, `check-features`, `check-build-scripts`, `check-workflows`, the third-party notices generated and not uploaded | `.github/workflows/ci.yml:962-1035` | D6, D9, D11 |
| `deny.toml`: vulnerabilities and yanked crates deny; unmaintained for direct dependencies; `ignore = []` | `deny.toml` (`[advisories]`) | D11 |
| `check-features` reads the graph of every shipped binary for every target and fails on `egui_mcp`, `ew-testids` or a banned feature outside dev edges (`ew-commands/testkit`, `ew-viewmodel/testkit`, egui `inspection`, rhai `unchecked`, `ew/test-confirm`) | `ci/dep-rules.toml:30-32`, `:253-268`; `xtask/src/features.rs` | D12 |
| `ew` declares no `test-confirm` feature: doc 18 §3.15's hook is an injected terminal, and the ban stays as a guard (ID-1082) | `ci/dep-rules.toml:263-268`; `bin/ew/src/cli.rs:2364-2366` | D12 |
| `NOTICE` lists every bundled font file with its family, version, source and SHA-256, and the Tabler icons with their version and licence; the crates' notices are generated per release by cargo-about with `ci/third-party-notices.hbs` (one section per licence, each crate by name and version) | `NOTICE:10-83`; `ci/third-party-notices.hbs` | D9 |
| The workspace version `0.0.0`; release notes in `docs/release-notes/` (`unreleased.md`, `m1-draft.md`) | `Cargo.toml:15`; `docs/release-notes/` | D5 |
| `actions/upload-artifact` v7.0.1 and `actions/download-artifact` v7.0.0 pinned by SHA | `.github/workflows/package.yml` (since cycle 48, which removed `cli-artefact.yml`; ID-1265); `ci.yml:400` | Part 1 |
| ID-476's distribution hold (WP-S6.06 §13 O1) | `docs/plan/06-decision-log.md` ID-476 | D3's upload |

## 3. Requirements traced

| Requirement | State and what this package delivers | Part | Acceptance |
|---|---|---|---|
| NFR-SEC-10 | **Built per pull request** (CI-PR-09: `Cargo.lock`, the pinned toolchain, cargo-deny, cargo-audit, cargo-vet, the build-script allowlist). Here the release half: the network-less Linux build (WP-S6.06 D6) built with `cargo auditable` and `--locked` | 1 | RP-3 |
| NFR-SEC-11 | No release with a known unpatched high or critical advisory: `cargo deny check advisories` and `cargo audit` on the lock, `cargo audit bin` on every shipped binary; waivers by hand at RC-03 (D11) | 2 | RP-11 |
| NFR-SEC-12 (0.1.0 part) | Built by CI from a tag; `SHA256SUMS` (unsigned) and SLSA build provenance for every file; `sha256sum -c` and `gh attestation verify` pass on every file before the draft and at the promote (D8, D10, D13) | 1 | RP-4 to RP-6 |
| NFR-SEC-13 | A CycloneDX SBOM per artefact with SQLite, the fonts, the icons and the packaging runtimes, complete against `cargo metadata`; `cargo-auditable` data in every binary, read back from each (D9, D12) | 1, 2 | RP-3, RP-9, RP-11 |
| NFR-SEC-19 (0.1.0 part) | **Built:** actions pinned by SHA and read-only default tokens (`check-workflows`). Here: the jobs that write run in the protected `release` environment, credentials only the run's token and OIDC, and a lint rule that keeps it so (D4); the second approval is 1.0's | 1 | RP-1, RP-5, RP-6 |

## 4. Decisions

Each is a lead ruling that the owner may reverse.

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **The size.** Parts 1 and 2 at 1.0 pw and 0.1 pw reserve, the PERT of 1.1 pw (O 0.5 · L 1 · P 2): the PERT stays | WP-S6.06 builds and tests the artefacts; this package adds checks, sums, attestations and the publishing path around them |
| D2 | **The candidate is the release.** A tag `v<X.Y.Z>-rc.<n>` builds a **draft prerelease** named after it, holding every release file; the go publishes those same files under `v<X.Y.Z>` (D13). No final tag is built again and no public tag is moved: a blocker is fixed on `main` and the next candidate `-rc.<n+1>` is cut | The bytes the checklist tested are the bytes published (a rebuild would differ: builds are not reproducible before 1.0); a tag pushed to a public repository is public at once, so it is never re-pointed. Pre-1.0 has no release branches (doc 18 §7.3) |
| D3 | **One workflow, `release.yml`.** Triggers: `push` of tags matching `v[0-9]+.[0-9]+.[0-9]+-rc.[0-9]+`, and `workflow_dispatch` with the inputs `candidate` (a candidate tag) and `dry` (default `true`) for the promote. Jobs on a tag: `verify` (CI-R-01, CI-R-02), `package` (WP-S6.06's `package.yml` through `workflow_call` with `version: <X.Y.Z>` and `upload: true`, which runs CI-R-03, CI-R-05, CI-R-07, CI-R-09 and CI-R-10 per artefact), `sums` (CI-R-06), `attest` (CI-R-08), `release` (CI-R-11, the draft). On a dispatch: `release` alone (the promote). The `package` job's `upload: true` waits on ID-476's hold (WP-S6.06 §13 O1): until it lifts, no tag is pushed | One file to review; the per-artefact checks run in the job that holds the bytes, so the nightly runs them every night before any tag |
| D4 | **Permissions and the `release` environment.** Top level `contents: read`. `verify` adds `actions: read` (to read the commit's runs). `attest` adds `id-token: write` and `attestations: write`; `release` adds `contents: write`; both run in the environment `release` (no reviewer at 0.1.0; the second maintainer from 1.0). No job uses a secret: `release` uses the run's `GITHUB_TOKEN` through `${{ github.token }}`, and `attest` the run's OIDC token, both short-lived. `check-workflows` gains one rule: a job that grants any `write` scope must declare `environment: release` ("<file>:<line>: job `<job>` grants `<scope>: write` outside the `release` environment") | NFR-SEC-19 and ADR-0023 amendment 7: a protected environment with short-lived OIDC credentials. The rule keeps write scopes out of every other job, which no workflow grants today; the environment's settings are the owner's (§13 O1) |
| D5 | **CI-R-01, 0.1.0's form** (no tag signature, ID-1187). `ci/release-verify.py` (standard library; `--test` its offline unit test): the tag's `X.Y.Z` equals `[workspace.package].version`; `docs/release-notes/<X.Y.Z>.md` exists and is not empty; after the builds `git status --porcelain` is empty (no build step wrote into the checkout). `elektrawatt --version`, `ew --version` and the setup's `DisplayVersion` print `X.Y.Z` (WP-S6.06 IN-6, IN-8). The refusals, exact: "the tag v<t> does not match the workspace version <w>"; "no release notes: docs/release-notes/<X.Y.Z>.md is missing or empty" | Doc 18's version consistency (Cargo, installers, About) without the manifest of 1.0; the About dialog prints the crate version, so it follows by construction |
| D6 | **CI-R-02, 0.1.0's form.** `verify` reads the runs of the tagged commit (`gh run list --commit <sha> --workflow <file> --json conclusion`) and requires a run concluded `success` of `ci.yml` (the pull-request set with the acceptance runner and `check-features`) and of `deep-nightly.yml` (the full oracle, proptest, fuzz); refusal, exact: "no successful <workflow> run on <short sha>; run it on this commit before tagging". `perf.yml` is report-only and not asked (owner's rule). The advisory half is D11; the Golden A-R archive is 1.0's | The gates CI-R-02 lists, read from the runs that already exist instead of re-running them; the procedure (D14) dispatches the deep nightly on the commit before the tag |
| D7 | **CI-R-03.** `package.yml`'s two release builds run `cargo auditable build --release --locked` (WP-S6.06 D5 and D6 otherwise unchanged: the network-less container build on Linux, `ci/release-check.sh env` and `scan` there); on Windows `release-check.sh env` runs too (through bash; `PKG_CONFIG=false` is harmless there), so `ICU4X_DATA_DIR` and `CARGO_ENCODED_RUSTFLAGS` are asserted on both (FT-02). The tool: **cargo-auditable** (MIT OR Apache-2.0), pinned to its newest release at the lane's base (0.7.x when this was written, **verify**), installed by `taiki-e/install-action` when its manifest carries the tool with a checksum, else `cargo install --locked --version`; inside the Linux container it is built inside the container (WP-S6.06 D6). It embeds data and adds no dependency to the graph | Doc 18 CI-R-03 and NFR-SEC-13; one build, checked where it is made |
| D8 | **CI-R-06.** `sums` downloads every release file and writes `SHA256SUMS` in `sha256sum`'s format (`<hex>  <name>`, sorted by name) over every artefact, every SBOM and nothing else; unsigned at 0.1.0 | Doc 18 §9.2: offline users check with `sha256sum -c` |
| D9 | **CI-R-07: the SBOM and the notices** (`ci/sbom.py`, standard library; `--test` its offline unit test). Per artefact, in its OS job: **cargo-cyclonedx 0.5.9** (Apache-2.0; doc 18 and ADR-0023 name it) writes the CycloneDX JSON of each binary the artefact holds for its target (spec 1.5); `sbom.py merge` joins them (one component per purl) and adds the non-crate components: each file block of `NOTICE` (the bundled fonts with family, version, licence and SHA-256; the Tabler icons with their version), SQLite with the version read from the `sqlite3.h` that `libsqlite3-sys` bundles (from `cargo metadata`'s manifest path), and the entries of WP-S6.06's `packaging/components.toml` that the artefact contains (the type-2 runtime in the AppImage, NSIS's stub in the setup); written as `<artefact>.cdx.json`. `sbom.py complete` fails on any package of the binaries' normal-dependency closure for the target (`cargo metadata --filter-platform <target>`) that the SBOM lacks, naming it. `sbom.py notices` appends a "Non-crate components" section to the cargo-about output (each non-crate component with its licence and notice text from `NOTICE`, SQLite with its public-domain dedication) and fails on any SBOM component the resulting `THIRD-PARTY-NOTICES.txt` does not name (FT-04). The file is the one WP-S6.06's tree carries, as `.txt` (the window reads that name) | NFR-SEC-13 ("complete against `cargo metadata`"), 19 §5.3 and NFR-LIC-08; reading the non-crate components from `NOTICE`, the bundled header and the components list keeps one source for each and no new hand-kept list. OCCT joins with `ew-step-worker` at 1.0 |
| D10 | **CI-R-08.** `attest` runs **`actions/attest-build-provenance`** (MIT; v3 when this was written, the release current at the lane's base, pinned by commit SHA with its version comment, **verify**) once over every release file, `SHA256SUMS` included; `release` then runs `gh attestation verify <file> --repo <owner>/<repo>` on each file it downloads back from the draft (the GitHub CLI of the hosted image) | Doc 18 §9.2: Sigstore-signed SLSA provenance, free for public repositories; one attest step keeps `id-token: write` in one job |
| D11 | **The advisory gate (NFR-SEC-11).** In `package.yml`'s Linux job: `cargo deny check advisories` and `cargo audit` on `Cargo.lock` (the pinned versions of the supply-chain job); in each OS job: `cargo audit bin` on every shipped binary, which reads its `cargo-auditable` data. A waiver is an `ignore` entry in `deny.toml` and in `.cargo/audit.toml` with its reason, the owner's handle and an expiry within 90 days, checked by hand at RC-03. The "SBOM scan" of the non-crate components at 0.1.0 is SQLite's bundled version checked against SQLite's own vulnerability list by hand at RC-03; the fonts and icons are data. No scanner tool is added (FP-1) | Doc 18 §10 (`cargo-audit` on the binaries' embedded data at release); with OCCT absent, SQLite is the one non-crate component with code, and a scanner for one component is more pins than checks |
| D12 | **CI-R-09: the canary assertions, 0.1.0's form.** (a) **Graph:** `check-features` in `ci.yml`'s green run on the commit (D6). (b) **Binaries:** `ci/audit-data.py` (standard library; `--test` builds minimal ELF and PE images in memory) reads the `.dep-v0` section of every shipped binary, fails when it is missing ("no cargo-auditable data in <file>"), and fails when its package list names `egui_mcp`, `ew-testids` or `ew-testkit` ("<file> contains the test-only crate <name>"). (c) **No feature canary strings:** `ew/test-confirm` is not declared (ID-1082) and egui's `inspection` holds no string of EW's, so (a) is their proof. (d) **No listening socket:** on Linux `strace -f -e trace=listen` over the installed `elektrawatt --smoke` and `ew export pdf` on the sample records no `listen` call (NFR-SEC-09's smoke-start half; the network sandbox CI-N-06 is WP-S6.05's) | Doc 18's two canary strings presuppose a feature that ID-1082 left undeclared and a string egui does not carry; the embedded dependency list is read from the binary itself, which is the point of a canary |
| D13 | **CI-R-11: the draft and the promote.** On a candidate tag `release` creates the draft prerelease `v<X.Y.Z>-rc.<n>` with the release notes of D5, uploads every file, downloads them back and runs `sha256sum -c SHA256SUMS` and D10's verification; it publishes nothing. On a dispatch with `candidate` it finds that draft, repeats both verifications on its files, refuses when `v<X.Y.Z>` exists ("v<X.Y.Z> exists already; promote a new candidate") or a verification fails, and with `dry: false` creates the tag `v<X.Y.Z>` on the candidate's commit and a published release `v<X.Y.Z>` (not a prerelease, marked latest) with the candidate's files byte for byte and the notes; with `dry: true` (the default) it prints what it would publish and changes nothing. The draft stays as the record of the candidate | One job holds `contents: write`; the promote cannot publish bytes other than the verified candidate's; the dry run lets the first candidate prove the promote before the go |
| D14 | **The release procedure and the checklist.** `docs/release/README.md`: before the tag (the release notes, the deep nightly dispatched on the commit), cutting the candidate, reading the draft, running the checklist, the promote (dry, then real), a blocker (fix on `main`, next candidate), and how a user verifies a download (`sha256sum -c SHA256SUMS`; `gh attestation verify <file> --repo <owner>/<repo>`). `docs/release/checklist.md`: RC-01 to RC-10 in their 0.1.0 form (§7), each with its check, the evidence it asks for and who signs, which WP-S6.08 copies into the release dossier (GP-11). The guide's install page (WP-S6.06 part 2) gains "Verify a download" | The items a pipeline cannot prove (doc 18 §7.4), written once and filled per release |
| D15 | **What is not this package's:** CI-R-04 (`ew-step-worker`) is 1.0's; CI-R-05 (packaging) and CI-R-10 (the install tests) are WP-S6.06's, which `package.yml` runs inside this workflow | One owner per job |
| D16 | **Every part closed;** each part is re-read at dispatch against the delivered earlier parts and WP-S6.06 as built, without reopening D1 to D15 | — |

**The supply-chain rules.** Every tool is a CI tool outside the workspace's dependency graph, so `deny.toml`,
`supply-chain/` (cargo-vet) and `ci/dep-rules.toml` do not change and the licence boundary is not touched; each is
installed at a pinned version with checksum verification (doc 18 §5.4): cargo-auditable and cargo-cyclonedx 0.5.9 as
D7 says, cargo-deny 0.20.2, cargo-audit 0.22.2 and cargo-about 0.9.2 as the supply-chain job pins them; the one new
action, `actions/attest-build-provenance`, pinned by commit SHA (`check-workflows`); the GitHub CLI and `strace` are
the hosted image's, pinned by its label (doc 18 §5.3). No decision changes an ADR or an architecture edge.

## 5. Part 1: the release workflow

**Scope.** D2 to D8, D10, D13, D14. **0.6 pw.**

**Build list.**
1. `.github/workflows/release.yml` (new; D3): `verify`, `package`, `sums`, `attest`, `release`.
2. `ci/release-verify.py` (new; D5, D6's refusal) with `--test`.
3. `package.yml` (WP-S6.06's): `cargo auditable build` in both release builds and `release-check.sh env` on Windows
   (D7); its `upload: true` path takes the SBOM files too.
4. `xtask/src/workflows.rs`: D4's rule with its unit tests.
5. `docs/release/README.md` and `docs/release/checklist.md` (new; D14), linked from `docs/README.md`'s map; the
   "Verify a download" section of `docs/book/src/how-to/install.md`; `ci/README.md`'s release section.

**Acceptance.**
- **RP-1** `cargo xtask check-workflows` passes on every workflow with `release.yml`; D4's rule has unit tests: a job
  granting `contents: write`, `id-token: write` or `attestations: write` without `environment: release` fails with
  the exact text of D4, with it passes; every existing workflow passes unchanged.
- **RP-2** `python -I ci/release-verify.py --test` passes: a matching tag and notes pass; a tag of another version,
  missing notes and empty notes are refused with D5's texts, whole; a commit without a successful run is refused with
  D6's text.
- **RP-3** (CI run on the first candidate) both release builds run `cargo auditable build`; `release-check.sh env`
  passes on both OSes and `scan` on Linux; `git status --porcelain` is empty after them.
- **RP-4** (CI run) `SHA256SUMS` lists every artefact and SBOM of the run, sorted, and `sha256sum -c SHA256SUMS`
  passes on the files downloaded back from the draft.
- **RP-5** (CI run) `gh attestation verify` passes for every release file; `attest` is the only job holding
  `id-token: write`, and `attest` and `release` the only jobs in the `release` environment.
- **RP-6** (CI run) The candidate tag creates a draft prerelease with every release file and publishes nothing; a
  dispatch with `dry: true` names the tag and files it would publish and changes nothing. The real promote is
  WP-S6.08's go.
- **RP-7** `doccheck.py` passes with the two new documents; the guide's link check passes with the new section.

## 6. Part 2: SBOM, advisories and canaries

**Scope.** D9, D11, D12. **0.4 pw.**

**Build list.**
1. `ci/sbom.py` (new; D9: `merge`, `complete`, `notices`) and `ci/audit-data.py` (new; D12) with their `--test`.
2. `package.yml`'s OS jobs: cargo-cyclonedx and the three `sbom.py` steps per artefact, the notices file the trees
   take (D9), `cargo audit bin` and `audit-data.py` on every shipped binary, the lock's advisory checks in the Linux
   job (D11), the `strace` check (D12 (d)); `.cargo/audit.toml` with an empty `ignore` and its comment (D11's waiver
   form).
3. `ci/README.md`'s rows for the steps.

**Acceptance.**
- **RP-8** `python -I ci/sbom.py --test` passes: a fixture cargo-cyclonedx output, a fixture `NOTICE`, a fixture
  `sqlite3.h` and a fixture components list give an SBOM holding the non-crate components with their versions and
  licences; a package of the fixture closure missing from the SBOM fails naming it; a component the notices do not
  name fails naming it; the notices gain the "Non-crate components" section.
- **RP-9** (CI run) every artefact has its `<artefact>.cdx.json`, complete against `cargo metadata` for its binaries and
  target, with SQLite at the bundled version, the bundled fonts, the Tabler icons, and NSIS in the setup's and the
  type-2 runtime in the AppImage's.
- **RP-10** `python -I ci/audit-data.py --test` passes: the package list is read from a minimal ELF and a minimal PE
  image; an image without the section and an image naming a banned crate fail with D12's texts, whole.
- **RP-11** (CI run) every shipped binary carries its `cargo-auditable` data and names no banned crate; `cargo audit
  bin` passes on each; `cargo deny check advisories` and `cargo audit` pass on the lock; the `strace` log shows no
  `listen` call; all of these also run in the nightly `package.yml`.

## 7. The release jobs in 0.1.0's form

| Job | 0.1.0's form | Where |
|---|---|---|
| CI-R-01 | Version and notes (D5); no tag signature | Part 1 |
| CI-R-02 | Green `ci` and `deep-nightly` runs on the commit (D6); advisories (D11) | Parts 1, 2 |
| CI-R-03 | `cargo auditable build --release --locked`, network-less on Linux, the environment assertions on both OSes (D7) | Part 1; WP-S6.06 D6 |
| CI-R-04 | — (1.0) | — |
| CI-R-05 | The setup, the AppImage, the CLI archives | WP-S6.06 |
| CI-R-06 | `SHA256SUMS`, unsigned (D8) | Part 1 |
| CI-R-07 | SBOM per artefact, complete, with the notices (D9) | Part 2 |
| CI-R-08 | Build provenance and its verification (D10) | Part 1 |
| CI-R-09 | Graph, embedded data, no listening socket (D12) | Part 2 |
| CI-R-10 | The install tests (CI half; the desktops by hand) | WP-S6.06 D11 |
| CI-R-11 | The draft, then the promote in the `release` environment (D13) | Part 1 |

| Item | 0.1.0's form in `docs/release/checklist.md` |
|---|---|
| RC-01 | The threat model (WP-S6.04) reviewed for the parsers and extension points added since the last review |
| RC-02 | No open fuzz crash (`fuzz/regressions/` and the deep nightly's findings); quarantined tests at most 10 |
| RC-03 | Every advisory waiver in `deny.toml` and `.cargo/audit.toml` has a reason, an owner and an expiry within 90 days; SQLite's bundled version has no open vulnerability on SQLite's list (D11) |
| RC-04 | `SECURITY.md`'s supported-versions table names 0.1.0 and its support end |
| RC-05 | Manual tests: RDP without GPU, NVDA and Orca, two printers, Windows network observation, and WP-S6.06's four desktop installs (its IN-15) |
| RC-06 | Translation status meets NFR-I18N-01; the guide is built and in every installer |
| RC-07 | Content audit of the bundled assets: licences, provenance, no standards text, no Tahoma |
| RC-08 | Derivation profiles, format changes and CLI or API changes listed in the release notes |
| RC-09 | The engineering-verification disclaimer in About, the check panel, the guide and the release report |
| RC-10 | LG-1, the employer copyright statement, on file (ID-1203); LG-4 and the steward checklist are 1.0's |

## 8. Neighbours and who owns what

| Neighbour | It owns | This package |
|---|---|---|
| WP-S6.06 | `package.yml`, the artefacts, the builds, the install tests, `packaging/components.toml` | Calls `package.yml` with `upload: true`; adds `cargo auditable` and the per-artefact steps to its OS jobs |
| WP-S6.05 | The network sandbox (CI-N-06), NFR-SEC-09's full proof | D12 (d) only checks for a listening socket |
| WP-S6.04 | `SECURITY.md`, the threat model | RC-01 and RC-04 check them |
| WP-S6.08 | The version, the notes, the candidates, the checklist with its evidence, the dossier, the go and the promote | Gives the workflow, the procedure and the template |
| WP-S6.10 | Signing, the signed tag and sums, the release key, the second approval, reproducible builds, crates.io | Leaves the `release` environment ready for its reviewers and secrets |

## 9. Files owned and what a lane may not decide

### 9.1 Files owned

**Part 1:** `.github/workflows/release.yml`, `ci/release-verify.py`, `.github/workflows/package.yml` (the build
command and the Windows `env` step), `xtask/src/workflows.rs` (the rule and its tests), `docs/release/**`,
`docs/README.md` (one map line), `docs/book/src/how-to/install.md` (one section), `ci/README.md`. **Part 2:**
`ci/sbom.py`, `ci/audit-data.py`, `.cargo/audit.toml`, `package.yml` (the per-artefact steps), `ci/README.md`.

Shared files (`package.yml`, `ci/README.md`) may be touched by WP-S6.06's parts; the second to integrate rebases.
Read-only for every lane: every crate, `deny.toml`, `supply-chain/**`, `ci/dep-rules.toml`, `Cargo.lock`, `NOTICE`,
`golden/**`, `docs/**` except the files above.

### 9.2 What a lane may not decide

A lane stops and reports instead of choosing when:
1. a step needs a secret, a write scope outside `attest` and `release`, a self-hosted runner, or a GitHub setting
   (§13 O1);
2. a change needs a new Rust dependency, a crate change, or a change to `deny.toml`, `supply-chain/` or `NOTICE`;
3. an advisory, a missing notice or an incomplete SBOM appears on the real graph: it is a finding, reported with its
   package, never waived or listed away by the lane;
4. a tool cannot be pinned by version with a checksum, or its licence is outside the NFR-LIC-04 allowlist;
5. a path would publish bytes other than a verified candidate's, or move a pushed tag;
6. the work of a neighbour (§8) is reached.

The lane decides freely: job and step names, the scripts' structure, the CycloneDX serial number and metadata fields,
test fixtures, and the wording of the procedure.

## 10. Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 The workflow can be proven end to end only by a tag, which waits on ID-476's hold and the owner's settings | Certain · medium | Every per-artefact check runs nightly in `package.yml`; the scripts have offline tests; the promote has a dry run |
| R2 cargo-cyclonedx 0.5.9's output for a binary misses or adds packages against `cargo metadata`'s closure (build dependencies, platform filters) | Medium · medium | `complete` names each difference; a systematic one is reported with its probe, not patched in the SBOM |
| R3 A new advisory turns the nightly or a candidate red | Medium · low | That is the gate working; the fix or a waiver follows RC-03 |
| R4 The attestation action's major version moves | Low · low | Pinned by SHA; Renovate proposes the update |
| R5 A GitHub setting (environment, tag ruleset, immutable releases) is missing at the first candidate | Medium · medium | §13 O1 names each; the procedure checks them before the tag |

## 11. Size, order and routing

**Size.** 1.0 pw plus 0.1 pw reserve: part 1 **0.6** (the workflow 0.25, verify and its script 0.1, the build
switch 0.05, the lint rule 0.05, the procedure, checklist and guide section 0.15), part 2 **0.4** (the SBOM script and
its steps 0.2, the embedded-data script and the canaries 0.1, the advisory steps and the notices 0.1). **De-scope
rule:** when a part is projected or found more than 20 % over its size, these leave as follow-ups without a further
ruling, in this order: for part 1 the promote's dry run (the promote verifies as before), then D4's lint rule; for
part 2 the `strace` check (the graph and embedded-data checks stay).

**Order and routing** (`.claude/orchestration.md` rule 0). Part 1 **Opus at `high`** (the publishing path, tokens and
the environment: a mistake publishes), after WP-S6.06 parts 1 and 2; part 2 **Sonnet at `medium`** (decided scripts
with offline tests), after part 1, or beside it once WP-S6.06 part 1 has landed, since it touches `package.yml` and
the scripts only.

## 12. The gate items and what WP-S6.08 keeps

- **G-M2-13** (CI-R-01 to CI-R-11 in 0.1.0's form): this package delivers CI-R-01 to CI-R-03, CI-R-06 to CI-R-09 and
  CI-R-11; WP-S6.06 CI-R-05 and CI-R-10; CI-R-04 is 1.0's. "Ran end to end on the release candidate" is WP-S6.08's
  candidate run.
- **G-M2-14**: the advisory half of CI-R-02 (D11) is its log; RC-02 and RC-03 are WP-S6.08's.
- **G-M2-15**: CI-R-09's listening-socket half (D12 (d)); the rest is WP-S6.05's.
- **G-M2-17**: the checklist template (D14, §7); its completion with evidence is WP-S6.08's.
- **G-M2-19** stays not applicable; signing is WP-S6.10's.
- **WP-S6.08 keeps:** the version bump, the release notes `docs/release-notes/0.1.0.md`, the deep nightly on the
  commit, cutting the candidates, the checklist with its evidence, the release dossier (GP-11), the go and the
  promote.

## 13. Open questions, follow-ups and the documents to update

**Open questions for the orchestrator** (O1 to O4 are the owner's; none is decided here).
- **O1** (the owner's GitHub settings, with FT-960) Before the first candidate: (a) the environment `release`, its
  deployment rule limited to `v*` tags and `main`, no reviewer at 0.1.0; (b) a tag ruleset on `v*` that lets only
  maintainers create tags, forbids updating and deleting them, and lets the workflow's token create the final tag at
  the promote; (c) immutable releases on, if offered; (d) the default workflow token read-only; (e) the required
  checks with WP-S6.06's `c-only` job and FT-960's `api-guard`. Options: (1) set all five before the first candidate;
  (2) set (a), (b) and (d) only. Recommendation: (1).
- **O2** (the owner's) ID-723 accepted the AI-performed audits and recorded exemptions for the pilot's 0.0.x builds,
  and ID-1203 puts the burn-down before 1.0's first signed release; 0.1.0, a public binary release, lies between.
  Options: (1) confirm that they stand for 0.1.0; (2) audit or replace the shipped `safe-to-deploy` exemptions
  first (not sized here, large). Recommendation: (1).
- **O3** (the owner's: legal) RC-10 at 0.1.0 is LG-1, the employer copyright statement (ID-1203), while ID-454 took
  legal items off the gates. Options: (1) the owner files the statement, or states that no employer's staff
  contributed, in the dossier; (2) RC-10 is not applicable at 0.1.0 by a ruling. Recommendation: (1).
- **O4** (the owner's) Who cuts candidates and dispatches the promote at 0.1.0. Recommendation: the owner, the one
  maintainer; the second approval is 1.0's.
- **O5** The PERT stays at 1.1 pw (D1). Recommendation: keep it.

The signing identity stays the owner's (ID-1187, ID-1210); nothing here asks for it.

**Follow-ups, proposed** (the design pass opens the rows). **FT-02** closes with part 1 (the release build's
assertion); **FT-04** with part 2.
- **FP-1** An SBOM vulnerability scanner over the non-crate components, with OCCT at 1.0 (WP-S6.10).
- **FP-2** The waiver expiry of RC-03 checked by a script instead of by hand.
- **FP-3** `SECURITY.md` points to "Verify a download" (WP-S6.04).
- A de-scoped item of §11 becomes a new FT at the design pass.

**Documents to update** (at the design pass after each part). **After part 1:** doc 18 §5.1 (the flow: candidate,
draft, promote), §5.2 (the CI-R rows as built; the pre-release row of CI-R-03 folded into the release builds), §5.4
(the `release` environment holds `attest` and `release`; the lint rule), §7.4 (CI-R-01, CI-R-02, CI-R-06, CI-R-08 and
CI-R-11 in 0.1.0's form; the checklist file), §9.2 (verification with `--repo`); `ci/README.md`; `docs/README.md`.
**After part 2:** doc 18 §7.4 CI-R-07 (the sources of the non-crate components) and CI-R-09 (D12's form instead of
the canary strings), §10 (the waiver form in two files); 02c NFR-SEC-11 (the "SBOM scan" at 0.1.0, D11).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S6.07, WP-S6.06, WP-S6.08, WP-S6.10, WP-S6.05, WP-S6.04; FT-02, FT-04,
  FT-333, FT-960.
- [Decision log](../06-decision-log.md): ID-454, ID-476, ID-723; [decisions](../decisions.md): ID-1082, ID-1187,
  ID-1188, ID-1203, ID-1210.
- [Quality](../../design/18-quality-security-release.md) §5, §7.4, §9, §10, §16;
  [non-functional requirements](../../design/02c-nonfunctional-requirements.md);
  [open-source governance](../../design/19-open-source-governance.md) §3.3, §5.3; [gates](../03-gates.md) G-M2, GP-11.
- [ADR-0023](../../adr/0023-supply-chain-and-release-integrity.md),
  [ADR-0030](../../adr/0030-legal-steward-cra-role-support-signing.md).
- The specification of [WP-S6.06](WP-S6.06-pilot-installers-and-platform-matrix.md).
