<!-- file: tools/blind/tasks/selftest/task.md sha256 973376ea5c9193284ee9b3ee73ac6237cb334ac8d0b668d5f418aa895a8de6ab -->
# The self-test task

This task is for the harness, not for an agent. A scripted agent (`agent.jsonl`) plays the part of a drafter and
calls the run's copy of the harness exactly as a real agent would, through `«run»\bin/xtask.exe blind`: it looks at the interface and at
the page, places two contactors on page 101 of a new project with the Place tool, makes one step that the product
refuses (Redo with nothing to redo) and one that the harness refuses, changes some views, saves, exports the PDF and
files its hand-back.

Run `selftest` is a `free` run with the caps 300 turns, 800 steps, 400 looks and 50 harness refusals.
The prelude creates the new project and the page; the epilogue of `stop` describes page 101, saves nothing (the scripted
agent saves its own work) and quits.

`cargo xtask blind selftest` runs it twice, with the session keeper and with `--stateless`, blesses the first run as
the recording `tools/blind/recordings/selftest/` and replays that recording twice. A change of the product that breaks
the recording is repaired by running the self-test again in the same change.

<!-- file: tools/blind/HELP.md sha256 3d17fdde0a1afb2ed430cdc2c0585852cde67bef813415690d488ce0bde262c8 -->
---
prompt: blind-help
version: 6
---
# The harness: how to work with it

You work with ElektraWatt through one program, `«run»\bin/xtask.exe blind`, and nothing else. You never see a picture: you read text, and
you act through the application's own actions. `«run»\bin/xtask.exe blind help` prints this text, the application's vocabulary and every
action with its parameters, a page at a time: each page ends with the line that names the next (`«run»\bin/xtask.exe blind help 2`, …).

## The commands

| Command | What it does |
|---|---|
| `«run»\bin/xtask.exe blind do selftest <action> [key=value ...]` | One step. A value is typed by the action's parameters: `x_mm=254` is a number, `shift=true` a switch, `folio=101` the text `101`; a list or an object is JSON (`fields='["tag","text"]'`). `key=null` leaves an optional parameter out. |
| `«run»\bin/xtask.exe blind do selftest -` | A batch of 1 to 20 steps, one JSON object per line on standard input: `{"action":"canvas.press","params":{"x_mm":254,"y_mm":127}}`. |
| `«run»\bin/xtask.exe blind look selftest [section ...] [--diff] [--menus]` | The interface as text, by sections (`title:`, `banner:`, `page`, `navigator`, `canvas`, `properties`, `card`, `toast`, `find`, `jobs`, `preview`, `history`, `clipboard`, `status`, `notifications`, `palette`, `dialog`, `wizard`, `first`, `start`, `workspace`, `help:`, `notices:`). `jobs` shows while View ▸ Jobs is open, `notifications` while View ▸ Notifications is, `preview` while the Preview tab is open (it opens with every preview card) and `history` while Edit ▸ History is. The menus are left out unless you ask with `--menus`. With `--diff` you get only the lines added (`+`) or removed (`-`) since your last look at the same sections. |
| `«run»\bin/xtask.exe blind look selftest --page [<folio>]` | The content of a page as text: what is drawn on it, in reading order. |
| `«run»\bin/xtask.exe blind note selftest "patience <0-10> flow <0-10> <text>"` | The note of your last turn. |
| `«run»\bin/xtask.exe blind handback selftest -` | Your hand-back on standard input; it ends the run. |
| `«run»\bin/xtask.exe blind report selftest <which>` | For reviewers (a check run) only: `<which>` is `check`, `bom` or `score`, the check report, the parts list and the score of the drafter's project. A drafter has nothing to read here. |

## A turn

A turn is one `do` call. Before the next `do` you give exactly one note: `patience` and `flow` are scores from 0 to 10
(10 is calm and smooth), followed by at most 1,200 characters: what you expected, what you saw, how it felt. `look` calls
need no note, but you may make at most 3 between two `do` calls.

