#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""The golden-table change list (FT-300, gate criterion G-M1-09). Python 3 standard library only.

  python docs/plan/tools/golden_changes.py --write   # regenerate docs/plan/gate-records/golden-change-list.md
  python docs/plan/tools/golden_changes.py --check   # exit 1 if the committed list is stale (CI, git history needed)
  python docs/plan/tools/golden_changes.py --test    # offline unit test over a fixture repository in a temp dir
  python docs/plan/tools/golden_changes.py           # print the generated block

Reads the git history of `golden/J-E-2416/tables/*.csv` and of the tables of doc 20
(`docs/design/20-golden-project.md`), and lists every commit that changed a table row, newest first: the commit's
short hash and date (no author), its subject, the decision IDs it cites, and per table the rows touched (by the
first column; the changed columns for a changed row). Prose changes of doc 20 are not listed.

Decision IDs of a commit (ID-n, FT-n, DEC-n, ADR-n):
  1. cited in its message (trailer lines are ignored) or in the touched rows of doc 20;
  2. an FT-n cited in the message also pulls in the decision-log entries that cite that FT-n and a golden table;
  3. a range in the message (`ID-33…ID-63`, or open `ID-324..`, which ends before the next range start of the
     history) pulls in the entries of that range that cite doc 20 or a golden table and name a touched table: its
     CSV name, or its section number of doc 20 (`§8.2`, or the parent `§8`);
  4. a decision-log entry that names the commit's short hash as inline code (`abc1234`, as listed) cites it: a design
     pass maps a commit whose message cites nothing this way (decision log ID-550).
A commit without any of these reads "no ID - review".

