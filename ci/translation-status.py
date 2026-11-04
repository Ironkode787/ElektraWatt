#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""The translation status report (WP-S7.05 D15; NFR-I18N-01, NFR-I18N-07): the share of each locale's texts.

  python3 -I ci/translation-status.py [--repo <dir>]          # prints the report; exit 1 below the bar
  python3 -I ci/translation-status.py --stale <fresh> <committed>   # are two PO templates' messages the same?
  python3 -I ci/translation-status.py --test                  # its offline unit test

The report is a Markdown table per kind of text, a row per locale:

  - the window (`crates/ew-viewmodel/locales/<locale>/*.ftl`): the en-GB messages a locale has, and the core flow's
    share, which is the same set, since WP-S2.08's completeness gate asks every key of every shipped locale;
  - the rule messages (`crates/ew-checks/messages/<locale>/*.ftl`): each message's value and attributes (`.title`,
    `.hint`) that en-GB has;
  - the user guide (`docs/book/po/`): the messages of the template `messages.pot` that each `<language>.po` translates
    (a fuzzy or empty translation is untranslated), and of those the tutorials' (the messages that a page of
    `src/tutorials/` holds); the source language, English, is the template itself.

It exits 1, naming why, when nl-NL has less than 95 % of the window's texts or `nl.po` leaves a tutorial message
untranslated. WP-S6.08 attaches its output to a release (RC-06); Weblate's statistics replace it once a Weblate
project exists. `--stale` compares the messages (not the references or the header) of two templates and exits 1,
saying what differs, when they differ: `ci/build-book.sh` warns with it when the committed template is stale.
Standard library only.
"""
import argparse
import math
import os
import re
import sys
import tempfile
import unittest

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
UI = os.path.join("crates", "ew-viewmodel", "locales")
RULES = os.path.join("crates", "ew-checks", "messages")
PO_DIR = os.path.join("docs", "book", "po")
TEMPLATE = "messages.pot"
SOURCE = "en-GB"
# The bar of the Dutch window (NFR-I18N-01) and the locale it holds for.
BAR = 95.0
BAR_LOCALE = "nl-NL"
# The PO language whose tutorials must be whole (NFR-I18N-07's Dutch tutorials).
TUTORIALS_LANGUAGE = "nl"
TUTORIALS = "src/tutorials/"

MESSAGE = re.compile(r"^(-?[A-Za-z][A-Za-z0-9_-]*)\s*=\s*(.*)$")
ATTRIBUTE = re.compile(r"^\s+\.([A-Za-z][A-Za-z0-9_-]*)\s*=\s*(.*)$")


class Refusal(Exception):
    """A file the report cannot read."""


def ftl_texts(text):
    """The texts of a Fluent file: each message with a value by its id, each attribute as `<id>.<attribute>`."""
    texts = set()
    current = None
    for line in text.splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        m = MESSAGE.match(line)
        if m:
            current = m.group(1)
            if m.group(2).strip():
                texts.add(current)
            continue
        a = ATTRIBUTE.match(line)
        if a and current:
            texts.add(f"{current}.{a.group(1)}")
        elif line[0].isspace() and current and not any(t.startswith(f"{current}.") for t in texts):
            # A value that starts on the next line (before any attribute).
            texts.add(current)
    return texts


def locale_texts(root, kind):
    """{locale: the texts of its .ftl files} for the folder `kind` under `root`."""
    folder = os.path.join(root, kind)
    if not os.path.isdir(folder):
        raise Refusal(f"{kind} is missing")
    out = {}
    for locale in sorted(os.listdir(folder)):
        path = os.path.join(folder, locale)
        if not os.path.isdir(path):
            continue
        texts = set()
        for name in sorted(os.listdir(path)):
            if name.endswith(".ftl"):
                with open(os.path.join(path, name), encoding="utf-8") as f:
                    texts |= ftl_texts(f.read())
        out[locale] = texts
    if SOURCE not in out:
        raise Refusal(f"{os.path.join(kind, SOURCE)} is missing")
    return out


def unquote(s):
    """The text of a PO string literal's inside."""
    out = []
    i = 0
    while i < len(s):
        c = s[i]
        if c == "\\" and i + 1 < len(s):
            nxt = s[i + 1]
            out.append({"n": "\n", "t": "\t", '"': '"', "\\": "\\"}.get(nxt, "\\" + nxt))
            i += 2
            continue
        out.append(c)
        i += 1
    return "".join(out)


