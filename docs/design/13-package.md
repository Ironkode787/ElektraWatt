# Manufacturing outputs: export profiles and the production package

A chapter of [Manufacturing outputs](13-manufacturing-outputs.md): how a file for another program is written from a
list's rows (§7), and the one file a job's workshop gets (§8). The wire records, the lists and the label files it
writes are §2 to §6 of [13](13-manufacturing-outputs.md), [the workshop lists](13-workshop-lists.md) and
[markers and labels](13-markers-labels.md); a reference to §7 or §8 there reads here.

---

## 7. Export profiles

### 7.1 What a profile is (FR-MFG-018)

An **export profile** says how a file for another program is written from the rows of a list: which of the list's
columns, in which order and under which headings, the form of the file, and for a machine what the machine takes. It
is project data, a typed member of `WorkshopSettings` ([13](13-manufacturing-outputs.md) §2.3), seen and changed in the window (§7.4); a project without
one of its own uses the generic profile of the kind (§7.5). There are two kinds, declared once
(`ew_derive::ProfileKind`), and everything that lists them reads that declaration:

| Kind | Member | Rows of | Its file | Written by |
|---|---|---|---|---|
| Label file | `workshop.label_profile` | `labels` ([13](13-manufacturing-outputs.md) §5.3) | one per label set and marker part ([13](13-manufacturing-outputs.md) §5.4) | Labels…, `ew export labels`, the package |
| Wire file for a machine | `workshop.machine_profile` | `wire_list` ([13](13-manufacturing-outputs.md) §3.1) | the wire file and the list for the hand (§7.3) | the package, `ew export package` |

```rust
pub struct ExportProfile {                    // absent members: the defaults below; a newer format's member is kept
    pub columns: Vec<ProfileColumn>,          // in file order
    pub file: Option<FileForm>,               // csv | xlsx; absent: the kind's (the label file xlsx, the wire file csv)
    pub delimiter: Delimiter,                 // comma (default) | semicolon | tab; a CSV's
    pub encoding: Encoding,                   // utf8_bom (default) | utf8; a CSV's
    pub decimal: DecimalMark,                 // point (default) | comma
    pub no_header: bool,                      // default false: the first row holds the headings
    pub values: ValueForm,                    // as_printed (default) | numbers
    pub takes: Option<MachineTakes>,          // what the machine takes; read by the wire file alone (§7.3)
}
pub struct ProfileColumn {
    pub key: String,                          // a column key of the kind's list (`text_1`, `length`)
    pub heading: String,                      // what the file prints above it
    pub required: bool,                       // default false; a wire whose cell is empty goes to the hand (§7.3)
}
pub struct MachineTakes {
    pub csa_min: Option<Dec3>,                // mm²; absent: no lower bound
    pub csa_max: Option<Dec3>,                // mm²; absent: no upper bound
    pub ends: Option<Vec<TreatmentKind>>,     // ferrule | twin_ferrule | ring_lug | bare; absent: every one
}
```

A key the list does not have (a column of a newer build) gives an empty column, and `project.settings.set` refuses
a key that no column of the kind's list has, naming the keys it takes. A profile equal to its kind's generic one with
nothing more is stored as absent, so a project that never changed its profile follows the generic profile of the
program that opens it.

### 7.2 The form of a file

- **CSV:** the profile's delimiter; UTF-8 with a byte-order mark, which spreadsheets and label programs read as
  UTF-8, or without one for a program that would read the mark as text; CR LF line ends; the header row unless
  `no_header`; a field quoted exactly when it holds the delimiter, `"`, CR or LF, quotes doubled. Every value is
  written as it is: never the formula guard of the Excel presets ([09](09-output.md) §4.6), which a
  label program or a machine would take as part of the value.
- **Excel:** one sheet, named after the label set or the list; the header row unless `no_header`; text cells for
  texts and number cells for numbers; without the `id` column and the hidden sheets of the round trip.
- **Values.** *As printed*: every cell as the list prints it in the project's language (`1.5 mm²`, `400 mm`,
  `ferrule 1.5 BK`). *Numbers*: a cell that holds a quantity prints its number in the unit of its column and no
  unit (a cross-section in mm², a length and a strip length in mm: `1.5`, `400`, `8`); a count is a number either
  way; a text stays a text. A colour is its code in both forms (`BK`, `GNYE`), as every list prints it.
