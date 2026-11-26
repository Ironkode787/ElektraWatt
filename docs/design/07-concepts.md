# Symbols, parts and macros: symbols, function definitions, parts and macros as data

A chapter of [Symbols, parts and macros](07-symbols-parts-macros.md), whose table says where each section lives.

---

## 1. Scope and position in the architecture

Master data is **reusable knowledge that a project embeds but does not own**. A symbol says how a function looks.
A FnDef says what it is. A part says what is bought. A macro is a reusable project fragment. A basic project
carries a company's conventions. All five follow one life cycle:

1. They are authored in a **library**.
2. A project uses an item by **embedding a copy** with `{library_id, item_id, version, sha256}` (ARCH §4.2, G2).
3. They change in a project only by a **previewed "update from library"** command (ARCH §5.4).

A project therefore opens, renders, checks and exports without any library installed (NFR-REL-11).

```mermaid
flowchart LR
  subgraph SRC["Sources (user-initiated, local)"]
    QET[".qet / .elmt"]
    EDZ["EDZ, EPLAN parts XML"]
    CAT["CSV/XLSX, BMEcat, ETIM xChange"]
    DXF["DXF blocks"]
    ED["Symbol, part and macro editors"]
  end
  subgraph LIBS["Libraries (.ewlib directories, canonical JSONL)"]
    SYS["System (shipped, read-only)"]
    COM["Community packs (installed, read-only)"]
    CO["Company (file share or git)"]
    USR["User"]
  end
  IDX[("Per-user SQLite + FTS5 index<br/>(derived cache)")]
  PRJ["Project lib/: embedded copies<br/>SymbolDef, FnDef, PartCopy, MacroDef, FormDef"]
  SRC -->|"importers → records (ew-interop-api)"| CO & USR
  SYS & COM & CO & USR --> IDX
  IDX -->|"search, select"| CMD["Commands: PlaceSymbol, AssignPart,<br/>InsertMacro, UpdateFromLibrary"]
  CMD -->|"embed closure"| PRJ
  PRJ -->|"render, report, check"| OUT["Pages, BOM, checks"]
```

**Crates** (ARCH §3):
- `ew-schema` (permissive) defines every library record kind, so third parties can read libraries without GPL code
  ([vision](00-vision-and-scope.md) SC-11).
- `ew-format` (permissive) reads and writes library directories with the project writer.
- `ew-library` (GPL) owns the registry of installed libraries, the SQLite/FTS5 index, search and **closure
  resolution** (which items a `LibRef` pulls in). It depends only on `ew-schema` (ARCH §3) and is the only crate
  besides `ew-sqlview` that links rusqlite (NFR-MAINT-01).
- `ew-commands` holds `PlaceSymbol`, `AssignPart`, `InsertMacro` and `UpdateFromLibrary`. `ew-api` resolves the
  library items and passes their records **inside the command**, so a journal replay or a CI run reproduces the
  same transaction without the library installed. The update diff is the ordinary preview fork (ARCH §5.4).

**Out of scope here:**
- placing and wiring symbols on pages ([06](06-schematic-editor.md));
- frames and report forms, which are `FormDef` library items but are specified in
  [09](09-reports-forms-output.md);
- the importer internals ([14](14-interop.md));
- panel macros in 3D ([12](12-panel-layout.md)).

---

## 2. Concepts and data

### 2.1 Library levels and identity

| Level | Where | Writable by | Typical content |
|---|---|---|---|
| **System** | installation directory (NFR-PLAT-06) | nobody (FR-PRT-012) | starter symbols, FnDefs, starter catalogue, frames, manufacturer master, ETIM data pack |
| **Community pack** | user data directory, installed from a file or a git clone | nobody in EW; updated by reinstalling | `ew-iec60617`, `ew-iso1219`, manufacturer packs with explicit licences |
| **Company** | a directory on a file share or in a git repository | library maintainers (P6) | company parts, macros, basic project, symbol forks |
| **User** | OS data directory | the user | drafts, imports in progress, on-demand QET conversions |
| **Project** | `lib/` inside the project (ARCH §7.2) | only through commands | the embedded copies actually used |

**Identity:**
- Every library has a UUID (`library_id`).
- Every item has an `item_id` that stays the same across versions, a human key (`pxc.2904602`, `ew.iec.mcb-1pn`)
  and a semantic `version` (`MAJOR.MINOR.PATCH`). Authored items get UUIDv7; imported items get
  `v5(import namespace, source key)` (ADR-0004), so re-importing the same CSV or EDZ updates items in place. The
  version is stored once per document, never in `ItemMeta`: a library record carries it as its top-level `version`
  member after `id` (the item ID), an embedded copy in its copy metadata
  ([05](05-project-format-and-persistence.md) §13.2).
- **Identity is the pair `(library_id, item_id)`, never a name.** Two libraries may both contain a part called
  "2904602" without either one shadowing the other.
- **Published versions are immutable.** `ew lib pack` refuses to publish different content under an existing
  version, so `(library_id, item_id, version)` determines the content.
- **Item status** (`ItemMeta.status`): `Draft | Released | Deprecated{successor: Option<LibRef>} | Withdrawn`.
  Selection dialogs hide deprecated and withdrawn items unless a filter shows them; a project copy of a deprecated
  item stays valid, and the library manager offers its successor through "update from library".

**Library items reference each other by `LibRef{library, item, version_req}`.** Examples: a symbol's FnDef, a
part's accessories, a macro's parts.

**Project records reference copies by copy ID.** Examples: `Representation.symbol`, `Function.def`,
`PartRef.copy`.

- **Copy ID** (ADR-0004 amendment 2; [05](05-project-format-and-persistence.md) §13.2):
  `copy_id = v5(NS_LIBCOPY, library_id ‖ item_id)`, one copy per item and project. "Update from library" rewrites
  that one `lib/` file and leaves every referencing record untouched, so diffs and merges stay small, and two
  branches that embed the same version write identical files.
- **Keeping two versions.** A cross-project paste of a different version ([06](06-schematic-editor.md) §3.6) asks:
  update the existing copy (a previewed update), adapt the pasted records to the existing copy, or keep both. Only
  "keep both" mints `v5(NS_LIBCOPY, library_id ‖ item_id ‖ version)` for the second copy; the library manager
  then lists both and offers to unify them.
- **Hash.** `sha256` is taken over the canonical compact JSON of the item's own fields, without `id`, `library` and
  the copy metadata (ARCH §7.3 writer rules; 05 §13.2, integrity rule EW-CORE-014), so the copy content equals the
  published library item.
