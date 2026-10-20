# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: CC0-1.0
"""Generate the golden project source tables (Golden A, J-E-2416) from doc 20.

Reads ``docs/design/20-golden-project.md`` (the ONLY data source) and writes the 23 CSV tables of
``golden/J-E-2416/tables/`` in the dialect documented in ``golden/J-E-2416/README.md``: UTF-8, no BOM, LF, NFC, a
field quoted exactly when it contains ``,``, ``"``, LF, CR or begins/ends with a space, rows in the natural order of
the ``key`` column (``ew_mltext::natural_key``, version 1).

Wherever doc 20 has a markdown table for the data, the script parses that table (so a change to doc 20 changes the
output or fails a check). Values that exist only in prose, test inputs (triangle marks) and fixture choices are
explicit constants with a ``# doc 20 section ...`` comment; every constant that doc 20 states is checked against the
doc cell it comes from, and every fixture choice is recorded in ``FIXTURE_CHOICES``, which the script renders as the
"Test inputs" table of ``golden/J-E-2416/README.md``.

Every check is an explicit ``check(...)`` call (never ``assert``), so it survives ``python -O``.

Usage (from the repository root)::

    python -I golden/J-E-2416/tools/tables_from_doc20.py           # write tables/*.csv and the README test-input table
    python -I golden/J-E-2416/tools/tables_from_doc20.py --check   # exit 1 if a table or the README block differs
    python -I golden/J-E-2416/tools/tables_from_doc20.py --doc PATH [--check]   # read another copy of doc 20

Standard library only; deterministic; no network.
"""

import re
import sys
import unicodedata
from decimal import ROUND_HALF_UP, Decimal, getcontext
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
DEFAULT_DOC_PATH = SCRIPT_DIR / ".." / ".." / ".." / "docs" / "design" / "20-golden-project.md"
TABLES_DIR = SCRIPT_DIR / ".." / "tables"
README_PATH = SCRIPT_DIR / ".." / "README.md"
README_BEGIN = "<!-- test-inputs:begin -->"
README_END = "<!-- test-inputs:end -->"


def fail(message):
    """Stop with a doc-20 mismatch; an exit status of 1 (survives ``python -O``)."""
    raise SystemExit(f"doc 20 check failed: {message}")


def check(condition, message):
    if not condition:
        fail(message)


MINUS = "−"  # U+2212 MINUS SIGN, kept exactly as doc 20 writes it

# ---------------------------------------------------------------------------------------------------------------
# natural order (port of ew_mltext::natural_key, version 1)
# ---------------------------------------------------------------------------------------------------------------


def natural_key(text):
    nfc = unicodedata.normalize("NFC", text)
    nfd = unicodedata.normalize("NFD", nfc)
    chars = []
    for c in nfd:
        if unicodedata.combining(c) != 0:
            continue
        chars.extend(c.lower())
    elems = []
    i = 0
    while i < len(chars):
        c = chars[i]
        if unicodedata.decimal(c, None) is not None:
            j = i
            value = 0
            while j < len(chars) and unicodedata.decimal(chars[j], None) is not None:
                value = value * 10 + unicodedata.decimal(chars[j])
                j += 1
            elems.append((4, value))
            i = j
            continue
        cat = unicodedata.category(c)
        if c.isspace() or cat.startswith("Z") or cat == "Cc":
            cls = 1
        elif cat.startswith("P"):
            cls = 2
        elif cat.startswith("S"):
            cls = 3
        else:
            cls = 5
        elems.append((cls, ord(c)))
        i += 1
    secondary = [ord(c) for c in nfc.lower()]
    tertiary = [ord(c) for c in nfc]
    return (elems, secondary, tertiary)


# ---------------------------------------------------------------------------------------------------------------
# CSV writer
# ---------------------------------------------------------------------------------------------------------------


def csv_field(value):
    v = unicodedata.normalize("NFC", value)
    if any(ch in v for ch in ',"\n\r') or v.startswith(" ") or v.endswith(" "):
        return '"' + v.replace('"', '""') + '"'
    return v


def csv_bytes(columns, rows):
    lines = [",".join(csv_field(c) for c in columns)]
    for row in rows:
        lines.append(",".join(csv_field(row.get(c, "")) for c in columns))
    return ("\n".join(lines) + "\n").encode("utf-8")


# ---------------------------------------------------------------------------------------------------------------
# doc 20 markdown parsing
# ---------------------------------------------------------------------------------------------------------------

DOC_LINES = []
DOC_TEXT = ""


def load_doc(path):
    """Read doc 20 (or a mutated copy of it, for the tests) into the globals the parsers use."""
    global DOC_LINES, DOC_TEXT
    DOC_LINES = Path(path).read_text(encoding="utf-8").splitlines()
    DOC_TEXT = "\n".join(DOC_LINES)


def clean(cell):
    return cell.replace("`", "").strip()


def strip_tri(cell):
    """Remove the unknown markers (triangle, triangle + U-nn) and surrounding blanks."""
    s = re.sub(r"\s*▲(?:U-\d+)?", "", cell)
    return re.sub(r"\s+", " ", s).strip()


def tri_marks(cell):
    """The ``U-nn`` numbers of the unknown marks (``▲U-nn``, ``▲U-nn, U-mm``) in a doc cell, in order, once each."""
    out = []
    for m in re.finditer(r"▲((?:\s*U-\d+,?)+)", cell):
        for u in re.findall(r"U-\d+", m.group(1)):
            if u not in out:
                out.append(u)
    return out


# Every fixture choice the script makes: a value doc 20 gives only as a triangle (▲), only in prose, or that the
# fixture chooses (positions and offsets of test-input representations, junction cells, the frozen tags, ...).
# ``strip_tri`` removes the marks for the values; this registry keeps the provenance. Each entry is recorded once as
# (table, column(s), key(s) or "all", value, doc-20 section, U-nn or "—", reason) and rendered into the README.
FIXTURE_CHOICES = []


def as_text(value):
    return "; ".join(value) if isinstance(value, (list, tuple)) else value


def fixture(table, columns, keys, value, section, reason, cell=None, marks=None):
    """Record one fixture choice. The U-nn marks come from the consumed doc cell (``cell``) or from ``marks``."""
    found = list(marks) if marks is not None else tri_marks(cell or "")
    entry = (table, as_text(columns), as_text(keys), value, section, ", ".join(found) if found else "—", reason)
    check(entry not in FIXTURE_CHOICES, f"fixture choice recorded twice: {entry!r}")
    FIXTURE_CHOICES.append(entry)


def render_fixture_choices():
    """The markdown table between the README's test-input markers."""

    def md(text):
        return text.replace("|", "\\|")

    lines = [
        "| Table | Column(s) | Key(s) | Value | Doc 20 | U-nn | Reason |",
        "|---|---|---|---|---|---|---|",
    ]
    for table, columns, keys, value, section, marks, reason in FIXTURE_CHOICES:
        shown = f"`{value}`" if value != "" else "(empty)"
        shown_keys = keys if keys == "all" else f"`{keys}`"
        cells = (f"`{table}.csv`", f"`{columns}`", shown_keys, shown, section, marks, reason)
        lines.append("| " + " | ".join(md(c) for c in cells) + " |")
    return "\n".join(lines) + "\n"


def find_line(marker):
    for i, line in enumerate(DOC_LINES):
        if line.startswith(marker):
            return i
    fail(f"marker not found: {marker!r}")


def split_row(line):
    return [clean(c) for c in line.strip()[1:-1].split("|")]


def table_after(marker, n=0):
    """Return (header, rows) of the n-th markdown table after the first line starting with ``marker``."""
    i = find_line(marker) + 1
    found = -1
    while i < len(DOC_LINES):
        line = DOC_LINES[i]
        if line.startswith("# ") or line.startswith("## "):
            break
        if line.startswith("|"):
            block = []
            while i < len(DOC_LINES) and DOC_LINES[i].startswith("|"):
                block.append(DOC_LINES[i])
                i += 1
            found += 1
            if found == n:
                header = split_row(block[0])
                rows = []
                for b in block[1:]:
                    cells = split_row(b)
                    if all(re.fullmatch(r":?-+:?", c) for c in cells):
                        continue
                    check(len(cells) == len(header), f"ragged row {b!r}")
                    rows.append(cells)
                return header, rows
            continue
        i += 1
    fail(f"no table {n} after {marker!r}")


def table_dicts(marker, n=0):
    header, rows = table_after(marker, n)
    return [dict(zip(header, r)) for r in rows]


def dec(text):
    """Canonical Dec3 text: no exponent, no trailing zeros."""
    return format(Decimal(text).normalize(), "f")


def expand_range(cell):
    """'-201U3 ... -201U8' -> ['-201U3', ..., '-201U8']; a plain tag returns [tag]."""
    m = re.match(r"^(\S*?)(\d+) … (\S*?)(\d+)\b", cell)
    if not m:
        return [cell.split()[0]]
    check(m.group(1) == m.group(3), f"tag range {cell!r} mixes two prefixes")
    return [f"{m.group(1)}{k}" for k in range(int(m.group(2)), int(m.group(4)) + 1)]


# ---------------------------------------------------------------------------------------------------------------
# table registry
# ---------------------------------------------------------------------------------------------------------------

TABLES = {}  # name -> (columns, rows sorted in natural key order)


def register(name, columns, rows, expected=None):
    check(columns[0] == "key", f"{name}: the first column is not 'key'")
    keys = [r["key"] for r in rows]
    check(len(set(keys)) == len(keys), f"{name}: duplicate keys")
    if expected is not None:
        check(len(rows) == expected, f"{name}: expected {expected} rows, got {len(rows)}")
    for r in rows:
        extra = set(r) - set(columns)
        check(not extra, f"{name}: unknown columns {extra}")
    TABLES[name] = (columns, sorted(rows, key=lambda r: natural_key(r["key"])))


FRAME_GRID = "ew.frame.qet-a4-landscape-grid"
FRAME_NOGRID = "ew.frame.qet-a4-landscape-nogrid"
FUNCTION_KEY = "=P01"
LOCATION_KEY = "+J-E-2416"
FIELD_KEY = "+FIELD"
STORED_FOLIOS = ["1", "3", "101", "102", "103", "106", "201", "202", "203", "204", "205", "206", "902"]

# ---------------------------------------------------------------------------------------------------------------
# project, parties, structure, external refs, sections
# ---------------------------------------------------------------------------------------------------------------


