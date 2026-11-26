# Schematic editor: commands, normalisers, placing, moving and connecting

A chapter of [Schematic editor](06-schematic-editor.md), whose table says where each section lives.

---

## 3. Behaviour and rules

---

### 3.1 Editor commands

Every command has a stable dotted name and a JSON Schema (FR-EXT-015). `Cost::Bulk` commands always run as preview
jobs (ARCH §5.1). The Bulk threshold of `edit.delete` counts the records affected, which is the size of the delete
plan, not the number of selected targets.

The commands are generated from their registry, and so is their reference: `ew commands` lists them with their
exposure, cost and stability, and the user guide's command reference (`docs/book/src/reference/commands/`, written by
`cargo xtask docs-gen` from the parameter schemas) has one page per command with its parameters and the description
that states what it does and what it refuses. What holds for all of them:

- A placement whose `rep_type` the page type does not allow is refused (§2.2, `allowed_rep_types`), and an
  autoconnect hint whose target vanished is dropped and reported in the outcome's notices.
- A new optional parameter member is left out when absent, so existing command logs serialise unchanged.
- A new project is made without a supply: a supply guessed at its creation would print on its cover before the
  person chose it. A schematic project without one takes the default supply with the first member that
  `project.settings.set` sets (`supply.<path>`), so Properties can give it one; a basic or macro project has none.
- A part change of a PLC module in the window goes through `plc.set_module_part`, which removes the channels the new
  part does not have; `fields.set` on `parts` never removes a channel ([10](10-plc-and-bus.md) §3.3).

---

### 3.2 Normalisers that run behind the editor

Normalisers add ops to the same transaction, at most three passes (ARCH §5.2). ARCH §5.2 lists the normaliser
families and names this table as their **registry**: a new normaliser is added here, with its owner, before it
ships. The order is the order within one pass; a normaliser whose input another one creates in the same pass sees
it in the next pass. All are deterministic and break ties by ID. "At most three passes" means three productive passes
plus one confirming pass that changes nothing; a fourth pass that still changes something is non-convergence,
reported as `CmdError::Internal`, and the command is refused. Each normaliser converges within
one run for its own cascades, so the passes only carry effects from one normaliser to another: junction clean-up,
for example, works through a worklist, so a dissolve that leaves another junction with two connections is handled in
the same run.

`ew-commands` runs the rows it has built in the table's order (`ORDER`, asserted by a test; the head of its
`normalise` module lists them), one pass after another until a pass changes nothing. Row 2 records nothing, because
CPs are derived and only overrides persist. Row 4 numbers within the scope
(page of the main representation, letter) and skips devices with a terminal aspect, a frozen tag or a scheme other
than the page-based default. Row 7 splits by an exact integer point-on-route test; W′ keeps its ID and
number and W″ is new and `Unassigned`. Row 8 keeps, when a junction dissolves, the member that carries an assigned
wire number (rows 8 and 11, FR-NUM-023); if both members carry different assigned numbers the command is rejected
rather than losing one. Row 12 leaves every new connection but a fluid line `Unassigned`: no wire-numbering scheme
runs online. Rows 3, 13 and 14 (N1, N2, N3) are [10](10-plc-and-bus.md) §3.3's; row 3 removes no surplus channel,
because FR-PLC-053 removes them only through the preview of the Bulk command `plc.set_module_part` (§3.1). Row 5
(N-FL-2) is not a normaliser: its counter rule is applied inside `valve.add` and `assembly.attach`, and its push
insert is not built. Normaliser notices are merged once into the command's outcome.

**Explicit counter edits** (FR-NUM-029). Row 4 renumbers only collisions that the edit itself
creates by placing a new device, moving a main representation or changing a page. A `fields.set` that sets a tag's
letter or counter to a value that collides on the page is rejected in the strict style of ADR-0009, with the next
free counter proposed, and is never silently replaced. Duplicates that already exist in a lenient project stay
findings.

The refusal of a colliding letter edit offers the
proposal: the window shows "<refusal>. Use <tag> to accept it." with a "Use <tag>" button under the error, and the hidden
parameterless action `props.accept_tag_proposal` accepts it in one step (the counter cleared and the letter set in one
`fields.set_many`, so online numbering assigns the counter and the tag stays unfrozen). A script or blind-run failure
message reads "<refusal>. Use <tag> to accept it: props.accept_tag_proposal takes it." The proposal is computed from the
preview's tag diff, never parsed from the refusal's text: the tag is the device's printed tag after the proposal and the
toast's counter is the device's `tag.counter` in the preview's after-state. It is cleared when the selection changes,
and accepted only while its refusal is still under the field on the same revision; otherwise the action is refused with
`reason-no-tag-proposal`.

