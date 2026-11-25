# ElektraWatt architecture

**Scope.** The structure of ElektraWatt (EW): the core idea, the crates and their order, where each kind of thing
lives, what is declared once, how a text reaches a person, what the shell and the window own, what a test may
depend on and what a design document holds.

**How to read it.** This file is what a builder reads before a change that crosses a crate or adds a kind of
thing. It is kept under 45 KiB, about 11,000 tokens, so that it can be read whole beside the code of the change.
The chapters are in four files beside it, under the section numbers that the code cites as `ARCH §n`; read a
chapter by the section you need:

| Sections | File |
|---|---|
| §2 process and thread model, §4 core model, §5 editing, §7 persistence | [04a](04a-model-editing-persistence.md) |
| §6 derived-data engine | [04b](04b-derived-data.md) |
| §8 rendering, §10 extensibility, §11 reports and forms, §12 checks | [04c](04c-rendering-extensibility-reports-checks.md) |
| §14 defects prevented, §15 first build order, §16 risks and non-goals, §17 alternatives, Appendix A | [04d](04d-defects-risks-alternatives.md) |

The decisions behind this document and the alternatives that lost are in the [decision records](../adr/README.md);
[ADR-0035](../adr/0035-homes-order-texts-shell.md) decides §3, §9, §13, §18 and §19. Where the code is not yet as
this document says, the row that brings it over is in [the backlog](../plan/backlog.md).

**Conventions, also of the chapters.** D-nn are the defects of the reference drawing set
([reference-drawing.md](../research/reference-drawing.md) §3). "Golden project" is the reference project that was
retired at 0.3.5 ([glossary](glossary.md)); the sample project (`fixtures/sample/`) takes its place in the tests.
G-n and C-n are items of [gaps-and-corrections.md](../research/gaps-and-corrections.md). **(verify)** marks a
version or an interface the research notes do not confirm; performance figures are estimates and not measured.
The document follows the public-repository rules: it names no customer, end user, project, drawing or person.

---

## 0. The architecture in short

A typed Rust domain model in persistent immutable maps is the single source of truth. Every edit is a transaction
of typed operations. Everything printed is a derived query.

| Layer | Choice |
|---|---|
| Core model | Plain Rust record structs in `imbl::OrdMap` tables; O(1) snapshots; UUID identity |
| Derived data | Pure query functions over a tracked-read context; cold first, `ew-memo` memoisation where profiled; oracle mode |
| Edits | `Command → Transaction(Vec<Op>)`; ops hold before and after rows; project-wide undo; a preview is a fork |
| Persistence | UUID-named JSON Lines project directory, sharded by the ID's last byte, manifest-last save, journal outside the project |
| Query and SQL surface | Read-only SQLite projection (`api_v1_*` views), sandboxed; never the source of truth |
| Texts | A text for a person travels as a key with its arguments and is rendered once, at the edge, in the window's or the project's language (§18) |
| UI | egui and eframe behind `ew-ui`, over the toolkit-neutral shell `ew-viewmodel`, which decides what is shown and said (§9) |
| 2D rendering | One display-list IR (`ew-dl`); a wgpu canvas and an always-built CPU path; PDF/A-2b |
| 3D and CAD | Specified: an own wgpu 3D renderer and an out-of-process STEP worker (§8.5, §8.6) |
| Extensibility | The `ew` command line; rhai, WASM components, MCP and a Python wheel are specified for later releases |
| Licensing | Application GPL-3.0-or-later; the lower layer of values, records and the file format MIT OR Apache-2.0, closed under its dependencies |

---

## 1. The core idea and the principles

### 1.1 Core idea

The worst defects in the reference set share one root cause: **a value that should have been computed was stored
as a copy, and the copy went stale** (a tag that kept its old number after a renumber, D-01; a layout label that
named another device, D-02; four revision formats, D-05; a count of valves that disagreed with the drawing, D-10).

EW removes that root cause:

- **A project is an immutable value:** typed tables of plain Rust structs in persistent maps, cloned in O(1).
- **The value changes only through a transaction of typed operations.** That transaction is also the undo record,
  the journal entry, the macro step and the diff.
- **Everything else is a query over that value:** tags, cross-references, nets, potentials, wire properties, title
  blocks, lists, check findings and display lists. A derived value cannot go stale, because no stored copy exists.

