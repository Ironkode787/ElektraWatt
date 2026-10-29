# WP-S1.16: the MVP rule packs (closed specification)

**Status:** specified in cycle 34 (documents only), an M2 package on WP-S1.16a's engine, in five parts: part 1
potential naming, the 0 V–PE bond quick fix of EW-POT-001 and the user-named potential of the Potentials table
(0.5 pw, D3; FT-551, FT-577; lane F34 builds it in cycle 34), part 2 the EW-POT pack completed (1.0 pw, D4), part 3
the EW-CORE rules of cables, parts and devices (1.1 pw, D4), part 4 pages, texts, designations, terminals and the rule
system (1.0 pw, D4), part 5 Fluent messages, waivers, the AC-21 subset and the AC-24 rows (1.1 pw, D4); part 1 first;
parts 2 to 4 after it in any order; part 5 last · **Parts 1 and 2 built in cycle 34** (lanes F34 and I34, decision log ID-801, ID-803; Golden A's data fixed to doc 08 by lane X34, ID-804); parts 3 to 5 open · **Date:** 2026-10-29 · **Work package:**
[WP-S1.16](../02-work-packages.md#wp-s116-mvp-rule-packs-ew-core-ew-iec81346-ew-term-ew-pot) · **Decision log:**
ID-784 · **Part of:** ElektraWatt plan, specifications

**Purpose.** This is the closed specification of the package that turns the 19 rules WP-S1.16a shipped for the pilot
preview into the four MVP packs of doc 15 §4: every MVP rule of EW-CORE, EW-IEC81346 (EW-RDS and EW-NUM), EW-TERM and
EW-POT, their messages in Fluent, waiver recording, and the four-pack part of AC-21 and the package's AC-24 rows. Its
first slice closes the two G-PP leftovers on potentials (FT-551, FT-577) that lane F34 builds in cycle 34. It is
written against the code at `357124dc` (§2.1). Where this document and doc 15 differ, this document is the narrower and
wins for the lanes; doc 15 stays the target and is updated at the design pass after each part (§13.3). A choice a lane
meets that this document does not settle is escalated, never guessed (`CLAUDE.md`, rule 2).

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [Part 1: potential naming and the 0 V–PE bond](#5-part-1-potential-naming-and-the-0-vpe-bond)
6. [Part 2: the EW-POT pack completed](#6-part-2-the-ew-pot-pack-completed)
7. [Part 3: cables, parts and devices](#7-part-3-cables-parts-and-devices)
8. [Part 4: pages, texts, designations, terminals and the rule system](#8-part-4-pages-texts-designations-terminals-and-the-rule-system)
9. [Part 5: messages, waivers and the acceptance sets](#9-part-5-messages-waivers-and-the-acceptance-sets)
10. [Determinism and the golden rule](#10-determinism-and-the-golden-rule)
11. [Files owned and what a lane may not decide](#11-files-owned-and-what-a-lane-may-not-decide)
12. [Risks](#12-risks)
13. [Size, order, routing and the documents to update](#13-size-order-routing-and-the-documents-to-update)
14. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package `ew check --scheme pre-delivery` on Golden A reports exactly the rows of
`findings_mvp.csv` that belong to the four packs, each message renders in en-GB and nl-NL from Fluent, a finding can
be waived with a recorded reason, and the package's defect regressions (AC-24) pass. A potential without a definition
is named after the supply it comes from before it falls back to a CP's name, and a 0 V that is bonded to PE is
accepted when the bond is declared, which one quick fix declares.

**Non-goals** (each is named with its package in §13.4):
- No rules of other packs: EW-PLC, EW-BUS (WP-S1.21), EW-FL (WP-S1.20), EW-LAY (WP-S4.01), EW-MFG, EW-IMP, EW-REV
  (their packages). AC-21's rows of those packs are compared when they are built (FT-589).
- No rhai or SQL rules and no company packs (WP-S1.27, 1.0); the four packs stay Rust (ADR-0019 as built).
- No new rule IDs: every rule here has its row in doc 15 §4 and `rules.json`; a lane that needs another stops (§11.2).
- No change of a rule's severity, class or mode against doc 15 except where §4 rules it.
- EW-CORE-008's other discriminators (FT-266, FT-350) and the layout half of every rule stay with their packages.

## 2. Inputs and the code as it is

| Input | What the lanes take from it |
|---|---|
| [Checks, revisions and collaboration](../../design/15-checks-revisions-collaboration.md) §2 (engine, findings, fingerprints, waivers, quick fixes), §3.8 (messages), §4 (the catalogue) | Every rule's text, class, modes, severity, target, FR and defect link |
| [Connections, terminals and cables](../../design/08-connections-terminals-cables.md) §3.5 to §3.8, §8 | Names, flow, bonds, rail bonding, the rule algorithms of EW-POT and EW-TERM |
| [Golden project](../../design/20-golden-project.md) §18 | The expected findings of Golden A (AC-21's source) |
| ADR-0009, ADR-0019 (as amended), ADR-0024 | Invariants against rules, the rule engine and packs, the dependency licences (Fluent is MIT OR Apache-2.0) |
| Requirements | The 19 rows of §3; FT-184, FT-186, FT-188, FT-189, FT-190, FT-192, FT-194, FT-195, FT-196, FT-209, FT-229, FT-236, FT-238, FT-239, FT-260, FT-261, FT-266, FT-350, FT-368, FT-494, FT-551, FT-577, FT-587, FT-588 |

### 2.1 What exists at `357124dc` (the base contract)

| Item | Where | Used here for |
|---|---|---|
| The engine (`Rule`, `Registry`, schemes, modes `o`, `r`, `p`, findings with `targets`, `discriminator`, `fingerprint`, `fix: Option<QuickFix{label, command, params}>`), 19 rules in `rules.json` (EW-CORE-023, -033, -047, -051, -052, -053, -065, -081; EW-NUM-001; EW-POT-001, -002, -008 rail half, -011, -012, -014; EW-RDS-004; EW-TERM-001, -002, -005), the catalogue test | `crates/ew-checks/src/**`, `crates/ew-checks/catalogue/rules.json`, `crates/ew-checks/tests/**` | The packs this package completes |
| The quick-fix action `messages.fix` of the message panel, which runs a finding's `QuickFix` as a command | `crates/ew-viewmodel/src/actions.rs`, `navigators/messages.rs` | Part 1's bond fix and every later fix |
| The potential name memo: step 1 definition, step 2 source IP (external last), step 3 fallback `<tag>:<designation>`; EW-POT-001 and EW-POT-017 as derive notes | `crates/ew-derive/src/names.rs` | Part 1's step 2b |
| `bond.create {a, b, kind, via?}` with `BondKind::Pelv0vPe`; a bond with `via` keeps 0 V and PE as two potentials | `crates/ew-commands/src/commands/connection.rs`; `crates/ew-derive/src/potentials.rs` | Part 1's quick fix |
| The Potentials table's defining cells: Type, Voltage and `env.ac_dc` on a CP-only row compile to one `potential.define` that stores the shown name (frozen, pinned by a test, ID-780) | `crates/ew-viewmodel/src/navigators/table.rs` (`compile_define`), `crates/ew-api/src/output/navigators.rs` | Part 1's FT-577 |
| The blind recording `gpp-2` that replays the defining path | `tools/blind/recordings/gpp-2/**` | Re-blessed in part 1 |
| `waiver` (rank 61, `waivers.jsonl`), reserved | `crates/ew-schema/src/kind.rs` | Typed in part 5 (ID-334) |

## 3. Requirements traced

| Requirement | Rule(s) | Part | Acceptance |
|---|---|---|---|
| FR-CON-012 (as it bears on names), FR-CON-014 | Naming step 2b; EW-POT-001's bond fix | 1 | P1-1 to P1-6 |
| FR-CHK-019 | EW-POT-013 | 2 | D-13 |
| FR-CHK-020 | EW-POT-006 | 2 | D-38 |
| FR-CHK-030 | EW-POT-003 | 2 | D-16 |
| FR-CHK-055 | EW-POT-008 (PE-island half) | 2 | D-21 (MVP part) |
| FR-CHK-057 | EW-POT-014 (as built), FT-188 | 2 | P2 rows |
| FR-CHK-032, FR-CHK-056 | EW-CORE-031, -032, -035 | 3 | AC-21 rows |
| FR-CHK-034 | EW-CORE-043, -044 | 3 | D-15 |
| FR-CHK-036 | EW-CORE-045 (reads WP-S1.19's `Part.contacts`, or the FnDef pattern until it lands) | 3 | AC-21 rows |
| FR-CHK-022 | EW-CORE-046 | 3 | AC-21 rows |
| FR-CHK-060 | EW-CORE-054, EW-RDS-007 | 3, 4 | AC-21 rows |
| FR-CHK-023, FR-CHK-051 | EW-CORE-061 (one finding per style) | 4 | D-31 |
| FR-CHK-026 | EW-RDS-003 | 4 | AC-21 rows |
| FR-CHK-027 | EW-RDS-002 | 4 | AC-21 rows |
| FR-CHK-037 | EW-NUM-002, EW-NUM-001's warning half | 4 | AC-21 rows |
| FR-CHK-039 | The device half only (EW-CORE-041's "no part" and the placement half stay with EW-LAY-001, WP-S4.01) | 3 | FT-589 |
| FR-CHK-042 | The pre-delivery scheme: the four-pack subset of `findings_mvp.csv` | 5 | AC-21 (subset) |
| NFR-COMP-05 | Rule data with a `verified` flag (EW-POT-006's sizing table) | 2 | P2 row |

## 4. Decisions

Each is a lead ruling that the owner may reverse. The design pass records them under ID-784.

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **Naming step 2b, the supply's name.** Between steps 2 and 3 of doc 08 §3.5: a potential with no definition and no source IP that is entered through the pass-through pair of a function f (its `upstream (f, pair)` of doc 08 §3.6) is named `<root>/<tag>:<designation>`, where `<root>` is the name of the nearest upstream potential named by step 1 or 2 (walking `upstream` through potentials named by 2b), `<tag>` is f's displayed tag without its leading aspect sign and `<designation>` is the designation of the pair's downstream CP: `L/S51:2`. The name is unique by construction (one CP is in one potential). A potential with no upstream, or whose walk reaches no step-1 or step-2 name, keeps step 3 | FT-551's "the supply's name before its first CP's name"; the downstream CP makes it unique without a global pass, so the memo stays per potential. The trade-off is a longer name than the fallback |
| D2 | **A bonded 0 V stays its own potential.** EW-POT-001's member-role conflict that contains `M` and `PE` gets the quick fix "Declare 0 V–PE bond (PELV)": `bond.create {a, b, kind: pelv_0v_pe, via}` on a carrier wire of the potential (D3). The alternative of FT-551, one potential with role PE, is rejected: 0 V wires would take PE's colour and class and EW-POT-002 (exactly one 0 V–PE bond) would go silent | The design already models the bond (doc 08 §3.7, FR-CON-014); the bond keeps both potentials' properties and the PELV rule meaningful |
| D3 | **The carrier.** A connection c of the merged potential is a carrier when removing it splits the potential's connection graph (members, bridges and rail bonds included) into two parts, one with every `M`-role member CP and the other with every `PE`-role member CP. Candidates order by: one end on a PE-role member CP first, then the wire's displayed number in natural order, then the two ends' fallback texts (D1's `tag:designation`); the fix takes the first; when no carrier exists, or the first two share every key, the finding has no fix and its hint names Draw ▸ Bond. `a` is c's end on the M side, `b` its end on the PE side. The computation is a derive query, never a rule-side walk | Deterministic on printed keys (new projects have random IDs, ID-677); no guess when the drawing gives none |
| D4 | **FT-577: the user names the potential.** On a Potentials row known only at its CPs, the Name cell is a defining cell: typing a name compiles to `potential.define {at: the row's minimum member CP, name}` with the other defining cells of the gesture; Type, Voltage and `env.ac_dc` on such a row without a name in the same gesture refuse in the cell with "Name the potential first". The stored shown name, its frozen behaviour and the test that pins it are retired; `gpp-2` is re-blessed (§5.4) | One rule for every definition: a name the user chose. The trade-off is one more keystroke before a Type |
| D5 | **Rule data with a verified flag.** EW-POT-006's PE sizing table ships as rule data in `crates/ew-checks/data/pe_sizing.toml` with `verified = false` and the clause it implements cited, never reproduced (NFR-COMP-05); a finding from unverified data says so in its message | The table must be checked against the standard by a person with access; shipping it flagged keeps the rule useful meanwhile |
| D6 | **Fluent in `ew-checks`.** `fluent-bundle` (exact pin, `cargo vet` and `cargo deny` entries, ADR-0023) in `ew-checks`; one file per language `crates/ew-checks/messages/<lang>/rules.ftl` with `rule-<id lower case>` and `.title`, `.hint` (doc 15 §3.8); `Finding.args` holds the typed arguments; `Finding.message` stays the en-GB rendering, byte-equal to the text each rule prints today; `ew_checks::render(&Finding, lang)`; `ew check --lang <tag>`. A CI test fails on a rule without its message, `.title` or `.hint` in en-GB, and lists missing nl-NL ones as warnings until 1.0 | Doc 15's design with the least churn: every output that prints `message` today stays byte-equal |
| D7 | **Waivers (FT-494).** `waiver` is typed under ID-334 (`{id, fingerprint, rule, reason, author, at, expires?}`; `at` and `author` are parameters, never read from a clock or the OS in a command); `check.waive` and `check.unwaive` are `HumanOnly` commands; the engine marks a finding whose fingerprint matches as `Waived` | FR-CHK-009, FR-UI-021; a scripted waiver would defeat the record |
| D8 | **Fingerprints of undefined potentials (FT-588 item 1).** For EW-POT-001, -002, -011, -012 and -014 on a potential without a definition, the fingerprint is computed from the discriminator member (the potential's minimum member CP by D1's printed key) alone, while every member stays a display target; `finding_node`'s primary-target tie breaks on the printed target text (item 2) | A waiver must survive adding a member; display keeps every member |
| D9 | **Definition order (FT-587).** A potential with several definitions ranks them by natural order of name, then by the minimum member CP's fallback text, then by ID (*as built, cycle 34, 10b79802: the second key is the defining point's printed key, a junction after every CP; the lead accepts the code, ID-803*); the first applies its values; every definition is a target; the EW-POT-001 text names CPs in that order | Doc 08 §3.6's natural order (ID-175) and replay stability |
| D10 | **The AC-21 subset.** Part 5 writes `crates/ew-api/tests/expected/findings_mvp.csv` whole from doc 20 §18 and compares the rows of the four packs plus EW-CORE-083 and -084; rows of other packs are listed in the test as pending with their package and switched on by FT-589 | The golden file is written once and correct; the gate's full comparison waits for the packages that own the rules |
| D11 | **Part 1 at 0.5 pw ahead of the pack.** Naming, the bond fix and FT-577 are one slice because they share the potential name and the gpp-2 re-bless; they land in cycle 34 on WP-S1.16's budget | G-PP's leftovers close first; the later parts do not touch names |

No decision changes an ADR. ADR-0019 (Rust rules in packs, Fluent messages), ADR-0009 (rules against invariants),
ADR-0011 amendment 4 (typing `waiver` within 0.1), ADR-0023 (the Fluent dependency) and ADR-0024 (Fluent's licence is
in the allowed set; `ew-checks` is GPL) are applied as written.

## 5. Part 1: potential naming and the 0 V–PE bond

**Scope.** D1 to D4, FT-551 and FT-577, built by lane F34 in cycle 34. **0.5 pw, D3.** Consistent with FT-551 (a
declared name, then the supply's name, before a CP's name; a declared bond the rule accepts, so EW-POT-001 does not
fire) and FT-577 (the name cell defines the row, or Type and Voltage refuse with "name the potential first"; the
stored derived name retired; the recording re-blessed).

### 5.1 Build list

1. **`ew-derive` `names.rs`: step 2b (D1).** In `PotentialNameQ::run`, after the source-IP branch and before
   `fallback_name`: read the potential's `upstream (f, pair)` from the flow query (`potential_flow`, doc 08 §3.6);
   walk `upstream` until a potential named by step 1 or 2 (recursion through the name memo of the upstream potential
   is allowed; flow is acyclic by construction, and a revisit ends the walk at step 3); build
   `<root>/<tag without aspect sign>:<downstream CP designation>`. `NameSource` gains `Supply { via: Id<Function> }`.
   Doc comment cites doc 08 §3.5 step 2b.
2. **`ew-derive`: the carrier query (D3).** A new memo `BondCarrierQ(PotentialId) -> Option<Carrier{via:
   Id<Connection>, a: EndpointKey, b: EndpointKey}>` in a new module `bond_carrier.rs`, over the potential's member
   connections, bridges and rail bonds; bridges of the graph found by one DFS (Tarjan) in a deterministic node order
   (printed key); only called for a potential that has both `M` and `PE` role members.
3. **`ew-checks` `rules/pot.rs`: the fix on EW-POT-001.** When the role conflict contains `M` and `PE` and the
   carrier exists, `with_fix(QuickFix { label: "Declare 0 V–PE bond (PELV)", command: "bond.create", params: {a, b,
   kind: "pelv_0v_pe", via} })`; otherwise the message gains the hint "declare the bond with Draw ▸ Bond". The finding
   text and fingerprint stay as built otherwise.
4. **`ew-viewmodel` `navigators/table.rs`: FT-577 (D4).** The Name cell of a CP-only row becomes a defining cell
   routed to `name`; `compile_define` takes `name` from that cell and no longer from `at.name`; a gesture whose
   defining cells set Type, Voltage or `env.ac_dc` without a Name refuses the cell with the reason "Name the potential
   first" (the strings table gains the text; the cell shows it as the shell shows other refusals); `ew-api`'s
   `NavNode::define_at` stops carrying the shown name if nothing else reads it.
5. **Retire the frozen path:** remove the test that pins the stored derived name (ID-780) and replace it with D4's
   tests (§5.3 P1-4, P1-5).
6. **Re-bless `gpp-2`** by the harness's re-bless procedure (WP-S6.13): its replay must still pass its two replay
   requirements (twice-run equal, and equal to the recording apart from the changed lines); the lane lists every
   changed line of the recording (expected: the potential names that step 2b changes, `S51:1` among them, and the
   defining steps that now carry a typed name). If the procedure cannot hold its requirements, the lane stops and
   reports.
7. **Docs (as built, by the lane in its owned files only):** none; the design pass updates doc 08 §3.5 (§13.3).

### 5.2 What part 1 must not change

Golden A's exported bytes, its findings and AC-15's command log: every Golden A potential is named by step 1 or 2 (the
lane asserts it in P1-6 before changing anything; if one is named by step 3, the lane stops and reports the potential,
because the golden outputs would change). No severity, class or ID changes; no new command (the fix uses `bond.create`
as built).

### 5.3 Acceptance of part 1

| # | Check | Where | Proves |
|---|---|---|---|
| P1-1 | A fixture: L defined, fuse -F1 (pair 1→2), switch -S51 (pair 1→2), lamp; the potential between -F1 and -S51 is `L/F1:2`, the one after -S51 is `L/S51:2`; a potential with no upstream keeps the fallback; twice-run and ID-permuted copies give the same names | `ew-derive` tests | D1 |
| P1-2 | 0 V and PE joined by one wire W27 through a terminal: EW-POT-001 carries the fix with `via` = the wire on the PE-role side (the first by D3's keys); applying it (through `messages.fix`'s command path in an `ew-api` test) leaves two potentials, 0V and PE, and no EW-POT-001; EW-POT-002 is satisfied by the one bond; undo restores the finding | `ew-checks`, `ew-api` tests | D2, D3, FT-551 |
| P1-3 | Two parallel wires between the 0 V and PE groups: no carrier, no fix, the hint names Draw ▸ Bond; a member-role conflict without `PE` (M against L) has no fix | `ew-checks` tests | D3 |
| P1-4 | On a CP-only Potentials row, typing a Name defines the potential with that name at the minimum member CP; typing Name and Type in one gesture gives one `potential.define` with both | `ew-viewmodel` tests | D4 |
| P1-5 | Typing Type or Voltage on a CP-only row without a name refuses with "Name the potential first" and creates nothing; on a defined row Type sets as built | `ew-viewmodel` tests | D4, FT-577 |
| P1-6 | Golden A: no potential named by step 3 (asserted first); the golden output, acceptance and AC-15 tests unmodified and green on the lane's targeted runs | `ew-api` tests | §5.2 |
| P1-7 | `gpp-2` replays under the re-blessed recording, twice-run equal; the lane's report lists the changed lines | harness test | §5.1 item 6 |
| P1-8 | Targeted checks for `ew-derive`, `ew-checks`, `ew-viewmodel`, `ew-api`: nextest, clippy `-D warnings`, `cargo fmt --all --check`, `check-deps`, `check-features`, `docs-gen --check` | lane | — |

## 6. Part 2: the EW-POT pack completed

**Scope.** EW-POT-003, -004, -005, -006 (D5), -008 PE-island half (FT-189, on the own locations as built), -009, -013,
-015 (as doc 15 defines it), -016, -017 as a rule (today a derive note); FT-587 (D9), FT-588 (D8), FT-188, FT-236.
**1.0 pw, D4.** Each rule: the registry row, the Rust implementation in `rules/pot.rs`, tests from doc 08 §8's
algorithm with one positive and one negative fixture, the Golden A rows of `findings_mvp.csv` it owns asserted.
**Acceptance (outline closed to the checks):** P2-1 each rule's fixtures; P2-2 D-13 (EW-POT-013 on T55, S56), D-16
(EW-POT-003 on -201U1 pin 3), D-21 MVP part (clearing `rail_bonded` on -X1032 gives one EW-POT-008 warning), D-38
(EW-POT-006 with a 2.5 mm² PE) pass; P2-3 the ID-permutation test of FT-587 and the fingerprint test of FT-588; P2-4 the
sizing table carries `verified = false` and the finding says so; P2-5 targeted checks.

## 7. Part 3: cables, parts and devices

**Scope.** EW-CORE-031 (live and prevent, FT-261), -032, -034, -035, -036 (prevent), -037, -041 (with the
`supply_scope` exemption once FR-DEV-037's attribute exists; else the exemption is FT-209's and the lane reports it),
-042, -043, -044, -045 (reads `Part.contacts` of WP-S1.19 part 1 when merged, else the FnDef's contact pattern), -046,
-048, -054; FT-190, FT-239, FT-260; FT-368 as EW-CORE-008 with the discriminator `form`. **1.1 pw, D4.**
**Acceptance (outline closed to the checks):** P3-1 each rule's fixtures (positive, negative; prevent rules refuse the
candidate and only it); P3-2 D-15 (the TBD address and the placeholder parts), D-19 (EW-CORE-034) pass; P3-3 the
Golden A rows of these rules in `findings_mvp.csv`; P3-4 targeted checks.

## 8. Part 4: pages, texts, designations, terminals and the rule system

**Scope.** EW-CORE-021, -022 (prevent), -025, -026, -049 (if the hand-drawn part-list data the rule needs is not in
the model, the lane stops and reports instead of building it), -061 (one finding per style, FR-CHK-051), -063, -066,
-071, -083 (caught panics, operation limits), -084; EW-RDS-001, -002, -003, -007, -009 (prevent), -010; EW-NUM-001's
warning half, EW-NUM-002; EW-TERM-003, -004 (prevent); FT-194, FT-195, FT-196; FT-229 as EW-CORE-008 with the
discriminator `channel`. **1.0 pw, D4.** **Acceptance (outline closed to the checks):** P4-1 each rule's fixtures;
P4-2 D-31 (exactly two EW-CORE-061 findings on Golden A); P4-3 a rule that panics yields EW-CORE-083 and the other
rules' findings unchanged; P4-4 the Golden A rows; P4-5 targeted checks.

## 9. Part 5: messages, waivers and the acceptance sets

**Scope.** D6, D7, D10; FT-494; FT-186 (EW-TERM-002's "renumber 1–n, keep labels as function text" quick fix, a
previewed `terminal.designate`). **1.1 pw, D4.** **Acceptance (outline closed to the checks):** P5-1 every registered
rule has `rule-<id>` with `.title` and `.hint` in en-GB (CI test); P5-2 `Finding.message` byte-equal to the text before
the change for every Golden A finding; `ew check --lang nl-NL` renders the Dutch text where present and en-GB
otherwise; P5-3 `check.waive` records a waiver with a reason, the finding turns `Waived`, the waiver survives a member
added to an undefined potential (D8), `check.waive` is refused from a script (`HumanOnly`); P5-4 the four-pack subset of
`findings_mvp.csv` equals the report of `ew check --scheme pre-delivery` on Golden A; the pending rows name their
packages; P5-5 the AC-24 rows of this package (D-13, D-15, D-16, D-19, D-21 MVP part, D-31, D-38) run in the defect
runner; P5-6 the quick fix of FT-186 previews and commits one undo step; P5-7 targeted checks plus `cargo vet` and
`cargo deny` for the new dependency.

## 10. Determinism and the golden rule

1. Every finding's order, text, target and fingerprint depends only on the project's content and printed keys, never
   on IDs where a new project's IDs are random (ID-677, FT-587, FT-588), on hash-map order or on time.
2. Waiver authors and times are command parameters (D7).
3. Golden A's exported bytes do not change in any part; `findings_mvp.csv` is new in part 5; the findings each part
   adds to Golden A are exactly its rows of that file.

## 11. Files owned and what a lane may not decide

### 11.1 Files owned

**Part 1:** `crates/ew-derive/src/{names.rs,bond_carrier.rs,lib.rs}` and its tests; `crates/ew-checks/src/rules/pot.rs`,
`crates/ew-checks/tests/pot.rs`; `crates/ew-viewmodel/src/navigators/table.rs`, the strings table entry, its tests;
`crates/ew-api/src/output/navigators.rs` (`define_at` only), the `ew-api` tests named in §5.3;
`tools/blind/recordings/gpp-2/**`. **Parts 2 to 5:** `crates/ew-checks/**` (rules, catalogue, data, messages, tests),
the derive queries a rule needs, `crates/ew-schema` and `ew-format` for `waiver` (part 5), `crates/ew-commands` for
`check.waive` (part 5), `crates/ew-api/tests/expected/findings_mvp.csv` (part 5), fixed by each part's dispatch brief.
Shared append-only files may be touched by other lanes; the second to integrate rebases. Read-only: `golden/**`,
`ci/**` except the vet and deny entries of D6, `.github/**`, `docs/**`.

### 11.2 What a lane may not decide

A lane stops and reports when:
1. a rule would need an ID, severity, class or mode not in doc 15 §4 and `rules.json`;
2. a Golden A export byte changes, or a Golden A finding appears that `findings_mvp.csv` (doc 20 §18) does not list;
3. part 1 finds a Golden A potential named by step 3 (§5.2), or the `gpp-2` re-bless cannot keep its replay
   requirements;
4. a rule needs model data that does not exist (EW-CORE-049, EW-CORE-041's attribute) beyond the fallback this
   document names;
5. a new dependency other than D6's, or a licence outside ADR-0024's set, would be needed;
6. a fix would need a new command (quick fixes use existing commands);
7. the work of another part is reached.

The lane decides freely: private helper names, fixture shapes, test names and the module layout inside its files.

## 12. Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 Step 2b renames potentials a user has learned (blind recordings, potential-oriented numbers built from fallback names) | Medium · low | Only undefined, source-less potentials change; the fallback was already unstable under renumber (doc 08 §11 item 12); gpp-2 re-blessed with its changed lines listed |
| R2 The carrier is ambiguous on real drawings | Medium · low | No fix rather than a guess (D3); Draw ▸ Bond stays |
| R3 `findings_mvp.csv` disagrees with what the rules report | Medium · medium | Doc 20 §18 is the source; a disagreement stops the lane (§11.2 item 2) and goes to a lead ruling on which is wrong |
| R4 The Fluent switch changes printed messages | Low · medium | `Finding.message` stays the en-GB rendering, byte-checked (P5-2) |
| R5 The package overruns 4.7 pw | Medium · medium | De-scope order (§13.1) |

## 13. Size, order, routing and the documents to update

### 13.1 Size

**4.7 pw PERT** as the CSV says, **no reserve**: part 1 **0.5** (step 2b 0.15, carrier and fix 0.15, FT-577 0.1,
re-bless and tests 0.1), part 2 **1.0**, part 3 **1.1**, part 4 **1.0**, part 5 **1.1** (Fluent 0.45, waivers 0.35,
AC-21 subset and AC-24 0.2, FT-186 0.1). **De-scope rule:** over 20 % at a lane's midpoint or review, these leave as
follow-ups in this order: nl-NL message texts beyond the golden findings, FT-186's quick fix, EW-RDS-010's merge quick
fix, EW-CORE-066; the lane reports which ones left.

### 13.2 Order and routing

Part 1 in cycle 34 (lane F34). Parts 2, 3 and 4 after part 1, in any order or side by side (they share `rules.json`
and `findings_mvp.csv`'s absence; the second to integrate rebases). Part 5 last. **Routing** (Opus or Haiku only):
part 1 **Opus at `medium`** (derive and the recording); parts 2 to 4 **Opus at `medium`**; part 5 **Opus at `high`**
(a new dependency, a typed record, the acceptance file). **Depends on:** WP-S1.16a (built); WP-S1.19 part 1 for
EW-CORE-045's typed contacts (optional, §7). **Unblocks:** G-M2-03's four-pack share; WP-S1.18's release blockers read
the waivers.

### 13.3 Documents the design passes update

**Now (cycle 34):** plan 02 (WP-S1.16's status and this specification); decision log ID-784; FT-589, FT-590.
**After part 1:** doc 08 §3.5 (step 2b, the stored-name text replaced), §3.7 (the bond fix); doc 15 §4 EW-POT-001's
quick fix; doc 17 (the Potentials table's name cell). **After parts 2 to 4:** doc 15 §4 rows as built. **After part
5:** doc 15 §2 (waivers, fingerprints of undefined potentials), §3.8 (messages as built); doc 02 appendix (the Fluent
dependency).

### 13.4 Follow-ups

1. AC-21's rows of other packs (EW-PLC-012, EW-BUS-001, -004 in WP-S1.21; EW-FL-001, -004 in WP-S1.20; EW-LAY-001 in
   WP-S4.01) and the full-file comparison: FT-589.
2. The blind task gpp-101-103 tells the drafter to declare bond B1 (PE-0VDC) so the oracle's "no EW-POT-001" is
   reachable by the drafting path as well as by the quick fix: FT-590 (WP-S6.13).
3. FT-266, FT-350 (EW-CORE-008's other discriminators) with their packages.

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S1.16a, WP-S1.16, WP-S1.18, WP-S1.19, WP-S1.20, WP-S1.21, WP-S4.01,
  WP-S6.13; Appendix B (FT-184, FT-186, FT-188, FT-189, FT-190, FT-192, FT-194 to FT-196, FT-209, FT-229, FT-236,
  FT-238, FT-239, FT-260, FT-261, FT-266, FT-350, FT-368, FT-494, FT-551, FT-577, FT-587, FT-588, FT-589, FT-590).
- [Decision log](../06-decision-log.md): ID-175, ID-305, ID-334, ID-677, ID-772, ID-776, ID-780, ID-781.
- [ADR-0009](../../adr/0009-invariant-catalogue.md), [ADR-0019](../../adr/0019-rule-engine-and-rule-packs.md),
  [ADR-0023](../../adr/0023-supply-chain-and-release-integrity.md),
  [ADR-0024](../../adr/0024-licence-set-and-permissive-gpl-boundary.md).
- [Checks, revisions and collaboration](../../design/15-checks-revisions-collaboration.md) §2, §3.8, §4;
  [connections, terminals and cables](../../design/08-connections-terminals-cables.md) §3.5 to §3.8, §8;
  [golden project](../../design/20-golden-project.md) §18.
- Requirements: FR-CHK-019, FR-CHK-020, FR-CHK-022, FR-CHK-023, FR-CHK-026, FR-CHK-027, FR-CHK-030, FR-CHK-032,
  FR-CHK-034, FR-CHK-036, FR-CHK-037, FR-CHK-039, FR-CHK-042, FR-CHK-051, FR-CHK-055 to FR-CHK-057, FR-CHK-060,
  FR-CON-012, FR-CON-014, NFR-COMP-05.
