# Use the application window

This guide describes the desktop application `elektrawatt`: what is on the start page, how to make or open a project,
how to arrange the window, and where F1 leads. Every action of the window is an action of one registry, so the same
names work in the menu, in the command palette (`Ctrl+K`) and in a headless UI script
(`elektrawatt --headless --script`).

## Start page

With no project open the window shows the start page. It has no news feed and opens no connection.

- **New project** opens the New project dialog: the name, the folder, and what the project starts from (see
  [Start a project and use templates](start-a-project.md)).
- **Open...** opens a project directory; the recent projects are listed below it.
- **Open sample project** opens the CC0 sample project read-only; use **Save as…** to make your own copy.
- **Import QElectroTech project** reads a `.qet` file into a new project directory.
- **Templates…** lists your templates, makes a new one and adds one somebody gave you (see
  [Your templates](start-a-project.md#your-templates)).
- **Tutorials and guides** lists the tutorials (lessons you follow step by step) and the how-to guides (how
  to do one job), as **Help ▸ Tutorials** and **Help ▸ Guides** do.
- The quick switches set the language and the keymap preset. The privacy line says that this version has no update
  check; **change** opens the privacy step of the [first-run setup](#first-run-setup).

## Project view

With a project open the window shows the page list, the page view and the status bar. The page view shows the current
page read-only. `Page Down` and `Page Up` move between pages, `Ctrl+G` goes to a folio, `Ctrl+Z` and `Ctrl+Shift+Z`
undo and redo, `Ctrl+S` saves and `Ctrl+Shift+S` saves under a new name.

## Sample project

**File, Open sample project** opens the sample project read-only, so that nothing is written into the installation.
**Save as…** writes a copy into a directory of your own and continues in the copy.

## Import a QElectroTech project

**File, Import...**, beside **Open...**, asks for a `.qet` file and then for a new, empty directory. The importer
reads the file into a new project and writes it to the directory; a banner counts the pages and the findings of the
migration report. The same entry takes a project archive (`.ewz`): it asks whether to extract it into a new folder
and open that, or to open it read-only.

## Unsaved changes and recovery

Closing, quitting or opening another project with unsaved changes asks whether to save, discard or cancel. If the
application ends while a project has unsaved changes, the next open finds the journal of that session and opens the
project read-only with a banner. **File, Discard unsaved changes of the earlier session** deletes the journal after a
confirmation and opens the project for editing.

## Command palette

`Ctrl+K` (or `Ctrl+Shift+P`) opens the palette: type a part of a name and press `Enter` to run the selected entry.
Disabled entries are listed last with the reason. `Escape` closes it.

## Keyboard shortcuts

**Help, Keyboard shortcuts** lists the bindings of the EW preset. `F1` opens the guide at what is under the pointer
([below](#help-for-what-is-under-the-pointer)).

## Help for what is under the pointer

Point at something and press `F1`: the guide opens at the page about it. It answers for the most specific thing under
the hand:

1. what the pointer rests on: a button of the command bar or a menu entry (the page of its command), a row of
   Properties (what that row means, in [Properties](../reference/properties.md)), a row of a list (what its thing
   is; a finding its rule's page), a card of a station's view, a part of a drawing in Parts, or a thing on the page by
   what it is: a contactor's coil the devices', a wire the wires', a cable's mark the cables' section of
   [Things on a page](../reference/window/things.md);
2. else what has the keys: the row of a list or of Properties the cursor is on;
3. else what is selected on the page;
4. else the active tool;
5. else the screen: an open dialog or wizard (those come first), Parts and its drawing, a station's view, the guide of
   the page's kind (a fluid, bus-topology or panel layout page), the project view or the start page.

While the pointer rests on something, the status bar says what `F1` opens there, such as "F1: Wires", and a button's
or a menu entry's tooltip ends with "F1: help". **Help ▸ What is this?** opens the page of what has the keys, is
selected or is in front, as `F1` does with the pointer on nothing. The **?** in the PLC, Bus and Fluid panels' headers
and the tabs' **Guide** open the guide of that job. The guide opens in your browser from the copy installed with the
program; in a Dutch window it opens the Dutch page where the guide has one.

## Appearance

The **View** menu sets how the window looks and behaves: **Theme**, **Density**, **UI scale**, **Minimum text
size**, **Reduce motion**, **Show tips**, **Language**, **Wire colours**, **Mouse wheel scrolls** (instead of
zooming), **Collapse command bar** and the **Bottom dock**. **Use the CPU renderer for the canvas** draws the page
without the graphics card where its driver gives trouble, and **Frame statistics** shows how fast the window draws.
The settings are kept for the next start.

## Notifications

What a command did, such as "Cable -1W1 made from 2 wires", shows as a toast at the window's corner for a few
seconds; a pointer resting on it keeps it, and its button undoes the change where it can be undone. **Notifications**
in the status bar opens the log of every toast of this session.

## Workspaces

The project view is a dock: drag the border between two panes to resize them, and drag a pane's title onto another
pane to rearrange or tab it. The program ships the workspaces **Schematic**, **PLC** (see
[Work in the PLC workspace](plc-workspace.md)) and **Fluid power** (see
[Work in the Fluid workspace](fluid-workspace.md)). The switch at the right of the tab row, beside the command
search, shows them with the active one marked (a saved workspace by its name while it is active); a click loads
another. **View ▸ Workspaces** lists them and the saved
workspaces, saves the current layout under a name and resets the layout (a shipped workspace to its own layout). A workspace is a file `workspaces/<name>.json` in the
configuration directory; the layout is saved a second after its last change.

## First-run setup

On the first start the application asks for the language and what you come from (EPLAN, QElectroTech, KiCad or other),
says what it does with your data (this version has no update check and sends no crash reports; **Privacy
(PRIVACY.md)** opens the full statement), asks for the appearance (theme and scale), and then what to do first. Every
step can be skipped. **Help, First-run setup** runs it again. Nothing in it makes a network connection.

## About

**Help, About ElektraWatt** shows the version, the page-view renderer, the privacy state, the licence, the
engineering-verification disclaimer, the support line (before 1.0 only the latest 0.x release is supported), and the third-party
notices.
