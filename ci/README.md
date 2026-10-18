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
| CI-PR-01 | `ci.yml` / `lint` (ubuntu-24.04) | `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings` | `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| CI-PR-02 | `ci.yml` / `test-linux` (ubuntu-24.04) | `cargo build --workspace --locked`; `cargo nextest run --workspace --locked --profile ci` | `cargo build --workspace --locked` and `cargo nextest run --workspace --locked` |
| CI-PR-03 | `ci.yml` / `test-windows` (windows-2025) | as CI-PR-02 | as CI-PR-02, on Windows |
| CI-PR-04 | `ci.yml` / `test-macos` (macos-15, arm64) | as CI-PR-02 | as CI-PR-02, on macOS |
| CI-PR-09 | `ci.yml` / `supply-chain` (ubuntu-24.04) | `cargo deny check`; `cargo audit`; `cargo vet`; `cargo xtask check-deps`, `check-build-scripts`, `check-workflows`; MSRV check of the permissive crates with Rust 1.85.0 | `cargo deny --locked check`, `cargo audit`, `cargo vet --locked`, the three `cargo xtask` checks, and the MSRV commands below |
| CI-PR-10 | `docs.yml` / `legal-docs` (ubuntu-24.04) | Skeleton: `docs/plan/tools/{totals,schedule,trace}.py --check` and `doccheck.py`. WP-S0.02 adds `reuse lint`, the DCO check, the licence-map check and the anonymisation guard to this job | `python3 -I docs/plan/tools/totals.py --check` (and the other three) |
| CI-PR-15 | `docs.yml` / `docs-diagrams` (ubuntu-24.04) | Renders every Mermaid block under `docs/` and in the root Markdown with mermaid-cli 12.0.0 (container image pinned by digest) and fails on any error | `python3 -I ci/render-mermaid.py --image <image@sha256:…>` (needs docker); `--extract-only` lists the blocks without docker |

`cargo xtask ci` runs fmt, clippy, nextest and the three policy checks in sequence and prints a summary: the
local mirror of the Rust part of the PR set.

The MSRV step builds the permissive crates against a lockfile that the MSRV-aware resolver generates inside
the job (18 §6.2), so the committed application lockfile can move ahead of the MSRV:

```sh
CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback cargo +1.85.0 generate-lockfile
cargo +1.85.0 check --locked -p ew-units -p ew-ids -p ew-mltext -p ew-rds -p ew-schema -p ew-format \
  -p ew-interop-api -p ew-dl -p ew-step-proto -p ew-memo -p ew-plcaddr -p ew-caex -p ew-devdesc -p ew-ewpp
git checkout Cargo.lock   # locally: restore the committed lockfile afterwards
```

## Policy files

| File | Checked by | Content |
|---|---|---|
| [`dep-rules.toml`](dep-rules.toml) | `cargo xtask check-deps` | Layers, the permitted crate-to-crate edges (the ARCH §3 graph), external-crate owners (rusqlite, egui, wgpu), the importer rule and the `ew-import-qet` confinement (NFR-MAINT-01, ADR-0024) |
| [`build-scripts.allowlist`](build-scripts.allowlist) | `cargo xtask check-build-scripts` | Every package with a build script, after review (ADR-0023, NFR-SEC-10) |
| [`../deny.toml`](../deny.toml) | `cargo deny check` | Licence allowlist (NFR-LIC-04), advisories, bans, sources |
| [`../supply-chain/`](../supply-chain) | `cargo vet` | Audits and the exemption list frozen at the first commit (ADR-0023). No public audit sets are imported yet, because `imports.lock` would record the auditors' names and e-mail addresses (open decision) |
| [`render-mermaid.py`](render-mermaid.py) | CI-PR-15 | Mermaid extraction and rendering |

## Rules for the workflows

- Third-party actions are pinned by full commit SHA with a `# vX.Y.Z` comment; Renovate
  ([`.github/renovate.json`](../.github/renovate.json)) proposes updates weekly, grouped. Container images are
  pinned by digest.
- Every workflow declares a top-level `permissions:` block with `contents: read` and no write scope; a job that
  needs more widens it for that job only. `permissions: write-all` is never used. `cargo xtask check-workflows`
  enforces these rules (NFR-SEC-19).
- No workflow runs fork code with secrets; `pull_request_target` is not used.
- CI tools are installed at pinned versions: through `taiki-e/install-action`, which verifies checksums, or with
  `cargo install --locked --version`.
- Tests are never retried (NFR-MAINT-13; `retries = 0` in [`.config/nextest.toml`](../.config/nextest.toml)).

## Branch protection (owner action on GitHub)

These settings cannot be committed; a repository owner applies them under Settings → Rules (or Branches) for
`main` and `release/*` (18 §5.4):

- Require status checks to pass before merging, with these required checks: `lint`, `test-linux`,
  `test-windows`, `test-macos`, `supply-chain`, `legal-docs`, `docs-diagrams`. Require branches to be up to
  date before merging.
- Block force pushes and deletions; no direct pushes (changes land through pull requests only, administrators
  included).
- Require one approving review by someone other than the author; dismiss stale approvals on new commits.
- Require review from Code Owners (the `CODEOWNERS` file arrives with WP-S0.02/WP-S0.03).
- Settings → Code security: enable secret scanning with push protection.
- Settings → Actions → General: require actions to be pinned to a full-length commit SHA, and set the default
  workflow permissions to "Read repository contents".

Both workflows run on every pull request, so all seven jobs can be required checks. `docs-diagrams`
renders every diagram each time (about 90 blocks, a few minutes) rather than filtering on changed paths, because
a required check whose workflow did not run blocks the merge.
