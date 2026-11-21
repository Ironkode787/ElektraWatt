<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Unreleased changes

The changes since the last release, 0.3.0, one line each, and nowhere else: a change that a user, a script or a
reader of the format can see writes its line here, in the commit that makes it. A line starts with its kind:
`api-add`, `api-break`, `behaviour-change`, `dependency`, `format-variant` or `text-change`; a fix is a
`behaviour-change`. `cargo xtask api-guard` reads the `format-variant` lines.

When a release branch is cut, `docs/release-notes/<X.Y.Z>.md` is written from these lines, with its summary
(`## In short`, at most 30 one-line bullets for a draughtsman: docs/release/README.md, section 3), and the lines go
from this file on both branches. From then on a fix to a candidate writes its line in `<X.Y.Z>.md`, and every other
change here.

- behaviour-change: a dash pattern with a zero entry (as an SVG asset may give one) is dashed on the screen as it is
  in the PDF, the SVG and the DXF; one with a negative entry or no positive one is solid everywhere (the PDF and SVG
  rule)
- behaviour-change: a table import of a CSV whose header follows title lines takes the decimal comma of `excel-nl`
  when the header is separated by semicolons, as the reader reads its cells (the first line decided before)
- behaviour-change: a Terminal… row is one Undo step only until the next commit, a property, table or card edit as
  much as a drawing tool's; after an undone row and as many property edits as the row had terminals, one Undo took
  those edits back together
- behaviour-change: inside the symbol editor the command search and the window's panes ask the same check as the menus:
  a drawing tool that cannot run there is no longer offered as enabled, and Save, which saves the symbol, is no
  longer greyed
- behaviour-change: on a page of the QElectroTech grid a text's box keeps its size wherever its symbol stands, so a
  second press of Arrange and route finds the page the first stored; texts on such pages may stand a sixth of a
  millimetre from where they stood, and a wire beside such a text may take another route (06a §2.1, FT-1848)
- text-change: the example keys of `ew queries` and the guide's query reference name records of the new sample
  project (`-104A1-A2`, `-201K1-Q2`, `-X1011:L1`, `-102Q1` and two of its IDs), as the reference project's records
  are gone; every example answers on the sample
- text-change: the words that named the retired reference project say what is meant: the help of `ew import` and
  `ew export` speaks of the `ew` dialect (it said "golden dialect"), `ew sql schema` no longer cites a deleted design
  document, the argument texts of `ew queries` name the sample's records, and the guide's command-line how-to is "Use
  the command line on the sample project"
- text-change: the company-standard wizard's `report_blocks` value `golden` is `standard` (every list and overview,
  the default); a script that sets `golden` is refused and must set `standard`; there is no alias
- behaviour-change: File ▸ Open sample project and Help ▸ Tutorials open the new sample project, a smaller control
  cabinet the program draws itself from its own script; the release ships it as `samples/sample.ew` in place of
  `samples/J-E-2416.ew`
- behaviour-change: a print cancelled between two pages ends as cancelled; it could end with the printer's refusal
  of the next page's paper ("The handle is invalid"), because the cancel was only read before each band
- behaviour-change: a device takes a second symbol of its own: a symbol of another kind than a contact (a lamp, a
  supply, an element brought in from QElectroTech) joins any device but a terminal or a strip as an auxiliary
  function, by J while placing, by `place.pick device=<tag>`, by typing the tag in its Tag row and by
  `rep.join_device`; a contact keeps its rule (a device with a coil or a part naming it), and a terminal, an
  interruption point and a graphic join no device (FT-1860)
- behaviour-change: `place.pick` takes a library symbol's address `library:<library>:<item>` also after its first
  placement, and a symbol it does not find is refused as unknown, no longer as a missing parameter (FT-1860)
- behaviour-change: `ew import parts` and File ▸ Import ▸ Parts table… take a function template that names a function
  definition of the project's own by its key; it was refused as an unknown FnDef (FT-1860)