- **Decimal mark:** a point or a comma in every number the file holds, a CSV's and an Excel file's number cells
  alike; a CSV whose delimiter is a comma quotes a number with a decimal comma.
- **Rows:** the list's rows in its order; a heading row of the list is no row of a file ([13](13-manufacturing-outputs.md) §3.1's Bridges heading: a
  bridge is mounted with its strip, [the workshop lists](13-workshop-lists.md) §6, and stands in neither the wire file
  nor the list for the hand).

The one writer of a file by profile is `ew-reports`' (`profile`), over the list's table ([09](09-output.md)
[13](13-manufacturing-outputs.md) §4.6's `Table`, whose cells are typed): the label files and the wire file both call it.

### 7.3 The wire file and the list for the hand (FR-MFG-024, FR-MFG-050)

The **wire file** holds every wire of the wire list the machine makes; the **list for the hand** holds every other
wire, each with the reason. Each wire stands in exactly one of them, and the two together are the wire list's wires
(AC-35). A wire's row in the wire file is its row of the wire list in the profile's columns and form: the same rows,
never computed again ([13](13-manufacturing-outputs.md) §3.3: its length is rounded once, where its record is made, and the profile never rounds).

A wire goes to the hand for the first reason that applies, in this order; the reason is a text with arguments in the
project's language ([04](04-architecture.md) §18):

| Reason | When | By hand because (the sample's words) |
|---|---|---|
| Cut with its cable | a cable's core (`WithCable`) | "Cut with its cable -101W1" |
| Made on site | a field wire (`flags.field`, [13](13-manufacturing-outputs.md) §2.1) | "One end is made on site: +FLD-104B1" |
| No length | `Missing` | "No length: Route on page 902 gives it one, or type it"; in a project without a panel layout "No length: type it, or the fitter measures it" |
| Its way no longer holds | `Stale` ([12-3d](12-3d-routing-thermal.md) §8.5) | "Its way no longer holds: -X1031 turned. Route on page 902 gives it a new one" |
| A size it does not take | the cross-section outside `takes.csa_min` … `csa_max` | "2.5 mm²: the machine takes 0.5 to 1.5 mm²" |
| An end it does not make | an end's treatment outside `takes.ends`; a connector's contact and an end the rule table cannot decide (`?`) always | "Twin ferrule at -X1011:PE: the machine does not make it" |
| A cell it needs is empty | a `required` column's cell is empty | "Wire part is empty: the workshop stock has no wire of 0.75 mm² BK"; "From part is empty: the workshop stock has no ring lug for 2.5 mm²"; for a column without a datum of a part, "<heading> is empty" |

**The list for the hand** is an Excel file a person reads: the wire list's printed columns ([13](13-manufacturing-outputs.md) §3.1, the fitter's) and
By hand because, in the wire list's order. It does not take the profile's form: the profile is the machine's.

**A part of one's own.** The cells a machine picks its material by are part data: the wire part (`part`: the
workshop stock's wire of the wire's size and colour, [07](07-symbols-parts-macros.md) §2.5) and the ferrule or lug of
each end (`from_part`, `to_part`, [13](13-manufacturing-outputs.md) §4.4). A person enters them in Parts as a part of his own (a wire with its
cross-section and colour; a ferrule with its sizes, twin or single, its sleeve; a ring lug with its stud) and adds it
to the workshop stock ([07](07-place.md) §5.10). Where a required cell is empty for that reason, the reason says what
the stock lacks, and the package dialog's row of the wire file offers **Workshop stock…** (§8.5).

The **generic machine profile** (§7.5) requires no cell and takes every size and every end, so its wire file holds
every wire with a length that is exact, no core and no field wire; the sample's holds 41 of its 54 wires, and its
list for the hand the 10 cores and the 3 field wires.

### 7.4 Where a profile is seen and changed

- **Project properties ▸ Workshop**: the rows **Label file** and **Wire file for a machine**, each with its summary
  ("Excel · Text 1, Text 2, Text 3, Copies, For"; "Generic · CSV · 17 columns · every size and end") and Change….
  Output ▸ Workshop ▸ Workshop settings opens these rows ([13](13-manufacturing-outputs.md) §12).
