# Reports, forms and page output: the page output pipeline and exports

A chapter of [Reports, forms and page output](09-reports-forms-output.md), whose table says where each section lives.

---

## 4. Page output pipeline

### 4.1 Scene and display-list layers

`page_display_list(page, layer, lang)` (Memo + LRU) builds the IR in nm. The display language is a query key
(ARCH §4.5, §6.4; ADR-0015 amendment 2). Schematic and report pages use the layers below; PanelLayout pages use
surface, carriers, placements, features and dims plus frame, labels, tables and highlights
([12](12-panel-layout.md) §5.2).

| Layer | Drawn pages | Generated pages | Depends on |
|---|---|---|---|
| `frame` | frame form, grid labels, title block | no-grid frame, title block | form, `title_block(page)` |
| `symbols`, `wires`, `labels` | representations, connections, derived texts | — | [06](06-schematic-editor.md) |
| `tables` | embedded blocks | report data area | `RowSet`, form |
| `revision` | revision clouds and marks (1.0) | — | page change status ([15](15-checks-revisions-collaboration.md) §5.6, §5.8) |
| `highlights` | screen only, never exported | — | session |

Layers paint from bottom to top in the table order: frame, symbols, wires, labels, tables, revision, highlights; panel
pages paint surface, carriers, placements, features, dims, then frame, labels, tables, highlights.

The frame layer of folio 103 depends on Prev/Next (102 and 106) and project data, so a wire edit on 103 never
rebuilds its title block, and renumbering 103 → 104 rebuilds only the title blocks of 102, 104 and 106 (plus the
`labels` layers that print "103-…" xrefs or the strip tags -X1031/-X1032, §8.2). The page-to-sheet transform of the
frame form is applied once, identically, by every writer.