def po_entries(text):
    """The entries of a PO file, without its header: [(msgid, msgstr, fuzzy, references)]."""
    entries = []
    entry = None
    field = None

    def finish():
        if entry is not None and entry["msgid"]:
            entries.append((entry["msgid"], entry["msgstr"], entry["fuzzy"], entry["refs"]))

    for line in text.splitlines() + [""]:
        line = line.strip()
        if not line:
            finish()
            entry, field = None, None
            continue
        if entry is None:
            entry = {"msgid": "", "msgstr": "", "fuzzy": False, "refs": []}
        if line.startswith("#,"):
            entry["fuzzy"] = entry["fuzzy"] or "fuzzy" in [flag.strip() for flag in line[2:].split(",")]
        elif line.startswith("#:"):
            entry["refs"].extend(line[2:].split())
        elif line.startswith("#"):
            continue
        elif line.startswith("msgid "):
            field = "msgid"
            entry[field] = unquote(line[len("msgid ") :].strip()[1:-1])
        elif line.startswith("msgstr "):
            field = "msgstr"
            entry[field] = unquote(line[len("msgstr ") :].strip()[1:-1])
        elif line.startswith('"') and field:
            entry[field] += unquote(line[1:-1])
    return entries


def read(path):
    try:
        with open(path, encoding="utf-8") as f:
            return f.read()
    except FileNotFoundError:
        raise Refusal(f"{path} is missing") from None


def guide_status(root):
    """[(language, translated, total, tutorials translated, tutorial messages)] of the template and each PO file."""
    folder = os.path.join(root, PO_DIR)
    template = po_entries(read(os.path.join(folder, TEMPLATE)))
    ids = [msgid for msgid, _, _, _ in template]
    # A reference reads `src\\tutorials/...` when the template was made on Windows.
    tutorial = {
        msgid for msgid, _, _, refs in template if any(r.replace("\\", "/").startswith(TUTORIALS) for r in refs)
    }
    rows = [("en (source)", len(ids), len(ids), len(tutorial), len(tutorial))]
    for name in sorted(os.listdir(folder)):
        if not name.endswith(".po"):
            continue
        entries = po_entries(read(os.path.join(folder, name)))
        done = {msgid for msgid, msgstr, fuzzy, _ in entries if msgstr and not fuzzy}
        language = name[: -len(".po")]
        rows.append((language, sum(1 for i in ids if i in done), len(ids), len(tutorial & done), len(tutorial)))
    return rows


def share(n, total):
    """`n` of `total` as a percentage, rounded down to one decimal: a share short of the whole never reads 100."""
    return 100.0 if total == 0 else math.floor(1000 * n / total) / 10


def pct(value):
    return f"{value:.1f} %"


def report(root):
    """(the Markdown report, the reasons it fails the bar)."""
    lines = ["# Translation status", ""]
    failures = []

    ui = locale_texts(root, UI)
    source = ui[SOURCE]
    lines += [
        "## The window",
        "",
        f"The messages of `{UI.replace(os.sep, '/')}/`. The core flow is the set that WP-S2.08's completeness gate "
        "asks of every shipped locale: every key, so its share is the window's.",
        "",
        "| Locale | Messages | Of en-GB | Share | Core flow |",
        "|---|---|---|---|---|",
    ]
    for locale, texts in ui.items():
        have = len(texts & source)
        part = share(have, len(source))
        lines.append(f"| {locale} | {have} | {len(source)} | {pct(part)} | {pct(part)} |")
        if locale == BAR_LOCALE and part < BAR:
            failures.append(f"{locale} has {pct(part)} of the window's texts, below {pct(BAR)}")
    if BAR_LOCALE not in ui:
        failures.append(f"{os.path.join(UI, BAR_LOCALE)} is missing")

    rules = locale_texts(root, RULES)
    source = rules[SOURCE]
    lines += [
        "",
        "## The rule messages",
        "",
        f"The messages of `{RULES.replace(os.sep, '/')}/`: each rule's message, title and hint. A text a locale "
        "lacks falls back to en-GB.",
        "",
        "| Locale | Texts | Of en-GB | Share |",
        "|---|---|---|---|",
    ]
    for locale, texts in rules.items():
        have = len(texts & source)
        lines.append(f"| {locale} | {have} | {len(source)} | {pct(share(have, len(source)))} |")

    lines += [
        "",
        "## The user guide",
        "",
        f"The messages of the template `{PO_DIR.replace(os.sep, '/')}/{TEMPLATE}` (the hand-written pages and the "
        "table of contents) that each PO file translates; the tutorials are the messages of the pages of "
        f"`{TUTORIALS}`. An untranslated message shows in English.",
        "",
        "| Language | Translated | Of the template | Share | Tutorials translated | Of the tutorials "
        "| Tutorials share |",
        "|---|---|---|---|---|---|---|",
    ]
    for language, done, total, tut_done, tut_total in guide_status(root):
        lines.append(
            f"| {language} | {done} | {total} | {pct(share(done, total))} | {tut_done} | {tut_total} | "
            f"{pct(share(tut_done, tut_total))} |"
        )
        if language == TUTORIALS_LANGUAGE and tut_done < tut_total:
            failures.append(f"{language}.po leaves {tut_total - tut_done} tutorial message(s) untranslated")
    return "\n".join(lines) + "\n", failures


