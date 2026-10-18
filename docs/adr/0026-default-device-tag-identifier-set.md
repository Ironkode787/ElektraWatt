# ADR-0026: Default device-tag identifier set for new projects

**Status:** Accepted · **Date:** 2026-10-17 · **Deciders:** project owner (decided 2026-10-17)

## Context

- **The reference set uses classic single letters.** Examples: F (-101F1, -102F1, -102F2), G (-102G1), S (S51,
  S56), T (T55), E (E55, E56), X (1X1, +JE2416-X1031), W (-3W1, -106W1), U (-201U1 … -201U9) and Q. Device tags
  mostly follow the page-based scheme `-{page}{letter}{counter}`.
- **Three letter-code families are in use** ([standards](../research/standards.md) §1.4–§1.5):

  | Device (folio) | As drawn | EN 81346-2:2009 | IEC 81346-2:2019 |
  |---|---|---|---|
  | MCB -101F1 (101) | F | F | FC |
  | 24 V PSU -102G1 (102) | G | G | TB or TC |
  | Main switch S51 (101) | S | Q | QB |
  | Thermostat T55 (101) | T | B | BT |
  | Door switch S56 (101) | S | B | BG |
  | Terminal strip X1031 (103) | X | X | XD |
  | Bus node -201U1 (201) | U | K | KE |

  - **IEC 81346-2:2019** introduces two-letter classes (FC, QB, BT, TB, XD …) with changed meanings: U now means
    "holding object", and V is reserved.
  - **EN 81346-2:2009** single letters are what the pilot's engineers and their customers read today.
  - **IEC 60750 / DIN 40719-2 letters** (T for transformers, S for switches, U for converters; list **unverified**,
    [standards](../research/standards.md) §1.5) explain part of the reference set, but not T for a thermostat.
    Migrated QET projects may follow them.
- **Prior recommendation.** The standards research recommended the 2019 two-letter codes as the default for new
  projects. The project owner declined that recommendation on 2026-10-17 and chose classic single letters.
- **What an identifier set is.** In EPLAN terms, an identifier set maps a function category (for example
  `ProtectionOvercurrent`) to a letter code. The function definition, not the user, supplies the letter for a new
  device ([standards](../research/standards.md) §1.6).
- **Where the letter lives.** EW stores the letter in each device's `TagSpec`. The printed tag is derived (ADR-0001,
  ADR-0003).

## Decision

1. **Default.** New projects default to the **EN 81346-2:2009 single-letter** identifier set (key `EN81346-2:2009`).
2. **Shipped sets.** EW also ships **IEC 81346-2:2019** (`IEC81346-2:2019`, two-letter classes) and **IEC 60750
   legacy** (`IEC60750-legacy`), selectable per project. A company basic project, and the company-standard wizard,
   can make another set the company default.
3. **Sets are data in `ew-rds`, not code.** Each set maps function categories to a letter code and lists the
   accepted alternatives, with short descriptions written by the project. No standard text or table is copied
   (NFR-COMP-05), and every mapping row carries a `verified` flag.
4. **How a set is used:**
   - **New devices.** A new device gets its letter from the active set through its `FnDef` or part category
     (FR-NUM-004). The engineer may override it, and the override is stored in `TagSpec.letter`.
   - **Conformance.** EW-RDS-003 (warning) reports a letter that does not match the device's category in the active
     set, with a suggestion (FR-CHK-026).
   - **Display.** Printing and display never re-map stored letters. A tag reads the same until someone changes it.
5. **The identifier set is pinned per project** and is part of the project's derivation profile (ADR-0003).
   Upgrading EW never changes a project's set.
6. **Switching sets is a previewed renumbering command** (FR-NUM-014).
   - It rewrites the letters of non-frozen tags from their categories, for example -101F1 → -101FC1 and -102G1 →
     -102TC1 under the 2019 set (FR-NUM-014).
   - It shows the tag, BOM and cross-reference diffs before commit (ADR-0008).
   - It leaves frozen tags untouched, and the numbering report lists them.
