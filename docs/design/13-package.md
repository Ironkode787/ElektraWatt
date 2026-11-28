# Manufacturing outputs: the label file's export profile and the production package

A chapter of [Manufacturing outputs](13-manufacturing-outputs.md): how a label file is written from the labels list's
rows (§7), and the one file a job's workshop gets (§8). The wire records, the lists and the label files it writes are
§2 to §6 of [13](13-manufacturing-outputs.md), [the workshop lists](13-workshop-lists.md) and
[markers and labels](13-markers-labels.md); a reference to §7 or §8 there reads here.

---

## 7. Export profiles

### 7.1 What a profile is (FR-MFG-018)

An **export profile** says how a file for another program is written from the rows of a list: which of the list's
columns, in which order and under which headings, and the form of the file. It is project data, a typed member of
`WorkshopSettings` ([13](13-manufacturing-outputs.md) §2.3), seen and changed in the window (§7.3). One is built: the
**label file's**, `workshop.label_profile`, over the `labels` list ([13](13-manufacturing-outputs.md) §5.3), which
Labels…, `ew export labels` and the package write by. A project without one of its own uses the generic profile
([13](13-manufacturing-outputs.md) §5.4).

```rust
pub struct ExportProfile {                    // workshop.label_profile; absent: the generic profile
    pub columns: Vec<ProfileColumn>,          // in file order: { key: a column key of the labels list, heading }
    pub delimiter: Delimiter,                 // comma (default) | semicolon; a CSV's
    pub file: Option<FileForm>,               // csv | xlsx; absent: xlsx
}
```

`file` is the profile's one member beside its columns and delimiter: the form the shop's label program imports, kept
with the project so that Labels…, the command line and the package write the same files. Absent, it is Excel, which
the label program of the shop the defaults are made for imports. Like every member it is optional and additive, and a
value of a newer format is kept as it is. A profile equal to the generic one with nothing more is stored as absent.

### 7.2 The form of a file

The label files' form is [13](13-manufacturing-outputs.md) §5.4's: an Excel file has one sheet named after the label
set, a header row and text cells, without the `id` column and the hidden sheets of the round trip; a CSV is UTF-8 with
a byte-order mark, CR LF, a header row and the profile's delimiter, its values as they are, never the formula guard
of the Excel presets ([09](09-output.md) §4.6). The writers are those of today (`ew-reports`' `label_csv` and
`plain_xlsx`); the profile only chooses between them.

### 7.3 Where a profile is seen and changed

- **Project properties ▸ Workshop ▸ Label file**, a row with the profile's summary ("Excel · Text 1, Text 2, Text 3,
  Copies, For") and Change…, in place of the row Label file columns. Output ▸ Workshop ▸ Workshop settings opens these
  rows ([13](13-manufacturing-outputs.md) §12).
- **Labels…** ([13](13-manufacturing-outputs.md) §5.4) and **Package…** (§8.5) show the same summary on the label
  files' line, with Change…. Labels… has no format field of its own: the form is the profile's.
- **Change…** opens the form "Label file": the labels list's columns, each with a tick, its words and the heading in
  the file, in the file's order (Up and Down, as List columns… orders a list's, [09](09-reports-forms.md) §3.8); File
  (Excel, CSV); and for a CSV the Delimiter (Comma, Semicolon). **Generic columns** fills the form with the generic
  profile. Save is one `project.settings.set` of `workshop.label_profile` and one undo step; Cancel changes nothing. A
  member of a newer format that the form does not show stays as it is.
- The form refuses in words what a file cannot be: no column ticked ("A label file needs at least one column"), two
  columns with one heading ("Two columns are headed Text 1").

### 7.4 What is not built: presets, and a wire file for a machine

**No preset of a named label program ships.** A preset is a profile made from that program's published import
format; it would be offered by the form's Generic columns as a second choice. Until a format is read, a shop maps the
generic file once in its program.