The list is written between `<!-- generated:golden-changes -->` markers. Its stamp is the newest listed commit (not
HEAD), so that committing the list does not make it stale. Needs a clone with the full history. The decision log is
read at HEAD on purpose: when a design pass records an ID for a listed commit, the mapping changes and `--check` goes
stale until `--write` is run again, so the list follows the log. It runs at every delivery gate, not in CI (a shallow
checkout lacks the history, and the list embeds commit hashes).
"""
import csv, io, re, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
TABLES_DIR = "golden/J-E-2416/tables"
DOC20 = "docs/design/20-golden-project.md"
DECISION_LOG = "docs/plan/06-decision-log.md"
OUT = "docs/plan/gate-records/golden-change-list.md"
BEGIN, END = "<!-- generated:golden-changes -->", "<!-- /generated:golden-changes -->"
ID_RE = re.compile(r"\b(ID|FT|DEC|ADR)-(\d+)\b")
RANGE_RE = re.compile(r"\b(ID|FT)-(\d+)(\s*(?:…|\.{2,3}|\s+to\s+)\s*(?:(?:ID|FT)-)?(\d+)?)")
TRAILER_RE = re.compile(r"^(Co-Authored-By|Signed-off-by):.*$", re.I | re.M)
NO_ID = "no ID - review"
PD15 = ("FT-29", "FT-40", "FT-63")
CAP = 30  # row keys listed per category and table

# the two kinds of source file: (git path -> parser)
SEP = "\x1e"


def git(root, *args, check=True):
    p = subprocess.run(["git", "-c", "core.quotepath=off", "-C", str(root), *args], capture_output=True)
    if check and p.returncode != 0:
        raise SystemExit("golden_changes.py: git %s failed: %s" % (" ".join(args), p.stderr.decode("utf-8", "replace")))
    return p.stdout.decode("utf-8", errors="replace").replace("\r\n", "\n")


def blob(root, rev, path):
    """Text of `path` at `rev`, or None if it does not exist there."""
    p = subprocess.run(["git", "-C", str(root), "show", "%s:%s" % (rev, path)], capture_output=True)
    return p.stdout.decode("utf-8", errors="replace").replace("\r\n", "\n") if p.returncode == 0 else None


# ---------------------------------------------------------------------------------------------------- tables

def keyed(rows):
    """[(key, cells)] -> {key: cells}; the second row with a key is `key#2`, the third `key#3`, ..."""
    seen, out = {}, {}
    for cells in rows:
        k = cells[0].strip() if cells else ""
        seen[k] = seen.get(k, 0) + 1
        out[k if seen[k] == 1 else "%s#%d" % (k, seen[k])] = cells
    return out


def parse_csv(text):
    """{table label: (header, {key: cells})} for one CSV file."""
    rows = list(csv.reader(io.StringIO(text)))
    return {"": (rows[0] if rows else [], keyed(rows[1:]))}


def split_cells(line):
    s = re.sub(r"`[^`]*`", lambda m: m.group(0).replace("|", "\x00"), line.strip()).replace("\\|", "\x00")
    s = s[1:] if s.startswith("|") else s
    s = s[:-1] if s.endswith("|") else s
    return [c.strip().replace("\x00", "|") for c in s.split("|")]


def parse_markdown(text):
    """{label: (header, {key: cells})} for every table; label = nearest heading, `, table n` after the first."""
    out, heading, count, fence, lines, i = {}, "(before the first heading)", {}, False, text.split("\n"), 0
    while i < len(lines):
        line = lines[i]
        if line.startswith("```"):
            fence = not fence
        m = re.match(r"^#{1,6}\s+(.*)", line)
        if not fence and m:
            heading = re.sub(r"\s+", " ", m.group(1).strip())
        if not fence and line.lstrip().startswith("|") and i + 1 < len(lines) and re.match(r"^\s*\|\s*:?-{3,}", lines[i + 1]):
            header, j, body = split_cells(line), i + 2, []
            while j < len(lines) and lines[j].lstrip().startswith("|"):
                body.append(split_cells(lines[j]))
                j += 1
            count[heading] = count.get(heading, 0) + 1
            label = heading if count[heading] == 1 else "%s, table %d" % (heading, count[heading])
            out[label] = (header, keyed(body))
            i = j
            continue
        i += 1
    return out


def parse(path, text):
    if text is None:
        return {}
    return parse_markdown(text) if path.endswith(".md") else parse_csv(text)


def diff_tables(path, old, new):
    """The changes of one file between two parsed versions: a list of dicts (label, new_table, removed_table, ...)."""
    res = []
    for label in list(new) + [l for l in old if l not in new]:
        o, n = old.get(label), new.get(label)
        if o is None and n is None:
            continue
        entry = {"path": path, "label": label, "rows_text": []}
        if o is None:
            entry.update(kind="new", count=len(n[1]))
            entry["rows_text"] = [" ".join(c) for c in n[1].values()]
        elif n is None:
            entry.update(kind="removed", count=len(o[1]))
            entry["rows_text"] = [" ".join(c) for c in o[1].values()]
        else:
            oh, orows = o
            nh, nrows = n
            added = [k for k in nrows if k not in orows]
            removed = [k for k in orows if k not in nrows]
            changed = []
            for k in nrows:
                if k in orows and nrows[k] != orows[k]:
                    cols = [nh[x] if x < len(nh) else "col %d" % (x + 1) for x in range(max(len(nrows[k]), len(orows[k])))
                            if (nrows[k][x] if x < len(nrows[k]) else None) != (orows[k][x] if x < len(orows[k]) else None)]
                    changed.append((k, cols))
            header_changed = oh != nh
            if not (added or removed or changed or header_changed):
                continue
            entry.update(kind="rows", added=added, removed=removed, changed=changed, header_changed=header_changed)
            # only what changed counts as cited: whole added and removed rows, the new text of changed cells
            entry["rows_text"] = [" ".join(nrows[k]) for k in added] + [" ".join(orows[k]) for k in removed]
            for k, _ in changed:
                entry["rows_text"] += [c for x, c in enumerate(nrows[k]) if x >= len(orows[k]) or orows[k][x] != c]
        res.append(entry)
    return res


# ---------------------------------------------------------------------------------------------------- decision log

def parse_log(text, table_names):
    """{n: (text, cites_table)} for the `| ID-n | ... |` rows of the decision log."""
    pat = re.compile(r"doc 20|20-golden-project|golden table|golden project|golden/J-E-2416|tables/", re.I)
    names = re.compile(r"\b(?:%s)\b" % "|".join(re.escape(t) for t in sorted(table_names))) if table_names else None
    out = {}
    for line in (text or "").split("\n"):
        m = re.match(r"^\|\s*ID-(\d+)\s*\|", line)
        if m:
            out[int(m.group(1))] = (line, bool(pat.search(line) or (names and names.search(line))))
    return out


def message_ids(message):
    """(explicit IDs, [(first, last or None)] ID ranges) of a commit message without its trailers."""
    msg = TRAILER_RE.sub("", message)
    ranges = []

    def cut(m):
        if m.group(1) == "ID":
            ranges.append((int(m.group(2)), int(m.group(4)) if m.group(4) else None))
        return " "

    rest = RANGE_RE.sub(cut, msg)
    return {"%s-%s" % (k, n) for k, n in ID_RE.findall(rest)}, ranges


def markers(changes):
    """Regexes that tie a decision-log entry to the touched tables: a CSV name, or a section number of doc 20 (§8.2
    also matches an entry that cites §8)."""
    res = []
    for c in changes:
        if c["path"].endswith(".csv"):
            res.append(re.compile(re.escape(Path(c["path"]).name)))
            continue
        m = re.match(r"(\d+(?:\.\d+)*)", c["label"])
        while m:
            num = m.group(1)
            res.append(re.compile(r"(?:§|\bsections?\s+)\s*%s(?!\d)" % re.escape(num), re.I))
            num = num.rpartition(".")[0]
            m = re.match(r"(\d+(?:\.\d+)*)$", num)
    return res


def decision_ids(message, row_texts, log, range_starts, marks=(), commit=None):
    ids, ranges = message_ids(message)
    if commit:  # rule 4: an entry that names the commit's short hash as inline code
        named = re.compile(r"`%s`" % re.escape(commit))
        ids |= {"ID-%d" % n for n, (text, _) in log.items() if named.search(text)}
    for k in [i for i in ids if i.startswith("FT-")]:
        for n, (text, cites) in log.items():
            if cites and re.search(r"\b%s\b" % re.escape(k), text):
                ids.add("ID-%d" % n)
    top = max(log) if log else 0
    for a, b in ranges:
        if b is None:
            later = [s for s in range_starts if s > a]
            b = (min(later) - 1) if later else top
        ids |= {"ID-%d" % n for n, (text, cites) in log.items()
                if a <= n <= b and cites and (not marks or any(m.search(text) for m in marks))}
    for t in row_texts:  # cited by the touched rows of doc 20
        ids |= {"%s-%s" % (k, n) for k, n in ID_RE.findall(t)}
    order = {"ID": 0, "FT": 1, "DEC": 2, "ADR": 3}
    return sorted(ids, key=lambda s: (order[s.split("-")[0]], int(s.split("-")[1])))


# ---------------------------------------------------------------------------------------------------- history

def history(root):
    """[(hash, date, subject, message, [paths touched])] over both sources, newest first (merges not expanded)."""
    out = git(root, "log", "--abbrev=7", "--no-merges", "--format=%x1e%h%x1f%ad%x1f%B%x1f", "--date=short",
              "--name-only", "--", TABLES_DIR, DOC20)
    commits = []
    for chunk in out.split(SEP)[1:]:
        h, d, body, files = chunk.split("\x1f", 3)
        body = body.strip("\n")
        commits.append((h, d, body.split("\n", 1)[0].strip(), body, [f for f in files.split("\n") if f.strip()]))
    return commits


def changes_of(root, h, paths):
    parent = h + "^"
    has_parent = subprocess.run(["git", "-C", str(root), "rev-parse", "--verify", "-q", parent + "{commit}"],
                                capture_output=True).returncode == 0
    res = []
    for p in sorted(paths):
        if not (p.startswith(TABLES_DIR + "/") and p.endswith(".csv")) and p != DOC20:
            continue
        old = parse(p, blob(root, parent, p) if has_parent else None)
        new = parse(p, blob(root, h, p))
        res.extend(diff_tables(p, old, new))
    return res


def collect(root):
    commits = history(root)
    log_text = blob(root, "HEAD", DECISION_LOG)
    names = {Path(p).name for c in commits for p in c[4] if p.endswith(".csv")}
    log = parse_log(log_text, names)
    starts = set()
    for m in RANGE_RE.finditer(TRAILER_RE.sub("", git(root, "log", "--format=%B", "--no-merges"))):
        if m.group(1) == "ID":
            starts.add(int(m.group(2)))
    entries = []
    for h, d, subject, message, paths in commits:
        chg = changes_of(root, h, paths)
        if not chg:
            continue
        ids = decision_ids(message, [t for c in chg if c["path"] == DOC20 for t in c["rows_text"]], log, starts,
                           markers(chg), h)
        pd15 = [ft for ft in PD15 if ft in ids or
                set(ids) & {"ID-%d" % n for n, (t, _) in log.items() if re.search(r"\b%s\b" % ft, t)}]
        entries.append({"hash": h, "date": d, "subject": subject, "ids": ids, "changes": chg, "pd15": pd15})
    return entries


# ---------------------------------------------------------------------------------------------------- rendering

def code(k):
    """A row key as inline code (a backtick inside the key is dropped, a pipe is escaped for tables)."""
    return "`%s`" % k.replace("`", "").replace("|", "\\|")


def keys(ks):
    shown = [code(k) for k in ks[:CAP]]
    return ", ".join(shown) + (" and %d more" % (len(ks) - CAP) if len(ks) > CAP else "")


def render_change(c):
    where = "`%s`" % c["path"] if c["path"].endswith(".csv") else "doc 20, %s" % c["label"].replace("|", "\\|")
    if c["kind"] == "new":
        return "- %s: new table, %d rows" % (where, c["count"])
    if c["kind"] == "removed":
        return "- %s: table removed, %d rows" % (where, c["count"])
    parts = []
    if c["added"]:
        parts.append("added %d (%s)" % (len(c["added"]), keys(c["added"])))
    if c["changed"]:
        items = ["%s (%s)" % (code(k), ", ".join(cols)) for k, cols in c["changed"][:CAP]]
        more = " and %d more" % (len(c["changed"]) - CAP) if len(c["changed"]) > CAP else ""
        parts.append("changed %d (%s%s)" % (len(c["changed"]), ", ".join(items), more))
    if c["removed"]:
        parts.append("removed %d (%s)" % (len(c["removed"]), keys(c["removed"])))
    if c["header_changed"]:
        parts.append("header changed")
    return "- %s: %s" % (where, "; ".join(parts))


def render(entries):
    L = []
    no_id = [e for e in entries if not e["ids"]]
    L.append("Generated by [`golden_changes.py`](../tools/golden_changes.py) from the git history of "
             "`%s` and of the tables of doc 20 ([20](../../design/20-golden-project.md)): " % TABLES_DIR +
             "**%d changes** (commits that touched a table row), **%d without a decision ID** (read \"%s\"). " %
             (len(entries), len(no_id), NO_ID) +
             ("Newest listed commit `%s` of %s." % (entries[0]["hash"], entries[0]["date"]) if entries else "No change."))
    L.append("")
    L.append("Per change: the commit's short hash and date, its subject, the decision IDs it cites (in its message "
             "or in the touched rows of doc 20; for a design pass's ID range, the entries of the range that cite doc 20 "
             "or a golden table, see the script's header), and per table the rows touched, by the first column, with the "
             "changed columns of a changed row. Prose of doc 20 is not listed. Newest first.")
    L.append("")
    L.append("**PD-15 items (the pilot's acknowledgement):**")
    for ft in PD15:
        hs = ["`%s`" % e["hash"] for e in entries if ft in e["pd15"]]
        L.append("- %s: %s" % (ft, ", ".join(hs) if hs else "no commit cites it or a decision-log entry that cites it"))
    L.append("")
    for e in entries:
        L.append("### `%s`, %s" % (e["hash"], e["date"]))
        L.append("")
        L.append(e["subject"])
        L.append("")
        L.append("Decision IDs: %s" % (", ".join(e["ids"]) if e["ids"] else NO_ID))
        L.append("")
        L.extend(render_change(c) for c in e["changes"])
        L.append("")
    return "\n".join(L).rstrip("\n") + "\n"


HEAD = """# Golden-table change list

