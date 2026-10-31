# WP-S4.01: layout model and 2D panel pages, the MVP slice (closed specification)

**Status:** specified in cycle 39 (documents only), an M2 package, in five parts: part 1 the layout records and
commands (1.1 pw, D3 to D7), part 2 the layout queries and Golden A's layout data (1.1 pw, D5, D8, D13), part 3 the
panel page (1.2 pw, D9, D10), part 4 cut in cycle 41 into part 4a, layout objects in the BOM and the legend query
(0.7 pw, D11), and part 4b, the embedded report blocks drawn (0.55 pw, D16 to D18), part 5 the EW-LAYOUT rules
(0.8 pw, D12); part 1 first (cycle 40); part 2 after part 1; parts 3 and 5 after part 2, side by side; part 4a after
part 3; part 4b after part 4a · **Part 4 cut in cycle 41** (lane S41b, decision log ID-949: no path drew an embedded
block but the cover's, LB-2's legend did not fit its rect, and its export clause had no named path; D11 amended as
built by part 4a; D16 to D18 added; 0.35 pw taken from D15's reserve) · **Part 1 built in cycle 40** (lane B40 with fix lane FIX40b, decision log ID-924; LM-1 to LM-8 met; the refusals beyond D6's letter and `part.edit_copy`'s re-pointing of a carrier's part and a placement's `part` accepted; `replace_all` over several placed part uses moves only the first replaced use's placements; `edit.delete` on layout kinds, `source_ref`, grid editing and the missing edit commands are FT-837 to FT-840; LQ-3 and LQ-4 amended to the surface frame) · **Part 2 built in cycle 40** (lane J40 with fix lane FIX40c, decision log ID-932, ID-933; LQ-1 to LQ-6 met; the queries in `ew_derive::layout`, none stored, as built in doc 12 §2.6 and §2.7; footprints in the surface frame and occupancy in s; no ":" suffix for a function's placement, since functions carry no designation member; no clearance padding; D13's golden change with the re-blesses that follow from it, listed in D13; save as basic omits the layout with the devices and keeps the layout spaces, WP-S1.19 D14 as built; EW-LAY-010's geometry cases FT-845) · **Part 3 built in cycle 41** (lane A41, decision log ID-951 to ID-957; LP-1 to LP-7 met, no de-scope; LP-1 as built: a second `page.surface_view.set` replaces the view in place; D4's cascade of the view with its surface; D9's not-to-scale view at 1:1 and D10's label constant and ink box as built; LP-6's rename set amended; the 2D projection in `ew-scene`, arch §3 amended; D13's part-3 list and the files beyond it; follow-ups FT-868 to FT-871) · **Part 4a built in cycle 41** (lane C41, decision log ID-963 to ID-967; LB-1, LB-2 as cut, LB-4 and LB-5 met, no de-scope; D11 as built: one carrier line per part copy, folios in the Folios cell only, the legend as plain functions, `enclosure_legend` from its embedding page only; EW-CORE-043 counts carrier lines, EF-17 stays at 4; D13's part-4a list and the files beyond it; follow-ups FT-874 to FT-876) · **Date:** 2026-10-30 ·
**Work package:** [WP-S4.01](../02-work-packages.md#wp-s401-layout-model-and-2d-panel-pages-mvp-slice) ·
**Decision log:** ID-893 · **Part of:** ElektraWatt plan, specifications

**Purpose.** WP-S4.01 is the thin S4 slice that [panel layout](../../design/12-panel-layout.md) §17 asks to be staffed
explicitly: folio 902 generated from placements bound to devices, so that D-02 ("T51" against T55, free rectangles)
and D-32 (layout incomplete, not to scale) cannot recur. Nothing of it is built: the layout kinds are reserved, the
part mechanical data is typed but empty in Golden A, and folio 902 prints its frame only. This document closes the
package to the field for part 1 and to its items, decisions and acceptance for parts 2 to 5 (D14). It is written
against the code at `4772bf49` (§2.1). Where this document and a design document differ, this document is the
narrower and wins for the lanes; the design documents stay the target, and the documents of §13 are updated at the
design pass after each part. A choice a lane meets that this document does not settle is escalated, never guessed
(`CLAUDE.md`, rule 2).

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [Part 1: the layout records and commands](#5-part-1-the-layout-records-and-commands)
6. [Part 2: the layout queries and Golden A's layout data](#6-part-2-the-layout-queries-and-golden-as-layout-data)
7. [Part 3: the panel page](#7-part-3-the-panel-page)
8. [Part 4: layout objects in the BOM, the legend and PART DATA (parts 4a and 4b)](#8-part-4-layout-objects-in-the-bom-the-legend-and-part-data-parts-4a-and-4b)
9. [Part 5: the EW-LAYOUT rules](#9-part-5-the-ew-layout-rules)
10. [Files owned and what a lane may not decide](#10-files-owned-and-what-a-lane-may-not-decide)
11. [Risks](#11-risks)
12. [Size, order and routing](#12-size-order-and-routing)
13. [Follow-ups and the documents to update](#13-follow-ups-and-the-documents-to-update)
14. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package Golden A holds one layout space for +J-E-2416 with the enclosure -UC1, its
plate, three rails and five ducts in plate millimetres, and twenty placements in the order of doc 20 §13; folio 902
is a true-scale 1:10 view of the plate generated from those records, with live labels ("T55", "-X1031"), the PART
DATA block bound to -UC1, a generated legend and "Cabinet Layout · Scale 1:10" in the title block; rails and ducts
reach the BOM by their derived lengths and print folio 902; EW-LAY-001 lists exactly F55, E55, E56, S56 and -201U9
(EF-05); and no command can create a placement without a device. Renaming T55 to T57 changes folio 902 in the same
commit and no file under `layout/`. AC-33 then passes except the four end brackets of rail R2, which WP-S4.02 derives
(FR-LAY-060; §12).

**Non-goals** (each with its package):
- No window gesture for layout work: no drag from the unplaced list, no rail snapping (free, push, swap), no strip,
  assembly or bulk placement tool, no group brackets, no Unplaced devices navigator (WP-S4.02: FR-LAY-014 to
  FR-LAY-016, FR-LAY-060, FR-LAY-061, FT-486). In this package a layout is authored by commands (`ew run`, scripts),
  the import tables and the test DSL (D1).
- No 3D, STEP, CP3D, collision, clearance, routing, lengths of wires, features, dimensions or thermal (WP-S4.03 onward).
  No door, gland, side or roof surfaces, no install context or thermal inputs, no busbars, no piggy-back mounting, no
  strip segments, no panel-macro variants (1.0; their members stay unknown, D3).
- No balloons (FR-LAY-062's last step; doc 12 §5.2 puts balloons at 1.0, D10). No frozen legend positions (PQ-50).
- No QET import of folio 902 (Golden C, FR-IMP-019) and no SQL view `api_v1_placement` (1.0).
- No change to Golden A's exported bytes except the golden changes listed per part (D13).

## 2. Inputs and the code as it is

| Input | What the lanes take from it |
|---|---|
| [Panel layout](../../design/12-panel-layout.md) §1.3, §2.1 to §2.7, §3.1 to §3.7, §4, §5.1 to §5.4, §14, §16.1, §17 | Design rules, frames and units, records, placement and pose constraints, carrier parameters, the part data read, the derived queries, the mountable predicate, commands, the rule table, the page model, layers, labels, legend, the golden expectations, the MVP column |
| [Domain model](../../design/03-domain-model.md) §4.2, §16 | Delete policies, the layout records and their invariants |
| [Project format](../../design/05-project-format-and-persistence.md) §2.2, §10.3, §11 | Kinds 50 to 58, `layout/<space>.jsonl`, the follow relations, reserved kinds, typing within 0.1 |
| [Symbols, parts and macros](../../design/07-symbols-parts-macros.md) §2.5 | `MechData`, `MountKind`, `Clearance`, the carrier sections |
| [Reports](../../design/09-reports-forms-output.md) §2.2, §3.4, §4.1 | `enclosure_legend`, `part_data`, the layer set |
| [Checks](../../design/15-checks-revisions-collaboration.md) §4.5.4, §4.8 | The EW-LAYOUT rows, the release scheme |
| [Golden project](../../design/20-golden-project.md) §5, §6.1, §13, §14.1, §17.5, §18.1, §19 (D-02, D-32), §21 (AC-33) | Folio 902, the device table, the carriers and their order, the BOM rows 22 to 26, the label style, EF-05, the defect rows, AC-33 |
| ADR-0001, ADR-0004, ADR-0005, ADR-0008, ADR-0009, ADR-0011 amendment 4, ADR-0015 amendment 2, ADR-0019 | IDs not tags, v7/v5 IDs, `Nm` and `Pose3`, one command per gesture, structural invariants, typing reserved kinds within 0.1 (ID-334), the panel layers, rules and packs |
| Specifications | [WP-S1.16](WP-S1.16-mvp-rule-packs.md) (rule pages, Fluent messages, the AC-21 subset), [WP-S1.19](WP-S1.19-parts-libraries-and-macros.md) D2 (the typed L3 members), [WP-S2.06](WP-S2.06-navigators-and-table-mode.md) D4 (FT-486) |
| Follow-ups | FT-204, FT-205, FT-486 |

### 2.1 What exists at `4772bf49` (the base contract)

The lanes rely on exactly these items. If one differs, the lane stops and reports it.

| Item | Where | Used here for |
|---|---|---|
| The kinds `layout_space` (50), `enclosure` (51), `mounting_surface` (52), `carrier` (53), `placement` (54), `route_override` … `surface_feature` (55 to 58), all `KindStatus::Reserved`, file rule `layout/<space>.jsonl` | `crates/ew-schema/src/kind.rs` | Part 1 implements 50 to 54 |
| `MechData` (width, height, depth, `mount_kind`, `rail_offset`, `clearance`, `front_2d`, mass), `MountKind` (TS 35×7.5, TS 35×15, TS 32, TS 15, screw, busbar adapter, door cut-out, on parent, none), `Clearance`; `PartSections.enclosure` (outer W, H, D, wall, plate part), `.duct`, `.rail` (profile, stock length), `.mounting_plate` (W, H) | `crates/ew-schema/src/library.rs` (WP-S1.19 part 1) | Footprints, mount kinds, enclosure and plate dimensions |
| `Pose3` and `Quat9` (`{"t":[x,y,z],"q":[w,x,y,z]}`, nm and 10⁻⁹) | `crates/ew-units/src/pose.rs` | Placement and frame poses |
| `PageType::PanelLayout`; `Page.scale` (`{"one_to": n}`); the title-block key `page.scale` ("1:10"); `PageItemKind::Unknown` keeps `surface_view` verbatim | `crates/ew-schema/src/page.rs`, `page_item.rs`; `crates/ew-derive/src/title_block.rs` | Part 3 |
| `ew_dl::Layer` with `Surface`, `Carriers`, `Placements`, `Features`, `Dims` and `Layer::PANEL`; `page_layer` returns an empty layer for them; `page_kind` gives `PageKind::Panel` | `crates/ew-dl/src/layer.rs`; `crates/ew-api/src/output/layers.rs` | Part 3 fills three of them |
| `Tag::displayed(page structure)` (identifier suppression) and `DisplayedTag` | `crates/ew-derive/src/tag.rs` | `layout_label` |
| `device_part_lines`, `LineSource` (device, strip accessory, bridge, assembly config), `bom_rows` and their folio column from representations | `crates/ew-derive/src/bom.rs`; `crates/ew-reports/src/kinds/parts.rs` | Part 4 adds carrier lines |
| `ReportKind::EnclosureLegend` (`enclosure_legend`) declared, no row source; `part_data` sources the devices drawn on its page | `crates/ew-reports/src/kind.rs`, `kinds/part_data.rs` | Part 4 |
| EW-CORE-041 (device without part; strips, cables, PLC configuration devices and devices without functions left out) | `crates/ew-checks/src/rules/device.rs` | D8 reuses its predicates |
| No EW-LAY rule and no EW-LAY catalogue row; EF-05 pending for WP-S4.01 | `crates/ew-checks/catalogue/rules.json`; `crates/ew-api/tests/expected/findings_mvp.csv`, `tests/checks_part5.rs` | Part 5 |
| `ew-panel`: an empty crate (edges to `ew-commands`, `ew-step-proto`) | `crates/ew-panel/src/lib.rs`; `ci/dep-rules.toml` | Stays empty (D2) |
| Golden A: folio 902 a `panel_layout` page, scale 1:10, title "Cabinet Layout", one embedded `part_data` block, printing the frame only (`expected/text/902.csv`, 41 frame rows); -UC1 carries NSYPLA1074G and the TBD rail (2.192 m) and duct (3.81 m) part uses (ID-316); -UC1-plate is its child with NSYPMM107; no part has mechanical data; no layout file; BOM rows 22 to 25 print no folio, row 26 "101, 103" (ID-317) | `golden/J-E-2416/tables/`, `golden/J-E-2416.ew/`; `tools/tables_from_doc20.py` (reads doc 20 §13 and §14.1) | The golden baseline |
| The golden build paths: (a) `ew import csv` over `tables/`, (b) the test DSL, (c) the checked-in fixture | `crates/ew-import-tables/src/emit/`; `crates/ew-testkit/src/golden/` | Part 2 extends (a) and (b) |

### 2.2 Gaps found (why the parts are shaped as they are)

| Gap at `4772bf49` | Consequence |
|---|---|
| The layout kinds are reserved: no table, no link, no invariant, no command | Part 1 (D3 to D7) |
| No layout query; `ew-checks`, `ew-reports` and `ew-scene` may not depend on `ew-panel` (`ci/dep-rules.toml`) | The MVP queries live in `ew-derive` (D2) |
| Golden A has no layout data, no part dimensions, and doc 20 §13 gives the carriers in paper positions "for order only" | Part 2 fixes the test geometry (D13) |
| The rail and duct BOM quantities come from paper lengths of a drawing that is not to scale (2.192 m, 3.81 m; ID-316) and do not fit the test enclosure | Part 4 derives them from the carriers (FT-205), a golden change |
| Folio 902 has no view item, no panel layer is drawn, and the title block does not print the scale | Part 3 (D9, D10) |
| `enclosure_legend` has no rows; PART DATA on 902 lists nothing | Part 4a (D11) |
| *Found in cycle 41 (ID-949):* only `profile_tables` embedded blocks are drawn (`ew-api` `output/view.rs` `PageEmbeddedOf`; `ew-scene` `embedded.rs` has the one variant `Cover`), so PART DATA is derived for 201, 202 and 902 and drawn on no page; `part_data` sources only from the page's representations, so on 902 it lists nothing | Part 4b (D16) |
| No EW-LAY rule | Part 5 (D12) |

## 3. Requirements traced

The 19 rows of the package.

| Requirement | What this package delivers | Part | Acceptance |
|---|---|---|---|
| FR-LAY-001 | One layout space per location; `unplaced(space)` keyed on the location | 1, 2 | LM-2, LQ-2 |
| FR-LAY-002 | Enclosure and plate surface from part data, poses relative to the surface frame; a scale change moves no pose | 1, 3 | LM-3, LP-4 |
| FR-LAY-004 | Rails as parametric carriers; in the BOM by derived length | 1, 4a | LM-4, LB-1 |
| FR-LAY-005 | Ducts as parametric carriers with the generated "W×H" label; in the BOM | 1, 3, 4a | LM-4, LP-2, LB-1 |
| FR-LAY-006 | Placements reference a device (and optionally one function); no tag text; `layout_label` | 1, 2 | LM-5, LQ-5 |
| FR-LAY-007 | Geometry from the part; a part change re-derives at the same pose (D7); the overlap is reported, never resolved | 1, 2, 5 | LM-7, LQ-4, LC-4 |
| FR-LAY-008 | Layout-only parts (rails, ducts) counted in the BOM without a function; free graphics counted nowhere | 4a | LB-1, LB-4 |
| FR-LAY-009 | Abbreviated and full labels by the view's mode | 3 | LP-3 |
| FR-LAY-010 | A page is a view of one surface at a scale; the scale in the title block; paper-height text | 3 | LP-2, LP-4 |
| FR-LAY-011 | `front_2d` outline, else the footprint rectangle | 3 | LP-2 |
| FR-LAY-012 | The generated legend per surface, embedded on 902 | 4a (rows), 4b (drawn, exported) | LB-2, LB-7, LB-8 |
| FR-LAY-013 | The list half: `unplaced(space)` through the API and EW-LAY-001; the navigator and drag stay with WP-S4.02 (FT-486) | 2, 5 | LQ-2, LC-1 |
| FR-LAY-017 | The mountable predicate (D8); EW-LAY-001, EW-LAY-002 (structural), EW-LAY-003 online | 1, 2, 5 | LM-6, LQ-2, LC-1, LC-2 |
| FR-LAY-018 | EW-LAY-004 and the 2D overlap check, online | 5 | LC-3, LC-4 |
| FR-LAY-059 | The placement names the part use it realises, written at placement (D6); EW-LAY-003 | 1, 5 | LM-5, LC-2 |
| FR-LAY-062 | Paper-height labels, rotated, then a leader; never scaled; balloons at 1.0 (D10) | 3 | LP-3 |
| FR-REP-007 | Enclosure, plate, rails and ducts in the BOM; the end brackets stay authored part uses until WP-S4.02 | 4a | LB-1 |
| FR-CHK-039 | Unplaced devices (EW-LAY-001); "layout graphics not bound to a device" met by EW-LAY-002 (D12) | 1, 5 | LM-6, LC-1 |
| FR-CHK-005 | The EW-LAYOUT MVP subset: EW-LAY-001, -002, -003, -004, -006 (2D), -010, -016 | 5 | LC-1 to LC-7 |

## 4. Decisions

Each is a lead ruling that the owner may reverse. The design pass records them under ID-893 (and the as-built
entries after each part).

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **Model, outputs and checks; no window gesture.** Every authoring gesture (drag, snap, draw rail, draw duct, create enclosure from the window, the placement's property-grid rows) is WP-S4.02's; here a layout is authored by commands, scripts, the import tables and the test DSL. Placements on folio 902 are hit-testable (each item carries its `EntityRef`), so selection sync works | The package's 19 rows are the model and its outputs; the gestures are WP-S4.02's rows. Doc 12 §12.1 lists "draw rail, draw duct" under MVP, which no package row names: proposed follow-up FP-1 gives them to WP-S4.02 |
| D2 | **Where the code lives.** The MVP layout queries (`mountable`, `unplaced`, `placement_geometry` in 2D, `layout_label`, `rail_occupancy`, carrier geometry, `layout_legend`) live in `ew-derive` (`src/layout/`); the commands in `ew-commands` (`commands/layout.rs`); the panel layers beside the schematic layers (`ew-api` output families over `ew-scene`); the rules in `ew-checks`. `ew-panel` stays empty until the 1.0 work (collision reconciler, routing, glTF). No edge of `ci/dep-rules.toml` changes | `ew-checks`, `ew-reports` and `ew-scene` must read these queries and may not depend on `ew-panel`; arch §6.4 already lists `layout_label` among the `ew-derive` queries. The trade-off is that arch §3's `ew-panel` row ("layout queries") is reworded at the design pass (§13); no ADR changes, because no edge or rule changes |
| D3 | **The five kinds typed within format 0.1** (ADR-0011 amendment 4, ID-334: reserved kinds with rank and ownership; additive, `schemas/0.1` re-blessed). Typed members: `layout_space {location, name}`; `enclosure {space, device, pose}`; `mounting_surface {enclosure, kind, device?, frame, usable, pe_bond}`; `carrier {surface, kind, part?, path, params, pe_bond}` with params `rail {profile}` and `duct {width, height, slot_pitch?, cover?}`; `placement {device, function?, part?, carrier, pose, locked}` with `carrier` = `{"carrier": id}` or `{"surface": id}`. Everything else stays verbatim: `install`, `thermal`, `policy`, `install_type`, `doors`, `max_depth`, `segment`, `variant`, the `placement` variant of `CarrierRef`, the `busbar` and `surface` carrier kinds, surface kinds other than `plate`. **MVP limits** (refused by the commands, reported by EW-LAY-010 on loaded data): one layout space per location, one enclosure per space, one `plate` surface per enclosure | The members doc 03 §16 names are typed only where this package uses them, so nothing typed now is unused or untested; no 0.1 build ever wrote layout records, so the exposure of amendment 4's "older build" clause is nil. One space per location keeps `unplaced` and EW-LAY-004 unambiguous; bayed suites and further surfaces arrive with their rows |
| D4 | **Delete policies** (the link table of `crates/ew-model/invariants.md` §4): `placement.device` cascade (EW-LAY-002, doc 03 §4.2); `placement.carrier` forbid; `placement.function` cascade; `placement.part` none (a placement whose part use left the device is EW-LAY-003's, never a silent change); `carrier.surface`, `mounting_surface.enclosure`, `enclosure.space` cascade; `enclosure.device` forbid; `mounting_surface.device` unlink; `layout_space.location` forbid; `carrier.part` forbid like a part use's copy. *As built (part 3, cycle 41, ID-952):* `page_item.kind.surface_view.surface` cascade (a view without its surface shows nothing; its row in `invariants.md` §4) | A placement cannot outlive its device (D-02); an occupied rail cannot vanish without the explicit `then` of doc 12 §3.6; an enclosure cannot lose its device silently, as an assembly cannot lose its children |
| D5 | **Geometry conventions** (integer nm, exact). Carriers are **axis-parallel** at MVP: a rail is two points on one horizontal or vertical line; a duct an orthogonal polyline; a segment at another angle is refused. A rail placement's pose is `t = (s, 0, 0)`, the part's clip point on the rail axis at distance `s` from `path[0]`, `q` identity or 180° about w; the footprint is the part's W×H rectangle with `rail_offset` at the clip point (absent `rail_offset`: the centre). A surface placement's pose is `t = (u, v, 0)`, the part origin, rotated by k·90° about w about that point. A **strip** device is placed as one placement: its footprint is the sum of its terminals' widths in strip order by the largest terminal height, its clip point the left end at mid-height. A part without W or H gives an **unknown footprint**: a 20 × 20 mm square about the pose, flagged unknown and drawn dashed. The plate surface's frame centres the plate in the enclosure's outer W×H (`z = 0` until 3D); its usable polygon is the plate rectangle | Axis-parallel geometry keeps every length and overlap exact without square roots and covers every golden carrier; doc 12 §2.3's pose table is kept as stated; the unknown footprint makes a placed device without data visible instead of invisible |
| D6 | **The commands** (all Interactive, one undo step each; the deletes Bulk and previewed above 200 placements): `layout.space.create {location, name?}`, `layout.space.delete`, `layout.enclosure.create {space, device, pose?, pe_bond?}`, `layout.enclosure.delete`, `layout.carrier.add {surface, kind, part?, path, params, pe_bond?}`, `layout.carrier.edit`, `layout.carrier.delete {carrier, then: remove_placements \| rehome_to_surface}`, `layout.place {device, part?, function?, carrier, pose}`, `layout.move {placement, carrier?, pose}`, `layout.remove`, `layout.lock {placement, locked}`. `layout.place` takes an **explicit pose** (no snapping; WP-S4.02 adds the `mode` member) and **always writes `part`**: the given part use, else the primary mountable part at that moment (D8); a strip and a device without a part write none. Refusals: a missing device or carrier; a second placement of one (device, part, function); a mount kind that does not fit the rail profile (the compatibility table is data in `ew-derive`, doc 12 §3.3: "-102F1 (TS 35) does not fit a TS 32 rail"); a pose outside the carrier kind's degrees of freedom or off the carrier (`s ∉ [0, L]`); a part not used by the device. `layout.enclosure.create` refuses a device whose parts carry no enclosure section ("the device has no enclosure part"); the plate surface's `device` is the child of the enclosure device that carries the section's plate part, if any | One gesture, one transaction (ADR-0008). Writing `part` at placement makes FR-LAY-059 hold ("removing 195740 raises EW-LAY-003") and avoids the silent retarget doc 12 §3.5 forbids; a `None` read from an import still means "the primary" |
| D7 | **A part change follows the placement.** In `part.assign`, a placement whose `part` copy leaves the device follows the one copy the command adds, in the same transaction, when the command adds exactly one copy (modes `replace` and `replace_all`); otherwise it stays and EW-LAY-003 reports it | FR-LAY-007 ("changing -102F1's part to a wider MCB widens its footprint") and FR-LAY-059 ("removing 195740 raises EW-L003") both hold: replacing a part is a change of the same use, removing it is not |
| D8 | **The mountable predicate as built** (doc 12 §2.7), first match wins: (1) a device whose location has no layout space, or supplied by others once FT-209 exists: not considered; (2) the device of an enclosure or of a surface: exempt; (3) a strip with terminals: mountable, placed as a strip; (4) a terminal of a strip, a cable, a PLC configuration device, a device without functions (EW-CORE-041's predicates): exempt; (5) a part use with `mount_kind` other than `none` and `on_parent`: mountable, the first such use is the primary part; (6) part uses that are all `on_parent` or `none`: exempt; (7) part uses without `mount_kind`: mountable, unknown footprint; (8) no part use and every function's FnDef category is the PE point (`EarthingBar/ProtectiveEarth/Point`, `ew.fn.pe`): exempt as an integral PE point; (9) otherwise (no part): mountable, unknown footprint. Assembly children are not exempt by being children | Golden A's -XPE has no parent and no kind, so doc 12 §2.7's "kind `PeStud`, parent = a surface's device" cannot hold without a golden change; the FnDef category gives the same result from data that exists. The valve terminal's members (-201U3 … -202U3, -201U9) are placed or reported in their own right (EF-05 names -201U9), so "child of a placed sub-base" applies only through `on_parent` |
| D9 | **The panel page.** The page item variant `surface_view {surface, datum_at, label_mode}` (`label_mode` `abbreviated` (default) or `full`; `side`, `clip`, `layers` stay verbatim) is typed within 0.1 (doc 05 §2.2 names it); a `panel_layout` page holds at most one, set by `page.surface_view.set {page, surface, datum_at, label_mode?}` (refused on another page kind). Projection doc 12 §5.1: `x = datum.x + u·num/den`, `y = datum.y − v·num/den`, i128, one rounding. Layers: `surface` (enclosure outline, plate outline, usable polygon dashed), `carriers` (rail outline and axis chain line; duct outline and its "W×H" label from `params`), `placements` (`front_2d` transformed, else the footprint rectangle; unknown dashed), `labels`. The **title block's** `page.title` value on a page with scale 1:n reads "<title> · Scale 1:n"; the TOC keeps the plain title (it reads `PageTitle`) | FR-LAY-010 binds the scale into the title block, and doc 20 §13 prints "Cabinet Layout · Scale 1:10"; composing the value avoids a form change, and every not-to-scale page is unchanged. *As built (part 3, cycle 41, ID-951, ID-955):* a second `page.surface_view.set` replaces the page's view in place (its ID kept), paste and copy leave views out, and a file that holds two draws the lowest ID; a view on a page without a scale draws at 1:1; a rail's axis line carries no pick entity, so a click selects the placement |
| D10 | **Labels** (FR-LAY-009, FR-LAY-062, doc 12 §5.3). `layout_label(p)` = the device's displayed tag on the view's page (abbreviated: identifier suppression against the page's structure; full: the full DT); a placement of one function adds ":" and the function's designation where it has one. The label uses the slot kind `layout.label` bound to `xref.device` (1.83 mm em, doc 20 §17.5) at its paper height; the fit test is the ink box from `shaped_text` plus 0.1 mm per side against the footprint's paper rectangle: horizontal, else rotated 90°, else outside above the footprint with a leader (below when above leaves the surface). Never scaled. **Balloons are 1.0** (doc 12 §5.2), so FR-LAY-062's last step is not built | The fit needs text metrics, so it runs in the scene (`ew-text`), not in `ew-derive`; doc 12 §5.2 and FR-LAY-062 disagree on balloons, and the doc's 1.0 placement wins because a balloon needs the legend positions of a stable legend (FP-3). *As built (part 3, cycle 41, ID-953, ID-954):* `layout.label` is a scene constant bound to the `xref.device` style, not a `SlotKind`; the ink box is estimated as the advance + 0.2 mm by the cap height + 0.2 mm (+ the descent where there is one); the leader starts 1.5 mm off the footprint; a true ink-box query is FT-870 |
| D11 | **BOM, legend and PART DATA.** A carrier with a part gives one part line (`LineSource::Carrier`) owned by the enclosure device of its surface, quantity = its path length in metres, exact to the millimetre (`Dec3`), no cutting increment (FR-MFG-030, 1.0); -UC1's two authored rail and duct part uses are removed (FT-205). A part line of a layout object (enclosure device, surface device, carrier) prints the folios of the panel pages whose surface view shows its surface (FT-204, rows 22 to 25); a placed device's own row keeps its representation folios. `enclosure_legend` reads `layout_legend(surface)`: the enclosure, the plate, one row per carrier (rails, then ducts, each top to bottom then left to right by `path[0]`, the DT cell empty, the length as quantity), then the placements by DT in natural order; positions 1 … n, derived. `part_data` on a panel page sources the enclosure device of its surface view and its descendants (the existing rule from that root) | Carrier lines owned by the enclosure keep the BOM's tag column at "-UC1" as doc 20 §14.1 prints it; one legend row per carrier is what a shop cuts; carriers have no name member in doc 03 §16, so the DT cell is empty (FP-5). *As built (part 4a, cycle 41; project owner (delegated), ID-949; amends the decision):* **one line per part:** in `DevicePartLines` the carriers of one enclosure device that share a part copy give **one** part line (`LineSource::Carrier`), its quantity the sum of their path lengths in metres, exact to the millimetre, after the device's authored part uses; the legend keeps one row per carrier. **Folios on the BOM only:** the folio rule for layout objects applies to the BOM's Folios cell only; no other reader of a device's folios changes. **A carrier without a length** adds nothing to its line's quantity; the line is kept, at quantity 0 when no carrier of that part has a length, and EW-LAY-010 (part 5) reports the carrier. **Golden source:** part 4a's golden change goes through doc 20 §14.1 and `golden/J-E-2416/tools/tables_from_doc20.py`. PART DATA's source rule (the last sentence of the decision) is built by part 4b (D16) |
| D12 | **The EW-LAYOUT MVP rules** (doc 15 §4.5.4), each with its catalogue row, rule page and Fluent messages (en-GB, nl-NL; WP-S1.16's pattern) and scope kinds `space`, `surface`, `carrier`: **EW-LAY-001** per mountable unplaced device, only where its location has a layout space; **EW-LAY-002** is the structural invariant of D4 (no rule run); **EW-LAY-003** a placement whose `part` is not a part use of its device; **EW-LAY-004** a placed device whose location differs from the space's; **EW-LAY-006 (2D)** a positive-area intersection, on one surface, of two placements, of a placement and a duct, or of a rail and a duct (never a placement with its own carrier, never duct with duct: joints are by design); **EW-LAY-010** mount kind against rail profile, a pose off its carrier or outside the kind's degrees of freedom, a footprint or carrier outside the usable polygon, a rail whose occupied intervals exceed `[0, L]` (a grown strip), and D3's MVP limits; **EW-LAY-016** placed members of one assembly or PLC station on one carrier (the parent first, then `Device.position` or the module slot) out of order by `s`, or with another device's placement between two members. Overlap by sort-and-sweep per surface, no new dependency. **FR-CHK-039's second half** ("layout graphics not bound to a device") is met by EW-LAY-002: a graphic never binds, and FR-LAY-008 permits free graphics, so no rule reports them | The MVP subset as catalogued; a sweep over at most a few hundred footprints per surface needs no R-tree (rstar stays with `ew-scene`); reporting permitted free graphics would be noise |
| D13 | **Golden A's layout data** (test values ▲, `pe_bond = true` on the plate and the rails) are §6.1's tables, copied verbatim into doc 20 §13 by part 2, with one new U-row in doc 20 §23.1. **Golden changes, each decided by the orchestrator under the owner's standing delegation (2026-10-27), re-blessed and recorded:** part 2 the new layout file, the part copies that gain `mech` or a carrier section (and their stored hashes) and the tables; part 3 folio 902's expected outputs and its page file (the surface view); part 4 rows 22 to 25 of 901 and 901.1 (folio 902; rows 24 and 25 at 1.71 m and 3.17 m), -UC1's part uses, the legend block on 902 and PART DATA, *as cut in cycle 41 (ID-949)* into **part 4a:** doc 20 §14.1 rows 24 and 25 and the lines that repeat their values; the generator's outputs (`tables/devices.csv`, `expected/parts_list_14_1.csv`, the README's generated block); `golden/J-E-2416.ew/**` by its bless test; 901 and 901.1 in text, CSV and rows (`expected/text/901.csv`, `901.1.csv`, `expected/csv/bom_parts.csv`, `bom_assembly.csv`, `expected/rows/901.csv`, `901.assembly.csv`); the AC-07 constants and the tests that pin 2.192 and 3.81; `findings_mvp.csv`'s EF-17; `expected/acceptance_headless.md` and the blind selftest recording's golden hash; and **part 4b:** doc 20 §17.3's one new "Legend box" row and the generator's list of embedded blocks (`tables_from_doc20.py`, its report-block count 15 → 16), so `tables/report_blocks.csv` gains the `legend-902` row; `golden/J-E-2416.ew/**` by its bless test (the new report block, the import lineage hash); the expected text and PNG of 201, 202 and 902 and the PDF hash (`expected/pdf/sha256.txt`; `structure.json` only where its counts move); `crates/ew-api/tests/reports_golden.rs`' PART DATA assertion for 902 (no rows → two); `expected/acceptance_headless.md` and the blind selftest recording's golden hash where they follow; part 5 the expected findings (EF-05 no longer pending). Every other exported byte is unchanged, proven by the byte-equal tests. *As built (part 2, cycle 40; decision log ID-932, the review's part-4 finding 6):* beyond the part 2 list, accepted by the orchestrator because each follows from it: `golden/J-E-2416.ew/project.jsonl` (the import lineage's `last_source_sha256`, since the tables changed); `expected/acceptance_headless.md` (the files a save writes, the path (b) command count and the memoised families: the layout file, 30 layout commands, four new families; FT-847 asks that these totals stop being pinned); `crates/ew-api/tests/expected/fragment_102F1.txt` and the three `*_fndef_part.json` fixtures built from golden copies (each copy gains its `mech` and hash); the blind selftest recording's golden hash. No exported output changed. *As built (part 3, cycle 41; decision log ID-956, the review's part-1 finding 6):* beyond the part 3 list, accepted by the orchestrator because each follows from it: `tables/pages.csv` (the view columns), the golden README, `tables_from_doc20.py`, `golden/J-E-2416.ew/project.jsonl` (the import lineage's digest) and `manifest.json`, `expected/pdf/sha256.txt`, `expected/acceptance_headless.md` (AC-06, AC-13 to AC-15, AC-16's folios) and the blind selftest recordings' golden hash; generated: `field_catalogue.txt`, `schemas/0.1/page_item.schema.json`, the command schema and index, the book's command pages. *As built (part 4a, cycle 41; decision log ID-963, the review's part-2 finding 5):* beside the part 4a list, `expected/pdf/sha256.txt`, `expected/png/901.png` and `expected/png/901.1.png`, which follow from the BOM rows, and the selftest recordings' golden hash; `findings_mvp.csv`'s EF-17 is unchanged (EW-CORE-043 counts carrier lines, ID-967) | The paper lengths of a not-to-scale drawing (2.192 m, 3.81 m) cannot fit a 750 mm enclosure; test geometry in plate millimetres is what FT-205 asks for, and doc 12 Open question 3 recommended exactly this |
| D14 | **Parts 2 to 5 in the detail they need now.** Part 1 is closed to the field; later parts are closed to their items, decisions and acceptance; each part is re-read against the delivered earlier parts at dispatch without reopening D1 to D13 | Part 1 dispatches first (cycle 40) |
| D15 | **Size and reserve.** The parts sum to **5.1 pw** of the 5.7 pw PERT; **0.6 pw** is held as reserve for the golden changes, with the de-scope rule of §12. *Amended in cycle 41 (ID-949):* part 4b takes **0.35 pw** of the reserve (the drawing path 0.25, its golden change 0.1), so the parts sum to 5.45 pw and **0.25 pw** remains. *Checked at the cycle-41 design pass (ID-949, S41b's Q1): confirmed.* Parts 1 to 3 drew nothing from the reserve (no lane reported an excess or a de-scope, and their golden changes were sized inside them); 5.1 − 0.9 + 0.7 + 0.55 = 5.45 pw against the 5.7 pw PERT of `work-packages.csv` (O 4 · L 5.5 · P 8; 5.67 unrounded, so 0.22 pw exactly) | Four of five parts change Golden A; the reserve is spent only on golden rework, not on scope. The cycle-41 draw is the one exception: the base contract (§2.1) took for granted that an embedded block could be drawn, and only the cover's could, so the drawing path is missing groundwork, not new scope |
| D16 | **Embedded report blocks are drawn, every kind through one path** (part 4b). `ew-api` assembles, `ew-scene` lowers (ARCH §3: `ew-scene` has no `ew-derive` edge; no edge changes). `PageEmbeddedOf` returns a view for **every** embedded block on the page: `profile_tables` keeps `EmbeddedContent::Cover` (folio 1 byte-equal); every other kind gives a new variant `EmbeddedContent::Table`, filled from the block's `BlockRows` with each cell rendered by `ew_reports::render::render` for that page, as the generated pages' tables are (neutral language, displayed tags); a block whose rows do not derive prints its reason as the note. What prints is the kind's: a new `ReportKindDef` method in `ew-reports` gives the printed columns (keys and weights), the box title and the number of flow columns; its default prints every column at weight 1, no title, one flow column. The block's `title`, when not empty, replaces the kind's title. **Geometry** (house values in printed mm, like the cover's proportions not read from the form, FT-203): the block's rect drawn as a box at the house grid width with a 2.61 mm drop shadow (doc 20 §17.3's PART DATA box); a title band of 1.5 rows in style `box.title` when there is a title; in each flow column a header row with the house fill when the layout has headers, then data rows of the house `ROW_HEIGHT` (6.52 mm) in `text.normal`; a table without headers draws no inner grid. Flow columns split the rect's width with a 4 mm gutter; rows fill the first column to its capacity, then the next. Cell texts follow `table.rs` (left aligned, shrink to the 1.8 mm cap height, then reported, never clipped); rows beyond the capacity are not drawn and are reported (`TooManyRows`, FR-REP-039). **PART DATA on 201 and 202 starts drawing.** **`part_data`'s sources** (D11's last sentence): the root ancestors of the devices drawn by the page's representations **and** the enclosure devices of the surfaces that the page's surface views show (a page-item test, not a page-kind test); lines with `LineSource::Carrier` are left out of PART DATA. Its layout: title "PART DATA", no header, the label and the part number at weights 1 : 3 (manufacturer and description stay in the rows) | One general path instead of a panel-page exception. Doc 20 §17.3 gives the reference's PART DATA box for 201, 202 and 902, and the fixture rects are exactly those boxes (16.31–89.38 × 16.31–52.85 printed); §3.4 lists no intended difference for a missing box, so a box drawn on no page is an unrecorded deviation, which drawing it closes. Keeping 201 and 202 undrawn would need a page-kind exception or a block member (§10.2 item 2). The cost is a golden change on 201 and 202 (D13's part 4b list). Carrier lines leave PART DATA because the legend on the same page lists every carrier: one fact in one place |
| D17 | **The legend's fit: two flow columns at the house row height** (part 4b). The block `legend-902` (kind `enclosure_legend`, page 902) has the rect **108.00–278.00 × 19.50–167.50 mm printed** (page ≈ (99.58, 9.85)–(271.94, 159.90) mm through doc 20 §17.1's map), a doc 20 §17.3 row read by the generator; title "LEGEND"; two flow columns of 83.0 mm with a 4 mm gutter; printed columns position, DT, quantity and part number at weights 10 : 22 : 16 : 35 (the description stays in the rows and the CSV). Capacity: (148.0 − 9.78 title) / 6.52 = 21.2, so a header and **20 data rows per column, 40 rows**. Golden A's 30 rows print 20 + 10; with F55 placed, 31 print 20 + 11; WP-S4.02's end-bracket row fits too. The surface view's enclosure outline ends at 103.49 mm printed (page 95 mm, §7), so the legend keeps a 4.5 mm gap; the PART DATA box (16.31–71.11 × 16.31–52.85) is clear of it. **LB-2's stop rule, kept for this capacity:** if Golden A's rows do not fit 40, or a legend text of Golden A reports an overflow, the lane stops and reports | One column cannot hold 31 rows and a header at 6.52 mm: 32 × 6.52 = 208.64 mm, more than the grid frame's whole drawing area (15.01 to 182.02 mm printed, 167.0 mm), so no rect on the A4 sheet holds it. A smaller row height would need at most 4.48 mm (148 / 33), leave two rows of headroom and break the house rhythm that the BOM on 901 uses. Two columns keep the rhythm and leave room to grow, at the cost of the description, which stays off the sheet. LB-2's first rect (page (105, 20)–(280, 175), printed 113.35–285.95 × 29.52–182.39) crossed the frame's right edge (281.18) and ran into the title block, so it moves either way. Doc 16a says it does not touch the sheet; its principles (hierarchy title > header > rows, one rhythm, no fact twice) are kept |
| D18 | **The legend's CSV export uses the existing named-block path; no exclusion lifts** (part 4b). `ew export report --block <ID> --format csv` (`RowExport::Report`; `find_block` by ID does not filter embedded blocks) already writes any block's rows, an embedded block's included, in the golden CSV dialect. The exclusions at `crates/ew-reports/src/export.rs:73` and `crates/ew-api/src/output/rows.rs:193` choose the **default** block of a kind-specific export (`ew export cables`, `bom`, `wires`, `terminals` without `--block`) and stay: an embedded block is never that default. An embedded block has no first folio, so it is named by its ID only | The cycle-41 premise that an embedded block has no export path was wrong: the code wins. Lifting the exclusions would only let a PART DATA or legend block compete as the default of an export of another kind; LB-2's export clause is kept (LB-8) with no code change |

No decision changes an ADR.

## 5. Part 1: the layout records and commands

**Scope.** D3 to D7. **1.1 pw.**

**Build list.**
1. `crates/ew-schema/src/layout.rs`: the five records of D3 with their members in doc 03 §16's order, unknown members
   and variants kept (`Extra`, `Unknown(Value)`); `kind.rs` marks ranks 50 to 54 `Implemented`; `schemas/0.1`
   re-blessed.
2. Storage: `layout/<space>.jsonl` ownership (space via carrier → surface → enclosure, doc 05 §2.2) and the follow
   relations of doc 05 §2.2 (`carrier.surface`, `mounting_surface.enclosure`, `enclosure.space` re-key their
   dependants), in `ew-format`/`ew-store`; `ew-model` tables, indexes (placements by device, by carrier; carriers by
   surface) and the link rows of D4 in `crates/ew-model/invariants.md` §4.
3. `crates/ew-derive/src/layout/`: carrier geometry (axis check, length), the mount-kind compatibility table (data),
   the pose validator of D5 shared with part 5's EW-LAY-010.
4. `crates/ew-commands/src/commands/layout.rs`: the eleven commands of D6 with previews and the refusal texts quoted
   there; `assign.rs`: the follow of D7.
5. The command fuzzer gains the layout commands (append-only arms).

**Acceptance.**
- **LM-1** A project with every typed member and with unknown members and variants loads and saves byte for byte; an
  incremental save after a random layout command sequence equals a full save (NFR-REL-16); moving a carrier to
  another surface re-keys its placements' file entries.
- **LM-2** `layout.space.create` for +J-E-2416 creates one space; a second for the same location is refused naming
  the first; deleting the location's structure node is refused while the space exists.
- **LM-3** `layout.enclosure.create` on a device with a part whose enclosure section is 750 × 1000 × 420 mm and a child
  carrying a 650 × 900 mm plate part gives one enclosure and one `plate` surface with `device` = the child, frame
  `t = (50, 50, 0)` mm and the usable rectangle (0, 0)–(650, 900) mm; a device without an enclosure part is refused
  with D6's text.
- **LM-4** `layout.carrier.add` of a rail (two points, profile from its part's rail section) and of a duct (an
  orthogonal polyline, 40 × 60 mm) succeeds; a diagonal segment is refused; `layout.carrier.delete` of an occupied
  rail with `remove_placements` and with `rehome_to_surface` (each pose converted to the equivalent surface pose)
  previews and commits one step; without `then` it is refused.
- **LM-5** `layout.place`, `layout.move`, `layout.remove`, `layout.lock` are one undo step each; `part` is written as
  D6 says (a device with two mountable part uses gets the first); a TS 35 device on a TS 32 rail, a pose off the rail,
  a second placement of one (device, part, function) and a part the device does not use are each refused.
- **LM-6** Deleting a placed device removes its placement in the same transaction, listed in the preview; deleting
  an occupied carrier other than through `layout.carrier.delete` and deleting an enclosure's device are refused; the
  fuzzer finds no command sequence that leaves a placement without a device (EW-LAY-002, AC-33's last clause).
- **LM-7** `part.assign replace` on a placed device moves the placement's `part` to the new copy in the same step;
  `part.assign` removing the placed use without adding one leaves `part` unchanged.
- **LM-8** Golden A and AC-15 path (b): every exported byte and the acceptance result unchanged; twice-run
  determinism; `cargo nextest run -p ew-schema -p ew-format -p ew-model -p ew-store -p ew-derive -p ew-commands`,
  the matching clippy, `cargo fmt --all --check`, `cargo xtask check-deps`.

## 6. Part 2: the layout queries and Golden A's layout data

**Scope.** D5, D8, D13's part 2 change. **1.1 pw.**

- The queries of doc 12 §2.6 that the MVP needs, memoised: `mountable(dev)` (D8), `unplaced(space)` (natural order of
  tag), `placement_geometry(p)` (footprint and outline in the surface frame, unknown flag; D5), `rail_occupancy(carrier)`
  (sorted intervals), `layout_label(p)` (D10's text), carrier geometry and length; `unplaced` through the `Session`
  API.
- Golden A's layout: doc 20 §13 gains §6.1's tables verbatim and one U-row in §23.1; `tables_from_doc20.py` writes the
  new tables (layout space, enclosure, carriers, placements) and the mechanical columns of `parts.csv`, each checked
  against its doc 20 cell; `ew-import-tables` emits the records (path (a)); the test DSL issues the layout commands of
  part 1 (path (b)); the fixture is regenerated (path (c)).

### 6.1 Golden A's test values (▲, copied into doc 20 §13)

**Enclosure and plate.** -UC1's part NSYPLA1074G: enclosure section 750 × 1000 × 420 mm, plate part NSYPMM107;
NSYPMM107: mounting-plate section 650 × 900 mm; the plate surface's device is -UC1-plate. Plate frame `t = (50, 50,
0)` mm. Coordinates below are plate millimetres (u right, v up, datum the plate's bottom-left corner).

| Carrier | Kind, part | Path (u, v) | Length |
|---|---|---|---|
| D1 | duct 40×60, TBD wiring duct | (0, 880) → (650, 880) | 650 |
| D2 | duct 40×60, TBD wiring duct | (20, 860) → (20, 0) | 860 |
| D3 | duct 40×60, TBD wiring duct | (630, 860) → (630, 340) | 520 |
| D4 | duct 40×60, TBD wiring duct | (40, 600) → (610, 600) | 570 |
| D5 | duct 40×60 (type ▲U-26), TBD wiring duct | (40, 360) → (610, 360) | 570 |
| R1 | rail TS 35×7.5, TBD DIN rail | (40, 740) → (610, 740) | 570 |
| R2 | rail TS 35×7.5, TBD DIN rail | (40, 480) → (610, 480) | 570 |
| R3 | rail TS 35×7.5, TBD DIN rail | (40, 200) → (610, 200) | 570 |

Rails 1.71 m, ducts 3.17 m. The TBD DIN rail part gains a rail section (profile TS 35×7.5); the TBD end bracket part
a width of 10 mm (used by WP-S4.02).

**Part mechanical data** (W × H × D mm; mount kind TS 35×7.5 unless stated; `rail_offset` absent, so the clip point
is the centre):

| Part | W × H × D | Part | W × H × D |
|---|---|---|---|
| 2904602 | 70 × 130 × 125 | UK 5 N | 6.2 × 47 × 42 |
| A9P52610 | 18 × 85 × 75 | USLKG 5 | 6.2 × 47 × 42 |
| A9F78116 | 18 × 85 × 75 | PT 2.5-TWIN | 5.2 × 72 × 37 |
| MICO 4.10 | 45 × 90 × 75 | PT 2.5-TWIN-PE | 5.2 × 72 × 37 |
| SK 3110.000 | 33 × 67 × 38 | 195740 | 50 × 107 × 70 |
| 533356 | 45 × 90 × 70 | 550208 | 50 × 107 × 70 |
| 5SY4204-7 | 18 × 90 × 70 | 552286 | 18 × 107 × 70 |
| SK 3102.000 | 50 × 120 × 45, screw | 533361 | 40 × 107 × 70 |
| 571334, 532216 | on parent, no dimensions | 533354 | 20 × 107 × 70 |

**Placements** (`s` in mm on the rail; a strip's `s` is its left end):

| Rail | Device (s) |
|---|---|
| R1 | -102G1 (96), -101F1 (140), -102F1 (158), -102F2 (189.5), T55 (228.5), S51 (267.5) |
| R2 | 1X1 (30), -X1031 (58.6), -X1032 (172.6) |
| R3 | -201U1 realising 195740 (45), -201U2 (79), -201U3 (108), -201U4 (148), -201U5 (188), -201U6 (228), -201U7 (268), -201U8 (308), -202U1 (353), -202U2 (398), -202U3 (428) |

R1 and R3 abut from 61 mm and 20 mm (-102G1's clip at 96 mm and -102F1's at 158 mm are doc 12 §2.3's and §12.4's
examples). On R2 the strips sit 10 mm apart and 10 mm in from 20 mm, one end-bracket width at each strip boundary and
run end, so the four brackets WP-S4.02 derives (FR-LAY-060) take the gaps without moving a pose. -201U9 and F55 stay
unplaced (EF-05); A-R inserts them.

**Acceptance.**
*As built (cycle 40, ID-932):* LQ-1 to LQ-6 met (`crates/ew-api/tests/golden_layout.rs`, `crates/ew-derive/tests/layout.rs`, the
byte-equal golden tests); LQ-6's golden change as D13's as-built text lists it.

- **LQ-1** Golden A holds one layout space for +J-E-2416, the enclosure -UC1, one plate surface, the eight carriers and
  the twenty placements of §6.1; (a) and (c) byte-identical; path (b) gives the same records except `id` and
  `source_ref`; `tables_from_doc20.py --check` passes.
- **LQ-2** `unplaced` of the space is exactly F55, E55, E56, S56, -201U9; -XPE, -3X1, the valves, the terminals, the
  cables, -UC1, -UC1-plate and the +FIELD valves are not in it, each asserted with its D8 case; a device of another
  location is never listed (FR-LAY-001).
- **LQ-3** `placement_geometry`: -102G1's footprint is (101, 675)–(171, 805) mm; -X1031's is (98.6, 444)–(202.6, 516)
  mm, both in the surface frame (*amended in cycle 40, ID-924:* the earlier (61, 675)–(131, 805) and (58.6, 444)–
  (162.6, 516) were not in the surface frame);
  flipping -102G1 gives the same rectangle; a screw part on the surface rotated by 90° swaps W and H about its origin;
  a placed device without dimensions gives the unknown 20 × 20 mm square.
- **LQ-4** Assigning -102F1 a 27 mm-wide test part whose `rail_offset` is (9, 42.5) mm, the 18 mm part's clip point
  (D7), widens its rail-occupancy interval to s = 149 … 176 mm (in the surface frame (189, …)–(216, …); *amended in
  cycle 40, ID-924:* the interval is in s, not in the surface frame) at the same pose and
  `rail_occupancy(R1)` shows its interval overlapping -102F2's.
- **LQ-5** `layout_label` of -X1031 is "-X1031" (abbreviated) and "=P01+J-E-2416-X1031" (full); T55 renumbered to T57
  changes its label and no file under `layout/` (FR-LAY-006, D-02).
- **LQ-6** Every exported output of Golden A (PDF, PNG, text, rows, CSV) byte-equal; the golden change is exactly
  D13's part 2 list; twice-run determinism; targeted checks for `ew-derive`, `ew-import-tables`, `ew-testkit`,
  `ew-api`.

## 7. Part 3: the panel page

**Scope.** D9, D10. **1.2 pw.**

- The `surface_view` page item variant and `page.surface_view.set`; Golden A's 902 gets its view (a fixture choice:
  `datum_at` = (25, 167) mm, so the enclosure outline spans (20, 72)–(95, 172) mm on the sheet, below the PART DATA
  box, which ends at 43.7 mm; `label_mode` abbreviated), through the tables.
- The `surface`, `carriers`, `placements` and `labels` layers of `page_display_list` for panel pages, each its own memo
  family; label fitting in the scene with `shaped_text`; the title-block value of D9.
- The window renders 902 through the same layers (no new view code); hit-testing returns placements and carriers.

**Acceptance.** **LP-1** the surface view round-trips; a second one on a page and one on a schematic page are refused (*as built, cycle 41, ID-951:* a second set replaces the view in place, so a second view is never added; the schematic page is refused).
**LP-2** 902 draws the enclosure outline (20, 72)–(95, 172) mm and the plate (25, 77)–(90, 167) mm at ±0.2 mm, the
rails with their axis lines, the ducts with "40×60", every placement's footprint (FR-LAY-011: a `front_2d` fixture
draws its outline instead). **LP-3** the labels read "T55", "S51", "1X1", "-X1031", "-X1032", "-201U3" in `xref.device`
at 1.83 mm em, none scaled; "-101F1" is rotated inside its 1.8 mm-wide footprint (FR-LAY-062); `label_mode` full
prints "=P01+J-E-2416-X1031"; a fixture with a 6 mm part gives a leader. **LP-4** the title block reads "Cabinet
Layout · Scale 1:10"; the TOC row of 902 is unchanged; setting 902 to 1:5 changes no record under `layout/` and keeps
the label height (FR-LAY-002, FR-LAY-010). **LP-5** a `layout.move` rebuilds only the `placements` and `labels`
families of 902, never `frame` or `tables` (memo counts); clicking a footprint selects its placement. **LP-6** 902's
expected files change as D13's part 3 list says, every other page byte-equal; renaming T55 to T57 changes folios 101,
901, 902 and 910 in one commit (D-02; doc 12 §16.1 item 6, doc 20 §19; *amended in cycle 41, ID-956:* the set is 101, 901, 902, 907 and 910.2, since 910 does not print T55). **LP-7** targeted checks for `ew-schema`, `ew-commands`,
`ew-derive`, `ew-scene`, `ew-api`.

## 8. Part 4: layout objects in the BOM, the legend and PART DATA (parts 4a and 4b)

*Cut in cycle 41 (ID-949).* The feasibility reads of part 4's brief found three gaps: nothing drew an embedded report
block but the cover's (§2.2), LB-2's 30 rows did not fit its rect at the form's row height, and its export clause had
no named path. Part 4 is therefore built as **part 4a** and **part 4b**, in that order; part 5 does not depend on
part 4b. Part 4a keeps the acceptance IDs LB-1, LB-2 (as cut), LB-4 and LB-5; part 4b takes LB-3 and LB-6 to LB-10.

### 8.1 Part 4a: layout objects in the BOM and the legend query

**Scope.** D11 with its as-built rulings, FT-204 (rows 22 to 25), FT-205: the carrier part lines and the layout
objects' folios; `layout_legend(surface)` in `ew-derive` with its rows and F55's renumbering; the `enclosure_legend`
report kind registered in `ew-reports` with rows from that query and placed on no page (no `report_blocks.csv` row).
Not in part 4a: drawing any embedded block, the legend's rect, its export, PART DATA's sources (part 4b). **0.7 pw.**

**Acceptance.** **LB-1** the BOM's rows 24 and 25 read 1.71 m and 3.17 m from the carriers, tagged -UC1; -UC1 carries
no rail or duct part use; rows 22 to 25 print folio 902; the row count stays 27 (AC-07) and every other cell of 901
and 901.1 is byte-equal. **LB-2** *(as cut)* `layout_legend` of Golden A's plate surface and the `enclosure_legend`
rows list 30 rows: -UC1, -UC1-plate, R1 … R3, D1 … D5, then the twenty placements by DT, at positions 1 … 30; placing
F55 on R1 (fixture) adds one row and renumbers the rows after it; the kind has no block in Golden A. The block, its
rect and its export moved to LB-7 and LB-8. **LB-4** a free graphic on 902 appears in neither the BOM nor the legend
nor `unplaced` (FR-LAY-008). **LB-5** the golden change is exactly D13's part 4a list; targeted checks for
`ew-derive`, `ew-reports`, `ew-api`.

### 8.2 Part 4b: the embedded report blocks drawn

**Scope.** D16 to D18 and LB-3: every embedded report block drawn through one path; PART DATA's sources on a page
with a surface view; the legend block on 902; PART DATA drawn on 201, 202 and 902; the legend's CSV through the
named-block export. **0.55 pw** (0.35 pw of it from D15's reserve).

**Build list.**
1. `crates/ew-reports`: the `ReportKindDef` method of D16 (printed columns with weights, title, flow columns; the
   default every column at weight 1, no title, one column); `part_data`'s layout and sources (D16) and its leaving out
   of `LineSource::Carrier` lines; `enclosure_legend`'s layout (D17).
2. `crates/ew-scene/src/embedded.rs`: the variant `EmbeddedContent::Table` and its lowering (D16's geometry, flow
   columns, `TooManyRows`); the column split and cell rule shared with `table.rs`, whose generated-page tables stay
   byte-equal.
3. `crates/ew-api/src/output/view.rs`: `PageEmbeddedOf` over every embedded block (D16); the module documentation's
   "PART DATA blocks are not drawn yet" goes.
4. Golden A: doc 20 §17.3's "Legend box" row (D17's printed rect ▲) and the generator's embedded list, then
   `tables_from_doc20.py` and its `--check`, which write the `legend-902` row; the fixture by its bless test; path (b)
   of the test DSL creates the same block.

**Acceptance.** **LB-3** PART DATA on 902 lists exactly NSYPLA1074G and NSYPMM107 (no carrier line) and draws them
under the title "PART DATA". **LB-6** one path: folio 1 is byte-equal; a fixture with an embedded block of a kind that
declares no layout (a `wire_list` block on a schematic page) draws every column; a block whose rows do not derive
draws its reason; a fixture with more rows than the rect holds draws the capacity and reports `TooManyRows`; lowering
twice gives the same items. **LB-7** the legend block on 902 draws the rect of D17 with the title "LEGEND", two flow
columns, each with its header, and Golden A's 30 rows as 20 + 10; placing F55 on R1 (fixture) draws 31 as 20 + 11 and
renumbers; no text of the legend reports an overflow; LP-5 still holds (a `layout.move` rebuilds no `tables` family
of 902; if the legend's rows make it rebuild, the lane stops and reports). If Golden A's rows do not fit the capacity
of D17, the lane stops and reports. **LB-8** `ew export report --block <legend-902's ID> --format csv` writes a header
and 30 rows with every column of the kind, the description included; `ew export bom`, `cables`, `wires` and
`terminals` without `--block` are byte-equal. **LB-9** PART DATA on 201 and 202 draws "Order Code 1"
51E-F13GCQP-D+H2GG and "Order Code 2" 32P-VGL-R-MAHAHAHAHAHAHUL3LAHU-28MS under the title "PART DATA", with no
overflow finding (an overflow is a stop: the orchestrator rules the column weights); their rows are unchanged. **LB-10** the golden change is exactly D13's part 4b list, every other
page byte-equal; twice-run determinism; targeted checks for `ew-reports`, `ew-scene`, `ew-api` (and `ew-derive` if a
lookup is added there); the snapshot gate and both blind selftests.

## 9. Part 5: the EW-LAYOUT rules

**Scope.** D12, FR-CHK-005's EW-LAYOUT row, the AC-21 subset. **0.8 pw.**

**Acceptance.** **LC-1 (EF-05)** EW-LAY-001 on Golden A reports F55, E55, E56, S56 and -201U9 as
`expected/findings_mvp.csv` states; placing F55 removes its finding; a project without a layout space reports none.
**LC-2** removing 550208 from -201U1 changes nothing; removing 195740 gives one EW-LAY-003 (FR-LAY-059). **LC-3** a
device of +J-E-206 placed on the plate gives EW-LAY-004 (FR-LAY-018). **LC-4** LQ-4's wider -102F1 gives one EW-LAY-006
against -102F2; a placement over D4 and a rail across D2 give one each; Golden A gives none. **LC-5** EW-LAY-010 on
loaded fixtures: a TS 32 rail with a TS 35 device, a pose beyond the rail's end, a footprint outside the plate, -X1031
grown past R2's end by added terminals, a second enclosure in a space; Golden A gives none. **LC-6** swapping -201U3 and
-201U4 on R3 gives one EW-LAY-016 (FR-LAY-061's acceptance); Golden A gives none. **LC-7** the catalogue rows, rule
pages and messages exist (`cargo xtask docs-gen` clean); the AC-21 subset gains EF-05; `checks_part5.rs`' pending list
loses EF-05; the release scheme counts EW-LAY-001 at error only where a layout space exists (doc 05 §14.1). **LC-8**
targeted checks for `ew-checks`, `ew-api`.

## 10. Files owned and what a lane may not decide

### 10.1 Files owned

**Part 1:** `crates/ew-schema/src/{layout.rs,kind.rs,lib.rs}`, `schemas/0.1/**`; the layout ownership and follow code
in `crates/ew-format/**` and `crates/ew-store/**`; `crates/ew-model/**` (tables, indexes, `invariants.md` §4);
`crates/ew-derive/src/layout/**` and its data table; `crates/ew-commands/src/commands/{layout.rs,assign.rs,mod.rs}`,
`crates/ew-commands/tests/**`. **Part 2:** `crates/ew-derive/src/layout/**`, `crates/ew-import-tables/**`,
`crates/ew-testkit/src/golden/**`, `golden/J-E-2416/{tables,tools,README.md}`, `golden/J-E-2416.ew/**`, and in
`docs/design/20-golden-project.md` only §13 (§6.1's tables) and one row of §23.1. **Parts 3 and 5:** named by crate in
their scope and fixed by each part's dispatch brief; golden files only as D13 lists for the part. **Part 4a:**
`crates/ew-derive/src/bom.rs`, `crates/ew-derive/src/layout/**`, `crates/ew-reports/src/kinds/` (the
`enclosure_legend` kind and its `mod.rs` line; `parts.rs` only where a carrier line needs it), `crates/ew-derive/tests/**`,
`crates/ew-reports/tests/**`, `crates/ew-reports/README.md` (the `enclosure_legend` row), and the golden files, tests,
generator rows and doc 20 §14.1 lines of D13's part 4a list. **Part 4b:** `crates/ew-scene/src/{embedded.rs,table.rs}`,
`crates/ew-scene/tests/**`; `crates/ew-api/src/output/{view.rs,layers.rs}`, `crates/ew-api/tests/**`;
`crates/ew-reports/src/{kind.rs,kinds/part_data.rs}`, the `enclosure_legend` kind's file, `crates/ew-reports/tests/**`;
`crates/ew-derive/src/layout/**` only for a lookup of the surfaces a page shows, where part 4a's folio lookup does not
serve; the golden files of D13's part 4b list, doc 20 §17.3's one new row and the generator's embedded list.

Shared append-only files (registry lists, the fuzzer arms, DSL verbs, the strings table, `schemas/0.1`, the rule
catalogue's generated pages) may be touched by other lanes; the second to integrate rebases. Read-only for every lane:
other crates, `golden/**` except as named, `ci/**`, `.github/**`, `docs/**` except generated reference pages and part
2's doc 20 rows. Each lane commits on its own branch and does not merge or push.

### 10.2 What a lane may not decide

A lane stops and reports instead of choosing when:
1. a change needs a new workspace package, a new dependency edge in `ci/dep-rules.toml` or a new external crate;
2. a member, kind or variant beyond D3 and D9 would be typed, or a required member added;
3. any exported byte of Golden A, AC-15's path (b) or the acceptance result changes outside D13's list for its part;
4. a golden fact here (EF-05's five devices, the twenty placements, the 1.71 m and 3.17 m, the 30 legend rows, the
   footprints of LQ-3) is contradicted by the data, or a §6.1 value would need changing;
5. a command beyond D6 and D9, or a refusal beyond D6's, would be added, or `layout.place` would snap or push;
6. a placement would change without a command naming it (beyond D4's cascade and D7's follow);
7. the work of a later part, or of WP-S4.02, is reached (D14, D1).

The lane decides freely: private helper names, module layout inside the owned files, the exact notice and refusal
texts beyond those quoted here, test fixtures, the column names of the new golden tables within the README's dialect.

## 11. Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 Typing the layout kinds breaks file ownership or incremental-save equality | Medium · high | LM-1's property test; the follow relations are doc 05's; Opus at `xhigh` for part 1 |
| R2 The mechanical data moves the part-copy hashes and something prints differently (EW-CORE-014, a library-update preview) | Medium · medium | LQ-6 compares every exported output; §10.2 item 3 stops the lane |
| R3 Generated 902 looks unlike the reference (true scale, rails drawn) | High · low | Doc 20 §3.4 row 17 already lists 902 as an intended difference; the values are ▲ and PQ-24 asks the pilot |
| R4 Label fitting differs between hosts | Low · medium | Integer metrics from `ew-text`'s bundled fonts; both gates run the 902 goldens |
| R5 The legend overflows its block | Medium · low | D17's two columns hold 40 rows against Golden A's 30; LB-7 stops the lane; the rect is a fixture choice the orchestrator may move |
| R7 Drawing PART DATA on 201 and 202 moves bytes beyond the box (part 4b) | Low · medium | LB-10 compares every page; the box is drawn into the existing rect only |
| R6 Scope creep into WP-S4.02 (snapping, brackets, drag) | Medium · medium | D1 and §10.2 item 7 |

## 12. Size, order and routing

**Size.** 5.1 pw of the 5.7 pw PERT, 0.6 pw reserve (D15): part 1 **1.1** (records and storage 0.4, invariants 0.15,
commands 0.35, tests 0.2), part 2 **1.1** (queries 0.5, golden data through paths (a) to (c) 0.45, tests 0.15), part 3
**1.2** (view item 0.15, layers 0.45, labels and fitting 0.3, title block 0.1, golden 0.1, tests 0.1), part 4 **0.9** as first sized
(carrier lines and folios 0.4, legend 0.3, PART DATA 0.1, golden 0.1), *cut in cycle 41 (ID-949)* into part 4a **0.7**
(carrier lines and folios 0.4, legend query and kind 0.2, golden 0.1) and part 4b **0.55** (the drawing path 0.25:
scene variant, flow columns and box 0.15, assembly and kind layouts 0.1; legend block 0.1; PART DATA sources and
drawing 0.1; golden 0.1), 1.25 pw against 0.9, the 0.35 pw excess (the drawing path and 4b's golden change) taken from
D15's reserve, part 5 **0.8** (six rules 0.45, catalogue,
pages and messages 0.2, tests 0.15). **De-scope rule:** when a part is projected (at its lane's midpoint report) or
found (at its review) more than 20 % over its size, these items leave as follow-ups without a further ruling, in this
order: the leader step of D10 (labels rotate or overflow, reported), EW-LAY-016, `rehome_to_surface`, the `front_2d`
outline (footprint rectangles only), EW-LAY-004; for part 4b, the drop shadow, then the default layout for kinds
other than `part_data` and `enclosure_legend` (an FT row "embedded report blocks of other kinds are drawn on no page"
is then opened); the lane reports which ones left.

**Order and routing** (Opus or Haiku only, ID-700): part 1 first, **Opus at `xhigh`** (format typing, file ownership
and invariants; cycle 40); part 2 after part 1, **Opus at `high`** (core queries and a golden change; it may run as the
next wave of the same cycle); parts 3 and 5 after part 2, side by side, part 3 **Opus at `high`** (the page output and
a golden change), part 5 **Opus at `medium`** (rules on WP-S1.16's pattern); part 4a after part 3, **Opus at `high`**
(the BOM and a golden change); part 4b after part 4a, **Opus at `high`** (a page output path and a golden change on
three folios; its feasibility read is flagged for Opus `low`); part 5 does not wait for part 4b. The feasibility read before dispatch flags parts 1 and 2 (format, golden data).
**AC-33** passes after parts 1 to 5 except rail R2's end brackets, which WP-S4.02 adds (FR-LAY-060); AC-33 is shared
with WP-S4.02 in the traceability. **Unblocks:** WP-S4.02 after parts 1 to 3; FT-486 (the Unplaced devices navigator)
after part 2; WP-S4.05 and WP-S4.07 build on part 1's records.

## 13. Follow-ups and the documents to update

**Follow-ups.** FT-205 closes with part 4; FT-204 closes for rows 22 to 25 with part 4 and stays open for row 26 until
WP-S4.02 derives the end brackets; FT-486 stays with WP-S4.02. Proposed (not opened; the design pass numbers them):
- **FP-1** WP-S4.02 gains the window gestures doc 12 §12.1 lists and no package row names: create the layout space and
  the enclosure from a part, draw a rail, draw a duct, set the surface view, and the placement's property-grid rows.
- **FP-2** Generic layout parts in `ew-starter-parts` (an enclosure, a plate, a TS 35 rail, a 40×60 duct, an end
  bracket) and L3 dimensions on the generic device parts, so a new project can lay out a panel (with FP-1).
- **FP-3** FR-LAY-062's balloon step with the legend positions (1.0, with doc 12 §5.2's balloons).
- **FP-4** The cutting increment of the shop policy on carrier lengths, in the BOM and the cut list (FR-MFG-030, 1.0).
- **FP-5** A carrier designation member (R1, D1) for the legend's DT cell, a format member doc 03 §16 does not name:
  for the schema steward at 1.0.
- **FP-6** End brackets derived as group accessories (FR-LAY-060, WP-S4.02): the strips' authored bracket part uses
  leave, BOM row 26 prints 902 (FT-204's rest), and AC-33 completes.
- A de-scoped item of §12 becomes a new FT at the design pass.

**Documents.** **After part 1:** doc 03 §16 and §4.2 (as built: typed members, MVP limits, policies), doc 05 §2.2 (the
kinds implemented), doc 12 §2.2, §2.3, §3.7 (as built), arch 04 §3's `ew-panel` row and doc 12 §1.2 (D2: the MVP
queries in `ew-derive`). **After part 2:** doc 12 §2.6, §2.7 (the predicate as built, -XPE by its FnDef category) and
Open question 3 (resolved by §6.1); doc 20 §13 (the lane), §23.1. **After part 3:** doc 12 §5.1 to §5.3, doc 06 (the
page item variant and `page.surface_view.set`), doc 09 §4.1, 02b FR-LAY-062 (balloons at 1.0, D10), doc 20 §17.2
(the scale in the description line). **After part 4a:** doc 09 §2.2 (`enclosure_legend` as built), doc 20 §13 and §14.1 (the
as-built notes of ID-316 and ID-317 superseded). **After part 4b:** doc 09 §2.2 and §4.1 (embedded blocks of every kind
drawn, the kind's embedded layout, PART DATA's sources), doc 12 §5.4 (the legend block as built), doc 20 §13 (the
legend box) and §17.3 (its row, written by the lane as the golden source), doc 17 §3.3 (the legend's CSV through
`ew export report --block`). **After part 5:** doc 15 §4.5.4 (rule pages), doc
20 §18.1 (EF-05 no longer pending), the WP-S1.16 specification's AC-21 subset.

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S4.01, WP-S4.02, WP-S4.05, WP-S4.07; Appendix B (FT-204, FT-205,
  FT-209, FT-486).
- [Decision log](../06-decision-log.md): ID-316, ID-317, ID-334, ID-682, ID-700, ID-893.
- [Panel layout](../../design/12-panel-layout.md); [domain model](../../design/03-domain-model.md) §4.2, §16;
  [project format](../../design/05-project-format-and-persistence.md) §2.2, §10.3, §11;
  [symbols, parts and macros](../../design/07-symbols-parts-macros.md) §2.5;
  [reports](../../design/09-reports-forms-output.md) §2.2, §4.1;
  [checks](../../design/15-checks-revisions-collaboration.md) §4.5.4;
  [golden project](../../design/20-golden-project.md) §13, §14.1, §18.1, §21.
- [ADR-0004](../../adr/0004-identity-uuidv7-uuidv5.md), [ADR-0008](../../adr/0008-transactions-undo-preview.md),
  [ADR-0009](../../adr/0009-invariant-catalogue.md), [ADR-0011](../../adr/0011-format-versioning-and-migration.md),
  [ADR-0015](../../adr/0015-display-list-renderers-pdf.md), [ADR-0019](../../adr/0019-rule-engine-and-rule-packs.md).
