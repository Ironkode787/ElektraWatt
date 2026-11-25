# ADR-0035: One home for each thing, an order of crates, one path for a text, and a shell that decides what is said

**Status:** Accepted (the release manager, 2026-11-23, on the conditions in [the plan](https://github.com/Ironkode787/ElektraWatt/blob/cf5d3e1fb778c424c56f8a6827e6058dce11cdb6/docs/plan/m3.5-architecture.md) section 4; amended 2026-11-24 and 2026-11-25, amendments 1 to 3) · **Date:** 2026-11-23 ·
**Deciders:** the release manager (the redesign); the project owner (the items of the owner's list in
[the plan](https://github.com/Ironkode787/ElektraWatt/blob/cf5d3e1fb778c424c56f8a6827e6058dce11cdb6/docs/plan/m3.5-architecture.md))
**Amends:** [ADR-0024](0024-licence-set-and-permissive-gpl-boundary.md) amendment 5 (the source of the edges),
[ADR-0019](0019-rule-engine-and-rule-packs.md) decision 1 and amendment 4 (a finding's message),
[ADR-0014](0014-gui-toolkit-egui.md) decisions 3 and 4 (what the shell and a tool own),
[ADR-0011](0011-format-versioning-and-migration.md) (the `min_reader` stamp).

## Context

- **What a scan found.** Seven readers read the repository region by region at the second candidate of 0.3.5,
  each a region it had not built, and listed about 230 findings. Most are one thing written in several places,
  some already apart: where a wire ends is decided in four places, a size and a slot print differently in the
  window and in a list, one copy of the list of record kinds lacks five kinds.
- **The copies are symptoms.** [Architecture](../design/04-architecture.md) §20 names seven gaps of structure
  behind them: domain knowledge without a home, edges nobody could read off, declarations that hand out nothing,
  texts without a carrier below the session, what a person is told decided in the window, a check narrower than a
  release's, and documents that record history. Merging the copies alone would let them come back.
- **What it costs.** A fix touches 7 files at the median and 12 or more in a quarter of the fixes; a lane spends
  55,000 to 180,000 tokens on one fix; each persona run still meets 8 to 13 findings, the largest theme being that
  the window did not say why.
- **Binding rulings of the owner.** The code is read and changed by models that start cold: one place for every
  item, no list kept in step by hand. A replaced thing is removed in the same change. Proportion: an early alpha,
  one developer, one pilot; no abstraction with one user. The licence boundary holds. The latest version leads
  (ADR-0011 amendment 8). No behaviour changes by a redesign.

## Decision

1. **One home.** A rule, a list, a deciding number, the printed form of a value and a word are written once. The
   home of a thing is the crate that defines the type it is about, when every user can depend on that crate and
   the thing needs nothing above it; else the first crate in the order that has what it needs. When a user cannot
   reach the home, the thing moves down. Architecture §3.4 is the table of homes. A test that holds two copies
   equal is removed with the copy.
2. **An order of crates is the source of the edges.** `ci/dep-rules.toml` lists every workspace crate once, in
   tiers (values, records, importers, project, files, drawing, operations, session, shell, window, programs). A crate
   depends only on crates before it (normal and build dependencies), and `cargo xtask rules` fails on any other
   edge and on a crate without a place. The manifest states the edges; the crate map shows them with the tier.
   The architecture document holds no graph and no table of crates.
3. **Rule 6 of the architecture reads: any earlier crate, named in the manifest.** The confinements stay as they
   are: the licence boundary, raw reads of the model, the owners of SQLite, egui and wgpu, the importers, the QET
   importer, the lists of the shell, the window and `ew-drafting`. The edges this adds are `ew-commands` to
   `ew-format` and `ew-checks`, `ew-library` to `ew-store`, `ew-text` to `ew-schema`, `ew-drafting` to `ew-units`,
   and to `ew-l10n` from the seven crates that own text files (the shell's list gains it).
4. **Rule 7 reads: a crate names what it uses.** Only the three facades whose users are confined to them re-export
   workspace crates: `ew-api` for the shell, `ew-viewmodel` for the window, `ew-interop-api` for the importers.
5. **The licence boundary without copies.** The permissive crates hold what is stored and how it is written. How
   a value is printed for a person and where a thing is drawn are the application's and live in `ew-derive` and
   `ew-scene`. A thing both sides need lives on the permissive side and is written there; existing GPL code moves
   there only with its copyright holders' consent, named in the commit.
6. **The scene answers where things are.** `ew-api` fills a page's view through `Q`; `ew-scene` draws it as a pure
   function, one memo family per layer, and hands out what it computed: the point of each connection point, the
   rectangle of each entity, the box and the slot of each text. The session computes no page geometry.
7. **A declaration hands out what its users need.** `entities!` gives `Model` its kind-erased accessors, and no
   crate above matches over the record kinds; the same holds for library kinds, check rules, list kinds, settings,
   actions and the subcommands of `ew` (architecture §3.5).
8. **Facts below, a `Text` above.** A crate returns typed facts and no sentence. A text for a person is a `Text`,
   a key with its arguments, in the new crate `ew-l10n` (values tier, GPL): the type, the generator of the key
   tables for the build scripts, and the one renderer over Fluent. The words stand in the text files of the crate
   that says them (`texts/<language>/<area>.ftl`): `ew-derive` for the words of the domain's kinds, `ew-checks`,
   `ew-commands`, `ew-reports`, `ew-api`, `ew-viewmodel` and `ew`. Every seam that carries words for a person
   carries a `Text`. Of the four ways a text travels today, the keyed text of `ew-commands` and the window's
   Fluent files with keys checked at build stay and become this one; the checks' own catalogue with a message
   string beside each pattern, pairs of English and Dutch words in code, and English built in code and shown to
   a person go.
9. **A text is rendered once, at the edge:** the window in its language, a page, a list, an export and a stamp in
   the project's language, the command line in English. en-GB is mandatory and the
   fallback; every language holds every key.
10. **A finding carries its text.** `Finding` holds the key of its rule's pattern and its arguments; its en-GB
    message is that pattern rendered, not a second string. The rule messages are a part of the one catalogue, and
    `ew-checks` keeps no renderer of its own.
11. **The shell decides what is said and where.** `ShellModel::dispatch` ends with the outcome placed (under a
    row, in the dialog, in the HUD, in a toast, in the banner). The shell answers which screen shows, why an
    entry is greyed (a text with arguments) and what a key does (the keymap, with a context).
12. **A tool states what follows its commit.** The host calls the tool with what was done, and the tool answers
    the sentences to say, the selection to leave and whether it gives way. The host holds no branch on a command's
    or a tool's name, and no state of one feature.
13. **A notification is a record** of a tone, a title, sentences and offered actions; an offered action is an
    action of the table with its parameters.
14. **The window draws.** It holds no branch on an action's identifier, a tool's name or the words of a message.
15. **Tests and the check.** A test depends on its crate, `ew-testdir` and its crate's one support module; window
    tests run on the frame clock; the integrity check runs after every command in the harness. `check -p` adds by
    the paths a change touches what only a release's exit ran, and writes down the tree it passed on. The rules
    gain a size ceiling for source files, a reason for every action that can be greyed, and the rule of decision
    16.
16. **A design document states what is and what is intended.** No note headed "As built", no dated amendment, no
    number of a row or a round, nothing a generator produces, no table of requirements covered, no open
    questions. A document is split where a chapter is read alone; section numbers stay. `cargo xtask rules`
    fails on history in `docs/design` once the documents are brought over.
17. **`min_reader` is the format.** With the latest version leading, the stamp is the format the build writes; the
    tables that computed it per kind, member and variant go.
18. **The rework changes no behaviour.** It moves and merges. Where two copies give different results, both
    forms are kept side by side under their names in the one home, and each caller keeps its form; making them
    one, like every other change of what the program does, says or stores, is decided item by item from the
    owner's list of the plan.

### Amendment 1 (release manager, 2026-11-24): the command line keeps its sentences

Decision 9 gave the command line a text folder of its own. Built, it was 482 entries in each of two languages
and about 2,000 lines more: a sentence that was read in the code where it is printed became a key with numbered
arguments, and the Dutch entries were printed by no program and read by no check. The command line is an edge
with one language. Its own sentences stay English in the code, one printer spells the prefix of an error, a
warning and a notice, and what comes from the layers below is a `Text` rendered in English. A text folder for
the command line is added on the day it is given a language.

### Amendment 2 (release manager, 2026-11-25): the gaps, recorded here

The architecture's §20 named the seven gaps of the context with their cost; a design document states what is, so
the record of what they cost moved here and §20 keeps the gaps by name with the section that closes each.

| # | The gap | What it cost | Closed by |
|---|---|---|---|
| 1 | **Domain knowledge had no home.** The crates were ordered by mechanism (record, model, derivation, command, scene, list, session, shell, window); nothing said where a value's printed form, a rule that a command and a drawing both need, plain arithmetic or a deciding number lives. The right crate often held it as a private helper | A wire's end decided in four places; a size, a slot and a length printed differently by the window and the lists; twelve rounding helpers with at least three tie rules | §3.4 |
| 2 | **Nobody could read off whether an edge is allowed.** The document's graph was the source, the rule file said the manifests were, and types travelled through re-exports; a missing edge was taken as forbidden, the thing restated and the copy pinned by a test | Five restated items and four pin tests in the commands; two lock implementations whose stale rules differ; the session computing again where things are on a page (connection points, text slots, the size of pin texts) beside the scene that drew them | §3.1, §3.2 rules 6 and 7 |
| 3 | **A declaration did not hand out what its users need.** `entities!`, the rule registry, `ReportKind`, the action table, the clap tree and the settings were each one declaration, yet every user matched over the members again | A new record kind touched over 30 files; one copy lacked five kinds, so a preview could count a changed table as unchanged; a new command with its action touched 18 files | §3.5 |
| 4 | **A text had no carrier below the session.** The keyed text lived in `ew-commands` and its words in the window's file, so a crate below or beside them could only return English, a pair of English and Dutch in code, or keep a second catalogue; facts travelled inside sentences and were parsed back | Every finding's English written twice (about 180 blocks) and held equal by tests; English in a Dutch window from five crates; the window's text file, changed in 117 commits since 0.3.0, rebuilt the commands, the session and the shell each time | §18 |
| 5 | **What a person is told was decided where no test sees it, and what follows a commit was found by the command's name.** The window compared the shell's state before and after an act to place a refusal; the tool host read a command's name and its JSON back to learn its duty | The largest theme of the persona rounds, "it did not say why"; the files changed most since 0.3.0 were the shell's (`properties.rs` 18 commits, `jobs.rs` 15, `shell.rs` 13); a 370-line commit arm | §9 |
| 6 | **The check of a change was narrower than the check of a release, and each test built what it shares.** | A replay that reported success while it checked nothing; two tests that failed only under load; about 1,000 lines of repeated test set-up | §13 |
| 7 | **Documents and comments recorded how a thing came beside what it is, and restated what a generator produces.** | 698 notes headed "As built"; eight documents near 200 KiB of which a builder needed a twentieth; contradictions that gave a cold builder a wrong rule | §19 |

The findings that built a concept ahead of its use and the special cases of the placer were not gaps of
structure: principle 14 covers the first, and the placing rules are the pilot's and the owner's.

### Amendment 3 (release manager, 2026-11-25): where an outcome is placed, and an offer's label

Decision 11 said that `ShellModel::dispatch` ends with the outcome placed. Built, `dispatch` runs an action and
returns its outcome, and `ShellModel::run_said` runs an act and places that outcome: the window runs every act it
dispatches through it, and the headless runner every step of a script, so a script still sees what a person sees.

Decision 13's offered action keeps a word of its own as its button's label, not the action's menu text: a toast's
button is short and names the act of the toast's sentence ("Undo", "Connect", "Keep the old tags"), where a menu
text names the act among the others of its menu. A toned toast's title is the part of its first text before the first `": "`.

Why an entry is greyed stays a text with arguments (decision 11): a reason that names what is missing (a page kind,
a selection) carries it as an argument, as every other text does.

### Amendment 4 (release manager, 2026-11-26): what the rework weighed and left as it is

The rework to this record met twelve places where one form for two was possible and was not made. Each stays as
it is, for the reason given, until that reason goes:

1. A rule's six-digit code stays as each rule declares it: a designed number that the message panel prints and
   the findings sort by ([15](../design/15-rule-engine.md) §2.2).
2. The lists keep their refill loop (`ew-reports/src/paginate.rs`).
3. The command parameters' schema types stay in `ew-commands/src/proxy.rs`: sharing them renames entries of the
   published schemas.
4. A toast's button keeps its own short word (amendment 3), and an offer of a toast runs with its own parameters.
5. The window's commit and the headless one stay two named forms (`ShellModel::commit`, `run_logged`), as do the
   two preview-then-commit helpers: each records and words a refusal its own way.
6. The describe text and the state's JSON of a headless run stay two forms (`ew-viewmodel/src/describe.rs`,
   `script.rs`): each prints other members in other units, and one view for both changes bytes of every
   transcript.
7. A refused preview's card keeps the command error's words.
8. Of the shell's long functions `resolve_at`, `act_macros_save` and `station_boxes` stay whole: their steps
   would share fourteen or more locals.
9. The window's events that call a setter of the shell stay events (the palette's typing, a form's fields, a
   toast's Close and timeout): as actions each would add a line to every run's transcript, and none is an act a
   script needs.
10. The prelude of the persona brief on a timed circuit keeps its three embedded part copies: they are copies of
    the sample parts library, and an action that embeds a library part by its number would exist for that one
    prelude.
11. The table of settable members (`ew-commands/src/commands/fields.rs`, `SETTABLE`) stays: its line decides that
    a member may be set.
12. The size ceiling of a source file is the one of `xtask/src/ceiling.rs`; at 2,000 lines thirty files would be
    split.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **Merge the copies where they stand, no change of structure** | The reasons a copy was written remain: the next builder who needs a rule from a crate that is not in the manifest, or a word below the session, writes the next copy |
| **A crate for shared domain rules** (`ew-domain`) below the derivation | A second place to look beside the crate that defines the type; most of the rules need derived facts and so belong in `ew-derive`, which every user already reaches. It would have one reason to exist: that helpers were private |
| **Layers as a matrix** (named layers, each with the layers it may use) | It would also forbid sideways edges such as a renderer reading a record. No such edge was ever written; the two accidental edges in the history pointed upward, which the order catches. A matrix costs a judgement per new crate and a table to keep true |
| **The layers in prose only, no check** | The document's graph was that, and it drifted from the manifests and the rule file until builders took a missing edge for a forbidden one |
| **Printed forms as methods of the permissive types** (`Display` in `ew-schema`, `ew-units`) | It moves GPL code across the boundary for every value and puts the application's taste (decimal places, the word for an unknown colour) into the format crates that third parties build on |
| **One crate that holds every text file** | The simplest structure, but every text edit would rebuild all crates from the checks upward, and a word would stand far from the code that says it. Today's most-changed file would sit under seven crates |
| **Keep the four mechanisms and mend each** (render findings from their patterns inside `ew-checks`, leave headings as pairs in code) | No carrier below the session remains the cause: the next layer that must tell a person something writes a fifth |
| **Words as `MlText` in every shipped language, made where they are found** | Every producer formats each language eagerly, arguments cannot change order per language, and a third language is a change of code in every crate |
| **`Text` and its renderer in `ew-mltext`** | It puts Fluent into a format crate that third parties depend on |
| **A file-system crate** (`ew-fs`) for the seam, the lock, the clock and the atomic write | A new crate for what `ew-store` already holds; the edge from `ew-library` costs one manifest line and a longer first build of that crate |
| **Move the page-view glue from `ew-api` into `ew-scene`** | The two changed together in 4 of 17 commits; the report tables of a page need `ew-reports` and would stay behind, splitting the glue; the copies came from the scene handing out too little, which decision 6 mends without the move |
| **A crate for `ew-api`'s output; two crates for the view-model** | Measured: one edit costs 13 to 22 s before its tests start, so a split saves seconds for a crate more to find one's way in |
| **A command declares what follows its commit** (in `ew-commands`) | What follows is the window's affair (a selection, a sentence, the next tool), and several tools commit one command with different follow-ups |
| **The follow-up as data on the commit, no call back** | Most follow-ups read the page after the commit: the wires a placement drew, an error a wire brought |
| **The window keeps placing a refusal, on a richer outcome** | The place depends on the shell's state (an open dialog, a row with an error, the tool's notice); the window would keep reading that state, and a script would still not see it |
| **A rule that detects copies** | Not decidable. A public home and the type of a seam do the work: a seam that carries a `Text` cannot carry a sentence |
| **Generate the record kinds from a schema file** | `entities!` is one declaration already; it lacks accessors, not a generator |
| **Keep the large documents and bound their growth by a count** | A count pins a total and keeps the notes; a builder still opens 200 KiB for one section |

## Consequences

### Positive

- A builder finds the home of a new thing from one table and the answer to "may this crate use that one" from one
  list.
- Adding a record kind, a rule, a list kind, a setting or a subcommand touches its declaration.
- A wording is one entry per language; a word of the window rebuilds the shell and the window only.
- A script and a view-model test see what a person sees, so "it did not say why" can be tested below the window.
- The differences between copies become visible side by side, each a decision of two lines.

### Negative

- One new crate, `ew-l10n`, and a text folder in seven crates. A word of the domain rebuilds everything from the
  derivation upward when it changes, which is seldom.
- `ew-library` builds `ew-store` and with it `ew-model`.
- A new crate needs its place in the order.
- A rework of about 13,000 lines deleted and about 5,000 moved to another home (the readers' estimates, not
  measured), in twelve stages, during which the code is in part not yet as the architecture says; the plan names
  what is open.
- A lane that merges two copies must keep each caller's result, which is slower than taking one of them.
- The architecture document names things a third party never sees (the shell's rules); the format's own
  documents stay the place for those readers.

## Related

- **Design:** [architecture](../design/04-architecture.md) §3, §9, §13, §18 to §20;
  [project format](../design/05-project-format-and-persistence.md) §4;
  [checks](../design/15-checks-revisions-collaboration.md) §3.8; [UX and UI](../design/16-ux-ui.md);
  [design language](../design/16a-design-language.md).
- **Plan:** [the rework](https://github.com/Ironkode787/ElektraWatt/blob/cf5d3e1fb778c424c56f8a6827e6058dce11cdb6/docs/plan/m3.5-architecture.md): its map, its order and the owner's list.
- **ADRs:** [ADR-0001](0001-core-model-persistent-maps.md), [ADR-0008](0008-transactions-undo-preview.md),
  [ADR-0011](0011-format-versioning-and-migration.md), [ADR-0014](0014-gui-toolkit-egui.md),
  [ADR-0019](0019-rule-engine-and-rule-packs.md), [ADR-0024](0024-licence-set-and-permissive-gpl-boundary.md),
  [ADR-0034](0034-schematic-drafting.md).
