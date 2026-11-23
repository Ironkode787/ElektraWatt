# Interoperability: import, export and migration: QElectroTech: texts, terminals, tables, the verifier and the symbol map

A chapter of [Interoperability: import, export and migration](14-interop.md), whose table says where each section lives.

---

### 4.10 Fonts and text styles

- Qt `QFont::toString()` strings come in the Qt 5 form (10 fields, or 11 with a trailing style name; weight 0–99)
  and the Qt 6 form (16 or more fields, weight 1–1000), both within one file. A string is read as Qt 6 when it has
  at least 16 fields or its weight exceeds 99, otherwise as Qt 5: the field count alone does not tell an 11-field
  Qt 5 string from Qt 6. Fields used: family, point size (pixel size × 0.75 when the point size is
  −1), weight (bold at ≥ 63 on the Qt 5 scale, ≥ 600 on the Qt 6 scale), italic.
- Families map through the font table of the hints: Tahoma, "Sans Serif", "MS Shell Dlg 2", Arial, Helvetica and
  Liberation Sans → the bundled default family (Noto Sans, proposed ADR-0028). Bundled families map to themselves.
  Each substitution is recorded once per family (EW-IMP-006), in the migration report and the PDF XMP (FR-IMP-020,
  FR-OUT-009), with a warning that line breaks may change. To print a set in QET's own face, the project's typeface
  takes that face's file ([09](09-reports-forms-output.md) §4.8).
- One import text style per distinct (family, size, weight, italic) is created, named `qet.<size>[.b][.i]`. These
  styles are not bound to slot kinds (EW-IMP-114); "apply `tag.main`" restores one tag height (D-29).
- Sizes stay in native points: QET 12 px → 9.00 pt native → 8.88 pt printed through the frame transform (20 §17.5);
  a size that is no whole number of px takes QET's whole px (below).
- The project import adds each import style the drawing names (symbol texts and slots, free texts) to
  the project's text styles (`settings.text_styles`, ADR-0032): `qet.<size>` has the em height QET prints a font of
  that many points at, bold for `.b`; italic is not a text-style property and is dropped.
- *In whole pixels.* QET's own PDF shows that QElectroTech 0.100 prints its texts as Qt lays them out on
  its screen, at Qt's 96 dpi in whole pixels; the import follows it, so an imported text stands where QET's print has
  it in QET's face (a computer at another logical dpi rounds otherwise; the file does not say). A font of `size`
  points has `round(size × 4/3)` px to the em, halves up (7 pt is 9 px, 5 pt is 7 px, not 9⅓ and 6⅔), a px being
  `793,750 / 3` nm. A line's ascent and descent are each rounded up to a whole px, from the face QET's texts print in
  (Liberation Sans and Arial: 0.905 and 0.212 em): a free text's first baseline stands that ascent below its box's
  4 px margin, and its lines, an element's static texts and the box a cross-reference is placed under are that ascent
  plus descent apart (8 px: 8 + 2; 12 px: 11 + 3). Each glyph's advance is a
  whole px too; a title block's cells carry it as their `ew.cell.pixel` of one QET px
  ([09](09-reports-forms-output.md) §2.5), so a right-aligned or centred cell stands where QET's does. The drawing's
  free and element texts keep the face's own advances.
- *A title block's cell* prints as QET's `TitleBlockTemplate::finalTextForCell` writes it, after one space
  (` label: value`, or ` value`), and a cell with `hadjust` takes the overflow `fit_width`
  ([09](09-reports-forms-output.md) §2.5): QET 0.100's `renderTextCell` scales a text wider than its cell by the cell's
  share of its width in both directions (`painter.scale(ratio, ratio)`) and centres the scaled cell's height in the
  cell; it never shrinks a text for its height, so a cell whose lines are taller than it overflows it. Only QET's DXF
  export condenses a text across alone; the print does not. Where another face's line pitch, and not the text, makes
  the lines a cell was made for taller than the cell (a four-line notice in Noto Sans), EW closes the lines up, their
  size unchanged and never to less than 1 em apart, until their box lies inside the cell, and scales the block down
  whole only where 1 em apart is still too high, so no line runs over a border; in QET's face they stand as QET's
  print has them.
