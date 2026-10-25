<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC0-1.0
-->

# Real exporter SVG corpus (FT-418)

The files come from Wikimedia Commons, each public domain or CC0 as its file page states; they are kept byte for byte
except where the table says; they contain no personal data (metadata was checked); `tests/exporters.rs` asserts each
file's verdict.

| Path | Exporter | Source | Licence | Change | Verdict |
|---|---|---|---|---|---|
| `affinity/action-potential-schematic.svg` | Affinity Designer (`xmlns:serif`) | https://commons.wikimedia.org/wiki/File:Action_potential_schematic.svg | CC0 | renamed | accepted |
| `affinity/water-analogy.svg` | Affinity Designer (`xmlns:serif`) | https://commons.wikimedia.org/wiki/File:Current_voltage_resistance_water_analogy.svg | CC0 | renamed | accepted |
| `coreldraw/alefbet.svg` | CorelDRAW (generator comment) | https://commons.wikimedia.org/wiki/File:Alefbet_ivri.svg | PD-self | renamed | accepted |
| `coreldraw/aeroplane-sketch.svg` | CorelDRAW (generator comment) | https://commons.wikimedia.org/wiki/File:Avionmaldibujado.svg | PD-self | renamed | accepted |
| `drawio/rl-series-circuit.svg` | diagrams.net, optimised by vecta.io (Commons description) | https://commons.wikimedia.org/wiki/File:AC_RL_series_circuit.svg | CC0 | renamed | accepted |
| `drawio/music-classes-html-labels.svg` | draw.io (`data-drawio-colors`, HTML labels) | https://commons.wikimedia.org/wiki/File:Hierarchy_of_music_classes_at_Wikidata_2023.svg | PD-user | renamed | refused `ForeignObject` |
| `illustrator/ephedrine-enantiomers.svg` | Adobe Illustrator, edited in Inkscape | https://commons.wikimedia.org/wiki/File:(pseudo)ephedrine_enantiomers.svg | PD-self | renamed | accepted |
| `illustrator/keyboard-109.svg` | Adobe Illustrator (generator comment) | https://commons.wikimedia.org/wiki/File:109keyboard.svg | PD-self | renamed | accepted |
| `illustrator/euro-edge-inscription.svg` | Adobe Illustrator | https://commons.wikimedia.org/wiki/File:2_standard_euro_edge_inscription.svg | PD (text) | renamed | accepted |
| `illustrator/sign-2-4mr.svg` | Adobe Illustrator | https://commons.wikimedia.org/wiki/File:2.4mR_black.svg | PD (text) | renamed | accepted |
| `illustrator/calendar-2015.svg` | Adobe Illustrator, edited in Inkscape | https://commons.wikimedia.org/wiki/File:2015_calendar.svg | CC0 | renamed | accepted |
| `illustrator/four-bit-counter.svg` | Adobe Illustrator | https://commons.wikimedia.org/wiki/File:4_bit_counter.svg | PD | renamed | accepted |
| `illustrator/dot-pattern.svg` | Adobe Illustrator | https://commons.wikimedia.org/wiki/File:Adobe_Illustrator_Pattern_Punktmuster.svg | PD | renamed | accepted |
| `illustrator/animal-cell-hex-colours.svg` | Adobe Illustrator | https://commons.wikimedia.org/wiki/File:Animal_cell_structure_en.svg | PD | renamed | accepted (was `HugeNumber`) |
| `illustrator/beetle-range-css-hex.svg` | Adobe Illustrator 26.4.1 | https://commons.wikimedia.org/wiki/File:Areal_Colorado_potato_beetle.svg | PD | renamed | accepted (was `HugeNumber`) |
| `illustrator/arpanet-preserve-editing.svg` | Adobe Illustrator 10, preserve editing | https://commons.wikimedia.org/wiki/File:Arpanet_1974.svg | PD-self | renamed | refused `Entity` (was `UnknownFormat`) |
| `illustrator/bmi-chart-preserve-editing.svg` | Adobe Illustrator 12, preserve editing | https://commons.wikimedia.org/wiki/File:Body_mass_index_chart.svg | PD-self | renamed | refused `Entity` (was `UnknownFormat`) |
| `inkscape/torus-knot-7-9.svg` | Inkscape (generator comment) | https://commons.wikimedia.org/wiki/File:(7,9)_torus_knot.svg | CC0 | renamed | accepted |
| `inkscape/torus-knot-8-9.svg` | Inkscape (generator comment) | https://commons.wikimedia.org/wiki/File:(8,9)_torus_knot.svg | CC0 | renamed | accepted |
| `inkscape/binomial-cube.svg` | Inkscape | https://commons.wikimedia.org/wiki/File:(x%2By)3.svg | PD (ineligible) | renamed | accepted |
| `inkscape/insignia-gradient-chains.svg` | Inkscape (gradient `xlink:href` chains) | https://commons.wikimedia.org/wiki/File:01-IRIN-SR.svg | PD | renamed; the `inkscape:export-filename` attribute (a local path) removed | accepted |
| `inkscape/ten-pointed-stars.svg` | Inkscape | https://commons.wikimedia.org/wiki/File:10_pointed_star_shapes.svg | CC0 | renamed | accepted |
| `inkscape/three-phase-rectifier.svg` | Inkscape | https://commons.wikimedia.org/wiki/File:3_phase_rectification_2-ru.svg | CC0 | renamed | accepted |
| `inkscape/treaty-map-55-groups.svg` | Inkscape 1.2 (an Illustrator base map) | https://commons.wikimedia.org/wiki/File:ADR_Treaty_-_Members.svg | PD | renamed | accepted (was `TooComplex`) |
| `libreoffice/rgb-colour-wheel.svg` | LibreOffice (`xmlns:ooo`) | https://commons.wikimedia.org/wiki/File:RGB_color_wheel_12.svg | CC0 | renamed | accepted |
| `visio/choke-diagram.svg` | Microsoft Visio | https://commons.wikimedia.org/wiki/File:Dzia%C5%82anie_d%C5%82awika_przeciwzwarciowego_1_Workaround.svg | CC0 | renamed | accepted |
| `visio/spaghetti-markers.svg` | Microsoft Visio (`xmlns:ev`, `mrkr` ids) | https://commons.wikimedia.org/wiki/File:Grl_spaghetti.svg | PD-self | renamed | accepted (was `TooComplex`) |