`ew-scene` builds each layer (ADR-0015 amendment 7) as a
pure function of a caller-filled `PageView` (records by reference, derived texts as slot values); each layer reads
only its own inputs, records are taken in ID order, a CP's position comes from the representation with the smallest
ID that draws it, and slot visibility is the override, else the arrangement, else the symbol default. The `revision`
and `highlights` layers are empty so far; the `tables` layer draws the house report table of a generated page from a
caller-filled `TableView` of printed texts (in its frame's list area, §3.8: x 16.31 to 277.27 mm on the starter A4
frame; rows of 6.52 mm, header #C0C0C0, column widths by the kinds' weights, text shrunk to a printed cap height of 1.8 mm and then reported, never cut). A wire
without a stored route is drawn as the unrouted dashed line. The `Session`
export job (`ew_api::output`) owns the view's glue rules: a divider or a page without a form uses the no-grid frame,
the wire-marker text is "csa colour" (both provisional), and the slots filled are the tag, the CP
designation, the function text, the process tag, the cross-reference with its item links, the technical data, the
manufacturer, the part number and the type designation. Each part of the view is a query family, and
each layer is its own Memo + LRU family (`page_display_list_frame`, `_symbols`, `_wires`, `_labels`, `_tables`) over
the parts it reads, so a wire edit re-runs no frame layer and a renumber re-runs exactly the frames whose title-block
values change (tested; the memo oracle compares every layer with its cold value). The labels
knock out the wires on a schematic page (ADR-0034 amendment 3): the `wires` layer cuts a line round a text of
the `labels` layer that it runs through, reading those texts' boxes through a family of their own (`page_knockouts`),
so a renumber that moves no such text still re-runs no wires layer; it also draws a strip's bridges where their
terminals stand in one row ([08](08-connections-terminals-cables.md) §6.5).

A PanelLayout page draws its
surface view through the `surface`, `carriers`, `placements` and `labels` layers of `ew-scene`'s `panel` module, each
its own memo family (`page_display_list_surface`, `_carriers`, `_placements` and the shared `_labels`), from a view
that `ew-api` fills from the layout queries; `features` and `dims` are empty until 1.0, and the `tables` layer of a
panel page draws its embedded blocks as on any page. The projection divides by the frame's page-to-sheet
factor, so the page's scale holds on paper; the title block's `page.title` on a page of scale 1:n reads "<title> ·
Scale 1:n" in the language the page prints in ("Schaal 1:n" in Dutch, a title typed as one text too). A placement move rebuilds `placements`, `labels` and `carriers` (the labels knock out the carriers),
never `frame` or `tables` ([12](12-panel-layout.md) §5.1 to §5.3).

The `tables` layer draws every embedded
block on its page through one path: `ew-api`'s `PageEmbeddedOf` fills a view per block with each cell rendered for
that page; the cover (`profile_tables`) keeps its own variant `EmbeddedContent::Cover` (folio 1 byte-equal), and
every other kind is a boxed table, `EmbeddedContent::Table`. House values in printed mm: a 2.61 mm drop shadow, the
box at 0.418 mm, a title band of 1.5 rows in style `box.title` when there is a title, flow columns split by a 4 mm
gutter and filled in turn; with headers, a header row with the house fill, data rows of 6.52 mm and a 0.183 mm grid;
without headers (PART DATA) no inner grid; a block without columns (a block whose rows do not derive) draws no header
and prints its reason in one spanning row. Columns split by weight with the house cell rule shared with the generated
pages' tables (`column_edges_in`; left aligned, shrunk to the 1.8 mm cap height, then reported, never clipped); rows
beyond the capacity are not drawn and are reported (`TooManyRows`, FR-REP-039). PART DATA is drawn on 201, 202 and
902, the legend on 902.

### 4.2 PDF (krilla)

Exports run as a job on an O(1) fork: global queries first, then per-page display lists in parallel (rayon), then
sequential assembly in page order (FR-OUT-001, ARCH §2). The page phase runs on `ew_export::par_map`, scoped `std` threads with a strided split and results collected by page index, instead of rayon; serial and parallel share one code path and their output is byte-identical (AC-14).

| IR item or document feature | PDF construct | Requirement |
|---|---|---|
| `Path(BezPath, Stroke, Brush)` | path with stroke or fill, caps, joins, dash pattern from line style; a path with neither stroke nor fill draws nothing and is a valid invisible link or pick region (ADR-0015 amendment 9) | FR-OUT-018 |
| `Glyphs(ShapedRun)` | glyph run from the shared shaping result, subset-embedded font, text with real space glyphs for extraction | FR-OUT-008, D-33 |
| `Image`, SVG asset | raster image; SVG logos as vectors via krilla-svg | FR-SYM-025 |
| `Instance(symbol, transform)` | the symbol's paths under the instance transform | — |
| `LinkSource` | link annotation over the xref text's bounding box (the `layout_box` of its `ShapedRun`: advance × ascent/descent in run space, filled by `ew-text`) → named destination. A text that lists several items carries one link per item instead: an invisible path over that item's glyph span only (not the "∞ " prefix or the parentheses), written as a link annotation over the axis-aligned box of the path's control points | FR-OUT-005, FR-XRF-019 |
| `LinkTarget` | named destination at the anchor (XYZ, zoom unchanged) | FR-OUT-007 |
| Row `LinkRef` | link annotation per linked cell (BOM folio, wire ends, PLC xref, TOC rows) | FR-REP-042 |
| `page_order` + sections | outline: front matter, "Section 1 – Wiring Definition" → 100 … 106, "Section 2 – Pneumatics", "Section 9 – Nomenclature"; each entry "101 Power Supply 1 - Main Current"; optional structure level | FR-OUT-004 |
| Folio labels | `/PageLabels`: one range per page with the folio label as literal prefix ("101", "901.1") | FR-OUT-006 |
| Metadata | XMP and Info: title (project description), drawing id, revision, generator, derivation profile, font substitutions, language; author omitted under personal-data omission | FR-OUT-009 |
| Page size | `MediaBox` per page from its frame form's sheet, so A4 and A3 pages mix in one PDF; landscape pages are not rotated | FR-PAG-014, FR-PAG-018 |
| Conformance | PDF/A-2b with output intent; A-3b only with attachments after a confidentiality confirmation | FR-OUT-002, FR-OUT-003 |
| Revision clouds (1.0) | cloud paths around the representations [15](15-checks-revisions-collaboration.md) §5.6 marks as changed, per scheme option | FR-REV-016 |
| Tagged PDF (1.0) | structure tree: page content as sections, symbols as figures with alternative text "tag, function text", report tables as tables with header cells, title block last | FR-OUT-012, NFR-A11Y-05 |

- **Destination names** derive from stable IDs: `ew:p:<page uuid>`, `ew:r:<representation uuid>`,
  `ew:cp:<v5(function, idx)>`, generated pages `v5(block, index)`, a section divider its section's ID; a device placed
  on a panel layout `ew:pl:<placement uuid>` (its footprint) and its row on the layout's legend page `ew:lr:<placement
  uuid>` (0.3.5, [12](12-panel-layout.md) §5.4). Only targets
  referenced by a link get a destination, plus one per page, which bounds the file size at XL.
- `/PageLabels` are krilla's own page labels (ADR-0015 amendment 5).
  Glyphs map to text one char per glyph when the counts agree, else the run is one cluster (clusters in
  `ShapedRun` follow); `.notdef` glyphs are dropped; link annotations use a rectangle; raster images are not drawn; links out of a partial export are plain text.
  Glyph coverage: the font set holds Noto Sans Math Regular as its one symbol fallback face, only
  for characters the style's own face lacks in Latin, Greek and Cyrillic text; a text with such glyphs is one group
  of one glyph run per face, so such a PDF embeds both fonts and drops no `.notdef` glyph. A `.notdef` glyph
  remains only for a character no bundled face drew, and only such a character is reported (FR-OUT-027).
- **Links:** one xref link annotation per listed item (a link covers the item's glyph span only, not the "∞ " prefix
  or the parentheses) plus report back-links (AC-13); "103-G6" lands on cell G6 of folio 103.
- **Partial exports** print links to non-exported pages as plain text, or as remote links into a companion file
  named by the scheme (FR-OUT-020). A remote link is a GoTo-remote action to the companion's named destination,
  which PDF/A-2 permits; krilla's support for it is unverified, the fallback is a URI action
  `<file>.pdf#nameddest=<name>` (viewer-dependent).
- **Status mark (1.0).** An unreleased revision gets "DRAFT V3.0" in the sheet margin outside the frame
  (FR-OUT-021). The pilot basic project ships with the mark off until the pilot decides.
- **Monochrome** maps every stroke and text to black and every non-white fill to its luminance grey; wire-colour
  codes stay as text (FR-OUT-011, G8).

### 4.3 Determinism and reproducibility

The export input is the tuple (snapshot revision, export scheme, date, release context, EW version, derivation
profile, font files by hash). Rules:

1. **Dates.** PDF, XMP, XLSX and ZIP dates come from, in order: the release context, an explicit scheme date,
   `SOURCE_DATE_EPOCH`, the model's edited date. Never the wall clock (FR-OUT-026, NFR-REL-07). The export job takes the caller's date (`ew export` reads `SOURCE_DATE_EPOCH`; an unparsable value exits
   2), else the model's edited date, else its created date, else 1980-01-01; release contexts and scheme dates do not exist.
2. **Identifiers.** The PDF document ID and XMP instance ID are hashes of the export input, not random. The document ID is krilla's hash of the text `ew-pdf/1 <project id> <derivation
   profile> <date> <pages>[ mono] ew <EW version> sha256:<digest>`, `<pages>` being `all` or the selected folio
   labels, and `<digest>` the SHA-256 over every exported page (label, outline title and section, sheet size,
   destination, sheet display list through its `Hash` order) and the pinned SHA-256 of every bundled font. The digest is computed through the std `Hash` order of the display lists, which is not guaranteed stable across toolchains; sha2 over canonical bytes is not built. The pages
   are derived values, so the ID is the same however the model was loaded (the bootstrapped tables, the saved
   project, a shuffled record order; AC-14, AC-15); it is per build target like the PNGs.
3. **Order.** Pages in `page_order`; objects and resources in first-use order; maps are ordered (`BTreeMap`); parallel
   page building collects results by page index, so serial and parallel exports are identical (AC-14).
4. **Arithmetic.** nm → pt conversion runs in integer arithmetic (milli-pt = nm × 72 / 25 400, rounded half to
   even) before f32 values reach krilla. Arcs and rotations use a pure-Rust `libm`-style implementation in `ew-dl`,
   never the platform's `sin`/`cos`, whose last bit can differ between platforms; kurbo's arc-to-Bézier conversion
   calls `f64::sin_cos` and is therefore not used on the export path.
5. **Fonts.** Bundled font files and the project's typeface (§4.8) only; subset tags are content hashes (krilla 0.8.2, ADR-0015 amendment 5). A glyph run holds one font, so **a text line may be a group of runs, one per
   font**: a line whose characters come from the style's face and the symbol fallback is one `Group` with the
   origin, the angle, the entity and the one link of the text, holding one run per face in text order; a line drawn
   by one face stays one run (ADR-0015 amendment 8).
6. **Images** are passed through or re-encoded with fixed encoder settings.

GUI and CLI call the same exporter. The release check exports the sample project on Windows and Linux and in a display-less Linux
container (NFR-PLAT-04) and compares SHA-256 for PDF, SVG, XLSX and CSV (FR-OUT-019, FR-OUT-022) and PNGs within
the dify tolerance (§4.4); veraPDF validates PDF/A-2b; hayro rasterises the PDF for the ±0.2 mm frame check
(ARCH §13).

The application shell's File ▸ Export PDF and its headless
`file.export_pdf` call `Session::export_pdf` with the default options, the exporter of `ew export pdf`, as a job with
progress and cancel ([16](16-ux-ui.md) §8).

**Budgets.** Paginating every block cold takes S/M ≤ 3 s, L ≤ 15 s and XL ≤ 60 s (NFR-PERF-10).
PDF export averages ≤ 50 ms per page, 500 pages in ≤ 10 s on a snapshot while editing continues, and
`ew export pdf` of the sample project ≤ 3 s (NFR-PERF-11); not measured. The export never holds the UI thread for more than 100 ms and
cancels within 500 ms (NFR-PERF-19); it writes `<name>.tmp` and renames on success, so a cancelled export leaves no
partial file (FR-UI-024).

### 4.4 SVG, PNG and DXF

- **SVG (MVP, FR-OUT-013).** Own writer from the IR; one file per page; `viewBox` in mm; numbers with 3 decimals;
  text as `<text>` with the font subset embedded as `@font-face` data; element order equals IR order; `id`
  attributes from entity IDs in NCName form (`ew-r-<uuid>`: the PDF destination name with ":" replaced, because
  XML IDs may not contain colons); links as `<a href="103.svg#ew-r-<uuid>">`.
  `ew_export::svg` writes (FR-OUT-013, with named fonts) one `<label>.svg` per page from the sheet display lists of the PDF:
  `viewBox` and lengths in mm with at most 3 decimals from integer nm, element order = IR order, text as `<text>`
  with one `x` per character; ids are written for link targets only (`id="ew-r-<uuid>"`, once per page on the first drawn item of the target; links point at the first page that draws it), sources as `<a href="<file>#<id>">` (SVG 2 `href`, so the root carries no `version` attribute). The fonts are
  named (`font-family`, weight, style), not embedded: subset embedding needs a subsetter that keeps the `cmap`
  (open: it needs a `cmap`-keeping subsetter, the `@font-face` embedding above is the target). The per-page phase runs on `ew_export::par_map` (scoped `std` threads, results by page index) instead of
  rayon; serial and parallel exports are byte-identical (AC-14). The SVG SHA-256 printed in the headless report's AC-14 entry is an SVG golden: a change to the SVG output re-blesses the headless report in the same commit, with the reason in the commit message, as the PDF SHA-256 does. Cross-platform comparison of the PDF and SVG digests is not built. The `ew export svg` and `ew export pdf` subcommands are built over the `Session` export job, which runs `par_map` in the shipped export path; AC-14 tests that path. The `--out` directory of `ew export svg`, `ew export png` and `ew export dxf` is replaced as a whole through a sibling temporary directory, so no stale or partial set remains. A set leaves the file `.elektrawatt-export` in it, which names the set's kind and every file it wrote; the next export of the same kind replaces only a directory that is empty or holds just that file and the files it names, and refuses any other in words (exit 2), whatever the extensions of its files, leaving it as it was: a folder of a person's own pictures is never taken for an earlier export (a directory written by 0.3.0, which left no such file, is refused once). The PDF is written to `<file>.tmp`, synced and renamed.
- **PNG (MVP, FR-OUT-014).** vello_cpu through `ew-render-cpu` at a chosen DPI (`ew render --page 103 --png --dpi
  150`). vello_cpu may choose different SIMD paths per CPU, so PNGs compare within the dify tolerance across
  platforms; `ew-render-cpu` runs on vello_cpu's portable `Level::baseline()`, so PNGs are byte-identical per build
  target (ADR-0015 amendments 3 and 6).
- **DXF (1.0, FR-OUT-015).** AC1024 (§4.9), `$INSUNITS` = mm, layer mapping per scheme (default `EW-FRAME`,
  `EW-SYMBOLS`, `EW-WIRES`, `EW-TEXTS`, `EW-GRAPHICS`, `EW-TABLES`); symbols as `BLOCK`/`INSERT` named by symbol and
  variant; Béziers flattened to polylines at 0.01 mm; dash patterns as `LTYPE`; texts as `TEXT` with a `STYLE` per
  face. Re-import as a graphic page within 0.01 mm (AC-37).
  `ew_export::dxf` is an own writer (no dxf crate, whose graph brings random GUIDs
  and clock-stamped headers) from the sheet display lists, one `<label>.dxf` per page, AutoCAD 2010 (`AC1024`) in
  UTF-8 (§4.9: the version, the type, the picture, the blocks and attributes, the links), `$EXTMIN`/`$EXTMAX` the
  sheet so a re-import puts the drawing back where it was. A fill is a solid `HATCH`, a colour other than black a
  true colour. A `STYLE` is one per face, not per text style, and names the face's font file; a `TEXT` is as high as
  its face's capitals (§4.8). The layer names are options (`ew export dxf --layer <class>=<name>`), and printed
  underlays go to `EW-UNDERLAY`. No date, user name or GUID is written: the same pages give the same bytes. AC-37's
  DXF half is `crates/ew-api/tests/it/dxf_round_trip.rs`.

### 4.5 Printing

PDF is the print master (ADR-0015). A print job builds the PDF for the chosen range on a fork, then hands it to an
adapter of the GPL crate `ew-print` (per-OS features; ADR-0015 amendment 1), which both `elektrawatt` and `ew print`
link:

| Platform | Adapter | Notes |
|---|---|---|
| Linux | xdg-desktop-portal Print (ashpd 0.13.13): `PreparePrint`, then `Print` with the PDF descriptor; CUPS fallback | Works in Flatpak |
| Windows | `PrintDlgEx` (windows 0.62.2) → HDC; the **same display lists** that produced the PDF rendered by vello_cpu in bands at printer DPI | a 600 dpi A3 band of 256 lines is 7–10 MB RGBA |
| All | "open in the system PDF viewer" fallback | — |

- **Ranges** by pages, sections or folio list ("101-106" prints the drawn folios 101, 102, 103 and 106).
- **Scale-to-fit** stamps the factor in the margin of the print PDF only: an A3 sheet on A4 carries "scaled 71 %"
  (FR-OUT-017). Monochrome follows §4.2.
- **Print preview** renders the print PDF's display lists in the app. There is no macOS adapter: macOS is not a supported platform.
- `ew print --pages 101-106 --printer <name>` uses the same adapters without a dialog (FR-EXT-009).

The print set (`ew_api::output::print`: `Paper`, `Fit::{Actual, ToPaper}`, `PrintOptions`,
`Session::print_set`) places each page on its paper (`PagePlacement`, in `ew-render-cpu`); a set with no placement
change is the export's PDF byte for byte. Fit scales each page to the whole paper, up as well as down (an A4 set on A3
at 141 %, §3.8), and stamps "scaled <n> %" in the bottom margin;
without it a larger page is clipped and named in a notice. Ranges are
folio labels and ranges (`--pages`) or section numbers (`--sections`). `ew-print`'s adapters form a chain with one
fallback notice: on Linux the portal (ashpd 0.13.13 with `async-io`, no tokio), then `lp`, then the viewer; on
Windows the Windows adapter, then the viewer; elsewhere the viewer. The Windows adapter shows `PrintDlgExW`
(print-to-file hidden, copies and collation left in the printer's settings) or prints to `--printer` without it,
asks for each page's paper by `ResetDCW` (A1 and A0 by their sides, having no Windows paper size), and sends the
page rendered by `ew_render_cpu::render_placed_band` in bands of 256 device rows with `StretchDIBits`; its `unsafe`
stands in `ew-print`'s `windows.rs` alone ([18](18-quality-security-release.md) §11.2). The window prints through
Print… (Ctrl+P) in the Output menu, whose pre-dialog previews the print set, built as a job
([16a](16a-design-language.md) §6.26).

### 4.6 XLSX and CSV

Every report block and navigator table exports from its `RowSet` with exactly the rows and order of its pages
(FR-REP-030, FR-EXP-002). `ew export bom|wires|terminals|cables|report` and `ew sql` use the same code and take
`--format xlsx`, `--preset ew|excel-nl|excel-en|rfc4180` (`ew`, the default), `--bom` and `--line-end`.

- **XLSX** comes from `ew-reports`' own writer (`xlsx()`, over `ew-zip`, inline strings): one sheet per block, named
  after its kind; a header row from the localised column labels with units ("Cross-section [mm²]"); typed cells,
  where a quantity with a unit and a number past 15 significant digits stay text; frozen header, autofilter, column
  widths from the form. The `id` column (`RowId`) is always written; for the round trip (FR-EXP-004) a hidden `_ew`
  sheet (format `ew-table/1`, project ID, block ID, export revision, locale, column schema) and a hidden `_ew_base`
  sheet with the editable cells as exported, as [14](14-interop.md) §8.1 specifies. All text, tags such as "-101F1"
  and "+J-E-2416" included, is written as string cells, so Excel never evaluates it. Workbook properties and ZIP entry
  dates follow §4.3.
- **CSV** is UTF-8 in every dialect (FR-EXP-003). The default **`ew`** dialect (`ew_reports::csv`) has LF line ends,
  no byte-order mark, a header line `row,<column labels>` (the first column `data` or `heading`), and a field quoted
  exactly when it holds `,`, `"`, LF or CR or begins or ends with a space. The presets write the `id` column first and
  no `row` column: `excel-nl` (`;`, decimal comma in number cells, byte-order mark, CR LF), `excel-en` (`,`, decimal
  point, byte-order mark, CR LF) and `rfc4180` (`,`, CR LF, no byte-order mark, values verbatim). `--bom` and
  `--line-end` override a preset's byte-order mark and line end.
- **The formula guard** (NFR-SEC-17): in `excel-nl` and `excel-en` a text cell and a header label whose first
  character is `=`, `+`, `-`, `@`, tab or CR gets a leading apostrophe, which the dialog says, and the Excel round-trip
  import (FR-EXP-004) strips exactly that prefix again; a number cell never gets it. `ew`, `rfc4180` and machine
  profiles never alter a value. XLSX is the format for Excel users.
- **Label files** ([13](13-manufacturing-outputs.md) §5.4), read by a label program, have a form of their own: CSV in
  UTF-8 with a byte-order mark, CR LF, a header row, the export profile's delimiter and every value as it is (never
  the formula guard, which the label would print); or an XLSX of one sheet with a header row and text cells, without
  the `id` column and the hidden sheets of the round trip. Opened in a spreadsheet, a CSV value that starts with `-`,
  `+` or `=` is read as a formula: the XLSX is the form for a spreadsheet, and the guide says so.