- *On QET's baseline.* Against QET's own PDF of a pilot project (QElectroTech 0.100, Qt 5.15), every
  imported text stands on QET's baseline (a free text with `anchor: baseline`, an element's texts `baseline_left`),
  so a project whose typeface is QET's face prints it where QET does; the dynamic texts and free texts of 4 to 9 pt
  stand within 0.3 mm. A title block's cell text has no inset (`ew.cell.inset` 0, [09](09-reports-forms-output.md)
  §2.5): QET draws it on the cell's edges.
- *What still moves.* Against QET's own PDF (`ew verify-migration`), a text that still moves has a cause the
  import does not decide:
  - the folio list: EW draws QET's folio list as its own table of contents;
  - rich texts in `Sans Serif` that QET's computer printed in Tahoma, 0.6 to 1.4 mm higher (Tahoma's ascent in
    whole px is about 1.0 em, Liberation Sans's 0.905): which face QET prints such a text in is its computer's
    substitution, which the file does not say. The project's typeface does not close it: with Tahoma as the typeface
    they stand apart, because the import stands each text on the baseline QET's default face gives;
  - masters' contact lists and their contact symbols: QET draws them with a contact symbol per row 10 px apart
    (§4.5);
  - texts QET prints turned to read downward or upside down, which EW turns to read from the bottom or the right
    ([07](07-symbols-parts-macros.md) §4.1, [09](09-reports-forms-output.md) §4.8);
  - in the default typeface, title-block texts: Noto Sans's widths, and the four-line cell's lines closed up
    to stay inside it, are not QET's face's; with that face as the typeface they stand where QET's do.

---

### 4.11 Strips, terminals and cables

- **QET 0.100+ strip data** (FR-IMP-013): `terminal_strip_data` → strip device; `physical_terminal` → terminal device
  in `seq` order; each `real_terminal` element → one level, whose element becomes a representation of that level's
  CPs through `pin_map`; `terminal_strip_bridge` → `Connection{kind: Bridge}` over consecutive terminals. The
  designation is the terminal element's label ("1+"); empty and duplicate designations are emitted as found
  (EW-TERM-001/002, MF-08/09).
- **Older projects:** terminal elements are grouped by label prefix (`X1031:1+` → strip X1031, designation "1+"),
  else by the strip header element on the same folio; each inferred strip is EW-IMP-005.
- A terminal element joins a strip by, in this order: (1) its label
  prefix, with an optional structure prefix (`+JE2416-X1031:1+`); (2) the strip header that heads its line, a header
  being an element labelled with a strip designation (optional structure prefix and sign, then digits, letters and
  digits: `X1031`, `-X1031`, `1X1`); lines are per folio, a column per x for elements turned a quarter, else a row
  per y, and the header is itself a terminal of its strip; (3) on a folio without any header or label prefix, the
  strip the folio before it (in file order) names last: a strip drawn on over a second folio, EW-IMP-005;
  (4) the fallback strip `X?{page}` of its folio (§14 item 5, EW-IMP-005). An earth symbol QET files as a terminal
  (one terminal and an earth name in its path: `terre`, `earth`, `ground`) is no terminal: it is an earth point of
  its own, `EarthingBar / ProtectiveEarth / Point` with a PE connection point, as the starter's `ew.fn.pe`; a junction
  element is a junction (§4.7). A strip is identified by its own designation including the structure prefix (`+A-X1` and `+B-X1` are
  two strips), the fallback by its folio, never by a line number, so its key (§2.3) is stable under re-import
  (FR-IMP-040). One designation drawn in several lines or folios is one strip (EW-IMP-109). A terminal's designation
  is its label suffix, else its first `UserText` dynamic text (where the reference set writes "1+" and "PE"), else a
  label that is not a strip designation, else its sequence number with EW-IMP-008, because an empty designation
  cannot be stored (`Ident` is never empty). The 0.100+ `<terminal_strips>`
  data and its bridges are not read (EW-IMP-004).
- **Segments.** Several drawn segments or header elements with one strip identity become one strip with several
  segment representations (D-42), reported as EW-IMP-109 with a "split" quick fix.
- **Cables, 0.200:** `<cable>` → cable device (`designation` → tag, `length` → `LengthSource::Manual`, `type` →
  part text); `<core index part conductor>` → `Conductor` assigned to the connection.
