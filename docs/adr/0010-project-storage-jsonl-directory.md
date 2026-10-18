# ADR-0010: Project storage: UUID-named JSONL directory, manifest-last save, external journal, lock file

**Status:** Accepted · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the architecture judge panel)

## Context

- **Where projects live.** Panel builders keep projects on SMB shares, in OneDrive folders and in git repositories.
  EW is offline-first, with projects as files on disk.
- **Multi-file saves are not atomic.** POSIX `rename` is atomic for one file; a set of files never is
  ([gaps and corrections](../research/gaps-and-corrections.md) G1).
- **Windows constraints.** `MAX_PATH` is 260 characters unless both `LongPathsEnabled` and `longPathAware` are set.
  `CON`, `PRN`, `AUX`, `NUL`, `COM1–9` and `LPT1–9` are reserved names. Case-insensitive file systems collide -K1
  with -k1. A file per device tag would break on all three.
- **Merges.** Pretty-printed JSON arrays conflict on comma lines in plain-git and GitHub web merges. A file per object
  reaches 10⁴–10⁵ files at L/XL, which Windows Defender and OneDrive scan one by one.
- **Sharding by ID prefix fails.** All UUIDv7 IDs (ADR-0004) share their first byte until 2039, because the leading
  bytes are a timestamp.
- **No database at rest.** SQLite on network shares is unreliable, and a working database would be a second source of
  truth (ADR-0001).

## Decision

### Layout

```text
J-E-2416.ew/                     # folder name chosen by the user; content is UUID-named
  manifest.json                  # written LAST: format, min_reader, generator, generation, sha256 per file
  project.jsonl                  # project row, parties, settings, profiles, identifier set, derivation_profile
  structure.jsonl                # structure nodes (=P01, +J-E-2416, +J-E-206 …), sections 1/2/9
  revisions.jsonl  report-blocks.jsonl  waivers.jsonl  comments.jsonl
  pages/<page-uuid>.jsonl        # page row + page-owned rows (representations, connections, junctions, IPs)
  devices/<xx>.jsonl             # 256 shards by the LAST id byte: devices, functions, conductors, channels,
                                 #   potential/net definitions, bonds
  layout/<space-uuid>.jsonl      # enclosures, surfaces, carriers, placements, route overrides, committed routes
  lib/{symbols,fndefs,forms,parts,macros}/<uuid>.json   # embedded library copies, pretty-printed
  rules/<uuid>.toml (+ rules/<uuid>/)  reports/<uuid>.sql
                                 # project rule packs and user reports (untrusted until the project is trusted)
  assets/<sha256>.svg|png
  .gitattributes  .gitignore  .ew/ (per-user view state, git-ignored)  .~ew.lock (git-ignored)
```

**Ownership determines the file:** page-scoped rows go to their page file, layout-scoped rows to their layout-space
file, everything else to the shard of its owning device. The kind registry of
[project format](../design/05-project-format-and-persistence.md) §2.2 fixes the anchor per kind: for example, an
interruption-point function without a device goes to the page file of its representation, and a connection
without a page (an undrawn bridge) to the shard of the device that owns endpoint `a`. A typical edit rewrites one
to three files.

### Canonical writer

- One record per line, with `$k` (kind) and `id`.
- Lines ordered by kind (schema order), then by the ID's **random bits (bytes 8–15)**, then by the full ID, so
  concurrent additions on two branches land on scattered lines.
- Fields in schema order; defaults and nulls omitted.
- Values are integers (nm) and `Dec3` strings only, never floats (ADR-0005). Strings are NFC, line endings LF, and
  entities carry no timestamps.
- `ew fmt [--check]` re-canonicalises hand edits.

### Save protocol (G1)

Run by the writer thread from an immutable snapshot, never blocking the UI:
1. write every dirty file to `name.tmp-<gen>` and `sync_all` it;
2. rename it over the target;
3. write `manifest.json` last, with the new generation and a SHA-256 per file;
4. sync the directory where the OS supports it.

On open, the hashes are verified against the manifest. A mismatch (interrupted save, hand edit or merge) loads
anyway, runs `ew check --integrity` and offers journal replay or the last good generation.

### Journal, lock and transport