**Status:** generated · **Gate:** G-M1 criterion G-M1-09 · **Task:** FT-300 · **Part of:** ElektraWatt plan, gate
records

**Purpose.** The list of every change to the golden tables (`golden/J-E-2416/tables/`) and to the tables of
[doc 20](../../design/20-golden-project.md) since the first commit, for the pilot's acknowledgement
([readiness report](G-M1-2026-10-readiness.md), G-M1-09 and item O9). It is generated from the repository history; do
not edit the block by hand. Regenerate it with `python docs/plan/tools/golden_changes.py --write`; `--check` fails when it
is stale.

"""


def block(root):
    return BEGIN + "\n" + render(collect(root)) + END


def write(root):
    p = root / OUT
    new = block(root)
    cur = p.read_text(encoding="utf-8") if p.exists() else HEAD + BEGIN + "\n" + END + "\n"
    if BEGIN in cur and END in cur:
        cur = cur[:cur.index(BEGIN)] + new + cur[cur.index(END) + len(END):]
    else:
        cur = cur.rstrip("\n") + "\n\n" + new + "\n"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(cur, encoding="utf-8", newline="\n")


def stale(root):
    p = root / OUT
    if not p.exists():
        return True
    cur = p.read_text(encoding="utf-8").replace("\r\n", "\n")
    return BEGIN not in cur or END not in cur or cur[cur.index(BEGIN):cur.index(END) + len(END)] != block(root)


# ---------------------------------------------------------------------------------------------------- test

def selftest():
    import os, tempfile, unittest

    env = dict(os.environ, GIT_AUTHOR_NAME="Some Person", GIT_AUTHOR_EMAIL="person@example.org",
               GIT_COMMITTER_NAME="Some Person", GIT_COMMITTER_EMAIL="person@example.org")

    class T(unittest.TestCase):
        def run_git(self, *a):
            subprocess.run(["git", "-C", str(self.root), "-c", "commit.gpgsign=false", *a], check=True, env=env,
                           capture_output=True)

        def commit(self, files, msg):
            for rel, text in files.items():
                p = self.root / rel
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_text(text, encoding="utf-8", newline="\n")
            self.run_git("add", "-A")
            self.run_git("commit", "-q", "-m", msg)

        def setUp(self):
            self._tmp = tempfile.TemporaryDirectory()
            self.root = Path(self._tmp.name)
            self.run_git("init", "-q")
            t = TABLES_DIR + "/parts.csv"
            d20 = "# Doc\n\nProse.\n\n## 8.2 Wires\n\n| W | From | To |\n|---|---|---|\n| W1 | A:1 | B:1 |\n| W2 | A:2 | B:2 |\n"
            self.commit({t: "id,name,rating\nP1,fuse,10 A\nP2,lamp,24 V\n", DOC20: d20, "other.txt": "x"},
                        "docs: first (ID-5)\n\nCo-Authored-By: Some Person <person@example.org>")
            self.commit({"other.txt": "y"}, "unrelated (ID-9)")
            self.commit({t: "id,name,rating\nP1,fuse,16 A\nP2,lamp,24 V\nP3,relay,5 A\n"},
                        "fix(golden): fuse rating (FT-29)\n\nSigned-off-by: Some Person <person@example.org>")
            self.commit({DOC20: d20.replace("Prose.", "More prose.")}, "prose only (ID-1)")
            self.commit({DOC20: d20.replace("| W2 | A:2 | B:2 |", "| W2 | A:2 | B:3 |\n| W3 | C:1 | D:1 | x (ID-77) |")
                         .replace("| W | From | To |\n|---|---|---|", "| W | From | To |\n|---|---|---|")},
                        "docs: wires\n\nsomething")
            self.commit({t: "id,name,rating\nP1,fuse,16 A\nP3,relay,5 A\n"}, "docs(cycle 9): design pass ID-50..ID-70")
            cur = (self.root / DOC20).read_text(encoding="utf-8")
            self.commit({DOC20: cur.replace("| W1 | A:1 | B:1 |", "| W1 | A:1 | B:9 |")}, "plain change without any reference")
            log = ("| ID-29 | **Fuse.** golden tables carry it (FT-29). | Core | x |\n"
                   "| ID-55 | **Part.** doc 20 and parts.csv carry it. | Core | x |\n"
                   "| ID-57 | **Wire.** doc 20 section 8.2, not a part. | Core | x |\n"
                   "| ID-56 | **Other.** nothing about the tables. | Core | x |\n"
                   "| ID-80 | **Late.** doc 20 but outside. | Core | x |\n")
            self.commit({DECISION_LOG: log}, "docs: log")

        def tearDown(self):
            self._tmp.cleanup()

        def test_list(self):
            es = collect(self.root)
            self.assertEqual(len(es), 5)  # the unrelated and the prose-only commits are not listed
            subjects = [e["subject"] for e in es]
            self.assertEqual(subjects[0], "plain change without any reference")
            by = {e["subject"].split(" (")[0]: e for e in es}
            self.assertEqual(by["plain change without any reference"]["ids"], [])
            self.assertEqual(by["fix(golden): fuse rating"]["ids"], ["ID-29", "FT-29"])  # FT in message pulls in ID-29
            self.assertEqual(by["docs: first"]["ids"], ["ID-5"])
            self.assertEqual(by["docs: wires"]["ids"], ["ID-77"])  # cited by the touched row
            self.assertEqual(by["docs(cycle 9): design pass ID-50..ID-70"]["ids"], ["ID-55"])  # range, table-citing only
            ch = by["fix(golden): fuse rating"]["changes"][0]
            self.assertEqual((ch["added"], [k for k, _ in ch["changed"]], ch["changed"][0][1]), (["P3"], ["P1"], ["rating"]))
            wires = by["docs: wires"]["changes"][0]
            self.assertEqual((wires["label"], wires["added"], wires["changed"]), ("8.2 Wires", ["W3"], [("W2", ["To"])]))
            self.assertEqual(by["docs: first"]["changes"][0]["kind"], "new")

        def test_render_is_clean(self):
            text = render(collect(self.root))
            self.assertIn("**5 changes**", text)
            self.assertIn("**1 without a decision ID**", text)
            for banned in ("Some Person", "example.org", "Co-Authored-By", "Signed-off-by", str(self.root)):
                self.assertNotIn(banned, text)

        def test_write_and_check(self):
            self.assertTrue(stale(self.root))
            write(self.root)
            self.assertFalse(stale(self.root))
            self.run_git("add", "-A")
            self.run_git("commit", "-q", "-m", "docs: list")  # committing the list must not make it stale
            self.assertFalse(stale(self.root))
            self.commit({TABLES_DIR + "/parts.csv": "id,name,rating\nP3,relay,6 A\n"}, "later (ID-1)")
            self.assertTrue(stale(self.root))
            write(self.root)
            self.assertFalse(stale(self.root))
            body = (self.root / OUT).read_text(encoding="utf-8")
            self.assertTrue(body.startswith("# Golden-table change list"))
            self.assertEqual(body.count(BEGIN), 1)

        def test_hash_named_in_log(self):
            plain = [e for e in collect(self.root) if e["subject"] == "plain change without any reference"][0]
            log = (self.root / DECISION_LOG).read_text(encoding="utf-8")
            rows = ("| ID-90 | **Mapped.** golden commit `%s`. | Core | x |\n"
                    "| ID-91 | **Not it.** a bare mention %s does not count. | Core | x |\n")
            self.commit({DECISION_LOG: log + rows % (plain["hash"], plain["hash"])}, "docs: map")
            again = [e for e in collect(self.root) if e["hash"] == plain["hash"]][0]
            self.assertEqual(again["ids"], ["ID-90"])

        def test_markers(self):
            ms = markers([{"path": DOC20, "label": "8.2 Wires"}, {"path": TABLES_DIR + "/parts.csv", "label": ""}])
            hit = lambda t: any(m.search(t) for m in ms)
            self.assertTrue(hit("doc 20 section 8.2 W3") and hit("doc 20 §8") and hit("see parts.csv"))
            self.assertFalse(hit("doc 20 §9") or hit("doc 20 §80") or hit("doc 20 §7.3"))

        def test_message_ids(self):
            ids, ranges = message_ids("x ID-3, FT-4, ID-10…ID-20, ID-30.., FT-8.. DEC-2\n\nCo-Authored-By: n ID-99")
            self.assertEqual((ids, ranges), ({"ID-3", "FT-4", "DEC-2"}, [(10, 20), (30, None)]))

    suite = unittest.defaultTestLoader.loadTestsFromTestCase(T)
    return 0 if unittest.TextTestRunner(verbosity=1).run(suite).wasSuccessful() else 1


if __name__ == "__main__":
    if "--test" in sys.argv:
        sys.exit(selftest())
    if "--write" in sys.argv:
        write(ROOT)
        print("golden_changes.py: wrote", OUT)
    elif "--check" in sys.argv:
        if stale(ROOT):
            print("golden_changes.py: stale: %s - run python docs/plan/tools/golden_changes.py --write" % OUT)
            sys.exit(1)
        print("golden_changes.py: up to date")
    else:
        print(block(ROOT))