- **Files.** Each copy is one pretty-printed file, `lib/<kind>/<copy_id>.json`: `$k`, `id` (the copy ID),
  `library_id`, `item_id`, `version`, `sha256`, then the item's fields (05 §3.4, example in §3.5). In Rust this is
  `LibCopy<T>{id, library_id, item_id, version, sha256, item: T}` (`id` is the copy ID, typed `Id<T>`) with `item`
  flattened on write.

The system tier holds two compiled-in
libraries: the starter (`ew-starter`: symbols, FnDefs, forms) and **`ew-starter-parts`**, the generic starter parts.
`ew-starter-parts` (`content/libraries/ew-starter-parts.ewlib`, version 0.1.0, CC0-1.0) holds the manufacturer
"Generic" and the generic parts, numbered `EW-GEN-<kind>-<n>`: the common kinds of a control cabinet (relays,
contactors, breakers, switches and lamps, a motor, a power supply, terminals, cables and plugs, PLC CPUs and cards),
of a pneumatic page (valves, valve terminals, cylinders, air preparation, tubes and fittings) and of a panel layout
(an enclosure, a rail, a duct). The test that writes the directory is the list; this paragraph names no part, so it
cannot fall behind. Each carries its template
functions on starter FnDefs, its CP pattern, contact data and the technical data the starter symbols print, and no
order or catalogue data; its description is in English and Dutch, as every starter symbol's name is but the junction dot's and bevel's (their seals belong to a derivation
profile), so a Dutch
project's lists and the symbol browser in Dutch read in Dutch; item IDs are `v5(library, "<kind>:<key>")`. The directory is written by a test
(`crates/ew-library/tests/starter_parts.rs`) with the library writer and compiled in, so every session holds it with
or without configured tiers; a directory library of the same ID replaces the compiled-in copy. The relay and the
contactor template their coil only and list their contacts as contact data.

### 2.2 Library directory and index

A library has the same on-disk form as a project. The same writer, manifest-last save, lock file and merge driver
apply (ADR-0010, ADR-0012):

```text
company-std.ewlib/                 # example name; content is UUID-named
  library.json                       # written LAST: id, name, version, default licence, format, min_reader,
                                     #   generation, depends [{library, version_req}], sha256 per file
  manufacturers.jsonl                # manufacturer master records
  fndefs/<xx>.jsonl                  # 256 shards by last id byte, as projects
  symbols/<xx>.jsonl  parts/<xx>.jsonl  macros/<xx>.jsonl  forms/<xx>.jsonl
  assets/<sha256>.svg|png            # previews, logos, part images whose licence allows storage
  REUSE.toml  LICENSES/              # per-path licences (NFR-LIC-01)
```

**The SQLite catalogue is a derived index.** `ew-library` builds one SQLite database with FTS5 per user. It lives
in the cache directory (NFR-PRIV-05) and covers all installed libraries. It is rebuilt incrementally when a
library's manifest generation changes, and from scratch when it is missing or corrupt.

- **Requirements met.** This is FR-PRT-012 (libraries at system, company and user level with full-text search
  through a derived index) and FR-PRT-028 (JSONL for version control), both at MVP.
- **Why not SQLite on the share.** A company library on an SMB share is never opened as a SQLite file. ADR-0010
  rejected SQLite on network shares for projects, for the same reason.
- **Read-only levels.** Writes to system and community levels are refused at the registry.
- **Hostile files.** Foreign catalogue databases are read-only imports, opened with `trusted_schema=OFF` and
  defensive mode (NFR-SEC-06).

ARCH §7.7 and the tier table of [05](05-project-format-and-persistence.md) §13.1 use the same form (Open
question 1).

`ew-library` opens the four tiers (system: the
compiled-in libraries of §2.1 and `content/libraries/*.ewlib`; community; company, from the settings' paths and a
repeatable `--company`; user). The `.ewlib` reader applies the project loader's limits and reads only the files `library.json` lists; a records
file of the directory that it does not list is a note of the library, which `ew lib list` prints. The writer writes the shards
first and `library.json` last; a library lock follows [05](05-project-format-and-persistence.md) §7.4: a live lock is
`Locked`, a stale one `Stale`, taken over only on explicit confirmation. The per-user SQLite FTS5 index lives in the
cache directory. A session opens the tiers at its first library use and falls back to the compiled-in libraries,
saying why, when they cannot be opened. `ew lib list|search|index --rebuild|create` drive it from the command line.

### 2.3 Symbols

A symbol is the library item kind `symbol`: the struct `SymbolDef` of `crates/ew-schema/src/library.rs`, with its
connection points (`SymbolCp`), text slots (`TextSlot`) and graphics (`Primitive`, each with its stroke and fill),
each member with what it holds; its stored form is the generated schema (`schemas/0.1/symbol.schema.json`). Its
`meta` holds the key (`ew.iec.mcb-1pn`), licence, sources and status of §2.1, and no version. Its `grid` is the
native connection grid (ADR-0027), or QET's 10 px for imports and the generated QET-grid edition of the starter set
(§3.2). Its `fndef` names exactly one FnDef (FR-SYM-005); its arrangements are named slot sets such as "full" or
"tag + rating" (FR-SYM-012); its `iec60617_ref` is an identifier of the form `S00nnn`, metadata only (FR-SYM-019). A
rectangle's `shadow` is the filled drop shadow of FR-SYM-003 (2.61 mm on the PART DATA boxes).

**Rules.**
- **Pens.** `Pen` is one of the house pens 0.13, 0.18, 0.26, 0.42, 0.52 and 1.30 mm, or an explicit `Nm` width.
  Dash patterns scale with pen width (FR-SYM-003), so the S51 linkage prints dash 1.04 / gap 0.52 mm at 0.26 mm. The patterns, as dash:gap ratios in units of the pen
  width, are `Dash42` 4:2, `Dot12` 1:2 and `DashDot` 4:2:1:2 (dash, gap, dot, gap), fixed by `ew-dl`. House pens
  default to butt caps and miter joins so dashes print at nominal length, and every writer sets the miter limit to 4.0
  explicitly because PDF defaults to 10 and SVG to 4. In the display list, `ew-dl`'s `Stroke` carries the stroke
  colour as a `brush` (default black); lowering resolves `ByLayer` to a concrete colour, so wire colours render
  (ADR-0015 amendment 4).
