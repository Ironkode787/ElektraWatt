# Checks, revisions and collaboration: the rule catalogue

A chapter of [Checks, revisions and collaboration](15-checks-revisions-collaboration.md), whose table says where each section lives.

---

## 4. Rule catalogue

This is the catalogue of the rules not built yet: every rule of every pack, the import findings and the library
lint that the rule registry of `ew-checks` does not hold, one row per ID. The owning documents in the Def. column
specify the algorithms, examples and quick fixes; IDs, kinds, modes, default severities and scopes are fixed here
until the rule is built. A built rule's row leaves this catalogue: its ID, title, kind, modes, severities, scope and
release are the registry's, and the rule reference that `cargo xtask gen` writes from the registry (one page per
rule, with its notes) prints them; the rule's doc comment names its
requirement and owning document.

**The EW-POT pack.** Every order, text, target and fingerprint follows printed keys, never the IDs of a new project.
Findings of a defined potential target every definition and are keyed on the potential's identity; findings of a
potential without a definition target every member and are keyed on its discriminator member alone, the first member
CP by printed text (device CPs before interruption points, then `<tag>:<designation>` in natural order, then the CP
index), so a waiver survives adding a member. A wire note of a bond closure is reported once, by the potential of its
subject that prints first. The bonds and the potential are read as their own parts of the partition (`BondsOf`).
When a finding has several primary targets, `finding_node` breaks the tie on the printed label.

**Columns.**
- **Kind** (§2.3): `safety`, `conflict`, `completeness`, `hygiene`; `integrity` for findings about the stored
  project, the check run and the release workflow (never waivable, never pending); `import` for import-run
  findings, which are acknowledged rather than waived; `structural` for guarantees of the schema or an invariant,
  listed for traceability, which never produce a finding.
- **Modes:** o online, f offline, p prevent, r release, x export (§3.1); g release gate only (§3.2); n integrity
  run (on open, after merges and `ew check --integrity`); i import run (the migration report of one import);
  l library linter (§4.11). A rule runs only in the modes it lists, so a rule without r reports nothing in
  `pre-delivery`, except that release mode reports every violation of a prevent rule (§3.5).
- **Severity:** `a/b` is severity / release severity (§3.2). Schemes may override both (§2.4).
- **Scope:** the memo key of §2.3, not the finding granularity.
- **Req.:** the requirement the rule implements; a research source follows in brackets (STD =
  [standards](../research/standards.md), PMF = [panel-manufacturing](../research/panel-manufacturing.md), G15 =
  [gaps-and-corrections](../research/gaps-and-corrections.md) G15).
- **D (defects):** the defects of the reference drawing set the rule detects, then the expected findings (EF-nn,
  MF-nn) of the retired reference project (see the [glossary](glossary.md)).
- **Def.:** the document that specifies the rule's content; "15" is this document.

### 4.1 Packs

| Pack | Families | Classes | MVP | 1.0 | 1.x |
|---|---|---|---|---|---|
| EW-CORE | EW-CORE | 003, 100–105 | all except the rows marked 1.0 | complete | — |
| EW-IEC81346 | EW-RDS, EW-NUM | 017 | complete | — | — |
| EW-TERM | EW-TERM | 001 | complete | — | — |
| EW-POT | EW-POT | 005 | all except EW-POT-007, -010, -018, -019 | complete | EW-POT-020 |
| EW-PLC | EW-PLC, EW-SAF, EW-PRC | 110 | EW-PLC-001, -002, -003, -007, -008, -012, -015 | complete | — |
| EW-BUS | EW-BUS, EW-IOL | 111 | EW-BUS-001, -004, -011 | complete | EW-BUS-008 |
| EW-FL | EW-FL | 120 | EW-FL-001, -002, -004, -011, -013 | complete | EW-FL-003 |
| EW-LAYOUT | EW-LAY | 130 | EW-LAY-001 … 004, 006 (2D), 010, 016 | complete | EW-LAY-013 |
| EW-THM | EW-THM | 131 | — | complete | — |
| EW-MFG | EW-MFG | 140 | — | complete | EW-MFG-011 |
| EW-EN60204 | EW-EN60204 | 150 | — | complete, opt-in | — |
| EW-REL | EW-REL | 190 | EW-REL-001, -002, -004 | complete | — |
| importers | EW-IMP | 160 | 001 … 011, 101 … 123, 301 … 305 | 201 … 207, 251 … 257 | — |
| EW-LIB | EW-LIB | 170 | — | complete | — |

FR-CHK-004 (MVP packs) and FR-CHK-005 (domain packs: their MVP subsets at MVP, complete at 1.0) are satisfied by
this table. EW-LIB is not a project pack: the library linter runs it (§4.11), and no project scheme enables it.

`RuleMeta.release` (`Release::Mvp`, `Release::V1_0`) is a built rule's release column and gates nothing: a rule
that is built runs in every shipped scheme that its modes allow and that does not switch it off.

### 4.2 EW-CORE

**Integrity (class 100).** The findings of the integrity check of [05](05-project-format-and-persistence.md) §6.3
are EW-CORE-001 … 019, with the detection and quick fixes given there. They run on open, after merges and in
`ew check --integrity` (FR-CHK-012, MVP), including the "renumber the newer" fix of EW-CORE-013 (§6.4);
cross-file reconciliation after merges is FR-COL-016 (1.0).
- **Blocking release:** EW-CORE-002, -003, -005 … -009, -011, -012, -013, -015, -017, -018 and -019; EW-CORE-016 (unknown
  data from a newer format) only where outputs are affected.
