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
