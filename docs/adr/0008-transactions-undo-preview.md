# ADR-0008: Transactions, undo/redo and preview-as-fork

**Status:** Accepted · amended 2026-10-18, 2026-10-21 (the first command that edits project settings), 2026-10-22 (the transaction-read rule), 2026-10-23 (a transaction carries assets) · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the architecture judge panel)

## Context

- **Many writers, one path.** GUI tools, the `ew` CLI, rhai scripts, importers, WASM plugins and MCP agents all
  change projects. Each change needs undo, a crash journal entry, a macro-recorder step and a diff
  ([gaps and corrections](../research/gaps-and-corrections.md) G3).
- **Prior art.** EPLAN's data-model undo is project-wide; an API change made without an `UndoStep` clears the
  history. KiCad stages `CHT_ADD/REMOVE/MODIFY` items in a `COMMIT` and pushes one undo point.
- **Bulk operations need an exact preview.** Renumbering 10,000 tags, a QET import, PLC re-addressing and "update
  parts from library" must show their effect (tags, BOM rows, wire-list rows, PLC addresses, xref texts, rendered
  pages) before anything is committed (FR-UI-022).
- **Jobs run on forks.** Background work (imports, routing, plugins) cannot write to the live model directly.
- **Derived data** (ADR-0001) is never part of undo; it is recomputed from the restored model.

## Decision

### Three layers

| Layer | What it is | Persisted in |
|---|---|---|
| `Command` | Serialisable intent with a stable dotted name (`device.place`, `numbering.renumber`) and a JSON Schema for its parameters (FR-EXT-015) | Journal, macro recorder |
| `Transaction` | `Vec<Op>` plus `{command, origin, page/viewport/selection, coalesce_key, undoable}`; `origin` is GUI, CLI, script, `plugin:<id>` or MCP | Undo stack, journal |
| `Op` | `Put { key, before: Option<AnyRow>, after: Option<AnyRow> }`, rows as `Arc`s | Undo stack (`Arc`s); journal (field diffs) |

`Command::run(&self, tx: &mut TxBuilder) -> Result<Outcome, CmdError>` is deterministic in (snapshot, command).
`TxBuilder` is a scratch snapshot with read-your-writes, so a handler can ask for "the next free counter on page 103".

### Commit pipeline (UI thread)

1. **Run** the command against `TxBuilder`.
2. **Normalise:** deterministic normalisers add ops to the same transaction, at most three passes (non-convergence is
   a bug). They number new devices, instantiate CPs, clean up junctions, turn autoconnect hints into connections
   (ADR-0007) and assign conductors; the full list of normaliser families is the registry in
   [schematic editor](../design/06-schematic-editor.md) §3.2.
3. **Validate:** structural invariants, then prevent rules in the project's mode (ADR-0009).
4. **Commit:** swap in the new `Model` (O(1)), bump `Rev`, push to `History` (coalescing on a matching key) and hand
   the transaction to the writer thread for the journal (ADR-0010).
5. **Notify:** publish `{rev, touched keys, touched pages}` on the change feed; view-models re-query (ADR-0014).

### Undo and redo

- Undo applies the inverse ops as a **new revision**, which keeps `changed_at` monotonic and memo verification sound
  (ADR-0002).
- One project-wide stack, bounded at 500 steps or 256 MB. Each entry restores page, viewport and selection.
- Coalescing: one drag is one step; typing in one cell merges until focus leaves; a fill-down or Excel paste is one
  step.
- The stack survives saving; a saved marker drives the dirty flag. An external reload arrives as one undoable
  "External change" transaction.
- **Not undoable**, flagged in command metadata and confirmed: library writes outside the project, exports, file
  operations, and a bulk import into an empty project (a checkpoint). Amendment 1 limits the confirmation to the
  destructive ones.

### Preview is a fork

- A command declared `Cost::Bulk` always runs as a preview job. The result is a candidate `World` plus
  `diff_model(current, candidate)` (entity and field changes) and `diff_query(current, candidate, q)` for tags, BOM
  rows, wire-list rows, PLC addresses, xref texts and affected pages, with rendered candidate pages.
