# Importing a QElectroTech project

In this tutorial you bring a small QElectroTech project into ElektraWatt: a 24 V supply that feeds a lamp through a
switch. You import it, read the migration report that says what the import changed, guessed or left out, and fix the
findings that are yours to fix, until the report shows only the record of the import itself. At the end you have a
saved EW project.

## What you need

- ElektraWatt installed and started; see [Install ElektraWatt](../how-to/install.md).
- The tutorial's QElectroTech project, [lamp-circuit.qet](files/lamp-circuit.qet). Save it in a folder of your own.

The project has one folio with four elements: the supply G1, the switch S1, a lamp without a label and an earth
symbol wired to the supply's PE terminal. One conductor is missing on purpose: the lamp's return to the supply.

## Steps

1. Choose **File ▸ Import…**, pick `lamp-circuit.qet`, and name the new project's
   folder `lamp-circuit.ew`.
   The import's card says what it will make: "Create project makes a new project in the folder lamp-circuit.ew, with
   1 page, 4 devices and 3 connections." Its verdict follows: "The file was imported. 7 points are worth a look, and
   none stops you from working." Under **Worth a look** and **For your information** it lists the kinds of findings,
   each with its count. Nothing is written yet.
2. Choose **Create project**.
   EW creates the project and opens it. The folio looks as it did in QElectroTech: the frame and the title block,
   the labels S1 and G1, and the three conductors where QElectroTech drew them. The Messages pane shows the
   **Migration report**, chosen in its header beside the project's own scheme: no errors, 7 warnings and 7 infos.
3. Read the report, rule by rule. Some findings are the record of the import, and stay:
   - EW-IMP-119: the supply's element has no meaning EW knows, so G1 came in as a generic black box.
   - EW-IMP-103 and EW-NUM-001: the labels S1 and G1 are kept as frozen tags, because they do not follow the
     project's numbering.
   - EW-IMP-122: the lamp and the earth had no label. EW gave them the tags -1A1 and -1X1, and the page prints no
     tag for them, as QElectroTech printed none.
   - EW-IMP-117: the file uses QElectroTech's default cross-reference formula.
   - EW-IMP-006: the file's font, Sans Serif, is printed in Noto Sans.

   The others are live checks of the project, and yours to fix: EW-POT-009 says that the lamp's pin X2 and the
   supply's pin - are not connected; and EW-CORE-041 says that none of the four devices has a part.
4. Choose **Draw ▸ Connect ▸ Wire** (`W`). Click the lamp's pin X2, at its bottom, then the supply's pin -, at the
   bottom of G1: the return wire is drawn. Choose **Draw ▸ Select**.
   The two EW-POT-009 warnings are gone. While you edit, the Messages pane shows the project's own scheme, drafting.
5. Click the lamp and choose **Edit ▸ Choose part…**. Type `lamp`, choose **EW-GEN-LMP-1**, and confirm with
   **Choose**.
   The lamp has its part.
6. Click the switch S1, choose **Edit ▸ Choose part…**, type `on/off` and choose **EW-GEN-SW-1**.
7. Click the supply G1, choose **Edit ▸ Choose part…**, type `power supply` and choose **EW-GEN-PSU-1**.
8. Click the earth, choose **Edit ▸ Choose part…**, type `earth` and choose **EW-GEN-TRM-2**.
   Every device has its part, and EW-CORE-041 is gone.
9. Choose **Check ▸ Run checks** (`F7`).
   The Messages pane shows the migration report again, with no error, 1 warning (EW-IMP-119) and 7 infos
   (EW-IMP-006, EW-IMP-103 twice, EW-IMP-117, EW-IMP-122 and EW-NUM-001 twice): the record of the import, nothing
   left to fix.
10. Choose **File ▸ Save** (`Ctrl+S`).
    The title bar no longer marks the project as changed.

## What you learnt

- **File ▸ Import…** says what an import will make before it writes anything, and makes a
  new EW project of its own; the QElectroTech file is not changed.
- The folios arrive as QElectroTech drew them: the wires on their paths, the texts, shapes and images, the frame
  and the title block.
- The migration report lists two kinds of finding: the import's own record (the EW-IMP rules, and the frozen tags),
  which stays as a note of what changed, and the live checks of the project, which go away when you fix them.
- In the Messages pane's header you switch between the migration report and the project's own scheme.
- [QElectroTech and EW](../explanation/qet-and-ew.md) explains how QElectroTech's elements, labels, conductors and
  folio reports become EW's functions, devices, wires and interruption points.
