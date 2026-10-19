# ADR-0018: Reports and forms engine; report blocks as derived pages; machine-export profiles as data

**Status:** Accepted · amended 2026-10-18, 2026-10-19 · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the judge panel)

## Context

- **What the reference set lacks.** It has a table of contents (folio 2) and a part list (folio 901). It has no
  terminal diagram, cable diagram, wire list, PLC overview, potential overview or wire numbers (D-09). Its part list
  is incomplete:
  - -106W2 and E56/S56 part data are missing;
  - +JE2416-X1031 is listed twice;
  - there is no quantity column;
  - it still shows -4W1/-4X1 after the renumber to -3W1/-3X1 (D-01, D-04).
- **What drifts.** Title-block fields drift from page to page: four revision formats (D-05), two project
  descriptions (D-06), customer versus end user (D-30). The PDF has no bookmarks, links or page labels (D-07).
- **Embedded tables.** Some report-like content sits on drawn pages:
  - the "PART DATA" boxes on folios 201, 202 and 902;
  - the cover's cable-colour and standard-size tables (REF-005), which replace the free-text rule "if not indicated,
    standard size" (D-08).
- **Machine files.** Manufacturing needs wire lists with lengths and ferrules, terminal plans, labels and machine
  files (Komax, Rittal WT, Weidmüller WPC), and these must agree with the printed lists to the wire.
- **The ADR-0001 rule.** Everything printed is a query result. A report stored as pages would recreate the
  stale-copy defect class.

## Decision

1. **Two kinds of row source, one engine.**
   - **Built-in report types** are typed Rust row queries: fast, type-checked and memoised. The catalogue, with
     releases, is [09](../design/09-reports-forms-output.md) §2.4. At MVP:
     - title page, TOC and section divider;
     - parts list in assembly and component modes, summarised parts list, device list;
     - terminal diagram and strip overview;
     - cable diagram and overview, wire list;
     - PLC overview (one row per channel of a station, folio 908) and I/O assignment list;
     - potential, structure-identifier, revision and numbering overviews, open items, check report.

     At 1.0: PLC card list (one row per module), terminal line-up, device connection and plug diagrams, bus
     reports, enclosure legend as pages and
     mounting list, fluid lists (tubes, valve terminals, media), load summary, purchasing views, thermal report,
     verification checklists.
   - **User reports** (1.0) are an SQL query over the `api_v1_*` views (ADR-0017) plus a form. They are stored in
     the project or in the company basic project and run sandboxed, behind the trust prompt for received projects.
2. **Forms are declarative data.** A `FormDef` is held in nm and contains:
   - header, data and footer areas;
   - a row height and continuation forms. The reference's house form has a 6.52 mm header and rows, a header
     fill of `#C0C0C0`, 0.183 mm lines, 8.88 pt text and 23 data rows per page, so pages = ⌈rows / 23⌉ (REF-100;
     [20](../design/20-golden-project.md) §16);
   - static graphics, including drop-shadow boxes;
   - group breaks: one terminal diagram per strip is EPLAN's "function-related" report;
   - cells with **typed placeholders** bound to row fields, project or page properties, party roles or derived
     values (page, Prev, Next, revision, the `=` and `+` aspects).

   The placeholders offered depend on the form type. Title blocks are frame forms that reproduce
   [reference-drawing](../research/reference-drawing.md) §5.1 to ±0.2 mm (REF-096). **Per-page free-text title-block
   fields do not exist.** The page description inherits the project description, and an override is an explicit,
   flagged page property (D-06).
3. **Report blocks are stored; report pages are derived.**
   - The stored record is `ReportBlock { kind | query, filter, sort, form, section, first_folio }`.
   - `report_pages(block)` paginates the rows into virtual pages with IDs `v5(report_block_id, page_index)`
     (ADR-0004).
   - `page_order()` merges those pages with the drawn pages, so the TOC, Prev/Next, the PDF outline and `/PageLabels`
     always include them.
   - There is no "update reports" command. A check reports folio collisions and pages outside the section range
     (FR-CHK-038, FR-REP-041).
   - The golden report folios 2, 901 and 903–911 are blocks (FR-REP-001): TOC (2, 2.1), parts list (901, 901.1),
     terminal diagrams 1X1, -X1031 and -X1032 (903–905), cable overview (906), wire list (907–907.2), PLC overview
     -201U1 (908–908.2), potential overview (909), device-tag overview (910) and revision overview (911). Continuation
     pages take sub-page folios.
4. **Embedded report blocks** are page items bound to a filtered row source: the PART DATA boxes, and the cover's
   colour and size tables generated from the active profile.
