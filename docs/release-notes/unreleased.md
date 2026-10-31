<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Unreleased changes

- api-break: cli lib list --json (prints `{"$schema": "ew.lib-list-result/1", "libraries": [...]}` instead of a bare
  array; a Preview surface, WP-S1.22 D3 (d))
- api-break: cli lib search --json (prints `{"$schema": "ew.lib-search-result/1", "hits": [...]}` instead of a bare
  array; a Preview surface, WP-S1.22 D3 (d))
- api-add: cli elektrawatt --headless --visual --window <W>x<H> --ppp <n> (a visual run chooses its window in points
  and its scale; the default 1280x800 at 1.2 and every earlier frame and transcript are unchanged; `xtask blind start
  --senses visual` takes the same flags)
- api-add: cli ew fmt [--check] [--json] (lists or rewrites the files not in canonical form through the save
  protocol; result `ew.fmt-result/1`)
- api-add: cli ew recover [--replay | --discard] [--json] (summarises, replays or deletes the journal of the crashed
  session with the latest change, one at a time; result `ew.recover-result/1`)
- api-add: cli ew import csv <tables-dir> --out <dir> [--content <dir>] [--strict | --lenient] [--json] (creates a new
  project from the CSV tables; result `ew.import-csv-result/1`)
- api-add: cli ew query [<project-dir>] <name> <key> [--explain] [--json] (answers one of the twelve public queries as
  a reader; `--explain` adds the memo policy, the dependencies and the provenance; result `ew.query-result/1`)
- api-add: cli ew queries [--json] (lists the query registry with each query's output schema; result
  `ew.queries-result/1`, the text of `crates/ew-api/api/queries.json`)
- text-change: cli ew import csv, the window's import (a table refused for a stray quote, a stray carriage return or
  an unterminated quoted field names the column too: "table refused: line 2, column 3: stray quote"; FT-880)
- text-change: reports parts_summary, parts_list, part_data, enclosure_legend (a manufacturer whose copy is missing
  and whose reference keeps no key prints "manufacturer missing" instead of the item's UUID; FT-876)
- text-change: cli ew apply, fmt, recover, import parts and the human-only subcommands (a project that opens read-only
  is refused in words: "error: <dir>: the project opens read-only: <reason>; nothing was written", the held lock as
  "another session holds the project (<user> on <host>, since <time>)"; no session or process ID; FT-877)
- text-change: cli ew project checks set (the off-terminal refusal loses its run of embedded spaces; FT-878)
- api-add: window script state and describe text, the Interruption point tool's ghost (`canvas.ghost.connects`, the
  pin a click connects the point to or `null`; the describe line ends ` connects "<pin>"`; FIXPT41)
- text-change: window Interruption point tool (a point whose connection point lies on a pin connects to it in one undo
  step: the command bar adds "connects to -102F2:14" and the toast says "Placed the interruption point 0V at <cell>
  and connected it to -102F2:14. Alt+Click or Ctrl+Enter places it without connecting"; a drop on another
  interruption point, and an external point's on a pin, are refused in words; FIXPT41)
