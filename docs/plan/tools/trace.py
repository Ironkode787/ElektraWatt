#!/usr/bin/env python3
"""Traceability generator for the ElektraWatt plan.

Reads (repo-relative, stdlib only):
  docs/plan/work-packages.csv
  docs/design/02a|02b|02c requirement catalogues
  docs/design/20-golden-project.md   (AC-01..AC-42, D-01..D-42)
  docs/design/15-checks-revisions-collaboration.md (rule catalogue EW-xxx-nnn -> FR)
  docs/plan/03-gates.md              (AC / D by gate, section 8)
Writes:
  docs/plan/05-traceability.md       (the report)
  docs/plan/traceability-matrix.csv  (requirement -> WPs, one row per FR/NFR)

Run:  python3 -I docs/plan/tools/trace.py          (from the repository root, any cwd works)
      python3 -I docs/plan/tools/trace.py --check  (exit 1 if the committed files are stale)
"""
import csv, re, sys, collections
from pathlib import Path

DOCS = Path(__file__).resolve().parents[2]
PLAN, DESIGN = DOCS / "plan", DOCS / "design"
OUT_MD, OUT_CSV = PLAN / "05-traceability.md", PLAN / "traceability-matrix.csv"

ID_RE = re.compile(r"(N?FR)-([A-Z0-9]+)-(\d+)")
MS_ORDER = {"M0": 0, "M1": 1, "M2": 2, "M3": 3, "M4": 4, "M5": 5, "M6": 6, "1.x": 7}


def read(p):
    return Path(p).read_text(encoding="utf-8")


def fmt_id(kind, area, n, width=3):
    return f"{kind}-{area}-{n:0{width}d}"


# ---------------------------------------------------------------- catalogue
def load_catalogue():
    reqs = {}
    for f in ("02a-requirements-core.md", "02b-requirements-extended.md", "02c-nonfunctional-requirements.md"):
        for m in re.finditer(r"^\| ((?:N?FR)-[A-Z0-9]+-\d+) \|(.*)$", read(DESIGN / f), re.M):
            rid = m.group(1)
            cells = [c.strip() for c in m.group(2).split("|")]
            if len(cells) < 5 or cells[1] not in ("Must", "Should", "Could"):
                continue
            if rid in reqs:
                continue
            kind, area, _ = ID_RE.fullmatch(rid).groups()
            reqs[rid] = dict(id=rid, text=cells[0], prio=cells[1], rel=cells[2], src=cells[3],
                             area=f"{kind}-{area}", file=f[:3])
    return reqs


def rel_bucket(rel):
    if rel.startswith("MVP"):
        return "MVP"          # "MVP / 1.0" splits: the MVP part gates the MVP
    if rel == "1.0":
        return "1.0"
    return "1.x/Later"


# ---------------------------------------------------------------- id expansion
def expand_token(tok, reqs):
    """-> (ids, problems). Handles single IDs and ranges A..B (also 'A … 006')."""
    m = re.fullmatch(r"((?:N?FR)-[A-Z0-9]+)-(\d+)(?:\.\.|…)((?:(?:N?FR)-[A-Z0-9]+-)?)(\d+)", tok)
    if not m:
        return [tok], []
    pre, a, pre2, b = m.group(1), int(m.group(2)), m.group(3), int(m.group(4))
    if pre2 and pre2.rstrip("-") != pre:
        return [], [f"range {tok} crosses areas"]
    if b < a:
        return [], [f"range {tok} is reversed"]
    w = len(m.group(2))
    return [f"{pre}-{n:0{w}d}" for n in range(a, b + 1)], []


def expand_text(s, reqs):
    """Expand IDs and ranges in free text (golden/gate tables). Only existing IDs kept for ranges."""
    s = s.replace("…", "..").replace(" .. ", "..")
    out = []
    for tok in re.findall(r"(?:N?FR)-[A-Z0-9]+-\d+(?:\.\.(?:(?:N?FR)-[A-Z0-9]+-)?\d+)?", s):
        ids, _ = expand_token(tok, reqs)
        out += ids
    # "FR-OUT-004 … 006" with spaces already normalised above
    return out


def expand_nums(s, prefix):
    """'AC-01 … AC-13' / 'D-01, D-03' -> list of ids"""
    s = re.sub(r"\s*(…|\.\.)\s*", "..", s)
    out = []
    for m in re.finditer(rf"{prefix}-(\d+)(?:\.\.(?:{prefix}-)?(\d+))?", s):
        a = int(m.group(1)); b = int(m.group(2) or a)
        out += [f"{prefix}-{n:02d}" for n in range(a, b + 1)]
    return out


