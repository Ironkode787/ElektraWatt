<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# The hostile corpus

Files made to attack a parser of external bytes (WP-S6.04 D10; doc 18 §3.12; NFR-SEC-02, NFR-SEC-06). They are the
project's own work, CC0-1.0 (the REUSE annotations hold the licence, since a data file carries no header). Each file
stands in the folder of its class and is named after what it tries; this index names its parser and the verdict
expected. A test asserts each verdict whole; the files of `sql/` are also the seed corpus of the fuzz target
`sql_sandbox` (`fuzz/corpus/sql_sandbox/`, the same bytes).

Every class but `sql/` runs in the test binary `hostile_corpus` of `ew-testkit` (`tests/hostile_corpus/`): each file
through its parser's public entry point, the importers through WP-S3.09's front door (`ew_api::import::front_door`),
its verdict asserted whole and its extra peak allocation at most 256 MiB, counted by that binary's own global allocator
(NFR-SEC-02; no time is asserted). The binary files are made by its generator (`tests/hostile_corpus/generate.rs`):
those marked *generated* below are made at each run and never committed (they are large on disk); the other binary
files are committed small, and the test compares them with the generator's bytes (`EW_BLESS=1` writes them). A project
copy whose page file is a symbolic link (NFR-SEC-16) is made by the test itself, on Unix only: it opens read-only with
the page missing, `EW-CORE-003 …: listed in the manifest but is not a regular file; links are not followed`.

## `sql/`: hostile SQL text (FT-223)

Parser: the SQL sandbox, `ew_sqlview::Reader::query`, under `ew sql`'s profile with a 200 ms deadline, on the small
image of the sandbox tests (`t_item` behind `api_v1_item`, `t_other` behind `api_v1_other`). The test is
`the_hostile_sql_corpus_is_refused_file_by_file` in `crates/ew-sqlview/src/sandbox_tests.rs`.

| File | Tries | Verdict |
|---|---|---|
| `attach.sql` | to attach another database | Refused by the statement gate: `ATTACH` is not a query |
| `clock-in-a-blob.sql` | to read the clock through `date()` given `now` as a BLOB | Refused by the clock-free date function |
| `cte-named-after-a-view.sql` | to read an internal table through a CTE named after a view | Denied by the authorizer: a read of `t_other` |
| `deep-expression.sql` | an expression nested 300 deep | Stopped by `SQLITE_LIMIT_EXPR_DEPTH` (200) |
| `load-extension.sql` | to load a native extension | Denied by the authorizer: the function `load_extension` |
| `pragma-read.sql` | to read the schema through a table-valued `PRAGMA` | Denied by the authorizer: a read of `pragma_table_info` |
| `recursive-cte-count.sql` | an endless recursive CTE | Stopped by the deadline |
| `recursive-cte-doubling.sql` | a recursive CTE that doubles a string | Stopped by `SQLITE_LIMIT_LENGTH` (16 MiB) |
| `two-statements.sql` | two statements in one call | Refused by the statement gate |
| `vacuum-into-behind-a-query.sql` | `VACUUM INTO` after a query, to write a file | Refused by the statement gate; no file is written |
| `write-behind-a-cte.sql` | a `DELETE` behind a `WITH` | Denied by the authorizer: a `DELETE` |

## `qet/`: QElectroTech projects

