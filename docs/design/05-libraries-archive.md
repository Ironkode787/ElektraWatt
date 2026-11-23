# Project format and persistence: libraries, revisions and the archive, platforms and the sample project

A chapter of [Project format and persistence](05-project-format-and-persistence.md), whose table says where each section lives.

---

## 13. Libraries, catalogues and embedding

### 13.1 Tiers

Libraries are `.ewlib` text directories in the project format at every tier
([symbols, parts and macros](07-symbols-parts-macros.md) §2.1, §2.2; FR-PRT-012, FR-PRT-028):

| Tier | Location | Form | Writes |
|---|---|---|---|
| System | Installation directory (NFR-PLAT-06) | `<name>.ewlib/`: canonical JSONL, sharded like `devices/`, manifest `library.json` written last | Never (FR-PRT-012) |
| Community pack | User data directory, installed from a file or a git clone | `<name>.ewlib/` | Never in EW; updated by reinstalling |
| Company | A directory on a file share (path from settings or an MSI policy key, often SMB) or in a git repository | `<name>.ewlib/`, written with this document's writer, lock and merge driver | Library maintainers |
| User | OS data directory | `<name>.ewlib/` | Yes |
| Project | `lib/` | Pretty JSON per item (§3.4) | Commands only |

- **Derived search index.** `ew-library` builds one SQLite FTS5 index per user (rusqlite bundled) in the OS cache
  directory, over all installed libraries. It is rebuilt incrementally when a library's manifest generation
  changes and from scratch when it is missing or corrupt ([07](07-symbols-parts-macros.md) §4.7). It is never
  authoritative and can be deleted at any time.
- **No SQLite on a share.** No SQLite file is ever opened on a network share: a company library there is a text
  directory, read and saved like a project (§5, §7), so ADR-0010's reason for rejecting SQLite on shares does not
  arise.
- **Immutable versions.** A published version never changes: `ew lib pack` refuses different content under an
  existing version, so `(library_id, item_id, version)` determines the content ([07](07-symbols-parts-macros.md)
  §2.1). The starter library's draft items (status `draft`, every one at 0.1.0, [07](07-symbols-parts-macros.md)
  §2.3) are not published before the first release and may change in place. The consequence: a project
  made before such a change refuses a cross-project paste of the changed item (a terminal) until
  "update from library" (FR-PRT-014) exists.
- **Foreign SQLite files** (QETZ, third-party catalogues) open read-only with `trusted_schema=OFF`, defensive mode
  and a schema check (ARCH §7.7, NFR-SEC-06).
- **Item content** is specified in [symbols, parts and macros](07-symbols-parts-macros.md). The record kinds
  (`part`, `symbol`, `fndef`, `form`, `macro`, `manufacturer`; from 1.0 also the profile kinds of §2.2) are the
  same in every tier, so embedding is a copy.

`ew-library` implements the
tiers above except the project row, which is `ew-store`'s. The system tier is the compiled-in starter, the compiled-in
generic starter parts `ew-starter-parts` (a `.ewlib` whose files are compiled in; a directory of the same ID replaces
it) and `content/libraries/*.ewlib`; community, company (settings' paths) and user follow. Reading applies the project
loader's limits, writing puts `library.json` last, and the library lock follows §7.4 (a stale lock is taken over only
on explicit confirmation). The derived index is as described, one SQLite FTS5 file per user in the cache directory;
writes to the system and community tiers are refused.

### 13.2 Embedding

