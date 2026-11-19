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

Decided by the owner on the second candidate: three screens are designed anew (starting a project and templates, the
symbol editor, the PLC rack); a lane finishes its whole brief and carries on when its context is summarised, with no
stop for size; after the lanes one review of the whole round, a fix lane if it needs one, and a persona round that has
the complete-cabinet job in it.

Decided by the owner on the fourth candidate: pin texts of a drawn box stay as they were (turned a quarter on the top and bottom sides, horizontal on the left and right); the golden project has done its work and is retired after this release, and until then a decided change regenerates it instead of bending round it; after the fix lanes one full review with its fixes and a persona round, and the complete-cabinet job run again when every lane has delivered; the title block's layout and the frame's columns and rows become editable in the next milestone, design first; the milestone after that is given to fluid power, and the later milestones move one up.

## 2. The candidates

| Candidate | Cut from | Checklist | State |
|---|---|---|---|
| `v0.3.0-rc.1` | `release/0.3`, the commit the tag names | [`checklist-rc.1.md`](checklist-rc.1.md) | Cut on 2026-11-16; its release build passed (run 11 of `release.yml`). **Tested by the owner by hand: not passed.** He found 29 points (the plan's log lists them as F1 to F29); no go |
| `v0.3.0-rc.2` | `release/0.3`, the commit the tag names | [`checklist-rc.2.md`](checklist-rc.2.md) | Cut on 2026-11-16; its release build passed (run 12 of `release.yml`). **Tested by the owner by hand: not passed.** He wrote sixteen points (the plan's log lists them as G1 to G16), three of them asking for a screen to be designed anew; no go |
| `v0.3.0-rc.3` | `release/0.3`, the commit the tag names | [`checklist-rc.3.md`](checklist-rc.3.md) | Cut on 2026-11-18. Its release build made every package (run 13 of `release.yml`: verify, the guide, the Linux and the Windows package and the sums passed) and **failed at the draft**: the release notes had grown longer than GitHub takes as a release's text. No draft exists; nobody tested it |
| `v0.3.0-rc.4` | `release/0.3`, the commit the tag names | [`checklist-rc.4.md`](checklist-rc.4.md) | Cut on 2026-11-18: the third candidate's program with the release script mended; its release build passed (run 14 of `release.yml`). **Tested by the owner by hand: not passed.** He wrote seventeen points and answered one open question (the plan's log lists them by theme); two of the points are asks for the next milestone |
| `v0.3.0-rc.5` | `release/0.3`, the commit the tag names | [`checklist-rc.5.md`](checklist-rc.5.md) | Cut on 2026-11-19; waits for its release build and for the owner's hands-on test |

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

Before the third cut: thirteen lanes for the owner's sixteen points (three of them redesigns: the symbol editor with a
part's device box, a station's rack as a view in a tab, New project with a list of templates), one review of the whole
round (no major finding; ten small fixes), a persona round of five runs with a complete cabinet among them, seven fix
lanes from it, two re-tests, and one panic found by the second cabinet run and mended (the log's entries from "The
owner's test of the second candidate" on). On the candidate's tree the whole workspace (7,522 of 7,522 tests) and
the deep run with the private corpus (18 of 18) passed. The release branch was moved forward to the working
branch's tip again, for the same reason.

Before the fifth cut: five lanes for the owner's seventeen points, each told to find the root cause (a wire from 0 V to earth is the bond; a page position stored one nanometre apart from itself; a box armed with something an Undo took away; a cable that only carries a bus line counted as on no page), one review of the whole round (one major finding, a symbol fitted to another grid that could not be moved, mended), a persona round of three runs, five fix lanes from it, two re-tests, a last round of fixes, and the complete cabinet drawn once more with twice the turns, which finished every step (the log's entries from "The owner's test of the fourth candidate" on). One decision was read wrongly on the way (the pin texts were built horizontal) and put back before the cut. On the candidate's tree the whole workspace (7,619 tests) and the deep run with the private corpus (18 of 18) passed; the deep run had first failed twice, and both causes were mended. The release branch was moved forward to the working branch's tip again.

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
  Pin texts of a drawn box (a PLC card) stay as they were: on the top and bottom edges turned a quarter, reading
  from bottom to top, on the left and right edges horizontal; the multipin plug's pins run down its sides with
  horizontal texts.
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

With the third candidate:

- The window says one word, "template", for what it called a company standard and a basic project; a template's
  pages come into a new project only when the person ticks that.
- A text style chooses among typefaces that come with the program (four), not among a computer's fonts, so that a
  drawing looks the same everywhere. No font under a GPL-only licence is bundled; the technical one is Routed Gothic.
- Reroute replaces Edit route and Route selection and applies at once; a selected wire has handles. The router may
  step half a grid step between close pins, and corners of different wires never meet (ADR-0033 amendment 3).
- A wire in a cable has its core's cross-section and colour, whatever the wire itself said; a marker over a
  cable's cores draws as the cable's mark.
- Supply points are grouped by a stored group; drawings made before print each point's place as they did.
- An earth symbol without a part is an earth connection and asks for no part; Golden A lost one finding by it.
- The check level offers Pre-delivery and Full only; Drafting checked the same as Full.
- A cable without a length is a warning until the check for delivery.
- Terminal numbers have a style of their own; a project that set the device tag's size keeps its terminal numbers.
- Connecting on placement never joins a contact to a PLC output or two different conductors.
- A turned or mirrored symbol turns about the middle of its body.
- The persona round was not repeated after the last five fixes and the mended panic: the whole workspace and the
  deep run passed on that tree, and the owner's test is the gate.

With the fifth candidate:

- A wire drawn from a 0 V conductor to protective earth is the bond, and one to functional earth is no conflict
  (the check then says where the protective bond belongs); earth symbols of one location are one earth for the
  PE-island check; the potential findings say what is wired to what and what to do.
- Reroute starts from the drawing and gives no wire a route worse than the one it had; a page routed for the
  first time takes the better of two orders where that costs nothing (ADR-0033 amendments 4 and 5, with the golden
  project's folios 106 and 203 regenerated).
- One position of the grid is one point wherever points are compared (a step of one nanometre on a grid of thirds
  is no step); with the Wire and Tube tools a click within about ten screen points of a connection point takes it.
- A paste onto a page of another grid and a page's grid change fit each item to its nearest grid point and say
  so; a fitted symbol keeps to the grid by its first pin.
- A box drawn without a part takes pins where it stands, and two pins of a box may share a name; a resized box
  keeps its pins centred and goes no lower than its turned pin texts need. A box the person draws keeps the size
  drawn.
- A bus cable is numbered on its line's page; an older project's bus cable whose tag now meets another is
  reported with a renumbering offered, not renumbered silently. A plug at a line end without a device counts in
  the parts list.
- On a page without a location of its own, a device without a location no longer counts when the page's shared
  location is found, so giving a cabinet's devices their location leaves the drawn tags as they were.
- Ctrl+Z with something on the pointer first puts it away; every Undo and Redo says what it took back and where.
- A station's To do has one line per side; a tag changed in the station view is taken at Enter.
- A new terminal strip keeps the tag of the wizard's rule (letter, page, counter), which is the reference
  project's convention.
- No persona run followed the fixes that came after the last complete-cabinet run: the whole workspace and the
  deep run passed on that tree, and the owner's test is the gate.
- A redesign of the router that also places devices and parts, asked for by the owner at the end of this round,
  is not in this candidate.

Only a real desktop shows three things (the checklist's RC-05): the installers, a printer, a screen reader. The
owner's test says whether an open row holds the release.

## 4. The go

Not given yet.
