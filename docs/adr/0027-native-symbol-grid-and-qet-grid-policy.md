# ADR-0027: Native symbol and connection grid; policy for QET-grid pages

**Status:** Proposed · **Date:** 2026-10-17 · **Deciders:** project owner (pilot user), after consulting two EPLAN
users

## Context

- **The QET grid.** QET draws in 96-dpi pixels: 1 px = 25.4/96 mm, and its wiring grid is 10 px =
  127/48 mm ≈ 2.6458 mm. EW stores linear coordinates as i64 nanometres with rational grids (ADR-0005), so the QET
  grid is exactly `Grid{num: 7_937_500, den: 3}` nm and its 1 px fine grid is `793_750/3` nm.
- **Exact cell lookups.** Cross-references are computed by integer arithmetic on the frame's rational grid
  (FR-PAG-020). The QET-compatible frame of the reference set has 17 columns (0–16) of 60 px and 8 rows (A–H) of
  80 px, so imported geometry keeps "102-A1", "103-G6" and "101-H16" exact.
- **EW needs its own grid.** EW's own symbol library (the starter set of about 40 IEC 60617 and ISO 1219 symbols
  used by J-E-2416, then the full library) must place connection points on one native connection grid. Every native
  symbol, function definition, macro and window macro will depend on it, so the choice cannot be changed cheaply
  later.
- **EPLAN conventions.** EPLAN schematic grids are commonly 1, 2, 4, 8 and 16 mm, and IEC symbols are commonly built
  on a 4 mm connection grid. Both points are **unverified** (G4).
- **Density.** The QET frame's drawable area is 1,020 × 640 px, which is 269.9 × 169.3 mm.

  | Grid | Columns | Rows | Positions per axis compared with QET |
  |---|---|---|---|
  | QET 10 px | 102 | 64 | 100 % |
  | 2.5 mm | 107 | 67 | about 105 % |
  | 4 mm | 67 | 42 | about 66 % |

  A 4 mm grid therefore gives a sparser page than the pilot is used to.
- **Incommensurable grids.** Neither 4 mm nor 2.5 mm is an integer multiple of 127/48 mm. CPs must lie on their
  page's connection grid (FR-SYM-009), so native symbols cannot be placed on QET-grid pages, and QET-import symbols
  cannot be placed on native pages.
- **Timing.** The spine's lane 2 draws the starter set in weeks 2–5. The symbol set cannot be drawn before this
  decision ([07](../design/07-symbols-parts-macros.md) OQ9, PQ-06).

## Decision

**Pending.** Independent of the outcome:
- symbol definitions carry their grid (`SymbolDef.grid`), so code paths do not depend on the choice;
- the golden project is reproduced on the QET 10 px grid (ADR-0005; [20](../design/20-golden-project.md) §2.1);
- imported QET pages keep their grid (FR-PAG-021).

## Recommendation

1. **Native connection grid: 4 mm.** Native symbol CPs lie on `Grid{num: 4_000_000, den: 1}` nm. Text slots use a
   1 mm fine grid. Graphics use a 0.5 mm fine grid, because arrow heads and small arcs of IEC 60617-style symbols
   need it ([07](../design/07-symbols-parts-macros.md) OQ9; FR-SYM-009's acceptance already accepts 0.5 mm
   graphics). The symbol editor and the library lint refuse an off-grid CP at save (FR-SYM-009, FR-SYM-020).
2. **QET-grid pages are kept permanently.** QET-imported pages and their per-project "QET import" symbols keep the
   native 10 px rational grid. QET-import symbols may be placed only on QET-grid pages, and native symbols only on
   native-grid pages.
3. **Conversion is explicit and later.** A previewed "convert page to native grid" command follows in v1.x
   (FR-PAG-022, Could). It must keep every connection and every cross-reference cell, or refuse with a report.
   Pages are never converted automatically.
4. **A basic-project setting "grid for new pages"** is restricted to the native grid (default) or the QET 10 px grid
   ([06](../design/06-schematic-editor.md) OQ6). A migrating company can then keep extending QET-style projects
   until it adopts native symbols.
5. **Visible grids.** The status bar and a page badge show the active grid ("4 mm" or "QET 10 px"), and the cursor
   readout shows the page cell (FR-UI-018, FR-UI-034).
6. **If the pilot prefers QET-like density on A4, choose 2.5 mm instead.** Everything above applies unchanged with
   `Grid{num: 2_500_000, den: 1}`.
