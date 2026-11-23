# Interoperability: import, export and migration: tables, exports, legal limits, hostile input and corpora

A chapter of [Interoperability: import, export and migration](14-interop.md), whose table says where each section lives.

---

## 8. Tables: XLSX and CSV with stable IDs

### 8.1 Export

Every report row source and navigator table exports as XLSX and CSV with the rows and order of the report
(FR-EXP-002, FR-REP-030; [09](09-reports-forms-output.md) §4.6). For the round trip (FR-EXP-004) an XLSX carries:
- a hidden `id` column: the record ID of the row's owner (terminal device, channel, connection, device), or the
  documented composite for aggregated rows (a BOM row keys on `(part, mode)` and is read-only);
- a hidden sheet `_ew`: format `ew-table/1`, project ID, table key, export revision, locale, and the column schema
  `{key, editable, type, language}`;
- a hidden sheet `_ew_base`: the editable cells as exported, with a SHA-256 over the sheet, so an edited base is
  detected and the import falls back to a two-way diff.

Tags and locations are string cells, so Excel never evaluates "-101F1" or "+J-E-2416". The CSV dialects and the
formula-injection guard of the Excel presets are [09](09-reports-forms-output.md) §4.6 (FR-EXP-003).

*As built.* The workbook comes from `ew-reports`' own writer over `ew-zip` (inline strings), its `id` column always
written; `_ew_base`'s SHA-256 is made in `ew-api` (`with_base_digest`), since `ew-reports` takes no hash. `ew export
table <navigator>` writes a navigator's table unfiltered (`--strip` for one strip, the key `nav:strips/<uuid>`); its
CSV defaults to the `ew` preset, which has no `id` column, so a round-trip CSV takes `excel-nl`, `excel-en` or
`rfc4180`; every preset is UTF-8. The window's Export table… writes the shown rows and columns of a
navigator in table mode as a job, the `id` column and the `_ew` sheets always kept, so the file re-imports.

### 8.2 Re-import

1. **Read** with calamine (formulas never evaluated; cached values only) or csv; locate `_ew`. A CSV, or an XLSX
   without `_ew`, needs a visible `id` column and is diffed two-way against the current model.
2. **Match** rows by `id`. Row order, sorting, filtering and hidden rows are irrelevant. Unknown IDs: a new row
   where the table supports creation (terminals with a strip column, channels with a module column), else
   EW-IMP-303. Rows missing from the file are listed, never deleted.
3. **Parse** each editable cell by its column type: `Ident` (NFC, confusable check), `MlText` per language column
   ("Function text [nl-NL]"), `Dec3` with the locale recorded in `_ew` ("0,75"), enums by code or localised label.
   Excel's conversions are detected: a number in a string column is converted canonically and warned when lossy
   ("1.10" → "1.1", EW-IMP-305); a date where a designation was expected is refused. A CSV written with an `excel-*`
   preset carries the formula-injection guard prefix on cells such as "-101F1" and "+J-E-2416"; the importer strips
   exactly that prefix again and nothing else ([09](09-reports-forms-output.md) OQ 8). An `rfc4180` file is read
   unaltered.
4. **Three-way per cell** against `_ew_base`: changed only in the file → apply; changed only in EW since the export
   → keep, listed (EW-IMP-302); changed in both to different values → conflict, default keep EW (EW-IMP-304).
5. **Derived columns** (tag, xref, potential, effective colour with provenance) are read-only; an edit there is
   reported with a hint ("rename through Renumber"; "set an override in the colour column") (EW-IMP-301).
6. **Apply** as one command `table.apply_changes{table, changes}`: one transaction, one undo step, previewed with a
   per-cell diff grid.

Acceptance (FR-EXP-004): editing five function texts of -X1031 in the exported table and re-importing changes
exactly five fields in one undo step; editing a row whose terminal was deleted in EW meanwhile is reported.

*As built.*
Step 1 reads an XLSX with WP-S3.08's own bounded reader (`ew_import_catalog::sheet`, `_ew` and `_ew_base` read whole),
not calamine, and a CSV with its table named (`--table`, or the window's view key) and its preset inferred from the
separator. Rows match by ID, the I/O list's also by station, slot and channel; an unknown ID is reported
(EW-IMP-303), never created. A value the column's parser refuses is a cell outcome, not a finding; EW-IMP-305
applies the value with a warning; in three-way mode a read-only column has no base. Step 6 is one `fields.set_many`
of the applied cells, not `table.apply_changes`: `ew import table` previews, and commits with `--apply`. The window's
Import table… opens the import card in the Preview tab ([16](16-ux-ui.md) §7.7); a conflict keeps EW's value until
the switch "Take the file's values" lands.

### 8.3 BOM for purchasing

FR-EXP-005 is a column-map profile over `bom_component` or `bom_assembly`, like the machine profiles of
[13](13-manufacturing-outputs.md): aggregated by part with quantity and unit, manufacturer, part number, type,
description, ETIM or ECLASS class and DTs (compressed into ranges). The golden component export lists each -X1031
terminal part once with its quantity and 571334 once with quantity 28 (D-04).

### 8.4 CSV bootstrap

`ew import csv <tables dir>` (FR-IMP-022) reads tables of the `ew` dialect (pages, structure, devices, parts,
potentials, connections, terminals, cables, PLC, fluid, placements), with IDs `v5(namespace, "{table}|{row key}")` in
the fixed lineage `Fixed("golden")`, whose namespace is `v5(NS_IMPORT, "csv␟fixed␟golden")` (§2.3); the label
keeps the name of the reference project the bootstrap was built for, because a new label would change every imported
ID. It runs in **strict** mode by default. Its tests run on the hand-written tables of `fixtures/tables/` (the
importer's own and `bin/ew`'s `import csv` test).

*As built.* `TablesImporter` (§2.1) reads the tables whose schemas `crates/ew-import-tables/src/schema.rs` gives:
project, parties and structure, sections, pages and report blocks, parts, devices, terminals, functions,
representations and the connectivity tables. PLC and bus, fluid, layout and the configured products were
not covered at first, because their record kinds are reserved in schema v0, and
`colour_profile`, `size_profile` and `revisions` are read but not emitted (one EW-IMP-004 each). The importer never
repairs a table: a malformed or duplicate row is dropped with EW-IMP-002, a row with an unresolved reference with
EW-IMP-003, and a position or symbol anchor off the page's connection grid is an EW-IMP-002 error that drops the
row, never a snap, because the grid rule binds every origin ([06](06-schematic-editor.md) §5.7); a key that is not in NFC is refused with EW-IMP-002 as well. A function is template-origin when its device carries a part whose template declares a function
with the same FnDef (and role, if given); the part's CP pattern then defines its CP index space. A
manufacturer is referenced by its name in a `LibRef`'s `extra.key` until a manufacturer library copy exists.
Not covered at first: the MICO 4.10 13-14 signal-contact template, FR-DEV-037 "supplied by others", the -201U1:DP
port, and the PLC, fluid, layout and configured-product tables.

*As built.* `ImportContext.mode` (`Strict`, the default, or `Lenient`) reaches the table readers, and `bootstrap_context()` is strict. A colour cell whose two-letter codes are IEC 60757 codes or aliases of §4.12, at least one of them an alias, holds a foreign code. A strict import refuses the row (EW-IMP-002, naming EW-CORE-031). A lenient one keeps `ColourCode::Foreign` and reports EW-IMP-008 with the quick fix "use BN" on conductors and connection overrides: a declarative `fields.set` with the alias mapped, which the session applies like any command (D-17). Cable cores and the colour profile get the finding without a fix. `assembly_configs.csv` emits the assembly aspect (doc 20 §14.2). The QET project import sets `Lenient`. `ew import csv <dir> --out <project>` takes `--strict` (the default: an import with error findings writes nothing and exits 1) and `--lenient` (it writes the project with them). The "GN/YE" alias of §4.12 is not yet accepted by the table reader (FT-259); EW-CORE-031 as a live check, which then owns the fix, is FT-261.

*As built.* A black box without drawn geometry gets its `Representation.box` from the
part's CP pattern: the first ⌊n/2⌋ pattern CPs sit on the top edge leaving up, the rest on the bottom edge leaving
down, at x = 1 + 2i grid steps, and the outline is 2·max(n_top, n_bottom, 5) × 6 grid steps, all on exact page-grid
points. The representation table carries an `arrangement` column for the hidden tags of doc 20 §6.2.

### 8.5 Translation exchange: XLIFF 2.1 and CSV (1.0)

No other document specifies the file side of FR-I18N-006 and FR-I18N-007, so it is fixed here; the language model
itself is [09](09-reports-forms-output.md) §4.7 and [03](03-domain-model.md) §3.

- **Export.** One XLIFF 2.1 file per target language: `<xliff version="2.1" srcLang="en-GB" trgLang="nl-NL">`, one
  `<file id>` per project, one `<unit id="{entity uuid}/{field path}">` per `MlText` field (for example the function
  text of -102G1), `<segment state>` with the XLIFF states `initial`, `translated`, `reviewed`, `final`, and a
  read-only `<notes>` context (DT and folio, "-102G1, folio 102"). Identifiers never appear (FR-I18N-002); neutral texts
  are exported only when the user asks to translate them.
- **Stale targets.** The export records a hash of each source string. On import, a unit whose source changed in EW
  since the export keeps its old target with `subState="ew:source-changed"` and the target is marked "needs review".
- **Import.** Units match by ID; unknown IDs are EW-IMP-303, units missing from the file are left alone. The
  changes apply as one previewed transaction (one undo step), as for `table.apply_changes` (§8.2).
- **CSV variant** (FR-I18N-007): columns `id`, `field`, `source [en-GB]`, `target [nl-NL]` …, read through the
  dialects and the guard-prefix rule of §8.2, with the same matching and result as XLIFF.
- **Hostile input.** XLIFF is parsed with quick-xml under the XML limits of §11 (DTD refused), with its own fuzz
  target `xliff`.

Acceptance is FR-I18N-006's: 30 Dutch targets round-tripped through a CAT tool set 30 nl-NL strings in one undo
step.

---

## 9. Other exports

| Export | Rule | Release |
|---|---|---|
| QET `.qet` (FR-EXP-010) | `version="0.100.0"`; UUID conductor ends; EW symbols converted to embedded definitions; xref formula `%F-%l%c`; EW-only fields (process tag, channel, potential names) as folio `<properties>`; every lossy item listed in an export report | 1.x Could |
| EPLAN-importable XML (FR-EXP-011) | not designed in v1 | Later |
| Personal-data stripping (FR-EXP-009) | §2.2 | 1.0 |
| Determinism (FR-EXP-006) | §2.2; CI compares SHA-256 of CSV, XLSX, AML, PLCopen, glTF and `.ewpp` of the golden project on Windows and Linux | MVP |

---

## 10. Legal constraints

Not legal advice; the rows marked *verify* need a review before the release that ships the feature.

| Subject | Constraint | EW handling |
|---|---|---|
| EPLAN Data Portal | Terms reportedly restrict downloaded data to EPLAN products (*verify* the primary text); downloads need an EPLAN service agreement | User-initiated, local EDZ import of factual part data; notice once per session; no bundling, mirroring, scraping or download; provenance per part; prefer manufacturers' own download sites (NFR-LIC-06, K-5) |
| Manufacturer EDZ, STEP, DXF, pictures | Per vendor; usually use but no redistribution | User or company library only; `embeddable` flag per part; EW-LIB-011 blocks packs without an explicit licence |
| EPLAN `.epj`/`.ema` | Undocumented; EU Directive 2009/24/EC permits observation for interoperability (*verify*) | Later, experimental, built from observed outputs only; no decompilation of EPLAN binaries |
| EPLAN `.elk`/`.edb`/`.zw1` | Closed | Never |
| QET source code | GPL-2.0-or-later | Ported only into `ew-import-qet`, with notices and `NOTICE` entry; no plugin exception; plugins never link it (NFR-LIC-05, C-8) |
| QET element collection | Free inside diagrams; CC-BY 3.0 outside; anti-ML clause | Never bundled; elements embedded in a user's project go to its "QET import" library; on-demand conversion only into a user library (07); never training data; geometry-fingerprint check EW-LIB-006 |
| QET example projects | GPL; embed collection elements | Test fixtures fetched at a pinned commit in CI, never committed or shipped (§12) |
| IEC 60617 database | No extraction for sharing | Own drawings; optional S-number reference |
| ECLASS | Licensed | IRDIs and values from user files only; no dictionary content |
| ETIM | ODC-By | English model bundled with attribution ([07](07-symbols-parts-macros.md)) |
| BMEcat, AML AR APC, DXF reference | Public specifications (*verify* BMEcat and AR APC download terms) | Implemented from the specifications |
| DWG | No official specification; ODA converter licence binds its user | acadrust (MPL-2.0, file-level copyleft, compatible with GPL-3.0); ODA converter user-installed, never bundled; "DWG" used only as a format name |
| SHX fonts, Tahoma | Proprietary | Never bundled; substitution tables |
| GSD, GSDML, ESI, EDS, IODD files | Vendor files; some specifications members-only | Parsed from user files; never redistributed ([10](10-plc-and-bus.md)) |
| Catalogue databases | EU database right (96/9/EC) on substantial extraction | No bulk import into community packs without a licence |

---

## 11. Hostile input: limits and fuzzing

Every parser of external bytes enforces NFR-SEC-02 and FR-IMP-005 and has a cargo-fuzz target (NFR-SEC-03). A limit
violation yields EW-IMP-001 or EW-IMP-002 for the item, or refuses the file; it never crashes the session
(NFR-REL-12). A hostile input is rejected within 1 s and 256 MB of extra allocation, measured by the counting
allocator of [18](18-quality-security-release.md) §3.12. The per-parser values:

| Parser | Limits beyond NFR-SEC-02 |
|---|---|
| XML (QET, parts XML, AML, GSDML, IODD, BMEcat) | DTD refused (no internal or external entities, no XInclude); depth ≤ 256; attribute value ≤ 16 MiB; base64 payload ≤ 64 MiB decoded; decoded images ≤ 16,384 × 16,384 px; ≤ 8,192 attributes per element (`max_xml_attributes_per_element`) and ≤ 1,048,576 per document (`max_xml_attributes`); attribute work ≤ 2^26, the sum over elements of the squared attribute count (`max_xml_attribute_work`); ≤ 1,000,000 nodes per document (`max_xml_nodes`), counting nodes (root, elements and comments in the pre-scan; texts and processing instructions by the parser), not attributes; a project importer may raise the node limit explicitly under its own budget; at most 2 × `max_xml_nodes` + `max_xml_attributes` markup bytes (`<` and `=`, 3,048,576 by default), because roxmltree 0.21.1 reserves about 72 bytes per such byte before any node limit applies; each text node or CDATA section ≤ 1 MiB (`max_text_bytes`), because text is copied several times downstream and no other bound covers it; a standalone element definition is read under the preset `Limits::element_definition()`, an input of ≤ 16 MiB with ≤ 131,072 attributes and ≤ 262,144 nodes (review N7: under the general limits a 16 MiB file of about 349,000 three-attribute `<text>` primitives allocated about 0.5 GB extra; under the preset about 44,000 fit, well under 100 MiB) |
| Archives (EDZ 7z/ZIP, QETZ, `.ewz`, XLSX) | ≤ 100,000 entries; ≤ 2 GiB expanded; ≤ 512 MiB per entry; ratio ≤ 200 : 1 per entry; names with `..`, absolute or drive paths, `\`, `:` or NUL rejected; symlink and encrypted entries refused; 7z solid block ≤ 512 MiB |
| DXF | file ≤ 512 MiB; line ≤ 1 MiB; **one memory budget of 192 MiB** (`MAX_BUDGET`, an estimate in bytes, not a measurement) for the whole conversion: the file's pairs and entities (64 bytes a pair and its string, 128 an entity), counted before any is held, then every entity and block expansion visited (its key and the pairs it is read from, so an empty cell of an array and a block that inserts itself twice are charged), every shape made, every point a curve flattens to and every note kept for the report, each refused as `LimitExceeded` before the memory is taken (a legal drawing of about 150,000 plain entities fits; the shapes are freed as the page items are made, so the peak stays near the budget); block nesting ≤ 32; INSERT/MINSERT expansion ≤ 1 million primitives, an array insert checked before it expands; spline control points ≤ 100,000; a hatch's loops, edges and vertices end with its pairs (a count the pairs cannot hold is cut to the pairs left); a handle (code 5) longer than 32 characters names nothing (the entity is keyed by its place), so a key cannot grow; coordinates finite (an item with another is EW-IMP-002) and within ±2^62 nm after scaling |
| CSV, XLSX | ≤ 1,000,000 rows; ≤ 1,024 columns; cell ≤ 32 KiB (each XLSX shared string too; the parts-table importer reads under the preset `table_limits()`, `max_text_bytes` = 32 KiB); formulas never evaluated |
| SQLite (QETZ) | read-only, `trusted_schema=OFF`, defensive mode, schema check, views and triggers refused, ≤ 1 GiB |
| SVG logos | usvg with external references disabled; ≤ 8 MiB; ≤ 100,000 nodes |
| PDF underlays | ≤ 10,000 pages; rendering as a cancellable job with a per-page deadline |
| External tools (ODA converter) | runs on a copy in a temporary directory; timeout 120 s; output read under the DXF limits |

**Report volume.** One hostile element definition could produce a finding per primitive. Report entries are
therefore capped at 1,000 per element definition (the crate constant `MAX_ENTRIES_PER_DEFINITION`); the rest are
summarised in one EW-IMP-001 entry that gives their count. At most 64 style pairs are parsed per primitive, and at most 8 unsupported pairs per primitive and 1,000 per definition are kept in `extra`; the rest are reported by count. The attribute counts, the attribute work budget, the
node limit, the text-node limit, the 16 MiB element-definition input preset and this cap (`Limits`, §2.1) serve the rule above: a hostile input is rejected within 1 s and 256 MB of
extra allocation. Report locations use a sparse line index (one checkpoint per 64 KiB of input), so memory stays
O(size / 64 KiB) whatever the number of newlines. The attribute work budget and the node default are what make the
1 s / 256 MB rule hold against inputs that stay under the per-element and per-document attribute counts.

**Algorithmic complexity.** Bounding the input is not enough: the work an importer does per item must not grow with
the input either. Every structural walk of an importer (parent and inheritance walks, junction cascades, clean-ups,
reference resolution) is linear or O(n log n) in the number of rows or nodes, never quadratic, so a hostile input
under every limit still meets the 1 s / 256 MB rule.

**Fuzz targets:** `qet_project`, `qet_element`, `qet_titleblock`, `qetz`, `edz_7z`, `edz_zip`, `parts_xml`,
`eplan_table`, `dxf`, `dwg` (1.x), `table_xlsx`, `table_csv`, `xliff`, `svg_logo`, `pdf_underlay`, `bmecat` (1.x), plus the
AML, GSD, GSDML and IODD targets of [10](10-plc-and-bus.md). Each target asserts three properties: no panic; the
batch validates against the `ew-schema` JSON Schemas; `ApplyImport` of the batch on an empty project either commits
in lenient mode or refuses with a report, never violating a structural invariant. A grammar-based generator for the
QET DOM reaches the mapping code beyond the XML parser. Fuzz corpora live in the separate `ew-fuzz-corpus`
repository; every crash becomes a committed regression input ([18](18-quality-security-release.md) §3.12).

*As built.* The importer
targets that exist are `qet_project` (one `.qet` through the front door, then `ApplyImport` on an empty project),
`qet_element` (one `.elmt` under `Limits::element_definition()`, through `QetElementImporter::import`),
`golden_tables` and `parts_table` (a CSV or XLSX supplier table), with WP-S6.04's `table_round_trip` for the table
import's own read; each asserts no panic, a strict re-read of every emitted record and D3's contract. Their seeds are
committed in the fuzz crate (`fuzz/corpus/<target>/`, generated by `seed_corpus` and kept current by settle), not in
a separate corpus repository, and `fuzz/tests/replay.rs` runs them in CI. The other names above come with their
importers. The DXF importer's hostile files (WP-S3.19) are the `dxf/` class of the hostile corpus
(`crates/ew-testkit/hostile/`), each verdict asserted whole within the 256 MB rule. *As built (M2.5, WP-S3.16).* The
cargo-fuzz workspace is cut until 1.0; the AML reader, the TIA tag workbook and the instrument list have
deterministic damage tests in their crates' test binaries instead (truncations and byte damage of a real file: read
or refused in words, never a panic).

---

## 12. Test corpora

| Format | Corpus | Storage | Assertions |
|---|---|---|---|
| QET | 24 examples of the QET mirror (0.80–0.200.1, C-14) | fetched at a pinned commit in CI, never committed | §4.15; zero lost connections; deterministic batches |
| QET | pilot projects including the reference project (PQ-02); three received 2026-10-20 | private store outside the repository, private CI job (20 §3.5); agents read sanitised derived lists only, documents carry counts only | AC-25 … AC-27; 0 lost connections (§4.16) |
| QET | hostile set: NaN coordinates, shared UUIDs, unknown link types, billion-laughs DTD | committed in `tests/hostile/` (own files, CC0; [18](18-quality-security-release.md) §3.12) | EW-IMP-001/002, no crash |
| QETZ | `.qetz` saved by QET 0.200.1-dev and its `.qet` export; a SQLite file with a view | fetched / generated | equal import; view rejected |
| EDZ | own synthetic 7z and ZIP EDZ built from CC0 `part.xml` files; vendor EDZ for golden parts as available | committed / private | FR-IMP-025, AC-38 |
| Parts XML | synthetic 50-part export with variants and accessories | committed | FR-IMP-028 |
| EPLAN tables | terminal-diagram XLSX in German and English layouts (from the pilot or EPLAN users, PQ-04) | private | FR-IMP-029 |
| DXF | R12, 2000 and 2018 samples generated with open tools; EW's own DXF exports | committed (CC0) | FR-IMP-032, AC-37 |
| DWG | DWG files with ODA-converted DXF twins | private until licence-clean samples exist | FR-IMP-036, FR-IMP-037 |
| Tables | golden exports edited by script and by hand in Excel with nl-NL and en-US locales | committed | FR-EXP-003, FR-EXP-004 |
| STEP, AML | as in [12](12-panel-layout.md) and [10](10-plc-and-bus.md) | — | — |

*As built (WP-S3.13).* The 26 `.qet` files of the QET mirror's `examples/` at commit
`aff5edf3905d111289a9a9328d480e39f9591368` (a duplicate under an accented name left out) are fetched by hand into a
scratch folder, never committed (GPL, 18 MB):
`https://raw.githubusercontent.com/qelectrotech/qelectrotech-source-mirror/aff5edf3905d111289a9a9328d480e39f9591368/examples/<name>.qet`.
The deep-run test `qet_examples` (`EW_QET_EXAMPLES=<folder> cargo nextest run -p ew-api --test it -E
'test(/^qet_examples::/)' --run-ignored all`) imports each, expects at most two refusals, no lost connection in
`photovoltaique` and `schema_indus`, and the same batch from a second import of each file. All 26 import, with no lost
connection. The private pilot runs are `qet_private_corpus` and `golden_c` (`EW_PRIVATE_CORPUS`).