- **Pens on QET-grid pages**. The `@qet10` edition (§3.2 rule 2) expresses its pens
  in QET px rather than scaling the house pens: 0.26 mm becomes 1.0 px (264,583 nm), 0.13 mm 0.5 px and 0.52 mm
  2.0 px (and 0.18 mm 0.7 px, 0.42 mm 1.6 px, 1.30 mm 5.0 px), and wires and junction graphics
  on QET-grid pages use the 1.0 px pen. After the QET frame's page-to-sheet
  scale they print 0.261, 0.130 and 0.522 mm, the reference drawing's pens. Native-grid pages keep the
  house pens.
- **Orientation variants A–H are never stored.** They are derived from the source variant (§4.1, FR-SYM-004), with
  the letter mapping of [06](06-schematic-editor.md) §2.5. Only the contact-image and CP variants are stored as
  extra variants.
- **CP indices.** `SymbolCp.idx` is the symbol's own index. `fn_cp` points at the FnDef CP it draws. A symbol may
  draw a subset of the FnDef's CPs: a single-line MCB draws two of four.
- **Text slots** are typed. The kinds are:
  - Tag, FunctionText, TechData, Manufacturer, PartNumber, TypeDesignation;
  - CpDesignation(idx), Xref, PlcAddress, ProcessTag, UserProp(key), ContactImage;
  - OrderCode: the order code of the device's first assembly config (D-36).

  A slot prints only with `"visible": true`: `visible` is `false` when absent, and the canonical form leaves `false`
  out (`ew fmt` rewrites a written `"visible": false` to its absence).

  Text height comes from the project's text style bound to the slot kind (FR-SYM-013); a slot may name its own
  text style in the optional member `style`, which replaces that binding for the slot. A
  `style` that names no known text style falls back to the slot kind's style and is reported as a note naming the
  symbol, the slot and the name, never as an error: `style` is
  an optional identifier, absent by default, and `ew-scene` reports an unknown name as
  `PageFinding::UnknownSlotStyle`. The styles are `tag.main`,
  `data.small`, `cp.small` and the others of the built-in table, which removes D-29. A slot's `style` resolves
  ([ADR-0032](../adr/0032-per-project-text-styles.md)) against the project's effective table, so it may name a key the project's `text_styles` adds; the scene resolves keys through `ew_text::TextStyleTable::from_rows` over the rows its caller passes (the first row of a duplicated key wins).
- **Built-in symbols.** The interruption point `ew.gen.ip`, the black-box frame `ew.gen.blackbox` (its outline and
  CPs live in the representation's `box: BoxSpec`, [03](03-domain-model.md) §6, [06](06-schematic-editor.md) §2.7),
  the junction dot and bevel, and the derived linkage are system symbols with fixed keys. Importers and the engine
  rely on these keys.
- **Keys and item IDs of the starter library**. An item ID is v5(library, key), and a symbol
  and a FnDef must never share one, so FnDefs are keyed `ew.fn.<name>`: the interruption point is the symbol
  `ew.gen.ip` with the FnDef `ew.fn.ip`. Every starter item is version 0.1.0 with status `draft`. Importers
  reference a built-in FnDef by key with `version_req` `^0.1`: `ew-import-qet` has exactly one, `ew.fn.ip`
  ([14](14-interop.md) §4.3). The starter content and the importer use the `library_id`
  7202fd8b-1024-4bf1-9e99-670772eaa589, the system library's ID `ew_schema::starter::LIBRARY`.
- **Coverage of the pilot corpus** ([23](23-symbol-coverage-pilot-corpus.md)). Against the 81 QET element types that the
  pilot's three real projects use (2,170 placed elements), the 36-symbol starter set covers 37 types and 74 % of the uses,
  two more types (the nomenclature block and a mounting plate) are covered by report blocks and panel layout, 29 types
  (vendor PLC, valve-terminal and safety modules, plus 11 unproven elements) stay black boxes with the embedded QET
  graphics, and 13 types lack a symbol. One family dominates the gap: the female connector half (224 of 251 missing
  uses), followed by cable screen, temperature sensor, two-pole MCB, disconnectors, socket, RCD, power supply, emergency
  stop, electronic switch and level sensor. The note ranks the families, records the pilot's drawing style (wiring marks,
  terminal and interruption-point density, nomenclature blocks as bill of materials, slice-wise valve terminals,
  one-terminal-per-channel PLC modules) and proposes fifteen symbol tickets and nine importer mapping entries. It adds no
  requirement and changes no key. The seven types that a pilot project uses at least twice are to join the starter set
  (`ew.iec.plug-socket`, `ew.gen.screen`, `ew.iec.sensor-temp`, `ew.iec.mcb-2p`, `ew.iec.disconnector-1p`,
  `ew.iec.switch-disconnector-4p`, `ew.iec.socket`; of them `ew.iec.mcb-2p` has joined); the end-terminal and the other variants
  follow the same rule after inspection. A QET cross-section mark that disagrees with the derived wire property is
  reported as EW-IMP-123 with the mark's value ([14](14-interop.md) §4.12).

A text slot may carry `fallback: process_tag`: it prints the
function text and, when that is empty, the tracked target's process tag (doc 11 §3.3); `ew.iso.valve-5-2-sol-spring`
0.3.0 (both editions) sets it on its function-text slot. Pilot and drain lines are drawn with this section's `dashed`
pattern, `verified = false`.

`ew.iec.proximity-sensor-pnp` draws a three-wire PNP proximity sensor in IEC 60617 practice: a
box around a normally open contact worked by a proximity actuator (the rhombus, linked by the dashed line), its pins
the cores BN (L+, top), BK (the switched output, below the contact) and BU (M, beside it). Its FnDef
`ew.fn.proximity-sensor-pnp` (SensingPosition, contact kind NO) passes BN to BK when the contact closes; the starter
part EW-GEN-PRX-1 (24 V DC) is drawn on it.

### 2.4 Function definitions and connection-point designations

`FnDef` is defined in [03](03-domain-model.md) §7:
- `category` and `contact_kind`;
- `cps: Vec<CpDef{idx, designation, alias, discipline, potential_role, source_output, optional, level, side,
  clamp, port_role, physical}>`;
- `internal: Vec<{a, b, kind: Short | PassThrough{opens}}>`;
- `flags{switches_n_with_l, n_isolation}`;
- `valve_model`.

This document adds the library rules.

**The category drives three things:**
- the letter code through the identifier set (FR-NUM-004);
- part compatibility (§4.3);
- the quality profile (§3.4).

