# The plan

**Running:** M3.5, hygiene and then a fix round, since 2026-11-20 ([m3.5.md](m3.5.md)): release 0.3.5. **Done:**
M3, automation breadth, closed with the release of 0.3.0 on 2026-11-20 ([m3.md](https://github.com/Ironkode787/ElektraWatt/blob/935e1507e1f6a175c7d93615fb441ada68cc82dc/docs/plan/m3.md)); M2.5-Fix, a properly
working program, closed with the release of 0.2.0 on 2026-11-13 ([m2.5-fix.md](https://github.com/Ironkode787/ElektraWatt/blob/0926a57cebcde0316023c66b992d075549a3a5fd/docs/plan/m2.5-fix.md));
M2.5, maintenance, closed on 2026-11-11 ([m2.5.md](https://github.com/Ironkode787/ElektraWatt/blob/03848ea6b52e2a933988bc81cee043627f88681b/docs/plan/m2.5.md)).

| File | What it is |
|---|---|
| [m3.5.md](m3.5.md) | The hygiene milestone that is running: the scan, the steps, the loop measured |
| [m3.5-reference-project.md](m3.5-reference-project.md) | Step 9's design: what ends with the reference project, the sample that replaces it, the lanes |
| [m3.5-architecture.md](m3.5-architecture.md) | Step 15's rework to the architecture of ADR-0035: the rule every lane keeps, what was decided and not built, the second scan |
| [m5.md](m5.md) | The fluid milestone, decided and not started: its scope, its work packages, the order |
| [backlog.md](backlog.md) | Open work, one row each, by milestone. A finished row is deleted |
| [backlog-later.md](backlog-later.md) | Unscheduled rows: a seed for planning, not a commitment |
| [work-packages.csv](work-packages.csv) | The work breakdown: one row per work package, built and unbuilt, with its milestone |
| [../../ROADMAP.md](../../ROADMAP.md) | The milestones by name and content |

**How the plan is kept.**
- A milestone has one file like [m3.5.md](m3.5.md): what it is for, its steps, and at the end what it did. When it closes, the file goes and this page links its last text in git.
- Open work is a row of the backlog. There is no separate list of decisions: a decision that changes how something
  works is written into the document that describes it, and the commit message says why.
- The plan carries no dates.

**History.** Up to 0.1.0 the plan folder also held the global plan with a section per development cycle, the gates,
the risk register, two decision logs with about 1,500 rulings, the follow-up lists, the generated records and the
specifications of the work packages as they were built. They were removed in M2.5 and are in git at commit
`5516edba4` under `docs/plan/`. The code and the design documents state what is true now; the old files state how it
came about.
