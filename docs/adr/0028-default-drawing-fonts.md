# ADR-0028: Default drawing fonts

**Status:** Proposed · **Date:** 2026-10-17 · **Deciders:** project owner (pilot user)

## Context

- **Tahoma cannot be bundled.** The reference drawing set uses Tahoma throughout, and Tahoma may not be
  redistributed (G14). NFR-LIC-06 lists it among the content EW never ships.
- **Embedding matters.** PDF output embeds font subsets (FR-OUT-008, ADR-0015), so the font licence must permit
  embedding in documents that users distribute to customers.
- **Determinism.** A project must open, render, check and export without system fonts (NFR-REL-11). The same model
  must produce byte-identical output on Windows, macOS and Linux (NFR-REL-07). Font files are part of the export
  input, by hash ([09](../design/09-reports-forms-output.md) §4.3).
- **Small sizes and special glyphs.**
  - The reference prints text down to 3.70 pt (D-31). The golden text styles are 8.88 pt normal text, 6.66 pt
    small data, 5.92 pt pin designations and 5.18 pt device cross-references
    ([20](../design/20-golden-project.md) §17.5).
  - Cross-reference texts use "∞" and "⇄", which a text font may not cover.
- **Languages.** Dutch, English and German come first. CJK and right-to-left fallbacks follow later (FR-I18N-016,
  1.x).
- **The candidates** (G14):

  | Font | Licence | Notes |
  |---|---|---|
  | Noto Sans | OFL-1.1 | Broad Latin, Greek and Cyrillic coverage; a family with symbol companions |
  | DejaVu Sans | Bitstream Vera licence (Vera-derived) | Broad coverage; its licence is not on the NFR-LIC-04 allowlist |
  | osifont | GPL-3, GPL-2 or LGPL-3, each with a font exception that keeps embedding documents free of copyleft | An ISO 3098-style CAD lettering font |
  | WineTahoma | Vera-derived | A Tahoma substitute whose metric compatibility is **unverified** |

- **One text stack.** Screen and PDF share one text stack (parley, fontique, skrifa, harfrust). Line breaks are
  computed once, in the memoised `shaped_text` query, so the chosen font changes both outputs identically.

## Decision

**Pending.** In the interim, spine lane 2 builds `ew-scene`, the frame forms and the PNG/PDF goldens with Noto Sans,
as [09](../design/09-reports-forms-output.md) §2.8 and [14](../design/14-interop.md) §4.10 already assume. If the
decision differs, the goldens are re-baselined once.

## Recommendation

1. **Default: Noto Sans** (OFL-1.1). It is bundled in the weights the shipped text styles use, and it is the default
   drawing font of new projects and the substitution target for imports.
2. **Optional: osifont** as an ISO 3098-style font, selectable per text style or per project. Its SPDX expression is
   to be verified (likely `GPL-3.0-or-later WITH Font-exception-2.0`). Bundling it adds that expression to the
   bundled-asset allowlist of ADR-0024; NFR-LIC-04 currently names only OFL-1.1 for fonts and must be amended with
   it.
3. **A symbol fallback.** Bundle an OFL symbol fallback for glyphs the default lacks, such as "∞" and "⇄". The
   candidates are the Noto symbol and math families, with coverage to be verified. Export reports every character
   without a glyph in the project's font set ([09](../design/09-reports-forms-output.md) P09-9).
4. **Never bundle Tahoma.** QET import maps Tahoma, and the other UI families QET writes ("Sans Serif",
   "MS Shell Dlg 2", Arial, Helvetica, Liberation Sans), to the default font.
   - It keeps the point sizes.
   - It warns that line breaks may change.
   - It records the substitution once per family in the migration report (EW-IMP-006) and in the PDF XMP metadata
     (FR-IMP-020, FR-OUT-009).
5. **System fonts are the user's choice and responsibility.** A user may select an installed system font, for
   example their own licensed Tahoma.
   - The export then states "output depends on system font X".
   - The byte-identity guarantee no longer holds for that project.
   - The as-built archive records the font by name and hash, without embedding the font file beyond the PDF subset.
6. **Pin font versions.** A later EW release that updates a bundled font ships it as a new font version. Projects
   keep the version their derivation profile names (ADR-0003), so line breaks do not shift in reopened projects.

## Consequences

If the recommendation is accepted:

**Positive**
- Fonts are identical on screen and in the PDF on every platform and in CI containers, and no licence question
  arises when PDFs are sent to customers.
- The Latin, Greek and Cyrillic coverage is enough for NL, EN, DE and FR project texts. The Noto family offers
  matching CJK and RTL fallbacks later.
- Users who want an ISO 3098 drawing style get one without third-party installs.

**Negative**
- **Reflow on import.** Imported QET pages may reflow slightly, because Noto Sans metrics differ from Tahoma's. Long
  function texts may wrap differently. The migration report lists the substitution, and the migration verifier's
  overlay (FR-IMP-021) shows the differences page by page.
- **Small-size legibility.** Legibility at 5–6 pt and below must be verified on paper. The minimum printed-height
  rule (FR-CHK-023) flags texts below the configured minimum at print scale (D-31).
- **Installer size.** Installers grow with each bundled family, and CJK fallbacks are large (1.x, possibly an
  optional download through a user action).
- **Version pinning.** Bundled font files must be kept and their versions retained across releases.

## Alternatives considered

| Alternative | Assessment |
|---|---|
| **DejaVu Sans as the default** | Good coverage, but the Bitstream Vera licence is not on the NFR-LIC-04 font allowlist (OFL-1.1). There is no advantage over Noto Sans that would justify widening it |
| **osifont as the default** | ISO 3098 look, but less familiar to the pilot's customers, who are used to sans-serif drawings. Its GPL-with-font-exception licence needs more explanation than OFL. Kept as an option |
| **Shipping a Tahoma-metric clone** (WineTahoma) | Metric compatibility is unverified, so the promise "looks like your QET drawings" could fail silently. The Tahoma name invites confusion with the original |
| **System fonts by default** | Breaks NFR-REL-11 (export without system fonts) and byte identity across platforms. Results would depend on what each workstation has installed |
| **Bundling Tahoma** | Not permitted (NFR-LIC-06) |

**Decision needed from:** the project owner (pilot user; PQ-07: keep the Tahoma look with a self-installed font, or
switch to Noto Sans or osifont). It is due before spine lane 2 builds the text stack, frames and goldens in weeks
3–5 ([vision](../design/00-vision-and-scope.md) OQ9).

## Related

- **ADRs:** [ADR-0003](0003-freshness-contract.md), [ADR-0015](0015-display-list-renderers-pdf.md), [ADR-0021](0021-importers-emit-format-level-records.md),
  [ADR-0024](0024-licence-set-and-permissive-gpl-boundary.md) (bundled-asset licences),
  [ADR-0027](0027-native-symbol-grid-and-qet-grid-policy.md).
- **Design:** [04 architecture](../design/04-architecture.md) §8.3; [09 reports, forms and output](../design/09-reports-forms-output.md)
  §2.8, §4.3; [14 interop](../design/14-interop.md) §4.10; [19 open-source governance](../design/19-open-source-governance.md)
  §2.1 (L14); [20 golden project](../design/20-golden-project.md) §17.5;
  [gaps-and-corrections](../research/gaps-and-corrections.md) G14.
- **Requirements:** FR-SYM-013, FR-OUT-008, FR-OUT-009, FR-IMP-020, FR-IMP-033, FR-CHK-023, FR-I18N-016; NFR-REL-07,
  NFR-REL-11, NFR-LIC-04, NFR-LIC-06, NFR-LIC-08, NFR-PLAT-06.
