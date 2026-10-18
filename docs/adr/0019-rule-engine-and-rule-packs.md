# ADR-0019: Rule engine and rule packs

**Status:** Accepted · amended 2026-10-18 · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the judge panel)

## Context

- **What the checks must cover.** The reference drawing set shows why checks matter:
  - designations and numbering: mixed tag schemes and "+JE2416" against "+J-E-2416" (D-03);
  - terminals: empty and duplicate designations (D-11, D-12);
  - potentials:
    - single-pole switching in the N potential at T55 and S56 (D-13);
    - PE continuity that is not drawn (D-21);
    - 0 V merged with PE by a wire (D-22);
    - PE smaller than the phase conductor (D-38);
  - PLC: a "TBD" address (D-15) and odd-bit addresses (D-26);
  - fluid: an incomplete pneumatic model (D-28);
  - layout: an incomplete layout (D-32);
  - release: author = checker (D-34).
- **Online checks.** EPLAN's message management runs checks online and offline, in classes such as 001 terminals,
  003 cables, 005 connections and 017 numbering ([eplan-features](../research/eplan-features.md) §9).
- **Company rules and licensed data.**
  - Companies need their own rules without forking EW.
  - Some thresholds come from standards whose tables may not be reproduced, so EW cites standards by number and
    clause only (NFR-COMP-05). Licensed threshold tables must plug in per organisation.
- **Incremental and inside the budget.** Online checks must be incremental and fit the frame budget (ADR-0022): a
  wire edit on folio 102 must not re-check folio 203.
- **Legacy projects.** The prevent class of invariants (ADR-0009) needs a rule mechanism that is strict while editing
  and lenient for legacy QET projects.

## Decision

1. **Rule shape.** A rule is
   `{id, class, severity, modes, scope, impl, params, message (Fluent key), docs_url}`.
   - IDs are `<OWNER>-<FAMILY>-<NUMBER>`, for example EW-POT-013 or EW-PLC-012. The layout family is `EW-LAY`
     (EW-LAY-001 unplaced device, EW-LAY-006 collision, EW-LAY-011 stale route). The `EW-` prefix is reserved
     for shipped packs. IDs are never reused; a withdrawn rule keeps its ID marked "Retired".
   - Classes mirror EPLAN's message-class topics (001 terminals, 003 cables, 005 connections, 017 numbering;
     [eplan-features](../research/eplan-features.md) §9), not EPLAN's message numbers. EW adds classes for integrity,
     PLC, bus, fluid, layout and routing, thermal, manufacturing, EN 60204-1, import and the release gate
     (amendment 3).
2. **Three implementation kinds:**
   - `rust:<fn>` registered functions, for graph-heavy and online rules;
   - `rhai:<expr>` pure expressions, sandboxed with no I/O and operation limits, for simple company rules (for
     example "letter T requires a temperature-sensing definition");
   - `sql:<file>` over the projection (ADR-0017), offline and release only, for domain experts writing rules
     without Rust.
3. **Modes:**

   | Mode | Engine | Freshness |
   |---|---|---|
   | online | memoised `checks(scope)` queries per page, potential, strip or station; budget 5 ms per rule per commit, over budget → demoted to deferred and logged | Class I |
   | offline | Class II fork job, parallel per scope (PE continuity across rails, configurator validation, clearance) | "as of revision n" |
   | prevent | commit validator on the candidate snapshot (ADR-0009): strict rejects, lenient reports | — |
   | release | every enabled rule cold on a fork at the saved generation | fresh |
   | export *(amendment 1)* | before an exporter writes, on the exporter's fork | fresh |

4. **Rule packs** are versioned directories of TOML, Rust references, rhai, SQL, Fluent messages and test fixtures,
   for example `ruleset = "EN60204-1:2018+A1:2025"`.
   - **Shipped packs:** EW-CORE, EW-IEC81346, EW-TERM, EW-POT, EW-PLC, EW-BUS, EW-FL, EW-LAYOUT and EW-REL.
     EW-CORE, EW-IEC81346, EW-TERM, EW-POT and EW-REL ship at MVP (FR-CHK-004). EW-PLC, EW-BUS, EW-FL and
     EW-LAYOUT are complete at 1.0 (FR-CHK-005), but the subsets Golden A needs ship at MVP, for example EW-PLC-012,
     EW-BUS-001 and EW-LAY-001 ([15](../design/15-checks-revisions-collaboration.md) §4.1). Amendment 2 adds
     EW-THM, EW-MFG and EW-EN60204.
   - **Company packs** are added without forking. Licensed threshold tables plug in per organisation, and data tables
     carry a `verified` flag and a citation.
   - **Version record.** A released revision records the pack versions it was checked against (ADR-0013).
