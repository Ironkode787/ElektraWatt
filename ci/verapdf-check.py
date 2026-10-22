#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Verdict of a veraPDF report over the golden PDF exports (CI-N-09, FT-294; Python 3 standard library only).

  python3 -I ci/verapdf-check.py <report.xml> [--expect-jobs N] [--profile 2B] [--summary <file.md>]
  python3 -I ci/verapdf-check.py --selftest     # run the offline checks

`<report.xml>` is the XML (machine-readable) report of `verapdf --format xml`, the default format, for one or more
PDF files. The check fails (exit code 1) unless

  - every job of the report validated its file against the wanted profile (`--profile`, default `2B`, matched in
    the profile name, so `PDF/A-2B validation profile`) and the file `isCompliant="true"`;
  - the batch summary says that no job failed to parse, was encrypted, ran out of memory or hit a veraPDF
    exception, and that `compliant` equals the number of jobs and `nonCompliant` is 0;
  - the number of jobs is `--expect-jobs`, when given (so a run that silently skipped a file is a failure).

The verdict goes to stdout, the first failed rules of every non-compliant file with it; `--summary` appends the same
text as Markdown (the job summary of the workflow). The acceptance runner's AC-13 entry reads the same report
(`crates/ew-api/tests/acceptance/checks.rs`, `EW_VERAPDF_REPORT`) and applies the same rules.
"""
import argparse
import sys
import xml.etree.ElementTree as ET

# The batch summary attributes that must be zero.
ZERO_ATTRS = ("failedToParse", "encrypted", "outOfMemory", "veraExceptions")
MAX_RULES = 5


def local(tag):
    """The tag without its namespace."""
    return tag.rsplit("}", 1)[-1]


def children(element, name):
    return [c for c in element.iter() if local(c.tag) == name]


def check(text, expect_jobs=None, profile="2B"):
    """Returns (errors, lines): the reasons for a failure (empty when the report passes) and the verdict lines."""
    errors, lines = [], []
    try:
        root = ET.fromstring(text)
    except ET.ParseError as e:
        return [f"the report is not XML: {e}"], lines
    jobs = children(root, "job")
    if not jobs:
        errors.append("the report has no job")
    if expect_jobs is not None and len(jobs) != expect_jobs:
        errors.append(f"the report has {len(jobs)} job(s), expected {expect_jobs}")
    for job in jobs:
        names = children(job, "name")
        name = (names[0].text or "?").strip() if names else "?"
        reports = children(job, "validationReport")
        if not reports:
            errors.append(f"{name}: no validation report (the file did not parse or was not validated)")
            continue
        report = reports[0]
        profile_name = report.get("profileName", "")
        compliant = report.get("isCompliant") == "true"
        if profile.lower() not in profile_name.lower().replace("-", ""):
            errors.append(f"{name}: validated against `{profile_name}`, expected a profile with `{profile}`")
        verdict = "compliant" if compliant else "NOT compliant"
        lines.append(f"{name}: {verdict} ({profile_name})")
        if not compliant:
            errors.append(f"{name}: {report.get('statement', 'not compliant')}")
            # The failed rules are `<rule status="failed">` in veraPDF's machine-readable report and
            # `<ruleSummary status="failed">` in its summary form: both are read.
            failed = [r for r in children(report, "ruleSummary") + children(report, "rule") if r.get("status") == "failed"]
            for rule in failed[:MAX_RULES]:
                desc = " ".join((rule.findtext("description") or "").split())
                lines.append(
                    f"  failed rule {rule.get('specification', '?')} {rule.get('clause', '?')}"
                    f"-{rule.get('testNumber', '?')} ({rule.get('failedChecks', '?')} checks): {desc}"
                )
            if len(failed) > MAX_RULES:
                lines.append(f"  and {len(failed) - MAX_RULES} more failed rule(s)")
    summaries = children(root, "batchSummary")
    if not summaries:
        errors.append("the report has no batch summary")
    else:
        summary = summaries[0]
        for attr in ZERO_ATTRS:
            value = summary.get(attr)
            if value != "0":
                errors.append(f"batch summary: {attr}={value} (expected 0)")
        counts = children(summary, "validationReports")
        if not counts:
            errors.append("batch summary: no validationReports count")
        else:
            c = counts[0]
            if c.get("nonCompliant") != "0" or c.get("failedJobs") != "0":
                errors.append(
                    f"batch summary: nonCompliant={c.get('nonCompliant')} failedJobs={c.get('failedJobs')} (expected 0)"
                )
            if c.get("compliant") != str(len(jobs)):
                errors.append(f"batch summary: compliant={c.get('compliant')} for {len(jobs)} job(s)")
    return errors, lines


def verdict_text(errors, lines):
    out = list(lines)
    if errors:
        out.append("veraPDF cross-check FAILED:")
        out.extend(f"  - {e}" for e in errors)
    else:
        out.append("veraPDF cross-check passed")
    return "\n".join(out)


def selftest():
    def report(jobs, summary_attrs=None, counts=None):
        body = "".join(jobs)
        attrs = {"totalJobs": "1", "failedToParse": "0", "encrypted": "0", "outOfMemory": "0", "veraExceptions": "0"}
        attrs.update(summary_attrs or {})
        a = " ".join(f'{k}="{v}"' for k, v in attrs.items())
        c = counts or 'compliant="1" nonCompliant="0" failedJobs="0"'
        return (
            f'<?xml version="1.0" encoding="utf-8"?><report><jobs>{body}</jobs>'
            f"<batchSummary {a}><validationReports {c}>1</validationReports></batchSummary></report>"
        )

    def job(name="/data/a.pdf", compliant="true", profile="PDF/A-2B validation profile", rules=""):
        return (
            f"<job><item size=\"1\"><name>{name}</name></item>"
            f'<validationReport jobEndStatus="normal" profileName="{profile}" '
            f'statement="PDF file is {"" if compliant == "true" else "not "}compliant with Validation Profile requirements." '
            f'isCompliant="{compliant}"><details>{rules}</details></validationReport></job>'
        )

    errors, lines = check(report([job()]), expect_jobs=1)
    assert errors == [] and lines == ["/data/a.pdf: compliant (PDF/A-2B validation profile)"], (errors, lines)
    assert verdict_text(errors, lines).endswith("passed")
    assert check(report([job(), job("/data/b.pdf")], {"totalJobs": "2"}, 'compliant="2" nonCompliant="0" failedJobs="0"'), 2)[0] == []

    rule = (
        '<ruleSummary specification="ISO 19005-2:2011" clause="6.2.4.3" testNumber="2" status="failed" '
        'passedChecks="0" failedChecks="3"><description>DeviceRGB shall only be used with an output intent</description></ruleSummary>'
        '<ruleSummary specification="ISO 19005-2:2011" clause="6.1.2" testNumber="1" status="passed" passedChecks="1" failedChecks="0"/>'
    )
    errors, lines = check(
        report([job(compliant="false", rules=rule)], counts='compliant="0" nonCompliant="1" failedJobs="0"')
    )
    assert any("not compliant" in e for e in errors), errors
    assert any("batch summary: nonCompliant=1" in e for e in errors), errors
    assert any("6.2.4.3-2 (3 checks): DeviceRGB shall only be used" in x for x in lines), lines
    assert not any("6.1.2" in x for x in lines), "a passed rule is not listed"
    assert "FAILED" in verdict_text(errors, lines)

    # the `<rule>` shape of the machine-readable report: the failed rule is listed, the passed one is not
    rule_shape = (
        '<rule specification="ISO 19005-2:2011" clause="6.2.11.4.1" testNumber="2" status="failed" '
        'passedChecks="0" failedChecks="2"><description>Fonts shall be embedded</description>'
        '<check status="failed"><context>root/font</context></check></rule>'
        '<rule specification="ISO 19005-2:2011" clause="6.1.2" testNumber="1" status="passed" passedChecks="1" failedChecks="0"/>'
    )
    errors, lines = check(
        report([job(compliant="false", rules=rule_shape)], counts='compliant="0" nonCompliant="1" failedJobs="0"')
    )
    assert any("6.2.11.4.1-2 (2 checks): Fonts shall be embedded" in x for x in lines), lines
    assert not any("6.1.2" in x for x in lines), "a passed rule is not listed"

    errors, _ = check(report([job(profile="PDF/A-1B validation profile")]))
    assert any("expected a profile with `2B`" in e for e in errors), errors
    assert check(report([job(profile="PDF/A-1B validation profile")]), profile="1B")[0] == []
    assert any("expected 2" in e for e in check(report([job()]), expect_jobs=2)[0])
    assert any("failedToParse=1" in e for e in check(report([job()], {"failedToParse": "1"}))[0])
    assert any("encrypted=1" in e for e in check(report([job()], {"encrypted": "1"}))[0])
    assert any("failedJobs=1" in e for e in check(report([job()], counts='compliant="1" nonCompliant="0" failedJobs="1"'))[0])
    no_report = "<job><item><name>/data/c.pdf</name></item></job>"
    assert any("no validation report" in e for e in check(report([no_report], counts='compliant="0" nonCompliant="0" failedJobs="1"'))[0])
    assert any("no job" in e for e in check(report([]))[0])
    assert any("no batch summary" in e for e in check("<report><jobs>" + job() + "</jobs></report>")[0])
    assert any("not XML" in e for e in check("not xml at all")[0])
    assert any("not XML" in e for e in check("")[0])
    print("verapdf-check selftest: ok")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("report", nargs="?", help="the veraPDF XML report")
    parser.add_argument("--expect-jobs", type=int, help="the number of PDF files the report must cover")
    parser.add_argument("--profile", default="2B", help="the profile the files must be validated against (default 2B)")
    parser.add_argument("--summary", help="append the verdict as Markdown to this file (the job summary)")
    parser.add_argument("--selftest", action="store_true", help="run the offline checks and exit")
    args = parser.parse_args()
    if args.selftest:
        selftest()
        return 0
    if not args.report:
        parser.error("the report file is required")
    try:
        with open(args.report, encoding="utf-8") as f:
            text = f.read()
    except OSError as e:
        print(f"cannot read the report: {e}", file=sys.stderr)
        return 1
    errors, lines = check(text, args.expect_jobs, args.profile)
    out = verdict_text(errors, lines)
    print(out)
    if args.summary:
        with open(args.summary, "a", encoding="utf-8") as f:
            f.write("### veraPDF PDF/A-2b cross-check\n\n```text\n" + out + "\n```\n")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
