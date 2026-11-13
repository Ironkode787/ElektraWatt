<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Unreleased changes

Line kinds: `api-add`, `api-break`, `behaviour-change`, `dependency`, `format-variant`, `text-change`.

- api-add: commands `plc.freeze {config_project}`, `plc.readdress {config_project | modules}` and
  `plc.convert_manual_to_auto {config_project | modules}`, all Bulk; the re-address preview lists every changed
  address as `channel 3 of -201U5: Q10.3 → Q2.3` (WP-S1.24, FR-PLC-014, FR-PLC-015)
- api-add: the built-in tag-based address profile `rockwell-logix@2027.1`: `Local:3:O.Data.1` from the station and
  the slot; a tag-based profile reads no typed address or start (WP-S1.24, FR-PLC-009)
- api-add: `fields.set` takes a channel's `data_type`, `signal_range`, `mode` and `safety`, a module's
  `plc_module.safety`, a configuration project's `symbols` (the automatic-symbol template, refused when it does not
  expand) and a port's `bus_port.mrp`; `project.settings.set` takes `process_tag_scheme` (WP-S1.24)
- api-add: report kinds `plc_card_list`, `network_address_list`, `bus_overview`, `bus_cable_list`,
  `iolink_port_list` and `safety_io_list`, and the SQL view `api_v1_plc_channel` (WP-S1.24)
- api-add: the 1.0 rules EW-PLC-004, -005, -009, -010, -011, -013, -014, EW-SAF-001 … -003, EW-PRC-001,
  EW-BUS-002, -003, -005, -006, -007, -009, -010 and EW-IOL-001, -002, in a scheme with the 1.0 content; the shipped
  schemes run the MVP content as before (WP-S1.24)
