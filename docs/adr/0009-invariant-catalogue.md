# ADR-0009: Invariant catalogue: structural vs configurable prevent rules; lenient import

**Status:** Accepted · amended 2026-10-18, 2026-10-19, 2026-10-21, 2026-10-22, 2026-10-23 (specified: two invariants for page items and structure boxes) · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the architecture judge panel)

## Context

- **Some rules must always hold.** References resolve; a device has exactly one main function; a placement belongs
  to a device. Violating them makes derivations undefined.
- **Some rules must be enforceable but relaxable.** The pilot user's legacy QET projects contain exactly the data
  these rules forbid:
  - the second terminal of group "3+" on +JE2416-X1031 has no designation (D-11);
  - X1031 carries "1+" and "1-" four times each and "2+", "4+", "2-", "3-" and "4-" twice each, and all five
    terminals of X1032 read "PE" (D-12).

  A migration that refuses such projects fails the pilot's primary path (NFR-UX-06).
- **Per-row constraints block legitimate transient states.** Proposal C's SQLite `UNIQUE`/`CHECK` constraints cannot
  be deferred. Swapping two terminal designations or renumbering a strip would need a two-phase rename, and staging a
  legacy import with the same DDL is impossible.
- **Checking only at release is too late.** Bad data accumulates, and xrefs and terminal plans built on it are wrong
  in the meantime.
- **Release has its own hard rules.** Author = checker on every page of the reference set (D-34).

## Decision

1. **When.** Invariants are evaluated **on the candidate snapshot at transaction end**, limited to touched keys and
   their index neighbours. Transient states inside a transaction are legal.
2. **Structural invariants** are always on and reject the transaction:
   - references resolve, with generated delete policies `cascade`, `unlink` or `forbid` per link field (from D);
   - one main function per device;
   - the assembly graph is acyclic;
   - a placement references a device;
   - a channel belongs to exactly one module;
   - connection endpoints exist and differ;
   - a representation's functions exist;
   - a released revision has a checker who is not its author (D-34).
3. **Prevent rules** are configurable through rule packs (ADR-0019):
   - terminal designation non-empty (D-11) and unique per strip (D-12);
   - device tag unique within its structure scope;
   - PLC address unique per area, tested as bit-range overlap within the configuration project (`IW0` against
     `I1.3`; EW-PLC-001, FR-PLC-044), and a CP in at most one channel (FR-PLC-005);
   - interruption-point name unique per scope and direction (reworded by amendment 1);
   - conductor colours consistent with the cable part (D-17).
