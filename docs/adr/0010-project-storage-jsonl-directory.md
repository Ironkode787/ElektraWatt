# ADR-0010: Project storage: UUID-named JSONL directory, manifest-last save, external journal, lock file

**Status:** Accepted · amended 2026-10-18 · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the architecture judge panel)

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
  lib/manufacturers/<copy_id>.json                      # manufacturer copies (MVP; amendment 2)
  lib/plc-profiles/<copy_id>.json                       # PLC address profiles (1.0; amendment 2)
  lib/profiles/<copy_id>.toml    # machine and label profiles in their TOML source form (1.0; amendment 2)
  rules/<uuid>.toml (+ rules/<uuid>/)  reports/<uuid>.sql
                                 # project rule packs and user reports (untrusted until the project is trusted)
  scripts/<uuid>.rhai  jobs/<uuid>.ewjobs   # automation scripts and jobsets (1.0, untrusted; amendment 2)
  imports/<run-uuid>.jsonl       # one import run per file (MVP; amendment 2)
  exchange/<config-project-uuid>.jsonl      # PLC exchange records and foreign IDs (1.0; amendment 2)
  assets/<sha256>.svg|png|jpg|glb           # logos, images; opt-in embeddable meshes (.glb, 1.0; amendment 2)
  trust.sig                      # project trust signature (1.0; amendment 2)
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

## Amendments

Accepted on 2026-10-18. The decision text above is unchanged; these entries refine it.

1. **Commit point, roll-forward and lock record** (2026-10-18;
   [project format](../design/05-project-format-and-persistence.md) §5.3, §7). With the order temp → rename →
   manifest, a crash between two renames would leave generation g partly overwritten, so NFR-REL-01 ("previous or
   new generation") would not hold. The save protocol therefore writes the complete new manifest as a durable
   **intent** (`manifest.json.tmp-<g+1>`) after all temps are durable and *before* the first rename; the next open
   rolls forward every temp whose hash matches the intent, then the intent. A fault-injection harness kills the
   writer at random points on ext4, NTFS and APFS, and every reopen must yield g or g+1. The lock record is
   `{lock: 1, session, user, host, pid, pid_started, app_version, started_at}`. The holder refreshes the lock's
   modification time every 5 minutes and re-reads the session ID, so it detects a lost lock. A lock is stale when
   the PID on the same host is dead or its start time differs, or when the lock is older than `lock.stale_after`
   (default 60 minutes). A confirmed takeover renames the stale lock **without replacement** before creating its
   own, so of two instances taking over the same stale lock only one succeeds.
2. **Further project files** (2026-10-18; [project format](../design/05-project-format-and-persistence.md) §2).
   The layout above gains: `scripts/<uuid>.rhai` and `jobs/<uuid>.ewjobs` (1.0, under the trust store);
   `imports/<run-uuid>.jsonl` (MVP; one import run per file, so runs never conflict with project data);
   `exchange/<config-project-uuid>.jsonl` (1.0; PLC exchange records and foreign IDs, included in the as-built
   archive); `lib/manufacturers/`, `lib/plc-profiles/` and `lib/profiles/` (machine and label profiles kept in
   their TOML source form, so their SHA-256 equals the published item); `trust.sig` at the root (1.0); and opt-in
   embeddable meshes `assets/<sha256>.glb` (1.0). Asset extensions are svg, png, jpg and glb.

## Consequences

### Positive

- About 2,300 files in the XL-A estimate case (2,000 page files, 256 shards and the root files), plus one file
  per embedded library item: few enough for Defender, OneDrive and git, local enough for merges.
- Internal file names contain no user text, so no reserved names, no case collisions and short paths.
- Concurrent additions on two branches scatter instead of colliding at the end of a file, so even GitHub web merges
  usually succeed.
- The format is readable and writable with the permissive crates alone (`ew-format`, `ew-schema`).
- The journal never reaches git or a sync client.
- Libraries reuse this storage design: a `.ewlib` library is a directory in the same canonical format, with the
  same writer, manifest-last save, lock and merge driver
  ([symbols, parts and macros](../design/07-symbols-parts-macros.md) §2.1–§2.2).

### Negative

- File names are opaque. Humans navigate through `ew diff --textconv` and the GUI, not the file browser.
- Small projects still get many shard files: the golden project fills about 80 of the 256.
- Moving a representation to another page rewrites two page files; git sees a delete and an add.
- Directory sync is not available through Rust's std on Windows (**verify**), so the durability of renames relies
  on NTFS's metadata journal. The fault-injection harness of amendment 1 must cover NTFS.

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
  [project format](../design/05-project-format-and-persistence.md) §2–§9, §12, §13, §15;
  [symbols, parts and macros](../design/07-symbols-parts-macros.md) §2.
- **Requirements:** FR-COL-001, FR-COL-002, FR-COL-003, FR-COL-004, FR-COL-005, FR-COL-006, FR-COL-007,
  FR-COL-008, FR-COL-009, FR-COL-026, NFR-REL-01, NFR-REL-02, NFR-REL-03, NFR-REL-04, NFR-REL-11, NFR-PLAT-05, NFR-PERF-12,
  NFR-PRIV-05.