def build_project():
    props = {r["Property"]: r["Golden A value"] for r in table_dicts("### 4.1 Project properties")}
    settings = {r["Setting"]: r["Golden A value"] for r in table_dicts("### 4.4 Project settings and profiles")}
    created, edited = [s.strip() for s in props["Created / edited"].split(",")[0].split("/")]
    date = r"\d{4}-\d\d-\d\d"
    check(re.fullmatch(date, created) and re.fullmatch(date, edited), f"section 4.1 dates: {created!r} / {edited!r}")
    # doc 20 section 4.1 "Supply": phases 1, N, PE, 230 V AC, 50 Hz; TN-S (U-02)
    for needle in ("phases 1, N, PE", "230 V AC", "50 Hz", "TN-S"):
        check(needle in props["Supply"], f"section 4.1 Supply lacks {needle!r}: {props['Supply']!r}")
    # doc 20 section 4.1 "Control voltage" 24 V DC, "Rated consumption" 6 A
    check(props["Control voltage"].startswith("24 V DC"), f"section 4.1 Control voltage: {props['Control voltage']!r}")
    check(props["Rated consumption"] == "6 A", f"section 4.1 Rated consumption: {props['Rated consumption']!r}")
    # doc 20 section 4.4: identifier set, date format and decimal point
    check(settings["Identifier set"].startswith("EN 81346-2:2009"), "section 4.4 Identifier set")
    check("dd-mm-yyyy" in props["Created / edited"], "section 4.1 Created / edited date format")
    check("dd-mm-yyyy" in settings["Date / units"], "section 4.4 Date / units date format")
    check("decimal point" in settings["Date / units"], "section 4.4 Date / units decimal point")
    # doc 03 section 20 / FR-PRJ-011 "first shipped profile"; doc 20 section 4.4 "Derivation profile"
    check(settings["Derivation profile"].startswith("the first shipped profile"), "section 4.4 Derivation profile")
    # doc 20 section 4.4 "Numbering" (doc 03 section 5.1 DeviceNumberingScheme): the page-based default format,
    # the strip rule and the child separator, parsed from the cell
    nm = re.fullmatch(
        r"page-based (\{[^ ;]*); strips (\{[^ ;]*)(?: \([^)]*\))?(?:; ([^;]*) frozen [^;]*)?; children "
        r"\{parent\}([^{ ;]*)\{letter\}\{counter\}; .*",
        settings["Numbering"],
    )
    check(nm, f"section 4.4: unexpected Numbering cell {settings['Numbering']!r}")
    # the cell's frozen clause (decision log ID-93, U-34) must name exactly the fixture's frozen test-input tags
    if nm.group(3) is not None:
        named = set(re.findall(r"-[A-Z0-9]+(?:-[a-z]+)?", nm.group(3)))
        check(named == {"-XPE", "-UC1", "-UC1-plate"}, f"section 4.4: frozen clause names {sorted(named)}")
    fixture(
        "project", "supply_earthing", "all", "tn_s", "§4.1",
        "the Supply cell names TN-S; the earthing system is a ▲", cell=props["Supply"],
    )  # fmt: skip
    fixture(
        "project", "supply_origin", "all", "J-E-206-L", "§4.3, §15",
        "the supply origin is the external point of the L supply, named only in prose",
        cell=next(
            r["Code"] for r in table_dicts("### 4.3 Structure identifiers and sections")
            if r["Aspect"].startswith("External device")
        ),
    )  # fmt: skip
    fixture(
        "project", "derivation_profile", "all", "2027.1", "§4.4",
        'doc 20 names "the first shipped profile"; the fixture pins its version label',
    )  # fmt: skip
    row = {
        "key": props["Object / cabinet id"],
        "document_title": props["Document title (cover subtitle)"],
        "description": props["Project description"],
        "long_description": props["Long description (cover)"],
        "project_ref": props["Project reference"],
        "drawing_id": props["Drawing identification"],
        "manufacturing_year": props["Manufacturing year"],
        "created": created,
        "edited": edited,
        "supply_phases": "1",
        "supply_neutral": "true",
        "supply_pe": "true",
        "supply_earthing": "tn_s",
        "supply_u_nom": "230",
        "supply_frequency": "50",
        "supply_origin": "J-E-206-L",  # doc 20 sections 4.3 / 15: the external point J-E-206-L
        "control_voltage": "24",
        "rated_consumption": "6",
        # ADR-0026 / `ew_rds::sets::EN_81346_2_2009`: stored by key, doc 20 §4.4 shows the display name (ID-92, FT-29)
        "identifier_set": "EN81346-2:2009",
        "date_format": "dd_mm_yyyy",
        "decimal_separator": "point",
        "derivation_profile": "2027.1",
        "numbering_scheme": "page-based",
        "numbering_format": nm.group(1),
        "numbering_strip_format": nm.group(2),
        "numbering_child_separator": nm.group(4),
    }
    cols = [
        "key", "document_title", "description", "long_description", "project_ref", "drawing_id",
        "manufacturing_year", "created", "edited", "supply_phases", "supply_neutral", "supply_pe",
        "supply_earthing", "supply_u_nom", "supply_frequency", "supply_origin", "control_voltage",
        "rated_consumption", "identifier_set", "date_format", "decimal_separator", "derivation_profile",
        "numbering_scheme", "numbering_format", "numbering_strip_format", "numbering_child_separator",
    ]  # fmt: skip
    register("project", cols, [row], 1)


def build_parties():
    rows = {r["Role"]: r["Name"] for r in table_dicts("### 4.2 Parties and bindings")}
    end_user = [p.strip() for p in rows["End user"].split(",")]
    check(len(end_user) == 3, f"section 4.2 End user is not name, street, country: {end_user!r}")
    # no name: logo and notice only
    check("pilot panel builder" in rows["Panel builder"], "section 4.2 Panel builder")
    # doc 20 section 4.2: person and party fields are personal data
    check("marked personal data" in DOC_TEXT, "section 4.2 'marked personal data'")
    cols = ["key", "role", "name", "street", "postcode", "city", "country", "personal_data"]
    data = [
        {"key": "customer", "role": "customer", "name": rows["Customer"], "personal_data": "true"},
        {"key": "end_user", "role": "end_user", "name": end_user[0], "street": end_user[1],
         "country": end_user[2], "personal_data": "true"},
        {"key": "engineer", "role": "engineer", "name": rows["Author / engineer"], "personal_data": "true"},
        {"key": "panel_builder", "role": "panel_builder", "personal_data": "true"},
    ]  # fmt: skip
    # doc 20 section 4.2: the checker is unset in Golden A (U-03): no row
    register("parties", cols, data, 4)


def build_structure_and_external():
    structure = []
    external = None
    for r in table_dicts("### 4.3 Structure identifiers and sections"):
        aspect = r["Aspect"]
        code_cell = strip_tri(r["Code"])
        if aspect.startswith("External device"):
            m = re.match(r"^(\S+) in (\+\S+)", code_cell)
            external = (m.group(1), m.group(2))
            continue
        alias_m = re.search(r'alias "([^"]+)"', code_cell)
        key = code_cell.split(" ")[0]
        check(key[0] == aspect[0], f"section 4.3: code {key!r} does not start with the aspect {aspect!r}")
        structure.append(
            {
                "key": key,
                "aspect": "function" if "function" in aspect else "location",
                "code": key[1:],
                "aliases": alias_m.group(1) if alias_m else "",
                # description left empty: the "Description" column of the doc is commentary (U-05)
            }
        )
    register("structure", ["key", "aspect", "code", "aliases", "parent", "description", "order"], structure, 4)
    fixture(
        "structure", "description", "all", "", "§4.3",
        "the Description column is commentary, not a stored value (the =P01 description is unknown)",
        cell=" ".join(r["Description"] for r in table_dicts("### 4.3 Structure identifiers and sections")),
    )  # fmt: skip
    device, location = external
    # doc 20 section 15 "External connection points": names, cells and the connection letter
    m = re.search(
        r"External connection points \(4, no in-project partner\):\*\* (J-E-206-L), (J-E-206-N), (J-E-206-PE) at"
        r"\s+(\d+)-(G\d+) and (DP) at\s+(\d+)-(G\d+)",
        DOC_TEXT,
    )
    check(m, "section 15: the External connection points paragraph is missing or changed")
    names = [m.group(1), m.group(2), m.group(3), m.group(6)]
    ext = []
    for name in names:
        conn = name[len("J-E-206-"):] if name.startswith("J-E-206-") else name
        ext.append({"key": name, "location": location, "device": device, "connection": conn})
    register("external_refs", ["key", "location", "device", "connection", "project"], ext, 4)
    return m


def build_sections():
    line = next(ln for ln in DOC_LINES if ln.startswith("Sections: "))
    found = re.findall(r'(\d+) "([^"]+)" \((\d+)–(\d+)\)', line)
    rows = [
        {"key": n, "number": n, "folio_from": a, "folio_to": b, "title": t, "divider": "true"}
        for n, t, a, b in found
    ]
    register("sections", ["key", "number", "folio_from", "folio_to", "title", "divider"], rows, 3)


# ---------------------------------------------------------------------------------------------------------------
# pages and report blocks
# ---------------------------------------------------------------------------------------------------------------

PAGE_KIND = [
    ("Title page", "title_page"),
    ("Bus topology", "bus_topology"),
    ("Multi-line schematic", "multi_line"),
    ("Assembly overview", "assembly_overview"),
    ("Fluid-power schematic", "fluid"),
    ("Panel layout", "panel_layout"),
]


def expand_folios(cell):
    if "…" in cell:
        a, b = [int(x) for x in cell.split("…")]
        return [str(k) for k in range(a, b + 1)]
    return [x.strip() for x in cell.split(",")]


def build_pages():
    out = []
    for r in table_dicts("## 5. Pages"):
        if r["Status"] == "generated":
            continue
        folios = expand_folios(r["Folio"])
        desc = r["Page description (title block)"]
        if len(folios) > 1:
            m = re.match(r"^(.*\S) (\d+)(?:, | … )(\d+)$", desc)
            check(m, f"section 5: cannot read the folio titles of {desc!r}")
            titles = [f"{m.group(1)} {k}" for k in range(int(m.group(2)), int(m.group(3)) + 1)]
        else:
            titles = [desc]
        check(len(titles) == len(folios), f"section 5: folios {folios} do not match titles {titles}")
        kind = next(v for k, v in PAGE_KIND if r["Page type (FR-PAG-001)"].startswith(k))
        section = "" if r["Section"] == "—" else r["Section"]
        form = FRAME_GRID if r["Frame"] == "grid" else FRAME_NOGRID
        for f, t in zip(folios, titles):
            out.append(
                {
                    "key": f,
                    "number": f,
                    "section": section,
                    "kind": kind,
                    "title": t,
                    "form": form,
                    "function": FUNCTION_KEY,  # doc 20 section 5 last sentence: every page carries =P01, +J-E-2416
                    "location": LOCATION_KEY,
                    # doc 20 section 13: folio 902 is a view of the plate at 1:10 ▲ (test input)
                    "scale": "10" if f == "902" else "",
                    "grid": "qet10",  # doc 20 section 2.1 rule 4
                }
            )
    cols = ["key", "number", "label", "section", "kind", "title", "form", "function", "location", "scale", "grid"]
    register("pages", cols, out, 13)
    stored = sorted(p["key"] for p in out)
    check(stored == sorted(STORED_FOLIOS), f"section 5: stored folios {stored}, STORED_FOLIOS is {STORED_FOLIOS}")
    fixture(
        "pages", "scale", "902", "10", "§13", "folio 902 is a view of the plate at 1:10, the scale is a ▲",
        cell=next(ln for ln in DOC_LINES if ln.startswith("mounting plate NSYPMM107")),
    )  # fmt: skip


REPORT_KIND = {
    "2": "toc",
    "901": "parts_summary",
    "903": "terminal_diagram",
    "904": "terminal_diagram",
    "905": "terminal_diagram",
    "906": "cable_overview_diagrams",
    "907": "wire_list",
    "908": "plc_overview",
    "909": "potential_overview",
    "910": "structure_overview",
    "911": "revision_overview",
}


def printed_to_nm(mm, scale, offset):
    getcontext().prec = 40
    v = (Decimal(mm) - offset) / scale * Decimal(1000000)
    return str(int(v.quantize(Decimal(1), rounding=ROUND_HALF_UP)))