### 4.7 Multi-language output

- **One derivation, another key.** A per-language PDF reruns `page_display_list(page, layer, lang)` and the
  `RowSet` formatting with another display language (FR-I18N-005, 1.0). Identifiers never change (FR-I18N-002).
- **Stacked texts.** Function texts may print "NL / EN" in one field via `ml(stacked: …)` (FR-I18N-004).
- **Fallback.** A missing translation prints the source text and appears in the `untranslated` report.
  `ew_derive::text_in`, every output's one rule: a text prints in the project's language: its neutral
  text, else the string of that language (or of its language, `nl` for `nl-NL`), else English's, else the first
  language's in tag order. A text in many languages (a QElectroTech element's name: `ar`, `cs`, `de`, `en` …) thus
  prints in English where the project names no language or the text lacks it, not in the first of them.
- **Language-independent pagination.** With the default fixed-height rows (§3.2), every language yields the same
  page set, folios, TOC and links, so AC-42 holds: only translated texts differ. A block with wrapping enabled may
  paginate differently per language; the export then warns and lists the page-count differences (FR-REP-049).
- **Overflow per language.** Cells shrink to the minimum cap height and then report (FR-I18N-012). Colour names come
  from the profile per language (FR-I18N-013).
- **Cost.** Each additional language reuses the language-independent memos (geometry, rows) and costs at most the
  NFR-PERF-11 budget (NFR-I18N-05).
