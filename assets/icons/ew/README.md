<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: GPL-3.0-or-later
-->

# ElektraWatt glyphs

The glyphs Tabler Icons lacks (WP-S2.17 part 6, D9; doc 16a §4.11), drawn on Tabler's 24-unit grid with the same
rules: stroke-only `path` elements, round caps and joins, stroke 2 at 24 px in the files (the application draws them
at the token stroke). `cargo xtask gen-icons` reads them with the vendored set and keys them `ew-<name>`.

| File | Key | Glyph |
|---|---|---|
| `black-box.svg` | `ew-black-box` | A dashed rectangle: a device drawn as a black box with an open outline |
| `cable.svg` | `ew-cable` | A cable end: two conductors leave a round sheath end, with a sheath band |
| `interruption-point.svg` | `ew-interruption-point` | A conductor ending in an arrow at a page edge: the connection continues elsewhere |
| `potential.svg` | `ew-potential` | A conductor with two node dots and two taps ending in connection points: one electrical potential (net) |
| `qet-grid.svg` | `ew-qet-grid` | A frame with a three-by-three dot grid: the QElectroTech grid badge |
| `structure-box.svg` | `ew-structure-box` | A frame with a "+" in its corner: a location (structure) box |
| `terminal-strip.svg` | `ew-terminal-strip` | Three terminals in a row with their leads above and below |