7. **Before the starter set is drawn**, confirm the EPLAN conventions (the 4 mm connection grid and the usual grid
   steps) with two EPLAN users from the NFR-UX-01 test group.
8. **A generated QET-grid edition of the starter set only.** Golden A sits on the QET grid, yet FR-SYM-007 requires
   its folios to render from the starter library, and Golden C exchanges QET-import symbols for starter symbols on
   QET-grid pages. The starter set therefore ships a second, generated edition (item-key suffix `@qet10`): every CP
   keeps its count of grid steps (`k · 4 mm` → `k · 7_937_500/3` nm), graphics and slot positions scale by the same
   ratio (about 0.66 at 4 mm, 1.06 at 2.5 mm) and round to the nm, and text heights stay style-driven. It is linted
   like the native edition and never drawn by hand. The full 1.0 library gets no such edition
   ([07](../design/07-symbols-parts-macros.md) §3.2 rule 2, OQ14; proposed FR-SYM-nnn in 07's "Proposed new
   requirements").

## Consequences

If 4 mm is accepted:

**Positive**
- New symbols align with EPLAN-trained habits and with symbols drawn to IEC 60617 proportions on a 4 mm pitch
  (to verify).
- Coordinates are whole millimetres in the editor, in DXF export and in symbol files, which makes library
  contributions easier to review.
- The golden project's acceptance (±0.2 mm frame, REF-096; cell-exact cross-references) is untouched, because it
  stays on its own grid.

**Negative**
- **Lower density.** New A4 pages hold about a third fewer grid positions per axis than QET pages. Dense pages such
  as folio 103's distribution need A3 or more pages.
- **Mixed-grid projects.** A migrated project mixes grids. Editing an old page means using the "QET import"
  library or the `@qet10` starter edition, and moving a circuit to a native page means redrawing it until the
  conversion command exists (1.x). Clear per-page indicators are mandatory.
- **Two starter editions.** The generated `@qet10` edition doubles the starter set's lint and golden surface, and
  its scaled graphics land on non-round nm values.
- **Hard to reverse.** The decision is effectively irreversible once the community library grows. A later change
  would need a library-wide conversion.

## Alternatives considered

| Alternative | Assessment |
|---|---|
| **2.5 mm native grid** | QET-like density on A4 and familiar to the pilot. Less aligned with the (unverified) EPLAN 4 mm convention that EPLAN-trained hires bring. A valid choice if the pilot values density over EPLAN alignment |
| **The QET grid (127/48 mm) for new symbols** | Rejected in G4: a non-metric pitch with non-round millimetre coordinates in every file and export, tied to QET's 96-dpi pixel. It would make QET's historical choice permanent for EW's own library |
| **Converting imported pages automatically on import** | Snapping to a new grid moves anchors across cell boundaries, so "102-A1"-style references could change. It also breaks the migration verifier's overlay against the original PDF (FR-IMP-021) |
| **A per-symbol free grid** (no project-wide rule) | Symbols from different authors would not connect cleanly, and autoconnect and the "next connection point" navigation (FR-UI-015) would degrade |

**Decision needed from:** the project owner (pilot user), after confirming EPLAN conventions with two EPLAN users.
It is due before spine lane 2 draws the starter symbol set, that is by week 2 of the spine (PQ-06;
[vision](../design/00-vision-and-scope.md) OQ9).

## Related

- **ADRs:** [ADR-0005](0005-value-types-nm-dec3-mltext.md) (rational grids), [ADR-0007](0007-explicit-connections-and-autoconnect.md) (autoconnect on CP alignment), [ADR-0015](0015-display-list-renderers-pdf.md),
  [ADR-0021](0021-importers-emit-format-level-records.md) (QET import library),
  [ADR-0028](0028-default-drawing-fonts.md).
- **Design:** [06 schematic editor](../design/06-schematic-editor.md) §2.4, OQ6;
  [07 symbols, parts and macros](../design/07-symbols-parts-macros.md) §3.2, OQ9;
  [14 interop](../design/14-interop.md) §4.2; [16 UX/UI](../design/16-ux-ui.md);
  [20 golden project](../design/20-golden-project.md); [gaps-and-corrections](../research/gaps-and-corrections.md) G4.
- **Requirements:** FR-PAG-020, FR-PAG-021, FR-PAG-022, FR-SYM-007, FR-SYM-009, FR-SYM-015, FR-SYM-018, FR-SYM-020, FR-XRF-021,
  FR-IMP-008, FR-IMP-021, FR-UI-015, FR-UI-018, FR-UI-034; NFR-UX-01, NFR-UX-08.
