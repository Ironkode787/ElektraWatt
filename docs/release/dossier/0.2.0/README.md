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
| `v0.2.0-rc.1` | `release/0.2`, the commit the tag names | [`checklist-rc.1.md`](checklist-rc.1.md) | Not promoted: the owner's hands-on test gave 28 findings, four of them major |
| `v0.2.0-rc.2` | `release/0.2`, the commit the tag names | [`checklist-rc.2.md`](checklist-rc.2.md) | No downloads: its release build failed at the Windows setup (the icon's path in the setup's script) |
| `v0.2.0-rc.3` | `release/0.2`, the commit the tag names | [`checklist-rc.3.md`](checklist-rc.3.md) | Promoted to `v0.2.0` on 2026-11-13 |

Before the first cut, on the tip that the release branch starts from: the whole workspace passed
(`cargo xtask check`), the deep run passed, and a persona round of four picture-led runs was triaged (the log's
"Step 9" entries). After the version was set on the release branch, the files that carry the version were blessed
again and both runs were repeated there.

The second candidate holds the fixes for the owner's 28 findings (the log's entries "The second candidate"). Before
its cut two picture-led runs walked the fixed flows; what they found that would have stopped the owner again was
fixed, the rest is a row of the later backlog. The whole workspace and the deep run passed on the release branch's
tip. The third candidate is the second with the setup's script put right.

## 3. Known when the candidate was cut

For the first candidate: the open rows of [`docs/plan/backlog.md`](../../../plan/backlog.md), section "The persona
round before 0.2.0", and the release notes' section 7. The two rows the round marked High, both about interruption
points (the tool for a newcomer, the texts of points placed close together), were fixed before the cut on the
owner's word; what is left of them is a row of the later backlog.

For the second and the third candidate: the same section of the backlog, the release notes' section 7 (it gained what the fixes
left: a terminal row's undo steps, the cable warning on a supply's entry, the offered potential's need of a known
supply, the built-in template's pages without a frame), and three things that only a real desktop shows (the
checklist's RC-05). The owner's test says whether an open row holds the release.

## 4. The go

Given by the owner on 2026-11-13 for `v0.2.0-rc.3` ("Promote the build."). The release manager dispatched the
promote from `main`: its dry run listed the candidate's five files, and the real run created the tag `v0.2.0` on
the candidate's commit and published the release `v0.2.0` (latest) with those files, verified again against
`SHA256SUMS`. The draft `v0.2.0-rc.3` stays as the record of the candidate.
