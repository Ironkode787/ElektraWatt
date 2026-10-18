#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Effort totals and CSV consistency checks of the ElektraWatt plan (Python 3 standard library only).

Reads docs/plan/work-packages.csv. Writes the effort tables (generated blocks) of docs/plan/02-work-packages.md
and docs/plan/01-global-plan.md, and checks that
  - the CSV is well-formed: milestones, lean milestones, levers, releases, dependencies and lagged dependencies;
  - every WP of the CSV has a section in 02 §9 whose effort, milestone, lean milestone, dependencies and
    requirement count equal the CSV, and every section has a CSV row;
  - the lever tables of 02 §7 show the CSV's PERT values.

Run from anywhere:
  python3 -I docs/plan/tools/totals.py          # rewrite the generated blocks, then run the checks
  python3 -I docs/plan/tools/totals.py --check  # exit 1 if a block is stale or a check fails (for CI)
"""
import math, re, sys
from collections import defaultdict
from pathlib import Path

sys.dont_write_bytecode = True          # keep the repository free of __pycache__
sys.path.insert(0, str(Path(__file__).resolve().parent))
from wbs import (PLAN, MS, RANK, load_rows, pert, sigma, milestone, lagged, f1, md_table,  # noqa: E402
                 write_or_check)

TOOL = "totals.py"
DOCS = PLAN.parent
STREAMS = {"S0": "Project foundation and governance", "S1": "Core model and derivation", "S2": "Interaction and UI",
           "S3": "Documents, reports and migration", "S4": "Physical: 2D/3D layout, routing, thermal",
           "S5": "Manufacturing outputs", "S6": "Quality, security and release engineering",
           "S7": "Content: symbols, parts, golden project, docs"}
WBS_MS = {"M0": "M0 Kick-off and foundations", "M1": "M1 Spine exit: headless golden PDF",
          "M-PP": "M-PP Pilot preview 0.0.x", "M2": "M2 MVP: pilot release",
          "M3": "M3 Automation breadth: PLC, bus, fluid, exchange", "M4": "M4 Manufacturing and 2D/3D panel",
          "M5": "M5 1.0 feature complete (beta)", "M6": "M6 1.0 release", "1.x": "1.x backlog bucket (no milestone)"}
LEVER_MS = {"a": "M5", "b": "M3", "c": "1.x"}
RELEASE = {"M0": "MVP", "M1": "MVP", "M-PP": "MVP", "M2": "MVP", "M3": "1.0", "M4": "1.0", "M5": "1.0", "M6": "1.0",
           "1.x": "1.x"}

ROWS = load_rows()
ROWD = {r["wp_id"]: r for r in ROWS}


def in_10(r, scope):
    return milestone(r, scope) != "1.x"


# ------------------------------------------------------------------ 02 §4
def b_stream():
    rows, t = [], defaultdict(float)
    for s, name in STREAMS.items():
        rs = [r for r in ROWS if r["stream"] == s and in_10(r, "full")]
        o = sum(float(r["effort_opt_pw"]) for r in rs)
        l = sum(float(r["effort_likely_pw"]) for r in rs)
        p = sum(float(r["effort_pess_pw"]) for r in rs)
        mean = sum(pert(r) for r in rs)
        lean = sum(pert(r) for r in ROWS if r["stream"] == s and in_10(r, "lean"))
        for k, v in (("n", len(rs)), ("o", o), ("l", l), ("p", p), ("m", mean), ("lean", lean)):
            t[k] += v
        rows.append([s, name, len(rs), f1(o), f1(l), f1(p), f1(mean), f1(mean * 1.2), None, f1(lean)])
    for r in rows:
        r[8] = f"{round(100 * float(r[6]) / t['m'])} %"
    rows.append(["**All**", "**MVP and 1.0**", f"**{t['n']:.0f}**", f"**{f1(t['o'])}**", f"**{f1(t['l'])}**",
                 f"**{f1(t['p'])}**", f"**{f1(t['m'])}**", f"**{f1(t['m'] * 1.2)}**", "**100 %**",
                 f"**{f1(t['lean'])}**"])
    bx = [r for r in ROWS if r["milestone"] == "1.x"]
    rows.append(["1.x", f"Backlog bucket ({len(bx)} WPs, not in the totals above)", len(bx),
                 f1(sum(float(r["effort_opt_pw"]) for r in bx)), f1(sum(float(r["effort_likely_pw"]) for r in bx)),
                 f1(sum(float(r["effort_pess_pw"]) for r in bx)), f1(sum(pert(r) for r in bx)),
                 f1(sum(pert(r) for r in bx) * 1.2), "—", "—"])
    return md_table(["Stream", "Name", "WPs", "O", "L", "P", "PERT mean", "Mean + 20 % reserve", "Share of 1.0",
                     "PERT mean, lean scope"], rows)


def ms_table(scope):
    rows, cum, cvar = [], 0.0, 0.0
    for m in MS:
        rs = [r for r in ROWS if milestone(r, scope) == m]
        mean = sum(pert(r) for r in rs)
        var = sum(sigma(r) ** 2 for r in rs)
        sd = math.sqrt(var)
        o = sum(float(r["effort_opt_pw"]) for r in rs)
        l = sum(float(r["effort_likely_pw"]) for r in rs)
        p = sum(float(r["effort_pess_pw"]) for r in rs)
        name = WBS_MS[m].split(" ", 1)[1]
        rel = RELEASE[m]
        if m == "1.x":
            rows.append([m, name, rel, len(rs), f1(o), f1(l), f1(p), f1(mean), f1(sd), "—", "—", "—"])
            continue
        cum += mean
        cvar += var
        p80 = (cum + 0.84 * math.sqrt(cvar)) * 1.2
        rows.append([m, name, rel, len(rs), f1(o), f1(l), f1(p), f1(mean), f1(sd), f1(cum), f1(cum * 1.2), f1(p80)])
    return md_table(["Milestone", "Name", "Release", "WPs", "O", "L", "P", "PERT mean", "σ", "Cumulative mean",
                     "Cumulative + 20 % reserve", "Cumulative P80 + reserve"], rows)


def b_matrix():
    cols = MS
    rows, tot = [], defaultdict(float)
    for s in STREAMS:
        cells, st = [s], 0.0
        for m in cols:
            v = sum(pert(r) for r in ROWS if r["stream"] == s and r["milestone"] == m)
            tot[m] += v
            if m != "1.x":
                st += v
            cells.append(f1(v) if v else "—")
        cells.append(f1(st))
        rows.append(cells)
    rows.append(["**All**"] + [f"**{f1(tot[m])}**" for m in cols] +
                [f"**{f1(sum(v for m, v in tot.items() if m != '1.x'))}**"])
    return md_table(["Stream"] + cols + ["Total to 1.0"], rows)


def catalogue_rows():
    n = 0
    for f in ("02a-requirements-core.md", "02b-requirements-extended.md", "02c-nonfunctional-requirements.md"):
        seen = set()
        for m in re.finditer(r"^\| ((?:N?FR)-[A-Z0-9]+-\d+) \|(.*)$", (DOCS / "design" / f).read_text(encoding="utf-8"),
                             re.M):
            cells = [c.strip() for c in m.group(2).split("|")]
            if len(cells) >= 5 and cells[1] in ("Must", "Should", "Could") and m.group(1) not in seen:
                seen.add(m.group(1))
                if cells[2].startswith("MVP") or cells[2] == "1.0":
                    n += 1
    return n


def b_plausibility():
    full = sum(pert(r) for r in ROWS if in_10(r, "full"))
    n = catalogue_rows()
    spine = [r for r in ROWS if r["milestone"] in ("M0", "M1")]
    lane1 = sum(pert(r) for r in spine if r["wp_id"][:6] == "WP-S1." and int(r["wp_id"][6:8]) <= 10)
    lane2 = sum(pert(r) for r in spine if r["wp_id"][:6] == "WP-S3." and int(r["wp_id"][6:8]) <= 5)
    tot = sum(pert(r) for r in spine)
    return "\n".join([
        f"- **Per requirement.** The catalogues hold {n} rows released at MVP or 1.0. {f1(full)} pw over them is",
        f"  {full / n:.2f} pw, about {full / n * 5:.1f} productive days per row, each with an acceptance criterion, "
        "tests and documentation. For an ECAD application whose rows include an AML exchange with three-way merge, "
        "a STEP worker and negotiated-congestion routing, this is not slack.",
        "- **The spine.** Architecture §15.1 plans the spine as two developers for 12 weeks: 24 calendar person-weeks, "
        "about 17 productive pw under PA-3. The M0 and M1 WPs here total "
        f"{f1(tot)} pw: lane 1 (WP-S1.01 … WP-S1.10) is {f1(lane1)} pw, lane 2 (WP-S3.01 … WP-S3.05) is {f1(lane2)} pw, "
        f"and {f1(tot - lane1 - lane2)} pw are the CI, licensing, legal, signing enrolment, test kit, determinism "
        "tests, performance gate, golden data and starter symbols that the M1 exit criteria need but the lane table "
        "does not list. The lane table assumes roughly one week per crate-level component; the estimates here assume "
        "three to six.",
    ])


def slug(h):
    s = h.strip().lower()
    s = re.sub(r"[^\w\- ]", "", s)
    return s.replace(" ", "-")


def b_stream_table(s):
    text = (PLAN / "02-work-packages.md").read_text(encoding="utf-8")
    heads = dict(re.findall(r"^#### (WP-S\d\.\d\d[a-z]?) (.*)$", text, re.M))
    rows = []
    for r in ROWS:
        if r["stream"] != s:
            continue
        k = r["wp_id"]
        anchor = slug(f"{k} {heads.get(k, r['title'])}")
        ml = r["milestone_lean"]
        lean = "same" if ml == r["milestone"] else f"{ml} (lever {r['lever']})"
        rows.append([f"[{k}](#{anchor})", r["title"], r["milestone"], lean, f1(pert(r)), r["owner_role"]])
    return md_table(["WP", "Title", "Milestone", "Lean scope", "PERT (pw)", "Owner role"], rows)


# ------------------------------------------------------------------ 01 §3.1, §3.2
def b_effort_parts():
    lean = [r for r in ROWS if in_10(r, "lean")]
    tot = sum(pert(r) for r in lean)
    groups = [("Feature work, streams S1–S5", {"S1", "S2", "S3", "S4", "S5"},
               "Core model and derivation, UI, documents and migration, physical, manufacturing"),
              ("Quality, security and release engineering, S6", {"S6"}, None),
              ("Content and documentation, S7", {"S7"}, "Golden data, starter symbols and parts, user documentation and tutorials"),
              ("Foundation, governance, pilot and community, S0", {"S0"},
               "Repository, licences, legal gates, signing enrolment, docs site, funding, pilot programme")]
    rel = sum(pert(ROWD[k]) for k in ("WP-S6.06", "WP-S6.07", "WP-S6.08", "WP-S6.09", "WP-S6.10", "WP-S6.12"))
    rows = []
    for name, ss, txt in groups:
        v = sum(pert(r) for r in lean if r["stream"] in ss)
        if txt is None:
            txt = (f"Test kit, perf gate, fuzzing, privacy, packaging, signing, SBOM, CRA; release engineering and "
                   f"stabilisation {f1(rel)} pw (WP-S6.06 … WP-S6.10, WP-S6.12)")
        rows.append([name, f1(v), f"{100 * v / tot:.1f} %", txt])
    rows.append(["**Subtotal**", f"**{f1(tot)}**", "**100 %**", ""])
    rows.append(["Integration, hardening and bug-fix reserve (20 %)", f1(tot * 0.2), "", "GA-6"])
    rows.append(["**Total to 1.0**", f"**{f1(tot * 1.2)}**", "", ""])
    doc = sum(pert(ROWD[k]) for k in ("WP-S0.06", "WP-S7.05", "WP-S7.07"))
    pil = sum(pert(ROWD[k]) for k in ("WP-S0.08", "WP-S0.09", "WP-S2.12", "WP-S2.16", "WP-S6.11"))
    return (md_table(["Part", "pw", "Share", "Contains"], rows) +
            f"\n\nCross-cutting lines inside these totals: documentation {f1(doc)} pw (WP-S0.06, WP-S7.05, WP-S7.07, "
            f"plus the crate docs inside every WP); pilot, usability and external acceptance {f1(pil)} pw (WP-S0.08, "
            "WP-S0.09, WP-S2.12, WP-S2.16, WP-S6.11).")


def b_effort_scope():
    rows = []
    for scope, name in (("full", "Full scope"), ("lean", "**Lean: levers (a), (b) and (c) — plan of record**")):
        pp = sum(pert(r) for r in ROWS if milestone(r, scope) != "1.x" and RANK[milestone(r, scope)] <= RANK["M-PP"])
        mvp = sum(pert(r) for r in ROWS if milestone(r, scope) != "1.x" and RANK[milestone(r, scope)] <= RANK["M2"])
        v10 = sum(pert(r) for r in ROWS if in_10(r, scope))
        bx = sum(pert(r) for r in ROWS if milestone(r, scope) == "1.x")
        cells = [f1(pp), f1(pp * 1.2), f1(mvp), f1(mvp * 1.2), f1(v10), f1(v10 * 1.2), f1(bx)]
        if scope == "lean":
            cells = [f"**{c}**" for c in cells]
        rows.append([name] + cells)
    return md_table(["Scope", "M-PP (PERT)", "M-PP with reserve", "MVP (PERT)", "MVP with reserve", "1.0 (PERT)",
                     "1.0 with reserve", "1.x backlog (PERT)"], rows)


# ------------------------------------------------------------------ checks
def expand(tok):
    m = re.fullmatch(r"((?:N?FR)-[A-Z0-9]+)-(\d+)\.\.(?:(?:N?FR)-[A-Z0-9]+-)?(\d+)", tok)
    if not m:
        return [tok]
    w = len(m.group(2))
    return [f"{m.group(1)}-{n:0{w}d}" for n in range(int(m.group(2)), int(m.group(3)) + 1)]


def checks():
    errs = []
    ids = set(ROWD)
    if len(ids) != len(ROWS):
        errs.append("duplicate wp_id in the CSV")
    for r in ROWS:
        k = r["wp_id"]
        if not re.fullmatch(r"WP-S\d\.\d\d[a-z]?", k):
            errs.append(f"{k}: malformed WP ID")
        for col in ("milestone", "milestone_lean"):
            if r[col] not in RANK:
                errs.append(f"{k}: {col} '{r[col]}' is not one of {', '.join(MS)}")
        lev = r["lever"]
        if lev and lev not in LEVER_MS:
            errs.append(f"{k}: lever '{lev}' is not a, b or c")
        want = LEVER_MS[lev] if lev else r["milestone"]
        if r["milestone_lean"] != want:
            errs.append(f"{k}: milestone_lean {r['milestone_lean']} but lever '{lev}' implies {want}")
        if r["release"] != RELEASE.get(r["milestone"]):
            errs.append(f"{k}: release {r['release']} does not match milestone {r['milestone']}")
        for d in r["depends_on"].split() + list(lagged(r)):
            if d not in ids:
                errs.append(f"{k}: dependency {d} names no WP")
        for d in lagged(r):
            if d not in r["depends_on"].split():
                errs.append(f"{k}: lagged dependency {d} is not also in depends_on")
        o, l, p = (float(r[c]) for c in ("effort_opt_pw", "effort_likely_pw", "effort_pess_pw"))
        if not o <= l <= p:
            errs.append(f"{k}: estimates not ordered O <= L <= P")
    # 02 §9 sections
    text = (PLAN / "02-work-packages.md").read_text(encoding="utf-8")
    secs = re.split(r"^#### (WP-S\d\.\d\d[a-z]?) ", text, flags=re.M)
    found = {}
    for i in range(1, len(secs), 2):
        found[secs[i]] = secs[i + 1].split("\n#", 1)[0]
    for k in sorted(set(found) - ids):
        errs.append(f"02 §9 has a section for {k}, which the CSV lacks")
    for r in ROWS:
        k = r["wp_id"]
        b = found.get(k)
        if b is None:
            errs.append(f"02 §9 lacks a section for {k}")
            continue
        mm = re.search(r"\| Effort \(pw\) \| O ([\d.]+) · L ([\d.]+) · P ([\d.]+) · \*\*PERT ([\d.]+)\*\* \(σ ([\d.]+)\)", b)
        if not mm:
            errs.append(f"{k}: 02 effort line missing or malformed")
        else:
            o, l, p, pe, sd = mm.groups()
            if (float(o), float(l), float(p)) != tuple(float(r[c]) for c in
                                                       ("effort_opt_pw", "effort_likely_pw", "effort_pess_pw")):
                errs.append(f"{k}: 02 effort O/L/P {o}/{l}/{p} differs from the CSV")
            if pe != f1(pert(r)) or sd != f1(sigma(r)):
                errs.append(f"{k}: 02 PERT {pe} (σ {sd}) differs from the CSV {f1(pert(r))} (σ {f1(sigma(r))})")
        want = f"| Milestone · release | {WBS_MS[r['milestone']]} · {r['release']}"
        if r["lever"]:
            want += f"; lean scope: {r['milestone_lean']} (lever {r['lever']})"
        want += " |"
        if want not in b:
            errs.append(f"{k}: 02 milestone line should read '{want}'")
        dm = re.search(r"^\| Depends on \| (.*) \|$", b, re.M)
        dtext = dm.group(1) if dm else ""
        if "the CSV lists" in dtext:          # stabilisation WPs: prose rule, then the CSV's reduced list
            dtext = dtext.rsplit(":", 1)[1]
        mdeps = set(re.findall(r"WP-S\d\.\d\d[a-z]?", dtext))
        if mdeps != set(r["depends_on"].split()):
            errs.append(f"{k}: 02 'Depends on' {sorted(mdeps)} differs from the CSV {sorted(r['depends_on'].split())}")
        for d, lag in lagged(r).items():
            if f"{d} + {lag:g} months" not in (dm.group(1) if dm else ""):
                errs.append(f"{k}: 02 'Depends on' should state the lag '{d} + {lag:g} months'")
        rq = re.search(r"^\| Requirements \| .* \((\d+) rows?\) \|$", b, re.M)
        n = len([x for t in r["requirements"].split() for x in expand(t)])
        if rq and int(rq.group(1)) != n:
            errs.append(f"{k}: 02 requirement count {rq.group(1)} differs from the CSV ({n})")
    # lever tables of §7
    for mm in re.finditer(r"^\| (WP-S\d\.\d\d[a-z]?) \| [^|]* \| [^|]* \| ([\d.]+) \| (\d+) \|", text, re.M):
        k, pe = mm.group(1), mm.group(2)
        if k in ROWD and pe != f1(pert(ROWD[k])):
            errs.append(f"02 §7 lever table: {k} PERT {pe} differs from the CSV {f1(pert(ROWD[k]))}")
    return errs


def blocks():
    p01, p02 = PLAN / "01-global-plan.md", PLAN / "02-work-packages.md"
    b02 = {"wbs-totals-stream": b_stream(), "wbs-totals-milestone": ms_table("full"),
           "wbs-totals-milestone-lean": ms_table("lean"), "wbs-totals-matrix": b_matrix(),
           "wbs-plausibility": b_plausibility()}
    for s in STREAMS:
        b02[f"wbs-stream-{s}"] = b_stream_table(s)
    return {p01: {"effort-parts": b_effort_parts(), "effort-scope": b_effort_scope()}, p02: b02}


if __name__ == "__main__":
    check = "--check" in sys.argv
    if "--block" in sys.argv:
        name = sys.argv[sys.argv.index("--block") + 1]
        for f, bl in blocks().items():
            if name in bl:
                print(bl[name])
        sys.exit(0)
    rc = write_or_check(blocks(), check, TOOL)
    errs = checks()
    for e in errs:
        print(f"{TOOL}: {e}")
    if not errs:
        print(f"{TOOL}: CSV and WBS sections consistent ({len(ROWS)} WPs)")
    sys.exit(1 if rc or errs else 0)