def build_report_blocks():
    out = []
    for r in table_dicts("## 16. Generated report folios"):
        folio = r["Folio"]
        desc = strip_tri(r["Page description ▲"])
        kind = REPORT_KIND[folio]
        m = re.match(r"^(.*\S) (\d+)(?:–|, )(\d+)$", desc)
        title = m.group(1) + " {n}" if m else desc
        include = ""
        if kind == "terminal_diagram":
            include = desc.split()[-1]
        elif kind == "plc_overview":
            include = m.group(1).split()[-1]
        out.append(
            {
                "key": folio,
                "kind": kind,
                "form": FRAME_NOGRID,
                "section": "" if folio == "2" else "9",
                "first_folio": folio,
                "title": title,
                "include": include,
            }
        )
    check(sorted(b["key"] for b in out) == sorted(REPORT_KIND), f"section 16: report folios {[b['key'] for b in out]}")

    # doc 20 section 17.1: page-to-sheet scale and offset
    m = re.search(r"scale (0\.\d+), offset (\d+\.\d+) mm", DOC_TEXT)
    scale, offset = Decimal(m.group(1)), Decimal(m.group(2))
    check((scale, offset) == (Decimal("0.986282578"), Decimal("9.79")), f"section 17.1: scale {scale}, offset {offset}")

    def rect(x0, y0, x1, y1):
        return ";".join(printed_to_nm(v, scale, offset) for v in (x0, y0, x1, y1))

    geo = {r["Element"]: r["Geometry (printed mm)"] for r in table_dicts("### 17.3 Cover, section dividers, embedded boxes")}
    num = r"(\d+\.\d+)"
    cm = re.match(rf"^{num}–{num} × {num}–{num}", geo["Colour table"])
    sm = re.match(rf"^{num}–{num} and {num}–{num} × {num}–{num}", geo["Size tables"])
    pm = re.match(
        rf"^{num}–{num} × {num}–{num} \(201, 202\), {num}–{num} × same \(902\)",
        geo["PART DATA box"],
    )
    check(cm and sm and pm, "section 17.3: cannot read the colour table, size tables or PART DATA box geometry")
    # doc 20 section 17.3: the cover-tables block spans the colour table plus the size tables
    cover_mm = (cm.group(1), cm.group(3), sm.group(4), sm.group(6))
    check(cover_mm == ("128.52", "47.63", "274.66", "172.89"), f"section 17.3: cover tables {cover_mm}")
    pd_mm = (pm.group(1), pm.group(3), pm.group(2), pm.group(4))
    check(pd_mm == ("16.31", "16.31", "89.38", "52.85"), f"section 17.3: PART DATA box {pd_mm}")
    check((pm.group(5), pm.group(6)) == ("16.31", "71.11"), "section 17.3: PART DATA box on folio 902")
    cover = rect(*cover_mm)
    pd_201 = rect(pm.group(1), pm.group(3), pm.group(2), pm.group(4))
    pd_902 = rect(pm.group(5), pm.group(3), pm.group(6), pm.group(4))
    # doc 20 section 17.3: embedded blocks (geometry through the section 17.1 transform, native nm)
    embedded = [
        ("cover-tables", "profile_tables", "1", cover),
        ("part-data-201", "part_data", "201", pd_201),
        ("part-data-202", "part_data", "202", pd_201),
        ("part-data-902", "part_data", "902", pd_902),
    ]
    for key, kind, page, rc in embedded:
        out.append({"key": key, "kind": kind, "form": FRAME_NOGRID, "embedded_page": page, "embedded_rect": rc})
    cols = [
        "key", "kind", "form", "section", "first_folio", "title", "continuation", "include", "embedded_page",
        "embedded_rect",
    ]  # fmt: skip
    register("report_blocks", cols, out, 15)
    fixture(
        "report_blocks", ["title", "include"], "all", "(text of the §16 cell)", "§16",
        "the whole 'Page description' column is a ▲: titles and the include filters are read from it unchanged",
    )  # fmt: skip


# ---------------------------------------------------------------------------------------------------------------
# parts, part CPs, part templates, cable cores, conductors
# ---------------------------------------------------------------------------------------------------------------

PART_KIND = {
    "UK 5 N": "terminal",
    "USLKG 5": "terminal",
    "PT 2.5-TWIN": "terminal",
    "PT 2.5-TWIN-PE": "terminal",
    "NEBU-G78W5-K-2-N-LE5": "cable",
    "6XV1830-0EH10": "cable",
    "NSYPLA1074G": "enclosure",
    "NSYPMM107": "mounting_plate",
}
UNIT = {"pc": "piece", "m": "m", "set": "set"}
NEBU = "NEBU-G78W5-K-2-N-LE5"
PROFIBUS_CABLE = "6XV1830-0EH10"


def row61(tag):
    """The doc 20 section 6.1 row (a dict of cells) of a device tag."""
    for t, num, r, k in device_rows():
        if t == tag:
            return r
    return fail(f"section 6.1 has no row for {tag!r}")


def parse_cp_list(cell):
    """Expand a section 6.1 'Connection points' list into designations: 'X#n' is the n-th X, a bare '#n' repeats
    the previous designation, 'Out1 … Out4' is Out1 to Out4. The ordinals must run 1, 2, ... in order."""
    out, count, base = [], {}, None
    for tok in [t.strip() for t in cell.split(",")]:
        m = re.fullmatch(r"(.*)#(\d+)", tok)
        if m:
            base = m.group(1) or base
            check(base, f"connection points {cell!r}: {tok!r} has no designation to repeat")
            count[base] = count.get(base, 0) + 1
            check(int(m.group(2)) == count[base], f"connection points {cell!r}: {tok!r} is not #{count[base]}")
            out.append(base)
            continue
        base = None
        m = re.fullmatch(r"(\D+)(\d+) … \1(\d+)", tok)
        if m:
            out.extend(f"{m.group(1)}{k}" for k in range(int(m.group(2)), int(m.group(3)) + 1))
        else:
            out.append(tok)
    return out


X1_ROLE = {"0V": "m", "FE": "fe", "24V": "l_plus"}


def parse_x1_pins(cell):
    """'X1: 1 0V Val, 2 0V El, 3 FE (printed "PE") ▲U-18, ...; FE screw; ...' -> [(designation, potential role)]."""
    m = re.match(r"X1: (.*?); FE screw(?:;|$)", strip_tri(cell))
    check(m, f"connection points of the X1 power block: unexpected cell {cell!r}")
    out = []
    for tok in m.group(1).split(", "):
        pm = re.match(r"(\d) (0V|FE|24V)\b", tok)
        check(pm, f"X1 pin {tok!r} of {cell!r} is not '<pin> <0V|FE|24V> ...'")
        out.append((pm.group(1), X1_ROLE[pm.group(2)]))
    out.append(("FE", "fe"))  # the FE screw
    return out


def build_parts():
    out = []
    for r in table_dicts("### 14.1 Component mode (folio 901)"):
        pn = strip_tri(r["Part number"])
        if pn == "TBD":
            continue
        out.append(
            {
                "key": pn,
                "manufacturer": r["Manufacturer"],
                "part_number": pn,
                "kind": PART_KIND.get(pn, ""),
                "description": strip_tri(r["Description"]),
                "unit": UNIT[r["Unit"]],
            }
        )
    cols = ["key", "manufacturer", "part_number", "kind", "type_designation", "description", "unit", "test_data"]
    register("parts", cols, out, 23)
    for kind in dict.fromkeys(PART_KIND.values()):
        fixture(
            "parts", "kind", [pn for pn, k in PART_KIND.items() if k == kind], kind, "§14.1",
            "the component BOM gives no part kind; the kind follows the part's role in the project",
        )  # fmt: skip
    return {p["key"] for p in out}


def build_part_cps_templates(part_keys):
    cps = []

    def add(part, idx, designation, role="", level="", side="", clamp=""):
        check(part in part_keys, f"part {part!r} of the pattern CPs is not in section 14.1")
        cps.append(
            {
                "key": f"{part}:{idx}",
                "part": part,
                "idx": str(idx),
                "designation": designation,
                "potential_role": role,
                "level": level,
                "side": side,
                "clamp": clamp,
            }
        )

    # doc 20 section 6.1 row 10 (CP column), doc 07 section 9.2, doc 03 section 11: PSU 2904602
    psu = [("L", "l"), ("N", "n"), ("PE", "pe"), ("+", "l_plus"), ("+", "l_plus"), (MINUS, "m"), (MINUS, "m")]
    cell = row61("-102G1")["Connection points"]
    check([d for d, _ in psu] == parse_cp_list(cell), f"section 6.1 row 10: 2904602 CPs are not {cell!r}")
    for i, (d, role) in enumerate(psu):
        add("2904602", i, d, role)
    # doc 20 section 6.1 row 12: MICO 4.10 (no roles)
    mico = ["24VDC", "24VDC", "GND", "GND", "13", "14", "Out1", "Out2", "Out3", "Out4"]
    cell = row61("-102F2")["Connection points"]
    check(mico == parse_cp_list(cell), f"section 6.1 row 12: MICO 4.10 CPs are not {cell!r}")
    for i, d in enumerate(mico):
        add("MICO 4.10", i, d)
    # doc 20 section 6.1 row 19 (X1 pins), doc 07 section 9.2: pin 3 is FE (U-18); row 28: pins 1-5, FE screw
    pins = [("1", "m"), ("2", "m"), ("3", "fe"), ("4", "l_plus"), ("5", "l_plus"), ("FE", "fe")]
    cell = row61("-201U1")["Connection points"]
    check(pins == parse_x1_pins(cell), f"section 6.1 row 19: the 550208 pins and roles are not {cell!r}")
    check(row61("-202U1")["Connection points"] == "X1: pins 1–5; FE screw", "section 6.1 row 28: -202U1 pins")
    for i, (d, role) in enumerate(pins):
        add("550208", i, d, role)
    fixture(
        "part_cps", "potential_role", "550208:2", "fe", "§6.1 row 19",
        "pin 3 is the functional earth, printed 'PE': its role is a ▲", cell=cell,
    )  # fmt: skip
    # doc 20 section 6.1 rows 13/14, doc 03 section 11: clamp a (side 1), b and c (side 2), shaft s1
    check("clamps a, b, c" in row61("-X1031")["Connection points"], "section 6.1 row 13: clamps a, b, c")
    check("clamps a, b, c, rail contact" in row61("-X1032")["Connection points"], "section 6.1 row 14: clamps")
    for part, role in (("PT 2.5-TWIN", ""), ("PT 2.5-TWIN-PE", "pe")):
        add(part, 0, "a", role, "1", "side1", "a")
        add(part, 1, "b", role, "1", "side2", "b")
        add(part, 2, "c", role, "1", "side2", "c")
        add(part, 3, "s1", role, "1", "", "s1")
    add("PT 2.5-TWIN-PE", 4, "RF", "pe", "1", "", "rail")  # rail contact (doc 20 section 9, FR-TRM-017)
    # 7 (2904602) + 10 (MICO 4.10) + 6 (550208) + 4 (PT 2.5-TWIN) + 5 (PT 2.5-TWIN-PE) = 32 pattern CPs.
    cols = ["key", "part", "idx", "designation", "potential_role", "level", "side", "clamp"]
    register("part_cps", cols, cps, 32)

    # The template-origin rule (README "CP and endpoint notation"): a function whose device carries a part whose
    # template declares a function with the same FnDef and role takes its CPs from the part's pattern. `main` is
    # the role: 550208 is the power block of -201U1 and -202U1, whose main function is the overview module
    # (doc 20 section 6.1 rows 19 and 28), so its black box is an auxiliary function.
    templates = [
        ("2904602", "ew.fn.blackbox", 7, "true"),
        ("MICO 4.10", "ew.fn.blackbox", 10, "true"),
        ("550208", "ew.fn.blackbox", 6, "false"),
        ("PT 2.5-TWIN", "ew.fn.terminal", 4, "true"),
        ("PT 2.5-TWIN-PE", "ew.fn.terminal-pe", 5, "true"),
    ]
    rows = []
    for part, fndef, n, main in templates:
        check(sum(1 for c in cps if c["part"] == part) == n, f"part template of {part}: {n} CPs expected")
        rows.append(
            {
                "key": f"{part}:0",
                "part": part,
                "n": "0",
                "fndef": fndef,
                "main": main,
                "cps": ";".join(str(i) for i in range(n)),
            }
        )
    register("part_templates", ["key", "part", "n", "fndef", "main", "cps"], rows, 5)
    fixture(
        "part_templates", "main", "550208:0", "false", "§6.1 rows 19, 28",
        "550208 is the power block of -201U1 and -202U1, whose main function is the overview module",
    )  # fmt: skip