Parser: the QElectroTech project importer through the front door into a new project, `ew_api::import::import_qet`
(`ew import qet`, the window's Import). The verdict is the refusal's text, or the import's report.

| File | Tries | Verdict |
|---|---|---|
| `attributes-over-the-limit.qet` | one element with 8,193 attributes (*generated*) | Refused: the XML attributes per element limit of 8,192 |
| `billion-laughs.qet` | nine levels of entities, each ten of the one before (10^9 bytes expanded) | Refused: a DOCTYPE is not accepted |
| `depth-over-the-limit.qet` | elements nested 257 deep | Refused: the XML depth limit of 256 |
| `external-entity.qet` | an external entity naming `/etc/passwd` | Refused: a DOCTYPE is not accepted |
| `infinite-coordinate.qet` | an element at `x="Infinity"` | Imported without the element: EW-IMP-002 "element skipped: bad x/y" |
| `nan-coordinate.qet` | an element at `x="NaN"` | Imported without the element: EW-IMP-002 "element skipped: bad x/y" |

## `elmt/`: QElectroTech element definitions

Parser: the QElectroTech front door as the window's Import and `ew import qet` take a file,
`ew_api::import::import_qet`. The standalone element importer has no caller outside the project importer at 0.1.0 and
`ew-import-qet` is confined to `ew-api` (ADR-0024 decision 4); the fuzz target `qet_element` reads these bytes through
it under `Limits::element_definition()`.

| File | Tries | Verdict |
|---|---|---|
| `billion-laughs.elmt` | the billion laughs in an element's name | Refused: a DOCTYPE is not accepted |
| `external-entity.elmt` | an external entity naming `/etc/passwd` | Refused: a DOCTYPE is not accepted |

## `eplan/`: EPLAN symbol inputs

Parser: the symbol editor's EPLAN import through the front door, `ew_api::symbols::import_eplan` (`ew symbol import`,
the editor's Open), under `Limits::element_definition()` (M25-p2#3). The verdict is the refusal's words, as `ew`
prints them.

| File | Tries | Verdict |
|---|---|---|
| `billion-laughs.xml` | the billion laughs in a part's article number | Refused: the XML is malformed or has forbidden declarations |
| `depth-10000.xml` | elements nested 10,000 deep, which overflowed the stack (SM25#1) (*generated*) | Refused before parsing: the input exceeds the supported size or complexity |
| `element-of-200000-attributes.xml` | one element of 200,000 attributes, the last repeating the first: the parser's duplicate check is quadratic in them (M25-p4#3) (*generated*) | Refused before parsing: over the 256 attributes of one element |
| `entry-parent.edz` | a parts document named `../x.xml` | Refused: unsafe or duplicate member names |
| `external-entity.xml` | an external entity naming `/etc/passwd` | Refused: the XML is malformed or has forbidden declarations |
| `parts-pins-over-the-budget.xml` | 17 parts of 4,000 connections, each within a symbol's 4,096 (M25-p2#2) (*generated*) | Refused: over the 65,536 graphics and connection points of one read |
| `ratio-over-200.edz` | 1 MiB of spaces that pack to about a thousandth, honestly declared | Refused: over the 200 : 1 ratio |
| `seven-zip-dictionary-4-gib.edz` | an LZMA2 folder that declares a 4 GiB dictionary | Refused by the 7z header walk before decoding |
| `seven-zip-entries-over-the-limit.edz` | a 7z header that declares 5,000 entries | Refused by the 7z header walk: over 4,096 entries |

The same files are read a second time by the EDZ and parts XML part reader of WP-S3.18 (`ew_api::eplan::parts_import`,
the test `eplan_part_inputs`): the verdict is the kind of refusal, `limit` for a limit of the XML or of the archive and
`malformed` for a DOCTYPE, an unsafe member name or a file that is no part data; `parts-pins-over-the-budget.xml` is
read (its 17 parts cut to the pin budget of 4,096 a part and 262,144 a file), as the other class's test allows.

## `dxf/`: DXF drawings

Parser: the DXF importer through the front door into a new page, `Session::dxf_page_batch` (`ew import dxf`, the
window's Import), in millimetres where the drawing names no unit.

| File | Tries | Verdict |
|---|---|---|
| `code-not-a-number.dxf` | a group code that is a word | Refused: a group code is not a number |
| `entities-over-the-limit.dxf` | 5,000,001 entities of one point each (*generated*) | Refused before parsing: the DXF entity count limit of 5,000,000 |
| `insert-cycle.dxf` | a block that inserts itself | Imported without the insert: EW-IMP-001, nested deeper than 32 levels |
| `line-over-1-mib.dxf` | a text value one byte over 1 MiB (*generated*) | Refused: the DXF line length limit of 1 MiB |
| `minsert-bomb.dxf` | an array insert of 30,000 × 30,000 | Refused: the block expansion limit of 1,000,000 shapes |
| `nan-coordinate.dxf` | a line from `x = NaN` to `y = inf` | Imported without the line: EW-IMP-002 |
| `truncated-binary.dxf` | a binary DXF that ends inside a real number (*generated*) | Refused: the binary file ends inside a value |

## `xlsx/`: workbooks

Parser: the parts import's XLSX reader through the front door, `ew_api::catalog::parse_parts` (`ew import parts`, the
window's parts import); the round trip's read takes the same reader.

| File | Tries | Verdict |
|---|---|---|
| `billion-laughs.xlsx` | the billion laughs in the package relationships | Refused: a part declares a DOCTYPE |
| `entry-absolute.xlsx` | an entry named `/x` | Refused: the entry name is an absolute path |
| `entry-drive.xlsx` | an entry named `C:x` | Refused: the entry name contains `:` (a drive path) |
| `entry-parent.xlsx` | an entry named `../../x` | Refused: the entry name leads out of the archive |
| `external-entity.xlsx` | an external entity in the package relationships | Refused: a part declares a DOCTYPE |
| `ratio-over-200.xlsx` | an entry honestly declared at 1 MiB, packed about 1,000 : 1 | Refused: the archive entry expansion ratio limit of 200 |
| `zip-bomb.xlsx` | an entry declared at 4,096 bytes whose deflate stream unpacks to 10 MiB | Refused when the inflation passes the declared size |

## `ewz/`: transport archives

Parser: the `.ewz` reader as every reader opens one, `ew_api::Session::open_ewz`.

| File | Tries | Verdict |
|---|---|---|
| `65536-entries.ewz` | 65,536 entries, one more than the 16-bit counts hold (*generated*) | Refused: bytes between the central directory and the end record. Not by `MAX_ENTRIES` (65,535, `crates/ew-store/src/ewz.rs`): without ZIP64 the end record's 16-bit count cannot pass 65,535, so the entry limit equals the format's cap and no file can be refused by the limit alone; the 65,536th entry shows as bytes past the counted directory, and this file stands for the limit (FT-1377) |
| `parent-entry.ewz` | an entry named `../x` | Refused: a `..` segment |

## `jsonl/`: project files

Parser: the project loader, on Golden A's files with its first page file replaced, opened from memory as `ew diff`'s
revision side opens one (`ew_api::Session::open_in_memory`).

| File | Tries | Verdict |
|---|---|---|
| `line-over-16-mib.jsonl` | a line one byte over the 16 MiB line limit (*generated*) | EW-CORE-019 on the line: the file is not read |
| `nested-past-depth-64.jsonl` | arrays nested 70 deep | EW-CORE-019 on the line: nesting deeper than 64 |

## `svg/`: SVG assets

Parser: the asset gate before any SVG parser, `ew_api::asset_ref_of` (`sniff` and `svg_hazard`), as the window and
`item.place` take a picked or dropped file.

| File | Tries | Verdict |
|---|---|---|
| `external-href.svg` | an image linking `/etc/passwd` | Refused: it references something outside the document |
| `internal-subset.svg` | an internal DTD subset with an entity | Refused: it declares an entity |
| `script.svg` | a `<script>` | Refused: it has a script |

## `raster/`: PNG and JPEG assets

Parser: the asset gate (`ew_api::asset_ref_of`), then the renderers' decoder (`ew_render_cpu::decode_image`), which
every renderer and writer of an image goes through.

| File | Tries | Verdict |
|---|---|---|
| `jpeg-without-end-marker.jpg` | a JPEG cut before its end marker | Taken as an asset; not decoded |
| `png-2-31-pixels.png` | a header declaring 65,536 × 32,768 pixels (2^31) | Taken as an asset; not decoded (over 16 Mi pixels) |