- **Labels…** ([13](13-manufacturing-outputs.md) §5.4) and **Package…** (§8.5) show the same summary on the profile's line, with Change….
- **Change…** opens the form named after the kind, "Label file" or "Wire file for a machine": the kind's list's columns,
  each with a tick, its words, the heading in the file and, for the wire file, Required, in the file's order (Up and
  Down, as List columns… orders a list's, [09](09-reports-forms.md) §3.8); File (CSV, Excel); for a CSV
  Delimiter (Comma, Semicolon, Tab) and Encoding (UTF-8 with a byte-order mark, UTF-8); Decimal mark (Point, Comma);
  Header row; Values (As the list prints them: 1.5 mm², 400 mm · Numbers only: 1.5, 400); and for the wire file **The
  machine takes**: sizes from and to (mm², empty: any) and the ends it makes (Bare, Ferrule, Twin ferrule, Ring lug).
  **Generic columns** fills the form with the generic profile. Save is one `project.settings.set` of the member and
  one undo step; Cancel changes nothing. A member of a newer format that the form does not show stays as it is.
- The form refuses in words what a file cannot be: no column ticked ("A file needs at least one column"), a heading
  ticked twice the same ("Two columns are headed Length"), sizes from above to.

### 7.5 The generic profiles

The **label file**: the columns of [13](13-manufacturing-outputs.md) §5.4 (Text 1, Text 2, Text 3, Copies, For), Excel, every other member at its
default.

The **wire file for a machine**: CSV, a comma, UTF-8 with a byte-order mark, a point, the header row, numbers; it
takes every size and every end; no column is required:

| Heading | Column | Heading | Column |
|---|---|---|---|
| Wire | `no` | From part | `from_part` |
| From | `from` | From strip | `from_strip` |
| From CP | `from_cp` | To end | `to_end` |
| To | `to` | To part | `to_part` |
| To CP | `to_cp` | To strip | `to_strip` |
| Colour | `colour` | From marker | `from_marker` |
| Size | `cross_section` | To marker | `to_marker` |
| Length | `length` | Wire part | `part` |
| From end | `from_end` | | |

The headings of a generic profile are data and the same in every language: the program that imports the file maps
them once.

### 7.6 Presets, and what a profile does not express

**No preset of a named machine or label program ships.** A preset is a profile made from that program's published
import format; it would stand beside the generic profiles in `ew-reports` and be offered by the form's Generic
columns as a second choice. Until a format is read, a shop maps the generic file once in its program
([the later backlog](../plan/backlog-later.md)).

**What a profile does not express**, and is not built: a value mapped by a table (a colour code to a machine's own
code), a key made of two cells (a wire type from size and colour), a row per end or per bundle, a file per bundle or
per machine magazine, fixed-width fields, an encoding other than UTF-8, a range of lengths the machine cuts, and an end
finished by hand while the machine cuts the wire (the whole wire goes to the hand). Each comes with the first shop
whose machine needs it ([the later backlog](../plan/backlog-later.md)).

---

## 8. The production package

### 8.1 What it is (FR-MFG-031)

**One ZIP file with what the workshop needs to build the cabinet**, each file as the workshop gets it from Output, a
text that says what is in it, and a manifest for a program. Output ▸ Workshop ▸ **Package…** writes it (§8.5), and
`ew export package`. Every member is the file the program's own export of that kind writes: the package adds no
second writer of a list, a page or a drawing.

### 8.2 The members

In the project's language; the sample's names (drawing number SAMPLE-E, location +CAB):

