# ADR-0005: Values: i64 nm coordinates, rational grids, `Dec3` quantities, `MlText`

**Status:** Accepted · amended 2026-10-18 · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the architecture judge panel)

## Context

**Coordinates.** The research notes disagreed (C-12 in [gaps and corrections](../research/gaps-and-corrections.md)):
µm integers (rust-ecosystem), f64 mm quantised to 0.1 mm (panel-manufacturing) and QET pixels at 96 dpi
(interop-formats). The migration path makes the choice concrete:
- QET scene units are 1 px = 25.4/96 mm = 0.264583… mm. The wiring grid is 10 px = 2.645833… mm (G4). Neither µm nor
  nm represents this exactly.
- The reference xrefs ("102-A1", "103-G6", "101-H16") are cell lookups on a QET frame grid. Any rounding drift can
  move a symbol across a cell boundary.
- A signed 32-bit nm range is ±2.147 m, too small for bayed cabinet suites in 3D.
- Floats in files produce spurious diffs and merge conflicts, and output that differs between platforms
  (NFR-REL-07).

**Electrical quantities.** The reference uses 0.75 mm², 1.5 mm², 2.5 mm², 6 A, 10 A, 230 V and 24 V. Summing load
currents per MICO channel or comparing cross-sections in f64 invites 5.9999… results and unstable check findings.
The text layer also shows "50hz" and "0.75 mm ²" (D-18): units stored as free text.

**Multilingual text.** EPLAN's `MultiLangString` is either language-neutral or a set of language strings: adding a
language removes the neutral string and vice versa (G7). EDZ serialises it as `de_DE@text;en_US@text;`. Identifiers
(-101F1, 51E-F13GCQP-D+H2GG, Q0.1, TA05-V-15) must never be translated, and a Cyrillic "А" in a tag must not pass as
a Latin "A".

## Decision

All value types live in the permissive crates `ew-units` and `ew-mltext`.

| Type | Definition | Rules |
|---|---|---|
| `Nm(i64)` | Linear coordinate in nanometres | The only linear unit in the model and in files: page coordinates, symbol geometry, 3D translations, wire and cable lengths. Range ±9.2 × 10⁹ m |
| `Grid { num, den }` | Rational grid step in nm | QET 10 px = `7_937_500/3` nm. Snapping uses the grid index `k`; the stored coordinate is `round(k × num / den)`, an error of at most 0.5 nm. Cell lookups use integer arithmetic on the frame's rational grid (FR-PAG-020). The native connection grid is ADR-0027's decision |
| `Orient2` | Eight variants: four rotations, each optionally mirrored (FR-SYM-004) | The orientation of every symbol representation; no float angles in 2D. Free texts and graphics at other angles use `Angle2` (amendment 1) |
| `Pose3` | Three `Nm` plus a unit quaternion | Quaternion components stored as `i64` in units of 10⁻⁹. f64 mm (glam `DVec3`) only in 3D maths, f32 only in GPU buffers; neither is persisted |
| `Dec3` | Fixed point, three decimals (`i64` thousandths), written as a decimal string ("0.75") | Cross-sections, currents, voltages, frequencies, pressures, temperatures, losses and part quantities. `Quantity { value: Dec3, unit }` carries its unit; AWG and kcmil stay in their original unit (FR-CON-032) |
| `MlText` | `Neutral(String) \| Langs(BTreeMap<LangTag, String>)` | EPLAN's mutual exclusion; BCP 47 `LangTag` (`nl-NL`, `de-DE`, `en-GB`). The display language is a query key |
| Identifiers | NFC plain strings | Tags, part numbers, PLC addresses, wire numbers, process tags and terminal designations are NFC-normalised on input, letter codes restricted to A–Z, confusables and mixed scripts flagged (unicode-security), natural-sort keys via ICU collation. **Never translatable** |

Rendering ("0.75 mm²", "6 A", a decimal comma in nl and de) is a query over the output language and project
settings. Rendered text is never stored.

## Amendments

Accepted on 2026-10-18. The decision text above is unchanged; these entries refine it.

1. **Integer angle for free texts and graphics** (2026-10-18; [domain model](../design/03-domain-model.md) §3.1,
   [interop](../design/14-interop.md)). QET free texts keep their source rotation, and DXF `INSERT` and `TEXT`
   entities may be rotated by any angle; `Orient2` holds neither. `ew-units` therefore adds `Angle2(i32)`, an angle
   in units of 10⁻³ degree, used **only** by free texts and graphics. Symbol representations keep `Orient2`. The
   trigonometry runs in `ew-dl`'s deterministic helpers. The JSON encoding is the integer, and the default 0 is
   omitted. The type exists before the QET importer's free-text pass lands.