def stale(fresh, committed):
    """What differs between the messages of two templates; empty when they are the same."""
    new = {msgid for msgid, _, _, _ in po_entries(read(fresh))}
    old = {msgid for msgid, _, _, _ in po_entries(read(committed))}
    added, removed = len(new - old), len(old - new)
    if not added and not removed:
        return ""
    return f"{committed} is stale: {added} message(s) added and {removed} removed; run `bash ci/build-book.sh --pot`"


def main(argv):
    parser = argparse.ArgumentParser(prog="translation-status.py", description=__doc__.split("\n")[0])
    parser.add_argument("--test", action="store_true", help="run the offline unit test")
    parser.add_argument("--repo", default=REPO, help="the repository root (default: this checkout)")
    parser.add_argument("--stale", nargs=2, metavar=("FRESH", "COMMITTED"), help="compare two templates' messages")
    args = parser.parse_args(argv)
    if args.test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(StatusTest)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    try:
        if args.stale:
            why = stale(*args.stale)
            if why:
                print(why)
                return 1
            print(f"translation-status: {args.stale[1]} has the template's messages")
            return 0
        text, failures = report(args.repo)
    except Refusal as refusal:
        print(f"translation-status: {refusal}", file=sys.stderr)
        return 1
    sys.stdout.write(text)
    for why in failures:
        print(f"translation-status: {why}", file=sys.stderr)
    return 1 if failures else 0


