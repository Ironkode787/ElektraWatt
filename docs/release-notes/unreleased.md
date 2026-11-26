<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Unreleased changes

The changes since the last release, 0.3.5, one line each, and nowhere else: a change that a user, a script or a
reader of the format can see writes its line here, in the commit that makes it. A line starts with its kind:
`api-add`, `api-break`, `behaviour-change`, `dependency`, `format-variant` or `text-change`; a fix is a
`behaviour-change`. `cargo xtask api-guard` reads the `format-variant` lines.

When a release branch is cut, `docs/release-notes/<X.Y.Z>.md` is written from these lines, with its summary
(`## In short`, at most 30 one-line bullets for a draughtsman: docs/release/README.md, section 3), and the lines go
from this file on both branches. From then on a fix to a candidate writes its line in `<X.Y.Z>.md`, and every other
change here.

- behaviour-change: Undo and Redo show a page the step changed, also when it was made from another page that is still on show; a step on a panel layout page (its plate, rails, ducts, placements) names that page in the history.
