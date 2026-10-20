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
- **Arrangement** (`arrange.rs`): manual exclusion, typed filters, natural-order sorting with `RowId` tie-breaks,
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
  (`BlockPages`, MemoLru). `ReportEngine` implements `ew_derive::ReportPagesProvider`; `ew-api`'s `Session`
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
| `part_data` | derived (embedded) | part lines of the root devices drawn on the embedded page |
| `structure_overview` | derived | structure nodes and device tags |
| `terminal_diagram`, `cable_overview_diagrams`, `wire_list`, `potential_overview` | derived | the Derivation II connectivity queries (`potential_partition`, `physical_wires_of`, `wire_props_of`), which read the drawn page order, so these kinds derive inside the pagination too |
| `plc_overview` | not derived yet | the `plc_rows` query (no channel records) |
| `profile_tables`, `revision_overview` | not derived yet | colour and size profiles and revisions have no record shape in this format |
| `device_list`, `terminal_overview`, `io_list`, `open_items`, `numbering_report`, `enclosure_legend`, `check_report` | placeholder (`NotDerived::Unsupported`) until their WP | not registered yet |

A kind that is not derived yet has its columns and an empty `RowSet` whose `status` is `NotDerived(reason)`. A
page-oriented block of such a kind paginates to **one placeholder page** whose `note` carries the reason, so its
folio, its title and its TOC entry exist before the rows do (the rule ADR-0017 amendment 1 gives SQL blocks before
their first result). Other schema kinds are not registered and report `NotDerived::Unsupported`.

## Not yet

XLSX and CSV writers (rust_xlsxwriter, csv), user SQL reports (1.0), the persisted encoding of a block's `filter`
and `params` members, and opt-in row wrapping from form data (the schema's `WrapPolicy` has no wrapping variant
yet; `RowLines` is the measurement hook).