**Categories are data.** They form a native taxonomy with an optional ETIM class mapping, shipped in the system
FnDef library. A category key has the three levels category / group / definition that FR-SYM-005 requires, for
example `ProtectionOvercurrent / MCB / 1P+N` for -101F1 and F55. The identifier set and the part-compatibility
filter read the first two levels; EDZ `functiondefcategory/group/id` triples map onto these keys.

**Designation precedence.** The printed designation of an instantiated CP is resolved from the highest level that
sets it (lowest number wins):

| # | Source | Example |
|---|---|---|
| 1 | `Function.cp_overrides[idx].designation` (FR-DEV-008) | a re-labelled MICO output |
| 2 | Part CP pattern of the template function the device uses (FR-PRT-008) | 2904602: L, N, PE, +, +, −, − |
| 3 | `CpDef.designation` of the FnDef | MCB 1P+N: 1, 2, N, N |
| — | The symbol never designates; it only places `fn_cp` | — |

**Designation rules:**
- **Duplicates are legal.** Uniqueness lives in the internal index (REF-029, FR-DEV-006). The wire list prints the
  occurrence where it is ambiguous: `-102G1:+ #1`.
- **Aliases** print as "1 / L1" (FR-DEV-007).
- **Terminal CPs** carry level, side and clamp. PT 2.5-TWIN has level 1, clamp a on side 1 and clamps b and c on
  side 2 ([03](03-domain-model.md) §11).
- **Template-origin functions**. A function is template-origin when its device carries
  a part whose function template declares a function with the same FnDef (and role, if given). Its CP index space is
  then the part's CP pattern, not the FnDef's CPs: PT 2.5-TWIN gives a, b, c and the shaft, and the starter FnDef
  `ew.fn.terminal` with its two CPs applies only to a terminal without such a part
  ([08](08-connections-terminals-cables.md) §6.1). `function_cps` and the importers apply the rule now; a typed
  template marker on `Function`, tied to the `v5(device, template_index)` ID rule of ADR-0004 amendment 2, is a later
  format decision.
- **Contacts.** FnDefs default to IEC 60947-style function digits (13-14 NO, 21-22 NC, A1-A2 coil). A part's contact
  data (FR-PRT-009) overrides them and feeds contact images (FR-XRF-017).
- **Fluid ports** use the ISO 5599-3 port numbers 1/2/3/4/5/12/14/82/84 as designation and `port_role`
  (FR-FLU-001). ISO 5599-3 is withdrawn; [plc-bus-fluid](../research/plc-bus-fluid.md) cites ISO 11727 as the
  current source, edition unverified.
- **Non-physical CPs.** An internal CP (`physical = false`) is never drawn and never wired. The coil of a VMPA valve
  is the example: it is reached only through a channel binding.
- **Source outputs.** `CpDef.source_output` marks the output CPs of a source function: PSU + and −, a transformer
  secondary, a battery. Potential flow roots there, so -102G1:L is an ordinary member of L-1 and -101F1 stays its
  protective device ([08](08-connections-terminals-cables.md) §3.6). The starter library sets it on the PSU and
  transformer FnDefs.
- **Optional CPs.** `CpDef.optional` marks CPs that may stay free by design: the Out entry of a terminating bus
  connector, a parallel duplicate pin. The unconnected-CP rule EW-POT-009 skips them
  ([08](08-connections-terminals-cables.md) §8.2).

**Internal connectivity is the FnDef's job, not the symbol's.**
- A **feed-through terminal** declares `Short` between all CPs of a level.
- An **MCB pole, switch or contact** declares `PassThrough{opens: true}`. The thermostat FnDef declares 5–3 as an
  NC pass-through, so the nets on folio 101 split at T55 (FR-SYM-005), and its derived potential is "N, switched"
  (D-13, EW-POT-013).
- A **2-pole switch** that disconnects L and N sets `switches_n_with_l`.
- A **neutral isolating terminal** declares `PassThrough{opens: true}` and sets `n_isolation`, so EW-POT-013 reports
  it as info, not as an error ([08](08-connections-terminals-cables.md) §6.2, §8.2).
- A **load** (lamp, heater) declares nothing, so its two sides stay separate potentials.

### 2.5 Parts and their data levels

The library item `Part` is the full record. A `PartCopy` (03 §7) is `LibCopy<Part>`, the same record embedded.

Its members, each with what it holds, are the struct `Part` of `crates/ew-schema/src/library.rs`, and its stored form
is the generated schema (`schemas/0.1/part.schema.json`). What the code does not say: the part number is kept
verbatim (`SK 3110.000`) and search uses a normalised form (§4.3); the description is a noun phrase, never ratings
(D-35, FR-DEV-010); `free_props` hold EPLAN's `<freeproperty>` and user fields; quantities in the order unit are
`Dec3` (FR-PRT-026); each section of `sections` is owned by its area (FR-PRT-010).

