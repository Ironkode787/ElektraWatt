<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Release dossier: ElektraWatt 0.1.0

The record of the first release (GP-11; WP-S6.08 D11): its candidates, the owner's items before the first candidate,
each candidate's filled checklist, the acceptance run over every G-M2 item, the translation report, the manual tests
and the go. The procedure is [the release procedure](../../README.md); the release notes are
[`docs/release-notes/0.1.0.md`](../../../release-notes/0.1.0.md). People are named by their role only ("the owner",
"the release manager", "the tester"); no person's name, address, machine name or serial number goes into this folder.

| File | What it holds |
|---|---|
| `README.md` | This page: the owner's items, the candidates and the go |
| `checklist-rc.<n>.md` | The [release checklist](../../checklist.md) of candidate `v0.1.0-rc.<n>`, copied and filled, one file per candidate |
| [`acceptance.md`](acceptance.md) | The acceptance run: one row per G-M2 item and per AC-21, AC-23, AC-40, AC-41 and NFR-REL-11, with its evidence and result |
| [`translation.md`](translation.md) | The translation report on the last candidate's commit (G-M2-12, RC-06) |
| [`manual.md`](manual.md) | The manual tests of RC-05 on the last candidate's files: IN-15, PV-13 and the others |

## 1. The owner's items before the first candidate

WP-S6.08 §5; each blocks what its last column says until it is on file here.

| # | Item | State | On file |
|---|---|---|---|
| G1 | The repository's settings of [the release procedure](../../README.md#2-before-the-first-candidate-the-repositorys-settings) §2: the environment `release`, the tag ruleset (it forbids updating and deleting `v*` tags and leaves their creation open, decision log ID-1366), immutable releases, the read-only workflow token, the required checks `c-only` and `api-guard` | On file | Set by the owner and read back through the API on 2026-11-04 (decision log ID-1368) |
| G2 | Whether the AI-performed audits and recorded exemptions of ID-723 cover 0.1.0 | On file | The owner's decision: they cover 0.1.0 as they cover the 0.0.x builds (decision log ID-1331 (2)) |
| G3 | LG-1: the employer copyright statement, or the statement that no employer's staff contributed (RC-10) | **Open** | The owner's statement, to be filed here before the go |
| G4 | Who cuts the candidates, signs the checklist as release manager and dispatches the promote | **On file** | The owner (2026-11-05, decision log ID-1400): the orchestrator, or an agent it dispatches; the gate review and the go stay the owner's |
| G5 | Private vulnerability reporting switched on, so that `SECURITY.md`'s channel works | On file | Switched on by the owner and read back on 2026-11-04 (decision log ID-1368) |

## 2. The candidates

One row per candidate `v0.1.0-rc.<n>`, in order. A blocker found on a candidate is fixed on `main` and the next
candidate is cut ([the procedure](../../README.md#8-a-blocker) §8); every earlier row stays.

| Candidate | Commit | Draft | Checklist | Result |
|---|---|---|---|---|
| — | — | — | — | No candidate yet |

## 3. The go and the published release

| Item | Record |
|---|---|
| The gate review G-M2 | Open |
| The owner's go | Open |
| The promote's dry run | Open |
| The published release `v0.1.0` | Open |

## 4. The drafts, the SBOMs and the attestations

Each candidate's draft prerelease on the repository's releases page holds its files, the SBOM of every artefact
(`<artefact>.cdx.json`) and `SHA256SUMS`; the build provenance of every file is on the repository's attestations
page. Each candidate's row of section 2 links its draft; the release job's log ends with the number of verified files.
