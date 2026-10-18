#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Milestone dates, schedules and budget of the ElektraWatt plan (Python 3 standard library only).

Reads docs/plan/work-packages.csv and runs the schedule model of engine.py for every scenario of the plan.
Writes:
  docs/plan/milestones.csv        scenario x milestone -> month number and calendar month
  docs/plan/wp-schedule.csv       per WP: start and finish month per main scenario; developer in the baseline
  generated blocks in docs/plan/01-global-plan.md and docs/plan/02-work-packages.md

Run from anywhere:
  python3 -I docs/plan/tools/schedule.py          # rewrite the CSVs and the generated blocks
  python3 -I docs/plan/tools/schedule.py --check  # exit 1 if any of them is stale (for CI)
  python3 -I docs/plan/tools/schedule.py --print  # print the milestone table of every scenario
"""
import csv, io, math, sys
from collections import defaultdict
from pathlib import Path

sys.dont_write_bytecode = True          # keep the repository free of __pycache__
sys.path.insert(0, str(Path(__file__).resolve().parent))
import engine  # noqa: E402
from wbs import (PLAN, MS, RANK, MS_NAMES, GATE, RESERVE, PROD, LOE, STABILISATION, load_rows, pert,  # noqa: E402
                 milestone, label, mlabel, f1, r1, week_date, md_table, write_or_check, START, wp_ids_text)

TOOL = "schedule.py"
SHOWN = ["M0", "G-SP", "M1", "M-PP", "M2", "M3", "M4", "M5", "M6"]   # columns of the milestone tables

# ------------------------------------------------------------------ scenarios
# key: (label, scope, joins, efficiency from the last join, capacity factor, estimate, AI speed-up)
SCENARIOS = {}


def scen(key, lab, scope, joins, eff=1.0, cap=1.0, est="mean", ai=1.0):
    SCENARIOS[key] = dict(label=lab, scope=scope, joins=joins, eff=eff, cap=cap, est=est, ai=ai)


TEAMS = {2: ([0, 0], 1.0, "2 developers"), 3: ([0, 0, 0], 1.0, "3 developers"),
         4: ([0, 0, 0, 4], 0.9, "4 developers (fourth from month 4)")}
for n, (joins, eff, name) in TEAMS.items():
    for scope in ("lean", "full"):
        k = f"{n}d-{scope}"
        scen(k, f"{name}, {scope} scope", scope, joins, eff)
        scen(k + "-cap80", f"{name}, {scope} scope, capacity -20 %", scope, joins, eff, 0.8)
        scen(k + "-cap120", f"{name}, {scope} scope, capacity +20 %", scope, joins, eff, 1.2)
        scen(k + "-best", f"{name}, {scope} scope, best case (optimistic estimates, capacity +20 %)", scope, joins,
             eff, 1.2, "o")
SCENARIOS["3d-lean"]["label"] = "3 developers, lean scope (plan of record)"
scen("3d-lean-ai-moderate", "3 developers + AI-moderate, lean scope (internal target)", "lean", [0, 0, 0], ai=1.4)
scen("3d-lean-ai-strong", "3 developers + AI-strong, lean scope", "lean", [0, 0, 0], ai=1.8)
scen("3d-lean-ai-moderate-cap80", "3 developers + AI-moderate, lean scope, capacity -20 %", "lean", [0, 0, 0],
     cap=0.8, ai=1.4)
for n, joins in ((5, [0, 0, 0, 4, 6]), (6, [0, 0, 0, 4, 6, 6]), (8, [0, 0, 0, 4, 6, 6, 6, 6]),
                 (10, [0, 0, 0, 4, 6, 6, 6, 6, 6, 6])):
    for scope in ("lean", "full"):
        scen(f"{n}d-{scope}", f"{n} developers (funded growth, outside the 2-4 decision), {scope} scope", scope,
             joins, 0.85)

MAIN = ["3d-lean", "3d-lean-ai-moderate", "3d-lean-ai-strong", "3d-full", "4d-lean", "2d-lean"]

# Strict stream roles (01 §10.2)
ROLES_STRICT = [{"S0", "S1"}, {"S2", "S4", "S6"}, {"S3", "S5", "S6", "S7"}]
ROLES_SHARED = [{"S0", "S1", "S6"}, {"S2", "S4", "S6", "S7"}, {"S3", "S5", "S6", "S7", "S1"}]

_cache = {}
_wps = {}


def wps(scope):
    if scope not in _wps:
        _wps[scope] = engine.load(scope)
    return _wps[scope]


def run(key):
    if key not in _cache:
        s = SCENARIOS[key]
        _cache[key] = engine.simulate(wps(s["scope"]), s["joins"], s["eff"], s["cap"], s["est"], s["ai"])
    return _cache[key]


def run_custom(scope="lean", joins=(0, 0, 0), eff=1.0, cap=1.0, est="mean", ai=1.0, roles=None, override=None):
    wp = wps(scope)
    if override:
        import copy
        wp = copy.deepcopy(wp)
        for k, m in override.items():
            wp[k]["ms"] = m
    res = engine.simulate(wp, list(joins), eff, cap, est, ai, roles)
    if override:      # milestones are judged with the real milestones, only the priority changed
        real = wps(scope)
        run_, out = 0.0, {}
        for m in MS[:-1]:
            run_ = max([run_] + [f * 12 / 52 for k, f in res.finish.items()
                                 if RANK[real[k]["ms"]] <= RANK[m] and k not in LOE])
            out[m] = math.ceil(run_ - 1e-9)
        out["G-SP"] = res.ms["G-SP"]
        res.ms = out
    return res


def m(key, ms):
    return run(key).ms[ms]


def ml(key, ms):
    return mlabel(m(key, ms))


# ------------------------------------------------------------------ effort helpers
ROWS = load_rows()
ROWD = {r["wp_id"]: r for r in ROWS}


def effort(scope, upto="M6", ai=1.0, reserve=False):
    tot = 0.0
    for r in ROWS:
        ms = milestone(r, scope)
        if ms != "1.x" and RANK[ms] <= RANK[upto]:
            tot += pert(r) * engine.ai_factor(r["wp_id"], r["stream"], ai)
    return tot * (1 + RESERVE) if reserve else tot


def capacity_bound(effort_pw, joins, eff, cap=1.0):
    """Month at which the team has delivered effort_pw with perfect parallelism (efficiency from last join)."""
    last = max(joins)
    t, done, step = 0.0, 0.0, 1 / 64
    while done < effort_pw - 1e-9:
        n = sum(1 for j in joins if j <= t + 1e-9)
        e = eff if t >= last - 1e-9 else 1.0
        rate = n * 3 * cap * e          # 3 pw per developer-month (GA-3)
        if done + rate * step >= effort_pw:
            return t + (effort_pw - done) / rate
        done += rate * step
        t += step
    return t


# ------------------------------------------------------------------ outputs: CSV files
def milestones_csv():
    out = io.StringIO()
    w = csv.writer(out, lineterminator="\n")
    w.writerow(["scenario", "label", "scope", "developers", "joins", "efficiency", "capacity", "estimate",
                "ai_speedup", "milestone", "gate", "month", "calendar"])
    for k, s in SCENARIOS.items():
        res = run(k)
        for ms in SHOWN:
            gate = "G-SP" if ms == "G-SP" else GATE[ms]
            w.writerow([k, s["label"], s["scope"], len(s["joins"]), " ".join(map(str, s["joins"])), s["eff"],
                        s["cap"], s["est"], s["ai"], ms if ms != "G-SP" else "", gate, res.ms[ms], label(res.ms[ms])])
    return out.getvalue()


def wp_schedule_csv():
    out = io.StringIO()
    w = csv.writer(out, lineterminator="\n")
    head = ["wp_id", "title", "stream", "milestone", "milestone_lean", "lever", "pert_pw", "developer_baseline"]
    for k in MAIN:
        head += [f"start_{k}", f"finish_{k}"]
    w.writerow(head)
    base = run("3d-lean")
    for r in ROWS:
        k = r["wp_id"]
        row = [k, r["title"], r["stream"], r["milestone"], r["milestone_lean"], r["lever"], f1(pert(r)),
               f"dev {base.dev[k] + 1}" if k in base.dev else ""]
        for sk in MAIN:
            res = run(sk)
            if k in res.start:
                s0, f0 = res.months(k)
                row += [f1(s0), f1(f0)]
            else:
                row += ["", ""]
        w.writerow(row)
    return out.getvalue()


# ------------------------------------------------------------------ blocks for 01
def b_summary():
    rows = [
        ("**3 developers, lean scope: plan of record, external commitments**", "3d-lean", True),
        ("Same, capacity −20 % / +20 %", ("3d-lean-cap80", "3d-lean-cap120"), False),
        ("**3 developers + AI-moderate, lean scope: internal target**", "3d-lean-ai-moderate", True),
        ("3 developers + AI-strong, lean scope (upside, not planned)", "3d-lean-ai-strong", False),
        ("3 developers, full scope (if PD-1 rejects the levers)", "3d-full", False),
        ("4 developers (fourth from month 4), lean scope", "4d-lean", False),
        ("2 developers, lean scope", "2d-lean", False),
        ("6 developers (funded growth, outside the 2–4 decision), lean scope", "6d-lean", False),
    ]
    out = []
    for name, key, bold in rows:
        cells = []
        for ms in ("M1", "M-PP", "M2", "M6"):
            if isinstance(key, tuple):
                c = f"{ml(key[0], ms)} / {ml(key[1], ms)}"
            else:
                c = ml(key, ms)
            cells.append(f"**{c}**" if bold else c)
        out.append([name] + cells)
    return md_table(["Scenario", "M1 spine exit", "M-PP pilot preview 0.0.x", "M2 MVP 0.1.0", "M6 release 1.0"], out)


def b_team_scenarios():
    def per_month(joins, eff, t):
        n = sum(1 for j in joins if j <= t)
        return n * 3 * (eff if t >= max(joins) else 1.0)

    def cum(joins, eff, months):
        return sum(per_month(joins, eff, t + 0.5) for t in range(months))
    rows = []
    for n, joins, eff, name, profile in (
            (2, [0, 0], 1.0, "2 developers", "2 from month 0"),
            (3, [0, 0, 0], 1.0, "**3 developers (baseline)**", "3 from month 0"),
            (4, [0, 0, 0, 4], 0.9, "4 developers", "3 from month 0, 1 from month 4"),
            (6, [0, 0, 0, 4, 6, 6], 0.85, "6 developers (funded growth, for comparison)",
             "3 from month 0, 1 from month 4, 2 from month 6")):
        effs = "100 %" if eff == 1 else f"{round(eff * 100)} % from month {max(joins)}"
        pm = sorted({per_month(joins, eff, t + 0.5) for t in range(24)}, key=lambda x: x)
        pms = ", then ".join(f1(x) for x in sorted(set(pm), key=lambda x: [per_month(joins, eff, t + .5) for t in
                                                                          range(24)].index(x)))
        rows.append([name, profile, effs, pms, f"{r1(per_month(joins, eff, 30) * 12):.0f}",
                     f"{cum(joins, eff, 15):.0f}", f"{cum(joins, eff, 18):.0f}"])
    return md_table(["Scenario", "Developers", "Efficiency", "pw per month", "pw per year (steady state)",
                     "pw in 15 months", "pw in 18 months"], rows)


def b_capacity_bound():
    rows = []
    for n, (joins, eff, name) in TEAMS.items():
        cells = [name]
        for scope in ("lean", "full"):
            for upto in ("M-PP", "M2", "M6"):
                b = capacity_bound(effort(scope, upto, reserve=True), joins, eff)
                cells.append(f"{f1(b)} / {m(f'{n}d-{scope}', upto)}")
        rows.append(cells)
    return md_table(["Team", "Lean M-PP: bound / simulated", "Lean MVP: bound / simulated",
                     "Lean 1.0: bound / simulated", "Full M-PP: bound / simulated", "Full MVP: bound / simulated",
                     "Full 1.0: bound / simulated"], rows)


def dates_table(keys, names, bold_key=None):
    rows = []
    for k, name in zip(keys, names):
        cells = [ml(k, ms) for ms in SHOWN]
        if k == bold_key:
            cells = [f"**{c}**" for c in cells]
            name = f"**{name}**"
        rows.append([name] + cells)
    return md_table(["Team", "M0", "G-SP spike", "M1", "M-PP 0.0.x", "M2 MVP", "M3", "M4", "M5 beta", "M6 1.0"],
                    rows)


def b_dates(scope):
    keys = [f"{n}d-{scope}" for n in TEAMS]
    names = [TEAMS[n][2] for n in TEAMS]
    return dates_table(keys, names, "3d-lean" if scope == "lean" else None)


def b_sensitivity(with_calendar=True):
    rows = []
    for scope in ("lean", "full"):
        for n in TEAMS:
            k = f"{n}d-{scope}"
            fmt = ml if with_calendar else m
            cells = [str(n), scope.capitalize()]
            for ms in ("M2", "M6"):
                cells += [str(fmt(k + "-cap80", ms)), str(fmt(k, ms)), str(fmt(k + "-cap120", ms))]
            cells.append(f"{m(k + '-best', 'M2')} / {m(k + '-best', 'M6')}")
            if k == "3d-lean":
                cells = [f"**{c}**" for c in cells]
            rows.append(cells)
    return md_table(["Team", "Scope", "M2 at −20 %", "M2 base", "M2 at +20 %", "M6 at −20 %", "M6 base",
                     "M6 at +20 %", "Best case M2 / M6"], rows)


def add_months(n):
    k = START.month - 1 + n
    return f"{START.year + k // 12}-{k % 12 + 1:02d}-01"


def b_gantt_teams():
    L = ["```mermaid", "gantt", "  title M1, pilot preview, MVP and 1.0 per scenario, lean scope, base capacity",
         "  dateFormat YYYY-MM-DD", "  axisFormat %Y", "  section Architecture indicative",
         "  Spine to M1         :i1, 2026-11-01, 2027-02-01", "  M1 to MVP           :i2, 2027-02-01, 2027-06-01",
         "  MVP to 1.0          :i3, 2027-06-01, 2028-05-01"]
    for i, (k, name) in enumerate((("2d-lean", "2 developers"), ("3d-lean", "3 developers plan of record"),
                                   ("3d-lean-ai-moderate", "3 developers AI-moderate"),
                                   ("3d-lean-ai-strong", "3 developers AI-strong"),
                                   ("4d-lean", "4 developers"), ("6d-lean", "6 developers funded"))):
        r = run(k).ms
        L.append(f"  section {name}")
        pts = [0, r["M1"], r["M-PP"], r["M2"], r["M6"]]
        for j, nm in enumerate(("Spine to M1", "M1 to pilot preview", "Preview to MVP", "MVP to 1.0")):
            L.append(f"  {nm:<20}:s{i}{j}, {add_months(pts[j])}, {add_months(pts[j + 1])}")
    L.append("```")
    return "\n".join(L)


def b_ai():
    base = "3d-lean"
    rows = []
    for k, name in (("3d-lean", "Baseline: conservative estimates (plan of record, external)"),
                    ("3d-lean-ai-moderate", "AI-moderate: development × 1/1.4 (internal target)"),
                    ("3d-lean-ai-strong", "AI-strong: development × 1/1.8 (upside)"),
                    ("3d-lean-ai-moderate-cap80", "AI-moderate with capacity −20 %")):
        s = SCENARIOS[k]
        e = effort("lean", "M6", s["ai"], reserve=True)
        ratio = effort("lean", "M6", reserve=True) / e
        rows.append([name, f"{e:.0f}", f"{ratio:.2f}"] + [ml(k, ms) for ms in ("M1", "M-PP", "M2", "M6")])
    return md_table(["Scenario (3 developers, lean scope)", "Effort to 1.0 with reserve (pw)",
                     "Equivalent velocity ratio", "M1", "M-PP 0.0.x", "M2 MVP 0.1.0", "M6 1.0"], rows)


def b_phases():
    res, wp = run("3d-lean"), wps("lean")
    ms = res.ms
    first = defaultdict(lambda: math.inf)
    for k, w in wp.items():      # a phase starts with its first non-S0 WP (S0 work runs beside everything)
        if w["stream"] != "S0" or w["ms"] == "M0":
            first[w["ms"]] = min(first[w["ms"]], res.start[k] * 12 / 52)
    defs = [
        ("**P0 Foundations**", ["M0"], "M0", "Repository, CI on three OSes, licences and the licence boundary, "
         "pilot kick-off, signing enrolment and trademark search started, early ADRs", "S0, S6", "M0"),
        ("**P1 Spine**", ["M1"], "M1", "Two lanes ([architecture](../design/04-architecture.md#15-spine-build-order-and-parallel-streams) "
         "§15.1): core (crates, schema, model, commands, derivation, `ew-memo`, persistence, SQL projection) and output "
         "(display list, forms, reports, CPU renderer, PDF), plus test kit and golden data", "S1, S3, S6, S7", "M1"),
        ("**P2 Pilot preview**", ["M-PP"], "M-PP", "Shell, 2D canvas, viewer, placement and wiring tools, property "
         "grid, navigators, messages; model breadth for devices, terminals, cables, numbering; rule engine and core "
         "online checks; project parts", "S1, S2", "M-PP, release 0.0.x (pilot only)"),
        ("**P3 Interactive MVP**", ["M2"], "M2", "Rule packs, revisions, collaboration, parts libraries and macros; "
         "PLC, bus and fluid MVP model; domain tables, palette, keymaps; 2D panel; packaging, signing, CRA processes; "
         "R0 and R1", "S1, S2, S4, S6, S7", "M2, release 0.1.0"),
        ("**P4 Automation breadth and migration**", ["M3"], "M3", "PLC, bus and fluid 1.0; QET project migration; AML, "
         "TIA and PLCopen exchange; EDZ; DXF/DWG", "S1, S2, S3", "M3"),
        ("**P5 Physical and manufacturing**", ["M4"], "M4", "STEP worker, 3D model and view, collision, routing, "
         "lengths; wire records, ferrules, labels, machine profiles, `.ewpp`", "S4, S5", "M4"),
        ("**P6 Completion**", ["M5"], "M5", "Model, rule-pack and report completion; deferred MVP Should items; beta",
         "S1, S2, S3", "M5, 1.0 beta"),
        ("**P7 Hardening and release**", ["M6"], "M6", "1.0 quality layers, R2, external acceptance (SC-7, SC-8), "
         "documentation and German, format freeze, stabilisation", "S6, S2, S7", "M6, release 1.0"),
    ]
    rows = []
    for name, groups, end, goal, streams, ends in defs:
        s = min(first[g] for g in groups)
        s0 = 0 if s < 1e-9 else math.floor(s)
        rows.append([name, f"{s0}–{ms[end]}", f"{label(s0 + 1)} – {label(ms[end])}", goal, streams, ends])
    rows.append(["**P8 1.x**", f"from {ms['M6'] + 1}", f"from {label(ms['M6'] + 1)}",
                 "Quarterly minors with the 1.x themes (§6.4)", "all", "—"])
    return md_table(["Phase", "Months", "Calendar", "Goal", "Main streams", "Ends with"], rows)


def b_pp_scope():
    wp = wps("lean")
    rows = []
    tot = 0.0
    for r in ROWS:
        if r["milestone_lean"] == "M-PP":
            rows.append([r["wp_id"], r["title"], r["stream"], f1(pert(r))])
            tot += pert(r)
    rows.append(["**Sum**", f"**{len(rows)} WPs**", "", f"**{f1(tot)}**"])
    cum = effort("lean", "M-PP")
    return (md_table(["WP", "Title", "Stream", "PERT (pw)"], rows) +
            f"\n\nWith the M0 and M1 WPs, M-PP needs {f1(cum)} pw ({f1(cum * 1.2)} pw with the reserve); the MVP "
            f"needs {f1(effort('lean', 'M2'))} pw. The preview is therefore {round(100 * cum / effort('lean', 'M2'))} % "
            "of the MVP effort and reuses every M1 output unchanged.")


def b_release_train():
    res, wp = run("3d-lean"), wps("lean")
    ms = res.ms
    land = defaultdict(list)
    for k in wp:
        if k in LOE:
            continue
        land[math.ceil(res.finish[k] * 12 / 52 - 1e-9)].append(k)
    rows = []
    for n in range(ms["M2"] + 1, ms["M5"]):
        if not land[n]:
            continue
        rel = f"0.{n - ms['M2'] + 1}.0"
        mark = next((f"**{x}** at {rel}" for x in ("M3", "M4") if ms[x] == n), "")
        rows.append([rel, mlabel(n), mark, "; ".join(f"{k} {ROWD[k]['title']}" for k in sorted(land[n]))])
    pre = [k for n in range(0, ms["M2"] + 1) for k in land[n] if RANK[wp[k]["ms"]] > RANK["M2"]]
    t = md_table(["Release", "Month (calendar)", "Milestone", "WPs that land (finished in that month)"], rows)
    if pre:
        t += ("\n\nFinished before the MVP although they are later scope (in the 0.1.0 build, but neither promised "
              "nor gated by the MVP): " + ", ".join(sorted(pre)) + ".")
    return t


def b_betas():
    res, wp = run("3d-lean"), wps("lean")
    ms = res.ms
    land = defaultdict(list)
    for k in wp:
        if k not in LOE:
            land[math.ceil(res.finish[k] * 12 / 52 - 1e-9)].append(k)
    rows, n5, n6 = [], ms["M5"], ms["M6"]
    rc1 = max(n5 + 1, n6 - 2)
    for n in range(n5, n6 + 1):
        if n == n6:
            rel, note = "**1.0.0**", "**M6**"
        elif n == rc1:
            rel, note = "1.0.0-rc.1", "**Format freeze**, 8 weeks before 1.0 ([quality](../design/18-quality-security-release.md#72-format-change-window) §7.2)"
        elif n > rc1:
            rel, note = f"1.0.0-rc.{n - rc1 + 1}", ""
        else:
            rel, note = f"1.0.0-beta.{n - n5 + 1}", "**M5**: feature complete" if n == n5 else ""
        wl = "; ".join(f"{k} {ROWD[k]['title']}" for k in sorted(land[n]))
        rows.append([rel, mlabel(n), "; ".join(x for x in (note, wl) if x) or "Fixes"])
    return md_table(["Release", "Month (calendar)", "Content"], rows)


THEMES = [
    ("Review and productivity: revision compare, change lists, review and merge UI, editor completion, macros 1.0",
     ["WP-S1.31", "WP-S2.15", "WP-S2.14", "WP-S1.29"], 3),
    ("Panel engineering: thermal calculation, model views, busbars, 3D export, verification checklists, wiring viewer",
     ["WP-S4.10", "WP-S4.09", "WP-S5.08", "WP-S5.09"], 6),
    ("Content, forms and languages: full IEC 60617 and ISO 1219 libraries, forms editor and ISO 5457 frames, project "
     "translations, GSD/GSDML/IODD", ["WP-S7.04", "WP-S3.14", "WP-S1.27", "WP-S3.17"], 9),
    ("Extensibility and backlogs: WASM plugins and SDK, automation 1.0, interop, physical and manufacturing 1.x",
     ["WP-S1.33", "WP-S1.32", "WP-S3.20", "WP-S4.11", "WP-S5.10", "WP-S1.34"], 12),
]


def first_march_after(n_months):
    k = n_months
    while True:
        lab = label(k)
        if lab.startswith("Mar"):
            return lab
        k += 1


def b_themes():
    n6 = m("3d-lean", "M6")
    rows = []
    for i, (name, ids, off) in enumerate(THEMES):
        p = sum(pert(ROWD[k]) for k in ids)
        rel = f"1.{i + 1} ({label(n6 + off)})"
        if i == 3:
            rel += f", then 2.0 in the format window ({first_march_after(n6 + 12)} at the earliest)"
        rows.append([name, rel, ", ".join(ids), f1(p), f1(p * 1.2)])
    back = sum(pert(r) for r in ROWS if milestone(r, "lean") == "1.x")
    return (md_table(["Theme", "Earliest release", "WPs", "PERT (pw)", "With reserve"], rows) +
            f"\n\nThe 1.x backlog (lever (c) WPs plus the original 1.x WPs) is {f1(back)} pw, {f1(back * 1.2)} pw with "
            f"the reserve: about {round(back * 1.2 / 9)} months of the 3-developer team before any new 1.x scope.")


GANTT_GROUPS = [
    ("S0 Foundation and governance", [
        ("Pilot kick-off", ["WP-S0.08"]), ("Repository and CI bootstrap", ["WP-S0.01"]),
        ("Signing enrolment and trademark search", ["WP-S0.05a"]),
        ("Licences REUSE DCO and LG-1 LG-2 LG-5", ["WP-S0.02", "WP-S0.04"]),
        ("Docs site and governance files", ["WP-S0.06", "WP-S0.03"]),
        ("Pilot programme and community", ("SPAN", "M1", "M6")),
        ("Enrolment lead time external", "LEAD"),
        ("Steward signing identities LG-3 LG-4 close", ["WP-S0.05"])]),
    ("S1 Core model and derivation", [
        ("Foundation crates schema model commands", ["WP-S1.01", "WP-S1.02", "WP-S1.03", "WP-S1.04"]),
        ("Persistence core", ["WP-S1.09"]),
        ("Derivation I to III", ["WP-S1.06", "WP-S1.07", "WP-S1.08"]),
        ("Perf spike ew-memo and SQL projection", ["WP-S1.05", "WP-S1.10"]),
        ("Preview breadth numbering devices terminals parts", ["WP-S1.11", "WP-S1.12", "WP-S1.13", "WP-S1.14",
                                                               "WP-S1.19a"]),
        ("Rule engine and core online checks", ["WP-S1.15", "WP-S1.16a"]),
        ("Parts macros rule packs revisions", ["WP-S1.19", "WP-S1.16", "WP-S1.18"]),
        ("PLC bus fluid models", ["WP-S1.21", "WP-S1.20"]),
        ("Collaboration and CLI", ["WP-S1.17", "WP-S1.22"]),
        ("PLC bus and fluid 1.0", ["WP-S1.24", "WP-S1.25"]),
        ("Parts 1.0", ["WP-S1.28"]),
        ("Model 1.0 rule packs 1.0 and MCP", ["WP-S1.26", "WP-S1.30", "WP-S1.23"])]),
    ("S2 Interaction and UI", [
        ("Shell canvas and viewer", ["WP-S2.01", "WP-S2.02", "WP-S2.03"]),
        ("Placement wiring tools property grid navigators messages", ["WP-S2.04", "WP-S2.05", "WP-S2.06",
                                                                      "WP-S2.09"]),
        ("Domain tables palette keymaps", ["WP-S2.07", "WP-S2.08"]),
        ("GUI tests and usability R0 R1", ["WP-S2.11", "WP-S2.12"]),
        ("Automation workspaces", ["WP-S2.13"]), ("Symbol editor", ["WP-S2.10"]),
        ("Usability R2 and fixes", ["WP-S2.16"])]),
    ("S3 Documents and migration", [
        ("Display list frames forms report engine PDF", ["WP-S3.01", "WP-S3.02", "WP-S3.03", "WP-S3.04"]),
        ("Spine importers CSV and QET elements", ["WP-S3.05"]),
        ("Importer contracts and migration report", ["WP-S3.09"]),
        ("MVP reports SVG printing table exports", ["WP-S3.06", "WP-S3.07", "WP-S3.08"]),
        ("QET project import and corpus", ["WP-S3.10", "WP-S3.11", "WP-S3.13"]),
        ("AML TIA PLCopen EDZ DXF DWG exchange", ["WP-S3.16", "WP-S3.18", "WP-S3.19"]),
        ("1.0 reports and QET assistants", ["WP-S3.15", "WP-S3.12"])]),
    ("S4 Physical", [
        ("2D panel model and tools", ["WP-S4.01", "WP-S4.02"]),
        ("STEP worker 3D model and 3D view", ["WP-S4.03", "WP-S4.04", "WP-S4.05"]),
        ("Collision routing and wire lengths", ["WP-S4.06", "WP-S4.07", "WP-S4.08"])]),
    ("S5 Manufacturing outputs", [
        ("Wire records ferrules labels", ["WP-S5.01", "WP-S5.02", "WP-S5.03"]),
        ("Drilling and assembly lists", ["WP-S5.05", "WP-S5.06"]),
        ("Machine profiles and production package", ["WP-S5.04", "WP-S5.07"])]),
    ("S6 Quality and release", [
        ("Test kit determinism perf gate", ["WP-S6.01", "WP-S6.02", "WP-S6.03"]),
        ("Packaging security signing privacy", ["WP-S6.04", "WP-S6.05", "WP-S6.06", "WP-S6.07"]),
        ("MVP stabilisation", ["WP-S6.08"]), ("Monthly 0.x releases", ("SPAN", "M2", "M6")),
        ("1.0 hardening and stabilisation", ["WP-S6.10", "WP-S6.12"]),
        ("External acceptance SC-7 SC-8", ["WP-S6.11"])]),
    ("S7 Content", [
        ("Golden data and starter symbols", ["WP-S7.01", "WP-S7.02"]),
        ("MVP docs and starter parts", ["WP-S7.03", "WP-S7.05"]),
        ("Golden 1.0 fixtures", ["WP-S7.06"]),
        ("1.0 docs and manufacturer pack", ["WP-S7.07", "WP-S7.08"])]),
]


def r0_r1(res):
    """R0 when the placement and wiring tools land; R1 six weeks before the MVP."""
    return res.finish["WP-S2.04"], res.ms["M2"] * 52 / 12 - 6


def b_gantt_baseline():
    res, wp = run("3d-lean"), wps("lean")
    L = ["```mermaid", "gantt", "  title Baseline - 3 developers, lean scope (plan of record)", "  dateFormat YYYY-MM-DD",
         "  axisFormat %Y", "  section Milestones"]
    last = {}
    for ms in MS[:-1]:
        last[ms] = max(res.finish[k] for k in wp if RANK[wp[k]["ms"]] <= RANK[ms] and k not in LOE)
        last[ms] = max([last[ms]] + [last[x] for x in MS[:RANK[ms]]])
    names = {"M0": "M0 foundations", "M1": "M1 spine exit", "M-PP": "M-PP pilot preview 0.0.x",
             "M2": "M2 MVP 0.1.0", "M3": "M3 automation", "M4": "M4 manufacturing and 3D", "M5": "M5 beta",
             "M6": "M6 release 1.0"}
    for i, ms in enumerate(MS[:-1]):
        L.append(f"  {names[ms]} :milestone, m{i}, {week_date(last[ms])}, 0d")
    L.append(f"  G-SP spike decision :milestone, gsp, {week_date(res.finish['WP-S1.03'] + 52 / 12)}, 0d")
    r0, r1_ = r0_r1(res)
    L.append(f"  R0 formative usability :milestone, r0, {week_date(r0)}, 0d")
    L.append(f"  R1 summative usability :milestone, r1, {week_date(r1_)}, 0d")
    t = 1
    for sec, groups in GANTT_GROUPS:
        L.append(f"  section {sec}")
        for name, ids in groups:
            if ids == "LEAD":
                s, f = res.finish["WP-S0.05a"], res.finish["WP-S0.05a"] + 52
            elif isinstance(ids, tuple):           # level-of-effort WP drawn as a span between two milestones
                s, f = last[ids[1]], last[ids[2]]
            else:
                ids = [k for k in ids if k in wp]
                if not ids:
                    continue
                s, f = min(res.start[k] for k in ids), max(res.finish[k] for k in ids)
            L.append(f"  {name} :t{t}, {week_date(s)}, {week_date(f)}")
            t += 1
    L.append("```")
    return "\n".join(L)


INTERFACES = {
    ("S1", "S3"): ("Schema, derivation queries, rule engine, PLC model", "WP-S1.06/WP-S1.07 → WP-S3.03 (report engine); WP-S1.24 → WP-S3.16 (PLC exchange)"),
    ("S1", "S2"): ("`Session` facade, view-model queries, model breadth", "WP-S1.04 → WP-S2.01; WP-S1.13/WP-S1.07 → WP-S2.04 (tools)"),
    ("S1", "S6"): ("Determinism and perf targets need the model and the memo engine", "WP-S1.05 → WP-S6.03"),
    ("S1", "S4"): ("Parts and device breadth", "WP-S1.19 → WP-S4.01"),
    ("S1", "S7"): ("Part data for content", "WP-S1.28 → WP-S7.08"),
    ("S4", "S5"): ("Layout and routed lengths feed the manufacturing outputs", "WP-S4.08 → WP-S5.04"),
    ("S2", "S4"): ("Renderer and navigators for 3D", "WP-S2.02 → WP-S4.04"),
    ("S2", "S6"): ("Shell and renderer for packaging", "WP-S2.01 → WP-S6.06"),
    ("S3", "S2"): ("The display list feeds the 2D canvas", "WP-S3.01 → WP-S2.02"),
    ("S0", "S6"): ("Steward, signing identities and CI for the release pipeline", "WP-S0.05 → WP-S6.07"),
}


def b_cross_stream():
    wp = wps("lean")
    cnt = defaultdict(int)
    for k, w in wp.items():
        if k in STABILISATION:
            continue
        for d in w["deps"] + list(w["lag"]):
            if d in STABILISATION:
                continue
            a, b = wp[d]["stream"], w["stream"]
            if a != b:
                cnt[(a, b)] += 1
    tot = sum(cnt.values())
    s1 = sum(v for (a, b), v in cnt.items() if a == "S1")
    rows = []
    for pair, v in sorted(cnt.items(), key=lambda x: (-x[1], x[0])):
        if pair in INTERFACES:
            rows.append([f"{pair[0]} → {pair[1]}", v, INTERFACES[pair][0], INTERFACES[pair][1]])
    other = tot - sum(r[1] for r in rows)
    rows.append(["Other pairs", other, "Smaller interfaces", "—"])
    return (f"Cross-stream dependencies in the CSV (lean scope, without the two stabilisation WPs) concentrate on S1: "
            f"{s1} of the {tot} cross-stream edges start in S1. The strongest interfaces:\n\n" +
            md_table(["From → to", "Edges", "Interface", "Typical WP pair"], rows))


def chain_text(path):
    return " → ".join(path)


def crit(scope, target, upto=None):
    wp = wps(scope)
    f, prev, base = engine.critical_path(wp)
    if upto:
        ks = [k for k in wp if RANK[wp[k]["ms"]] <= RANK[upto] and k not in LOE]
        target = max(ks, key=f)
    path, k = [], target
    while k:
        path.append(k)
        k = prev[k]
    return list(reversed(path)), f(target), base, f


def b_critical(scope="lean"):
    rows = []
    for ms in ("M1", "M-PP", "M2", "M6"):
        path, fin, _, _ = crit(scope, None, ms)
        bound = capacity_bound(effort(scope, ms, reserve=True), [0, 0, 0], 1.0)
        res = run(f"3d-{scope}")
        sim = max(res.finish[k] for k in res.finish if RANK[wps(scope)[k]["ms"]] <= RANK[ms] and k not in LOE) * 12 / 52
        rows.append([ms, f"{f1(fin * 12 / 52)} months ({wp_ids_text(path) if len(path) > 6 else chain_text(path)})",
                     f"{f1(bound)} months", f"{f1(sim)} → month {math.ceil(sim - 1e-9)}"])
    return md_table([f"Milestone ({scope} scope)", "Longest dependency chain, unlimited developers",
                     "Capacity bound, 3 developers", "Simulated, 3 developers"], rows)


def impacts():
    """Simulated what-if cases (01 §9.3, §9.2, 03 §4.2): name -> milestone dict."""
    out = {}
    lean3 = dict(scope="lean", joins=(0, 0, 0))
    for name, kw in (
            ("Plan of record (3 developers, lean scope)", {}),
            ("Effort +10 %", dict(cap=1 / 1.1)),
            ("Effort +25 %", dict(cap=1 / 1.25)),
            ("Velocity ratio 0.90 (effort +11 %)", dict(cap=0.9)),
            ("Velocity ratio 0.80 (effort +25 %)", dict(cap=0.8)),
            ("Velocity ratio 0.67 (effort +50 %)", dict(cap=2 / 3)),
            ("Velocity ratio 1.25 on every WP", dict(cap=1.25)),
            ("Velocity ratio 1.50 on every WP (clamp limit)", dict(cap=1.5)),
            ("Capacity −20 %", dict(cap=0.8)),
            ("Third developer joins at month 6", dict(joins=(0, 0, 6))),
            ("Third developer joins at month 12", dict(joins=(0, 0, 12))),
            ("Strict roles: dev 1 S0 and S1; dev 2 S2, S4, S6; dev 3 S3, S5, S6, S7", dict(roles=ROLES_STRICT)),
            ("Strict, but dev 3 also takes S1, dev 2 also S7 and dev 1 also S6", dict(roles=ROLES_SHARED)),
    ):
        args = dict(lean3)
        args.update(kw)
        out[name] = run_custom(**args).ms
    out["4 developers, plan order"] = run("4d-lean").ms
    out["4 developers, STEP worker (WP-S4.03) before M2"] = run_custom(
        "lean", (0, 0, 0, 4), 0.9, override={"WP-S4.03": "M2"}).ms
    out["3 developers, STEP worker (WP-S4.03) before M2"] = run_custom("lean", override={"WP-S4.03": "M2"}).ms
    return out


def b_impacts():
    rows = [[n] + [str(v[ms]) for ms in ("M1", "M-PP", "M2", "M6")] for n, v in impacts().items()]
    return md_table(["Case (lean scope)", "M1", "M-PP", "M2", "M6"], rows)


def b_silos():
    imp = impacts()
    rows = []
    for name, key in (("**Pull rule, owners review (plan of record)**", "Plan of record (3 developers, lean scope)"),
                      ("Strict: dev 1 S0 and S1; dev 2 S2, S4, S6; dev 3 S3, S5, S6, S7",
                       "Strict roles: dev 1 S0 and S1; dev 2 S2, S4, S6; dev 3 S3, S5, S6, S7"),
                      ("Strict, but dev 3 also takes S1, dev 2 also S7 and dev 1 also S6",
                       "Strict, but dev 3 also takes S1, dev 2 also S7 and dev 1 also S6")):
        v = imp[key]
        rows.append([name] + [str(v[ms]) for ms in ("M1", "M-PP", "M2", "M6")])
    return md_table(["Working mode (3 developers, lean scope)", "M1", "M-PP", "M2", "M6"], rows)


def b_dev4():
    base = run("3d-lean").ms
    rows = [["Not at all", base["M1"], base["M-PP"], base["M2"], base["M6"], "—", "—"]]
    for j in (4, 9, 12, 15, 24, 30, 40, 46):
        r = run_custom("lean", (0, 0, 0, j), 0.9).ms
        rows.append([f"Month {j} (from {add_months(j)})", r["M1"], r["M-PP"], r["M2"], r["M6"],
                     f"{base['M2'] - r['M2']} / {base['M6'] - r['M6']}".replace("-", "−"), f1((r["M6"] - j) / 12)])
    return md_table(["Developer 4 joins", "M1", "M-PP", "M2", "M6", "Gain against 3 developers (M2 / M6, months)",
                     "Developer-years added"], rows)


def b_beyond():
    rows = []
    for n, txt in ((5, "3 from month 0, 1 from month 4, 1 from month 6, 85 % efficiency from month 6"),
                   (6, "3 from month 0, 1 from month 4, 2 from month 6, 85 % efficiency from month 6")):
        k = f"{n}d-lean"
        rows.append([f"{n}: {txt}", ml(k, "M-PP"), ml(k, "M2"), ml(k, "M6")])
    return md_table(["Team (lean scope)", "M-PP", "M2", "M6"], rows)


NLNET = [
    ("NL-1", 1, "Derivation engine: connectivity, potentials, wire properties, cross-references and interruption points",
     ["WP-S1.07", "WP-S1.08"], "AC-05, AC-09, AC-10, AC-12"),
    ("NL-2", 1, "SQL projection `api_v1` and the acceptance runner", ["WP-S1.10", "WP-S6.01"], "AC-15"),
    ("NL-3", 1, "`ew-memo` engine with oracle and the performance gate", ["WP-S1.05", "WP-S6.03"], "AC-16, AC-17"),
    ("NL-4", 1, "PDF/A-2b of the golden drawing set; published format specification and a permissive-only sample reader",
     ["WP-S3.04"], "AC-13, AC-14 (Linux), SC-11"),
    ("NL-5", 2, "Rule engine and `ew check` CLI with the MVP packs", ["WP-S1.15", "WP-S1.16a", "WP-S1.16", "WP-S1.22"],
     "AC-21"),
    ("NL-6", 2, "QET project import with lineage and migration report", ["WP-S3.09", "WP-S3.10", "WP-S3.11"],
     "AC-25, AC-26"),
    ("NL-7", 2, "QET corpus and migration verifier", ["WP-S3.13"], "AC-27, SC-4"),
    ("NL-8", 3, "AML AR APC, TIA tag tables and PLCopen XML", ["WP-S3.16"], "AC-30"),
    ("NL-9", 3, "EDZ and EPLAN parts XML import; DXF and DWG exchange", ["WP-S3.18", "WP-S3.19"], "AC-37 (DXF), AC-38"),
    ("NL-10", 3, "Machine export profiles and the production package", ["WP-S5.04", "WP-S5.07"], "AC-35"),
]


def b_nlnet():
    res = run("3d-lean")
    rows = []
    for i, p, d, ids, ev in NLNET:
        fins = sorted(math.ceil(res.finish[k] * 12 / 52 - 1e-9) for k in ids)
        mo = str(fins[-1]) if fins[0] == fins[-1] else f"{fins[0]}–{fins[-1]}"
        rows.append([i, p, d, ", ".join(ids), ev, mo])
    return md_table(["ID", "Proposal", "Deliverable", "WPs", "Evidence", "Baseline month (WPs finish)"], rows)


# ------------------------------------------------------------------ budget (01 §13.5)
BUDGET = {   # every amount is an estimate in euro, excluding VAT, to be verified before it is relied on
    "loaded_cost_per_dev_year": 100_000,    # illustrative; the employer replaces it with its own figure
    "ai_tooling_per_dev_month": {1.0: 50, 1.4: 200, 1.8: 400},
    "counsel_one_off": 20_000,              # LG-1 statement, ADR-0025 wording, ADR-0030 and CRA scope
    "counsel_per_year": 3_000,              # after M1: contribution questions, CRA updates
    "trademark": 4_500,                     # clearance search, EU word mark classes 9 and 42, attorney
    "signing_per_year": 250,                # Artifact Signing plan and Apple Developer Program
    "ci_hosting_per_year": 1_500,           # runners beyond the free tier, Weblate, domain, website
    "test_hardware": 6_000,                 # Windows, macOS and Linux test machines and a performance runner
    "test_hardware_refresh_years": 3,
    "test_hardware_refresh": 2_000,
    "usability": 6_000,                     # R1 and R2 participants, optional external moderator
    "external_acceptance": 5_000,           # partner-shop time, TIA Portal access for SC-7 and SC-8
    "stichting": 2_000,                     # notary and first-year bookkeeping (LG-6, phase 2)
    "pilot_hours_per_month": 6,             # reviews, demos, golden acknowledgements, test sessions (in kind)
    "pilot_rate_per_hour": 90,
    "wbso_share_of_staff_cost": 0.25,       # about 36 % of wage cost on R&D hours, wage about 70 % of loaded cost
    "nlnet_per_proposal": 30_000,           # typical first-proposal range EUR 5k-50k; upside only
    "nlnet_proposals": 3,
}
BUDGET_SCEN = [("3d-lean", "3 devs (baseline)"), ("3d-lean-ai-moderate", "3 devs + AI-moderate"),
               ("3d-lean-ai-strong", "3 devs + AI-strong"), ("4d-lean", "4 devs"), ("2d-lean", "2 devs")]


def budget(key):
    s, B = SCENARIOS[key], BUDGET
    n6 = m(key, "M6")
    dev_months = sum(max(0, n6 - j) for j in s["joins"])
    dev_months_m2 = sum(max(0, m(key, "M2") - j) for j in s["joins"])
    years = n6 / 12
    lines = [
        ("Developer staff to 1.0 (developer-years × loaded cost)", dev_months / 12 * B["loaded_cost_per_dev_year"]),
        ("AI tooling (subscriptions or API use per developer-month)",
         dev_months * B["ai_tooling_per_dev_month"][s["ai"]]),
        ("Counsel: LG-1, ADR-0025, ADR-0030, CRA scope (one-off)", B["counsel_one_off"]),
        ("Counsel after M1 (per year)", max(0, n6 - m(key, "M1")) / 12 * B["counsel_per_year"]),
        ("Trademark: clearance search, EU word mark (classes 9, 42), attorney", B["trademark"]),
        ("Code signing: Artifact Signing and Apple Developer Program (per year from enrolment)",
         years * B["signing_per_year"]),
        ("CI runners beyond the free tier, Weblate, website (per year)", years * B["ci_hosting_per_year"]),
        ("Test hardware: three OSes and a performance runner, refreshed every 3 years",
         B["test_hardware"] + int(years // B["test_hardware_refresh_years"]) * B["test_hardware_refresh"]),
        ("Usability rounds R1 and R2: participants, optional moderator", B["usability"]),
        ("External acceptance: partner-shop time, TIA Portal access", B["external_acceptance"]),
        ("Stichting founding (LG-6, phase 2)", B["stichting"]),
        ("Pilot time, in kind (hours per month × rate)", n6 * B["pilot_hours_per_month"] * B["pilot_rate_per_hour"]),
    ]
    gross = sum(v for _, v in lines)
    wbso = -B["wbso_share_of_staff_cost"] * lines[0][1]
    nl = -B["nlnet_per_proposal"] * B["nlnet_proposals"]
    return lines, gross, wbso, nl, dev_months, dev_months_m2


def eur(v):
    s = f"{abs(round(v / 1000)):,}".replace(",", " ")
    return ("−" if v < 0 else "") + f"€{s}k"


def b_budget():
    data = {k: budget(k) for k, _ in BUDGET_SCEN}
    names = [n for _, n in BUDGET_SCEN]
    rows = [["M6 1.0 at month"] + [str(m(k, "M6")) for k, _ in BUDGET_SCEN],
            ["Developer-years to 1.0 (to the MVP)"] + [f"{f1(data[k][4] / 12)} ({f1(data[k][5] / 12)})"
                                                     for k, _ in BUDGET_SCEN]]
    for i, (lab, _) in enumerate(data["3d-lean"][0]):
        rows.append([lab] + [eur(data[k][0][i][1]) for k, _ in BUDGET_SCEN])
    rows.append(["**Gross cost to 1.0**"] + [f"**{eur(data[k][1])}**" for k, _ in BUDGET_SCEN])
    rows.append(["WBSO offset (share of staff cost, **verify** rates with RVO)"] + [eur(data[k][2]) for k, _ in BUDGET_SCEN])
    rows.append(["**Net cost to 1.0, plan of record (no grants)**"] + [f"**{eur(data[k][1] + data[k][2])}**"
                                                                    for k, _ in BUDGET_SCEN])
    rows.append([f"NLnet, if {BUDGET['nlnet_proposals']} proposals are granted at {eur(BUDGET['nlnet_per_proposal'])} "
                 "each (upside, not planned)"] + [eur(data[k][3]) for k, _ in BUDGET_SCEN])
    rows.append(["Net cost with NLnet"] + [eur(data[k][1] + data[k][2] + data[k][3]) for k, _ in BUDGET_SCEN])
    params = md_table(["Parameter", "Value used (estimate, **verify**)"], [
        ["Loaded cost per developer-year (salary, employer charges, workplace, overhead)",
         eur(BUDGET["loaded_cost_per_dev_year"]) + " (illustrative; the employer replaces it)"],
        ["AI tooling per developer-month: baseline / AI-moderate / AI-strong",
         " / ".join(f"€{BUDGET['ai_tooling_per_dev_month'][x]}" for x in (1.0, 1.4, 1.8))],
        ["Pilot time", f"{BUDGET['pilot_hours_per_month']} h per month at €{BUDGET['pilot_rate_per_hour']} per hour"],
        ["WBSO", f"{round(BUDGET['wbso_share_of_staff_cost'] * 100)} % of staff cost (about 36 % of the wage cost of "
                 "R&D hours in the first bracket, wage about 70 % of loaded cost)"],
        ["Staff period", "Each developer from the join month to the M6 month of the scenario; no cost after 1.0"],
    ])
    return params + "\n\n" + md_table(["Cost line (euro, excluding VAT; all amounts are estimates to verify)"] + names,
                                      rows)


def b_headline():
    full, lean = effort("full"), effort("lean")
    path, fin, _, _ = crit("lean", None, "M6")
    return (f"**The headline.** The full v1 scope is **{f1(full)} person-weeks** (pw) of estimated work, "
            f"**{f1(full * 1.2)} pw** with the 20 % integration and bug-fix reserve: {f1(full / 36)} productive "
            f"developer-years, {f1(full * 1.2 / 36)} with the reserve. The lean scope of record is {f1(lean)} pw "
            f"({f1(lean * 1.2)} pw with the reserve). Three developers deliver 108 productive pw a year. **The "
            "architecture's indicative 15–18 months to 1.0 cannot be met by 2–4 developers.** With three developers, "
            f"month {m('3d-lean', 'M1')} is when the spine exits (headless golden PDF, M1). No team size reaches 1.0 in "
            f"18 months, because the longest dependency chain alone takes {f1(fin * 12 / 52)} months.")


def b_movers():
    base = run("3d-lean").ms
    imp = impacts()
    cases = [("AI-moderate (internal target, §3.7)", run("3d-lean-ai-moderate").ms),
             ("Fourth developer from month 4 (§10.3)", run("4d-lean").ms),
             ("Effort +10 % (estimates optimistic)", imp["Effort +10 %"]),
             ("Capacity −20 %", run("3d-lean-cap80").ms),
             ("Third developer joins at month 6", imp["Third developer joins at month 6"]),
             ("Strict stream silos instead of the pull rule (§10.2)",
              imp["Strict roles: dev 1 S0 and S1; dev 2 S2, S4, S6; dev 3 S3, S5, S6, S7"]),
             ("Full scope instead of the levers (PD-1)", run("3d-full").ms)]

    def d(a, b):
        x = a - b
        return "±0" if x == 0 else (f"+{x}" if x > 0 else f"−{-x}")
    rows = [[n] + [f"{v[ms]} ({d(v[ms], base[ms])})" for ms in ("M-PP", "M2", "M6")] for n, v in cases]
    return md_table([f"Change against the plan of record (M-PP {base['M-PP']}, M2 {base['M2']}, M6 {base['M6']})",
                     "M-PP month (change)", "M2 month (change)", "M6 month (change)"], rows)


def b_pilot_timeline():
    res = run("3d-lean")
    ms = res.ms
    r0, r1_ = r0_r1(res)
    qet = math.ceil(res.finish["WP-S3.13"] * 12 / 52 - 1e-9)
    rows = [
        [mlabel(ms["M1"]), "M1", "The reference set as a PDF generated from the model; nightly `ew` CLI builds"],
        [mlabel(ms["M-PP"]), "M-PP, release 0.0.x",
         "**Pilot preview**: draws new projects interactively (place, wire, terminals, cables, title block, core "
         "reports, PDF, online checks, save and load); unsigned, pilot only"],
        [mlabel(math.ceil(r0 * 12 / 52 - 1e-9)), "R0", "Formative usability round on the preview build"],
        [mlabel(math.ceil(r1_ * 12 / 52 - 1e-9)), "R1", "Summative usability round on the MVP release candidate"],
        [mlabel(ms["M2"]), "M2, release 0.1.0", "Signed release for new projects on three OSes"],
        [mlabel(qet), f"release 0.{qet - ms['M2'] + 1}.0", "Migration of existing QET projects (lever b)"],
        [mlabel(ms["M6"]), "M6, release 1.0", "PLC exchange, 3D layout, wire lengths and machine files"],
    ]
    return md_table(["Month (plan of record)", "Milestone or event", "What the pilot gets"], rows)


# ------------------------------------------------------------------ blocks for 02
def b_wbs_key_results():
    full, lean = effort("full"), effort("lean")
    back_full = sum(pert(r) for r in ROWS if r["milestone"] == "1.x")
    sig = math.sqrt(sum(((float(r["effort_pess_pw"]) - float(r["effort_opt_pw"])) / 6) ** 2
                        for r in ROWS if r["milestone"] != "1.x"))
    n10 = sum(1 for r in ROWS if r["milestone"] != "1.x")
    path, fin, _, _ = crit("lean", None, "M6")
    return "\n".join([
        "> **Key results**",
        f"> - **Effort.** The MVP and 1.0 scope is {n10} WPs with a PERT mean of **{f1(full)} person-weeks** (pw), σ ≈",
        f">   {f1(sig)} pw, or **{f1(full * 1.2)} pw** with the 20 % integration and bug-fix reserve. The MVP alone is "
        f"{f1(effort('full', 'M2'))} pw",
        f">   ({f1(effort('full', 'M2', reserve=True))} pw with the reserve). The 1.x backlog adds {f1(back_full)} pw. "
        f"1.0 is about {f1(full / 36)} productive",
        ">   developer-years.",
        f"> - **The full v1 scope does not fit 15–18 months.** Three developers have about 162 pw in 18 months;",
        f">   1.0 needs {f1(full * 1.2 / 162)} times that. No team size reaches 1.0 before month {math.ceil(fin * 12 / 52)}, "
        "because the longest",
        ">   dependency chain alone takes that long (§6).",
        f"> - **Realistic dates, full scope, base capacity.** 3 developers: spine exit (M1) at month {ml('3d-full', 'M1')}, "
        f"MVP at month {ml('3d-full', 'M2')}, 1.0 at month {ml('3d-full', 'M6')}. 2 developers: MVP at month "
        f"{ml('2d-full', 'M2')}, 1.0 at month {ml('2d-full', 'M6')}. 4 developers: MVP at month {ml('4d-full', 'M2')}, "
        f"1.0 at month {ml('4d-full', 'M6')}.",
        f"> - **Levers.** Deferring the MVP's Should items, moving the QET migration to the 0.x release after the MVP and",
        f">   moving 13 1.0 WPs to 1.x (the `milestone_lean` column) brings 3 developers to the MVP at month "
        f"{ml('3d-lean', 'M2')} and 1.0 at month {ml('3d-lean', 'M6')}. Even with every WP at its optimistic estimate "
        f"and 20 % more capacity, that lean scope lands at month {m('3d-lean-best', 'M2')} (MVP) and month "
        f"{m('3d-lean-best', 'M6')} (1.0). §7.4 recommends a combination.",
        f"> - **Pilot preview.** {sum(1 for r in ROWS if r['milestone'] == 'M-PP')} WPs, two of them slices split off "
        f"MVP WPs, form milestone M-PP (release 0.0.x, pilot only): interactive 2D drawing of new projects at month "
        f"{ml('3d-lean', 'M-PP')} with three developers, {m('3d-lean', 'M2') - m('3d-lean', 'M-PP')} months before "
        "the MVP.",
    ])


def b_wbs_ms_months():
    rows = []
    for ms in SHOWN[2:] if False else ["M0", "M1", "M-PP", "M2", "M3", "M4", "M5", "M6"]:
        rows.append([ms, MS_NAMES[ms], GATE[ms], ml("3d-full", ms), ml("3d-lean", ms)])
    return md_table(["ID", "Name", "Gate", "Realistic month, 3 developers, full scope",
                     "Realistic month, 3 developers, lean scope (plan of record)"], rows)


def b_wbs_capacity():
    rows = []
    for n, (joins, eff, name) in TEAMS.items():
        per = n * 3 * (eff if n == 4 else 1)
        rows.append([str(n), f1(per), f"{per * 15:.0f} pw" if n != 4 else f"{9 * 4 + 10.8 * 11:.0f} pw",
                     f"{per * 18:.0f} pw" if n != 4 else f"{9 * 4 + 10.8 * 14:.0f} pw",
                     f"{math.ceil(capacity_bound(effort('full', 'M2', reserve=True), joins, eff))}",
                     f"{math.ceil(capacity_bound(effort('full', 'M6', reserve=True), joins, eff))}"])
    return md_table(["Team", "Capacity per month (pw, steady state)", "15 months", "18 months",
                     "Months for MVP effort + reserve", "Months for 1.0 effort + reserve"], rows)


def b_wbs_dates_full():
    keys = [f"{n}d-full" for n in TEAMS]
    names = ["2 developers", "3 developers (baseline)", "4 developers"]
    return dates_table(keys, names)


def b_wbs_larger():
    rows = []
    for n, prof in ((3, "3 from month 0"), (4, "3 from month 0, 1 from month 4, 90 % efficiency from month 4"),
                    (6, "3 from month 0, 1 from month 4, 2 from month 6, 85 % efficiency from month 6"),
                    (8, "3 from month 0, 1 from month 4, 4 from month 6, 85 % efficiency from month 6"),
                    (10, "3 from month 0, 1 from month 4, 6 from month 6, 85 % efficiency from month 6")):
        rows.append([str(n), prof, ml(f"{n}d-full", "M1"), ml(f"{n}d-full", "M2"), ml(f"{n}d-full", "M6"),
                     ml(f"{n}d-lean", "M6")])
    return md_table(["Team", "Profile", "M1", "M2 MVP", "M6 1.0", "M6 with the lean scope"], rows)


def b_wbs_critical():
    path, fin, base, f = crit("lean", None, "M6")
    wp = wps("lean")
    L = ["```mermaid", "flowchart LR"]
    for i, k in enumerate(path):
        t = ROWD[k]["title"]
        t = t if len(t) <= 34 else t[:32].rsplit(" ", 1)[0] + " …"
        L.append(f'  n{i}["{k}<br/>{t}<br/>{round(base[k])} wk · {wp[k]["ms"]}"]')
    for i in range(len(path) - 1):
        L.append(f"  n{i} --> n{i + 1}")
    L.append("```")
    rows = []
    for i, k in enumerate(path):
        rows.append([i + 1, k, ROWD[k]["title"], wp[k]["ms"], f1(pert(ROWD[k])), f1(base[k]), f1(f(k) * 12 / 52)])
    # near-critical chains and float
    p1, f1_, _, _ = crit("lean", None, "M1")
    p2, f2_, _, _ = crit("lean", None, "M2")
    pp, fpp, _, _ = crit("lean", None, "M-PP")
    succ = defaultdict(list)
    for k, w in wp.items():
        for d in w["deps"]:
            succ[d].append(k)
    late = {}

    def latest(k):
        if k not in late:
            late[k] = min([latest(s) - base[s] for s in succ[k]] or [fin])
        return late[k]
    flt = {k: latest(k) - f(k) for k in ("WP-S4.03", "WP-S5.01", "WP-S3.16")}
    text = "\n".join(L) + "\n\n" + md_table(["#", "WP", "Title", "Milestone (lean)", "PERT (pw)",
                                             "Duration with reserve (weeks)", "Finishes at month"], rows)
    text += ("\n\n**Near-critical chains** (lean scope, unlimited developers). "
             f"To M1: {chain_text(p1)}, {f1(f1_ * 12 / 52)} months. "
             f"To M-PP: {chain_text(pp)}, {f1(fpp * 12 / 52)} months. "
             f"To M2: {chain_text(p2)}, {f1(f2_ * 12 / 52)} months.\n\n"
             f"**Float** to the end of the chain: the STEP worker (WP-S4.03) about {round(flt['WP-S4.03'])} weeks, the "
             f"production wire records (WP-S5.01) about {round(flt['WP-S5.01'])} weeks and the PLC exchange "
             f"(WP-S3.16) about {round(flt['WP-S3.16'])} weeks.")
    return text


def lever_scope(levers, to_1x=()):
    """WP dict with only the named levers applied (and optionally further WPs moved to 1.x)."""
    rows = [dict(r) for r in ROWS]
    for r in rows:
        if r["lever"] and r["lever"] not in levers:
            r["milestone_lean"] = r["milestone"]
        if r["wp_id"] in to_1x:
            r["milestone_lean"] = "1.x"
    return engine.load("lean", rows)


def b_descoping():
    steps = [("DS-1", "Lever (a)", "a", ()), ("DS-2", "Lever (b)", "ab", ()), ("DS-3", "Lever (c)", "abc", ()),
             ("DS-6", "3D, routing and lengths chain to 1.1", "abc", tuple(f"WP-S4.0{i}" for i in range(3, 9)))]
    rows, prev = [], engine.simulate(lever_scope(""), [0, 0, 0]).ms
    prev_wp = lever_scope("")
    for key, name, lev, extra in steps:
        wp = lever_scope(lev, extra)
        r = engine.simulate(wp, [0, 0, 0]).ms
        moved_mvp = sum(w["mean"] for k, w in prev_wp.items() if RANK[w["ms"]] <= RANK["M2"]) - \
            sum(w["mean"] for k, w in wp.items() if RANK[w["ms"]] <= RANK["M2"])
        moved_10 = sum(w["mean"] for w in prev_wp.values()) - sum(w["mean"] for w in wp.values())
        rows.append([key, name, f1(moved_mvp), f1(moved_10), f"{prev['M2']} → {r['M2']}", f"{prev['M6']} → {r['M6']}"])
        prev, prev_wp = r, wp
    return md_table(["Step", "Option", "PERT pw out of the MVP", "PERT pw out of 1.0", "M2 (3 developers)",
                     "M6 (3 developers)"], rows)


def b_wbs_lever_dates():
    sc = lever_scope
    out = []
    for name, lev in (("Full scope", ""), ("Lever (a): MVP Should deferral", "a"),
                      ("Levers (a) and (b): plus MVP split", "ab"), ("Lever (c) only: 1.0-lean", "c"),
                      ("Lean: all three levers", "abc")):
        wp = sc(lev)
        e2 = sum(w["mean"] for w in wp.values() if RANK[w["ms"]] <= RANK["M2"]) * 1.2
        e6 = sum(w["mean"] for w in wp.values()) * 1.2
        cells = [name, f"{e2:.0f} pw", f"{e6:.0f} pw"]
        for n, (joins, eff, _) in TEAMS.items():
            r = engine.simulate(wp, joins, eff).ms
            cells.append(f"{r['M2']} / {r['M6']}")
        out.append(cells)
    return md_table(["Scope", "Effort to MVP (mean + reserve)", "Effort to 1.0 (mean + reserve)", "2 devs M2 / M6",
                     "3 devs M2 / M6", "4 devs M2 / M6"], out)


# ------------------------------------------------------------------ optional blocks for other plan documents
# Any plan document may place these markers; schedule.py then fills them (and --check verifies them).
GATES = [("G-SP", "G-SP"), ("G-M0", "M0"), ("G-M1", "M1"), ("G-PP", "M-PP"), ("G-M2", "M2"), ("G-M3", "M3"),
         ("G-M4", "M4"), ("G-M5", "M5"), ("G-M6", "M6")]


def b_gate_timing():
    keys = [("2d-lean", "2 developers"), ("3d-lean", "**3 developers (baseline)**"),
            ("4d-lean", "4 developers (fourth from month 4)"), ("3d-lean-ai-moderate", "3 + AI-moderate (internal)"),
            ("3d-lean-ai-strong", "3 + AI-strong")]
    rows = []
    for g, ms in GATES:
        rows.append([f"**{g}**"] + [(f"**{ml(k, ms)}**" if k == "3d-lean" else ml(k, ms)) for k, _ in keys])
    return md_table(["Gate"] + [n for _, n in keys], rows)


def b_gate_sens_3d():
    rows = []
    for g, ms in GATES:
        rows.append([f"**{g}**", ml("3d-lean-cap80", ms), ml("3d-lean", ms), ml("3d-lean-cap120", ms), ml("3d-full", ms)])
    return md_table(["Gate", "−20 % capacity", "Base", "+20 % capacity", "Full scope, base"], rows)


def b_gate_sens_2d4d():
    rows = []
    for g, ms in GATES:
        rows.append([f"**{g}**"] + [ml(f"{n}d-lean{x}", ms) for n in (2, 4) for x in ("-cap80", "", "-cap120")])
    return md_table(["Gate", "2 developers: −20 %", "base", "+20 %", "4 developers: −20 %", "base", "+20 %"], rows)


def b_gate_full_2d4d():
    rows = [[f"**{g}**", ml("2d-full", ms), ml("4d-full", ms)] for g, ms in GATES]
    return md_table(["Gate", "2 developers, full scope", "4 developers, full scope"], rows)


def b_velocity_zones():
    rows = []
    for ratio in (1.5, 1.25, 1.1, 1.0, 0.9, 0.8, 2 / 3):
        r = run_custom("lean", cap=ratio).ms
        rows.append([f"{ratio:.2f}", f"{(1 / ratio - 1) * 100:+.0f} %".replace("-", "−"), ml_(r, "M1"), ml_(r, "M-PP"), ml_(r, "M2"),
                     ml_(r, "M6")])
    return md_table(["Velocity ratio (every WP)", "Effort change", "G-M1", "G-PP", "G-M2", "G-M6"], rows)


def ml_(r, ms):
    return mlabel(r[ms])


OPTIONAL = {"gate-timing": b_gate_timing, "gate-sensitivity-3d": b_gate_sens_3d,
            "gate-sensitivity-2d4d": b_gate_sens_2d4d, "gate-full-2d4d": b_gate_full_2d4d,
            "velocity-zones": b_velocity_zones}


# ------------------------------------------------------------------ main
def blocks():
    p01, p02 = PLAN / "01-global-plan.md", PLAN / "02-work-packages.md"
    return {
        p01: {
            "summary-headline": b_headline(), "summary-dates": b_summary(), "summary-movers": b_movers(),
            "pilot-timeline": b_pilot_timeline(), "team-scenarios": b_team_scenarios(),
            "capacity-bound": b_capacity_bound(), "dates-lean": b_dates("lean"), "dates-full": b_dates("full"),
            "sensitivity": b_sensitivity(), "gantt-teams": b_gantt_teams(), "ai-scenarios": b_ai(),
            "phases": b_phases(), "pp-scope": b_pp_scope(), "release-train": b_release_train(), "betas": b_betas(),
            "themes-1x": b_themes(), "gantt-baseline": b_gantt_baseline(), "cross-stream": b_cross_stream(),
            "critical-path": b_critical(), "impacts": b_impacts(), "silos": b_silos(), "dev4-join": b_dev4(),
            "beyond-four": b_beyond(), "nlnet": b_nlnet(), "budget": b_budget(), "descoping": b_descoping(),
        },
        p02: {
            "wbs-key-results": b_wbs_key_results(), "wbs-milestone-months": b_wbs_ms_months(),
            "wbs-capacity": b_wbs_capacity(), "wbs-dates-full": b_wbs_dates_full(),
            "wbs-sensitivity": b_sensitivity(with_calendar=False), "wbs-larger-teams": b_wbs_larger(),
            "wbs-critical-path": b_wbs_critical(), "wbs-lever-dates": b_wbs_lever_dates(),
        },
        **optional_blocks(),
        PLAN / "milestones.csv": milestones_csv(),
        PLAN / "wp-schedule.csv": wp_schedule_csv(),
    }


def optional_blocks():
    """Optional blocks found in the other plan documents (03, 04, 06 ...)."""
    out = {}
    for p in sorted(PLAN.glob("*.md")):
        if p.name in ("01-global-plan.md", "02-work-packages.md", "05-traceability.md"):
            continue
        text = p.read_text(encoding="utf-8")
        found = {n: f() for n, f in OPTIONAL.items() if f"<!-- generated:{n} -->" in text}
        if found:
            out[p] = found
    return out


if __name__ == "__main__":
    if "--print" in sys.argv:
        for k in SCENARIOS:
            print(f"{k:28}", " ".join(f"{x}={run(k).ms[x]}" for x in SHOWN))
        sys.exit(0)
    if "--block" in sys.argv:
        name = sys.argv[sys.argv.index("--block") + 1]
        for f, bl in blocks().items():
            if isinstance(bl, dict) and name in bl:
                print(bl[name])
        sys.exit(0)
    sys.exit(write_or_check(blocks(), "--check" in sys.argv, TOOL))
