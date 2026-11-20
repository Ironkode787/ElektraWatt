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