- **Not blocking:** EW-CORE-001, -004, -010, -014, and EW-CORE-016 where no output is affected.
- **Not waivable:** every `integrity` row (§2.3). A dangling reference or a conflict record is repaired, never
  accepted.
- **Read-only:** EW-CORE-002, -003, -005, -006 and -019 open the project read-only (05 §6.3).
- **Not rules:** the reader's notices for line endings, foreign edits, non-canonical files and a truncated finding
  list carry no rule ID; the integrity block 001–019 is full (05 §6.3).

**Pages, cables, devices, references, texts and the rule system.**

| ID | Name | Checks | Kind | Modes | Severity | Scope | Rel | Req. | D (defects) | Def. |
|---|---|---|---|---|---|---|---|---|---|---|
| EW-CORE-024 | Required report block missing | A report block that the declared applicable standards require is missing: the EN 60204-1 clause 17 set (through EW-EN60204-015) or the IEC 61439-1 clause 11 routine-verification checklist; off when no standard is declared | completeness | o r | warning/error | project | 1.0 | FR-CHK-054, FR-MFG-036, FR-MFG-037 | D-09 | 15 |
| EW-CORE-035 | Core on the wrong pin | A pre-assembled cable core on a pin other than the part's pin map | conflict | o r | warning | cable | MVP | FR-CHK-056 | D-17 | 08 §8.1 |
| EW-CORE-046 | Overview differs from children | An overview or hand-drawn assembly representation differs from the assembly's children | conflict | o r | error | device | MVP | FR-CHK-022 | D-10; MF-14 | 15 |
| EW-CORE-049 | Hand-drawn part list differs | A hand-drawn part-list page kept as graphics differs from the derived BOM; differences seen only at import are EW-IMP-113 | hygiene | f r | info | page | MVP | FR-IMP-003 | D-01, D-04 | 15 |
| EW-CORE-054 | Overview without detail link | An overview representation without a detail link | hygiene | o r | warning | device | MVP | FR-CHK-060 (STD XR-04) | D-10 | 15 |
| EW-CORE-055 | Unplaced contacts | Contacts of the main function's part neither placed nor marked unused; off in `pre-delivery` | completeness | o | info | device | 1.0 | FR-CHK-061 (STD XR-03) | — | 15 |
| EW-CORE-056 | External reference without target | An external reference to a project that is open but has no matching target | completeness | o | info | project | 1.0 | FR-CHK-061 (G15) | — | 15 |
| EW-CORE-085 | Variant not in the part | A part use names a variant key its part does not hold; the lists print the part | conflict | o r | warning | device | 1.0 | FR-PRT-003 | — | 07 §2.5 |
| EW-CORE-062 | Text overflow | Text overflows a title-block cell or report column after shrink-to-fit, per display language | hygiene | o r | warning | page | 1.0 | FR-I18N-012, FR-REP-039 | — | 09 §7 |
| EW-CORE-064 | *Retired* | Font substitution at import is the import finding EW-IMP-006 | — | — | — | — | — | — | — | — |
| EW-CORE-066 | Glyph missing | A character that no bundled face drew (it prints as `.notdef`); a character drawn by the bundled symbol fallback face is not a finding | hygiene | o x r | warning/error | page | MVP | FR-OUT-027 | — | 09 §7 |
| EW-CORE-082 | Waiver expired | A waiver past its expiry date or release | hygiene | o r | warning | project | 1.0 | FR-CHK-049 | — | 15 |

EW-CORE-047 implements the first half of the rule on unplaced devices (FR-REV-007): a device without any
schematic representation always blocks release. The layout half is EW-LAY-001, which counts only where a layout
space exists for the device's location.

### 4.3 EW-IEC81346: designations and numbering

The IDs follow the RD-nn check list of [standards](../research/standards.md) §1.6, so EW-RDS-00n is RD-0n. The
structural rows keep their IDs so that the RD-nn traceability stays visible in the rule documentation.

| ID | Name | Checks | Kind | Modes | Severity | Scope | Rel | Req. | D (defects) | Def. |
|---|---|---|---|---|---|---|---|---|---|---|
| EW-RDS-005 | *Structural:* placements reference devices | Layout objects reference devices (EW-LAY-002) | structural | — | — | — | MVP | FR-CHK-039 | D-02 | 15 |
| EW-RDS-006 | *Structural:* one displayed form | The displayed designation form is one project setting | structural | — | — | — | MVP | FR-NUM-002 | D-03 | 15 |
| EW-RDS-008 | *Structural:* renumbering is one transaction | Renumbering is one transaction (ADR-0008) | structural | — | — | — | MVP | FR-NUM-008 | D-01 | 15 |

### 4.4 EW-TERM and EW-POT

[08](08-connections-terminals-cables.md) §8 specifies these rules and their algorithms; the safety rules
EW-POT-013, -006, -007 and -009 are detailed in §8.2 there. EW-POT-018 and EW-POT-020 are specified here.