**No wire file for a machine is written.** The shop the defaults are made for cuts its wires by hand, and no wire
machine's import format has been read. A file for a machine (the wire list's rows a machine makes, in its columns
and form, with the sizes it takes and the ends it makes, and every other wire on a list for the hand with the
reason) comes with a shop that has a machine and its published format; it would be a second kind of this profile over
the `wire_list` rows, not a record of its own ([the later backlog](../plan/backlog-later.md)). Until then the wire
list's own Excel export is the file a partner shop or a spreadsheet takes.

---

## 8. The production package

### 8.1 What it is (FR-MFG-031)

**One ZIP file with what the workshop needs to build the cabinet**, each file as the workshop gets it from Output, a
text that says what is in it, and a manifest for a program. Output ▸ Workshop ▸ **Package…** writes it (§8.5), and
`ew export package`. Every member is the file the program's own export of that kind writes: the package adds no
second writer of a list, a page or a drawing.

### 8.2 The members

In the project's language, in a folder per kind; the sample's names (drawing number SAMPLE-E, location +CAB):

| Member (kind) | Path | What it is | The same as |
|---|---|---|---|
| Contents (`contents`) | `Contents.txt` | what the package is for, the revision, whether it is a draft and why, what the check found (§8.6), how many wires have no length and why, apart from the check what the files lack of the workshop stock and of its parts' data (§8.5), how to read the papers (the wire list's first page's lines, without how a marker of two lines is written there, [13](13-manufacturing-outputs.md) §3.1), and each file with what it is for | — |
| Drawings (`pages`) | `SAMPLE-E V1.0 drawings.pdf` | every page in page order as one PDF, the list pages among them; a draft's pages carry the draft mark (§8.4) | Export PDF |
| Wire list (`wire_list`) | `Lists/SAMPLE-E Wire list.xlsx` | every wire with the columns its page prints ([13](13-manufacturing-outputs.md) §3.1): each end's treatment, part, strip length and marker among them; a wire without a length prints "—" as on the page | the list's Excel export |
| Assembly lists (`assembly_list`) | `Lists/SAMPLE-E Assembly lists.xlsx` | per strip, what to mount in order | the list's Excel export |
| Cut list (`cut_list`) | `Lists/SAMPLE-E Cut list.xlsx` | rails, ducts, cables, wires and tubes to cut | the list's Excel export |
| Consumables (`consumables`) | `Lists/SAMPLE-E Consumables.xlsx` | wire, ferrules and markers to order | the list's Excel export |
| Hole table (`hole_table`) | `Lists/SAMPLE-E Hole table.xlsx` | every hole of every surface | the list's Excel export |
| Label files (`labels`) | `Labels/SAMPLE-E Wire markers EW-GEN-MRK-1.xlsx`, … | every label, one file per label set and marker part, in the label file's profile | Labels… ([13](13-manufacturing-outputs.md) §5.4) |
| Drawings for drilling (`drilling`) | `Drilling/+CAB Door template.pdf`, `Drilling/+CAB Door.dxf`, … | per surface with holes, its template to print at full size on A4 (the project stores no paper) and its DXF | Drilling… ([13-drilling](13-drilling.md) §9.1) |
| Manifest | `manifest.json` | §8.3 | — |

- **The lists** are those whose kind is a workshop list: `ReportKindDef::workshop`, declared with the kind, which the
  package and nothing else reads. Each list is the whole project's rows in the kind's order with the columns its page
  prints, one row per row, as Export list… writes it (`list_output`): its headings and the program's words in its cells
  in the project's language, as its page prints them, its numbers numbers ([09](09-reports-forms.md) §3.8), and
  nothing of the round trip: no `id` column and no hidden sheet, which a list for people does not need and Export
  table… and `ew export report --format xlsx` keep. The engineering columns stay one tick away on the list's page (List
  columns…). A project without a page of that kind still gets the file. The cut list's Tubes group is the tube cut list of the package ([11](11-fluid-power.md) §8.2).
- **A member without content** is not written, and the dialog says why on its row: the drawings for drilling and the
  hole table of a project whose surfaces have no holes ("No surface of this project has holes yet"), label files
  without labels.
- **Names** are in the project's language, as the label files' are ([13](13-manufacturing-outputs.md) §5.4): the
  folders Lists, Labels and Drilling (Lijsten, Labels, Boortekeningen); the drawings, Contents and the manifest at the
  top. A program never reads a name: the manifest names each member's kind.

