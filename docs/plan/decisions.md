# Decisions

**Status:** hand-written, one line per new entry · **Part of:** ElektraWatt plan

**Purpose.** The decisions since the [decision log](06-decision-log.md) was frozen at ID-1050. One line per entry,
appended at the end, never rewritten:

```text
- ID-n (cycle N): <one sentence>. <refs>
```

`<refs>` names the specification, the ADR, the decisions or the follow-up tasks it rests on. A commit that implements
or records a decision cites it in a `Decision: ID-n` trailer (a golden change in `Golden: ID-n <reason>`); trailers,
not edits, follow an entry up ([CONTRIBUTING.md](../../CONTRIBUTING.md#records-commit-trailers)). `cargo xtask
records` lists every entry with the commits that cite it in [records.md](records.md), and `cargo xtask
check-trailers` refuses a trailer that cites an ID this file or the decision log does not hold at that commit.

- ID-1051 (cycle 43): Records come from commit trailers and one line per new entry; the decision log and Appendix B are frozen at ID-1050 and FT-998. RD43 §9 item 6; CONTRIBUTING.md "Records: commit trailers".
- ID-1052 (cycle 43): The `ew` CLI is one clap 4 derive tree lowered to the unchanged `Command`; its catalogue walks the tree for paths and flags; parse errors are clap's renderings (exit 2), a bare `--` ends options, `lib search` and `query` positionals take hyphen values, and `ew` or `ew lib` without a subcommand print help and exit 2. Supersedes the hand-written parser of WP-S1.22; doc 17 §3.1; RD43 §9 item 10.
- ID-1053 (cycle 43): A cargo-hakari workspace hack, `ew-hack` (GPL), unifies third-party features for the GPL members; it lands because second builds of other package sets compile 36% and 80% fewer crates (narrow first builds compile more); the 16 permissive crates, xtask, the importers, the external-owner crates and naga stay out; CI and the settle step `hakari` run `generate --diff` and `manage-deps --dry-run` (verify fails by design with these excludes) and need cargo-hakari 0.9.39. RD43 §3.2–3.3; `ci/README.md` "Workspace hack".