def build_cables_conductors(part_keys):
    by_cable = {}
    for r in table_dicts("## 10. Cables and conductors"):
        if "(A-R only)" in r["Cable"]:
            continue
        by_cable[r["Cable"]] = r
    cores = {}
    for cable in ("-106W1", "-106W2"):
        cores[cable] = re.findall(r"([A-Z]+) W\d+", by_cable[cable]["Cores → wire"])
    check(cores["-106W1"] == cores["-106W2"] == ["BK", "BU", "GNYE", "WH", "BN"], f"section 10: cores {cores}")
    check(NEBU in by_cable["-106W1"]["Part"], "section 10: the -106W1 part is not the NEBU cable")
    check(PROFIBUS_CABLE in by_cable["-3W1"]["Part"], "section 10: the -3W1 part is not the PROFIBUS cable")
    check({NEBU, PROFIBUS_CABLE} <= part_keys, "section 14.1 lacks a cable part")

    cc = []
    for n, core in enumerate(cores["-106W1"]):
        cc.append({"key": f"{NEBU}:{core}", "part": NEBU, "n": str(n), "core": core, "colour": core})
    # doc 20 section 10: -3W1 cores A, B, shield (no colours or cross-sections, U-21)
    profibus = [("A", ""), ("B", ""), ("SH", "true")]
    for n, (core, shield) in enumerate(profibus):
        cc.append(
            {"key": f"{PROFIBUS_CABLE}:{core}", "part": PROFIBUS_CABLE, "n": str(n), "core": core, "shield": shield}
        )
    register("cable_cores", ["key", "part", "n", "core", "colour", "csa", "shield", "pair"], cc, 8)
    sh_cell = row61("-3W1")["Connection points"]
    fixture(
        "cable_cores", "core", f"{PROFIBUS_CABLE}:SH", "SH", "§6.1 row 18, §10",
        "the shield core is named SH; the cores A, B and the shield carry no colour or cross-section", cell=sh_cell,
    )  # fmt: skip

    cond = []
    for cable in ("-106W1", "-106W2"):
        for core in cores[cable]:
            cond.append({"key": f"{cable}:{core}", "cable": cable, "core": core, "colour": core})
    for core, shield in profibus:
        cond.append({"key": f"-3W1:{core}", "cable": "-3W1", "core": core, "shield": shield})
    check("Golden A holds 13 conductors" in DOC_TEXT, "section 10: 'Golden A holds 13 conductors'")
    register("conductors", ["key", "cable", "core", "colour", "csa", "shield", "pair"], cond, 13)
    fixture(
        "conductors", "core", "-3W1:SH", "SH", "§6.1 row 18, §10",
        "the shield conductor takes the shield core of the cable part", cell=sh_cell,
    )  # fmt: skip
    return {c["key"] for c in cond}


# ---------------------------------------------------------------------------------------------------------------
# devices and terminals
# ---------------------------------------------------------------------------------------------------------------

DEVICE_COLS = [
    "key", "letter", "counter", "scheme", "frozen", "frozen_sign", "parent", "position", "function", "location",
    "kind", "parts", "function_text", "process_tag", "state", "strip_label_mode", "strip_groups",
    "strip_field_side", "cable", "cable_length",
]  # fmt: skip

# doc 20 section 6.1 "Part" column mapped to parts.csv keys. ``build_devices_terminals`` checks the map against the
# doc: for every row of section 6.1 the part numbers named in the Part cell (the cells of the strips, "terminals ...",
# name their terminal parts, which section 9 gives) must be exactly the keys listed here, and every tag must be a
# row of section 6.1. Tags without an entry carry no part ("none", "supplied by others").
PARTS_BY_TAG = {
    "S51": "533356",
    "-101F1": "A9P52610",
    "F55": "5SY4204-7",
    "T55": "SK 3110.000",
    "E55": "SK 3102.000",
    "-102G1": "2904602",
    "-102F1": "A9F78116",
    "-102F2": "MICO 4.10",
    "-106W1": NEBU,
    "-106W2": NEBU,
    "-3X1": "532216",
    "-3W1": PROFIBUS_CABLE,
    "-201U1": "195740;550208",
    "-201U2": "552286",
    "-201U9": "533354",
    "-202U1": "550208",
    "-202U3": "533354",
    "-UC1": "NSYPLA1074G",
    "-UC1-plate": "NSYPMM107",
}
MODULES = ["-201U3", "-201U4", "-201U5", "-201U6", "-201U7", "-201U8", "-202U2"]
for _m in MODULES:
    PARTS_BY_TAG[_m] = "533361"
# doc 20 section 2.1 rule 1: the frozen tags (every "(frozen)" tag of section 6.1, checked)
FROZEN = ["S51", "F55", "T55", "E55", "S56", "E56", "1X1"]
RANGE_ROWS = ("21–26", "61–68")


def device_rows():
    """Section 6.1 as ordered (tag, row number, row dict, index inside a range row) tuples."""
    out = []
    for r in table_dicts("### 6.1 Device table"):
        num = r["#"]
        tag_cell = strip_tri(r["Tag (Golden A)"])
        if num in RANGE_ROWS:
            for k, t in enumerate(expand_range(tag_cell.split(" (")[0])):
                out.append((t, num, r, k))
        elif num == "33–60":
            out.append(("VALVES", num, r, 0))
        elif num == "32":
            check("child of -UC1" in tag_cell, f"section 6.1 row 32: not a child of -UC1: {tag_cell!r}")
            out.append(("-UC1-plate", num, r, 0))  # doc 20 section 6.1 row 32: mounting plate, child of -UC1
        else:
            out.append((tag_cell.split(" (")[0].split(" ")[0], num, r, 0))
    return out


def strip_data():
    out = {}
    # 1X1 (doc 20 section 9)
    ts = []
    for k, r in enumerate(table_dicts("**1X1**")):
        raw = r["Part"]
        ts.append(
            {
                "seq": k + 1,
                "designation": r["Terminal"],
                "function_text": "",  # doc 20 section 9: 1X1 terminals carry no function text
                "parts": re.sub(r"\s*\([^)]*\)", "", strip_tri(raw)),
                "state": "",
                "rail_bonded": "true" if "rail-bonded" in raw else "",
            }
        )
    check([t["designation"] for t in ts] == ["1", "2", "PE"], "section 9: the 1X1 terminals are not 1, 2, PE")
    out["1X1"] = {"terminals": ts, "groups": ""}

    def heading_part(marker, rail):
        line = DOC_LINES[find_line(marker)]
        m = re.search(r"; (PT \S+?) ×(\d+)", line)
        check(m, f"section 9: no '; PT ... ×n' in the strip heading {line!r}")
        if rail:
            check("rail-bonded" in line, f"section 9: the strip heading {line!r} is not rail-bonded")
        return m.group(1), int(m.group(2))

    # -X1031
    part, count = heading_part("**-X1031**", False)
    ts, labels = [], []
    for r in table_dicts("**-X1031**"):
        for n in [int(x) for x in re.findall(r"\d+", r["Terminal"])]:
            ts.append(
                {
                    "seq": n,
                    "designation": str(n),
                    "function_text": re.sub(r"\s*\(.*\)$", "", r["Printed (function text)"]),
                    "parts": part,
                    "state": "spare" if r["State"] == "spare" else "",
                    "rail_bonded": "",
                }
            )
            labels.append((n, r["Group label"]))
    check([t["seq"] for t in ts] == list(range(1, count + 1)), f"section 9: -X1031 terminals are not 1..{count}")
    runs = []
    for n, label in labels:
        if runs and runs[-1][2] == label:
            runs[-1][1] = n
        else:
            runs.append([n, n, label])
    out["-X1031"] = {"terminals": ts, "groups": ";".join(f"{a}-{b}:{c}" for a, b, c in runs)}

    # -X1032
    part, count = heading_part("**-X1032**", True)
    ts = []
    for r in table_dicts("**-X1032**"):
        n = int(r["Terminal"])
        ts.append(
            {
                "seq": n,
                "designation": str(n),
                "function_text": r["Printed"],
                "parts": part,
                "state": "spare" if r["State"] == "spare" else "",
                "rail_bonded": "true",  # heading: rail-bonded (FR-TRM-017)
            }
        )
    check([t["seq"] for t in ts] == list(range(1, count + 1)), f"section 9: -X1032 terminals are not 1..{count}")
    out["-X1032"] = {"terminals": ts, "groups": ""}
    return out


def parts_in_cell(cell, part_keys):
    """The part numbers of ``part_keys`` that a section 6.1 Part cell names (whole tokens)."""
    return {pk for pk in part_keys if re.search(rf"(?<![\w.-]){re.escape(pk)}(?![\w.-])", cell)}


def build_devices_terminals(part_keys):
    rows61 = device_rows()
    # section 6.2, valves: state and manual function text per module
    valve_state, valve_text = {}, {}
    for r in table_dicts("### 6.2 Valve terminal and field devices", 0):
        st = r["State"].split(" ")[0]
        txt = r["Printed in the function-text slot (positions 1–4)"].split(" (")[0] if st == "spare" else ""
        for m in expand_range(r["Parent"]):
            valve_state[m] = "spare" if st == "spare" else ""
            valve_text[m] = txt
    check(set(valve_state) == set(MODULES), f"section 6.2: valve modules {sorted(valve_state)}")
    # section 6.2, field devices: process tags and location
    field = {}
    for r in table_dicts("### 6.2 Valve terminal and field devices", 1):
        tags = expand_range(strip_tri(r["Field device (DT ▲U-07)"]))
        ptags = [p.strip() for p in r["Process tag"].split(",")]
        check(len(tags) == len(ptags) == 4, f"section 6.2: field devices {tags} and process tags {ptags}")
        check(strip_tri(r["Location"]) == FIELD_KEY, f"section 6.2: field device location {r['Location']!r}")
        field.update(zip(tags, ptags))
    check(len(field) == 8, f"section 6.2: {len(field)} field devices, 8 expected")

    # children of -201U1 in manifold order (section 6.2): doc rows 20 .. 30
    children = [t for t, num, r, k in rows61 if num in {"20", "21–26", "27", "28", "29", "30"}]
    check(children == [
        "-201U2", "-201U3", "-201U4", "-201U5", "-201U6", "-201U7", "-201U8", "-201U9", "-202U1", "-202U2", "-202U3"
    ], f"section 6.1: children of -201U1 {children}")  # fmt: skip

    # the frozen tags and the PARTS_BY_TAG map against section 6.1 (F6)
    frozen = {t for t, num, r, k in rows61 if "(frozen)" in r["Tag (Golden A)"]}
    check(frozen == set(FROZEN), f"section 6.1: frozen tags {sorted(frozen)}, FROZEN is {sorted(FROZEN)}")
    tags61 = {t for t, num, r, k in rows61}
    check(set(PARTS_BY_TAG) <= tags61, f"PARTS_BY_TAG names tags missing in section 6.1: {set(PARTS_BY_TAG) - tags61}")

    strips = strip_data()
    devices = []

    def add(**kw):
        d = {c: "" for c in DEVICE_COLS}
        d.update(kw)
        devices.append(d)

    for tag, num, r, k in rows61:
        if tag == "VALVES":
            expected = [f"{m}-Q{q}" for m in MODULES for q in range(1, 5)]
            tag_cell = r["Tag (Golden A)"]
            check(tag_cell.startswith(f"{expected[0]} … {expected[-1]}"), f"section 6.1: valve tags {tag_cell!r}")
            check("571334" in r["Part (manufacturer, part number)"], "section 6.1: the valve part is not 571334")
            for m in MODULES:
                for q in range(1, 5):
                    add(
                        key=f"{m}-Q{q}", letter="Q", counter=str(q), scheme="child", parent=m, position=str(q),
                        function=FUNCTION_KEY, location=LOCATION_KEY, parts="571334",
                        state=valve_state[m], function_text=valve_text[m],
                    )  # fmt: skip
            continue
        want = PARTS_BY_TAG.get(tag, "")
        part_cell = strip_tri(r["Part (manufacturer, part number)"])
        named = set() if part_cell.startswith("terminals ") else parts_in_cell(part_cell, part_keys)
        check(
            set(filter(None, want.split(";"))) == named,
            f"section 6.1 Part cell of {tag}: {part_cell!r} names {sorted(named)}, PARTS_BY_TAG has {want!r}",
        )
        d = dict(key=tag, function=FUNCTION_KEY, location=LOCATION_KEY, parts=want)
        if tag in FROZEN:
            d.update(letter=re.search(r"[A-Z]", tag).group(0), frozen=tag, frozen_sign="hidden")
        elif re.fullmatch(r"-203Q\d", tag):
            d.update(letter="Q", counter=tag[-1], location=FIELD_KEY, process_tag=field[tag])
        elif re.fullmatch(r"-\d+[A-Z]\d+", tag):
            m = re.fullmatch(r"-\d+([A-Z])(\d+)", tag)
            d.update(letter=m.group(1), counter=m.group(2))
        elif re.fullmatch(r"-X\d{3}\d", tag):
            # strip rule {letter}{page}{counter} (doc 03 section 8): default scheme, counter = last digit
            d.update(letter="X", counter=tag[-1])
        # doc 20 section 2.1 rule 1: -UC1 and -XPE follow per-class rules (U-34), which the numbering settings do
        # not model yet; they are frozen tags with the sign shown (ADR-0003: a frozen tag is a typed override),
        # so Golden B's `include_frozen: true` renumbers -XPE as section 2.2 expects (test inputs, U-34)
        elif tag == "-XPE":
            d.update(letter="X", frozen="XPE", frozen_sign="shown")
        elif tag == "-UC1":
            d.update(letter="U", counter="1", frozen="UC1", frozen_sign="shown")
        elif tag == "-UC1-plate":
            # doc 20 section 6.1 row 32: the mounting plate, child of -UC1 (test input)
            d.update(letter="U", scheme="child", parent="-UC1", frozen="UC1-plate", frozen_sign="shown")
        else:
            fail(f"unexpected device tag {tag!r}")
        if tag in children:
            d.update(parent="-201U1", position=str(children.index(tag) + 1))
        if tag == "-102G1":
            d["kind"] = "psu"
        if tag in ("-106W1", "-106W2"):
            d.update(cable="true", cable_length="from_part")  # section 10: 2 m from the part's type code
        if tag == "-3W1":
            d.update(cable="true")  # section 10: no length (EF-08), cable_length stays missing (empty)
        # doc 20 section 9 intro: strips print through label_mode (1X1 designation, the others function text)
        if tag == "1X1":
            d["strip_label_mode"] = "designation"
        elif tag in ("-X1031", "-X1032"):
            d["strip_label_mode"] = "function_text"
            d["strip_groups"] = strips[tag]["groups"]
        add(**d)
    register("devices", DEVICE_COLS, devices, 68)
    fixture(
        "devices", ["frozen", "frozen_sign"], ["-XPE", "-UC1", "-UC1-plate"],
        "frozen = the tag without its sign (XPE, UC1, UC1-plate); frozen_sign = shown",
        "§2.1 rule 1, §6.1 rows 9, 31, 32",
        "per-class numbering rules are not modelled yet: the tags are frozen overrides with the sign shown, so"
        " Golden B's include_frozen renumbers -XPE as §2.2 expects",
        cell=row61("-XPE")["Tag (Golden A)"] + " " + row61("-UC1")["Tag (Golden A)"],
    )  # fmt: skip
    fixture(
        "devices", "key", "-UC1-plate", "-UC1-plate", "§6.1 row 32",
        "doc 20 gives the mounting plate no tag ('mounting plate, child of -UC1'): the key is the fixture's name",
        cell=row61("-UC1-plate")["Tag (Golden A)"],
    )  # fmt: skip
    fixture(
        "devices", "cable_length", ["-106W1", "-106W2"], "from_part", "§10",
        "the 2 m length is read from the part's type code; the source of the length is a ▲",
        cell=next(r["Length (source)"] for r in table_dicts("## 10. Cables and conductors") if r["Cable"] == "-106W1"),
    )  # fmt: skip

    term_cols = [
        "key", "strip", "seq", "designation", "function_text", "parts", "state", "rail_bonded", "function", "location",
    ]  # fmt: skip
    terms = []
    for strip_key in ("1X1", "-X1031", "-X1032"):
        for t in strips[strip_key]["terminals"]:
            terms.append(
                {
                    "key": f"{strip_key}:{t['designation']}",
                    "strip": strip_key,
                    "seq": str(t["seq"]),
                    "designation": t["designation"],
                    "function_text": t["function_text"],
                    "parts": t["parts"],
                    "state": t["state"],
                    "rail_bonded": t["rail_bonded"],
                    "function": FUNCTION_KEY,
                    "location": LOCATION_KEY,
                }
            )
    register("terminals", term_cols, terms, 28)
    return {d["key"]: d for d in devices}, terms