| Member (kind) | Path | What it is | The same as |
|---|---|---|---|
| Contents (`contents`) | `Contents.txt` | what the package is for, the revision, whether it is a draft and why, what stood in the check (§8.6), and each file with what it is for | — |
| Drawings (`pages`) | `SAMPLE-E V1.0 drawings.pdf` | every page in page order, the list pages among them; a draft's pages carry the draft mark (§8.4) | Export PDF |
| Wire list (`wire_list`) | `Lists/SAMPLE-E Wire list.xlsx` | every wire with every column ([13](13-manufacturing-outputs.md) §3.1) | the list's Excel export |
| Assembly lists (`assembly_list`) | `Lists/SAMPLE-E Assembly lists.xlsx` | per strip, what to mount in order | the list's Excel export |
| Cut list (`cut_list`) | `Lists/SAMPLE-E Cut list.xlsx` | rails, ducts, cables, wires and tubes to cut | the list's Excel export |
| Consumables (`consumables`) | `Lists/SAMPLE-E Consumables.xlsx` | wire, ferrules and markers to order | the list's Excel export |
| Hole table (`hole_table`) | `Lists/SAMPLE-E Hole table.xlsx` | every hole of every surface | the list's Excel export |
| Label files (`labels`) | `Labels/SAMPLE-E Wire markers EW-GEN-MRK-1.xlsx`, … | every label, one file per label set and marker part, in the label file's profile | Labels… ([13](13-manufacturing-outputs.md) §5.4) |
| Drawings for drilling (`drilling`) | `Drilling/+CAB Door template.pdf`, `Drilling/+CAB Door.dxf`, … | per surface with holes, its template to print at full size on the print settings' paper (A4 when none is set) and its DXF | Drilling… ([13-drilling](13-drilling.md) §9.1) |
| Wire file for a machine (`machine`) | `Machine/SAMPLE-E Wires.csv`, `Machine/SAMPLE-E Wires by hand.xlsx` | §7.3 | — |
| Manifest | `manifest.json` | §8.3 | — |

- **The lists** are those whose kind is a workshop list: `ReportKindDef::workshop`, declared with the kind, which the
  package and nothing else reads. Each list is the whole project's rows in the kind's order and every column, as
  `ew export report --kind <kind> --format xlsx` writes it; a project without a page of that kind still gets the
  file. The cut list's Tubes group is the tube cut list of the package ([11](11-fluid-power.md) §8.2).
- **A member without content** is not written, and the dialog says why on its row: the drawings for drilling and the
  hole table of a project whose surfaces have no holes ("No surface of this project has holes yet"), label files
  without labels.
- **The wire file** is in the package where the person ticks it; the dialog ticks it when the project keeps a machine
  profile of its own (§8.5).
- **Names** are in the project's language, as the label files' are ([13](13-manufacturing-outputs.md) §5.4): the folders Lists, Labels, Drilling and
  Machine (Lijsten, Labels, Boren, Machine). A program never reads a name: the manifest names each member's kind.

**Not members**, each for its reason: the parts list and the other list pages are pages of the drawings, and buying
is the order list's; the rework list compares with a release the person chooses and keeps its own dialog
([13](13-manufacturing-outputs.md) §3.5); the 3D model, since there is no 3D view; and the data for a wiring viewer (wire records as JSON, the positions
of the ends, a page index), which come with the viewer ([13](13-manufacturing-outputs.md) §11).

### 8.3 The manifest (FR-MFG-032)