| # | Normaliser | Effect | Owner |
|---|---|---|---|
| 1 | **Valve functions from the part** (N-FL-1) | Creates or removes a valve's valve and coil functions per the part's `solenoids` | [11](11-fluid-power.md) §4.2 |
| 2 | **CP instantiation** | No normaliser: a function's CPs are derived from its FnDef or part pattern (FR-DEV-002, `function_cps`) and only overrides persist, which the commands write themselves | this document; [07](07-symbols-parts-macros.md) §4.2 |
| 3 | **Channel instantiation** (N1) | Creates a module's channels with v5 IDs from the part's `channels` | [10](10-plc-and-bus.md) §3.3 |
| 4 | **Online numbering** | New devices get the next free counter in their scope (FR-NUM-007). Moving a main representation that makes a page-based tag collide assigns the next free counter in the same transaction (FR-NUM-029): -101F1 dragged to folio 102 becomes -102F3 | this document |
| 5 | **Valve position and child counter** (N-FL-2) | Lowest free or requested position in the module; the child counter follows the position | [11](11-fluid-power.md) §4.2 |
| 6 | **Autoconnect hint conversion** | Hints accepted in the ghost become `Connection` ops after re-validation against the candidate snapshot (ADR-0007); a hint whose target vanished is dropped and reported in the toast | this document |
| 7 | **Connection split** | A symbol dropped onto a wire splits it (FR-CON-008, §5.3) | this document |
| 8 | **Junction clean-up** | A junction left with two connections dissolves into one connection; a junction left with one connection is deleted together with it, and the preview lists both (invariant CI-04) | this document; [08](08-connections-terminals-cables.md) §3.1 |
| 9 | **Conductor assignment** | Wires crossed by a cable definition line become cores | [08](08-connections-terminals-cables.md) §7.2 |
| 10 | **Clamp pick** | A wire ending on a terminal without a clamp gets the first free clamp of the drawn side | [08](08-connections-terminals-cables.md) §6.3 |
| 11 | **Number transfer** | When an edit changes a physical wire's carrier connection, moves its `NumberSpec`, its `WireOverrides.ends` and `WireOverrides.length` to the new carrier and rewrites records keyed by the old wire ID (`Bond.via`) | [08](08-connections-terminals-cables.md) §5.1; end overrides [13](13-manufacturing-outputs.md) §2.3 |
| 12 | **Online wire numbering** | New wires get the next number of the active wire-numbering scheme when online numbering is on, otherwise they stay `Unassigned` and print "?" (FR-NUM-019, FR-NUM-021) | [08](08-connections-terminals-cables.md) §5.2 |
| 13 | **Solenoid binding** (N2 = N-FL-3) | Binds each declared solenoid's coil to the channel the part's position map assigns, if that channel is unbound and no channel binds the coil; an existing binding wins, and only the valves and channels of the transaction are considered (FR-PLC-051) | [10](10-plc-and-bus.md) §3.3 |
| 14 | **Module pin binding** (N3) | Binds a new physical channel to the module pin whose designation the part's channel → pin map gives | [10](10-plc-and-bus.md) §3.3 |
| 14a | **Bus ports** (N5) | On a new device or a changed bus part, or a bus-port function (FnDef contact kind `bus_port`) added to or moved onto a device with a bus part: the part's ports that no function of the device carries (by interface and label) go in list order to its port-less bus-port functions by ID, address `tbd` (`not_applicable` when passive), the part's termination, no nets. Never rewrites or removes a port; a disagreement gives a notice; a part port without a function seeds nothing. Duplicates are dropped first and unknown buses or roles filtered after, so a newer-format first port drops its whole (interface, label) pair. Runs after `module_pin_binding` | [10](10-plc-and-bus.md) §2.6 |
| 14b | **Bus nets** (N6) | On a new or changed `Bus` line between CPs of two functions with bus ports of one bus system: the ports of its segment (reached over bus lines) join one logical net, the one net they name (when it exists), else, with two ports or more, a new `LogicalNet` named for the bus system (`PROFIBUS DP`, `PROFIBUS DP 2` …) whose master is the CP of the one `master` or `controller` port. A passive port passes the net through and takes none. Never changes a net a port names; ports in different nets stay (EW-BUS-010). Runs after `bus_ports` | [10](10-plc-and-bus.md) §2.7 |
| 15 | **Logical-net inheritance** (N4) | A bus connection to a port without a logical net assigns the net of the other port | [10](10-plc-and-bus.md) §3.3 |
| 16 | **Drives link** (N-FL-4) | The first tube from a valve working port to a consumer port of another device adds `DeviceLink{Drives}` | [11](11-fluid-power.md) §4.2 |
| 17 | **Tube counter** (N-FL-5) | A new tube connection gets its counter when a line-tag scheme is active | [11](11-fluid-power.md) §4.2 |
| 18 | **Interruption-point form** (`ip_form`) | An interruption point drawn with one of EW's two arrow forms whose direction the transaction changed takes the other form, the one of its new direction, with the default orientation; a project that does not hold that form refuses the change: "the project does not hold the incoming arrow of symbol Interruption point: place one interruption point of that direction first" | this document §3.5 |
| 18a | **Texts of a symbol** (`bound_texts`) | A free text of a symbol (`of`) that the transaction did not change takes the displacement and the page of its symbol when the transaction moved it; a text the command moved itself is not moved twice | this document §3.8 |