**Not members**, each for its reason: the parts list and the other list pages are pages of the drawings, and buying
is the order list's; the rework list compares with a release the person chooses and keeps its own dialog
([13](13-manufacturing-outputs.md) §3.5); the 3D model, since there is no 3D view; a wire file for a machine (§7.4);
and the data for a wiring viewer (wire records as JSON, the positions of the ends, a page index), which come with the
viewer ([13](13-manufacturing-outputs.md) §11).

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
- **A release package is written only without an open error of the check before export** (§8.6); its warnings and
  infos are listed and stop nothing. The release gate ran that same check ([15](15-revisions-collaboration.md) §5.5),
  so a package written right after a release meets no error; where a newer program judges the release otherwise,
  Write package is greyed with its reason ("V1.0 is released, and 1 error stands now: mend it, which opens V1.1, and
  write its draft."), since a released revision is never changed.
- **A draft is written whatever the check says**: the person decides, and what stood goes with it in Contents and the
  manifest.
- **The date** is the release's date for a release package, else the package's date (§8.8), as the file name and the
  PDF print it.

### 8.5 Where it is in the window

**Output ▸ Workshop ▸ Package…** (`package.export`), the last command of the Workshop group and its second large one,
with the icon of a parcel. Its tooltip: "One file for the workshop: the drawings, the lists, the label files and the
drawings for drilling". Greyed in a project without pages: "This project has no pages yet".

The dialog **"Package for the workshop"** has two panes, so that what the check found is always in view; it is as
wide as the print pre-dialog (`size.dialog.preview`: 1248 px at 1920, [16a](16a-design-language.md) §6.12):

- **Left, What goes in**: one row per member of §8.2, a tick, its words, what it holds ("54 wires, 2 bridges"; "3
  strips"; "6 files, 149 labels"; "3 surfaces · 3 templates on A4 and 3 DXF") and its form; on the label files' row
  the profile's summary with Change… (§7.3). Every row is ticked. A row says what its member lacks of part data, with
  the way to it: "Cable markers: no marker part in the stock marks cables" with **Workshop stock…** (Parts narrowed to
  the stock, [07](07-place.md) §5.10), where a person adds a marker of his own (kind Marker: Marks cables, Lines,
  Characters per line); a reason that names one part's missing field offers **Edit part…** at that field
  ([07](07-place.md) §5.7). The wire list's row names the ends no ferrule or ring lug of the stock takes ("No ring lug
  of the workshop stock takes 2.5 mm²: those ends are listed without a part"), the wires no wire part of the
  stock matches, and each field a ferrule, a lug or a wire part it reads lacks ("MY-FER-075: Without a sleeve length
  the wire list prints no strip length."); the label files' row each field a marker part lacks (its characters per
  line). Each is the sentence `ew_derive::part_fields` declares for its reader, and Contents lists them all.
- **Right, Before it goes to the workshop**: the check's line, "Ready: nothing stands in the way." or "3 things: 1
  stops a release, 2 are warnings.", and under it every finding of the check, those that stop a release first, each
  with its rule's message, **Show** (its wires or devices listed under it, by their numbers and tags) and its fix as a
  button (Route for a way or a length, Mark not used, as in Messages), which runs the fix: a fix with a card of its
  own (Mark not used) stands in front of the dialog, and once the card is applied or cancelled the dialog is back,
  read again, the finding mended no longer offering its fix and the choices made kept; a fix that leads elsewhere
  (Route's card of the cabinet, Edit part… in Parts) closes the dialog. Wires without a
  length are named here ("2 wires of +CAB have no length: Route gives them one, or type it", Show lists 105.1 and
  105.2), as are the stale ones. **Open Messages** closes the dialog and opens the Messages pane.
- **Right, Revision**: "V1.0 is open, not released: the package is a draft, and every page says so." or "V1.0,
  released on 28-11-2026: the package is the release's."
- **Right, Personal data**: the check box **Without personal data**: "Leaves out the name and the address of Sample
  Panel Builder, marked as personal data" (§8.7).
- **Below, File**: the path, beside the project by default, with its name of §8.4 and Choose….

The Primary is **Write draft package** for a draft, **Write package** for a release. It runs as a job, shown in the
Jobs pane (View ▸ Jobs) as every export's job is; a package replaces no other export, and Jobs holds no saved export.
Its toast: "Wrote the draft package SAMPLE-E V1.0 2026-11-27 draft package.zip: 20 files." with **Show in folder**.
A file of that name asks first: "SAMPLE-E V1.0 2026-11-27 draft package.zip exists. Replace it?" (Replace, Cancel), as
Export PDF asks.

**The command line.** `ew export package --out <file.zip> [--leave-out <member>]… [--omit-personal-data]` writes the
package of the project as it is saved; `--leave-out` takes a member's kind (`labels`, `drilling`, …). It prints each
finding of the check as a warning or an error and then what it wrote; a release package with an open error is
refused, naming the errors (exit 2).

### 8.6 The check before export (FR-CHK-058)

**One check says whether a project is ready for the workshop: the release check**, the project's release scheme in
release mode (`ew_checks::gate::release_check`, [15](15-rule-engine.md) §3.2). The release gate is that check and the
EW-REL rules ([15](15-revisions-collaboration.md) §5.5), so the release, the package and its dialog judge the project
the same way, and nothing else is asked before a file for the workshop is written. **Ready** means no open error: an
open error stops a release package (§8.4), as it stops the release; a warning or an info is shown and stops nothing; a
draft lists whatever stood.

Among its findings are those that the workshop meets in its files, at their release severities:

| Rule | What the workshop meets | In the check |
|---|---|---|
| EW-LAY-011 | a way that no longer holds, so its length is an "≈" | error |
| EW-LAY-018 | a cable gland not placed on its surface | error |
| EW-LAY-006 | a device that overlaps another or meets the closed door | error |
| EW-POT-010 | a clamp that takes fewer conductors than land on it | error |
| EW-MFG-001 | a cabinet's wires that Route lays have no length (only where a panel layout exists) | info: shown with its wires and Route |
| EW-LAY-008, EW-LAY-012, EW-LAY-017 | a duct over its fill limit; holes too close; a cut-out part without holes | warning |
| EW-MFG-004, 008, 009, 020, 021, 022 | a marker that does not fit; an end that cannot be made or does not fit; a strip without its end plate; a bridge without a part (its line prints TBD); a wire part that does not match | warning |

**A wire without a length does not stop the package or the release**: the fitter may measure it on the cabinet. The
package says it instead: the dialog names the wires (EW-MFG-001 where a layout exists), Contents counts every wire
without a length with its reason ("3 wires have no length: one end of each is made on site (104.4, 104.5, 104.8)";
"12 wires of +CAB have no length yet: Route on page 902 gives them one"), and the wire list prints "—" as on its page.
A project without a panel layout has no EW-MFG-001; Contents still counts its wires without a length.

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
clock. A release package's PDF has Export PDF's document ID; a draft's, whose pages carry the mark (§8.4), has its
own, as fixed by the project: the print set's, Export PDF's followed by ` print sheet actual` (`output::print`). A
second export of an unchanged project is the same file, byte for byte.

### 8.9 Homes and the licence

- `ew-api` makes the package: what goes in and what each member holds (`Session::package_plan`, which the dialog and
  the command line read), and the job that writes it on a copy of the snapshot, calling the exports that exist; the
  manifest's shape is `ew-api`'s. `ew-checks` holds the release check (§8.6), `ew-zip` the container.
- **The licence side.** The package is an output of the application and is written by it, with its own ZIP container
  (`ew-zip`, GPL, which the archive and the Excel writer use); no code moves to the permissive layer. A third party
  reads a package without the program and without a crate of ours: it is a ZIP of PDF, Excel and DXF files, a text
  and a JSON manifest whose shape §8.3 states. No reader crate is made, since no program reads a package back but the
  tests.
- **Not in the archive.** The as-built archive of a release holds the project, its PDF and its frozen report rows
  ([05](05-libraries-archive.md) §14.2); the package of a release is made again from that release, and the archive
  keeps no second copy of the same files.