The same holds for the code. A rule, a list, a number or a word that is written twice is a stored copy, and it
drifts as a stored tag does: §3, §18 and §20 apply the core idea to the program itself.

### 1.2 Principles

1. **Store intent, derive presentation.** Only facts a human decided are persisted. Every string printed on a page
   is a query result.
2. **Derived by default, frozen on purpose.** Identifiers that leave the building (tags, wire numbers, PLC
   addresses, routed lengths) may be frozen explicitly; a check reports every frozen value that deviates from its
   rule (§6.5).
3. **One writer, many readers.** The window, the command line, scripts, importers and plugins edit only through
   `Command → Transaction → Op`. Readers get lock-free snapshots.
4. **Identity is a UUID, never a tag.** Every reference is an `Id<T>`. Tags, folios and wire numbers are
   properties.
5. **Typed, not property bags.** One Rust record struct per entity kind; multi-role devices get optional typed
   aspects (§4.3); unknown members are kept in `extra` maps.
6. **Cold first, cached where measured.** Queries are plain functions. Memoisation is added per query when a
   measurement asks for it, and an oracle verifies it (§6).
7. **Headless first.** Model, queries, commands, checks, lists and PDF all run without a window, and the window
   and a script act through the same shell (§9).
8. **Determinism everywhere.** Ordered maps, integer nanometres, fixed-point quantities and ID tie-breaks: the same
   model gives the same bytes, the same findings and the same PDF.
9. **Permissive bottom, GPL top.** The values, the records, the file format, the display list and the importer
   contract are permissive and closed under their dependencies (§3.3).
10. **Proved by tests that state a property** (§13): the oracle, undo and round-trip laws, shuffle-order
    determinism.
11. **One home.** A rule, a list, a deciding number and a word are written once, in the place §3.4 names, and
    everything else calls, derives or generates from it. A test that holds two copies equal marks a thing without
    a home.
12. **Facts below, words at the edge.** A crate returns typed facts; a text for a person is a key with arguments,
    made by one of the seven crates that own text files and rendered once, at the edge (§18).
13. **The shell decides, the window draws.** What is enabled, what is said and where, which screen shows and what
    a key does are the shell's; a script sees what a person sees (§9).
14. **In proportion.** A layer, a trait, a registry, a generator or a rule is added when it has a second user or
    catches a fault that happened, and says which.

---

## 3. Crates: the order, the rules, the homes, what is declared once

### 3.1 The order

Every workspace crate has one place in an order, written as tiers in `ci/dep-rules.toml`. **A crate depends only
on crates before it** (normal and build dependencies); `cargo xtask rules` fails on any other edge and on a crate
without a place. A crate's edges are its manifest's; the [crate map](../map/crates.md), generated from the
manifests, the rule file and the head comments, shows each crate's tier, edges and files. Nothing else restates
them.

| Tier | What its crates hold |
|---|---|
| values | What a length, an identifier, a multi-language text, a designation, a PLC address, a text by key and a display list are; the memo engine, the ZIP container, the job runner. No project record |
| records | The stored record of every kind, the file format, what an import hands over |
| importers | The bytes of another program's file as records |
| project | The project as a value (`ew-model`), and everything computed from it (`ew-derive`, `ew-plc`) |
| files | Projects and libraries on disk |
| drawing | A page in grid ticks, shaped text, a page as display lists, the renderers and the writers |
| operations | What is done with a project: it is judged (`ew-checks`), edited (`ew-commands`), listed (`ew-reports`), projected to SQL (`ew-sqlview`) |
| session | `ew-api`: one facade for every front end |
| shell | `ew-viewmodel`: what the application shows and does, toolkit-neutral |
| window | `ew-ui`: the window, in egui |
| programs | `ew`, the command line, and `elektrawatt`, the desktop program |

Inside a tier the order of the list binds as well. Beside these tiers the rule file places the workspace-hack
crate first and the test-only crates and `xtask` last. Test code (a dev-dependency) is not bound by the order; the
rule file lists what the tests of the shell and the window may use and keeps the test-only crates and features out
of a shipped program.

### 3.2 The rules