class StatusTest(unittest.TestCase):
    """The parsers and the report on a fixture tree in a temporary directory."""

    HEADER = 'msgid ""\nmsgstr ""\n"Language: nl\\n"\n\n'

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = self.tmp.name

    def tearDown(self):
        self.tmp.cleanup()

    def write(self, rel, text):
        path = os.path.join(self.root, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as f:
            f.write(text)

    def test_fluent_texts_are_values_and_attributes(self):
        text = (
            "# a comment\nmenu-file = File\nempty =\n    .title = A title\n    .hint = A hint\n"
            "multi =\n    on the next line\nselect = { $n ->\n        [one] one\n       *[other] many\n    }\n"
            "-term = EW\n"
        )
        self.assertEqual(
            ftl_texts(text), {"menu-file", "empty.title", "empty.hint", "multi", "select", "-term"}
        )

    def test_po_entries_join_lines_and_mark_fuzzy(self):
        text = (
            self.HEADER + '#: src/a.md:1 src/SUMMARY.md:3\nmsgid "One"\nmsgstr "Een"\n\n'
            '#, fuzzy\nmsgid "Two"\nmsgstr "Twee"\n\n'
            'msgid ""\n"Three "\n"\\"lines\\"\\n"\nmsgstr ""\n'
        )
        self.assertEqual(
            po_entries(text),
            [
                ("One", "Een", False, ["src/a.md:1", "src/SUMMARY.md:3"]),
                ("Two", "Twee", True, []),
                ('Three "lines"\n', "", False, []),
            ],
        )

    def fixture(self, nl_ui, nl_po):
        self.write("crates/ew-viewmodel/locales/en-GB/shell.ftl", "".join(f"k{i} = t\n" for i in range(20)))
        self.write("crates/ew-viewmodel/locales/nl-NL/shell.ftl", "".join(f"k{i} = t\n" for i in range(nl_ui)))
        self.write("crates/ew-checks/messages/en-GB/rules.ftl", "rule-a = m\n    .title = T\n    .hint = H\n")
        self.write("crates/ew-checks/messages/nl-NL/rules.ftl", "rule-a =\n    .title = T\n")
        self.write(
            "docs/book/po/messages.pot",
            self.HEADER
            + '#: src/SUMMARY.md:1 src/tutorials/index.md:1\nmsgid "Tutorials"\nmsgstr ""\n\n'
            + '#: src\\tutorials/first-page.md:3\nmsgid "Step"\nmsgstr ""\n\n'
            + '#: src/how-to/install.md:1\nmsgid "Install"\nmsgstr ""\n',
        )
        self.write("docs/book/po/nl.po", self.HEADER + nl_po)

    def test_the_report_counts_each_locale_and_the_tutorials(self):
        translated = 'msgid "Tutorials"\nmsgstr "Lessen"\n\nmsgid "Step"\nmsgstr "Stap"\n\n'
        self.fixture(20, translated + 'msgid "Gone"\nmsgstr "Weg"\n')
        text, failures = report(self.root)
        self.assertEqual(failures, [])
        self.assertIn("| en-GB | 20 | 20 | 100.0 % | 100.0 % |\n| nl-NL | 20 | 20 | 100.0 % | 100.0 % |\n", text)
        self.assertIn("| en-GB | 3 | 3 | 100.0 % |\n| nl-NL | 1 | 3 | 33.3 % |\n", text)
        self.assertIn(
            "| en (source) | 3 | 3 | 100.0 % | 2 | 2 | 100.0 % |\n| nl | 2 | 3 | 66.6 % | 2 | 2 | 100.0 % |\n", text
        )

    def test_the_bar_fails_a_short_window_and_an_untranslated_tutorial(self):
        self.fixture(18, '#, fuzzy\nmsgid "Tutorials"\nmsgstr "Lessen"\n\nmsgid "Step"\nmsgstr ""\n')
        text, failures = report(self.root)
        self.assertIn("| nl-NL | 18 | 20 | 90.0 % | 90.0 % |\n", text)
        self.assertEqual(
            failures,
            [
                "nl-NL has 90.0 % of the window's texts, below 95.0 %",
                "nl.po leaves 2 tutorial message(s) untranslated",
            ],
        )
        self.fixture(19, 'msgid "Tutorials"\nmsgstr "Lessen"\n\nmsgid "Step"\nmsgstr "Stap"\n')
        self.assertEqual(report(self.root)[1], [])

    def test_a_share_short_of_the_whole_never_reads_100(self):
        self.assertEqual(pct(share(1999, 2000)), "99.9 %")
        self.assertEqual(pct(share(0, 0)), "100.0 %")

    def test_a_missing_file_is_refused_by_name(self):
        self.write("crates/ew-viewmodel/locales/nl-NL/shell.ftl", "a = b\n")
        with self.assertRaises(Refusal) as refused:
            report(self.root)
        self.assertEqual(str(refused.exception), os.path.join(UI, SOURCE) + " is missing")

    def test_stale_compares_the_messages_only(self):
        self.write("a.pot", self.HEADER + '#: src/a.md:1\nmsgid "One"\nmsgstr ""\n')
        self.write("b.pot", 'msgid ""\nmsgstr ""\n"POT-Creation-Date: x\\n"\n\n#: src/a.md:9\nmsgid "One"\nmsgstr ""\n')
        self.write("c.pot", self.HEADER + 'msgid "Two"\nmsgstr ""\n\nmsgid "Three"\nmsgstr ""\n')
        a, b, c = (os.path.join(self.root, n) for n in ("a.pot", "b.pot", "c.pot"))
        self.assertEqual(stale(a, b), "")
        self.assertEqual(
            stale(c, a), f"{a} is stale: 2 message(s) added and 1 removed; run `bash ci/build-book.sh --pot`"
        )


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