5. **One row source, many sinks.** The same rows feed PDF pages, XLSX (rust_xlsxwriter), CSV and the `.ewpp`
   production package. The printed wire list and the machine file therefore cannot disagree.
6. **Machine-export profiles are versioned TOML data:**
   `{row_source (built-in query or api_v1 view), filter, column map, value transforms, limits, encoding, line endings, header}`.
   - **Shipped profiles (1.0):** a generic CSV/XLSX wire list (FR-MFG-019), Komax TopConvert CSV (FR-MFG-020),
     Rittal WT 24/36 and WT C CSV (FR-MFG-021) and Weidmüller WPC (FR-MFG-022, Should). Label-printer presets and
     the TIA tag XLSX also ship as profiles (ARCH §11; [13](../design/13-manufacturing-outputs.md) §5.4,
     [10](../design/10-plc-and-bus.md)); amendment 1 separates the two.
   - **Pre-flight.** Exports run the pre-flight rules EW-MFG-001 … 006 first (FR-MFG-024): length routed or explicitly
     estimated, cross-section and wire type within the machine's range, ferrule in the magazine, marker fits, no
     duplicate wire number, bundle fits the bin. Rejected wires go to `manual.csv` with the reason per row.
   - **Release mode** refuses stale Class II inputs and stale committed computations.
7. **Release freezes outputs.** At release, report outputs are frozen into the as-built archive as committed
   computations (ADR-0003, ADR-0013). A released revision reprints identically.
8. **Licences.** Built-in report packs and machine profiles are MIT OR Apache-2.0. Forms and frames are CC-BY-SA-4.0
   with the design exception (ADR-0024).

```toml
# Rittal WT profile, abridged; the full format is in 13 §7.1. Column names are unverified
# until a real WT template is obtained (PQ-21), so the profile ships with status = "preview".
schema     = "ew.machine-profile/1"
id         = "rittal-wt-csv"
version    = "0.9.0"
status     = "preview"                  # preview | validated
kind       = "wires"
row_source = "builtin:wire_records"     # or "sql:<file>.sql" over api_v1_* (trusted projects only)
filter     = "kind != 'core' && !flags.field && !flags.pre_assembled"
[limits]
csa_mm2             = { min = "0.5", max = "2.5" }
length_increment_mm = 1
wire_types_max      = 36
accept_estimated    = false
[output]
format      = "csv"
encoding    = "cp1252"
delimiter   = ";"
line_ending = "crlf"
header      = true
[mapping]
columns = [
  { header = "Source", value = "from.marker_dt" },
  { header = "Target", value = "to.marker_dt" },
  { header = "Length", value = "length_mm", transform = "round_up(limits.length_increment_mm)" },
]
```

## Amendments

Amendments 1 to 3 were accepted on 2026-10-18, amendment 4 on 2026-10-19. The decision text above is unchanged;
these entries refine it.

1. **Machine profiles, label profiles and the TIA tag XLSX** (2026-10-18;
   [13](../design/13-manufacturing-outputs.md) §5.4, §7; [10](../design/10-plc-and-bus.md) §4.5). Two profile kinds
   share the TOML profile machinery of decision 6: **machine profiles** (`schema = "ew.machine-profile/1"`, for
   wire-processing machines such as Komax and Rittal WT; shipped IDs `generic-csv`, `generic-xlsx`,
   `komax-topconvert-csv` and `rittal-wt-csv`) and **label profiles** (`schema = "ew.label-profile/1"`, for
   marking software and label printers). The TIA "PLC Tags" XLSX is **not** a profile: it is a fixed-format
   exporter and importer in `ew-plc` (FR-PLC-036), because its round trip needs code.
2. **Text measurement through `ew-text`** (2026-10-18; [09](../design/09-reports-forms-output.md), open question 4;
   [architecture](../design/04-architecture.md) §3). Fonts and the memoised `shaped_text` query live in the GPL
   crate `ew-text`, below both `ew-reports` and `ew-scene`, so pagination measures wrapped rows with the same
   shaping as the page renderer. `ew-reports` calls the sandboxed query API of `ew-sqlview` for SQL report blocks
   and never links rusqlite.
3. **Report terms** (2026-10-18; [09](../design/09-reports-forms-output.md) §2.4). The *PLC overview* is the row
   source `plc_overview`, one row per channel of a station (folio 908); the *PLC card list* is `plc_card_list`,
   one row per module (1.0); the *I/O assignment list* is `io_list` (FR-PLC-023).