- Accept commits the prepared transaction after a rebase check. Cancel drops the fork.
- **Jobs return commands or prepared transactions with their read set.** If no key in the read set changed since the
  job's revision, the session applies the transaction; otherwise it re-runs the deterministic command on the current
  snapshot (optimistic rebase).

## Amendments

Amendments 1 and 2 were accepted on 2026-10-18, amendment 3 on 2026-10-21, amendment 4 on 2026-10-22, amendment 5 on
2026-10-23. The decision
text above is unchanged;
these entries refine it.

1. **Confirmation of non-undoable operations** (2026-10-18; [UX/UI](../design/16-ux-ui.md) §7–§8). Confirming every
   export would train users to click through. Every non-undoable operation is therefore **flagged** as a marker line
   in the history, and only the **destructive** ones ask for confirmation: library writes outside the project,
   overwriting an existing file, purging library copies, and the empty-project import checkpoint. FR-UI-023 and
   NFR-UX-07 state the same rule.
2. **Normaliser passes, the undo stack, coalescing and validation** (2026-10-18, core team, after the first
   implementation, WP-S1.04, and its review; [architecture](../design/04-architecture.md) §5.1–§5.3,
   [schematic editor](../design/06-schematic-editor.md) §3.2, §3.9). This entry fixes what the decision leaves open:
   - **Passes.** "At most three passes" means three productive passes plus one confirming pass that changes nothing;
     a fourth productive pass is non-convergence (`CmdError::Internal`) and refuses the command. Each normaliser
     converges within one run for its own cascades (junction clean-up uses a worklist), so passes only carry effects
     from one normaliser to the next.
   - **Bounds.** The 500 counts history entries, marker lines included. The 256 MB counts only what the history
     alone keeps alive: the `before` rows of each step, and the `after` rows that are no longer live after an undo,
     estimated from the rows' in-memory size, not from their JSON length. The oldest entries go first; a single step
     larger than the budget is kept alone. If the saved state falls off the stack, the model stays dirty until the
     next save.
   - **Coalescing.** An undoable transaction whose `coalesce_key` equals that of the top step merges into it: the
     ops are concatenated, the step keeps the first transaction's page, viewport, selection and label, and takes the
     latest command and origin. A marker line or another command on top ends the run. Every new transaction drops
     the redo branch.
   - **Markers** (amendment 1). A non-undoable transaction without ops, such as an export, is a *transparent*
     marker that undo and redo pass over. One with ops, such as the empty-project import checkpoint, is a
     *barrier*: undo stops there and reports that nothing before it can be undone.
   - **Empty transactions.** An undoable transaction without ops commits nothing: no revision, no history entry and
     no change-feed entry.
   - **Validation of undo and redo.** Undo and redo apply their ops as a new revision through the structural
     invariants, but not through the prevent rules. Linear undo and redo only restore states that were committed
     before, and a prevent rule that refused an undo would trap the user, for example after the strictness or a
     rule pack changed. A violation in the restored state is reported by the online checks instead (ADR-0009).
   - **Bulk commands** sent to the interactive `execute` path are refused with `CmdError::NeedsPreview`; they run as
     preview, then commit. `commit` adopts the candidate when the snapshot is unchanged, applies the prepared ops
     when only unrelated keys changed (the read set is unchanged), and otherwise re-runs the command, which is the
     optimistic rebase of the decision.
