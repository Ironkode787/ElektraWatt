<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Release dossier: ElektraWatt 0.3.5

The record of the fourth release: its candidates, each candidate's filled checklist and the go. It is kept at an
early alpha's size. The procedure is [the release procedure](../../README.md); the release notes are
[`docs/release-notes/0.3.5.md`](../../../release-notes/0.3.5.md); what the release holds and how each part was
shown is the log of [`docs/plan/m3.5.md`](../../../plan/m3.5.md). People are named by their role only.

0.3.5 is the release of M3.5: a round of hygiene in the repository, then a fix round that worked the backlog down,
the rows of a first real drawing set made with 0.3.0 first. Its gate is the owner's go.

## 1. The owner's items

The items on file for 0.1.0 stand ([its dossier](../0.1.0/README.md) §1): the repository's settings, the audits,
the statement that no employer is involved. No new item is asked for 0.3.5.

Decided by the owner during M3.5 and on record in the plan's log and the documents named: the fix round is part of
this release, and its rows are worked, not moved to a later one; the reference project is retired; the latest
version leads, so a project saved by a newer version need not open in an older one (ADR-0011 amendment 8); until
1.0 a project that exists takes the latest version's derivation rules (ADR-0003 amendment 3); the texts of a symbol
turned a quarter stay level and are placed anew round it; the legend of a panel layout is a list page of its own;
before a candidate is cut, a review of the whole change with a fix lane; the rows still open at the first cut are
worked on after it.

## 2. The candidates

| Candidate | Cut from | Checklist | State |
|---|---|---|---|
| `v0.3.5-rc.1` | `release/0.3`, the commit the tag names | [`checklist-rc.1.md`](checklist-rc.1.md) | Cut on 2026-11-22 |

Before the first cut, on the tip that the release branch moves to: two reviews, each with its fixes. The first
read everything from 0.3.0 to the end of the fix round's sweep, in four parts (the format, the model and the
commands; the derivation, the lists and the drawing; the window; the tests, the documents and hygiene): four major
findings, mended or settled by the owner's ruling on versions, and about twenty small ones mended by the reviewers.
The second read
the three bundles that came after it (the window and what symbols print; the placer and the router; the model and
the format): one major finding, a report import that closed a project edited while its file was read, mended with a
test of that case, and eight small ones mended. No persona round was run for this release. After the version was
set, with nothing else building: the whole workspace (`cargo xtask check`, 7,774 of 7,774
tests), the deep run with the private corpus (9 of 9), the format guard and the guide's translation check passed.
The candidate's commit adds to that tree the update of `url`, which the tests of `ew` passed with, and documents
(this dossier, the plan's log, one line of the notes).

Fifteen rows of the fix round are open at the cut, each with what was tried
([backlog](../../../plan/backlog.md), "0.3.5: the fix round"); the notes' section 7 names them in a reader's words.

The release branch `release/0.3` was moved forward to the working branch's tip: everything since 0.3.0 on the
working branch is this release.

## 3. The release manager's decisions

- The README's pictures were made on the day of the cut from the program as the candidate's tree draws it; no
  picture of the suite changed between them and the cut, so they were not made again. Its download table and its
  "Where it stands" stay at 0.3.0 until 0.3.5 is published.
- One advisory appeared on the day of the cut (RUSTSEC-2026-0592, `url` 2.5.8, in the graph of the tests of `ew`
  only): `url` was updated to 2.5.9 before the cut; no waiver was written.
- The format guard read the unreleased notes only and so failed from a cut until the final tag; it now reads the
  notes of the release being cut as well (`xtask/src/api_guard.rs`).

## 4. The go

Not given yet.
