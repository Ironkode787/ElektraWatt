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

Decided by the owner on the first candidate: every one of his 29 points is answered before a second candidate; the
development loop is made shorter as the work goes (tests that fail easily, build time), with a measurement of the
lanes' time after each round; the persona re-test before the second candidate leaves the complete-cabinet job out.

## 2. The candidates

| Candidate | Cut from | Checklist | State |
|---|---|---|---|
| `v0.3.0-rc.1` | `release/0.3`, the commit the tag names | [`checklist-rc.1.md`](checklist-rc.1.md) | Cut on 2026-11-16; its release build passed (run 11 of `release.yml`). **Tested by the owner by hand: not passed.** He found 29 points (the plan's log lists them as F1 to F29); no go |
| `v0.3.0-rc.2` | `release/0.3`, the commit the tag names | [`checklist-rc.2.md`](checklist-rc.2.md) | Cut on 2026-11-16; its release build passed (run 12 of `release.yml`: verify, the guide, the Linux and the Windows package, the sums, the draft); waits for the owner's hands-on test |

Before the first cut, on the tip that the release branch starts from: eight reviews by area and one of the whole
milestone, each with its fix lane; a persona round of three picture-led runs on a new project, two re-tests, two
more runs and a last pass, each triaged and fixed (the log's entries from "The persona round and its fixes" on).
After the version was set, the files that carry the version were blessed again and the whole workspace
(`cargo xtask check`) and the deep run were repeated there.

Before the second cut: nine fix lanes for the owner's 29 points with their follow-ups; three tutorials and two
rewritten guides; a persona round of four runs on the fixes (one of them a complete cabinet, which logged forty
irritations), five fix lanes from it and a routing decision (ADR-0033 amendment 2); three re-tests and two last fix
lanes (the log's entries from "The owner's test of the first candidate" on). On the candidate's tree the whole
workspace (7,417 of 7,417 tests) and the deep run with the private corpus (18 of 18) passed. The release branch
was moved forward to the working branch's tip: the fixes are too many to carry one by one, and every one of them
is a fix for the candidate.

## 3. Known when the candidates were cut

The release notes' section 7, and the open rows of [`docs/plan/backlog.md`](../../../plan/backlog.md). Decisions of
the release manager that the owner has not confirmed, each said to him with the candidate.

With the first candidate:

- A device with two bus interfaces stands on a bus page as two boxes with one tag: a box of format 0.1 draws the
  connection points of one function. One box needs a format change or a generated symbol per part.
- A route may cross a text where no other route exists (ADR-0033 amendment 1); it never crosses a body, a port or
  another line.
- A new valve terminal's tag takes the letter K (it was A): the letter follows the group the terminal is filed in.
- Device description files (GSD, GSDML) are not read; a part is typed from its data sheet.
- Two advisories that appeared since 0.2.0 were answered by upgrades at the cut.

With the second candidate:

- The fluid symbols are drawn on a 12 mm module (it was 16), and Golden A was regenerated once for the two of
  them it holds (the 5/2 valve and the double-acting cylinder: folios 203 to 206).
- Turning or mirroring a symbol never turns its texts; an imported QElectroTech drawing keeps QET's text angles.
  Pin texts on the top and bottom edges of a drawn box (a PLC card) still stand vertical: put to the owner, not
  answered, left as it was; the multipin plug's pins run down its sides with horizontal texts.
- A route crosses a text where keeping clear of it is a far longer way, more than 8/5 of the route across (ADR-0033
  amendment 2); a short step aside is still taken, and bodies, ports and other lines stay forbidden.
- A bus device given to a PLC becomes a station of that PLC (a switch belongs to it without a rack); a station is
  made in a rack dialog, and placing a station opens it.
- There are two kinds of valve terminal, with a bus node and multipin, each drawn as its positions; a terminal drawn
  earlier as a free box keeps its look and offers Draw as slots.
- The project's edited date is set when a person saves with changes; dates are typed in the project's format.
- A tutorial's practice project opens in a second window of the program.
- The whole-window pictures of the tests were cut to two, and the guide's translation catalogue is made by its own
  command before a release, not with every change.
- The Windows program's main thread has a stack of 8 MiB: on the merged tip an offscreen run died of a stack
  overflow in about one run in three, which the recorded self-test showed.

Only a real desktop shows three things (the checklist's RC-05): the installers, a printer, a screen reader. The
owner's test says whether an open row holds the release.

## 4. The go

Not given yet.
