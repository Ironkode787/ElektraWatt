<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Symbol design standard

**Status:** Draft v0.1 · **Date:** 2026-11-05 (2026-11-06, the document set's header and purpose) · **Part of:** ElektraWatt design document set

**Purpose.** This document is EW authoring profile 1: the conventions a symbol follows, whoever draws it. It applies
equally to people drawing in Parts, AI programs and imported drafts, and refines the symbols of
[07: Symbols, parts and macros](07-symbols-parts-macros.md); the drawing of Parts implements it. It is an ElektraWatt convention inspired by ECAD practice, not an
IEC certification or a copy of a vendor library.

| Subject | Required convention |
|---|---|
| Coordinates | Exact integer nanometres in files; millimetres in editing controls. Origin is insertion point, x right, y down. |
| Connections | Native 4 mm grid. Each pin has a stable unique index, explicit outward direction and function-pin mapping. Printed designations may repeat; indices may not. Never infer connectivity from drawn lines. |
| Graphics | Default snap 0.5 mm. Clean monochrome outlines, no decorative shadows/gradients. House pen P026 for outlines, P013 for fine detail. Existing explicit imported strokes are retained. |
| Text | Default snap 1 mm; style-driven sizes. Tag and function text outside the body where practical; designation slots adjacent to the correct pins, clear of the connecting wire. Text remains readable after rotation/mirroring. On a symbol whose pins leave up and down the first places are those of [06a](06a-drafting.md) §5.2: the tag at the upper left, right-aligned 1 mm left of the graphics, its foot one step under the upper pin; make, type and rating under it on the same side, line under line 6 mm apart (4 mm on the QET grid); each pin number right of its pin's line at its outer end. An interruption point's name stands in line with its wire beyond the arrow. **The function text:** every device symbol of the IEC and ISO sets has a function-text slot, shown, and prints its device's function text there and nowhere else. Its place: on the tag's side, at the tag's x, right-aligned with its top on its line, one text line (6 mm, 4 mm on the QET grid) under the lowest of the texts under the tag (a cross-reference among them), or 1 mm under the tag's foot where none stands, so that no other text moves. Lying (a symbol turned a quarter) it takes its line among the texts placed anew under the body, as every text does ([06a](06a-drafting.md) §5.2). `cargo xtask symbols-qet10` applies the rule: it shows a hidden slot where it stands and adds a missing one at the rule's place; a symbol whose texts stand otherwise (the valve solenoid, its texts in line with its body 4 mm apart) has its place by hand. |
| Function | Separate drawing from electrical semantics. Assign one function definition and map each graphic pin to its corresponding function pin. Declare internal connectivity and potential roles explicitly. FE and PE are distinct. |
| Layout | Align related pins, use consistent spacing and short leads ending on the outline. Prefer symmetry where it helps reading; preserve circuit meaning over visual symmetry. Leave space for designations. |
| Variants | Derive A–D by quarter turns and E–H by mirroring then turning. Never renumber pins when transforming. Preview all eight before publishing. |
| Anchors | Insertion origin and cross-reference anchor are different concepts. Default cross-reference anchor: GraphicCentre. Named anchors and linkage points are explicit; resolved reference anchors use the connection grid. |
| Identity | Stable descriptive key, readable multilingual name, semantic version. Fork upstream content into a new key. Publishing does not silently update existing placements. |
| Provenance | Licence and source required. Keep imported foreign terms and source identity. Do not add an IEC reference unless verified; never copy standards' descriptions. |
| Imported grids | QElectroTech stays on QET 10 px. No automatic rescale or pin snapping; repair requires an explicit edit and preview. |
| Automation | Use the same draft model, validation and publisher as the editor. Deliver data, never executable drawing scripts. Review the preview and validation report before use. |

EPLAN's common properties and orientation variants inform the model; KiCad's explicit pin grid, consistent strokes
and text placement inform the authoring discipline. Their dimensions and vendor formats are not EW defaults.
Sources: [EPLAN variants](https://www.eplan.help/en-US/Infoportal/Content/Plattform/2026/Content/htm/symboleditorgui_h_mitsymbolvariantenarbeiten.htm),
[KiCad symbol conventions](https://klc.kicad.org/symbol/s3/), [EW grid policy](../adr/0027-native-symbol-grid-and-qet-grid-policy.md).

Editor errors block publishing: off-grid pins, invalid geometry, missing function mappings, invalid slot references,
unknown unsupported drawing kinds, missing provenance or an invalid version. Advice about layout or visual balance
does not silently alter the symbol. A valid file still needs engineering review of its electrical meaning.

Test placement requires identical symbol and page connection grids; otherwise the refusal offers a new page of its grid: Place on a new page with its grid places the published symbol while the draft is saved to a library and unchanged, Test on a new page a test copy otherwise (M25-p7#2).
Unknown active electrical semantics remain inspectable in a draft but block publication and placement.