| ID | Name | Checks | Kind | Modes | Severity | Scope | Rel | Req. | D (defects) | Def. |
|---|---|---|---|---|---|---|---|---|---|---|
| EW-POT-007 | PE continuity | Every PE-typed CP reaches the main PE point through wires, bridges, rails and carriers | safety | f r | error | project | 1.0 | FR-CHK-029 | D-21; EF-23 | 08 §8.2 |
| EW-POT-010 | Clamp capacity exceeded | More conductors on a CP than its part's clamp takes (`max_conductors`; a twin ferrule's two wires are one conductor, [13](13-manufacturing-outputs.md) §4.3), or a wire whose size in mm² lies outside the clamp's range (`csa_min`, `csa_max`); a CP whose part names neither is not judged | safety | o r | error | device | 1.0 | FR-CHK-021 (STD M-08) | — | 08 §8.1 |
| EW-POT-018 | Load exceeds rating | Summed load currents (part data × simultaneity) exceed the rating of the feeding source, protective device or electronic-breaker channel; off when no load data exists | safety | f r | warning | potential | 1.0 | FR-CHK-041 (G15) | D-25; EF-24 | 15 |
| EW-POT-019 | Mixed cross-section units | Cross-section units mixed within one potential (mm², AWG, kcmil) | hygiene | o | warning | potential | 1.0 | FR-CON-032 | — | 08 §8.1 |
| EW-POT-020 | Voltage drop | The voltage drop of a cable from its length, core cross-section and the load current of its potential exceeds the profile's threshold; the threshold table ships unverified | conflict | f r | warning | cable | 1.x | FR-CHK-059 (G15) | — | 15, 08 §8.1 |

EW-POT-018 complements EW-PLC-014, which covers module outputs: EW-POT-018 covers PSU and protective-device loads
such as -102G1 at 20 A and the MICO channels of -102F2 at 6 A. EW-POT-020 needs routed lengths and load currents,
both 1.0, so it follows at 1.x (Could).

### 4.5 Domain packs

#### 4.5.1 EW-PLC

| ID | Name | Checks | Kind | Modes | Severity | Scope | Rel | Req. | D (defects) | Def. |
|---|---|---|---|---|---|---|---|---|---|---|
| EW-PLC-006 | *Retired* | Spare channels are listed by reports and counts (FR-PLC-019), not as findings | — | — | — | — | — | — | — | 10 §3.10 |

The seven MVP rules run in the scopes `module` and `cfg` with en-GB and nl-NL messages and rule pages; EW-PLC-001 and EW-PLC-007 report online and in release mode, since they carry the prevent letter and release mode runs prevent-letter rules; EW-PLC-008 is a warning online (its export severity); EW-PLC-012 has a command quick fix and the unverified-data marker, and the window offers its part's PLC section ([10](10-plc-and-bus.md) §3.10); EW-PLC-015 supersedes EW-CORE-044 inside a configuration project. Two channels of one module with one number: EW-CORE-008 with the discriminator `duplicate_number`.

EW-PLC-001 reports one finding per overlap group of `address_index(cfg)` (each connected run of overlapping addresses in one area, so N channels on one address are one finding, not N(N−1)/2), every member channel a target and the first in address order the subject; the text names five channels with their addresses, then "and N more channels", in en-GB and nl-NL.

EW-PLC-016 and EW-PRC-002 are in the 1.0 content, with en-GB and nl-NL messages and rule notes. EW-PRC-002's findings are the open instrument tags; it is not a row of the open-item table, whose rules run in release mode of the shipped scheme.

The pack's 1.0 content: EW-PLC-004, -005, -009, -010, -011, -013 and -014, EW-SAF-001 … -003 and EW-PRC-001, with the kinds, modes, severities and scopes of the rule reference, en-GB and nl-NL messages and rule notes ([10](10-plc-and-bus.md) §3.10 has the decisions). The scope `station` exists for EW-PLC-011, -013 and -014 (§2.3). The prevent halves of EW-PLC-001, -007 and -011 refuse an edit that adds an error finding in the scopes it touches; EW-PLC-013 supersedes EW-POT-003 for the FE pins of a station's devices. EW-PLC-016 and EW-PRC-002 read the exchange stamp and the instrument list (above).

#### 4.5.2 EW-BUS

| ID | Name | Checks | Kind | Modes | Severity | Scope | Rel | Req. | D (defects) | Def. |
|---|---|---|---|---|---|---|---|---|---|---|
| EW-BUS-008 | E-bus budget | The EtherCAT E-bus current budget is exceeded | conflict | o r | error | station | 1.x | FR-BUS-020 | — | 10 §3.10 |

