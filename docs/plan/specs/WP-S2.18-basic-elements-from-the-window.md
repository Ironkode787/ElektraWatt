# WP-S2.18: basic elements from the window (closed specification)

**Status:** specified in the cycle-34 design pass (documents only), an M2 package, in four parts: part 1 joining an
existing device from the window (1.1 pw, D4), part 2 the generic starter parts and their reach from a new project
(1.0 pw, D3, after WP-S1.19 parts 2 and 3), part 3 every starter symbol placeable and the briefs without embed lines
(0.6 pw, D3), part 4 the brief `brief-plc-io` (0.3 pw, D2, after WP-S2.07 part 3 and WP-S1.21's channel assignment);
part 1 first and alone of any dependency; parts 2 and 3 in that order; part 4 last · **Part 1 built in cycle 35** (lane A35, decision log ID-817; D3 deferred, FT-628); **parts 2 and 3 built in cycle 35** (lane G35, decision log ID-830); D6's one undo step built in cycle 36 (lane E36, ID-843; FT-656) and the relay's and contactor's contacts templated (lane C36, ID-841; FT-655); **part 4 built in cycle 38** (lane K38, decision log ID-882; FT-593), **so WP-S2.18 is complete**; **D3 built in cycle 37** as `rep.join_device` (lane F37, decision log ID-859; FT-628) · **Date:** 2026-10-29 ·
**Work package:** [WP-S2.18](../02-work-packages.md#wp-s218-basic-elements-from-the-window-starter-symbols-parts-and-joined-functions)
· **Decision log:** ID-812 · **Part of:** ElektraWatt plan, specifications

**Purpose.** The owner asked on 2026-10-29 for "a seperate wp to implement basic elements like that", after the
play-test brief `brief-relays-valves` proved not completable on the cycle-34 build: a drafter cannot put a relay's
normally open contact on the relay from the window (ID-807, FT-599). This package makes the everyday elements of a
control circuit usable from the window, end to end: place a symbol, get a device with a usable part, and put the
device's other functions (contacts, auxiliary blocks) on other pages by naming or picking the device. It is written
against the delivered Place tool (WP-S2.04 §7.2), the picker and the Parts row (cycle 33, ID-770), the command
`device.place` with its `join_device` and `existing_function` targets (WP-S1.13), the starter symbols (WP-S7.02) and
WP-S1.19's library tiers, not against their code. A choice a lane meets that this document does not settle is
escalated, never guessed (`CLAUDE.md`, rule 2).

**As built (cycle 35, part 1).** Lane A35 (8863dde2, eb4e9ed6; decision log ID-817) built D1, D2 and D4: the Place
tool's HUD offers "J Join device…" for an auxiliary symbol; the Join chooser lists "New device", the slots ("slot
13-14") and the devices whose family takes auxiliary functions, in natural tag order with type-ahead; Enter arms the
target (ghost label `join -K1`), which is sticky, Escape keeps it and "New device" restores the default; the active
row's object takes the hover outline; the actions `place.join`, `chooser.row`, `chooser.type` and `chooser.choose`;
`place.pick device=<tag>` and `function=<tag>[:<slot>]`. `brief-relays-valves` and `brief-valve-switch` are runnable
(FT-599), the prelude's part line still in place. **D3 is not built:** the Tag row is read-only and no command moves
a function to another device, so the proposal's button has nothing to commit; FT-628 (an editable Tag row and a core
`function.move_to_device`, then D3).

**As built (cycle 35, parts 2 and 3).** Lane G35 (5d0636b4, 71d5b747, 04b67edc; decision log ID-830). **D5:** sixteen
CC0-1.0 parts of manufacturer "Generic" in `content/libraries/ew-starter-parts.ewlib` (version 0.1.0): EW-GEN-RLY-1,
CON-1, MCB-1, MCB-2, FUS-1, PB-1, SEL-1, POS-1, LMP-1, MOT-1, PSU-1, VLV-1, TRM-1, TRM-2, DI-1 and DO-1, written by the
test `crates/ew-library/tests/starter_parts.rs` (`EW_BLESS=1`) and compiled into `ew-library`, so every session holds
them; a directory of the same library ID replaces the compiled-in copy. Three details differ from D5: the selector
switch has two NO contacts, since the starter set has no one-contact selector FnDef (FT-658); the MCB 3P and the valve
3/2 are de-scoped by §5's rule (FT-657); the relay and the contactor template their coil only and carry their contacts
as contact data, because the CP derivation matches a function to its template entry by FnDef alone (FT-655). **D6:**
the picker's part rows and the Parts row list the library's parts after the embedded copies; the Parts row's choice is
one `part.assign` that carries the closure (one undo step, replays without a library); **a placement from the picker is
two undo steps**, a pre-arm `library.embed` and the placement, accepted for now (FT-656). **D7:**
`crates/ew-viewmodel/tests/starter_symbols.rs` places every starter symbol of a new project by its path (part, strip
terminal, black box, join through the chooser, device without a starter part, box device, drawing aid); the path table
must equal the starter set; the D3 half of "both ways" is not built (FT-628). **D8:** both briefs at version 2 without
`library.embed` lines pass `check-brief`, the potential-naming items still infeasible; `check-brief`'s `part:`
requirement is also met by a shipped starter part (FT-592 closed). The documents of §7 are updated as built in the
cycle-35 design pass.

**As built (cycle 36, D5 and D6).** Lane C36 (f365a043, b30ddfdd; ID-841, FT-655): a function records its template
entry, so EW-GEN-RLY-1 templates its coil and four change-over contacts and EW-GEN-CON-1 its coil and four NO
contacts, created as slots that the Join chooser and `place.pick device=` fill (the first free slot) and
`function=<tag>:<slot>` names. Lane F36 (ecdc79fc; ID-845): EW-GEN-SEL-1 has one NO contact on
`ew.fn.selector-switch-no` (FT-658) and EW-GEN-VLV-1's coil is `ew.fn.valve-coil` (tag letter M under EN
81346-2:2009), `ew-starter-parts` at 0.1.1. Lane E36 (f88e7c7a, fd556b8b; ID-843, FT-656): **D6's one undo step**:
the picker no longer pre-arms with `library.embed`; the Place and black-box tools carry the armed copies (a starter
symbol, its FnDef, a library part's closure) and each preview and commit sends them in `device.place`'s or
`box.black`'s `copies` member, the host leaving out what the project holds, so the placement is one transaction and
one undo step and replays without a library. The exchange flow, the IP tools and `place.pick device=` still embed
in a step of their own (FT-673).

**As built (cycle 38, part 4).** Lane K38 (c2e0d74a, e4990ee7; decision log ID-882). **D8's third brief:**
`tools/blind/tasks/brief-plc-io/`, grade 3: three selector switches on the cabinet door read by a digital input card and
three 24 V DC solenoid valves on the machine switched directly by a digital output card, on the generic CPU with the
starter parts `EW-GEN-DI-1` and `EW-GEN-DO-1` drawn as black boxes; a setup-only prelude without `library.embed` lines;
the reference solution and fourteen items, A1 to A10 hard and A11 to A14 soft. A11 (every used channel's address on
the drawing) is marked infeasible: no `api_v1` view carries a channel's binding or address, and no window flow
attaches a field device to a card, so the PLC navigator's drop moves a channel only between the card's own pins
(FT-753). The lane found that `plc.bind_channel` refused a black-box card's pins; the cycle-38 fix round made it check
the CPs the function draws (FT-750, ID-884). The `check-brief` replay is the lane's; the BV9 tests in
`xtask/src/blind/brief.rs` list only `brief-valve-switch` and `brief-relays-valves`.

## 1. Goal and non-goals

**Goal.** In a new project, with no setup line: every starter symbol can be placed from the window with a usable part;
a contact or other auxiliary function joins an existing device from the window, before or after it is placed; the text
interface reaches the same flows by printed tags. The three play-test briefs `brief-relays-valves`,
`brief-valve-switch` and `brief-plc-io` pass `cargo xtask blind check-brief` with no `library.embed` line in their
preludes.

**Non-goals.** New symbols (WP-S7.04); manufacturer catalogue parts, profiles, address maps, forms and the basic
project (WP-S7.03); the part selection dialog and part editor (WP-S2.07 part 4); macros (WP-S1.19 part 5); the PLC
editor itself (WP-S2.07 part 3); any change to the file format or to `device.place`'s targets.

## 2. What exists and what is missing

| Element | Exists | Missing (this package) |
|---|---|---|
| Symbols | WP-S7.02's starter set: coil, NO/NC/CO contacts, fuse, MCBs, pushbutton, selector and position switches, lamp, motor, heater, transformer, ISO valves and cylinder, terminals, PE, IP, black box | Nothing to draw; each must reach a usable device (part 3) |
| Parts | The picker and the Parts row choose embedded part copies (ID-770); WP-S1.19 part 1's typed part data | A new project has none: the briefs embed copies in their preludes (FT-592); WP-S1.19 part 2 ships `ew-starter-parts` empty |
| Joining a device | `device.place` target `join_device {device, role}` and `existing_function` (WP-S1.13, FR-DEV-038 at the command level); the Place tool joins a black box's device by position (WP-S2.04 §7.2) | No window path for a relay's contact (FT-599); doc 06 §3.3's typed-tag flow is not built; `place.pick function=<tag>` needs an ID |

## 3. Requirements traced

FR-DEV-038 (join an existing device as an auxiliary function, or as another representation of a function with the same
FnDef and CP designations, as one transaction), FR-DEV-005 (main and auxiliary functions on different pages, linked
for cross-references), FR-UI-017 (placing a part creates its device with the remaining functions unplaced), FR-SYM-015
(while placing, rotate, mirror and cycle variants). Four rows, shared with WP-S1.13 and WP-S2.04, which built their
command and tool halves. FT-592, FT-593 and FT-599 close with this package.

## 4. Decisions

| # | Decision | Why |
|---|---|---|
| D1 | **Two ways in, one command.** Joining is always `device.place` with `join_device {device, role: aux}`, or with `existing_function` when the device has an unplaced function of the symbol's FnDef (a part's template slot, FR-UI-017): the slot is filled before a new auxiliary function is made | One transaction, undo as one step, no new command for the before-placing path |
| D2 | **Before placing: Join in the Place tool.** The HUD shows "J Join device…" while the armed symbol's FnDef can be an auxiliary function. J opens the chooser list (doc 16a §6.22) of the project's devices that can take it: devices with an unplaced function of that FnDef first (their tag, then "slot 13-14" in the kind column), then devices whose main function's family accepts auxiliary functions (relays, contactors, motor protection, the device's part allowing it), each in natural tag order; type-ahead on the tag. Enter arms the target; the ghost label reads `join -K1` (WP-S2.04 §7.2's label vocabulary); the target is sticky across placements like the others; "New device" is the chooser's first row and J again reopens it. If J is taken in the Place tool's context, the lane takes the next free letter the keymap test accepts and reports it | The user sees the candidates instead of remembering tags; the chooser and the HUD are existing components (ADR-0031) |
| D3 | **After placing: the typed tag proposes the join.** In Properties, typing the tag of an existing device into the Tag field of a device that has exactly one function, no part and no other representation refuses the rename with the proposal under the field: "-K1 exists. Join it as an auxiliary contact?" and a Secondary button "Join -K1" (the pattern of FR-NUM-029's tag proposal, ID-780 (5)); the button re-targets the placed representation to the device (D1, at the same page, position and orientation, its wires kept) in one transaction; the toast says "Joined -K1 (NO contact 13-14) at 102-C4". A tag that names no device keeps today's behaviour (scheme parse, else the frozen-tag question, doc 06 §3.3) | It is doc 06 §3.3's flow, and the user's habit from other ECAD tools; a proposal, never a silent merge |
| D4 | **The text interface uses printed tags.** `place.pick` takes `device=<tag>` (arms `join_device`) and `function=<tag>:<slot>` or `function=<tag>` (arms `existing_function`; a tag with one unplaced function of the armed FnDef resolves to it, several are refused with their slots listed), resolved by FT-544's bare-tag grammar; an ID still works | Closes FT-599's text half; the blind drafters never type IDs |
| D5 | **The generic starter parts.** One CC0 part per starter function that needs a part to be usable (a device with a parts-list row, ratings on text slots, and the auxiliary slots of its type): a relay with four changeover contacts, a contactor with three main NO and one auxiliary NO, an MCB 1P, 1P+N and 3P, a fuse holder, a pushbutton with one NO, a selector switch with one NO, a position switch, a pilot lamp, a three-phase motor, a power supply 230 V AC to 24 V DC, a solenoid valve 5/2 and 3/2, a feed-through and a PE terminal, and, for part 4, a PLC digital input and a digital output card. Manufacturer "Generic", part numbers `EW-GEN-<kind>-<n>`, no order or catalogue data; in the system library `ew-starter-parts` (WP-S1.19 §7.1). They move here from WP-S7.03, which keeps the golden and pilot manufacturer parts (ID-812) | The window cannot be accepted without parts, and WP-S7.03 also waits for WP-S1.21; generic data needs no licence review beyond CC0 |
| D6 | **A new project reaches the starter parts with no setup.** The picker's part rows and the Parts row list the system library's parts beside the embedded copies (embedded first); choosing a library part embeds it by reference (WP-S1.19 part 3's `library.embed` reference form) in the same transaction as the placement or assignment | No prelude line; one undo step |
| D7 | **Every starter symbol is placeable, tested.** A headless script per starter symbol places it in a new project from the picker with a part where D5 gives one, and for each auxiliary symbol joins a device both ways (D2, D3); the test enumerates the starter set so that a symbol added later fails it until it has a path | The owner's "basic elements" as an executable list |
| D8 | **The briefs are the acceptance.** `brief-relays-valves`, `brief-valve-switch` and `brief-plc-io` pass `cargo xtask blind check-brief` with no `library.embed` line; `brief-relays-valves`' text says the power supply's part is among the starter parts instead of "already in the project's parts" | The play-tests need completable briefs (ID-808 (7)) |

*As built (cycle 37, lane F37, decision log ID-859; f93f4bca).* **D3:** the core command is `rep.join_device {rep, device}`, not FT-628's `function.move_to_device`, because D3 re-targets a placed representation. It fills a slot of the representation's FnDef first (the lowest template entry, then the lowest ID), re-pointing connections, potential definitions, bonds, channels and the pin map, else it keeps the Function as a new auxiliary function (role aux, no template entry); the wires are kept and the emptied source device is removed in the same transaction; a connected CP missing on the target is refused at `/device`. The Tag row is editable when one device is selected: an existing device's tag proposes "Join -K1" (`props.accept_tag_proposal`), and a text that names no joinable device writes `tag.frozen`. **D6:** since FT-673, `rep.exchange_symbol`, `ip.place` and `connection.draw_to_ip` carry `copies` as `device.place` and `box.black` do; only the wizard commits a `library.embed` of its own.

## 5. Parts

| Part | Scope | Size | Routing | After |
|---|---|---|---|---|
| 1 | D1 to D4: the Join chooser in the Place tool, the typed-tag proposal in Properties, `place.pick` by tag; tests per branch (slot filled first, new auxiliary function, refused FnDef, the proposal's button, undo as one step, the text forms and their refusals); doc 06 §3.3 and doc 16 as built; `brief-relays-valves` made runnable with its prelude's part line still in place (FT-599) | 1.1 pw | Opus at `medium` (viewmodel and api; no core change) | — |
| 2 | D5, D6: the generic parts as content with their SPDX lines, the system library's reach from the picker and the Parts row, embedding by reference on choose | 1.0 pw | Opus at `medium` | WP-S1.19 parts 2 and 3 |
| 3 | D7, D8 for `brief-relays-valves` and `brief-valve-switch`: the per-symbol script test, the preludes without embed lines (FT-592), `check-brief` on both | 0.6 pw | Opus at `low` | Part 2 |
| 4 | `brief-plc-io` written with its acceptance list and reference solution (FT-593), on the generic PLC cards | 0.3 pw | Opus at `low` | WP-S2.07 part 3; WP-S1.21's channel assignment |

**Size.** 3.0 pw against a PERT of 3.1 pw (O 2 · L 3 · P 4.5), no reserve. **De-scope rule:** a part projected more
than 20 % over leaves, in this order: the slot label in the chooser's kind column (D2), the toast's function name
(D3), the MCB 3P and the cylinder's valve 3/2 (D5).

## 6. Files a lane may not decide

The file format and `device.place`'s targets are fixed (a need to change either stops the lane); a new keymap chord
beyond D2's one letter; the part numbers' form; any golden change.

## 7. Documents to update at the design passes

Doc 06 §3.3 (both join paths as built), doc 16 §4.4 (the Join key), doc 16a §6.22 (the chooser's second use), doc 07
§2.1 (the generic starter parts in `ew-starter-parts`), the visual specification §6.3 (the briefs' runnable state),
02a's note on FR-DEV-038, the traceability.

## Sources

- [02a](../../design/02a-requirements-core.md) FR-DEV-005, FR-DEV-038; [06](../../design/06-schematic-editor.md) §3.3;
  [16a](../../design/16a-design-language.md) §6.22; [ADR-0031](../../adr/0031-design-language-and-task-ordered-command-bar.md).
- [WP-S2.04 specification](WP-S2.04-placement-wiring-drawing-tools.md) §7.2; [WP-S1.19 specification](WP-S1.19-parts-libraries-and-macros.md) §7, §8;
  [visual play-test specification](WP-S6.13-visual-playtest-runs.md) §6.
- Decision log ID-770, ID-780, ID-807, ID-808, ID-812.
