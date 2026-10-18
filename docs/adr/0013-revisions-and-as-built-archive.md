# ADR-0013: Revisions and the as-built archive outside the project directory

**Status:** Accepted · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the architecture judge panel)

## Context

- **No managed revisions in the reference set.** The revision field reads "1" on folio 1, "V1" on folio 100, "V1.0"
  on 13 pages and "V2.0" on folios 101 and 106, and there is no revision table (D-05). Author and checker carry the
  same person on every page, so no four-eyes check is recorded (D-34).
- **Customers and the shop floor ask concrete questions.** "What changed in V2.0?" and, for manufacturing, "which
  wires changed since V1.0?" ([panel manufacturing](../research/panel-manufacturing.md) §7.1).
- **A release must be reproducible for at least 10 years** (G2): the canonical text, the libraries it used, the PDF
  and the production data as delivered.
- **Snapshots inside the project directory bloat git history**: every release would add a full copy of the project
  to the repository.
- **Revision state must be one thing.** Per-page revision fields are how D-05 happened.

## Decision

1. **One `Revision` table** per project:
   `Revision { label, date, author, checker, description, state: Open | Released, generation, archive: Option<ArchiveRef>, rule_packs, derivation_profile }`,
   with one label format per project (FR-REV-002). Title blocks read the managed revision; there is no per-page
   revision field. The [domain model](../design/03-domain-model.md) §4.2 and §18 refine this record: they add the
   state `Withdrawn` and `page_notes` (FR-REV-012), make `checker` optional until release, and group the
   release-only fields (generation, rule packs, derivation profile, approval, archive reference, git tag) into a
   `ReleaseRecord` that the release step writes (FR-REV-020). This ADR accepts that refinement.
2. **Release is one command** (`ReleaseRevision`, `ew revision release V2.0`). It runs these steps in this order;
   the generations written and the failure handling are specified in
   [project format](../design/05-project-format-and-persistence.md) §14.1:
   1. the **release gate**, on a fork: zero unwaived errors in the release scheme, no `Tbd` values, checker ≠
      author, no stale Class II results or committed computations, no unplaced devices and no cables without
      length (FR-REV-007, ADR-0003, ADR-0019);
   2. the **freeze** (`FreezeNumbers`, committed routes brought up to date) as an ordinary transaction, then a save
      of generation g;
   3. in a git worktree and with the user's consent, a commit of the project directory at g;
   4. the **as-built archive**, built from g and verified before it is renamed into place: the canonical text, the
      embedded libraries, the PDF/A-2b, the `.ewpp` production package when present, frozen report rows and
      SHA-256 sums;
   5. after step 3, the annotated git tag `ew/rev/<label>` on that commit, carrying the archive's SHA-256;
   6. the **stamp**: state `Released` and the `ReleaseRecord` (generation g, rule-pack versions, derivation profile,
      approval, archive name and hash), saved as generation g+1.
3. **The archive lives next to the project**, by default `<parent>/<name>.ew-archive/<label>.ewarchive`, at a
   configurable path. It is never placed inside the project directory, and EW never adds archives to git.
   The release record stores only the archive's name and hash, never a path.
4. **Released revisions are immutable.** Later edits belong to the next open revision; withdrawing a release creates
   a new revision record (FR-REV-009).
5. **Comparison** loads the archived text into a second, read-only `Model` with its pinned derivation profile and
   rule packs, and diffs it against the current model:
   - `diff_model` gives the property comparison and the page change status;
   - `diff_query` gives BOM, wire-list, terminal and PLC-address deltas, "wires changed since V1.0", and revision
     clouds from the changed representations.

**Scope by release.** MVP: the revision table, one label format, the release command with its gate, the archive,
reproduction from the archive and page change status (FR-REV-001 … FR-REV-012, FR-REV-020). 1.0: property
comparison, BOM, wire, terminal and PLC deltas, revision clouds (FR-REV-014 … FR-REV-016), and the JSON Schemas and
format specification inside the archive (NFR-REL-10).

## Consequences

### Positive

- D-05 and D-34 are impossible: one revision format, read by every title block, and a structural four-eyes rule
  (ADR-0009).
- The revision diff is concrete and works across git merges, because it compares models, not files or undo history.
- The archive is the 10-year deliverable: self-contained, verifiable by checksums, and reproducible with its pinned
  profile and rule packs (NFR-REL-10).
- Git history stays small; the tag marks the released commit.

### Negative

- A missing archive disables comparison. EW explains why and points to `ew archive rebuild` from the git tag
  (proposed for 1.0).
- Archives must be stored and backed up separately from the project; a share or repository backup alone is not
  enough.
- Releasing in a git worktree requires a commit of the project directory at the released generation before the tag;
  EW must ask before committing and never push.
- Release becomes a multi-step operation that must be atomic: a failure after the freeze must leave the revision
  Open and remove partial archives and tags; the freeze itself stays as an ordinary, undoable transaction.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **Snapshots inside the project directory** | Bloats git history with every release |
| **Revision status typed per page** | The D-05 mechanism; no single answer to "which revision is this project" |
| **Comparison from the undo history only** | History does not survive sessions, merges or another engineer's machine |
| **Git tags only, no archive** | Needs git and the original libraries and EW version to reproduce; useless for share-based users |
| **PDF archive only** | No property comparison, no BOM or wire-list deltas, no reproduction |

## Related

- **ADRs:** [ADR-0003](0003-freshness-contract.md), [ADR-0008](0008-transactions-undo-preview.md),
  [ADR-0009](0009-invariant-catalogue.md), [ADR-0010](0010-project-storage-jsonl-directory.md),
  [ADR-0011](0011-format-versioning-and-migration.md), [ADR-0012](0012-git-integration.md),
  [ADR-0015](0015-display-list-renderers-pdf.md), [ADR-0018](0018-reports-forms-engine-and-machine-profiles.md), [ADR-0019](0019-rule-engine-and-rule-packs.md).
- **Design:** [architecture](../design/04-architecture.md) §7.8; [project format](../design/05-project-format-and-persistence.md)
  §14 (release sequence, archive layout); [checks, revisions and collaboration](../design/15-checks-revisions-collaboration.md)
  §5.
- **Requirements:** FR-REV-001, FR-REV-002, FR-REV-003, FR-REV-004, FR-REV-005, FR-REV-006, FR-REV-007, FR-REV-008,
  FR-REV-009, FR-REV-010, FR-REV-011, FR-REV-012, FR-REV-014, FR-REV-015, FR-REV-016, FR-REV-020, NFR-REL-10.
