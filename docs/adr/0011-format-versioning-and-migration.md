# ADR-0011: Format versioning, per-kind migration and format-change windows

**Status:** Accepted · amended 2026-10-18, 2026-10-21 (amendment 4 annotated as applied again on 2026-10-23), 2026-10-30 (amendment 5, the project fragment's version rule, WP-S1.19 part 5), 2026-11-05 (amendment 4 annotated as applied again for `import_run`), 2026-11-20 (amendment 7, the sample as fixture), 2026-11-22 (amendment 8, the latest version leads), 2026-11-23 (amendment 9, with ADR-0035) · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the architecture judge panel)

## Context

- **Long life.** Panel builders' customers expect projects from 2027 to open in 2037; the Machinery Regulation's
  retention of technical documentation for at least 10 years is the driver (**unverified**,
  [gaps and corrections](../research/gaps-and-corrections.md) G2).
- **Mixed versions in the field.** Old builds will meet new files: on a share, in a colleague's git clone, in a
  customer's archive. A reader must know whether it may open, edit or only view a file.
- **Prior art** ([OSS landscape](../research/oss-landscape.md) §3–§4):
  - KiCad bumps a date-stamped version token and upgrades on read, about 25 schematic bumps between 2023-11 and
    2026-03.
  - LibrePCB ties the file format to the major version and gives no forward compatibility.
  - QETZ carries `<qetz format="4" min-reader="4">`.
- **Small team.** Two to four developers cannot maintain chaotic format churn or hand-written migrations without
  rules.
- **Third parties** (integrators, panel-builder tooling, AI agents) want to validate EW files without linking GPL
  code (ADR-0024).

## Decision

1. **Two-part format version** in `manifest.json`: `format = "major.minor"`.
   - A **minor** bump is additive. Readers preserve unknown fields in each record's `extra` map and **unknown record
     kinds verbatim**.
   - A **major** bump breaks compatibility; an older reader refuses or opens read-only.
   - Before 1.0, formats are `0.x` and every step counts as breaking.
2. **Stamps.** `generator` ("ElektraWatt 0.9.0") and `min_reader` stamps tell a reader whether it may write.
3. **Per-kind schema versions** (`"$v"`, omitted when 1). Migrations are pure functions
   `fn(serde_json::Value) -> Result<Value>`, one per kind and step, run after parsing and before typed
   deserialisation, with `serde_path_to_error` reporting the failing path. **Migration steps are never deleted.**
   A `$v` bump breaks its kind, so it happens only in a format major; within a major, kinds change only by
   additions ([project format](../design/05-project-format-and-persistence.md) §10.2).
4. **Published contracts.** `ew-schema` generates JSON Schemas (schemars 1.2.3) for every kind and library document,
   validated in CI. One fixture per released format (the golden project and the QET-derived examples) is opened and
   saved by every build.
5. **Change windows.** Before 1.0: upgrade-on-read each release. After 1.0: format majors only in a **yearly window**.
   Development builds that write an unreleased format mark it `"unstable": true` and warn on every save; release
   builds open such projects read-only.
6. **Schema-steward rule.** Every PR that changes the format ships a migration, a fixture, the regenerated schemas
   and documentation. The steward role rotates.

Reader decisions ([project format](../design/05-project-format-and-persistence.md) §10.1; R = reader format,
F = file format, M = file `min_reader`):

| Condition | Behaviour |
|---|---|
| M > R | Refuse; nothing written |
| Same major, F ≤ R | Normal; a save writes `format = max(F, R)` and recomputes `min_reader` |
| Same major, F > R, M ≤ R | Read-write with a banner "written by a newer ElektraWatt; unknown data preserved"; the stamp stays F |
| F.major > R.major, M ≤ R | Read-only |
| F.major < R.major | Migrate in memory; the first save asks and writes `<name>.ew.pre-<F>.ewz` beside the project (NFR-REL-06) |
| `"unstable": true`, read by a release build | Read-only with a warning |

## Amendments

Accepted on 2026-10-18. The decision text above is unchanged; these entries refine it.

1. **Append-only members and original-line preservation** (2026-10-18;
   [project format](../design/05-project-format-and-persistence.md) §10.3, open question 3). "Fields in schema
   order" and byte-identical preservation (FR-COL-010, NFR-REL-04) could disagree when a newer minor inserts
   members. The rules are therefore:
   - new members are optional, **appended** after the existing members of their kind and tagged `since = "1.n"`,
     which feeds `min_reader`;
   - every persisted enum has an `Unknown(Value)` catch-all, so a new variant stays additive;
   - an untouched record with unknown content is written back as its **original line**.
2. **What `ew-schema` v0 fixed** (2026-10-18, core team;
   [project format](../design/05-project-format-and-persistence.md) §3.1, §6.3, §10.2, §10.4). The first
   implementation of decisions 1, 3 and 4 and of amendment 1 settles four points:
   - **`extra` at every depth.** Every JSON object of the format, records and nested value objects alike, carries
     its own `extra` map, so a member that a newer minor appends to a nested object survives a load and a save.
   - **Schemas describe the known form.** The `Unknown` catch-alls of amendment 1 are omitted from the generated
     JSON Schemas: the schemas of format F validate what F defines, and a value of a newer minor validates against
     that minor's schemas.
   - **`$v` is checked, not guessed.** Typed deserialisation rejects a record whose `$v` differs from the registry's
     version of its kind ("migrate first": the chain of decision 3 runs before it). A record with a newer `$v`,
     which only a newer major can write and which is therefore opened read-only, is kept verbatim like a record of
     an unknown kind and reported as EW-CORE-016.
   - **A malformed known variant is damage, not newer data.** A value whose enum tag names a variant this reader
     knows but whose payload does not parse is still preserved verbatim in `Unknown`, so nothing is lost, and the
     integrity check reports it as EW-CORE-017 (error, blocks release) instead of treating it as data from a newer
     format. Within a major a known variant's payload never changes type (decision 1, amendment 1), so such a value
     cannot come from a newer minor.
3. **Error paths and schema variants** (2026-10-18, core team;
   [project format](../design/05-project-format-and-persistence.md) §10.2, §10.4). Typed reading in `ew-schema`
   reports the JSON Pointer of the failing value itself (`SchemaError::path`, such as `"/kind/bridge/kind"`), so
   decision 3's `serde_path_to_error` is not used: `ew-format` reports the path that `ew-schema` returns, and a
   migration step reports its own. Decision 4's schemas are published in one form only for format 0.1, which allows
   additional members; the "strict" variant that forbids them is not generated, because the typed reader is the
   strict check. A strict variant returns only if a third-party consumer needs one.
4. **Implementing a reserved kind within an unreleased format** (2026-10-21, project owner (delegated); decision log
   ID-334; [project format](../design/05-project-format-and-persistence.md) §2.2, §3.3, §10.3). A record kind that the
   registry already reserves with a rank and an ownership rule, and a device aspect or part section that the format
   document already names, may get its typed table within the current, unreleased 0.x format as an additive change:
   no format bump, no `$v` bump, the published schemas of that format re-blessed, and the additive rules of
   decision 1 and amendment 1 followed (optional members, catch-all enums, no type change of an existing member).
   A build from before the change keeps such records verbatim (a reserved kind is not EW-CORE-016) and their unknown members in `extra`, so no data is lost; it does not enforce the delete links of the
   newly typed kind, so an older build that deletes a module leaves that module's channels dangling, which the newer
   build reports (FT-229, FT-230). Once a format is released, typing a reserved kind that adds cascade links, or any
   other semantics an older reader must apply to keep the file consistent, is a minor bump that raises
   `min_reader`, so an older build no longer writes such a file; typing without such semantics stays additive under
   decision 1 and amendment 1. First use: the PLC kinds `plc_config_project`, `channel` and `device_link`, the aspects
   `plc_station` and `plc_module` and the part section `plc` in format 0.1; the
   precedent is the optional `TextSlot.style` member. Applied again in cycle 14 to the optional
   member `Representation.anchor`: an optional member without cascade semantics, additive
   within 0.1. Applied again in cycle 18 to the members `positions` and `pin_of` of the part section `plc`, appended
   in that order (decision log ID-464, ID-465; [PLC](../design/10-plc-and-bus.md) §2.6). `pin_of` persists a small
   text language (a designation pattern with `{<expression in n>}` placeholders, or a list of designations); its
   forms are part of the format contract like a member's type: a later minor may add a form only as a new variant (a
   one-member object, kept verbatim by older readers under amendment 1), never by giving an existing string a new
   meaning, and a reader gives no pin for a pattern it cannot expand. Applied again in cycle 23, as a specification for
   WP-S2.04 part 4a, to the reserved kinds `page_item` (rank 43) and `structure_box` (rank 44),
   both owned by their page's file: typed within the unreleased format 0.1, `schemas/0.1` re-blessed, no format or `$v`
   bump, every `page_item` variant the reader does not type kept as `Unknown(Value)`. They add cascade and unlink links
   (ADR-0009 amendment 8), which an older 0.1 build does not enforce; because no format is released, the newer build
   reports a stale wire-marker entry as a load finding instead of raising `min_reader`.
   Applied again in cycle 50 to the fluid members `CpDef.port_role`,
   `ConnectionKind::Fluid` with `line_kind`, `CpOverride.closure`, `PartSections.fluid` and `TextSlot.fallback`, each
   optional with an `Unknown` arm, `schemas/0.1` re-blessed with additions only; `closure.part` is a new link to a
   part copy that an older 0.1 build does not enforce, accepted for the unreleased format (amendment 6).
   Applied again after the lock (WP-S1.25, M3) to the fluid 1.0 members (`FnDef.valve_model`, the 1.0 members of
   `PartSections.fluid`, `FluidLine.tube`, `.length`, `.fitting_a`, `.fitting_b`, `Junction.fitting`,
   `ProjectSettings.fluid`), the reserved kinds `medium` and `medium_def`, and five `ReportKind` values: each optional
   or caught by `Unknown`; every new link is load-tolerant (doc 11 §2.7), so a stale target an older build leaves is
   a load finding, never a refusal, and no `min_reader` is raised.
   Applied again after the lock (M3) to the PLC and bus 1.0 members (WP-S1.24: `PlcConfigProject.symbols`,
   `PlcModule.safety`, `Channel.data_type`, `.signal_range`, `.mode`, `.safety`, the part section `plc`'s
   `allowed_slots`, `limits`, `iolink` and `ratings`, the part's bus-port members, `BusPort.mrp`, `BusSection.cable_for`,
   the typed `NetAddress` forms), to the PLC exchange (WP-S3.16: `PlcConfigProject.ownership` and `.exchange`,
   `PlcStation.firmware`, `PlcModule.type_ident`, and the reserved kinds `exchange_record` and `foreign_id`), and to the
   underlay (WP-S3.19: the `page_item` variant `underlay` and the `AssetFormat` values `pdf` and `dxf`): each optional
   or caught by `Unknown`, except `AssetFormat`, which needs no `Unknown` arm: no image gate takes a PDF or a DXF, so a
   company logo never names one, and a reader takes an asset (from the asset files or the journal) only when its own
   sniffing confirms the format, so an older build reports a journal asset of a format it does not know as a recovery
   finding and keeps the asset file of an open as opaque bytes. The new links (a safety channel's `pair` and `test_pulse`, unlink) are load-tolerant: an
   older build kept `safety` in `extra`, so a stale target it leaves is absent to every reader, and an edit may not add
   one. The exchange kinds' link to their configuration project is a cascade, which the rule above would make a
   `min_reader` bump; it is not one, because the link is load-tolerant and the kinds are a snapshot: an older build
   keeps them verbatim, and a snapshot whose configuration project that build deleted is unused data, which a newer
   build neither reads as an error nor applies (the next exchange writes a new one), while the newer build deletes it
   with its project. A member added after the lock is appended after the members of the released format, `source_ref`
   included (amendment 1), where an older build that keeps it in `extra` writes it as well.
   Applied again in cycle 51 to the reserved kind `import_run` (rank 72,
   `imports/<run-uuid>.jsonl`), typed within 0.1 with `schemas/0.1` re-blessed with additions only, and to
   `ImportLineage.last_run`, now a link to the run without cascade; a build from before the change keeps the runs
   verbatim as a reserved kind. Applied again in 0.3.5 (FT-1875) to the optional member `FreeText.of`, the
   representation a text belongs to ([schematic editor](../design/06-schematic-editor.md) §3.8), appended after the
   released members: its link cascades and is load-tolerant, as the exchange kinds' link is. An older build keeps it in
   `extra`; a text that build leaves when it deletes the symbol is a text whose symbol is gone, which a newer build
   prints as a free text of no symbol (a symbol's binding empty) and never refuses, so no `min_reader` is raised.
   An older build that copies a symbol with its texts keeps `of` in `extra` as well, still naming the original
   symbol: in a newer build the pasted texts belong to the original, move with it and are deleted with it, and the
   copy has none, which the person binds again (accepted: no file is refused and no text is lost until the original
   goes).
   Applied again in 0.3.5 (doc 12 §5.4) to the optional member `ReportBlock.layout_page`, the layout page a legend list
   belongs to, appended after the released members: its link cascades and is load-tolerant for the same case. An
   older build keeps it in `extra` and may delete the layout page and leave the block; a newer build then opens the
   project, and the list, whose layout page is gone, has no pages.
5. **The project fragment widens within v1 for macros** (2026-10-30, WP-S1.19 part 5, cycle 36; lead programmer,
   delegated; [architecture](../design/04-architecture.md) §5.5, [project format](../design/05-project-format-and-persistence.md)
   §12, [symbols, parts and macros](../design/07-symbols-parts-macros.md) §2.7). WP-S1.19 D12 makes a macro variant a
   project fragment that also carries the record kinds `channel` and `device_link` and, for a page macro, the page
   records. `#ew-fragment v1` was defined for the clipboard with eight record kinds; no amendment so far
   said how the fragment itself is versioned. The rule is:
   - **A fragment version is bumped (`#ew-fragment v2`; the object form would then gain a version member, absent
     meaning v1) only when a reader of the previous version would misread new content or drop it silently.** New content that every older reader refuses
     with an error is an additive widening within the same version. Older fragments stay readable either way: a
     reader keeps every older version it ever read.
   - **New content never goes into an open place.** The `$fragment` header is open within v1 (an unknown member is
     ignored) and every record keeps unknown members in `extra`, so content an older reader must not lose may not be
     put there. It goes where an older reader refuses: a record line of a kind the older reader does not take, a
     member of the closed object form (`edit.paste`'s `fragment` parameter refuses unknown members), or an unknown
     `$` line of the text form.
   - **Applied to the macro widening: v1 is widened additively, no bump.** The object form gains two optional,
     default-empty, default-omitted members: `pages` (the page records of a page macro) and `lib` (the library
     references, `LibRef`, of the closure a macro names instead of carrying its documents; a library item holds no
     copies of other items). A macro fragment's records may also be of kinds `channel` and `device_link`. Every older
     reader refuses this content instead of misreading it: the cycle-27 `edit.paste` and `parse_fragment` refuse a
     record of a kind outside their eight with "is not pasted", and the object form's closed members refuse `pages`
     and `lib`; the cycle-34 macro copy (WP-S1.19 part 1) keeps a variant's fragment as an opaque JSON value, so it
     round-trips it byte-equal without reading it. A clipboard fragment written without the new content is unchanged
     byte for byte, and `edit.paste` keeps its eight kinds: the macro-only content is taken by `macro.insert` alone,
     and `edit.paste` refuses a fragment with page records.
   - **The text form is unchanged by this widening.** A macro stores its fragment in the object form inside the macro
     document (doc 07 §2.7). When a text export of a macro fragment is built, its new content is written as record
     lines and `$` lines that a v1 clipboard reader refuses, never as header members.
   Trade-off: one fragment version for clipboard and macros, so a fragment an older build cannot take is refused as a
   whole rather than read in part; against a v2 that would turn away also the clipboard fragments older builds read
   today.
6. **The format locks at the 0.1.0 release** (2026-11-04, the project owner; corrected). Until 0.1.0 is released, the owner's rule of 2026-10-31 stands: the format changes freely, with no
   migrations. When WP-S6.08 tags 0.1.0, the format that release ships is locked, and this ADR applies in full from
   there.
   - After the lock, a change is either additive within the released format under this ADR's own rules, or a new
     format version.
   - **Additive:** an optional member without cascade semantics; a new variant caught by an `Unknown(Value)` arm; a
     new record kind that older readers keep verbatim; unknown members kept in `extra`. It re-blesses the published
     schemas, and a test shows that a project of the release opens and saves unchanged.
   - **A new format version:** it adds a migration step from the released format to the chain, with the first save's
     backup and a fixture (decision 6).
   - Golden A as the 0.1.0 release ships it is the released format's fixture. WP-S6.08 adds the guard against
     non-additive changes to the released schemas.
7. **The sample is the fixture of formats released from now on** (2026-11-20, the project owner's standing
   delegation; M3.5 step 9). The reference project (Golden A) left the repository at 0.3.5. Its frozen copy in
   `fixtures/format/0.1/` stays byte for byte as format 0.1's fixture; every format released after it takes the sample
   project (`fixtures/sample/sample.ew`, made by its script) as its fixture in decision 4's place of the golden project.
8. **The latest version leads** (2026-11-22, the project owner). A project saved by a newer build need not open in an
   older one. Where a change needs it, the format may change in a way an older build cannot read.
   - Each such change is an entry below: what changed, and what an older build does with a project that uses it. Its
     line in the release notes says the same. No stand-in member and no raised `min_reader` stamp is added to keep an
     older build reading.
   - What still holds without exception: a project made by an earlier release opens in this build unchanged and loses
     nothing, and this build copes with what an earlier release may have done to a file in between (a member that
     release did not know is kept; a record it deleted is gone).
   - The published schemas say what this build's reader accepts.
   Entries:
   1. *External references hold what is known* (0.3.5, FT-1876; [schematic editor](../design/06-schematic-editor.md)
      §3.5). `external_ref.location` and `external_ref.device` became optional: a reference may hold an address
      alone, a location alone, or nothing for a far end not known. A reference of 0.3.0 holds both and reads
      unchanged. 0.3.0's reader takes both members as required: it opens a project with a reference without them for
      editing, sets that reference's line aside as a schema violation (EW-CORE-007), reports the points that name it,
      and writes the line back unchanged on save.
   2. *A device on a bus page is drawn with its bus symbol* (0.4.0, FT-1904; [bus](../design/10-bus.md) §2.7). A
      representation names the built-in symbol `ew.gen.bus`, whose body a page derives from the device's part; a
      symbol's connection point may name the interface and port it draws (`SymbolCp.interface`, `.port_label`) and a
      `bus_address` slot the interface whose address it prints (`TextSlot.interface`, `.port_label`), and a part may
      name a bus symbol of one's own that draws its devices in place of the derived one (`Part.bus_symbol`, a
      library reference that its embed closure brings), all optional. A project of 0.3.5 holds none of them and reads
      unchanged. 0.3.5's reader keeps the members in `extra` and draws such a
      representation as its embedded copy holds it: no body and no connection point, so the device is not seen on the
      page and its bus lines end where the copy draws nothing.
9. **The `min_reader` stamp is the format** (2026-11-23, [ADR-0035](0035-homes-order-texts-shell.md) decision 17;
   in force: the record is accepted, and the owner's ruling of 2026-11-22 that the latest version leads covers
   the item of its plan's list). The
   stamp was computed as the greatest format among the kinds, members and variants a project's files use, from
   `since` tables per kind, member and variant. Those tables serve one reader only, an older build that meets a
   newer project, which amendment 8 no longer provides for. The stamp a build writes is its own format, and the
   tables and the computation go; the migration chain, the `generator` stamp and the reader's refusal of a format
   above its own stay. Every kind, member and variant is of format 0.1 today, so no manifest changes by a byte.
   From the next format step on, an older build refuses a project that a newer build saved even when its files
   use nothing new, where the computed stamp would have let it open.

## Consequences

### Positive

- Forward compatibility within a major: a colleague on the previous minor can still edit a project without losing
  data.
- Third parties validate files against published JSON Schemas with the permissive crates alone (FR-COL-012).
- Integrators can plan around predictable, yearly breaks.
- A major that changes only one kind rewrites only that kind's records, so `ew upgrade` yields a small git diff.
- The fixture corpus proves the 10-year promise build by build.

### Negative

- Migration code accumulates forever, and every step must keep passing on the fixture corpus.
- The yearly window delays breaking improvements by up to a year after 1.0.
- Preserving unknown data means the writer must keep members it does not understand and must never reorder them in
  untouched records.
- Every persisted enum needs a catch-all variant so that a new variant stays additive (amendment 1).

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **KiCad-style frequent date-token bumps** | Every release breaks older readers; integrators cannot plan |
| **No forward compatibility** (LibrePCB) | A colleague one minor behind cannot open the project on the share |
| **Pruning old migration steps** | Breaks the 10-year promise for archived projects |
| **Typed migrations on Rust structs per version** | Every old version's structs kept alive in code; `Value`-level steps are smaller and independent of the current types |
| **Unversioned "best effort" reading** | Silent data loss when an old build saves a newer file |

## Related

- **ADRs:** [ADR-0003](0003-freshness-contract.md), [ADR-0010](0010-project-storage-jsonl-directory.md),
  [ADR-0012](0012-git-integration.md), [ADR-0013](0013-revisions-and-as-built-archive.md), [ADR-0020](0020-extensibility-tiers-and-security-model.md), [ADR-0024](0024-licence-set-and-permissive-gpl-boundary.md).
- **Design:** [architecture](../design/04-architecture.md) §7.6, §15.2;
  [project format](../design/05-project-format-and-persistence.md) §10;
  [quality, security and release](../design/18-quality-security-release.md).
- **Requirements:** FR-COL-010, FR-COL-011, FR-COL-012, FR-EXT-008, FR-EXT-023, NFR-REL-04, NFR-REL-05, NFR-REL-06,
  NFR-REL-10, NFR-DOC-06, NFR-DOC-07.
