#!/usr/bin/env python3
"""Schedule model of the ElektraWatt plan (GA-7 / PA-9), standard library only.

Resource-constrained list scheduling with generalist developers:
- every WP has one owner at a time; its duration is effort x (1 + reserve) / (0.69 pw per week x capacity
  factor x efficiency);
- a free developer takes the ready WP with the earliest milestone, then the one with the longest remaining
  dependency chain, then the lowest ID;
- a WP is ready when its dependencies are done and every lagged dependency (`lagged_deps`, external calendar
  lead time such as a signing enrolment) has been done for its lag;
- a milestone is reached when every WP of it and of all earlier milestones is done, except the level-of-effort
  WPs, which use capacity but gate nothing;
- the stabilisation WPs wait for every WP of their release.

Scenario parameters: developers' join months, an efficiency factor that applies from the month the last developer
joins (coordination overhead of a larger team), a capacity factor (sensitivity), the estimate used (PERT mean or
optimistic), optional AI-acceleration of development work, optional strict stream roles, and the scope (full or
lean, from the `milestone` and `milestone_lean` columns).
"""
import math
from wbs import (RANK, MS, PROD, RESERVE, LOE, STABILISATION, NO_GATE, load_rows, pert, milestone, lagged)

# AI-assisted scenarios (01 §3.7). Planning inputs, not measurements.
AI_REVIEW_SHARE = 0.25            # share of an S1-S7 WP that is review, design decision and verification
AI_NOT_ACCELERATED = {            # people-bound WPs inside S1-S7
    "WP-S2.12", "WP-S2.16",       # usability rounds with participants
    "WP-S6.09",                   # monthly release operations and support
    "WP-S6.11",                   # external acceptance with partners and their installations
}


def ai_factor(wp_id, stream, speedup):
    """Effort multiplier of one WP for an AI speed-up of development work (1.0 = no change)."""
    if speedup == 1.0 or stream == "S0" or wp_id in AI_NOT_ACCELERATED:
        return 1.0
    return AI_REVIEW_SHARE + (1 - AI_REVIEW_SHARE) / speedup


def load(scope="lean", rows=None):
    """WP dict for a scope: milestone, efforts, dependencies (filtered as the model needs them)."""
    rows = rows or load_rows()
    wp = {}
    for r in rows:
        wp[r["wp_id"]] = {"ms": milestone(r, scope), "mean": pert(r), "o": pert(r, "o"), "stream": r["stream"],
                          "deps": r["depends_on"].split(), "lag": lagged(r), "title": r["title"]}
    for cid, lim in STABILISATION.items():
        deps = set(wp[cid]["deps"]) | {k for k, w in wp.items() if RANK[w["ms"]] <= RANK[lim]
                                        and k not in LOE | NO_GATE | {cid}}
        wp[cid]["deps"] = sorted(d for d in deps if RANK[wp[d]["ms"]] <= RANK[wp[cid]["ms"]])
    for k, w in wp.items():          # nobody waits for work moved to a later release
        if k not in STABILISATION:
            w["deps"] = [d for d in w["deps"] if RANK[wp[d]["ms"]] <= RANK[w["ms"]]]
        w["lag"] = {d: v for d, v in w["lag"].items() if RANK[wp[d]["ms"]] <= RANK[w["ms"]]}
    return {k: w for k, w in wp.items() if w["ms"] != "1.x"}


class Result:
    def __init__(self, wp, start, finish, dev, ms):
        self.wp, self.start, self.finish, self.dev, self.ms = wp, start, finish, dev, ms

    def months(self, k):
        return self.start[k] * 12 / 52, self.finish[k] * 12 / 52


def simulate(wp, joins, eff=1.0, cap=1.0, est="mean", ai=1.0, roles=None, eff_from_join=True):
    """Run the list scheduler. joins: start month per developer. Returns a Result (times in weeks)."""
    base = {k: w[est] * ai_factor(k, w["stream"], ai) * (1 + RESERVE) / (PROD * cap) for k, w in wp.items()}
    succ = {k: [] for k in wp}
    for k, w in wp.items():
        for d in w["deps"]:
            succ[d].append((k, 0.0))
        for d, lag in w["lag"].items():
            succ[d].append((k, lag * 52 / 12))
    tail = {}

    def chain(k):
        if k not in tail:
            tail[k] = base[k] + max([chain(s) + lag for s, lag in succ[k]] or [0])
        return tail[k]

    last_join = max(joins) * 52 / 12
    todo, start, done, who = set(wp), {}, {}, {}
    free = [(j * 52 / 12, d) for d, j in enumerate(joins)]
    eps = 1e-9
    while todo:
        free.sort()
        t, dev = free.pop(0)

        def ready_at(k):
            w = wp[k]
            if any(done.get(d, math.inf) > t + eps for d in w["deps"]):
                return False
            return all(done.get(d, math.inf) + lag * 52 / 12 <= t + eps for d, lag in w["lag"].items())

        ready = [k for k in todo if ready_at(k)]
        if roles is not None:
            ready = [k for k in ready if wp[k]["stream"] in roles[dev]]
        if not ready:
            events = [f for f in done.values() if f > t + eps]
            events += [done[d] + lag * 52 / 12 for k in todo for d, lag in wp[k]["lag"].items()
                       if d in done and done[d] + lag * 52 / 12 > t + eps]
            if not events:
                # nothing finishes later: another developer (free at the same time) must start work first
                if t > 5000:
                    raise RuntimeError("schedule deadlock: no ready WP for developer %d at week %.1f" % (dev, t))
                free.append((t + 0.25, dev))
                continue
            free.append((min(events), dev))
            continue
        k = min(ready, key=lambda x: (RANK[wp[x]["ms"]], -chain(x), x))
        e = eff if (not eff_from_join or t >= last_join - eps) else 1.0
        todo.remove(k)
        start[k], who[k] = t, dev
        done[k] = t + base[k] / e
        free.append((done[k], dev))
    ms, run = {}, 0.0
    for m in MS[:-1]:
        run = max([run] + [f * 12 / 52 for k, f in done.items() if RANK[wp[k]["ms"]] <= RANK[m] and k not in LOE])
        ms[m] = math.ceil(run - eps)
    # G-SP: the performance spike runs right after WP-S1.03 (01 §9.2 rule 2) and takes about one month with its
    # report, whatever the model's start of WP-S1.05 is.
    ms["G-SP"] = math.ceil(done["WP-S1.03"] * 12 / 52 + 1 - eps)
    return Result(wp, start, done, who, ms)


def critical_path(wp, cap=1.0):
    """Longest dependency chain to the last milestone with unlimited developers: (path, finish weeks)."""
    base = {k: w["mean"] * (1 + RESERVE) / (PROD * cap) for k, w in wp.items()}
    fin, prev = {}, {}

    def f(k):
        if k not in fin:
            best, arg = 0.0, None
            for d in wp[k]["deps"]:
                if f(d) > best:
                    best, arg = f(d), d
            for d, lag in wp[k]["lag"].items():
                if f(d) + lag * 52 / 12 > best:
                    best, arg = f(d) + lag * 52 / 12, d
            fin[k], prev[k] = best + base[k], arg
        return fin[k]

    return f, prev, base