5. **Schemes** set enablement, severity and mode per rule. A project selects one. EW ships `pre-delivery`, and
   companies may lock rules in their basic project. Amendment 6 lists the other shipped schemes.
6. **Waivers and quick fixes.**
   - A waiver is an entity `{rule, target, reason, author, date}` and appears in the release report.
   - A quick fix is a previewed command (ADR-0008), for example "rename +JE2416 → +J-E-2416", "move override to
     profile" or "create incoming supply cable".
7. **Release gate.** `ew check --scheme pre-delivery` must report zero unwaived errors. Always blocking:
   - `Tbd` values, such as the DP station address of -201U1 and placeholder parts (FR-CHK-034, EW-BUS-001);
   - author = checker, or no checker (EW-REL-001; D-34);
   - stale Class II results or stale committed computations (EW-REL-002, EW-LAY-011);
   - unplaced devices (EW-LAY-001; F55, E55, E56, S56 and -201U9 on Golden A);
   - cables without length (FR-CHK-033; -3W1 on Golden A).
8. **Trust and exposure.**
   - Project-resident rule packs (`rules/<uuid>.toml` plus an optional `rules/<uuid>/` directory for rhai, SQL,
     Fluent and documentation files; the human pack ID lives inside the TOML) are untrusted until the project is
     trusted (NFR-SEC-05). Built-in packs are trusted. Shipped and company pack directories keep their own internal
     layout.
   - Waiving findings, editing schemes, changing strictness and releasing are human-only commands. Agents, plugins
     and scripts cannot reach them (ADR-0020).

```toml
[pack]
id = "EW-POT"
version = "1.0.0"
ruleset = "EW-POT:2027.1"

[[rule]]
id = "EW-POT-013"            # single-pole switching contact in a switched N potential (D-13: T55, S56)
code = "005013"              # six-digit code: class 005 + number (amendment 3)
severity = "error"
modes = ["online", "release"]
scope = "potential"
impl = "rust:pot.n_switching@1"   # or "rhai:<expr>", or "sql:pot/013.sql" (offline and release only)
message = "rule-ew-pot-013"       # Fluent message with .title and .hint attributes
docs = "rules/ew-pot-013"
```

## Amendments

Accepted on 2026-10-18 ([15](../design/15-checks-revisions-collaboration.md) open questions 1 and 2, §2–§4). The
decision text above is unchanged except for the rule IDs and the project-pack file names, which follow the
canonical forms of [15](../design/15-checks-revisions-collaboration.md) §2.2 and
[project format](../design/05-project-format-and-persistence.md) §2.1.

1. **A fifth mode, `export`** (2026-10-18). It runs EPLAN-style "module-specific" checks on the exporter's fork
   before an exporter writes, for example EW-PLC-005 and the manufacturing pre-flight EW-MFG-001 … 006. The modes
   are therefore online, offline, prevent, release and export (mode table above).
2. **Additional packs** (2026-10-18). The shipped packs are EW-CORE, EW-IEC81346, EW-TERM, EW-POT, EW-PLC, EW-BUS,
   EW-FL, EW-LAYOUT, EW-THM, EW-MFG, EW-EN60204 and EW-REL. The domain packs ship their MVP subsets
   ([15](../design/15-checks-revisions-collaboration.md) §4.1) at MVP and are complete at 1.0 (FR-CHK-005).
   EW-EN60204 is enabled only through a project's applicable standards or the `machinery-en60204` scheme
   (FR-CHK-052).
3. **Rule IDs and a registry of six-digit codes** (2026-10-18; FR-CHK-043). Every ID has three parts and a
   three-digit number, including layout (`EW-LAY-nnn`); the earlier draft forms (two-digit numbers, the fused
   `EW-Lnnn`) are replaced without aliases, because nothing has shipped with them. Every rule also has a six-digit
   code `CCCNNN` (three-digit class plus number) from a registry file in `ew-checks`. IDs and codes are never
   reused, and a CI test fails on duplicates, reuse, IDs outside their family's number blocks and rule IDs cited
   in the documentation but missing from the registry.
4. **Fluent message IDs `rule-<id>`** (2026-10-18), with `.title` and `.hint` attributes, because a dotted key is
   not a valid Fluent identifier. The example above uses this form.
5. **Gate-only release rules** (2026-10-18; FR-CHK-053). The rules of the release-gate pack EW-REL are evaluated
   only in the release gate, the approve dialog and `ew check --gate`, never in a plain `ew check` run.
6. **Shipped schemes** (2026-10-18; [15](../design/15-checks-revisions-collaboration.md) §4.9). Besides
   `pre-delivery`, EW ships `drafting` (default while editing), `migration` (the lenient run behind a migration
   report), `full`, `manufacturing`, `machinery-en60204` (1.0) and `assembly-iec61439` (1.0).