*As built.* The committed hostile set is
`crates/ew-testkit/hostile/`, not `tests/hostile/`: one folder per class (`qet`, `elmt`, `xlsx`, `ewz`, `jsonl`,
`svg`, `raster` and FT-223's `sql`), each file named after what it tries, with the index `README.md` naming its parser
and the verdict, CC0-1.0 by the REUSE annotations. The test binary `hostile_corpus` reads each through its parser's
public entry point (the importers through the front door, the `.elmt` files through `import_qet`), asserts the
verdict whole and an extra peak allocation of at most 256 MiB; the large files are generated at each run, the small
binaries committed and compared with the generator's bytes.

---

## 13. UI and UX touchpoints

Personas P1 (engineer) and P6 (library maintainer); journey J2 of [vision](00-vision-and-scope.md). Panel layouts
are specified in [16](16-ux-ui.md).

| Touchpoint | Behaviour |
|---|---|
| **Import wizard** | 1 Source: detected format, version and lineage ("re-import of …, 3 folios changed"). 2 Target: new project from a basic project, the open project, or a library. 3 Options: colour and size profile, identifier set, xref adoption, text patterns, font and symbol maps. 4 Preview: rendered pages, entity diff, report diffs. 5 Review queue grouped by proposal kind with confidence, bulk accept. 6 Commit, with the checkpoint confirmation for an empty project |
| **Migration report** | Check panel filtered to a lineage: code or rule ID, MF/D class, severity, folio and cell, quick fix, status; jump to location; export CSV, JSON or Markdown; F1 opens the code's documentation page |
| **Column mapper** | First 20 rows, header-row and dialect detection, drag-to-map, unit and locale per column, saved mappings |
| **EDZ notice** | Modal once per session, logged in the report |
| **DXF import** | Units with an extents preview, layer list with visibility and mapping, model or paper space, target choice |
| **DXF→symbol wizard** | Block list with thumbnails, attribute mapping, CP pick tool, grid-snap result with the scale proposal |
| **Table round trip** | "Export table…" and "Import changes…" with a per-cell diff grid and conflict resolution |
| **CLI** (FR-EXT-008) | `ew import qet <file> [--into <project>] [--basic <project>] [--lineage auto\|new\|<id>] [--proposals pending\|safe\|all] [--report json\|csv\|md] [--report-out <file>] [--strict]` (MVP; as built: `ew import qet <file.qet> [--content <dir>] [--report md\|csv\|json] [--report-out <file>] [--strict]`); `ew import qet --batch <dir> --out <dir>` (FR-IMP-023, 1.0); `ew import csv <dir>` (MVP); `ew import edz <file> --library <lib>` (1.0; as built also `--project <dir> [--apply]`, `--bump`, `--take-over`, `--as-of`, `--fndef-map`, and `ew import parts-xml` likewise, `ew import eplan-report <file> --map <map.toml>|--preset <name> --out <dir>`, `ew lib import qet-elements <library> <dir>`, WP-S3.18); `ew import dxf <file> --as page\|underlay\|symbol [--units mm]` (1.0; as built also `--as outline`, `--layout`, `--show-layer`, `--on <folio>`, `--embed`, `--library`, `--part`, with `ew import dwg <file> --converter <program>` and `ew import pdf <file> --on <folio> [--page n] [--embed]`, WP-S3.19); `ew import table <xlsx> --project <p>` (1.0, FR-EXT-028 of [17](17-extensibility.md) §3.3); `ew verify-migration <project> --reference <pdf>` (MVP). Exit codes follow FR-EXT-002 as [17](17-extensibility.md) §3.2 applies them to imports: a lenient import that completes exits 0 and prints its summary, whatever its findings, because the findings are the migration report's content; `--strict` exits 1 when a connection was lost or an error finding exists; 2 for an unreadable file or a usage error. With `--report` on stdout (no `--report-out`), stdout is exactly the report and the summary goes to stderr |
| **Concept glossary** | The wizard and the migration report link the QET ↔ EW glossary (element → symbol and FnDef, master/slave → main/auxiliary function, folio report → interruption point, conductor → connection; NFR-DOC-05) |

The CLI default `--proposals pending` applies no proposal, so headless imports are reproducible and Golden C lists
its findings unresolved; the GUI pre-ticks high-confidence proposals.

---

## 14. Edge cases and pitfalls

The body already covers default formulas (§4.8), shared UUIDs (§4.7), NaN and exponent notation (§4.2) and
`%id/%total` (§4.9). Further cases:

1. **"Fixing" while importing.** The importer reproduces what QET printed, "3-B0" and "6-A1" included, and offers
   corrections as quick fixes. Silent corrections would break the migration verifier and the user's trust.
2. **Rotated free texts** keep their source rotation as an `Angle2` (10⁻³ degree, counter-clockwise positive as
   seen on the sheet, [03](03-domain-model.md) §3.1): QET text `rotation` negated, DXF `TEXT`/`INSERT` angles
   unchanged (§4.13, §6.1); the "readable from bottom or right" rule of
   [06](06-schematic-editor.md) §2.5 applies only to text slots.
3. **Non-UTF-8 XML prologs** in old files: ISO-8859-1 and Windows-1252 are transcoded; other encodings are refused.
4. **A folio report linked to a report on the same folio** is kept and reported as a degenerate pair.
5. **Terminals without a strip header** and without a label prefix form a strip per folio named `X?{page}`,
   reported for review (EW-IMP-005); its key is the folio's (`qet:strip:folio:{folio key}`), never a line number,
   so it survives re-import (§4.11).
6. **Duplicate designations used as function texts** ("1+" ×4): the quick fix renumbers 1–n and moves the old
   designation into the terminal's function text, which is how Golden A prints "1+" ([08](08-connections-terminals-cables.md) §6.8).
7. **Excel damage:** dates made from "1-2", lost leading zeros, merged cells (refused per row), a deleted `id` column
   (the import refuses with a hint).
8. **DXF with `$INSUNITS` 0** is common in EPLAN exports in mm; the prompt pre-selects mm when the extents fit a
   sheet.
9. **Mirrored INSERTs with `MIRRTEXT` 0:** geometry mirrors, texts stay readable.
10. **EDZ pin "-" versus "−":** designations are kept as in the source; confusable detection flags mixed use.
11. **Re-import after renumbering in EW:** IDs do not depend on tags, so renumbered devices match; a QET label
    changed on a device frozen in EW is a conflict, not an overwrite.
12. **Import into a project with another numbering rule:** "keep source tags" (frozen) or "adopt target numbering"
    (normalisers renumber the imported devices in the preview).
13. **Large embedded images** in title blocks are stored once by hash; images above the limit become placeholders.

---

## 15. The reference project

### 15.1 Walk-through

No reference project is walked through here: the tests open the sample project (`fixtures/sample/`) and the QET
corpus. The migration findings MF-01 … MF-27 below name the cases of a QET import that the importer and the
migration report handle.

### 15.2 MF findings and their sources

"Import code" is the `EW-IMP` entry of the import run; "live rule" is the rule ID that
[15](15-checks-revisions-collaboration.md) §10.2 assigns. Where both exist, the report shows one row under the live
rule (§2.5).

| MF | Import code | Live rule | MF | Import code | Live rule |
|---|---|---|---|---|---|
| MF-01 | EW-IMP-101 | — | MF-15 | EW-IMP-111 | — |
| MF-02 | EW-IMP-102 (merge proposal) | EW-RDS-010 | MF-16 | — | EW-POT-001 (bond B1 quick fix) |
| MF-03 | EW-IMP-103 | — | MF-17 | EW-IMP-121 | — |
| MF-04 | EW-IMP-104 | — | MF-18 | EW-IMP-107 | — |
| MF-05 | EW-IMP-105 | EW-CORE-023 (FR-REP-036) | MF-19 | EW-IMP-107 | — |
| MF-06 | EW-IMP-106 | — | MF-20 | EW-IMP-112 | — |
| MF-07 | EW-IMP-107 | — | MF-21 | EW-IMP-113 | — (EW-CORE-049 only for part-list pages kept as graphics) |
| MF-08 | — | EW-TERM-001 | MF-22 | EW-IMP-006 | — (EW-CORE-064 retired) |
| MF-09 | — | EW-TERM-002 | MF-23 | — | EW-POT-013 |
| MF-10 | EW-IMP-109 | — | MF-24 | — | EW-POT-009 (FR-CHK-018) |
| MF-11 | EW-IMP-007 (alias proposal BN) | EW-CORE-031 | MF-25 | — | EW-CORE-061 (FR-CHK-023, 1.8 mm) |
| MF-12 | EW-IMP-007 / -008 (manufacturer master; FR-PRT-004 split of "UK5N / ULSKG5") | — | MF-26 | EW-IMP-114 | EW-CORE-063 |
| MF-13 | — | EW-CORE-053 (FR-CHK-035); quick fix: anchor policy "graphics" → "3-B2" | MF-27 | — | EW-CORE-034 |
| MF-14 | EW-IMP-110 | EW-CORE-046 once the assembly exists (FR-CHK-022) | | | |

### 15.3 Defects detected at import and prevented afterwards

| Defect | At import | Afterwards |
|---|---|---|
| D-01 stale -4W1 | MF-01; derived tag printed | tags are derived, never stored |
| D-02 T51 vs T55 | layout review queue | placements reference devices |
| D-03 mixed schemes, "+JE2416" | MF-02, MF-03 with rule inference | one structure node with alias; frozen tags listed |
| D-04 BOM gaps | auxiliary parts imported; MF-21 | BOM from devices |
| D-05, D-06 title-block drift | MF-04, MF-05 | revision record; bound title block |
| D-08 free-text wire rule | text to data, MF-07 | profile-derived properties |
| D-10 5 vs 4 valves | MF-14 | generated overview |
| D-11, D-12 designations | lenient MF-08, MF-09 | strict prevent rule |
| D-13 N switching | MF-23 (needs pass-through FnDefs) | EW-POT-013 |
| D-14 "3-B0" | reproduced, MF-13 (EW-CORE-053) | anchor policy |
| D-15 "TBD" | MF-18 as typed `Tbd` | release blocker |
| D-17 "BR" | MF-11 | `ColourCode` enum |
| D-18 spelling | MF-12, description lint | manufacturer master |
| D-19 missing cable | MF-27 | EW-CORE-034 |
| D-20 junctions | junction elements → `Junction` (EW-IMP-118) | explicit junction sequence |
| D-22 0 V–PE wire | MF-16 | `Bond` |
| D-24 IP names | MF-17 | canonical potential name |
| D-26 odd bits | MF-19, then EW-PLC-012 | derived addresses |
| D-27 untagged valves | MF-20 | child tags |
| D-29, D-31 text sizes | MF-26, MF-25 | text styles |
| D-30 parties | MF-06 | party roles |
| D-42 strip twice | MF-10 | one strip, several segments |