These members of a part are specified; not built: a supply CP's load current in `cp_pattern` (`i_load`, `Dec3` in
A), `classification` (`{scheme: etim | eclass | iec_cdd | native,
id, version}`), `model3d` (the STEP file's SHA-256, its source and whether it may be embedded, FR-LAY-022), `cp3d`
(3D connection points, FR-LAY-026), `drilling`, `commercial` (suppliers, supplier
numbers, prices with currency and date), `logistics` (pack quantity, minimum order, customs code, origin, lifecycle)
and `docs` (datasheet references as text, images only as asset hashes). The accessories, the variants and the
sections of the parts a wire consumes are below.

**Technical-data keys** (decided 2026-10-18). `tech_data` uses the one `TechValue{key, value}`
encoding of [05](05-project-format-and-persistence.md) §3.2 on parts and devices alike. A key qualified by a
classification scheme is spelled `"<scheme>:<id>"` inside `key` (`etim:EF001234`; schemes `etim`, `eclass`,
`iec_cdd`), and a plain key without a colon is a native or project property (`i_out`, `curve`). Booleans and
enumeration values are `Ident` values (`"true"`, `"false"`, the enumeration member) until a typed form is needed. One
encoding for every property keeps the v0 schema small; ETIM features (FR-PRT-018)
may append a typed value variant without changing stored keys.

Typed technical data on parts and devices use the native plain keys
`characteristic`, `rated_current`, `channel_current` and `setpoint`. A slot text follows
these rules: a device's own value wins over its part's; a miniature circuit-breaker prints characteristic and rated
current composed as "C10" (the reading of U-32); other values print as value and unit ("40 A", "15 °C"); several
values are joined with ", " in key order. Two interims apply. The main part of a device is its first part use, not
yet "the use that provides the main function"; the manufacturer name is read from the manufacturer
`LibRef`'s `extra["key"]`, a stopgap until typed manufacturer copies exist. The helpers `tech_data_text`,
`main_part` and `manufacturer_name` are built in `ew-derive` (`part_text.rs`); the slot-text mapping is a product
query of the `Session` export job (`ew_api::output::PageSlotsOf`, asked for it in
`ew-derive`). The 24 V DC
output of part 2904602 is recorded under the plain key `output_voltage` ("24 V") beside `rated_current`; a typed key for
an output voltage is not built.

Part data is organised in **data levels**. Each level lists its consumers and the release that first needs it:

| Level | Fields | Consumers | Needed from |
|---|---|---|---|
| L0 identity | manufacturer, part number, type, order number, GTIN, description, kind, unit | BOM (FR-REP-004), search | MVP |
| L1 technical | `tech_data` (native keys), contact data, CP pattern (designation, potential role, level, side, clamp, cross-section range, max conductors, twin ferrule, optional load current per supply CP), function template; ETIM/ECLASS classification | symbol texts, CP instantiation, contact images, potential types, FR-CHK-021; load currents: the load summary and `api_v1_cp.i_load` (ADR-0017 amendment 3) | MVP (classification FR-PRT-018, clamp checks FR-CHK-021 and load currents: 1.0) |
| L2 domain sections | PLC address map and solenoid map (FR-PLC-011), cable cores (FR-CBL-004), thermal P_n/I_n (FR-THM-001), tube data | `channel_addresses`, cable conductors, thermal | MVP (PLC, cores); 1.0 (thermal, tubes) |
| L3 mechanical | W×H×D, mass, mount kind, rail offset, clearance, 2D front outline | folio 902 placements (FR-LAY-007, FR-PRT-011) | MVP |
| L4 commercial and logistics | suppliers, prices, order unit, packaging, customs code, lifecycle, successor | order lists, ERP export (FR-EXP-005) | 1.0 (FR-PRT-025) |
| L5 3D | STEP reference by hash, mounting frames, drilling and cut-out patterns, keep-outs | 3D, collision, NC data | 1.0 |
| L6 3D connection points | per CP: position, insertion direction, clamp type, cross-section ranges, strip length, insertion depth, torque, test access | routing, wire lengths, ferrules, wiring viewer | 1.0 |

**Provenance and confidence.**
- `ItemMeta.sources` lists where each section came from: a datasheet reference, an EDZ file hash, a BMEcat file, a
  CSV import, or manual entry.
- `ItemMeta.confidence` marks each section `verified`, `unverified` or `test`. A clamp range copied from a
  datasheet is `unverified` until checked; the generic starter parts' clamp data, ferrule colours and widths are
  `test`.
- Findings computed from non-verified sections carry a "based on non-verified part data" marker. The release
  report lists every non-verified section in use (FR-PRT-031). Who may set `verified` in
  a community pack is governed by [19](19-open-source-governance.md) §3.6.

**Manufacturer master** (FR-PRT-002) records are `{key (short code, e.g. "PXC"), name, aliases, brands}`:
- `Murrelektronik` has the alias "Murr Elektronik";
- `Schneider Electric` has the alias "Schneider";
- `Eaton` has the alias "Eaton / Holec" and the brand Holec.

Importers resolve aliases. An unknown manufacturer creates a draft record for the maintainer to merge (D-18).
Projects embed the manufacturers they use as copies under `lib/manufacturers/` (kind `manufacturer`,
[05](05-project-format-and-persistence.md) §2.2); a part references its manufacturer by `LibRef`, as in the part
copy example of 05 §3.5.

`ew-schema` types the MVP members of a
part within format 0.1 (ADR-0011 amendment 4), in this order: `meta, kind, manufacturer,
part_number, type_designation, order_number, gtin, description, category, tech_data, free_props, unit, cp_pattern,
function_template, contacts, mech, sections, accessories, alternatives, configured, variants, macros, box`, then
`extra`. The order is fixed
because a copy's `sha256` is taken over the item's compact JSON in member order, so a later
re-ordering would re-hash every existing copy. The typed members:

| Member | Shape | Requirement |
|---|---|---|
| `contacts` | `Vec<ContactData{kind: ContactKind (the FnDef enum: no, nc, co), cps: Vec<Ident> such as ["13", "14"], seq?: u8, special?: Ident}>` | FR-PRT-009 |
| `mech` | `MechData{width?, height?, depth?: Nm, mount_kind?: MountKind, rail_offset?: Pt2 (rail pose to part origin), clearance?{top, bottom, left, right, front: Nm, an absent side is 0}, front_2d: Vec<Vec<Pt2>>, mass?: u32 grams}`; `MountKind` is `ts35x7_5, ts35x15, ts32, ts15, screw, busbar_adapter, door_cutout, on_parent, none` | FR-PRT-011 (L3) |
| `cp_pattern[]` | `CpData` gains, after `clamp`, `clamp_type?: screw \| spring \| push_in \| idc \| ring_stud \| plug`, `csa_min?`, `csa_max?: Dec3` mm², `max_conductors?: u8`, `twin_ferrule: bool` | FR-PRT-008 |
| `sections` | `PartSections` gains, after `cable_cores` and `plc`, `enclosure?{width, height, depth, wall: Nm, plate?: LibRef}`, `duct?{inner_width, inner_height, slot_pitch}`, `rail?{profile: MountKind, stock_length?}`, `mounting_plate?{width, height, thickness}` | FR-PRT-024 |
| `sections` (consumables) | after `fluid`: `wire?{csa: Dec3, colour?: ColourSpec, class?: u8}`, `ferrule?{csa_min, csa_max: Dec3, twin: bool, insulated: bool, sleeve?: Nm, colour?: ColourSpec, stud?: Ident}`, `marker?{marks: wire \| terminal \| device \| cable, lines?: u8, chars?: u8}` | FR-PRT-035 |
| `accessories` | `Vec<PartAccessory{part: LibRef, qty: Dec3, per: unit \| strip \| rail_group, kind?: AccessoryKind, optional: bool}>`; `AccessoryKind` is the strip accessory's (`end_plate`, `end_clamp`, `partition`, `marker`, `cover`) | FR-PRT-005, FR-REP-051 |
| `alternatives` | `Vec<AlternativeRef{part: LibRef, kind: equivalent \| successor \| approved}>` | FR-PRT-004 |
| `configured` | `Configured{order_code, components: Vec<{part: LibRef, qty: Dec3}>}` | FR-PRT-006 |
| `variants` | `KeyedList<PartVariant{key: Ident, name: MlText, part_number?, order_number?, type_designation?, gtin?: Ident, description?: MlText, tech_data: KeyedList<TechValue>}>` | FR-PRT-003 |
| `macros` | `Vec<MacroLink{rep_type, macro: LibRef}>` | FR-PRT-027 |
| `box` | `PartBox{width, height: Nm, pins: Vec<PartBoxPin{idx (into `cp_pattern`), side: up \| right \| down \| left (the edge, by the direction a wire leaves in), at: Nm (along the edge from the box's top-left corner)}>}`, after `macros` (added after the lock, 0.3.0) | the device box, below |

**The device box.**
A part whose main function is a black box is drawn as a box with its pins on the edges. Its pins are
its `cp_pattern`, which a person adds to, removes from and renames in the part editor and on the symbol editor's
screen (§5); `box` says where they stand: the box's size, and for each pin its edge and its distance along it. Placing
the part then draws that box with one click (`box.black`): the layout is read onto the page's connection grid in its
order and on its sides, each distance (corner to pin, pin to pin, pin to corner) taken up to whole steps of that grid,
so a box laid out on 4 mm comes out no tighter on a page of another grid and its pins stand on that page's grid points;
every pin at least one step from the corners and from its neighbours, the box widened where the pins need it
(`ew_api::part_box::steps`); a pin the layout does not list stands on the bottom edge after the others. A
part without `box` is drawn as before, by two clicks, the first half of its pins on top and the rest below. A pin's
place typed in the box editor between two 4 mm steps, or on a corner, goes to the nearest step clear of the corners,
and a note says where it stands; a copy of a starter or library part keeps that part's number, which the line over
the box says until the person gives it one of its own. The box editor's pin list stands in the order the
pins stand on the box, and keeps its rows while the same pins are edited, so a pin given another edge stays under the
pointer; a new selection sorts the list again. The
placed box is the page's: its edges and pins are still dragged there (`box.reshape`), and a later change of the
part's layout asks before it redraws. `box` is an optional member without cascade semantics (ADR-0011 amendment 6):
an older build keeps it in `extra`, writes it back where it stands, and lays the pins out its own way. A part with a
PLC section or bus interfaces takes its pins from its channels and ports, and a part drawn with a symbol from that
symbol: the box editor shows such a part and says so. *Editing a pin that comes from a channel or an interface*: what is the pin's own is its place, the edge it stands on and its distance along it, and with them its
text's place, which stands inside the box at the pin, turned by its edge; with the box's size these are edited in the
box as on any box, saved as the part's `box`, and drawn that way wherever the card is drawn by its box (a click of the
Device tool, Place here, Draw on a page…, Redraw them). What is the part's is the pin's name (the pin pattern's), what
it carries (a supply pin's potential), its channel and that channel's address and kind: the box shows them and says
that they are changed in the part editor, under PLC or Bus interfaces; such a box takes no pin and loses none. Its
layout is written only from a pin a person placed or a size a person gave: Save on such a box nobody changed says that
the part is unchanged, so the card stays the library's and Draw on a page draws it as a card ([06a](06a-drafting.md)
§7.5); a part with pins of its own takes the layout the box shows at its first Save. A part
drawn with a symbol has no device box to edit. `part.edit_copy` lets the fork of a library copy change
`cp_pattern`, `function_template` and `box` as it does the identity, so a library part's box is changed for the
project; its sections stay the library's.

Every new member is optional and default-omitted, every new enum keeps `Unknown(Value)` and every new object its own
`extra`. The members not built (above, and the thermal section) stay in `extra`, and take
their place in the member order when they are typed.
The `LibRef`s of these members name library items, not project records: the library resolves them. A copy that an
older build wrote, with such a member in `extra` (byte-wise member order), opens with the member typed, is reported
not canonical (info) and is rewritten in the member order above at the next save; its stored `sha256` is kept, so
EW-CORE-014 reports it where the canonical order differs, which is accepted within 0.1
([05](05-project-format-and-persistence.md) §10.3). The manufacturer is the kind `manufacturer`, `{meta, key, name:
MlText, aliases, brands}`, embedded under `lib/manufacturers/` by `library.embed` with the checks of the other kinds;
the slot texts take the manufacturer name from the embedded copy the `LibRef` resolves to, and fall back to the
`LibRef`'s `extra["key"]` only where the project holds no copy.
**The part bus section** `sections.bus` is typed
after `mounting_plate`: `{ports: [...], extra}`, each port written in the order `interface`, `port_label`, `bus`,
`role`, `termination`, then its `extra`; `interface`, `bus` and `role` are required and the enums keep `Unknown`. A
section `{"ports": []}` is written `{}`. A port also types its connector, rates, address range, IO-Link data and
signal map ([10](10-plc-and-bus.md) §2.6); the station address and the nets belong to the drawn port. An older
build's copy is rewritten canonically, and an older build reports EW-CORE-014 on a copy this build wrote.
EW-GEN-CPU-1 carries one port `DP`, `profibus_dp`, `master`.

The fluid section is typed whole ([11](11-fluid-power.md) §2.6); the k-th entry of `solenoids` is the part's k-th
template entry on a coil FnDef.

**Accessories: what a part brings with it** (FR-PRT-005, FR-REP-051). A part names what is mounted and ordered with
it: an end plate after the last terminal of a strip, the end brackets of a run of strips, the cover of a duct. Each
entry of `accessories` names the accessory part, how many (`qty`, in the accessory's order unit) and per what:

- `unit`: per piece of the part a device uses, or per metre of a part sold by the metre, so a duct's cover is as long
  as the duct;
- `strip`: once per terminal strip whose terminals use the part, standing after the last of those terminals; a
  strip whose terminals of two parts name the same accessory takes it once, after the last terminal that names it;
- `rail_group`: the end brackets of a run of strips on one rail: one before the run and one after each strip, so k
  abutting strips take k + 1 ([12](12-panel-layout.md) §3.4); a strip on no rail stands alone and takes two.

`kind` says what the accessory is to a strip, in the strip accessory's own words (End plate, End bracket, Partition
plate, Marker carrier, Cover): the assembly list prints it and the check finds the end plate by it. `optional` marks
an accessory a person may take that the part does not need; no list counts it until he takes it. A part of kind wire,
ferrule or marker is never an accessory: the consumables count it ([13](13-manufacturing-outputs.md) §3.6), and the
part editor refuses it as one. The accessory is a library reference, so embedding a part embeds its accessories
(its closure, §3.1).

Three hands write it, each above the one before: the library's part; the project's copy (the part editor; its fork
of a library copy changes `accessories` and `variants` as it changes the pins); and, for one device, that device's
choice, `Device.accessory_choices: Vec<AccessoryChoice{of: Id<Part>, part?: PartRef}>`. A choice names the accessory
copy a declaration names (`of`): without `part` the device does not take it, with `part` it takes it as that part
(`of` itself takes an optional one, another part swaps it). A choice lives on the device the accessory is counted
for: a `unit` accessory on the device that uses the part, a `strip` and a `rail_group` accessory on the strip.
Replacing a part (`part.assign`) drops the choices of accessories the new part does not name.

**One path.** `part_accessories(device)` of `ew-derive` gives every accessory of a device from its parts'
declarations and its choices: the accessory part, the quantity, the part it comes with and where it stands in a
strip. The parts list, the assembly list, a strip's length on its rail, the end brackets of the layout and the
checks read it and nothing else. Two other things name a part beside a device, each for a reason of its own:

- a strip's own accessories (`Strip.accessories`, the strip editor's Add accessory): an item the person puts at a
  place in one strip, a partition plate after terminal 4, an end plate where the terminal type changes, a bracket in
  the middle of a long strip; no part can name a place in one strip;