1. **Model access.** `ew-model`'s tables are `pub(crate)`, and its raw read API is compiled only with the feature
   `raw-read`, which `ew-derive`, `ew-commands` and `ew-store` alone enable on normal edges. Every other crate
   reads the model through the tracked context `Q` (inside queries) or through `TxBuilder` (inside commands); a
   query receives only `&Q`. The rule file's feature rule and the generated `clippy.toml` (the raw readers as
   `disallowed-methods`) enforce it. Rows do not leave by another route: the public surface of `ew-commands`
   carries summaries of ops, never rows. Asset bytes under a hash are outside the gate, since they never change;
   asset presence is read by no memoised query but the listed accessors of `Q` (ADR-0001, ADR-0002).
2. **SQLite.** Only `ew-sqlview` and `ew-library` link rusqlite.
3. **UI toolkit and GPU.** Only `ew-ui` and the desktop program know egui; only `ew-render2d` knows wgpu.
4. **Importers** depend on `ew-interop-api`, their parsers and permissive workspace crates only. They never see
   `ew-model`. Ported QElectroTech code lives only in `ew-import-qet`, and only `ew-api` depends on that crate.
5. **Model entities are the schema records.** `ew-model` wraps `ew-schema` record structs in tables. The GPL layer
   adds behaviour (ops, indexes, invariants, derivation) and no second data definition.
6. **Edges.** A crate may depend on any crate before it in the order, and names it in its manifest. Nothing else
   decides whether an edge is allowed but rules 1 to 4 and the three lists of the rule file: the shell depends on
   `ew-api` and on the value crates it names, the window on the shell, the renderers and those value crates, and
   `ew-drafting` on `ew-units` and `ew-ids` only, because the layout reads no project record. When a crate needs
   what a later crate holds, the thing moves down to its home (§3.4) in the same change. It is never written a
   second time, and no test holds two copies equal.
7. **A crate names what it uses.** A type or a function is named through the crate that defines it. Three facades
   re-export on purpose, because their users are confined to them: `ew-api` for the shell, `ew-viewmodel` for the
   window and `ew-interop-api` for the importers. No other crate re-exports another workspace crate or an item of one;
   the rule `re-exports` of `cargo xtask rules` holds it.

Three seams follow from the rules and are kept:

- **The shell sees the project only through `ew-api`,** the window only through the shell. `ew_api::output` holds
  what both front ends reach, `ew_api::window` what only the window shows and edits; `output` never names `window`.
- **A page is drawn as a pure function of a view.** `ew-api` fills a `PageView` through `Q` (what a page shows: its
  records and their derived texts), one memo family per layer; `ew-scene` draws the view and **answers where
  everything on the page is**: the point of each connection point, the rectangle of each entity, the box and the
  slot of each text. The scene calls the derivation's rules (rule 6 allows the edge) and hands out what it
  computed while drawing. `ew-api` asks the scene and computes no page geometry of its own.
- **The layout computes in grid ticks.** `ew-drafting` knows no nanometre and no record; `ew-scene` builds its
  page model once.

### 3.3 The licence boundary

- The **permissive layer** (MIT OR Apache-2.0) is what a third party needs to read, write, validate and import a
  project: the values, the records, the file format, the display list, the importer contract. No permissive crate
  depends on a GPL crate; `cargo xtask rules` reads the layer of each crate from its manifest's licence. A
  permissive crate builds on the Rust version its manifest names.
- Everything else is **GPL-3.0-or-later**. What only the application needs stays there even when it speaks of a
  permissive type: how a value is printed for a person and where a thing is drawn belong to `ew-derive` and
  `ew-scene`, not to the record.
- **A thing both sides need lives on the permissive side and is written there.** Existing GPL code moves into a
  permissive crate only with the consent of its copyright holders, named in the commit (ADR-0024). The project
  owner has given his, for his own code, once and for the moves to come (2026-11-23): a commit that moves such
  code says that it does so under that consent. Code that came from a third party under the GPL never moves.
  A thing is never kept twice with a test between the copies.
- `xtask` is permissive and links no GPL crate: it reads the committed parameter schemas and the rule catalogue's
  snapshot as files.

### 3.4 Where each kind of thing lives