- **Archive.** From 1.0 the as-built archive holds one PDF per display language
  ([05](05-project-format-and-persistence.md) §14.2).
- **Translation exchange.** Translations travel in the XLIFF 2.1 and CSV exchange format of
  [14](14-interop.md) §8.5 (a round trip with base values). This document owns the per-language output and
  [16](16-ux-ui.md) the translation UI.

### 4.8 The project's typeface and the baseline anchor

A drawing set that must match an existing print in another typeface (a QElectroTech set printed in Liberation Sans)
needs the program to print that typeface, and its texts to stand where that print has them whatever typeface prints
them. Both are built.

**The setting.** `ProjectSettings.typeface` is an optional member of the project record: `{"asset":
"sha256:<64 hex>", "name": "LiberationSans-Regular.ttf"}`, the font file as an asset of the project
(`assets/<sha256>.ttf` or `.otf`, [05](05-project-format-and-persistence.md) §13.3) and the file's name as the person
chose it, without its folder. Absent, the project prints as before. An older build keeps the member in
`settings.extra` and the file as an opaque file, and prints with the bundled faces.

- **In the window:** Project ▸ Text styles… has the line *Standard typeface* above the styles: "Noto Sans, comes
  with the program" or the file's family and name, with **Choose a font file…** (`text_styles.typeface {path}`, the
  file dialog) and **Use Noto Sans** (`text_styles.typeface_clear`). **By a command:** `project.typeface.set {file,
  name}`, `file` the font file in base64 as a list of parts joined in order and `name` its file name; an empty list
  removes the setting. One undo step; bulk, so the window previews it.