- a bridge's part (the `overrides.wire_part` of its group's carrier, [08](08-connections-terminals-cables.md) §2.3):
  a bridge is drawn where the circuit asks, over as many terminals as it asks, and its part is chosen for that bridge
  as a cable's is.

A strip that holds an end bracket as a part use of kind `end_bracket` (a project made before accessories) reads it
as that strip's choice of end bracket: the rails count it as before, and its terminals' declarations are not asked
for that strip. No command gives a strip such a part use: the part dialog of a strip offers no end bracket.

**Variants** (FR-PRT-003). A variant is the part as its maker also sells it, in another colour or rating, under its
own number: one record, the data shared, the numbers its own. It has a key (the `PartRef.variant` a part use
stores, `BU`), a name in the reader's language ("Blue") and what it changes: its part number, order number, type
designation, GTIN, description and technical values (the colour among them, the native key `colour`), each absent
where it is the part's. It changes nothing of the pins, the template, the size, the sections or the accessories, so
a device keeps its connection points when its variant changes. A variant gives at least one of its own part number,
order number or description, else the parts list could not tell it from the part, and the part editor refuses it.
Every list prints a variant's numbers and description where it gives them, else the part's, and its technical
values where the device has none of its own (a device's own value wins over its variant's, which wins over its
part's). A part use naming a key its part does not hold prints the part and is reported (EW-CORE-085).

**The parts a wire consumes** (FR-PRT-035). The wire, the ferrule and the marker carry a section each, and every
member has a reader:

| Section | Member | Read by |
|---|---|---|
| `wire` | `csa` (mm²), `colour` | the stock match of a wire's part; EW-MFG-022 |
| | `class` (IEC 60228: 1, 2, 5, 6) | the treatment of the wire's ends ([13](13-manufacturing-outputs.md) §4) |
| `ferrule` | `csa_min`, `csa_max` | which ferrule an end takes ([13](13-manufacturing-outputs.md) §4.4) |
| | `twin` | two wires in one ferrule ([13](13-manufacturing-outputs.md) §4.3) |
| | `insulated` | the shop's choice of insulated ferrules |
| | `sleeve` | the strip length the wire list prints |
| | `colour` | the word the wire list and the consumables print for the ferrule the fitter takes |
| | `stud` | a ring lug for that stud (`M4`), worded Ring lug |
| `marker` | `marks` | the marker part of a label set |
| | `lines`, `chars` | whether a marker text fits (EW-MFG-004) |

A wire's size and colour come from the project's colour and size rules and the overrides
([08](08-connections-terminals-cables.md) §4), never from its part: the part is the stock it is cut from. A wire's
part is the one the chain names (on the wire, on its potential's definition, the project default), else the first
wire part of the workshop stock whose size and colour are the wire's, else none; a wire without a part is listed by
its size and colour. A wire part whose size or colour is not the wire's is reported (EW-MFG-022), and every list
prints the wire's colour, the one the drawing shows: a shop that cut by the part would cut the wrong one.

**The workshop stock.** `ProjectSettings.workshop: WorkshopSettings{stock: Vec<PartRef>, cut_increment?: Nm}`, a
typed member of the project's settings. `stock` holds the wire, ferrule and marker parts the shop keeps, in its
order of preference: a wire takes the first stock wire part of its size and colour, an end the first stock ferrule
that fits it ([13](13-manufacturing-outputs.md) §4.4), a label set the first stock marker that marks it
([13](13-manufacturing-outputs.md) §5.3). `cut_increment` is what the cut list rounds a piece up
to ([13](13-manufacturing-outputs.md) §9.3) and a computed wire length once (13 §3.3), 5 mm when absent. A new project starts with the starter's generic
ferrules and markers as its stock (`file.new` embeds and names them in its one step, §9.2); a company's basic project
names its own; a project made before has none, and Project ▸ Add the generic stock (`workshop.add_generic`) adds the
starter's ferrules and markers the stock lacks after its own, in one step. Project ▸ Edit stock… (and the link under the project's Properties) opens the stock dialog: the stock in its
order, each row moved or removed, a part added by its number (held or from a library), Add the generic stock and Cut
to; Apply is one `workshop.stock` with the stock and the cut increment.

### 2.6 Classification: ETIM bundled, ECLASS identifiers only

| Scheme | Shipped | Stored per part | Use |
|---|---|---|---|
| **ETIM** (English, `ODC-By-1.0`) | yes, as a versioned system data pack with attribution in About (FR-PRT-019) | class `EC…`, features `EF…` with values and units | categorising, faceted search, BMEcat/ETIM xChange import |
| **ECLASS** | **no** dictionary content (NFR-LIC-06) | class and property IRDIs as supplied by the user's files | ERP exchange, matching |
| **IEC CDD** | free attributes only | IRDIs | thermal and short-circuit properties |
| **Native** | yes | `ew:` keys (`ew:rated_current`, `ew:characteristic`, `ew:poles`, `ew:p_loss`) | everything checks and queries read |

**Checks and queries read only native keys.** A shipped mapping table translates ETIM features (and user-supplied
ECLASS mappings) into native keys at import time. A part that arrives with ETIM data therefore feeds the same
rules as one typed by hand ([standards](../research/standards.md) §10). ECLASS labels are shown only when the
imported file itself carried them.

### 2.7 Macros, variables, value sets and placeholders

A macro is the library item kind `macro`: the struct `MacroDef` of `crates/ew-schema/src/library.rs` with its
variants, variables, placeholder groups and value sets, each member with what it holds; its stored form is the
generated schema (`schemas/0.1/macro.schema.json`). A variant's key runs `A` to `P`: one variant at MVP, several per
representation type from 1.0 (FR-MAC-009); EPLAN's A–P range is unverified. A variable's 1.0 `formula` (a rhai
expression, FR-MAC-015) is specified; not built. A variant's fragment is the project fragment of ARCH §5.5 (the
struct `Fragment` of `crates/ew-commands/src/commands/paste.rs`): project-format records with IDs local to the
fragment, the library references of the closure its records name, and for a page macro its pages.

