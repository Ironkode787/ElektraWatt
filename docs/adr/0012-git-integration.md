# ADR-0012: Git integration: semantic merge driver, textconv, in-file conflict records, post-merge integrity

**Status:** Accepted · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the architecture judge panel)

## Context

- **Collaboration happens through git and file shares.** Engineers branch per change request, merge on a server
  with a web UI, and pull on the workshop PC. Real-time co-editing is out of scope for v1.
- **Text merges can be semantically wrong.** A line-based merge of two branches can produce a project that parses but
  is invalid: two devices numbered -103F1, two star sources named 24VDC-1 in one scope, a connection whose
  endpoint was deleted on the other branch.
- **Git merges file by file.** A custom driver is called per path with `%O %A %B %L %P` (ancestor, ours, theirs,
  marker size, path), and `diff.<name>.textconv` turns a blob into readable text
  ([gaps and corrections](../research/gaps-and-corrections.md) G1). Cross-file semantics are therefore invisible to
  any driver.
- **Conflict markers break JSON.** `<<<<<<<` inside a JSONL file makes it unloadable for any JSON tool.
- **Raw diffs are unreadable.** A diff of UUID-keyed records says nothing about "-102F1" or folio 102.
- **Not every merge runs the driver.** GitHub web merges and fresh clones without setup use git's text merge.

## Decision

1. **Repository files.** EW maintains a marked block in the project's `.gitattributes` (`*.jsonl` and
   `lib/**/*.json` as `text eol=lf merge=elektrawatt diff=elektrawatt`, `manifest.json` with the merge driver,
   `assets/*` as binary) and in its `.gitignore` (`.ew/`, `.~ew.lock`, `*.tmp-*`). Git never takes driver
   definitions from a repository, which is a security property: `ew git setup [--global] [--hooks]` writes the
   driver and textconv into the git configuration after the user agrees
   ([project format](../design/05-project-format-and-persistence.md) §11.1).
2. **Merge driver.** `ew merge-driver %O %A %B %L %P` (the form `%O %A %B %P` is accepted too; `%L` is ignored,
   because the driver writes no markers) merges three-way **per `($k, id)` record, then per field**, with a
   strategy per `ew-schema` annotation: `object` (recursive), `atomic` (scalars, `Pt2`, routes, junction sequences),
   `keyed` (`cp_overrides` by `idx`, `parties` by role), `set` and `max`. Older sides are migrated to the newest `$v`
   first (ADR-0011). Disjoint field edits on one record merge cleanly.
3. **In-file conflict records.** A true conflict keeps *ours* in place (for a delete/modify conflict, the modified
   side) and appends
   `{"$k":"conflict","id":…,"kind":…,"base":…,"ours":…,"theirs":…}`. The file stays valid and loadable, git gets
   exit code 1, and EW opens a resolution panel that shows the conflict with derived tags and resolves it as one
   transaction.
4. **Manifest merge.** `generation = max(a, b) + 1`, `format` and `min_reader` as the maximum of both sides, file
   hashes merged per path with `pending` where both changed, and `merged: true`.
5. **Post-merge integrity.** Because the driver sees one file at a time, `ew check --integrity` runs after every
   merge (optional `post-merge` hook) and on every open. It reports dangling references, duplicate counters,
   duplicate interruption-point names and prevent-rule violations, each with a previewed quick fix.
6. **Diff.** `ew diff --textconv <file>` renders records with **derived** tags, for example
   `-102F1: part A9F78116 → …`. `ew diff <from> <to> [--queries bom,wires,terminals,plc,xref]` builds both models and
   runs `diff_model` and `diff_query`.
7. **Local history (1.x).** An optional git-based local history through gix 0.89.0, as KiCad does (FR-COL-020).

**Scope by release.** MVP: the managed `.gitattributes` block, `ew diff --textconv`, `ew check --integrity` on open
and after merges (FR-CHK-012) and the merge driver as a Should (FR-COL-014). 1.0: the merge driver as a Must, the
conflict-resolution panel (FR-COL-015) and the visual comparison of project states (FR-COL-017). 1.x: local history
(FR-COL-020). Later: real-time co-editing on the operation log (FR-COL-021).

## Consequences

### Positive

- Disjoint edits on the same page or shard merge cleanly, including two engineers editing different fields of one
  device.
- Semantic clashes surface as check messages with quick fixes instead of corrupt files. Branch-safe counters
  (renumbering the newer -103F1 to -103F2) are one example.
- Merges without the driver still work better than with pretty JSON, thanks to one record per line ordered by random
  ID bits (ADR-0010). Leftover conflict markers are converted into conflict records by the loader.
- Reviewers read changes as engineering facts ("-102F1: part …"), not UUIDs.

### Negative

- Users must run `ew git setup` once per clone, or once per machine with `--global`; without it, merges fall back
  to text and rely on the loader's marker handling.
- The `generation` line of `manifest.json` conflicts in every driver-less merge. Any resolution is acceptable, but
  users will see it.
- The merge driver parses hostile input and must be fuzzed (NFR-SEC-03).
- `ew diff --textconv` gets one blob only, so it labels records with tags derived from the *working tree*; historic
  tags may print as they read today.
- Cross-file moves (a representation moved to another page on one branch, edited on the other) need a reconciliation
  step at load.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **Conflict markers inside JSON** | Break the file for every reader |
| **Plain text merge only** | Semantically invalid projects without warning |
| **Real-time co-editing or a CRDT in v1** | Merges into semantically invalid states; opaque binary logs; large scope for a small team. The op log stays CRDT-ready (Loro is the candidate) |
| **A single merge over the whole project** (outside git's driver model) | Git calls drivers per path; would require a wrapper around `git merge` that web UIs cannot run |

## Related

- **ADRs:** [ADR-0004](0004-identity-uuidv7-uuidv5.md), [ADR-0008](0008-transactions-undo-preview.md),
  [ADR-0009](0009-invariant-catalogue.md), [ADR-0010](0010-project-storage-jsonl-directory.md),
  [ADR-0011](0011-format-versioning-and-migration.md), [ADR-0013](0013-revisions-and-as-built-archive.md), [ADR-0019](0019-rule-engine-and-rule-packs.md).
- **Design:** [architecture](../design/04-architecture.md) §7.5, §16 (risk 9);
  [project format](../design/05-project-format-and-persistence.md) §11;
  [checks, revisions and collaboration](../design/15-checks-revisions-collaboration.md) §6.
- **Requirements:** FR-COL-013, FR-COL-014, FR-COL-015, FR-COL-016, FR-COL-017, FR-COL-020, FR-COL-021, FR-CHK-012,
  NFR-SEC-03.