- **A large file.** A command's parameters hold at most 1 MiB in one string and 4 MiB in all (doc 18), and a
  picture comes into a project as one string (at most 512 KiB). A font file comes the same road in parts: each part is
  one string of at most 1 MiB, so a file over 768 KiB is two to four parts of one command. It stays one transaction
  (one undo step, one line of the crash journal with its file) and one line of a command log that `ew apply` replays;
  the window's action and a script send the same parts. The limits of command parameters stay as they are.
- **The ceiling: 2.5 MiB.** The text faces of one typeface weigh up to about 1.8 MiB: of 597 single-face files of a
  Windows computer, 584 are at most 2 MiB, among them Arial (1.0 MiB), Tahoma (0.9), Segoe UI (0.9), Times New Roman
  (1.1) and Calibri (1.6); only emoji, symbol and East Asian faces and faces for every script weigh more (2.4 to
  22 MiB), whose further letters no drawing of the program prints (its fallback serves Latin, Greek and Cyrillic). Its
  base64 fits the 4 MiB of one command with room to spare.
- **Refused in words** (both languages, at `/file` or `/name`): a file over 2.5 MiB, naming the ceiling and which fonts
  weigh more; text that is not base64; a file that is no font of
  one typeface (a TrueType or OpenType file: not a collection `.ttc`, not a web font `.woff`); a font whose tables
  cannot be read; a font without the letters A to Z, a to z and the digits; a font whose embedding permission forbids
  what the PDF does (below); an empty name, or one with a folder in it or over 255 characters.
- **Missing or unusable later.** A project whose file is gone (deleted by hand, a partial copy) or whose bytes are no
  font prints with the bundled faces, and the *Standard typeface* line says so; nothing is refused and nothing is
  lost (the setting stays, so the file can be added back).

**What the typeface replaces.** It is the face of the Standard typeface (`Family::Default` of `ew-text`) in both
weights: every text style whose face is Standard prints in it. Narrow, Mono and Technical drawing keep their bundled
faces, and Noto Sans Math stays the fallback for a character the file lacks (a character neither has prints `.notdef`
and is reported, FR-OUT-027). The file is one face: a bold Standard style prints in it regular and keeps its weight,
as with Technical drawing (ADR-0028 amendment 3). The font set of a typeface is built once per file per process and
kept (`ew_text::typeface_set`); the project's set is the memoised query `ProjectFontsQ`, which every page layer, the
panel labels and the export read, so a change of the setting redraws every page and an image added to the project
redraws none.

**The PDF** embeds the subset of the glyphs a page prints, as for the bundled faces (krilla; §4.3 rule 5), so the
document carries only those glyphs. The SVG embeds a TrueType file's subset (an OpenType file with PostScript outlines
is named only, §4.4); the PNG, the print and the window draw the outlines from the file.

**The licence.** The program ships no such font: the file is the user's, and the project carries the whole file, so
whoever gets the project, its `.ewz` or its as-built archive gets the file. The *Standard typeface* line says so.
The program honours the font's own embedding permission (`OS/2.fsType`, read as the OpenType specification says):
a font whose permission is *restricted* (the permission field 2), may be embedded as bitmaps only (bit 9) or may not
be subset (bit 8) is refused; *installable*, *editable* and *preview and print* fonts are taken. Whether the user may
pass the file on with the project is the user's licence question; the program cannot answer it.

**The DXF** cannot carry a font. Each face a page prints with is a `STYLE` that names it: `EW-<FAMILY>[-BOLD]`, its
font file (group 3: the typeface's own file name, a bundled face's `<Family>-<Regular|Bold>.ttf`). A `TEXT` stands at
its run's baseline start with the face's own cap height (the face's `OS/2` cap height over its units per em: 714/1000
for the Noto faces as before, the file's own for a typeface), so a reader that has the same font file draws every text
where the PDF has it; a reader without it substitutes a font of its own, and only the widths differ.

**The window** shapes and draws with the project's font set: the canvas, the page pictures and the print preview
draw the display lists the export writes, so the page on screen and the page on paper agree. The Text styles samples
of the Standard rows are drawn in the typeface. Nothing else of the window's own type changes: its menus and panes
keep Noto Sans.

**The baseline anchor.** A text says where its point lies on it vertically.

- A free text has the optional member `anchor`: `baseline` puts the first line's baseline on `at` (more lines go down
  from it); absent (or `middle`) centres the text from the first line's cap height to its last baseline on `at`, as
  before. `item.place` takes `anchor`; Properties shows it as *Anchor*.
- A symbol's text (a static text or a text slot) has three more alignments, `baseline_left`, `baseline_centre` and
  `baseline_right`: the baseline of the text's first line on its point, left, centred or right; the symbol editor
  offers *Baseline* beside top, middle and bottom.
- Turned upright or placed in a variant that brings its body to the other side of its point (a text at 180°, a
  symbol turned half round or mirrored top to bottom), a baseline text is placed as a bottom-aligned text is: by
  the top of its box ([`ew_scene::symbol::placed_align`]); turned a quarter, by the middle of its side.