7. **Rule-implementation registry** (2026-10-18; [architecture](../design/04-architecture.md) §3, §12).
   `ew-checks` owns the `RuleImpl` trait and the registry, and implements Rust and rhai rules itself. `ew-sqlview`
   implements SQL rules and `ew-plugin-host` plugin rules; `ew-api` registers both at session start. `ew-checks`
   never depends on `ew-sqlview` or `ew-plugin-host`.

## Consequences

**Positive**
- **Incremental panel.** The message panel updates incrementally: a commit re-evaluates only the scopes it touched,
  and online checks are queries, so they are fresh when read (ADR-0003).
- **Shared expected findings.** On Golden A, `ew check --scheme pre-delivery` reports exactly the rows of
  `expected/findings_mvp.csv` ([20](../design/20-golden-project.md) §18.1: 42 errors, 32 warnings, 28 infos,
  1 waived) and exits 1. The headline items are those of FR-CHK-042: EW-POT-013 on T55 and S56 (D-13, kept as
  drawn), 28 odd-bit coil bindings Q0.1 … Q6.7 (EW-PLC-012, D-26), the `Tbd` DP address (EW-BUS-001, D-15), S56
  and E56 without parts (FR-CHK-024) and five unplaced devices (EW-LAY-001, D-32). This is a CI fixture that the
  pilot user can read.
- **Rules without Rust.** Domain experts write SQL or rhai rules. Companies add packs, and licensed tables stay
  outside the repository.
- **Lenient migration.** The same rule serves strict editing and lenient migration. A migration report is simply a
  check run (ADR-0021).
- **Reproducible results.** Check results of a released revision can be reproduced from its recorded pack versions.

**Negative**
- **Three implementation languages** must be documented, tested and versioned. Every shipped rule ID needs a
  documentation page (NFR-DOC-02) and one passing and one failing fixture.
- **SQL rules are offline only.** Freshness differs by implementation kind, and the UI must show it.
- **Budget enforcement** needs per-rule timing and engine statistics on every commit (NFR-PERF-09).
- **Safety disclaimer.** Safety-relevant rules (PE sizing, N switching) depend on data that must be verified against
  licensed standards. A rule that reads an unverified table reports at most warning until a reviewed change sets
  the `verified` flag (FR-CHK-046; [15](../design/15-checks-revisions-collaboration.md) §3.2). Every report states
  that checks do not replace engineering verification (NFR-COMP-04).

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **Rules compiled into the binary only** | Companies could not add rules without forking, and licensed thresholds could not stay outside the repository |
| **Datalog** for rules | Unfamiliar to the domain experts who should write rules, no ecosystem tooling, and in practice compiled in |
| **Hard-coded severities** | No schemes: drafting and pre-delivery could not differ, and legacy projects could not be relaxed (ADR-0009) |
| **Checking only at release** | Late feedback. The incremental online engine costs little, because online rules are ordinary queries |
| **SQL for online rules** | The projection is Class II and asynchronous, so it cannot give synchronous answers on every commit |

## Related

- **ADRs:** [ADR-0003](0003-freshness-contract.md), [ADR-0008](0008-transactions-undo-preview.md), [ADR-0009](0009-invariant-catalogue.md), [ADR-0013](0013-revisions-and-as-built-archive.md), [ADR-0017](0017-read-only-sql-projection-api-v1.md),
  [ADR-0020](0020-extensibility-tiers-and-security-model.md), [ADR-0021](0021-importers-emit-format-level-records.md),
  [ADR-0022](0022-platform-matrix-and-performance-budgets.md).
- **Design:** [04 architecture](../design/04-architecture.md) §3, §4.6, §12, §14;
  [05 project format](../design/05-project-format-and-persistence.md) §2.1;
  [15 checks, revisions and collaboration](../design/15-checks-revisions-collaboration.md) §2–§4;
  [20 golden project](../design/20-golden-project.md).
- **Requirements:** FR-CHK-001, FR-CHK-002, FR-CHK-003, FR-CHK-004, FR-CHK-005, FR-CHK-006, FR-CHK-007, FR-CHK-008,
  FR-CHK-009, FR-CHK-010, FR-CHK-011, FR-CHK-012, FR-CHK-013, FR-CHK-014, FR-CHK-015, FR-CHK-019, FR-CHK-020,
  FR-CHK-024, FR-CHK-029, FR-CHK-033, FR-CHK-034, FR-CHK-042, FR-CHK-043, FR-CHK-046, FR-CHK-052, FR-CHK-053;
  FR-LAY-017; NFR-PERF-09, NFR-DOC-02, NFR-COMP-04, NFR-COMP-05, NFR-SEC-05.
