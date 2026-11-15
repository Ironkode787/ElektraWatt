<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Release dossier: ElektraWatt 0.3.0

The record of the third release: its candidates, each candidate's filled checklist and the go. It is kept at an
early alpha's size. The procedure is [the release procedure](../../README.md); the release notes are
[`docs/release-notes/0.3.0.md`](../../../release-notes/0.3.0.md); what the release holds and how each part was
shown is the log of [`docs/plan/m3.md`](../../../plan/m3.md). People are named by their role only.

0.3.0 is the release of M3, "Automation breadth": the PLC and bus work, fluid power, the exchange with PLC software,
the EDZ, EPLAN, DXF and DWG imports, the automation workspaces, and parts with ports made in the window. Its gate is
the owner's own hands-on test of a candidate.

## 1. The owner's items

The items on file for 0.1.0 stand ([its dossier](../0.1.0/README.md) §1): the repository's settings, the audits,
the statement that no employer is involved. No new item is asked for 0.3.0.

Decided by the owner during M3 and on record in the plan's log: the window is designed for 1920 × 1080 with
2560 × 1440 as its extension; no DWG export in this release; a person makes his own devices with ports in the
window before any candidate ("like IM152, or the scalance series switches", and generic PROFINET devices).

## 2. The candidates

| Candidate | Cut from | Checklist | State |
|---|---|---|---|
| `v0.3.0-rc.1` | `release/0.3`, the commit the tag names | [`checklist-rc.1.md`](checklist-rc.1.md) | Cut; waits for the owner's hands-on test |

Before the cut, on the tip that the release branch starts from: eight reviews by area and one of the whole
milestone, each with its fix lane; a persona round of three picture-led runs on a new project, two re-tests, two
more runs and a last pass, each triaged and fixed (the log's entries from "The persona round and its fixes" on).
After the version was set, the files that carry the version were blessed again and the whole workspace
(`cargo xtask check`) and the deep run were repeated there.

## 3. Known when the candidate was cut

The release notes' section 7, and the open rows of [`docs/plan/backlog.md`](../../../plan/backlog.md), section
"What M3's reviews left". Decisions of the release manager that the owner has not confirmed, each said to him with
the candidate:

- A device with two bus interfaces stands on a bus page as two boxes with one tag: a box of format 0.1 draws the
  connection points of one function. One box needs a format change or a generated symbol per part.
- A route may cross a text where no other route exists (ADR-0033 amendment 1); it never crosses a body, a port or
  another line.
- A new valve terminal's tag takes the letter K (it was A): the letter follows the group the terminal is filed in.
- Device description files (GSD, GSDML) are not read; a part is typed from its data sheet.
- Two advisories that appeared since 0.2.0 were answered by upgrades at the cut (the checklist's RC-03).

Only a real desktop shows three things (the checklist's RC-05): the installers, a printer, a screen reader. The
owner's test says whether an open row holds the release.

## 4. The go

Not given yet.