# ---------------------------------------------------------------------------------------------------------------
# potentials, profiles, revisions
# ---------------------------------------------------------------------------------------------------------------

# doc 20 section 7.3 "Members": the `at` endpoint of a potential is the first member of its Members cell (``ext.``
# entries skipped); ``build_potentials`` checks every entry against the doc.
POTENTIAL_AT = {
    "L": "1X1:1/field",
    "N": "1X1:2/field",
    "L-S": "S51:1",
    "N-S": "S51:3",
    "L-1": "-101F1:2",
    "N-1": "-101F1:N#2",
    "L-55": "F55:2",
    "N-55": "F55:N#2",
    "N-55T": "T55:3",
    "N-55S": "S56:14",
    "PE": "1X1:PE/field",
    "+24V": "-102G1:+#1",
    "+24V-F": "-102F1:2",
    "24VDC-1": "-102F2:Out1",
    "24VDC-2": "-102F2:Out2",
    "24VDC-3": "-102F2:Out3",
    "24VDC-4": "-102F2:Out4",
    "0V": f"-102G1:{MINUS}#1",
}


# doc 20 section 7.3 "Type, voltage, class" -> ew-schema potential type
POTENTIAL_TYPE = {"L": "l", "N": "n", "PE": "pe", "L+": "l_plus", "M": "m"}


def parse_potential_type(cell):
    """Parse a section 7.3 'Type, voltage, class' cell: ``<type>[ switched][, <n> V <AC|DC>][, <class>]``,
    or ``bus``. Returns None for ``bus``, else a dict (absent parts are ""/False)."""
    if cell == "bus":
        return None
    types = "|".join(re.escape(t) for t in sorted(POTENTIAL_TYPE, key=len, reverse=True))
    m = re.fullmatch(rf"({types})( switched)?(?:, (\d+) V (AC|DC))?(?:, (mains|control))?", cell)
    check(m, f"section 7.3: unexpected 'Type, voltage, class' cell {cell!r}")
    return {
        "ptype": POTENTIAL_TYPE[m.group(1)],
        "switched": bool(m.group(2)),
        "voltage": dec(m.group(3)) if m.group(3) else "",
        "ac_dc": m.group(4).lower() if m.group(4) else "",
        "circuit_class": m.group(5) or "",
    }


def first_member(row):
    """The first in-project entry of a section 7.3 Members cell. An entry of the incoming strip 1X1 that belongs to
    an external connection (the potential has a J-E-206-* alias, wire table W1 to W3) sits on the field side."""
    for entry in [e.strip() for e in row["Members"].split(",")]:
        if entry.startswith("ext."):
            continue
        if entry.split(":")[0] == "1X1" and "J-E-206-" in row["IP aliases"]:
            return entry + "/field"
        return entry
    return fail(f"section 7.3: potential {row['Potential']!r} has no in-project member")


def build_potentials():
    props = {r["Property"]: r["Golden A value"] for r in table_dicts("### 4.1 Project properties")}
    # doc 20 section 4.1 "Supply": 230 V AC, 50 Hz; "Control voltage": 24 V DC, PELV
    hz = re.search(r"(\d+) Hz", props["Supply"])
    check(hz, f"section 4.1: no frequency in the Supply cell {props['Supply']!r}")
    frequency = hz.group(1)
    names, ac_rows, naming_only = [], [], []
    rows = []
    for r in table_dicts("### 7.3 Potentials"):
        n = r["Potential"]
        t = parse_potential_type(r["Type, voltage, class"])
        if t is None:
            # the bus potential: not a row (doc 20 section 11, not covered yet)
            check(n == "DP", f"section 7.3: the bus potential is {n!r}, expected DP")
            continue
        names.append(n)
        check(n in POTENTIAL_AT, f"section 7.3: potential {n!r} is not in POTENTIAL_AT")
        first = first_member(r)
        check(
            POTENTIAL_AT[n] == first,
            f"section 7.3: first member of {n!r} is {first!r}, POTENTIAL_AT has {POTENTIAL_AT[n]!r}",
        )
        row = {"key": n, "at": POTENTIAL_AT[n]}
        if t["switched"]:
            # doc 03 section 9.4 derives the type, including "switched", across the PassThrough{opens: true} pair of
            # T55 and S56: storing it would persist a derived value, so the switched neutrals are naming only
            naming_only.append(n)
        else:
            row["ptype"] = t["ptype"]
            row.update(voltage=t["voltage"], ac_dc=t["ac_dc"], circuit_class=t["circuit_class"])
            if t["ac_dc"] == "ac":
                row["frequency"] = frequency  # doc 20 section 4.1 Supply: 50 Hz; the 7.3 cell gives none
                ac_rows.append(n)
        if n == "0V":
            # doc 20 section 4.1 "Control voltage": 24 V DC, PELV (one 0 V-PE bond, section 7.4)
            check(t["ptype"] == "m" and "PELV" in props["Control voltage"], f"section 4.1/7.3: 0V is not a PELV M: {t}")
            row["earthing"] = "pelv"
        rows.append(row)
    check(names == list(POTENTIAL_AT), f"section 7.3: potentials {names}, POTENTIAL_AT has {list(POTENTIAL_AT)}")
    fixture(
        "potentials", "frequency", ac_rows, frequency, "§4.1, §7.3",
        "the Type cell of §7.3 gives no frequency; every AC potential takes the 50 Hz of the §4.1 Supply", marks=[],
    )  # fmt: skip
    fixture(
        "potentials", ["ptype", "voltage", "ac_dc", "frequency", "circuit_class"], naming_only, "", "§7.3",
        "N switched: naming only; doc 03 §9.4 derives the type, including 'switched', across the"
        " PassThrough{opens: true} pair",
    )  # fmt: skip
    cols = ["key", "at", "ptype", "voltage", "ac_dc", "frequency", "earthing", "circuit_class"]
    register("potentials", cols, rows, 18)


def build_profiles():
    out = []
    n = 0
    for r in table_dicts('### 7.1 Colour profile "DBU/WH 24 V"'):
        if "(3 rows)" in r["Code"]:
            funcs = [x.strip() for x in r["Function"].split(",")]
            pts = [x.strip() for x in r["Potential type"].split(",")]
            check(len(funcs) == len(pts) == 3, f"section 7.1: the 3-row cell {r['Code']!r}: {funcs} / {pts}")
        else:
            funcs, pts = [r["Function"]], [r["Potential type"]]
        code = re.sub(r"\s*\(\d+ rows\)", "", r["Code"])
        for f, p in zip(funcs, pts):
            n += 1
            out.append(
                {
                    "key": str(n),
                    "voltage_class": r["Voltage class"],
                    "function": f,
                    "potential_type": p,
                    "colour": r["Colour"],
                    "code": code,
                }
            )
    register("colour_profile", ["key", "voltage_class", "function", "potential_type", "colour", "code"], out, 15)

    def mm(c):
        return "" if c == "—" else dec(c.replace("mm²", "").strip())

    sizes = [
        {"key": r["Code"], "control": mm(r["Control"]), "mains": mm(r["Mains"])}
        for r in table_dicts("### 7.2 Size profile and class rules")
    ]
    register("size_profile", ["key", "control", "mains"], sizes, 5)


def build_revisions():
    props = {r["Property"]: r["Golden A value"] for r in table_dicts("### 4.1 Project properties")}
    parts = [p.strip() for p in props["Revisions"].split("; V")]
    parts = [p if p.startswith("V") else "V" + p for p in parts]
    status = re.match(r"^(\d+) and (\d+) last changed in V(\d+\.\d+)", props["Page change status"])
    check(status, f"section 4.1: unexpected Page change status {props['Page change status']!r}")
    rows = []
    for p in parts:
        m = re.match(r'^(V\d+\.\d+) (\d{4}-\d\d-\d\d) "([^"]+)", state (\w+)', p)
        check(m, f"section 4.1: unexpected revision {p!r}")
        author = re.search(r"author (<[^>]+>)", p)
        label = m.group(1)
        rows.append(
            {
                "key": label,
                "date": m.group(2),
                "description": m.group(3),
                "state": m.group(4).lower(),
                "author": author.group(1) if author else "",
                # checker unset (U-03)
                "changed_pages": f"{status.group(1)};{status.group(2)}" if label == f"V{status.group(3)}" else "",
            }
        )
    register("revisions", ["key", "date", "description", "state", "author", "checker", "changed_pages"], rows, 2)
    fixture(
        "revisions", "checker", "V2.0", "", "§4.1", "the checker of V2.0 is unset in Golden A (unknown)",
        cell=props["Revisions"],
    )  # fmt: skip


# ---------------------------------------------------------------------------------------------------------------
# functions and representations
# ---------------------------------------------------------------------------------------------------------------

FUNC_COLS = [
    "key", "device", "fndef", "role", "kind", "part", "template", "cp_aliases", "ip_name", "ip_direction",
    "ip_external",
]  # fmt: skip
REP_COLS = ["key", "functions", "page", "symbol", "cell", "dx", "dy", "arrangement"]

