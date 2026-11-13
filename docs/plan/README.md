# The plan

**Running:** M3, automation breadth, since 2026-11-13 ([m3.md](m3.md)): new development, release 0.3.0. **Done:**
M2.5-Fix, a properly working program, closed with the release of 0.2.0 on 2026-11-13 ([m2.5-fix.md](m2.5-fix.md));
M2.5, maintenance, closed on 2026-11-11 ([m2.5.md](m2.5.md)).

| File | What it is |
|---|---|
| [m3.md](m3.md) | The milestone that is running: its scope, how a work package is built, the order, the log |
| [m2.5-fix.md](m2.5-fix.md) | The fix milestone: how a fix is made, the order of the rows, the log |
| [m2.5.md](m2.5.md) | The maintenance milestone: principles, steps, assessment, what was cut and why, the exit record |
| [backlog.md](backlog.md) | Open work, one row each, by milestone. A finished row is deleted |
| [backlog-later.md](backlog-later.md) | Unscheduled rows: a seed for planning, not a commitment |
| [work-packages.csv](work-packages.csv) | The work breakdown: one row per work package, built and unbuilt, with its milestone |
| [../../ROADMAP.md](../../ROADMAP.md) | The milestones by name and content |

**How the plan is kept.**
- A milestone has one file like [m2.5-fix.md](m2.5-fix.md): what it is for, its steps, and at the end what it did.
- Open work is a row of the backlog. There is no separate list of decisions: a decision that changes how something
  works is written into the document that describes it, and the commit message says why.
- The plan carries no dates.

**History.** Up to 0.1.0 the plan folder also held the global plan with a section per development cycle, the gates,
the risk register, two decision logs with about 1,500 rulings, the follow-up lists, the generated records and the
specifications of the work packages as they were built. They were removed in M2.5 and are in git at commit
`5516edba4` under `docs/plan/`. The code and the design documents state what is true now; the old files state how it
came about.
