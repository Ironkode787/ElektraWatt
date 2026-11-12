<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Release dossier: ElektraWatt 0.2.0

The record of the second release: its candidates, each candidate's filled checklist and the go. It is kept at an
early alpha's size. The procedure is [the release procedure](../../README.md); the release notes are
[`docs/release-notes/0.2.0.md`](../../../release-notes/0.2.0.md); what the release fixes and how each fix was shown
is the log of [`docs/plan/m2.5-fix.md`](../../../plan/m2.5-fix.md). People are named by their role only.

0.2.0 is the release of the fix milestone M2.5-Fix. Its gate is the owner's own hands-on test of a candidate: the
wizard, a new project, placing, wiring, routing, the symbol editor and the QElectroTech import work without
anything that stops the work.

## 1. The owner's items

The items on file for 0.1.0 stand ([its dossier](../0.1.0/README.md) §1): the repository's settings, the audits,
the statement that no employer is involved. No new item is asked for 0.2.0.

## 2. The candidates

| Candidate | Cut from | Checklist | State |
|---|---|---|---|
| `v0.2.0-rc.1` | `release/0.2`, the commit the tag names | [`checklist-rc.1.md`](checklist-rc.1.md) | Cut; waits for the owner's hands-on test |

Before the cut, on the tip that the release branch starts from: the whole workspace passed (`cargo xtask check`),
the deep run passed, and a persona round of four picture-led runs was triaged (the log's "Step 9" entries). After
the version was set on the release branch, the files that carry the version were blessed again and both runs were
repeated there.

## 3. Known when the candidate was cut

The open rows of [`docs/plan/backlog.md`](../../../plan/backlog.md), section "The persona round before 0.2.0", and
the release notes' section 7. The two rows the round marked High, both about interruption points (the tool for a
newcomer, the texts of points placed close together), were fixed before the cut on the owner's word; what is left
of them is a row of the later backlog. The owner's test says whether an open row holds the release.

## 4. The go

Not given yet. The go is the owner's, after their hands-on test of a candidate.