3. **Project settings are edited by one previewed command** (2026-10-21, project owner (delegated); decision log
   ID-290, ID-291, ID-292; [extensibility](../design/17-extensibility.md) §2.1, §2.3,
   [domain model](../design/03-domain-model.md) §5.1, §8.6,
   [project format](../design/05-project-format-and-persistence.md) §8.1). Until cycle 11 no command edited
   `ProjectSettings`, so a numbering or profile change could only be made by rewriting the project file outside the
   command path, without undo, journal or preview. `project.settings.set` closes that gap:
   - **One whitelisted member per command.** Parameters `{member, value}`; the members are `numbering`,
     `colour_profile`, `size_profile` and `xref_format`. `value` is the member's whole new value; `null` removes the
     member so that the pinned derivation profile's default applies; a value equal to the stored one commits nothing
     (an empty transaction, amendment 2).
   - **Validated twice.** The parameter schema first, then semantically by the parser the derivation reads the member
     with (`ew_derive::check_settings_member`): a value the derivation would only note as malformed or not apply is
     refused with `CmdError::InvalidParams` at `/value` (a `numbering` scheme the derivation would not apply, decision
     log ID-99 and ID-103; a `default` naming no scheme; unknown or unclosed format tokens; colour codes that are
     neither IEC 60757 nor company codes on an IEC 60757 base; cross-sections that are not positive mm², AWG or
     kcmil). Any other member is refused at `/member`. Ambiguous token sequences are refused once follow-up task
     FT-163 lands.
   - **Bulk, previewed, one undo step.** One stored value changes derived values on every page, so the command is
     `Cost::Bulk`; the preview shows the derived diff (`Session::derived_diff`: tags and cross-reference texts; wire
     properties, BOM and TOC rows are FT-152). Only the project record changes, so topology and every reference stay
     identical. The journal records the whole top-level `settings` member in `set` and `was`.
   - **Exposure `Edit`.** `HumanOnly` (ADR-0020) guards controls that decide whether a gate passes without changing
     the drawing: approvals, waivers, severities, packs, strictness and trust. Numbering, profiles and the
     cross-reference format change the drawing's content like `numbering.renumber`, which is `Edit`: every effect is
     in the preview, undoable and attributed to its origin, and reaches a released document only through
     `revision.release`, which stays `HumanOnly`. A settings member that decides a check outcome is therefore never
     added to this whitelist and keeps a `HumanOnly` command of its own; extending the whitelist needs an amendment
     of this ADR.
   - **Frozen tags change only by an explicit action.** No derivation changes a frozen tag. Two commands may:
     `fields.set` on `tag.frozen` (explicit freeze and unfreeze) and `numbering.renumber` with `include_frozen: true`,
     which unfreezes only the frozen devices its scheme can number; both are undoable, the renumber previewed. The
     renumber option `skip_frozen` is removed.
4. **A transaction never reads the memoised queries** (2026-10-22, lead designer, on the cycle-21 channel indexes;
   decision log ID-525; [architecture](../design/04-architecture.md) §5.2,
   [PLC and bus](../design/10-plc-and-bus.md) §2.10). The memoised queries `Q` are computed over committed snapshots; a
   transaction's pending state has none, so a `Q` read inside `Command::run` or a normaliser would see the base without
   the transaction's own writes.
   - **The rule.** A command handler or normaliser reads rows through `TxBuilder` only. Where it needs an index answer
     (which rows refer to this one through a link), it asks `TxBuilder` for a **structural-index lookup** over
     `ew-model`'s referrer index of that link, with read-your-writes, and recomputes any derived key it needs from the
     rows it read. The first two lookups are `TxBuilder::channels_of_module` and `channels_at_cp` (the links
     `Channel.module` and `Channel.binding`), used by N1, N2, N3, `plc.bind_channel` and `plc.set_module_part`.
   - **Soundness.** Every lookup is recorded in the transaction's read set, so `ReadSet::unchanged_since` and the
     optimistic rebase of amendment 2 see a change to a looked-up link as a conflict; a rebase test pins it.
   - **Cost.** A lookup scans the transaction's fresh rows besides the base index, so a transaction that creates many
     referrers and looks them up (a bulk import) is O(fresh²) within that transaction. Recorded, not measured (decision
     log ID-512); a bulk path that shows it gets a batched lookup.