**The rule.** Ask what the thing is about. Its home is the crate that defines that type, when every user can
depend on it and the thing needs nothing above it. When it needs more (other records, a derived fact, a
language), its home is the first crate in the order that has what it needs. When a user cannot reach the home, the
home is too high: the thing moves down. A function that exists is made public before a second one is written.

| The thing | Its home |
|---|---|
| Integer arithmetic: rounding with a named tie rule, `gcd`, the safe-integer bound, lengths to grid steps | `ew-units` (`arith`, `grid`), public |
| An identifier and every naming scheme of a derived (version 5) identifier | `ew-ids` |
| A multi-language text: the neutral text or the first language, language matching, natural order, confusable characters | `ew-mltext` |
| The validation and the parse of a stored value (a date, a hash, a version) | the value's type in `ew-schema`, as `FromStr` |
| A rule about one stored record alone (the envelope of a library copy, the function CP a symbol CP stands for) | a method of the record in `ew-schema` |
| The names of starter content that code refers to | `ew_schema::starter`, one table; the library's audit checks each name exists |
| A rule that needs several records or a derived fact (where a function CP is drawn, the open revision, the devices under a device, the grid labels of a frame, whether a slot shows) | one function or query of `ew-derive`, called by the commands, the checks, the scene and the lists |
| How a value of the domain is shown: a colour, a cross-section, a length, a slot, an address, a short tag as a string that no language changes; the word for a kind (a record kind, a port role, a potential type) as a text by key | `ew_derive::text`: one function per value; where two places print it differently, both forms stand there side by side under their names |
| Files on disk: the file-system seam, the lock, the clock and the civil date, the all-or-nothing write | `ew-store` |
| A page in ticks: sides, roles, rectangles, rounding to the grid, grouping; the numbers of the layout, a drawn mark's sizes and the commands' drawing distances; where a mark crosses its wires, in ticks and in nm | `ew-drafting` (`model`, `consts`, `marks`) |
| Where a thing is on a drawn page, the page-to-sheet map, the pens, how a marker is drawn | `ew-scene` |
| A list's rows, its cells and their constructors | `ew-reports` |
| A deciding number (a distance, a pitch, a clearance, a tolerance, a limit) | the `consts` module of the crate whose concern it is, named, with its unit and the section that sets it. A number used once stays beside its use, with its reason |
| A pointer tolerance; a default of a form | `ew_viewmodel::pointer`; the form's declaration |
| A size, a colour, a stroke, a share of the window | the tokens of `ew-ui` (`theme`) |
| A shortcut | the keymap of `ew-viewmodel`, with its context. The window binds no key |
| A word a person reads | a text file of the crate that says it (§18) |

### 3.5 Declared once

A kind of thing is declared in one place, and its users call what the declaration hands out. A user never
matches over the members by hand, and no second list is kept in step.

| A … | is declared as | and the declaration hands out |
|---|---|---|
| record kind | `Kind` in `ew-schema` and its row of `entities!` in `ew-model` | the kind-erased accessors of `Model` (a row's and a table's revision, a row as JSON, a row from a record, the tables in order), which the store, the commands and the memo keys call; its word by key |
| library kind | a variant of `LibDocument` | the envelope accessors; the directory name |
| command | its file and its row of `commands!` | the registry, the parameter schema, the reference page |
| action | its row of the action table: handler, enable check with its reason, text, icon, default key, parameters with their forms | the menus, the palette, the keymap, the action map, alias resolution |
| check rule | its `RuleMeta` (id, kind, modes, severities, scope, what it supersedes) and its row of `RULES` | the code, the pack, the documentation address, the registry, the catalogue; its texts by `rule-<id>` |
| list kind | a variant of `ReportKind` and its definition | its name (the stored spelling), its columns with their headings, every list of kinds a wizard, a navigator or an import shows |
| project setting | a typed member of `ProjectSettings` | its stored name, its reader, its member of `project.settings.set` |
| window setting | a field of `Settings` with its default | the reader and the writer of the settings file |
| `ew` subcommand | its clap arguments and its run function | the catalogue, the reference; the schema name of a `--json` result from the result's type |
| SQL view | a `View` with its fill function | the schema and the materialisation |
| text | one entry per language under one key | the key table, checked at build |

### 3.6 Adding a crate

