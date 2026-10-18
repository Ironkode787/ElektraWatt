# ADR-0017: Read-only SQL projection with versioned `api_v1` views

**Status:** Accepted · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the judge panel)

## Context

- **Demand for ad-hoc queries.** EPLAN's report types are predefined
  ([eplan-features](../research/eplan-features.md) §7). The pilot user, domain experts and AI agents want ad-hoc
  questions, custom reports and company rules without writing Rust. The reference project supplies the canonical
  example: "24 V loads with their protective device per MICO channel", because one 6 A channel of -102F2 feeds all
  four CPX supplies of -201U1 and -202U1 (D-25). A total current per channel needs a per-pin load current, which no
  part field or view holds yet (pending amendment 3).
- **Proposal C's SQL surface.** It was C's strongest feature: SQL reports, SQL rules, machine profiles over views,
  and acceptance tests as `.sql → .csv` pairs that a non-programmer can read.
- **But not SQLite as the live model.** ADR-0001 rejected that option: three representations of the truth, an
  asynchronous writer actor, and non-deferrable `UNIQUE`/`CHECK` constraints that cannot stage QET projects
  containing D-11 or D-12.
- **Consumers:**
  - user reports (ADR-0018);
  - offline and release rules (ADR-0019);
  - `ew sql`;
  - the MCP `sql_query` tool (ADR-0020);
  - golden acceptance tests;
  - machine-export profiles;
  - Excel round trips.
- **Threats.** SQL text can arrive inside a received project. Hostile SQL can try to escape (`ATTACH`,
  `VACUUM INTO`, recursive-CTE bombs).
- **Size and cost.** An XL projection is about 2M rows. SQLite is already a dependency for catalogues (`ew-library`).

## Decision

1. **A derived artefact.** `ew-sqlview` materialises the model, plus selected query results, into SQLite (rusqlite
   0.40.2, `bundled`) on a fork:
   - in memory up to size class L;
   - file-backed in the local cache directory at XL, capped at 500 MB (NFR-PERF-13).

   Only `ew-sqlview` and `ew-library` link rusqlite (ARCH §3 rule 2).
2. **Public contract.** Internal `t_*` tables sit behind versioned public views. The base set (FR-EXT-007) is:
   - MVP: `api_v1_device`, `_function`, `_page`, `_wire`, `_terminal`, `_cable`, `_conductor`, `_potential`,
     `_bom_component`, `_bom_assembly`, `_message`;
   - 1.0: `api_v1_plc_channel`, `api_v1_placement`.

   [17](../design/17-extensibility.md) §6.1 proposes further views: `_meta`, `_cp`, `_structure`, `_xref`,
   `_revision` and the fluid views.

   **Column conventions:**
   - `id` is the entity UUID as text, and references are `<entity>_id`;
   - `tag` is the displayed form (`-101F1`) and `tag_full` the full designation (`=P01+J-E-2416-101F1`);
   - multilingual text comes as `<name>` plus `<name>_ml`, read with `ew_ml(text, lang)`;
   - quantities are exact decimal text (`'0.75'`) and lengths are integer `_nm`;
   - derived values carry `prov_<field>`, for example "potential 24VDC-1 via profile DBU/WH 24 V".
3. **Freshness.**
   - The GUI's projection refreshes per table, keyed by the `changed_at` revision of each source table or query. It
     is Class II (ADR-0003) and shows "as of revision n".
   - `ew sql`, exports and release checks build the projection on their own fork at the opened generation, so their
     results are fresh by construction.
4. **Sandbox.**
   - **Connection.** A separate read-only connection per consumer (`SQLITE_OPEN_READONLY`), with `query_only`,
     `SQLITE_DBCONFIG_DEFENSIVE`, `trusted_schema = OFF` and extension loading off.
   - **Statement gate.** Exactly one statement per call, starting with `SELECT`, `WITH` or `VALUES`, for which
     `sqlite3_stmt_readonly()` is true. This refuses `VACUUM INTO` and `WITH … INSERT` before they run, whatever
     authorizer codes SQLite raises for them.
   - **Authorizer.** It allows `SELECT` and reads of `t_*` columns only through an `api_v1_*` view (the authorizer's
     view argument). It denies `ATTACH`, every `PRAGMA`, DDL, DML, transactions and direct reads of `t_*`.
   - **Function allowlist.** `random()` is denied and `'now'` is rejected, so results are deterministic. The host
     registers `ew_natural` (X2 < X10), `ew_ml` and `ew_regexp`.
   - **Deadlines** through the progress handler: report 2 s, rule 10 s, MCP 5 s, `ew sql` 60 s.
   - **Limits:** `SQLITE_LIMIT_*` limits (for example `ATTACHED` 0, `EXPR_DEPTH` 200, `VDBE_OP` 250,000) and result
     caps (report 100,000 rows; MCP 1,000 rows and 1 MiB), as tabulated in [17](../design/17-extensibility.md) §6.2.
5. **Trust.** SQL that arrives inside a received project (reports, rule packs, profiles) runs only after the
   per-project trust prompt (NFR-SEC-05). Built-in packs are trusted. SQL the user types, or sends through their own
   agent, is the user's own.
6. **Lifetime.**
   - `api_v1` is the contract of format major 1, and stored queries declare `api = "v1"`.
   - Within v1, columns are never removed, renamed or retyped. New views and columns are additive.
   - `api_v2` appears only with a format major (ADR-0011), and v1 keeps working for at least one more major.