4. **Strictness is a project setting.**
   - `Strict` (default for new projects): a violation rejects the transaction with
     `CmdError::Prevented { rule, finding }` and a proposed alternative ("designation 4 is taken on -X1031; use 21 or
     swap 3 ↔ 4").
   - `Lenient { since }`, set by an import: violations that arrive through `ApplyImport`, an external change or a merge
     are committed as findings of the same rule ID. Interactive and scripted edits are still validated strictly, so a
     new duplicate drawn afterwards is rejected.
   - "Switch to strict" is enabled when no lenient violation remains. In release mode every prevent violation is an
     error, whatever the strictness.
5. **Catalogue.** The enumerated catalogue (about 40 rules) lives in `ew-model/invariants.md`, with a stable ID per
   rule, and every rule has its own unit test. `ew check --integrity` re-evaluates the whole catalogue on open and
   after merges (ADR-0012).

## Amendments

Accepted on 2026-10-18. The decision text above is unchanged; these entries refine it.

1. **Interruption-point pairing** (2026-10-18; [domain model](../design/03-domain-model.md), open question 4).
   "Name unique per scope and direction" would forbid the star mode, in which one source feeds targets of the same
   name on several pages (FR-XRF-007, FR-XRF-008). The prevent rule therefore reads: **at most one star source per
   name and scope; in chain mode an even count, ordered by sort key.** Unpaired and ambiguous points are findings of
   the check of FR-CHK-017 (EW-CORE-052). The integrity check uses the same wording (ADR-0012, FR-CHK-012).
2. **Interruption-point functions without a device** (2026-10-18; same document, open question 5; ADR-0006,
   amendment 1). A new structural invariant: **`Function.device` is None if and only if the function's kind is
   `Ip`.**
3. **Interruption-point functions belong to their page** (2026-10-18, core team;
   [domain model](../design/03-domain-model.md) §4.2, §10;
   [project format](../design/05-project-format-and-persistence.md) §2.2). An IP function has no device to own it
   (amendment 2), and the format already files it with the page of its representation, so its lifecycle follows
   that file: deleting the page, or the IP function's representation, cascades the IP function, and the
   connections at its CP cascade as for any function. A new structural invariant, **EW-INV-013: a function of kind
   `Ip` has exactly one representation**, evaluated at transaction end like every structural invariant. An IP
   function therefore never lacks an owning file and never sits on two pages; the catalogue is
   `crates/ew-model/invariants.md` (decision 5).
4. **Acyclic structure inheritance** (2026-10-19, core team, after the independent review of cycle 6; decision log
   ID-101; [domain model](../design/03-domain-model.md) §8.2 step 5). Tags and structure are inherited along three
   kinds of edge: a terminal from its strip, a `Child`-scheme device from its parent, and an unplaced device from its
   parent. The combined graph of these edges is acyclic: a new structural invariant of `ew-model`'s catalogue
   (decision 5), so a derivation never meets an inheritance cycle. The fix round adds it with the next free number
   (EW-INV-015 at the cycle-6 integration) and names that number in its commit (task FT-36).
5. **Prevent rules at MVP; EW-RDS-004 without prevent** (2026-10-21, project owner (delegated), after the
   independent review of cycle 12; decision log ID-306, ID-311). Decision 3 lists "device tag unique within its
   structure scope" among the prevent rules; at MVP EW-RDS-004 runs online and in release only and declares no
   prevent mode. Device tags are derived, so a duplicate full designation usually arises from a derivation input
   (structure, frozen literals that do not yet reserve their counters) rather than from the batch that touches the
   device, and a prevent rule would refuse unrelated edits; duplicates stay visible at once and block release. As
   built, the prevent hook receives the base snapshot as well as the candidate (`PreventHook::check(base,
   candidate, touched)`), so only violations a batch creates are refused and a violation the model already held does
   not block an edit that leaves it alone (decision 4, FR-CHK-011). The hook is implemented by `ew-api`, which may
   depend on `ew-checks`; `ew-commands` may not. The one shipped prevent rule is EW-TERM-002 (unique per strip,
   D-12); EW-TERM-001 cannot fire while designations are non-empty identifiers (FT-141), and EW-CORE-052's prevent
   mode (amendment 1) follows with FT-194. Reading the project's strictness and the import origin is FT-199. The
   base-snapshot rule is the MVP rule for every project (decision log ID-323); for `Strict` projects the target is
   restore-on-touch, where a transaction that touches a key with an existing violation must restore it (FT-210).
6. **The released revision's checker and its immutability** (2026-10-22, project owner (delegated ruling R3), at
   the cycle-17 design pass after the typed revision, FT-165; decision log ID-430;
   [domain model](../design/03-domain-model.md) §18). Decision 2's last bullet, "a released revision has a checker
   who is not its author", cannot hold for a **historic** released revision, one released before the project came
   to EW: Golden A's V1.0 has neither author nor checker, and no check could have run on it. The structural
   invariant therefore reads: **a revision's checker, when set, is not its author** (EW-INV-018). That a revision
   released by EW has a checker is the release gate's (EW-REL-001, run by `revision.release`), which refuses a
   release without a valid sign-off. **Released revisions are immutable** (FR-REV-009) is a structural invariant of
   `ew-model` as well, **EW-INV-020**: a revision that was released or withdrawn before the batch is not updated;
   its removal (the undo of `revision.create`), Open → Released and a new withdrawal record stay possible. It is a
   model rule, not a command rule, so it holds for every writer (an import, a merge, a plugin). The label's format
   and natural order stay the prevent and release-gate rule EW-REL-004, not a structural invariant. The other
   revision invariants are EW-INV-016 (at most one open revision), EW-INV-017 (labels unique among the revisions
   not withdrawn) and EW-INV-019 (one page note per page); `Revision.withdraws` is a forbid link (decision 2's first
   bullet).