# doc 07 section 9.1: symbol of a function by its FnDef, always the "@qet10" edition
SYMBOL = {
    "ew.fn.switch-2p-manual": "ew.iec.switch-2p-manual@qet10",
    "ew.fn.mcb-1pn": "ew.iec.mcb-1pn@qet10",
    "ew.fn.mcb-1p": "ew.iec.mcb-1p@qet10",
    "ew.fn.thermostat-nc": "ew.iec.thermostat-nc@qet10",
    "ew.fn.heater": "ew.iec.heater@qet10",
    "ew.fn.position-switch-no": "ew.iec.position-switch-no@qet10",
    "ew.fn.lamp": "ew.iec.lamp@qet10",
    "ew.fn.pe": "ew.iec.pe@qet10",
    "ew.fn.blackbox": "ew.gen.blackbox@qet10",
    "ew.fn.graphic": "ew.gen.cable-marker@qet10",
    "ew.fn.dp-connector": "ew.bus.dp-connector@qet10",
    "ew.fn.dp-port": "ew.bus.dp-port@qet10",
    "ew.fn.overview": "ew.ovw.module@qet10",
    "ew.fn.valve-5-2-sol-spring": "ew.iso.valve-5-2-sol-spring@qet10",
    "ew.fn.process-valve-2way": "ew.iso.process-valve-2way@qet10",
    "ew.fn.cylinder-double-acting": "ew.iso.cylinder-double-acting@qet10",
    "ew.fn.strip-segment": "ew.gen.strip-segment@qet10",
    "ew.fn.terminal": "ew.iec.terminal@qet10",
    "ew.fn.terminal-pe": "ew.iec.terminal-pe@qet10",
    "ew.fn.ip": "ew.gen.ip@qet10",
}
EXTERNAL_ARROW = "ew.gen.external-arrow@qet10"

# F13: the offsets (QET px) of the k-th interruption point of a coincident group
IP_OFFSETS = [(0, 0), (0, 20), (0, -20), (20, 0)]
IP_GROUPS = {}  # (page, cell, dx, dy) -> representation keys, for the coincident groups (information only)

# doc 20 section 6 / doc 07 section 9.1: main function of each device
MAIN_FNDEF = {
    "S51": "ew.fn.switch-2p-manual",
    "-101F1": "ew.fn.mcb-1pn",
    "F55": "ew.fn.mcb-1pn",
    "T55": "ew.fn.thermostat-nc",
    "E55": "ew.fn.heater",
    "S56": "ew.fn.position-switch-no",
    "E56": "ew.fn.lamp",
    "-XPE": "ew.fn.pe",
    "1X1": "ew.fn.strip-segment",
    "-X1031": "ew.fn.strip-segment",
    "-X1032": "ew.fn.strip-segment",
    "-102G1": "ew.fn.blackbox",
    "-102F1": "ew.fn.mcb-1p",
    "-102F2": "ew.fn.blackbox",
    "-106W1": "ew.fn.graphic",
    "-106W2": "ew.fn.graphic",
    "-3W1": "ew.fn.graphic",
    "-3X1": "ew.fn.dp-connector",
}


def main_fndef(key):
    if key in MAIN_FNDEF:
        return MAIN_FNDEF[key]
    if re.fullmatch(r"-20[12]U\d", key):
        return "ew.fn.overview"
    if re.fullmatch(r"-20[12]U\d-Q\d", key):
        return "ew.fn.valve-5-2-sol-spring"
    if re.fullmatch(r"-203Q\d", key):
        return "ew.fn.process-valve-2way"
    return None  # -UC1, -UC1-plate: no function


def parse_main_reps(cell):
    """'201-E4, -E6, -E8' -> [(folio, cell)]; cells without a grid cell give []."""
    out = []
    folio = None
    for tok in [t.strip() for t in re.sub(r"\([^)]*\)", "", cell).split(",")]:
        m = re.fullmatch(r"(\d+)?-([A-H]\d+)", tok)
        if not m:
            continue
        if m.group(1):
            folio = m.group(1)
        check(folio, f"section 6.1: the Main rep. cell {cell!r} starts without a folio")
        out.append((folio, m.group(2)))
    return out


def parse_ips():
    """Section 15 interruption-point table -> ({name: (src_folio, src_cell, tgt_folio, tgt_cell)}, name order)."""
    ips, order = {}, []
    for r in table_dicts("## 15. Cross-reference acceptance table"):
        sf, sc = re.search(r"(\d+)-([A-H]\d+)", r["Source cell"]).groups()
        tf, tc = re.search(r"(\d+)-([A-H]\d+)", r["Target cell"]).groups()
        ips[r["Name"]] = (sf, sc, tf, tc)
        order.append(r["Name"])
    check(len(ips) == 22, f"section 15: {len(ips)} interruption-point pairs, 22 expected")
    return ips, order


def build_functions_representations(devices, terms, ips, ip_order, ext):
    funcs, reps = [], []

    def fn(**kw):
        d = {c: "" for c in FUNC_COLS}
        d.update(kw)
        funcs.append(d)

    def rep(key, page, cell, symbol, dx="", dy="", functions=None, arrangement=""):
        reps.append(
            {"key": key, "functions": functions or key, "page": page, "symbol": symbol, "cell": cell, "dx": dx,
             "dy": dy, "arrangement": arrangement}
        )  # fmt: skip

    rows61 = {}
    for tag, num, r, k in device_rows():
        rows61.setdefault(tag, r)

    # -- main functions -----------------------------------------------------------------------------------------
    for key in devices:
        fd = main_fndef(key)
        if fd is None:
            continue
        kw = dict(key=key, device=key, fndef=fd)
        if key in ("-102G1", "-102F2"):
            kw.update(kind="black_box", part=PARTS_BY_TAG[key])
        if key == "-102F1":
            aliases = re.findall(r"(\w+) \(alias (\w+)\)", rows61[key]["Connection points"])
            kw["cp_aliases"] = ";".join(f"{a}={b}" for a, b in aliases)
            check(kw["cp_aliases"] == "1=L1;2=T1", f"section 6.1 row 11: CP aliases {kw['cp_aliases']!r}")
        fn(**kw)

    # -- aux functions (doc 20 section 6.1 "Other reps.", 6.2) ----------------------------------------------------
    aux_defs = [
        ("-201U1", "bus", "ew.fn.dp-port", "", ""),
        ("-201U1", "power", "ew.fn.blackbox", "black_box", "550208"),
        ("-202U1", "power", "ew.fn.blackbox", "black_box", "550208"),
    ]
    for m in MODULES:
        aux_defs.append((m, "aux", "ew.fn.blackbox", "black_box", ""))
    for n in range(1, 9):
        aux_defs.append((f"-203Q{n}", "actuator", "ew.fn.cylinder-double-acting", "", ""))
    for dev, name, fd, kind, part in aux_defs:
        check(dev in devices, f"auxiliary function of the unknown device {dev!r}")
        fn(key=f"{dev}#{name}", device=dev, fndef=fd, role="aux", kind=kind, part=part)

    # -- terminals ----------------------------------------------------------------------------------------------
    for t in terms:
        if t["strip"] == "1X1":
            is_pe = t["designation"] == "PE"
            fn(key=t["key"], device=t["key"], fndef="ew.fn.terminal-pe" if is_pe else "ew.fn.terminal")
        elif t["strip"] == "-X1031":
            fn(key=t["key"], device=t["key"], fndef="ew.fn.terminal", part="PT 2.5-TWIN")
        else:
            fn(key=t["key"], device=t["key"], fndef="ew.fn.terminal-pe", part="PT 2.5-TWIN-PE")

    # -- interruption points (section 15) ---------------------------------------------------------------------
    for name in ip_order:
        sf, sc, tf, tc = ips[name]
        # The point at the "Source cell" is the source, the one at the "Target cell" the target (doc 20 section 15);
        # arrow glyphs are presentation, not direction (FR-XRF-009). PE-AIR1/2 and PE-1.3/4 draw an incoming arrow
        # at their source cell: an open point for the design lead.
        fn(key=f"{name}@{sf}", fndef="ew.fn.ip", kind="ip", ip_name=name, ip_direction="source")
        fn(key=f"{name}@{tf}", fndef="ew.fn.ip", kind="ip", ip_name=name, ip_direction="target")
    ext_defs = [
        (ext.group(1), ext.group(4), ext.group(5), "target"),
        (ext.group(2), ext.group(4), ext.group(5), "target"),
        (ext.group(3), ext.group(4), ext.group(5), "target"),
        (ext.group(6), ext.group(7), ext.group(8), "undirected"),
    ]
    for name, folio, cell, direction in ext_defs:
        # External points (doc 20 section 15): unpaired, named like the external reference they target.
        fn(key=f"{name}@{folio}", fndef="ew.fn.ip", kind="ip", ip_name=name, ip_direction=direction, ip_external=name)
    n_ip = sum(1 for f in funcs if f["fndef"] == "ew.fn.ip")
    check(n_ip == 48, f"section 15: {n_ip} interruption-point functions, 48 expected")
    # 66 main + 18 aux + 28 terminal + 48 interruption-point functions
    check(len(funcs) == 66 + 18 + 28 + 48, f"{len(funcs)} functions, 160 expected")
    register("functions", FUNC_COLS, funcs)
    fkeys = {f["key"] for f in funcs}
    fndef_of = {f["key"]: f["fndef"] for f in funcs}

    # ---- representations ----------------------------------------------------------------------------------------
    # main reps parsed from section 6.1 "Main rep." (devices 1-7, 9-12, 15-20, 21-26, 27-30)
    for tag, num, r, k in device_rows():
        if tag in ("VALVES", "-UC1", "-UC1-plate", "1X1", "-X1031", "-X1032") or tag.startswith("-203Q"):
            continue
        cells = parse_main_reps(r["Main rep."])
        if num == "21–26":
            cells = [cells[k]]
        check(len(cells) == 1, f"section 6.1: the Main rep. cell of {tag} names {len(cells)} cells")
        rep(tag, cells[0][0], cells[0][1], SYMBOL[fndef_of[tag]])
    # -X1031 (two segments) and -X1032 (section 6.1 rows 13, 14)
    for tag in ("-X1031", "-X1032"):
        segs = parse_main_reps(rows61[tag]["Main rep."])
        want = 2 if tag == "-X1031" else 1
        check(len(segs) == want, f"section 6.1: the Main rep. cell of {tag} names {len(segs)} cells, {want} expected")
        for i, (folio, cell) in enumerate(segs):
            rep(tag if i == 0 else f"{tag}.2", folio, cell, SYMBOL["ew.fn.strip-segment"], functions=tag)
    # 1X1 is drawn as three terminal symbols (doc 08 section 6.7); cell from section 6.1 row 8 main rep
    f1, c1 = parse_main_reps(rows61["1X1"]["Main rep."])[0]
    for des, dx in (("1", "-20"), ("2", "0"), ("PE", "20")):  # fixture choice (triangle): dx offsets
        sym = SYMBOL["ew.fn.terminal-pe"] if des == "PE" else SYMBOL["ew.fn.terminal"]
        rep(f"1X1:{des}", f1, c1, sym, dx=dx)
    fixture(
        "representations", "dx", ["1X1:1", "1X1:2", "1X1:PE"], "-20; 0; 20", "§6.1 row 8",
        "test input (fixture choice): the three terminal symbols of 1X1 sit side by side in the strip's cell",
        cell=rows61["1X1"]["Main rep."],
    )  # fmt: skip

    # aux reps parsed from section 6.1 "Other reps."
    aux_rep = {}
    for tag, num, r, k in device_rows():
        if tag in ("-201U1", "-202U1") or tag in MODULES:
            cells = re.findall(r"(\d+)-([A-H]\d+)", r["Other reps."])
            if num == "21–26":
                cells = [cells[k]]
            names = {"-201U1": ["bus", "power"], "-202U1": ["power"]}.get(tag, ["aux"])
            check(len(cells) == len(names), f"section 6.1: the Other reps. cell of {tag} names {len(cells)} cells")
            for nm, (folio, cell) in zip(names, cells):
                key = f"{tag}#{nm}"
                aux_rep[key] = (folio, cell)
                rep(key, folio, cell, SYMBOL[fndef_of[key]])

    # valves (triangle test inputs): row D, columns c-3, c-1, c+1, c+3 around the module's aux column c
    fixture(
        "representations", "cell", "-201U3-Q1 … -202U2-Q4",
        "row D, columns c-3, c-1, c+1, c+3 around the module's aux column c", "§6.1 rows 33-60, §15",
        "test input (fixture choice): doc 20 names only the pages 203-206 of the valves, not their cells",
        cell=rows61["VALVES"]["Main rep."],
    )  # fmt: skip
    valve_col = {}
    for m in MODULES:
        folio, cell = aux_rep[f"{m}#aux"]
        c = int(cell[1:])
        for q, off in zip(range(1, 5), (-3, -1, 1, 3)):
            valve_col[f"{m}-Q{q}"] = (folio, c + off)
            rep(f"{m}-Q{q}", folio, f"D{c + off}", SYMBOL["ew.fn.valve-5-2-sol-spring"])

    # field devices (triangle): row G (valve) and row F (actuator) in the column of the driving valve (section 6.2)
    fixture(
        "representations", "cell", "-203Q1 … -203Q8", "row G (§6.1 Main rep.), column of the driving valve",
        "§6.1 rows 61-68, §6.2",
        "test input (fixture choice): the column of a process valve is that of the valve that drives it",
        marks=tri_marks(table_after("### 6.2 Valve terminal and field devices", 1)[0][0]),
    )  # fmt: skip
    fixture(
        "representations", "cell", "-203Q1#actuator … -203Q8#actuator", "row F, column of the driving valve",
        "§6.2", "test input (fixture choice): the actuator is drawn above its process valve",
        marks=tri_marks(table_after("### 6.2 Valve terminal and field devices", 1)[0][0]),
    )  # fmt: skip
    for r in table_dicts("### 6.2 Valve terminal and field devices", 1):
        tags = expand_range(strip_tri(r["Field device (DT ▲U-07)"]))
        drv = re.match(r"^(-\d+U\d+)-Q1", r["Driven by (DeviceLink)"]).group(1)
        for q, t in enumerate(tags, start=1):
            folio, col = valve_col[f"{drv}-Q{q}"]
            fm = re.fullmatch(r"(\d+)-([A-H])", rows61[t]["Main rep."])
            main_rep = rows61[t]["Main rep."]
            check(fm and fm.group(1) == folio, f"section 6.1: Main rep. {main_rep!r} of {t}, expected folio {folio}")
            rep(t, folio, f"{fm.group(2)}{col}", SYMBOL["ew.fn.process-valve-2way"])
            rep(f"{t}#actuator", folio, f"F{col}", SYMBOL["ew.fn.cylinder-double-acting"])

    # interruption points (section 15)
    for name in ip_order:
        sf, sc, tf, tc = ips[name]
        rep(f"{name}@{sf}", sf, sc, SYMBOL["ew.fn.ip"])
        rep(f"{name}@{tf}", tf, tc, SYMBOL["ew.fn.ip"])
    # external points (section 15 prose); dy offsets of the three mains points are fixture choices (triangle)
    dys = {ext.group(1): "-20", ext.group(2): "0", ext.group(3): "20"}
    for name, folio, cell, direction in ext_defs:
        rep(f"{name}@{folio}", folio, cell, EXTERNAL_ARROW, dy=dys.get(name, ""))
    fixture(
        "representations", "dy", [f"{n}@{ext.group(4)}" for n in dys], "-20; 0; 20", "§15",
        "test input (fixture choice): the three mains points share the cell 101-G0 and are stacked",
    )  # fmt: skip

    # F13: up to four interruption points sit on the identical point (examples: 103-G6, 103-B6, 103-B14, 103-G1,
    # 106-E1, 101-H16, 102-A1, 103-A1). Each coincident group is spread by multiples of 10 px, inside the 60 x 80 px
    # cell and on the 10 px grid; the k-th member (natural key order) gets the k-th offset.
    ip_reps = [rp for rp in reps if fndef_of[rp["functions"]] == "ew.fn.ip"]
    groups = {}
    for rp in sorted(ip_reps, key=lambda rp: natural_key(rp["key"])):
        groups.setdefault((rp["page"], rp["cell"], rp["dx"] or "0", rp["dy"] or "0"), []).append(rp)
    for (page, cell, dx0, dy0), members in groups.items():
        check(len(members) <= len(IP_OFFSETS), f"{len(members)} interruption points coincide at {page}-{cell}")
        for k, rp in enumerate(members):
            if k:  # the first member keeps the point
                rp["dx"], rp["dy"] = str(int(dx0) + IP_OFFSETS[k][0]), str(int(dy0) + IP_OFFSETS[k][1])
    coincident = {g: [rp["key"] for rp in members] for g, members in groups.items() if len(members) > 1}
    places = [(rp["page"], rp["cell"], rp["dx"] or "0", rp["dy"] or "0") for rp in ip_reps]
    check(len(set(places)) == len(places), "two interruption-point representations share (page, cell, dx, dy)")
    # ▲ test input (fixture choice): separates coincident interruption points; the cell is what doc 20 §15 asserts
    fixture(
        "representations", ["dx", "dy"], "interruption points that share a cell",
        "k-th member of a coincident group (natural key order): +(0,0), +(0,20), +(0,-20), +(20,0) px", "§15",
        "▲ test input (fixture choice): separates coincident interruption points; the cell is what doc 20 §15"
        " asserts",
    )  # fmt: skip
    IP_GROUPS.update(coincident)
    # doc 20 §6.2: valve and field tags are not printed on 203-206 (property arrangement, FR-SYM-012). The fixture
    # selects the symbol's arrangement without the Tag slot, named "ports", for the representations of the three
    # valve-terminal and field-device symbols on those folios.
    check(
        "Valve and field tags are not printed on 203–206 (property arrangement, FR-SYM-012)" in DOC_TEXT,
        "section 6.2: 'Valve and field tags are not printed on 203–206 (property arrangement, FR-SYM-012)'",
    )
    tagless = {
        SYMBOL["ew.fn.valve-5-2-sol-spring"], SYMBOL["ew.fn.process-valve-2way"],
        SYMBOL["ew.fn.cylinder-double-acting"],
    }  # fmt: skip
    arranged = []
    for rp in reps:
        if rp["symbol"] in tagless and rp["page"] in ("203", "204", "205", "206"):
            rp["arrangement"] = "ports"
            arranged.append(rp["key"])
    check(len(arranged) == 28 + 8 + 8, f"section 6.2: {len(arranged)} valve/field representations, 44 expected")
    fixture(
        "representations", "arrangement",
        "-201U3-Q1 … -202U2-Q4; -203Q1 … -203Q8; -203Q1#actuator … -203Q8#actuator", "ports", "§6.2",
        "test input (fixture choice): the symbol's arrangement without the Tag slot (FR-SYM-012)",
        marks=[],
    )  # fmt: skip
    for rp in reps:
        check(rp["functions"] in fkeys, f"representation {rp['key']!r} shows the unknown function {rp['functions']!r}")
        check(rp["page"] in STORED_FOLIOS, f"representation {rp['key']!r} is on the unknown page {rp['page']!r}")
    register("representations", REP_COLS, reps)
    return fkeys