7. **Never a write path.** The projection is never written by users and never read back into the model. An Excel
   round trip exports from the projection and imports changes as commands with a diff preview (ADR-0008).
8. **CI.**
   - Every `.sql` file in report, rule and acceptance packs is prepared against the current schema.
   - An `EXPLAIN QUERY PLAN` lint flags full scans of L-sized tables.
   - The sandbox has a hostile-SQL fuzz target.

```sql
-- user report "24 V loads per protective channel" (D-25), api = "v1"; uses the proposed api_v1_cp view
SELECT p.name AS potential, p.protected_by_tag AS breaker, p.protected_by_cp AS channel,
       COUNT(c.id) AS loads
FROM api_v1_potential p JOIN api_v1_cp c ON c.potential_id = p.id AND c.role = 'sink'
WHERE p.ac_dc = 'dc' AND CAST(p.voltage AS REAL) = 24
GROUP BY p.id ORDER BY p.name COLLATE ew_natural;
```

## Consequences

**Positive**
- C's best contributor path survives with a single source of truth. Rules, reports and machine profiles can be
  written by people who do not write Rust.
- Golden acceptance is `acceptance/*.sql → expected/*.csv`, which the pilot user can read and extend.
- `ew sql dump` opens the whole derived state in any SQLite browser, which makes debugging and bug reports easier.
- Report pages, XLSX and machine files share one row source (ADR-0018).

**Negative**
- **Latency.** Results in the GUI lag by up to one projection refresh. A full build is a 2–5 s job at XL (budget
  NFR-PERF-14: S/M ≤ 1 s, L ≤ 4 s, XL ≤ 10 s).
- **SQL report page counts.** The page count of an SQL report block is not a tracked model read, so it cannot feed
  `page_order()` directly (see the amendment below).
- **Contract cost.** `api_v1` is a public contract that constrains schema evolution for years. Column snapshots guard
  it in CI.
- **Attack surface.** It is mitigated by the sandbox, the trust prompt and fuzzing. SQLite has no per-connection
  memory cap, so memory is bounded only indirectly, by limits, deadlines and the memory gate.

## Pending amendments

1. **Page counts of SQL report blocks** ([09](../design/09-reports-forms-output.md) OQ6, not yet accepted).
   *Recommendation:* the session holds the last projection result per SQL report block as a non-persisted,
   non-undoable query input stamped with its revision. With no result yet, the block counts as one placeholder
   page. Exports always compute fresh on their fork.
2. **Further views** ([17](../design/17-extensibility.md) §6.1, proposed FR-EXT-032). *Recommendation:* add
   `api_v1_meta`, `_cp`, `_structure`, `_xref`, `_revision` and the fluid views to the v1 contract before it
   freezes at 1.0.
3. **Load current per pin** ([17](../design/17-extensibility.md) OQ20, not yet accepted). *Recommendation:* add a
   load current to the part's electrical data and expose it as `api_v1_cp.i_load` (decimal text in A, empty when
   unknown), so that the D-25 report can add `SUM(CAST(c.i_load AS REAL))` per channel and EW-PLC-14 / FR-CHK-041
   can compare it with the 6 A rating of the MICO channel.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **SQLite as the live model** (proposal C) | Rejected by ADR-0001: three representations of the truth; non-deferrable constraints block lenient QET import |
| **No query language** (fixed report types only) | Only EPLAN parity. No D-25-style reports without a release, acceptance tests only in Rust, and agents limited to fixed tools |
| **A Datalog engine** | Unfamiliar to domain experts, no ecosystem tooling (SQLite browsers, Excel connectors), and rules would be compiled in |
| **User-writable SQL** (writes through views or triggers) | A second mutation path that bypasses commands, validation, undo and the journal (ADR-0008, ADR-0009) |
| **SQLite virtual tables over the live `imbl` maps** (no materialisation) | Every row read would call back into Rust, joins would lose real indexes, and the results would depend on the session's current revision rather than on a fixed fork. A materialised fork is simpler, isolates hostile SQL from the model, and meets NFR-PERF-14 |

## Related

- **ADRs:** [ADR-0001](0001-core-model-persistent-maps.md), [ADR-0003](0003-freshness-contract.md), [ADR-0008](0008-transactions-undo-preview.md), [ADR-0011](0011-format-versioning-and-migration.md), [ADR-0018](0018-reports-forms-engine-and-machine-profiles.md),
  [ADR-0019](0019-rule-engine-and-rule-packs.md), [ADR-0020](0020-extensibility-tiers-and-security-model.md).
- **Design:** [04 architecture](../design/04-architecture.md) §3, §6.8, §10.2, §13;
  [17 extensibility](../design/17-extensibility.md) §6, §11; [09 reports](../design/09-reports-forms-output.md)
  §3.5; [20 golden project](../design/20-golden-project.md).
- **Requirements:** FR-EXT-006, FR-EXT-007, FR-EXT-014, FR-EXT-016, FR-REP-031, FR-CHK-007, FR-CHK-041, FR-MFG-018,
  FR-I18N-010; proposed FR-EXT-032 ([17](../design/17-extensibility.md) §6.1);
  NFR-PERF-13, NFR-PERF-14, NFR-SEC-03, NFR-SEC-05, NFR-SEC-06, NFR-MAINT-01, NFR-REL-07.
