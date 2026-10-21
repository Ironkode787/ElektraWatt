# ADR-0011: Format versioning, per-kind migration and format-change windows

**Status:** Accepted · amended 2026-10-18, 2026-10-21 · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the architecture judge panel)

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
   A build from before the change keeps such records verbatim (a reserved kind is not EW-CORE-016, decision log
   ID-206) and their unknown members in `extra`, so no data is lost; it does not enforce the delete links of the
   newly typed kind, so an older build that deletes a module leaves that module's channels dangling, which the newer
   build reports (FT-229, FT-230). Once a format is released, typing a reserved kind that adds cascade links, or any
   other semantics an older reader must apply to keep the file consistent, is a minor bump that raises
   `min_reader`, so an older build no longer writes such a file; typing without such semantics stays additive under
   decision 1 and amendment 1. First use: the PLC kinds `plc_config_project`, `channel` and `device_link`, the aspects
   `plc_station` and `plc_module` and the part section `plc` in format 0.1 (cycle 13, decision log ID-333); the
   precedent is the optional `TextSlot.style` member (decision log ID-197). Applied again in cycle 14 to the optional
   member `Representation.anchor` (decision log ID-351): an optional member without cascade semantics, additive
   within 0.1.

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