4. **The TOC page count in closed form** (2026-10-19, core team, after the independent review of the cycle-7 report
   engine; decision log ID-155, ID-158; [09](../design/09-reports-forms-output.md) §3.1). The context's "iterates
   for at most three rounds" is replaced: the pages that do not depend on the TOC count, D, give the count directly
   as n = ⌈D / (per − 1)⌉, where per is the TOC capacity (the TOC form's data area, else the profile's
   `toc.rows_per_page`, at least 2), and one confirming round checks it. If the confirming round needs more pages,
   the order is rebuilt with that count and EW-CORE-025 is raised, so no page is ever missing.

## Consequences

**Positive**
- **No stale reports.** Renumbering -4W1 to -3W1 changes the BOM, TOC and wire list in the same transaction, so
  D-01, D-04 and D-07 are structural.
- **Cheap machine formats.** A new machine format is usually a TOML file reviewed like data, not a Rust release.
  Native formats that vendors have not documented (Komax WPCS, Rittal Perforex, Steinhauer) wait for vendor
  documentation or a plugin (FR-MFG-027, Later).
- **One reference for customers.** XLSX/CSV and PDF come from the same rows.
- **Reproducible releases.** Released revisions are reproducible from the archive, independent of later engine or
  profile changes (ADR-0003 derivation profiles).

**Negative**
- **Text measurement.** Pagination is a derived query that needs text measurement for wrapped rows. Shaping was
  planned in `ew-scene`, which depends on `ew-reports`; amendment 2 moves it into `ew-text` below both. Until
  `ew-text` exists in the spine, `ew-reports` paginates fixed-height rows only.
- **TOC fixpoint.** The TOC's page count depends on itself. It iterates inside one query, for at most three rounds
  (ADR-0002).
- **SQL report page counts** are Class II. ADR-0017 (amendment 1) defines how they enter `page_order()`.
- **Forms editor.** A forms editor with live preview is needed so that companies can reproduce their title blocks.
  Until 1.0 that is a real cost on stream S3.
- **Expressiveness.** Declarative profiles cannot express every vendor quirk. Value transforms may use pure rhai,
  and profile `expr` columns in received projects are trust-gated
  ([17](../design/17-extensibility.md) OQ8).

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **Reports stored as pages** (EPLAN and QET style, refreshed by an "update reports" action) | The stored copy goes stale between updates: exactly D-01, D-04 and D-07. Report pages would also need their own merge and undo semantics |
| **Code-only exporters per machine** | Every new machine or firmware variant needs a Rust change and a release. The column logic would duplicate the report rows, so printed lists and machine files could drift |
| **Fixed report types only** | Only EPLAN parity. No D-25-style user reports, no company nameplate lists |
| **A separate document engine for report pages** (Typst, HTML templates) | It would break the one-display-list rule of ADR-0015: screen, PDF, SVG and print could differ, and title blocks would have two renderers |
| **Per-page title-block text fields** (QET style) | The direct cause of D-05, D-06 and D-30 |

## Related

- **ADRs:** [ADR-0001](0001-core-model-persistent-maps.md), [ADR-0002](0002-derived-data-engine-ew-memo.md), [ADR-0003](0003-freshness-contract.md), [ADR-0004](0004-identity-uuidv7-uuidv5.md), [ADR-0013](0013-revisions-and-as-built-archive.md), [ADR-0015](0015-display-list-renderers-pdf.md),
  [ADR-0017](0017-read-only-sql-projection-api-v1.md), [ADR-0019](0019-rule-engine-and-rule-packs.md),
  [ADR-0024](0024-licence-set-and-permissive-gpl-boundary.md).
- **Design:** [04 architecture](../design/04-architecture.md) §3, §6.4, §11; [09 reports](../design/09-reports-forms-output.md);
  [10 PLC and bus](../design/10-plc-and-bus.md) §4.5; [13 manufacturing](../design/13-manufacturing-outputs.md) §5.4,
  §7; [20 golden project](../design/20-golden-project.md).
- **Requirements:** FR-REP-001, FR-REP-002, FR-REP-003, FR-REP-004, FR-REP-005, FR-REP-009, FR-REP-012, FR-REP-014,
  FR-REP-019, FR-REP-021, FR-REP-028, FR-REP-030, FR-REP-031, FR-REP-032, FR-REP-033, FR-REP-035, FR-REP-036,
  FR-REP-038, FR-REP-041, FR-REP-042; FR-MFG-018, FR-MFG-019, FR-MFG-020, FR-MFG-021, FR-MFG-022, FR-MFG-024, FR-MFG-025, FR-MFG-027,
  FR-MFG-031; FR-PLC-023, FR-PLC-036; FR-CHK-038; NFR-PERF-10, NFR-REL-07.