**Routing is one pure function, `ew_drafting::route` ([06a](06a-drafting.md) §3), reached through `ew_api::routing`, and
the tools call it; the commands do not.** The Wire tool computes its ghost with it and passes the result as the `route` of
`connection.draw`, which **stores the route it is given and stays unchanged**: an empty route stays route-less
(*unrouted*, below). `rep.move` moves the route's vertices by `geometry::follow_end` and takes the
routes the Select tool re-solved (§5.4). A script therefore gets the mouse's route by calling the same function and passing the result, not by a
command that routes. **Normaliser 7 (connection split) works on routes**, so it splits a connection that has a stored
route, which every connection drawn with the Wire tool has; a route-less connection has no route to test, and the
route editor's first edit stores its drawn polyline.

[ADR-0033](../adr/0033-schematic-routing-model.md) governs routing. A connection is drawn along its stored route, the full
vertex list with both ends, and a page connection without a route is *unrouted*:
drawn as a dashed right-angled line (below), not split by normaliser 7, not read by EW-TERM-006, reported by
EW-CORE-073. **The unrouted line is never a diagonal** (a diagonal reads as a route the router drew askew): ends on one horizontal or vertical line are joined straight, else by one corner
made of the ends' own coordinates, the first leg along the first end's direction, else the last leg along the second
end's, else horizontal first. It takes no notice of what lies under it and is no route: the page draws it dashed in
the thinnest pen, and the Wire tool's ghost and the router's own result for a request without a route are the same
line (`ew_api::routing::unrouted_line`). The
commands still never route: the router runs in the tools and in Route, which stores its plan with `layout.apply`
(06a §2.6); autoconnect hints store their straight run; `rep.move` takes the routes the Select tool re-solved. Row 18,
*interruption-point form*, gives an interruption point drawn with EW's IP symbol the arrow form of its direction.

The commands that store a route are `connection.draw`,
`connection.draw_many`, `connection.draw_to_ip`, `connection.route`, `layout.apply` (06a §2.6) and `rep.move` with its
optional `routes`, each checked by
`require_route` (on the grid, the first vertex at end `a`'s point and the last at end `b`'s, a junction end included);
`device.place`'s hints and `rep.move`'s optional `connect` hints store the straight run between their two ends, and a
hint whose ends are not aligned stays unrouted. `rep.move`'s `connect` hint names its connection point by function,
`{rep, cp: {function, idx}, to}`, not by symbol CP: the window knows a placed representation's points by function,
and a black box draws points no symbol CP indexes. Normaliser 7 splits routed connections only; row 18 runs
after row 16, before the revision normaliser of [15](15-checks-revisions-collaboration.md) §5.3.

Row 1 is N-FL-1, `valve_functions`, first in `ORDER`, and row 16 N-FL-4, `drives_link`, after `bus_ports`
(doc 11 §4.2); row 1
neither replaces nor removes a valve function on a part change (notice T21).

---

### 3.3 Placing symbols and assigning devices

