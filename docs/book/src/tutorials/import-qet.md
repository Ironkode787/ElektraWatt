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

1. Choose **Project ▸ Import QElectroTech project…**, pick `lamp-circuit.qet`, and name the new project's
   folder `lamp-circuit.ew`.
   The import's preview says what it will create: "1 page, 4 devices, 3 connections; 3 errors, 8 warnings and 5
   infos in the migration report". Nothing is written yet.
2. Choose **Apply** in the preview.
   EW creates the project and opens it. The Messages pane shows the **Migration report**, chosen in its header beside
   the project's own scheme: 3 errors, 8 warnings and 5 infos.
3. Read the report, rule by rule. Some findings are the record of the import, and stay:
   - EW-IMP-008: the folio-report format of QElectroTech is kept as text; EW's own cross-reference format applies.
   - EW-IMP-119: the supply's element has no meaning EW knows, so G1 came in as a generic black box.
   - EW-IMP-103 and EW-NUM-001: the labels S1 and G1 are kept as frozen tags, because they do not follow the
     project's numbering.
   - EW-IMP-122: the lamp and the earth had no label, so EW gave them the tags -1A1 and -1X1.

   The others are live checks of the project, and yours to fix: EW-CORE-073 says that the file's three conductors
   have no route, so they are drawn as dashed lines; EW-POT-009 says that the lamp's pin X2 and the
   supply's pin - are not connected; and EW-CORE-041 says that none of the four devices has a part.
4. Choose **Draw ▸ Connect ▸ Route page**.
   The preview says "Route 3 wires on folio 1: 3 routed, 0 not routed." Choose **Apply**: the three wires are drawn
   along their routes, the toast says "Routed 3 wires on folio 1.", and the three EW-CORE-073 errors are gone.
5. Choose **Draw ▸ Connect ▸ Wire** (`W`). Click the lamp's pin X2, at its bottom, then the supply's pin -, at the
   bottom of G1: the return wire is drawn. Choose **Draw ▸ Select**.
   The two EW-POT-009 warnings are gone. While you edit, the Messages pane shows the project's own scheme, drafting.
6. Click the lamp -1A1 and choose **Edit ▸ Choose part…**. Type `lamp`, choose **EW-GEN-LMP-1**, and confirm with
   **Choose**.
   The lamp has its part.
7. Click the switch S1, choose **Edit ▸ Choose part…**, type `on/off` and choose **EW-GEN-SW-1**.
8. Click the supply G1, choose **Edit ▸ Choose part…**, type `power supply` and choose **EW-GEN-PSU-1**.
9. Click the earth -1X1, choose **Edit ▸ Choose part…**, type `earth` and choose **EW-GEN-TRM-2**.
   Every device has its part, and EW-CORE-041 is gone.
10. Choose **Check ▸ Run checks** (`F7`).
    The Messages pane shows the migration report again, with no error, 2 warnings (EW-IMP-008 and EW-IMP-119) and 5
    infos (EW-IMP-103 twice, EW-IMP-122 and EW-NUM-001 twice): the record of the import, nothing left to fix.
11. Choose **File ▸ Save** (`Ctrl+S`).
    The title bar no longer marks the project as changed.

## What you learnt

- **Project ▸ Import QElectroTech project…** previews an import before it writes anything, and makes a new
  EW project of its own; the QElectroTech file is not changed.
- The migration report lists two kinds of finding: the import's own record (the EW-IMP rules, and the frozen tags),
  which stays as a note of what changed, and the live checks of the project, which go away when you fix them.
- In the Messages pane's header you switch between the migration report and the project's own scheme.
- [QElectroTech and EW](../explanation/qet-and-ew.md) explains how QElectroTech's elements, labels, conductors and
  folio reports become EW's functions, devices, wires and interruption points.
