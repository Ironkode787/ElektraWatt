# Project format and persistence: principles, directory layout and canonical encoding

A chapter of [Project format and persistence](05-project-format-and-persistence.md), whose table says where each section lives.

---

## 1. Principles and locations

1. **Text is canonical at rest, the `Model` in memory** (ARCH §7.1): no working database, no derived data, no blobs
   except content-addressed assets.
2. **Each record lives in exactly one file**, chosen by an ownership rule (§2.2); files group records by page,
   device shard or layout space, never one file per object. A typical edit rewrites one to three files.
3. **Names from UUIDs, hashes and fixed words only.** There are no reserved names or case collisions, and internal
   paths stay ≤ 120 characters (NFR-PLAT-05).
4. **A generation is complete or absent.** The manifest goes into place last (NFR-REL-01).
5. **No machine-, user- or time-specific project data:** no wall-clock save times, absolute paths or view state
   (NFR-REL-07, FR-UI-033). Authored dates (a revision date, the edited date of FR-PRJ-029) are model data.
6. **Unknown bytes are never dropped** (NFR-REL-04).
7. **Crash protection lives outside the project** (§8). EW writes only to the project, to sibling archive and
   backup files the user chose, and to per-user OS directories, never to its installation directory (NFR-PLAT-07).

| Location | Content | In git |
|---|---|---|
| `<name>.ew/` | Project data (§2) | Yes |
| `<name>.ew/.ew/u-<hash>/` | Per-user view state in `view.json`: open pages, viewports, jump history, workspace (FR-UI-033). `<hash>` = 8 hex digits of SHA-256(`user@host`), so the users of a shared folder do not overwrite each other. From 1.0 also `base.ewz`, the base generation of a work copy ([15](15-checks-revisions-collaboration.md) §6.8) | Ignored |
| `<name>.ew/.~ew.lock` | Lock record (§7); `.~ew.lock.stale-<session>` after a takeover | Ignored |
| `<name>.ew/.~ew.requests/` | "Request edit" files (1.x, [15](15-checks-revisions-collaboration.md) §6.2) | Ignored |
| OS state directory | Journal and recovery snapshots (§8); user-only permissions; removed on clean close, leftovers pruned after 30 days (NFR-PRIV-05). Also `releases/`, one record per release run of a project directory and archive (§14.1), removed when the run is settled or found stale | Never |
| OS cache directory | STEP meshes, thumbnails, the file-backed SQL projection at XL, read-only `.ewz` extractions, the derived per-user library search index (§13.1) | Never |
| OS data directory | User library and installed community packs ([07](07-symbols-parts-macros.md) §2.1) | Never |
| OS configuration directory | Settings, recent projects, **trust decisions** (§13.4) | Never |
| `<parent>/<name>.ew-archive/` (configurable) | As-built archives (§14) | Not in the project history |

The OS directories are `$XDG_STATE_HOME`, `$XDG_CACHE_HOME`, `$XDG_DATA_HOME` and `$XDG_CONFIG_HOME` (each with an
`elektrawatt/` subfolder) on Linux; `~/Library/Application Support/ElektraWatt` and `~/Library/Caches/ElektraWatt`
on macOS; and `%LOCALAPPDATA%\ElektraWatt` on Windows, never the roaming profile. Where one base directory serves
several roles (macOS, Windows), EW uses the subfolders `config/`, `data/`, `state/` and, on Windows, `cache/`. The
mapping crate (etcetera or directories) is **(verify)**, as in ARCH §3.

**Equivalents.** EW maps the EPLAN and QElectroTech project forms ([EPLAN features](../research/eplan-features.md)
§2, §13; [gaps and corrections](../research/gaps-and-corrections.md) G1) as follows:

| EPLAN | QElectroTech | ElektraWatt |
|---|---|---|
| `*.elk` file plus `*.edb` project database | one `.qet` XML file (or `.qetz`) | `<name>.ew/` text directory (§2) |
| `*.elp` packed project, `*.zw1` backup | copy of the `.qet` | `.ewz` (§12) |
| `*.ell` revision project, `*.elr` completed project | none (per-folio `indexrev` text) | as-built archive (§14); the completed state of [15](15-checks-revisions-collaboration.md) §5.10 (1.0) |
| `*.zw9` basic project | none (copy of an existing project) | an `.ew` directory with `Project.kind = Basic` ([07](07-symbols-parts-macros.md) §2.8) |
| Open modes Default, Exclusive, Read-only; multi-user editing (Pro tier) | none | Edit or read-only through `.~ew.lock` (§7); parallel work through git branches or work copies; co-editing Later (FR-COL-021) |
| — | three rotating crash snapshots | journal plus three snapshot slots outside the project (§8) |

---

## 2. Directory layout and record ownership

### 2.1 The layout, by example

