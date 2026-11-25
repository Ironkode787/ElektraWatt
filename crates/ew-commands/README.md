<!-- SPDX-FileCopyrightText: 2026 The ElektraWatt contributors -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ew-commands

The editing layer of ElektraWatt (ADR-0008): every change of a project is a `Command` that runs against a
`TxBuilder`, is completed by the normalisers, validated by `ew-model` and committed as one `Transaction` of
`Op`s. The crate holds the 37 v0 commands, the `CommandRegistry`, the `Engine` pipeline with previews, the
project-wide `History` (undo and redo) and the change feed. `ew_api::Session` wraps the engine.

## Parameter schemas

The JSON Schemas of the command parameters are made by `CommandRegistry::schema_files`; `cargo xtask gen` writes
them into the reference (`target/reference/commands/`, not committed) for the user guide.

Parameter schemas reject unknown tags of open enums on purpose: the schemas are the client contract, while
records stay open (unknown tags round-trip through the model).

Value types of the foundation crates appear in the schemas as `Cmd*` proxies (`CmdUuid`, `CmdPt2`, `CmdIdent`,
`CmdDec3`, `CmdMlText`, ...), named apart from the private proxies of `ew-schema` so that one `$defs` name
never has two definitions.

## Coordinates and row access

Commands that place or move representations or draw and route connections refuse positions off the page's
connection grid (`Page.grid`) and routes whose ends do not meet their connection points (doc 06 §5.7); front-ends
snap before they submit. Coordinates stay within ±(2⁵³ − 1) nm (ADR-0005): arithmetic that would leave that range
is refused, never saturated.

Previews, history entries and diffs expose summaries (change kind, record key, changed member paths) only. Whole
rows (`rows`, `Preview::ops`, `HistoryEntry::transaction`) are available with the test-only feature `testkit`, so
the crates above `ew-commands` keep reading records through tracked queries (ARCH §3 rule 1).

## Known gaps

The engine's prevent hook defaults to `NoPrevent`: the session plugs in `ew-checks`' hook (doc 15 §3.4), so the
engine alone rejects no transaction for a prevent rule. Numbering applies only page-scoped schemes (doc 03 §8.2), and
previews run on the window's thread: there is no job pool for them.
