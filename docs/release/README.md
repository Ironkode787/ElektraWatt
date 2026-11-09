<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Releasing ElektraWatt

The procedure of a binary release in its 0.1.x form (WP-S6.07 D14; [doc 18](../design/18-quality-security-release.md)
§5.1, §7.4, §9.2), slimmed to an early alpha's size (ID-1520): no CI run is waited for (CI is off until 1.0,
ID-1519), and the release builds carry no SBOM, no build provenance and no install tests. The workflow is [`release.yml`](../../.github/workflows/release.yml), its scripts are
[`ci/release-verify.py`](../../ci/release-verify.py) and the per-artefact steps of
[`package.yml`](../../.github/workflows/package.yml); the items a pipeline cannot prove are the
[release checklist](checklist.md). The specification is
WP-S6.07; the first release, its notes, its candidates and its dossier
are WP-S6.08's. The next release, 0.2.0, carries the symbol editor (milestone M2.5, ID-1402) and goes through this
procedure in the same form; it is the first after the format locks at 0.1.0, so its notes name every format change
and its migration.

## 1. The candidate is the release

A tag `v<X.Y.Z>-rc.<n>` builds a **draft prerelease** of that name that holds every release file. Nothing is public
until the go: then a dispatch of the same workflow publishes the candidate's own files, byte for byte, as `v<X.Y.Z>`.
No final tag is built again and no pushed tag is ever moved or deleted: a tag on a public repository is public at once,
and a rebuild would not give the same bytes (builds are not reproducible before 1.0). A blocker is fixed on the release
branch and the next candidate, `-rc.<n+1>`, is cut.

**The release branch** (ID-1412). A minor release `X.Y` is cut from its own branch `release/<X.Y>`, branched from the
working branch at the delivery that starts the release: the version, the notes, the candidates' commits and their
fixes are on it, and only candidate fixes land there. The promote is dispatched from `main` (the environment `release`
allows `main` and `v*` tags) and publishes the candidate's own files; no job requires the tagged commit to be on
`main`.

The release files of 0.1.0 are the x86_64 CLI archives (`ew-<X.Y.Z>-x86_64-pc-windows-msvc.zip`,
`ew-<X.Y.Z>-x86_64-unknown-linux-gnu.tar.gz`), the per-user Windows setup (`ElektraWatt-<X.Y.Z>-x64-user-setup.exe`)
and the Linux AppImage (`ElektraWatt-<X.Y.Z>-x86_64.AppImage`), and `SHA256SUMS` over them: five files. ID-476's distribution hold is lifted for the release (ID-1331 (2)); the
aarch64 archives are not built at 0.1.x. Nothing is signed at 0.1.0 (ID-1187): no signed tag, no
Authenticode, no signed `SHA256SUMS`.

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

1. The version: `[workspace.package].version` of `Cargo.toml` is `X.Y.Z`, on the release branch `release/<X.Y>`.
2. The release notes: `docs/release-notes/<X.Y.Z>.md`, with the changes, the format, CLI and API changes (RC-08) and
   the notices of the AppImage's runtime (NFR-LIC-04's amendment), on the release branch.
3. The settings of §2 hold.

The workflow's `verify` job refuses the candidate when 1 or 2 does not hold, with one of these texts: "the tag v<t>
does not match the workspace version <w>", "no release notes: docs/release-notes/<X.Y.Z>.md is missing or empty". It
asks for no CI run (ID-1519, ID-1520).

## 4. Cutting the candidate

```text
git tag v<X.Y.Z>-rc.<n> <commit>
git push origin v<X.Y.Z>-rc.<n>
```

The push starts `release.yml`:

1. `verify`: the tag's version and its release notes (§3).
2. `package`: `package.yml` with `upload: true`, x86_64 only. Each OS job writes the third-party notices, runs
   `cargo build --release` (on Linux in the pinned `ubuntu:22.04` container, with the release check and glibc 2.35
   at most), stages the installed tree, runs one smoke test on it (`--version` of both programs and `elektrawatt
   --smoke` on the sample) and writes the CLI archive; Windows then compiles the per-user setup (NSIS, pinned), Linux
   the AppImage. The `book` job builds the guide the installers ship.
3. `sums`: `SHA256SUMS` over the four files.
4. `release`: the draft prerelease with the release notes and the five files, downloaded back and checked with
   `sha256sum -c SHA256SUMS`.

## 5. Reading the draft

On the repository's releases page, the draft `v<X.Y.Z>-rc.<n>` lists the files of §1 and the release notes. The run's
`release` job log ends with "the draft prerelease v<X.Y.Z>-rc.<n> holds <n> verified files; nothing is published". The
summaries of `package`'s jobs carry the release-check reports and the container's packages.
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
draft; it downloads the draft's files and verifies them again (`SHA256SUMS` lists exactly the files, `sha256sum -c`),
creates the tag `v<X.Y.Z>` on the candidate's commit and the published release
`v<X.Y.Z>` (marked latest, not a prerelease) with those files and the draft's notes, and verifies the published files
once more. The draft stays as the record of the candidate.

## 8. A blocker

Fix it on the release branch `release/<X.Y>`, then cherry-pick the fix onto the working branch, so that the next
release has it too (ID-1412); then cut the next candidate, `v<X.Y.Z>-rc.<n+1>`, from §3. Leave the earlier candidate's
tag and draft as they are: they record what was tested.

## 9. How a user verifies a download

The guide's install page has the user's form ("Verify a download"). In short, with the downloaded files and
`SHA256SUMS` in one folder:

```text
sha256sum --check --ignore-missing SHA256SUMS
```

## 10. What the workflow proves

| Job (doc 18 §7.4) | Where | 0.1.x's form (ID-1520) |
|---|---|---|
| CI-R-01 | `release.yml` `verify` | The tag's version is the workspace version; the release notes exist; no tag signature |
| CI-R-03 | `package.yml` `windows`, `linux` | `cargo build --release --locked`, network-less on Linux; `ci/release-check.sh env` on both OSes, `scan` and glibc 2.35 at most on Linux |
| CI-R-05 | `package.yml` (WP-S6.06) | The setup, the AppImage and the CLI archives, after one smoke test of each staged tree |
| CI-R-06 | `release.yml` `sums` | `SHA256SUMS` over every release file, unsigned |
| CI-R-11 | `release.yml` `release` | The draft on a candidate tag; the promote, dry or real, on a dispatch |

Not run at 0.1.x: CI-R-02 (the CI runs and the advisory checks), CI-R-07 (the SBOMs), CI-R-08 (build provenance),
CI-R-09's binary half (the embedded dependency lists, the listening-socket check) and CI-R-10's CI half (the install
tests); CI-R-04 (`ew-step-worker`) is 1.0's. The advisory waivers in `deny.toml` and `.cargo/audit.toml` are still
checked by hand at RC-03.