- In a box (a form's cell), a baseline alignment puts the first line's baseline on the box's top edge less the
  padding; no shipped form uses it.
- The QElectroTech import writes it ([14](14-interop.md) §4.4.2, §4.10): a free text and an element's texts stand on
  QET's own baseline, so they keep their place in any typeface.
- A project without the member and the alignments prints every text where an older build did, to the nanometre, with
  the bundled faces only (`crates/ew-api/tests/it/typeface.rs`).

**What it does not do.** No system fonts (NFR-REL-11 holds: the file is in the project). One file only: no second
file for the bold or italic cut, no typeface for Narrow, Mono or Technical drawing, no font of a frame or title block
of its own. No font over 2.5 MiB. The DXF does not embed or outline the texts. The window's own type is not
the typeface. A text placed with the Text tool keeps the middle anchor.

### 4.9 The DXF set as the PDF

A person who must deliver the drawing set as CAD files gets from `ew export dxf` one file per page that is the page of
the PDF: the same texts in the same places and sizes, the title block with its picture, in a format AutoCAD 2010 and
later open, and with the page's structure, so that the receiver can work with it. It writes no DWG: a
DWG is made from the DXF by a converter (the ODA File Converter, which the import runs the other way, doc 14 §6.5; or
the receiver's CAD program). The program's own DWG export, which would run the user's converter on this DXF, is not built. So the DXF is made to become a good DWG of the 2010 format
(the choices marked *for the DWG* below).

**The version: AutoCAD 2010 (`AC1024`).** It is the format AutoCAD 2010 to 2012 write, and every AutoCAD since opens
it; so do the converters and readers of DXF (the ODA File Converter, BricsCAD, LibreCAD, ezdxf). *For the DWG:* a DWG
of the 2010 format is `AC1024` too, so a converter carries every object over as it is, without converting it to
another version's form. What it changes against the earlier `AC1015` file:

- Strings are UTF-8 (AutoCAD 2007 and later read a file of `AC1021` or later so), so a text is written as itself
  ("Ä", "∞", "²"), no longer as `\U+XXXX`; `$DWGCODEPAGE` stays `ANSI_1252`, as AutoCAD writes it.
- The CLASSES section holds the classes of the picture's objects (`IMAGE`, `IMAGEDEF`, `IMAGEDEF_REACTOR`,
  `RASTERVARIABLES`, with their instance counts) when a page has a raster picture, and is empty otherwise.
- Attributes are written as AutoCAD 2010 writes them (`ATTDEF`, and `ATTRIB` after its `INSERT` with `66` = 1 and a
  closing `SEQEND`).
- The APPID table names `PE_URL`, the application of AutoCAD's hyperlinks.

The rest stays: the sheet in millimetres in the model space, `$EXTMIN`/`$EXTMAX` and the limits the sheet, the tables,
the model and paper space with their layouts, no date, user name or GUID. The same pages give the same bytes. *For the
DWG:* every object has its handle and its owner (an attribute its insert, a picture's definition its dictionary and
the reactor that ties it to its `IMAGE`), as a DWG holds them, so a converter needs to repair nothing; and every name
(a layer, a block, a style, an attribute) is one a DWG may hold.

**Type.** As §4.8 merged it: a `STYLE` per face that names its font file, a `TEXT` at its run's baseline start, as high
as its face's capitals, at its run's angle. What a text further needs to stand where the PDF has it:

- *The face by its name.* Each `STYLE` carries its face's family and weight as extended data of application `ACAD`
  (1000 the family, 1071 the bold flag `0x2000000`), as AutoCAD writes a TrueType style, so a reader finds an
  installed face by its name whatever its file is called.
- *Widths.* The PDF's texts are shaped with the face's kerning; a CAD program draws a TrueType text with the face's
  own advance widths, unkerned (not verified in AutoCAD itself). So each text has the width factor (group 41) that
  makes its glyphs' advances in its face add up to its width in the PDF; it is written when it differs from 1. A
  reader with the face draws every text as long as the PDF does.
- *Alignment.* Every text is written left-aligned at its baseline start, the point the PDF places it at. The rule that
  placed it (centred in a cell, right of a pin) is not in the page's display list and is not written: a reader without
  the face draws its own font from the same start and as high, and only the text's end moves.
- *Turned texts* keep their run's angle (group 50), upright as the PDF draws them.
- *More lines.* Each line is its own `TEXT` (or attribute) at its own baseline, as the PDF draws it.
- *Two faces in a line* (a sign from the fallback face: the "∞" before a cross-reference): a `TEXT` per face, each at
  its own start; as an attribute the line is one `ATTRIB` in the face that draws most of its characters.
- *What the receiver needs*: the faces the `STYLE`s name, installed: Noto Sans and Noto Sans Math (SIL Open Font
  Licence: anyone may pass them on) and a project's own typeface (the user's file; whether it may be passed on is the
  user's licence question, §4.8). Without them a reader substitutes a font of its own: the texts still start where
  the PDF has them and are as high.

**The picture in the title block.** A DXF cannot hold a raster picture inside it for every reader (AutoCAD's OLE
object works on Windows only and no converter keeps it), so:

- An SVG picture is written as vectors, its paths as polylines and solid hatches, as the PDF draws it (the same
  conversion, `ew_scene::images`).
- A PNG or JPEG picture is an `IMAGE` that names a file the export writes beside the pages, `picture-<the first 12
  hexadecimal digits of its SHA-256>.png` (or `.jpg`), once for the set and by its name alone, with its `IMAGEDEF` in
  the drawing's image dictionary (`ACAD_IMAGE_DICT`). AutoCAD finds a picture named so in the drawing's own folder:
  the receiver keeps the picture files beside the page files, and so beside the DWG files a converter makes. *For the
  DWG:* a DWG holds a raster picture in the same way, as a reference to its file, so a converter keeps the `IMAGE`,
  its definition and the relative name as they are and the DWG set shows the picture as the DXF set does.
- A picture that cannot be drawn (its bytes not held, refused or not decodable) is a page finding of the export, as
  for the PDF.

**Blocks and attributes.**

- *A symbol as placed is a block*: one per symbol and variant, the geometry the page draws (`symbol_geometry(symbol,
  variant)`, a black box with its own outline a block of its own), inserted where the representation stands at the
  page-to-sheet scale; a junction's symbol likewise. The block is named in words: the symbol's name in the project's
  language as §4.7 prints a text (else English, else the first language of the name) and its variant (`Coil R0`,
  `NO contact M90`), a character a DXF name cannot hold as `_`; a second geometry
  of the same name takes ` 2`, ` 3`, in the order of its key, alike in every file of the set; a geometry whose symbol
  the page names nowhere keeps the name `EW_<symbol id>_<variant letter>`. The symbol's own fixed texts (the "M" of a
  motor) are in its block, as its first insert on the page draws them; an insert whose own texts differ from them by
  more than a micrometre keeps its own as `TEXT`s beside it.
- *A device's texts are attributes of its insert*: every text the page prints for the representation (its slot texts,
  a black box's pin texts, a terminal's strip tag) is an `ATTRIB` of the insert, where and as large as the PDF has it,
  on the texts layer. A text is named by what printed it, as the scene records it, wherever it stands: a text moved
  onto another slot's keeps its own name. In a card's cell the line that is the pin's designation is its attribute;
  the channel's function text beside it stays a `TEXT`. The block has an `ATTDEF` for each name its inserts on the page carry, where the first of them
  has it, so that a CAD program's attribute tools list them. *For the DWG:* blocks, attribute definitions and
  attributes are what a DWG holds itself, not a DXF-only form, so the DWG of a page has the page's structure.