`device.place`'s target decides which device the new representation belongs to. Its forms, each with its members,
are the enum `PlaceTarget` of `crates/ew-commands/src/commands/device.rs`: a new device, another representation of
an existing function (FR-DEV-003), a new function of an existing device (FR-DEV-038: the person typed the tag of an
existing device), a new device made from a part (its template's functions; the chosen one is placed, FR-DEV-017)
and a new terminal of a strip. A new device's letter is given; taking it from the FnDef category through the
identifier set (FR-NUM-004) is specified; not built. An accepted autoconnect hint is a `ConnectHint` (same file):
the symbol CP and its other end, a facing free CP, a junction, or a point on an existing wire that the wire is
split at (FR-CON-008).

Rules:
- **From the symbol library** (search by name, category, IEC 60617 reference, FR-SYM-021): a new device whose letter
  comes from the FnDef category; the ghost label shows the tag the fork predicts ("-102F3") before the click.
- **From a part** (IDEA-016, FR-UI-017): the part's function template creates all functions; the one matching the
  chosen symbol is placed and the rest appear as unplaced entries under the new device. Part texts (manufacturer,
  number, rating) appear through text slots with no typing (FR-DEV-016).
- **Typing a tag** in the tag field after placement: the text is parsed by `ew-rds`. If it names an existing device,
  the editor offers to join it as an auxiliary function, or as another representation of an existing function with
  the same FnDef and CP designations. If it parses under the device's scheme, it sets letter and counter. Otherwise
  it becomes a frozen tag after confirmation ("S51 does not match `{page}{letter}{counter}`; keep as frozen tag?"),
  reported at info (FR-NUM-010, FR-CHK-025).
- **Displayed tag**: the canvas prints the displayed tag, which suppresses structure identifiers equal to the
  page's (FR-NUM-002); the property grid shows the full tag `=P01+J-E-2416-102F1` beside it, read-only.
- **Representation type mismatch**: placing an overview symbol on a multi-line page is refused with the list of
  allowed types (§2.2).
- **Black box inside the drop point**: the function joins the box's device (§2.7); a valve inside a valve terminal's
  box is a new device that joins the terminal at its next free position ([11](11-fluid-power.md) §4.3).

**Joining an existing device.** Both paths are `device.place`: with `existing_function` while the device has an unplaced
function of the symbol's FnDef (a template slot, filled first), else with `join_device {device, role: aux}`; one undo
step per placement. **Before placing (D2):** while the armed symbol can be an auxiliary function, the Place
tool's HUD offers "J Join device…"; J opens the Join chooser ([16a](16a-design-language.md) §6.22): "New device", then
the slots ("slot 13-14"), then the devices whose family takes auxiliary functions, in natural tag order with
type-ahead. Enter arms the target (ghost label `join -K1`), which stays sticky; Escape keeps it; "New device" restores
the default. The text interface takes `place.pick device=<tag>` and `function=<tag>[:<slot>]`. **After placing (D3):**
`rep.join_device` moves a placed symbol's one function to another device, its wires kept, when its own device has that
function alone, no part and no other symbol. J on one selected symbol, or Join device… in the page's right-click menu,
opens the Join chooser of the devices it can join, one row per device (its first slot, else "auxiliary"); typing an
existing device's tag into the Tag row proposes the same join. **A second symbol of a device's own:** a
symbol of another kind than a contact (a lamp, a supply drawn once for its input and once for its outputs, an element
brought in from another program) joins any device but a terminal or a strip as an auxiliary function, by J, by
`place.pick device=<tag>` and by `rep.join_device` (the Tag row proposes a contact's join only, as the row means
another thing for a valve's solenoid or a taken tag); a contact keeps D2's family rule, and a terminal, an
interruption point and a graphic join no device. A library symbol's address `library:<library>:<item>` names it before
and after its first placement. **A part's function that a device of the part has
unplaced fills that device**: the browser's function row names it ("join -K1"), the Place tool arms that
join with the part's new device as its "New device", and once the device has no more of that function the next
placement is a new device of the part. **From a part:** the picker and the Parts row list the generic starter parts of a system library after the
project's embedded copies ([07](07-symbols-parts-macros.md) §2.1); a placement from one embeds it with its closure in a
`library.embed` before the placement, two undo steps. Every starter symbol has a tested path to a usable
device (`crates/ew-viewmodel/tests/starter_symbols.rs`).

**Device from part lists the
project's own part copies and the libraries' parts in one order**, by maker and part number, so a library part keeps
its row once it is embedded (its Library column then says "In this project"): the rows do not move between two
placements. The tree has All, then In this project once the project holds a part, then the heading Library with the trades and their groups under it, so that the library's categories do not
read as the project's; and
its empty state names Edit ▸ Choose part…. **A part without a function template** is placed through the symbols it
can take: `device.place` with `from_part` accepts such a part at template entry 0 and gives the new device the part
and one main function of the chosen symbol's function definition. **A black-box entry with a symbol of its own**:
a symbol drawn for the entry's own function definition, such as a power supply drawn for the starter's
`ew.fn.psu-ac-dc`, is placed by the Place tool like any symbol, and the entry's function is then a normal one; an
entry on the generic `ew.fn.blackbox`, or one no own symbol draws, stays the Black box tool's; placed from the Place
tool, its hint says that the part has no symbol of its own and is drawn as a box, and a toast says it as the
tool changes, naming the part, or the device whose box is placed again ("EW-GEN-PSU-1 has no symbol of its own and is
drawn as a box, by the Black box tool: …"). `place.pick` names an
installed library's symbol by its key as well as by its address, before its first placement and after it.

---

### 3.4 Move, rotate and delete

- **Moves never change connectivity** (FR-CON-004). Route vertices adjacent to moved CPs are re-solved (§5.4); other
  vertices stay. Live potential highlighting during a drag uses page-local nets only (NFR-PERF-05).
- **A turned or mirrored symbol stays where it stands**: `rep.transform` turns it about the grid point nearest
  the middle of its body (its graphics' bounding box, a black box's outline), not about its insertion point, so a
  turned cylinder does not land on its neighbour. Four quarter turns bring it back exactly; its wires and tubes
  follow its connection points, and each leaves its connection point along the point's new way out (a step out, then
  a turn to where it ran), never across the symbol's side. A text the person placed by hand keeps its place on the
  page beside the symbol and its own alignment, level as every text; a turn and its reverse give its position back.
- **Cross-page move** (Move to page… for the selected symbols, on the Page tab beside Copy pages… and in the
  right-click menu of selected symbols, which asks for the page: the other schematic pages and To a new page; a PLC
  card's Move to page; `rep.move` with a target page) keeps
  identities and, because it is a move, connectivity (FR-CON-004): each wire whose other end stays behind is
  kept as a **pair of interruption points**, and the window lists the pairs in a preview before anything changes
  (FR-CON-043); Apply moves, one undo step. Move to page… is greyed until symbols are selected ("Select the
  symbols to move"); selected texts and boxes stay on the page, which the preview says first, counted. `rep.move`
  makes the pairs with `keep` (the two forms of the interruption-point symbol and the library documents they need,
  embedded only when the move cuts a wire); without it a wire whose other end stays behind is refused, the wire
  named.
  - **The name** of a pair is the wire's potential's (its name, a fallback name such as `-K1:13` included; `IP` for
    a wire the potentials do not know), the next free name of its series (`24VDC-1`, §3.5) where an interruption
    point of the project, or another pair of the same move, has it already: the rule of a new interruption point.
  - **The source point** stands on the page left, where the wire left it: on the free grid point of the wire's old
    path nearest to its moved end (the moved pin's old place, unless a pin or a junction stands there), the wire
    shortened to it. **The target point** stands on the new page at the moved pin, joined to it by a wire of length
    zero that takes the old wire's kind and its values.
  - **A wire with one junction:** the junction stays with the larger part. It stays where it is when at least as
    many of its wires end at symbols that stay as at moved ones, and each wire from it to a moved pin becomes a pair
    as above; else it moves to the new page with the moved symbols and the wires between them, and each wire from it
    to a symbol that stays becomes a pair, the target point at the junction.
  - **Refused**, in words and with nothing written: a wire whose junction is joined to another junction (two
    junctions or more: move such a net by hand), and a wire whose old path has no free grid point left.
  Page-less connections (bridges) stay as they are. **Cut and paste** is a move of the devices and follows §3.6: the
  devices the cut left unplaced come back as themselves, and the wires the cut took to outside the selection are not
  pasted.
- **Structure follows the page** unless the device has an explicit slot; moving the only representation of a device
  to a page of another location asks whether to keep the old location as an explicit slot (FR-PRJ-019).
- **Deleting a representation** leaves its function unplaced; deleting a device follows the delete policies and
  previews every function, representation and connection removed (FR-DEV-030). Deleting a page lists its IPs and
  devices left unplaced (FR-PAG-025).
- **Delete on a page takes the wires drawn to the deleted symbols**: Delete (`edit.delete_selection`, and Cut through it) also deletes, in the same step, every
  connection on the page that ends at a connection point the deleted symbols draw, so no connection stays whose end's
  symbol went; the toast counts them. A terminal it leaves drawn by no symbol stays in its strip and the lists (a
  terminal is never deleted with its symbol, below); the toast says how many, with Remove them from the project (Remove it from the project for one) beside
  Undo, one `edit.delete` of those terminals and of a strip it leaves with no terminal and no segment. The Devices
  tree's menu of a device or a strip has Delete (`navigator.delete`). Cut's toast says what it took instead ("Cut 12
  terminals. 14 wires that ended at them are removed with them; a paste keeps the wires between the cut items only."),
  with Undo and no Remove (its paste brings the terminals back).
- **Delete removes an orphaned device.** Deleting a device's last representation (Delete, and Cut through the same
  path) deletes the device in the same `edit.delete` step, so one undo restores the device, its part use and the
  representation. "Last" means every representation of every function of the device is among the deleted ones,
  panel-front and contact-image representations included (a function no representation draws, such as a part's other
  contacts, keeps nothing), so a device that keeps a panel-front representation is kept and EW-CORE-047 reports it.
  The device is also kept when it has another role (a function the deletion leaves drawn, a terminal or strip role,
  child devices, or a cable, assembly or PLC aspect) or when one of its functions carries other intent: a CP of the
  device holds a potential definition, a bond, or a connection that is page-less or lies on another page than the
  deleted representations. A kept device stays unplaced and is left to EW-CORE-047
  ([15](15-checks-revisions-collaboration.md) §4.2).
- **A PLC card without settings is no other role.** A module whose `plc_module` aspect holds no setting (what the
  aspect normaliser N0 gives a card's device: positional slot, automatic starts, no part use, not integrated) goes
  with its last symbol, and its channels with it, so deleting a card to draw it again leaves no `-?A1`. A station, or
  a module with a set slot, start, part use or integrated I/O, is kept. When a run gives a device `plc_station` and
  the project then holds exactly one station that is no valve terminal, N0 attaches every module that waits for a
  station (not integrated, no station above it) to it, in the order of the modules' anchor cells, in the same
  transaction; with several stations EW-PLC-008 asks the user.

---

### 3.5 Connections, junctions, interruption points and potentials

- **Explicit connections.** A connection joins two endpoints: a CP or a junction (FR-CON-001). Several connections on
  one CP are a daisy chain whose order is preserved (FR-CON-006). Wires of one CP may share their run out of it and
  part with a 45° bevel, without a dot: a potential wired from pin to pin is then a rail with drops
  ([06a](06a-drafting.md) §3.4, D-20); a symbol is not dropped on a shared run.
- **Junctions.** Ending a wire on the interior of another connection's segment creates a `Junction` and splits that
  connection. Style defaults to Bevel; the Wire tool's dot key (`.`) asks for a Dot (owner, drafting step 2,
  [06a](06a-drafting.md) §7.4). Wiring defaults to Star for Dot and Chain for Bevel ([03](03-domain-model.md)
  OQ7). `sequence` stores connection IDs; the editor shows each member by its far endpoint, e.g. (+#1, -102F1:1,
  +#2) for J3. The initial sequence lists the two parts of the split connection ordered by flow distance from the
  potential's source ([08](08-connections-terminals-cables.md) §3.2, §3.6), with reading order (row, then column of
  the far endpoints' anchor cells) as the tie-break, so the feeding device comes first, and appends the new branch
  (PQ-27). The junction's property page shows its sequence as a
  reorderable list together with the
  **physical wires it produces** ("W13 F55:2 → E55:L; W14 F55:2 → E56:X2"), so the user edits the wiring order the
  shop will cut, not a graphic. A junction whose feed cannot be determined is reported by EW-POT-011
  ([15](15-checks-revisions-collaboration.md) §4.4).
- **Interruption points.** An IP is a `Function{kind: Ip}` with one CP. It is placed on its own and wired afterwards,
  or created at the free end of a wire being routed (`P`, §4.4). The name field proposes the potential's name,
  then the next name in a numbered series ("24VDC-2" after "24VDC-1"). **The direction is one state of the tool**:
  the tool options card's Source and Target show it, and the next click places it wherever it lands. It starts as
  Source, as a new pair begins with its source, unless the name typed for the point has its source already: then
  Target, as a name has one source; a name typed while the tool is armed starts it by the same rule, unless the person
  chose a direction (`D`, the card), which holds until the person chooses another, for every next point and name. A
  Target stays after a target whose name has no source yet, and the bar says "next: one more target of 24V, which has
  no source yet (D makes this point its source)"; a Source turns to Target after the source of a typed name, as the
  name has its source then, and the card shows it. A star source places a source and an external reference a target,
  the direction kept for after them. EW-CORE-051 names where
  the mark stands and what it lacks: a target its one source, where the potential comes from; a source its targets,
  where it goes on.
  Pairing (star or chain, scope) follows the project's `ip_pairing` setting with per-name overrides (FR-XRF-007,
  FR-XRF-010). "Continue on page…" creates the partner on a chosen page at the opposite edge in the same row, with
  the direction reversed (FR-XRF-014). "Arrange" sorts a page's IP stack by name or potential (FR-XRF-013, D-39).
  EW's interruption-point symbol comes in two
  forms, both with the arrow pointing right and its tip at the origin: the source form (`ew.gen.ip` 0.2.0), the wire
  on the tail and the CP one grid step left of the tip, leaving left; the target form (`ew.gen.ip-target` 0.1.0), the
  wire on the tip and the CP at the tip, leaving right; each with its `@qet10` twin. The name sits above the arrow and
  the cross-reference below it: right-aligned at the tip on a source, left-aligned at the tail on a target, so that it
  runs into the sheet. Where the point's own wire runs through that place (it leaves the tail and turns down under
  the arrow), the cross-reference takes the free side: in line with the arrow beyond its tip, else before its tail,
  else above the name (`ew_scene::page`, "Slot text collisions" rule 5; the tool's footprint keeps the place below,
  since the wire is not drawn when the point is placed). A point drawn with these forms is never turned: its form is
  its direction's (an undirected point takes either), D swaps the form with the direction (source and target, to
  and fro; undirected is a choice of a placed point's Direction in Properties), and the rotate and mirror
  keys are not offered. The tool takes the name first when it is chosen from the bar or a menu (the name field has
  the keys, its name selected; chosen by its key with the pointer on the sheet, the keys stay with the tool), a typed
  name stays until it is changed, so that after the pair each click places one more target of the same source,
  the pointer as near a pin as the Wire tool catches one puts the point's CP on it, in either form (a source's CP is at its tail), and the Supply tool's marks the same way (the first mark that comes so near; each mark whose CP then lies on a symbol's pin or on a drawn box's left side joins it, and the ghost and the HUD name each pin a mark joins), but on a drawn box's pin the arrow never lies along or in the box: on the box's lower side the point stands one grid step below the pin and on its upper side two steps above it (its cross-reference under the arrow needs the room), joined to the pin by a straight wire (`connection.draw_to_ip` with that route; the ghost shows it there), on its left side a target and on its right side a source stand on the pin, and a source on the left side or a target on the right side, whose arrow would point into the box, is refused in words (Alt places it free), a drop on a pin is not refused for room, Alt places a point unjoined and a point left within 6 mm of a pin without a wire says so on placing (EW-POT-009 then offers to join the two), a free point too close to another is refused with the grid steps and millimetres the two must stand apart, the HUD says before the click when the next point is the other half of a name ("next: where 24V arrives (its
  source is drawn)"), and a point whose arrow, name or
  cross-reference would print over another point's is refused with the reason, Alt placing it anyway: the name
  stands above the arrow and the cross-reference below, each as wide as its text, so a point in another's footprint
  prints over it (the footprint is reckoned by rule of thumb, 1.4 mm a character, and the cross-reference's room is
  kept before the point has a partner). `ip.place` refuses a turned form ("an interruption point's arrow points the way its signal
  flows: change its direction instead") and the other direction's form ("a source interruption point is drawn with
  the outgoing arrow, a target with the incoming arrow"); `rep.transform` refuses the first; `ip.continue` gives the
  partner the other form and refuses in words when the project lacks it; a direction change swaps the form by row 18.
  Making a point the source (`fields.set` of its direction, the Direction row of Properties) makes the other sources
  of its name and scope targets in the same step, unless the name has a star source: one source per name, without a
  second edit. While the Wire tool routes, P picks the form by the direction at the end point. Imported points, company symbols
  and the external arrow keep a free orientation.
- **External connection points** are IPs with `external = Some(ExternalRef)`. They never pair and print the external
  target ("+J-E-206 / JE-N1000-03") in the xref slot (FR-XRF-012).
- **What an external point's reference holds.** A drawing set whose signals come from another
  cabinet names each mark at the head of a current path for its signal ("Valve 1 open") and prints under it where the
  signal comes from: often an address alone ("Q4.1"), sometimes nothing, because the far end is not known yet.
  - *Fields.* `ExternalRef.location` and `ExternalRef.device` are optional, as `project` and `connection` were: a
    reference holds what is known of the far end, the device or an address in `device`. One that holds neither is a
    far end not known. A project of 0.3.0 reads unchanged. 0.3.0 takes both as required: it opens a project with a
    reference that holds no location or no device for editing, sets that reference's line aside (EW-CORE-007),
    reports the points that name it and writes the line back unchanged on save (ADR-0011 amendment 8).
  - *What prints* (the point's xref slot, a structure box's caption, a bus partner): the parts the reference holds,
    the location and the device joined by " / " ("+J-E-206 / JE-N1000-03"), the device or address alone ("Q4.1"),
    the location alone ("+MCC2"), and nothing for a far end not known; another project's name in front as before. The
    connection prints in the wire list's end, as before. The person chooses what prints by what the reference holds.
  - *Making one.* Supply comes in here… (`ip.supply_in`) keeps its two fields, the location and the device there; a
    field left empty is not known, so the device alone is an address and two empty fields a far end not known (a
    script that leaves a field out takes the proposal). The location is taken with or without its sign
    (`+UP` and `UP` are one node). `external_ref.create` takes any of them or none.
    The Interruption tool's external field names a reference by its printed device (and connection); one without a
    device is named by its ID.
  - *What the checks ask.* EW-CORE-034 still asks that the wires of an external point be a cable's cores; its finding
    names the place by the reference's location, else by what the reference prints ("Q4.1 → -1K2"), else says the
    connection comes from a place not known. EW-CORE-051 never asks to pair an external point. Nothing asks for a
    location or a device.
  - *Not built:* a reference that holds a location but prints only its device (the person leaves the location out of
    it instead); the stale check against the other project (FR-PRJ-022, 1.0).
- **Potential definition points** name and type a potential at an endpoint, a CP or a junction (FR-CON-010); they
  may be drawn as a flag symbol or kept undrawn and edited in the potential navigator, which lists each potential's
  CPs, devices, pages and IP aliases (FR-CON-012, FR-CON-027).
- **Connection definition points and wire markers.** The marker tool is drawn across one or more parallel
  connections. It sets the chosen overrides (cross-section, colour) on each crossed connection and creates one
  `WireMarker` page item that positions the group (FR-CON-023). The marker's text is derived: the cross-section at
  the left, one colour code per conductor ("2.5 BK BU"). The project's marker mode decides which connections show
  markers (all, deviations, explicit overrides, none; FR-CON-024). A wire whose Wire marker box is
  ticked (`overrides.show_marker`) always prints a marker, except in the mode none: with the mode absent (Per wire,
  a new project's) it prints the cross-section and colour the wire states itself, and what the wire is where it
  states neither; in a rule mode the tick adds the wire to those the rule selects. In modes that show markers for connections
  without a `WireMarker` item, the scene places them at a derived position. A group marker over several connections
  is one `PageItem::WireMarker{connections}`, whose list order is the printed order ([03](03-domain-model.md) §6,
  FR-CON-039). Colour fields accept only IEC 60757 codes and profile-declared company codes such as DBU
  (FR-CON-019); typing "BR" offers "BN" (D-17).
- **Wire numbers on the canvas** are derived texts placed by the project's display rule (each segment, once per
  page or at both ends, FR-CON-026); unassigned connections show "?" (FR-NUM-021). The editor never stores the
  printed number, so a page renumber or a route edit cannot leave a stale number behind (FR-NUM-023).

