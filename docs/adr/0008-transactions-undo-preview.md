# ADR-0008: Transactions, undo/redo and preview-as-fork

**Status:** Accepted · amended 2026-10-18 · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the architecture judge panel)

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

Accepted on 2026-10-18. The decision text above is unchanged; this entry refines it.

1. **Confirmation of non-undoable operations** (2026-10-18; [UX/UI](../design/16-ux-ui.md) §7–§8). Confirming every
   export would train users to click through. Every non-undoable operation is therefore **flagged** as a marker line
   in the history, and only the **destructive** ones ask for confirmation: library writes outside the project,
   overwriting an existing file, purging library copies, and the empty-project import checkpoint. FR-UI-023 and
   NFR-UX-07 state the same rule.

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
  [extensibility](../design/17-extensibility.md).
- **Requirements:** FR-UI-022, FR-UI-023, FR-EXT-012, FR-EXT-015, FR-EXT-017, FR-NUM-008, FR-MAC-005, FR-IMP-004,
  FR-COL-005, FR-COL-007, NFR-UX-07, NFR-PERF-08, NFR-PERF-19, NFR-REL-02, NFR-REL-08.
