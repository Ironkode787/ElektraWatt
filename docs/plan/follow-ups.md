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
