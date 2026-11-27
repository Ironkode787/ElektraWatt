# Export the labels, the drilling drawings and the package

Three exports take the cabinet to the workshop: the label files for your shop's label program, the drawings for
drilling the plate, the door and the walls, and the package, one ZIP file that holds them with the drawings and the
workshop's lists. All three are in the Workshop group of the Output tab.

You need the project's lists ([Print the workshop's lists](workshop-lists.md)); for the drawings for drilling, the
holes of the cabinet's surfaces ([Place devices in the door, and make holes and cable glands](cabinet-surfaces.md)).

## The label files

1. Choose **Output ▸ Workshop ▸ Labels…**. The dialog lists one file per label set and marker part (Wire markers,
   Terminal markers, Strip markers, Cable markers, Device labels, Tube markers, Plates), each with its marker part,
   its count of labels and a tick. A file is named by the drawing number, the label set and the marker part, such as
   `SAMPLE-E Wire markers EW-GEN-MRK-1.xlsx`.
2. Under **Labels**, choose **Every label**, or, after a release, only those changed since it (**Changed since
   V1.0**).
3. The line **Label file** says the form of the files and their columns, such as "Excel · Text 1, Text 2, Text 3,
   Copies, For". **Change…** sets, for the whole project, the columns a file has, their headings and their order,
   Excel or CSV, and a CSV's delimiter; **Generic columns** puts the generic ones back. Your label program imports the
   Excel file and maps its columns once.
4. A line such as "1 label does not fit its marker part" counts the labels too long for their marker; **Show** lists
   them.
5. Untick the files you do not need and choose **Export…**, then the folder. The toast says, for example, "Wrote 6
   label files to …". A CSV writes each value as it is, for the label program; opened in a spreadsheet, a value that
   starts with `-`, `+` or `=` is read as a formula, so open the Excel file there.

## The drawings for drilling

6. Choose **Output ▸ Workshop ▸ Drilling…** (greyed with "No surface of this project has holes yet." while no surface
   has any). The form "Export the drawings for drilling" lists the **Surfaces with holes**, each with its page, its
   holes and the sheets its template takes, such as "Door · page 904 · 8 holes · 5 sheets", with a tick.
7. Choose **Template to print at full size (PDF)**, **DXF for a machine** or both, the **Paper** (A4 or A3) and the
   **Folder** (Drilling, beside the project), and choose **Export**. Each surface gets its files, such as
   `+CAB Door template.pdf` and `+CAB Door.dxf`; the toast says "Wrote 6 files to …".
8. Print a template at actual size, not fitted to the page, and measure its 100 mm bar. Tape the sheets together at
   their crosses, lay the template's two thick edges against the surface's edges and centre-punch each hole. The DXF
   has its 0,0 at the corner the surface's holes are measured from, and a layer for each way of making a hole.

## The package

9. Choose **Output ▸ Workshop ▸ Package…** (the command bar's button reads Package). The dialog "Package for the
   workshop" has two panes.
10. On the left, **What goes in**: the drawings, the wire list, the assembly lists, the cut list, the consumables, the
    hole table, the label files and the drawings for drilling, each ticked and with what it holds, such as "54 wires,
    2 bridges". Untick what the shop does not need. A member without content has no tick, such as the drawings for
    drilling of a cabinet without holes. A line says what a member lacks, such as a marker part for cables in the
    workshop stock, with **Workshop stock…**; **Change…** changes the label file's form.
11. On the right, always in view, **Before it goes to the workshop** shows the release check: "Ready: nothing stands
    in the way.", or what stands, such as "3 things: 1 stops a release, 2 are warnings.". An error stops only a package
    of a released revision; a warning or an info is shown and stops nothing. **Show** lists a finding's wires or
    devices, its button mends it, and **Open Messages** shows them all.
12. A wire without a length stops nothing. The right pane and the package's contents name the wires without a length
    and say why, such as "3 wires have no length: one end of each is made on site", or that Route gives a wire of the
    cabinet one.
13. Below, the revision decides draft or release. While a revision is open, such as "V1.0 is open, not released.",
    the package is a draft: "The package is a draft: every page of the drawings says "Draft V1.0: not released", and
    so do its manifest and Contents." The package of a released revision carries no mark, and a second package of it
    is the same file.
14. To hand the package to someone outside the company, tick **Without personal data**: it leaves out the name and the
    address of each party marked as personal data; companies stay. A project without one says "No party of this
    project is marked as personal data (Project properties ▸ Parties)".
15. Under **File**, **Choose…** sets where the ZIP goes, such as `SAMPLE-E V1.0 draft package.zip`. Choose **Write
    draft package** (or **Write package** for a release). A file of that name is asked about first ("… exists.
    Replace it?"). The package is written as a job, and the toast names the file with **Show in folder**.

## What the shop finds in the ZIP

16. At the top, `Contents.txt`, which says the revision, whether it is a draft, what the release check found, which
    wires have no length and why, and what each file is; and the drawings as one PDF, every page in page order.
17. In `Lists`, each workshop list as an Excel file; in `Labels`, the label files; in `Drilling`, the template and the
    DXF of each surface with holes; and `manifest.json`, the same list for a program, with a SHA-256 of each file.

The same three exports run on the command line: `ew export labels`, `ew export drilling` and `ew export package`
([Use the command line on the sample project](headless-workflow.md)).