5. **A transaction carries assets beside its ops** (2026-10-23, project owner (delegated), on FT-337 as built; decision
   log ID-573 to ID-575; [project format](../design/05-project-format-and-persistence.md) §8.1, §13.3,
   [architecture](../design/04-architecture.md) §3 rule 1). Image bytes are content-addressed and immutable, so they are
   not rows and not ops.
   - **Commit.** `TxBuilder::put_asset(bytes)` sniffs, checks the limits, hashes and returns the `AssetRef` a record
     stores; the bytes join the model's asset store with the transaction's ops at commit. Putting a held asset does
     nothing.
   - **Undo keeps assets.** Undo and redo move references, never bytes: the model keeps every asset it was given for the
     session, so undoing a logo change restores the reference with its bytes. A save never removes an asset file
     (cleanup is an explicit, previewed purge, FT-380).
   - **Dirty and journal.** A transaction whose only content is an asset (no ops) still marks the project dirty and is
     journalled; the journal carries every asset the model after the transaction names unless it is on disk or already
     in the segment (base64 at the 512 KiB logo bound; side files later, FT-381). A damaged journal asset is a recovery
     finding (EW-CORE-015), not a refused replay.
   - **Freshness.** Asset presence can change without a revision, so no memoised query reads it (ARCH §3 rule 1; the one
     named exception `Q::logo_asset` until FT-382 makes the restriction mechanical).

## Consequences

### Positive

- Undo, journal, macro recorder, `ew diff` and the revision change log share one record format.
- Previews are exact, because they are the real command run on a fork.
- Every agent or plugin edit is journalled, undoable and attributed to its origin in the history panel (FR-UI-023,
  FR-EXT-017).
- The algebraic properties are testable: `undo(all) ∘ apply(cmds) = id`, preview + commit = execute, journal replay
  = live state (NFR-REL-08).

### Negative

- Every feature must be a command. There is no shortcut that mutates the model directly, which costs boilerplate
  per feature.
- `run` must be deterministic. Sources of nondeterminism (new IDs, dates) must come through the `TxBuilder` context so
  that tests can inject them.
- Normaliser convergence and ordering need their own tests.
- The undo cap (256 MB) and the preview budget at XL (≤ 10 s, NFR-PERF-08) must be gated.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **`undo` crate** 0.52.0 | Low adoption (about 8.7k downloads in 90 days); no fit with derived data |
| **Whole-snapshot undo** | Memory per step; no field-level journal or diff |
| **SQLite session extension** | Tied to SQLite as the live model (rejected in ADR-0001) |
| **Hand-written inverse commands** | Every command needs a second, error-prone implementation; before/after ops invert by swapping |
| **Per-page undo stacks** | Cross-page edits (renumber, IP pairing, import) have no single page |

## Related

- **ADRs:** [ADR-0001](0001-core-model-persistent-maps.md), [ADR-0002](0002-derived-data-engine-ew-memo.md),
  [ADR-0007](0007-explicit-connections-and-autoconnect.md), [ADR-0009](0009-invariant-catalogue.md),
  [ADR-0010](0010-project-storage-jsonl-directory.md), [ADR-0012](0012-git-integration.md),
  [ADR-0013](0013-revisions-and-as-built-archive.md), [ADR-0014](0014-gui-toolkit-egui.md), [ADR-0020](0020-extensibility-tiers-and-security-model.md), [ADR-0021](0021-importers-emit-format-level-records.md).
- **Design:** [architecture](../design/04-architecture.md) §2, §5; [UX/UI](../design/16-ux-ui.md) §7–§8;
  [extensibility](../design/17-extensibility.md) §2.1, §2.3; [domain model](../design/03-domain-model.md) §5.1, §8.6.
- **Requirements:** FR-UI-022, FR-UI-023, FR-EXT-012, FR-EXT-015, FR-EXT-017, FR-NUM-008, FR-MAC-005, FR-IMP-004,
  FR-COL-005, FR-COL-007, NFR-UX-07, NFR-PERF-08, NFR-PERF-19, NFR-REL-02, NFR-REL-08.
