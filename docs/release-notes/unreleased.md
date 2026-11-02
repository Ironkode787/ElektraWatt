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
- api-add: cli ew fmt --repair [--json] (discards the lines that are not JSON or not UTF-8, EW-CORE-005, in one save,
  after packing the project's files as found, a file edited outside EW since the last save included and counted ("as
  found: N files edited outside EW", `backup.edited`), as `<project folder>.before-repair-g<generation>.ewz` beside
  the project folder; refused before anything is written for an `.ewz`, a crashed session's journal, any other finding
  that forces read-only, files that cannot be packed (no manifest, an incomplete save, a file that cannot be read or
  changed since the open) and a backup name holding another file; "Nothing to repair" without such a line; result
  `ew.fmt-repair-result/1`; WP-S1.17)
- api-add: cli ew recover [--replay | --discard] [--json] (summarises, replays or deletes the journal of the crashed
  session with the latest change, one at a time; result `ew.recover-result/1`)
- api-add: cli ew import csv <tables-dir> --out <dir> [--content <dir>] [--strict | --lenient] [--json] (creates a new
  project from the CSV tables; result `ew.import-csv-result/1`)
- api-add: cli ew query [<project-dir>] <name> <key> [--explain] [--json] (answers one of the twelve public queries as
  a reader; `--explain` adds the memo policy, the dependencies and the provenance; result `ew.query-result/1`)
- api-add: cli ew queries [--json] (lists the query registry with each query's output schema; result
  `ew.queries-result/1`, the text of `crates/ew-api/api/queries.json`)
- api-add: cli ew print [<project-dir>] [--pages <folios> | --sections <numbers>] [--paper <size>] [--fit] [--mono]
  [--printer <name>] [--via lp|viewer] [--out <file.pdf>] [--json] (prints the print set through CUPS `lp` or the
  default PDF viewer, or writes it; `--fit` stamps a scaled page "scaled <n> %"; result `ew.print-result/1`), and
  `--sections <numbers>` on ew export pdf|svg|png
- api-add: cli ew export cables|bom|wires|terminals|report --format xlsx --out <file.xlsx> (writes the rows as a
  workbook: a hidden `id` column, a bold frozen header with a filter, numbers as numbers, text never a formula, one
  sheet per terminal strip and a hidden `_ew` sheet naming the table, the revision and the columns; WP-S3.08 part 1)
- api-add: cli ew sql --format xlsx --out <file.xlsx> (writes the result set as a workbook; FT-218)
- api-add: cli ew export cables|bom|wires|terminals|report and ew sql --preset ew|excel-nl|excel-en|rfc4180
  [--bom on|off] [--line-end crlf|lf] (the CSV dialect; `ew`, the golden dialect, stays the default; the Excel presets
  put an apostrophe before a text that starts with `=`, `+`, `-`, `@`, a tab or a carriage return)
- api-add: cli ew export table <navigator> [<project-dir>] [--strip <tag>] [--format csv|xlsx] [--preset …]
  [--out <file>] [--json] (writes a navigator's table unfiltered for a round trip; an XLSX also holds a hidden
  `_ew_base` sheet with the editable cells as exported; result `ew.export-table-result/1`; WP-S3.08 part 2)
- api-add: cli ew import table <file> [<project-dir>] [--table <key>] [--preset excel-nl|excel-en|rfc4180]
  [--take-file] [--apply] [--json] (previews the changed cells of a round-trip table with EW-IMP-301 to -305 and with
  `--apply` applies them as one `fields.set_many`, one undo step; result `ew.import-table-result/1`; WP-S3.08 part 2)
- text-change: cli ew export cables|bom|wires|terminals|report and ew sql --format xlsx (a number of more than 15
  significant digits is written as text, so it stays exact; the workbook writer's refusals end with what to do;
  WP-S3.08 part 2)
- api-add: cli ew export cables|bom|wires|terminals|report --json (the results `ew.export-*-result/1` gain `format`,
  `preset` and `rev`)
- api-add: cli ew export ewz [<project-dir>] --out <file.ewz> [--json] and ew extract <file.ewz> [--into <dir>]
  [--json] (pack the saved generation into a deterministic `.ewz` and unpack one into a new folder, all or nothing;
  results `ew.export-ewz-result/1` and `ew.extract-result/1`); check, sql, sql dump, the row and page exports, export
  table, print, render, verify-derived, query and fmt --check open an `.ewz` read-only, every other subcommand
  (import table included) refuses it with "an .ewz opens read-only; extract it first" (WP-S1.17 part 2)
- text-change: cli ew export and ew sql `--format xlsx` (no longer refused as not built; without `--out` it is refused
  with "`--format xlsx` writes a file: give `--out <file.xlsx>`"; `--preset`, `--bom` or `--line-end` with it with
  "`--preset` applies to CSV only; drop it, or give `--format csv`"; `--format pdf` on a row export names CSV or XLSX)
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
- text-change: window Insert ▸ Device… (the picker is titled "Insert device"; Draw ▸ Symbol… keeps "Insert symbol";
  FT-890)
- text-change: window Draw ▸ Symbol… and Black box tool refusals ("This part is placed as a black box: use the Black
  box tool (B)" and "This is not a black box: place it with Draw ▸ Symbol… (I)", the keys from the keymap and left out
  when unbound; FT-894, FT-896)
- text-change: window Black box tool command bar ("Click the box's first corner, then its opposite corner; Escape
  ends"; FT-896)
- text-change: window wire and drawing tools on a page that takes no symbols (the refusal names the page kind: "A
  section divider takes no symbols or wires: draw on a schematic page", likewise for a title page, a table of contents,
  a report page and a panel layout page; FT-892)
- text-change: window Insert ▸ Terminal strip (the toast says the strip takes its tag, such as -X1033, when its first
  terminal is placed, and names Draw ▸ Symbol…; off a schematic page the example is left out; FT-893)
- text-change: window click-to-cycle HUD ("3 objects overlap here · click again to select the next"; FT-910)
- text-change: window Interruption point tool command bar (the role in words: "source: feeds its targets", "target:
  fed by its source", "undirected: pairs either way", and "star source: feeds every point of the name"; FT-899)
- text-change: window page link on the canvas (the pointer turns to a hand and the tooltip says "Go to <page>";
  FT-902)
- text-change: checks EW-CORE-052 (the message counts the star sources or the sources and says what to change; the
  hint: "Keep one source per name: set the other sources to target, or flag exactly one point as the star source.";
  FT-901)
- text-change: checks EW-REL-001, cli ew apply set-field on a revision's author (the menu path is "Check ▸ Revision ▸
  Set revision author…"; FT-901)
- text-change: window Properties of a generated page's report block (the heading is the page's folio and title, such as
  "901 Partlist 1"; P-1)
- text-change: window Export PDF of the same file again (the toast says "Exported out.pdf again: 36 pages, … KiB";
  P-2)
- text-change: cli the read-only refusal of a project in another format (prints the reason it carries: "it was
  written by a newer major format" or "it was written by a development build with an unreleased format", for "it was
  written by a newer format"; a lock record's text prints the Unicode bidirectional controls escaped as `\u{202e}`;
  FT-943)
- text-change: window read-only reason of a project in another format ("a file format this version does not write",
  for "a newer file format"; FT-943)
- text-change: title block of a page drawn to scale (a Dutch title prints "Schaal", such as "Kastindeling · Schaal
  1:10", every other language "Scale"; a stored scale of 0 prints the plain title and no scale; FT-881)