The harness sends the steps of a batch in order and stops at the first step that is not done; the rest are reported
`skipped` and not sent. Each step prints `<n> <action> <outcome>` (`<n>` counts the steps of your `do` calls from 1):
`done`, `refused` (with the reason: the action is not possible now) or `failed` (with its message), and then the parts
of the interface's state that changed. A step that leaves a preview card open adds the line `  preview: <summary>; Apply or Cancel`: nothing is changed until you apply
it (`preview.apply`) or cancel it (`preview.cancel`).

If one step fails twice in a row with the same parameters on the same selection, the harness marks it broken and refuses
that exact step on that selection for the rest of the run (`harness: marked broken, move on`); a step that acts on the
selection is a new step once the selection changed. Three notes in a row with patience 2 or less, or five with flow 2
or less, end the run; you then file your hand-back. The run also ends at its caps: 300 turns, 800 steps,
400 looks and 50 refused steps.

## What the harness refuses

A refused step is never sent. The texts are:

- `harness: unknown action <id>`: the action is not in the list of `«run»\bin/xtask.exe blind help`.
- `harness: <id> is not available to the drafter`: a back door or a path to material you do not have.
- `harness: <action> has no parameter <name>`.
- `harness: paths are names inside the run's work folder`: a file or folder is a bare name, like `project` or `out.pdf`.
- `harness: use a label or a position, not an ID`: name a record by its tag, its label or its position, or by an alias
  `@n` that the application printed in its own output.
- `harness: the checker does not change the project`: in a check run, any action that changes the project, and the
  canvas actions that press, release or click (they can finish a drawing tool).
- `harness: marked broken, move on`: you sent a step that already failed twice in a row.
- `harness: parameters too large`, `harness: a batch has 1 to 20 steps`, `harness: look cap per turn`,
  `harness: note required for turn <n>`.

## The outcome words

Every step the application takes ends in one of three words:

- `done`: the application did it.
- `refused`: the application did not do it because it is not possible now; the reason follows: a key such as
  `reason-no-project`, then a colon and the sentence the window shows (`reason-no-project: <sentence>`).
- `failed`: the application tried and could not; the message says why.

`skipped` is the harness's own word: the step was not sent because an earlier step of the batch was not done.

## How to read a line of the interface

A line is structure and text. The words outside quotation marks are the application's own fixed words (`item`, `enabled`,
`disabled:`, `picker`, `ghost`, `reps`, ...); everything inside quotation marks, and everything after a marker such as
`title:`, `banner:`, `disabled:` or `error:`, is a user's or the application's wording of a label, a name or a reason.
`@n` is the application's name for one record in this run; use it wherever an action asks for an ID or a reference.
Positions are millimetres on the page; a page has a frame of cells (rows A to H, columns 0 to 16) that you can name in
Go to. These positions are the coordinates that `canvas.press` and `cursor.set` take. A `wire` line ends with `at X Y mm`: a
press there selects that wire. You can also select a wire by its ends:
`select.set refs='["wire:<one end> -> <other end>"]'`, with the ends as the page text prints them.

A reference (an entry of `select.set refs`, or any parameter that takes one) is one of: a tag as the page prints it
(`-101Q1`, or a terminal as `-X1031:5`, the tag and the terminal after a colon), the same with `tag:` in front
(`tag:-101Q1`), `section:<number>`, `folio:<label>`, or an alias `@n`. A tag that more than one device prints is
refused with a sentence that names the candidates; pick one of them.

`look --page` describes the content of one page, in reading order (row A to H of the anchor cell, then column 0 to 16):
the header gives the folio, the title and the counts (`reps` representations, `connections` drawn connections,
`junctions`); each `rep` line is one drawn device or symbol with the label it prints in quotation marks, its cell, its
symbol and variant and its position, followed by its `cp` lines (the connection points, with the number of `wires`
drawn to each and the `potential` where there is one); `wire` lines name the two ends of a drawn connection; a
`junction` line names its cell, style, wiring and the sequence of the far ends. Words inside quotation marks are the
user's text (tags, titles, symbol keys); the rest are the application's own words.

## The hand-back

Seven headings in this order, each a line that starts with its number and title:

1. Keep or drop
2. Where it lost me
3. What the tool is
4. Broken
5. The drawing set against the design language
6. Flow chart
7. Good

