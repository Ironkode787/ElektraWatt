<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Releasing ElektraWatt

The procedure of a binary release in its 0.1.0 form (WP-S6.07 D14; [doc 18](../design/18-quality-security-release.md)
§5.1, §7.4, §9.2). The workflow is [`release.yml`](../../.github/workflows/release.yml), its scripts are
[`ci/release-verify.py`](../../ci/release-verify.py) and the per-artefact steps of
[`package.yml`](../../.github/workflows/package.yml); the items a pipeline cannot prove are the
[release checklist](checklist.md). The specification is
[WP-S6.07](../plan/specs/WP-S6.07-release-pipeline.md); the first release, its notes, its candidates and its dossier
are WP-S6.08's. The next release, 0.2.0, carries the symbol editor (milestone M2.5, ID-1402) and goes through this
procedure in the same form; it is the first after the format locks at 0.1.0, so its notes name every format change
and its migration.

## 1. The candidate is the release

A tag `v<X.Y.Z>-rc.<n>` builds a **draft prerelease** of that name that holds every release file. Nothing is public
until the go: then a dispatch of the same workflow publishes the candidate's own files, byte for byte, as `v<X.Y.Z>`.
No final tag is built again and no pushed tag is ever moved or deleted: a tag on a public repository is public at once,
and a rebuild would not give the same bytes (builds are not reproducible before 1.0). A blocker is fixed on `main` and
the next candidate, `-rc.<n+1>`, is cut.

The release files of 0.1.0 are the x86_64 CLI archives (`ew-<X.Y.Z>-x86_64-pc-windows-msvc.zip`,
`ew-<X.Y.Z>-x86_64-unknown-linux-gnu.tar.gz`), the SBOM of every artefact (`<artefact>.cdx.json`: the two archives,
`ElektraWatt-<X.Y.Z>-x64-user-setup.exe` and `ElektraWatt-<X.Y.Z>-x86_64.AppImage`) and `SHA256SUMS` over them. The
setup and the AppImage join when ID-476's distribution hold lifts (WP-S6.06 §13 O1); the best-effort aarch64 archives
are not release files at 0.1.0. Nothing is signed at 0.1.0 (ID-1187): no signed tag, no Authenticode, no signed
`SHA256SUMS`.

## 2. Before the first candidate: the repository's settings

These are the owner's (WP-S6.07 §13 O1). Check each before the first tag:

1. The environment `release` exists, its deployment rule allows `v*` tags and `main` only, and it has no reviewer at
   0.1.0 (the second maintainer's approval comes with 1.0).
2. A tag ruleset on `v*` forbids updating and deleting tags, with no bypass. It does not restrict their creation:
   the promote creates the final tag with the workflow's own token, which a ruleset cannot list as a bypass actor, and
   the repository's only writers are its maintainers.
3. Immutable releases are on, if the repository offers them.
4. The default workflow token is read-only.
5. The required checks of `main` hold WP-S6.06's `c-only` job and FT-960's `api-guard`.

## 3. Before the tag

1. The version: `[workspace.package].version` of `Cargo.toml` is `X.Y.Z`, merged on `main`.
2. The release notes: `docs/release-notes/<X.Y.Z>.md`, with the changes, the format, CLI and API changes (RC-08) and
   the notices of the AppImage's runtime (NFR-LIC-04's amendment), merged on `main`.
3. `ci.yml` has a green run on the commit to be tagged (the push to `main` starts it).
4. The deep nightly runs on that commit and is green: `gh workflow run deep-nightly.yml --ref main` while `main` is
   that commit, then wait for its result. `perf.yml` is report-only and not asked (owner's rule).
5. The settings of §2 hold.

The workflow's `verify` job refuses the candidate when one of 1 to 4 does not hold, with one of these texts:
"the tag v<t> does not match the workspace version <w>", "no release notes: docs/release-notes/<X.Y.Z>.md is missing
or empty", "no successful <workflow> run on <short sha>; run it on this commit before tagging".

## 4. Cutting the candidate

```text
git tag v<X.Y.Z>-rc.<n> <commit>
git push origin v<X.Y.Z>-rc.<n>
```

The push starts `release.yml`: `verify`, then `package` (WP-S6.06's packaging with `upload: true`: the
`cargo auditable` builds, the release checks, the SBOMs, the install tests, the advisory and canary checks), `sums`
(`SHA256SUMS`), `attest` (the build provenance of every file, `SHA256SUMS` included) and `release` (the draft
prerelease, its files downloaded back and verified with `sha256sum -c SHA256SUMS` and `gh attestation verify`). No tag
is pushed while ID-476's hold stands. The first candidate is the first run of the tag half (RP-3's tag half and RP-4 to
RP-6 of WP-S6.07 are proven there, by WP-S6.08); the per-artefact checks run every night in `package.yml`.

