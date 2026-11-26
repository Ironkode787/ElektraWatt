# Symbols, parts and macros: behaviour, algorithms and library governance

A chapter of [Symbols, parts and macros](07-symbols-parts-macros.md), whose table says where each section lives.

---

## 3. Behaviour and rules

### 3.1 Embedding and the copy life cycle

**Embed closure.**
- `PlaceSymbol`, `AssignPart` and `InsertMacro` embed the item together with its `LibRef` closure: the symbol's
  FnDef, the part's accessories, template FnDefs and manufacturer, and the macro's symbols and parts.
- Within a project a `LibRef{library, item, version_req}` resolves to the embedded copy of `(library, item)`.
  If that copy's version does not satisfy `version_req`, the embedding command stops and proposes "update from
  library" instead of embedding a second copy silently (§2.1, keeping two versions).
- `library.embed` takes the
  copies themselves; `ew-api`'s `embed_command` builds them from library references with their closure.
  `part.assign` carries the missing closure in its `copies` and embeds it in its own transaction, reusing a held copy
  of the same hash. The tables
  bootstrap and the parts import write one project-local manufacturer copy per manufacturer name, and part references
  name it with no `key` (a legacy `key` is still read). A part chosen from a system library in the Parts row is one
  `part.assign` that carries the closure; from the picker it is a `library.embed` before the placement, two undo steps.

**Unused copies stay until purged.**
- A copy whose last reference disappears is **not** deleted implicitly, so undo and basic-project palettes stay
  intact.
- `library.purge_unused` lists unreferenced, unpinned copies in a preview and removes them as one transaction; the
  release dialog offers it too ([05](05-project-format-and-persistence.md) §13.2).
- An info finding counts the unused copies.

**Where a part lives, and what a person may change.** A person needs to know three things of a part, and the
window says no more (§5.8). The same holds for a symbol.

1. **A part he makes is his.** It is saved to **My parts**, the writable library of the user tier (key
   `user-parts`; it holds his symbols and their function definitions beside his parts), and it is there in his next
   project. Parts asks nothing about where to keep it. Only where the window has no user folder is a new part a part
   of the open project, and its sheet says so.
2. **A project carries the parts it uses.** A project takes a part when a device uses it (the embed above), so it
   opens, prints and checks on a PC that has none of his parts. A part the project holds that no library the reader
   can read has is **only in this project**: it is the project's own (its `library_id` is the project's), it is
   changed in place by `part.edit_copy`, any member, and **Keep in my parts** writes it into My parts and gives the
   project's devices that part of his (`part.assign` with `replace`, one previewed step), so that it is his here
   and in the next project. That is how parts travel to a second PC or a colleague: with the project, kept there
   with one click, one part or all of the project's.
3. **A part that is not his is never changed.** A starter part, a community pack's and a library he cannot write
   are the same on every PC that has them, and that is what the lock protects: it is the one lock, and it says so.
   **Make my own from it** makes a new part of his with everything the other has, under a maker and a number he
   gives; where devices of the open project are the other part, `part.assign` with `replace` gives them his, in one
   previewed step. `part.edit_copy` refuses a library's copy in words. No command makes a fork of a library's part
   inside a project; a project that holds such a fork (its `meta` names what it was forked from) reads it as a part
   only in that project, and the Libraries navigator still says what it was made from.

**Correcting a part of his own.** Saving a changed part of My parts writes the library item's next version and, in
the same step, brings the open project's copy to it by "update from library", whose card names the fields that
changed and the devices that follow. A person sees no version number to manage: the sheet says when the part was
last changed. Another project that holds an older copy keeps it until it is opened, where the part's sheet, the list
and the Libraries navigator say "Changed in My parts since this project took it" and Update… shows the same card:
nothing in a project changes unseen. A part only in the project is corrected in place, with the same card.

**Update from library** (FR-SYM-016, FR-PRT-014, FR-MAC-010) is a `Cost::Bulk` command previewed on a fork. The
preview shows:
- the field diff per copy;
- the rendered before/after pages for symbols;
- the BOM and wire-list `diff_query`;
- every representation or device touched.

**The update is classified by semantic version, and the classification is checked, not trusted (§4.5):**

| Change | Example | Update behaviour |
|---|---|---|
| patch: graphics, texts, metadata, with CP and anchor offsets unchanged | the lamp symbol redrawn; a clamp range given to EW-GEN-TRM-1 | applies; pages re-render |
| minor: additive (new slot, arrangement, variant, section, accessory) or a moved anchor | the variant Blue added to a terminal | applies; a moved anchor lists every changed xref text in the preview |
| major: CP removed or re-indexed, FnDef changed, function template changed | a contactor's contact 13-14 renamed 23-24 | needs a CP mapping (by index, then designation) in the preview; unmapped CPs with connections **block** the update until mapped or the connections are removed explicitly |

### 3.2 Symbol rules

1. **Grid.** CPs of native symbols lie on the native connection grid (ADR-0027, proposed 4 mm, not yet decided).
   Graphics use a 0.5 mm and text slots a 1 mm fine grid (ADR-0027 recommendation 1, FR-SYM-009). The editor
   refuses an off-grid CP at save (FR-SYM-009). A symbol is placeable only on pages
   of its own grid: QET-import symbols keep their 10 px grid (`7_937_500/3` nm) and stay on QET-grid pages until
   the page conversion of v1.x (FR-PAG-022).
2. **QET-grid edition of the starter set.** A project migrated from QET sits on the QET 10 px grid, and its
   QET-import symbols are exchanged for starter symbols on QET-grid pages. The starter set therefore ships a second, **generated**
   edition with `grid = QET 10 px`: every CP offset keeps its count of grid steps (`k·g` → `k·7_937_500/3` nm),
   graphics and slot positions scale by the same ratio and round to the nm, and text heights stay style-driven. The
   scaling is `v · 7_937_500 / 12_000_000` rounded with ties toward zero; 0.5 mm becomes 1.25 px, so the edition's
   graphics lie on no whole-pixel grid, and its lint requires instead that every graphic, slot and link-point
   coordinate equals the scaled value of its native twin, while every CP lies exactly on the QET 10 px grid. Its pens are QET px, not scaled house pens (§2.3). The
   edition has its own item IDs (key suffix `@qet10`), is linted like the native edition and is never drawn by hand (FR-SYM-030; ADR-0027 recommendation 8). The full 1.0 library gets no such edition.
3. **CP direction.** Every CP has a direction (N, E, S, W), which autoconnect and routing use. A CP must lie on the
   symbol outline or on a line end.
4. **Representation type.** A symbol has exactly one `rep_type`. Placement offers only the symbols of the function's
   FnDef whose `rep_type` the page type allows (FR-SYM-006).
5. **Anchor.** Every symbol declares its anchor policy explicitly; the drawing of a new symbol proposes `GraphicCentre`.
   Interruption points anchor at the arrow tip, the -201U1 bus port at its graphics ("3-B2"), the CPX power box at
   its left edge ("106-G3"). The xref cell comes from the anchor, never from the
   bounding box (D-14). The resolved anchor offset is snapped to the symbol's connection grid at save (ties toward
   the origin), because the integer cell lookup of [06](06-schematic-editor.md) §5.1 requires anchors on the grid.
6. **Text readability.** Text primitives and slots are never mirrored and never turned by the symbol's variant:
   turning or mirroring a placement leaves every text at the angle it has in the unturned symbol (owner,
   2026-11-15: "it should always remain upright"), made readable from the bottom or the right of the sheet
   ([06](06-schematic-editor.md) §2.5). The anchor moves with the symbol and the alignment is chosen as §4.1 decides
   for all eight variants, so the text stays on the same side of the symbol and of its connection point. Turned a
   quarter, a symbol's slot texts are placed anew round its body (§4.1).
7. **Exchange** (FR-SYM-022) maps CPs by `fn_cp`, then by designation. The preview lists unmapped CPs, and a CP that
   carries a connection must be mapped.