- **Copy identity** is `v5(NS_LIBCOPY, library_id ‖ item_id)`: one copy per library item and project
  (ADR-0004 amendment 2; [07](07-symbols-parts-macros.md) §2.1).
  - "Update from library" rewrites that one `lib/` file and leaves every `Representation.symbol`, `Function.def` and
    `PartRef.copy` untouched, so diffs and merges stay small.
  - Two branches embedding the same item version produce identical files.
  - Updates to different versions conflict, as they should.
  - Only an explicit "keep both" after a cross-project paste of another version
    ([06](06-schematic-editor.md) §3.6) mints `v5(NS_LIBCOPY, library_id ‖ item_id ‖ version)` for the second copy.
  - Project-local items, including the per-project "QET import" library, use UUIDv7 or element-hash UUIDv5 IDs,
    with `library_id` = the project ID. The "QET import" library's human key is `qet-import`; the importer
    receives the project ID as `ImportCx.project` ([14](14-interop.md) §2.1), because importers never read the
    model.
  - Manufacturer and macro copies follow the same
    identity, metadata and hash rules under `lib/manufacturers/` and `lib/macros/`. `library.embed` embeds them with
    the checks of parts, symbols, FnDefs and forms (a copy listed twice, copy identity, hash, an edited member, a
    project-local item), refuses one that is already held; `edit.paste` refuses a fragment that carries either kind, since a fragment's
    closure names neither. A
    part's `manufacturer` `LibRef` is not resolved to the embedded manufacturer copy; a
    `macro_instance` names its macro copy by copy ID, a forbid link (§2.2).
  - Project-local part copies made by the parts import take the IDs the CSV bootstrap mints: `library_id` = the project ID, `item_id` = `v5(ns, "parts|{key}")` from the part's key in the namespace of `ew-import-tables`' bootstrap, so an import and the bootstrap give the same copy; `meta.source` is the file's base name, no import date is stored (FR-PRT-017's reading, [02a](02a-requirements-core.md)), and an absent licence is `NOASSERTION`. A template's FnDef key resolves in the starter FnDefs, then in the project's own (a held FnDef with a key the starter has not). `library.embed` refuses project-local items except a form (a frame or title-block form of the project's own, which no command makes).
- **Metadata** is `{library_id, item_id, version, sha256}` (ARCH §4.2), with `version` the item's semantic version.
  `ItemMeta` carries no `version`. An embedded copy stores the version once, in this metadata; a library record
  (`.ewlib`, [07](07-symbols-parts-macros.md) §2.2) stores it once, in its own top-level `version` member after `id`.
  The hash rule therefore needs no exception.
  `sha256` is computed over the canonical compact JSON of the item's own fields, without `id`, `library` and these
  four members, so the copy's hash equals the published item's; EW-CORE-014 checks it. A project therefore opens,
  renders and exports without its libraries (NFR-REL-11).
- **Closure.** Embedding is transitive: a part pulls in its template's FnDefs, its macros and its manufacturer, a
  macro its symbols, parts and forms, a symbol its FnDef, and a form its assets. Within the project, a `LibRef`
  between embedded items resolves to the embedded copy and is a forbid link: a copy that another embedded copy
  references cannot be deleted or purged. A `LibRef` into a library that is not embedded is not a link
  ([03](03-domain-model.md) §4.2).
- **When copies come and go.**
  - The first command that uses an item embeds it in the same transaction (FR-PRT-013, FR-MAC-007).
  - "Update from library" is a previewed command (FR-PRT-014).
  - "Purge unused library copies" is explicit and previewed, and also offered at release. Saving never removes a
    copy, because that would be a hidden model change.
- **Provenance and licence** are stored per copy, in the item's `meta` object (§3.4; FR-PRT-017, NFR-LIC-07).
  Embedded copies do not bring projects under EW's copyleft (NFR-LIC-09).

### 13.3 Assets

The asset store below lives in `ew-model`, `ew-commands` and `ew-store` (ADR-0021 amendment 1).

- **Naming.** `assets/<sha256>.<ext>`, with `ext` ∈ {svg, png, jpg} at MVP (the format names in JSON are `svg`, `png`,
  `jpeg`) and `glb` from 1.0 (opt-in embeddable meshes, [12](12-panel-layout.md) §6.2; ADR-0010 amendment 2). Raster
  images are at most 16 MiB; SVG files at most 8 MiB and 100,000 elements. The limits are applied by the
  sniffer (below), which counts SVG elements textually; an SVG is also parsed by `ew_schema::svg_hazard`
  inside the sniff and drawn through usvg with external references refused ([14](14-interop.md) §11). The underlay
  sources `pdf` and `dxf` are at most 64 MiB, and the project's typeface is a `ttf` or `otf` file (by its sfnt magic)
  of at most 2.5 MiB, which `project.typeface.set` reads further ([09](09-reports-forms-output.md) §4.8). The
  extensions are `AssetFormat`'s, which the path classification of `ew-format` reads too (with `glb`).
- **The store in the model.** The model keeps image bytes by hash in an asset store beside its tables
  (`Model::asset`, `Model::assets`, `Model::put_asset`). Assets are not rows and not ops: putting an asset the model
  already holds does nothing, and no revision is made. A transaction brings its assets through
  `TxBuilder::put_asset(bytes)`, which sniffs the format and the intrinsic size from the headers (magic bytes; no
  decoding and no new dependency), applies the limits above, hashes the bytes and returns the `AssetRef` that a record
  stores; the bytes join the model with the transaction's ops at its commit. **Undo and redo keep assets:** the model
  keeps every asset it was given for the session, so undoing a logo change brings the reference back with its bytes.
  Bytes under a hash never change, so reading them by hash is no freshness hazard and sits outside the raw-read gate;
  the **presence** of an asset can change without a revision, so no memoised query may read it, with two named
  exceptions, `Q::logo_asset` and `Q::typeface_bytes` (the font file of the project's typeface), each recorded as a
  read of the project row and of the asset store (`DepKey::Assets`); an asset becomes present at open or with the
  command that names it (`project.company.set`, `project.typeface.set`), which also changes the project row. The rule
  is mechanical: the raw-read rule's `[raw_read.asset_presence]` flags the asset accessors in non-test `ew-derive`
  source, with those two as its allowlisted sites.
- **What a save writes.** The assets the records name (`Model::referenced_assets`: the company logo and
  the typeface of the project row, §10.3, and the files of image and underlay page items) that have no file on disk
  with that hash, through the **full save protocol** of §5: a temp
  `assets/<sha256>.<ext>.tmp-<g>` (`create_new`, `sync_all`), named in the intent manifest, the directory syncs, the
  renames after the commit point; a roll-forward (§5.3) completes asset renames like any other. The manifest lists each
  asset as a `FileHash::Sha256` in its sorted `files` map (§4). A file under an asset's name whose bytes differ
  (EW-CORE-015) is replaced by the asset the model holds; an asset a record names but the model does not hold writes
  nothing. **Save As and a new project write every asset the model holds**, not only the named ones, because kept
  unknown members and lines may name assets that no known record names (the flag is reset even when that save fails). A
  new project made from a basic project carries the template's assets unchanged (they are named by content), and its
  first save writes them.
- **Open.** Every file under `assets/` whose name is its SHA-256 and whose bytes sniff as the format of its
  extension is loaded into the model. Any other asset file stays an opaque file listed in the manifest, never rewritten
  or deleted; EW-CORE-015 reports bytes that do not match the hash in the name (§6.3).
- **Lifetime.** **A save never removes an asset file.** The model keeps the
  bytes of every asset it holds while the session lasts; a save writes the named assets that have no file and deletes
  none. EW cannot know every reference: kept unknown members and lines (§10.3) and future kinds that name assets (`glb`
  meshes, images in forms, `image` page items) may name a file that no known record does, so deleting on save could
  destroy a file that a newer writer's record still uses, and a hidden deletion is a model change, as with library
  copies (§13.2: saving never removes a copy). Cleanup is the **explicit, previewed purge command**: it deletes only
  asset files whose hash appears in no project file, the kept lines included, and in no undo entry; it is refused in
  read-only sessions and while lines are quarantined; and the deletions go through the save protocol's removal list
  (§5.2 step 8). The command is
  `project.assets.purge`, `Bulk`, a non-undoable marker line of ADR-0008 amendment 1, and **`HumanOnly`** (amendment 3): the `Session` checks its origin before touching any file (`PurgeError::HumanOnly`), the
  CLI `ew project assets purge [<dir>]` confirms with the typed word `purge` on a terminal (`--preview` lists only and
  needs none), and the GUI's confirmed step under Project settings → Storage is not built. The file side runs in the
  `Session` (`purge_preview`, `purge_assets`). An asset file goes only if its bytes match its name, **re-hashed at
  purge time** (a file rewritten from outside is never deleted by its old hash), and its hash is named nowhere: not by
  the model's records (`referenced_assets` over every record as the next save writes it, kept lines once
  JSON-decoded, so a `\u00XX`-escaped hash is caught), not by any file in the project directory (any 64 hex digits
  in any case: record files with kept and quarantined lines, stray and foreign files, the manifest apart from its file
  list; streamed in 1 MiB chunks with a 63-byte overlap through `Vfs::read_at`) and not by an undo or redo entry. It
  is refused when the session is read-only, has quarantined lines, unsaved journal steps or edits, a crashed session's
  pending journal, an incomplete save, or a manifest changed on disk; if a hash the preview listed is no longer
  purgeable, nothing is removed. Roll-forward and the journal's removals finish a purged asset's deletion after a
  crash, and when a save is incomplete the marker records the planned hashes.
- **Format and size**. When an asset is imported, the asset store sniffs its magic bytes and
  records its format (PNG, JPEG or SVG) and its intrinsic size (PNG `IHDR`, JPEG `SOF`, SVG `viewBox` or
  `width`/`height`). Readers such as the forms layer trust the resulting `AssetRef {sha256, format, size}` and never
  sniff; an asset without a known size fills its cell's inset area instead of being fitted. In `ew_schema::sniff`,
  the asset store's one entry, a PNG must start with its `IHDR` chunk and a JPEG must reach a start-of-frame marker, and a
  zero dimension or a cut-short header is `AssetError::Damaged`; an SVG is recognised by its root element `svg` after an
  optional byte-order mark, XML declaration, comments and a doctype, and takes its size from the root's `viewBox`, else
  `width` and `height` (a percentage gives none); other bytes are `UnknownFormat`.
- **The logo bound**. `ew_schema::MAX_LOGO_BYTES` is **512 KiB** (re-exported by `ew-api`). The
  logo travels as base64 inside the command parameter of `project.company.set` and inside a journal line (§8.1), and the
  base64 JSON string must stay under the command layer's `MAX_PARAM_STRING` of 1 MiB: 512 KiB is 699,052 base64
  characters. The bound is checked on the text length before decoding and again on the bytes; the wizard checks the file
  size before and after reading. Base64 in the journal is acceptable at this bound; larger rasters and meshes need the
  side files of §8.1.
- **SVG** is rendered by EW's own pipeline and never executed. **Sniffing is not sanitising**,
  so `ew_schema::svg_hazard` runs inside the sniff and
  refuses DOCTYPE internal subsets, entities, scripts, `on*` attributes, `foreignObject`, any `href` that is not
  `#…` (embedded `data:` images too), nesting deeper than 64, reference cycles, expansion past 200,000
  elements, markers inside markers, numbers above 1e9, overly complex CSS, `transform-origin` and `feConvolveMatrix`;
  the parse runs inside `catch_unwind` (threat-model row TM-18, [18](18-quality-security-release.md) §11). An accepted
  SVG is parsed by usvg 0.48.1 in `ew-scene` and drawn as `ew-dl` paths. **What renders:** SVG logos are drawn as vector paths by the CPU renderer, the PDF writer, the SVG writer and the viewer (the export job resolves them before any writer runs); PNG is drawn on the CPU path; the SVG writer embeds PNG and JPEG; JPEG on the CPU path and rasters in the PDF are not built. **The drawn SVG subset:** gradients are drawn in their mean colour, even-odd fills as
  non-zero; text, embedded images, clip paths, masks and filters are not drawn. **No silent loss:** an image an export cannot draw (its asset missing, or refused by the sniff) is reported as
  `PageFinding::Image` (missing or refused) among the export's findings. The rejecter's limits are not calibrated against
  real exporters' output, and a viewer-side parse cache by hash is not built.
- **The raster pixel limit.** A PNG
  or JPEG that declares more than `ew_schema::MAX_RASTER_PIXELS` (16 Mi pixels, the renderers' decode limit) or a side
  over 65,535 is refused, in words, **at the gates that bring an image in only**: `ew_api::asset_ref_of` (a file the
  user picks or drops) and the commands' `store_image` (the company logo and an image given as data to `item.place`);
  a held image placed again by its hash (`item.place {asset}`, the window's `image.pick`) is refused in the same words.
  `TxBuilder::put_asset` does not apply it, so `external.apply` keeps another
  build's assets as they are. **At open**, in a journal replay and in a template's copy, an over-limit image an earlier
  build accepted is **kept, never dropped**: the renderers still refuse to decode it, and the window's open banner and
  the banner of a project made from a template name each such image in words, by its folio or folios or as the company
  logo, with its pixel size and what to do (scale it down and add it again). `ew` says the same words
  (`Session::undrawn_image_notices`): `ew check --integrity` and `ew
  check` as a `notice:` line on stderr outside the counts, their JSON in the result's optional `notices` array, and a
  writer's open as a `notice:` line on stderr; no check rule reports it.
- **STEP** files are not embedded by default. `PartCopy` references `sha256(STEP)`; meshes live in
  the OS cache, and without the STEP file the 3D view shows the part's bounding box.

### 13.4 Trust store

**Where.** Trust decisions for `rules/`, `reports/` and, from 1.0, `scripts/`, `jobs/` and the gated library
copies below (NFR-SEC-05, FR-EXT-014) are kept in the user's configuration directory.

**What.** Each entry is `{project, content_sha256, signer?, decision}`. The hash (`ew-trust/1`) is taken over the
sorted paths and hashes of the regular files under `rules/`, `reports/`, `scripts/` and `jobs/`, of the embedded
profile copies under `lib/profiles/` that contain `expr` or `sql:` columns, and of the `lib/macros/` copies that are
command macros, so a changed rule file asks again. **Trust is never read from the project directory**, because a
received project could otherwise ship its own "trusted" flag. A `trust.sig` at the project root (1.0) only carries
a signature over this hash; it counts only for a signer key the user has trusted ([17](17-extensibility.md) §2.4).

---

## 14. Revisions and the as-built archive

### 14.1 Release sequence

`ew revision release V2.0` and the GUI command run, in order (FR-REV-006):

1. **Gate.** Run on a fork at the saved generation (FR-REV-007, ADR-0013 amendment 1). Its blockers are rules of
   the release scheme at error severity ([15](15-checks-revisions-collaboration.md) §4.8, §5.5):
   - zero unwaived errors, among them EW-CORE-047 (device without schematic representation), EW-LAY-001 (device
     without layout placement, counted only where its location has a layout space) and EW-CORE-033 (cable without
     length);
   - no `Tbd` values;
   - checker ≠ author and a current sign-off (EW-REL);
   - no stale Class II results or committed computations (EW-REL-002).

   The R findings of §6.3, among them conflict records (EW-CORE-011) and unknown data that affects outputs
   (EW-CORE-016), also block.
2. **Freeze.** A transaction applies the freeze policy (`FreezeNumbers`, committed routes brought up to date), then
   EW saves generation g. The archive therefore contains the frozen values.
3. **Git, when the project is in a worktree.** The project directory must be committed at g. With consent, EW
   commits only that directory as "Release V2.0" and records commit C.
4. **Archive.**
   - Build it from snapshot g plus the explicit **release context** (date, label).
   - Write `V2.0.ewarchive.tmp-<owner>-<g>` and sync it. The owner is the key of the project directory: the first 16 hex digits of the SHA-256 of the
     directory's path made absolute, cleaned and canonicalised where it exists, in the stored case. A leftover
     temp of the same archive name and owner, a crashed release of this directory, is removed first, and a temp
     of any other owner is never touched, because an archive root may be shared and another project's release,
     or a copy of this directory elsewhere (which carries the same project ID), may be writing in it. Only one run
     of a project directory writes at a time (its write lock), so a temp that carries the owner is never a live
     run's. An existing archive of the name is never replaced.
   - Verify it: re-read the ZIP, check the sums, and check that the canonical text hashes equal manifest g.
   - **Record the run**: write durably, in the per-user state directory, the record
     `<state>/releases/<key of the directory>-<key of the archive>.json`: the project directory and the archive
     (as path texts), the project, revision, label and generation, the archive's SHA-256 and modification time
     (the temp's, which the archive keeps), and the run's mark (a new session ID). It is the positive evidence
     that this directory's run wrote the archive, which a discard requires; it comes before the archive has its
     name, so a crash right after the rename stays discardable.
   - Put it into place without replacement: a hard link, which keeps the temp's time; on a file system without
     hard links the archive is created anew (`create_new`, never over an existing file, removed again if its write
     fails) with the temp's time set. A file system that cannot set the time keeps the archive and the record is
     rewritten with the archive's own time.
5. **Tag.** Create the annotated tag `ew/rev/V2.0` on C. Its message carries the archive's SHA-256 and the run's mark as a line `Run: <mark>`.
6. **Stamp.** A transaction sets `Released` and writes `ReleaseRecord{generation g, released_on, approval,
   rule_packs, derivation_profile, archive{name, sha256}, git_tag}`, plus `content_digest`, `page_digests` and
   `changed_pages` ([15](15-checks-revisions-collaboration.md) §5.5). EW then saves generation g+1 and leaves it
   for the user's next commit. The tag therefore points at the commit of generation g, whose `revisions.jsonl`
   still reads V2.0 Open; the archive's `ARCHIVE.json` carries the release facts.

**On failure.**
- A failure in steps 4–6 leaves the revision Open and removes the temp, plus any archive or tag created by this run.
  The freeze of step 2 stays, as an ordinary undoable transaction in the history.
- A crash leaves at most a complete but unreferenced archive. The next open offers "incomplete release V2.0: discard
  or complete".
- Discard and complete act only on an archive whose `ARCHIVE.json`
  names this project and the open revision. Any other (another project's, another revision's, a damaged one, or one
  whose `ARCHIVE.json` does not read) is kept and the action refused in words ("…; it is kept"); the person removes
  it or names another archive root (`--archive-dir`). The tag `ew/rev/V2.0` is deleted by a discard, or reused by a
  completion, only when its message carries the archive's `SHA-256:` line; a completion refuses another archive's
  tag before it commits. A discard removes only this project's leftover temps of the name.
- Two copies of one project directory carry the same
  project ID and, at the same generation, write the same archive bytes, so the archive cannot tell whose run wrote
  it. **Discard** therefore also needs this directory's run record naming that archive, its SHA-256 and its time;
  without it (a copy, a wiped state directory, a session without one) nothing is removed and the refusal says why
  ("unproven"), and the incomplete release says so up front. The tag is deleted only when its message carries the
  archive's SHA-256 and the record's `Run:` mark. **Complete** is unchanged: it destroys nothing. A failure in steps
  4 to 6 takes back the record with the temp and any archive this run created, never a file it did not create; a
  settled run removes its record. A record left **stale** (its archive gone, or its revision released with the
  archive's SHA-256 by a roll-forward or a completion) is removed when the project opens for editing and when the
  incomplete release is looked up; a removal or rewrite that fails is kept as release trouble for the front-end
  (the CLI prints it as a warning; the window does not). A temp of the form
  `<label>.ewarchive.tmp-<project id>-<g>` is never cleaned.

This is the release atomicity of NFR-REL-15.

### 14.2 Archive layout

```text
J-E-2416.ew-archive/V2.0.ewarchive      deterministic ZIP as in §12
  mimetype                              "application/vnd.elektrawatt.archive+zip"
  ARCHIVE.json                          project, revision {id, label}, generation g, release context,
                                        derivation profile, rule-pack versions, generator, format
  project/                              the canonical project directory at generation g, incl. lib/ and assets/
  pdf/<drawing id>_V2.0.pdf             PDF/A-2b (one per display language from 1.0)
  reports/<report-block id>.csv         frozen report rows: the committed snapshots of FR-REV-011
  check/report.json  check/report.pdf   findings, waivers, scheme, pack versions, disclaimer (FR-CHK-014)
  ewpp/<project>-V2.0.ewpp              production package when present (1.0)
  schemas/  spec/                       JSON Schemas and format specification (1.0, NFR-REL-10)
  SHA256SUMS                            `sha256sum -c` format over all other entries
```

- **File name.** The sanitised label: characters outside [A-Za-z0-9._-] become `_`, and Windows reserved names get
  a `_` prefix (a label "CON" is invalid even with an extension).
- **No path in the project.** `ReleaseRecord.archive` stores only name and hash. EW searches the default root
  `<parent>/<name>.ew-archive/` and the archive roots configured per user or company.
- **Inside a worktree.** If the default root lies inside a git worktree, EW warns and proposes a location outside
  it. It never adds archives to git.

### 14.3 Use

- **Comparison** (FR-REV-014, FR-REV-015). `project/` loads into a second, read-only `Model` with its pinned
  derivation profile and rule packs, and `diff_model` and `diff_query` run against the current model (ADR-0013). If
  no archive is found, EW explains why and points to `ew archive rebuild`.
- **Reproduction** (FR-REV-010). Rendering `project/` with the release context reproduces the archived PDF's page
  content.
- **Verify and rebuild** (1.0, FR-REV-025, ADR-0013 amendment 4).
  - `ew archive verify <file>` checks the sums, the schemas and the reproduction.
  - `ew archive rebuild <label>` recreates a missing archive from the git tag and marks it "rebuilt".
- **Immutability.** Archives are never modified (FR-REV-009).

---

## 15. Platform, share and cloud-sync pitfalls

| # | Pitfall | Mitigation |
|---|---|---|
| 1 | Windows `MAX_PATH` = 260. Long paths need both `LongPathsEnabled` and the `longPathAware` manifest flag, and relative paths are always limited (G1). Explorer, zip tools and git without `core.longpaths` still fail on long paths | Internal paths ≤ 120 characters; `longPathAware` set; std verbatim paths; warning when the project root path exceeds 130 characters |
| 2 | Reserved names (`CON`, `PRN`, `AUX`, `NUL`, `COM1–9`, `LPT1–9`, also with superscript digits and with any extension, G1); trailing dots or spaces; the characters `<>:"/\|?*` | No user text in internal names; folder names and archive labels validated and sanitised |
| 3 | Case-insensitive file systems (NTFS, default APFS) | Lower-case hex only; `.ewz` rejects case-insensitive duplicates |
| 4 | Antivirus, indexer or sync client holding a file during a rename; memory-mapped files blocking renames | Retry with backoff and roll-forward (§5.2); project files are read fully and closed, never mapped |
| 5 | Defender scanning every opened file | About 2,300 files plus one per embedded library item in the XL-A estimate case (ARCH §6.7, ADR-0010); measured in the Windows perf job; Dev Drive mentioned in the docs **(verify)** |
| 6 | `core.autocrlf` | `eol=lf`; CRLF recognised and classified (§6.2) |
| 7 | Partial cloud sync (OneDrive, Dropbox, iCloud, Google Drive) delivering a mixed generation | Manifest hashes; re-check after 5 s on cloud paths before reporting |
| 8 | Conflict copies: OneDrive `name-<HOST>.jsonl`, Dropbox `(… conflicted copy …)`, Syncthing `.sync-conflict-…`, iCloud `name 2.jsonl` **(verify patterns)** | EW-CORE-004 lists them; "merge this copy" applies its records as an external change |
| 9 | Online-only placeholders (OneDrive Files On-Demand; iCloud `.<name>.icloud` stubs on older macOS, dataless files on newer releases **(verify)**) | Detected by attribute or name; "N files not downloaded"; read-only open |
| 10 | Lock files reach a cloud folder late, so two editors can open the project within the sync delay | Cloud warning at open (FR-COL-008); git or SMB recommended for teams (PQ-26); a lock that changes under a live session raises a contention warning |
| 11 | Detecting cloud folders | OneDrive environment variables, `~/Library/CloudStorage/`, `~/Library/Mobile Documents/`, Dropbox `info.json` **(verify)** |
| 12 | SMB directory caching (about 10 s) and unreliable change notification; NFS attribute caching | The manifest is authoritative, not directory listings; 3 s polling; SMB 2.1/3 (NFR-PLAT-05); `O_EXCL` on NFSv3+ |
| 13 | Symbolic links or junctions in received projects | Not followed (§3.6). Hard links are harmless, because saving replaces files by rename |
| 14 | One project reached through two paths, or copied in Explorer | One physical lock file; "two copies of project … are open" when two open directories share a project ID |
| 15 | Read-only locations; a laptop asleep with the project open on a share | Read-only open with the reason; heartbeat and lock-loss handling (§7) |

---

## 16. UI and UX touchpoints

Screens are specified in [UX and UI](16-ux-ui.md). Persistence provides:

| Touchpoint | Behaviour |
|---|---|
| Status bar | Dirty dot, generation, badges (network share, cloud folder, read-only, merge driver missing), save progress and failures with cause and remedy (NFR-UX-07); "Merge driver not configured" in an editing window, a click sets it up after a question; the cloud folder is a banner at open |
| File commands | Save, Save As, Save a Copy, Export `.ewz`, Open `.ewz`; all non-blocking |
| Recovery dialog (§8.3) | At start-up and on open |
| Lock banner and dialogs (§7) | Holder, "since", "last seen"; take over, notify when free, switch to editing; lock lost |
| External-change toast | "External change applied: 12 records on folios 103 and 106 (git a1b2c3d)", with Undo and Show changes (the history at the step) |
| Merge panel | Base, ours and theirs per field, with derived tags. External-change collisions at MVP as a form dialog (mine or theirs per field, All mine, All theirs, Decide later, which leaves a banner and held saves); conflict records at 1.0 (FR-COL-015) |
| Banners | Integrity (read-only on EW-CORE-005); newer minor, newer major, older major (with backup), unstable format; untrusted rules, reports or scripts |
| Project settings → Storage | Save mode, lock stale age, archive roots, git status with "Set up merge driver", purge unused library copies; purge unused assets (the confirmed `HumanOnly` step with the preview in the `Destructive` dialog) |

---

## 17. The sample project on disk

### 17.1 Size

The reference project (until 0.3.5) had about 200 files and was well under 1 MB:
- 5 root JSONL files and the manifest;
- 13 page files;
- about 80 device shards (256 × (1 − e^(−96/256)) ≈ 80 for 96 devices, terminals included, with everything else
  following its anchor);
- 1 layout file, about 100 library documents and 1 asset.

**The sample.** `fixtures/sample/sample.ew/` is written by `cargo xtask gen` from the UI script
`fixtures/sample/sample.uiscript` and checked in: 146 files, about 250 KB. The test
`sample::the_committed_sample_is_what_its_script_makes` (`ew-viewmodel`) runs the script with a seeded ID source
and compares every file byte for byte with the committed folder; its bless mode (`EW_BLESS=1`) writes the folder
again as a whole. It opens with no error and no warning under the pre-delivery checks.

### 17.2 Edits and the files they write

| Edit (example) | Files written besides the manifest |
|---|---|
| Renumber folio 3 → 4 (AC-20, the D-01 replay) | Folio 3's page file only: one op on the page row. -3W1 becomes -4W1 everywhere because no tag is stored ([domain model](03-domain-model.md) §8.6) |
| Folio 106 renumbered to 104 | Folio 106's page file only. Two tags and 14 xref texts change in the outputs; none of them is in a file |
| Page-based renumbering of 10 tags | `project.jsonl` (the `numbering` settings line drops the strip rule, which renames -X1031/-X1032 without touching their records) and the shards of the devices whose `TagSpec` changes (frozen text cleared, counter set). After undo and save, every data file is byte-identical to before; only `generation` differs (AC-18) |
| 24 V DC colour changed in the profile (AC-12) | `project.jsonl` only. No wire record stores a derived colour |
| Company, frame defaults, languages or check scheme set by `project.company.set`, `project.frames.set`, `project.languages.set` or `project.checks.set` | `project.jsonl` only: the `Party` rows, `languages` and `settings` (with the `frames` and `checks` members in `extra`) are all inside the project record. Exception: a company logo also writes `assets/<sha256>.<ext>` at the first save that finds no file with that hash, and the manifest lists it (§13.3) |
| W29 colour override set, then undone | Nothing, not even the manifest: step 3 of §5.2 drops the unchanged file, so the dirty set is empty |
| Valve-manifold macro inserted on a new page | The new page file, the new devices' shards, and `lib/macros/…` if not yet embedded |

### 17.3 Defects the format prevents

| Defect | How persistence prevents it |
|---|---|
| D-01 stale -4W1 / -4X1 | Files hold `TagSpec{letter, counter}` and folios, never printed tags. No BOM is stored |
| D-02 "T51" on the layout, free rectangles | `placement` records reference a device ID and carry no label text. A missing device is EW-CORE-008 |
| D-03 "+JE2416" vs "+J-E-2416" | One `structure_node` with `aliases`. Device and page records hold node IDs, never spellings |
| D-04, D-42 BOM gaps, strip drawn twice | One strip `device` record, whatever the number of segment representations. Quantities are `Dec3` in `parts[].qty` |
| D-05 four revision formats | One `revisions.jsonl`; the schema has no per-page revision member |
| D-06 description drift on 101 | `Page.title` plus an optional, flagged `description_override`. No title-block text exists in any file |
| D-08 free-text wire rule | Only explicit overrides are stored (W7), never derived properties (W8) |
| D-11, D-12 empty and duplicate designations | Stored as authored data under lenient import and reported as findings (FR-CHK-016); strict mode prevents new ones, and duplicates that a merge creates are EW-CORE-013 |
| D-15 "TBD" address | The typed value `"tbd"`, which blocks release |
| D-17 core colour "BR" | `ColourSpec` admits IEC 60757 and declared codes. A foreign code exists only in a lenient (imported) project, written `{"foreign":"BR"}` and reported by EW-CORE-031 with the quick fix "use BN"; a strict project rejects it. A hand-edited bare "BR" parses as a company code and is reported by EW-CORE-031 |
| D-22 0 V–PE merged by a wire | A `bond` record with `via` W27 |
| D-30, D-34 parties and approval | Parties stored by role. `ReleaseRecord.approval` is written only by the release command, which enforces checker ≠ author |

QET stores evaluated labels and per-folio title-block strings, and these produced D-01, D-05 and D-06
([reference analysis](../research/reference-drawing.md) §3).

