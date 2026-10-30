# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
"""The CI-time trend of the performance gate (FT-269, NFR-MAINT-05, decision log ID-381).

Reads the per-job durations of the last N completed runs of the `ci.yml` workflow through the GitHub REST API
(the repository is public, so a token is optional) and prints, or appends to a report file, a Markdown section
that sets the wall time of each run against the 30-minute budget of NFR-MAINT-05.

  python3 -I ci/ci-times.py [--runs N] [--repo OWNER/NAME] [--workflow ci.yml] [--append REPORT.md]
  python3 -I ci/ci-times.py --dry-run     # print the requests, make none
  python3 -I ci/ci-times.py --selftest    # run the offline checks

Gentle by design: one request for the run list and one per run (`--runs` is at most 9, so at most 10 calls), a
pause between calls, no retries. An unauthenticated client has 60 calls an hour per address, and the hosted
runners share their egress addresses, so the API will often answer 403: when the environment variable
`GITHUB_TOKEN` is set (the workflow passes the automatic, read-only `github.token` of the job; no configured
secret), it is sent as `Authorization: Bearer <token>`. The token is never printed or written to the report.
A rate-limit answer is reported in the section, not retried. A failure of the API never fails the caller: the
section says why it has no data and the exit status stays 0, unless `--strict` is given.

Standard library only. A job's duration is `completed_at - started_at` of the jobs API. The wall time of a run is
the span from its first job start to its last job completion (jobs run in parallel), which is the figure the
budget talks about. GitHub reports a job stopped by its timeout as `cancelled`, so a cancelled test job is read as
a timeout (marked with a dagger).
"""

import argparse
import datetime
import json
import os
import statistics
import sys
import time
import urllib.error
import urllib.request

API = "https://api.github.com"
DEFAULT_REPO = "Ironkode787/ElektraWatt"
BUDGET_SECONDS = 30 * 60
MAX_RUNS = 9  # 1 call for the run list + 1 per run = at most 10 calls
PAUSE_SECONDS = 0.5
JOB_ORDER = ["lint", "test-linux", "test-linux-default", "test-windows-build", "test-windows-1", "test-windows-2", "test-windows-3", "supply-chain"]


def parse_time(text):
    """A GitHub timestamp (`2026-10-22T01:02:03Z`) as an aware datetime, or None."""
    if not text:
        return None
    return datetime.datetime.fromisoformat(text.replace("Z", "+00:00"))


def seconds(start, end):
    """Whole seconds between two timestamps, or None when either is missing."""
    a, b = parse_time(start), parse_time(end)
    if a is None or b is None:
        return None
    return max(0, int((b - a).total_seconds()))


def fmt(secs):
    """`21m14s`, or an em dash for a missing value."""
    if secs is None:
        return "—"
    return f"{secs // 60}m{secs % 60:02d}s"


def summarize_run(run, jobs):
    """One row: the run's identity, its jobs' durations and its wall time in seconds."""
    durations = {}
    cancelled = set()
    starts, ends = [], []
    for job in jobs:
        if job.get("conclusion") == "skipped" or not job.get("started_at"):
            continue
        durations[job["name"]] = seconds(job.get("started_at"), job.get("completed_at"))
        if job.get("conclusion") == "cancelled":
            cancelled.add(job["name"])
        starts.append(parse_time(job["started_at"]))
        if job.get("completed_at"):
            ends.append(parse_time(job["completed_at"]))
    wall = int((max(ends) - min(starts)).total_seconds()) if starts and ends else None
    return {
        "number": run.get("run_number"),
        "sha": (run.get("head_sha") or "")[:7],
        "event": run.get("event") or "",
        "result": run.get("conclusion") or run.get("status") or "",
        "url": run.get("html_url") or "",
        "durations": durations,
        "cancelled": cancelled,
        "wall": wall,
    }


def job_columns(rows):
    """The job names in the order of JOB_ORDER, then any others alphabetically."""
    names = {name for row in rows for name in row["durations"]}
    known = [n for n in JOB_ORDER if n in names]
    return known + sorted(names - set(known))