8. **Linkages** (FR-SYM-014) are derived lines between `link_points` of the functions related by a `DeviceLink`
   or by one device's actuator and contacts. A symbol without a link point gets its anchor as the fallback. A
   `SymbolDef` never draws a mechanical linkage in its graphics: pole to pole, pole to handle, actuator to contacts
   and actuator to process valve are all derived. A multi-pole symbol therefore declares a link
   point per pole and one for its actuator: `ew.iec.switch-2p-manual` declares `pole1`, `pole2` and `handle` at
   (11, 6) mm, and the derived chain runs pole1 → pole2 → handle. The one exception is the internal pole linkage
   of the MCB, which is part of the MCB graphic (0.13 mm, dashed 4:2).
9. **Licence.** Every symbol has an SPDX licence. Shipped symbols carry `CC-BY-SA-4.0 WITH
   KiCad-libraries-exception` if counsel confirms that the SPDX-listed exception covers control-cabinet projects
   and machine files, otherwise a project-specific exception (`AdditionRef-…`, SPDX 3.0 syntax, verify)
   (ADR-0024; [19](19-open-source-governance.md) LR-07). They carry no IEC description text (FR-SYM-019,
   NFR-LIC-07).
10. **Pole-count families (generated)**. One family source generates the
    concrete pole-count variants at content-build time; there is no run-time parameter and no new record kind.
    - *Source.* `content/families/<family>.json` declares the pole template (one pole's graphics, CPs and slots, and
      the pole pitch in connection-grid steps), the actuator and internal-linkage geometry, the pole counts to
      generate, the key pattern (`<base>-<n>p`, e.g. `ew.iec.switch-disconnector-4p`) and the FnDef template
      (`PassThrough{opens: true}` per pole, `ew:poles` = n, link points `pole1` … `pole<n>` and `handle`).
    - *Expansion.* `cargo xtask symbols-qet10` expands each family into ordinary SymbolDef and FnDef documents before
      sealing, then generates their `@qet10` twins as for every symbol (rule 2). The generated documents are committed;
      the lint regenerates them and fails on any difference, as for the `@qet10` edition.
    - *Items.* Each generated variant is an ordinary library item with its own key and item ID, linted like a
      hand-drawn item. Members of a family carry the keyword `family.<base>`, so pickers can group them.
    - *Existing keys.* The hand-drawn keys `ew.iec.mcb-1p`, `ew.iec.mcb-1pn` and `ew.iec.switch-2p-manual` stay
      hand-authored; a key moves into a family only when the generator reproduces its sealed document byte for byte
      (content hash).
    - *Unchanged.* ADR-0021 is unchanged: importers and `symbol-map.csv` rows reference concrete keys. FR-SYM-027
      (user-authored parametric families in the symbol editor, 1.x) is unchanged and may later reuse the source format.
    - *Trade-off.* The library holds one item per pole count (more items, more twins) in exchange for no schema change,
      no run-time geometry generation and unchanged import and placement paths.

### 3.3 Part assignment and selection rules

`AssignPart{device, part, variant, qty, mode: Keep | Align}` (FR-PRT-015, FR-DEV-015):

1. **Embed.** The part's closure is embedded, its accessories (§2.5) with it.
2. **Device with no functions yet** (part-first placement, IDEA-016). The template functions are created unplaced,
   with IDs `v5(device_id, template_index)` ([03](03-domain-model.md) §4.1; ADR-0004 amendment 2). The
   function matching the chosen symbol is placed (FR-DEV-017, FR-UI-017).
3. **Device with existing functions.** Template functions are matched to existing ones by FnDef category, then by
   CP designations (natural, normalised).
   - Each match sets `Function.def` to the template FnDef copy and drops CP overrides made redundant by the
     pattern.
   - Unmatched existing functions stay and are reported by FR-CHK-036 if they exceed the part's contacts.
   - Unmatched template functions are created unplaced.
4. **Derived texts.** Part text slots (manufacturer, part number, type), technical data defaults and the
   description default refresh in the same commit (FR-DEV-016). The device's own `tech_data` wins over the part's.
5. **Quantity** is in the part's order unit: the sample's cable -101W1 uses EW-GEN-CBL-17 in metres. A cable's conductors are
   generated from the part's cable-core section (FR-CBL-004).
6. **Several part uses per device** are allowed (a valve terminal and its bus node). Each use is matched separately.
   The first use that provides the main function is the device's main part.
7. **Variant.** `part.assign` takes `variant`, a key the part holds (another is refused in words), and writes it
   into the part use's reference (`PartRef.variant`). Properties ▸ Parts ▸ Variant and the strip editor's Variant
   cell change it alone, with `fields.set` of the device's `parts`; the device keeps its functions and connections.
8. **Accessories.** Nothing is written on the device: its accessories are derived (§2.5). Replacing a part drops the
   device's accessory choices that the new part does not name, in the same step.

**Replace part** (FR-PRT-023) is `AssignPart` over a set of devices, as one previewed transaction. **Configured
products** (FR-PRT-006) assign like parts; their components feed the component-mode BOM (FR-REP-005).

