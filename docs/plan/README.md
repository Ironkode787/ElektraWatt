# The plan

**Running:** M4, from the schematic to the built cabinet, since 2026-11-26 ([m4.md](m4.md)): release 0.4.0. **Done:**
M3.5, hygiene, one architecture and a fix round, closed with the release of 0.3.5 on 2026-11-26 ([m3.5.md](https://github.com/Ironkode787/ElektraWatt/blob/73b290039a04f6b58bd4b35a7004bfccbfee1de1/docs/plan/m3.5.md),
[its rework's plan](https://github.com/Ironkode787/ElektraWatt/blob/73b290039a04f6b58bd4b35a7004bfccbfee1de1/docs/plan/m3.5-architecture.md));
M3, automation breadth, closed with the release of 0.3.0 on 2026-11-20 ([m3.md](https://github.com/Ironkode787/ElektraWatt/blob/935e1507e1f6a175c7d93615fb441ada68cc82dc/docs/plan/m3.md)); M2.5-Fix, a properly
working program, closed with the release of 0.2.0 on 2026-11-13 ([m2.5-fix.md](https://github.com/Ironkode787/ElektraWatt/blob/0926a57cebcde0316023c66b992d075549a3a5fd/docs/plan/m2.5-fix.md));
M2.5, maintenance, closed on 2026-11-11 ([m2.5.md](https://github.com/Ironkode787/ElektraWatt/blob/03848ea6b52e2a933988bc81cee043627f88681b/docs/plan/m2.5.md)).

| File | What it is |
|---|---|
| [m4.md](m4.md) | The milestone that is running: what it is for, design then build, its work packages, the order |
| [m5.md](m5.md) | The fluid milestone, decided and not started: its scope, its work packages, the order |
| [backlog.md](backlog.md) | Open work, one row each, by milestone. A finished row is deleted |
| [backlog-later.md](backlog-later.md) | Unscheduled rows: a seed for planning, not a commitment |
| [work-packages.csv](work-packages.csv) | The work breakdown: one row per work package, built and unbuilt, with its milestone |
| [../../ROADMAP.md](../../ROADMAP.md) | The milestones by name and content |

**How the plan is kept.**
- A milestone has one file like [m4.md](m4.md): what it is for, its steps, and at the end what it did. When it
  closes, the file goes and this page links its last text in git. A step's own design file goes when the step is
  done, and the milestone's file links it in git (step 9's,
  [m3.5-reference-project.md](https://github.com/Ironkode787/ElektraWatt/blob/cf5d3e1fb778c424c56f8a6827e6058dce11cdb6/docs/plan/m3.5-reference-project.md)).
- Open work is a row of the backlog. There is no separate list of decisions: a decision that changes how something
  works is written into the document that describes it, and the commit message says why.
- The plan sets no target dates: a date in it says when something was decided or done.

**History.** Up to 0.1.0 the plan folder also held the global plan with a section per development cycle, the gates,
the risk register, two decision logs with about 1,500 rulings, the follow-up lists, the generated records and the
specifications of the work packages as they were built. They were removed in M2.5 and are in git at commit
`5516edba4` under `docs/plan/`. The code and the design documents state what is true now; the old files state how it
came about.
