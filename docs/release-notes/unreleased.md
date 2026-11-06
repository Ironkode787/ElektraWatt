<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Unreleased changes

Line kinds: `api-add`, `api-break`, `behaviour-change`, `dependency`, `format-variant`, `text-change`.

- api-break: cli lib list --json (prints `{"$schema": "ew.lib-list-result/1", "libraries": [...]}` instead of a bare
  array; a Preview surface, WP-S1.22 D3 (d))
- api-break: cli lib search --json (prints `{"$schema": "ew.lib-search-result/1", "hits": [...]}` instead of a bare
  array; a Preview surface, WP-S1.22 D3 (d))
- api-break: cli fmt exit 0 (its meaning gains `--repair`'s outcomes: "repaired, or nothing to repair"; a Preview
  surface, WP-S1.17 ID-1185 ruling 7)
- api-break: cli fmt exit 2 (its meaning gains `--repair`'s outcomes: "a repair refused before writing, its save
  failed, or its re-open found a finding (the new generation and the backup stay)"; a Preview surface, WP-S1.17
  ID-1185 ruling 7)
- api-break: cli merge-driver exit 1 (its meaning gains a merged file that would pass the reader's limits: `%A` is
  left as ours and theirs stays in git's index; WP-S1.17 ID-1273 ruling 6)
- api-break: cli merge-driver exit 129 (its meaning gains a usage error, with clap's usage text, as ADR-0012
  amendment 2 has it; WP-S1.17 ID-1273 ruling 9)
- api-break: cli merge-driver exit 2 (removed: a usage error of `merge-driver` exits 129, every other command keeps 2;
  WP-S1.17 ID-1273 ruling 9)
- api-add: cli elektrawatt --headless --visual --window <W>x<H> --ppp <n> (a visual run chooses its window in points
  and its scale; the default 1280x800 at 1.2 and every earlier frame and transcript are unchanged; `xtask blind start
  --senses visual` takes the same flags)
- api-add: format plc_profile (the library kind of PLC address profiles: `lib/plc-profiles/<id>.json` in a project,
  `plc-profiles/` in a library; the built-in TIA and IEC profiles ship as `content/plc-profiles/*.json`, their pins
  unchanged; a profile file in a library is chosen by `plc.set_profile` and the project holds its copy; WP-S7.03 D7,
  D8)
- api-add: content ew-sample-parts (the sample project's 23 catalogue parts and their seven manufacturers as a system
  library, CC0-1.0, with function templates, CP patterns, the PLC and bus sections and each section's confidence
  `test` or `unverified`; WP-S7.03 D3, D4)
- api-add: content profiles/colour/company.dbu-wh-24v.json, profiles/size/company.0.75-1.5.json (the sample project's
  colour and size profiles as shipped data, byte for byte its values; WP-S7.03 D5, D6)
- api-add: window company-standard wizard, step 2, "Pin numbers and markers" ("Readable (new projects)", the default,
  writes the new project's readable text styles; "As in the sample project" writes none; WP-S7.03 D11)
- api-add: cli ew apply (a `plc.set_profile` line naming a library's address profile embeds it first; WP-S7.03 D8)
- api-add: cli ew fmt [--check] [--json] (lists or rewrites the files not in canonical form through the save
  protocol; result `ew.fmt-result/1`)
- api-add: cli ew fmt --repair [--json] (discards the lines that are not JSON or not UTF-8, EW-CORE-005, in one save,
  after packing the project's files as found, a file edited outside EW since the last save included and counted ("as
  found: N files edited outside EW", `backup.edited`), as `<project folder>.before-repair-g<generation>.ewz` beside
  the project folder; refused before anything is written for an `.ewz`, a crashed session's journal, any other finding
  that forces read-only, files that cannot be packed (no manifest, an incomplete save, a file that cannot be read or
  changed since the open) and a backup name holding another file; "Nothing to repair" without such a line; result
  `ew.fmt-repair-result/1`, printed on every way out once the backup exists, with `save_error` after a failed save and
  `reopen_finding` after a re-open that still finds something wrong; WP-S1.17)
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
- api-add: cli ew merge-driver <O> <A> <B> [L] <P> (Git's merge driver for project files: merges one file three-way,
  record by record, and writes conflict records instead of markers; WP-S1.17 part 4)
- api-add: cli ew merge resolve [<project-dir>] --take ours|theirs [--kind <kind>] [--field <path>] (resolves the
  conflict records a merge left, those `--kind` and `--field` select, in one transaction of `merge.resolve`, and saves;
  WP-S1.17 part 5)
- api-add: command merge.resolve (takes ours or theirs for each conflict record named, in one undo step; WP-S1.17 D19)
- api-add: cli ew diff <from> <to> [--exit-code] [--json] and ew diff --textconv <file> (the records that differ
  between a directory, an `.ewz`, an `.ewarchive`, `git:<rev>` or `rev:<label>`, labelled with each side's tags;
  result `ew.diff-result/1`; git's textconv; WP-S1.17 D26, D27)
- api-add: cli ew git setup [--global] [--hooks] [--yes] (the merge driver and textconv in git's configuration after
  consent, the managed `.gitattributes` and `.gitignore` blocks, the `post-merge` hook; WP-S1.17 D28)
- api-add: cli ew check --integrity [--after-merge] (the store's integrity findings and the rules EW-CORE-001 to -019,
  exit 1 on any error; FT-191, WP-S1.17 D29)
- api-add: command external.apply (exposure `Internal`: only the session's external-change apply submits it, as one
  undo step "External change"; WP-S1.17 D22)
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
- api-add: window Output ▸ Print… (Ctrl+P, `file.print`; the print pre-dialog: Range, Paper, Scale to fit and
  Monochrome beside a preview of the print set, which is built as a job; Print… hands the set to the system's printer,
  on Linux through the desktop portal, then `lp`, and Open in PDF viewer to the viewer; WP-S3.07 part 3)
- text-change: window Interruption point tool (an external point connected on a pin says "Placed the external point
  <name> at <cell> and connected it to <pin>. Alt+Click or <key> places it without connecting", where it said
  "interruption point"; FT-1067)
- text-change: window Place tool (the next terminal designation of a strip is the lowest number free in it, again after
  an Undo or a Redo, so a strip keeps no gap; FT-1083)
- api-add: command device.create (an optional `frozen_tag` sets a new device's tag in the creating commit, one undo step;
  the window's New terminal strip form uses it; FT-1090)
- text-change: sheets, PDFs and prints (a part of the starter library no longer prints "Generic" as its maker, so a
  black box of a generic part prints its tag and its part number; FT-1094)
- text-change: window part chooser and Parts row (a library part's maker shows by the record's name, not its key, and
  the parts keep one order whether the project holds them or not; FT-1087)
- text-change: window canvas HUD (on a page that takes no symbols it gives the page type's reason while no tool is
  armed; FT-1089)
- text-change: command plc.set_profile (a name not at hand: "`<text>` is not an address profile of this build or of
  the project; the profiles at hand are <list>"; WP-S7.03 T1)
- text-change: cli ew lib list and command library.embed (an address profile that does not render: "the address
  profile <id@version> cannot be read: <reason>"; WP-S7.03 T2)
- text-change: window part chooser, place-tool picker, cli ew lib search (the sample project's catalogue parts are
  listed from the system library "Sample project parts"; WP-S7.03 D3)
- dependency: async-executor 1.14.0 → 1.14.1 (a use-after-free fixed upstream; 1.14.0 yanked)
- text-change: drawings (a technical-data text that would overlap a black box's pin text drops below it, as a
  cross-reference text does; a black box's caption stack that a stored wire crosses moves left in grid steps, within
  the box's outline, or stays where no place is free; FT-701, FT-772)
- text-change: window canvas (the EW-POT-001 conflict overlay draws the wires at the connection point that keys a
  potential without a definition, and a ring around such a point without wires; a pane beyond the graphics device's
  limits is drawn on the CPU for that frame and the GPU canvas stays; FT-696, FT-363)
- dependency: the AppImage's runtime, the AppImage project's static type-2 runtime 20251108 (new; a separate program
  at the start of the Linux AppImage, not linked into ElektraWatt, ID-1275; MIT, and it links statically musl libc
  (MIT), libfuse 3.15.0 (LGPL-2.1-only), squashfuse 0.5.2 (BSD-2-Clause), libzstd (BSD-3-Clause), zlib (Zlib) and
  mimalloc (MIT); the notices are in NOTICE, the source at https://github.com/AppImage/type2-runtime, tag 20251108,
  commit dd6cebedcbddde9c82f89b011e8e1d40b6e43868)
- behaviour-change: window canvas wheel (the sideways wheel, and Shift with the wheel, pan horizontally; FT-895)
- behaviour-change: window Messages detail card (as tall as its content; the list keeps at least two rows; FT-915)
- behaviour-change: window Messages (Show messages and Next message bring a hidden Messages tab to the front; FT-916)
- behaviour-change: window and cli every save that writes an unreleased format (the window shows the toast "Saved in a
  format that is not released yet. Released versions of ElektraWatt open this project read-only." and `ew` prints
  "warning: saved in a format that is not released yet; released versions of ElektraWatt open this project read-only"
  on standard error, the exit code unchanged; FT-1154, WP-S6.08 D3)
- behaviour-change: cli ew merge-driver (a project whose manifest on any side names a format newer than this reader's
  is not merged: `%A` stays ours, the driver prints "merge-driver: <path>: written by a newer ElektraWatt (format <f>;
  this one reads <r>); left as ours: merge it with that version" and exits 1; FT-1235, WP-S6.08 D4)
- text-change: window About (the support line reads "Support: only the latest 0.x release is supported, until the
  next minor release."; WP-S6.08 D7)
- api-add: window script transcript (a done `help.documentation` step carries `doc`, the file the request opens below
  the installed user guide, or null without one; FT-1267, WP-S6.08 D6)
- api-break: cli recover exit 1 (its meaning names a declined upgrade: "the replay refused, or its upgrade or the
  discard not confirmed"; a Preview surface, FT-1394, ID-1394)
- behaviour-change: cli ew recover --replay (on a project of an older format the upgrade's consent is asked before
  the replay, so a declined run leaves the crashed session's journal as it was; FT-1394)
- text-change: cli ew upgrade --check (on an `.ewz` of an older format: "the project is in the older format <f>: `ew
  extract` makes a project folder of the archive, and `ew upgrade` converts that folder to format <g> after a backup";
  FT-1214)
- text-change: commands refusals naming a library copy (a manufacturer copy reads "manufacturer <name> (<key>) at
  version <v>", a PLC profile "PLC profile <label>", a symbol and a macro "symbol <name>", "macro <name>"; a part and
  a document without a name as before; FT-1524)
- api-add: command check.waive subject (optional: the finding's subject as printed, which the window passes from the
  Messages row; a finding waived already is refused as "a waiver of <rule> for <subject> covers this finding
  already", the earlier text kept without it; FT-1509)