- behaviour-change: a channel bound to its card's pin takes the field device its signal reaches as its target, with
  the passed terminals (the I/O list's target and target pin), and that device's `PlcAddress` slot prints the
  channel's address; a manual target prints there too (WP-S1.24, FR-PLC-018, FT-700)
- behaviour-change: a configuration project with a symbol template gives a channel without a manual symbol the
  template's symbol, cleaned into an IEC identifier; EW-PLC-007 compares symbols so cleaned, and EW-PLC-001 and
  EW-PLC-007 refuse the edit that creates an overlap or a duplicate (WP-S1.24, FR-PLC-017, FT-726)
- behaviour-change: an IO-Link master's ports take their devices' process-data windows, and A1 allocates their sum
  (WP-S1.24, FR-BUS-012)
- behaviour-change: `library.update` with a part version that adds a PLC section or changes its bus ports gives the
  devices that use it their PLC aspect, channels and ports (FT-830); `rep.join_device` takes a bus-port symbol onto a
  device whose bus part has a free port (FT-841)
- format-variant: channel ChannelKind multi_function
- format-variant: part ChannelKind multi_function
- format-variant: function NetAddress {profinet}
- format-variant: function NetAddress {ethercat}
- format-variant: function NetAddress {io_link_port}
- format-variant: report_block ReportKind plc_card_list
- format-variant: report_block ReportKind network_address_list
- format-variant: report_block ReportKind bus_overview
- format-variant: report_block ReportKind bus_cable_list
- format-variant: report_block ReportKind iolink_port_list
- format-variant: report_block ReportKind safety_io_list
- api-add: a bus part port's `signals` (signal → pin); a bus cable's core lands on the pin of its signal at each
  port's multi-line representation, and the cable diagram prints those pins as the core's ends (FT-1724, FR-BUS-023)
- api-add: `project.drawing.set` takes `plc_address_at_terminals`: a terminal a traced channel's signal passes prints
  the channel's address in its `PlcAddress` slot (FT-1725, FR-PLC-018)
- behaviour-change: EW-BUS-002, -005 and -009 mark a finding that rests on a part's `bus` section of unverified
  confidence as based on non-verified part data (FT-323)
- dependency: `ew-derive` uses `regex` 1.13.1 (MIT OR Apache-2.0, already in the workspace) for the process-tag
  scheme (WP-S1.24)
- behaviour-change: window Cable… proposes the type `<n>G<cross-section>` when one of the selected wires is a protective
  conductor, so the PE wire has its green-yellow core; retyping the type proposes the cores of the wires not yet in
  a cable afresh, so a neutral no longer keeps the brown core of an earlier type; the form's wire column is as wide
  as its longest wire (after 0.2.0)
- api-add: window Draw ▸ Connect ▸ Route page and Route selection preview the page router's plan and commit it as one
  `connection.route_many` (WP-S2.19 D13)
- api-add: rep.move takes `routes` (applied after the follow, as connection.route_many applies them) and `connect`
  (autoconnect hints `{rep, cp: {function, idx}, to}`); a move in the window re-solves a followed route that breaks a
  hard rule, and connects its proposals unless Alt is held (`tool.connect_on_move`, C, turns that off) (WP-S2.19 D14,
  D15); a `connect` hint onto a point of a connection is refused, since a move never splits a wire (ID-1509)
- api-add: command `connection.route_many {routes: [{connection, route}]}`, several `connection.route` entries in one
  transaction (one undo step), 1 to 10,000 entries, an entry's error prefixed `/routes/<i>` (WP-S2.19 D12)
- api-add: `ew route [<project-dir>] [--page <folio>] [--selection <ids>] [--dry-run] [--json]` routes the pages'
  unrouted wires (or the selection's) with the page router and saves them in one step per page, result
  `ew.route-result/1`, exit 1 while a wire stays unrouted; `ew layout [<project-dir>] [--page <folio>] [--json]`
  prints the layout measures of the drawn pages, result `ew.layout-result/1` (WP-S2.19 D13)
- behaviour-change: a connection's `route` holds every vertex from end `a`'s point to end `b`'s, both included, and
  is drawn as stored; an empty route means the connection is unrouted. The QElectroTech importer stores each
  conductor's route that way (WP-S2.19 D2, D17)
- behaviour-change: an unrouted connection is drawn on the canvas and in every export as a dashed straight line
  between its ends, never as the provisional L, and is no wire for the connection split, EW-TERM-006 and autoconnect
  (WP-S2.19 D3)
- behaviour-change: `device.place`'s autoconnect hints store the straight route between their two ends; a hint whose
  ends are not aligned makes an unrouted wire (WP-S2.19 D12)
- behaviour-change: an interruption point's arrow always points right: `ew.gen.ip` 0.2.0, the source form, has its
  wire on the tail, and the new `ew.gen.ip-target` 0.1.0, the target form, has it on the tip (each with its `@qet10`
  twin). `ip.place`, `connection.draw_to_ip` and `ip.continue` refuse a turned form or the other direction's,
  `rep.transform` refuses to turn one, `macro.insert` refuses a turned macro holding one, and a direction change
  swaps the form (WP-S2.19 D16, ID-1509)
- behaviour-change: `ew.gen.strip-segment` 0.2.0 prints the strip's tag beside the strip, left of its first terminal,
  not above the terminal row (ID-1502)
- behaviour-change: `ew.iec.motor-3ph` 0.2.0, the three-phase motor, has leads one grid step longer, its connection
  points U1, V1 and W1 one grid step higher, so that its wires are routed clear of its texts (ID-1509)
- behaviour-change: `ew.iec.heater` 0.1.1, the heater, prints its technical data one grid step lower (ID-1509)
- behaviour-change: a target interruption point's cross-reference (`ew.gen.ip-target` and its `@qet10` twin) is
  left-aligned at the arrow's tail, below the arrow, so that it runs into the sheet from the left edge (ID-1509)
- api-add: the tables importer: `representations.csv` takes an optional `orient` column (`A` to `H`) and refuses an
  interruption-point form that is not its function's direction; a `route` that names end `b` as its one corner is a
  straight route (ID-1502, ID-1503)
- api-add: check rules EW-CORE-067 (wire through a connection point), -068 (wire along another wire), -069 (wire
  through a symbol), -070 (wire through a text), -072 (four-way junction) and -073 (unrouted wire, an error in
  release mode) (WP-S2.19 §9)
- behaviour-change: an imported or drawn connection without a route is an EW-CORE-073 finding; `ew check` exits 1 on
  it until Route page or `ew route` routes it (WP-S2.19 D3, D17)

- api-add: page.new takes `grid` (mm4 or qet_10px), the window's New page its Connection grid, the current page's by
  default; a page's Connection grid is a choice in Properties; picker.new_page places the Insert symbol picker's symbol
  on another grid on a new page with its grid (ID-1477)
- behaviour-change: fields.set of a page's grid is refused while the page holds any item (ID-1477)
- text-change: the Draw tab's button for the symbol editor reads "Symbols" (tooltip and menu "Symbol editor…") (ID-1477)
- text-change: a connection grid is named as the status bar names it ("4 mm", "QET 10 px") in the refusal of a symbol
  on another grid and in the page navigator's and page tab's badges, never "2.65 mm" or "QET grid" (ID-1477)
- api-add: symbol.new_page places the symbol editor's published, unchanged draft on a new page with its grid; the
  editor's refusal of another grid offers it, else Test on a new page (ID-1477)
- behaviour-change: symbol validation also refuses unknown members of a deprecated status and its successor; a test
  placement's item IDs join their v5 name's parts with U+001F (ID-1476)
- api-add: window Symbol editor (Draw ▸ Place ▸ Symbol editor…): drawing with fills and text styles, connection points on
  the native grid, function data with drag or list pin mapping, text slots, anchors, link points, live A–H previews,
  publication to a library whose folder the window remembers and whose symbols appear under Draw ▸ Place ▸ Symbol…
  on the matching grid, and test placement on a new page; F1 opens the editor's reference
- api-add: symbol.* actions for scripts and AI tools, the window's own (symbol.new to symbol.scratch, symbol.map_cp
  for pin mapping, symbol.role for a connection point's potential role); symbol.fork and symbol.function take explicit library/item sources
- api-add: window File ▸ Save project (file.save_project): while the symbol editor shows, Ctrl+S and File ▸ Save
  save the symbol, and Save project saves the project
- api-add: symbol publication gives each draft its own item identity (UUIDv7s, the optional `identity` of
  `ew.symbol/1`), refuses a key the library holds for another symbol, keeps an unchanged owned function definition's
  version and assigns an existing function as compatible (`^version`); a draft saved under another key than the one
  the library holds for its items publishes a new symbol, which the window announces (ID-1474); validation refuses
  unknown members of every object it publishes (`symbol-unknown-member`); no project or library format 0.1 change
  and no migration
- api-add: cli ew symbol template, schema, validate, import and publish (portable data-only authoring for
  automation and AI tools, using the editor's validator; import's key defaults to the file's name; publish prints
  `result`, added, updated or unchanged, and the draft's `identity`)
- api-add: symbol imports from QElectroTech .elmt, EPLAN EDZ (also named .zip or .7z)/PXF and the supported SLK
  1.2/FE53 drawing-table subset (SLK requires its sibling .sdb folder but no EPLAN installation; unsupported features produce review
  findings or refusal; binary macros and full EPLAN electrical semantics are not supported)
- dependency: EPLAN 7z archive decoding adds pinned sevenz-rust2 0.24.2 (no default features) and lzma-rust2
  0.23.0, both Apache-2.0 and audited, under ElektraWatt's own 7z header bounds (ID-1460)

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
- behaviour-change: command terminal.insert (a strip placed turned 180° grows left and pushes the items before it on
  its rail, leftwards, cascading; past the rail's start the positions are written and EW-LAY-010 reports
  `bracket_outside`; FT-1495)
- behaviour-change: command part.assign (a placed strip whose terminal the change widens pushes the items after it,
  as a growing strip does, in every mode; FT-1497)
- text-change: cli ew apply, fmt, recover, import parts and the human-only subcommands (a project read-only for a
  damaged library copy that no device names is refused naming the copy and the record that names it: "the library
  copy of a form named by page 1 is damaged; update the copy from its library first", instead of "it has integrity
  findings"; FT-1500)
- behaviour-change: cli ew check, ew check --integrity and the writers' open (an image the project keeps but no
  drawing shows, over the pixel limit, is named in the window's words as a `notice:` line on stderr, outside the
  counts; no check rule reports it; FT-1503)
- api-add: cli ew check --format json notices (optional: the notices outside the findings and their counts, in the
  window's words, absent when there is none; FT-1503)
- behaviour-change: command item.place (an image placed by the hash of an asset the project holds is refused when the
  asset is over the pixel limit, in the words its data is refused in; FT-1523)
- text-change: commands library.embed and paste refusals naming a held library item ("the project holds <item>
  already; take another version of it with update from library", "the project holds another version of <item>;
  update from library first", "the project's version of <item> differs from the pasted one in a major way (<why>);
  update instead", the item named as the copy refusals name it, "manufacturer Eaton (EATON)", a part by its number,
  instead of its schema kind; FT-1515)
- text-change: commands import.apply and external.apply refusals ("an item of an unknown kind (<kind>)", "a
  normaliser removed an item the import brought: <item>", instead of "a record"; FT-1526)
- behaviour-change: window and cli opening a project (a damaged library copy whose referrers also break another rule,
  such as a function definition's copy dropped while fluid lines end at its functions, opens the project read-only
  with the damaged copy's banner instead of refusing it; any other violation still refuses the open; ID-1484)
- behaviour-change: cli ew merge-driver (a record file one of whose sides holds a record whose `$v` is above this
  reader's version of its kind is not merged: `%A` stays ours, the driver prints "merge-driver: <path>: holds records
  this ElektraWatt cannot read (this one reads format <r>); left as ours: merge it with a newer version" and exits 1;
  a record of an unknown kind or with unknown members still merges, every member kept; FT-1490, WP-S6.08 D20)
- api-break: cli merge-driver exit 1 (its meaning names `%A` left as ours for a side in a newer format and for a side
  holding records this version cannot read; a Preview surface, FT-1516, WP-S6.08 D21)
- behaviour-change: window and cli every other write of a project in an unreleased format (in the window Save As, a
  new project, the wizard's basic project and a release; in `ew` the projects `ew import qet --out` and `ew import csv`
  write, `ew fmt`, `ew fmt --repair` and `ew revision release`) shows the toast and prints the warning of a save;
  FT-1489, WP-S6.08 D19)
- behaviour-change: command page.renumber (a new folio outside its section's folios is refused: "folio <n> lies
  outside section <s>'s folios <first> to <last>; choose another `start` or `step`"; FT-1511, WP-S6.08 D27)
- behaviour-change: command fields.set (a section's `divider` or `folio_range` that puts the divider page onto a folio
  a page or a page-oriented report block of the section holds is refused: "folio <n> is held by a page; choose another
  folio for section <s>'s divider", or "… held by a report block; …"; FT-1510, WP-S6.08 D27)
- api-add: cli ew update-copies [project-dir] [--project <dir>] [--company <dir>]... (the project's damaged library
  copies are read from the libraries of `ew lib`, checked, and written over their damaged files in one save, and the
  project opens again; each copy written is named with the library's version written, a copy no library holds stays
  damaged and is named; exit 0 when every damaged copy was updated
  and the project opens for editing, 1 when refused or a copy stays damaged, 2 on usage, open and save errors; FT-1499)
- api-add: window the damaged-copy banner's Update from library (`library.update_damaged`: every damaged library copy
  an installed library holds is written from it and the project opens again, writable when the integrity check
  passes, the toast naming each copy with the version written; a banner whose copy no library holds says so and
  offers nothing; FT-1499)
- format-variant: record medium.schema.json
- format-variant: record medium_def.schema.json
- format-variant: report_block ReportKind tube_list
- format-variant: report_block ReportKind tube_cut_list
- format-variant: report_block ReportKind tube_labels
- format-variant: report_block ReportKind valve_terminal_config
- format-variant: report_block ReportKind medium_overview
- api-add: fluid 1.0 records (WP-S1.25): `FnDef.valve_model` (switching positions, flow paths, actuators), the 1.0
  members of a part's fluid section (ports, positions, coil, galleries, tube, fitting, transform), a fluid line's
  `tube`, `length`, `fitting_a` and `fitting_b`, a junction's `fitting` (a fluid tee), the record kinds `medium` and
  `medium_def`, and `ProjectSettings.fluid` (default tube, line-tag scheme `page_counter`); additive within format 0.1
- api-add: commands `fluid.medium.edit`, `fluid.medium.define`, `fluid.tube.set` and `fluid.length.set`, and the
  `fluid` member of `project.settings.set` (WP-S1.25)
- api-add: derived manifold galleries of a valve terminal, media propagation over the valves' flow paths, effective
  tube values with their source, physical tubes across fluid interruption points; `ew query medium_zones` and
  `ew query tube_props` (WP-S1.25)
- api-add: report kinds `tube_list`, `tube_cut_list`, `tube_labels`, `valve_terminal_config` and `medium_overview`
  (`ew export report --kind <kind>`), the SQL views `api_v1_fluid_line`, `api_v1_valve_position` and `api_v1_medium`,
  and fluid BOM lines for tube, fitting, tee and closure parts (WP-S1.25)
- api-add: check rules EW-FL-005, -006, -007, -008, -009, -010, -012 and -014 in the pack's 1.0 content (WP-S1.25)
- api-add: the ISO 1219 pneumatic library: 33 symbols with their function definitions (directional valves, cylinders
  and drives, vacuum, air preparation, flow and logic valves, fittings, the air-supply point and the valve-terminal
  supply plate); the starter parts EW-GEN-VTM-1, EW-GEN-VLV-3 and EW-GEN-VTS-1, a generic valve terminal (FT-1326)
- behaviour-change: a new valve part whose valve type differs replaces the valve function's definition in place,
  its lines following by port designation, instead of keeping the old valve function (WP-S1.25, N-FL-1)
- behaviour-change: `valve.add` refuses a part with a PLC section (FT-1576); a `drives` link to the valve's own
  module or terminal fails the invariant EW-INV-025 (FI-06); a fluid port whose function definition copy is damaged
  no longer makes every other copy's update a misfit (FT-1569) (WP-S1.25)