The command is **`part.assign`** `{devices, part,
copies?, mode?, from?, qty?}` with `mode` `append` (the default: a further part use), `replace_all` (the part becomes
the devices' only use) or `replace` with `from` (each use of `from` swapped, its quantity kept); it is Interactive below
50 devices and `Bulk` from 50. It carries the library documents it needs in `copies` and never reads a library, so it
replays without one; it embeds what the project lacks, reuses a held copy of the same hash and refuses a held copy of another version ("update
from library first"). A device's main part is the part use whose template provides the main function (rule 6). The Properties Parts row sends `part.assign` (`replace_all`) when its new value is one use of a system
library's part the project does not hold, else its own `fields.set`. Rules 2 and 3's matching is not changed by the
command. A function made from a template records its entry index (`Function.template_entry`, an
optional member appended within 0.1 under ADR-0011 amendment 1), and the CP derivation reads that entry, falling back
to the first entry of the function's FnDef when the member is absent or names no such entry; an entry whose `cps` list
as many pattern CPs as its FnDef has CPs pairs them by position, so each function keeps the FnDef's CP indices (a
symbol's `fn_cp`, the FnDef's internal links) and prints the pattern's designations. A part may therefore template
several functions of one FnDef: the generic relay and contactor template their contacts.

**A part whose type is not known**.
The drawing shows that a part belongs to a device and nobody knows which yet. The model's placeholder part serves:
a part copy numbered `TBD` (`PLACEHOLDER_PART_NUMBER`, `Part::is_placeholder`; FR-CHK-034). No new member and no new
record kind.
- *Command.* `part.placeholder {devices, description, mode?, from?}` makes a project-local part copy numbered `TBD`
  with the person's words as its description, no manufacturer (its reference names the project library and no item)
  and the kind `terminal` when every device is a terminal, else `device`, and gives it to the devices as
  `part.assign` with that `mode` and `from` does, in one undo step. A project-local placeholder of the same kind and
  description is reused, so four grey terminals given "Grey terminal" in one step or in four share one copy. An empty
  description is refused in words.
- *Window.* In the part dialog (Properties ▸ Part ▸ Choose…, a strip's terminals without a part), Type not known yet…
  opens a field for what the part is and Use as placeholder (or Enter) gives it; the placeholder is a held part of the
  project from then on, so the dialog lists it among the project's parts for the next device. Type not known yet… is also in the Parts row's … menu beside Choose part… and in the command search (under
  placeholder, stand-in, not known, undecided, TBD: the glossary's Stand-in part), each opening the part dialog of the
  selection with the field open; Make variable in that menu says on hover that it is a macro's variable, not a stand-in
  part. New part… is in New station…'s split on the PLC tab and opens Parts with the kind of a station's card (§5.5).
- *What prints.* The parts list has its row: no manufacturer, part number TBD, the description, the count of its
  devices; the terminal diagram's Part column prints TBD.
- *The check.* The device has a part, so EW-CORE-041 is silent. EW-CORE-043 reports the placeholder as info online (a
  decision the person recorded, not a gap) and as an error in release mode, an open item that blocks release
  (FR-CHK-034). EW-CORE-042 does not ask a placeholder for a manufacturer.
- *Replaced in one step.* A real part chosen in the part dialog for a device that carries a placeholder replaces it on
  every device that carries it (`part.assign` with mode `replace`, `from` the placeholder, over its devices); a script
  sends that `part.assign`. The placeholder copy then has no user and stays in the project until it is purged.
- *Copied.* A copy of pages and a paste carry the part use as they carry any: the devices of the copy name the same
  placeholder.
- *Not built.* A placeholder of a cable sold by the metre (it is a piece); a placeholder offered from the library.

### 3.4 What a part lacks

**A part knows what it lacks, and every feature says it the same way.** What is lacking is **derived, never
stored** (FR-PRT-021), from the one declaration of a part's fields (§2.5): each field names the features that read
it, and for each what the feature does without the datum.

| Without the datum, the feature | Example | Shown as |
|---|---|---|
| **cannot use the part** | an enclosure without a width and a height cannot be chosen for a layout; a rail without a profile cannot be drawn | a warning at the field and under Ready for; the feature's own refusal or finding in the same words |
| **passes the part by** | a pin without a clamp range is skipped by the check that a wire fits its clamp; a cable without an outer diameter gets no gland proposed | a note at the field; the check or the list says once which parts it passed by |
| **takes a default** | a duct without an inner size is filled as 85 % of its outer size | a note at the field that names the default |
| **only prints it** | a description, a colour | nothing while it is empty |

**One list, three readers.**

- *The place* shows what the open part lacks at each field, in the contents list and under Ready for (§5.7), with
  what the datum is for, and the list's filter Lacks something finds such parts.
- *A feature* asks the declaration instead of testing a part's members itself: the Enclosure… offer, the rail and
  the duct tools, the placement on a layout, the treatment of a wire end, the choice of a ferrule, a marker and a
  gland, the hole table. Its refusal or its finding is the declared sentence with its arguments (the part, its
  devices, the fields lacking), so a finding in Messages and the line under the field read alike.
- *An outcome* made that way carries the part and the field as facts (`PartLack`), and the window offers **Edit
  part…** on it, which opens Parts at the field (§5.2).

A sentence is one text with arguments: the part's word for its kind, and the fields lacking as a list of their
words, joined by the reader's language ("This cabinet cannot be chosen for a layout until it has a width and a
height"). One reader named by several fields says its sentence once.

**A field nothing reads is not asked for.** The declaration refuses a field without a reader; a member of the
record that no feature reads is declared as stored and not asked for, with the reason, and the sheet shows a value
it holds under About ▸ Also stored (a maker's file may have brought one). A datum is asked for from the day
something reads it.

**Confidence.** A section marked `unverified` or `test` (§2.5) is not lacking: its findings carry the marker of
non-verified part data, and the release report lists it (FR-PRT-031).

### 3.5 Macro creation and insertion rules

**Save as macro** (FR-MAC-001, FR-MAC-002):
- The selection is copied into a fragment with local IDs, together with its embedded closure and its structure
  references.
- **Structure.** References equal to the page's structure become `PageRelative`, so the macro rebinds wherever it is
  inserted (FR-MAC-003).
- **Tags.** Frozen tags are cleared (S51 becomes a derived tag), unless the user marks a device "keep tag"; a kept
  tag makes every insertion a tag conflict, and EW-LIB-012 warns about it in shared libraries.
- **Open ends.** Connections to objects outside the selection are dropped; their CPs become the macro's open ends.
- **Insertion point.** The user picks it; it is snapped to the macro's grid.

A page macro's pages named by folio (`macros.save {pages}`) need no selection and no marked page,
and without a kind they make a page macro. A copy of pages ([06](06-schematic-editor.md) §3.6) starts from the same
fragment of the pages, widened by the cables, bonds and connections a second installation keeps together; the page
macro itself does not carry them. A page macro's records keep their positions as the pages held them, a symbol off
the grid too (it was refused as off the grid, though nothing moves it).

**Variables are created by picking.**
1. Select an object property in the property grid.
2. Choose "make variable".
3. Name it, then fill value sets in a table. CSV import and export of that table arrive at 1.0 (FR-MAC-014).

**`InsertMacro{macro, variant, at, orient, value_set | values, instance_id}`** is the pipeline of §4.6:
- one undoable transaction after a ghost preview (FR-MAC-005);
- `instance_id` is minted by the caller, so replaying from the journal reproduces the same IDs;
- page macros propose folios after the target page (FR-MAC-002, [06](06-schematic-editor.md) §5.6).

**Conflicts listed before commit:**
- duplicate frozen tags, and interruption-point names that would pair with existing points;
- PLC addresses already in use: manual ones, and derived ones that collide because a value set puts a module on
  an occupied slot of the same station. Inserting the folio-203 typical a second time with value set 1 doubles
  slots 1 and 2 and lists the eight bound addresses Q0.1, Q0.3 … Q1.7 (FR-MAC-005);
- duplicate process tags (exact duplicates at MVP; EW-PRC-001 with its scheme check from 1.0);
- occupied CPs under autoconnect;
- a macro grid that differs from the page grid (refused);
- a `rep_type` that is illegal on the page (refused).

### 3.6 Imports into libraries

All importers are pure functions from bytes to `ew-schema` records (FR-IMP-001). **In the window an import is a
door into Parts** (§5.9), not a place of its own: Import… on the Parts screen takes a file and goes on by what it
is, and File ▸ Import… goes on to it for a file of parts. What a file brings stands in the Parts list as a set,
unsaved, each row with what it is to what he has (new, changed, the same, refused with its reason) and with what it
lacks; Save writes the set into My parts (§3.1), and a project takes a part when a device uses it.

| Source | Door | Mapping | Rules |
|---|---|---|---|
| A table of parts, CSV or XLSX | Import… in Parts | the column map: each column of the file to a declared field of a part (§2.5), chosen by the field's word; saved per source and read again for that source's next table | dedupe on (manufacturer, normalised part number); every row new, changed, the same, duplicate or refused with its line (FR-PRT-016) |
| EPLAN parts, EDZ (7-Zip or ZIP) or parts XML | Import… in Parts | `*.part.xml`: `P_ARTICLE_PARTNR`, `_MANUFACTURER`, `_ORDERNR`, `_TYPENR`, `_DESCR1/2` (`de_DE@…;en_US@…;` → `MlText`, FR-I18N-008); `<functiontemplate>` pins (`connectionDesignation`, `connectiondescription`, `terminalNr`, `functiondefinition`) → the pins and what the part is drawn as, through the `functiondefcategory/group/id` mapping table; sizes; free properties; `partsmanagement` with variants and accessory lists (FR-IMP-025/027/028) | the **terms notice** before the first EDZ import of a session (FR-IMP-026); `.ema` geometry never reproduced; `.e3d` ignored; pictures stored only in the user library; source file hash and import date recorded |
| An outline drawing, DXF | Import outline… on the open part's front | the drawing's closed lines as the part's front outline, in millimetres (FR-IMP-035) | replaces the part's outline; Remove takes it out; the same reader as the command line's |
| A symbol file: a QElectroTech element, an EPLAN symbol file, a DXF block | Import… in Parts, on the Symbols shelf | a drawing with its import report; connection points snap to the connection grid (FR-IMP-034) | an EPLAN drawing is saved only once its review is acknowledged |
| QET project elements | the QElectroTech project import | `.elmt` definitions embedded in a `.qet` become symbols in the project's "QET import" library, on their native grid with source element name and UUID (FR-SYM-018, FR-IMP-008) | never leaves the project unless the user makes his own from it; keeps QET's element-origin anchor so imported xrefs equal QET's; the library's `library_id` is the project ID and its key `qet-import` ([05](05-project-format-and-persistence.md) §13.2) |
| A QET element collection | `ew lib import qet-elements <library> <dir>` | the `.elmt` files below a folder become symbols and function definitions of a writable library, `meta.licence = LicenseRef-QET-elements`, the flag `foreign_terms`, the source path in `meta.sources`, the folder's `ELEMENTS.LICENSE` named by its SHA-256 (FR-SYM-029) | QET's terms allow use in diagrams freely and redistribution under CC-BY 3.0 with a no-ML-training clause ([oss-landscape](../research/oss-landscape.md) §2.6). They are incompatible with EW's CC-BY-SA-4.0 packs, so such items are never bundled, never merged into an EW pack and never used for model training (NFR-LIC-06) |
| `symbol-map.csv` | the QElectroTech project import | QET element UUID or path → EW symbol key and CP map; generalises the pneumatic table of FR-IMP-018 | used by the symbol exchange (FR-SYM-022) |
| BMEcat 2005, ETIM xChange | specified, not built | product identification, ETIM or ECLASS feature blocks, order and price details (element names to verify against the specification) | FR-PRT-020 |

**The table's columns are the declared fields.** A column of a table goes to a field of §2.5's declaration: every
field that is one value (the identity, a size, how it mounts, a kind's own data, a named technical value) and, by a
name he gives, any other technical value or free property. The window offers the fields by their words under their
groups; a field that is the same datum in several kinds (Width, Height, Depth) is offered once and goes to the field
of that word the row's kind has. A column no field keeps is not imported, and the mapping says so beside it. The
session hands the importer its columns with each one's kind of value, so the importer holds no list of a part's
members. What a table cannot say (a part's pins and what it is drawn as, its clamps per pin, its holes) is entered
on the part's sheet, and the set's dot says which parts still lack it; or it is taken from a part like it: a column
mapped to **Like** names a part by its number, and a new row starts as that part and takes its own columns over it.

**The table reader** is `ew-import-catalog`: the column map (`member ← header`, saved as TOML per source key), a
CSV reader (UTF-8 with or without a BOM, comma or semicolon from the header line, RFC 4180 quoting), an own bounded
XLSX reader, and rows into part copies with item IDs `v5(lineage(catalog, library), parts|<maker key>|<pn_norm>)`,
deduplicated on manufacturer and normalised part number, so reading the same table again changes its parts in
place. Manufacturers resolve through the libraries' master list (every tier's, the starter parts' "Generic"
included), an alias reported EW-IMP-007, an unknown name EW-IMP-005, "A / B" split into two flagged candidates,
unmapped columns EW-IMP-004. **Export as a table…** in Parts writes the selected parts with the same columns, so a
table exported and read back unchanged brings every row as the same (§5.9).

**The command line** reads the same sources into a library or a project: `ew lib import parts` and `ew import
edz|parts-xml --library` write a library and refuse changed content under a held version unless `--bump`, taking a
stale lock over only with `--take-over`; `ew import parts --map` embeds a table's parts in a project with
project-local manufacturer copies; `ew import edz --project` previews the update of a project's part copy from a
file (14 §5.2). The tables of a project's bootstrap have their own `parts.csv` with columns of their own
([14](14-interop.md)); a table with exactly those headers is read by that reader in the window too.

**EDZ and parts XML** are read by `ew-import-eplan` into part drafts and placed by `ew-import-catalog`'s one placer
(the dedupe on manufacturer and `pn_norm`); the pins, descriptions, sizes, free properties and provenance are read,
accessories and further variants kept beside the part, the picture and the drilling positions reported (14 §5.2).

**The EDZ terms notice reads, in substance:**
- files from the EPLAN Data Portal may be subject to terms that restrict their use to EPLAN products;
- EW imports only factual part data from files the user obtained;
- the user is responsible for complying with those terms.

The notice is shown once per session and logged in the import report. The network sandbox job asserts that an EDZ
import opens no connection (NFR-PRIV-01).

**Import dates are command parameters.** The UI fills `as_of` with today's date. This keeps commands deterministic
and replayable, and entities carry no writer timestamps (ARCH §7.3).

### 3.7 Distributor connectors (bring your own key)

Prices, stock and datasheet links from distributors come only through **connector plugins** (WASM, v1.x,
FR-EXT-019). Examples: Mouser, DigiKey, Nexar/Octopart, Farnell.

**Rules:**
- **Capabilities.** A connector holds a network capability granted at install time (NFR-SEC-07) and the user's own
  credentials (an API key, or an OAuth2 token for DigiKey, [interop-formats](../research/interop-formats.md) §9),
  kept in the OS credential store and read through the host capability `secret-get(name)`, which is granted per
  plugin at install and never readable by other plugins (ADR-0020 amendment 4; 1.x, before the first connector).
- **When it runs.** Only on an explicit action ("fetch prices for this BOM"), as NFR-PRIV-01 requires.
- **Where results go.** Results land in the user library's L4 section, with `source = connector id` and `as_of`, and
  are cached no longer than the provider's terms allow.
- **What never happens.** Results are never written into company or community libraries automatically, never
  shipped, and never fetched in the background.
- **Projects.** A project sees prices only through an explicit "update from library".

### 3.8 Licence and provenance rules

| Content | Licence | Shipped by EW |
|---|---|---|
| EW-drawn symbols, frames, forms, macros | CC-BY-SA-4.0 with the design exception | yes |
| Project-authored part data (starter catalogue facts), examples | CC0-1.0 | yes |
| Manufacturer packs | the vendor's explicit redistribution licence, per record | only with that licence |
| ETIM model (English) | `ODC-By-1.0`, with attribution | yes, as a data pack |
| ECLASS dictionary, IEC 60617 database, QET element collection, EDZ/Data Portal content, vendor STEP/DXF/images | — | **never** (NFR-LIC-06) |
| Converted QET elements, EDZ imports | the source's terms (QET: CC-BY 3.0 plus no-ML clause; EDZ: Data Portal terms) | stay in user or company libraries |

**The design exception** (KiCad's text if counsel confirms it fits, ADR-0024; see [19](19-open-source-governance.md))
waives Article 3 of CC-BY-SA-4.0 (attribution and ShareAlike) for designs, drawings, PDFs, reports, machine files
and projects that use or embed library items. NFR-LIC-09 states the same for EW outputs. Redistributing the
library itself, modified or not, stays under CC-BY-SA-4.0.

Every shipped record carries a
licence and a source: `ew-library`'s test `content_audit` reads every library document under `content/` and every copy
under `fixtures/sample/sample.ew/lib/` as bytes and fails, naming the file and the item, on a missing or empty
`meta.licence`, a licence other than its kind's row above (the record's, not its file's), or an empty `meta.sources`;
under `content/` the file's row in `ci/licence-map.toml` must allow it too. The CSV bootstrap (`ew import csv`) writes
`CC0-1.0` on its part and manufacturer copies; a user's `ew import parts` writes none, the licence being the user's to state.
`content/README.md` lists each shipped content set (folder, kind, licence, source, `verified`, the package that made
it), which the test keeps complete: RC-07's evidence.

---

## 4. Algorithms

### 4.1 Variant derivation and on-grid invariance

Variant `v ∈ {R0, R90, R180, R270, M0, M90, M180, M270}` maps a point by an integer matrix:
- R90 is `(x, y) → (−y, x)` in sheet coordinates with y down;
- M mirrors x before rotating.

Geometry, CP positions and CP directions transform exactly in `i64` nm. Arcs swap their sweep direction under
mirror. Text anchors transform; text glyphs do not.

**Text under a variant** (decided by the project owner for all eight variants; revised 2026-11-15: a text stays
upright). A text primitive or slot is laid out in the symbol frame with its anchor, rotation and alignment, and
placed in three steps:
1. The anchor transforms with the variant's matrix like any point.
2. The glyphs are never mirrored and never turned by the variant: the text renders at its own rotation, made
   readable (a rotation in (90°, 270°] is turned by 180°; `upright_angle` of `ew-scene`). A rotation that a person
   set (a representation's slot override with an angle, a free text) is kept the same way.
3. The alignment keeps the text's side: the direction from the anchor into the text's box (opposite to the
   alignment; a text aligned middle right lies left of its anchor) is mapped by the variant and read back in the
   printed text's frame; each component of at least half the length keeps its sign, a smaller one becomes centre or
   middle (`placed_align` of `ew-scene`). A text whose angle a representation's override sets keeps its own
   alignment instead (only the readability turn flips it): that angle is the sheet's. A connection point's designation that this brings centred above or below
   its point while the point's line leaves sideways runs along the line, away from the body, instead (`slot_align`).

So a tag left of its body that the turn brings above the body stands centred above it, and a designation beside a
connection point stays beside it on the side away from the line. For a text without its own rotation, aligned
middle right:

| Variant | A R0 | B R90 | C R180 | D R270 | E M0 | F M90 | G M180 | H M270 |
|---|---|---|---|---|---|---|---|---|
| Rendered as | R0 | R0 | R0 | R0 | R0 | R0 | R0 | R0 |
| Alignment | middle right | bottom centre | middle left | top centre | middle left | top centre | middle right | bottom centre |

Under A, C, E and G this is the earlier flip (left ↔ right under the plain mirror E, both under the half turn C).

**Texts of a symbol turned a quarter** (owner, 2026-11-22, from a sketch). Under B, D, F and H the texts that stand
under each other beside an upright symbol would stand side by side on one line and run into each other. So a symbol
with connection points turned a quarter places its slot texts that print level anew round its body (`slot_places` of
`ew-scene`), so that none stands on another or on the symbol:
- Lying flat (no pin leaves up or down), its tag stands centred above the body, and its other texts (technical data,
  cross-reference, function text, PLC address, part data) centred below it, line under line in the order they stand
  down the unturned symbol. A symbol drawn lying that a quarter turn stands up has all of them right-aligned left of
  its body from its top, the tag first.
- The body is the bounds of the graphics and of the static texts' anchors, and a step above or below the point of
  each pin's designation that prints there. Each text stands a quarter step clear of it and a line (a step and a half,
  the starter symbols' pitch) from the one before; only the texts that print take a line.
- A pin's designation keeps its place at its pin (step 3 above). A text a person dragged keeps its place and the
  alignment of its slot (`slot_align`), the alignment its position was stored with, so a project made before these
  places prints it where it did; a drop stores the anchor that gives that alignment the box the ghost showed
  (`SlotTextBox::drop_at`), so the text lands where it was dropped. A slot whose angle an override sets keeps its
  turned point.
  A symbol without connection points (a drawn box) keeps the steps above.

The drawing's strip "Turned and mirrored" (§5.6) and a page show the same places; on a page they are the texts' first
places, which the text placer leaves only where something stands there ([06a](06a-drafting.md) §5.2).

**Invariance.** Every CP offset and the anchor offset from the symbol origin are multiples of the connection grid
`g` (§3.2 rules 1 and 5), and the matrix entries are 0 or ±1. Every transformed offset is therefore again a
multiple of `g`. So a symbol placed with its origin on the grid has all CPs and its anchor on the grid in all eight
variants, with no rounding and no float comparison.

**Cost.** `symbol_geometry(symbol, variant)` is memoised at library durability (ARCH §6.2) and tessellated once per
(symbol, variant) for instanced drawing (ARCH §8.2).

### 4.2 CP instantiation and pin mapping

`function_cps(f)` builds the CP list. For each FnDef CP `i`:

```text
designation(i) = override(f, i) ?? part_pattern(template_fn(f), i).designation ?? fndef.cps[i].designation
```

It also applies `unused` and alias overrides ([03](03-domain-model.md) §7).

**Pin map.** A representation's `pin_map` maps symbol CPs to function CPs and is derived by default:
`symbol.cps[k].fn_cp`.
- An explicit `pin_map` is stored only when it deviates, for example a symbol reused for a part whose pattern
  orders its pins differently.
- Black boxes take all CPs from the part pattern and lay them out on the outline in pattern order. This is how the
  EDZ pin list of 2904602 becomes the -102G1 box.

### 4.3 Part search, ranking and compatibility

**Normalisation.** `pn_norm(s)` upper-cases `s` and removes spaces, `-`, `.`, `/` and `_`:
- "SK 3110.000" becomes "SK3110000";
- "5SY4 204-7" and "5SY4204-7" both become "5SY42047".

The stored part number stays verbatim. Duplicate detection uses (manufacturer, `pn_norm`).

**Index tables** (per user, rebuilt from libraries): the SQL schema `SCHEMA` of `crates/ew-library/src/index.rs`.
A part is a row of `part`, its words a row of the full-text table `part_text` (every language of its description),
its normalised number a row of the trigram table `part_pn` (substring hits on numbers) and its template functions
rows of `part_fn`. Symbols, FnDefs and the other library items are rows of `item`, with their words in
`item_text`. A table of part properties (`Dec3` as a scaled integer) is specified; not built.

**Ranking.** Results are ordered by tier, then by score:
1. exact `pn_norm` match;
2. `pn_norm` prefix;
3. trigram match (queries of three or more characters; the trigram tokenizer cannot match shorter ones);
4. FTS5 `bm25` over `part_text`, with column weights in column order pn 10, type 6, manufacturer 3, aliases 3,
   descr 1, keywords 2, class_names 1.

Inside a tier, favourites and parts "used in this project" come first. Ties break by natural sort (`ew_natural`).

**What the part dialog searches and how it ranks.** A person types what a part is for ("sensor",
"normally open contact") as often as its words, and "sensor" found nothing while "normally open contact" ranked the
contactor first (its description names an auxiliary contact). The dialog searches what a part already carries; no
part gets new data and no list of synonyms is kept.
- *Its words:* the part number (tiers 1 to 3), and the type, the manufacturer, the description in every language and
  the keywords (tier 4; a held copy's description in every language too). A row shows the description in one
  language, the window's, else English, else the first (`ew_derive::text_in`), never every language joined;
  the device browser and Properties' list of library parts show it in the project's language by the same order.
- *What it is for:* the window reads from the text the function categories and contact kinds it names. A category is
  named when every typed word starts one of its words, or (from four letters) stands inside one, its words being its
  `function-category-<key>` text in English and Dutch and its key split at its capitals ("sensor" names Position
  sensors, Pressure sensors, Temperature sensors and `FluidSensor`); a contact kind likewise by its
  `symbol-kind-<kind>` texts ("normally open contact", "maakcontact" name `no`). A part is for it when its own
  category's first level, or its main function's FnDef's first category level or contact kind, is one of them. Such a
  part is found in tier 4 when its words are not (or keeps its better tier).
- *Order:* the fit to the target's function (with one), the tier, then inside a tier the parts found by what they are
  for before those found by their words only, then held before not held, favourites and recent parts, the full-text
  score and the natural part number.
- *No row is chosen for the person:* the list opens and refreshes with no active row; a click or Down chooses one,
  Choose and Enter wait for it (Enter took a first row nobody had looked at).

**What fits** (FR-PRT-015, FR-DEV-033). When a part is chosen for a device, an offer fits when its template has a
function that its main function can become (the same FnDef, else the same first two category levels and contact
kind), with every connection point the device has wired (its designation, natural-normalised), and each such clamp
takes the wires on it: their effective cross-sections ([08](08-connections-terminals-cables.md) §4) lie in its
`csa_min` … `csa_max`. A clamp without a range takes any wire, and the offer says that its clamp sizes are not
given (its row's badge "clamp sizes not given", `PartOffer::clamps_unsized` from `ew_derive::fit_report`). An offer
that does not fit says why in one sentence, the first reason found in this order: its function, both kinds as the
function kinds' navigator names them ("A part of another kind: A1A2 coil, not 1P MCB"; without both definitions at
hand, "its function is not the device's"), a pin it lacks ("It has no pin 4; -101F1:4 is
wired"), a clamp's size ("Its clamp 2 takes 0.5 to 1.5 mm²; wire 102.1 is 2.5 mm²"). For several devices (a
strip's terminals without a part, a selection of several) an offer fits when it fits every one; where it does not,
its reason says for how many and names the first it does not fit ("Does not fit 3 of 12; -X1:4: Its clamp 1 takes
0.5 to 1.5 mm²; wire 7 is 2.5 mm²"). The order of offers puts fitting ones first; Only parts that fit hides what fits
none of the devices, and the rest stand greyed with their reason. The fit reads
the part itself (a library part's document, a held part's copy; `ew_derive::part_fit`) and the device's wires through
`wire_props_of`; nothing is stored.

### 4.4 Quality level derivation

`part_quality(part, profile)` takes the requirement profile of the part's category: an ordered list of levels, each
with field predicates such as `tech has ew:rated_current` or `mech.whd present`. It returns the highest level whose
predicates hold, together with the list of missing fields of the next level.

It is a cold query in projects and is materialised as `part.quality` in the library index. A profile change
re-indexes only the parts of that category.

### 4.5 Update-from-library classification

For each embedded copy `c` with a newer library version `n`:
1. If `sha256(c) = sha256(n)`, nothing changes.
2. Compute the record diff. Classify it as **major** if any of these hold:
   - a CP disappears, or an index changes its designation;
   - `fndef` or `category` changes;
   - a template function is added to or removed from a part in use;
   - a variable is removed from a macro in use.

   Otherwise classify it as minor or patch, whatever the declared semver says. A semver lower than the observed
   class is a library lint error (EW-LIB-007).
3. For major changes, propose a CP mapping: by index if the designation is unchanged, else by designation. Every
   connected CP without a mapping blocks. The accepted mapping rewrites `cp_overrides` and the connection endpoints
   `Cp{function, idx}` of the affected functions (ADR-0004: CP identity is tied to `idx`).
4. Run the update on a fork: the copy file is replaced in place (same copy ID, §2.1), so referencing records change
   only where step 3 remaps them. Show `diff_model` (entities), `diff_query` (BOM, wires, xref texts) and the
   rendered pages that changed.

`classify_update` in `ew-commands` gives patch, minor or
major with the field diff: major by step 2's list (a CP gone or re-designated, a FnDef or category change, a template
function added or removed by FnDef reference, a macro variable removed), patch when only data that no derived structure
or geometry reads differs (`meta`, a part's manufacturer, numbers, description, technical data and free properties, a
symbol's name and keywords, every manufacturer member), else minor; the major rules apply whether or not the item is
in use. **`library.update`** `{copies, cp_maps?}` (`Bulk`, so always previewed) replaces held copies in place and maps
CPs by the explicit pairs, else by index with the same designation, else designation k-th to k-th; it rewrites
connection, potential, bond and channel endpoints and CP overrides, and refuses a connected CP left without a CP (an
overridden one does not block), a downgrade and a copy the project does not hold. `ew-api`'s `update_from_library`
plans it from a preview on a throwaway engine. It does not yet embed the new version's missing closure.

### 4.6 Macro insertion pipeline

```mermaid
flowchart TB
  A["InsertMacro{macro, variant, at, orient, value_set, instance_id}"] --> B["Pick variant by page rep_type<br/>(refuse if none)"]
  B --> C["Apply value set: typed coercion per VarDef,<br/>formulas (rhai, pure) after literals, include flags"]
  C --> D["Remap ids: new = v5(instance_id, local_id),<br/>rewrite every reference inside the fragment"]
  D --> E["Rebind structure: PageRelative → target page,<br/>Code → node by (aspect, code) or alias, unknown → listed"]
  E --> F["Transform geometry by at + orient (§4.1),<br/>check fragment grid = page grid"]
  F --> G["Clear tag counters, embed library closure<br/>(one copy per library item, §2.1),<br/>remap channel bindings, MacroInstance record"]
  G --> H["Normalisers: online numbering, CP instantiation,<br/>autoconnect hints on open ends, junction clean-up"]
  H --> I["Validate (strict) → ghost preview with conflict list"]
  I -->|accept| J["Commit one transaction"]
```

**Determinism.** Value-set application is deterministic. The values are typed. Formulas run in sandboxed rhai with
no I/O and see only the other variables and the insertion context (`page.folio`, `instance.index`). The order is
the topological order of variable dependencies, and a cycle is a save-time error.

**Placeholder groups.** A `PlaceholderGroup` whose `include` variable is false is removed before the ID remap, with
every connection to it. Folio 206 uses this to omit the second module, and folios 204–206 to omit the field
devices that only folio 203 draws (§9.3).

`macro.insert` takes `orient` (default r0, left out of the log at r0) and turns the fragment about its anchor before the move: representations compose it, route vertices, junctions, page items and box outlines turn as points in whole grid steps, text and arc angles turn as directions, markers modulo 180°, and route ends follow their CPs. A page macro refuses a non-default orient at `/orient`. A record a `DeviceRef` variable names stays referenced, a child tag's counter stays when its parent comes along, and a module landing on an occupied slot lists the addresses it doubles in the conflict list.

### 4.7 Library index build

On start-up and on a watcher event:
1. Compare each library's manifest generation with the index.
2. For a changed library, diff the per-file SHA-256 of the manifest against the index's file table.
3. Re-parse only the changed shards and upsert their items in one SQLite transaction per library.
4. Rebuild the FTS rows of changed items.

A full rebuild is the same loop with an empty file table. The index stores `(library_id, generation)`, so a stale
index is detected, never trusted. The target is NFR-PERF-21.

The index is one SQLite FTS5 database per user in the
cache directory (rusqlite bundled), rebuilt by `ew lib index --rebuild`; search ranks by the specification's rank tiers
and tie-breaks, and `Session::part_search` marks the hits the project holds. The compiled-in libraries
are indexed under a pseudo path. IX-1 to IX-8 run in `crates/ew-library/tests/registry.rs`; the 500,000-part test IX-9
is `#[ignore]`d (not measured, the owner's rule).

---

## 6. Library governance and the community library

### 6.1 Repositories and roles

The community library lives in its own repositories, separate from the application, with independent releases:

| Repository | Content | Licence (REUSE) |
|---|---|---|
| `elektrawatt-symbols` | `ew-iec60617` and `ew-iso1219` packs, FnDefs, frames and forms | CC-BY-SA-4.0 with the design exception |
| `elektrawatt-parts` | manufacturer master, starter catalogue, community part packs per manufacturer | CC0-1.0 for project-authored facts; vendor licence per record for vendor packs |
| `elektrawatt-macros` | typicals (motor starter, PSU block, valve module) and basic-project examples | CC-BY-SA-4.0 with the design exception |

**Roles:**
- **Library maintainers** (P6) have merge rights per pack.
- **Symbol and FnDef logic changes** need two reviewers, one of them a maintainer. A FnDef change alters nets and
  checks.
- **Cosmetic and part-data changes** need one reviewer.
- Contributions come under the DCO, with no CLA (NFR-LIC-03).

### 6.2 Contribution review

```mermaid
flowchart LR
  PR["Pull request<br/>(DCO signed)"] --> L["ew lib lint<br/>(EW-LIB-001 … 012)"]
  L --> R["ew lib render: PNG of every changed symbol<br/>in 8 variants + macro thumbnails (CPU renderer)"]
  R --> D["Data diff: changed fields, derived quality level,<br/>semver class vs declared (§4.5)"]
  D --> P["Provenance gate: source per record,<br/>licence per record, no forbidden content"]
  P --> H["Human review: previews posted to the PR,<br/>2 reviewers for FnDef or CP changes"]
  H --> M["Merge → ew lib pack: release<br/>(semver, manifest, SHA-256)"]
```

**Library lint rules.** These are the EW-LIB rules of [15](15-checks-revisions-collaboration.md) §4.11, which
registers them like every rule (codes, Fluent messages, rule pages). They run only in the library linter (the
symbol and part editors' lint panels, `ew lib lint`, this pipeline and `ew lib pack`), never in a project scheme:

| ID | Rule | Severity |
|---|---|---|
| EW-LIB-001 | CP off the symbol's connection grid (`SymbolDef.grid`); CP index not unique; CP not on outline or line end | error |
| EW-LIB-002 | Anchor policy missing; anchor outside the symbol's graphics or not on the connection grid | error |
| EW-LIB-003 | Required text slots missing for the category (tag; CP designations for multi-CP symbols) | error |
| EW-LIB-004 | `fn_cp` not in the FnDef; FnDef CP not drawn by any multi-line symbol | error / warning |
| EW-LIB-005 | Licence or source missing; licence not in the pack's allowlist; item flagged `foreign_terms` | error |
| EW-LIB-006 | IEC description text or QET element copied (geometry fingerprint matches a QET collection element) | error |
| EW-LIB-007 | Declared semver lower than the observed change class; content changed under an already published version | error |
| EW-LIB-008 | Part below "basic"; manufacturer not in the master; duplicate (manufacturer, `pn_norm`) | error |
| EW-LIB-009 | Rating or unit inside the description ("10A C", "50hz") (D-18, D-35) | warning |
| EW-LIB-010 | CP-pattern designations that differ from the template FnDef's only in spelling ("Out1" vs "Out 1"); template CP index not in the pattern | warning / error |
| EW-LIB-011 | Image, STEP or DXF asset without an explicit redistribution licence | error |
| EW-LIB-012 | Macro fragment with dangling references or a grid different from its declared grid (error); frozen tags kept with "keep tag" (warning) | error / warning |

**The QET fingerprint check (EW-LIB-006).** CI computes normalised geometry fingerprints of the public QET element
collection at build time, for comparison only, and never stores or ships them. A contributed symbol whose
fingerprint matches is rejected. This enforces "draw our own" ([standards](../research/standards.md) §4, §14)
without relying on reviewer memory. Comparing against the collection is use, not redistribution, and the
fingerprints never feed model training (QET's element licence, [oss-landscape](../research/oss-landscape.md) §2.6).

**Packs.** Packs are released with semver. A release note lists the major changes, so that projects can see in
advance which updates need a mapping. An installed pack is verified against its manifest hashes. Signed packs follow
the plugin-signing infrastructure in 1.x.

### 6.3 Shipped content plan

| Release | Symbols | Parts | Macros |
|---|---|---|---|
| MVP | starter set of about 40 symbols for the reference set (FR-SYM-007, §9.1) on the native grid, plus its generated QET 10 px edition (§3.2 rule 2); pole-count variants generated from family sources (§3.2 rule 10) | manufacturer master; starter catalogue with every part of the reference set as CC0 facts, sections flagged by confidence; `TEST-*` parts marked test | PSU block (folio 102), heater/thermostat/lamp group (folio 101), valve module and valve-manifold page (folios 203–206) |
| 1.0 | full IEC 60617-style control-cabinet set and ISO 1219 set (FR-SYM-008), linted in CI (FR-SYM-020) | ETIM data pack; first manufacturer pack under an explicit licence (B10, IDEA-091) | motor starter, contactor/relay, PLC I/O typicals with representation variants |
| 1.x | user-authored parametric families in the symbol editor (FR-SYM-027) | community packs (FR-PRT-030), BMEcat imports (FR-PRT-020) | rule-based project generation (FR-MAC-016) |

---

## 7. Outputs

| Output | Form | Consumers |
|---|---|---|
| Library directory (`.ewlib`) | canonical JSONL plus manifest, REUSE metadata | git, file shares, `ew lib` |
| Per-user search index | SQLite + FTS5 in the cache directory (derived) | browsers, part selection |
| Embedded copies | `lib/<kind>/<copy_id>.json` in the project | every query; the as-built archive (NFR-REL-10) |
| Part data in reports | BOM, device list, PART DATA blocks, enclosure legend (via `device_part_lines`) | [09](09-reports-forms-output.md) |
| Import reports | check-run style list with source locations | P6 |
| Lint and preview artefacts | JSON and JUnit lint results; PNG sheets of every symbol variant | CI, PR review |
| Value-set CSV (1.0) | header = variable names, one row per value set | engineers, I/O-list generation |
| Basic project | an `.ew` directory with `kind = Basic` | new projects |

---

## 8. Edge cases and pitfalls

| Case | Handling |
|---|---|
| Two "N" CPs on one MCB, two "+" on a PSU | internal index; occurrence printed where ambiguous (§2.4) |
| Symbol draws fewer CPs than the part pattern | the remaining CPs exist and can be wired from another representation; FR-CHK-018 reports them unless marked unused |
| Update removes a connected CP | blocked in the preview until mapped (§4.5); never a silent disconnect |
| Pasted records use another version of an embedded item | the paste asks: update, adapt to the existing copy, or keep both (§2.1); with "keep both" the library manager shows both and offers to unify them in one update |
| QET-grid symbol on a native-grid page, or native symbol on a QET-grid page | refused, with the hint to use the symbol's other grid edition (starter set), place it on a page of its grid, or fork it onto the other grid |
| Imported element with "3-B0"-style origin anchor | kept for fidelity to the QET file; FR-CHK-035 reports it; quick fix sets the anchor policy (MF-13) |
| "UK5N / ULSKG5" in one part-number field | split at "/" into two candidates and flagged; the importer cannot tell an alternative (FR-PRT-004) from a second part on another terminal. On 1X1 it is the latter: UK 5 N on terminals 1 and 2, USLKG 5 on PE (U-17). "ULSKG5" is resolved to USLKG 5 only by review (D-18) |
| "Murr Elektronik", "Schneider", "Eaton / Holec" | resolved through master aliases; an unknown name creates a draft manufacturer, never a silent new spelling |
| Same part number, different manufacturers | identity includes the manufacturer; search shows both |
| Configured product without decodable components | BOM in assembly mode only; component mode lists the configured row and a finding until components are entered (D-36) |
| EDZ or macro pin "Out 1" vs schematic "Out1" | mapping table per macro or part (FR-LAY-027); EW-LIB-010 in libraries |
| EDZ description with `;;` inside a language string | decoded as a literal separator, never as the start of a new language |
| 100k-row BMEcat import | streamed parse under the input limits (NFR-SEC-02); index build incremental and off the UI thread |
| Value-set type error ("Spare" into a `Part` variable) | refused at value-set edit time with the variable's type; a CSV import reports the row and column |
| Macro IP names pairing with existing points | listed in the ghost; optional per-instance name prefix variable |
| Macro inserted twice with the same value set | manual addresses, process tags, kept tags and doubled module slots (hence their derived addresses) conflict and are listed (FR-MAC-005, §3.5); derived tags simply number on |
| Company library edited by two maintainers at once | lock file as for projects; git branches merge per record (ADR-0012) |
| Library deleted while projects use it | projects unaffected; the copies read "Only in this project" |
| Vendor licence forbids embedding a mesh | part copy keeps only the hash and a re-download hint (FR-LAY-022) |
| Test data (`TEST-DOORSW-1`) in a release | allowed, but listed in the release report as test-confidence data |

---

## 9. The starter set

The tests open the sample project (`fixtures/sample/`). This section holds the
starter set's keys, the sample's parts library and the rules of an assembly's macros.

### 9.1 The starter set

A page on the QET 10 px grid places the `@qet10` edition of each key (§3.2 rule 2); a native-grid page places the
native edition.

| Symbol key | FnDef (CPs) and rules |
|---|---|
| `ew.iec.switch-2p-manual` | 2 poles, PassThrough{opens}, `switches_n_with_l` (1–2, 3–4); the dashed linkage is derived from the link points `pole1` → `pole2` → `handle` (§3.2 rule 8) |
| `ew.iec.mcb-1pn` | ProtectionOvercurrent, 1–2 and N–N pass-through (1, 2, N, N) |
| `ew.iec.mcb-1p` | 1–2 (1 alias L1, 2 alias T1 from part) |
| `ew.iec.thermostat-nc` | NC pass-through 5–3; text slots tag, technical data (the setpoint) and part number (D-33) |
| `ew.iec.position-switch-no` | NO 13–14 |
| `ew.iec.lamp`, `ew.iec.heater` | loads (X1, X2), (L, N, PE) |
| `ew.iec.terminal`, `ew.iec.terminal-pe`, `ew.gen.strip-segment` | feed-through `Short` per level; PE with rail contact; the segment gives each terminal it draws one CP per side, one grid step apart along its axis, terminals shared out over the strip's segments ([08](08-connections-terminals-cables.md) §6.7) |
| `ew.iec.pe`, `ew.iec.fe`, `ew.iec.plug-pin` | earth roles PE and FE; plug pin. PE is the earth sign inside a circle (IEC 60417-5019), FE the plain earth sign (IEC 60417-5018), not yet verified against the standard (like every `iec60617_ref`); their text slots sit at x = 6 mm (PE, clear of the circle) and 4 mm (FE), and a content lint guards the assignment |
| `ew.gen.blackbox` | from the part pattern; without drawn geometry the bootstrap derives the outline and CP positions from the pattern ([14](14-interop.md) §8.4); text slots tag, function text, cross-reference, part number and manufacturer (D-18) |
| `ew.gen.ip`, `ew.gen.external-arrow`, junction dot and bevel, cable marker, wire annotation | built in (the interruption point's FnDef is `ew.fn.ip`, §2.3; it prints its pairing name in the function-text slot); the wire annotation is the graphic of a `PageItem::WireMarker` ([03](03-domain-model.md) §6) |
| `ew.bus.dp-port`, `ew.bus.dp-connector` | BusPort; the anchor at the graphics gives the printed cell (D-14); the port's `manufacturer` and `order_code` slots (appended, slots 5 and 6) print the maker and the order code (D-36) |
| `ew.ovw.module`, `ew.ovw.valve-top` | overview of the assembly view |
| `ew.iso.valve-5-2-sol-spring` | fluid 5/2, ports 1–5, solenoid 14 with manual override, spring |
| `ew.iso.cylinder-double-acting`, `ew.iso.process-valve-2way` | fluid; linkage points |

Seven further symbols join the starter set for the pilot corpus (§2.3).

### 9.2 Parts and data

- **The sample's parts.** The sample project's catalogue parts ship as the system library `ew-sample-parts`
  (`content/libraries/ew-sample-parts.ewlib`, "Sample project parts", CC0-1.0), read by the installation's system
  tier and not compiled in. A part whose device has functions on starter function definitions carries a function
  template and a CP pattern from them; `meta.confidence` names each section (`part`, `mech`, `cp_pattern`,
  `function_template` and each data section) as `test` or `unverified`, never `verified`.
- **Manufacturer copies.** A project holds one manufacturer copy per manufacturer name, and its part copies name
  them with no `key`; the readers take the name from the held copy and fall back to a legacy `key`, so a file
  written with the `key` alone derives the same.
- **Typed data, not descriptions.** The symbol slots print the maker, the part number and the rating from typed
  data, never from the description (D-35); a PSU part's potential roles (+ = L+, − = M, PE = PE) type the 24 V
  potentials (FR-CON-011); a pin declared FE in a part pattern is checked against PE (FR-CHK-030, D-16); a cable
  part's cores generate its conductors (FR-CBL-004), and an imported "BR" is flagged (D-17); a valve module's
  solenoid-position map, with its confidence, decides EW-PLC-012 (D-26); a configured product prints its order
  code in assembly mode and its components in component mode (FR-PRT-006, D-04, D-36).
- **Accessories and variants.** The generic terminals `EW-GEN-TRM-1` and `EW-GEN-TRM-2` name the end plate
  `EW-GEN-EP-1` per strip and the end bracket `EW-GEN-EB-1` per rail group, `EW-GEN-TRM-3` the end plate
  `EW-GEN-EP-2`; the duct `EW-GEN-DUCT-1` names its cover `EW-GEN-COV-40` per metre. `EW-GEN-TRM-1` is grey
  (`colour` `GY`) and has the variant `BU`, blue, `EW-GEN-TRM-1-BU`; `EW-GEN-TRM-2` is green-yellow (`GNYE`).
  Plug-in bridges `EW-GEN-BRG-2`, `-3`, `-4` and `-10` serve the generic terminals. The terminals and the circuit
  breakers carry clamp data (type and cross-section range), as `test`.
- **The workshop stock.** Generic ferrules `EW-GEN-FER-0.5` … `-6` (insulated, single) and `EW-GEN-FER-2X0.5` …
  `-2X2.5` (twin), their colours those of DIN 46228-4 and flagged unverified; the markers `EW-GEN-MRK-1` (wire),
  `EW-GEN-MRK-2` (terminal, 5.2 mm) and `EW-GEN-MRK-3` (device). No wire part is shipped: a wire without one is
  listed and ordered by its size and colour. Cable glands `EW-GEN-GLD-M16` … `-M40`, clamping ranges unverified. A
  new project takes the ferrules, markers and glands as its stock (§2.5).
- **The cabinet.** Pushbuttons and lamps go in a 22 mm cut-out; `EW-GEN-HDL-1` is a door coupling handle; the
  enclosure names its door; the cables give their outer diameter ([12](12-placement-pages.md) §2.5).

### 9.3 Macros of an assembly

- **A module typical** (window macro) holds a module device with its part, parent `$assembly` (a `DeviceRef`
  bound at insertion) and its place in the manifold: `position = $pos` and `plc_module.slot = $slot`, or, where
  the modules use positional slots (`SlotPath::Positional`), a position and a tag counter variable instead of the
  slot.
  Its valve children carry positions, states (`$state1 …`: used, spare) and function texts (`$fn1 …`), which a
  value set fills only with the manual text "Spare" for spare valves: a used valve carries no function text, and
  its FunctionText slot prints the automatic function text, else the process tag of the device it drives through
  `DeviceLink{Drives}` ([11](11-fluid-power.md)).
- **Coil bindings** are stored as drawn; the part's solenoid map decides whether a binding is right (EW-PLC-012).
- **A placeholder group** (`field`), included by a boolean variable (`$has_field`), holds field devices, each with
  a 2-way process-valve and a double-acting actuator function joined by a linkage, process tags (`$pt1 …`), a
  `DeviceLink{Drives}` from its valve, and the fluid lines from valve ports 2 and 4.
- **A page macro** holds module groups and boolean variables (`$has_b`, `$has_field`); one value set fills one
  page (FR-MAC-004).
- **On insertion** the valves get child tags (D-27, D-41), the addresses are derived from slots and bindings, and
  frozen tags are cleared (§3.5). The assembly's overview does not regenerate from the inserted children.
- **A window macro of devices with interruption points** shows in its ghost how the points will pair (an incoming
  point joins an existing outgoing one, an outgoing one may become a second source), and the user may set a
  name-prefix variable before the commit (FR-MAC-003).

### 9.4 Defects prevented

| Defect | Mechanism in this area |
|---|---|
| D-02 | Panel-front graphics from the part's macro (FR-PRT-027), drawn for a placement that references the device; panel-front representations are derived from placements, never stored ([12](12-panel-layout.md)) |
| D-04 | Parts on devices with quantities; configured products and their components; accessories from part data |
| D-10, D-41 | Macros create assembly children; overviews are generated from children |
| D-13 | FnDef `PassThrough{opens}` and `switches_n_with_l` give derived "switched" potentials |
| D-14 | Anchor policy per symbol; never the invisible origin |
| D-16 | Pin potential roles (FE vs PE) from part patterns |
| D-17 | Cable cores from the part; colour enum |
| D-18 | Manufacturer master with aliases; EW-LIB-009; split alternatives |
| D-23 | Part pattern exposes 13-14, so the unconnected-CP rule sees them |
| D-26 | Address and solenoid maps as part data with confidence |
| D-27 | Typicals create tagged valve children and process tags |
| D-29, D-31 | Text slots take styles by kind; the minimum height is checked on styles |
| D-35 | Description and technical data are separate part fields |
| D-36 | Configured product with order code and component BOM mode |
| D-40 | Configurator validation as a 1.x decoder plugin (FR-DEV-036) |
| D-42 | `ew.gen.strip-segment` draws one strip device in several places, so -X1031 is one BOM row |

