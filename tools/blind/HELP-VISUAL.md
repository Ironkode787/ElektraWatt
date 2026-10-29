---
prompt: blind-help-visual
version: 1
---
# The harness: how to work with it (a visual run)

You work with ElektraWatt through one program, `{blind}`, and nothing else. You see the program's window as pictures,
one after every batch of inputs, and you act on it with the mouse and the keyboard, as a person at a desk would. You
get no text description of the window: everything the window says reaches you as pixels. `{blind} help` prints this
text and nothing more.

## The commands

| Command | What it does |
|---|---|
| `{blind} do {run} -` | A batch of 1 to 12 inputs, one JSON object per line on standard input: `{"input":"click","x":412,"y":96}`. |
| `{blind} do {run} <input> [key=value ...]` | One input in short form: `{blind} do {run} click x=412 y=96`, `{blind} do {run} key chord=ctrl+s`, `{blind} do {run} type text=Pump`. |
| `{blind} look {run}` | The window as it is now, painted again without any input. |
| `{blind} look {run} --zoom <x> <y> <w> <h>` | A region of the window (frame pixels, at most 768 × 497, inside the frame) painted twice as large, to read small print. A zoom is a picture only: its pixels are never coordinates for an input. |
| `{blind} note {run} "patience <0-10> flow <0-10> <text>"` | The note of your last turn. |
| `{blind} friction {run} --severity <s> --category <c> --wanted "…" --expected "…" --saw "…" [--frame <name>] [--region <x>,<y>,<w>,<h>] [--workaround "…"]` | One entry of the friction log (see below). |
| `{blind} handback {run} -` | Your hand-back on standard input; it ends the run. |

`do` prints exactly one line, `frame frames/t<turn>.png`: the picture of the window once your batch has settled. `look`
prints the path of the picture it painted. The pictures are PNG files in the folder `{frames}`; open each one and look
at it before you decide what to do next. `frames/t0000.png` is the window before your first input.

## The picture and the coordinates

A picture is 1536 × 994 pixels. The top 34 pixels are the window's title bar (the project's name, and a `*` while there
are unsaved changes); clicks there do nothing. Everything below it is the window. Coordinates are whole pixels of the
normal picture, `x` from 0 (left) to 1535 and `y` from 0 (top) to 993, the title bar included: to click a button, read
its centre off the picture and send those two numbers.

## The inputs

| Input | Members | What it does |
|---|---|---|
| `click` | `x`, `y`, `button` (`left` when left out, `right`, `middle`), `mods` (`ctrl`, `shift`, `alt`, joined by `+`) | Moves the pointer there, presses and releases the button. |
| `double_click` | `x`, `y`, `mods` | Two clicks in quick succession. |
| `right_click` | `x`, `y`, `mods` | A click with the right button. |
| `drag` | `x`, `y`, `to_x`, `to_y`, `button`, `mods` | Presses at the start, moves in a straight line, releases at the end. |
| `scroll` | `x`, `y`, `dx`, `dy` (lines, -20 to 20; one of the two at least), `mods` | Turns the wheel with the pointer there; `dy` above 0 scrolls down. With `mods: ctrl` it zooms where the window zooms. |
| `type` | `text` (1 to 200 characters, no line breaks or tabs) | Types the text into whatever has the keyboard. |
| `key` | `chord` (`ctrl+s`, `enter`, `escape`, `f2`, `shift+tab`, `delete`, `arrowdown`, …), `repeat` (1 to 20) | Presses and releases the keys, `repeat` times. |
| `hover` | `x`, `y` | Moves the pointer there and leaves it; a tooltip needs a `hover` and then a `wait` of about 600. |
| `wait` | `ms` (100 to 5,000) | Lets time pass (tooltips, messages that fade). A batch waits at most 10,000 ms in all. |

The pointer stays where your last input left it, so the next picture shows what it hovers over. A batch is sent in full:
an input that does nothing does not stop the ones after it, so keep a batch short when you are not sure what a click
will do, and look at the picture before you go on.

**What the system does for you.** You have no file dialog of your own: when the program asks the system for a folder or
a file name, the answer is given for you, always inside your work folder. A new, opened or saved project is the folder
`project`; an exported PDF is `project.pdf`; an import of another program's project is cancelled; a link to a web page
does not open. You have no clipboard from outside the program, cannot drop files on the window and cannot resize it.

If you close the program (its own quit command, or the window's close), the run ends: `do` then prints
`harness: the application has closed`, and you file your hand-back.

## A turn

A turn is one `do` call. Before the next `do` you give exactly one note: `patience` and `flow` are scores from 0 to 10
(10 is calm and smooth), followed by at most 1,200 characters: what you expected, what you saw, how it felt. `look`
calls need no note, but you may make at most 3 between two `do` calls.

Three notes in a row with patience 2 or less, or five with flow 2 or less, end the run; you then file your hand-back.
The run also ends at its caps: {turns} turns, {steps} inputs, {looks} looks and {refusals} refused calls. They are
counts, never times; there is no clock, so take the time you need to look.

## What the harness refuses

A refused call sends nothing to the program and counts against the refusal cap. The texts are:

- `harness: in a visual run you act with the mouse and the keys`: a line that is not an input.
- `harness: a visual run has no text view`: `look` with anything but `--zoom`.
- `harness: line <n>: <what is wrong>`: an input with a missing or wrong member, a coordinate outside the picture
  (`` `x` is outside the frame (0 to 1535) ``), an unknown key or input.
- `harness: a batch has 1 to 12 inputs`, `harness: a batch waits at most 10000 ms`, `harness: look cap per turn`,
  `harness: note required for turn <n>`.

## The friction log

Log a friction entry **every time** you are unsure, confused, surprised or stuck, have to guess, open the user guide,
or work around something, at that moment and not afterwards. The note keeps the two scores and your mood; the friction
entry keeps the facts. An entry needs:

- `--severity`: `blocker` (you could not go on), `confusing` (you did not understand), `annoying` (it cost you inputs or
  patience), `cosmetic` (it looked wrong but did not stop you).
- `--category`: `discoverability` (you could not find how to do it), `feedback` (you could not tell what happened),
  `layout-legibility` (you could not read or tell things apart), `wording` (the words misled you), `flow-dead-end` (you
  got into a state you could not leave), `behaviour-bug` (it did something wrong), or `missing-feature` (the program
  cannot do what the job needs, as far as you can tell).
- `--wanted`, `--expected`, `--saw`: what you wanted to do, what you expected to happen and what you saw or did
  instead, each at most 400 characters.

The entry refers to the newest picture unless you name another with `--frame t0007`; `--region x,y,w,h` marks the place
in it, in picture pixels. `--workaround` says how you got round it, if you did. The log takes at most 300 entries and
cannot be edited; it needs no note and is not a turn.

The user guide is in the folder `book/` of this run (HTML), as a real program's manual would be. You may read it at any
time; each time you open it because the window did not tell you what to do, log a `discoverability` entry first.

## The hand-back

Eight headings in this order, each a line that starts with its number and title:

1. Keep or drop
2. Where it lost me
3. What the tool is
4. Broken
5. My design against the brief
6. Flow chart
7. Good
8. What I could not read

The harness checks the eight headings; the content is yours. Filing it ends the run.
