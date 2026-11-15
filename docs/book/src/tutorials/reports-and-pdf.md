# Reports and PDF

In this tutorial you work on a copy of the sample project, a complete control-cabinet project. You read its report
pages, add two more, write the whole project as one PDF and look through the print preview. Report pages are not
drawn: EW generates them from the schematic, so they follow every change.

## What you need

- ElektraWatt installed and started; see [Install ElektraWatt](../how-to/install.md).

## Steps

1. Choose **Help ▸ Tutorials ▸ Reports and PDF**. This tutorial's page of the guide opens at once, and EW makes a
   fresh copy of the sample project in a new folder under `Tutorials` in your projects folder (`Documents`,
   `ElektraWatt`); a toast says so, and **Show in folder** shows it. With a project open, the copy opens in a second
   window and your project stays open; without one it opens here, and the banner says "A fresh copy of the sample
   project was made for the tutorials."
2. Read the report pages in the Pages navigator, in section 9 (folios 900 to 999). They are marked generated: the
   parts list (Partlist, 901 and 901.1), a terminal diagram for each terminal strip (903 to 905), the cable overview
   and diagrams (906), the wire list, the PLC overview, the potential overview, the device tag overview and the
   revision overview. Click one, such as 904, to see it.
3. Choose **Output ▸ Report block…** (on the Page or the Output tab: **Report**). In the form, choose the report **Parts list**,
   keep or type the first folio `912`, type the title `Parts list`, and choose **Create**.
   The toast says "Added the report block Parts list from folio 912". The Pages navigator lists the new pages 912 to
   912.3: the parts list device by device, where the Partlist on 901 sums the parts up.
4. Choose **Output ▸ Report block…** again, choose **Terminal overview**, the first folio `913` and the title
   `Terminal overview`, and choose **Create**.
   The page 913 lists every terminal strip of the project on one page.
5. Choose **Output ▸ Export PDF…** (`Ctrl+Shift+E`) and save the file as `reports.pdf`.
   When the export is done, the toast says "Exported reports.pdf: 13 drawing pages, 3 section divider pages and 25
   report pages", with the file's size. Open the file
   from the toast to see it: one PDF/A-2b file with every page, its outline and its cross-reference links.
6. Choose **Output ▸ Print…** (`Ctrl+P`).
   The Print dialog shows the range, the paper and a preview of the first page: "1 of 41". Step to the next page in
   the preview to see page 2. Choose **Cancel** to close the dialog without printing.
7. Choose **File ▸ Save** (`Ctrl+S`).
   The title bar no longer marks the project as changed.

## What you learnt

- A report block generates report pages from the project: a parts list, terminal diagrams, a cable list and more.
  **Output ▸ Report block…** (the Output tab's **Report**) adds one, from the first folio you give: every list, a
  tube list or a PLC card list too, is made this way; a list is no page type of **New page**. Without a title of your
  own, its pages take the report's name ("912 Tube list"). A block with nothing to list yet makes no page: its toast
  says so and what gives it rows, such as a valve terminal for the valve-terminal configuration.
- Report pages follow the drawing: change a device, a terminal or a cable, and its report pages change with it.
- **Output ▸ Export PDF…** writes every page into one PDF; **Output ▸ Print…** shows each page before it prints.
- [A terminal strip and a cable](terminal-strip-and-cable.md) shows where the terminal diagrams and the cable list
  take their rows from.