EW-BUS-011 (a part's own bus symbol out of step with its network ports, [10](10-bus.md) §2.7) runs in the scope `project`, once per part, with the quick fix Open the bus symbol.

The pack's 1.0 content: EW-BUS-002, -003, -005, -006, -007, -009 and -010 and EW-IOL-001 and -002, with en-GB and nl-NL messages and rule notes; the ranges, lengths and station counts they read are unverified data of the rules ([10](10-plc-and-bus.md) §3.10). EW-BUS-008 stays 1.x.

#### 4.5.3 EW-FL

| ID | Name | Checks | Kind | Modes | Severity | Scope | Rel | Req. | D (defects) | Def. |
|---|---|---|---|---|---|---|---|---|---|---|
| EW-FL-003 | Order-code occupancy | Slot occupancy contradicts the configurator order code (decoder plugin) | conflict | f r | warning | device | 1.x | FR-FLU-022 | D-40 | 11 §6 |

EW-FL-004 runs in modes `o r`, so
that the pre-delivery release run counts EF-14 (O2); the MVP rules EW-FL-001, -002, -004, -011
and -013 are built.

#### 4.5.4 EW-LAYOUT

| ID | Name | Checks | Kind | Modes | Severity | Scope | Rel | Req. | D (defects) | Def. |
|---|---|---|---|---|---|---|---|---|---|---|
| EW-LAY-002 | *Structural:* placement without device | Placements reference devices; deleting the device cascades; on lenient import an unmatched rectangle becomes a layout graphic, never a placement | structural | p | — | — | MVP | FR-LAY-017 | D-02 | 12 §4 |
| EW-LAY-005 | CP pattern incomplete | The CP3D pattern does not cover the device's physical schematic CPs after the macro mapping table; blocks routing of the affected wires | completeness | o r | error | space | 1.0 | FR-LAY-027 | — | 12 §4 |
| EW-LAY-007 | Clearance violated | A footprint or a duct inside the zone a device's part keeps free, with the side and the amount | conflict | o r | warning | space | 1.0 | FR-LAY-029 | — | 12 §7 |
| EW-LAY-008 | Duct fill over limit | A duct over the shop's fill limit where it is fullest, on the stored ways | conflict | o r | warning | carrier | 1.0 | FR-LAY-049 | — | 12 §8.6 |
| EW-LAY-011 | Stale way | Ways that no longer hold, one finding per moved device or changed duct with its wires as targets; supersedes EW-REL-002 for ways and their lengths | completeness | o r | warning/error | space | 1.0 | FR-LAY-046 | D-19 | 12 §8.5 |
| EW-LAY-012 | Holes too close | Two holes of a surface closer than the workshop's web between holes, a hole closer to the sheet's edge than its edge margin and that web, or a hole off the sheet; it names both holes by their owners | conflict | o r | warning | surface | 0.4.0 | FR-LAY-057 | — | 12 §4 |
| EW-LAY-013 | Door-swing collision | A door-mounted solid collides during the swing, reported with the angle | conflict | f r | error | space | 1.x | FR-LAY-031 | — | 12 §4 |
| EW-LAY-014 | Pre-assembled cable too short | A route longer than the cable length minus its connector allowances | conflict | f r | error | space | 1.0 | FR-LAY-050 | D-19 | 12 §4 |
| EW-LAY-015 | Segregation class violated | A wire in a duct whose class set excludes its segregation class (manual or pinned paths) | conflict | o r | warning or error (policy) | carrier | 1.0 | FR-LAY-048 | — | 12 §4 |
| EW-LAY-017 | Holes not known | A part placed in a cut-out or screwed to a surface has no holes, or a device placed there has no part, so the surface's hole table and drawings lack its holes; one finding per part, naming its devices; Edit part… opens Parts at the part's Holes, Choose part… for a device without one | completeness | o r | warning | surface | 0.4.0 | FR-LAY-057 | — | 12 §4 |
| EW-LAY-018 | Cable gland | A cable that leaves the cabinet has no gland placed; its gland is not known (no outer diameter, no stock gland seals it); a placed gland's hole differs from its gland's; a gland hole serves no cable any more | completeness | o r | warning; error at release for a gland not placed | space | 0.4.0 | FR-LAY-056 | — | 12 §4 |
| EW-LAY-021 | Thermal zoning | Placement rule: a device outside the thermal zone its part allows | hygiene | o r | warning | space | 1.0 | FR-LAY-033 | — | 12 §4 |
| EW-LAY-022 | EMC segregation distance | Placement rule: segregation distance between EMC classes below the policy value | hygiene | o r | warning | space | 1.0 | FR-LAY-033 | — | 12 §4 |
| EW-LAY-023 | Operator height | Placement rule: an operator device outside the operator heights of the installation | hygiene | o r | warning | surface | 1.0 | FR-LAY-033 | — | 12 §4 |
| EW-LAY-024 | Door mass | Placement rule: door-mounted mass above the door's rating | conflict | o r | warning | surface | 1.0 | FR-LAY-033 | — | 12 §4 |
| EW-LAY-025 | Cable-entry zone | Placement rule: a device inside the cable-entry zone | hygiene | o r | warning | surface | 1.0 | FR-LAY-033 | — | 12 §4 |
| EW-LAY-026 | Handle-axis alignment | Placement rule: handle and switch-body axes of a door-coupled rotary switch (S51) not aligned within the part's shaft-length range | conflict | o r | warning | space | 1.0 | FR-LAY-034 | — | 12 §4 |

EW-LAY-021 … 026 are Could rows of FR-LAY-033 and FR-LAY-034; their constants ship as data with a `verified` flag.
EW-LAY-009 (wire unroutable) and EW-LAY-020 (depth containment) are retired: a wire without a way is named by Route's
preview and its Properties and counted by EW-MFG-001, and what reaches the closed door is EW-LAY-006's
([12](12-panel-layout.md) §7, §8.2). At 1.0 EW-LAY-006 judges the closed door by number: a device or a duct of the
plate that reaches the door, and a device of the plate and one of the door that meet when it is closed, each finding
naming both things (12 §7).

The MVP rules EW-LAY-001, -003, -004,
-006 (2D), -010 and -016 ship in `ew-checks` (`rules/layout.rs`) with their catalogue rows, rule pages and en-GB and
nl-NL messages; EW-LAY-002 has no rule run (the structural invariant). **Scope:** the engine has no scope
space, surface or carrier; the rules run at the **project** scope until
checks can be scoped to a space or a surface, enumerate the layout objects through the layout queries, and target the
object each finding names (the device, placement, carrier, layout space or enclosure), shown on the panel page whose
surface view shows it; the rule pages say project. EW-LAY-001's "counts only where that location has
a layout space" is a filter in the rule's run, so its release severity error applies to what it reports. EW-LAY-006
lists at most 64 overlap findings per surface plus one summary finding on the surface, by a sweep of O(n log n + k);
an overlap is a positive-area intersection, and a rectangle without area never overlaps. EW-LAY-006
has no offline mode: the 3D half is 1.0. AC-21's compared subset admits EW-LAYOUT (EF-05).

#### 4.5.5 EW-MFG

EW-MFG-004, 008 and 009 judge the project before a print, online and in release mode; EW-MFG-001 judges it where a
length can be computed: online and in release mode in a project with a panel layout, an info in both, so a wire
without a length stops no release; the check before an export for the workshop is the release check
([13](13-manufacturing-outputs.md) §8.6), which names such wires. EW-MFG-002, 003, 005, 006 and 010 are retired with
the `manufacturing` scheme: no machine file, export mapping or bundle is built that would read them, and an unverified
table says so where its value is shown.

| ID | Name | Checks | Kind | Modes | Severity | Scope | Rel | Req. | D (defects) | Def. |
|---|---|---|---|---|---|---|---|---|---|---|
| EW-MFG-001 | Wires without a length | One finding per cabinet while a wire of it that is no core and no field wire has no length (the wire list prints —), its targets the layout space and those wires, its fingerprint the space's, Route its quick fix; only in a project that has a panel layout, none in a project without a layout; a stale length is EW-LAY-011's; supersedes EW-REL-002 for lengths | completeness | o r | info | project | 1.0 | FR-MFG-024, FR-MFG-051, FR-CHK-058 | D-19 | 13 §4.6 |
| EW-MFG-004 | Marker text does not fit | A marker text that does not fit its stock after the shortening of 13 §5.2 (characters per line, lines) | conflict | o r | warning | potential | 1.0 | FR-MFG-011, FR-MFG-024 | — | 13 §4.6 |
| EW-MFG-008 | End cannot be made | A wire end the rule table cannot decide: a fine-stranded wire into an insulation-displacement clamp | completeness | o r | warning | potential | 1.0 | FR-MFG-005, FR-CHK-058 | — | 13 §4.6 |
| EW-MFG-009 | End set by hand does not fit | An end set by hand whose ferrule part fits another size, a twin ferrule with no second wire on its clamp, or a ferrule of another collar colour than the workshop stock's ferrule of that size | conflict | o r | warning | potential | 1.0 | FR-MFG-007, FR-MFG-042 | — | 13 §4.6 |
| EW-MFG-011 | Feedback length deviation | A machine feedback length that deviates from the computed length beyond tolerance | hygiene | f | info | project | 1.x | FR-MFG-026 | — | 13 §4.6 |
| EW-MFG-020 | Strip without an end plate | A strip whose terminals have parts and after whose last terminal no end plate stands: none of its parts names one, it was left off, none was added | completeness | o r | warning | device | 1.0 | FR-MFG-016 | — | 13 §6.3 |
| EW-MFG-021 | Bridge without a part | A bridge group whose carrier connection names no part, so the parts list and the assembly list cannot name it | completeness | o r | warning (in release too: a project made before a bridge could have a part releases as it did) | device | 1.0 | FR-MFG-016 | — | 13 §6.3 |
| EW-MFG-022 | Wire part does not match the wire | A wire part whose section gives another cross-section or colour than the wire's effective one | conflict | o r | warning | potential | 1.0 | FR-PRT-035 | D-08 | 07 §2.5 |

### 4.6 EW-THM: IEC 61439 thermal (1.0)

| ID | Name | Checks | Kind | Modes | Severity | Scope | Rel | Req. | D (defects) | Def. |
|---|---|---|---|---|---|---|---|---|---|---|
| EW-THM-001 | Ambient limit exceeded | A device ambient limit below the predicted air temperature at its mounting height | safety | f r | error | space | 1.0 | FR-THM-006 | — | 12 §11.8 |
| EW-THM-002 | Climate capacity insufficient | Required cooling or heating not covered by the placed climate devices | safety | f r | error | space | 1.0 | FR-THM-007, FR-THM-012 | — | 12 §11.8 |
| EW-THM-003 | Loss data missing | A placed device without loss data; the result is marked incomplete | completeness | f r | warning | space | 1.0 | FR-THM-001, FR-THM-012 | — | 12 §11.8 |
| EW-THM-004 | Unverified coefficients | Unverified or test coefficients used in a release | hygiene | r | warning | space | 1.0 | FR-THM-012, NFR-COMP-05 | — | 12 §11.8 |
| EW-THM-005 | Thermal result stale | A thermal result (Class II) stale at release; supersedes EW-REL-002 for thermal results | completeness | r | error | space | 1.0 | FR-MFG-025 | — | 12 §11.8 |
| EW-THM-006 | Single-compartment conditions | Conditions of IEC 61439-1 10.10.4.2 not met: a component loaded above 80 % of its free-air rating, or InA above 630 A | conflict | f r | warning | space | 1.0 | FR-THM-004 | — | 15 |
| EW-THM-007 | Method not applicable | An enclosure with horizontal partitions treated as one compartment; multi-compartment calculation is 1.x (FR-THM-010) | conflict | f r | warning | space | 1.0 | FR-THM-010 | — | 15 |

Every thermal finding and the thermal report carry the statement that the calculation does not replace verification
by a competent person (NFR-COMP-04).

### 4.7 EW-EN60204: electrical equipment of machines (1.0, opt-in)

EN 60204-1 applies to the electrical equipment of machines. A control cabinet such as the reference project may
instead be delivered as an assembly under IEC 61439-1/-2 (which applies is a pilot question), so the pack is
**not** in `pre-delivery`. It is enabled by the `machinery-en60204` scheme or by the project's applicable-standards
setting (FR-CHK-052). The numbers follow the M-01 … M-15 list of [standards](../research/standards.md) §7
(EN 60204-1:2018+A1:2025), so EW-EN60204-0nn implements M-nn; the values behind M-04, M-07 and M-13 are marked
**(verify)** there. Threshold tables ship as data with `verified = false` until a reviewed change confirms them
against a licensed copy (NFR-COMP-05). "Includes" means that the pack enables the base rule, adds the clause
reference to its finding, and creates no second finding; the row then takes the base rule's modes and severity.
The D column gives the expectation with the pack enabled.

| ID | Name | Checks | Kind | Modes | Severity | Scope | Rel | Req. | D (defects) | Def. |
|---|---|---|---|---|---|---|---|---|---|---|
| EW-EN60204-001 | Supply terminal designation | Incoming-supply terminals designated L1/L2/L3/N and the PE terminal marked "PE", at the supply's external reference | hygiene | o r | warning | project | 1.0 | FR-CHK-052 (STD M-01) | 1X1:1, 1X1:2 | 15 |
| EW-EN60204-002 | Supply-disconnecting device | Exactly one supply-disconnecting device per incoming supply, role from part data, lockable; letter Q or QB, not S | safety | o r | warning | project | 1.0 | FR-CHK-052 (STD M-02) | S51 (class S) | 15 |
| EW-EN60204-003 | Excepted circuits | Excepted circuits upstream of the disconnector identified (orange or marking) | safety | o r | warning | potential | 1.0 | FR-CHK-052 (STD M-03) | 0 | 15 |
| EW-EN60204-004 | PE sizing and bonding | Includes EW-POT-006 and EW-POT-007 | safety | as base | as base | potential | 1.0 | FR-CHK-020, FR-CHK-029 (STD M-04) | D-38, D-21; EF-06 | 15 |
| EW-EN60204-005 | Control transformer | AC control circuits fed through a transformer with separate windings (circuit class `control`) | safety | o r | warning | potential | 1.0 | FR-CHK-052 (STD M-05) | 0 | 15 |
| EW-EN60204-006 | Control voltage | Nominal control voltage within the limits | conflict | o r | warning | potential | 1.0 | FR-CHK-052 (STD M-06) | 0 | 15 |
| EW-EN60204-007 | Overcurrent coordination | I_b ≤ I_n ≤ I_z and minimum cross-sections, over the effective cross-sections of 08 §4; info when the installation method is missing | safety | f r | warning | potential | 1.0 | FR-CHK-052 (STD M-07) | ▲ info (installation method missing) | 15 |
| EW-EN60204-008 | Conductors per terminal | Includes EW-POT-010 | safety | as base | as base | device | 1.0 | FR-CHK-021 (STD M-08) | 0 | 15 |
| EW-EN60204-009 | GNYE only for PE | Includes EW-POT-004 | safety | as base | as base | potential | 1.0 | FR-CHK-031 (STD M-09) | 0 | 15 |
| EW-EN60204-010 | BU only for N | Includes EW-POT-005 | conflict | as base | as base | potential | 1.0 | FR-CHK-031 (STD M-10) | EF-11 | 15 |
| EW-EN60204-011 | Colour profile deviation | A colour profile that deviates from the recommended colours; reported once per profile, naming its declared basis | hygiene | o r | info | project | 1.0 | FR-CON-020 (STD M-11) | D-37 ("DBU/WH 24 V") | 15 |
| EW-EN60204-012 | Conductor identification | Every conductor identifiable at each termination (wire number, core identity or marking scheme) | completeness | o r | warning | potential | 1.0 | FR-MFG-010 (STD M-12) | 0 | 15 |
| EW-EN60204-013 | Nameplate data | Nameplate data complete in the project properties | completeness | o r | warning | project | 1.0 | FR-PRJ-025 (STD M-13) | ▲ (no short-circuit rating) | 15 |
| EW-EN60204-014 | *Structural:* same designation | Devices carry the same designation as the documentation | structural | — | — | — | 1.0 | FR-CHK-052 (STD M-14) | — | 15 |
| EW-EN60204-015 | Documentation set | Report blocks present: TOC, circuit, terminal and cable diagrams, wire list, parts list, layout, PLC overview, clause 18 verification checklist (FR-MFG-036); includes EW-CORE-024 | completeness | as base | as base | project | 1.0 | FR-MFG-036 (STD M-15) | D-09; ▲ (checklist block absent) | 15 |

### 4.8 EW-REL: release gate

EW-REL rules run only in the gate (§3.2) and are never waivable. Offline and SQL rules cannot be stale there: the
gate evaluates every rule cold on a fork at the saved generation (mode `release`).

| ID | Name | Checks | Kind | Modes | Severity | Scope | Rel | Req. | D (defects) | Def. |
|---|---|---|---|---|---|---|---|---|---|---|
| EW-REL-003 | Open review issue | An open comment of kind `Issue` on the revision; resolve it, or convert it to a note with a reason | integrity | g | error | project | 1.0 | FR-REV-024 | — | 15 |

The gate (FR-REV-007) refuses release when there is any unwaived error in the release scheme, any EW-REL finding,
or EW-CORE-083/084 at error level. Its named blockers are release-scheme rules at error severity:
- EW-CORE-047, a device without schematic representation;
- EW-LAY-001, a device without layout placement where its location has a layout space (a further part use not placed
  beside it, a main switch's door handle, is a warning at release);
- EW-CORE-033, a cable without length;
- `Tbd` values: EW-CORE-043, EW-CORE-044, EW-BUS-001, EW-PLC-015;
- the integrity findings marked R in [05](05-project-format-and-persistence.md) §6.3, such as EW-CORE-011
  conflict records and EW-CORE-016 unknown data that affects outputs;
- EW-REL-001 (author = checker, missing or stale sign-off) and EW-REL-002 (stale committed computations).

Two of these rules are not registered yet: EW-REL-002 until a committed computation has an input hash to compare,
and EW-CORE-084 until a pack can be untrusted or read a data table that is not compiled in (both 1.0); a shipped
pack's compiled-in table that does not load is a build defect, which EW-CORE-083 reports.

On a project before its sign-off, EW-REL-001 reports "checker not set".

EW-REL-001, -002 and -004 are the EW-REL pack, in the gate-only mode `g`. `ew_checks::gate` evaluates the release scheme in release mode with the EW-REL rules and returns the blockers, the safety waivers in force (the waivers that waive a safety-kind finding of the gate report) and the pack versions (the build version plus the SHA-256 of each pack's definition; per-pack versions are not built). The pack definition names a rule's kind and scope by the catalogue's words (`RuleKind::name`, `ScopeKind::name`: `safety`, `conflict`, `completeness`, `hygiene`, `integrity`; `project`, `page`, `strip`, `device`, `cable`, `potential`, `module`, `cfg`), the `rules.json` text, not by a Rust type's `Debug` output, so the digest a sign-off binds does not follow a code change.

### 4.9 Shipped schemes

| Scheme | Purpose | Content | Rel |
|---|---|---|---|
| `drafting` | Default `active_scheme` while editing | All loaded packs online; completeness rules pending on the page being edited ([06](06-schematic-editor.md) §3.10); hygiene rules on, EW-RDS-002 included | MVP |
| `pre-delivery` | Default `release_scheme`; `ew check --scheme pre-delivery` | Every enabled rule whose modes include release, at release severity; EW-REL evaluated by the gate only (§3.2); EW-RDS-002, EW-FL-008, EW-FL-014 off (EW-CORE-055 joins them when it is built, 1.0). The release blockers of §4.8 are errors here | MVP |
| `migration` | Lenient check run that forms an import's migration report (FR-IMP-003) | `extends = "pre-delivery"` at its severities, run in lenient mode, so prevent violations become findings of their rules; plus the import run's EW-IMP findings (§4.10) | MVP |
| `full` | Audits and acceptance of hygiene rules | Everything, including off-by-default rules (FR-CHK-027's acceptance runs here) | MVP |
| `machinery-en60204` | Machinery projects | `extends = "pre-delivery"` plus EW-EN60204 | 1.0 |
| `assembly-iec61439` | Switchgear assemblies | `extends = "pre-delivery"` plus EW-THM and EW-CORE-024 for the IEC 61439-1 clause 11 routine-verification checklist (FR-MFG-037) | 1.0 |

### 4.10 EW-IMP: import findings

An import finding records what an importer did or saw; it lives in the import run (`imports/<run-uuid>.jsonl`,
FR-IMP-041) and appears only in that import's migration report. A live rule reports a condition that is still in
the model. Where both name the same targets and defect class, the migration report shows one row under the live rule
ID with the import code as its origin ([14](14-interop.md) §2.5). All rows: kind `import`, mode i, scope the import
run, defined in 14 §2.5.

Each EW-IMP rule has a catalogue kind (completeness, conflict or hygiene), mode `r` in the `migration` and `full` schemes, scope the project (the import runs its lineages name), and is waivable like any finding that is not of kind `integrity`: an import finding the user has reviewed is waived rather than left open, and the migration report groups it with the live rules by the same kinds. Where a rule's remedy is deterministic the finding offers it as a quick fix: EW-IMP-103 "Unfreeze the tag" (`fields.set` of `tag.frozen` to null); the others offer none, their remedy asking for a value or a judgement.

| ID | Name | Checks | Severity | Rel | Req. | D (defects) |
|---|---|---|---|---|---|---|
| EW-IMP-106 | Title-block party differs | The title-block party differs from the cover party | warning | MVP | FR-IMP-015 | D-30; MF-06 |
| EW-IMP-107 | Text converted to data | A wire annotation, address, port, process tag or bus address recognised and converted | info | MVP | FR-IMP-017 | D-08, D-15, D-26; MF-07, MF-18, MF-19 |
| EW-IMP-108 | Data-like text not converted | Listed for review | warning | MVP | FR-IMP-017 | D-08, D-26 |
| EW-IMP-110 | Overview differs from detail | A hand-drawn overview differs from the linked detail; EW-CORE-046 takes over once the assembly exists | warning | MVP | FR-CHK-022, FR-IMP-039 | D-10; MF-14 |
| EW-IMP-111 | Layout binding proposal | A layout item matched to a device, to confirm | warning | MVP | FR-IMP-019 | D-02; MF-15 |
| EW-IMP-112 | Untagged device-class symbol | A symbol of a device class without device tag; build assembly and children | warning | MVP | FR-IMP-039 | D-27; MF-20 |
| EW-IMP-113 | Imported table differs | Imported nomenclature rows differ from the derived BOM | info | MVP | FR-IMP-016 | D-01, D-04; MF-21 |
| EW-IMP-121 | Several IP names on one potential | Several interruption-point names on one potential | info | MVP | FR-IMP-012 | D-24; MF-17 |
| EW-IMP-123 | Cross-section mark differs | A QET cross-section mark differs from the derived wire property; the derived value is kept and the mark's value reported | warning | MVP | FR-IMP-017 | D-08 |
| EW-IMP-201 | EDZ terms notice | The Data Portal terms notice was shown | info | 1.0 | FR-IMP-026 | — |
| EW-IMP-202 | Function definition unmapped | No FnDef mapping for an EPLAN function template; generic pin used | warning | 1.0 | FR-IMP-028 | — |
| EW-IMP-203 | ZIP-format EDZ | An EDZ in a ZIP rather than a 7-Zip container | info | 1.0 | FR-IMP-025 | — |
| EW-IMP-204 | EDZ extras ignored | Extras such as `.e3d` ignored in favour of STEP | info | 1.0 | FR-IMP-027 | — |
| EW-IMP-205 | Unit converted | A length unit other than mm converted | info | 1.0 | FR-IMP-028 | — |
| EW-IMP-256 | CP snapped | A connection point snapped to the grid by the symbol wizard | warning | 1.0 | FR-IMP-034 | — |
| EW-IMP-301 | Derived column edited | A derived column edited in a round-trip table | warning | MVP | FR-PLC-038, FR-EXT-028 | — |
| EW-IMP-302 | Row changed in EW since export | Kept, listed | info | MVP | FR-PLC-038, FR-EXT-028 | — |
| EW-IMP-303 | Unknown ID | A row whose ID is unknown | warning | MVP | FR-PLC-038, FR-EXT-028 | — |
| EW-IMP-304 | Cell conflict | Changed in both to different values; EW's value kept | warning | MVP | FR-PLC-038, FR-EXT-028 | — |
| EW-IMP-305 | Lossy Excel conversion | A value Excel changed ("1.10" → "1.1") | warning | MVP | FR-PLC-038, FR-EXT-028 | — |

The table rows 301 … 305 serve `ew import io-list` at MVP (FR-PLC-038) and `ew import table` at 1.0 (FR-EXT-028).
The severities of the 2nn, 25n and 30n rows are this catalogue's defaults within the "info–warning" range of 14.

### 4.11 EW-LIB: library lint (1.0)

The library linter of `ew-library` runs these rules over library items: in `ew lib lint <dir>` and the contribution
pipeline of [07](07-symbols-parts-macros.md) §6.2, and before `ew lib pack`, which refuses any error. They are
registered like every rule (codes, Fluent messages, rule pages) but belong to no project scheme. All rows: mode l,
scope the library item, defined in 07 §6.2.

| ID | Name | Checks | Kind | Severity | Rel | Req. | D (defects) |
|---|---|---|---|---|---|---|---|
| EW-LIB-001 | CP placement | A CP off the symbol's connection grid (`SymbolDef.grid`), a CP index not unique, or a CP not on the outline or a line end | conflict | error | 1.0 | FR-PRT-032, FR-SYM-020 | — |
| EW-LIB-002 | Anchor policy | Anchor policy missing; anchor outside the symbol's graphics or not on the connection grid | completeness | error | 1.0 | FR-PRT-032, FR-SYM-020 | D-14 |
| EW-LIB-003 | Required text slots | Text slots required for the category missing (tag; CP designations for multi-CP symbols) | completeness | error | 1.0 | FR-PRT-032, FR-SYM-020 | D-29 |
| EW-LIB-004 | FnDef CP mismatch | `fn_cp` not in the FnDef (error); a FnDef CP drawn by no multi-line symbol (warning) | conflict | error / warning | 1.0 | FR-PRT-032 | — |
| EW-LIB-005 | Licence or source missing | Licence or source missing, licence not in the pack's allowlist, or the item flagged `foreign_terms` | integrity | error | 1.0 | FR-PRT-032 | — |
| EW-LIB-006 | Copied content | IEC description text or a QET element copied (geometry fingerprint matches a QET collection element) | integrity | error | 1.0 | FR-PRT-032 | — |
| EW-LIB-007 | Semver class | Declared semver lower than the observed change class, or content changed under an already published version | conflict | error | 1.0 | FR-PRT-032 | — |
| EW-LIB-008 | Part quality and identity | A part below "basic", a manufacturer not in the master, or a duplicate (manufacturer, `pn_norm`) | completeness | error | 1.0 | FR-PRT-032 | D-18 |
| EW-LIB-009 | Rating in a description | A rating or unit inside the description ("10A C", "50hz"); the project-side counterpart is EW-CORE-065 | hygiene | warning | 1.0 | FR-PRT-032 | D-18, D-35 |
| EW-LIB-010 | CP-pattern spelling | CP-pattern designations that differ from the template FnDef's only in spelling ("Out1" against "Out 1") (warning); a template CP index not in the pattern (error) | hygiene | warning / error | 1.0 | FR-PRT-032 | — |
| EW-LIB-011 | Asset licence | An image, STEP or DXF asset without an explicit redistribution licence | integrity | error | 1.0 | FR-PRT-032 | — |
| EW-LIB-012 | Macro fragment | A macro fragment with dangling references or another grid than its declared grid (error); frozen tags kept with "keep tag" (warning) | conflict | error / warning | 1.0 | FR-PRT-032, FR-MAC-001 | — |

