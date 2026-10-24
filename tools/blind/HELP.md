# The harness: how to work with it

You work with ElektraWatt through one program, `{blind}`, and nothing else. You never see a picture: you read text, and
you act through the application's own actions. `{blind} help` prints this text, the application's vocabulary and every
action with its parameters.

## The commands

| Command | What it does |
|---|---|
| `{blind} do {run} <action> [key=value ...]` | One step. A value is typed by the action's parameters: `x_mm=254` is a number, `shift=true` a switch, `folio=101` the text `101`; a list or an object is JSON (`fields='["tag","text"]'`). `key=null` leaves an optional parameter out. |
| `{blind} do {run} -` | A batch of 1 to 20 steps, one JSON object per line on standard input: `{"action":"canvas.press","params":{"x_mm":254,"y_mm":127}}`. |
| `{blind} look {run} [section ...] [--diff] [--menus]` | The interface as text, by sections (`title:`, `banner:`, `page`, `navigator`, `canvas`, `properties`, `card`, `toast`, `find`, `clipboard`, `status`, `palette`, `dialog`, `wizard`, `first`, `start`, `workspace`, `help:`, `notices:`). The menus are left out unless you ask with `--menus`. With `--diff` you get only the lines added (`+`) or removed (`-`) since your last look at the same sections. |
| `{blind} look {run} --page [<folio>]` | The content of a page as text: what is drawn on it, in reading order. |
| `{blind} note {run} "patience <0-10> flow <0-10> <text>"` | The note of your last turn. |
| `{blind} handback {run} -` | Your hand-back on standard input; it ends the run. |

## A turn

A turn is one `do` call. Before the next `do` you give exactly one note: `patience` and `flow` are scores from 0 to 10
(10 is calm and smooth), followed by at most 1,200 characters: what you expected, what you saw, how it felt. `look` calls
need no note, but you may make at most 3 between two `do` calls.

The harness sends the steps of a batch in order and stops at the first step that is not done; the rest are reported
`skipped` and not sent. Each step prints `<n> <action> <outcome>`: `done`, `refused` (with the reason: the action is not
possible now) or `failed` (with its message), and then the parts of the interface's state that changed.

If one step fails twice in a row with the same parameters, the harness marks it broken and refuses that exact step for the
rest of the run (`harness: marked broken, move on`). Three notes in a row with patience 2 or less, or five with flow 2 or
less, end the run; you then file your hand-back. The run also ends at its caps: {turns} turns, {steps} steps,
{looks} looks and {refusals} refused steps.

## What the harness refuses

A refused step is never sent. The texts are:

- `harness: unknown action <id>`: the action is not in the list of `{blind} help`.
- `harness: <id> is not available to the drafter`: a back door or a path to material you do not have.
- `harness: <action> has no parameter <name>`.
- `harness: paths are names inside the run's work folder`: a file or folder is a bare name, like `project` or `out.pdf`.
- `harness: use a label or a position, not an ID`: name a record by its tag, its label or its position, or by an alias
  `@n` that the application printed in its own output.
- `harness: parameters too large`, `harness: a batch has 1 to 20 steps`, `harness: look cap per turn`,
  `harness: note required for turn <n>`.

## How to read a line of the interface

A line is structure and text. The words outside quotation marks are the application's own fixed words (`item`, `enabled`,
`disabled:`, `picker`, `ghost`, `reps`, ...); everything inside quotation marks, and everything after a marker such as
`title:`, `banner:`, `disabled:` or `error:`, is a user's or the application's wording of a label, a name or a reason.
`@n` is the application's name for one record in this run; use it wherever an action asks for an ID or a reference.
Positions are millimetres on the page; a page has a frame of cells (rows A to H, columns 0 to 16) that you can name in
Go to.

## The hand-back

Seven headings in this order, each a line that starts with its number and title:

1. Keep or drop
2. Where it lost me
3. What the tool is
4. Broken
5. The drawing set against the design language
6. Flow chart
7. Good
