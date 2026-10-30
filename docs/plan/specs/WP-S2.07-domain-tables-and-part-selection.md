# WP-S2.07: domain tables and part selection (closed specification)

**Status:** specified in cycle 34 (documents only), M2's first package, in four parts: part 1 the strip editor with
the strip accessories and the bridge parts that close G-PP's H4 gap, terminal and group labels, bridge selection and
the fallback bridge (1.4 pw, D4), part 2 the edit-cable table and the generic navigator row operations (0.6 pw, D3),
part 3 the PLC navigator with drag-bind and the PLC grid (0.7 pw, D3), part 4 the part selection dialog and the part
editor on WP-S1.19's catalogues (0.9 pw, D4); part 1 may run in cycle 34's later wave; parts 2 and 3 follow in any
order; part 4 waits for WP-S1.19's catalogues · **Part 3 built in cycle 38** (lane D38, decision log ID-877; racks, bus ports, the safety and bus facets and four columns de-scoped, FT-737 to FT-739; PG2 on a starter project, FT-740) · **Part 4 specified in cycle 39** (decision log ID-894; §9 closed to its decisions D16 to D24, §11.3's PS1 to PS10; split into part 4a, the part selection dialog and the part editor, 0.9 pw, first, and part 4b, FT-396's paste choice, 0.2 pw, after it; the package's PERT is exceeded by 0.2 pw, §14.1) · **Date:** 2026-10-29 · **Work package:**
[WP-S2.07](../02-work-packages.md#wp-s207-domain-tables-strip-cable-and-plc-editors-part-selection) ·
**Decision log:** ID-782 · **Part of:** ElektraWatt plan, specifications

**Purpose.** This is the closed specification of the package that gives the MVP its domain tables: the terminal strip
as an editor (insert, delete, move, re-designate, group labels, accessories, bridges and their parts, placing unplaced
terminals), the edit-cable table, the PLC navigator and grid, and part selection with a part editor. It is written
against **WP-S2.06's delivered table framework and WP-S2.04's and cycle 33's delivered bridge, part-chooser and
placement tools** (§2.1), not against their code. Where this document and a design document differ, this document is
the narrower and wins for the lanes; the design documents stay the target of the whole work package, and the documents
named in §14.4 are updated at the design pass after each part. A choice a lane meets that this document does not settle
is escalated, never guessed (`CLAUDE.md`, rule 2).

**As built (cycle 34).** **Part 1 is built** (lane H34): `terminal.insert` and `terminal.move` (58 commands),
`strip.accessories` through `fields.set`, the derived query `strip_bridges` with the bridge BOM line and
`bridge_groups`, the part resolver's kinds and description form, the strip editor below the shell
(`Session::strip_edit`), and the six actions of §6.9 in the shell (268 actions; with WP-S1.19 part 1's word, 357
describe words). One item of part 1 is **not built and moves to part 2**: §6.8 (Enter on an unplaced terminal row
dispatching `place.pick`, and the row as a drag source onto a canvas). **Alt+Up and Alt+Down are `strip.move`
(`step` −1 and +1) while the Terminal strips navigator has the focus, and `pages.move_up` and `pages.move_down`
elsewhere** (the lead's ruling, built in the cycle's fix round, ce9bad22: the shell maps the page-move chords to
`strip.move` in that focus, so the keymap keeps one binding per chord). SE7's order is corrected below to the
as-built natural order of the tags, which the tests assert.

**As built (cycle 35).** **Part 2 is built** (lane C35, decision log ID-819), with §6.8 from part 1: the Cables
table's derived potential, shield and pair columns after the report's columns, core, shield and pair editable on the
conductor, the typed `cable.length` (`12.5 m`, `missing`, `from part`), `cable.swap_cores` in the Edit menu (the table
has no row menu), Delete in navigator trees through the card (`navigator.delete`) and F2 rename on page titles and
potential names only (`navigator.rename`; the structure-code and device-tag renames are FT-633), and §6.8's Enter and
drag on an unplaced terminal row; FT-600 (`strip.add_accessory` with `before`), FT-605 and FT-618 (a terminal with no
strip opens the strip chooser), FT-621 (bridges in `page.describe`). No part-2 specification pass ran (D15); the
lane's choices are accepted as built. FT-622's stacking rule was built by the follow-up lane (ID-822): a terminal
placed on a point of its strip's terminal moves two grid steps right until free, with no autoconnect wires. FT-608
stays open. SE9's -X1031:15 is not unplaced (FT-630). Since WP-S1.16 part 4, EW-TERM-004 refuses a `terminal.move` or
`terminal.insert` that leaves a bridge group over terminals that are not consecutive, so §5.2's move of -X1031:1 to
the end is refused on Golden A (FT-641). 275 actions and 359 describe words at the cycle's tip.

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs](#2-inputs)
3. [Decisions](#3-decisions)
4. [Crates, boundaries and the public items](#4-crates-boundaries-and-the-public-items)
5. [Commands](#5-commands)
6. [Part 1: the strip editor](#6-part-1-the-strip-editor)
7. [Part 2: the edit-cable table and the navigator row operations](#7-part-2-the-edit-cable-table-and-the-navigator-row-operations)
8. [Part 3: the PLC navigator and grid](#8-part-3-the-plc-navigator-and-grid)
9. [Part 4: part selection and the part editor](#9-part-4-part-selection-and-the-part-editor)
10. [Determinism, the headless mode and exact counts](#10-determinism-the-headless-mode-and-exact-counts)
11. [Acceptance checks](#11-acceptance-checks)
12. [Files owned and what a lane may not decide](#12-files-owned-and-what-a-lane-may-not-decide)
13. [Risks](#13-risks)
14. [Size, order, routing and the documents to update](#14-size-order-routing-and-the-documents-to-update)
15. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of part 1 a terminal strip is edited as a table: its terminals and accessories in mounting order,
with the designation, the printed label, the group, the part, the potential, both sides, the bridge and the bridge's
part, the state; a terminal is inserted between two others, deleted, moved, re-designated and placed on a page from
its row; an end bracket or another strip accessory is added at a position; terminals are bridged and unbridged however
they were selected; and every gesture is one command and one undo step. The blind drafter can then enter the two
accessory rows of G-PP's H4 gap (the TBD end brackets ×4 and the plug-in bridges ×3 of task `gpp-101-103`), which
the parts list counts. At the end of the package the cable's cores are swapped in one step, the PLC navigator shows its
tree and binds a channel by drag, the PLC grid edits its columns in bulk, and parts are chosen in a dialog with search
and filters and edited in a validating part editor. Everything runs headless through `ew.uiscript/1`, reads back
through the transcript and `ui.describe`, and works by keyboard alone (NFR-A11Y-01).

**Non-goals** (each is named with its package in §14.3):
- No bridge proposals between adjacent terminals of one potential (FR-TRM-008, 1.0, WP-S1.26).
- No strip length, rail cut length or accessory widths (FR-TRM-024, 1.0, WP-S1.26); no terminal assembly list
  (FR-MFG-016, 1.0).
- No multi-level generation ("generate levels" of FR-TRM-013): multi-level terminals have one row per terminal until
  the terminal diagram models levels (doc 08 §6.3; the terminal diagram's "not modelled yet").
- No rack and slot editor (WP-S2.13); no plug diagrams (FR-TRM-023, 1.0).
- No several strip editors side by side (doc 16 §3.4's instances): the strip editor is the Terminal strips navigator
  (D1), one per window.
- No migration of Golden A's interim accessory and bridge part uses (decision log ID-316) onto strip accessories and
  bridge parts (D6; a follow-up of §14.3).
- No change to any exported byte of Golden A, to AC-15's command log or to the acceptance result.

## 2. Inputs

| Input | What the lanes take from it |
|---|---|
| [WP-S2.06's specification](WP-S2.06-navigators-and-table-mode.md) §6 to §8 as built | The navigator framework, `Session::navigator_table`, `NavTable`, `NavNode`, `NavColumn`, the facets, `SortBy`, table mode with its cell cursor, editing, fill-down, pattern fill, paste, copy, the compile rule (§8.3), the policy, the table card, `row_targets`, the 50-row describe window |
| [WP-S2.05's specification](WP-S2.05-property-grid-find-replace-clipboard.md) §4.2, §6 as built | The field catalogue and `SETTABLE`, `parse_field_input`, `InputError` (with cycle 33's `UnknownPart`), `field_choices`, `fields.set`, `fields.set_many`, the property sheet and its rows, the toast and the card |
| [WP-S2.04's specification](WP-S2.04-placement-wiring-drawing-tools.md) §7.2, §10.7 as built; cycle 33 (decision log ID-770, ID-771, ID-780) | `place.pick { function }` arming `existing_function`; `strip.bridge` (`ShellModel::bridge_command`, `crates/ew-viewmodel/src/shell/bridge.rs`) with `connection.draw` and `connection.draw_many`; the side-1 fallback; the minimal part chooser (`parse_part_uses`, `resolve_part`, `part_choices` in `crates/ew-api/src/output/fields.rs`); `resolve_nav_key` |
| Code at dad060e6 | `crates/ew-schema/src/{device.rs,connect.rs,library.rs}` (`Strip`, `StripAccessory`, `AccessoryKind`, `TerminalGroup`, `Terminal`, `BridgeSpec`, `PartKind`); `crates/ew-commands/src/commands/{terminal.rs,fields.rs,edit.rs}`; `crates/ew-derive/src/bom.rs` (`DevicePartLines`, `LineSource`); `crates/ew-reports/src/kinds/{conn.rs,terminal_diagram.rs,wire_list.rs,parts.rs}`; `crates/ew-api/src/output/{navigators.rs,fields.rs,properties.rs}`; `crates/ew-viewmodel/src/{navigators/table.rs,shell/table.rs,shell/bridge.rs,shell/properties.rs,actions.rs,describe.rs,strings.rs}` |
| [Connections, terminals and cables](../../design/08-connections-terminals-cables.md) §2.3, §6, §9 | `BridgeSpec.group` (one part per group, on the first connection's `overrides.wire_part`), `ClampKey`, strip groups, the printed label, the strip editor touchpoint, the edit-cable table |
| [Domain model](../../design/03-domain-model.md) §11, §12 | Strips, terminals, groups, accessories; cables and conductors |
| [UX/UI](../../design/16-ux-ui.md) §3.4, §5.2, §7.1, §7.2, §18.1 step 7; [design language](../../design/16a-design-language.md) §6.21, §6.22 | The Terminal strips navigator, table mode, the edit → command table (row insert, delete and move as domain commands), the data grid's look, the chooser list |
| [Golden project](../../design/20-golden-project.md) §9, §14.1; `golden/J-E-2416/tables/{devices.csv,connections.csv,parts.csv}` | -X1031's 20 terminals, groups and BR1 to BR5; -X1032 and BR6; the BOM rows 26 and 27 as built (part uses on the strips, ID-316) |
| Blind task `tools/blind/tasks/gpp-101-103/` (`oracle/bom.csv` rows 13 and 14, `gaps.toml`) | The H4 rows part 1 makes enterable: `End bracket` ×4 and `Plug-in bridge` ×3, both part number TBD, part kinds `end_bracket` and `accessory` |
| Requirements | The 7 rows of the work package: FR-TRM-013, FR-TRM-015, FR-CBL-016, FR-PLC-020, FR-PLC-021, FR-PRT-015, FR-PRT-022; with FR-TRM-002, FR-TRM-003, FR-TRM-007, FR-TRM-010, FR-TRM-016 (in part, D5), FR-TRM-021, FR-TRM-022, FR-TRM-026, FR-DEV-018, FR-UI-008, FR-UI-044, NFR-A11Y-01, NFR-REL-07 where they touch it; FT-394, FT-396, FT-429 (in part), FT-490, FT-548, FT-553, FT-576, FT-585 |

### 2.1 What the earlier packages provide (the base contract)

The lanes rely on exactly these delivered items and on nothing of their code beyond them. If a delivered item differs,
the lane stops and reports it.

| Item | Where | Used here for |
|---|---|---|
| The Terminal strips navigator (`NavKind::Strips`): strip nodes, terminal rows from each strip's `terminal_diagram`, the leading `strip` column, the editable columns `terminal`, `label`, `state`, the derived `potential` | WP-S2.06 §7.1, §8.2 | The strip editor's rows and columns (D1) |
| Table mode: `navigator.table`, the cell cursor, `table.edit`, `table.commit`, `table.set`, fill-down, pattern fill, paste, copy, clear, the card with `table.resolve` and `table.drop` | WP-S2.06 §8 | Every cell gesture of the strip editor, unchanged |
| The facets and `navigator.filter`; `SortBy` | WP-S2.06 §6.3 | The `strip` facet and the `Order` sort (D2) |
| `Selection { items, primary, generation }`; navigator → canvas synchronisation (records when nothing is drawn on the shown page) | WP-S2.06 §6.5 | Bridge selection by records (D9) |
| `strip.bridge`, its reason keys `reason-bridge-selection`, `reason-bridge-strips`, `reason-bridge-no-shaft`, `reason-bridge-exists`, and its group rule | cycle 33, ID-771 | Widened by D9 and D10 |
| `terminal.add`, `terminal.designate`, `edit.delete`, `fields.set`, `fields.set_many` | `ew-commands` | Delete rows, re-designate, the compiled edits |
| `DevicePartLines` with `LineSource::{Device, StripAccessory, AssemblyConfig}` | `ew-derive` `bom.rs` | Strip accessories are already counted; bridge lines are added (D6) |
| The Parts row's part chooser: `resolve_part`, `parse_part_uses`, the display form `<part number> (<manufacturer>)` | cycle 33, ID-770 | Widened by D7 |
| `place.pick { function }` | WP-S2.04 §7.2 | Placing an unplaced terminal from its row (§6.8) |

### 2.2 Gaps found below the shell (why part 1 is shaped as it is)

| Gap at dad060e6 | Consequence |
|---|---|
| `Strip.accessories` exists in the schema and the BOM counts it, but the catalogue routes it `read-only:no-command` (it is not in `SETTABLE`) | `strip.accessories` becomes settable (D5); no new accessory command |
| A bridge group's part has a home in the design (`overrides.wire_part` of its first connection, doc 08 §2.3) and a settable path, but the BOM does not read it | `DevicePartLines` gains one line per bridge group with a part (D6) |
| `resolve_part` refuses two copies with one part number ("use id:<uuid>"), and the blind drafter may not type UUIDs; the task's two accessory parts are both `TBD` | Part text filtered by part kind and a description form (D7) |
| Nothing inserts a terminal between two others or moves one: `terminal.add` appends, and `fields.set` on `terminal.seq` does not keep sequence numbers unique | Two commands, `terminal.insert` and `terminal.move` (§5) |
| Groups are a list on the strip; no row of a terminal shows or sets its group | The group compile (D8) |
| `strip.bridge` reads only representations on the focused page and compares bridge ends by CP | D9 (selection) and D10 (terminal identity) |
| The Terminal strips table sorts by label across strips; accessory rows do not exist | D2, D3 |

## 3. Decisions

Each is a lead ruling that the owner may reverse. The design pass records them under ID-782 (and the as-built entries
after each part).

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **The strip editor is the Terminal strips navigator in table mode, opened on one strip** by `strip.edit` (Edit ▸ Strip editor), which shows the navigator, switches it to table mode and sets the filter to `strip:<tag>`. No separate pane kind; the strip-editor actions of §6.9 act on that navigator's cursor and marked rows, in the tree or the table | One table, one widget, one set of cell gestures, already tested and styled (WP-S2.06 part 3, 16a §6.21); the trade-off is doc 16 §6.1's side-by-side strip editors, deferred |
| D2 | **Mounting order.** The Terminal strips navigator gains the facet `strip` (its value the case-folded strip tag) on strip, terminal and accessory nodes, the own column `pos` ("Pos", after `strip`) and a sort `SortBy::Order`, the provider's node order (strips in natural tag order, then each strip's rows in mounting order), which is the navigator's default sort in both views. Header clicks still sort by a column | FR-TRM-003: the sequence is the mounting order and independent of the designation; sorting by designation stays one click away. Extends WP-S2.06 D6's closed facet list by one |
| D3 | **Accessory rows.** Each `Strip.accessories` entry is a row of its strip (role `accessory`), placed after the terminal whose `seq` is the greatest not above its `after_seq` (before the first terminal when `after_seq` is 0), in list order among equal `after_seq`. Its `pos` is `<after_seq>.<k>` (k from 1 among equal `after_seq`, so 0.1, 20.1, 20.2 sort naturally between the terminals), its label and `label` cell the kind word (End plate, End bracket, Partition plate, Marker carrier, Cover), its `part` cell the part number; every other cell is empty and read-only ("accessory row"). Rows of other kinds (bridges, groups) are not added: a bridge's part is a column (D6), a group a column (D8) | Accessories are positional, so they need rows; bridges and groups span terminals, so they read best as columns on the terminals they cover (doc 08 §9's "bridge comb column") |
| D4 | **Inserting and moving terminals keep sequence numbers unique and keep everything positional in place.** `terminal.insert` shifts the sequence numbers of the following terminals, the group bounds and the accessory positions at or after the insertion point by the count; `terminal.move` permutes the sequence values of the changed range only (the rule of `page.reorder`, WP-S2.06 D8), leaving groups and accessories where they are, so a moved terminal takes the group of its new place. Designations never change (FR-TRM-013's acceptance) | A terminal's place, not its identity, decides its group and its neighbouring accessory, as on a rail; the permutation touches the fewest records |
| D5 | **Strip accessories through `fields.set`.** `strip.accessories` joins `SETTABLE`; adding, removing and choosing the part of an accessory compile to one `fields.set` of the strip's whole list, kept sorted by `after_seq` (stable). FR-TRM-016 is brought forward **in part**: the five accessory kinds at positions with their parts, counted in the BOM (already built); the strip length and widths stay 1.0 (WP-S1.26) | The schema and the BOM already carry accessories; one route and no new command is the least code; the trade-off is that the accessory list is replaced whole, which the record diff shows as one change of the strip |
| D6 | **A bridge group's part is the group's carrier connection's `overrides.wire_part`, and the BOM counts it once per group on the strip.** The carrier is the group's first connection that has a part, else its first connection, in (lower `seq` of its two terminal ends, connection ID) order. A new memoised query `strip_bridges(strip)` in `ew-derive` groups a strip's bridge connections by group (a connection belongs to the strip of its first terminal end, `a` then `b`); `DevicePartLines(strip)` appends one line of quantity 1 per group with a part (`LineSource::Bridge`, after the accessories, in group order); `ew-reports`' `bridges` is rebuilt on it with its output byte-equal. Golden A's bridges carry no part, so its BOM is unchanged; its interim part uses (ID-316) stay until a follow-up migrates them | Doc 08 §2.3 and FR-TRM-026 as designed: the part moves and disappears with its bridge, and the count is right by construction; one grouping for the BOM, the terminal diagram and the wire list. The trade-off is a double count if a user also adds bridge part uses to the strip by hand, which the BOM shows plainly |
| D7 | **Part text by kind and description.** The part resolver takes an optional set of allowed part kinds; a third display form `<part number> (<manufacturer>) · <description>` parses back; and two or more matches are `InputError::Ambiguous` with every match as a choice (no more "use id:<uuid>"). The allowed kinds: end bracket accessories → `end_bracket`; marker carriers → `marker`, `accessory`; other accessories and bridge parts → `accessory`; the Parts row → every kind, as built | The blind drafter cannot type UUIDs, and Golden A's and the task's TBD parts share the number `TBD`; the kind filter makes them unique, the description form and the choice list settle any remaining tie |
| D8 | **Group labels are edited on terminals.** Setting the group label L on a set E of terminals of one strip rewrites the strip's `groups`: each existing group that covers a member of E is split into the parts of its range that exclude E's sequence numbers (each part keeps the label and unknown members; a part that covers no terminal is dropped); groups covering none of E stay verbatim; for L non-empty, each run of E consecutive in the strip's terminal order becomes a group labelled L, merged with a directly adjacent part labelled L (an adjacent part with unknown members is not absorbed into another); the list is sorted by `from_seq` (stable). An empty L removes E from every group. The compile is one `fields.set` of `strip.groups` per strip (`fields.set_many` over several strips) | FR-TRM-010 and FT-553 on the row the user is looking at, without a range dialog; groups the edit does not touch keep their bytes, and unknown members survive |
| D9 | **Bridge selection by records (FT-548).** `strip.bridge` and `strip.unbridge` read the window selection's items of any kind: a representation with one function, a function, or a device; each resolves to the terminal function of a terminal device (a device: its main function). No canvas and no page is needed, and the enabled predicate is "a writable project", not "an editable page" | The strip editor marks records, `select.set` with `tag:` selects devices, and page-less bridges need no page |
| D10 | **A bridge joins terminals, not CPs, for every editor rule (FT-585).** Two terminals are bridged when a bridge connection joins any CP of one to any CP of the other; Bridge terminals keeps such a pair and refuses when every pair is bridged; the group rule's "touches" reads terminals. The stored CPs stay what was drawn: a side-1 fallback bridge stays on its clamps when a part with a shaft is chosen later. Re-seating onto the shaft waits for the first output that reads a bridge's CP (the terminal assembly list, FR-MFG-016, 1.0) | Closes FT-585's duplicate without a format change or a hidden normaliser; potentials, reports and the BOM do not depend on the bridge's CP, because a feed-through's Short links join clamps and shaft (doc 08 §6.2) |
| D11 | **Removing bridges never splits a group.** `strip.unbridge` deletes the bridge connections with both ends on the selected terminals in one `edit.delete`, and refuses when the deletion would leave a group in two pieces ("Unbridge from an end of the bridge, or the whole bridge") | A group is one comb (doc 08 §2.3: consecutive `seq` values on one shaft); renumbering a split piece would be a second change the user did not ask for |
| D12 | **Row deletes are previewed; cell edits keep the table policy.** Deleting terminal rows compiles to one `edit.delete` of the terminal devices and always opens the table card (it removes their wires and bridges); deleting accessory rows is one `fields.set` committed directly with the toast; a mix is refused. Insert, move and accessory adds commit directly with the toast and an undo step | A terminal delete takes wiring with it (FR-DEV-030), so the user sees it first; the other gestures are small and undoable |
| D13 | **The printed label and the group in Properties (FT-553).** For a device with the terminal aspect the property sheet lists, in its Terminal section after Designation, the `function_text` row labelled "Printed label" (its route unchanged) and a derived, editable "Group" row compiled by D8; other devices are unchanged | The label the strip prints is the terminal's function text (doc 08 §2.3, FR-TRM-021); naming it where the user looks for it was the blind run's finding #11 |
| D14 | **Inserted terminals copy their neighbour.** `strip.insert` takes the function definition and the part uses of the reference terminal (the cursor terminal, else the strip's last terminal), designates the new terminals with the next free whole numbers above the strip's highest whole-number designation (or the `designation` given, count 1), and leaves their function text, state and group empty | A strip is mostly one terminal type; the user changes the part in the Part cell when it differs |
| D15 | **Parts 2 to 4 in outline, closed before their dispatch.** Parts 2 and 3 are fixed here to the level of their commands, columns, facets and acceptance; part 4 waits for WP-S1.19's catalogue API and is re-read at that package's delivery, without reopening D1 to D14 | Part 1 is the cycle's lane; the later parts depend on packages that are not delivered |

No decision changes an ADR. ADR-0006 (a terminal is a device), ADR-0008 (one command per gesture, undo and preview),
ADR-0009 (prevent rules at transaction end) and ADR-0014 (egui, egui_table) are applied as written.

## 4. Crates, boundaries and the public items

### 4.1 Roles

1. **`ew-schema`**: unchanged. No format change in any part.
2. **`ew-commands`** (part 1): the commands `terminal.insert` and `terminal.move` (§5), `strip.accessories` in
   `SETTABLE`, their schemas, DSL verbs and fuzzer arms.
3. **`ew-derive`** (part 1): the memoised query `StripBridges` (`strip_bridges(strip)`) and `LineSource::Bridge` in
   `DevicePartLines` (D6).
4. **`ew-reports`** (part 1): `bridges` rebuilt on `StripBridges`, its output byte-equal; a public `bridge_groups(q)`
   for the navigator (the groups with their labels `BR<n>`, carriers and parts).
5. **`ew-api`** (part 1): the Terminal strips provider's `pos` and `bridge_part` columns, the accessory rows, the
   `strip` facet and the order key (D2, D3, D6); the part resolver's kinds, description form and ambiguity (D7);
   `Session::strip_edit` (§4.2), the only door through which the view model compiles a strip gesture; the terminal
   Properties rows (D13).
6. **`ew-viewmodel`** (part 1): the actions of §6.9, the strip-editor cells in the table compile, `SortBy::Order`,
   the bridge rules (D9 to D11), the describe and transcript lines, the reason strings.
7. **`ew-ui`** (part 1): accessory rows' look, the chooser list (16a §6.22, its first use), the drag source on
   unplaced terminal rows, the kittest flows.

`ew-viewmodel` and `ew-ui` still name no `ew-model` or `ew-schema` type (WP-S2.06 §4.1); no new package; no new
dependency edge.

### 4.2 The public items added below the shell (part 1)

```rust
// ew-derive
pub struct StripBridges;                       // Query: Key = Id<Device> (the strip), Value = Vec<BridgeGroup>, Memo
pub struct BridgeGroup {
    pub group: u16,
    pub kind: BridgeKind,
    pub terminals: Vec<Id<Device>>,            // (seq, ID) order, each once
    pub connections: Vec<Id<Connection>>,      // (lower seq of the two terminal ends, ID) order
    pub carrier: Id<Connection>,               // D6: the first connection with a part, else the first
    pub part: Option<PartRef>,                 // the carrier's overrides.wire_part
}
pub enum LineSource { Device, StripAccessory, Bridge, AssemblyConfig }   // Bridge: one per group with a part

// ew-reports
pub fn bridge_groups(q: &Q) -> Vec<(String /* BR<n> */, Id<Device> /* strip */, BridgeGroup)>;

// ew-api
pub enum StripEdit {
    GroupLabel   { terminals: Vec<Uuid>, label: String },              // D8; "" clears
    BridgePart   { terminals: Vec<Uuid>, part: String },               // D6, D7; "" clears
    AccessoryPart{ strip: Uuid, index: u32, part: String },            // D5, D7
    AddAccessory { strip: Uuid, after_seq: u32, kind: String, part: Option<String> },
    RemoveAccessories { strip: Uuid, indexes: Vec<u32> },
    Insert       { strip: Uuid, before: Option<Uuid>, count: u32, designation: Option<String> },  // D14
    Move         { terminals: Vec<Uuid>, before: Option<Uuid> },       // D4
}
pub enum StripEditError {
    Refused(&'static str),                     // a reason key of §6.10
    Invalid(String),                           // a sentence, for the error line or the card
    Ambiguous(Vec<FieldChoice>),               // D7
}
impl Session {
    pub fn strip_edit(&self, edit: &StripEdit) -> Result<AnyCommand, StripEditError>;
}
pub enum StripCell { Group, BridgePart, AccessoryPart }   // NavColumn gains `strip_cell: Option<StripCell>`
// SortBy-independent: NavNode gains nothing; the provider's node order is the Order sort.
```

The exact field names may follow the crate's conventions; the variants, their meaning and the one door
(`strip_edit`) may not change without a lead ruling.

## 5. Commands

Both are edit commands (`CommandMeta::edit`), cost `Interactive`, additive (the command count goes from 56 to 58),
with JSON schemas, a DSL verb (`terminal_insert`, `terminal_move`) and an arm in the command fuzzer.

### 5.1 `terminal.insert` (part 1)

```text
terminal.insert { strip: uuid, before?: uuid, fndef: uuid, terminals: [TerminalSpec], parts?: [PartUse] }
```

`TerminalSpec` as `terminal.add`'s, with `seq` forbidden. Let s be `before`'s `seq` (absent: the strip's highest
`seq` + 1, or 1) and n the number of terminals. In one transaction: every terminal of the strip with `seq ≥ s` gets
`seq + n`; every group with `from_seq ≥ s` gets `from_seq + n` and every group with `to_seq ≥ s` gets `to_seq + n`;
every accessory with `after_seq ≥ s` gets `after_seq + n`; then the new terminals are inserted as `terminal.add` inserts
them (its `insert_terminal`), with `seq` s, s + 1, … in list order, tag counter = `seq`, and `parts` as given on each.
**Rejected:** the strip is not a device with the strip aspect; `before` is not a terminal of the strip; an empty list; a
spec with `seq`; a designation already used in the strip or twice in the list; `seq + n` beyond `u32::MAX`. A missing
`fndef` or part copy is `NotFound`. **Outcome:** the first new terminal device.

*Bridge groups (FT-641, cycle 36).* A bridge is a connection between terminal CPs, so a bridge group follows its
terminals. New terminals between two terminals of a bridge group that is consecutive would split it (CI-06): the
command refuses before EW-TERM-004's prevent check, with "inserting before terminal 3 would split bridge group 1
(terminals 1, 2, 3, 4); insert before terminal 1 or after terminal 4, or delete the group's bridge there first". A group
that was not consecutive already (a lenient import's) is EW-TERM-004's finding, not a refusal.

*Example (Golden A):* before -X1031:5 with one terminal designated 21: terminals 5 … 20 become seq 6 … 21; the groups
1–4, 5–10, 11–14, 15–20 become 1–4, 6–11, 12–15, 16–21; the new terminal has seq 5 and no group; no designation and no
bridge changes; BR1 still joins 1–4.

### 5.2 `terminal.move` (part 1)

```text
terminal.move { terminals: [uuid], before?: uuid }
```

Let T be the strip's terminals in (`seq`, ID) order. The new order O is T without the moved terminals, with them (in
their T order) inserted directly before `before` (absent: at the end). Let a and b be the first and last indexes where
T and O differ; for i in a..=b, O[i] gets T[i]'s old `seq`. Nothing else changes: groups and accessories stay, so a
moved terminal takes the group and neighbours of its new place. **Rejected:** an empty list; a terminal listed twice;
terminals of two strips; a device that is not a terminal; `before` among the moved or of another strip; an order that
does not change ("the terminals are already there"). **Outcome:** none.

*Bridge groups (FT-641, cycle 36).* The bridge connections stay with their terminals, so a move that keeps every bridge
group consecutive is accepted and the group follows (the whole of BR1 to the end commits). A move that would split a
consecutive group is refused, naming the group and the way out: "the move would split bridge group 1 (terminals 1, 2,
3, 4); remove terminal 1 from the group first (delete its bridges), or move the whole group", or, when the moved
terminals are not in the group, "put the terminals before terminal 1 or after terminal 4, or delete the group's bridge
there first". Bridges are never moved, split or deleted by the command itself.

*Example (Golden A):* -X1031:13 before :11 gives seq 11 to :13, 12 to :11 and 13 to :12; :1 to :10 and :14 to :20 keep
theirs; all three stay in the group 11–14 "Pneumatics"; BR5 still joins 11 to 20.

### 5.3 Commands used as built

`terminal.designate`, `edit.delete`, `fields.set` (with `strip.accessories` added to `SETTABLE`, D5), `fields.set_many`,
`connection.draw`, `connection.draw_many`, `place.pick` (an action). Parts 2 and 3 use `fields.set_many` and
`plc.bind_channel` as built (§7, §8).

## 6. Part 1: the strip editor

### 6.1 Opening it

`strip.edit { strip?: text }` (Edit ▸ Strip editor): the strip is the parameter (a printed tag such as `-X1031`, by
`resolve_nav_key`'s rules), else the selected strip, else the strip of the selected terminal, else the strip of the
Terminal strips navigator's cursor row; none refuses `reason-no-strip`. It shows the Terminal strips navigator, focuses
it, switches it to table mode, sets its filter to `strip:<tag>` (case-folded) and puts the cursor on the strip's first
row. Clearing the filter shows every strip; the editor actions act wherever the cursor is.

### 6.2 Rows and columns

Rows (D2, D3), in the default `Order` sort: for each strip in natural tag order, its accessories with `after_seq` 0,
then each terminal in (`seq`, ID) order followed by the accessories placed after it.

| Column | Key | Terminal row | Accessory row | Editable (terminal / accessory) |
|---|---|---|---|---|
| Strip | `strip` | strip tag | strip tag | no (key column) |
| Pos | `pos` | `seq` | `<after_seq>.<k>` | no: "move rows with Alt+Up and Alt+Down" |
| Terminal | `terminal` | designation | empty | `fields.set terminal.designation` (as built) / no |
| Function text | `label` | function text | kind word | `fields.set function_text` (as built) / no |
| Group | `group` | the group's label | empty | **D8** (`StripCell::Group`) / no |
| Part | `part` | first part number | part number | as built (the Parts route) / **D5** (`StripCell::AccessoryPart`) |
| Potential | `potential` | derived | empty | no (derived) |
| Side 1, Side 2 | `side1`, `side2` | as built | empty | no |
| Bridge | `bridge` | `BR<n>` | empty | no |
| Bridge part | `bridge_part` | the group's part number, or empty | empty | **D6** (`StripCell::BridgePart`) / no |
| State | `state` | as built | empty | as built / no |

The terminal row's Part cell keeps its as-built route (the Parts row's choice list, D7's widening applies). The new
read-only reasons (string keys): `table-accessory-row` "accessory row", `table-pos-move` "move rows with Alt+Up and
Alt+Down", `table-no-bridge` "the terminal has no bridge". A Bridge part cell on a terminal without a bridge is invalid
with `table-no-bridge`.

### 6.3 Cell compiles (D5 to D8)

A cell gesture of WP-S2.06 §8.3 to §8.6 (one cell, Ctrl+Enter on the marked rows, fill-down, pattern fill, paste,
clear) on a column with a `strip_cell` compiles through `Session::strip_edit`, not through `parse_field_input`:
- **Group** over the gesture's terminal rows: one `StripEdit::GroupLabel` per distinct text, all rows of one text in
  one edit; the commands of a gesture are merged into one `fields.set` (one strip) or one `fields.set_many` (several
  strips, or several texts on one strip applied in row order to the same list, D8 applied in sequence to one value).
- **Bridge part** over the gesture's terminal rows: the distinct groups of the rows; one `fields.set` of
  `overrides.wire_part` on their carriers when all texts resolve to one part, else one `fields.set_many`; two rows of
  one group with different texts are invalid ("same bridge as row <n>", the D13 rule of WP-S2.06).
- **Accessory part** over accessory rows: one `fields.set` of `strip.accessories` per strip with each row's part
  replaced.

A `StripEditError::Ambiguous` cell is an ambiguous cell of the table card (`table.resolve` picks); `Invalid` is an
invalid cell; `Refused` refuses the gesture with its reason. The policy (FR-UI-044 rules 1 to 3) applies to the
compiled command as built; none of these paths is an identifier route.

### 6.4 Insert, delete and move rows

- **Insert** (`strip.insert { count?: uint = 1, designation?: text }`, key Insert): before the cursor terminal row; on
  an accessory row, before the next terminal (after the accessory); with no terminal after the cursor, at the end. One
  `terminal.insert` (D14): `fndef` the reference terminal's function definition, else the project's held copy of the
  starter `ew.fn.terminal`, else refused `reason-no-terminal-def`; `parts` the reference terminal's part uses;
  `designation` (count 1 only, else `error-bad-param`) or the next free whole numbers. Direct commit; toast "Inserted
  <n> terminals before <strip>:<designation> · Undo"; the cursor moves to the first new row.
- **Delete** (`strip.delete_rows`, key Ctrl+Delete): the marked rows (else the cursor row). Terminal rows only: one
  `edit.delete` of the terminal devices, always the table card (D12) with the cascade counts. Accessory rows only:
  `StripEdit::RemoveAccessories`, direct commit with the toast. A mix: refused `reason-strip-mixed-rows`.
- **Move** (`strip.move { step?: int, before?: text }`, keys Alt+Up and Alt+Down for `step` −1 and +1): the marked
  terminal rows of one strip (else the cursor row) move one terminal up or down, or before the row `before` (a printed
  designation of the same strip, or `end`); one `terminal.move`; direct commit with the toast; the marks and the
  cursor follow the moved rows. Accessory rows in the marks: refused `reason-strip-mixed-rows` (an accessory moves by
  removing and adding it). Nothing to move (already first or last): refused `reason-strip-move-none`.
- **Re-designate**: the Terminal column as built (one cell, fill-down, pattern fill, paste; designations of several
  terminals open the card by rule 2, and EW-TERM-002 is checked at transaction end, FR-TRM-022).

### 6.5 Accessories

`strip.add_accessory { kind?: enum(end_plate|end_clamp|partition|marker|cover) = end_clamp, part?: text }` (Edit ▸ Add
accessory; key none): adds an accessory to the cursor row's strip after the cursor row (a terminal row: `after_seq` =
its `seq`, after the accessories already there; an accessory row: directly after it, same `after_seq`; the strip node:
`after_seq` 0). The part: `part` parsed by D7 with the kind's allowed part kinds; absent, the one project part copy of
those kinds; none: refused `reason-no-accessory-part`; several: in the window, the chooser list of 16a §6.22 opens at the
row (its rows `<part number> (<manufacturer>) · <description>`, the pick dispatching `strip.add_accessory` again with
`part`), headless `Failed` with `error-accessory-part-ambiguous` listing the choices. One `fields.set` of the strip's
list (D5); toast "Added <kind word> after <strip>:<designation> · Undo"; the cursor moves to the new row.

**Golden check (the H4 rows):** on the blind task's prelude state with the strips 1X1, -X1031 and -X1032 created, four
end brackets (1X1 at 0 and after its last terminal, -X1031 and -X1032 after their last) and the part TBD bridge on BR1,
BR5 and BR6 give the parts list rows `End bracket` ×4 and `Plug-in bridge` ×3 of `oracle/bom.csv` (SE6).

### 6.6 Bridges (D9 to D11; FT-548, FT-576, FT-585)

- **Bridge terminals** (`strip.bridge`, as built, menu Draw ▸ Bridge terminals and the strip editor's row menu):
  selection by records (D9); terminal identity for "already bridged" and for the group rule (D10); ends, chain, the
  group rule's order and the commands otherwise as built. The enabled predicate: a writable project and
  `bridge_command` Ok.
- **Remove bridge** (`strip.unbridge`, the strip editor's row menu): the bridge connections whose two ends are on the
  selected terminals, in one `edit.delete`, direct commit with the toast; none: refused `reason-unbridge-none`; a split
  group: refused `reason-unbridge-split` (D11).
- **Bridge part**: the column of §6.2, compiled by §6.3.

### 6.7 Group labels and the printed label (D8, D13; FT-553)

The Group column (§6.3) and, in Properties of a terminal, the "Group" row (one `StripEdit::GroupLabel` over the
selected terminals; a selection across strips compiles one `fields.set_many`) and the "Printed label" row (the
`function_text` route, relabelled, in the Terminal section after Designation). A strip's Properties keep the `groups`
list and the label mode as built.

### 6.8 Placing unplaced terminals (FR-TRM-015, FR-DEV-018)

*As built (cycle 34): not built in part 1; moved to part 2.*

A terminal row whose terminal has no representation and is drawn by no strip segment shows the `unplaced` badge (as
built) and is a drag source onto a canvas; Enter on it, with a page open in the focused tab, dispatches `place.pick
{ function: <its main function> }` (WP-S2.06 §6.7's drop); with no page open, Enter refuses `reason-no-page`. A placed
terminal's row opens its drawing as built.

### 6.9 Actions of part 1 (6 new; 262 → 268)

| ID | Kind | Label | Menu | Shortcut | Context | Params |
|---|---|---|---|---|---|---|
| `strip.edit` | View | Strip editor | Edit | — | global | `strip?:text` |
| `strip.insert` | Command | Insert terminal | — | Insert | strips navigator | `count?:uint, designation?:text` |
| `strip.delete_rows` | Command | Delete rows | — | Ctrl+Delete | strips navigator | `` |
| `strip.move` | Command | Move terminals | — | Alt+Arrow Up, Alt+Arrow Down (in the Strips navigator's focus; ce9bad22) | strips navigator | `step?:int, before?:text` |
| `strip.add_accessory` | Command | Add accessory | Edit | — | strips navigator | `kind?:enum(end_plate\|end_clamp\|partition\|marker\|cover), part?:text` |
| `strip.unbridge` | Command | Remove bridge | — | — | strips navigator | `` |

"Strips navigator" context: the Terminal strips navigator is open and focused (headless: open); otherwise the action
refuses `reason-not-strip-editor`. The row menu of the Terminal strips navigator lists Insert terminal, Delete rows,
Add accessory, Bridge terminals and Remove bridge. A shortcut conflict reported by the keymap test stops the lane.

### 6.10 Strings, transcript and describe

New reason and error keys with their English text: `reason-not-strip-editor` "Open the strip editor first",
`reason-no-terminal-def` "The project has no terminal definition to insert", `reason-strip-mixed-rows` "Mark only
terminals or only accessories", `reason-strip-move-none` "The terminals cannot move further",
`reason-no-accessory-part` "The project has no part for this accessory; embed one first", `reason-unbridge-none` "No
bridge joins the selected terminals", `reason-unbridge-split` "Unbridge from an end of the bridge, or the whole bridge",
`error-accessory-part-ambiguous` "Several parts fit: {list}; give part", and the three `table-*` keys of §6.2.

The navigator's describe line prints `sort "Order"` for the new sort; accessory rows print as rows with their cells,
the row word `accessory`. **New describe word (1):** `accessory` (355 → 356). No new transcript member.

```text
navigator "Terminal strips" kind strips view table sort "Order" ascending filter "strip:-x1031" rows 21 showing 1 to 21
  columns "Strip" "Pos" "Terminal" "Function text" "Group" "Part" "Potential" "Side 1 (clamp a)" "Side 2 (clamp b)" "Bridge" "Bridge part" "State"
  row "-X1031" "1" "1" "1+" "Pneumatics" "PT 2.5-TWIN" "24VDC-1" "-102F2:Out1" "-201U1:5 via -106W1 BN" "BR1" "TBD" "used" cursor "Bridge part"
  accessory "-X1031" "20.1" "" "End bracket" "" "TBD" "" "" "" "" "" ""
```

The 50-row window of WP-S2.06 D15 applies unchanged.

## 7. Part 2: the edit-cable table and the navigator row operations

**Scope.** FR-CBL-016 on the Cables navigator's table, and FT-490's generic half.

- **Core assignment columns.** The Cables table gains, per core row, `from` and `to` (the far ends of the core's
  connections, as the cable diagram prints them) and `potential` (derived); `colour`, `cross_section` stay editable
  (WP-S2.06 §8.2) and `core`, `shield` and `pair` become editable (`fields.set` on the conductor; `core` an identifier
  route).
- **Swap cores** (`cable.swap_cores`, key none, row menu): exactly two marked core rows of one cable; one
  `fields.set_many` setting each connection's `kind` to `Core(<the other conductor>)`, so the assignment, not the
  colour, moves; direct commit with the toast. FR-CBL-016's acceptance: swapping WH and BN on -106W1 is one undo step and
  the cable diagram follows.
- **Typed editor for `cable.length`** (FT-429's member): the length source as `<number> m` (manual) or `missing`,
  through `parse_field_input`.
- **FT-490's generic half:** Delete in navigator trees with the cascade preview (one `edit.delete` of the marked rows'
  records, always the card); F2 renames the authored label of a tree row (a page title, a potential name, a structure
  node's code, a device's tag letter and counter through the tag proposal of FR-NUM-029) through the row's editable route.
  *As built (cycle 37, lane I37, decision log ID-862; a2cd4760, FT-633):* F2 on a structure node edits its code through
  the `fields.set` route on `structure_node.code`; F2 on a device row selects the device, opens Properties and sends the
  typed tag through the Tag field's rows (the Join proposal, the Letter or Counter row with FR-NUM-029's refusal and
  proposal, else the frozen tag; a tag that changes letter and counter is frozen); the refusal and the Use or Join
  button show under the inline field.

**Acceptance (outline):** CT1 swap on -106W1 (one `fields.set_many`, one undo step, the cable diagram's rows), CT2 the
editable core columns' routes, CT3 the length editor, CT4 tree Delete with the card and F2 rename on Pages and
Potentials, CT5 transcript and counts. **Size 0.6 pw:** the columns 0.15, swap 0.15, the length editor 0.05, FT-490's
half 0.2, tests 0.05.

## 8. Part 3: the PLC navigator and grid

**Scope.** FR-PLC-020 and FR-PLC-021 on WP-S1.21's model as built.

- **Tree** (`tree` view of the PLC navigator): configuration project → station (role word as its badge) → rack → module
  (slot) → channel, with bus ports under their station; unplaced channels (no binding) carry the `unplaced` badge.
- **Drag-bind:** a channel row is a drag source; dropping it on a canvas item that is a schematic channel symbol's
  function or a fluid valve coil's non-physical CP dispatches one `plc.bind_channel` (as built); the drop target is
  resolved by the canvas pick; any other target refuses. FR-PLC-020's acceptance: dragging channel 1 of -201U3 onto the
  coil of -201U3-Q1 on folio 203 binds it and the valve shows the address as derived text.
- **Grid** (table view): the columns of PBF §2.8 that WP-S1.21 models (station, rack, slot, module, channel number,
  designation, kind, address, symbol, data type, function text, process tag, state, safety, target, placed-on, signal
  range, comment); the editable ones are those the catalogue routes to `fields.set` as scalars (WP-S2.06 D10); the
  facets `unassigned`, `duplicate`, `spare`, `unplaced`, `safety`, `bus:<name>`, `config:<name>`, `tbd`. Fill-down,
  pattern fill (`TA05_V_{n:02}`) and paste as built.
- **Typed editors** for `plc_module.slot` and `plc_module.start.input`/`.output` (FT-429's members).

**Acceptance (outline):** PG1 the tree of Golden A (-201U1 with -201U3 … -201U8 and -202U2, 56 channels), PG2 the
drag-bind of FR-PLC-020, PG3 pasting 8 symbols from Excel into the grid is one undo step (FR-PLC-021), PG4 each facet on
Golden A, PG5 the typed editors, PG6 transcript and counts. **Size 0.7 pw:** tree 0.15, drag-bind 0.2, grid columns and
facets 0.2, editors 0.05, tests 0.1.

*As built (cycle 38, lane D38, decision log ID-877; d7d17422, d437380a).* PG1 to PG5 hold. PG6 does not hold as a check: `crates/ew-viewmodel/tests/plc_navigator.rs` names it in its header, but no test asserts the PLC navigator's transcript members, its describe words or exact counts as the other packages' "transcript and counts" checks do; the tests assert one command per edit and read `describe` for the pasted symbols and the slot only (cycle-38 design pass, part 2; the cycle-38 review, n5). **Tree:** configuration project → station (its role word as the badge) → module → channel, with the `unplaced` and `spare` badges; modules not attached to a station are not shown. **Not built (de-scoped):** the rack level and the bus ports under their station (FT-739). **Grid:** 14 columns, station, module, slot, channel, designation, kind, address, symbol, function text, process tag, state, target, placed on and comment; the rack, data-type, safety and signal-range columns are not built (FT-737, FT-739). The slot prints the slot alone for rack 0 without a subslot, else `r.s` or `r.s.u`. **Facets:** the PLC flags `spare`, `unplaced` (no binding; a card pin bound by N3 counts as placed), `unassigned` (Excluded or Unresolved), `tbd` and `duplicate` (EW-PLC-007 and the EW-PLC-001 overlap), applying in the PLC navigator only, and the config facet; `safety` and `bus:<name>` are not built (FT-738). **Typed editors:** address (the profile's text through `fields.set`, FT-690), target (a device tag resolving to its main function), slot (`n`, `r.s`, `r.s.u`, `auto`) and the module start in the property grid through `plc.set_start` (no start column). **Drag-bind:** a hidden `navigator.bind` action runs `plc.bind_channel` (no new command). **PG2** was tested on a starter project (the generic CPU, the output card and a starter coil, channel 0 dropped on the coil's CP), not on -201U3's channel 1 onto -201U3-Q1 on folio 203: the orchestrator's feasibility ruling, because Golden A's valve coils have no drawn internal CPs to drop on (FT-740).

## 9. Part 4: part selection and the part editor

**Scope.** FR-PRT-015 and FR-PRT-022, FT-394 and FT-396, FT-629's ruling and FT-444's ruling, on WP-S1.19's catalogue
search and previewed update as built (its parts 2, 3 and 6: `Session::part_search`, `Session::assign_part`,
`part.assign`, `library.update`, `Session::library_status`, the Libraries navigator). *Specified in cycle 39 (decision
log ID-894)*: the outline of cycle 34 is closed below to its decisions D16 to D24, its public items, actions, strings
and describe lines, and the acceptance table of §11.3. Part 4 runs as **part 4a** (the part selection dialog and the
part editor, 0.9 pw, first) and **part 4b** (FT-396's paste choice, 0.2 pw, after part 4a; §14.1).

**What exists (the base contract of part 4).** The Parts row's choice list (cycle 33, D7): the project's part copies,
then the system libraries' parts the project does not hold (WP-S2.18 D6), as `<part number> (<manufacturer>)`; a
system-library choice commits `part.assign` (`replace_all`, `Session::part_uses_assign`), any other `fields.set`. The
strip editor's Part, Bridge part and accessory Part cells resolve held copies only (D7), and `strip.add_accessory`'s
ambiguity opens the chooser list (16a §6.22). `Session::part_search` searches every installed tier (the compiled-in
starter libraries included) by WP-S1.19 D6, with `held` filled from the project; `PartHit.compatible` is the
category-level rule (first two category levels and the contact kind), so a 1P MCB is compatible with a 1P+N function
(FT-629). `part.assign` carries the closure in `copies`, accepts a held copy with no `copies`, and refuses a held copy of
another version. No command edits a part copy's content (FR-DEV-032's amendment, ID-598); a copy whose
`meta.source` names `fork_of` is a local fork (`is_fork`, as built). `edit.paste` refuses a copy the project holds in
another version ("update from library first", doc 06 §3.6 item 4 as built). Company libraries from the settings are not
wired into the tiers (FT-754 (f)), so the fixture catalogues of §11.3 are user libraries under `EW_DATA_DIR`.

### 9.1 Decisions of part 4

Lead rulings of the specification pass, recorded under ID-894; the owner may reverse them. They do not reopen D1 to
D15.

| # | Decision | Reason and trade-off |
|---|---|---|
| D16 | **Choosing a part is `part.assign` for devices and `fields.set` with copies for strip cells; no new command for choosing.** The dialog commits, for a device or terminal target, one `part.assign` (WP-S1.19 D8) for a held copy (no `copies`) and a catalogue part (its closure in `copies`) alike: opened on a part use row `parts[i]` it is `replace` with `from` = that use's copy (on a multi-selection whose row varies, `replace_all`); opened with **Add part…** or on a device without part uses, `append`; on the strip editor's Part cell, `replace_all` on the marked terminal rows (the cell's compile of D7). For the strip editor's Bridge part and accessory Part cells, `Session::strip_edit`'s compile (D5, D6) is unchanged except that **`fields.set` and `fields.set_many` gain an optional `copies` member** (the library documents a value names that the project lacks), embedded in the same transaction exactly as `device.place` and `box.black` do since FT-656; a held copy of another version is refused at `/copies/<i>` ("update from library first") | One command per gesture and one undo step (ADR-0008); a log replays without a library (WP-S1.19 D7). `part.assign` already embeds and sets in one transaction, so a composite of `library.embed` and a set (§9's outline) is not needed; the `copies` member follows the FT-656 and FT-673 precedent instead of a second embedding step. The trade-off is one more optional member on the most used edit command |
| D17 | **One offer list, ordered for the target (FT-629 ruled: two levels).** `Session::part_offers` merges the project's part copies and the catalogue hits, one row per `(library, item)` (a library item the project holds is shown once, as the held copy). Order: (1) when the target has a function (a device's main function, a terminal's function), the fit: **same FnDef** (the part's function template declares the function's FnDef item, the same library and item at any version) before **same category** (WP-S1.19 D6's rule, as built) before **other** (no template, or neither); (2) the rank tier (exact `pn_norm`, prefix, trigram, words; no text: 0), applied to project copies by the same `pn_norm` rules and a word match on manufacturer and description; (3) held before not held; (4) favourites, then recently used (D18); (5) the full-text score (catalogue hits; project copies 0); (6) natural part number, manufacturer, library ID, item ID. `FnCompat` gains `fndef: Option<(library, item)>` and `PartHit` gains `same_fndef: Option<bool>`; `compatible` keeps its meaning. **Filters:** the text; the kind; the category (first level, then the second); **Only parts that fit** (hides `other`; off by default); the view **All**, **In this project**, **Favourites**, **Recent**; **Show deprecated and withdrawn** (off; a deprecated row names its successor in the detail line, FR-PRT-034). The strip editor's cells have no function and preset the kind filter by D7's kinds (`end_bracket`; `marker`, `accessory`; `accessory`) | FR-PRT-015: "offers MCB 1P+N parts first" needs the 1P+N parts above the 1P ones, which the category rule cannot tell apart; the function's FnDef can, and §9's outline already named it ("parts whose function template declares the device's FnDef first"). Ordering instead of hiding keeps near matches one scroll away; the filter hides them on request. Golden A's copies carry no function template (the bootstrap writes templates for coils only), so on Golden A they rank as `other` |
| D18 | **Favourites and recently used live in the user settings, not in the project.** `settings.toml` gains a `[parts]` table: `favourites`, the sorted `<library ID>/<item ID>` texts, and `recent`, most recent first, no duplicates, at most 20 (`MAX_RECENT_PARTS`). Only library items are kept (a project-local copy has no identity outside its project); a choice is recorded when its command commits; a favourite is toggled by `part.favourite`. Unknown keys of the table are kept, as the file's other tables | Doc 16 §2.5 keeps per-user data out of the project (FR-UI-033); favourites span projects, so the per-project `view.json` (not built) is the wrong home, and §9's "view state" is read as the user half of doc 16 §2.5. The order is the order of the user's choices, not a clock (NFR-REL-07), and the file is written sorted, so it is deterministic |
| D19 | **The part selection dialog is a modal form dialog** (16a §6.12, 640 px; `Dialog::PartChoose`), titled by its target ("Choose a part for F55", "Choose a bridge part for -X1031 BR1", "Choose a part for the end bracket -X1031 20.1"): the search field (first focus), a filter row (kind, category, Only parts that fit, the four-way view as a segmented control, 16a §6.20, Show deprecated and withdrawn), the result list and a detail line for the active row (library and tier, version, status and successor, the template's function names), then **Choose** (Primary) and **Cancel**. Rows: part number in `type.body.strong`, manufacturer, description (elided), the kind word, a source badge ("In this project", else the library's name), a fit badge ("Fits F55's function", "Same category"; none for `other`), a favourite star (icon with the accessible name "Favourite"). Keys: typing edits the search, Up and Down move the active row, Enter chooses, Escape cancels. **Opened from:** the Parts row (its row menu's Choose part… and Add part…, and a trailing button on the row), the strip editor's Part, Bridge part and accessory Part cells (their row menu's Choose part…), and the last row of the accessory chooser list, "Search all parts…", which opens it with the kind preset. D7's choice list stays the keyboard path for held copies | Choosing a part blocks the edit it belongs to, so the dialog is modal (doc 16 §4.1); the 640-px form dialog is the existing pattern (WP-S2.17 part 4), so no new widget is made. The trade-off is a modal over the canvas, where doc 07 §5's part browser was a dock; a dock browser stays a follow-up |
| D20 | **The part editor's command is `part.edit_copy`, `Bulk` and always previewed (FT-394).** For a **project-local copy** (`library_id` is the project ID, forks included) it rewrites the copy in place: the same copy ID, item ID and version, the item replaced, `sha256` recomputed. For a **copy from a library** it forks the item (doc 07 §3.1, "local edits fork the item"): a new project-local copy, `item_id` = the `fork` parameter, `id` = `v5(NS_LIBCOPY, project ‖ fork)`, the original's version, `meta.source` = `{"fork_of": {"library", "item", "version"}}` (the member `is_fork` reads, kept untyped in `extra`), and every reference to the original copy that `ew_model::LINKS` lists as a part-copy link (the part uses, a connection's `overrides.wire_part`, a strip accessory's `part`, and any other link to a part copy that the table names) re-pointed to the fork in the same transaction; the original copy stays, unused, until purged. The fork ID is minted by `Session::next_fork_item(copy)` as `v5(copy ID, "fork")`, skipping held items with a counter as `next_macro_instance` does, and is carried in the command, so a replay reproduces it. **Editable members:** `manufacturer`, `part_number`, `type_designation`, `order_number`, `gtin`, `description`, `kind`, `category`, `unit`, `tech_data`; every other member must equal the copy's, so no edit changes a CP, the function template, the contacts, the sections or the mechanical data (no CP map is needed). The card shows the field diff, the devices and representations that use the copy, the BOM rows that change and, for a fork, "Saving creates a project copy of this part; the library is not changed" | A library item is never edited from inside a project (doc 07 §3.1, ARCH §5.3); the fork keeps `library_status` honest (Local fork, not Update available). Re-pointing every use keeps one copy per item and project in use, which is what the user sees as "the part". `Bulk` makes the preview the only door (FT-394's "previewed"). The trade-off is that the CP pattern, sections and mechanical data are not editable in part 4 (a follow-up, which needs the major class's CP map) |
| D21 | **FR-PRT-022's validation, twice.** The draft (`ew-api`'s `PartDraft`) checks each field as it is typed and the dialog shows the message under the field; Save is refused while a field is invalid, with the first message as the refusal. The mandatory fields: a **manufacturer** (one of the project's manufacturer copies or a catalogue manufacturer, chosen from a list; a catalogue one travels in `copies`), a **part number** (not empty after trimming) and a **unit** (`piece`, `m` or `set`). Technical values parse by the property grid's quantity rule (`parse_field_input`, `FieldType::Quantity`: `4 A`, `0.75 mm²`) or as an identifier (`C`); a key given twice is refused. An unchanged draft is refused ("Nothing to save: the part is unchanged"). The command checks the same rules again and refuses with a JSON pointer, so a script or log cannot bypass them | FR-PRT-022's acceptance: "Saving a part without manufacturer is refused with a message"; the command's own check keeps the rule for every front end (the headless mode, the MCP, a log) |
| D22 | **FT-396: two of the three choices in part 4b; "keep both" is not offered.** A paste whose fragment carries another version of a held copy opens a **choice card** in the Preview tab (16a §6.23) that lists each such copy ("FX-MCB-1PN (Fixture): this project 1.1.0, pasted 1.0.0, patch") and offers: **Use the project's copy** (adapt): one `edit.paste` with a new optional member `adapt: [copy ID]`, which reuses the held copies named there instead of refusing; offered when every listed copy differs by a patch or minor change (`classify_update`), else disabled with its reason, and the command refuses a major one at `/adapt/<i>`; one undo step. **Update the project's copy:** the bulk card of `library.update` built from the fragment's documents (`Session::update_from_documents`, the sibling of `update_from_library` that takes documents instead of reading the libraries); after its Apply the shell runs the pending paste again; offered when every listed pasted version is newer, else disabled with its reason; two commands and two undo steps. **Keep both** is not offered: `library.embed` and `edit.paste` refuse a copy ID that is not `v5(NS_LIBCOPY, library ‖ item)`, and every reference from a library item to another (a template's FnDef, a symbol's FnDef) resolves through that one identity, so a version-qualified second copy (doc 07 §2.1) needs a format-level change of its own (a follow-up). Headless: `edit.paste_clip` refuses with `reason-paste-versions`, and `paste.resolve` with `choice` `adapt` or `update` runs the choice | Copy IDs are equal across versions, so adapting is a matter of not refusing; updating is the existing previewed command, and keeping it a separate undo step lets the user undo the paste and keep the update. Keep both is the only choice that touches identity, which is a file-format concern above part 4's routing |
| D23 | **FT-444 ruled; built outside part 4.** (a) A connection point added to a black box by hand (Edit ▸ Add connection point, or any CP of a box drawn without a part) is a **device CP** of the black-box function: an index above the function's FnDef and pattern CPs, designated by the function's `cp_overrides` entry at that index (`CpOverride.designation`, no format change); the tool asks for the designation and proposes the next free whole number among the function's designations (natural order); `function_cps` instantiates, for a function of kind black box only, each such override as a physical CP with that designation. (b) The property sheet's Connection points section lists every CP of the function (pattern, FnDef and device CPs) and marks one that no box CP draws "not drawn"; the box tool's ghost draws only what will be drawn. (c) **The part editor shows the part's CP pattern** (designation, alias, description, the clamp members) and the function template read-only (D20); device CPs belong to a device, not to the part, and never show there; making a device CP part of a pattern is a part edit of the CP pattern, which part 4 does not offer | The designation has a home in the format already (`CpOverride`), so no member is added; the rule touches `ew-derive` and the box tool, which are not part 4's crates, so the ruling is recorded here and FT-444 keeps its row for the build. Golden A's black boxes draw pattern CPs only, so no golden byte depends on the rule |
| D24 | **Boundaries.** No `ew-schema` change, no new package, no new dependency edge, no change to Golden A's files or expected outputs; one new command (`part.edit_copy`, part 4a) and two new optional command members (`copies` on `fields.set` and `fields.set_many`, part 4a; `adapt` on `edit.paste`, part 4b), with their schemas, the DSL verb `part_edit_copy` and fuzzer arms; the `ew-library` change is additive (D17) | Part 4 is UI and commands on delivered data; the routing of §14.2 (Opus at `high`) fits commands that write copies and re-point references, not a format change |

### 9.2 The public items added (part 4a, and part 4b where marked)

```rust
// ew-library (D17, FT-629)
pub struct FnCompat { /* as built */ pub category: [String; 2], pub contact_kind: ContactKind,
                      pub fndef: Option<(Uuid, Uuid)> }            // (library, item) of the function's FnDef
pub struct PartHit  { /* as built */ pub same_fndef: Option<bool> }  // None without `fndef`

// ew-commands
pub struct EditPartCopy { copy: Id<Part>, part: Value /* the edited Part item, without the copy metadata */,
                          fork: Option<Id<LibItem>>, copies: Vec<Value> /* a manufacturer the project lacks */ }
// `fields.set`, `fields.set_many`: `copies: Vec<Value>` (optional, default empty)            (D16)
// `edit.paste`: `adapt: Vec<Id<()>>` (optional, default empty)                               (D22, part 4b)

// ew-api
pub enum PartTarget {
    Devices { devices: Vec<Uuid>, mode: AssignMode },                // part.assign (D16)
    BridgePart { terminals: Vec<Uuid> },                             // StripEdit::BridgePart with copies
    AccessoryPart { strip: Uuid, index: u32 },                       // StripEdit::AccessoryPart with copies
}
pub enum PartPick { Held(Uuid /* copy ID */), Library(LibRef) }
pub enum Fit { SameFnDef, SameCategory, Other }
pub enum OfferView { All, Project, Favourites, Recent }
pub struct PartOfferQuery { pub text: String, pub kinds: Vec<PartKind>, pub category: Vec<String>,
                            pub function: Option<Uuid> /* the target's function */, pub fits_only: bool,
                            pub view: OfferView, pub include_inactive: bool,
                            pub favourites: Vec<(Uuid, Uuid)>, pub recent: Vec<(Uuid, Uuid)> }
pub struct PartOffer { pub pick: PartPick, pub part_number: String, pub manufacturer: String,
                       pub description: String, pub kind: PartKind, pub fit: Fit, pub held: bool,
                       pub library: Option<(Uuid, String /* name */, Tier)>, pub version: String,
                       pub status: ItemStatus, pub favourite: bool, pub recent: bool }
pub struct PartDraft { /* the editable members of D20 as texts, the copy, its source, the read-only members */ }
pub enum PartDraftError { Refused(&'static str /* reason key */), Field { field: String, message: String } }
impl Session {
    pub fn part_offers(&self, q: &PartOfferQuery) -> Vec<PartOffer>;
    pub fn part_pick_command(&self, target: &PartTarget, pick: &PartPick) -> Result<AnyCommand, ApiError>;
    pub fn part_draft(&self, copy: Uuid) -> Result<PartDraft, ApiError>;
    pub fn set_draft_field(&self, draft: &mut PartDraft, field: &str, text: &str) -> Result<(), PartDraftError>;
    pub fn part_edit_command(&self, draft: &PartDraft) -> Result<AnyCommand, PartDraftError>;   // D20, D21
    pub fn next_fork_item(&self, copy: Uuid) -> Uuid;                                         // D20
    pub fn paste_versions(&self, fragment: &str) -> Vec<VersionConflict>;                      // part 4b
    pub fn update_from_documents(&self, docs: &[LibDocument], cp_maps: &[CpMap])
        -> Result<UpdatePlan, ApiError>;                                                      // part 4b
}
pub struct VersionConflict { pub copy: Uuid, pub title: String, pub held: String, pub pasted: String,
                             pub class: ChangeClass, pub newer: bool }                        // part 4b
```

The exact field names may follow the crates' conventions; the variants, their meaning and the doors (`part_offers`,
`part_pick_command`, `part_edit_command`) may not change without a lead ruling.

### 9.3 `part.edit_copy` (part 4a)

```text
part.edit_copy { copy: uuid, part: object, fork?: uuid, copies?: [doc] }
```

`Bulk`, an edit command, with its JSON schema, the DSL verb `part_edit_copy` and an arm in the command fuzzer.
**Rejected**, at the pointer named: `/copy` not a part copy the project holds; `/part` not a part item, or one whose
members other than D20's editable ones differ from the copy's ("only the identity, the description, the category, the
unit and the technical data can be edited"); `/part/manufacturer` a reference to a manufacturer copy the project does
not hold and `copies` does not carry; `/part/part_number` empty; `/part/unit` not `piece`, `m` or `set`;
`/part/tech_data/<i>` a key given twice; `/fork` absent for a copy from a library, given for a project-local copy, or
naming an item whose copy the project holds; `/copies/<i>` as `library.embed` refuses, or another version of a held
copy; the whole command when the item equals the copy's ("the part is unchanged"). **Effects:** D20 (in place or fork,
the re-pointed references, the embedded manufacturer). **Outcome:** the edited copy (the fork's copy ID for a fork).
Example on Golden A: `part.edit_copy { copy: <5SY4204-7's copy>, part: {…, "description": {"en": "MCB 1P+N C4"}},
fork: <next_fork_item> }` adds one copy file under `lib/parts/`, re-points F55's part use, and leaves the original
copy file unchanged.

### 9.4 Actions (part 4a: 7; part 4b: 1)

| ID | Kind | Label | Menu | Shortcut | Context | Params |
|---|---|---|---|---|---|---|
| `part.choose` | View | Choose part… | Edit | — | Properties on a device, or the strip editor's cursor on a Part, Bridge part or accessory Part cell; a writable project | `target?:enum(parts\|bridge_part\|accessory_part), use?:uint, add?:bool` |
| `part.search` | View (hidden) | Search parts | — | — | the part dialog | `text?:text, kind?:text, category?:text, fits?:bool, view?:enum(all\|project\|favourites\|recent), inactive?:bool` |
| `part.pick` | Command (hidden) | Choose | — | Enter (the dialog's Primary) | the part dialog | `index?:uint, part?:text` |
| `part.favourite` | View (hidden) | Favourite | — | — | the part dialog | `index?:uint` |
| `part.edit` | View | Edit part… | Edit | — | Properties on a device with a part use, or a `library_copy` row of kind part in the Libraries navigator | `part?:text` |
| `part.field` | View (hidden) | Edit field | — | — | the part editor | `field:text, value:text` |
| `part.save` | Command (hidden) | Save | — | Enter (the editor's Primary) | the part editor | `` |
| `paste.resolve` (4b) | Command (hidden) | Resolve paste | — | — | the paste choice card | `choice:enum(adapt\|update)` |

`part.pick` takes the active row, a 1-based `index` or a part text in D7's forms (`<part number> (<manufacturer>)`,
with ` · <description>` when two match). `part.field`'s fields: `manufacturer`, `part_number`, `type_designation`,
`order_number`, `gtin`, `description`, `kind`, `category`, `unit` and `tech:<key>` (an empty value removes the key).
`part.save` opens the `part` card; its Apply and Cancel are the Preview tab's, unchanged. Both visible actions are in
the Parts row's row menu (Choose part…, Add part… as `part.choose` with `add`, Edit part…) and `part.choose` in the
strip editor's row menu; the Libraries navigator's part rows gain Edit part…. No shortcut is added, so the keymap test
is unchanged.

### 9.5 Strings

New reason and error keys with their English text (part 4a): `reason-part-choose-target` "Select a device, a terminal
or a part cell first", `reason-part-choose-none` "Choose a part from the list first", `reason-part-not-installed` "The
library of this part is not installed", `reason-part-other-version` "The project holds another version of this part;
update it from the Libraries navigator first", `reason-part-edit-target` "Select a device with a part, or a part in the
Libraries navigator", `reason-part-no-manufacturer` "A part needs a manufacturer", `reason-part-no-number` "A part
needs a part number", `reason-part-unit` "Choose the unit: piece, m or set", `reason-part-unchanged` "Nothing to save:
the part is unchanged", `error-part-tech-value` "{key}: {message}", `error-part-tech-twice` "{key} is given twice",
`error-part-manufacturer` "No manufacturer is called “{name}”; choose one from the list". Labels and lines:
`dialog-part-choose-title` "Choose a part for {target}", `part-choose-fits` "Only parts that fit", `part-choose-view-all`
"All", `part-choose-view-project` "In this project", `part-choose-view-favourites` "Favourites",
`part-choose-view-recent` "Recent", `part-choose-inactive` "Show deprecated and withdrawn", `badge-fits-function` "Fits
{tag}'s function", `badge-same-category` "Same category", `part-choose-empty` "No part matches “{text}”",
`part-choose-more` "Search all parts…", `dialog-part-edit-title` "Edit part {part}", `part-edit-fork-note` "Saving
creates a project copy of this part; the library is not changed", and the field labels Manufacturer, Part number, Type
designation, Order number, GTIN, Description, Kind, Category, Unit, Technical data, Functions, Connection points,
Source. Part 4b: `reason-paste-versions` "The pasted records use another version of {list}; choose Use the project's
copy or Update the project's copy", `reason-paste-adapt-major` "The versions differ in their connection points; update
instead", `reason-paste-update-older` "The pasted version is not newer than the project's", `card-paste-versions-title`
"The pasted records use other part versions", `paste-adapt` "Use the project's copy", `paste-update` "Update the
project's copy".

### 9.6 Describe and transcript

The dialogs print under `dialog` (WP-S2.17 part 4's line); the card under `card` with the origin word `part`, as the
other cards. The result list is windowed like a navigator's (WP-S2.06 D15, 50 rows), and its count line names the rows
shown, so a test asserts named rows and their order, never the total (the starter library grows):

```text
dialog "Choose a part for F55"
  query "" kind all category all fits off view all inactive off
  results showing 1 to 50
  result 1 "EW-GEN-MCB-2" "Generic" "Miniature circuit breaker 1P+N" library "ew-starter-parts" compatible same-fndef cursor
  result 2 "FX-MCB-1PN" "Fixture" "MCB 1P+N" library "Fixture parts" compatible same-fndef
  result 3 "EW-GEN-MCB-1" "Generic" "Miniature circuit breaker 1P" library "ew-starter-parts" compatible same-category favourite
  result 4 "FX-MCB-1P" "Fixture" "MCB 1P" library "Fixture parts" compatible same-category
  result 5 "<Golden A's first copy in natural order>" "<its manufacturer>" "<its description>" project compatible other
dialog "Edit part 5SY4204-7 (Siemens)"
  field "Manufacturer" "Siemens"
  field "Part number" "5SY4204-7"
  field "Description" "MCB 1P+N C4"
  field "Unit" "piece"
  tech "rated_current" "4 A"
  read-only "Connection points" "none"
  source library "<the copy's library>" version "<its version>" fork on
  invalid "Manufacturer" "A part needs a manufacturer"
card part "Edit part 5SY4204-7" changes 1 devices 1
```

**New describe words, part 4a (at most 12; fewer where an existing word fits, a reviewed vocabulary change):**
`fits`, `all`, `inactive`, `compatible`, `same-fndef`, `same-category`, `other`, `favourite`, `favourites`, `tech`,
`version`, `fork`. **Part 4b (at most 5):** `adapt`, `update`, `patch`, `minor`, `major`, in the card's lines
`conflict "<part>" project "<version>" fragment "<version>" <class>` and `choice adapt|update enabled` or `disabled:
"<reason>"`. No new transcript member: the choices are the actions' own transcript lines, and the favourites and recent
lists are not printed outside the dialog.

## 10. Determinism, the headless mode and exact counts

1. **Every flow is a script.** Each acceptance flow of §11 that runs in kittest also runs as an `ew.uiscript/1` script
   through the built binary, byte-equal on two runs; the chooser list's pick is window-only (headless passes `part`).
2. **Every commit is reproducible.** The `--record` log of each script, applied with `ew apply` to the starting
   project, gives equal records; `terminal.insert`, `terminal.move` and the compiled `fields.set` carry explicit values.
3. **Tie-breaks are named:** the mounting order (D2, D3), the carrier (D6), the group rewrite (D8), the part choices'
   order (natural part number, then manufacturer, description, ID), the new designations (D14), the move permutation
   (§5.2). None depends on a hash map's order or on time (NFR-REL-07).
4. **Exact counts** (NFR-PERF-20): a strip-editor gesture makes exactly one `preview_with` and then one `commit` (window)
   or one `run_script` (headless), or none more until Apply; moving the cursor or marking rows causes 0
   `navigator_table` calls; `StripBridges` executes once per strip per revision that changes the strip's terminals or
   their bridge connections.
5. **No time values, paths, process IDs or UUIDs** in transcripts or describe text.

## 11. Acceptance checks

All tests are ordinary nextest tests in the existing jobs, on temporary copies of Golden A (or a wizard project), with
`EW_CONFIG_DIR` set to a temporary directory. A golden fact of this document that the data contradicts stops the lane
for a lead ruling.

### 11.1 Part 1

| # | Check | Where | Proves |
|---|---|---|---|
| SE1 | `terminal.insert`: every rejection with its pointer; §5.1's example on Golden A (seqs, groups, no designation or bridge change); an accessory after 4 stays before the inserted terminal; undo in one step; DSL verb and fuzzer arm | `ew-commands`, `ew-testkit` tests | §5.1, D4, FR-TRM-013's acceptance |
| SE2 | `terminal.move`: every rejection; §5.2's example; a move to the end; groups and accessories unchanged; undo in one step; verb and arm | `ew-commands`, `ew-testkit` tests | §5.2, D4 |
| SE3 | `strip.accessories` is a `fields.set` route (the catalogue file regenerated: `read-only:no-command` → `fields.set:strip.accessories` for its five lines); a dangling part link is refused by the link check | `ew-commands`, `ew-api` tests | D5 |
| SE4 | `StripBridges` and the BOM: on a fixture strip with BR groups 1 (two connections, part on the second) and 2 (no part), the carrier is group 1's second connection, `DevicePartLines(strip)` has one `Bridge` line; Golden A's BOM, terminal diagrams (903 to 905) and wire list (907) byte-equal to the expected files; `bridges` and `bridge_groups` give BR1 to BR6 as built | `ew-derive`, `ew-reports`, `ew-api` tests | D6 |
| SE5 | The part resolver: kind filter (TBD with `end_bracket` resolves to the end bracket, with `accessory` to the bridge), the description form parses back, two matches are `Ambiguous` with both choices, the Parts row unchanged for a unique number | `ew-api` tests | D7 |
| SE6 | **G-PP's H4 rows**, script and kittest: from the task's prelude, three strips and their terminals, four end brackets by `strip.add_accessory` (no `part`, the one end-bracket copy), BR1, BR5 and BR6 by `strip.bridge` on terminals selected by `select.set` with tags, the Bridge part "TBD" by one fill-down over the bridged rows; the exported parts list's rows for End bracket (4) and Plug-in bridge (3) equal `oracle/bom.csv` rows 13 and 14 | `bin/elektrawatt/tests` | §6.5, §6.6, D5, D6, FT-548 |
| SE7 | The strips table on Golden A: `Order` sort (-X1031 1 … 20, then -X1032 1 … 5, then 1X1: in natural order `-` sorts before a digit, `ew_mltext::natural_cmp`); the `pos` cells; `strip:-x1031` shows 20 rows; an accessory row placed and numbered as D3 for `after_seq` 0, 20 and two at 4; the read-only reasons of §6.2 | `ew-api`, `ew-viewmodel` tests | D2, D3 |
| SE8 | Group labels: on Golden A, "Reserve" on -X1031:7 and :8 gives 1–4, 5–6, 7–8, 9–10, 11–14, 15–20 with the old labels; "Pneumatics" on 5 … 10 gives 1–14 and 15–20; clearing 5 … 10 gives 1–4, 11–14, 15–20; a group with an unknown member keeps it on each piece; each is one `fields.set` and one undo step; the Properties "Group" row of a terminal does the same | `ew-api`, `ew-viewmodel` tests | D8, D13, FT-553 |
| SE9 | Strip-editor gestures, kittest and script: Insert before -X1031:5 (§6.4, toast, cursor), Ctrl+Delete on two terminal rows opens the card with their bridges and wires, on an accessory row commits, on a mix refuses; Alt+Up on :13 three times puts it at seq 10's place (three commands, three undo steps); Enter on an unplaced spare terminal (-X1031:15) with folio 103 open arms the Place tool with its function | `ew-viewmodel`, `ew-ui` tests | §6.4, §6.8, D12, D14, FR-TRM-015 |
| SE10 | **FT-548:** `strip.bridge` works with the terminals selected by click (representations), by `select.set` with `tag:` (devices), by navigator marks (devices) and by functions, with one result; two strips refuse `reason-bridge-strips` | `ew-viewmodel` tests | D9 |
| SE11 | **FT-576:** a bridge between two part-placed terminals whose part has a shaft-1 entry ends on the shaft CPs; a terminal whose function definition has no side-1 CP and no part shaft refuses `reason-bridge-no-shaft` | `ew-viewmodel` tests | FT-576 |
| SE12 | **FT-585:** two terminals bridged without parts (side-1 fallback), then given a part with a shaft: Bridge terminals on the pair refuses `reason-bridge-exists`; on the pair and a third terminal it adds one connection to the same group; the side-1 connection is unchanged | `ew-viewmodel` tests | D10 |
| SE13 | `strip.unbridge`: removes BR1 whole (three connections, one step); removes the end link 3–4; refuses the middle link 2–3 with `reason-unbridge-split`; refuses `reason-unbridge-none` | `ew-viewmodel` tests | D11 |
| SE14 | Transcript and describe: §6.10's lines; 268 actions and 356 words; the blind references (`actions.txt`, `vocabulary.txt`) re-blessed; the scripts twice byte-identical; replay by `ew apply`; exact counts of §10 item 4; `cargo xtask blind replay gpp-2` holds (R1, R2), or its transcript differs only by the lines of D2, D13 and §6.10, in which case the lane re-blesses it by the harness's documented procedure and lists the changed lines | lane; tests | §6.10, §10 |
| SE15 | Golden unchanged (golden output, acceptance and AC-15 tests unmodified); the accessible-name walker on the strip editor (the accessory row a row of the grid, the chooser list a `ListBox`); targeted checks for the changed crates (`ew-commands`, `ew-testkit`, `ew-derive`, `ew-reports`, `ew-api`, `ew-viewmodel` with and without `testkit`, `ew-ui`, `elektrawatt`): nextest, clippy `-D warnings`, `cargo fmt --all --check`, `check-deps`, `check-features`, `check-raw-read`, `check-licence-map`, `reuse lint`, `docs-gen --check`; no new package | lane | §4.1 |

The workspace suite, the acceptance runner and the performance runs are not run by the lanes (testing policy, ID-512);
the integrating session runs `cargo xtask ci` at the cycle delivery.

### 11.2 Parts 2 and 3

As outlined in §7 and §8 (built in cycles 35 and 38; their as-built notes record what holds). Part 4's outline is
closed in §11.3 (D15).

### 11.3 Part 4

**The fixture catalogue** (PS1 to PS4, PS8): a user library "Fixture parts" written by the test into a temporary
`EW_DATA_DIR` with the library writer (as `crates/ew-library/tests/starter_parts.rs` writes the starter parts), with
the manufacturer "Fixture" and the parts FX-MCB-1PN (an MCB 1P+N whose template names the starter's `ew.fn.mcb-1pn`),
FX-MCB-1P (template `ew.fn.mcb-1p`), FX-CON-1 (a contactor, template coil), FX-ENDB-1 (kind `end_bracket`, no
template), FX-BRG-1 (kind `accessory`, a plug-in bridge) and FX-MCB-OLD (deprecated, successor FX-MCB-1PN). `EW_CONFIG_DIR` is a temporary directory in every test.
No test pins the number of offers or of starter parts (CONTRIBUTING.md); they assert named rows and their order.

| # | Check | Where | Proves |
|---|---|---|---|
| PS1 | **F55's offer order** (4a): on a temporary copy of Golden A with the fixture catalogue, F55 selected, `part.choose` on its Parts row (`use` 0), no text: the first four offers are EW-GEN-MCB-2 and FX-MCB-1PN (`same-fndef`), then EW-GEN-MCB-1 and FX-MCB-1P (`same-category`), in that order; every later offer is `other`, held copies first; FX-MCB-OLD is absent. `ew-library`: `search` with `fndef` set gives `same_fndef` true for the two 1P+N parts and false for the two 1P parts, which stay `compatible`; IX-5 holds unchanged without `fndef`. If F55's function's FnDef is not the starter's `ew.fn.mcb-1pn` item, the lane stops (a golden fact) | `ew-library`, `ew-api`, `ew-viewmodel` tests | FR-PRT-015's acceptance, D17, FT-629 |
| PS2 | **Search and filters** (4a), on a wizard project with the fixture catalogue: "FX-MCB-1P" gives FX-MCB-1P (exact) before FX-MCB-1PN (prefix); "mcb 1p+n" finds both 1P+N parts by their descriptions; the kind `end_bracket` leaves FX-ENDB-1 and no MCB; the category's first level of the MCBs leaves MCBs only; Only parts that fit, for a 1P+N MCB device, hides `other`; Show deprecated and withdrawn adds FX-MCB-OLD with "successor FX-MCB-1PN" in the detail line; the view In this project lists held copies only; a library item the project holds shows once, as held | `ew-api`, `ew-viewmodel` tests | D17 |
| PS3 | **Favourites and recent** (4a): `part.favourite` on FX-MCB-1P writes `[parts] favourites` to `settings.toml`; a new shell on the same `EW_CONFIG_DIR` lists it in the view Favourites and, in the view All with no text, before the non-favourite, not held offers of its fit and tier; choosing FX-CON-1 then EW-GEN-MCB-2 gives `recent` = [EW-GEN-MCB-2, FX-CON-1]; choosing a project-local copy records nothing; 21 recorded choices keep 20; no tracked project file changes through any of it | `ew-viewmodel` tests (`settings.rs`, the shell) | D18, FR-UI-033 |
| PS4 | **Choosing commits one command** (4a): on Golden A, F55's use 0 → EW-GEN-MCB-2: one `part.assign` (`replace`, `from` = 5SY4204-7's copy, `copies` = the part, the manufacturer "Generic" and the FnDefs the project lacks), one undo step back to the original bytes, and its `--record` log applied with `ew apply` to the starting project with an empty `EW_DATA_DIR` gives equal records; Add part… gives `append`; a held copy gives `part.assign` without `copies`; the strip editor's Part cell on two marked terminals gives `replace_all` on both; BR1's Bridge part cell (kind preset `accessory`) with FX-BRG-1 gives one `fields.set` with `copies` and one undo step (the BOM's `Bridge` line names FX-BRG-1); "Search all parts…" in the accessory chooser opens the dialog with the kind preset. `ew-commands`: `fields.set` and `fields.set_many` with `copies` embed the closure in the same transaction, reuse a held copy of the same hash, and refuse another version at `/copies/<i>`; the fuzzer arms cover the member | `ew-commands`, `ew-api`, `ew-viewmodel` tests | D16 |
| PS5 | **The part editor's refusals** (4a; FR-PRT-022), on a wizard project with a copy from `parts.import` (project-local): `part.edit`, `part.field manufacturer ""`, `part.save` is refused "A part needs a manufacturer", printed under the field and as the refusal line, and the history is unchanged; likewise an empty part number, the unit "pcs", the technical value "4 Amp" (the quantity rule's message after "rated_current: "), a key twice, an unchanged draft and a read-only project. `part.edit_copy` itself refuses each case of §9.3 at its pointer | `ew-api`, `ew-commands`, `ew-viewmodel` tests | D20, D21 |
| PS6 | **FT-394's previewed edit in place** (4a), on the same project: the description changed opens the `part` card with the field diff, the devices that use the copy and the BOM rows that change; Apply is one undo step; the copy ID, item ID and version are unchanged and `sha256` is the hash of the new item (EW-CORE-014 silent); undo restores the copy file's bytes | `ew-api`, `ew-viewmodel` tests | D20, FT-394 |
| PS7 | **FT-394's fork** (4a), on a temporary copy of Golden A: Edit part… on F55, the description "MCB 1P+N C4", Save: the card carries the fork line; Apply adds one copy under `lib/parts/` with the project's library ID, the item ID `next_fork_item` gave and `meta.source.fork_of` naming the original's library, item and version; F55's part use names the fork; the original copy's file is unchanged; the Libraries navigator shows the fork as Local fork; the exported parts list differs from Golden A's expected one only where it prints 5SY4204-7's description, every other export byte-equal; undo restores; the `--record` log replays equal; a second edit of the fork is in place (no second fork) | `ew-api`, `ew-viewmodel` tests | D20, doc 07 §3.1 |
| PS8 | **FT-396's paste choice** (4b): wizard projects P1 holding FX-MCB-1PN 1.0.0 on -F1 and P2 holding 1.1.0 (a description change, patch). -F1 copied in P1 and pasted in P2: the choice card lists the copy with "patch"; Use the project's copy is one `edit.paste` with `adapt` (the pasted device's part use names P2's copy, one undo step); Update the project's copy is disabled with `reason-paste-update-older`. Pasted the other way: Update opens `library.update`'s card from the fragment's document, Apply, then the paste runs (two undo steps; P1 holds 1.1.0). A fixture version that removes a CP: Use the project's copy disabled with `reason-paste-adapt-major`, and `edit.paste` with that `adapt` refused at `/adapt/0`. Headless: `edit.paste_clip` refused with `reason-paste-versions`; `paste.resolve choice=adapt` runs. Keep both is not offered | `ew-commands`, `ew-api`, `ew-viewmodel` tests | D22, FT-396, doc 06 §3.6 item 4 |
| PS9 | **Transcript, describe and the window** (4a and 4b): §9.6's lines; the actions of §9.4 and the words of §9.6 present in the registry and `VOCABULARY` (asserted by membership, not by totals); the blind references re-blessed; one `ew.uiscript/1` script per flow of PS1, PS4, PS5 and PS7 (and PS8 in 4b), twice byte-identical and replayed by `ew apply`; kittest flows of the dialog and the editor by keyboard alone (NFR-A11Y-01); AccessKit: the result list a `ListBox` named "Parts" with `ListBoxOption` rows (position and set size), the editor's fields named by their labels, the favourite star named "Favourite" | lane; `ew-viewmodel`, `ew-ui`, `bin/elektrawatt` tests | §9.4 to §9.6, §10 |
| PS10 | **Golden unchanged and targeted checks:** golden output, acceptance and AC-15 tests unmodified; for the changed crates (`ew-library`, `ew-commands`, `ew-testkit`, `ew-api`, `ew-viewmodel` with and without `testkit`, `ew-ui`, `elektrawatt`): nextest, clippy `-D warnings`, `cargo fmt --all --check`, `check-deps`, `check-features`, `check-raw-read`, `check-licence-map`, `reuse lint`, `docs-gen --check`; no new package | lane | D24 |

## 12. Files owned and what a lane may not decide

### 12.1 Files owned

**Part 1:** `crates/ew-commands/src/commands/{terminal.rs,fields.rs,mod.rs}` (the two commands, `SETTABLE`),
`crates/ew-commands/schemas/**`, `crates/ew-commands/tests/**`; `crates/ew-testkit/src/golden/dsl.rs` (two verbs),
`crates/ew-testkit/tests/command_fuzz/**` (two arms); `crates/ew-derive/src/{bom.rs,lib.rs}` and a new
`crates/ew-derive/src/strip_bridges.rs`, `crates/ew-derive/tests/**`; `crates/ew-reports/src/kinds/conn.rs`
(`bridges` on `StripBridges`, `bridge_groups`), `crates/ew-reports/src/lib.rs` (the re-export); `crates/ew-api/src/output/{navigators.rs,fields.rs,properties.rs,mod.rs}`,
a new `crates/ew-api/src/output/strip_edit.rs`, `crates/ew-api/src/lib.rs` (methods and re-exports only),
`crates/ew-api/tests/**` with `tests/expected/field_catalogue.txt` (regenerated); `crates/ew-viewmodel/src/**`
(`shell/bridge.rs`, `shell/table.rs`, `navigators/table.rs`, `navigators.rs`, `shell/properties.rs`, a new
`shell/strip.rs`, `actions.rs`, `describe.rs`, `strings.rs`, `script.rs`); `crates/ew-ui/**` (the grid's accessory
rows, the chooser list, the drag source); `tools/blind/reference/*.txt` (re-blessed); `tools/blind/recordings/gpp-2/**`
only under SE14's rule; `bin/elektrawatt/tests/**`; the generated reference pages (`docs-gen`).

**Parts 2 and 3:** named by their dispatch briefs (built).

**Part 4a:** `crates/ew-library/src/{search.rs,index.rs}` (D17's additive members) and `crates/ew-library/tests/**`;
`crates/ew-commands/src/commands/{fields.rs,mod.rs}` (the `copies` member) and a new
`crates/ew-commands/src/commands/part_edit.rs` (`part.edit_copy`), `crates/ew-commands/schemas/**`,
`crates/ew-commands/tests/**`; `crates/ew-testkit/src/golden/dsl.rs` (the verb `part_edit_copy`),
`crates/ew-testkit/tests/command_fuzz/**` (one arm, and `copies` on the `fields.set` arms);
`crates/ew-api/src/library_ops.rs`, new `crates/ew-api/src/part_offers.rs` and `crates/ew-api/src/part_editor.rs`,
`crates/ew-api/src/output/{strip_edit.rs,fields.rs}`, `crates/ew-api/src/lib.rs` (methods and re-exports only),
`crates/ew-api/tests/**`; `crates/ew-viewmodel/src/**` (`dialogs.rs`, `settings.rs`, `actions.rs`, `describe.rs`,
`strings.rs`, `script.rs`, `shell/properties.rs`, `shell/strip_editor.rs`, `shell/parts.rs` or a new
`shell/part_dialog.rs`, the Libraries navigator's row menu); `crates/ew-ui/**` (`dialog.rs`, `chooser.rs`, a new
part-dialog module, the kittest flows); `tools/blind/reference/*.txt` (re-blessed); `bin/elektrawatt/tests/**`; the
generated reference pages (`docs-gen`).

**Part 4b:** `crates/ew-commands/src/commands/paste.rs` (the `adapt` member), its schema and tests, the
`edit.paste` fuzzer arm; `crates/ew-api/src/library_ops.rs` (`update_from_documents`, `paste_versions`),
`crates/ew-api/tests/**`; `crates/ew-viewmodel/src/{shell/editing.rs,shell/clipboard.rs,preview.rs,actions.rs,describe.rs,strings.rs}`;
`crates/ew-ui/src/preview.rs`; the blind references and the generated reference pages as part 4a.

Shared append-only files (registry lists, `VOCABULARY`, `params.rs`, fuzzer arms, DSL verbs, the strings table) may be
touched by other lanes; the second to integrate rebases onto the first. Read-only for the lanes: every other crate,
`golden/**`, `tools/blind/tasks/**` (the H4 gap entry of `gaps.toml` is removed by the design pass after part 1's
delivery), `ci/**`, `deny.toml`, `.github/**`, `docs/**` except the generated reference pages. Each lane commits on its
own branch and does not merge or push.

### 12.2 What a lane may not decide

A lane stops and reports instead of choosing when:
1. a change would touch `ew-schema`, the project format, a record log or any byte of Golden A's expected outputs (SE4,
   SE15), or would need a new package or dependency edge;
2. `bridges`' output on `StripBridges` differs from the as-built output in any report (D6);
3. a route of §6.2 is not what the catalogue says (an editable column whose route is not a `FieldsSet` of a scalar or
   of `parts`, `strip.groups`, `strip.accessories` or `overrides.wire_part`);
4. a requirement or golden fact of this document is contradicted by the data;
5. a shortcut of §6.9 conflicts in the keymap test;
6. `gpp-2`'s replay differs beyond SE14's allowance, or its outcomes (not only its describe text) change;
7. it would add a command beyond §5, an action beyond §6.9, a describe word beyond §6.10, or change the behaviour of
   an as-built action other than `strip.bridge` (D9, D10) and the Parts row's resolver (D7);
8. part 2, 3 or 4 work is reached (D15); for part 4's lanes: part 4a reaches part 4b's work (D22), or either reaches
   FT-444's build (D23);
9. (part 4) a command beyond `part.edit_copy`, a command member beyond `copies` on `fields.set` and `fields.set_many`
   and `adapt` on `edit.paste`, an action beyond §9.4 or a describe word beyond §9.6 would be needed;
10. (part 4) the part editor would edit a member outside D20's list (a CP, the function template, the contacts, the
    sections, the mechanical data), write to a library (promoting a fork to a company library is a library write, not
    part 4), or offer "keep both" (D22);
11. (part 4) favourites or recently used parts would be written into the project or its `.ew/` directory (D18);
12. (part 4) `PartHit.compatible` or WP-S1.19 D6's order without `fndef` would change (IX-5 must hold unchanged), or a
    reference to a part copy that a fork must re-point is not in `ew_model::LINKS` (D20);
13. (part 4) a golden fact of §11.3 is contradicted by the data (F55's FnDef, the starter's MCB templates).

The lane decides freely: private helper names and module layout inside the owned files, the exact Rust field names of
§4.2's items (their variants and meaning fixed), test fixtures, and the kittest mechanics.

## 13. Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 Rebuilding `ew-reports`' bridges on `StripBridges` changes a golden byte (label order, a cross-strip bridge) | Low · high | SE4's byte-equal checks; §12.2 item 2 stops the lane; the old function may stay beside the new query if equality cannot be shown |
| R2 `gpp-2` stops replaying (the Properties labels of D13, the `Order` sort, the `strip.bridge` predicate) | Medium · medium | SE14's rule: describe-only differences are re-blessed and listed; outcome changes stop the lane |
| R3 Double counting of bridge parts where a project keeps Golden A's interim part uses on the strip and also sets bridge parts | Medium · low | D6 names it; the follow-up migrates Golden A; the BOM row shows the sum plainly |
| R4 The group rewrite surprises a user who kept groups over empty ranges for future terminals | Low · low | Only groups touching the edit change (D8); a piece without terminals is the only loss, and undo restores it |
| R5 The strip editor as a navigator filter (D1) is less discoverable than a dedicated pane | Medium · low | Edit ▸ Strip editor and the strip's row menu open it; the pane title names the strip while the `strip:` facet is set; doc 16's side-by-side editors are a follow-up |
| R6 Part 1 overruns 1.4 pw | Medium · medium | The de-scope rule of §14.1 |
| R7 A fork re-points a reference kind the lane did not foresee, or leaves the original copy referenced | Low · high | D20 reads the references from `ew_model::LINKS`, not from a hand list; PS7 checks every export byte-equal but the parts list; §12.2 item 12 stops the lane |
| R8 `copies` on `fields.set` lets a cell edit embed a copy of another version beside a held one | Low · high | The same identity and version check as `library.embed` and `part.assign` at `/copies/<i>` (PS4); the copy ID rule of doc 07 §2.1 is unchanged |
| R9 Part 4a overruns 0.9 pw | Medium · medium | The de-scope rule of §14.1 |

## 14. Size, order, routing and the documents to update

### 14.1 Size

WP-S2.07 keeps **3.6 pw PERT** (O 2.5 · L 3.5 · P 5); the CSV is unchanged. The minimal part chooser left the package
in cycle 33 and the strip accessories joined it; the four parts fill the PERT with **no reserve**: **part 1, 1.4 pw**
(the two commands with schemas, verbs, arms and tests 0.25; `SETTABLE` and the catalogue 0.02; `StripBridges`, the
BOM lines and `bridges` on it 0.15; the provider's columns, accessory rows, facet and order 0.2; the resolver 0.08;
`strip_edit` with the group, bridge-part and accessory compiles 0.2; the actions, the table plumbing, bridge rules D9
to D11 and Properties 0.3; ew-ui and kittest 0.1; transcript, describe, scripts and re-bless 0.1), **part 2, 0.6 pw**,
**part 3, 0.7 pw**, **part 4, 0.9 pw** (re-sized at its specification pass, below). **De-scope rule:** when part 1 is projected (at its lane's midpoint report) or
found (at its review) more than 20 % over its size, these items leave it as follow-ups without a further ruling, in
this order, until the projection fits: Enter-to-place (§6.8; the drag stays), then `strip.move`'s `before` parameter
(the step stays), then the Properties "Group" row (the table column stays); the lane reports which ones left.

**Part 4 at its specification pass (cycle 39, ID-894): 1.1 pw, split.** Part **4a, 0.9 pw:** the two-level fit in
`ew-library` 0.05; `part_offers` and `part_pick_command` 0.15; `copies` on `fields.set` and `fields.set_many` with
schemas, arms and tests 0.08; `part.edit_copy` (in place, fork, re-pointing, validation, schema, verb, arm, tests) 0.2;
`PartDraft`, `part_edit_command` and the card's plan 0.1; the two dialogs, the actions, the settings table, describe and
strings in `ew-viewmodel` 0.17; `ew-ui` and kittest 0.1; scripts, replay and re-bless 0.05. Part **4b, 0.2 pw:**
`adapt` on `edit.paste` 0.05, `paste_versions` and `update_from_documents` 0.07, the choice card and `paste.resolve`
0.05, tests 0.03. The package's 3.6 pw PERT has no reserve, so part 4b is **0.2 pw over it** (3.8 pw used); the CSV
is unchanged and the overrun is recorded in ID-894. **De-scope rule for part 4a** (> 20 % over, as above): the
favourites (the recent list stays), then the catalogue path of the Bridge part and accessory Part cells (held copies
only; `copies` on `fields.set` leaves with it), then the part editor's technical data rows (the identity fields stay);
the lane reports which ones left.

### 14.2 Order and routing

Part 1 runs in cycle 34's later wave (WP-S2.06 is delivered; it depends on no other open package). Parts 2 and 3 follow
in any order after part 1's delivery (they share the table plumbing part 1 extends). Part 4 starts after WP-S1.19's
catalogue search is delivered. **Routing** (ID-667, ID-700: Opus or Haiku only): part 1 to one lead-programmer lane,
**Opus at `high`** (two core commands, a memoised query below the reports, and compiles from user input into
commands); parts 2 and 3 to programmer lanes, **Opus at `medium`** (closed work in a decided pattern with oracles);
part 4 to a lead-programmer lane, Opus at `high`. *Part 4 at its specification pass (ID-894):* **part 4a first**,
one lead-programmer lane, Opus at `high` (a command that writes copies and re-points references, and a member on the
core edit command), in cycle 39's second wave; **part 4b after part 4a's integration**, one programmer lane, Opus at
`medium` (closed work on `edit.paste` and the delivered `library.update`), in a later wave or cycle. FT-444's build
(D23) is not part 4's. Each part is one lane plus the cycle's single independent review and
its fix round.

### 14.3 Follow-ups to record (numbered by the design pass that records part 1's delivery)

1. Golden A's interim accessory and bridge part uses (ID-316) moved onto `Strip.accessories` and bridge parts, the
   expected outputs re-blessed (WP-S2.07 or WP-S1.26, M2).
2. Re-seating a side-1 fallback bridge onto the part's shaft, with the terminal assembly list (D10; FR-MFG-016, 1.0).
3. Several strip editors side by side as table panes (doc 16 §3.4) (WP-S2.14, 1.0).
4. `fields.set` on `terminal.seq` keeps sequence numbers unique (refuse a duplicate at transaction end, or route the
   column to `terminal.move`) (WP-S1.14, M2).
5. FR-TRM-008's bridge proposals and FR-TRM-024's strip length in the strip editor (WP-S1.26, 1.0).

### 14.4 Documents the design passes update

**Now (cycle 34):** plan 02 (WP-S2.07's status and this specification); decision log ID-782. **After part 1:** doc 08
§2.3 (the carrier and `strip_bridges`, the terminal-identity rule replacing the as-built FT-585 paragraph), §6.1 (the
printed label in Properties), §9 (the strip editor as built); doc 16 §5.2 (the Terminal strips navigator's `Pos` and
`Bridge part` columns, the `Order` sort, the `strip` facet), §7.2 (row insert, delete and move as built); doc 16a §6.21
(accessory rows) and §6.22 (the chooser list's first use); doc 06 §3.1 (the command count 58, `strip.bridge`'s
predicate); doc 17 §2.1 (the count); doc 02a FR-TRM-013's and FR-TRM-016's notes (built, in part); WP-S1.26's row (strip
accessories brought forward in part); `tools/blind/tasks/gpp-101-103/gaps.toml` (H4's gap entry removed); the
traceability and the glossary (strip editor, accessory row, bridge carrier). **After part 4a:** doc 07 §3.1 (the fork
as built), §3.3 (the dialog's commands), §4.3 (the two-level fit, FT-629), §5 (the part browser and the part editor as
built); doc 06 §3.1 (the command count, `part.edit_copy`, `copies` on `fields.set`); doc 17 §2.1 (the count); doc 16
§2.5 (the `[parts]` table of the user settings); doc 16a §6.12 (the part dialog as a form dialog with a list); doc 02a
FR-PRT-015's and FR-PRT-022's notes; FT-394, FT-629 and FT-754 (i) closed; FT-444 annotated with D23. **After part
4b:** doc 06 §3.6 (item 4 as built: adapt and update, keep both a follow-up); FT-396 closed or narrowed to keep both.

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S1.14, WP-S1.19, WP-S1.21, WP-S1.26, WP-S1.28, WP-S2.04, WP-S2.05,
  WP-S2.06, WP-S2.07, WP-S2.13, WP-S2.14; Appendix B (FT-394, FT-396, FT-411, FT-429, FT-444, FT-490, FT-548, FT-553,
  FT-576, FT-585, FT-629, FT-656, FT-673, FT-754).
- [Decision log](../06-decision-log.md): ID-316, ID-512, ID-598, ID-646, ID-667, ID-681 to ID-689, ID-700, ID-768,
  ID-770, ID-771, ID-776, ID-777, ID-779, ID-780, ID-818, ID-820, ID-843, ID-885, ID-894.
- [Gates](../03-gates.md) §5.4 (G-PP's record and its H4 gap).
- [ADR-0006](../../adr/0006-function-device-representation-model.md),
  [ADR-0008](../../adr/0008-transactions-undo-preview.md), [ADR-0009](../../adr/0009-invariant-catalogue.md),
  [ADR-0014](../../adr/0014-gui-toolkit-egui.md);
  [domain model](../../design/03-domain-model.md) §11, §12; [schematic editor](../../design/06-schematic-editor.md) §2.7,
  §3.6; [symbols, parts and macros](../../design/07-symbols-parts-macros.md) §2.1, §3.1, §3.3, §4.3, §5; [connections, terminals and
  cables](../../design/08-connections-terminals-cables.md) §2, §6, §9; [UX/UI](../../design/16-ux-ui.md) §3.4, §5.2,
  §7, §18.1; [design language](../../design/16a-design-language.md) §6.21, §6.22; [golden
  project](../../design/20-golden-project.md) §9, §14.1.
- Requirements: FR-TRM-002, FR-TRM-003, FR-TRM-007, FR-TRM-010, FR-TRM-013, FR-TRM-015, FR-TRM-016, FR-TRM-021,
  FR-TRM-022, FR-TRM-026, FR-CBL-016, FR-PLC-020, FR-PLC-021, FR-PRT-015, FR-PRT-022, FR-DEV-018, FR-UI-008,
  FR-UI-044, NFR-A11Y-01, NFR-REL-07, NFR-PERF-20.