**Variable types:**
- `Text` (MlText), `Ident`, `Number` (Dec3 + unit), `Bool`;
- `State` (used, spare, reserved, not fitted), `Part` (a `LibRef`), `ProcessTag`;
- `ChannelBinding` (channel number in the module), `Address` (sets `AddrSpec::Manual`), `TagCounter`;
- `DeviceRef` (an existing device chosen at insertion, such as the assembly head a module is added to);
- `TextSlots`: a representation's text-slot
  overrides as a list, so a value set may move, turn, show or hide a symbol's texts; MC-4 keeps one `<group>_slots`
  variable per placeholder group.

A
`text_slots` value is typed and shown in words in the macro value table, in the window's language: "as the symbol"
for the empty list, else one clause per slot, "; " between clauses, each clause `slot <n>: at <x>; <y> mm, turned
<a>°, hidden` (or "shown") with every part optional and in any order, slots counted from 1. A number is written with
the project's decimal separator, and "; " separates the two coordinates in every language, so "at 10; -5,5 mm" and
"at 10; -5.5 mm" read the same, and the form written before ("at 10, -5.5") still reads back; a text that does not
read stays a text, which the command refuses in words.

A `FieldPath` is an `ew-schema` path such as `device.function_text`, `device.state`, `device.position`,
`device.parts[0].part` or `function.cp_overrides[3].designation`. Bindings are checked against the schema when the
macro is saved.

