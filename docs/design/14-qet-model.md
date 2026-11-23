# Interoperability: import, export and migration: QElectroTech: geometry, elements, conductors and folios

A chapter of [Interoperability: import, export and migration](14-interop.md), whose table says where each section lives.

---

## 4. QElectroTech importer

`ew-import-qet` is the only crate that may contain ported QET code (GPL-2.0-or-later, with original notices; NFR-LIC-05,
C-8). Porting is used where fidelity depends on QET's own algorithms: formula evaluation (`assignvariables.cpp`),
conductor auto-routing for paths without stored segments, and title-block layout. A `.qet` embeds every placed
element definition, so the importer never reads the user's QET library (FR-IMP-006).

---

### 4.1 Versions

| Source version | Handling |
|---|---|
| 0.3–0.6 | Element texts as `<input tagg="label">`; no terminal UUIDs; conductors reference per-folio integer terminal IDs only |
| 0.7 | `dynamic_elmt_text` with `text_from` UserText, ElementInfo or CompositeText; text groups |
| 0.8 | Terminal UUIDs (mixed with integer ends); `graphics_table` (nomenclature, summary) |
| 0.9 | `properties/element_type` on terminals; slave type "Other" |
| 0.100 | `terminal_strips` data model and drawn `terminal_strip` items; folio `version="0.100.0-dev"` (the pilot's files) |
| 0.200 | UUIDs on project, folio, wire and shape; `<cables>` with cores; `mirror`; `plcMasterData`; `slaveContactGroups`; wire hops; `wiring_list_view` |

Element definitions inside one project carry their own versions (0.3 … 0.90 in one corpus file) and are mapped per
definition. The XML is parsed with roxmltree with DTDs refused; a non-UTF-8 prolog encoding is transcoded or
rejected (§14).

---

### 4.2 Geometry on the native rational grid

- **Units.** 1 QET px = 25.4/96 mm = `793_750/3` nm; the 10 px grid is `7_937_500/3` nm (ADR-0005). Qt writes
  coordinates with `QString::number`, so values such as `12.5`, `-3.33333` and `1.23457e+06` occur. The importer
  parses each decimal string **exactly** into a mantissa and a power of ten, computes
  `nm = round_half_even(m × 793_750 / (3 × 10^e))` in `i128`, and never passes through `f64`. A point on the
  10 px grid therefore lands on `round(k × 7_937_500 / 3)` nm with grid index `k`, and FR-IMP-008's "0 nm deviation"
  is an equality test on indices.
- **Axes.** QET's origin is the top-left and y points down, like `Pt2` (03 §3.1); no flip is applied.
- **Instances.** Element `x`, `y` places the definition origin, the hotspot. QET writes definition coordinates
  already relative to the hotspot; `hotspot_x`, `hotspot_y` only place the bounding box (`width`, `height`) around
  it. The importer therefore takes definition coordinates as written (hotspot = definition origin). A terminal's
  page position is `instance + R(orientation, mirror)·terminal`, exact in integers because rotations are multiples
  of 90°.
- **Orientation.** `orientation` k = 0–3 maps to R(90k): R0, R90, R180, R270, clockwise on the sheet
  ([06](06-schematic-editor.md) §2.5, where Mn mirrors about the symbol's vertical axis and then rotates by n). With
  QET 0.200 `mirror`, and assuming QET mirrors in the element frame before rotating: `horizontal` → M(90k),
  `vertical` → M(180 + 90k), `both` → R(180 + 90k), angles mod 360. The assumption is unverified and is fixed by
  rendering mirrored corpus elements in QET and in EW. QET 0.100, the pilot's
  version, writes no `mirror` (the pilot's projects hold none), and the importer reads none, so an element a later
  QET mirrors arrives unmirrored.
- **Grid-line positions.** On QET's default border (17 × 60 px columns,
  8 × 80 px rows, 20 px headers), an element whose cell a cross-reference prints (an interruption point
  `next_report`/`previous_report`, a master or a slave) that lies exactly on an inner column or row line
  (x = 25 + k × 60, y = 25 + k × 80 px: the border sits 5 px from the scene origin) moves 2 nm left or up. QET's `convertPosition` takes the ceiling, so a line
  point belongs to the left column or upper row; EW's cell rule (floor, unchanged) would put it right or below, and
  2 nm leave the cell rule's 1 nm snap window, so the printed cells are QET's.
  Every other element (terminals, simple elements) keeps its exact position. This is an exception to FR-IMP-008's
  "0 nm deviation": the connection points of a nudged element are 2 nm off the 10 px grid (in the reference file:
  three masters and one slave). The proper home is a cell-boundary policy for imported pages (ADR-0027 open item), which would remove the nudge. The row "element position" of `crates/ew-import-qet/README.md` states the
  same rule.
- **Pages.** Imported pages keep the 10 px grid permanently (ADR-0027, FR-PAG-021); "convert to native grid" is the
  previewed command of FR-PAG-022 (1.x).
- **Frame.** Each distinct border becomes a frame form of the project's import library (§4.9). The form carries the
  page-to-sheet transform of QET's fit-to-page export (FR-PAG-029; on QET's default border scale 0.986282578, offset
  9.79 mm), so printed geometry matches the reference to ±0.2 mm (REF-096).

---

### 4.3 Element definitions → symbols and function definitions

Each distinct definition (identified by the hash of its canonical XML, not by its `embed://` path) becomes a
`SymbolDef` and a generated `FnDef` in the per-project "QET import" library (FR-SYM-018, FR-IMP-008), with source
element name, path, UUID and definition version as provenance. The library's `library_id` is the target project's
ID (`ImportCx.project`, §2.1) and its human key is `qet-import` ([05](05-project-format-and-persistence.md) §13.2).
Built-in FnDefs that the importer references live in the system library, whose `library_id` is
`ew_schema::starter::LIBRARY` (7202fd8b-1024-4bf1-9e99-670772eaa589, the starter content's ID). The importer has exactly one built-in FnDef,
the interruption point `ew.fn.ip` of §4.6 (`starter::FN_IP`), referenced by key in `LibRef.extra["key"]` with
`version_req` `^0.1` (`starter::VERSION_REQ`), because every starter function definition is version 0.1; `ew.gen.ip` is its
*symbol* key ([07](07-symbols-parts-macros.md) §2.3). The generic black box of EW-IMP-119 is not
a built-in: it is a generated FnDef in the "QET import" library at version 1.0.0, referenced with `^1`. Primitives
map one to one:

| QET primitive | EW geometry | Notes |
|---|---|---|
| `line x1 y1 x2 y2 end1 end2 length1 length2` | path segment with end markers | `simple`, `triangle`, `circle`, `diamond` ends; unsupported ends expand to polygons |
| `rect x y width height rx ry` | rectangle, optionally rounded | corner radius semantics per definition version (verify on corpus) |
| `ellipse x y width height`, `circle x y diameter` | ellipse, circle | `x y` is the bounding box's top-left |
| `arc x y width height start angle` | elliptic arc | degrees; 0° at 3 o'clock; positive span counter-clockwise on screen, so a point is `(cx + rx·cosθ, cy − ry·sinθ)`; the importer keeps `start` and `angle` as given, and lowering in `ew-scene` evaluates the arc with `ew-dl`'s deterministic trigonometry |
| `polygon x1 … xn y1 … yn closed` | polyline or closed polygon | |
| `text x y text size\|font color rotation` | static symbol text | font per §4.10; `rotation` is Qt's clockwise-positive angle and becomes the negated `Angle2` (counter-clockwise positive, [03](03-domain-model.md) §3.1; ADR-0005 amendment 2); `x y` is the first line's baseline (QET subtracts the font's ascent for the top of its text document, whose first baseline is that ascent lower), so EW, which aligns the bottom of the text's box, stands it 0.293 em (the bundled family's descent) lower; a text of several lines becomes one text per line, each 1.15 em lower in the text's own frame |
| `dynamic_text`, legacy `input tagg` | text slot | `label` → tag slot, `function` → function-text slot, `CompositeText` → slot with a format; `UserText` → static text. In a project the placed elements print their own texts instead (§4.4.2) |
| `terminal x y orientation name type uuid` | connection point | index = definition order; designation = `name` (empty allowed); direction from `orientation`; `type` Generic, Inner or Outer kept |

- **Style mini-language.** `line-style: normal|dashed|dotted|dashdotted`, `line-weight: none|thin|normal|hight|eleve`
  (the historical tokens `hight` and `eleve` are accepted), `filling`, `color`, `antialias`. Line weights map to the
  house widths (`thin` 0.5 px, `normal` 1 px).
- **Anchor policy.** QET-import symbols keep QET's **element-origin** cross-reference anchor, so imported xref
  texts equal QET's, "3-B0" included (FR-IMP-038; [07](07-symbols-parts-macros.md) §3.6). The MF-13 quick fix sets
  the anchor policy "graphics" (a named anchor on the bus-port graphics), which gives "3-B2".
- **Function semantics.** The generated FnDef is resolved in this order:
  1. a `symbol-map.csv` row (element UUID or path → EW FnDef and CP map; generalises FR-IMP-018), built for
     coils, contacts, terminals and interruption points (§4.17);
  2. `kindInformations`: master `coil|protection|commutator|plc`; slave `simple|power|delayOn|delayOff|delayOnOff`
     with `state` NO, NC or SW → a contact FnDef with `PassThrough{opens: true}` between its two terminals; terminal
     `generic|fuse|sectional|diode|ground` with function `phase|neutral|PE`;
  3. a shipped table from QET collection category paths (`10_electric/…/switches`) to EW categories;
  4. otherwise a generic black box (EW-IMP-119).

  Rules such as EW-POT-013 need `PassThrough` semantics, which steps 1–3 must resolve for a switch's element.
- **Link type.** `simple` and `master` form devices; `slave` forms auxiliary functions; `next_report` and
  `previous_report` become interruption points (the built-in FnDef `ew.fn.ip`, §4.3, not generated); `terminal`
  forms terminals; `thumbnail` becomes a graphic (FnDef category `Graphic / QET / Thumbnail`, a black box, without a
  finding).
- **Coverage of the pilot corpus.** [23](23-symbol-coverage-pilot-corpus.md) maps the 81 element types of the pilot's
  three real projects to the starter set (§4.3 step 1) and lists the mapping entries for the project importer (M-01 to
  M-09): 37 types resolve to starter symbols, 29 stay black boxes (EW-IMP-119), 2 are replaced by report blocks or
  layout, 13 wait for new symbols.

---

### 4.4 Element instances → devices, functions, representations

| QET element information | EW target |
|---|---|
| `label`, `formula`, `prefix` | `TagSpec` by the inference of §4.4.1 |
| `plant` (=), `location` (+), or prefixes inside `label` | structure nodes through `ew-rds` lenient parsing (FR-NUM-015) and the alias hints; spelling variants ("+JE2416" against "+J-E-2416") stay two nodes with a merge proposal (FR-PRJ-018) |
| `function` | `Device.function_text` (MlText neutral) |
| `description`, `designation` | `function_text` when `function` is empty, else user property `description`; `designation` → part type text |
| `manufacturer`, `manufacturer_reference`, `supplier`, `machine_manufacturer_reference`, `quantity`, `unity` | `PartUse` + project `PartCopy` (source "QET element information", no catalogue link); manufacturer through the master's aliases (FR-PRT-002) |
| `auxiliary1..4` with `*_auxiliaryN` | further `PartUse` entries (D-04: the second part 550208 of -201U1) |
| `voltage_rating`, `current_rating` | `tech_data` as `Quantity` when parsable ("40A" → 40 A), else text with EW-IMP-008 |
| `width`, `height`, `depth` | part dimensions when the part is created from element information |
| `plc_type`, `plc_address`, `plc_function`, `plc_comment`, `plc_crossref` | pending channel binding (P7) |
| `exclude_from_bom` | device scope "supplied by others" (FR-DEV-037) |
| `comment`, `notes` | `Comment` anchored to the representation |

#### 4.4.1 Tag inference

1. **Formula present.** Translate the QET formula into an EW tag format (§4.8) and evaluate it for the element. The
   result is what QET printed. If the stored `label` differs, the derived tag wins and EW-IMP-101 reports the stale
   copy (D-01: stored -4W1, printed and derived -3W1).
2. **No formula.** Parse `label` leniently. Test the product part against candidate rules with the folio label of
   the master's page: R1 `{page}{letter}{counter}` (-101F1), R2 `{letter}{page}{counter}` (X1031 on folio 103),
   R3 `{letter}{counter}`. The leading aspect sign is added by the formatter and is never part of a format
   ([03](03-domain-model.md) §8.2).
3. **Choose rules.** The project rule is the candidate explaining most labels; a letter class gets its own rule when
   another candidate explains all its labels (X → R2). The choice is an `AdoptRule` proposal, accepted by default.
4. **Conforming** labels become derived `TagSpec{letter, counter, scheme}` with the counter taken from the label.
   **Non-conforming** labels become `FrozenTag{text, sign}`, with `sign: Hidden` when the label has no product sign
   (S51), and EW-IMP-103 lists them (FR-NUM-010).
5. The project importer applies one rule, R1 on the element's own
   folio: a label of the form `-{page}{letter}{counter}` whose page is the folio of the element, whose letters are
   upper case and whose counter has no leading zero becomes a derived `TagSpec{letter, counter}`. Every other label
   stays a `FrozenTag` that prints the source text unchanged, so a label EW would print differently (`-101K01`,
   `-101f1`) is frozen rather than re-derived, and EW-IMP-009 never fires for a tag. EW-IMP-103 lists the frozen
   labels, strips excepted. A label that several devices carry stays a `FrozenTag` on each, though it fits the
   rule: two derived tags with one counter would print as two numbers, where QET prints the label twice;
   EW-IMP-103 says so, and EW-RDS-004 reports the duplicate. A structure prefix (`+…-X1`) is kept in `Device.extra`. An element
   without any label gets `TagSpec{letter: prefix attribute, else "A", counter: none}` (numbered on import), so
   the device keeps an identity, and is reported once per folio as EW-IMP-122. Its tag is not printed, because QET prints none: the
   symbol's tag slot is hidden and only an element's own label text shows it (§4.4.2). Hiding the slot was taken
   over a hidden slot per representation or a placeholder tag because the symbol of a project import shows no
   text of its own anyway, so nothing is needed per unlabelled element.
6. **Identifier set.** EN 81346-2:2009 single letters unless the letter analysis is explained only by IEC 60750, in
   which case that set is proposed (ADR-0026). The importer never switches to IEC 81346-2:2019; that is the
   previewed renumber of FR-NUM-014.

#### 4.4.2 The texts an element prints

QET prints a placed element's own texts, not its definition's: since 0.7 an element carries `<dynamic_texts>`
(each bound to user text, an element information or a composite text, with its own position, font and rotation),
and an older one `<inputs>` with the values of the definition's texts. The definition's dynamic texts are only the
templates new elements copy.

A definition read for a project gives a symbol whose texts from `dynamic_text` and `input` print
nothing: user texts and untagged inputs are left out, and every text slot is hidden, aligned `baseline_left` and named
by its import style (§4.10). Each placed element then prints its own texts, each where QET draws it:

- **Where.** QET places a text item's box by its top-left corner and turns it about that corner. EW stands the
  text on the baseline of a line's left end ([09](09-reports-forms-output.md) §4.8), so it keeps its place in any
  typeface: the corner plus QET's 4 px text margin, plus 0.905 em down to the first line's baseline (Qt's ascent of
  Arial and Liberation Sans), plus 1.15 em per further line (Qt's line spacing). A text of several lines becomes one
  text per line, so that its lines keep QET's spacing. The offset turns with the text's rotation rounded to a
  quarter turn, then with the element. A text with a fixed width (`text_width` above 0) QET 0.100 lays out centred
  in its box, whatever its `Halignment` (which only keeps the box's anchor when the text changes), so its lines are
  centred on the box's middle: a centred free text, or a slot aligned `baseline_centre` that the import adds to the
  symbol. An element's static text (`text`), which QET writes by its baseline, is aligned `baseline_left` at QET's
  point.
- **What.** A user text prints as written, an element information its value (a slave's label is its master's,
  as QET links them), a composite text its format with each `%{name}` replaced by that information. Qt rich text
  prints its plain text, one line per paragraph, in the size of its first span or body (§4.13).
- **As a slot.** The label and the `function` text show the symbol's tag and function-text slot through a
  slot override (position, angle, shown). The angle is the one QET draws the text at on the sheet, its own rotation
  plus the element's (a text QET keeps upright with `keep_visual_rotation` holds the element's turn taken back in its
  `rotation`, so it comes out upright): EW prints an override's angle as it is ([06](06-schematic-editor.md) §2.5), so
  a turned element keeps QET's look while EW's own symbols keep their texts upright; so the printed tag stays the device's live tag. That needs a device
  whose tag prints the label unchanged (a derived or frozen tag without a structure prefix), a symbol slot of
  that kind in the text's own style, and a text of one line.
- **Else as a text of the element's symbol** (a free text `qet:el:<key>#text<n>` whose `of` names the element's
  representation, [06](06-schematic-editor.md) §3.8): it prints the value as QET printed it, moves with the element
  and goes with it.
- An element without its own texts prints none; an empty text prints nothing.

The element information beyond the label stays in `Device.extra["qet.info"]` (EW-IMP-008), as before: it prints
through the texts above, but parts, user properties and comments from it are not built (§4.16).

---

### 4.5 Master and slave → main and auxiliary functions

- A master element and its linked slaves (`links_uuids`) form **one device**: the master's function is `Main`, each
  slave an `Aux` function with its own representation. On the reference set, -201U1's master sits on 201-E1 and its
  slaves on folio 3 (bus port) and 106-G3 (power connector), which print "(201-E1)".
- A slave's own `label` is ignored when it equals the master's; a different one is reported (EW-IMP-009).
- An orphan slave becomes a device whose single function is promoted to `Main` (EW-IMP-116); EW's "one main
  function per device" invariant holds by construction.
- More slaves than the definition's `max_slaves` are kept and reported.
- The cross-references print where QET draws them, through a cross-reference slot the import adds
  to the symbol (hidden, in QET's 5 pt) and shows on each representation. A master lists its slaves' positions
  one per line directly under its first label text (QET's `snapto="label"`); a linked slave prints its master's
  position under its label text, or above it for `xrefpos="AlignTop"`, `slave_offset` px away, as the `<xref>` of
  its master's type says, its first line on QET's baseline (the cross-reference item's 4 px margin and Qt's ascent of
  its 7 px text below the label's box; Qt's descent above the box for a slave above it), so it stands where QET prints
  it in any typeface. The text is centred on the middle of the label's box, as QET centres it: half a fixed
  `text_width`, else the 4 px margin and half the label, counted at 0.55 em a character (the import has no
  text widths, so this middle is an estimate). A master's contact list (`displayhas="contacts"`) has its texts
  28 px right of that middle, where QET prints them after the contacts it draws. What differs from QET: a master
  prints positions only, without QET's contact symbols, terminal names and prefixes, and its rows stand one
  line of the 5 pt style apart (about 7.7 px) where QET's stand 10 px apart: both need a drawing of the list that
  the scene does not have. `displayhas="cross"`, `snapto="bottom"` and a slave beside its label print as this
  list under the label, each reported once per type (EW-IMP-009).

---

### 4.6 Folio reports → interruption points

QET pairs reports explicitly, one to one; EW pairs IPs by `(name, scope)` in star or chain mode (03 §10). The
importer reconstructs QET's pairs exactly:

1. **Collect pairs** from `links_uuids` between `next_report` (→ `direction: Source`) and `previous_report`
   (→ `Target`). An unlinked report becomes an unpaired IP (FR-CHK-017).
2. **Name** each pair from the first non-empty of: the source's `label`, the target's `label`, the `function` of
   the conductor at the source. Both ends get the same name; a differing target name is kept as an alias and
   reported (EW-IMP-115). A pair without any name gets `IP-{page}-{cell}` and a warning.
3. **Group** pairs by `(name, scope key)`. A single-pair group needs no mode.
4. **Multi-pair groups** use chain mode with `IpSpec.sort = 2i, 2i+1` for pair `i`, so EW's chain order
   reproduces QET's pairs. If the target project requires star mode, the group is reported (EW-IMP-115) with the
   quick fix "rename the extra pairs".
5. **Verify** (pass P3): the derived pairs must equal the QET pairs; every difference is an error finding.

   Step 2 takes one name per linked pair from the first non-empty of: the
   source's first `UserText` dynamic text, the target's first `UserText`, the source's `label`, the target's
   `label`. The text comes first because it is the name QET prints: a report's label text prints its partner's
   position, so its `label` information can hold anything; both ends get that name and the other end's
   differing text is kept as an alias and reported in one EW-IMP-115 entry that quotes both texts. A pair without any
   name gets one synthesised name `IP-{folio}-{n}` for both ends (EW-IMP-115). Several pairs that share a name get
   `IpSpec.sort` 2i and 2i+1 and an EW-IMP-115 saying the project needs chain mode. The conductor `function` of the
   specified step 2 is not read. When pairs
   share a name the import sets the project's `ip_pairing` to chain mode, so that EW pairs exactly QET's links.
   An unlinked report keeps its own text as its name only when no other report has that name; otherwise it
   gets a name of its own (`IP-{folio}-{n}`, EW-IMP-115) and its text prints as written, so it stays
   unpaired, as in QET.

   *Printing.* A report keeps its definition's drawing. The text QET binds to its `label` (which QET
   fills with the partner's position by the report formula), as the label information or as a composite text of
   `%{label}` alone, shows the symbol's cross-reference slot, in that
   text's own style and place; a text that is the pair's name shows the function-text slot, which prints the
   interruption point's name. The import adds both slots to the symbol, hidden, and shows them per
   representation. The position stored in the file is not read: EW derives it.
6. **External ends.** A report whose partner is missing but which sits inside or next to a dashed external box with
   a target text gets the proposal `IpSpec.external = Some(ExternalRef)`, the reference parsed from the box text
   (REF-018, FR-PRJ-021); such a point prints the reference ("+J-E-206 / JE-N1000-03"). Each one drawn as an unpaired QET report gets the
   proposal; one drawn as a plain element or text is listed for a manual conversion (which applies depends on the
   file, PQ-02).

Cross-reference texts are recomputed, never imported (FR-IMP-009).

---

### 4.7 Conductors → connections

**End resolution**, per end independently (FR-IMP-007):

1. If `element{i}` is present, look the element up by UUID. Unique → done; shared by copy-paste → candidates on
   this folio only (EW-IMP-120), decided by the route position of the conductor's end (§2.3: linear in the number of copies and conductors; ambiguous pairs beyond 16 probes bind the first copy with a finding).
2. If `terminal{i}` is a UUID, find it among the candidate's terminals. If the definition has no terminal UUIDs or
   none matches, fall through.
3. If `terminal{i}` is an integer, use the folio's map `terminal id → (element, definition index)` built from
   `<element><terminals><terminal id>`.
4. If both forms resolve and disagree, a unique UUID result wins and the disagreement is reported.
5. Unresolvable → the conductor is dropped (EW-IMP-003) and counted in `lost_connections`. A conductor whose two
   ends resolve to the same CP is dropped as well.

**Mapping.**
- The connection is `Connection{kind: Wire, a: Cp{function, idx}, b: Cp{…}, page}`; `idx` is the definition index of
  the terminal.
- **Route.** Stored `<segment>`s are replayed from end 1 into `route` points. Conductors without segments get the
  path QET computes (ported profile algorithm), stored explicitly, so EW draws them exactly as QET did
  (FR-IMP-043).
  `<segment>`s are read directly under `<conductor>` (QET's form) and inside a `<segments>` wrapper.
  Like QET, the importer sets stored segments aside when they do not end within 1 px of end 2 on both axes. QET's
  automatic path is QET's `Conductor::generateConductorPath`, rewritten: each terminal is extended 10 px in the
  direction it faces (its definition `orientation` turned with the element) and rounded onto the 10 px grid, and the
  extensions are joined from left to right by one corner or a two-corner bridge on the 10 px grid. A corner keeps
  an end's coordinate when it lies in the same QET px as that end, else it takes the nearest whole px. A stretch
  that runs out and back over itself, and a vertex in the middle of a straight run, are removed. A terminal's page
  position can be up to 2 nm off its whole px (§4.2), so the segment at an end may slant by that much. A
  connection keeps a route only when that route is one `connection.route` accepts: every coordinate on the half
  grid (5 px) or equal to an end's, and no segment over another. A stored corner between the half grid's lines (QET
  stores corners at any px, and at fractions of one) moves to the nearest of them, at most 2.5 px, so the stored path
  keeps its way. A stored path that fails this is replaced by the
  automatic path with EW-IMP-009. When the automatic path fails too, the connection stays unrouted with
  EW-IMP-009 and the reason. Two terminals that touch get the segment between them, or no route at one point.
- **Properties.** A non-empty `num` → `NumberSpec::Frozen` (FR-IMP-012, 08 §5.3), an empty one → `Unassigned`; `conductor_color` and `conductor_section` →
  overrides, pruned by pass P2; colour text through the alias table (BR → BN proposed, §4.12); `function` and
  `tension_protocol` → `PotentialDef` candidates (pass P1); display attributes (`numsize`, `onetextperfolio`,
  text position) → wire-number slot overrides; `cable`/`cable_uuid`/`cable_slot` (0.200) → `Connection.kind =
  Core(conductor)`.
- **Junctions.** QET has no conductor-to-conductor joints. A junction element (one terminal, three or more
  conductors) becomes `Junction{style: Dot, wiring: Star}` with the conductors in document order and EW-IMP-118
  (08 §12 item 14). Bevelled corners where conductors overlap (D-20) are separate terminal-to-terminal conductors and
  stay separate connections, so the topology is right. QET files its own junction elements (the T
  junctions of its connections category, `114_connections`) as terminals; such an element with three wires or more
  becomes a junction where its strokes meet, its origin, and the stroke to each of its terminals becomes part of the
  wire that ends there. One with fewer wires stays the element it
  is drawn as, because a junction of fewer dissolves (normaliser #8).

**Nets and potentials** are never imported: QET has no such objects. EW derives nets, signals and potentials from
the imported connections, IP pairs and bridges (03 §9.3). Wire properties then come from the profile chosen in the
import wizard, with provenance (D-08).

---

### 4.8 Formulas and cross-reference formats

The importer reads each project's own formulas and never assumes QET's defaults (C-15, FR-IMP-010). Every QET
variable has an EW xref-format token (FR-XRF-003):

| QET | EW token | Note |
|---|---|---|
| `%F` | `{page}` | folio label: "102" |
| `%f` | `{page_index}` | folio position: folio 102 is the 6th page, so "6" |
| `%l`, `%c` | `{row}`, `{column}` | of the target cell on the target page's own frame |
| `%M`, `%LM` | `{function}`, `{location}` | = and + aspects |
| `%id`, `%total` | page index, page count | |
| `%prefix` | letter | tag formats |
| sequential parts of `<element_autonums>` / `<conductor_autonums>` (unit, ten, hundred, folio-scoped) | counter | variable spelling (`%sequ_n` style in QET's source) to verify on the corpus |
| `%wf %wv %wc %ws` | potential function, voltage, colour, cross-section | wire-number formats |
| `%{key}` | the EW field that element-information `key` maps to (§4.4), else a user property | |

- `<newdiagrams><report label>` becomes the IP xref format; `<xrefs><xref type>` per master type gives the main and
  auxiliary formats (`master_label`, `slave_label` "(…)" → aux wrap, `…prefix` → main marker such as "∞ ",
  `displayhas cross|contacts` → list or contact image, `snapto`).
- If the master types use different formats, the majority becomes the project format and the others are reported
  as print-equivalence deviations (EW-IMP-009) unless the target supports per-category formats.
- `formula` is kept next to the resolved label, and "convert to EW numbering rule" is offered (FR-IMP-010).
- A project still on the default `%f-%l%c` prints "6-A1" for folio 102, exactly as QET did; EW-IMP-117 offers the
  switch to `{page}`.
- The import writes the project's `xref_format` when the file has a `<report>` or an `<xrefs>`
  (else the profile's format applies): `ip` is the report label, `aux_to_main` the `slave_label` and
  `main_to_aux` `{list}` with one item per line (matrix 1 per row, no limit), as QET draws its contact lists.
  QET's variables are EW tokens as written; what a file leaves out is QET's default (`%f-%l%c`,
  `(%f-%l%c)`). EW has one item format for interruption points and master lists, so a `master_label` other
  than the report label is reported (EW-IMP-009), and so is every type whose formulas differ from the ones most
  types share.

---

### 4.9 Folios, sections and title blocks

- **Pages.** `<diagram>` → `Page{folio, kind: Schematic, title, structure, form}` in `order`. The folio label is the
  evaluated `folio` attribute; QET's default `%id/%total` would yield "6/18", so the importer takes `%id` as the
  label and reports it.
- **Sections.** Proposed from numeric folio hundreds (1xx → section 1, 2xx → 2, 9xx → 9). Separator pages such as
  100, 200 and 900 stay drawn pages; converting them to generated dividers is a later, previewed step.
- **Title-block templates** (FR-IMP-015) become title-block forms. Rows are absolute px. Columns are px, `tN%` (of the
  total width) or `rN%` (of the width left after absolute and `t` columns); they are resolved to nm for the
  diagram's actual width, producing one form per distinct width. Cells keep alignment, `hadjust` (shrink to fit),
  font size, label and value; `<translation lang>` values become `MlText`. Logos (`png|svg`, base64 or XML) become
  content-addressed assets; SVG is parsed with usvg without external resources (FR-SYM-025). The reference's raster
  logo stays raster until replaced in the company basic project.
- **Variables → bindings.**

| QET | EW binding | Per-folio conflict handling |
|---|---|---|
| `%title` | `page.title` | per page by design |
| `%author` | revision author | differing values: EW-IMP-104 |
| `%date`, property `saveddate` | `project.created`, `project.edited` | latest value wins; reported |
| `%indexrev` | current revision label | differing values: EW-IMP-104 (D-05) |
| `%plant`, `%locmach` | `page.function`, `page.location` | through structure nodes |
| `%folio`, `%folio-id`, `%folio-total`, `%auto_page_num` | folio label, page index, page count | derived |
| `%previous-folio-num`, `%next-folio-num` | Prev, Next | derived |
| `%filename` | proposed as `Project.master.drawing_id` (FR-PRJ-002) | — |
| `%version` | generator version (derived; QET's version is kept in the import run) | — |
| `%{var}` (custom) | project property if equal on all folios, else page user property | EW-IMP-104/105 |

- The project importer lifts a title-block value or folio property to
  the project only when it is equal on **all** folios, in one linear pass; every other value stays on its page and
  differing `author`, `date` and `indexrev` values and properties are EW-IMP-104. The project property `savedfilepath` (a local path of the author's
  machine) is dropped and reported with EW-IMP-004; the import runs under the default `Limits` of §2.1.
- **Lifting.** EW has no per-page revision, author or description text (ARCH §11). For each value the majority goes
  to the project or revision record; the per-folio values stay listed in the finding with "lift to project
  property" and "create revision history" quick fixes. A deviating description becomes `Page.description_override`
  with EW-IMP-105 (D-06). The importer creates one open revision labelled with the natural-sort maximum ("V2.0" on
  the reference); the history V1.0 → V2.0 is the MF-04 quick fix.
- **Parties.** Cover texts and title-block cells naming parties stay text; EW-IMP-106 compares the title-block
  "Customer" value with the cover's and proposes role bindings (FR-PRJ-003, D-30).
- **Frame.** QET draws a diagram's border 5 px right of and below the scene origin
  (`Diagram::margin`), its 20 px header bands inside it, then `cols × colsize` and `rows × rowsize`. The importer
  keeps the scene origin as the page origin, so imported positions stay on the 10 px grid, and makes one frame form
  per distinct border in the project's import library: the header band counts from the origin (25 px) and the
  outer rectangle starts at the margin (the frame member `ew.frame.margin`), so a cell is the cell QET prints;
  columns are numbered from 1, QET's default (QET keeps the start in its application settings, not in the
  project); `displaycols` and `displayrows` both `false` hide the bands (one hidden band alone is reported and both
  show). The sheet is A4, landscape unless the drawing is taller than wide, and the page maps to it as QET's PDF
  export maps the scene: the border's corner on the sheet's corner, scaled to fit the border and title block one
  pixel wider and higher. A title block QET draws at the right is drawn below. Counts and sizes outside 1 to 1000
  and 1 to 10 000 px keep QET's default border (reported).
- **Title block.** The diagram's template becomes a title-block form as wide as the border; equal forms
  are one copy, so the forms differ only where the folios' values differ. Columns resolve to whole px; QET's
  `rowspan` and `colspan` count the cells beyond the first. Every cell's edges are drawn once; the outline is the
  frame's. A field prints `label: value` when it shows its label, in the style `qet.<pt>` (a project text style
  of `pt × 4/3` px em, ADR-0032), shrinking to a 0.5 mm cap height where QET adjusts (`hadjust`). Of the
  translations the `en` one is taken, else the first. `%title`, `%folio`, `%folio-id`, `%folio-total`,
  `%previous-folio-num` and `%next-folio-num` bind to `page.title`, `page.folio`, `doc.page_index`,
  `doc.page_count`, `page.prev` and `page.next`. QET's own variables whose values the import writes into the
  project's and the page's fields bind to those fields, so a value changed in the program prints: `%plant`
  and `%locmach` to `page.function` and `page.location`, the page's structure, a node of each code (`=P01`, `+AQ60`;
  one atomic code after the sign, else the value stays text); and, when every folio has one value, `%indexrev` to
  `revision.current.label` (an open revision of that label), `%author` to `revision.current.author.name` (the designer,
  a party of the role author), `%filename` to `project.drawing_id`; `%projecttitle` binds to `project.description`
  and `%saveddate` (`-eu`, `-us`) to `project.edited`, set from the ISO `saveddate` or `saveddate-us`. A variable of the
  person's own (`%{customer}`) and a value the folios do not share stay text: a field of one's own is not built.
  Every other variable is filled with the diagram's value, from its
  title-block attributes (`date="null"` is empty), then the project's title (`%projecttitle`) and properties, then
  the diagram's properties, and an unknown variable stays as written, as in QET. `%projectpath`,
  `%projectfilename` and `%savedfilepath` print nothing (EW-IMP-004). A logo stored as base64 (QET's form for a
  PNG) becomes an image primitive of the form, as large as its cell allows at the picture's proportions and
  centred, its bytes a project asset through the same path as a drawing's images (§4.13); a logo stored as XML
  (an SVG) is the same, its asset the SVG element as the file writes it, drawn as vectors (a form names an asset
  by hash alone, so the export takes an image's format from its bytes). A logo the asset rules refuse is
  reported (EW-IMP-004) and its cell stays empty. A diagram without an embedded template gets the shipped
  `ew.titleblock.qet-a4` (QET's default template).