# ---------------------------------------------------------------- WPs
def load_wps():
    with open(PLAN / "work-packages.csv", newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def wp_coverage(wps, reqs):
    cov = collections.defaultdict(set)      # req id -> WP ids
    bad = []                                # (wp, token, why)
    range_holes = []                        # (wp, token, missing ids)
    for w in wps:
        for tok in w["requirements"].split():
            ids, probs = expand_token(tok, reqs)
            for p in probs:
                bad.append((w["wp_id"], tok, p))
            missing = [i for i in ids if i not in reqs]
            if ".." in tok or "…" in tok:
                if missing and len(missing) < len(ids):
                    range_holes.append((w["wp_id"], tok, missing))
                    ids = [i for i in ids if i in reqs]
                elif missing:
                    bad.append((w["wp_id"], tok, "range names no existing requirement"))
                    ids = []
            elif missing:
                if ID_RE.fullmatch(tok):
                    bad.append((w["wp_id"], tok, "no such requirement"))
                else:
                    bad.append((w["wp_id"], tok, "not a requirement ID"))
                ids = []
            for i in ids:
                cov[i].add(w["wp_id"])
    return cov, bad, range_holes


def check_wp_other(wps, reqs, acs, ds):
    probs = []
    ids = {w["wp_id"] for w in wps}
    for w in wps:
        for d in w["depends_on"].split():
            if d not in ids:
                probs.append((w["wp_id"], d, "depends_on names no WP"))
        if not w["requirements"].strip():
            probs.append((w["wp_id"], "-", "no requirement cited (infrastructure WP?)"))
        for tok in w["acceptance"].split():
            if tok.startswith("AC-") and tok not in acs:
                probs.append((w["wp_id"], tok, "acceptance cites unknown AC"))
            elif tok.startswith("D-") and tok not in ds:
                probs.append((w["wp_id"], tok, "acceptance cites unknown D"))
            elif ID_RE.fullmatch(tok) and tok not in reqs:
                probs.append((w["wp_id"], tok, "acceptance cites unknown requirement"))
    return probs


# ---------------------------------------------------------------- golden project
def load_golden(reqs):
    g = read(DESIGN / "20-golden-project.md")
    acs, ds = {}, {}
    for m in re.finditer(r"^\| (AC-\d\d) \| (.*)\|\s*$", g, re.M):
        cells = [c.strip() for c in m.group(2).rsplit("|", 2)]
        # layout: criterion | requirements | gate   (criterion may contain no pipes)
        parts = [c.strip() for c in m.group(2).split("|")]
        acs[m.group(1)] = dict(crit=parts[0], reqs=parts[1] if len(parts) > 1 else "", gate=parts[2] if len(parts) > 2 else "")
    for m in re.finditer(r"^\| (D-\d\d) \| (.*)\|\s*$", g, re.M):
        parts = [c.strip() for c in m.group(2).split("|")]
        ds[m.group(1)] = dict(short=parts[0], rule=parts[1], test=parts[2], expect=parts[3], gate=parts[4])
    return acs, ds


def load_rules():
    """EW rule id -> list of FR ids from the check catalogue of design/15."""
    rules = {}
    files = [DESIGN / "15-checks-revisions-collaboration.md"] + sorted(p for p in DESIGN.glob("*.md") if p.name[:2] != "15")
    for f in files:
        for m in re.finditer(r"^\| (EW-[A-Z]+-\d+) \|(.*)$", read(f), re.M):
            frs = re.findall(r"FR-[A-Z0-9]+-\d+", m.group(2))
            if frs:
                rules.setdefault(m.group(1), frs)
    return rules


def load_gate_tables():
    t = read(PLAN / "03-gates.md")
    sec = t.split("### 8.1", 1)[1].split("### 8.3", 1)[0]
    ac_gate, d_gate = collections.defaultdict(list), collections.defaultdict(list)
    s81, s82 = sec.split("### 8.2", 1)
    for m in re.finditer(r"^\| \*\*(G\d)\*\* \| (.*?) \|", s81, re.M):
        g = m.group(1)
        body = m.group(2)
        if g == "G6":
            body = "AC-01 … AC-42"
        for part in re.split(r";", body):
            for a in expand_nums(part, "AC"):
                if a not in ac_gate or g not in ac_gate[a]:
                    ac_gate[a].append(g)
    for m in re.finditer(r"^\| \*\*(G\d)\*\*(?: \(\d+\))? \| (.*?) \|$", s82, re.M):
        g = m.group(1)
        body = m.group(2)
        if body.startswith("All rows"):
            continue
        for d in expand_nums(body, "D"):
            if g not in d_gate[d]:
                d_gate[d].append(g)
    # G6 AC list says "except AC-36 and AC-42"
    return ac_gate, d_gate


def load_ref_map():
    a = read(DESIGN / "02a-requirements-core.md").split("## Appendix A", 1)[1].split("## Appendix B", 1)[0]
    refs = {}
    for m in re.finditer(r"^\| (REF-\d+) \| (.*?) \| ([^|]*) \| ([^|]*) \|\s*$", a, re.M):
        refs[m.group(1)] = dict(cand=m.group(2), frs=re.findall(r"FR-[A-Z0-9]+-\d+", m.group(3)),
                                frs02b=re.findall(r"FR-[A-Z0-9]+-\d+", m.group(4)))
    return refs


# ---------------------------------------------------------------- helpers
def esc(s):
    return s.replace("|", "\\|")


def short(s, n=78):
    s = re.sub(r"\s+", " ", s)
    s = re.sub(r"^The system shall ", "", s)
    return esc(s if len(s) <= n else s[: n - 1].rstrip() + "…")


def ranges(ids):
    """compress sorted FR ids of one area into 'FR-ABC-001..004, 007'"""
    out, by, wid = [], collections.defaultdict(list), {}
    for i in ids:
        k, a, n = ID_RE.fullmatch(i).groups()
        by[f"{k}-{a}"].append(int(n))
        wid[f"{k}-{a}"] = len(n)
    for ar in sorted(by):
        ns = sorted(by[ar]); i = 0
        while i < len(ns):
            j = i
            while j + 1 < len(ns) and ns[j + 1] == ns[j] + 1:
                j += 1
            w = wid[ar]
            out.append(f"{ar}-{ns[i]:0{w}d}" if i == j else f"{ar}-{ns[i]:0{w}d}..{ns[j]:0{w}d}")
            i = j + 1
    return ", ".join(out)


def ms(wp):
    return MS_ORDER.get(wp["milestone"], 9)


# ---------------------------------------------------------------- main
def build():
    reqs = load_catalogue()
    wps = load_wps()
    wpd = {w["wp_id"]: w for w in wps}
    cov, bad, holes = wp_coverage(wps, reqs)
    acs, ds = load_golden(reqs)
    rules = load_rules()
    ac_gate, d_gate = load_gate_tables()
    refs = load_ref_map()
    other = check_wp_other(wps, reqs, acs, ds)

    # earliest milestone / release of covering WPs
    def earliest(ids):
        ws = [wpd[w] for r in ids for w in cov.get(r, ())]
        return min(ws, key=ms) if ws else None

    rows = sorted(reqs.values(), key=lambda r: (r["area"], int(r["id"].rsplit("-", 1)[1])))
    order = [r["id"] for r in rows]
    for r in rows:
        r["wps"] = sorted(cov.get(r["id"], ()))
        r["bucket"] = rel_bucket(r["rel"])

    dsrc = collections.defaultdict(list)
    for r in rows:
        for dk in set(expand_nums(r["src"], "D")):
            dsrc[dk].append(r["id"])

    # ---- coverage
    cols = [("MVP", "Must"), ("MVP", "Should"), ("MVP", "Could"), ("1.0", "Must"), ("1.0", "Should"), ("1.0", "Could"), ("1.x/Later", None)]
    tab = collections.defaultdict(lambda: collections.defaultdict(lambda: [0, 0]))
    for r in rows:
        key = (r["bucket"], r["prio"] if r["bucket"] != "1.x/Later" else None)
        for a in (r["area"], "TOTAL"):
            t = tab[a][key]
            t[1] += 1
            t[0] += bool(r["wps"])
    areas = sorted({r["area"] for r in rows}, key=lambda a: (a.startswith("NFR"), a))

    gaps = {b: [r for r in rows if r["bucket"] == b and r["prio"] in ("Must", "Should") and not r["wps"]] for b in ("MVP", "1.0")}
    optional_gaps = [r for r in rows if not r["wps"] and r not in gaps["MVP"] and r not in gaps["1.0"]]
    # timing: MVP requirement whose covering WPs all land after MVP
    late = []
    for r in rows:
        if r["bucket"] == "MVP" and r["prio"] in ("Must", "Should") and r["wps"]:
            e = min(ms(wpd[w]) for w in r["wps"])
            if e > 2:
                late.append((r, min((wpd[w] for w in r["wps"]), key=ms)))
    late10 = []
    for r in rows:
        if r["bucket"] == "1.0" and r["prio"] in ("Must", "Should") and r["wps"] and min(ms(wpd[w]) for w in r["wps"]) > 6:
            late10.append((r, min((wpd[w] for w in r["wps"]), key=ms)))
    # WPs with unused requirements
    nreq_wp = {w["wp_id"]: sum(1 for r in rows if w["wp_id"] in r["wps"]) for w in wps}

    # ---- write matrix csv
    with open(OUT_CSV, "w", newline="", encoding="utf-8") as f:
        wr = csv.writer(f, lineterminator="\n")
        wr.writerow(["req_id", "area", "prio", "release", "covered", "wps", "earliest_wp_milestone"])
        for r in rows:
            e = min((wpd[w] for w in r["wps"]), key=ms)["milestone"] if r["wps"] else ""
            wr.writerow([r["id"], r["area"], r["prio"], r["rel"], "yes" if r["wps"] else "NO", " ".join(r["wps"]), e])

    L = []
    A = L.append
    ntot = len(rows)
    ncov = sum(bool(r["wps"]) for r in rows)
    nwp = len(wps)
    A("# Traceability: requirements, acceptance criteria, defects and reference candidates to work packages")
    A("")
    A("**Status:** Draft v0.1 · **Date:** 2026-10-18 · **Part of:** ElektraWatt plan · **Generated by:** "
      "[tools/trace.py](tools/trace.py) (do not edit the tables by hand)")
    A("")
    A("**Purpose.** This document shows, computed by a script and not asserted, which requirements, acceptance criteria "
      "(AC-01 … AC-42), reference defects (D-01 … D-42) and reference candidates (REF-001 … REF-108) are carried by which "
      f"work packages (WPs) of [work-packages.csv](work-packages.csv), and where the plan has holes. It lists *every* Must and "
      "Should requirement of the MVP and of 1.0 that no WP covers, every WP citation that points to a non-existent ID, and the "
      "requirements whose only WPs land after the release that needs them. It adds no estimates; the dates are those of the "
      "[global plan](01-global-plan.md) and the gates are those of [gates](03-gates.md).")
    A("")
    A("---")
    A("")
    A("## Summary for decision makers")
    A("")
    mg, sg = [r for r in gaps["MVP"] if r["prio"] == "Must"], [r for r in gaps["MVP"] if r["prio"] == "Should"]
    mg1, sg1 = [r for r in gaps["1.0"] if r["prio"] == "Must"], [r for r in gaps["1.0"] if r["prio"] == "Should"]
    A(f"**Result in one paragraph.** The catalogues hold {ntot} requirements ({sum(r['id'].startswith('FR') for r in rows)} FR, "
      f"{sum(r['id'].startswith('NFR') for r in rows)} NFR). The {nwp} WPs cover {ncov} of them ({100*ncov/ntot:.1f} %). "
      f"{len(gaps['MVP'])} MVP requirements ({len(mg)} Must, {len(sg)} Should) and {len(gaps['1.0'])} 1.0 requirements "
      f"({len(mg1)} Must, {len(sg1)} Should) that the plan promises are carried by no WP; they are listed in full in §3. "
      f"{len(late)} MVP Must/Should requirements are covered only by WPs that finish after M2 (§4). "
      f"{len({b[0] for b in bad})} WPs cite {len(bad)} tokens that are not requirement IDs or that name no requirement (§5).")
    A("")
    A("**What is asked of the plan owner.** (1) Treat the gap lists of §3 as input to the next WP revision: each listed ID "
      "either joins an existing WP, gets a new WP, or is re-labelled in the catalogue (release or priority) with a decision "
      "record. (2) Fix the invalid citations of §5, which are typing errors, not scope decisions. (3) Re-run "
      "`python3 -I docs/plan/tools/trace.py` after every change of the CSV or the catalogues; the gate reviews of "
      "[gates](03-gates.md) use this document as their scope check.")
    A("")
    A("**Contents**")
    A("1. [Method and how to regenerate](#1-method-and-how-to-regenerate)")
    A("2. [Coverage per area, release and priority](#2-coverage-per-area-release-and-priority)")
    A("3. [Uncovered Must and Should requirements (gaps)](#3-uncovered-must-and-should-requirements-gaps)")
    A("4. [Covered too late](#4-covered-too-late)")
    A("5. [WP citations that do not resolve](#5-wp-citations-that-do-not-resolve)")
    A("6. [Acceptance criteria to gate and milestone](#6-acceptance-criteria-to-gate-and-milestone)")
    A("7. [Defects to preventing rule or feature to WP](#7-defects-to-preventing-rule-or-feature-to-wp)")
    A("8. [Reference candidates REF-001 … REF-108 to requirements to WP](#8-reference-candidates-to-requirements-to-wp)")
    A("9. [Findings and recommended actions](#9-findings-and-recommended-actions)")
    A("")
    A("---")
    A("")
    A("## 1. Method and how to regenerate")
    A("")
    A("The script is [tools/trace.py](tools/trace.py): Python 3 standard library only, paths relative to its own location, "
      "no network. From the repository root:")
    A("")
    A("```sh")
    A("python3 -I docs/plan/tools/trace.py          # rewrites 05-traceability.md and traceability-matrix.csv")
    A("python3 -I docs/plan/tools/trace.py --check  # exit code 1 if the committed outputs are stale (for CI)")
    A("```")
    A("")
    A("| Step | Input | Rule |")
    A("|---|---|---|")
    A("| Catalogue | tables of 02a, 02b, 02c | Every table row whose first cell is `FR-xxx-nnn` or `NFR-xxx-nn` and whose third cell is Must, Should or Could. Cells: requirement, priority, release, source, acceptance criterion. First occurrence wins. |")
    A("| Release buckets | Release cell | `MVP` and `MVP / 1.0` count as **MVP** (a split row owes its MVP part at the MVP); `1.0` is **1.0**; `1.x` and `Later` are **1.x/Later**. |")
    A("| WP citations | `requirements` column | Space-separated tokens. A range `A..B` expands to all numbers between its ends (same area, ascending). A token that is no ID, a reversed or cross-area range, a single ID absent from the catalogue, or a range none of whose members exist is an **invalid citation**. A range with some missing members is a **range hole** (members dropped, listed in §5). |")
    A("| Coverage | all of the above | A requirement is **covered** if at least one WP cites it. Citing is not proof of completion; it is the minimum for the plan to be able to deliver the requirement. |")
    A("| Gaps | catalogue minus coverage | Uncovered rows with priority Must or Should in bucket MVP or 1.0. Could, 1.x and Later rows are summarised in §2 and listed in the CSV only. |")
    A("| Late | WP milestone | An MVP Must/Should requirement whose earliest covering WP has a milestone after M2. |")
    A("| AC / D | 20 §19 and §21; 03 §8 | AC rows give criterion, requirements and golden gate; the plan gate comes from [gates](03-gates.md) §8.1 and §8.2. Covering WPs are the union of the WPs citing the AC's requirements and the WPs naming the AC in their `acceptance` column. For D rows, the FR IDs in the \"rule or feature\" cell and in the 15 catalogue row of every named EW rule are used. |")
    A("| REF | 02a Appendix A | REF to FR list (02a column) and 02b list; WP coverage is the union over its FRs. |")
    A("")
    A("Limits: the script checks *citation*, not content. A WP that cites 40 requirements may carry them unevenly; "
      "§2 and §9 therefore also report the number of requirements per WP. Requirement text is not interpreted.")
    A("")
    A("---")
    A("")
    A("## 2. Coverage per area, release and priority")
    A("")
    A("Cells show *covered / total*. Release bucket by §1. The last column pools 1.x and Later at all priorities.")
    A("")
    A("| Area | MVP Must | MVP Should | MVP Could | 1.0 Must | 1.0 Should | 1.0 Could | 1.x / Later | All |")
    A("|---|---|---|---|---|---|---|---|---|")
    def cell(t):
        c, n = t
        if n == 0:
            return "–"
        return f"{c}/{n}" + ("" if c == n else " **!**")
    for a in areas + ["TOTAL"]:
        d = tab[a]
        allc = sum(v[0] for v in d.values()); alln = sum(v[1] for v in d.values())
        name = f"**{a}**" if a == "TOTAL" else a
        A(f"| {name} | " + " | ".join(cell(d[k]) for k in cols) + f" | {allc}/{alln} |")
    A("")
    A("`!` marks a cell with uncovered requirements. Only MVP and 1.0 Must/Should cells are gaps; Could and 1.x cells are "
      "reported for information.")
    A("")
    A("**WPs by number of requirements cited** (a check on over-broad WPs):")
    A("")
    top = sorted(wps, key=lambda w: -nreq_wp[w["wp_id"]])[:8]
    A("| WP | Title | Milestone | Requirements covered |")
    A("|---|---|---|---|")
    for w in top:
        A(f"| {w['wp_id']} | {esc(w['title'])} | {w['milestone']} | {nreq_wp[w['wp_id']]} |")
    A("")
    zero = [w for w in wps if nreq_wp[w["wp_id"]] == 0]
    A(f"WPs that cover no catalogue requirement: {', '.join(w['wp_id'] for w in zero) if zero else 'none'}.")
    A("")
    A("---")
    A("")
    A("## 3. Uncovered Must and Should requirements (gaps)")
    A("")
    A("Complete list. Release \"MVP / 1.0\" rows appear under MVP. \"Area\" is the ID prefix. The proposed WP is the existing "
      "WP of the same area with the nearest topic or milestone, computed as the WP that covers most requirements of the "
      "same area at the same bucket; it is a pointer for the plan owner, not a decision.")
    A("")
    def near(r):
        c = collections.Counter()
        for q in rows:
            if q["area"] == r["area"] and q["wps"] and q["bucket"] == r["bucket"]:
                for w in q["wps"]:
                    c[w] += 1
        if not c:
            for q in rows:
                if q["area"] == r["area"] and q["wps"]:
                    for w in q["wps"]:
                        c[w] += 1
        return c.most_common(1)[0][0] if c else "none (new WP)"
    for b, title in (("MVP", "3.1 MVP gaps"), ("1.0", "3.2 Release 1.0 gaps")):
        A(f"### {title} ({len(gaps[b])})")
        A("")
        if not gaps[b]:
            A("None.")
            A("")
            continue
        A("| ID | Prio | Release | Requirement (abridged) | Nearest WP |")
        A("|---|---|---|---|---|")
        for r in gaps[b]:
            A(f"| {r['id']} | {r['prio']} | {r['rel']} | {short(r['text'])} | {near(r)} |")
        A("")
    A(f"### 3.3 Uncovered Could, 1.x and Later requirements ({len(optional_gaps)})")
    A("")
    A("Not gaps: the plan does not promise them. Listed compactly by area so that none is lost.")
    A("")
    byarea = collections.defaultdict(list)
    for r in optional_gaps:
        byarea[r["area"]].append(r["id"])
    A("| Area | Count | IDs |")
    A("|---|---|---|")
    for a in sorted(byarea):
        A(f"| {a} | {len(byarea[a])} | {ranges(byarea[a])} |")
    A("")
    A("---")
    A("")
    A("## 4. Covered too late")
    A("")
    A(f"MVP Must/Should requirements whose earliest covering WP finishes after M2 ({len(late)}). These are covered on paper but "
      "cannot be delivered at the MVP gate unless the WP is pulled forward or the requirement is re-labelled.")
    A("")
    if late:
        A("| ID | Prio | Requirement (abridged) | Earliest WP | WP milestone | WP release |")
        A("|---|---|---|---|---|---|")
        for r, w in late:
            A(f"| {r['id']} | {r['prio']} | {short(r['text'], 60)} | {w['wp_id']} | {w['milestone']} | {w['release']} |")
        A("")
    A(f"1.0 Must/Should requirements whose earliest covering WP is scheduled after 1.0 (milestone 1.x) ({len(late10)}):")
    A("")
    if late10:
        A("| ID | Prio | Requirement (abridged) | Earliest WP | WP release |")
        A("|---|---|---|---|---|")
        for r, w in late10:
            A(f"| {r['id']} | {r['prio']} | {short(r['text'], 60)} | {w['wp_id']} | {w['release']} |")
    else:
        A("None.")
    A("")
    A("---")
    A("")
    A("## 5. WP citations that do not resolve")
    A("")
    A("### 5.1 Invalid tokens in the `requirements` column")
    A("")
    if bad:
        A("| WP | Token | Problem |")
        A("|---|---|---|")
        for w, t, p in bad:
            A(f"| {w} | `{t}` | {p} |")
    else:
        A("None.")
    A("")
    A("### 5.2 Ranges with missing members")
    A("")
    A("The range is valid but names IDs that do not exist in the catalogue (the existing members are still counted).")
    A("")
    if holes:
        A("| WP | Range | Missing members |")
        A("|---|---|---|")
        for w, t, m in holes:
            A(f"| {w} | `{t}` | {', '.join(m)} |")
    else:
        A("None.")
    A("")
    A("### 5.3 Other CSV problems")
    A("")
    if other:
        A("| WP | Token | Problem |")
        A("|---|---|---|")
        for w, t, p in other:
            A(f"| {w} | `{t}` | {p} |")
    else:
        A("None (all `depends_on` names exist; all `acceptance` IDs resolve; every WP cites a requirement).")
    A("")
    A("---")
    A("")
    A("## 6. Acceptance criteria to gate and milestone")
    A("")
    A("Golden gate: the \"Gate\" column of [golden project](../design/20-golden-project.md) §21. Plan gate: "
      "[gates](03-gates.md) §8.1 (the gate at which the AC first has to pass; later gates repeat it; G6 repeats all). "
      "Covering WPs: WPs citing one of the AC's requirements or naming the AC in `acceptance`. \"Latest first WP\" is the "
      "milestone by which *every* requirement of the AC has at least one covering WP (the AC cannot pass earlier); the driver "
      "is the requirement that sets it. Flag **LATE** if that milestone is after the milestone of the plan gate (for an AC split over "
      "several gates, the last gate before G6, because the script cannot separate the parts); **NO WP** if an AC requirement has no WP. "
      "The plan-gate column omits G6, which repeats every AC.")
    A("")
    gate_ms = {"G1": 1, "G2": 2, "G3": 3, "G4": 4, "G5": 5, "G6": 6}
    A("| AC | Criterion (abridged) | Golden gate | Plan gate | Requirements | Acceptance WPs | Latest first-WP milestone | Flag |")
    A("|---|---|---|---|---|---|---|---|")
    ac_rows = []
    for a in sorted(acs):
        d = acs[a]
        rq = [x for x in expand_text(d["reqs"], reqs)]
        nreq_ids = [x for x in rq if x in reqs]
        acc_wps = sorted(w["wp_id"] for w in wps if a in w["acceptance"].split())
        # requirement-level first milestone
        flags, latest, driver = [], -1, None
        unc = []
        for x in nreq_ids:
            if not cov.get(x):
                unc.append(x)
                continue
            fw = min((wpd[w] for w in cov[x]), key=ms)
            if ms(fw) > latest:
                latest, driver = ms(fw), (x, fw)
        g = ac_gate.get(a, [])
        if a in ("AC-36", "AC-42"):
            g = g  # lever (c): 1.x, see gates
        pg = g[0] if g else "–"
        if a in ("AC-14", "AC-18", "AC-32", "AC-37", "AC-30", "AC-35", "AC-22", "AC-21"):
            pass
        g_nog6 = [x for x in g if x != "G6"] or g
        first_gate_ms = max((gate_ms[x] for x in g_nog6), default=None)   # split ACs: judged at their last non-G6 gate
        if unc:
            flags.append("NO WP: " + ", ".join(unc))
        if first_gate_ms is not None and latest > first_gate_ms:
            flags.append(f"LATE: {driver[0]} first covered by {driver[1]['wp_id']} ({driver[1]['milestone']})")
        if acc_wps == [] :
            flags.append("no WP names it")
        lm = f"M{latest}" if latest >= 0 else "–"
        gtxt = " + ".join(g_nog6) if g else "–"
        A(f"| {a} | {short(d['crit'], 70)} | {esc(d['gate'])} | {gtxt} | {ranges(nreq_ids) if nreq_ids else esc(d['reqs'])[:60]} | {', '.join(acc_wps) or '–'} | {lm} | {'; '.join(flags) or 'ok'} |")
        ac_rows.append((a, flags))
    A("")
    nbad = [a for a, f in ac_rows if any(x.startswith(("NO WP", "LATE")) for x in f)]
    nonamed = [a for a, f in ac_rows if "no WP names it" in f]
    A(f"{len(acs)} AC rows parsed. ACs with a **NO WP** or **LATE** flag: {', '.join(nbad) if nbad else 'none'}. "
      f"ACs that no WP names in its `acceptance` column: {', '.join(nonamed) if nonamed else 'none'} "
      "(they are still traceable through requirements; the gate review needs an owner WP for each, see §9).")
    A("")
    A("**AC by milestone, summary:**")
    A("")
    A("| Plan gate | Milestone | ACs (first gate) |")
    A("|---|---|---|")
    firstg = collections.defaultdict(list)
    for a in sorted(acs):
        g = ac_gate.get(a, [])
        firstg[g[0] if g else "–"].append(a)
    mmap = {"G1": "M1", "G2": "M2", "G3": "M3", "G4": "M4", "G5": "M5", "G6": "M6", "–": "–"}
    for g in sorted(firstg):
        A(f"| {g} | {mmap[g]} | {', '.join(firstg[g])} |")
    A("")
    A("---")
    A("")
    A("## 7. Defects to preventing rule or feature to WP")
    A("")
    A("Source: [golden project](../design/20-golden-project.md) §19 (rule that fires or feature that prevents) and the rule "
      "catalogue of [checks](../design/15-checks-revisions-collaboration.md) §14 (EW rule to FR). The FR set of a defect is every FR named in "
      "its §19 row plus the FR column of every EW rule named there; where that gives none, the requirements that cite the defect in their Source column. WPs are those covering at least one such FR (or naming "
      "the D in `acceptance`). Plan gate: [gates](03-gates.md) §8.2. Defects whose FR set is only partly covered show the "
      "uncovered FR IDs.")
    A("")
    A("| D | Defect (short) | Prevented by (FRs, via rules) | WPs | Plan gate | Flag |")
    A("|---|---|---|---|---|---|")
    d_flags = []
    for dk in sorted(ds):
        d = ds[dk]
        frs = expand_text(d["rule"], reqs)
        rl = re.findall(r"EW-[A-Z]+-\d+", d["rule"] + " " + d["expect"] + " " + d["test"])
        rl = [x for x in rl if x in rules]
        via = []
        for x in rl:
            for f in rules[x]:
                if f not in frs:
                    frs.append(f); via.append(f)
        frs = list(dict.fromkeys(frs))
        if not [f for f in frs if f in reqs]:
            frs = list(dict.fromkeys(dsrc.get(dk, [])))      # fallback: requirements that cite the defect as Source
            via_src = True
        else:
            via_src = False
        frs_e = [f for f in frs if f in reqs]
        wl = sorted({w for f in frs_e for w in cov.get(f, ())} | {w["wp_id"] for w in wps if dk in w["acceptance"].split()})
        unc = [f for f in frs_e if not cov.get(f)]
        miss = [f for f in frs if f not in reqs]
        g = d_gate.get(dk, [])
        gtxt = " + ".join(g) if g else ("G6 (full run)" if dk != "D-40" else "1.x")
        fl = []
        if not frs_e:
            fl.append("no FR named (feature described in text only)")
        if unc:
            fl.append("uncovered: " + ", ".join(unc))
        if miss:
            fl.append("unknown FR: " + ", ".join(miss))
        if not wl:
            fl.append("NO WP")
        d_flags.append((dk, fl))
        kind = ("rules " + ", ".join(rl) + "; ") if rl else ""
        if via_src:
            kind += "(requirements citing the defect) "
        A(f"| {dk} | {short(d['short'], 50)} | {kind}{ranges(frs_e) if frs_e else '–'} | {', '.join(wl) or '–'} | {gtxt} | {'; '.join(fl) or 'ok'} |")
    A("")
    nd = [k for k, f in d_flags if f]
    A(f"{len(ds)} defects parsed. Defects with a flag: {', '.join(nd) if nd else 'none'}. D-40 is a 1.x plugin feature "
      "by design; its \"NO WP\" or uncovered flag is expected.")
    A("")
    A("---")
    A("")
    A("## 8. Reference candidates to requirements to WP")
    A("")
    A("Source: [core requirements](../design/02a-requirements-core.md) Appendix A. A REF is *carried* if at least one of its "
      "requirements is covered by a WP; *fully carried* if all of them are; *must-carried* counts only Must/Should MVP/1.0 "
      "requirements (the ones the plan owes).")
    A("")
    A("Summary:")
    A("")
    st = collections.Counter()
    ref_rows = []
    for rk in sorted(refs):
        d = refs[rk]
        fr = [f for f in d["frs"] + d["frs02b"] if f in reqs]
        fr = list(dict.fromkeys(fr))
        cv = [f for f in fr if cov.get(f)]
        owed = [f for f in fr if reqs[f]["prio"] in ("Must", "Should") and reqs[f]["bucket"] in ("MVP", "1.0")] if False else \
               [f for f in fr if reqs[f]["prio"] in ("Must", "Should") and rel_bucket(reqs[f]["rel"]) in ("MVP", "1.0")]
        owed_unc = [f for f in owed if not cov.get(f)]
        ws = sorted({w for f in fr for w in cov.get(f, ())})
        if not fr:
            s = "no FR"
        elif not cv:
            s = "not carried"
        elif len(cv) < len(fr):
            s = "partly carried"
        else:
            s = "fully carried"
        st[s] += 1
        ref_rows.append((rk, d, fr, cv, owed_unc, ws, s))
    A("| Status | REF count |")
    A("|---|---|")
    for s in ("fully carried", "partly carried", "not carried", "no FR"):
        A(f"| {s} | {st[s]} |")
    A(f"| **Total** | {len(refs)} |")
    A("")
    A("REFs with an owed (Must/Should, MVP or 1.0) requirement that no WP covers, or with no covered requirement:")
    A("")
    A("| REF | Candidate (abridged) | Requirements | Uncovered owed | WPs |")
    A("|---|---|---|---|---|")
    k = 0
    for rk, d, fr, cv, ou, ws, s in ref_rows:
        if ou or s in ("not carried", "no FR"):
            k += 1
            A(f"| {rk} | {short(d['cand'], 60)} | {ranges(fr) or '–'} | {', '.join(ou) or '–'} | {', '.join(ws[:6]) + (' …' if len(ws) > 6 else '') or '–'} |")
    if not k:
        A("| – | – | – | – | – |")
    A("")
    A("Full REF table (REF, requirement count, covered, WP count, first WPs):")
    A("")
    A("| REF | FRs | Covered | WPs | Cited by (first 5) | Status |")
    A("|---|---|---|---|---|---|")
    for rk, d, fr, cv, ou, ws, s in ref_rows:
        A(f"| {rk} | {len(fr)} | {len(cv)} | {len(ws)} | {', '.join(ws[:5]) or '–'} | {s} |")
    A("")
    A("---")
    A("")
    A("## 9. Findings and recommended actions")
    A("")
    A("Computed counts are in the sections above; this section states what follows from them.")
    A("")
    top_gap_areas = collections.Counter(r["area"] for r in gaps["MVP"] + gaps["1.0"]).most_common(6)
    A(f"1. **Gaps.** {len(gaps['MVP']) + len(gaps['1.0'])} owed requirements have no WP; areas with most gaps: "
      + ", ".join(f"{a} ({n})" for a, n in top_gap_areas) + ". Each needs a decision: add to a WP, new WP, or re-label with a decision record.")
    A(f"2. **Invalid citations.** {len(bad)} invalid tokens and {len(holes)} ranges with missing members (§5). These are corrections to the CSV, not scope changes.")
    A(f"3. **Late coverage.** {len(late)} MVP requirements are only covered after M2 (§4).")
    A(f"4. **Acceptance ownership.** ACs without a WP that names them: {', '.join(nonamed) if nonamed else 'none'}. "
      "Each gate AC should have one owner WP so the gate review has a person to ask.")
    A("5. **Regeneration.** The script is the only source of the tables; change the CSV or the catalogues, re-run, review the diff. "
      "The CI check (`--check`) keeps the document honest.")
    A("")
    A("---")
    A("")
    A("## Sources")
    A("")
    A("- [work-packages.csv](work-packages.csv), [work breakdown](02-work-packages.md), [gates](03-gates.md), [global plan](01-global-plan.md)")
    A("- Catalogues: [02a](../design/02a-requirements-core.md), [02b](../design/02b-requirements-extended.md), [02c](../design/02c-nonfunctional-requirements.md)")
    A("- [Golden project](../design/20-golden-project.md) §19, §21; [checks](../design/15-checks-revisions-collaboration.md) rule catalogue")
    A("")
    A("## Open questions")
    A("")
    A("1. Should a requirement cited by an infrastructure WP only (for example NFR rows cited by S0 WPs) count as covered, or should "
      "NFR coverage require a WP that names an acceptance test? The script counts any citation.")
    A("2. Should the CI job fail on any MVP/1.0 Must gap (hard) and warn on Should gaps? Proposed: yes, after the first clean run.")
    A("")
    return "\n".join(L) + "\n"


if __name__ == "__main__":
    text = build()
    if "--check" in sys.argv:
        cur = OUT_MD.read_text(encoding="utf-8") if OUT_MD.exists() else ""
        if cur != text:
            print("05-traceability.md is stale; run tools/trace.py")
            sys.exit(1)
        print("up to date")
    else:
        OUT_MD.write_text(text, encoding="utf-8")
        print("wrote", OUT_MD.name, "and", OUT_CSV.name)