# ---------------------------------------------------------------------------------------------------------------
# connections, junctions, bonds
# ---------------------------------------------------------------------------------------------------------------

CONN_COLS = [
    "key", "wire", "page", "a", "b", "kind", "core", "bridge_group", "bridge_kind", "bridge_shaft", "csa",
    "colour", "show_marker",
]  # fmt: skip


def normalise_endpoint(cell, folio):
    s = strip_tri(cell)
    m = re.match(r"^ext\. (J-E-206-(?:L|N|PE))\b", s)
    if m:
        return f"ip:{m.group(1)}@{folio}"
    s = re.sub(r"\s*\([^)]*\)\s*$", "", s).strip()
    m = re.match(r"^(\S+) (field|internal)$", s)
    if m:
        return f"{m.group(1)}/{m.group(2)}"
    return s


def parse_junction_sequence(cell):
    """A section 8.3 Sequence cell: 'F55:2 → W13, W14' gives ('from', 'F55:2', [W13, W14]); 'W20, W21 into
    -102F1:1' gives ('into', '-102F1:1', [W20, W21])."""
    m = re.fullmatch(r"(W\d+(?:, W\d+)*) into (\S+)", cell)
    if m:
        return "into", m.group(2), re.findall(r"W\d+", m.group(1))
    # decision log ID-113: 'W20, -102F1:1, W21 (far ends in order: A, -102F1:1, B)' lists the chain in sequence order
    # with the shared CP in the middle; whether the wires end or start at it comes from the wire table
    m = re.fullmatch(r"(W\d+), (\S+), (W\d+) \(far ends in order: ([^)]*)\)", cell)
    if m:
        far = [s.strip() for s in m.group(4).split(",")]
        check(len(far) == 3 and far[1] == m.group(2), f"section 8.3: far ends {far!r} do not centre on {m.group(2)!r}")
        return "chain", m.group(2), [m.group(1), m.group(3)]
    m = re.fullmatch(r"(\S+) → (W\d+(?:, W\d+)*)", cell)
    check(m, f"section 8.3: unexpected junction Sequence cell {cell!r}")
    return "from", m.group(1), re.findall(r"W\d+", m.group(2))


def junction_plan(junction, wires):
    """The chain arms and the junction sequences that section 8.3 and the wire table (section 8.2) state, as
    ({junction: [(key, wire, a, b)]}, {junction: 'key;key;...'}). ``junction`` carries the parsed Sequence cell."""
    ends, split = {}, set()
    for r in wires:
        folio = re.match(r"^(\d+)(?:→(\d+))?$", r["Folio"])
        f, t = folio.group(1), folio.group(2)
        ends[f"W{r['W']}"] = (normalise_endpoint(r["From"], f), normalise_endpoint(r["To"], t or f))
        if t:
            split.add(f"W{r['W']}")
    chain, seqs = {}, {}
    for jn, v in junction.items():
        how, endpoint, ws = v["sequence"]
        # a wire that crosses to another folio is drawn in two parts: the part on the junction's page is W<n>.<folio>
        shown = [f"{w}.{v['page']}" if w in split else w for w in ws]
        if v["mode"] == "chain":
            check(len(ws) == 2, f"section 8.3: chain junction {jn} has {len(ws)} wires, 2 expected")
            first, second = ws
            if how == "chain":
                # ID-113 notation: the direction follows the wire table
                if ends[first][1] == endpoint and ends[second][1] == endpoint:
                    how = "into"
                else:
                    how = "from"
                far = re.search(r"far ends in order: ([^)]*)\)", v["sequence_cell"]).group(1)
                far = [s.strip() for s in far.split(",")]
                other = (lambda w: ends[w][0] if how == "into" else ends[w][1])
                check(far == [other(first), endpoint, other(second)],
                      f"section 8.3: {jn} far ends {far} do not match the wire table {[other(first), endpoint, other(second)]}")
            if how == "into":
                # W20, W21 into E: both wires end at E; the junction sits on the first wire's way into E
                check(ends[first][1] == ends[second][1] == endpoint, f"wire table: {ws} do not end at {endpoint}")
                chain[jn] = [
                    (f"{jn}.1", "", ends[first][0], f"@{jn}"),
                    (first, first, f"@{jn}", endpoint),
                    (second, second, ends[second][0], f"@{jn}"),
                ]
            else:
                # F -> W24, W25: both wires start at F; the junction sits on the first wire's way out of F
                check(ends[first][0] == ends[second][0] == endpoint, f"wire table: {ws} do not start at {endpoint}")
                chain[jn] = [
                    (f"{jn}.1", "", f"@{jn}", ends[first][1]),
                    (first, first, endpoint, f"@{jn}"),
                    (second, second, f"@{jn}", ends[second][1]),
                ]
            seqs[jn] = ";".join([f"{jn}.1"] + shown)
        else:
            check(v["mode"] == "star" and how == "from", f"section 8.3: star junction {jn} needs a 'from' Sequence")
            check(endpoint == v["feed"], f"section 8.3: {jn} Sequence feed {endpoint!r}, Wiring feed {v['feed']!r}")
            seqs[jn] = ";".join([f"{jn}.feed"] + shown)
    return chain, seqs