7. **The `LibRef` keep-alive is a delete-time rule, not a structural invariant** (2026-10-22, lead programmer,
   FT-316 at cycle 19, with the project owner's delegated rulings on its review; the design pass records the
   ruling in the decision log; [architecture](../design/04-architecture.md) §4.6, §5.3, §13;
   `crates/ew-model/invariants.md`). The defect was recorded by the lead designer as decision log ID-462, which
   placed the fix as a delete-time prevent check in `edit.delete` and the purge. Decision 2's first
   bullet makes "references resolve, with generated delete policies" structural. For the two `LibRef` links
   (`symbol.fndef`, `part.function_template[].fndef`) the forbid policy is not a property of a state: a `LibRef`
   to an item the project does not embed names nothing and is valid (NFR-REL-11), so the state before
   `library.embed` is valid. Enforcing the keep-alive inside the structural check refused the undo of the
   embedding and broke `undo(all) ∘ apply(cmds) = id` (ARCH §13). The rulings:
   - **Beyond ID-462: a model rule, not a command check.** ID-462 put the check into the two delete commands.
     The rule is instead the model's own, EW-INV-021 below, evaluated for every writer on the edit path: a
     command that removes a copy without the delete plan, a later command, a plugin, the replay of a journal on
     a moved base. A check in two commands would leave each new writer to remember it, which is the class of
     defect the structural catalogue exists to prevent (decision 2); the delete plan still refuses the removal
     first, so `edit.delete` and the purge report it before they build a batch.
   - **Two apply paths.** The edit path, `Model::apply_with`, by which every command commits, evaluates the
     structural invariants, then the model's delete-time rule, then the prevent hook. The history path,
     `Model::apply_history`, by which undo and redo restore a committed state, evaluates the structural
     invariants only (ARCH §5.3: undo and redo skip prevent rules). The history path was `Model::apply`; it is
     renamed so that its name says what it skips, and no edit takes it by accident. `Model::delete_plan`, from
     which every delete command builds its batch, refuses the same removal first.
   - **The rule.** A new catalogue rule, **EW-INV-021, class delete-time**: an edit does not remove an embedded
     library copy that a record still names through a `LibRef` link. The refusal is `ModelError::Referenced`,
     with the same message as before. It is the model's own rule and always strict: no strictness relaxes it,
     because it guards against the silent loss of a definition that a part or a symbol needs, not a drawing
     convention. It holds for every writer on the edit path, a command that bypasses the delete plan included.
     The integrity check never evaluates it. EW-INV-001 no longer covers `LibRef` links, in either half.
   - **Its ID.** EW-INV-021 takes the next number of the `ew-model` catalogue (decision 5). `EW-INV` stays outside
     the rule families of doc 15 §2.2: the rule has no code, Fluent message or registry entry, and a refused edit
     produces no finding.
   - **The other forbid links stay structural.** Every other forbid link stores its target's ID, so a missing
     target is a dangling reference in any state (EW-CORE-008 on open): the lineage, structure-node, section,
     device-parent and terminal-strip links, the station's configuration project, the part copies of
     `PartRef.copy`, `Function.def`, `Representation.symbol`, the forms of pages, report blocks and forms, and
     `Revision.withdraws`. No committed state holds a referrer without its target, so their forbid never blocks
     an undo or a redo. The per-class table is in `crates/ew-model/invariants.md` §4.
   - **The journal on a moved base.** Crash recovery replays journalled transactions on the history path. When
     the base moved (another writer saved), the replay's net removals are judged against the moved base as an
     edit on it would be: a library copy of the moved base that the replay removes while a record of the result
     names it through a `LibRef` is a collision (`ew_store::journal::Collision`, the member being the link), and
     nothing is applied. The judgement is net, so a journal that embeds a copy and undoes the embedding is no
     collision. Example: the journal deletes symbol S1 and the FnDef copy it named; the other writer added a
     symbol S2 that names the copy: collision, not a removal.
   - **No format change.** The schema and the files are unchanged; a project that opened before opens the same.
     `edit.delete` takes no library copy, and the purge (`library.purge_unused`, not built) inherits the rule
     through the delete plan.
   - **A third place for a rule.** The delete-time class sits beside the structural invariants and the
     rule-pack prevent rules. It is kept to this one rule; a further delete-time rule needs an amendment.

   The golden build now undoes completely, to the shell (607 steps); the command fuzzer's `library.embed` arm
   builds the shape, and the undo-all oracle undoes it.