The layout, shown on a project of the reference drawing set (its names and counts are that project's). The sample
project `fixtures/sample/sample.ew` has the same layout. Records imported from tables have UUIDv5 IDs derived from
their rows, as `ew import csv` derives them; records created in the GUI get UUIDv7 IDs (ADR-0004).

```text
J-E-2416.ew/                      # folder name chosen by the user; everything inside is UUID-named
  manifest.json                   # written LAST (§4): format, generation, SHA-256 of every file below
  project.jsonl                   # project (master data, parties as placeholders, supply, settings: identifier set,
                                  #   numbering, xref format, profile "DBU/WH 24 V", size profile, derivation profile),
                                  #   plc_config_project, logical_net and physical_net (PROFIBUS)
  structure.jsonl                 # =P01, +J-E-2416 (alias JE2416), +J-E-206, +FIELD; sections 1, 2, 9;
                                  #   external_ref ×4 (J-E-206 L, N, PE; DP master JE-N1000-03)
  revisions.jsonl                 # V1.0 (released, historic), V2.0 (open)
  report-blocks.jsonl             # 11 page-oriented report blocks: TOC, 901, 903 … 911
  waivers.jsonl                   # W-1 on -XPE      (comments.jsonl absent: empty tables write no file)
  pages/<uuid>.jsonl              # 13 files, one per drawn page: 1, 3, 101–103, 106, 201–206, 902. Generated pages
                                  #   (2, 2.1, 100, 200, 900, 901, 901.1, 903–911) have no file: they are derived
  devices/<xx>.jsonl              # ≈ 80 of 256 shards, e.g. c2.jsonl holds -102G1 and everything anchored to it
  layout/<uuid>.jsonl             # layout space +J-E-2416: -UC1, plate, ducts, rails, placements
  lib/{symbols,fndefs,forms,parts,macros,manufacturers}/<uuid>.json
                                  # embedded copies (§13): ≈ 40 symbols, the QET-compatible frame and report forms,
                                  #   23 parts (533356, A9P52610, 2904602, PT 2.5-TWIN, 571334 …), the 203–206 typical,
                                  #   the manufacturers the parts reference
  assets/<sha256>.svg             # placeholder logo
  .gitattributes  .gitignore      # §11.1; not listed in the manifest
```

Optional directories and files, absent in the example:
- `rules/<uuid>.toml` (a project rule pack; its rhai, SQL, Fluent and doc files go into `rules/<uuid>/`; the human
  pack ID such as `EW-POT` lives inside the file, [15](15-checks-revisions-collaboration.md) §3.7) and
  `reports/<uuid>.sql` (user SQL reports);
- `imports/<run-uuid>.jsonl` (MVP): one file per import run (kind `import_run`), so import runs never conflict with
  project data; the lineage they belong to is an `import_lineage` record in `project.jsonl`
  ([14](14-interop.md) §2.3, ADR-0004 amendment 1); the QET project import writes
  one such file, one line, per import into a lineage, never changed after its transaction and deleted by no command;
  `ImportLineage.last_run` links the lineage's last run; the CSV bootstrap, the parts table import and the
  table round trip write none;
- from 1.0, `scripts/<uuid>.rhai` and `jobs/<uuid>.ewjobs` (ADR-0010 amendment 2);
- from 1.0, `exchange/<config-project-uuid>.jsonl`: the PLC exchange records and foreign IDs of one configuration
  project (kinds `exchange_record`, `foreign_id`), included in the as-built archive; the `PlcConfigProject` keeps
  only the exchange stamp ([10](10-plc-and-bus.md) §4.2). Both kinds are typed within format
  0.1 (ADR-0011 amendments 4 and 6); a record names its configuration project (`config_project`, a load-tolerant
  link that cascades) and the record its object is (`target`, history and no link); the file exists once a
  configuration project was exchanged or an instrument list applied;
- from 1.0, `lib/plc-profiles/<copy_id>.json` (user PLC address profiles, kind `plc_profile`; built-in profiles are
  versioned and never removed) and `lib/profiles/<copy_id>.toml` (machine and label profiles kept in their TOML
  source form, so that the SHA-256 equals the published item; the `schema` key tells the two apart,
  [13](13-manufacturing-outputs.md));
- from 1.0, `trust.sig` at the root (a signature over the trust hash, §13.4) and opt-in embeddable meshes
  `assets/<sha256>.glb` ([12](12-panel-layout.md) §6.2).

User PLC address profiles in
`lib/plc-profiles/<copy_id>.json` (kind `plc_profile`, §2.2's row) are the MVP's, not 1.0's (FR-PLC-008), and the built-in profiles are library documents of the kind too. The
machine and label profiles of `lib/profiles/` stay 1.0's.

The kind `plc_profile` is `PlcProfileDef` in `ew-schema` (doc 10 §2.5's members), `LibDocument::PlcProfile`, the record directory
`plc-profiles` of a library and `lib/plc-profiles/<copy_id>.json` of a project, item keys `ew.plc-profile.<id>`, the
schema in `schemas/0.1`. The built-in profiles are compiled into `ew-derive` from `content/plc-profiles/`; a project's
copy of a profile is found before the built-in of the same pin everywhere.

None of the rule, report, script or job content runs until the user trusts the project (NFR-SEC-05, §13.4).

**Temps, orphans and strays** (ADR-0010 amendment 3). The save protocol's
own temps ("our temps") are the intents `manifest.json.tmp-<g>` and the `.tmp-<g>` files of record files, library
documents and assets (`assets/<sha256>.<ext>.tmp-<g>`); an editing open
deletes or rolls forward only those (§5.3, §6.1). Every other file that the
manifest does not list is left alone: with a manifest present it is never read, never deleted and reported as
EW-CORE-004 (§6.2). A save that needs the path of such a file first moves it aside to
`<stem>.unlisted-<g>[-<n>].<ext>` (§5.2), a **stray** name that the application never reads, writes or deletes.

### 2.2 Kind registry and ownership

`ew-schema` holds a **kind registry**: the `$k` name, a fixed **rank** (sort order, §3.3) and an **ownership
rule**. Ranks are never reused or renumbered.

| Rank | `$k` | File | Anchor |
|---|---|---|---|
| 10–14 | `project`, `plc_config_project`, `logical_net`, `physical_net`, `medium` | `project.jsonl` | — |
| 15 | `import_lineage` | `project.jsonl` | — |
| 20–22 | `structure_node`, `section`, `external_ref` | `structure.jsonl` | — |
| 30 | `page` | `pages/<id>.jsonl` | own ID |
| 31 | `device` (with every aspect: strip, terminal, cable, assembly, PLC, bus, valve terminal) | shard | own ID |
| 32 | `function` | shard; an interruption-point (IP) function without a device goes to its page, which owns it: deleting the page or the IP representation deletes the function (ADR-0009 amendment 3), and an IP function has exactly one representation (EW-INV-013) | `device`; IP: page of its representation |
| 33, 35 | `representation`, `junction` | page file | `page` |
| 34 | `connection` | page file if `page` is set, else a shard | `page`, else the device owning endpoint `a` |
| 36, 37 | `conductor`, `channel` | shard | `cable`, `module` |
| 38–41 | `potential_def`, `net_def`, `medium_def`, `bond` | shard (junction endpoint: its page) | device owning `at` (bond: `a`) |
| 42 | `device_link` | shard | `from` |
| 43–45 | `page_item` (variants include `SurfaceView`, `ModelView`, `Dimension` and `Underlay`), `structure_box`, embedded `report_block` | page file | `page` / `embedded.page` |
| 45 (same kind) | page-oriented `report_block` | `report-blocks.jsonl` | — |
| 46 | `macro_instance` | page file of the macro's anchor page | anchor page |
| 47 | `placeholder_group` (reserved: [03](03-domain-model.md) §7 keeps placeholder groups inside `MacroDef`, so no project record uses it yet) | page file of the anchor page | anchor page |
| 50–55 | `layout_space`, `enclosure`, `mounting_surface`, `carrier`, `placement`, `route_override` | `layout/<space>.jsonl` | space via carrier → surface → enclosure |
| 56 | `committed_route` | `layout/<space>.jsonl` | its `space` |
| 57, 58 | `routing_path`, `surface_feature` | `layout/<space>.jsonl` | space |
| 60–62 | `revision`, `waiver`, `comment` | own root file | — |
| 63 | `transmittal` | `revisions.jsonl` | its revision |
| 70, 71 | `exchange_record`, `foreign_id` (1.0) | `exchange/<config-project-uuid>.jsonl` | configuration project |
| 72 | `import_run` | `imports/<run-uuid>.jsonl` | own ID |
| — | `part`, `symbol`, `fndef`, `form`, `macro`, `manufacturer` (library copies) | `lib/<kind>s/<id>.json`, one pretty document each (§3.4) | own ID |
| — | `plc_profile` (library copy, 1.0; implemented at the MVP) | `lib/plc-profiles/<id>.json` (§3.4) | own ID |
| — | `machine_profile`, `label_profile` (library copies, 1.0) | `lib/profiles/<id>.toml`, the TOML source form; the `schema` key tells the kinds apart | own ID |
| 255 | `conflict` | file of its target | target |

**The project record's text styles** ([ADR-0032](../adr/0032-per-project-text-styles.md)). `ProjectSettings.text_styles` is an optional member of the `project` record in `project.jsonl`: an object keyed by style key, each value `{"size": <em height in nm>, "weight": "regular" | "bold"}` (weight optional) plus unknown members kept verbatim, written in key order. It is additive within format 0.1 (ADR-0011 amendment 4): no version step, and an older 0.1 build keeps it in `settings.extra`. An absent or empty member means the built-in table; nothing writes it into a file that lacks it except `Session::new_project`, which writes `cp.small` and `marker` at 1,852,083 nm (7 QET px). On reading, an override whose size lies outside 500,000 … 50,000,000 nm is ignored (the built-in row stays) and a weight of a newer format reads as absent; `project.settings.set` refuses both at `/text_styles/<key>/size` or the member's path. `ProjectSettings.revision_label` is declared before `text_styles`, in byte order of the names, so a line that held both as unknown members stays canonical when a newer build types them. `ProjectSettings.typeface` ([09](09-reports-forms-output.md) §4.8), `{"asset": "sha256:<64 hex>", "name": "<file name>"}`, is the last declared member, additive in the same way: an older build keeps it in `settings.extra` and its file as an opaque asset file.

**`page_item` and `structure_box`.** `page_item` is rank 43 and `structure_box` rank 44, each
owned by the page file of its `page` member; in `ew-schema` both are `KindStatus::Implemented`. Their typed tables
are additive within format 0.1 (ADR-0011 amendment 4, §10.3 rule 2): no format bump and no `$v` bump, and an
older 0.1 build keeps the records verbatim. The domain model's [03](03-domain-model.md) §5 (`StructureBox`) and §6
(`PageItem`) carry the fields. Illustrative records on a folio 101 (IDs shortened, the connection is W7):

```json
{"$k":"page_item","id":"…","page":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a","kind":{"wire_marker":{"connections":["b29e4f17-8d63-5a0c-9f71-3c6a2e8d05b1"],"at":[90000000,24000000],"fields":["csa","colour"]}}}
{"$k":"page_item","id":"…","page":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a","kind":{"graphic":{"shape":{"rect":{"min":[20000000,40000000],"max":[80000000,64000000]}},"dash":"dashed"}}}
{"$k":"structure_box","id":"…","page":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a","outline":[[20000000,40000000],[80000000,40000000],[80000000,64000000],[20000000,64000000]],"style":"dotted","caption":"External Connection","assigns":{"location":"a41f6c2d-8e05-5d97-b3c1-5f2e8a7d9c60"}}
```

- **`page_item`:** `{"$k", "id", "$v"?, "page", "layer"?: Ident, "kind": <variant>}`, the variant an externally tagged
  object: `graphic`, `free_text`, `wire_marker` or `surface_view` (`{"surface", "datum_at", "label_mode"?}`, on a
  `panel_layout` page only, set by `page.surface_view.set`), `image` and `underlay`. Their members,
  enumerations and defaults are in [03](03-domain-model.md) §6. Any other variant (`macro_box`, `model_view`,
  `dimension`) is kept verbatim as `PageItemKind::Unknown(Value)` (§10.3 rule 3) and round-trips
  byte for byte, as does a variant of a newer format.
- **`structure_box`:** `{"$k", "id", "$v"?, "page", "outline": [Pt2] (at least 3), "style": "dotted" | "dash_dot",
  "caption"?: MlText, "assigns": StructureAssignment, "external"?: IdS}` ([03](03-domain-model.md) §5).
- **Links and invariants.** The `page` of either kind is a cascade link (deleting a page deletes its items and boxes,
  FR-PAG-025), and the connections a wire annotation lists are held by two new structural invariants of ADR-0009
  (EW-INV-022 and EW-INV-023, [03](03-domain-model.md) §6): a deleted connection leaves the list, and a marker left
  empty is deleted in the same plan. `StructureBox.external` (an `ExternalRef` ID) is an unlink
  link: deleting the reference clears the member.
- **Load finding.** A build from before these kinds were typed keeps `page_item` records verbatim and does not enforce that delete
  link, so a project can arrive with a wire annotation that lists a deleted connection or a connection on another page.
  This is **reported, not refused**: a load finding that names the marker and the connection (a derivation finding, not
  an integrity rule of §6.3; specified; not built: `Session::load_findings` is gone, so nothing is reported). The stale entry is ignored
  by the derivation and the drawing, and the project opens; the next edit of that marker writes a clean list.
- **Migration test.** A page file that holds both kinds and an unknown `page_item` variant is read and written back byte
  for byte (an unknown variant is `Unknown(Value)`, written back unchanged, §10.3); the same records as an earlier build wrote them (reserved kinds, so every object's members in byte-wise order) load typed with a `NonCanonical` finding and are rewritten in the typed kinds' member order (§10.3; the revision precedent). `free_text.text` is omitted when empty, `arc.start` when 0 and `$v` when 1, so an explicit `"$v": 1` is not canonical.

**`macro_instance`, `macro` and `manufacturer`**.
The three kinds turn `Implemented`, each at schema version 1, under the same rule (ADR-0011 amendment 4, §10.3 rule 2):
`schemas/0.1` is re-blessed with the new `macro.schema.json`, `macro_instance.schema.json` and
`manufacturer.schema.json`, no format and no `$v` bump, no `required` list changed and no property removed.
- **`macro_instance`** (rank 46) lives in the page file of its `page` member, the anchor page:
  `{"$k", "id", "$v"?, "macro", "variant", "value_set"?, "values"?, "members"?, "page"}`, where `macro` is the macro
  copy's ID and `members` a list of `[fragment-local ID, created ID]` pairs ([03](03-domain-model.md) §7). Its links:
  `page` cascade, `macro` forbid, `members[][1]` a kind-erased unlink (a deleted member leaves the list; the instance
  stays). An older 0.1 build keeps its lines verbatim as a reserved kind and does not apply these links.
- **`macro` and `manufacturer`** are library copies under `lib/macros/` and `lib/manufacturers/`, one pretty document
  each (§3.4), read and written and hashed like the other library kinds (§13.2).
- **`placeholder_group`** (rank 47) stays reserved.

Channel IDs are `v5(module, "channel", number)` ([10](10-plc-and-bus.md) §2.4, ADR-0004 amendment 2), so a channel
keeps its file and line position however often its module is re-addressed.

**Anchor fallbacks** (format level). Every record of a known kind has exactly one
deterministic file, also when its anchor cannot be resolved the normal way:
- a page-less record anchored at a connection point of an interruption-point function (which has no device), such
  as a page-less connection or a potential definition at that point, follows the function into its page file;
- an interruption-point function without a representation (a broken EW-INV-013) goes to the shard of its own ID;
- a dangling anchor (the referenced function or junction does not exist, EW-CORE-008) goes to the shard of the
  referenced ID, so the record keeps its file when the reference is repaired; an endpoint of a newer format goes to
  the shard of the record's own ID;
- a record of a reserved or unknown kind gets a file only where its rule needs nothing but the record itself (a
  root file, `imports/<own id>.jsonl`, the page named by its `page` member); otherwise it stays in the file it was
  read from (§10.3).

A shard is `devices/<xx>.jsonl`, where `xx` is the anchor ID's last byte as two lower-case hex digits. The
[domain model](03-domain-model.md) §14 allows a `LogicalNet` in its master's shard or in `project.jsonl`; this
format always uses `project.jsonl`, so that a change of master (an external partner such as JE-N1000-03) does not
re-key the net.

**Follow relations.** Some records take their file from another record. A transaction that changes one of these
source fields re-keys the dependent records, and the writer marks both their old and their new files dirty:

| Source field changes | Records re-keyed |
|---|---|
| `Representation.page` of an IP representation | its IP function |
| `Function.device` | page-less connections, potential, net and medium definitions and bonds at the function's CPs |
| `Carrier.surface`, `MountingSurface.enclosure`, `Enclosure.space` | dependent layout records |

Ranks 50 to 54 (`layout_space`, `enclosure`,
`mounting_surface`, `carrier`, `placement`) live in `layout/<space>.jsonl`: a space owns itself, an
enclosure names its space, and a surface, carrier and placement reach it through carrier → surface → enclosure. The
follow closure re-keys the dependants of `Carrier.surface`, `MountingSurface.enclosure` and `Enclosure.space`, so a
carrier moved to another surface moves its placements' lines, and an incremental save equals a full save (LM-1).
Rank 56, `committed_route`, is typed (below); ranks 55, 57 and 58 (`route_override`, `routing_path`,
`surface_feature`) have no table and are kept and written as unknown records.
Ranks 50 to 54 and 58 (`layout_space`, `enclosure`,
`mounting_surface`, `carrier`, `placement`, `surface_feature`) live in `layout/<space>.jsonl`: a space owns itself,
an enclosure names its space, and a surface, carrier, placement and surface feature reach it through carrier → surface
→ enclosure. The follow closure re-keys the dependants of `Carrier.surface`, `MountingSurface.enclosure` and
`Enclosure.space`, so a carrier moved to another surface moves its placements' lines, and an incremental save equals
a full save (LM-1). Ranks 55 to 57 (`route_override`, `committed_route`, `routing_path`) have no table and are
kept and written as unknown records.

A property test guards the table: after any random command sequence, incremental saves must equal a full save,
byte for byte (NFR-REL-16).

**Committed computations** (`Committed<T>{value, input_hash, rev}`, ADR-0003) are persisted without `rev`, which is
a session-local counter. Frozen wire numbers live in `Connection.number`. A released revision's report snapshots live
in its archive (§14).

**`committed_route`** ([12](12-panel-layout.md) §8.5): the stored way of one wire, written only by Route, in the
layout file of its `space`, with ID `v5(wire, "route")`: `{"$k", "id", "$v"?, "space", "wire", "ends": [End, End],
"way": [Step], "run": <nm>}`, where `End` is `{"cp": [<function>, <idx>], "surface",
"at": [<u>, <v>], "h": <nm>}` and `Step` is externally tagged, `{"duct": {"carrier", "from", "to"}}` or `{"door":
<surface>}`, a step of a newer format kept as `Unknown(Value)`. `space` and `wire` (the wire's carrier connection) are
cascade links. The kind was reserved with its rank, so its typed table is additive within format 0.1 (ADR-0011
amendments 4 and 6): no format and no `$v` bump, the published schemas re-blessed with `committed_route.schema.json`.
An older build keeps the lines verbatim and does not apply the links; a record whose wire or space is gone is not read
and the next Route removes it.

---

## 3. Canonical encoding

### 3.1 Files and records

- **Files.** UTF-8 without BOM, LF line endings on every platform, one record per line, a final LF, no blank lines.
  The reader accepts one leading BOM in a JSONL file and in a library document (§3.4) and the next save rewrites the
  file without it: a reader tolerance, not a format change (no ADR).
  A file that would be empty is not written; if it exists, it is deleted.
- **Lines.** Each line is a compact JSON object: no whitespace outside strings. String escaping equals serde_json's
  compact output:
  - `"` and `\` are escaped;
  - control characters become `\b \f \n \r \t` or `\u00xx`;
  - non-ASCII characters stay raw;
  - `/` is never escaped.

  The form is close to RFC 8785 (JCS), but members follow the schema order and numbers are integers only. The
  byte-wise key order of `extra` relies on serde_json's sorted `Map`; its feature `preserve_order`, which would
  switch every `Map` in the build to insertion order, is banned by `ci/dep-rules.toml` and guarded by a unit test
 .
- **Strings are NFC** (ARCH §7.3). A non-NFC string read from disk is normalised in memory, and its file counts as
  non-canonical, so it is rewritten at the next save; `ew fmt --check` reports it.
- **Member order:**
  1. `"$k"`, `"id"`, `"$v"` (per-kind schema version, omitted when 1, §10.2);
  2. known members in **schema declaration order**;
  3. flattened device aspects (`strip`, `terminal`, `cable`, `assembly`, `plc_station`, `plc_module`, `bus_node`,
     `valve_terminal`), as in the ARCH §7.3 example (`assembly` is written before `plc_station`);
  4. the `extra` map (unknown members and namespaced `plugin.<id>.*` keys, FR-EXT-021), in byte-wise key order.
- **Defaults omitted.** A member equal to its schema default is not written: `None`, empty arrays and maps, `false`
  flags and default enum variants. Where the default is not the type's zero value the schema names it. The
  defaults: `Page.scale` `not_to_scale`; `Representation.rep_type` and `SymbolDef.rep_type` `multi_line`;
  `Junction.wiring` absent means the style's default (Dot: star, Bevel: chain); `Cable.length` `missing`;
  `ItemMeta.status` `released`; `CpDef.discipline` and `SymbolCp.discipline` `electrical`; `Strip.field_side`
  `side2`; `CpDef.physical` `true` (written only as `false`).
- **`extra` at every depth.** Every JSON object of the format, records and nested value objects (`Quantity`,
  `PartRef`, `Folio`, `WireOverrides` …) alike, has its own `extra` map, flattened last, so a member that a newer
  minor appends at any depth survives a load and a save (ADR-0011 amendment 2).
- **Import provenance last.** `source_ref?{lineage, key}` ([14](14-interop.md) §2.3) is the last declared member of
  every project record kind except `import_lineage`, among the members of the released format; in a device only the
  flattened aspects and `extra` follow it. A member added after the format locked is appended after `source_ref`
  (ADR-0011 amendment 6, §10.3 rule 1), where an older build that keeps it in `extra` writes it too, so that a record
  an older build rewrites keeps its member order.
- **`$v` is checked.** A typed reader accepts only the registry's `$v` of each kind; older records are migrated first
  (§10.2).

### 3.2 Value encodings

| Type | JSON | Example |
|---|---|---|
| `Id<T>` | lower-case hyphenated UUID | `"c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2"` |
| Integer, `Nm` | integer with \|n\| ≤ 2⁵³ − 1 (safe for JavaScript and jq); no `-0` or exponent | `137583333` |
| `Pt2`, `Grid` | `[x, y]` in nm; reduced `{"num","den"}` with `den` > 0 | `[42333333,15875000]`, `{"num":7937500,"den":3}` |
| `Rect` | `[[x0, y0], [x1, y1]]`: two `Pt2`, the minimum corner first (`x0 ≤ x1`, `y0 ≤ y1`); a degenerate rectangle is allowed, a reversed one is a schema violation. Persisted by the embedded `report_block` (`embedded.rect`), form `DataArea.rect` and `FormCell.rect` (title-block and report cells), and `SurfaceView.clip`. `ew-schema` implements the encoding; `ew-units` keeps `Rect` free of serde | `[[10000000,180000000],[120000000,270000000]]` |
| `Orient2` | string; default `"r0"` omitted | `"r90"`, `"m0"` |
| `Angle2` | integer in 10⁻³ degree; default 0 omitted. Free texts and graphics only; symbol representations use `Orient2` | `"angle":45000` |
| `Pose3` | `{"t":[x,y,z],"q":[w,x,y,z]}`, `q` in 10⁻⁹, sign-normalised (w > 0, else the first non-zero component > 0) | `{"t":[0,500000000,0],"q":[1000000000,0,0,0]}` |
| `Dec3` | decimal string: at most 3 fraction digits, no trailing zeros, no `+`, `-0` or exponent, never a unit | `"0.75"`, `"40"`, `"1"` |
| `Quantity`; `TechValue` | `{"value": Dec3, "unit": Unit}` with the unit spellings below; `{"key", "value"}` whose value is a `Quantity` object or an `Ident` string, told apart by JSON type. A key qualified by a classification scheme is `"<scheme>:<id>"` (schemes `etim`, `eclass`, `iec_cdd`); a plain key without a colon is a native or project property. Booleans and enumeration values are `Ident`s (`"true"`, `"false"`, the member) until a typed form is needed ([07](07-symbols-parts-macros.md) §2.5) | `{"key":"i_out","value":{"value":"20","unit":"A"}}`, `{"key":"curve","value":"C"}`, `{"key":"etim:EF001234","value":"true"}` |
| `Csa` | a bare `Dec3` string in mm²; `{"awg": n}` with 1/0 as 0, 2/0 as −1, 3/0 as −2, 4/0 as −3; `{"kcmil": Dec3}` | `"2.5"`, `{"awg":18}`, `{"kcmil":"250"}` |
| `MlText` | string (neutral) or object keyed by sorted BCP 47 tags | `"1+"`, `{"en-GB":"Power Supply 1 - Main Current"}` |
| `Ident` | NFC string without surrounding white space | `"A9F78116"` |
| `ColourSpec` | an array of one or two `ColourCode`s. A `ColourCode` is a string: one of the 14 IEC 60757 codes (`BK` … `SR`), else a company code, which the colour profile must declare (EW-CORE-031 checks it; it is not a parse error); a foreign code (lenient projects only, EW-CORE-031) is the data variant `{"foreign": code}` | `["BK"]`, `["GN","YE"]`, `["DBU"]`, `[{"foreign":"BR"}]` |
| Authored date | `"YYYY-MM-DD"` | `"2025-04-14"` (a revision date, entered by a person) |
| Enum | unit variant as a snake_case string; data variant as a one-member object. Persisted enums are open: an unknown value is kept as `Unknown` and written back unchanged (§10.3); the JSON Schemas omit the catch-all (§10.4) | `"tbd"`, `{"core":"<id>"}`, `{"manual":"Q0.1"}` |
| `Endpoint` | `{"cp":[function, idx]}` or `{"junction": id}` | `{"cp":["d40b6e93-…",0]}` |
| Structure slot | absent = inherit; a node ID; `"none"` | `"location":"a41f6c2d-…"` |
| `Aspect` (of a structure node) | snake_case string: `functional_assignment`, `function`, `installation_site`, `location`, `document_kind`, `user_defined`, `type`. `product` (`-`) is the device tree and never a `StructureNode.aspect`: the reader rejects it there and the JSON Schemas omit it ([03](03-domain-model.md) §5); another unknown string is kept like any open enum value (§10.3) | `"location"` |
| Sequence / set / keyed list | A sequence keeps model order. A **set** is sorted by the compact JSON text of its elements in byte order, so `"ab!"` precedes `"ab"` (`!` is 0x21, `"` is 0x22); the sets are `aliases`, `translations`, `keywords`, `manual.include`, `manual.exclude` and `Arrangement.slots`. A **keyed list** is sorted by its key in the key type's order (integers numerically, `Ident` keys byte-wise): `cp_overrides`, `cp_pattern` and `cps` (of `SymbolDef`, `FnDef` and `ExtraVariant`) by `idx`, `tech_data` and `free_props` by `key`, `pin_map` by `symbol_cp`, `text_slots` by `slot`. The reader rejects an unsorted set or keyed list and a duplicate element or key. NFC normalisation on load (§3.1) can change a set's order; `ew-format` re-sorts it when it rewrites the non-canonical file | `route`; `aliases`; `cp_overrides` by `idx` |
| `Rgb` | `"#rrggbb"`, lower-case hex | `"#c0c0c0"` |
| `Pen` | a house pen `"p013"`, `"p018"`, `"p026"`, `"p042"`, `"p052"`, `"p130"`, or `{"width": Nm}` | `"p026"`, `{"width":183000}` |
| `Primitive` | a one-member object named by the primitive kind: `line`, `polyline`, `rect`, `circle`, `ellipse`, `arc`, `polygon`, `text`, `image` | `{"line":{"from":[0,0],"to":[0,8000000],"stroke":{"pen":"p026"}}}` |
| `CategoryKey` | a three-element array: category, group, definition | `["ProtectionOvercurrent","MCB","1P+N"]` |
| `SortKey` | `{"key": Ident, "descending": bool}`, `descending` omitted when false | `{"key":"folio"}` |
| Hash | `"sha256:<64 lower-case hex>"` | manifest, library copies |

**Unit spellings** (`Quantity.unit`, ADR-0005 amendment 4). Units are written in ASCII so files stay greppable;
rendering ("mm²", "°C") is a query. Geometric lengths are `Nm`, never a `Quantity`.

| JSON | Unit | Used for |
|---|---|---|
| `mm2` | square millimetre | cross-sections |
| `A` | ampere | currents |
| `V` | volt | voltages |
| `Hz` | hertz | frequencies |
| `bar` | bar | pressures |
| `degC` | degree Celsius | temperatures |
| `kg` | kilogram | masses |
| `W` | watt | losses and power |
| `m` | metre | lengths in technical data; the order quantity of cables |
| `piece` | piece | counts; the default order unit |
| `set` | set | order unit |
| any other string | `Unknown`, kept verbatim | a unit of a newer format |

**There are no floats anywhere** (ADR-0005). A number with a fraction or an exponent is a schema violation
(EW-CORE-007).

### 3.2a Derived identities

UUIDv5 IDs (ADR-0004 amendments 1, 2 and 4) are computed from a namespace and a name. The bytes that enter the hash
are part of the format and never change; `ew-ids` (`v5` module) implements them and pins them with fixed test
vectors.

- **Namespace:** the parent UUID the formula names, or a fixed constant. Each constant is a random v4 UUID minted
  once, fixed forever and held by `ew-ids`:

  | Constant | Value | Used by |
  |---|---|---|
  | `NS_LIBCOPY` | `04c1640f-8857-4dfc-9bff-e9baad47fb47` | embedded library copies (§13.2) |
  | `NS_IMPORT` | `7b75186c-f28e-49ce-b7f7-26b137095a65` | import-lineage namespaces ([14](14-interop.md) §2.3) |

- **Name:** the UTF-8 bytes of the components' canonical text forms, joined by U+001F (UNIT SEPARATOR, written ␟):
  UUIDs lower-case hyphenated; integers in decimal without sign, padding or separators; the literals `channel` and
  `route` verbatim; source keys and version strings exactly as given. U+001F cannot occur in a UUID or a decimal
  integer, so the joined name is unambiguous.
- **Hash:** `Uuid::new_v5(namespace, name)` (SHA-1, RFC 9562 §5.5).

| ID | Formula | Name bytes | Stored? |
|---|---|---|---|
| Connection point | `v5(function, idx)` | `2` | no; only `CpOverride` rows |
| Generated report page | `v5(report_block, page_index)` | `7` | no |
| Function from a part's function template | `v5(device, template_index)` | `3` | yes |
| PLC channel | `v5(module, "channel", number)` | `channel␟3` | yes |
| Committed route | `v5(connection, "route")` | `route` | yes |
| Embedded library copy | `v5(NS_LIBCOPY, library ‖ item)` | `<library uuid>␟<item uuid>` | yes (file name) |
| "Keep both" library copy | `v5(NS_LIBCOPY, library ‖ item ‖ version)` | `<library uuid>␟<item uuid>␟2.1.0` | yes (file name) |
| Import-lineage namespace | `v5(NS_IMPORT, importer ‖ anchor kind ‖ anchor value)` | `dxf␟sha256␟<64 lower-case hex>` | yes (`ImportLineage.namespace`) |
| Imported record | `v5(lineage namespace, source_key)` | the source key byte for byte, e.g. `qet:el:<uuid>` | yes |

`ew-ids::v5::lineage_namespace(importer, anchor)` implements the lineage-namespace row; the importer is required in
lower-case ASCII.

**Worked example.** For the module `0192f3a4-5b6c-7d8e-8f01-23456789abd0`, `plc_channel_id(m, 3) = v5(m,
"channel␟3")` = `c3cb5781-8135-54b1-8948-9f118e22553d`. For an import lineage anchored on the SHA-256 of an empty
file, `v5(NS_IMPORT, "dxf␟sha256␟e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")` =
`a1ba512b-c515-50c5-af0b-26fa3b5599f5`. Importers emit source keys in NFC (§3.1), so the stored `source_ref.key`
re-derives the record's ID.

### 3.3 Line order

Records sort by kind rank, then by the ID's bytes 8–15 read as a big-endian `u64` (the random bits), then by the
full ID.

**Why the random bits.** UUIDv7's leading bytes are a timestamp, so time order would make additions on two branches
append at the same spot. Random-bits order scatters them, so plain-git and web merges succeed far more often
(ADR-0010).

**The order needs truly random bytes 8–15.** `ew-ids` therefore generates UUIDv7 by RFC 9562 §6.2 method 3
(ADR-0004 amendment 3): a sub-millisecond fraction in `rand_a` and 62 fresh CSPRNG bits in `rand_b` for every ID.
It puts no counter in `rand_b`. The uuid crate's `ContextV7` keeps a 42-bit counter spanning `rand_a` and the
top 30 bits of `rand_b` (checked in uuid 1.28.0), so `ew-ids` does not use `Uuid::now_v7` and assembles the bits
itself. UUIDv5 bytes are hash-random anyway. Sharding uses
byte 15, which is random under both schemes. Loading records in any order yields the same tables, and therefore
the same bytes on save (NFR-REL-07).

**Unknown and reserved kinds.** Records of a kind this reader does not know, and of a reserved kind that has a
rank but no table (such as `route_override`), are kept and written in the same
order: by rank where the registry knows the kind, else after every known kind, ordered by the `$k` text byte-wise,
and then by the ID order above. A duplicate `($k, id)` among them is EW-CORE-009 like any other (§6.3). Records of
a **reserved** kind are kept verbatim without a finding, because this format version defines them; only kinds,
members and variants the reader does not know at all are EW-CORE-016.

### 3.4 Library documents

Library copies (`lib/**`) are pretty JSON, one document per item (ARCH §7.3):
- 2-space indentation, member order as in §3.1, LF and a final newline;
- every array or object whose compact form is ≤ 100 bytes is written inline, with `": "` and `", "` separators, so
  a 200-point symbol stays readable;
- the copy metadata `library_id`, `item_id`, `version` and `sha256` (ARCH §4.2) follow `$k` and `id` (§13.2).
- the item's members follow. Every item kind carries `meta: ItemMeta`, a provenance object with `key?`
  (`pxc.2904602`), `licence?` (an SPDX expression), `sources: Vec<String>`, `as_of?: Date`, `confidence` (section
  name → `verified`, `unverified` or `test`), `status` (`draft`, `released`, `deprecated{successor?}` or
  `withdrawn`; default `released`) and `foreign_terms: bool` ([07](07-symbols-parts-macros.md) §2.1, §2.5).
  `ItemMeta` has no `version`: a document stores its version once (§13.2).

The exception is `lib/profiles/` (1.0): machine and label profiles keep their TOML source form, so that the hash
over the item's own text equals that of the published item. The copy metadata `{library_id, item_id, version,
sha256}` is a leading `[copy]` table, which the hash excludes as it excludes the JSON copy metadata
(FR-MFG-047).

*Reader tolerance.* The writer never writes a BOM (§3.1), but the reader
forgives one, as it does in a JSONL file: one leading BOM before a library copy is stripped and the copy is marked
non-canonical, so the next save rewrites it without the BOM; a second BOM is a syntax error and the copy stays opaque.
A copy with a BOM whose content is malformed is a kept document: it is preserved byte for byte, BOM included, and
never rewritten. The copy's `sha256` (EW-CORE-014) is computed over the item's compact JSON, so a BOM never changes
it, while the manifest hash stays over the file's bytes.
Before this, a BOM made the copy opaque and the project failed to open with an integrity error (EW-INV-001), the
first finding of the `migration_chain` fuzz target ([18](18-quality-security-release.md) §3.12).

### 3.5 Example records

Folio 101's page file holds the page row, the -101F1 representation and wire W7 (S51:1 → -101F1:1):

```json
{"$k":"page","id":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a","folio":{"section":"0e8b3d61-a74c-5f12-b5e9-82d4c1f06a3e","number":101},"kind":"multi_line","structure":{"function":"7d2c95e0-1f4a-5b38-a6e7-0c93b5d1f284","location":"a41f6c2d-8e05-5d97-b3c1-5f2e8a7d9c60"},"title":{"en-GB":"Power Supply 1 - Main Current"},"grid":{"num":7937500,"den":3}}
{"$k":"representation","id":"6f03d8a5-c912-5e7b-8a40-1d5b39e2f7c6","functions":["3e7a1c58-90d4-5b26-8f3a-6c0d2e9b417e"],"page":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a","symbol":"2c7e9b40-5a18-5f63-9d2c-b8a1e07f4396","pos":[137583333,31750000]}
{"$k":"connection","id":"b29e4f17-8d63-5a0c-9f71-3c6a2e8d05b1","a":{"cp":["d40b6e93-25f7-5c1a-9e8d-7a3f1c05b6e2",0]},"b":{"cp":["3e7a1c58-90d4-5b26-8f3a-6c0d2e9b417e",0]},"page":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a","route":[[42333333,15875000],[137583333,15875000]],"overrides":{"csa":"2.5","colour":["BK"],"show_marker":true}}
```

- **Page.** The record has no description override and no revision, Prev/Next or title-block text (D-05, D-06). It
  uses `title` and `description_override` as the [domain model](03-domain-model.md) §1.2 amends ARCH §4.2.
- **Representation.** Its position (10 px grid indices 52, 12) lies in cell B8. The optional member `anchor` (an
  `AnchorPolicy`, absent here) overrides the anchor policy of the symbol copy for this one representation; adding it
  is additive within 0.1 (ADR-0011 amendment 4).
- **W7.** Its 2.5 mm² BK is an explicit override (CDP) with a marker. W8 (-101F1:1 → F55:1) has **no**
  `overrides` member: its 1.5 mm² BK is derived (D-08).
  `route` is, in the schema's words, "the orthogonal
  route in nm, in model order: every vertex from end a's point to end b's, both included. Empty: the connection is
  not routed." W7's two vertices are its two ends; `schemas/0.1/connection.schema.json` carries the text.

Shard `devices/c2.jsonl` holds -102G1 (ID ending `c2`) and, after it (rank 32 after 31), its main function:

```json
{"$k":"device","id":"c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2","tag":{"letter":"G","counter":1},"kind":"psu","parts":[{"part":{"copy":"e5d2a8f1-0c37-5b94-8e6a-4f1d93c7b025"},"qty":"1"}],"function_text":{"en-GB":"Power supply 1 AC / 24 V DC"},"tech_data":[{"key":"i_out","value":{"value":"20","unit":"A"}}]}
{"$k":"function","id":"1d6f3a92-7c05-5b8e-94a1-e3c2087f5d19","device":"c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2","def":"5b9e2d70-3c41-5f8a-b6d2-19e4c7a03f58","kind":"black_box"}
```

Shard `devices/2a.jsonl` holds terminal 1 of -X1031 (ID ending `2a`). It prints its function text "1+" through
the strip's `label_mode` ([domain model](03-domain-model.md) §8.4):

```json
{"$k":"device","id":"f19c3a7e-6b02-5e48-9d5a-c7e1340b8f2a","tag":{"letter":"X"},"kind":"terminal","parts":[{"part":{"copy":"a07c4e19-d25b-5f83-9c16-4e8b0d7a2f35"},"qty":"1"}],"function_text":"1+","terminal":{"strip":"58e2b1d0-4c97-5a3f-8e16-d09a72c5e3f1","seq":1,"designation":"1"}}
```

`parts[].part` is a `PartRef{copy, variant?}` and `qty` a `Dec3` in the part's order unit (-3W1 counts in metres;
[domain model](03-domain-model.md) §3.2, §7).

The text "-102G1" appears in no file. It derives from `{G, 1}`, the page-based scheme and the folio of the main
representation ([domain model](03-domain-model.md) §8.2). An excerpt of `lib/parts/e5d2a8f1-….json` (2904602;
library IDs and the hash abbreviated). The manufacturer is a `LibRef`, which resolves to the project's embedded
copy of that item under `lib/manufacturers/` ([07](07-symbols-parts-macros.md) §2.5, §3.1):

```json
{
  "$k": "part",
  "id": "e5d2a8f1-0c37-5b94-8e6a-4f1d93c7b025",
  "library_id": "6a1d0f3e-…",
  "item_id": "8b42c7d9-…",
  "version": "1.2.0",
  "sha256": "sha256:9f2c…",
  "meta": {"licence": "CC0-1.0", "sources": ["golden tables"]},
  "manufacturer": {"library": "6a1d0f3e-…", "item": "0f7b2c94-…", "version_req": "^1"},
  "part_number": "2904602",
  "description": {"en-GB": "Power supply 1 AC / 24 V DC, 20 A"},
  "cp_pattern": [
    {"idx": 0, "designation": "L"},
    {"idx": 1, "designation": "N"},
    {"idx": 2, "designation": "PE", "potential_role": "pe"},
    {"idx": 3, "designation": "+", "potential_role": "l_plus"},
    {"idx": 4, "designation": "+", "potential_role": "l_plus"},
    {"idx": 5, "designation": "-", "potential_role": "m"},
    {"idx": 6, "designation": "-", "potential_role": "m"}
  ]
}
```

`meta` is the part's provenance object (§3.4); the part's version is the copy metadata `version` alone.

**A part with `mech`**. This excerpt is the `ew-format` test fixture of 2904602 (dimensions and offsets are test values,
not checked against a datasheet), with an alternative and a macro link; library IDs and the hash abbreviated. Lengths
are nanometres, the mass grams. The typed members follow `cp_pattern` in the order of
[07](07-symbols-parts-macros.md) §2.5 (`contacts`, `mech`, `sections`, `alternatives`, `configured`, `macros`). The
line breaks follow §3.4, except that the abbreviated `LibRef` objects are shown inline: with their full IDs they
exceed 100 bytes and the writer breaks them over lines.

```json
{
  "$k": "part",
  "id": "e5d2a8f1-0c37-5b94-8e6a-4f1d93c7b025",
  "library_id": "6a1d0f3e-…",
  "item_id": "8b42c7d9-…",
  "version": "1.2.0",
  "sha256": "sha256:…",
  "meta": {"key": "pxc.2904602", "licence": "CC0-1.0"},
  "manufacturer": {"library": "6a1d0f3e-…", "item": "0f7b2c94-…", "version_req": "^1"},
  "part_number": "2904602",
  "description": {"en-GB": "Power supply 1 AC / 24 V DC, 20 A"},
  "cp_pattern": [{"idx": 0, "designation": "L"}, {"idx": 1, "designation": "N"}],
  "mech": {
    "width": 60000000,
    "height": 130000000,
    "depth": 125000000,
    "mount_kind": "ts35x7_5",
    "rail_offset": [30000000, 65000000],
    "clearance": {"top": 50000000, "bottom": 50000000},
    "front_2d": [[[0, 0], [60000000, 0], [60000000, 130000000], [0, 130000000]]],
    "mass": 1100
  },
  "alternatives": [{"part": {"library": "6a1d0f3e-…", "item": "…", "version_req": "^1"}, "kind": "successor"}],
  "macros": [{"rep_type": "multi_line", "macro": {"library": "6a1d0f3e-…", "item": "…", "version_req": "^1"}}]
}
```

A clearance side that is absent is 0. A copy that an older build wrote with `mech` in `extra` (members in byte-wise
order) is read typed, reported not canonical and rewritten in this order at the next save (§10.3).

### 3.6 Loader limits

The JSONL loader is a hardened parser under NFR-SEC-02 and is fuzzed together with the migration chain
(NFR-SEC-03). Its limits:
- lines ≤ 16 MiB, files ≤ 512 MiB, ≤ 100,000 files, JSON nesting ≤ 64; a file or line beyond a limit is not
  read and is EW-CORE-019 (error, read-only), checked by a linear pre-scan before any parse;
  reading the manifest, the lock and an intent with the same bound before their size is checked is
  not built;
- duplicate member names rejected;
- only regular files are read. Symbolic links and junctions inside the project are not followed (NFR-SEC-16),
  because a received project could otherwise make EW read, export or overwrite a file outside its directory.