- **Cables, older** (FR-IMP-014): a cable-marker element (label -106W1) crossing conductors becomes a cable whose
  cores are the crossed conductors in crossing order; colours from `conductor_color` or colour texts beside the
  crossing; EW-IMP-005 for review. "BR" is not an IEC 60757 code, so the core keeps the foreign code with the
  proposal "BN" (§4.12). A mark is an element without terminals; one labelled with a
  tag of the letter W heads a cable device (the `Cable` aspect) whose main function it draws, and the unlabelled
  marks of its definition in its row (the same y, for a row across) draw auxiliary functions of it, as do those of
  a folio without a cable mark of its own that continue the cable the folio before it heads last. Each wire a mark
  crosses (its stored route runs through the mark's insertion point, across the row) becomes a core, numbered from
  1 in the order of the marks, folio by folio; a wire two marks cross is one core. Colours and the cable's part
  (the mark's element information) are not read: they stay in `Device.extra["qet.info"]`.

---

### 4.12 Text to data

Free texts carry data in QET drawings (D-08). After geometry mapping, every free text and `UserText` runs through
the recognisers of FR-IMP-017:

| Recogniser | Pattern (default, configurable in the hints) | Attachment | Target |
|---|---|---|---|
| Wire marker | `^(?:(\d+(?:[.,]\d+)?)\s*(?:mm²\|mm2)?\s+)?COLOUR(?:\s+COLOUR)*$`, COLOUR from IEC 60757, profile codes (DBU) and aliases; matches "2.5 BK BU", "1.5 DBU", "WH", "2,5 BK" | conductors crossing the text's reach band (perpendicular to the text, ±2 grid steps), ordered along the text; one colour per crossed wire, or one colour for all of them (medium confidence) | CDP overrides (level 7), pruned by pass P2: "1.5 DBU" keeps only the 1.5 mm² override, because DBU equals the profile |
| PLC address | `^%?(I\|Q\|E\|A)(\d+)\.([0-7])$`, `^%?(IW\|QW\|EW\|AW)\d+$` | nearest coil or I/O element within one frame cell | pending channel binding (pass P7), then a binding: the channel keeps its `Auto` address when the text equals the derived address, else the text is kept as `Manual(Q0.1)` ([10](10-plc-and-bus.md) §3.3, A7) |
| Process tag | project regex, default `^[A-Z]{2}\d{2}-[A-Z]-\d{2}$`; "Spare" | nearest valve or field element | `Device.process_tag` ("TA05-V-15"); "Spare" → state Spare proposed |
| Port number | `^(1\|2\|3\|4\|5\|12\|14\|82\|84)$` and grouped "3 1 5" | nearest terminal of a fluid symbol | CP designation and port role |
| Bus address | `^Node\s+(\d+\|TBD)$` | bus node element | `BusPort.address = Value(n)` or `Tbd` on the node's bus port (D-15; [03](03-domain-model.md) §14) |
| Termination | `Switch to "?ON"?` | bus connector element | `BusPort.termination = On` (FR-BUS-004) |

- **Confidence.** High: exact pattern, one candidate within one grid step, counts match. Medium: one candidate
  within one cell. Low: several candidates or a count mismatch. High-confidence matches are pre-ticked in the review
  queue; medium ones are listed; low ones are not proposed but appear in the review list (EW-IMP-108), so "every
  unconverted address-like text" is visible.
- **Cross-section marks**. QET cross-section mark elements (`section1conducteur`,
  `section2conducteurs`, 456 uses in the pilot corpus, [23](23-symbol-coverage-pilot-corpus.md) M-03) are not
  symbols. A mark whose value equals the crossed wire's derived cross-section is dropped. A mark whose value differs
  is dropped as well, the wire keeps its derived property, and EW-IMP-123 carries the mark's value with the quick fix
  "set the override to the mark's value", so nothing the mark said is lost and no override is created silently.
  This is not built: a mark arrives as a placed element that prints its own texts (QET users write cable tags,
  part texts and core colours on it). It has no terminals, and EW-CORE-069 counts only the bodies of
  representations that draw a connection point, so the marks QET draws across their wires raise no "wire through
  a symbol".
- Converted texts are removed from the page in the same transaction and listed (EW-IMP-107), with a revert action.
  The conversion of the reference's four marker groups into ten CDPs is MF-07.
- **Colour aliases.** The shipped table maps foreign colour notations to IEC 60757: BR → BN, SW → BK, BL → BU,
  RT → RD, GE → YE, WS → WH, GR → GY, GNGE and "GN/YE" → GNYE. GR is ambiguous (German *grau*, but "green" in some
  English shorthand), so its proposal is medium confidence. Mapping is a proposal; until accepted the core keeps
  `ColourCode::Foreign("BR")`, which only lenient imports can create (FR-IMP-044, EW-CORE-031, FR-CBL-005,
  FR-CHK-032; [03](03-domain-model.md) §3.4).
- **Fluid symbols** (FR-IMP-018): QET's 5/2 valve has six unnamed generic terminals. A `symbol-map.csv` row maps
  terminal positions to ports 1–5 and the coil 14 function, so the valves import with ports and coils.

---

### 4.13 Tables, shapes, images and layout pages

- **`graphics_table`** (FR-IMP-016): SQL against `element_nomenclature_view` → BOM block (component mode);
  `project_summary_view` → TOC block; `wiring_list_view` (0.200) → wire-list block. Custom SQL and columns are
  dropped (EW-IMP-004). A table filling its folio replaces the drawn page by a `ReportBlock{first_folio}`; a partial
  table becomes an embedded report block. Pass P5 compares QET's nomenclature rows, evaluated by the importer from
  element information, with the derived BOM (EW-IMP-113: on the reference, rows -4W1 and -4X1 disappear and missing
  parts appear).
- **`graphics_table`.** A table holding a query heads a chain; a table naming it as `previous_table`
  continues it on another folio. The view the query reads gives the kind: `project_summary_view` → `toc`,
  `element_nomenclature_view` → `parts_list`, `wiring_list_view` → `wire_list`; any other query is not imported. The
  block lists EW's own columns in EW's table style (one EW-IMP-004). When every folio of the chain holds nothing
  but its table, those folios give way to one page-oriented block: first folio the head's, continuation
  `next_folio` when QET's folios are consecutive (else sub-pages), the head's frame form, and the head's title,
  where titles `T 1`, `T 2` … become `T {n}`; so the reader finds the same rows on the same folios when EW's pages
  hold as many rows as QET's (a block that needs more pages takes the next folio numbers). Otherwise the block is
  embedded in the head's folio in the head table's rectangle, and continuations elsewhere are reported. Pass P5 is
  not built.
- **Shapes, `<inputs>` and `<images>`** become `PageItem::Graphic`, `FreeText` and `Image` items; QET pen styles map
  to the dash patterns of 20 §17.4. A free text's `rotation` counts clockwise (Qt), so the importer stores it
  negated as an `Angle2`, which counts counter-clockwise as seen on the sheet (ADR-0005 amendment 2): QET 90°
  becomes `Angle2(-90_000)`.

  In `project/items.rs` a free text is placed as an element's own text is (§4.4.2), one free text per
  line; QET writes free texts as Qt rich text, whose paragraphs are the lines and whose first span or body sets the
  size. A line, rectangle and polygon map to their graphic shapes, a square ellipse to a circle; a rectangle with
  corner radii and any other ellipse become a polygon through 7 points per quarter (integer cosines, so the batch
  is the same on every platform). Pen `SolidLine`, `DashLine`, `DotLine`, `DashDotLine` give solid, dashed,
  dotted and dash-dot, `widthF` the stroke width (0: the page's wire stroke); a brush other than `NoBrush` fills.
  An image is drawn in its picture's own pixel size scaled by `size` about its middle (QET turns and scales an
  image about its centre); its bytes travel in the batch (`ImportBatch.assets`) and `import.apply` stores them as
  project assets, as `item.place` takes an image (at most 512 KiB, a known format, the pixel limit; else the image
  is left out with EW-IMP-002). What the records cannot carry is reported with EW-IMP-004 and the item kept: a pen
  colour other than black, a dash-dot-dot pen (drawn dash-dot), another brush pattern (drawn solid), an image's
  rotation. A thumbnail element becomes a graphic (§4.4).
- **Layout pages** (FR-IMP-019): a page whose rectangles carry tag-like texts is offered as a layout page. Pass P6
  scores each text against derived tags: exact (-102G1), after identifier suppression ("X1031" → -X1031), and fuzzy
  within the same letter and number group (Levenshtein ≤ 1: "T51" → T55, folio 101's 5x group). Every binding is a
  review proposal (EW-IMP-111); confirmed ones become placements on a plate in drawn order ([12](12-panel-layout.md)
  §16.3), unmatched items stay layout graphics.
- **Assemblies.** Elements of valve or module classes without a tag (the 28 valves on 203–206) are reported
  (EW-IMP-112). Drawn overview positions per module are counted against the slaves on the detail pages
  (EW-IMP-110: 5 drawn on -202U2 against 4). The assembly and its children are built after `ApplyImport` by the
  valve-terminal reconstruction assistant of [11](11-fluid-power.md) (FR-IMP-039): -201U1 with its 11 children -201U2 … -202U3 and the 28 valves, bound to Q0.1 … Q6.7. EW-CORE-046
  (FR-CHK-022) then takes over from EW-IMP-110.

---

### 4.14 QETZ (1.x)

`.qetz` formats 2–4 are re-joined into the `.qet` DOM, as QET does, and passed to the same mapper (FR-IMP-024):
`mimetype` first and stored (`application/x-qet-project+zip`, or the older `application/x-qelectrotech-project+zip`);
`manifest.xml` `<qetz format min-reader writer>` checked (a `min-reader` above 4 is refused); `<qetz-part file>`
and `<qetz-db block>` markers resolved from `folios/<uuid>.xml`, `elements/…`, `titleblocks/…`,
`images/<sha256>.png` and `project.sqlite`; member names with `..`, `\`, `:` or an absolute path are refused. Only `ew-sqlview` and `ew-library` may link
rusqlite (ARCH §3 rule 2), and importers depend only on `ew-interop-api` and their parsers (rule 4; both enforced
by NFR-MAINT-01). The host
therefore opens the SQLite member with `ew-library`'s hardened opener (read-only, `trusted_schema=OFF`, defensive
mode, schema check against `meta`, `project_property`, `folio`, `folio_property`, `element`, `element_info`, `link`, `conductor`,
`terminal_strip`, `strip_terminal`, `strip_bridge`, `strip_bridge_terminal`; any view or trigger rejects the file,
ARCH §7.7) and passes the importer a neutral `TableImage` of those tables through `ImportCx` (ARCH §10.6).

---

### 4.15 Migration verifier and acceptance

- **Verifier** (FR-IMP-021): renders each imported page with the CPU renderer at 150 dpi through the frame
  transform, rasterises the matching page of a reference PDF with hayro, and compares (1) text runs (string and
  anchor within 0.5 mm) and (2) the raster per frame cell with text boxes masked, so font substitution does not
  swamp the result. Pages pair by folio label, the pages no label names by order (the reference PDF has physical page labels, D-33).
  Expected deltas are declared per project (AC-27), such as a font substitution. `ew verify-migration <project> --reference <pdf>` prints the deviations. The verifier makes
  hayro a runtime dependency of the shipped `ew` binary (ARCH §3).
- **Corpus acceptance** (SC-4, FR-IMP-006; [vision](00-vision-and-scope.md) §3.2): at least 22 of the 24 QET example projects (C-14) and every pilot
  project import without a structural error; `photovoltaique.qet` (31 of 64 wires with modern ends) and
  `schema_indus.qet` (integer ends only) lose no connection; pass P4 reports no print-equivalence deviation on the
  corpus except declared ones; each project's batch is byte-identical across platforms.
- **Pilot acceptance:** AC-25 … AC-27 and NFR-UX-06 (one hour from import to zero migration errors on a
  reference-size project).

The verifier is `ew_api::migration` (`ew verify-migration <project> --reference <pdf>
[--expected <file.json>] [--json]`, `ew.verify-migration-result/1`). It lives in `ew-api` because it needs the session's
page export, the frame cells of `Session::page_cells` and the CPU raster of `output::sheet_image`; a crate of its own
would depend on `ew-api` for all three. The PDF side (`ew_scene::pdf_text`: the text runs, the page labels) sits in
`ew-scene` beside `underlay`, the one place that uses hayro.

- **Pairing.** Pages pair by folio label wherever the reference's `/PageLabels` name the folio; the project pages no
  label claims take the reference pages no folio named, in order (a missing page leaves the labelled pages
  after it where they are; a reference without labels pairs wholly by order). A page without a partner is a deviation (`page-not-in-reference`, `page-not-in-project`);
  pages whose sizes differ by more than 1 mm are `size-differs` and are not compared further.
- **Text.** Both sides give texts with their anchor (the first glyph's origin), segmented by one rule
  (`ew_scene::pdf_text::Segmenter`): hayro draws a PDF page into a device that gives each glyph as a piece, the
  project's sheet gives each run of one font as a piece, and a piece joins the text before it when it continues that
  text's baseline at the pen (from 0.3 em before to 0.1 em after it, within 0.15 em across; a kerned `r,` or `r…` draws
  its comma more than 0.15 em back), whatever its font. So
  a text with a glyph of another font (², ∞, −, ⇄) is one text, a producer that draws every glyph by itself (Qt's PDF
  engine, which draws its spaces as glyphs) gives whole texts, and a gap splits: the texts of neighbouring cells stay
  apart. White space is not compared (where a word
  breaks is the font's business).
  Per string the nearest pairs match; a pair within 0.5 mm is a match, a farther pair is `text-moved` (with the
  offset in words), what is left is `text-missing` (the reference has it) or `text-extra`.
- **Raster.** Both pages at 150 dpi on the project page's pixel grid (the reference stretched to it: hayro's raster
  is a pixel short on some sheets, and is resampled to the grid, never skipped). A page whose raster would exceed 4,096
  pixels a side (about 693 mm, so A1) is refused in words: the reference cannot be drawn at that size. A pixel is ink
  below luminance 200; the boxes of every run of either side, padded 1 mm, are cleared; an ink pixel left with no ink
  within 0.5 mm on the other side, looked for before the boxes were cleared, counts for its frame cell (the page's
  frame grid; a page without one is a single cell). A
  cell with more than 60 such pixels (about 10 mm of a hairline) is a `raster-cell` deviation. The partner is looked for
  before the boxes are cleared, so a line just outside a text box on one side and just inside it on the other keeps
  its partner.
- **Expected deltas** are a JSON file, an array of `{"kind", "page", "text", "cell"}` (the last three optional): a
  deviation one entry matches is reported as expected, an entry that matches none is reported as not found, and
  either way a clean exit needs every entry found and no other deviation. Font substitution needs no entry for the
  raster (the text boxes are masked) but moves texts by a font's side bearings and baseline, so a project with another
  font declares `{"kind": "text-moved"}`.
- **PDF text map.** The PDF the program writes maps each glyph to the letters it stands for: a glyph per
  character, a ligature (`fi`, `fl`, `ffi`) its letters, found by the font's `cmap`; a glyph's map is the same in every
  run of the document, so a run's text never gives wrong letters to later runs that use its glyphs.
- **Limits.** A text a PDF draws as paths (outlined) has no text to compare; the raster still compares it unmasked.
  Instance symbols' own texts are part of the sheet list as the export gives it. The tolerances are constants of the
  module, not options.

---

### 4.16 The project importer

The lenient import of whole projects (`QetProjectImporter`, `ew import qet`) is the front door (§2.1) plus
`ApplyImport` into an empty project, so the import writes a run and its migration report is the check run of §2.5.
Its rules are in the sections above: keys (§2.3), the report (§2.4, §2.5), tags (§4.4.1), interruption points (§4.6),
conductor ends (§4.7), title blocks (§4.9), strips (§4.11) and cross-section marks (§4.12). Beyond them:

- **Findings.** Definition findings are reported only for definitions that placed elements use, and a font
  substitution once per family.
- **Built-in documents.** The importer resolves `ew.fn.ip` and `ew.titleblock.qet-a4` in the starter content the
  host passes (`ew import qet --content`, the window's content directory). Without a content directory both take
  the documents compiled into the program (`StarterLibrary::compiled`: the starter symbols and FnDefs and that
  title block), so a folio report, and with it its connections, is never dropped for want of the content.
- **Not imported** (each reported, mostly as EW-IMP-004 or EW-IMP-008 with counts): formulas and EW-IMP-101,
  rules R2/R3, the structure nodes of labels and EW-IMP-102 (a page's `=` and `+` of its title block are imported,
  §4.9), parts from element information,
  text-to-data recognisers, the cables of QET 0.200 (`<cables>`), conductor properties as potential
  definitions and overrides, external points, `<terminal_strips>` and their bridges, empty terminal
  designations, and the exchange of QET-import symbols for starter symbols through `symbol-map.csv`
  ([23](23-symbol-coverage-pilot-corpus.md) M-01 to M-09). The import detections EW-IMP-105, EW-IMP-106, EW-IMP-110 and EW-IMP-112,
  EW-IMP-111, EW-IMP-114 and EW-IMP-121 are not built. A junction element is counted against the first
  of several pasted copies (no corpus file has one).
- **Determinism.** The import of a file is byte-identical across runs.

---

### 4.17 The symbol map: elements bound to functions

A set brought in from QElectroTech keeps its look while it becomes a project: the symbol map says which of the set's
element definitions is which EW function, which of its terminals is which connection point and which of its texts
prints the tag, the number and the cross-reference. The element keeps its own drawing, place and texts; only what
they stand for changes. It is §4.3's step 1, for four kinds: coils, contacts, terminals and interruption points.

**Where it lives.** A table `symbol-map.csv` in the folder of the `.qet` file. File ▸ Import… and File ▸ Re-import
QElectroTech… in the window, and `ew import qet` (with or without `--into`, and each file of `--batch`), read it when
it is there; one map serves every set of its folder. The import card and the command's summary say that it was read
and how many rows it has; the migration report says what each row bound.

**Its form** (the `ew` CSV dialect of §8.4: UTF-8, a header row, comma, quotes):

```csv
element,function,cps,tag,number,xref
bobine3.elmt,ew.fn.coil,A1 A2,,,
con_simple.elmt,ew.fn.contact-no,13 14,,,
borne_continuite.elmt,ew.fn.terminal,1 2,,,
```

| Column | Holds | Empty |
|---|---|---|
| `element` | the element definition: its collection path or the end of it after a `/` (`01_coils/bobine3.elmt`, `bobine3.elmt`; `embed://import/` may be left off), or the UUID of its `<uuid>` (braces may be left off). Of two rows whose paths both end a definition's, the longer wins | refused |
| `function` | the key of a starter FnDef that is a coil (contact kind coil), a contact (NO, NC, change-over), a terminal (category `Terminal` or `TerminalEarth`) or an interruption point (category `InterruptionPoint`) | refused |
| `cps` | the FnDef's CP designations, one for each terminal of the element in definition order, separated by spaces | terminal *i* is the FnDef's CP of index *i* |
| `tag` | the element's own text that prints a coil's or contact's tag, an interruption point's name | `label` (an interruption point: `user`) |
| `number` | the text that prints a terminal's number (its designation, which a terminal prints as its tag) | `user` |
| `xref` | the text that prints an interruption point's cross-reference | `label` |

A text is named by what QET binds it to: `label` (the label information, also a composite text of it alone), `user`
(the first user text), the name of another element information (`function`, `designation`, …), or `-` (none).

**What a row does.** The element's symbol becomes a copy of its own (another item than the unmapped symbol, so a
re-import adds it) that names the row's FnDef, each terminal the CP the row gives it; no FnDef is generated. A coil is
its device's main function, and a `master`'s linked slaves stay its auxiliary functions; a contact linked to a master
(`slave`) is an auxiliary function of that device, any other one a device of its own; a terminal is a terminal of a
strip (§4.11), its designation read from its number text; an interruption point drawn as a folio report keeps its
pairing (§4.6), any other one is an interruption point of its own, named by its tag text, without direction, which
pairs by name. The named texts show the symbol's slots where they stand (§4.4.2): the tag, the terminal's number and
the cross-reference follow the project from then on. A bound coil's and contact's cross-references stand where QET
prints them (§4.5), by the definition's own `kindInformation` `type`, which the row does not change.

**A set already imported** takes the map by its re-import (D12): the same file re-imported into its import with the
map beside it. The bound records are changes of the source; a record a person changed since the import is kept and
listed, as every re-import keeps it.

**Refused in words, nothing imported:** a table outside the dialect; a missing `element` or `function` column or an
unknown one; an empty element or function; a function no starter FnDef has, or one of another kind ("`ew.fn.motor-3ph`
is a motor: the map binds coils, contacts, terminals and interruption points"); a CP designation the FnDef does not
have, or one named twice; a `cps` list longer or shorter than the element's terminals; a text column the row's kind
does not take (a number on a coil); two rows of one element. Each refusal names the file, the line and the column.
**Reported** (EW-IMP-007, info): each row used, with the number of elements it bound; a row that no element of the
set uses.

**What it does not do:** change the drawing; make parts, or exchange the symbol for a starter symbol (the "C" rows of
[23](23-symbol-coverage-pilot-corpus.md)); bind other kinds (motors, breakers, black boxes, cables); map element
information or title-block values to fields.