- **Journal.** Each commit appends `{command, field diffs, rev, base_generation}` to a JSON Lines journal in the OS
  state directory, never in the project, synced at least once per second. Three rotating recovery snapshots are
  written every 2 minutes while the project is dirty.
- **Lock.** `.~ew.lock` is created with `create_new` and holds `{user, host, pid, app_version, started_at}`. A held
  lock opens the project read-only; a stale lock (same host and a dead pid, or older than a configurable age) can be
  taken over. No OS byte-range locks are used over SMB.
- **Cloud folders.** OneDrive, Dropbox and iCloud paths are detected and warned about; files not listed in the
  manifest (sync conflict copies) are reported.
- **Transport.** `.ewz` is a ZIP of the directory, opened by extraction or read-only.
- **Nothing else.** No working database and no derived data on disk.

## Consequences

### Positive

- About 2,300 files in the XL-A estimate case (2,000 page files, 256 shards and the root files), plus one file
  per embedded library item: few enough for Defender, OneDrive and git, local enough for merges.
- Internal file names contain no user text, so no reserved names, no case collisions and short paths.
- Concurrent additions on two branches scatter instead of colliding at the end of a file, so even GitHub web merges
  usually succeed.
- The format is readable and writable with the permissive crates alone (`ew-format`, `ew-schema`).
- The journal never reaches git or a sync client.

### Negative

- File names are opaque. Humans navigate through `ew diff --textconv` and the GUI, not the file browser.
- Small projects still get many shard files: the golden project fills about 80 of the 256.
- Moving a representation to another page rewrites two page files; git sees a delete and an add.
- Directory sync is not available through Rust's std on Windows (**verify**), so the durability of renames relies
  on NTFS's metadata journal. The fault-injection harness of pending amendment 1 must cover NTFS.

## Pending amendments

1. **Commit point and roll-forward** ([project format](../design/05-project-format-and-persistence.md) §5.3, open
   question 5). With the order temp → rename → manifest, a crash between two renames leaves generation g partly
   overwritten, so NFR-REL-01 ("previous or new generation") is not guaranteed. The proposed fix writes the complete
   new manifest as a durable **intent** (`manifest.json.tmp-<g+1>`) after all temps and *before* the first rename; the
   next open rolls forward every temp whose hash matches the intent. A fault-injection harness kills the writer at
   random points on ext4, NTFS and APFS. *Recommendation:* accept and fold into the save protocol above.
2. **Scripts and jobsets in the project** (same document, open question 11). *Recommendation:* add
   `scripts/<uuid>.rhai` and `jobs/<uuid>.ewjobs` at 1.0, under the trust store.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **A single SQLite file** | Unreliable on network shares; binary diffs; a second truth next to the model |
| **S-expressions with an own parser** (proposal D) | Avoidable parser work; weaker tooling than JSON (jq, JSON Schema) |
| **Pretty JSON arrays** (proposal A as written) | Comma-line conflicts in plain-git merges |
| **File per object** | 10⁴–10⁵ files at L/XL |
| **Sharding by UUID prefix** (proposals B, D) | Every v7 ID lands in the same shard until 2039 |
| **Files named by tag or title** | Reserved names, case collisions, renames on every renumber |

## Related

- **ADRs:** [ADR-0001](0001-core-model-persistent-maps.md), [ADR-0004](0004-identity-uuidv7-uuidv5.md),
  [ADR-0005](0005-value-types-nm-dec3-mltext.md), [ADR-0008](0008-transactions-undo-preview.md),
  [ADR-0011](0011-format-versioning-and-migration.md), [ADR-0012](0012-git-integration.md),
  [ADR-0013](0013-revisions-and-as-built-archive.md), [ADR-0022](0022-platform-matrix-and-performance-budgets.md), [ADR-0024](0024-licence-set-and-permissive-gpl-boundary.md).
- **Design:** [architecture](../design/04-architecture.md) §7.1–§7.4;
  [project format](../design/05-project-format-and-persistence.md) §2–§9, §12, §15.
- **Requirements:** FR-COL-001, FR-COL-002, FR-COL-003, FR-COL-004, FR-COL-005, FR-COL-006, FR-COL-007,
  FR-COL-008, FR-COL-009, NFR-REL-01, NFR-REL-02, NFR-REL-03, NFR-REL-04, NFR-REL-11, NFR-PLAT-05, NFR-PERF-12,
  NFR-PRIV-05.
