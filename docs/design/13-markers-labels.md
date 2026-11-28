# Manufacturing outputs: markers and labels

A chapter of [Manufacturing outputs](13-manufacturing-outputs.md): what each wire end's marker says, the rule that
shortens it, the label engine that makes every label of the project and the label files a label program prints from.
The wire records and the ends they mark are §2 to §4 of [13](13-manufacturing-outputs.md); a reference to §5 there
reads here.

---

## 5. Markers and labels

### 5.1 Marking

**The marker text is a project setting**, `WorkshopSettings.marking`, in Project properties ▸ Workshop:

| Setting | Each end prints | 101.1 (-X1011:L1 → -101Q1:1) at -X1011:L1 / at -101Q1:1 |
|---|---|---|
| Destination (default) | the far end (§5.2) | `-101Q1:1` / `-X1011:L1` |
| Wire number | the wire's number | `101.1` / `101.1` |
| Both ends | this end, then the far end, on two lines | `-X1011:L1`, `-101Q1:1` / `-101Q1:1`, `-X1011:L1` |
| Potential | the potential's name | `L1` / `L1` |

A **core** carries no marker unless the shop sets `mark_cores`: its colour or number and its cable's marker identify it
at each end, as EN 60204-1 13.2.1 allows. A **jumper** on one device carries none unless the shop sets `mark_jumpers`.
A marker set by hand ([13](13-manufacturing-outputs.md) §2.3, a text or "none") wins for its end: typed in the From marker or To marker cell of the
Connections table or the wire's Properties, `—` for none, an empty text for the marking's own. The wire list's marker columns print what each end
carries; the label engine (§5.3) makes a label of it.

**The wire annotation is not a marker.** A schematic page prints beside a wire its number, cross-section and colours
where the project's annotation mode asks ([08](08-connections-terminals-cables.md) §4.5): it is drawing notation. A
marker is the printed sleeve on the wire's end in the cabinet. With wire-number marking both carry the number, so a
fitter can match them; with destination marking they differ. Two things had one name, "wire marker", in the window and
in Dutch ("adermarkering"); the drawing's is called the **wire annotation** ("draadaanduiding"), its tool Annotation,
its project setting Wire annotations, and "wire marker" means the sleeve only. The stored names (`wire_marker`,
`marker_mode`, `show_marker`) stay.

### 5.2 The marker text and the rule that shortens it

**The far end, as seen from the end the marker sits on.** The text is the far end's tag and pin as [13](13-manufacturing-outputs.md) §3.2 prints them,
without the structure the two ends share: within one location `-101Q1:1`; at the cabinet end of 104.4, whose far end is
in the field, `+FLD-104B1:13`; at that field end, `+CAB-104A1-A2:L+`. A terminal is named by tag (`-X1011:L1`); a
level of a block prints its own number; its clamp letter and side are never printed; an external end prints
`ext. L1`; a pin the device has twice prints its number (`-101G1:+#1`). The text is data a language does not change.

**Shortening, the same every time.** Where the label's marker part names its lines and characters per line
(`sections.marker.lines`, `chars`), a text that does not fit is shortened by these steps, in order, until it fits:

1. The text as made, one line (two for Both ends).
2. A line that begins with the product sign `-` loses it: `-104A1-A2:L+` becomes `104A1-A2:L+`. A structure sign
   (`+`, `=`) is never dropped: it says where the far end is.
3. A line too long is broken after its last `:` that leaves the first part within the line, else after its last `-` or
   `+` that does, into the marker's further lines: `+CAB-104A1-A2:L+` on two lines of 12 is `+CAB-104A1-` and `A2:L+`.
4. What still does not fit keeps its text whole: the label is marked "does not fit" and EW-MFG-004 names it. A
   character is never cut, because a cut marker names a wrong place.

The wire list prints the lines a label carries, joined by ` / `.

### 5.3 The label engine

`label_rows()` makes every label of the project in the order the fitter applies them, as `LabelRecord`s ([13](13-manufacturing-outputs.md) §2.4). One
engine serves every **label set**, the tube markers of the fluid lists among them.

