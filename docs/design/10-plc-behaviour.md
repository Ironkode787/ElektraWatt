# PLC, I/O and fieldbus: the PLC behaviour and rules

A chapter of [PLC, I/O and fieldbus](10-plc-and-bus.md), whose table says where each section lives.

---

## 3. Behaviour and rules

---

### 3.1 Address derivation

**A1. Module starts (positional packed allocation).**

```rust
fn module_starts(q: &Q, cfg: Id<PlcConfigProject>) -> Arc<StartMap> {
    let prof = q.profile(cfg);
    let order = q.get::<ModuleOrder>(cfg);                // stations by (order, id); modules by slot path
    // Pass 1: every Manual range is occupied before any Auto range is placed.
    let mut used = AreaIntervals::from_manual(&order, |m, r| m.start.manual(r.dir).map(|a| prof.range(a, r)));
    let mut cursor = prof.default_starts();               // per (direction, class): digital, analog
    let mut out = StartMap::default();
    for m in order.iter() {
        for r in q.part_io(m) {                           // { dir, bits, class } from the module's part
            let at = prof.align(cursor[r.key()], r.bits);
            let units = r.bits.div_ceil(prof.unit_bits);
            let (start, next) = match m.start.get(r.dir) {
                AddrSpec::Manual(a) => (Start::At(a), at + units), // kept; reserved in pass 1
                AddrSpec::Excluded  => (Start::None,  at + units),
                AddrSpec::Tbd       => (Start::Tbd,   at + units),
                AddrSpec::Auto      => {                  // first free range at or after the cursor, per area
                    let s = used.first_free(r.area(), at, units, &prof);
                    used.insert(r.area(), s, units);
                    (Start::At(prof.byte_address(r.dir, s)), s + units)
                }
            };
            out.insert(m.id, r.dir, start);
            cursor[r.key()] = next;                       // advances for every module, whatever its mode
        }
    }
    Arc::new(out)
}
```

- **Stable cursor.** The cursor advances by each module's own width, whatever its start mode, so a Manual start
  outside the Auto path never moves another module. This makes FR-PLC-012's acceptance hold: changing the start of
  -201U5 to Manual Q10 re-addresses only its 8 channels, and the engine statistics show one `channel_addresses`
  re-execution, while the other six are backdated (ARCH §6.2).
- **Auto never overlaps.** Auto ranges skip occupied ranges of the same area: Manual ranges (pass 1) and ranges
  already placed for another class. The digital and analog cursors of a profile whose default starts coincide
  (`start = { digital = 0, analog = 0 }`, unverified TIA defaults) therefore cannot collide, and an inserted module
  after a PLC import does not land on a PLC-owned start (§3.9). A Manual start set *inside* the Auto path (for
  example -201U5 = Q4) makes the following Auto modules skip it (-201U7 → Q5, -201U8 → Q6, -202U2 → Q7); the
  re-address preview lists every such move. EW-PLC-001 therefore fires only where a Manual value is involved: two
  overlapping Manual starts, or a Manual channel address inside another channel's range.
- **Inserting or widening** a module shifts the following Auto modules, which the preview also lists (§3.9).

**A2. Channel addresses.** `Auto` gives `start(module, dir) + bit_of(number)`, converted into byte and bit by the
profile's `unit_bits`. `Manual(a)` gives `a`. `Excluded` gives nothing, and `Tbd` prints "TBD". For a tag-based
profile, `channel_addresses` yields `ModuleMember { module, member }`, rendered as `Local:3:O.Data.1` from the
station, slot and the part's member pattern.

| Golden A module | Slot | Output start | Channels | Change "-201U5 output start = Manual Q10" |
|---|---|---|---|---|
| -201U3, -201U4 | 1, 2 | Q0, Q1 | Q0.0 … Q1.7 | unchanged |
| -201U5 | 3 | Q2 | Q2.0 … Q2.7 | Q10.0 … Q10.7 (8 changes; the 4 bound coils on folio 204 reprint) |
| -201U6 … -201U8, -202U2 | 4–7 | Q3 … Q6 | Q3.0 … Q6.7 | unchanged |

**Renumbering never re-addresses.** Tags, folios and page order are not inputs of A1 or A2. Golden B1 and B2 leave
every channel address unchanged (FR-PLC-055).

---

### 3.2 Channel states (A3)

The effective state comes from `channel_states(module)`:

1. An explicit `Channel.state` (Spare, Reserved, Deactivated) wins.
2. `address = Tbd` gives `Tbd`.
3. A bound channel takes the state of its target device (FR-DEV-026): used → `Used`, spare → `Spare`, reserved →
   `Reserved`, not fitted → `Deactivated`.
4. An unbound channel that the position map *claims* gives `Reserved`, and EW-FL-002 reports the unbound solenoid.
   The map claims channel c if it assigns c to solenoid s of position p, a valve child at p declares s, and s is
   unbound or bound to c.
5. Any other unbound channel gives the part's `unbound` state: `Deactivated` for valve-terminal modules, `Spare` by
   default.

On Golden A, every coil is bound "as drawn" to channel 2p + 1. Under H1, channel 2p is assigned to coil 14, but
that coil is bound elsewhere, so the channel is not claimed. Under H2, channel 2p is assigned to coil 12, which no
monostable 571334 declares. **Both hypotheses therefore give 8 used, 20 spare and 28 deactivated channels**.
The PLC overview lists all 56. New I/O-list blocks default to
`state != Deactivated` and list 28 rows (FR-PLC-019, FR-PLC-023).

**As built.** `channel_states` applies all five steps; step 4 reads
`solenoid_positions(module)`. "s is unbound" means that **no channel of any module** binds the coil CP, whatever that
channel's ID: the CP → channel index is read (the whole channel table, as N2 and `plc.bind_channel`
read it), so a valve moved to another
module and a duplicate channel never make a channel `Reserved`. Only an unbound channel reaches step 4, so the case
"bound to c" cannot arise there. On Golden A under H1 no channel is claimed and the counts stay 8 used, 20 spare and
28 deactivated (tested under H1 and H2, memoised against cold). EW-FL-002, which step 4 names, and EW-PLC-012 are not
built (FT-192). A channel bound to a pin of its own module (N3) has no target until tracing is built (§3.5), so step
3 does not apply to it: it is `Used` when a wire ends at the pin, else it takes step 4 or 5. Before this a card's 16
channels printed "used", with the card itself as their target, while EW-POT-009 called their pins unconnected.

---

### 3.3 Binding and normalisers

The normalisers run in the commit pipeline (ARCH §5.2); [06](06-schematic-editor.md) §3.2 holds the registry and
their order.

