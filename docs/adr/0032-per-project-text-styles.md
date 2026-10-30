# ADR-0032: Per-project text styles as overrides of the built-in table

**Status:** Accepted · **Date:** 2026-10-30 · **Deciders:** lead designer (cycle-36 design lane; the project owner may
reverse it)

## Context

- **Text styles are a constant today.** Every renderer resolves a style key through `ew_text::builtin()`, the
  fourteen rows of doc 20 §17.5 in QET pixels (`cover.title` 48 px … `cp.small` and `marker` 5 px). `ew-checks` may
  not link `ew-text` (`ci/dep-rules.toml`), so it keeps a copy, `TEXT_STYLES`, held equal by a test in `ew-api`.
  Nothing in a project file names a style size.
- **Requirement.** FR-SYM-013 says text styles are **model data** (font, height, weight, alignment) bound to slot
  kinds, so that changing the "device tag" style changes every tag. Doc 09 §2.1 places project-level text styles in
  `project.jsonl`; doc 03 §5.1 keeps them in `extra` "until their work packages". `ew-text`'s own module comment says
  project styles "will override or extend this table".
- **The defect (FT-662, ID-833).** `cp.small` and `marker` at 5 px have an em of 1.323 mm and a printed cap height of
  0.94 mm (cap ratio 0.714, doc 20 §18.1), below EW-CORE-061's 1.2 mm minimum (U-31). EW-CORE-061 reports only styles
  a drawn text uses (ID-825), so an empty project is clean, but the first wire marker or MCB pin number in a **fresh
  project** raises the warning: the product's own default violates its own rule. 7 px gives an em of 1.852 mm and a
  cap height of 1.32 mm, the smallest pixel size that passes.
- **Fidelity.** Golden A reproduces the reference drawings, where these texts are 3.70 pt (EF-12 expects the two
  EW-CORE-061 findings). Golden A must not change, and neither may any existing project file or its output. Golden C
  (the QET import) reproduces the source's sizes.
- **Constraints.** Format 0.1 is unreleased, so additive members are allowed without a version step (ADR-0011
  amendment 4, decision log ID-334). Copies of the same data in two crates drift (FT-648 is one such drift already).
  The team is two to four people: the smallest model that meets FR-SYM-013 wins.

## Decision

1. **A project stores its text styles as overrides of the built-in table**, in the optional member
   `ProjectSettings.text_styles` of the project record (`project.jsonl`): a JSON object keyed by style key, each value
   `{ "size": <em height in nm, integer>, "weight": "regular" | "bold" (optional) }` plus unknown members kept
   verbatim. Example for a new project:
   `"text_styles": {"cp.small": {"size": 1852083}, "marker": {"size": 1852083}}`.
   - The **effective table** is the built-in table with each override applied: a key in both takes the override's
     size and, when given, its weight; a key only in the overrides **extends** the table (a new style a symbol slot
     or form cell may name; regular unless stated). Keys are identifiers; sizes lie in 0.5 mm … 50 mm of em height
     (500,000 … 50,000,000 nm); members are written in key order by the canonical writer; a duplicate key is the
     reader's duplicate-member finding (EW-CORE-007), as for every object.
   - Family, italic and alignment are not typed: the bundled face is the only family at MVP (ADR-0028) and alignment
     belongs to the slot. They stay in each entry's unknown members until a package types them.
2. **An absent or empty member means "no overrides"**: the built-in table as it is today, `cp.small` and `marker` at
   5 px (0.94 mm cap height). Every existing file therefore renders as before, with **no migration and no format
   version step**: the member is additive within the unreleased 0.1. Nothing writes the member into a file that lacks
   it: no normaliser, no load or save, no importer.
3. **New projects get the 7 px defaults.** The built-in basic project of `Session::new_project` writes the two
   overrides of the example (`cp.small` and `marker` at `Grid::QET_1PX.at(7)` = 1,852,083 nm, 1.32 mm cap height). A
   company basic project is copied as it is, member or not, and `save_as_basic` keeps it. Importers (the golden
   tables emitter, QET, later EPLAN) write no member, so **Golden A and Golden C are unchanged** and EF-12 still
   reports its two findings.
4. **The built-in table is part of the derivation profile.** Its rows are the base every project's overrides apply
   to; changing a built-in row changes the output of every project without overrides, so it is a derivation-profile
   change (ADR-0003), never a silent edit. New defaults for new projects go into the built-in basic project's
   overrides, not into the base.
5. **One source for the project's styles.** The base rows (key, QET px, weight) move to `ew-schema` beside the member
   type, because the meaning of an absent member is format semantics, and a pure function there computes the
   effective table. `ew-derive` exposes it as a memoised query of the project record (`text_styles`); `ew-checks`
   reads that query, and **its `TEXT_STYLES` copy is deleted**. `ew-scene` receives the effective rows from its
   caller and resolves keys through a `ew_text::TextStyleTable` built from them (`TextStyleTable::from_rows`); the
   scene, forms, tables and embedded blocks stop calling `ew_text::builtin()`. `ew-text` keeps `builtin()` only for
   callers with no project (font tests, library previews), and a test in `ew-scene`, which links both crates, keeps
   its rows equal to `ew-schema`'s. No new dependency edge.