## 5. Reading the draft

On the repository's releases page, the draft `v<X.Y.Z>-rc.<n>` lists the files of §1 and the release notes. The run's
`release` job log ends with "the draft prerelease v<X.Y.Z>-rc.<n> holds <n> verified files; nothing is published". The
summaries of `package`'s jobs carry the release-check reports, the container's packages and the SBOMs written.
Download the files and check them as a user would (§9).

## 6. The checklist

Fill [the checklist](checklist.md) for the candidate: each item with its evidence and its signature. WP-S6.08 copies
the filled checklist into the release dossier (GP-11). A failed item is a blocker (§8).
The dossier of 0.1.0 is [`dossier/0.1.0/`](dossier/0.1.0/README.md): the checklist of each candidate as
`checklist-rc.<n>.md`, the acceptance run, the translation report, the manual tests and the go.

## 7. The promote

First the dry run, which prints the tag, the commit and the files it would publish and changes nothing:

```text
gh workflow run release.yml -f candidate=v<X.Y.Z>-rc.<n> -f dry=true
```

Then, after the go, the real one:

```text
gh workflow run release.yml -f candidate=v<X.Y.Z>-rc.<n> -f dry=false
```

It refuses when `v<X.Y.Z>` exists ("v<X.Y.Z> exists already; promote a new candidate") or when the candidate is no
draft; it downloads the draft's files and verifies them again (`SHA256SUMS` lists exactly the files, `sha256sum -c`,
`gh attestation verify` on each), creates the tag `v<X.Y.Z>` on the candidate's commit and the published release
`v<X.Y.Z>` (marked latest, not a prerelease) with those files and the draft's notes, and verifies the published files
once more. The draft stays as the record of the candidate.

## 8. A blocker

Fix it on `main` through the usual pull request, then cut the next candidate, `v<X.Y.Z>-rc.<n+1>`, from §3. Leave
the earlier candidate's tag and draft as they are: they record what was tested.

## 9. How a user verifies a download

The guide's install page has the user's form ("Verify a download"). In short, with the downloaded files and
`SHA256SUMS` in one folder, and the GitHub CLI signed in once with `gh auth login`:

```text
sha256sum --check --ignore-missing SHA256SUMS
gh attestation verify <file> --repo Ironkode787/ElektraWatt --signer-workflow Ironkode787/ElektraWatt/.github/workflows/release.yml
```

`--signer-workflow` binds the file to `release.yml`, not only to the repository. The promote also passes
`--source-digest <the candidate's commit>`, so the tag `v<X.Y.Z>` lands only on the commit the bytes were built from.

## 10. What the workflow proves

| Job (doc 18 §7.4) | Where | 0.1.0's form |
|---|---|---|
| CI-R-01 | `release.yml` `verify` | The tag's version is the workspace version; the release notes exist; no tag signature |
| CI-R-02 | `release.yml` `verify`; `package.yml` | Green `ci.yml` and `deep-nightly.yml` runs on the commit; `cargo deny check advisories`, `cargo audit` on the lock and `cargo audit bin` on every binary |
| CI-R-03 | `package.yml` `windows`, `linux` | `cargo auditable build --release --locked`, network-less on Linux; `ci/release-check.sh env` on both OSes, `scan` on Linux; the checkout clean after the build |
| CI-R-05, CI-R-10 | `package.yml` (WP-S6.06) | The setup, the AppImage, the CLI archives and their install tests |
| CI-R-06 | `release.yml` `sums` | `SHA256SUMS` over every release file, unsigned |
| CI-R-07 | `package.yml` | The SBOM of every artefact, complete against `cargo metadata`, and the notices of its non-crate components |
| CI-R-08 | `release.yml` `attest`, `release` | Build provenance of every file; `gh attestation verify` after the upload |
| CI-R-09 | `ci.yml` `check-features`; `package.yml` | The release graph without test-only crates or features; each binary's embedded dependency list without them; no listening socket on the smoke start and an export |
| CI-R-11 | `release.yml` `release` | The draft on a candidate tag; the promote, dry or real, on a dispatch |

CI-R-04 (`ew-step-worker`) is 1.0's. The waivers of an advisory are an `ignore` entry in `deny.toml` and in
`.cargo/audit.toml`, each with its reason, owner and expiry; RC-03 checks them by hand.