**A placed macro leaves a `MacroInstance`** `{macro: copy_id, variant, value_set, values, members: Vec<(local_id,
id)>}`. It is a record of [03](03-domain-model.md) §7, stored in the anchor page's file (kind `macro_instance`,
rank 46 of [05](05-project-format-and-persistence.md) §2.2), and gives the macro's provenance and "where used".
From 1.0 it is also the placeholder object that a different value set can be re-applied to (FR-MAC-008).

The kinds `macro` and `macro_instance` are
typed in `ew-schema` within format 0.1; `macros.save` writes a macro and `macro.insert` inserts one (§3.5).
- **`MacroDef`** is written in the member order of its struct: `meta, name, description, kind (window | symbol
  | page), variants, variables, groups, value_sets, thumbnail`, then `extra`. A variant is `{rep_type, key,
  fragment}`; the `fragment` is the `#ew-fragment v1` text format of ARCH §5.5, kept as JSON in the permissive schema
  crate and parsed by `ew-commands`, so `Fragment` is not a schema type. A variable is `{name, ty,
  bindings: Vec<{record (fragment-local ID), field}>, default?}` with `VarType` `text, ident, number, bool, state,
  part, process_tag, channel_binding, address, tag_counter, device_ref, text_slots` (`text_slots`, a representation's text slots as a list); the 1.0 `formula` stays in `extra`. A
  placeholder group is `{name, members (fragment-local IDs), include?}` and lives only inside the macro, so the
  project kind `placeholder_group` (rank 47) stays reserved. The copy lives under `lib/macros/` and
  `library.embed` embeds it with the checks of the other kinds.
- **`MacroInstance`** is `{id, $v?, macro, variant, value_set?, values, members, page}` in the anchor page's file.
  Its links are those of [03](03-domain-model.md) §7: the anchor page cascades, the macro copy is a forbid link (it
  cannot be removed while an instance names it), and each created member is a kind-erased unlink link, so deleting a
  member removes it from the list and the instance stays, also with an empty list.

**Addresses stay derived.** A value set normally sets the module position and the channel binding
("channel = 2*pos+1, module = {slot}", FR-MAC-015). Q0.1 then follows from `channel_addresses`; it is never typed
into the macro. Formula-computed `Address` variables exist for projects with manual addressing only (Open
question 8).

**Macro projects** (FR-MAC-013, Could, 1.0) are projects of kind `Macro` (FR-PRJ-026). A **macro box** is a page
item that names a macro key, kind, representation type, variant and target library and frames its content.
"Generate macros" saves every box as a macro version in one run (the acceptance: ten boxes, ten macros). Boxes
stay linked to the macros they generated, so a later run publishes new versions instead of duplicates; inserting
a macro never inserts its box.

### 2.8 Basic projects (company templates)

A basic project is an ordinary EW project with `Project.kind = Basic`. The `kind` field exists from MVP, because
FR-PRJ-009, FR-PRJ-010 and FR-MAC-011 need it; restricting commands by kind (FR-PRJ-026, for example "a basic
project cannot be released") is 1.0. It holds:
- the schemes and profiles of FR-PRJ-008: numbering (including per-class rules such as the pilot's strip rule
  `{letter}{page}{counter}`, which gives -X1031; the formatter adds the aspect sign), xref format, colour and size
  profile "DBU/WH 24 V", identifier set, check scheme and derivation profile;
- the grid for new pages: the native grid or the QET 10 px grid (ADR-0027 recommendation 4);
- frames and forms, text styles, sections with dividers, structure nodes;
- the revision format, languages, report blocks 903–911 and optional template pages;
- a **palette**: embedded copies pinned for use, such as favourite symbols, standard parts and macros.

It is produced by the company-standard wizard (FR-PRJ-010, FR-UI-030) or by "save as basic project", which can
leave out pages, devices or both (FR-MAC-011).

**Creating a project from a basic project** (FR-PRJ-009):
- copies every record, with new IDs `v5(new_project_id, source_id)` (instantiated records, ADR-0004), where
  `new_project_id` is the new project's UUIDv7;
- keeps library copies unchanged, because their IDs derive from library and item, not from the project;
- records `Project.basic_project = {id, version}` for later comparison.

The member is typed as
`Project.basic_project = {id, generation}`: the basic project's project ID and its manifest generation when it was
copied, because a basic project has no semantic version. A project made from a directory records it
(`Session::new_project`).