| Label set | One label per | Text | Copies | Order | Marker part (`marks`) |
|---|---|---|---|---|---|
| Wire markers | marked wire end | §5.1, §5.2 | 1 | the wire list's order, the From end before the To end | wire |
| Terminal markers | terminal and level (one for a terminal of one level) | the designation, a level's own; a second line, the function text, where the strip prints function texts ([08](08-connections-terminals-cables.md) §6.1) | 1 | strips in natural tag order, terminals in mounting order | terminal |
| Strip markers | strip | the strip's tag | 1 | natural tag order | terminal |
| Cable markers | cable | the cable's tag | 2 (one per end) | natural tag order | cable |
| Device labels | device with a part of its own that is no terminal, strip, cable, enclosure, plate, rail or duct | its tag as on the device itself (`-101Q1`) | 1; 2 where the shop sets labels on the device and beside it | placed devices in mounting order (carriers from the top, left to right), then the others in natural tag order | device |
| Tube markers | tube end | the tube's tag, else the far end as seen from this end ([11](11-fluid-power.md) §8.2) | 1 | the tube list's order | wire |
| Plates | device placed in the door | its function text in the project's language | 1 | mounting order of the door | device |

A label's **marker part** is the first marker of the workshop stock whose `marks` is the set's (the table's last
column). A label without one is printed by the label program on what the person chooses there.

The list kind `labels` lists every label with the columns `set`, `for` ("101.1 at -X1011:L1"), `text_1`, `text_2`,
`text_3`, `copies`, `marker_part` and `note` ("does not fit its marker part"); `tube_labels` lists the tube markers
with the same columns. Both are lists of the report engine, so a person can print them and check them before the
labels are printed, and `ew export report --kind labels` writes them as every list. A row is what it labels: the wire or
the tube with its end, the terminal (with its level where it has several), the strip, cable or device (a plate apart from the device's label),
with that record as its source, so that a block leaves out the rows of a record it excludes and a row opens its record.
The For cell names a tube in the language the list prints.

### 5.4 Label files and the export profile

**One file per label set and marker part**: Output ▸ Workshop ▸ Labels… writes, for each that has labels, one file into
a folder the person chooses, named by the drawing's number, the set and the marker part's number (`SAMPLE-E Wire
markers EW-GEN-MRK-1.xlsx`; a set without a marker part, `SAMPLE-E Cable markers.xlsx`), in the form of the label
file's profile. Each file is written whole or not at all. The package holds the same files ([13](13-manufacturing-outputs.md) §8.2).

**The columns and the form are the label file's export profile**, `WorkshopSettings.label_profile`
([13-package](13-package.md) §7): each column of the `labels` list with the heading the file prints, Excel or CSV,
and a CSV's delimiter. Change…, in
Labels… and in Project properties ▸ Workshop ▸ Label file, opens the form of [13](13-manufacturing-outputs.md) §7.3 on
what the project holds: what this build does not know of the profile and of a column it keeps stays as it is. Without
one of its own the project uses the **generic profile**, an Excel file with these columns, which every label program
that imports a table takes, mapping its columns once in its import; Excel because the label program of the shop the
defaults are made for imports Excel:

| Heading | Column |
|---|---|
| Text 1 | `text_1` |
| Text 2 | `text_2` |
| Text 3 | `text_3` |
| Copies | `copies` |
| For | `for` |

**The form of a file** is [13](13-manufacturing-outputs.md) §7.2's. An Excel file has one sheet named after the set; a
CSV has CR LF line ends and its values as they are, never the formula guard of the Excel presets, which a label
program would print ([09](09-output.md) §4.6). A row is one label; its Copies tells the label program how many to
print.

**The Labels… dialog** chooses Every label or the labels changed since a release (a project without a released revision
says so in that place: "Changed since a release: none yet, the project has no released revision"), lists the files, each
with its set, its marker part, its count and a tick; on one line the
profile ("Excel · Text 1, Text 2, Text 3, Copies, For") with Change…; and the line that counts the labels whose text
does not fit their marker part, with Show. The form of the files is the profile's, so the dialog has no form of its
own; `ew export labels --format csv|xlsx` sets it for that one export.

**Presets of named label programs** are left out: a preset that names a program's columns is made from that program's
published import format, when it is at hand ([13](13-manufacturing-outputs.md) §7.4). The export profile is the one
record of an export by profile; a file for a wire machine would be its second kind ([13](13-manufacturing-outputs.md) §7.4).

**Labels for changes.** From the Rework list…, Labels for these changes… writes the same files with only the labels the
rework list names: the markers of new, replaced and changed wire ends, the labels of mounted or changed devices,
terminals, strips and cables, and the marker of a new strip (the rework list adds each of its terminals and has no row
for the strip itself).