def render(rows, repo, workflow, note=""):
    """The Markdown section for the rows (newest first)."""
    lines = [
        "## CI time trend (NFR-MAINT-05, FT-269)",
        "",
        f"Per-job durations of the last {len(rows)} completed runs of `{workflow}` in `{repo}` "
        "(GitHub jobs API). The budget is 30 min wall time per PR (NFR-MAINT-05); the 50-minute "
        "job timeout is a guard, not the budget (decision log ID-381).",
        "",
    ]
    if note:
        lines += [note, ""]
    if not rows:
        lines += ["No run data.", ""]
        return "\n".join(lines)
    columns = job_columns(rows)
    lines.append("| Run | Commit | Event | Result | " + " | ".join(columns) + " | Wall |")
    lines.append("|---|---|---|---|" + "---:|" * len(columns) + "---:|")
    for row in rows:
        cells = []
        for name in columns:
            cell = fmt(row["durations"].get(name))
            if name in row["cancelled"]:
                cell += " †"
            cells.append(cell)
        wall = fmt(row["wall"])
        if row["wall"] is not None and row["wall"] > BUDGET_SECONDS:
            wall = f"**{wall}** (over)"
        run = f"[{row['number']}]({row['url']})" if row["url"] else str(row["number"])
        lines.append(f"| {run} | `{row['sha']}` | {row['event']} | {row['result']} | " + " | ".join(cells) + f" | {wall} |")
    walls = [r["wall"] for r in rows if r["wall"] is not None]
    lines.append("")
    if walls:
        over = sum(1 for w in walls if w > BUDGET_SECONDS)
        lines.append(
            f"Wall time: median {fmt(int(statistics.median(walls)))}, slowest {fmt(max(walls))}, "
            f"fastest {fmt(min(walls))}; {over} of {len(walls)} runs over the 30-minute budget."
        )
    if any(row["cancelled"] for row in rows):
        lines.append("")
        lines.append(
            "† cancelled: GitHub reports a job stopped by its timeout as cancelled, so a cancelled test job "
            "is read as a timeout, not as a user cancel."
        )
    lines.append("")
    return "\n".join(lines)


def request_headers(token=None):
    """The headers of an API call; `Authorization: Bearer <token>` only when a non-empty token is given."""
    headers = {
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "elektrawatt-ci-times",
    }
    token = (token or "").strip()
    if token:
        headers["Authorization"] = f"Bearer {token}"
    return headers


def get_json(url):
    request = urllib.request.Request(url, headers=request_headers(os.environ.get("GITHUB_TOKEN")))
    with urllib.request.urlopen(request, timeout=20) as response:
        return json.load(response)


def collect(repo, workflow, runs, fetch=get_json, pause=PAUSE_SECONDS):
    """The summary rows of the last `runs` completed runs: at most 1 + runs calls."""
    listing = fetch(f"{API}/repos/{repo}/actions/workflows/{workflow}/runs?status=completed&per_page={runs}")
    rows = []
    for run in listing.get("workflow_runs", [])[:runs]:
        time.sleep(pause)
        jobs = fetch(f"{API}/repos/{repo}/actions/runs/{run['id']}/jobs?per_page=100")
        rows.append(summarize_run(run, jobs.get("jobs", [])))
    return rows


