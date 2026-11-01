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