| Normaliser (ARCH §5.2) | Trigger | Effect |
|---|---|---|
| N1 channel instantiation | `PlcModule` added, or the module's part changed | Creates channels 0 … n−1 with v5 IDs from the part's `channels` and gives existing ones the part's channel kind. Surplus channels are removed only after a confirmation that lists their bindings, symbols and exchange records (FR-PLC-053): the preview of the Bulk command `plc.set_module_part` (§3.9); N1 itself never removes a channel and names the surplus in a notice |
| N2 solenoid binding | a valve child added to, moved within or re-parted on a module that has a position map; a coil function added to such a valve (by N-FL-1 of [11](11-fluid-power.md) after a part change); or a channel of that module created in the same transaction | Binds each declared solenoid's coil CP to the channel the map assigns, if that channel is unbound **and no channel binds the coil**; otherwise the coil stays as it is, with a notice (an unbound coil is reported by EW-FL-002, a binding that differs from the map by EW-PLC-012). An existing binding wins: an explicit binding in the command (QET import "as drawn", A7; a macro value set of type `ChannelBinding`, [07](07-symbols-parts-macros.md) §2.7), and the earlier binding of a moved valve's coil. Pairs whose valve and channel the transaction did not touch are never re-examined; a rebinding is the explicit `plc.bind_channel` (FR-PLC-051). This is the single implementation, in `ew-commands`; N-FL-3 in [11](11-fluid-power.md) §4.2 is a reference to it |
| N3 module pin binding | a channel created on a module with physical pins | `binding` = the module CP whose designation the part's channel → pin map (`pin_of`, §2.6) gives, on the module's main function, if no other channel binds it (FR-PLC-005); otherwise the channel stays unbound with a notice |
| N4 logical-net inheritance | a `Bus` connection joins a port without a logical net to one that has one | Assigns that net; conflicting nets are left as they are and reported (EW-BUS-010, FR-BUS-024) |

**Bindings point at CPs, never at copies.** For a physical channel, `binding` is the module pin. For an internal
channel (valve-terminal solenoid outputs, IO-Link process data), `binding` is the served coil's internal CP
(`CpDef.physical = false`) directly. No connection record exists, so wire lists, terminal diagrams and wire-length
calculation contain nothing to exclude, while the PLC overview, the I/O list and target tracking read the binding
(FR-PLC-006). `Internal` connections remain for other device-internal links between CPs of one part's functions
([08](08-connections-terminals-cables.md) §2.3).

**Drag-bind** (FR-PLC-020). Dropping channel 1 of -201U3 from the PLC navigator onto the coil of -201U3-Q1 on folio
203 runs `plc.bind_channel{channel, endpoint}`. It clears a previous binding of the same CP in the same transaction
(FR-PLC-005) and is one undo step.

**A7. Imported address texts** (QET pass P7 of [14](14-interop.md) §3.2; FR-IMP-017; MF-19). The QET importer
recognises "Q0.1" next to a coil as a *pending binding* `{cp, address}`; it never writes an address. Once the
module exists (Golden C fix-script step 6), `ApplyImport` resolves each pending binding, parsed with the
configuration project's profile (prefixes "" and "%"):
1. **Derived match.** The coil's module (the valve's parent `PlcModule`) has exactly one unbound channel whose
   derived `Auto` address equals the text: bind the coil to it and keep `Auto`. This is the "as drawn" binding;
   on Golden C all 28 texts resolve this way to channels 2p + 1, and EW-PLC-012 then reports EF-02 under H1.
2. **No derived match.** Bind the coil to the channel the position map assigns, set that channel's address to
   `Manual(text)` so the print is preserved, and let EW-PLC-012 (discriminator `manual`) report the deviation.
3. **Ambiguous or no module.** The text goes to the import review list (FR-IMP-017 uncertain matches); nothing is
   bound.

**As built (cycle 13; decision log ID-333; superseded for N1 by the cycle-16 paragraph below).** The channel-creating normaliser N1 was not built: channels came only from import (the golden importer reads `channels.csv`, §2.4), and a change of a module's part created or removed none. `Channel.binding` is stored as an `Endpoint` (a CP of the module or of the served coil); the golden project binds channel 2p + 1 to coil 14 of valve position p + 1, as drawn.

**As built.** N1 is built in `ew-commands` (registry row 3 of [06](06-schematic-editor.md) §3.2):
adding the `plc_module` aspect (`fields.set`), creating a device with it, or changing its parts or `part_use`
creates the missing channels of the part's channel spec with their v5 IDs and the spec's kind; surplus channels
stay. `plc.bind_channel` and `plc.config_project.create` are built (§3.9; the latter has no row there: it creates
the configuration project that `ew new` gives a new project). N2 (the solenoid-position map) and N3 are not built:
the map is not typed part data yet, so the golden builder binds the 28 coils with explicit `plc.bind_channel`
commands, which win over the map by definition. `plc.bind_channel` refuses a CP outside the channel's module
and its descendants and unbinds another channel on the same CP (FR-PLC-005). N1 keys existing channels on (module,
number), keeps their authored members, applies no kind change and removes no surplus channel until
FR-PLC-053's confirmation exists. The channel members are not yet settable by `fields.set`.

**As built.** Rows 3, 13 and 14 of the registry run;
normaliser notices are merged once into the command's outcome.
- **N1** (`channel_instantiation`) picks the module's PLC section by the derivation's rule (`plc_module.part_use`,
  else the first part use whose part has a PLC section), creates the missing channels, and gives an existing channel
  0 … count − 1 the part's channel kind with a notice, keeping its binding; a kind of a newer format is kept with a
  notice. Surplus channels stay and are named in a notice; the derivation reads only the channels below the count.
- **N2** (`solenoid_binding`): for the valve at position q (p = q − 1; of two children at one position the lower ID)
  and each map entry, the channel `solenoid_channel(entry, p)` below the channel count and the coil `coil_cp_idx`
  over the valve's functions in ID order, the same evaluation as `solenoid_positions`. A pair is considered when its
  valve is new in the transaction, moved (parent or position), re-parted or gained a function, or when its channel is
  new in the transaction (N1 created the channels after the valves existed). A moved valve whose coil stays bound
  elsewhere gets a notice. Ties break by ID; modules run in ID order.
- **N3** (`module_pin_binding`): a channel new in the transaction, unbound, on a module whose part has `pin_of`, takes
  the CP of the main function (the lowest ID among the device's functions with role main) whose designation
  `channel_pin(pin_of, number)` gives, searched in the part's CP pattern when it has one, else in the main function's
  FnDef copy, the lowest index first. Channels that existed before the transaction are never touched.
- **FR-PLC-051 as built** (the project owner (delegated) accepted the deviation and may reverse
  it): a move keeps an earlier binding, and untouched pairs are never re-examined, so an explicit unbinding stays; a
  rebinding is `plc.bind_channel`. FR-PLC-051 in [02b](02b-requirements-extended.md) is reworded to this.
- **FR-PLC-053** is met through the Bulk command `plc.set_module_part{module, parts?}` (§3.9): it sets the part uses
  and deletes the channels at or above the new count with their bindings, symbols and comments; its preview lists
  them, so committing it is the confirmation (ADR-0008). A part change of a module in the UI goes through this
  command; `fields.set` on `parts` stays the scripting form that never removes a channel.

*As built (M3 persona round).* N0 gives a station whose part's PLC section maps solenoids to valve positions
(`positions`) the role `valve_terminal`, any other station `cpu`: the starter valve terminal EW-GEN-VTM-1 is such a
station ([11](11-fluid-power.md) §2.6). A valve terminal is never its configuration project's `cpu`; a CPU that joins a
project without one becomes it. D4 and the attachment of waiting cards count only the stations that are no valve
terminal, so a card drawn beside a valve terminal still attaches to the project's CPU.

*As built (persona run m3-p4).* With several stations that take cards, D4 attaches a new card to the station drawn
beside it: the one with a representation on the page of the card's first representation, nearest by insertion point
(ties by ID); with none drawn on that page the card waits, and the placement's toast says where it gets a station.
In the PLC navigator a card's row dropped on a station's row runs the station's Attach (`assembly.attach`, which
moves a card of another station there); a station that takes no cards, a card of that station already and a device
that is no card are refused in words. A head station of one's own and the starter's attach alike: the persona's
card waited because his project held two stations, not because the head station was his own part.

