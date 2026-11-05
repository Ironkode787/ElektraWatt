<!-- SPDX-FileCopyrightText: 2026 The ElektraWatt contributors -->
<!-- SPDX-License-Identifier: CC-BY-4.0 -->

# Review the picture-based result

This development brief requires a privately supplied `frames/reference.png`. The reference picture must not
be committed or copied into the offline guide. The tester sees that picture, the task and the normal guide,
never this rubric, the solution or source code.

The harness's hard checks establish that authoring, a text slot, an orientation preview, publication and
saved project placement occurred. They do **not** establish picture fidelity, electrical completeness or
readability. A run is accepted only after an independent manual check of all these points:

- Five numbered plug connections run across the top of the rectangular body; the FE connection is on the right.
- The printed supply labels and connector/earth details follow the picture, resized to the native connection grid.
- All six connection points lie on the 4 mm grid and map to the intended function pins; electrical roles agree
  with the drawing. A printed FE designation alone does not establish the FE potential role.
- The device-tag slot is present; the cross-reference anchor lies on the left edge. A–H preserve the geometry,
  pin directions and legible text, and a placed copy agrees with the preview.
- A published library, saved editable project and the tester's notes/friction/handback exist. Report each item
  as met, unmet or not observed. Record assistance and incomplete work; never turn a harness pass into a claim
  that the whole picture or release gate passed.

The kittest and built-binary reference acceptance additionally wire all six pins and inspect the wire report.
They supplement the blind run and cannot replace it. The release candidate needs its own G-M2.5 round.