def build_connections(ips, device_keys, conductor_keys, fkeys):
    wires = table_dicts("### 8.2 Wire table")
    check(len(wires) == 44, f"section 8.2: {len(wires)} wires, 44 expected")
    junction = {}
    for r in table_dicts("### 8.3 Junctions and bridges", 0):
        wiring_cell = r["Wiring"]
        explicit = "(set)" in wiring_cell
        if not explicit:
            check("(default)" in wiring_cell, f"section 8.3: Wiring cell {wiring_cell!r}: neither (set) nor (default)")
        mode = wiring_cell.split(" ")[0].lower()
        fm = re.search(r"from (?:the PSU CP )?(\S+)$", wiring_cell)
        junction[r["Junction"]] = {
            "page": r["Folio"],
            "style": r["Style"].lower(),
            "mode": mode,
            "wiring": mode if explicit else "",
            "feed": fm.group(1) if fm else None,
            "sequence": parse_junction_sequence(r["Sequence"]),
            "sequence_cell": r["Sequence"],
        }
    check(list(junction) == ["J1", "J2", "J3", "J4", "J5", "J6"], f"section 8.3: junctions {list(junction)}")
    plan_chain, plan_seqs = junction_plan(junction, wires)

    rows = []

    def conn(key, wire, page, a, b, kind="", core="", csa="", marker=""):
        rows.append(
            {"key": key, "wire": wire, "page": page, "a": a, "b": b, "kind": kind, "core": core, "csa": csa,
             "show_marker": marker}
        )  # fmt: skip

    # doc 20 section 8.3 + doc 03 section 9.2: chain junctions J3, J4; the non-first arm carries the wire ID
    chain = {
        "J3": [("J3.1", "", "-102G1:+#1", "@J3"), ("W20", "W20", "@J3", "-102F1:1"),
               ("W21", "W21", "-102G1:+#2", "@J3")],
        "J4": [("J4.1", "", "@J4", "-102F2:24VDC#1"), ("W24", "W24", "-102F1:2", "@J4"),
               ("W25", "W25", "@J4", "-102F2:24VDC#2")],
    }  # fmt: skip
    check(chain == plan_chain, f"section 8.3: the chain arms are {plan_chain}, the script has {chain}")
    chain_arm = {key: (jn, a, b) for jn, arms in chain.items() for key, wire, a, b in arms}
    feed_done = set()
    prev_via = ""
    for r in wires:
        wid = f"W{r['W']}"
        fm = re.match(r"^(\d+)(?:→(\d+))?$", r["Folio"])
        f, t = fm.group(1), fm.group(2)
        via = prev_via if r["Via"] == "idem" else r["Via"]
        prev_via = via
        tokens = [x.strip() for x in via.split(";")]
        ipname = next((x[3:] for x in tokens if x.startswith("IP ")), None)
        jn = next((x for x in tokens if re.fullmatch(r"J\d", x)), None)
        frm = normalise_endpoint(r["From"], f)
        to = normalise_endpoint(r["To"], t or f)
        size = r["Size, colour (source)"]
        csa = marker = kind = core = ""
        cm = re.match(r"^([\d.]+) \(CDP\)", size)
        if cm:
            csa, marker = dec(cm.group(1)), "true"  # the CDPs set the cross-section only (doc 20 section 8.1)
        km = re.match(r"^(-\d+W\d+) ([A-Z]+) \(core", size)
        if km:
            kind, core = "core", f"{km.group(1)}:{km.group(2)}"
            check(core in conductor_keys, f"section 8.2: wire {wid} names the unknown core {core!r}")

        def put(key, page, a, b):
            primary = key == wid  # CDP csa / marker only on the connection keyed W<n>
            conn(key, wid, page, a, b, kind, core, csa if primary else "", marker if primary else "")

        if jn in chain:
            plain = ipname is None and t is None and wid in chain_arm
            check(plain, f"section 8.2: chain wire {wid} is not a plain chain arm")
            _, a, b = chain_arm[wid]
            put(wid, f, a, b)
            continue
        a = frm
        if jn:
            check(junction[jn]["mode"] == "star", f"section 8.2: wire {wid} runs through {jn}, which is not a star")
            feed = junction[jn]["feed"]
            check(frm == feed, f"section 8.2: wire {wid} starts at {frm!r}, {jn} is fed from {feed!r}")
            a = f"@{jn}"
            if jn not in feed_done:
                feed_done.add(jn)
                conn(f"{jn}.feed", "", junction[jn]["page"], junction[jn]["feed"], f"@{jn}")
        if ipname:
            sf, _, tf, _ = ips[ipname]
            check(t and {f, t} == {sf, tf}, f"section 8.2/15: wire {wid} folios {f}->{t}, IP {ipname} folios {sf}/{tf}")
            put(wid if tf == f else f"{wid}.{f}", f, a, f"ip:{ipname}@{f}")
            put(wid if tf == t else f"{wid}.{t}", t, f"ip:{ipname}@{t}", to)
        else:
            check(t is None, f"section 8.2: wire {wid} crosses folios without an interruption point")
            put(wid, f, a, to)
    # the chain junction arms that carry no wire ID
    for jn, arms in chain.items():
        for key, wire, a, b in arms:
            if not wire:
                conn(key, "", junction[jn]["page"], a, b)

    # bridges (doc 20 section 8.3 second table)
    per_strip = {}
    bridges = 0
    for r in table_dicts("### 8.3 Junctions and bridges", 1):
        n = int(r["Bridge"][2:])
        m = re.match(r"^(\d+)–(\d+)", r["Terminals"])
        i, j = int(m.group(1)), int(m.group(2))
        strip = r["Strip"]
        per_strip[strip] = per_strip.get(strip, 0) + 1
        for k in range(1, j - i + 1):
            rows.append(
                {
                    "key": f"BR{n}.{k}", "wire": f"BR{n}", "a": f"{strip}:{i + k - 1}/s1",
                    "b": f"{strip}:{i + k}/s1", "kind": "bridge", "bridge_group": str(per_strip[strip]),
                    "bridge_kind": "plug_in",
                }
            )  # fmt: skip
            bridges += 1
    check(bridges == 19, f"section 8.3: {bridges} bridge connections, 19 expected")
    n_wires = sum(1 for x in rows if x["wire"].startswith("W") and x["key"] == x["wire"])
    check(n_wires == 44, f"{n_wires} wire connections, 44 expected")
    register("connections", CONN_COLS, rows, 72 + 19)

    # endpoint sanity: device or terminal exists, IP function exists
    for x in rows:
        for end in (x["a"], x["b"]):
            if end.startswith("ip:"):
                check(end[3:] in fkeys, f"connection endpoint {end!r}: no such interruption point")
            elif not end.startswith("@"):
                check(end.split(":")[0] in device_keys, f"connection endpoint {end!r}: no such device or strip")

    # junctions.csv: cells are fixture choices (test positions, triangle); doc 20 section 8.3 gives the folio
    cells = {"J1": "C11", "J2": "C13", "J3": "C6", "J4": "D7", "J5": "D5", "J6": "D6"}
    fixture(
        "junctions", "cell", list(cells), "; ".join(cells.values()), "§8.3",
        "test input (fixture choice): doc 20 gives the folio of a junction, not its cell",
    )  # fmt: skip
    seqs = {
        "J1": "J1.feed;W13;W14",
        "J2": "J2.feed;W15;W16",
        "J3": "J3.1;W20;W21",
        "J4": "J4.1;W24;W25",
        "J5": "J5.feed;W22;W27.102",
        "J6": "J6.feed;W23;W26.102",
    }
    check(seqs == plan_seqs, f"section 8.3: the junction sequences are {plan_seqs}, the script has {seqs}")
    keys = {x["key"] for x in rows}
    jr = []
    for j, v in junction.items():
        missing = [k for k in seqs[j].split(";") if k not in keys]
        check(not missing, f"junction {j}: the sequence {seqs[j]!r} names the missing connections {missing}")
        jr.append(
            {"key": j, "page": v["page"], "cell": cells[j], "style": v["style"], "wiring": v["wiring"],
             "sequence": seqs[j]}
        )  # fmt: skip
    register("junctions", ["key", "page", "cell", "dx", "dy", "style", "wiring", "sequence"], jr, 6)

    # bonds.csv (doc 20 section 7.4); b carries the clamp of W27's To end
    line = next(ln for ln in DOC_LINES if ln.startswith("**B1:**")).replace("`", "")
    bm = re.search(r"Bond\{a: (\S+), b: (\S+), kind: (\w+), via: (W\d+)\}", line)
    check(bm and bm.group(3) == "Pelv0vPe", "section 7.4: the bond B1 is not a Pelv0vPe")
    w = next(x for x in rows if x["key"] == bm.group(4))
    check(w["b"].startswith(bm.group(2) + "/"), f"section 7.4: {bm.group(2)!r} is not an end of {bm.group(4)}")
    register(
        "bonds",
        ["key", "a", "b", "kind", "via"],
        [{"key": "B1", "a": bm.group(1), "b": w["b"], "kind": "pelv_0v_pe", "via": bm.group(4)}],
        1,
    )


# ---------------------------------------------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------------------------------------------


def build_all():
    build_project()
    build_parties()
    ext = build_structure_and_external()
    build_sections()
    build_pages()
    build_report_blocks()
    part_keys = build_parts()
    build_part_cps_templates(part_keys)
    conductor_keys = build_cables_conductors(part_keys)
    devices, terms = build_devices_terminals(part_keys)
    build_potentials()
    build_profiles()
    build_revisions()
    ips, ip_order = parse_ips()
    fkeys = build_functions_representations(devices, terms, ips, ip_order, ext)
    build_connections(ips, set(devices) | {t["key"] for t in terms}, conductor_keys, fkeys)
    check(len(TABLES) == 23, f"{len(TABLES)} tables built, 23 expected: {sorted(TABLES)}")


def readme_with_block(text, block):
    """``text`` (the README) with the test-input table between the markers replaced by ``block``; None when a marker
    is missing."""
    b, e = text.find(README_BEGIN), text.find(README_END)
    if b < 0 or e < b:
        return None
    return text[: b + len(README_BEGIN)] + "\n\n" + block + "\n" + text[e:]


def usage(message):
    print(f"{message}\nusage: tables_from_doc20.py [--check] [--doc PATH]", file=sys.stderr)
    return 2


def main(argv):
    args, check_only, doc = list(argv), False, DEFAULT_DOC_PATH
    while args:
        arg = args.pop(0)
        if arg == "--check":
            check_only = True
        elif arg == "--doc" and args:
            doc = Path(args.pop(0))
        else:
            return usage(f"unknown arguments: {argv}")
    if doc != DEFAULT_DOC_PATH and not check_only:
        return usage("--doc only together with --check (the committed tables are written from doc 20 itself)")
    load_doc(doc)
    build_all()
    outputs = {name: csv_bytes(cols, rows) for name, (cols, rows) in sorted(TABLES.items())}
    for name, (cols, rows) in sorted(TABLES.items()):
        print(f"{name}.csv: {len(rows)} rows")
    print(f"README test inputs: {len(FIXTURE_CHOICES)} rows")
    readme = README_PATH.read_text(encoding="utf-8")
    new_readme = readme_with_block(readme, render_fixture_choices())
    if check_only:
        bad = []
        for name, data in outputs.items():
            path = TABLES_DIR / f"{name}.csv"
            if not path.exists():
                bad.append(f"missing: {name}.csv")
            elif path.read_bytes() != data:
                bad.append(f"differs: {name}.csv")
        if TABLES_DIR.exists():
            for path in sorted(TABLES_DIR.glob("*.csv")):
                if path.stem not in outputs:
                    bad.append(f"extra: {path.name}")
        if new_readme is None:
            bad.append(f"README.md: the markers {README_BEGIN} and {README_END} are missing")
        elif new_readme != readme:
            bad.append("differs: the test-input table of README.md")
        for line in bad:
            print(line)
        return 1 if bad else 0
    check(new_readme is not None, f"README.md lacks the markers {README_BEGIN} and {README_END}")
    TABLES_DIR.mkdir(parents=True, exist_ok=True)
    for name, data in outputs.items():
        (TABLES_DIR / f"{name}.csv").write_bytes(data)
    if new_readme != readme:
        with open(README_PATH, "w", encoding="utf-8", newline="") as f:
            f.write(new_readme)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
