#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Shared helpers for the ElektraWatt plan tools (Python 3 standard library only).

Used by schedule.py and totals.py. Reads docs/plan/work-packages.csv; knows the milestone order, the calendar
convention of the plan (month n is labelled with the calendar month in which it ends; month 0 = 2026-11-01) and
the generated-block markers that the tools rewrite in the plan documents:

    <!-- generated:NAME -->
    ...table written by a tool...
    <!-- /generated:NAME -->

Nothing between the two markers may be edited by hand; `--check` fails if it differs from what the tool computes.
"""
import csv, math, re
from datetime import date, timedelta
from pathlib import Path

PLAN = Path(__file__).resolve().parents[1]
DOCS = PLAN.parent
CSV_PATH = PLAN / "work-packages.csv"

# Milestones in order. M-PP (pilot preview, release 0.0.x) sits between the spine exit and the MVP.
MS = ["M0", "M1", "M-PP", "M2", "M3", "M4", "M5", "M6", "1.x"]
RANK = {m: i for i, m in enumerate(MS)}
MS_NAMES = {
    "M0": "Kick-off and foundations", "M1": "Spine exit: headless golden PDF", "M-PP": "Pilot preview 0.0.x",
    "M2": "MVP: pilot release 0.1.0", "M3": "Automation breadth and migration", "M4": "Manufacturing and 2D/3D panel",
    "M5": "1.0 feature complete (beta)", "M6": "1.0 release", "1.x": "1.x backlog",
}
GATE = {"M0": "G-M0", "M1": "G-M1", "M-PP": "G-PP", "M2": "G-M2", "M3": "G-M3", "M4": "G-M4", "M5": "G-M5",
        "M6": "G-M6"}
START = date(2026, 11, 1)
PROD = 36 / 52          # productive pw per calendar week per developer (GA-3 / PA-3)
RESERVE = 0.20          # integration, hardening and bug-fix reserve (GA-6 / PA-8)
LOE = {"WP-S0.09", "WP-S6.09"}                     # level of effort: uses capacity, gates no milestone
STABILISATION = {"WP-S6.08": "M2", "WP-S6.12": "M5"}  # waits for every earlier WP up to this milestone
NO_GATE = {"WP-S0.07"}                               # funding work: never a stabilisation predecessor
MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]


def load_rows(path=CSV_PATH):
    with open(path, newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def pert(r, est="mean"):
    o, l, p = (float(r[k]) for k in ("effort_opt_pw", "effort_likely_pw", "effort_pess_pw"))
    if est == "o":
        return o
    return (o + 4 * l + p) / 6


def sigma(r):
    return (float(r["effort_pess_pw"]) - float(r["effort_opt_pw"])) / 6


def milestone(r, scope):
    """Milestone of a CSV row in the full or the lean scope."""
    if scope == "full":
        return r["milestone"]
    return r["milestone_lean"] or r["milestone"]


def lagged(r):
    """'WP-S0.05a+12' tokens -> {dep: months}."""
    out = {}
    for tok in r.get("lagged_deps", "").split():
        wp, lag = tok.rsplit("+", 1)
        out[wp] = float(lag)
    return out


def r1(x):
    """Round half up to one decimal, as the tables of the plan do."""
    return math.floor(x * 10 + 0.5 + 1e-9) / 10


def f1(x):
    return f"{r1(x):.1f}"


def label(n):
    """Calendar label of month n: the calendar month in which month n ends (month 15 -> 'Jan 2028')."""
    k = START.month - 1 + int(n) - 1
    return f"{MONTHS[k % 12]} {START.year + k // 12}"


def mlabel(n):
    return f"{n} ({label(n)})"


def week_date(weeks):
    """Calendar date of a point given in weeks after month 0."""
    return START + timedelta(days=round(weeks * 7))


def month_date(m):
    return START + timedelta(days=round(m * 365.25 / 12))


# ------------------------------------------------------------------ generated blocks
BLOCK_RE = re.compile(r"(<!-- generated:([A-Za-z0-9_.-]+) -->\n)(.*?)(<!-- /generated:\2 -->)", re.S)


def apply_blocks(text, blocks):
    """Replace the content of every known block; return (new_text, names_found)."""
    found = []

    def sub(m):
        name = m.group(2)
        if name not in blocks:
            return m.group(0)
        found.append(name)
        body = blocks[name].rstrip("\n") + "\n"
        return m.group(1) + body + m.group(4)

    return BLOCK_RE.sub(sub, text), found


def write_or_check(files_blocks, check, tool):
    """files_blocks: {Path: {block: text}} plus {Path: str} for whole generated files. Returns exit code."""
    stale, missing_any = [], False
    for path, content in files_blocks.items():
        old = path.read_text(encoding="utf-8") if path.exists() else ""
        if isinstance(content, str):
            new = content
        else:
            new, found = apply_blocks(old, content)
            missing = sorted(set(content) - set(found))
            if missing:
                print(f"{tool}: {path.relative_to(DOCS)} lacks block(s): {', '.join(missing)}")
                missing_any = True
                continue
        if new != old:
            stale.append(path)
            if not check:
                path.write_text(new, encoding="utf-8")
    if check:
        for p in stale:
            print(f"{tool}: {p.relative_to(DOCS)} is stale; run python3 -I docs/plan/tools/{tool}")
        if not stale and not missing_any:
            print(f"{tool}: up to date")
        return 1 if stale or missing_any else 0
    for p in stale:
        print(f"{tool}: wrote {p.relative_to(DOCS)}")
    if not stale:
        print(f"{tool}: nothing to change")
    return 1 if missing_any else 0


def md_table(header, rows):
    out = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
    for r in rows:
        out.append("| " + " | ".join(str(c) for c in r) + " |")
    return "\n".join(out)


def wp_ids_text(ids):
    """Compact list: 'WP-S1.01 … WP-S1.04' for consecutive runs of one stream (no suffix IDs)."""
    ids = list(ids)
    out, i = [], 0
    while i < len(ids):
        j = i
        while (j + 1 < len(ids) and re.fullmatch(r"WP-S\d\.\d\d", ids[j]) and re.fullmatch(r"WP-S\d\.\d\d", ids[j + 1])
               and ids[j][:5] == ids[j + 1][:5] and int(ids[j + 1][6:]) == int(ids[j][6:]) + 1):
            j += 1
        out.append(ids[i] if j - i < 2 else f"{ids[i]} … {ids[j]}")
        if 0 < j - i < 2:
            out[-1] = ids[i]
            out.extend(ids[i + 1:j + 1])
        i = j + 1
    return ", ".join(out)