7. **Import.**
   - The QET importer keeps letters as found.
   - It proposes `IEC60750-legacy` when only that set explains a project's letters.
   - It never switches a project to the 2019 set; that is the explicit renumber above.

## Consequences

**Positive**
- **No configuration needed.** The golden project reproduces from the shipped defaults plus the pilot's company
  basic project, with no further settings edits (NFR-UX-08). Folio 101 still prints -101F1, and folio 102 prints
  -102G1 and -102F2.
- **Familiar letters.** New projects use the letters the pilot's engineers, shop floor and customers already know.
  Migrated and new projects in one company look alike.
- **Expected findings on Golden A.** EW-RDS-003 reports exactly T55, a T for a temperature switch (golden EF-18).
  S51 and S56 are accepted as alternatives in the shipped 2009 mapping. For the count to stay 1, U must also be
  accepted for the valve-terminal assemblies -201U1 … -201U9 and -202U1 … -202U3, which the 2009 list would class
  as K (bus node) or U (holding). That mapping assumption (golden U-37, which names only S51 and S56 so far) is to
  be confirmed with the pilot (PQ-08).
- **Later sets are data.** Further sets (an NFPA 79 letter set, a company set) can be added later as data, without a
  code change.

**Negative**
- **Not the current edition.** New projects do not follow IEC 81346-2:2019 by default. Customers who require it
  must select the 2019 set per project or in their basic project.
- **Exports carry 2009 letters.** Exports (AML, EPLAN XML, CSV) carry 2009 letters, and partners on 2019 tooling may
  have to map them.
- **Mapping maintenance.** Three mapping tables must be maintained and verified against licensed copies of the
  standards. Until verified, their conformance findings stay advisory.
- **Switching cost.** Switching a mature project to another set renumbers many tags. The preview makes that visible,
  but released revisions keep their old tags in the archive.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **IEC 81346-2:2019 two-letter codes as the default** | Recommended by the standards research, but **declined by the project owner** (2026-10-17). The pilot's drawings, engineers and customers use single letters, and the golden project would need configuration or a renumber |
| **IEC 60750 legacy letters as the default** | Superseded by the IEC 61346/81346 series. They remain available for migrated projects |
| **Letters hard-coded per symbol** (QET style) | No per-project choice, no conformance check, and no clean way to move a project to another standard (the D-03 class) |
| **No identifier sets, free letters only** | The letter would carry no meaning EW can check. Category-based defaults and EW-RDS-003 would be impossible |
| **Re-mapping letters at display time** from the active set | Changing the set would silently change every printed tag, and archived drawings would not reprint identically (ADR-0003) |

## Related

- **ADRs:** [ADR-0001](0001-core-model-persistent-maps.md), [ADR-0003](0003-freshness-contract.md), [ADR-0006](0006-function-device-representation-model.md), [ADR-0008](0008-transactions-undo-preview.md), [ADR-0019](0019-rule-engine-and-rule-packs.md),
  [ADR-0021](0021-importers-emit-format-level-records.md).
- **Design:** [03 domain model](../design/03-domain-model.md) §8.3; [07 symbols, parts and macros](../design/07-symbols-parts-macros.md);
  [14 interop](../design/14-interop.md) §4.4; [15 checks](../design/15-checks-revisions-collaboration.md) §4.3;
  [16 UX/UI](../design/16-ux-ui.md) (company-standard wizard); [20 golden project](../design/20-golden-project.md);
  [standards](../research/standards.md) §1.4–§1.6.
- **Requirements:** FR-NUM-001, FR-NUM-003, FR-NUM-004, FR-NUM-005, FR-NUM-013, FR-NUM-014, FR-NUM-016, FR-PRJ-010,
  FR-PRJ-011, FR-CHK-025, FR-CHK-026; NFR-UX-08, NFR-COMP-05, NFR-REL-09.