6. **Editing.** `project.settings.set` gains the whitelisted member `text_styles` (the whole object replaced, one undo
   step, refused outside decision 1's key and size rules with the member path). Removing an entry restores the base row.
   A window editor for the table is a follow-up (FT-669).

## Alternatives considered

| Option | For | Against | Verdict |
|---|---|---|---|
| **A. The full table in every project**, a migration writing the 5 px table into existing files | Every file self-describing; no base table | Rewrites every existing project and Golden A's `project.jsonl`; a migration for a pre-release format; future built-in styles never reach old projects | Rejected |
| **B. Overrides of a built-in base, absent = today's table (chosen)** | No migration, no golden change, minimal file text; new projects explicit and visible; old projects keep their look | The output of a project depends on the base table, so the base is pinned by the derivation profile (decision 4) | **Chosen** |
| **C. Absent = the new 7 px defaults**, old files gaining 5 px overrides by migration | New projects need no member | Changes Golden A and every existing file; the meaning of "absent" would change between builds of one format | Rejected |
| **D. A record kind `text_style`**, one record per style in `project.jsonl` | Per-style git merges; room for per-style provenance | A kind, a rank, IDs and CRUD commands for fourteen rows; the settings member merges per key in git already (one line per project record either way) | Rejected for MVP |
| **E. Text styles as library documents** (a style sheet embedded like forms) | Company standards shared through libraries | Copy identity, update classification and embedding for a handful of numbers; the basic project already carries company standards (FR-PRJ-009) | Deferred to 1.0 if pilots ask |
| **F. Raise the built-in 5 px rows to 7 px** | One-line change | Changes Golden A's output and every existing project; the reference set's 3.70 pt texts could no longer be reproduced | Rejected |

## Consequences

- **Format.** `schemas/0.1` gains the optional member, re-blessed additively. An older 0.1 build keeps the member in
  `settings.extra` and renders with the base table (5 px): a known mixed-build difference, acceptable before 0.1's
  release. Doc 05 §2.2 (the project record's members) and doc 03 §5.1 (text styles no longer in `extra`) are amended
  by notes that point here; doc 03 §5.1 now, doc 05 §2.2 with the as-built text after the lane.
- **Rendering and checks.** The scene, the PDF and SVG exports and EW-CORE-061 read one effective table. In a fresh
  project, wire markers and small pin numbers print at 1.85 mm em and EW-CORE-061 stays silent; Golden A keeps EF-12.
  FT-648 (the style *choice* copied into `ew-checks`) is not closed by this ADR, but its shared query lands beside
  `text_styles` in `ew-derive`.
- **Tests and goldens.** Golden A and Golden C are byte-identical. Expected outputs that start from the built-in basic
  project (new-project tests, blind and play-test references whose pages print markers or small pin numbers) change
  in those texts only and are re-blessed once in a reviewed commit, listed by the lane.
- **Documents** (at the lane's design pass, except doc 03 §5.1's note, written with this ADR). Doc 09 §2.8 (styles as
  project data, as built), doc 20 §17.5 (a note: new projects override
  `cp.small` and `marker` to 7 px), doc 07 §2.3 (a slot's `style` may name a project-extended key), FR-SYM-013's
  acceptance unchanged. The implementation is specified in
  [WP-S1.12 project text styles](../plan/specs/WP-S1.12-project-text-styles.md); FT-662 closes with it.
- **Work packages.** WP-S1.12 (FT-662's owner) carries the lane; FR-SYM-013 stays traced to WP-S3.01, which delivered
  the style binding.
- **Risk.** A renderer path that still calls `ew_text::builtin()` ignores the project's overrides. The specification's
  acceptance renders an override through every text source and asserts that only the project-less callers name
  `builtin()`.

## Sources

- Requirements: FR-SYM-013, FR-PRJ-009; checks: EW-CORE-061, EW-CORE-063, EW-CORE-007.
- [Golden project](../design/20-golden-project.md) §17.5, §18.1 (EF-12, U-31); [reports and forms](../design/09-reports-forms-output.md)
  §2.1, §2.8; [domain model](../design/03-domain-model.md) §5.1; [symbols](../design/07-symbols-parts-macros.md) §2.3.
- Decision log ID-334, ID-825, ID-833; FT-648, FT-662, FT-669.
- [ADR-0003](0003-freshness-contract.md), [ADR-0011](0011-format-versioning-and-migration.md),
  [ADR-0024](0024-licence-set-and-permissive-gpl-boundary.md), [ADR-0028](0028-default-drawing-fonts.md).