8. **Page items and structure boxes** (2026-10-23, lead designer, specified for WP-S2.04 part 4a; decision log
   ID-566; [domain model](../design/03-domain-model.md) §4.2, §6; [project format](../design/05-project-format-and-persistence.md)
   §2.2). Two structural invariants join the `ew-model` catalogue with the next numbers (decision 5) when part 4a types
   the reserved kinds `page_item` and `structure_box`:
   - **EW-INV-022:** the `page` of a page item or a structure box exists. The link is a cascade: deleting a page
     deletes its page items and structure boxes in the same plan (FR-PAG-025).
   - **EW-INV-023:** every connection that a wire marker lists exists and lies on the marker's page. The delete policy
     is unlink: a deleted connection leaves the list, and a marker left without connections is deleted in the same
     plan. `StructureBox.external` is an unlink link as well (deleting the external reference clears it).
   - **Older files are reported, not refused.** A build from before part 4a keeps `page_item` records verbatim and does
     not enforce these links (ADR-0011 amendment 4), so a project can arrive with a marker that lists a deleted
     connection or one on another page. Opening it is a **load finding** that names the marker and the connection;
     the derivation and the drawing ignore the stale entry and the next edit of the marker writes a clean list. This
     differs from amendment 6's open failure, because a stale list loses no data and the record stays editable.
   Until part 4a both numbers are reserved; `crates/ew-model/invariants.md` gains them with the part.

## Consequences

### Positive

- Legacy projects import with a migration report instead of failing; the report is the lenient check run (FR-IMP-003).
- Swaps, renumbers and strip reorders need no two-phase rename (FR-TRM-022).
- Structural integrity is guaranteed for every writer, including plugins and MCP agents.
- Each invariant is individually tested and documented by its stable ID.

### Negative

- Rules are split across two mechanisms: structural invariants in `ew-model` and prevent rules in rule packs. The
  catalogue must say which is which.
- Lenient projects carry known violations until they are resolved, and every derivation must tolerate them (for
  example two terminals both designated "1+").
- Evaluating over touched keys and index neighbours depends on correct index declarations; the field-mask test of
  ADR-0001 guards them.
- A transaction can be rejected late, after normalisers ran. The error must name the rule and the object.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **Database constraints** (non-deferrable `UNIQUE`/`CHECK`) | Block transient states and legacy staging |
| **Checking only at release** | Bad data accumulates; derived outputs are wrong until then |
| **Everything configurable, including structural rules** | Derivations would have to handle dangling references and multiple main functions |
| **Per-op validation** | Same transient-state problem as database constraints |

## Related

- **ADRs:** [ADR-0001](0001-core-model-persistent-maps.md), [ADR-0006](0006-function-device-representation-model.md),
  [ADR-0008](0008-transactions-undo-preview.md), [ADR-0012](0012-git-integration.md),
  [ADR-0013](0013-revisions-and-as-built-archive.md), [ADR-0019](0019-rule-engine-and-rule-packs.md), [ADR-0021](0021-importers-emit-format-level-records.md).
- **Design:** [architecture](../design/04-architecture.md) §4.6, §17; [domain model](../design/03-domain-model.md)
  §4.2; [checks, revisions and collaboration](../design/15-checks-revisions-collaboration.md) §3.5;
  [interop](../design/14-interop.md) §3.3.
- **Requirements:** FR-DEV-004, FR-DEV-030, FR-TRM-002, FR-TRM-022, FR-NUM-012, FR-PLC-005, FR-PLC-044,
  FR-XRF-007, FR-XRF-008, FR-CHK-011, FR-CHK-012, FR-CHK-016, FR-CHK-017, FR-IMP-003, FR-REV-005, NFR-MAINT-06, NFR-UX-06.