`manifest.json` is UTF-8 JSON, its members in this order, written by the program and read back by its tests (the
sample's draft):

```json
{
  "format": "elektrawatt-package/1",
  "made_by": "ElektraWatt 0.4.0",
  "date": "2026-11-27",
  "project": { "id": "019b76da-a801-7000-8000-000000000001", "drawing": "SAMPLE-E",
               "description": "Conveyor drive with a pneumatic clamp" },
  "revision": { "label": "V1.0", "state": "open" },
  "draft": true,
  "personal_data": "kept",
  "findings": [],
  "members": [
    { "path": "Contents.txt", "kind": "contents", "bytes": 2741, "sha256": "…" },
    { "path": "Drilling/+CAB Bottom template.pdf", "kind": "drilling", "bytes": 41377, "sha256": "…" }
  ]
}
```

- `revision.state` is `open` or `released`, a released one with `released_on`; `draft` is true for a draft (§8.4).
- `personal_data` is `kept` or `left_out` (§8.7).
- `findings` are those of the check before export (§8.6) that were open when it was written, each `{rule, severity,
  message}`, the message in the project's language; empty for a package that met none.
- `members`: every other entry of the ZIP, in byte order of its path, each with its kind (§8.2), its size and the
  SHA-256 of its bytes in lower-case hex. The manifest names no hash of its own.
- No member and no manifest holds a path of the user's disk, a user name or the time of day.

### 8.4 Draft and release (FR-MFG-025)

**The revision decides, not a choice.** Where the project's last revision is released and nothing has changed since
(no revision is open), the package is that release's: its manifest says `released`, its pages carry no mark, and its
file is `SAMPLE-E V1.0 2026-11-28 package.zip`. Where a revision is open, the package is a **draft**: the manifest says
`"draft": true`, Contents says so in its first line, and every page of the drawings carries the **draft mark** "Draft
V1.0: not released", so that a page of the shop never passes for a released one. Its file is `SAMPLE-E V1.0
2026-11-27 draft package.zip`. A project with no revision at all makes a draft without a label ("Draft: no revision").

- **The draft mark** stands in the sheet's margin above the frame, right-aligned to the frame's right edge, in the
  house font, black, its cap height 2.5 mm or half the margin where that is less; a sheet with no margin above its
  frame takes it inside the frame's top right corner. It is a stamp of the print set (`output::print`: a page's
  overlay, which also carries the print's scale), in the project's language. The lists, the label files and the
  drawings for drilling of a draft carry no mark: a label is printed as it reads, and the draft is said by the
  package's name, its contents and its pages.
- **A release package is written only without an open error of the check before export** (§8.6). The release gate
  ran that same check ([15](15-revisions-collaboration.md) §5.5), so a package written right after a release meets none; where a newer program judges
  the release otherwise, Write package is greyed with its reason ("V1.0 is released, and 1 error stands now: mend it,
  which opens V1.1, and write its draft."), since a released revision is never changed.
- **A draft is written whatever the check says**: the person decides, and what stood goes with it in Contents and the
  manifest.
- **The date** is the release's date for a release package, else the package's date (§8.8), as the file name and the
  PDF print it.

### 8.5 Where it is in the window

**Output ▸ Workshop ▸ Package…** (`package.export`), the last command of the Workshop group and its second large one,
with the icon of a parcel. Its tooltip: "One file for the workshop: the drawings, the lists, the label files and the
drawings for drilling". Greyed in a project without pages: "This project has no pages yet".

The form dialog **"Package for the workshop"** ([16a](16a-design-language.md) §6.12):

1. **Revision**, one line: "V1.0 is open, not released: the package is a draft, and every page says so." or "V1.0,
   released on 28-11-2026: the package is the release's."
2. **Before it goes to the workshop**, one line from the check (§8.6): "Ready: nothing stands in the way." or "3
   things: 1 stops a release, 2 are warnings." with **Show**, which lists them in the dialog, each its rule's message,
   the ones that stop a release first, and **Open Messages**, which closes the dialog and opens the Messages pane,
   where each has its fix.
3. **What goes in**: one row per member of §8.2, a tick, its words, what it holds ("54 wires, 2 bridges"; "3 strips";
   "6 files · Excel"; "3 surfaces · 3 templates on A4 and 3 DXF"; "41 wires for the machine, 13 by hand"), and on
   the label files' and the wire file's rows the profile's summary with Change… (§7.4). Every row is ticked, the wire
   file's where the project keeps a machine profile of its own. A row says what its member lacks of part data, with
   the way to it: "Cable markers: no marker part in the stock marks cables" with **Workshop stock…** (Parts narrowed
   to the stock, [07](07-place.md) §5.10); "4 wires by hand: the stock has no wire of 0.75 mm² BK" with Workshop
   stock…; a reason that names one part's missing field offers **Edit part…** at that field ([07](07-place.md) §5.7).
4. **Without personal data**, a check box: "Leaves out the name and the address of Sample Panel Builder, marked as
   personal data" (§8.7).
5. **File**: the path, beside the project by default, with its name of §8.4 and Choose….

The Primary is **Write draft package** for a draft, **Write package** for a release. It runs as a job, shown in the
Jobs pane (View ▸ Jobs) as every export's job is; a package replaces no other export, and Jobs holds no saved export.
Its toast: "Wrote the draft package SAMPLE-E V1.0 2026-11-27 draft package.zip: 20 files." with **Show in folder**.
A file of that name asks first: "SAMPLE-E V1.0 2026-11-27 draft package.zip exists. Replace it?" (Replace, Cancel), as
Export PDF asks.

**The command line.** `ew export package --out <file.zip> [--leave-out <member>]… [--machine] [--omit-personal-data]`
writes the package of the project as it is saved: `--leave-out` takes a member's kind (`labels`, `drilling`, …), and
`--machine` adds the wire file where the project keeps no machine profile of its own. It prints each finding of the
check as a warning and then what it wrote; a release package with an open error is refused, naming the errors
(exit 2).

### 8.6 The check before export (FR-CHK-058)

**One check says whether a project is ready for the workshop: the release check**, the project's release scheme in
release mode (`ew_checks::gate::release_check`, [15](15-rule-engine.md) §3.2). The release gate is that check and the
EW-REL rules ([15](15-revisions-collaboration.md) §5.5), so the release, the package and its dialog judge the project the same way, and nothing else
is asked before a file for the workshop is written. An open error stops a release and a release package (§8.4); a
draft lists what stood.

Among its findings are those that the workshop meets in its files:

| Rule | What stops the shop | Severity in the check |
|---|---|---|
| EW-MFG-001 | a cabinet's wires that Route lays have no length (only where a panel layout exists) | error |
| EW-LAY-011 | a way that no longer holds, so its length is an "≈" | error |
| EW-LAY-018 | a cable gland not placed on its surface | error |
| EW-MFG-021 | a bridge without a part | error |
| EW-LAY-006 | a device that overlaps another or meets the closed door | error |
| EW-LAY-008, EW-LAY-012, EW-LAY-017 | a duct over its fill limit; holes too close; a cut-out part without holes | warning |
| EW-MFG-004, 008, 009, 020, 022, EW-POT-010 | a marker that does not fit; an end that cannot be made or does not fit; a strip without its end plate; a wire part that does not match; a clamp that takes fewer conductors | warning (EW-POT-010 an error) |

A finding the shop accepts on purpose (the fitter measures the long wires himself) is waived in Messages with its
reason, as at every release. A project without a panel layout has no EW-MFG-001: its wire list prints "—" and the
wire file sends every wire to the hand ("No length: type it, or the fitter measures it").

### 8.7 Without personal data (FR-EXP-009)

What the members carry of persons is the parties' names and addresses: the title block's party cells and the
revision's author and checker, which name a party by its role ([09](09-reports-forms.md) §2.7). **Without
personal data**, every member is made from a copy of the project in which the name and the address of each party
marked as personal data are empty (Project properties ▸ Parties ▸ Personal data, `Party.personal_data`; the fields
`ew-schema` marks personal, `PERSONAL_DATA_FIELDS`); the copy is never saved, and nothing else changes. A party not
marked (a customer that is a company) stays. Without a party marked, the check box is greyed: "No party of this
project is marked as personal data (Project properties ▸ Parties)". The members hold no path of the user's disk and no
user name either way.

### 8.8 The same bytes

The same project, options and program give the same file (FR-EXP-006, NFR-REL-07): the entries in byte order of
their paths, each deflated at `ew-zip`'s fixed level with its fixed date; the date inside the members is the release's
for a release package, else `SOURCE_DATE_EPOCH`, else the model's edited date ([09](09-output.md) §4.3), never the
clock; the PDF's document ID is Export PDF's. A second export of an unchanged project is the same file, byte for byte.

### 8.9 Homes and the licence

- `ew-api` makes the package: what goes in and what each member holds (`Session::package_plan`, which the dialog and
  the command line read), and the job that writes it on a copy of the snapshot, calling the exports that exist; the
  manifest's shape is `ew-api`'s. `ew-reports` holds the profile writer and the wire file (§7.2, §7.3), `ew-checks`
  the release check (§8.6), `ew-zip` the container.
- **The licence side.** The package is an output of the application and is written by it, with its own ZIP container
  (`ew-zip`, GPL, which the archive and the Excel writer use); no code moves to the permissive layer. A third party
  reads a package without the program and without a crate of ours: it is a ZIP of PDF, Excel, CSV and DXF files and a
  JSON manifest whose shape §8.3 states. No reader crate is made, since no program reads a package back but the tests.
- **Not in the archive.** The as-built archive of a release holds the project, its PDF and its frozen report rows
  ([05](05-libraries-archive.md) §14.2); the package of a release is made again from that release, and the archive
  keeps no second copy of the same files.