*As built (WP-S1.24, FT-830).* `library.update` keeps a copy's ID, so N0 also triggers the devices that use a part
copy the transaction changed when the copy had no PLC section in the base snapshot, and N5 those whose bus part's
ports changed; a newer part version that adds a PLC section or a port reaches its devices in the update's step.

The golden builder keeps its 28 explicit `plc.bind_channel` commands: under H1 the map assigns coil 14 of position p
to channel 2p, while the drawing binds it to 2p + 1, and an explicit binding wins. Not built: the confidence marker
(FT-323), the exchange records in the surplus preview, the unbinding when a valve leaves its module,
N3 after a part change and N-FL-1.

---

### 3.4 Symbols, designations and function texts

**Templates** (1.0, FR-PLC-017) are strings over these tokens: `{target.process_tag}`, `{target.dt}` (the derived
tag without the product sign and suppressed identifiers), `{target.function_text}`, `{pin}`, `{module.dt}`,
`{channel.number}` and `{address}`. `a|b` falls back to `b` when `a` is empty. The result passes through
`ident()`, which in strict IEC mode (FR-PLC-047):
- replaces characters outside `[A-Za-z0-9_]` with `_`;
- collapses runs of underscores;
- strips leading and trailing underscores;
- prefixes `_` when the result starts with a digit. IEC 61131-3 allows a leading underscore followed by a digit
  (verify against Ed. 4); FR-PLC-017's example is `_201U3_Q1_14`.

| Template | Target | Result |
|---|---|---|
| `{target.process_tag\|target.dt}` | -201U3-Q1, which drives -203Q1 (TA05-V-15) | `TA05_V_15` |
| same | spare valve -201U5-Q1 (no process tag) | `_201U5_Q1`, unless the template is `{target.process_tag}` alone, which gives no symbol (Golden A at 1.0) |
| `{target.dt}_{pin}` | -201U3-Q1:14 | `_201U3_Q1_14`; after renumbering -201U3 to -201U4, `_201U4_Q1_14` |

A manual symbol always wins and never follows renumbering. Uniqueness (EW-PLC-007) is tested on the determined,
cleaned symbol, so "TA05-V-15" and "TA05_V_15" on two channels collide.

*As built (WP-S1.24).* `channel_texts` derives the automatic symbol from the configuration project's
`symbols.template` when the channel has no manual one (`symbol_from_template`): every placeholder that resolves empty
leaves the channel without a symbol, and under `strict_iec` the result passes through `iec_ident`. The tokens are the
seven above; `fields.set` refuses another. The process-tag scheme of FR-PLC-049 is the project setting
`process_tag_scheme`, a regular expression with named groups that the whole tag matches, set with
`project.settings.set` (refused when it does not compile); `process_tag_parts` gives the parts, and EW-PRC-001
reports a tag outside it. Sorting by the parts and tokens over them are not built.

**Function texts** are never typed at the channel. The function-text column of the PLC overview and the I/O list
prints the target's function text, else the tracked process tag of the device the target drives
(`DeviceLink{Drives}`, [11](11-fluid-power.md) §3.1, §3.3). Used valves carry no function text of their own, so
-201U3-Q1 prints TA05-V-15 from -203Q1; "Spare" stays a manual function text of the spare valves. Reports keep
function text and process tag as separate columns. Editing -203Q1's process tag, or a valve's manual function
text on folio 203, changes folio 908, the I/O list and the AML comment in the same commit (FR-PLC-022).

---

### 3.5 Target tracking (A4)

```rust
fn channel_targets(q: &Q, module: Id<Device>) -> Arc<TargetMap> {
    let mut out = TargetMap::default();
    for ch in q.channels_of(module) {
        if let Some(f) = ch.target { out.emit(ch, f, Source::Manual, []); continue; }
        let Some(cp) = ch.binding else { out.none(ch); continue; };
        let f = q.cp_function(cp);
        if f.device != Some(module) { out.emit(ch, f.id, Source::Direct, []); continue; } // coil of -201U3-Q1
        let sig = q.signal_of(cp);                 // nets joined by wires, cores, IP pairs, terminal levels, plug pins
        let cands = sig.cps().filter(|c| is_field_function(q, c) && !is_plc_cp(q, c) && !same_device(q, c, module));
        let Some(best) = cands.min_by_key(|c| (category_rank(q, c), page_order(q, c), cell(q, c), c.id)) // deterministic
            else { out.none(ch); continue; };
        out.emit(ch, best.function, Source::Traced, sig.path(cp, best).terminals());     // "-X1031:5"
    }
    Arc::new(out)
}
```

- **Within the signal only.** Tracking follows the signal level of [03](03-domain-model.md) §9.3: wires, cores,
  interruption-point pairs, terminal levels and plug pins. It never crosses bridges, which join potentials, so a DI
  never "finds" every load on a common 24 V rail. FR-PLC-018 says the same: through connections, terminals,
  terminal levels and plug pins.
- **Field functions** are sensors, actuators, coils, lamps, contacts and valve solenoids. Terminals, plugs, cables
  and PLC CPs are passed through, never chosen.
- **Ambiguity** (several candidates) picks deterministically by category rank (coil and actuator before contact),
  then page order, cell and ID. The row is flagged "ambiguous" in the I/O list and in the "why this value?" view
  (FR-UI-011). A manual `target` resolves it (FR-PLC-054).
- **Process tags** follow one `DeviceLink{Drives}` hop: the valve -201U3-Q1 drives the field device -203Q1
  (+FIELD, ▲U-07), so `target.process_tag` of channel 1 is TA05-V-15 (FR-DEV-012, FR-DEV-028).
- **Address at terminals** (FR-PLC-018 option, EPLAN 20854/20855): with the option set, the text slot `PlcAddress`
  of a terminal on a traced path shows the channel address.

At MVP only `Direct` and `Manual` targets exist. This covers every channel of the golden project, because all of
them are internal valve-coil channels.