- *The names are an interface*, so they are few, stable and in words: one per kind of slot (doc 07 §2.3), the same in
  every language.

  | The text prints | Attribute |
  |---|---|
  | the device tag | `TAG` |
  | the function text | `FUNCTION_TEXT` |
  | the technical data | `TECHNICAL_DATA` |
  | the manufacturer, part number, type designation | `MANUFACTURER`, `PART_NUMBER`, `TYPE_DESIGNATION` |
  | the designation of connection point n (a slot, or a black box's pin text) | `DESIGNATION_<n>` |
  | the cross-reference | `CROSS_REFERENCE` |
  | the PLC address, the process tag | `PLC_ADDRESS`, `PROCESS_TAG` |
  | the contact image, the order code | `CONTACT_IMAGE`, `ORDER_CODE` |
  | the bus address, the bus termination | `BUS_ADDRESS`, `BUS_TERMINATION` |
  | a user property | `PROPERTY_<its name>` |
  | a terminal's strip tag | `STRIP` |
  | a slot of a newer format | `TEXT` |

  The second line of a text, and a second text of one name, take `_2`, `_3` (`CROSS_REFERENCE_2`). n counts a
  symbol's connection points from 1. A user property's name keeps its letters a to z and its digits in capitals, any
  other character as `_`. No name begins with `PIN`, `CONN`, `TERM` or `X`, which the DXF import reads as connection points
  (doc 14 §6.2), while `TAG` and `FUNCTION_TEXT` are what it reads as the tag and the function text: a block of this
  export imports as a symbol with both slots.
- *A cross-reference says where it leads*: in words, as its text (`103-G6`: the page and the cell), and as a hyperlink
  (extended data of `PE_URL`, which AutoCAD follows on Ctrl+click) to the file of the page that draws its target, the
  text its description. Each line of a contact list links to its own contact; a line that lists several targets
  (`∞ 3-B2, 106-G3`) has no hyperlink: its items name them in words. A target outside the exported pages leaves the
  text without a link, as a partial PDF does. Every other text that links in the PDF (a row of the contents to its
  page) links so too. A link names `<label>.dxf`; with `ew export dxf --dwg-links` it names `<label>.dwg`, for a set
  that a converter turns into DWG (a converter keeps a link as it is).
- *The frame and the title block* are entities on the frame layer, each page its own: a title block prints its page's
  values, so as a block it would be one per page. Its texts are `TEXT`s; its picture as above.

**Layers** stay as §4.4 names them and as the options rename them (`--layer`): the frame with the title block and its
picture, the symbol inserts, the wires, the texts (the attributes among them: an attribute is on the texts layer, its
insert on the symbols layer), the graphics, the tables and a printed underlay. A block's own content is on layer `0`,
so it takes its insert's layer.

**What it does not do.** It writes no DWG (an export would run the user's converter on this DXF, as the DWG import
runs it the other way). No font in the file: a text is a `TEXT` in a
named face, never its outlines. No `MTEXT` and no attribute of more lines. No title block as a block with attributes,
no named views (a link opens the page and the text names the cell), no external references between the page files,
no paper-space layout of the sheet, no plot styles. A raster picture is never inside the file, and the rule that
aligned a text is not written. The window's export writes the default layer names and `.dxf` links: `--layer` and
`--dwg-links` are the command line's.

**In the window**: Export DXF… stands on the Output tab and in the Output menu beside Export PDF… and
Print…. It asks for a folder and writes the set there as a job (`file.export_dxf {dir}`), as `ew export dxf` does: the
folder is replaced as a whole, so it must be empty or hold just what an earlier DXF export wrote there (by its file
`.elektrawatt-export`, the rule of the command line's exports); a folder that holds any other file, whatever its extension, is refused
in words before the job starts and left as it was, and so is a folder inside the open project. A folder that holds
an earlier set is replaced only after a question, since its files may hold a person's later edits in a CAD program
(doc 16a §6.12: "Replace the DXF set in <folder>?", the count of the files that go, Cancel the Primary and Replace the
danger button); a script says `"confirm": true`, and without it the step fails in words. The command line's `ew export
dxf|png|svg` replaces its own earlier set without a question, as its help says. Its end says "Exported the DXF set,
one file per page, to <the folder's whole path>: <pages>, <size> KiB." with Open (the folder); a cancel before the
folder is replaced writes nothing. A PDF's or a table's end names the file and the folder it is in ("Exported set.pdf:
<pages>, <size> KiB, in <folder>.") with Open (the file) and Show in folder.

**Proven** by reading the files back, never by their bytes (`crates/ew-api/tests/it/dxf_as_pdf.rs`): on every page of
the sample and on a small project of the test's own (a coil on one page and its contact on another, an input card
drawn as a black box, a terminal of a strip, a logo), every text of the PDF's display list comes back through
`ew-import-dxf` where the PDF has it, as high and with its characters; there a device's texts are the attributes of
its insert under the names above, the coil's block is named by its symbol, each cross-reference links to the file of
the other page, the logo is an `IMAGE` of the picture file written beside the pages, and the symbol wizard reads the
coil's block with its tag slot. Measured: the largest distance between where the PDF starts a text and
where the DXF read back has it was 1 nm, on the sample's pages and on the test's own, the importer's rounding (the
writer writes the PDF's coordinates to the nanometre); how long a CAD program draws a text depends on the faces it
has. ezdxf 1.4.5 reads every page of the sample without an error and repairs nothing; drawn by its matplotlib add-on,
pages 101 and 104 show the PDF's drawing, texts and logo.

---

## 5. UI and UX touchpoints

| Touchpoint | Behaviour |
|---|---|
| Page navigator | generated pages appear in page order with a report badge and read-only state; dividers likewise (FR-PAG-024) |
| Report blocks panel | list of blocks; "add block" gallery by kind with a live first-page preview; filter, sort, group and folio in the property grid; folio collisions shown inline |
| Generated page on the canvas | read-only; an edit attempt shows "generated by block 'Wire list'" with links to the block and to the clicked row's source object; linked cells jump (FR-REP-042) |
| Embedded block tool | "Place report block" on a page; resize the rectangle; overflow badge |
| Company-standard wizard | picks the starter frame and title block and takes the company's logo and notice (FR-PRJ-010, FR-UI-030); the block itself is changed afterwards on a page of the template, with Edit frame |
| Edit frame | the frame and title block changed on a page that prints them: a cell's text, what it shows, its size and place, pictures, fields of one's own, the columns and rows, a title block or a frame of its own (§3.9; FR-PAG-027, FR-REP-040) |
| Title block fields… | the values a page's title block prints, written where each belongs, the fields of one's own among them (§2.10) |
| Export dialog | scope, format, scheme, PDF/A level, colour mode, languages, personal-data omission, file name preview; progress with cancel (FR-UI-024); a result list of warnings (font substitutions, stale values, untrusted SQL blocks, overflows) |
| Print | native dialog; EW pre-dialog only for scale-to-fit and monochrome where the native dialog lacks them |
| SQL report pages | "as of revision n" badge while the projection catches up |

A report form's table is not edited in the window: a list's columns, caption, legend and rows of its own are List
columns… and Rows of your own… (§3.8), its frame and title block Edit frame's (§3.9). The starter frames, title
blocks, cover and report forms are data files of the program's content.

---

## 6. Outputs

| Output | Format | Source | Deterministic | Release |
|---|---|---|---|---|
| Drawing set, sections, ranges, selection | PDF/A-2b | display lists | byte-identical | MVP |
| Per-language set | PDF/A-2b per language | display lists with `lang` | yes | 1.0 |
| Tagged set | PDF/A-2b and PDF/UA-1 | same | yes | 1.0 (Should) |
| Set with attachments | PDF/A-3b or PDF/A-4f | same + CSV/AML/native project | yes | 1.0 (Could) |
| Page images | SVG, PNG | display lists | SVG yes; PNG within the dify tolerance (§4.4) | MVP |
| Page drawings | DXF AC1024, its pictures beside it | display lists | yes | 1.0 |
| Report tables | XLSX, CSV | `RowSet` | yes | MVP |
| Print | OS spooler | PDF / display lists | — | MVP |
| Archive content | PDF, `reports/<block>.csv` | release snapshot | yes | MVP |

---

## 7. Edge cases and pitfalls

1. **TOC fixpoint.** The closed form of §3.1 settles the count in one confirming round; if a TOC row ever stands
   for more than one page, the larger count wins and the warning EW-CORE-025 is raised
   ([15](15-checks-revisions-collaboration.md) §4.2). A blank row is preferable to a missing one.
2. **Folio collisions.** `NextFolio` continuation or group breaks push pages into folios used by drawn pages or other
   blocks (a fourth strip under a grouped terminal block would take 906). Sub-page continuation is the default; the
   collision check EW-CORE-022 reports the rest (FR-REP-041).
3. **Report coverage.** With one block per strip, a new strip -X1033 has no terminal
   diagram until a block is added. The coverage rule EW-CORE-026 reports strips, cables and stations without a block
   of their family (FR-REP-047, [15](15-checks-revisions-collaboration.md) §4.2). EW-CORE-024 and EW-EN60204-015
   check the documentation set against declared standards at block-kind level only.
4. **Empty blocks.** Per-entity kinds omit their pages; list kinds such as open items print one "No entries" page,
   so the release documentation proves the list was empty.
5. **Text that does not fit.** Fixed-height cells shrink to the minimum cap height and then ellipsise with a
   warning (EW-CORE-062 from 1.0); never silent clipping (FR-REP-039). A report table's cell ellipsises at the floor
   (two lines at the floor exceed its 6.52 mm row), so it never prints over its neighbour. Before that, a column whose
   text would be cut takes room from the columns with room to spare at the full size, and where that is not enough
   (the I/O list's 20 columns) what it lacks at the floor from the room the others spare at the floor, so
   only a table whose texts do not all fit at the floor cuts one. The 35-character order code 2 in the 73 mm
   PART DATA box fits only on a value line of its own, as in the reference (estimate: ≈ 1.7 mm average advance per
   character at 8.88 pt gives ≈ 60 mm, while "Order Code 2: " plus the code needs ≈ 84 mm).
6. **Language-dependent pagination** (§4.7): wrapping makes page sets differ between languages.
7. **Floating point.** Platform `libm` differences and SIMD paths change last bits, so trig needs the controls of
   §4.3, and PNGs compare within the dify tolerance (§4.4).
8. **Glyph coverage.** Report and xref texts use "²", "∞" ("∞ 203-B4") and "⇄" ("0V ⇄ PE" for W27). The default
   family may lack some of them; the font set therefore includes a bundled symbol fallback, and a character that
   no bundled face drew is the finding EW-CORE-066 instead of a silent blank (FR-OUT-027). The fallback serves
   Latin, Greek and Cyrillic text, so a symbol inside CJK or Arabic text is reported until those fallbacks exist
   (FR-I18N-016, 1.x).
9. **Text extraction.** Spaces must be real glyphs ("SK 3110.000", D-33). Identifier styles disable ligatures and
   discretionary features so search finds every tag as typed.
10. **Link volume at XL.** BOM folio columns of 50,000 devices produce hundreds of thousands of links. The scheme's
    `links = "xrefs"` drops report back-links; destinations exist only where referenced.
11. **PDF size and memory at XL.** krilla assembles in memory (verify a streaming option); a 5,000-page export may
    approach several hundred MB. The export offers one PDF per section; NFR-PERF-13 covers peak memory.
12. **Stale Class II values in drafts** print with "≈" and the draft mark; release exports refuse them.
13. **Untrusted projects.** SQL blocks render placeholder pages; nothing executes (NFR-SEC-05).
14. **Personal data.** Omission must cover page text, report columns (party names), XMP, Info and file names;
    bound cells render empty, never "<redacted>" text that shifts layouts.
15. **Page-to-sheet transform.** Link rectangles, destinations, SVG `viewBox`, PNG and print use the transformed
    coordinates; cell lookup and xref texts use the untransformed model (FR-PAG-020).
16. **Spreadsheet limits.** Excel cells hold at most 32,767 characters and sheet names 31; tag lists in BOM rows are
    range-compressed, and overflowing cells are truncated with a finding.
17. **Scaled printing and thin lines.** 0.130 mm lines printed at 71 % become 0.09 mm. An opt-in printer-profile
    minimum line width (default off) applies to the print job only, never to the exported PDF; when it is active the
    print carries it in the margin stamp next to the scale, so it is the one declared exception to FR-OUT-018's
    identical geometry.
18. **Large-format Windows printing.** A0 at 600 dpi is about 20,000 × 28,000 px; banding keeps memory bounded, but
    spool size is large.
19. **BOM position numbers.** `Pos` is the row index after sorting (§2.4), so inserting a part renumbers every later
    position. Purchasing
    lists of a released revision keep their positions through the frozen `reports/<block id>.csv` (§3.7); stored
    item numbers (EPLAN item numbering) are a 1.x candidate.
20. **DXF and SVG identifiers.** PDF destination names contain ":" (`ew:r:…`), which is invalid in XML IDs; SVG
    and DXF writers use the "-" form (§4.4).

---

## 8. The tests

The tests open the sample project (`fixtures/sample/`), and each report
kind is specified in §2.4 and §4.

