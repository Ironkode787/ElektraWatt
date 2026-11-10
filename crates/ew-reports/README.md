<!-- SPDX-FileCopyrightText: 2026 The ElektraWatt contributors -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ew-reports

Report row queries, pagination into generated pages, and (later) XLSX and CSV tables: doc 09 §1.3, §2.3, §3.2–§3.4,
ADR-0018. Work package WP-S3.03.

## Pipeline

- **Kinds** (`kinds/`): each built-in report kind implements `ReportKindDef` (name, columns, `rows(q, block)`), and
  `Registry::builtin()` maps the schema's `ReportKind` to it.
- **`RowSet`** (`rowset.rs`): typed cells with late formatting, a stable `RowId` from source entity IDs, links,
  groups. `RowId` prints as typed, escaped components joined by `/` (`e:<uuid>`, `i:<n>`, `n:<percent-escaped>`),
  an injective form that parses back (`RowId::parse`); it is the hidden `id` column of XLSX (FR-EXP-004).
- **Arrangement** (`arrange.rs`): manual exclusion, natural-order sorting with `RowId` tie-breaks,
  grouping with optional header and footer rows. Groups follow the sort direction of the group column and never
  span two heading segments; group texts print a tag in its displayed form for the block's section structure,
  else its short form.
- **Pagination** (`layout.rs`, `paginate.rs`): the form's data area gives the capacity and the row heights
  (23 rows of 6.52 mm for the house form and for forms without a data area); keep-with-next headers, continuation
  forms, `NewPage` / `NewFolio` group breaks, sub-page (`907.1`) or next-folio numbering, `{n}`, `{count}` and
  `{group}` titles (`{{` and `}}` are literal braces), `OnePage` / `OmitPages` for empty blocks. A data area
  whose row heights are not positive, or that holds less than one data row, is invalid: the house geometry
  applies and a form note (`DeriveNote::MalformedShim`, FR-REP-039) is pushed.
- **TOC page capacity** (decision D3): the TOC form's data area (capacity over the data-row height) when the
  form has a valid one, else the profile's `toc.rows_per_page` (`toc_rows_per_page`). `page_order` settles the
  TOC page count from the closed form `n = max(1, ⌈D / (r − k)⌉)` (D other pages, r rows per TOC page, k TOC
  blocks) and never leaves a page out of the TOC.
- **Queries** (`queries.rs`): `report_rows(block)` (`BlockRows`, Memo) and `report_block_pages(block, toc_pages)`
  (`BlockPages`, Memo). `ReportEngine` implements `ew_derive::ReportPagesProvider`; `ew-api`'s `Session`
  installs it on every `Q`, so `page_order` includes the generated pages and settles the TOC page count.
  `tag_class_members()` (`TagClassMembers`, Memo, `render.rs`) groups the tagged devices per tag class once per
  `Q` for the tag-range compression. A row source that reaches the page order from inside the pagination is a
  query cycle: the block gets one placeholder page (`NotDerived::Cycle`) and a `QueryCycle` note, never a panic.

## Kinds of this build

| Kind | Status | Row source |
|---|---|---|
| `toc` | derived | `toc_rows()` (page order) |
| `parts_summary` | derived | `bom_rows(component)`, or `assembly` mode |
| `parts_list` | derived | `device_part_lines` per device |
| `part_data` | derived (embedded) | part lines of the root devices drawn on the embedded page and of the enclosure device of the surface its surface view shows, carrier lines left out; printed as "PART DATA", label and part number at 3 : 10 (WP-S4.01 D16) |
| `structure_overview` | derived | structure nodes and device tags |
| `terminal_diagram`, `cable_overview_diagrams`, `wire_list`, `potential_overview` | derived | the Derivation II connectivity queries (`potential_partition`, `physical_wires_of`, `wire_props_of`), which read the drawn page order, so these kinds derive inside the pagination too |
| `plc_overview` | derived | the `plc_rows` query (channel records, module aspects, the part's PLC section) |
| `profile_tables` | derived | the colour and size profiles of the project settings (the cover's tables) |
| `numbering_report` | derived | frozen tags (`TagSpec.frozen`) and frozen wire numbers with their rule-derived values (`rule_product`, `wire_numbers`; FR-REP-022) |
| `revision_overview` | derived | `revision_rows` (FT-207): the revision records in label order, a withdrawal record after the revision it withdraws, the changed pages they record; `ew-api` completes the pages from the page digests at draw time (WP-S1.18 part 2) |
| `enclosure_legend` | derived (embedded; drawn as "LEGEND" in two flow columns of position, DT, quantity and part number, WP-S4.01 D17) | `layout_legend(surface)` of the surface the embedded page's `surface_view` shows: the enclosure, the plate, one row per carrier (rails, then ducts; the length in metres as the quantity), then the placements by designation |
| `io_list` | derived | the `plc_rows` query per station (FR-PLC-023, WP-S1.21 part 5); deactivated channels only with the block parameter `include_deactivated` |
| `device_list` | derived | the device rows of `structure_overview` (one shared predicate: every device that is not a terminal), natural order of the full tag; parts from the device's own part lines, the main representation's cell (FR-REP-008, WP-S3.06 D5) |
| `terminal_overview` | derived | one row per strip in `terminal_diagram`'s strip order: the terminal counts by `terminal_diagram`'s one state rule, the terminal parts, the segment cells (FR-REP-010, WP-S3.06 D6) |
| `open_items`, `check_report` | placeholder (`NotDerived::Unsupported`) until their WP | not registered yet |

A kind that is not derived yet has its columns and an empty `RowSet` whose `status` is `NotDerived(reason)`. A
page-oriented block of such a kind paginates to **one placeholder page** whose `note` carries the reason, so its
folio, its title and its TOC entry exist before the rows do (the rule ADR-0017 amendment 1 gives SQL blocks before
their first result). Other schema kinds are not registered and report `NotDerived::Unsupported`.

## Block parameters

A block's `params` member (WP-S3.06 D9) is a top-level JSON object kept in the block's `extra` (no schema member):
snake_case keys, JSON scalar values. A kind declares the keys it reads through `ReportKindDef::params()` (key, type
`Bool` or `Choice`, default; none by default), and reads the block's values through `kind_params(q, block)`, one value
per declared key in declaration order. A value outside its kind (another JSON type, a word the choice does not list,
"Assembly" for "assembly") takes the default, an undeclared key is ignored, and a `params` that is not an object gives
every default; each pushes one `DeriveNote::MalformedShim` that names the block in words (`report block 912
params.mode`). Declared now: `parts_summary` `mode` ("component", the default, or "assembly") and `io_list`
`include_deactivated` (boolean, default false).

## The export by kind

`ew-api`'s `row_export(q, &RowExport::Kind(kind), None)` (`ew export report --kind <kind>`) writes `project_rows(q,
kind, None)`, the kind's whole-project rows in its default order with no block arrangement, in the golden CSV dialect
(WP-S3.06 D4). Golden A's kinds without a folio have their golden rows as `expected/csv/<kind>.csv`: `device_list`,
`terminal_overview` and `io_list`.

## Not yet

XLSX and CSV writers (rust_xlsxwriter, csv), user SQL reports (1.0), the persisted encoding of a block's `filter`
member and the typing of `filter` and `params` (FT-385's `report_block.set`), and opt-in row wrapping from form data (the schema's `WrapPolicy` has no wrapping variant
yet; `RowLines` is the measurement hook).