*As built (WP-S1.24).* The `Traced` branch is built: a channel bound to a pin of its own module walks the stitch
graph's net and signal edges from the pin (wires, cores, interruption-point pairs, terminal levels, shorts; never
bridges), passes terminals (contact kind `terminal`), never chooses a CP of a PLC device, of the module or one a
channel binds, and picks the lowest (category rank: coil and valve 0, black box 1, contacts 2, other 3; drawn page
order; function ID; CP index); the cell is not a tie-break. The target carries its source, its CP (the I/O list's
target pin), the passed terminals and the ambiguity flag. The passed terminals are those on the path to the chosen
target (`sig.path(cp, best)`): the walk is breadth first, and a terminal is passed when the shortest path from the pin
to the target crosses it (two of the path's nodes hold its CPs), so a terminal on a branch to another candidate, or
one that only hangs off the path, is not. Only a module with a pin-bound channel reads the stitch graph. A traced target gives the channel the state of its device (§3.2 step 3). The "address at terminals" option is
the drawing setting `plc_address_at_terminals` (`project.drawing.set`, FT-1725): with it, every terminal a traced
signal passes prints the channel's address in its `PlcAddress` slot (`channels_by_target` holds the terminals). The
shipped terminal symbol `ew.iec.terminal` and its QET twin (version 0.1.2, FT-1734) have the slot to the right of the
terminal under its function text, in the arrangement `full`; the window's switch is Project ▸ Channel addresses at
terminals.

---

### 3.6 Representations

| Representation | Page type ([06](06-schematic-editor.md) §2.2) | Shows | Joins nets | Cross-reference |
|---|---|---|---|---|
| Module box with channel pins | MultiLine | pin designation and the address, symbol and function-text slots per channel | yes | main representation of each channel |
| Coil on a fluid valve | Fluid (203–206) | address and function text slots of the valve | no wire (internal CP) | target of the PLC-overview xref |
| PLC overview box (FR-PLC-052) | Overview | every channel of a module or station: number, address, symbol, function text, state | no (`Overview` connections) | per channel to its main representation (EPLAN 20301) |
| Station bus port | BusTopology (3) | DT, order code 1, part type, "Node {address}" | bus nets | aux → main "(201-E1)" |
| Card power connector | MultiLine (106) | pins 1–5 and FE | yes | aux → main "(201-E1)" |
| Assembly front view | AssemblyOverview (201–202) | modules and valves in manifold order | no | "∞ 203-B4" (FR-XRF-016) |
| PLC overview report | Report (908) | 56 rows, a link per row | — | link annotation to the coil's anchor |

- **One main representation per channel:** the representation that draws the binding CP. A valve representation
  draws two functions of one device, the 5/2 fluid function and the coil-14 function, so the coil's main
  representation is the valve symbol on 203.
- **The `PlcAddress` text slot** ([07](07-symbols-parts-macros.md) §2.3) resolves:
  - on a bound CP's function, to that channel's address;
  - on a fluid function whose device has bound coils, to the coil addresses in solenoid order 14, 12 ("Q0.0 /
    Q0.1" on a bistable valve, as in [11](11-fluid-power.md) §3.1);
  - on a module pin, to its channel's address.
- **Placing a channel symbol binds it**; it never copies properties (PBF §2.7, rule 2). Every representation
  renders the same determined values.

*As built (WP-S1.24, FT-700).* A function that no channel binds but that is the manual or traced target of channels
prints their addresses in its `PlcAddress` slot (`channels_by_target`), so a relay coil wired to a card output prints
the channel's address.

*As built.* The `PlcAddress` slot prints as above; Golden A's embedded valve, lamp and position-switch symbols carry it, so folios 203 to 206 print Q0.1 … Q6.7. A black box's pin text is the function CP's designation as stored and, for a CP bound to a channel, the printed address after one space ("1 I0.0"); a designation the symbol's `cp_designation` slot prints is left out. Pin texts never move; a cross-reference text that would overlap one drops below it (folios 102 and 106 of Golden A).

**A card drawn in cells** (FT-1898, 0.3.5; as the pilot's reference set draws a card). A card's box in the one-row form
of [06a](06a-drafting.md) §7.5 (at least two of its box CPs bound to channels, all of them on its upper edge, or all
on its lower edge) prints its channels in cells: a thin line between each two neighbouring channel pins, at the middle
between them, across the box, and in each cell, level and centred on its pin, the pin's text as above ("2 Q1.1") next
to the pin and the channel's function text (§3.4, the function text of the device wired to it) further inside, on
the next line; where it has none yet (nothing is wired to it), the channel's own text, its symbol name, so the page
says what an input is for before its device is drawn (FT-2133), and the refusal of a function text typed at such a
channel says where its text will come from and that the cell prints the symbol name until then. A text wider than its cell is made smaller, down to half its size, and below that wrapped at its words
at half its size. Nothing is stored: the view decides from the box and the bindings, and the labels layer draws the
lines with the texts. Every other box keeps its pin texts as before (turned a quarter on its upper and lower edges): a
card's box with channel pins on two edges, which only a project of an earlier version may still hold, a box whose
part lays it out with pins on several edges, a box without bound pins, a card's box turned a quarter (its channel pins
then leave sideways). Not
done: the symbol beside a function text and the process tag per channel, which the reference set prints in some
cells.

**Every placement lays a card out in one row** (the owner: the two-row form is retired). One function lays a
card's box out, `CardPins::boxes` over `part_box::card_box`: its inputs' pins in one row on the upper edge of a box,
its outputs' in one row on the lower edge of another, its supply down the first box's left side, a row wider than the
drawing area split as below. Every path that makes or redraws a card's box calls it: Draw on a page, Draw the card in
one row, the Device tool and Insert device's Place of a card part that has no device box of its own (its box dragged
by two corners takes the card's box at the first corner, in the size its row needs; a card of inputs and outputs gets
its inputs' box, and its outputs' waits in the station view's To do), the Black box tool with such a part, and Place
here; the device box editor starts a card's box with every pin in one box (`CardPins::one_box`: its channels in one
row, the inputs' then the outputs'), and the device browser shows the box the Device tool places. Not a card's form,
and so not changed: a part's own device box, which may put pins on several edges (doc 07 §2.5), a station's head and
any other box without channel pins (its supply and bus pins laid by the black box tool, a source's outputs along the
lower edge), a bus device's sockets, a paste or a copy of pages (it copies the boxes as they stand) and an import (it
keeps the source's drawing). A project of an earlier version keeps drawing the two-row boxes it stores: nothing
recomputes them, and Draw the card in one row is the person's way to bring one over, as it draws the wires anew.

**A card of the older form drawn in one row** (FT-1897, 0.3.5). *Draw the card in one row* (`plc.card_redraw`, in the
box's menu, offered for one selected box with channel pins on both its upper and its lower edge)
draws the card again as Draw on a page lays a card out, in its place and in one undo step: one `plc.draw_station` whose
`replace` names the old box, its inputs in a box where the old box stood and its outputs in a box a gap under that.
The device, its functions, channels and connections stay, because a connection joins the function's connection
points and not the box; a toast says that Arrange and route draws the wires to the new pins. The old box's texts a
person moved or hid go with it: the new boxes print their texts where the one-row layout puts them. A row wider than
the drawing area from the old box's left edge to the area's right is split into boxes of as many pins as fit there,
as Draw on a page splits a row (`part_box::card_share`), each box a gap under the one before; where Draw on a page
leaves the rest of the row for a page of its own, the redraw keeps every pin on the page, where its wire is (FT-2083).

**The card's wires on the terminals' facing pins** (FT-2083, 0.3.5). After the redraw a terminal of a channel that now
faces the other way still has the card's wire on the pin it had. *Terminal wires to card side*
(`plc.card_terminals_face`, in the box's menu of any card, beside Draw the card in one row) runs `terminal.face_card
{card}`: each wire between one of the card's channel pins and a terminal drawn with two pins on the wire's page goes
to the terminal's pin that leaves the opposite way to the channel pin (the lower pin of a terminal over a card of
inputs, whose pins face up), the field's wire to the other: the wires of the terminal's two pins on that page change
places and lose their routes, and Arrange and route sets the terminal in its channel's column on the card's side
([06a](06a-drafting.md) §4.8, rule E4) and draws them. `terminal.face_card {strip}` does the same for the terminals of
a strip, wired to any card. A terminal drawn with more than two pins (a two-level block), one whose pins leave
sideways and a wire drawn on another page than the terminal stay as they are; a card or a strip with nothing to change
is refused in words.

---

### 3.7 Valve terminals

The valve terminal is **one device tree in three disciplines** (ARCH §4.3, REF-098):

| Device | Parts | Aspects and functions | Drawn on |
|---|---|---|---|
| -201U1 | 195740 + 550208 | `PlcStation` (role ValveTerminal), `BusNode`, `Assembly` (order codes 1 and 2), `ValveTerminal` (pilot supply, [11](11-fluid-power.md)); main function = assembly view; aux functions = DP bus port, card power connector | 201 (main), 3, 106 |
| -201U2, -201U9 | 552286, 533354 | children, no I/O | 201 |
| -201U3 … -201U8, -202U2 | 533361 | children with `PlcModule` (positional slots 1–7), 8 channels each | 201, 202 (overview), 203–206 (aux label "(201-E4)") |
| -202U1, -202U3 | 550208, 533354 | children, no I/O; -202U1 carries the second power connector | 202, 106 |
| -201U3-Q1 … -202U2-Q4 | 571334 | 28 valve children: fluid function (ports 1–5) + coil-14 function with internal CP | 203–206 |

- **Spare positions** are real valves in state spare (U-36) whose coils are bound. Their channels are spare. A
  blanked position (no valve) gives deactivated channels.
- **Other vendors** use the same model: SMC EX600 SI units, Festo CPX-AP and VTUX, Bürkert AirLINE, Emerson AES. Only
  their part data differs (PBF §8.2). EW ships the Festo CPX/MPA part data of the golden project as test data.
- **Order codes** (51E-F13GCQP-D+H2GG, 32P-VGL-R-MAHAHAHA…-28MS) are stored verbatim on the assembly. Children are
  built by hand or in the rack/slot editor at 1.0. Configurator decoders that expand an order code into children are
  1.x plugins (IDEA-026). Slot-occupancy validation (EW-FL-003, D-40) is in [11](11-fluid-power.md).

---

### 3.9 Commands, freezing and ownership

| Command | Effect | Cost | Release |
|---|---|---|---|
| `plc.set_start{modules, dir, spec}` | sets Auto, Manual, Excluded or Tbd starts | Interactive | MVP |
| `plc.readdress{scope}` | recomputes starts for a selection or a configuration project; previews old → new address, folios and symbol changes; one transaction (FR-PLC-014) | Bulk (preview) | MVP Should |
| `plc.convert_manual_to_auto{scope}` | drops manual values that equal the derived value; lists the others | Bulk | MVP Should |
| `plc.freeze{cfg}` | sets the derived address of every used or reserved channel to `Manual(current)` and pins every module start (FR-PLC-015) | Bulk | 1.0 |
| `plc.unlock{scope, group}` | lifts PLC ownership protection for a field group | Interactive | 1.0 |
| `plc.bind_channel{channel, endpoint}` | binds or rebinds (§3.3) | Interactive | MVP |
| `plc.set_module_part{module, parts?}` | changes a module's part uses and removes the channels the new part no longer has, with their bindings; the preview is FR-PLC-053's confirmation (§3.3) | Bulk (preview) | MVP |
| `plc.set_profile{cfg, profile}` | changes rendering only; the files differ in one setting (FR-PLC-007) | Interactive | MVP |
| `plc.apply_exchange{cfg, image, resolutions}` | applies an AML or XLSX import with its snapshot (§4.4) | Bulk (preview) | 1.0 |
| `plc.apply_io_list{rows, resolutions}` | applies a CSV/XLSX I/O-list import (§4.7) | Bulk (preview) | MVP Should |

**Re-address preview.** With Auto starts, inserting a module between -201U4 and -201U5 previews 40 address changes
(5 modules × 8 channels), 20 of them on bound coils on folios 204–206. Undo restores everything in one step.

**Protection.** After the first applied PLC import, the field groups owned by `Plc` (§2.2) are protected: module
starts, firmware, IP addresses, PROFINET names and DP station addresses. An edit of a protected field is rejected in
strict mode with the message "address owned by PLC1 (TIA import of <date>)" and an "Unlock" quick fix. In lenient
mode it is reported. Inserting the same module after an import moves nothing: the five later starts are
`Manual` (PLC-owned), so A1 gives the new module the first free byte, Q7, and the preview lists only its 8 channels.

*As built.* `plc.set_start` and `plc.set_profile` are built (Interactive; the IEC profile prints `%QX0.1`); a start edit re-derives only its own module's channel addresses (`ModuleStartOf`).

*As built (WP-S1.24, FT-725).* `plc.freeze {config_project}`, `plc.readdress {config_project | modules}` and
`plc.convert_manual_to_auto {config_project | modules}` are built, all Bulk. They start from the transaction's base
snapshot, whose derived addresses a command may read (as `device.renumber` does); ADR-0008 amendment 4 forbids reading
the memoised queries of the pending state, so the re-address preview computes its old → new notices on a model the
transaction's ops give, through a fresh `Q`. `plc.freeze` writes the derived address of every used or reserved channel
(its effective state, §3.2) as `manual`, and every `auto` module start A1 places as a `manual` start; a spare or
deactivated channel keeps its `auto` address, which cannot move while its module's start is pinned, so a frozen
project gives no spare channel EW-PLC-009's "manual address but no connection". The pinned starts are written so that a module inserted before a frozen one is placed around it
(A1 reserves manual starts first) instead of moving it from under its frozen channels; it is refused in words under a
tag-based profile. A frozen channel that a later start move leaves outside its module is EW-PLC-002's finding alone,
not also EW-PLC-012's `manual` one. `plc.readdress` sets the scope's manual starts and manual channel addresses to
`auto` and lists every changed address of the affected configuration projects (`channel 3 of -201U5: Q10.3 → Q2.3`);
`plc.convert_manual_to_auto` drops the manual channel addresses equal to the derived ones and the manual starts that
A1 places at the same byte, judged on the result and repeated until stable: a start whose conversion moves the printed
address of any channel of the affected configuration projects (an automatic module after it) stays as well, as
`plc.readdress` compares them. It names every value it keeps. The ownership
protection and `plc.unlock` came with WP-S3.16 (below). `plc.bind_channel` also takes tags in a command log: a channel as `<module tag>#<number>` or `<module tag>:<designation>`, a function as `<tag>` or `<tag>:<slot>`.

*As built (fix round).* `plc.bind_channel` checks the CP against the CPs the function draws, by the one accessor the commands share with the join (`common::function_cp_indices`): the template entry's pattern, else a black box's part pattern, else the FnDef. A black-box card's pins therefore bind; before, the check read the generic black-box FnDef, which defines no CPs, and refused every pin (found by the play-test brief `brief-plc-io`).

*As built (WP-S3.16).* **Protection.** From the first applied PLC import on (`ownership.imported`), an edit of a field
of a PLC-owned group is refused: a module's start (`plc.set_start`, `plc.readdress`, `plc.convert_manual_to_auto`,
`fields.set` and `fields.set_many` on `plc_module`), a station's firmware, the address of an existing bus port of a
station's device tree (`fields.set`; a port added to a function is no such edit) and, when symbols are given to the
PLC, a channel's symbol, data type and comment (`fields.set` on `channel`). The refusal is judged on what the
transaction changed against its base, so every path of those commands is covered, and reads `-201U6: the start address
is owned by configuration project PLC1 (PLC import of <date>); unlock it first with plc.unlock`. An import under the
PLC's ownership writes every start the file gives a matched module as a `manual` start, also one equal to the
project's, so a module inserted afterwards moves none of them, as the paragraph above asks. `plc.unlock {config_project,
group}` (`module_starts`, `plc_data`, or `symbols` when symbols are given to the PLC) lifts the protection of one group
until the next applied import that gives a value of that group; it refuses a group that is not protected. The scope is the configuration project. The
refusal is the commands' own, not a check rule with a quick fix, and there is no lenient mode for it: an import applies
PLC-owned values through `plc.apply_exchange`, which is the PLC side. `plc.record_exchange {config_project, stations?,
format, version?, file_sha256, date, comment_lang?}` records an export (§4.4), `plc.apply_exchange {config_project, image, format,
version?, file_sha256, date, complete?, comment_lang?, resolutions?}` applies an import (Bulk, §4.4) and
`plc.apply_instrument_list {config_project, rows}` an instrument list (Bulk, §4.7).

---

### 3.10 Check rules

The EW-PLC pack ships at MVP with the rules marked MVP and is complete at 1.0 (FR-CHK-005). Rules are registered
Rust functions unless noted. Scopes are `checks(scope)` keys (ARCH §12). EW-PLC-001, -005, -007, -015 and -016 use the
configuration-project scope `cfg`, which [15](15-checks-revisions-collaboration.md) §2.3 lists at this document's
request. Pack membership and the MVP subset match [15](15-checks-revisions-collaboration.md) §4.1 and §4.5.

| Rule | Checks | Scope | Modes | Severity | Release | Requirement | Golden A |
|---|---|---|---|---|---|---|---|
| EW-PLC-001 | Duplicate address as bit-range overlap in configuration project and area (IW0 vs I1.3); word alignment warning | cfg | online, prevent (strict) | error | MVP | FR-PLC-044 | 0 |
| EW-PLC-002 | Address outside the module's I/O range | module | online | error | MVP | FR-PLC-044 | 0 |
| EW-PLC-003 | Bit index outside the profile's unit width | module | online | error | MVP | FR-PLC-044 | 0 |
| EW-PLC-004 | Channel kind ≠ connected function (DO to a proximity sensor) | module | offline | warning | 1.0 | FR-PLC-046 | 0 |
| EW-PLC-005 | Symbol not a valid identifier for the target profile | cfg | export, release | error | 1.0 | FR-PLC-047 | 0 |
| EW-PLC-006 | *Retired.* Spare channels are listed by reports and counts (FR-PLC-019), not as findings | — | — | — | — | — | — |
| EW-PLC-007 | Duplicate determined symbol in a configuration project | cfg | online, prevent | error | MVP | FR-PLC-016 | 0 |
| EW-PLC-008 | Used channel without address | module | online (warning), export (error) | warning | MVP | FR-PLC-044 | 0 |
| EW-PLC-009 | Channel with address or symbol but no connection, not spare | module | online | warning | 1.0 | FR-PLC-046 | 0 |
| EW-PLC-010 | Data-type width ≠ channel width (Bool on a 16-bit AI) | module | online | error | 1.0 | FR-PLC-046 | 0 |
| EW-PLC-011 | Two modules in one slot; module in a disallowed slot; `Max_Module`; maximum I/O length | station | prevent, online | error | 1.0 | FR-PLC-028 | 0 |
| EW-PLC-012 | Binding or address ≠ part map (discriminators `binding` and `manual`); supersedes EW-NUM-002 (FR-CHK-037) for addresses | module | online | error | MVP | FR-PLC-045 | **28 under H1, 0 under H2 (EF-02)** |
| EW-PLC-013 | Card supply group unconnected or fed from a wrong potential type (FE on PE); supersedes EW-POT-003 (FR-CHK-030) for PLC supply CPs | station | online | warning | 1.0 | FR-PLC-048 | 2 (EF-22) |
| EW-PLC-014 | Summed output load (coil power from part data) > channel, group, module or feeding-channel rating | station | offline | warning | 1.0 | FR-PLC-048 | 0; evaluation lists MICO Out1 (EF-24, D-25) |
| EW-PLC-015 | `Tbd` module start or channel address | cfg | online (info), release (error) | error at release | MVP | FR-PLC-013 | 0 (the `Tbd` bus address is EW-BUS-001) |
| EW-PLC-016 | Exchange drift: the configuration project changed since the last exchange (§4.4) | cfg | online | info | 1.0 | FR-PLC-035 | 0 |
| EW-SAF-001 | F-destination address missing, duplicate in a logical net or in the project, or outside the DD range | project | online | error | 1.0 | FR-PLC-042 | — |
| EW-SAF-002 | 1oo2 pair incomplete or wired to different devices | module | offline | error | 1.0 | FR-PLC-043 | — |
| EW-SAF-003 | F-DI sensor not supplied from the module's test-pulse pin | module | offline | warning | 1.0 | FR-PLC-043 | — |
| EW-PRC-001 | Process tag does not match the scheme; duplicate on different devices | project | online | warning | 1.0 | FR-PLC-049 | 0 |
| EW-PRC-002 | Instrument-list tag without device or channel (open item) | project | online | info | 1.0 | FR-PLC-050 | — |
| EW-BUS-001 | Bus address missing (open item) or duplicate in a logical net | lnet | online, release | error | MVP | FR-BUS-015 | 1 (EF-03) |
| EW-BUS-002 | Address outside the bus range | lnet | online | warning until verified | 1.0 | FR-BUS-018 | 0 |
| EW-BUS-003 | PROFINET name invalid or duplicate; IP duplicate or outside the subnet | lnet | online | error | 1.0 | FR-BUS-017 | — |
| EW-BUS-004 | Termination not On at exactly both segment ends | pnet | online | error; info for unknown external ends | MVP | FR-BUS-016 | 1 info (EF-16) |
| EW-BUS-005 | Rate not supported by a slave (GSD) | lnet | online | error | 1.0 | FR-BUS-009, -018 | 0 (rate unset ▲U-22) |
| EW-BUS-006 | Segment length, station count or stubs over the limit for the rate | pnet | offline | warning | 1.0 | FR-BUS-018 | 0 |
| EW-BUS-007 | Logical net without master (an external partner counts) | lnet | online | warning | 1.0 | FR-BUS-018 | 0 |
| EW-BUS-008 | EtherCAT E-bus current budget | station | online | error | 1.x | FR-BUS-020 | — |
| EW-BUS-009 | Bus port on a non-bus cable or the wrong connector type | pnet | online | warning | 1.0 | FR-BUS-018 | 0 |
| EW-BUS-010 | Ports joined by `Bus` connections but in different logical nets | pnet | online | warning | 1.0 | FR-BUS-024 | 0 |
| EW-IOL-001 | Port class mismatch; IODD device on a port not in IO-Link mode | module | online | error | 1.0 | FR-BUS-019 | — |
| EW-IOL-002 | Process data > port window; cable longer than the maximum | module | online | warning | 1.0 | FR-BUS-019 | — |

**EW-PLC-012 and the odd-bit question (D-26).** Folios 203–206 bind coil 14 of position p to channel 2p + 1. If the
VMPA1-FB-EMG-8 assigns the lower bit to solenoid 14 (H1, suspected but unverified), the printed Q0.1 … Q6.7
address unused coil-12 outputs: a functional bug, not a style choice. EW does not decide this in code. The
solenoid-position map of part 533361 is data with confidence `test`:
- **H1** gives 28 findings, one per binding;
- **H2** ("coil 14 on 2p + 1") gives none;
- switching between them is a part-data edit (AC-29).

Each finding names the bound channel, the channel the map expects, both addresses and the unverified-data marker.
It offers two quick fixes: "rebind to the map's channel" (a previewed command; folios 203–206 then print Q0.0,
Q0.2 …) and "open the part's PLC section". Setting the map's confidence to `verified` needs a source (vendor manual
reference or the pilot's PLC program, PQ-15).

**One defect, one finding.** Following the supersession table of [15](15-checks-revisions-collaboration.md) §3.6
(FR-CHK-045): EW-PLC-012 supersedes EW-NUM-002 (FR-CHK-037) for addresses, EW-PLC-013 supersedes EW-POT-003
(FR-CHK-030) at 1.0 (EF-10 → EF-22), EW-BUS-001 supersedes EW-CORE-044 (FR-CHK-034) for bus addresses, and
EW-PLC-015 supersedes EW-CORE-044 for module starts and channel addresses. EW-PLC-015 covers only module starts and
channel addresses, so it never reports the bus `Tbd`, and each `Tbd` yields exactly one finding. Every rule ID has
its documentation page (NFR-DOC-02).

*As built.* EW-PLC-001, -002, -003, -007, -008, -012 and -015 are built in the scopes `module` and `cfg`, with en-GB and nl-NL messages; EW-PLC-007 compares symbols exactly, and findings name a channel as `<full tag>:<designation>`. Their prevent half (EW-PLC-001, -007) is not built (FT-726). EW-PLC-012 offers "rebind to the map's channel" and carries the unverified-data marker; "open the part's PLC section" is FT-727; Golden A reports its 28 findings under H1 (EF-02). Two channels of one module with one number are reported by EW-CORE-008 `duplicate_number`, and the channel whose ID is v5 of its module and number wins.

*As built (WP-S1.24, FT-323).* Every rule that reads a part's `plc` or `bus` section marks its findings "based on
non-verified part data" when that section's confidence (`ItemMeta.confidence`) is not `verified`: EW-PLC-010, -011,
-012, -013, -014, EW-IOL-001, -002, and EW-BUS-002 (a part's range), -005 and -009.

*As built (WP-S1.24).* The 1.0 content of the EW-PLC and EW-BUS packs is built: EW-PLC-004, -005, -009, -010, -011,
-013, -014; EW-SAF-001 … -003; EW-PRC-001; EW-BUS-002, -003, -005, -006, -007, -009, -010; EW-IOL-001, -002, each
with en-GB and nl-NL messages and a rule note. They run in every shipped scheme that their modes allow (doc 15
§4.1; FT-1728). Decisions: EW-PLC-004
judges contacts against outputs and coils or valves against inputs, not black boxes; EW-PLC-009 reports a manual
address or a symbol on a channel that reaches nothing and that nobody marked spare, reserved or deactivated
(`Channel.state` unset); EW-PLC-010 compares the IEC elementary types with one bit for DI and DO and a word for AI and
AO; EW-PLC-011 reads `allowed_slots` and `limits` and has its prevent half; EW-PLC-013 reports an FE pin on PE within
a station's device tree (the finding that supersedes EW-POT-003's, EF-22) and the L+ and M pins of `supply_groups`
that are unconnected or of another potential type; EW-PLC-014 reads the target device's technical data `i_load` (A),
else `power` (W) at `voltage` (V, 24 V when absent), against `ratings`; EW-SAF-001 judges uniqueness project-wide;
EW-SAF-003 reads `SafetyChannel.test_pulse` and the signal of that pin; EW-BUS-002 takes the part's `address_range`,
else PROFIBUS 0–125; EW-BUS-003 judges the subnet against the master port's address and mask; EW-BUS-006 holds
PROFIBUS limits (32 stations, 1200 … 100 m by rate, no stubs above 1.5 Mbit/s) as unverified data, for a segment
that is PROFIBUS by its logical net or, when its ports name none, by every port's bus, with lengths compared to the
nanometre (as EW-IOL-002's cable length); EW-PLC-013 reads no pin of a bus port as a supply pin and accepts an M pin on
L−; EW-SAF-002 reports a 1oo2 channel named as its own pair as incomplete; EW-BUS-009 reports a
cable whose part's `cable_for` excludes the port's bus and mated ports whose parts name different connectors. The
prevent halves of EW-PLC-001 and EW-PLC-007 are built (FT-726), judged per channel: a batch is refused when a channel
that was in no overlap group (or duplicate-symbol group) before is in one after, or when a touched channel's group
gains a channel it was not grouped with. A repair of a held overlap (one of three channels moved away, which leaves a
group of two) and an edit that keeps or shrinks the groups commit. The touched configuration projects are those of the
touched channels, devices and parts (`library.update`) in the snapshots before and after the batch, so a deleted
module whose removal moves the next one onto a manual address is checked; EW-PLC-011's prevent half finds its
stations the same way, and its byte sums take an IO-Link master's port windows (`module_io`). EW-PLC-007 compares symbols cleaned by `iec_ident` under a strict policy. EW-BUS-008 is
1.x. EW-PLC-012's second quick fix opens the part's PLC section in the window (WP-S2.13) through the existing
`part.edit_copy`; a finding carries one command fix. *As built (FT-1766).* The Messages row of an EW-PLC-012 finding
offers **Open the part's PLC section…** (the window's offer of its rule, beside the command fix "Rebind to the map's
channel"): `part.edit` on the part of the channel's module, whose part editor shows the PLC section with the map for a
part of one's own and offers Copy as a part of your own… for a library's; the rule in `ew-checks` is unchanged.

*As built (fix round).* `address_index(cfg)` holds overlap groups, not pairs: each connected run of channel addresses in one area whose bit ranges overlap in a chain, once, so N channels on one address make one group of N. EW-PLC-001 reports one finding per group, with every member channel as a target and the first in address order as the subject; the text names the first five channels with their addresses and counts the rest (`addresses overlap in configuration project <cfg>: <channel> (<address>), …; and <N> more channels; give all but one of them another address`, the "and N more" clause only beyond five), in en-GB and nl-NL.

*As built (WP-S3.16).* EW-PLC-016 and EW-PRC-002 are built, in the 1.0 content. EW-PLC-016 compares the exchange
image with the snapshot's records field by field instead of a hash (nothing derivable is stored): one finding per
configuration project whose exchanged stations differ, with the exchange's format, direction and date, the number of
differing objects and the first three; after an import, a value the merge kept on the project's side is such a
difference (the pending export). EW-PRC-002 reports a row of an applied instrument list without a channel, or whose
placeholder device is gone. It is a finding of the 1.0 content and not yet a row of the open-item table
(`open_items`): the table's rules run in release mode of the shipped scheme, which runs the MVP content, and the
rule is online only (doc 15 §4.5.1).

---

## 8. Edge cases and pitfalls

1. **Unconfirmed absolute addresses.** Golden starts are relative to the node (▲U-22). Until the PLC side owns the
   starts, exported tag addresses carry a warning, and the I/O list shows a "relative" marker per station.
2. **Word and bit overlap.** `IW0` overlaps `I1.3`. EW-PLC-001 compares bit intervals, never strings, and word
   alignment is a warning only.
3. **Module part changes and station deletion** shift later Auto ranges; both are previewed (§3.3, §3.9), and
   PLC-owned starts never move.
4. **Git merges.** Two branches adding modules to one station cannot create overlapping Auto ranges: Auto starts
   are derived after the merge and skip occupied ranges (§3.1). Overlapping Manual values set on both branches
   surface in the post-merge integrity check (EW-CORE-013, [05](05-project-format-and-persistence.md) §6.3) and in
   EW-PLC-001. v5 channel IDs prevent duplicate channel records.
5. **Symbols.** Cleaning can make two symbols equal ("TA05-V-15" and "TA05_V_15"); EW-PLC-007 tests the cleaned
   value. Leading digits get "_" (FR-PLC-017, FR-PLC-047). TIA quoting rules are opt-in.
6. **Foreign GUIDs.** TIA may replace EW's IDs on re-export (unverified). Matching then falls back to IEC tags, and
   ambiguous cases go to the parking list instead of being guessed.
7. **Partial exports.** A file holding one station never deletes the others (§4.4 scope rule).
8. **Multilingual comments.** AML carries every language. The TIA XLSX carries one, and its re-import touches only
   that language.
9. **Bistable valves and blanking plates.** Both solenoids are claimed and bound by N2. Empty positions give
   deactivated channels. EW-FL-002 compares solenoids with bound channels.
10. **IO-Link mode changes** alter channel kind and data width, and thereby the module's I/O width and the starts
    of later modules. *As built:* a port's mode is an ordinary field edit (`fields.set` on `Channel.mode`, a row of
    Properties), not a previewed Bulk command; the re-address preview of §3.9 shows the moves it causes when run.
11. **External partners.** Their address and termination are often unknown. EW-BUS-004 reports an info, and
    EW-BUS-007 accepts an external master. Plant-wide F-address uniqueness can only be checked within the project
    and against values entered on external partners.
12. **`Tbd` vs `NotApplicable` vs empty.** Only `Tbd` is an open item. An empty value on a used channel is
    EW-PLC-008.
13. **Spreadsheet edits of derived columns** are reported, never applied (§4.7).
14. **Scale.** At XL (10,000 channels) `address_index(cfg)` sorts once per commit that touches addresses; AML is
    written streaming. NFR-PERF-22 budgets AML export and import preview at 10 s each.
15. **Unverified constants.** Bus ranges, segment lengths, IO-Link limits, PROFINET name rules and the analog
    default starts ship with `verified = false` and report at most warnings (FR-CHK-046, NFR-COMP-05).

---

## 9. The reference project

No reference project is mapped here: the tests open the sample project (`fixtures/sample/`).

### 9.1 Folio by folio

The PLC and bus content of a page is specified in §2 and §3; no folio is mapped here.

### 9.2 Module -201U3 under H1 and H2

The symbol of a used channel follows from the process tag of the device its valve drives (§3.4), and a binding
that differs from the module's solenoid-position map is reported by EW-PLC-012 (§3.3).

### 9.3 Defects prevented

| Defect | How this design prevents or reports it |
|---|---|
| D-01 -4W1 / -4X1 in the BOM | -3W1 and -3X1 are real devices; their tags are derived, and the topology page holds no free text |
| D-09 no PLC overview or I/O list | `plc_overview` (908) and `io_list` blocks at MVP; bus reports at 1.0 |
| D-10 overview 5 vs 4 valves | overview generated from the assembly's children; channels and positions come from the same children |
| D-15 "Node TBD"; termination as text | typed `Tbd` (EW-BUS-001 blocks release, EF-03); termination is a property checked by EW-BUS-004 |
| D-16 FE on PE at -201U1:3 | pin role FE in part data; EW-POT-003 (FR-CHK-030, EF-10) at MVP, superseded by EW-PLC-013 (EF-22) at 1.0 |
| D-25 crossed supply pairs, one 6 A channel | supply groups El/Val; EW-PLC-014 load evaluation; load summary (FR-REP-026) |
| D-26 odd-bit addresses | addresses derived from part maps; EW-PLC-012 under H1; the decision is part data with confidence |
| D-27 untagged valves | child devices with derived tags; process tags on field devices; automatic symbols from them |
| D-36 order code vs components | order codes on the `Assembly` aspect; channels from component parts |
| D-40 CPX block inside the MPA section | -202U1 carries no `PlcModule`, so addressing is unaffected; slot validation EW-FL-003 (1.x) |
| D-41 assembly tagged from two pages | positional slots from `Device.position`, never from tags or pages |