def selftest():
    """Offline checks of the arithmetic and the layout, on made-up API answers."""
    assert fmt(1274) == "21m14s" and fmt(None) == "—" and fmt(59) == "0m59s"
    assert seconds("2026-10-22T10:00:00Z", "2026-10-22T10:21:14Z") == 1274
    assert seconds(None, "2026-10-22T10:21:14Z") is None

    def job(name, start, end, conclusion="success"):
        return {"name": name, "started_at": start, "completed_at": end, "conclusion": conclusion}

    run = {"run_number": 7, "head_sha": "0123456789abcdef", "event": "pull_request", "conclusion": "failure",
           "html_url": "https://example.invalid/runs/7"}
    jobs = [
        job("lint", "2026-10-22T10:00:00Z", "2026-10-22T10:05:00Z"),
        job("test-windows-1", "2026-10-22T10:00:10Z", "2026-10-22T10:35:10Z", "cancelled"),
        job("test-linux", "2026-10-22T10:00:05Z", "2026-10-22T10:21:19Z"),
        job("docs-only", None, None, "skipped"),
    ]
    row = summarize_run(run, jobs)
    assert row["wall"] == 35 * 60 + 10 and row["cancelled"] == {"test-windows-1"}
    assert "docs-only" not in row["durations"]
    fast = summarize_run({**run, "run_number": 6}, [job("test-linux", "2026-10-22T09:00:00Z", "2026-10-22T09:20:00Z")])
    text = render([row, fast], "o/r", "ci.yml")
    assert "| lint | test-linux | test-windows-1 |" in text
    assert "35m10s** (over)" in text and "21m14s" in text and "35m00s †" in text
    assert "median 27m35s, slowest 35m10s, fastest 20m00s; 1 of 2 runs over" in text
    assert "† cancelled" in text
    assert "No run data." in render([], "o/r", "ci.yml")

    calls = []

    def fake(url):
        calls.append(url)
        if "/jobs" in url:
            return {"jobs": jobs}
        return {"workflow_runs": [dict(run, id=i) for i in range(20)]}

    rows = collect("o/r", "ci.yml", MAX_RUNS, fetch=fake, pause=0)
    assert len(rows) == MAX_RUNS and len(calls) == 1 + MAX_RUNS <= 10

    # the request headers: no Authorization without a token, a Bearer header with one, nothing printed
    plain = request_headers(None)
    assert "Authorization" not in plain and "Authorization" not in request_headers("")
    assert "Authorization" not in request_headers("  ")
    assert plain["X-GitHub-Api-Version"] == "2022-11-28" and plain["Accept"] == "application/vnd.github+json"
    authed = request_headers("dummy-token-value")
    assert authed["Authorization"] == "Bearer dummy-token-value"
    assert {k: v for k, v in authed.items() if k != "Authorization"} == plain
    assert "dummy-token-value" not in render([row], "o/r", "ci.yml")
    print("ci-times selftest: ok")


def main(argv=None):
    parser = argparse.ArgumentParser(description="CI-time trend of the last runs of ci.yml (FT-269).")
    parser.add_argument("--runs", type=int, default=8, help=f"completed runs to read (1..{MAX_RUNS}, default 8)")
    parser.add_argument("--repo", default=DEFAULT_REPO, help=f"owner/name (default {DEFAULT_REPO})")
    parser.add_argument("--workflow", default="ci.yml", help="workflow file name (default ci.yml)")
    parser.add_argument("--append", metavar="REPORT", help="append the section to this Markdown file")
    parser.add_argument("--dry-run", action="store_true", help="print the requests and make none")
    parser.add_argument("--selftest", action="store_true", help="run the offline checks and exit")
    parser.add_argument("--strict", action="store_true", help="exit 1 when the API cannot be read")
    args = parser.parse_args(argv)
    # The section has an em dash and a dagger; a legacy Windows console would choke on them.
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8")

    if args.selftest:
        selftest()
        return 0
    if not 1 <= args.runs <= MAX_RUNS:
        parser.error(f"--runs must be between 1 and {MAX_RUNS} (at most 10 API calls)")

    if args.dry_run:
        print(f"GET {API}/repos/{args.repo}/actions/workflows/{args.workflow}/runs?status=completed&per_page={args.runs}")
        print(f"GET {API}/repos/{args.repo}/actions/runs/<id>/jobs?per_page=100   (once per run, {args.runs} at most)")
        auth = "with the token of GITHUB_TOKEN" if os.environ.get("GITHUB_TOKEN") else "no token (GITHUB_TOKEN is not set)"
        print(f"{1 + args.runs} calls at most, {PAUSE_SECONDS} s apart, {auth}")
        return 0

    note = ""
    rows = []
    status = 0
    try:
        rows = collect(args.repo, args.workflow, args.runs)
    except urllib.error.HTTPError as err:
        note = f"The GitHub API answered HTTP {err.code} ({err.reason}); there is no CI-time data for this report."
        status = 1
    except (urllib.error.URLError, OSError, ValueError, KeyError) as err:
        note = f"The GitHub API could not be read ({err}); there is no CI-time data for this report."
        status = 1
    section = render(rows, args.repo, args.workflow, note)
    if args.append:
        with open(args.append, "a", encoding="utf-8", newline="\n") as report:
            report.write("\n" + section)
        print(f"ci-times: section appended to {args.append}")
    else:
        print(section)
    if note:
        print(f"ci-times: {note}", file=sys.stderr)
    return status if args.strict else 0


if __name__ == "__main__":
    sys.exit(main())
