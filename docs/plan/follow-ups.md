# Follow-up tasks

**Status:** hand-written, one line per new entry · **Part of:** ElektraWatt plan

**Purpose.** The follow-up tasks since Appendix B of the [work breakdown](02-work-packages.md) was frozen at FT-998:
the small tasks an implementation cycle or a review leaves for a work package. One line per entry, appended at the
end, never rewritten:

```text
- FT-n (cycle N, <Severity>, <host WP or crate>; due: <due>): <text>. Source: <IDs>
```

`cycle N` is the cycle that opens the task; `<due>` names the cycle it is due in (`cycle N`) or the event it waits
for. The commit that opens a task carries `FT-Opens: FT-n`, and the commit that closes it `FT-Closes: FT-n`: a
trailer, not an edit, closes an entry ([CONTRIBUTING.md](../../CONTRIBUTING.md#records-commit-trailers)). `cargo
xtask records` lists the open tasks of this file and of Appendix B in [records.md](records.md), grouped as
scheduled, expired and other.
- FT-999 (cycle 43, Low, WP-S1.22; due: cycle 44): `ew import csv --strict` is accepted, hidden and ignored; give it a meaning or remove it. Source: ID-1052
- FT-1000 (cycle 43, Low, ew-hack; due: cycle 45): narrow GPL test builds now compile the hack's whole union (`windows`, harfrust, skrifa, regex): try excluding the platform crates and count the crates of a cold `-p ew-commands` build again. Source: ID-1053
- FT-1001 (cycle 43, Medium, ew-ui; due: cycle 44): `view_changed` in `crates/ew-ui/src/canvas.rs` takes the page model's address as its identity (`Arc::as_ptr(canvas.page()) as usize`), the pattern CIFIX1 removed from the navigator: a page model rebuilt at the old address skips the pointer replay; key it on a weak reference or a revision, with a test that a new model of the same revision replays. Source: ID-1056
- FT-1002 (cycle 43, Low, ci/dep-rules.toml; due: cycle 44): `ci/dep-rules.toml` allows the feature `ew/test-confirm` on dev edges only (`banned_features`), but `ew` declares no such feature; remove the entry or make `check-features` refuse a banned feature that no crate declares. Source: ID-1059
- FT-1003 (cycle 43, Low, xtask; due: cycle 45): nine historical commits carry a `Claude-Session:` trailer, and `check-trailers` refuses an unknown key; decide whether to allow that key, should a tool add it again, or keep refusing it and say so in CONTRIBUTING.md. Source: ID-1062
- FT-1004 (cycle 43, Low, WP-S4.02; due: cycle 45): `a_refused_turn_names_the_rebound_keys_on_every_path` rebinds only `tool.rotate_cw`, so a hard-coded "X" in the second slot of `layout-wrong-turn` still passes; rebind `tool.mirror_x` too and assert both keys in the whole text. Source: ID-1058
- FT-1005 (cycle 43, Low, xtask; due: cycle 44): settle's bail in `bless_group` for a filter term that matches no test, and its shared-binary branch, are untested, so a `bless_group` that ignored the empty list would pass; make the choice a pure function of the group, the matched binaries and the per-step lists, and test it. Source: ID-1057; ID-1058
- FT-1006 (cycle 43, Low, xtask; due: cycle 44): lane-check's `doc` step and the merge-point verify run no doctests, so a broken doctest (such as `key!`'s `compile_fail` example) shows first in CI at a delivery; run `cargo test --doc` over the selected crates in both. Source: ID-1058; ID-1068
- FT-1007 (cycle 43, Low, xtask; due: cycle 44): nothing before a delivery runs `cargo xtask api-guard`, so a CLI or `api_v1` surface change reaches CI unflagged (C43-p4#1); lane-check and the merge-point verify run it when a snapshot under an `api/` folder changes. Source: ID-1058; ID-1066
- FT-1008 (cycle 43, Low, WP-S1.22; due: cycle 44): `ew check --help` prints a 465-character line, because clap does not wrap the long doc paragraphs without its `wrap_help` feature (compare FT-988); enable `wrap_help`, with its vet entries, or break the paragraphs. Source: ID-1067
- FT-1009 (cycle 43, Low, ew-api; due: cycle 45): no test scans ew-api's source for key literals against `TEXT_KEYS`, so a new refusal key that has no text is caught only when the window shows `?`; add the scan, its failure naming the key. Source: ID-1074; C43-p5#3
- FT-1010 (cycle 43, Low, ew-api; due: cycle 45): the run-time label keys `field-<kind>-<path>` of `crates/ew-api/src/output/fields.rs` have no reader found and no test that each built key has a text; find the reader or remove them, else test them. Source: ID-1074
- FT-1011 (cycle 43, Low, xtask; due: cycle 44): `check-features` runs `cargo tree` with `--target all`, but no test fails without it, because `shipping_graph` builds its command inline; assert the arguments through a small builder function. Source: ID-1070; C43-p6#8
- FT-1012 (cycle 43, Low, ew-api; due: cycle 45): replace the kind-CSV test's `PENDING` list of 13 derived kinds without a golden CSV by a registry property, or bless the 13 golden CSVs under a `Golden:` decision. Source: ID-1075; C43-p6#1
- FT-1013 (cycle 43, Low, xtask; due: cycle 44): lane-check's fmt step snapshots only the diff's files, so a file outside the diff that `cargo fmt -p` rewrites, or a child module that the per-file rustfmt reformats, goes unreported; snapshot every file the step can rewrite. Source: ID-1070; C43-p1#5
- FT-1014 (cycle 44, Low, ew-viewmodel; due: cycle 45 (FT batch)): `format_count` (`crates/ew-viewmodel/src/strings.rs:3752`) takes any `Key`, so a new `format_count(key!("x"), n)` compiles with no check of its `-one` text, and `count_key!` is opt-in (all 18 sites use it today); make `count_key!` yield a `CountKey` newtype and `format_count` take only that. Source: ID-1074; ID-1094; C43-p7#7
- FT-1015 (cycle 44, Low, ew-viewmodel; due: cycle 45 (polish lane)): Rail, Duct, Auto-fill rail and Enclosure… show placeholder icons: Rail shares Align's `layout-align-left` (`crates/ew-viewmodel/src/actions.rs`) and Enclosure shares Symbols'; give each concept its own icon (doc 16a §4.11) and extend the icon tests. Source: ID-1090; C44-p2#8
- FT-1016 (cycle 44, Medium, WP-S4.02; due: cycle 45 (FT batch)): a new panel layout page keeps the scale 1:1, so the plate its surface view shows overhangs the sheet; rule and build the scale a new panel page takes (a 1:n that fits the surface on the sheet, when the page is created or its surface view is set), with a test on a new project. Source: ID-1090
- FT-1017 (cycle 44, Medium, ew-commands; due: cycle 45 (FT batch)): a far point of Draw rail or Draw duct is refused in raw nanometres by `ew-commands`' parameter range text: "Add carrier: Path, item 1: -9223372036854775808 is below the minimum -9007199254740991"; word a coordinate outside the range in terms a user reads (millimetres, or the drawing area), asserted whole. Source: ID-1090; C44-p2#1
- FT-1018 (cycle 44, Low, ew-ui; due: cycle 45 (FT batch)): `create::a_refusal_stays_in_the_dialog_and_the_type_is_a_choice` failed once (0.85 s) in merge point 1's `--all-features` workspace run and passes alone and in `ew-ui`'s suite; find the load-dependent wait (likely the asynchronous folio refusal before `settle`) and wait on the state. Source: ID-1094
- FT-1019 (cycle 44, Low, ew-api; due: cycle 45 (polish lane)): 902's older page rows in Properties (the Placement category) print raw values ("Den 3", "Num 7937500", `Scale {"one_to":10}`) from the `of_member` table (`crates/ew-api/src/output/fields.rs:219`); give them labels and words, or leave them out. Source: ID-1094; W44.2
