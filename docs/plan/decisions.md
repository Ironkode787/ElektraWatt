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
