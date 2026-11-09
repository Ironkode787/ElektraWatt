<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Manual tests

The protocols of the checks that no pipeline can run: a screen reader on each platform and the Linux clipboard
between two instances (WP-S2.11 D13). The automated suite proves what it
can: every interactive widget's name, role and state on every surface (`crates/ew-ui/tests/it/census.rs`) and the
five core tasks from the keyboard (`keyboard_core.rs`). The runs belong to the release: RC-05 of the
[release checklist](checklist.md) asks for one report per protocol, on the candidate's build. Later manual protocols
are added here.

Each protocol names its setup, its steps, the expected result and a result line. A tester copies the protocol into the
release dossier, runs it, and fills the result line: the build, the platform, the date, pass or fail, and what was seen
for a failure. A failure is a blocker of the release ([the release procedure](README.md)).

## MT-1: NVDA on Windows (NFR-A11Y-02, M)

**Setup.** Windows 11 with the latest NVDA release, its speech viewer on (NVDA menu, Tools, Speech viewer). The
candidate's installer, installed for the user. A copy of the sample project (Golden A, `J-E-2416.ew`), opened from the
start page with Open sample project. No mouse is used once the project is open.

**Steps.**
1. Press F6 until NVDA announces the Devices navigator (the Project entry of the left rail, its Pages or devices
   control on Devices). Type `-102F1` into the navigator's filter.
2. Move to the row of -102F1 with the arrow keys. Listen to the row.
3. Press Enter to select the device on its page. Listen to the announcement of the selection.
4. Press F6 until NVDA announces the property grid. Move through its rows with the arrow keys to the part row and to
   the connections.
5. Return to the navigator row and open the device's page from it (Enter on the row, then the page view).

**Expected.**
- Every control NVDA reaches is announced with a name and its role (row, button, edit, tab), and a control with a
  state with it (selected, expanded or collapsed, checked).
- The row of -102F1 is announced with its tag and its description.
- The selection is announced as a live region: the tag, the device's kind, the number of its connection points and of
  its wires (the form of NFR-A11Y-02: "-102G1, power supply 24 V DC, 7 connection points, 9 wires").
- The property grid reads the part of -102F1 (its part number and its manufacturer) and its connections.
- The page view shows folio 102 with -102F1 selected, and NVDA announces the page.

**Result.** Build: `<version and commit>` · Windows: `<edition and build>` · NVDA: `<version>` · Date: `<YYYY-MM-DD>` ·
Pass or fail: `<pass | fail>` · Notes: `<what was heard where it differed>`

## MT-2: Orca on Linux (NFR-A11Y-02, M)

**Setup.** Ubuntu 24.04 LTS on GNOME (Wayland), with Orca on (Super+Alt+S) and its speech history at hand. The
candidate's AppImage, made executable and started from a terminal. A copy of the sample project, opened from the start
page with Open sample project. No mouse is used once the project is open.

**Steps.** The steps of MT-1, with Orca in place of NVDA.

**Expected.** The expectations of MT-1, through AT-SPI: every control Orca reaches has a name, its role and its state;
the selection's live region is read; -102F1's part and connections are read; its page opens.

**Result.** Build: `<version and commit>` · Linux: `<distribution, desktop and session>` · Orca: `<version>` · Date:
`<YYYY-MM-DD>` · Pass or fail: `<pass | fail>` · Notes: `<what was heard where it differed>`

## MT-3: The Linux clipboard between two instances (FT-455)

The window's clipboard is egui's, through `arboard`, its only path; the pipeline has no desktop session to prove it in.

**Setup.** Ubuntu 24.04 LTS with two sessions to choose at the login screen: GNOME on Wayland and GNOME on Xorg. The
candidate's AppImage. Two copies of the sample project, `A.ew` and `B.ew`, in two folders. A plain text editor (GNOME
Text Editor).

**Steps.** In each of the two sessions:
1. Start two instances of ElektraWatt; open `A.ew` in the first and `B.ew` in the second (File, Open…).
2. In the first, on folio 101, select -101F1 and copy it (Ctrl+C).
3. In the second, on folio 101, paste it (Ctrl+V), and place it on a free place of the page.
4. In the first, open the Devices navigator, select the row of -101F1 and copy it (Ctrl+C); paste into the text editor
   (Ctrl+V).
5. In the text editor, type `-101K9`, select it and copy it (Ctrl+C); in the second instance, open Go to page or cell
   (Ctrl+G) and paste into its field (Ctrl+V).

**Expected.**
- Step 3 places a copy of -101F1 on the second instance's folio 101 with a new tag, as a paste within one instance
  does, in one undo step.
- Step 4 puts the copied text in the text editor.
- Step 5 shows `-101K9` in the Go to field.
- The same under Wayland and under Xorg.

**Result.** Build: `<version and commit>` · Wayland: `<pass | fail>` · Xorg: `<pass | fail>` · Date: `<YYYY-MM-DD>` ·
Notes: `<what differed>`