2. **Rotation sense of `Angle2`** (2026-10-18, project owner (delegated, 2026-10-18); [domain model](../design/03-domain-model.md)
   §3.1, [interop](../design/14-interop.md) §4.3, §4.13, §6.1). 0 points along +x (3 o'clock), and a positive
   `Angle2` rotates **counter-clockwise as seen by the viewer on the sheet**, although sheet y points down: a point
   (x, y) rotated by θ becomes (x cos θ + y sin θ, −x sin θ + y cos θ). This is QET's arc convention ("positive span
   counter-clockwise on screen") and leaves DXF angle values unchanged after the importer's y flip. QET free-text
   and static-text rotations follow Qt's clockwise-positive convention and are negated by the importer. `Orient2`
   keeps its clockwise Rn (QET element `orientation`), so R90 corresponds to `Angle2(270_000)`. `ew-dl`'s
   deterministic helpers implement the rotation; `ew-units` only stores the value.
3. **Natural order without ICU** (2026-10-18, project owner (delegated, 2026-10-18);
   [architecture](../design/04-architecture.md) §3, [02b](../design/02b-requirements-extended.md) FR-I18N-010).
   `ew-mltext` implements a *specified, deterministic natural order* instead of ICU collation. **Primary level:** NFD
   with accents stripped, lower-cased; ASCII digit runs compare by value; character classes order white space <
   punctuation < symbols < digits < letters (the Unicode collation root order). **Secondary level:** accents and
   leading zeros. **Tertiary level:** case, upper case first. Keys are byte-comparable, and equal keys mean equal NFC
   text. *Why:* `icu` 2.3.1 declares `rust-version` 1.88, above the permissive crates' MSRV 1.85 (NFR-MAINT-03); it
   pulls about twenty crates into a format crate that third parties use to read and write files; and ICU table
   updates would silently reorder sort keys, whereas a specified order is stable across EW versions and platforms
   (NFR-I18N-03, NFR-REL-07). ICU stays available for language-sensitive *display* sorting in the GPL query layer
   (`ew-derive` already lists icu). The `ew-mltext` row of the architecture drops icu from its dependencies.

## Consequences

### Positive

- Imported QET geometry stays exactly on its grid, so "102-A1"-style cell lookups and xrefs are exact.
- Files never carry floats, so saves are byte-stable and outputs identical across Windows, macOS and Linux.
- Check findings on cross-sections, currents and voltages are deterministic.
- "0.75 mm ²" and "50hz" cannot be stored, because units are typed.
- "PDF per language" is the same derivation with another key, and EPLAN/EDZ multilingual texts import and export
  losslessly.

### Negative

- Boundary conversions (kurbo f64 geometry, GPU vertices, 3D maths) must round consistently. `ew-units` owns the
  rounding helpers and their property tests.
- Rational snapping needs overflow guards on `k × num`; page sizes keep it far from the i64 limit.
- `Dec3` resolves 0.001 of its unit. A quantity that needs finer resolution must use a smaller unit or a new
  fixed-point type introduced by an ADR.
- Mixed-grid projects (QET pages next to native pages) need a visible grid indicator per page (FR-UI-018).
- Every text field must be classified as translatable (`MlText`) or identifier, once, in `ew-schema`.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **µm integers** | QET px is 264.583… µm, not representable; imported geometry would drift off its grid |
| **f64 mm quantised to 0.1 mm on save** | Too coarse for symbol geometry, text baselines and arc control points; floats remain in memory comparisons |
| **f64 in files** | Float noise in diffs and merges; platform-dependent output |
| **i32 nm** | ±2.147 m is too small for bayed cabinets in 3D |
| **Floats for electrical values** | Non-deterministic sums and comparisons in checks and reports |
| **Plain strings plus a separate translation table** | Loses EPLAN's neutral-or-languages semantics; translations drift from their source |

## Related

- **ADRs:** [ADR-0001](0001-core-model-persistent-maps.md), [ADR-0010](0010-project-storage-jsonl-directory.md),
  [ADR-0015](0015-display-list-renderers-pdf.md), [ADR-0021](0021-importers-emit-format-level-records.md), [ADR-0027](0027-native-symbol-grid-and-qet-grid-policy.md).
- **Design:** [architecture](../design/04-architecture.md) §3, §4.5; [domain model](../design/03-domain-model.md) §3;
  [project format](../design/05-project-format-and-persistence.md) §3.2; research G4 and G7 in
  [gaps and corrections](../research/gaps-and-corrections.md).
- **Requirements:** FR-PAG-020, FR-PAG-021, FR-PAG-022, FR-IMP-008, FR-CON-019, FR-CON-032, FR-PRT-026,
  FR-NUM-016, FR-I18N-001, FR-I18N-002, FR-I18N-004, FR-I18N-005, FR-I18N-008, FR-I18N-009, FR-I18N-010,
  FR-I18N-011, FR-SYM-004, FR-UI-018, NFR-I18N-03, NFR-I18N-04, NFR-REL-07.