A crate is added when something with more than one user can be hosted by no existing crate under §3.4; it takes
a place in the order, a licence and a head comment, and nothing else lists it. Specified and not in the workspace, each
to come when it has code: `ew-panel`, `ew-render3d`, `ew-mfg` (GPL); `ew-ewpp`, `ew-devdesc`, `ew-step-proto`,
`ew-plugin-wit`, `ew-plugin-sdk` (permissive); `ew-rhai`, `ew-script`, `ew-plugin-host`, `ew-step-worker`,
`ew-sysprobe`, `ew-viewer`, `ew-py` (GPL).

---

## 9. The shell and the window

`ew-api` is the session: the model, the history, the queries and the files behind one facade. `ew-viewmodel` is
the shell: `ShellModel` and the action table. `ew-ui` is the window. The desktop program opens the window or,
headless, runs a script on the same shell. **Every act goes through `ShellModel::dispatch`:** a menu entry, a key,
a palette entry, a dialog's button, a pointer event on the canvas and a script step are an action of the table
with its parameters. What the window sets on the shell beside `dispatch` belongs to the window alone (a pane's
size, the time of a frame); anything a script must be able to do is an action.

### 9.1 What the shell owns

- **Which screen shows:** `ShellModel::screen()` answers one `Screen` (the start page, the first run, a project,
  the symbol editor, …), and each model the window draws (the command bar, the panes, the dialog) is the shell's
  for that screen.
- **What can be done:** the action table's enable check is the one gate (`enabled_with`). It answers a reason as
  a text with arguments (§18); every greyed entry, on every surface, shows that reason. A handler reads the
  selection through the same typed readers as its enable check, so the two cannot disagree.
- **What a key does:** every key is a binding of the keymap with a context.
- **What is said, and where.** `ShellModel::run_said` runs an act and places its outcome: under the row or the
  field that caused it, in the open dialog, in the HUD of the tool, in a toast, or in the banner. The rule that
  picks the place (one fact in one place, [16a](16a-design-language.md)) is that one function of the shell; the
  window runs every act it dispatches through it, and the headless runner every step of a script. A script's
  transcript and a view-model test therefore show what a person sees.
- **The selection, the active tool and its host, the clipboard's content, the jobs.**

### 9.2 What a tool owns

A tool is a state machine over canvas events (`Tool`, `ToolCx`: the prepared page tables, the snapper, the
selection; no session). It owns:

- its state and its options (what the next click places, the rows of a strip being placed);
- the command it previews or commits;
- **what follows its commit:** after the host committed the tool's command it calls the tool with what was done
  (the command, the page tables before and after, the records made), and the tool answers the sentences to say,
  the selection to leave and whether it gives way to Select. The host applies that answer and holds no branch on
  a command's or a tool's name.

The host owns what is the same for every tool: the preview, the commit, the overlay, the one undo step. State of
one feature is never a member of the host.

### 9.3 A notification

A notification is a record: a tone, a title, its sentences as texts (§18) and the actions it offers. A sentence
is never glued to another in code or cut back out of one; the window lays the parts out. An offered action is an
action of the table with its parameters and the records to select, labelled by its own short word (not by the
action's menu text), with the `Validity` that says how long it can be taken. Whether a toast stays follows from its tone.

### 9.4 What the window decides

How a model of the shell is laid out and painted, what moves and fades, where the focus is, and which action an
input maps to. The window holds no branch on an action's identifier, a tool's name or the words of a message; it
does not work out which screen shows, whether a refusal is shown, or what a key means. One renderer per kind of
part (a menu, a tooltip, a raised surface, a form row, an action's button with its reason) is in `components`,
and sizes and colours are tokens.

### 9.5 The symbol editor

The symbol editor's canvas is the shell's canvas model and its tools are tools of §9.2, as every other tool's;
the window holds no tool framework of its own.

---

## 13. Tests and tools

### 13.1 What the tests prove

| Property | How |
|---|---|
| Memoised equals cold | Random command sequences on the sample and synthetic projects: after every step the memoised results equal a cold recomputation; the command fuzzer feeds the same oracle |
| Laws | `undo(all) ∘ apply(cmds) = id` for the model and the derived data; `load(save(m)) = m`, byte-stable; `merge(o,a,a) = a`; preview then commit equals execute; journal replay equals the live state |
| Determinism | Loading in shuffled record order gives identical files, findings and PDF bytes; serial and parallel exports give identical digests |
| Integrity after every command | In every test build the model's structural check (`Model::integrity_check`) runs after each changing command (`ew-commands`, feature `testkit`), so a command that leaves the project failing an invariant fails where it is made; a shipped program never builds it |
| Hostile input | Fuzzing of the loader, the migration chain, the merge driver, the importers and the SQL sandbox |

A behaviour is tested at the lowest layer that has it; a layer above tests what it adds. The test strategy in
full is [18](18-quality-security-release.md) §3.

### 13.2 What a test may depend on

- The crate under test through its public surface, and its `testkit` feature where it has one.
- `ew-testdir`: the temporary folder (`Tmp`), the sample project and a copy of it, the comparison of an expected
  file with its re-bless, and the sample's facts read from the sample.
- Its crate's one support module: the one builder of a project, a shell or a window driver, and the walkers over
  what a frame drew.
- Never: a copy of a helper; the total of a set that grows, a generated name or a path; the wall clock or a count
  of frames. A window test runs on the frame clock that `App::new` is given and waits on an event (a job ended, a
  tile is ready, a preview is ready).
- A test file is named for what it guards.

### 13.3 The check

`cargo xtask check -p <crate>…` formats, lints, runs the tests of the named crates and then the repository rules.
What a change needs beyond those tests is chosen by the paths it touches, not remembered: a change of the persona
harness, the content or `xtask` builds the programs, writes the actions and replays every brief, and fails where
something is missing, as does every check of the whole workspace; the paths a change touches are those of the commits
no remote holds yet and those not committed. A change of a text file needs nothing more, as the translation status
is a rule. `--deep` runs
the tests marked for the deep run. The check writes down the tree it passed on, and a gate runs it again only for a
tree that differs.

`cargo xtask help` lists the rules of `cargo xtask rules`, each with what it catches. Two of them hold the
structure of this document: **a source file over the size ceiling** (`xtask/src/ceiling.rs`; a file a generator
writes is not bound), with no list of exceptions; and **a control the window greys without a reason in words** (an
action of the registry cannot be greyed without one: its check returns its reason as a text). History in a design
document is §19's rule.

No test asserts a time. There is no continuous integration until 1.0: `check` is the check.

### 13.4 Tools

`cargo xtask outline` and `where` answer where a thing is by span; the maps under `docs/map/` are generated.
`cargo xtask blind replay <run> --until <frame>` puts a builder at the frame where a persona run was stuck.

---

## 18. How a text reaches a person

One path, from the place a thing is found to the words on a screen or a page.

1. **A crate returns facts.** An error is an enum with its data, a kind is an enum, a note of the derivation
   carries typed fields. A `Display` is English for a log and a developer; no window, list or page shows it, and
   no crate reads a fact back out of a sentence. Only the six crates of point 2 make a text for a person.
2. **A text for a person is a `Text`: a key and its arguments** (`ew-l10n`). An argument is a value as written (a
   tag, a name), a name a person wrote in several languages (printed in the reader's language), a number, another
   `Text` or a list of them joined by the words of the reader's language. It is made by the crate whose concern it is
   to tell a person, and its words stand in that crate's text files, `texts/<language>/<area>.ftl`:

   | Crate | Says |
   |---|---|
   | `ew-derive` | the word for a kind of the domain (a record kind, a port role, a line kind, a potential type), which the checks, the lists, the commands and the session all print |
   | `ew-checks` | a finding by its rule's pattern, a rule's title and hint, the label of a quick fix |
   | `ew-commands` | a refusal, a notice of an outcome, a record in words |
   | `ew-reports` | the name of a list kind, a column's heading, a word in a cell |
   | `ew-api` | what the session refuses or names beyond those: an edit of a table, a navigator's words, a stamp on a print |
   | `ew-viewmodel` | what the window says: labels, hints, toasts |

   The command line is an edge that speaks one language. Its own sentences are English, written where they are
   printed, and one printer spells the prefix of an error, a warning and a notice. What it prints of the
   layers below reaches it as a `Text` and is rendered in English. It has no text files: an entry in a second
   language that no program prints is carried unused.

3. **A seam carries a `Text`, never a `String`:** a command's error, a finding, a session error's refusal, an
   action's outcome, an enable check's reason, the sentences of a notification, a cell's word and a column's
   heading. The type is the guard: a `String` for a person exists only after rendering.
4. **Rendering happens once, at the edge:** `ew_l10n::render(text, language)`. The window renders in the window's
   language; a page, a list, an export and a stamp in the project's language; the command line in English. en-GB is the mandatory language and the fallback, and every language holds every key.
5. **Keys are checked when the code is built.** Each owning crate declares its text folder once; the one generator
   of `ew-l10n` writes the key table, and `text!` and `key!` fail the build on an unknown key or a wrong argument.
   A key is named through the macro of the crate that owns it, so a search finds the owner. A closed set names
   each of its keys literally in one method; no key is built from a name at run time.
6. **One word, one key.** A thing of the domain (a field, a choice, a column, a record kind) has one key, whichever
   surface shows it. A count and an optional clause are selectors over a numeric argument, not a second key.
7. **What the text files never hold:** what a person typed in several languages (`MlText`, project content), and
   values (a tag, a number, an address: printed by `ew_derive::text`). A text is never built from an identifier,
   a raw key or a path.

A text edit rebuilds from the crate that owns the file upward: a word of the window does not rebuild the
commands or the session. Who decides where a text is shown is §9.1.

---

## 19. The documents

| A statement of this kind | lives in |
|---|---|
| What is, and what is intended: a rule, a shape, the reason that still binds | a design document, in the present tense |
| A decision with the alternatives that lost | a decision record; an accepted record is amended, never rewritten |
| What a crate and a file hold | the head comment; the maps are generated from it |
| The rule a function implements | the function's doc comment, once; a module's head says what the file holds and cites the section |
| The steps of a running milestone | its plan; open work is a row of the backlog; a finished row is deleted |
| A change a person can see | one line of the release notes |
| How a thing came to be | git |

A design document never holds: a note headed "As built" or a dated amendment; the number of a row, a work package
or a round; what a generator produces (the crates, the commands and actions, the built rules, the keymap, the
command line's reference, the queries); a table of requirements covered, a scope by release, open questions; a
milestone's plan. When behaviour changes, the sentence that describes it is changed in the same commit.

A document that is read by section is split where a chapter is read alone, and a ceiling on the size of such a
file keeps it so, its number set from the sizes after the split as the source files' ceiling is (§13.3); the
requirement lists and the glossary are read by identifier and stay whole. Section numbers are stable, because
the code cites them; a document that is split keeps its file as the index of its chapters. A
comment in the code gives its reason in words and may cite a section; it names no row, round or brief.
`cargo xtask rules` fails on a note headed "As built", a dated amendment or a row's number in `docs/design`.

---

## 20. The gaps this structure closes

Seven gaps of structure are the reasons a second copy gets written: domain knowledge without a home (§3.4), an edge
nobody can read off (§3.1, §3.2 rules 6 and 7), a declaration that does not hand out what its users need (§3.5), a
text without a carrier below the session (§18), what a person is told decided where no test sees it and what
follows a commit found by the command's name (§9), a check of a change narrower than a release's (§13), and
documents that record how a thing came beside what it is (§19). What each gap cost is in
[ADR-0035](../adr/0035-homes-order-texts-shell.md), amendment 2. A concept built ahead of its use is not a gap of
structure but principle 14's, and the placing rules are the pilot's and the owner's ([06a](06a-drafting.md)).

**What is sound and stays.** The model as records in persistent maps and `entities!` as its table list; the link
table with its field-mask test; the strict reader, the canonical line order and the three-way merge of the format;
the store's file-system seam with its in-memory twin; the tracked context `Q`, the query families and the oracle;
the rule trait and a finding's identity by fingerprint; `commands!` and refusals as keys checked at build; integer
geometry everywhere and ticks in the layout, with `consts.rs` as its one table of numbers and one measure for the
router, the placer and the checks; one row set with typed cells rendered late for PDF, CSV and XLSX, and one rule
for pagination and drawing; one memoised family per layer; the action table with `enabled_with` as the one gate;
the window and the script through one `dispatch`; one token source and one dialog frame in the window; the injected
clocks of the store's tests; one `check`, one `gen`, generated maps.
