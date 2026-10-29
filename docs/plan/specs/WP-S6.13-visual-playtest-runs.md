# WP-S6.13 parts 4 and 5: visual play-test runs and open design briefs (closed specification)

**Status:** **part 4 built in cycle 34** (lane P34; §3 to §11 state what was built, each difference from the specified
text in an "As built (cycle 34)" note; the build wins) · specified in cycle 34 (documents only) as two new parts of
WP-S6.13 and the runs that follow them: part 4
(V1: the visual run kind in the product and the harness, the friction log, the brief task kind with one brief, the
`first-timer` persona and a visual self-test recording; 1.2 pw, one Opus lane in one cycle), part 5 (V2: the rest of
the brief library, the `keyboard-expert` and `reviewer` personas, the play-test checker and the cross-run friction
ledger; 0.6 pw), and the runs (the calibration round after part 4, then the visual rounds at the gates of the
polish cycles of §9; from the 20 % reserve, as part 3) · **Amended:** cycle-34 design pass: as built, owner rulings of
2026-10-29 (§1, §9) · **Date:** 2026-10-29 · **Work package:**
[WP-S6.13](../02-work-packages.md#wp-s613-blind-drafter-gate-agent-harness-personas-and-prompt) · **Decision log:**
ID-785 to ID-794; ID-341 amended for the visual kind only (ID-787); follow-ups FT-592, FT-593 · **Part of:**
ElektraWatt plan, specifications

**Purpose.** The blind drafter of WP-S6.13 parts 1 to 3 ([its specification](WP-S6.13-blind-drafter-harness.md), here
"the base specification") sees EW only as text: `ui.describe` and `page.describe`. That tells whether the product can
do a job, but not whether its window is usable: "the UI might be a big fat mess, and we would never find out
otherwise" (the project owner, 2026-10-29). This specification adds a second run kind, **visual**, in which the agent
sees only offscreen pictures of the whole window and acts only with the mouse and the keyboard, and it reshapes the
runs into **play-tests**: the tester gets an open design brief ("three relays switching three 24 V DC solenoid
valves …"), plans and draws it its own way, and keeps a **live friction log** of every moment it is unsure, confused,
surprised, stuck or has to guess, like the play-testers of a game studio. The friction log is the main deliverable; the
score against the brief comes second. Everything the base specification says holds for text runs unchanged; where this
document and the base specification differ, this document governs visual runs and brief runs only.

---

**Contents**
1. [Goal, non-goals and the owner's rulings](#1-goal-non-goals-and-the-owners-rulings)
2. [The decisions](#2-the-decisions)
3. [What the agent sees](#3-what-the-agent-sees)
4. [What the agent does](#4-what-the-agent-does)
5. [Determinism, recording and replay](#5-determinism-recording-and-replay)
6. [Tasks: open briefs and the regression task](#6-tasks-open-briefs-and-the-regression-task)
7. [The friction log, the checks and the hand-backs](#7-the-friction-log-the-checks-and-the-hand-backs)
8. [Personas](#8-personas)
9. [Where it runs: the gate rule and the polish cycles](#9-where-it-runs-the-gate-rule-and-the-polish-cycles)
10. [Parts, estimates and routing](#10-parts-estimates-and-routing)
11. [Acceptance checks](#11-acceptance-checks)
12. [Risks](#12-risks)
13. [Files owned, out of scope and documents to update](#13-files-owned-out-of-scope-and-documents-to-update)

---

## 1. Goal, non-goals and the owner's rulings

**The owner's rulings of 2026-10-29** (decision log ID-785), recorded in meaning:

1. The blind testers also smoke-test the UI **by sight**, because a text-only tester cannot find out that the window is
   a mess. Picture-only persona runs are used when heavy UI work is done.
2. Heavy UI work is grouped into **one or two cycles**, with the visual personas running at those cycles' gates. The
   orchestrator decides the details (§9).
3. The testers are **play-testers**: they get a task such as "design a cabinet with three relays controlling three
   valves" and execute it their own way, logging unclarities, confusing things and workarounds as they go. Open briefs
   are the main task form; the copy task `gpp-101-103` stays as the comparable regression task. Personas have
   different backgrounds and an honest voice. Offscreen renders only, input by coordinates and keys, never the
   desktop, deterministic replay, and a first run as soon as the harness exists (since ruling 4: the calibration
   round of §9.1).

Rulings 1 and 2 are superseded in their placement of the visual runs by the owner's further rulings of the same day,
recorded in the cycle-34 design pass:

4. **Visual rounds run only at the gates of pure UI-polish cycles**, cycles with no feature WP in them. The "UI-heavy
   cycle" rule of the first specification is withdrawn.
5. **One polish cycle is placed right before each milestone gate**, so the milestone's visual round is that polish
   cycle's gate round.
6. **Briefs only on frozen areas.** A visual round runs only briefs whose areas (the brief's `areas`, §6.1) are on the
   freeze list for the coming cycles, which the plan keeps in [the global plan](../01-global-plan.md).
7. **Friction in areas still in feature work is parked** in the ledger (§7.3), not fixed.
8. **Text runs stay at feature gates.**
9. **Cycle 34 runs a calibration round only**: the reading probe and `brief-valve-switch` visual and text; its
   findings are parked, except blockers in stable areas.

**Goal.** At the end of part 4 the orchestrator starts a visual brief run with one command, hands the composed prompt
to an agent that can read images, and gets back a run folder with the agent's frames, its inputs, its notes, its
friction log, its hand-back, the saved project, the product's reports and a replay that reproduces every action the
inputs caused. At the end of part 5 a play-test round (several personas on several briefs, visual and text) ends in a
ranked list of FT drafts that a UI fix lane can take directly, and a ledger that shows which friction recurs across
runs and personas.

**Non-goals.**
- **Never the owner's desktop, a real window or a screenshot of the system.** Frames are offscreen renders of the
  application's own egui window by the product's offscreen path (§3.1).
- **No text channel into the window** in a visual run: no `look`, no `ui.describe`, no `page.describe`, no
  accessibility tree, no action reference, no transcript text to the agent (§3.3).
- **No pixel gate and no image comparison as a check.** Frames are evidence; replay is decided by the actions the
  inputs caused (§5.3).
- **No model in CI and no blocking.** As the base specification's D13 and ID-341: the orchestrator runs the agents; a
  visual run is evidence for the gate, never a criterion (§9.1 states the one consequence that is binding: due dates of
  blocker findings).
- **No fix of what a run finds** (follow-ups through the design pass), no moderated usability round (WP-S2.12).
- No change to the golden fixture, the project format, the command catalogue, `Cargo.lock` or any workspace edge.

## 2. The decisions

Each is a lead ruling the owner may reverse; the decision-log rows are ID-785 to ID-794.

| # | Decision | Reason and trade-off |
|---|---|---|
| V-D1 | **Home: two new parts of WP-S6.13** (part 4 and part 5) in this companion specification, built in M2, with the runs from the 20 % reserve as part 3's; no new WBS row (ID-786) | The harness, the run folder, the keeper, the filter, the scoring and the replay are shared code; a new package would split one tool across two specifications and regenerate the WBS tables and the schedule for 1.8 pw. Trade-off: WP-S6.13's PERT row (1.6 pw, M-PP) understates the package's total, which its status paragraph states |
| V-D2 | **The senses are a second axis of a run**, `--senses text` (the default, the base specification unchanged) or `--senses visual`, combinable with the kinds `brief`, `gate` and `free`; a check run is always text (ID-787) | One harness, one protocol, comparable runs: the same brief or the same gate task can be run both ways, which separates "the window is unclear" from "the product cannot do it" (§7.4) |
| V-D3 | **What a visual agent sees:** a PNG of the whole window, 1280 × 800 points plus a 28-point title strip, rendered at 1.2 physical pixels per point (1536 × 994 pixels), written after every `do` batch, on request (`look`) and as a 2× zoom of a region (`look --zoom`); nothing textual (§3) (ID-787) | 1536 pixels is the widest whole-window frame that stays under the about 1568-pixel long edge at which model vision inputs are downscaled, so no pixel is thrown away and small text gets 20 % more pixels than at 1.0; the zoom covers what is still too small. Trade-off: about 2,000 image tokens per frame |
| V-D4 | **The visual driver is product code in `ew-ui`**: `elektrawatt --headless --visual` runs the real `App` frame in an egui context fed with synthetic input events and a frame clock, in the window's mode (`Mode::Gui`), and paints frames with the offscreen renderer of the chrome snapshots, moved from the tests into `ew-ui` (§3.1, §4.3) (ID-787) | The same `App::ui` and `ShellModel::dispatch` the window runs, so what the agent sees is what a user sees; no new package and no new edge (egui-wgpu and the headless device are already `ew-ui`'s). ADR-0015 holds: the canvas inside the window is the CPU canvas, the chrome is painted the way the WP-S2.17 snapshots paint it |
| V-D5 | **Input by coordinates and keys**: click, double-click, right-click, drag, scroll, type, key chords, hover and wait, in frame pixels, 1 to 12 inputs per turn (§4) (ID-788) | What a user's hands do, and nothing a user cannot do; frame pixels so the agent never converts scales |
| V-D6 | **Determinism by construction**: a frame clock instead of the wall clock (FT-570's dwell and job-field rule move onto it in part 4), fixed theme, density, language, fonts and scale, the caret's blink off, settling on jobs and repaints; replay decides on the actions the inputs dispatched, frames are compared as a report (§5) (ID-788) | Pixels may drift between graphics-driver versions; what the inputs did may not. Replay without frames also runs on every OS in CI |
| V-D7 | **Open design briefs are the main task form**: a brief states a goal in a user's words with an acceptance list in engineering terms (hard must-haves, soft design quality), scored by the checker from the product's own outputs; a library of five graded briefs, four in M2; `gpp-101-103` stays as the regression task (§6) (ID-789) | The owner's play-test picture; an open brief tests planning and discovery, a copy task tests drawing speed. Trade-off: no copy-oracle, so a brief's hard items are judged by an agent against exports, not compared row by row |
| V-D8 | **The friction log is the main deliverable**: `{blind} friction` appends a structured entry (turn, frame, wanted, expected, saw, severity, category) to `friction.jsonl` without ending the turn; the checker and the lead turn the logs into ranked FT drafts, de-duplicated across runs and personas in a committed ledger (§7) (ID-790) | Friction recorded at the moment it happens is more precise than a hand-back written afterwards; a structured entry makes de-duplication and ranking mechanical enough for a lead to do per round |
| V-D9 | **Personas as play-testers**: `first-timer` (an electrician new to ECAD), `qet-veteran` and `switcher` (another tool's habits), `keyboard-expert` (hurried, shortcuts), optionally `reviewer` (careful); each speaks in its own voice in the log (§8) (ID-791) | Different backgrounds find different friction; the same friction found by several personas ranks higher (§7.3) |
| V-D10 | **The UI checks U1 to U7** (discoverability, dead ends, input cost, visual misreads, unreadable or clipped text, missing feedback, layout against the design language) are scored by the checker from the frames, the inputs, the dispatched-action log and the friction log; the hand-backs gain fixed headings and the FT drafts a fixed format (§7) (ID-792) | Makes a visual run's reading comparable between rounds and directly usable by a fix lane |
| V-D11 | **The gate rule**: a calibration round in cycle 34, the cycle that built part 4 (the reading probe and `brief-valve-switch` visual and text); then a visual round only at the gate of a pure UI-polish cycle, with one polish cycle right before each milestone gate from G-M2, whose round is the milestone's; briefs only on frozen areas; friction in areas still in feature work parked; text runs at feature gates (§9) (ID-793; *amended in the cycle-34 design pass by the owner's rulings 4 to 9 of 2026-10-29, §1*) | The owner's rulings 4 to 9 (which replace the first specification's baseline round and its UI-heavy-cycle rule) |
| V-D12 | **Parts and routing**: part 4 (1.2 pw) one Opus lane in one cycle; part 5 (0.6 pw); runs from the reserve; a de-scope order (§10) (ID-794) | The owner's sizing: one lane delivers the harness, one brief and the friction log |

## 3. What the agent sees

### 3.1 The frame

- **Source.** `elektrawatt --headless --visual` builds the window's `App` (the same `App::new`, `App::ui` and
  `ShellModel` in `Mode::Gui` as the binary's eframe frame), runs its frames in an `egui::Context` that the driver feeds
  with `RawInput`, and paints a frame with the offscreen renderer of the chrome snapshots (texture replay, paint into a
  texture, read back; `ew-render2d`'s headless device, DX12 WARP on Windows, the software adapter elsewhere). The
  renderer core moves from `crates/ew-ui/tests/offscreen/` into `crates/ew-ui/src/offscreen.rs`; the tests keep only
  the `egui_kittest` adapter and call the moved core, so the pixel gate is unchanged. Never a real window, never the
  desktop, never a screen capture.
- **Geometry.** The window is **1280 × 800 points**, the size of the chrome snapshots and of the mockup. Above it the
  driver draws a **28-point title strip** in the theme's chrome colours with the window title as the operating system
  would show it (project name and unsaved mark), because the title is feedback a user sees; clicks in the strip do
  nothing. The frame is **1280 × 828 points at 1.2 pixels per point = 1536 × 994 pixels**, RGBA PNG with the DPI chunk
  of `ew-render-cpu`'s encoder.
- **Fixed rendering inputs.** Theme `light`, density `comfortable`, language en-GB, the bundled fonts only (the Noto
  family of `ew-ui`'s `fonts.rs`; no system font is read), `ui_scale` 1, the canvas backend `cpu` (the switch rule's
  headless path), the text caret's blinking off (the caret is always drawn, a deliberate difference from the window,
  so a frame never catches it off). A run may name another theme or density at `start` (`--theme`, `--density`) for a
  dedicated round; the default round is fixed.
- **When a frame is taken.** (1) After every `do` batch, once the batch has settled (§5.1): `frames/t<turn>.png`.
  (2) On request, `look`: the current state re-rendered without input, `frames/t<turn>-l<k>.png`. (3) On request,
  `look --zoom <x> <y> <w> <h>`: the region (frame pixels, at most 768 × 497, inside the frame) rendered at 2.4 pixels
  per point (twice the frame's), `frames/t<turn>-z<k>.png`. A zoom is a picture only: its pixels are never input
  coordinates. (4) Before the first turn, the start frame `frames/t0000.png`.
- **Size.** A frame is about 80 to 200 KB (flat UI colours compress well; not measured, owner's rule). Frames stay in
  the run folder (outside the repository) and are never committed; a recording keeps their SHA-256 (§5.3), and
  `replay --frames` regenerates them.

*As built (cycle 34).* The driver is `ew_ui::visual` (`crates/ew-ui/src/visual.rs`) and the moved renderer core
`ew_ui::offscreen`; the pixel gate stayed byte-equal (13 of 13). The product flags are
`elektrawatt --headless --visual [--frames <dir> | --no-frames]`: `--visual` without `--headless`, `--frames` or
`--no-frames` without `--visual`, or both of them, is a usage error with exit code 2 (`--visual needs --headless`,
`--frames and --no-frames need --visual`, `--frames and --no-frames exclude each other`). Differences from the text
above:

- **The title strip is 34 pixels** (`STRIP_PX`, 28⅓ points), not 28 points (33.6 pixels): the window is
  1280 × 800 points = 1536 × 960 pixels at 1.2 pixels per point and the frame `1536 × (960 + 34)` = **1536 × 994**
  pixels, as specified. The PNG's resolution chunk says 115 dpi.
- **A zoom is a doubled region of the normal frame** (each pixel drawn twice as wide and high), not a re-render at
  2.4 pixels per point; the region is at most 768 × 497 frame pixels inside the frame, as specified, and stays a
  picture only.
- **No `--theme` or `--density` at `start`** (de-scope order, §10): every run uses the fixed rendering inputs.
- **Settling** also waits, on the frame clock only, for the canvas tiles and the page view's image within the frame.
- **The wall clock** in the frame path is read only through `ew_ui::clock` (the frame-phase statistics), which a
  visual run switches off and counts (BV4); the dwells, the job-field rule and the Jobs pane's elapsed time read
  egui's frame time (FT-570 closed).

### 3.2 What `do` and `look` print

`do` prints exactly one line, `frame frames/t<turn>.png`, or a harness refusal (`harness: …`). `look` prints the path of
the frame it wrote. Nothing else: no outcome word, no reason, no state member, no count of dispatched actions. When the
application closes (the agent quit it through the window), `do` prints `harness: the application has closed` and the
run ends (`end: app-closed`; §5.4).

### 3.3 Nothing textual

In a visual run the harness refuses `look` with sections, `look --page`, `look --diff`, `look --menus` and every action
line (`harness: a visual run has no text view` and `harness: in a visual run you act with the mouse and the keys`).
`{blind} help` prints only `HELP-VISUAL.md` (the harness, the input vocabulary, the friction command, the hand-back):
**no vocabulary list, no action reference, no `elektrawatt --help`**, since a user of the window has none of them. Window
text reaches the agent only as pixels.

**The user guide stays readable** (the run's `book/`, as in text runs): a user of a real product has its manual. Every
time the tester opens the guide because the window did not tell it what to do, it logs a friction entry of category
`discoverability` (the persona prompts say so), which turns the guide's use into a signal instead of a leak.

*As built (cycle 34).* `HELP-VISUAL.md` (`prompt: blind-help-visual`, version 1) is the harness help of a visual run;
`PLAYTESTER.md` (`prompt: play-tester`, version 1) says the guide rule, and `HELP-VISUAL.md` says it again under the
friction log. A refused text view or action line is recorded as a turn of kind `refusal` and counts against the
refusal cap. `look --zoom` in a text run is refused (`harness: --zoom belongs to a visual run`).

## 4. What the agent does

### 4.1 The input vocabulary

One input per line in a `do` batch, as JSON (`{"input":"click","x":412,"y":96}`) or as the `key=value` form of the base
specification's §5.2.1 for a single input. Coordinates are **integer frame pixels** of the normal frame (origin top
left, `0 ≤ x < 1536`, `0 ≤ y < 994`, title strip included); the driver converts them to points by dividing by 1.2.

| Input | Parameters | egui events, in one frame unless said |
|---|---|---|
| `click` | `x`, `y`, `button?` (`left` default, `right`, `middle`), `mods?` (`ctrl`, `shift`, `alt`, joined by `+`) | `PointerMoved`, then `PointerButton` pressed and, next frame, released |
| `double_click` | `x`, `y`, `mods?` | two clicks in consecutive frames (within egui's double-click time on the frame clock) |
| `right_click` | `x`, `y`, `mods?` | `click` with `button: right` |
| `drag` | `x`, `y`, `to_x`, `to_y`, `button?`, `mods?` | press at the start, 8 `PointerMoved` steps on the straight line, one per frame, release at the end |
| `scroll` | `x`, `y`, `dx?`, `dy?` (lines, `−20` to `20`), `mods?` | `PointerMoved`, then `MouseWheel` in lines (Ctrl + scroll zooms the canvas, as in the window) |
| `type` | `text` (1 to 200 characters, no control characters) | one `Text` event per character, one frame each |
| `key` | `chord` (`ctrl+s`, `enter`, `escape`, `f2`, `shift+tab`, `delete`, `arrowdown`; key names as egui's `Key::from_name`), `repeat?` (1 to 20) | `Key` pressed and released with the modifiers, per repeat |
| `hover` | `x`, `y` | `PointerMoved`; the pointer stays there |
| `wait` | `ms` (100 to 5,000) | frames at the frame clock's step until the time has passed (tooltips, toasts) |

The pointer stays where the last input left it, so hover highlights show in the next frame as on a screen; a tooltip
needs `hover` and `wait 600`. Clipboard paste from outside, file drops, window resizing and system dialogs are not in the
vocabulary (§4.3).

### 4.2 Caps

| Cap | Visual `brief` | Visual `gate` (`gpp-101-103`) | Visual `free` |
|---|---|---|---|
| Inputs per `do` | 1 to 12 | 1 to 12 | 1 to 12 |
| `wait` per batch | ≤ 10,000 ms | ≤ 10,000 ms | ≤ 10,000 ms |
| Turns | 250 | 600 | 150 |
| Inputs | 2,000 | 4,000 | 1,000 |
| Looks (plain and zoom) | 300, at most 3 between two `do` | 800, at most 3 | 200, at most 3 |
| Friction entries | 300 | 300 | 300 |
| Harness refusals | 50 | 100 | 50 |

Notes, quit rules and the hand-back work as in the base specification's §5.2.2, with two differences: **every input of
a batch is sent** (a hand does not stop after a click that did nothing; the agent sees the effect in the frame), and
**the broken rule does not apply** (the agent cannot know that an input was refused; the dead inputs are U2's and U6's
evidence, §7.2). The caps are counts, never times. The orchestrator may lower them for a round (§12, token cost).

*As built (cycle 34).* The visual caps are those of the table (`Kind::caps_for`); a **text** brief run has a free
run's text caps (300 turns, 800 steps, 400 looks, 50 refusals); a check run is always text. The input vocabulary is
§4.1's; `scroll` needs at least one of `dx` and `dy`. Batch refusals: `harness: a batch has 1 to 12 inputs`,
`harness: a batch waits at most 10000 ms`, `harness: line <n>: <what is wrong>` (for example
`` harness: line 1: `x` is outside the frame (0 to 1535) ``), `harness: look cap per turn`; the harness's copy of
the key names is checked against `ew-ui`'s `parse_chord` by a test (xtask has no edge to `ew-ui`).

### 4.3 What the driver answers in place of the system

- **File dialogs** (the `Platform` of `ew-ui`): the visual driver's platform answers without a dialog, with bare names
  in the run's `work/` folder: a new or saved project `project`, an opened project `project`, a PDF `project.pdf`; a
  QElectroTech import is cancelled; `open_url` returns false (the window then shows its own fallback). `HELP-VISUAL.md`
  states it ("your system's file dialogs are answered for you: …"). The usability of the system's own dialogs is out of
  scope; the window's reaction to them is in.
- **Prelude and epilogue** are action lines of the harness, not of the agent: the driver dispatches them on the shell
  before the first frame and after the last, admitting the headless-only actions they need (`command.run` in a
  prelude, `page.describe` in the epilogue); the filter refuses action lines from the agent (§3.3).
- **The dispatch log.** `ShellModel` gains an off-by-default log of every `dispatch` (action ID, outcome, reason key),
  which the visual driver drains per input; it is the replay's evidence (§5.3) and the checker's, never the agent's.

*As built (cycle 34).* As specified: new, opened and saved projects are `project`, a PDF `project.pdf`, a QET import is
cancelled, `open_url` returns false. `visual::run` (the binary's path) answers in the working directory;
`visual::run_in_folder` answers in a given folder (the tests' path). The dispatch log records, while it is on, the
action, the outcome's word and a refusal's reason key, in the order the calls started.

## 5. Determinism, recording and replay

### 5.1 Clocks, settling and jobs

- **The frame clock.** `RawInput.time` starts at 0 and advances by 1/60 s per frame; nothing in visual mode reads the
  wall clock. Part 4 moves the two wall-clock rules FT-570 names, the canvas dwell (80 ms) and the 200 ms job-field
  rule, onto egui's frame time (closing FT-570); toasts already read it (E33's fix, ID-766). A test asserts that a
  visual run reads no `Instant` in `ew-ui`'s frame path (a clock trait or a lint allow-list, the lane's choice).
- **Settling.** After each input the driver runs frames until egui requests no repaint within the next frame and no
  shell job is running, at most 120 frames (2 s of frame time); a job still running is waited for by blocking on its
  completion, never on time (the headless mode's rule, FT-513), so a frame never shows a half-done export. A batch's
  frame is painted after its last input has settled. Consequence: the agent never sees a progress field; the
  checker treats a long job without feedback as invisible to it, not as a U6 finding.
- **Equal inputs give equal frames on one host**, and equal dispatch logs on every host (layout is pure Rust on the
  bundled fonts; §12 names the risk).

### 5.2 The script and the transcript

- The run's `steps.uiscript` keeps the `ew.uiscript/1` header and holds the agent's inputs as `{"input":…}` lines
  exactly as sent (frame pixels), with the frame scale recorded in `run.json` (`senses`, `ppp`, `window`). Input lines
  are accepted only under `--visual`; every existing script, transcript and exit code is byte-identical without it.
- Per input line the transcript `ew.uitranscript/1` gains, additively, `input` (the line), `dispatched` (a list of
  `{action, outcome, reason}` in dispatch order) and `state` (the 15 members as today); a frame line adds `frame` (file
  name and SHA-256). A reviewed transcript change under ID-474's rule; no new describe word.

### 5.3 Recording and replay

- `outcomes.jsonl` in a visual recording holds per input `{step, input, dispatched}`.
- **R1v (decides):** every input's `dispatched` list equals the recording's. **R2 (decides):** the epilogue's
  `page.describe` texts equal `epilogue.txt`, as in the base specification. **R4 (reported):** the number of frames
  whose SHA-256 differs; pixels may drift with a graphics-driver update, and a drift that changes what an input hits
  shows up in R1v.
- `replay` runs without frames (`--no-frames`: the driver lays out every frame but paints none), so the CI test
  `blind_replay` replays visual recordings with `ci = "pr"` in the existing test jobs on all three operating systems,
  twice in fresh folders; `replay --frames` (Windows, the snapshots' platform) regenerates the PNGs for a reader.
- `bless` copies, besides the base specification's files, `friction.jsonl`, `frames.sha256` and, for a brief run, the
  checker's `checker.md` with its acceptance table; never the PNGs.

### 5.4 The keeper and the end of a run

The keeper serves a visual run as a text run: its child is `elektrawatt --headless --visual --script - --keep-going
--frames <run>/frames …` with the environment of the base specification's §5.4; `--stateless` replays the inputs
without frames and paints only the new frame. If the agent quits the application, the run ends `app-closed`; `stop`
then opens `work/project` in a fresh headless shell for the epilogue (an unsaved project is scored as not saved, H6 or
the brief's hard item). `start` refuses a visual run on a host without an offscreen adapter (`harness: no offscreen
adapter for a visual run`).

*As built (cycle 34).* A frame line of the script is `{"frame":"t0003.png"}` with the optional members
`"settle": false` (a look), `"zoom": [x, y, w, h]` and `"paint": false` (default true): the keeper's stateless calls
and `replay` settle as the run did and **paint only the frames they do not have**, rather than "without frames" in
general; `replay --frames <dir>` paints them. `blind_replay` replays visual recordings with `--visual --no-frames`.
`bless` records `friction.jsonl` (in text and visual runs), `frames.sha256` (`<sha256>  <file>` per painted frame)
and `checker.md` when the run has one, never a PNG. `start` probes for the adapter by running
`elektrawatt --headless --visual` on an empty script; the visual run's `run.json` carries `senses = "visual"`,
`ppp = 1.2` and `window = [1280, 800]` (absent in a text run, whose `run.json` is unchanged). When an input closes
the application, the `do` that sent it prints `harness: the application has closed`, the inputs after it in the batch
are not sent, the turn has no frame and the run ends `app-closed`; a later `do` prints
`harness: the run has ended (app-closed)`, and the hand-back is still filed.

## 6. Tasks: open briefs and the regression task

### 6.1 The brief task kind

A brief lives in `tools/blind/tasks/brief-<name>/`:

| File | Content |
|---|---|
| `task.md` | The brief in a user's words: the goal, the situation, the constraints (voltages, how many of what, what goes to the field), what to hand over (saved project, PDF, parts list). Never a drawing to copy, never a layout, never EW vocabulary |
| `task.toml` | `kind = "brief"`, `grade` (1 to 3), `version`, `turn_budget` (reported, never enforced), `senses` it is meant for (`text`, `visual` or both) |
| `acceptance.toml` | The acceptance list (§6.2) |
| `prelude.uiscript`, `prelude.toml` | Setup only (`class = "setup"`): the first-run defaults, the company-standard wizard and, until a starter parts library exists, the part copies the brief needs by `command.run library.embed` (FT-592); never a page or a drawing |

`check-brief <task>` (part 4) asserts: `task.toml` valid; at least three hard items, each with a `how`; every prelude
line `setup`; the brief text free of customer or personal names, project or drawing numbers and EW action IDs
(`check-anonymisation` and a word scan against `reference/actions.txt`).

*As built (cycle 34).* The brief kind goes further than the text above:

- **`solution.uiscript`**, a reference solution in the text vocabulary, is a sixth file of every brief.
- **`task.toml`** has, besides `kind`, `grade`, `version`, `turn_budget` and `senses`, two members:
  **`requires`**, what the brief needs of the product (`action: <id>`, `fndef: <id>`, `symbol: <name>`,
  `part: <number>`), each checked against the current build; and **`areas`** (required, not empty), the window's
  areas the brief exercises, in plain words (`brief-valve-switch`: "new page", "symbol picker", "terminal strips",
  "wiring", "save and export"). A visual round picks briefs by `areas` against the freeze list (§9.1).
- **`check-brief <task> [--bin <dir>]`** checks the files as above (unknown members refused; `grade` 1 to 3;
  `version` and `turn_budget` 1 or more; at least three hard items, each with a `how`; a setup-only prelude; the
  anonymisation deny-list; no action ID in the brief) and then **proves the brief runnable on the current build**: it
  replays the prelude and the reference solution headless in a scratch folder with the built `elektrawatt` and `ew`,
  makes the exports and decides every acceptance item on them (§6.2). A solution line in the command form
  (`command.run`) is reported as a finding (the window has no path for it). It prints one line per item
  (`A1 hard: met`), the findings and problems, and a last line saying the brief is runnable or
`` brief `<task>` is NOT runnable: <n> problem(s) ``.
- **`start --kind brief`** refuses a brief that is not runnable
  (`` harness: the brief `<task>` is not runnable on this build: `` followed by the problems).

### 6.2 The acceptance list

Each item of `acceptance.toml`: `id` (`A1`, …), `kind` (`hard` or `soft`), `what` (one sentence in engineering terms),
`how` (which product output decides it: `check`, `bom`, `connections`, `pages`, `pdf`, or a named read-only query in the
task's `queries.sql`). The common hard items, adapted per brief:

| Item | Hard requirement (engineering terms) | Decided from |
|---|---|---|
| Function | The required devices exist and are connected so that the function is met (each traced from source through protection and switching element to the load and back) | `connections`, `pages` |
| Potentials | The supply potentials are named and typed (for example L1, N, PE, 24 V DC, 0 V) | `connections` (potentials), `check` |
| Protection | Every circuit is protected by a fuse or circuit breaker sized as the brief asks | `pages`, `bom` |
| Field wiring | Every field device is connected through terminals, never directly to a cabinet device | `connections`, `pages` |
| Check | The check report is clean, or every remaining finding is explained in the tester's hand-back | `check`, `handback.md` |
| Parts list | Every placed device carries a part, quantities equal the drawn devices, nothing foreign | `bom` |
| Delivered | The tester saved the project and exported the PDF itself (H5 and H6 of the base specification) | `outcomes`, `work/` |

Soft items (design quality): current paths readable top to bottom or left to right, a consistent tag scheme,
cross-references between coils and contacts usable, page titles, the page not crowded, texts not overlapping in the
PDF. **Outcome of a brief run:** `met` (every hard item ok), `partly met` (at least half of the hard items ok, including
Delivered), `not met`, or `incomplete` (ended by cap, quit, orchestrator, lost or diverged before the epilogue). Soft
items never change the outcome.

**Who scores.** The checker (§7.3) decides each item `ok`, `miss` or `unclear` with the evidence line of the output it
read. For it, `{blind} report <run>` gains `connections` (a fixed `ew sql` over the wire and potential views:
end tags, CPs, potential name and type per wire), `pages` (the epilogue's `page.describe` texts) and `pdf` (the path of
the harness's PDF, which the checker may read as images); `check`, `bom` and `score` stay. The checker may read these
text exports; the tester may not (`report` stays refused to a drafter).

*As built (cycle 34).* An item's deciding output is named inline rather than by a `queries.sql` file: `how` is one or
more of `check`, `bom`, `connections`, `pages`, `pdf`, `outcomes`, `work` and `handback`, joined by `, `; an item may
carry a read-only `query` (a SELECT over the `api_v1_*` views) and an `expect` on its row count (`>= 3`, `== 0`, …),
which `check-brief` runs on the reference solution's project; an item without a query is decided by its output
(`check`: no error in the check report; `pdf`: the solution exported `project.pdf` itself; `bom`: the parts list has
rows). A **soft** item the current build cannot meet carries **`infeasible = "<reason / package>"`** and is decided
`infeasible (…)` instead of failing the brief; a **hard** item marked `infeasible`, or a hard item not met, makes the
brief not runnable. `brief-valve-switch` marks A7 (named and typed potentials: the window cannot name or type a
potential yet) and A8 (parts on every device: no starter parts library, FT-592) infeasible. The run outcomes `met`,
`partly met`, `not met` and `incomplete` are the checker's, unchanged. `report <run> connections|pages|pdf` serves a
check run only (BV10), and `stop` writes `out/connections.txt` and `out/pages.txt` for the checker; neither is in a
recording.

### 6.3 The brief library

| Brief | Grade | The goal (summary of `task.md`) | In |
|---|---|---|---|
| `brief-valve-switch` | 1 | *Added as built.* One 24 V DC solenoid valve in the field switched by an on/off switch on the cabinet door; +24 V and 0 V arrive on two terminals of a strip; a fuse right after the incoming +24 V; the valve only through terminals; the project saved and a PDF exported; one page or more as the tester likes. Turn budget 30 | **Part 4, runnable: the calibration brief** (§9.1) |
| `brief-24v-supply` | 1 | A 24 V DC control supply from a 230 V AC feed: incoming terminals, main switch, circuit breaker, power supply unit, fused 24 V distribution to two outgoing terminals, PE; one page | Part 5 |
| `brief-relays-valves` | 2 | **The owner's example:** three relays switching three 24 V DC solenoid valves in the field, each relay commanded by its own selector switch on the cabinet door; 24 V supply from 230 V AC with protection on both sides; the valves and the door switches through a terminal strip; parts list and PDF | **Part 4, files committed, not runnable on this build** (below) |
| `brief-dol-starter` | 2 | A direct-on-line starter for a 400 V three-phase motor: motor protection circuit breaker, contactor, start and stop with self-holding, a run lamp, 24 V control voltage, the motor through terminals; power and control on two pages with cross-references | Part 5 |
| `brief-pump-duty-standby` | 3 | Two pumps in duty and standby with a selector, a level switch and a fault input from the field through one cable onto a terminal strip, fault lamps, three pages | Part 5 |
| `brief-plc-io` | 3 | `brief-relays-valves` driven by a small PLC with a digital input and output card instead of selector logic | When WP-S1.21's PLC scope and WP-S2.07 part 3 are delivered (FT-593) |

**The rule for adding a brief.** A brief is added by a design pass when (1) it exercises a capability that a delivered
package made reachable from the window; (2) it is written in a user's words with placeholder data only; (3) its
acceptance list has at least three hard items, each with a `how` the product's outputs can decide; (4) the orchestrator
ran it once as a text run and it was doable (a sanity run, not a recording); (5) it has a grade and a turn budget. A
brief whose meaning a product change alters gets a version bump with a history line in `task.md`, never a silent edit;
a brief no longer useful is retired, not deleted, so old recordings keep their source.

*As built (cycle 34).* Part 4 delivered two briefs instead of one. **`brief-valve-switch`** (grade 1, version 1,
`senses = ["text", "visual"]`, prelude: the company-standard wizard with its defaults and a new project, no part copy)
passes `check-brief`: every hard item (A1 to A6) is met by its reference solution, the soft A7 and A8 are infeasible
and A9 is met. It is the calibration brief of §9.1. **`brief-relays-valves`** (grade 2, turn budget 120, its prelude
embeds the power supply's part copy, FT-592) is committed but **not runnable on this build**: the window cannot place
a normally open contact on an existing relay, because `device.place` with `/target/existing_function/function` gets
the relay's 6-character tag where a 36-character ID is required; its reference solution therefore misses A1 and A2
(each relay's coil and contact on one device, the contact switching its valve), while A3 to A8 are met. `start --kind
brief` refuses it until a window path exists. Rule (4) above is, as built, checked by `check-brief`'s replay of the
reference solution rather than by a sanity text run.

### 6.4 The regression task

`gpp-101-103` runs unchanged as a visual gate run (the copy task with its oracle, H1 to H7 and S1 to S8), so its score,
turns and inputs are comparable with the text recording `gpp-2` and with earlier visual runs. Its task sheet stays the
only text that names tags and cells; it is the drawing the user would get on paper.

### 6.5 Text runs of briefs

Every brief also runs as a text run (`--senses text`) with the base specification's prompt, which gains the friction
command (`HELP.md` version 7). The pairing rule of §7.4 reads both.

*As built (cycle 34).* `HELP.md` version 7 adds the friction command without `--frame` and `--region` (a text run
has no frames; the last step sent is the entry's reference) and the section "The friction log" with the seven
categories of §7.1; its hand-back keeps seven headings. The text self-test logs one friction entry and was re-blessed
(prompt and file digests changed; steps, transcript and outcomes unchanged).

## 7. The friction log, the checks and the hand-backs

### 7.1 The friction log

`{blind} friction {run} --severity <s> --category <c> --wanted "<text>" --expected "<text>" --saw "<text>"
[--frame <name>] [--region <x>,<y>,<w>,<h>] [--workaround "<text>"]` appends one line to `friction.jsonl`
(`ew.blindfriction/1`): `n` (from 1), `turn` and `step` (filled in by the harness: the current turn and the last step
sent), `frame` (default the newest frame; `null` in a text run, where `step` is the reference), `region` (frame pixels,
optional), `wanted` (what the tester wanted to do), `expected`, `saw` (what it saw or did instead), `severity`
(`blocker`, `confusing`, `annoying`, `cosmetic`), `category` (`discoverability`, `feedback`, `layout-legibility`,
`wording`, `flow-dead-end`, `behaviour-bug`), `workaround` (optional). Each text at most 400 characters. The command
needs no note and opens no turn; at most 300 entries per run; append-only (no edit or delete command). Refusals:
`harness: friction needs severity, category, wanted, expected and saw`, `harness: unknown severity <s>`,
`harness: unknown category <c>`, `harness: friction cap`.

**The rule the prompts give the tester:** log an entry **every time** it is unsure, confused, surprised, stuck, has to
guess, has to open the user guide, or works around something, at that moment and not afterwards; the turn note keeps
the two scores and the mood, the friction entry the facts.

*As built (cycle 34).* `ew.blindfriction/1` as specified (a header line `{"$schema":"ew.blindfriction/1"}`, then one
JSON object per entry with `n`, `turn`, `step`, `frame`, `region` as `[x, y, w, h]`, `wanted`, `expected`, `saw`,
`severity`, `category`, `workaround`), with one difference: **a seventh category, `missing-feature`**, "the program
cannot do what the job needs, as far as you can tell". It is a finding about the product, not the window: the checker
does not count it against the window (U1 to U7) and the pairing rule (§7.4) reads it as a product finding. Further
refusals as built: `harness: a friction text has at most 400 characters`,
`harness: a text run has no frames; the step is the reference` (`--frame` or `--region` in a text run),
`harness: no frame <name> in this run`, `harness: --region is x,y,w,h in frame pixels inside the frame` and
`harness: the hand-back is filed`. The command prints `friction <n> logged`. An example entry, from the visual
self-test recording:

```json
{"n":1,"turn":2,"step":6,"frame":"t0002.png","region":[640,390,280,40],"wanted":"read the tags of the two contacts","expected":"tags I can read at the page's first zoom","saw":"the tags are small; I zoomed into the frame to read them","severity":"cosmetic","category":"layout-legibility","workaround":"look --zoom"}
```

### 7.2 The UI checks

Scored by the checker per run, `ok`, `finding` or `n/a`, each finding with frame, region and evidence:

| ID | Check | Evidence |
|---|---|---|
| U1 | **Discoverability:** per task element (new page, place a device, choose a part, wire, terminal, bridge, potential, cross-reference, save, export) how many inputs and turns until its first successful action, and how it was found (command bar, menu, context menu, shortcut, palette, the guide) | `outcomes.jsonl` (`dispatched`), inputs, friction |
| U2 | **Dead ends:** stretches of 3 or more turns without a done action, modal states left only by Escape or undo, a state the tester could not leave | `dispatched`, frames |
| U3 | **Input cost:** inputs per folio and per task element, against the text run of the same task where one exists | counts from the harness's `report <run> inputs` |
| U4 | **Visual misreads:** a note or friction entry that contradicts the frame it cites or the dispatched log (read a label wrongly, clicked the neighbour, took an icon for another), with the cause (size, contrast, ambiguity, overlap) | notes, friction, frames, zoom frames |
| U5 | **Unreadable or clipped text:** elided, clipped, overlapping or low-contrast text in any frame of the run | frames |
| U6 | **Missing feedback:** an input whose frame shows no visible change although an action was refused or done, or a refusal without a visible reason | `dispatched` against frame hashes |
| U7 | **Layout against the design language:** defects of hierarchy, alignment, spacing and component use against [doc 16a](../../design/16a-design-language.md), ranked A, B or C as the window checks rank them | frames |

The harness computes the mechanical parts (part 5): `report <run> inputs` prints per turn the inputs, the dispatched
actions and whether the frame hash changed, and flags **silent inputs** (no done action and an unchanged frame) and
**refused-unseen** inputs (a refused action and an unchanged frame).

### 7.3 Triage: FT drafts and the ledger

**The checker** (prompt `play-checker`, `tools/blind/PLAYCHECKER.md`, part 5; the calibration round uses the base
specification's checker for the acceptance list and the lead for the rest) works in two passes, in this order: first
the acceptance list from the exports only, written before it opens anything of the tester's; then the friction log,
the notes, the inputs and the frames, for U1 to U7 and the FT drafts. Its audit `checker.md` has six headings:
`1. Acceptance` (each item with status and evidence), `2. Outcome`, `3. UI checks` (U1 to U7), `4. FT drafts, ranked`,
`5. Misreads`, `6. Confidence`.

**An FT draft:**

```text
### D<n> [blocker|confusing|annoying|cosmetic] <one-line title>
- category: <one of the seven>
- friction: <run>#<entry>[, <run>#<entry> …]; ledger FX-<k> when known
- where: frame t0042 region 812,96,140,22 (or: step 311 in a text run)
- seen: <what the tester saw or did>
- expected: <what it expected; the doc 16a section where the design language says so>
- cost: <inputs or turns lost, or blocked>
- owner: <the owning package, or "UI fix lane">; due: as §9.1 says (parked while its area is in feature work)
```

Ranked by severity, then the number of distinct personas, then the number of runs, then cost.

**The ledger** `tools/blind/friction/ledger.toml` (part 5), committed and kept by the lead's audit: one `[[cluster]]`
per distinct friction (`id = "FX-<k>"`, `title`, `category`, `severity` (the highest seen), `entries` (`<run>#<n>`),
`personas`, `senses`, `ft` (the FT it became), `status` (`open`, `fixed`, `by-design`)). **Rank** = severity weight
(blocker 8, confusing 4, annoying 2, cosmetic 1) × the number of distinct personas + the number of distinct runs; the
design pass lists the top of the ledger per round, so cross-run trends are visible. Clustering is the lead's judgement;
`cargo xtask blind friction-report <run> …` (part 5) merges the runs' logs grouped by category and frame region to
help it. A cluster closed as `fixed` that recurs is reopened with its history.

*Amended in the cycle-34 design pass (the owner's ruling 7, §1):* friction in an area still in feature work is
**parked** in the ledger, not turned into a fix; §9.1 says which findings are not parked. The ledger and
`friction-report` are part 5's and not built in cycle 34.

### 7.4 The pairing rule (text against visual)

For a brief or the gate task run both ways in one round: friction only in the visual run is a **window** finding;
friction in both is a **flow or product** finding; a hard item missed in both says the product (or the brief) cannot do
it; missed only in the visual run, the window hides a capability the product has.

### 7.5 The hand-backs

The play-tester's hand-back (prompt `play-tester`, `tools/blind/PLAYTESTER.md`; text runs of briefs keep
`BLIND_DRAFTER.md` with its headings) has **eight headings**: `1. Keep or drop`, `2. Where it lost me`, `3. What the
tool is`, `4. Broken`, `5. My design against the brief` (what it built, what it would still do, and the time it would
need in its usual tool), `6. Flow chart`, `7. Good`, `8. What I could not read` (frames and regions; in a text run
"none"). The harness checks the headings.

*As built (cycle 34).* As specified: `PLAYTESTER.md` lists the eight headings and the harness checks them for a
visual run (`handback.md`); a text run, briefs included, keeps the drafter's seven headings of `HELP.md`.

## 8. Personas

Personas are fictional roles, never people; each is a versioned file under `tools/blind/personas/` (base
specification's D10) and independent of the senses, so one persona runs text and visual.

| Persona | Background and voice | What it tests | In |
|---|---|---|---|
| `first-timer` | An electrician who wires cabinets from paper drawings and has never used ECAD; plain words, no jargon, reads every label, opens the guide when lost and says so | Discoverability, wording, first-run flow, whether the window teaches | **Part 4** |
| `qet-veteran` | As the base specification (unchanged, version 1) | Speed against habits from QElectroTech, drag habits, folios and cells | Exists |
| `switcher` | As the base specification (EPLAN or E3 habits, unchanged) | Expectations of a professional tool: navigators, master data, macros, device tags | Exists |
| `keyboard-expert` | A hurried expert who avoids the mouse: shortcuts, the palette, Tab and Enter, typed values; impatient with anything that needs a pointer | Keyboard reach (NFR-UX-03's direction), the palette, focus order, shortcut feedback | Part 5 |
| `reviewer` (optional) | A careful checking engineer who draws slowly and verifies every number, cross-reference and list | Feedback and trust: whether what the window shows matches what was saved and printed | Part 5 |

Every persona file ends with a **voice** paragraph: how this person writes friction entries (the first-timer: "I wanted
…, I didn't know …"; the expert: terse, with the shortcut tried). A visual run's composed prompt is `PLAYTESTER.md`,
the persona, the brief or task sheet and `HELP-VISUAL.md`, by the base specification's `compose`.

*As built (cycle 34).* `tools/blind/personas/first-timer.md` (`persona: first-timer`, version 1) as specified, with
the sections "Your habits and terms", "What you do not know", "Your standard" and "Your voice"; `compose` and
`start` take `--senses text|visual` and `--persona <p>`. `keyboard-expert` and `reviewer` stay part 5's.

## 9. Where it runs: the gate rule and the polish cycles

*Amended in the cycle-34 design pass:* the owner's rulings 4 to 9 of 2026-10-29 (§1) replace the first
specification's baseline round by the calibration round of item 1, and its visual round at the gate of every UI-heavy
cycle by the polish-cycle rule of items 2 and 3.

### 9.1 The gate rule

1. **Calibration round, cycle 34** (the cycle that built part 4, after the merged-tip verify): the reading probe
   (BV13), then `brief-valve-switch` visual with `first-timer` and the same brief as a text run with `first-timer`; the
   checker; the lead's triage. Its findings are parked (item 5), except blockers in stable areas. It is the only
   visual run of cycle 34: `brief-relays-valves` is not runnable on this build (§6.3), and `gpp-101-103` visual is
   not part of it.
   Result: recorded at the cycle-34 gate (ID-pending).
2. **A visual round runs only at the gate of a pure UI-polish cycle**, a cycle with no feature WP in it: at least
   three personas on at least two briefs, one of them the brief whose areas the cycle's polish touches most, plus a
   text run of one of them for the pairing rule (§7.4). A cycle with a feature WP runs no visual round, however much
   it changes the window.
3. **One polish cycle is placed right before each milestone gate from G-M2**, and its gate round is the milestone's
   visual round: the brief library available then (within item 4), every persona once, and `gpp-101-103` visual as the
   regression run, read beside the gate's other evidence.
4. **Briefs only on frozen areas.** A round runs a brief only when its `areas` (§6.1) are frozen for the coming
   cycles, by the freeze list the plan keeps in [the global plan](../01-global-plan.md).
5. **Parking.** Friction in an area still in feature work is parked in the ledger (§7.3), not fixed.
6. **Text runs stay at feature gates**: the base specification's runs and the text runs of briefs run at the gates of
   feature cycles.
7. **Never a criterion** (ID-341, ID-612): a round is evidence. One binding consequence: a `blocker` cluster that two
   or more personas hit is due before the next milestone gate, and the gate record names it.
8. Testers and checkers run on Opus at `low` (ID-700), the lead's triage on Opus; the orchestrator may lower the caps
   of §4.2 for a round after the calibration round's token count (§12).

### 9.2 M2's cycles and its visual round

M2's heavy UI feature work stays grouped into **two UI cycles**, with the non-UI packages (WP-S1.19's catalogues,
WP-S1.16's rule packs, WP-S1.21, WP-S1.18, WP-S3.06, WP-S4.01, WP-S4.02, WP-S7.03) in the cycles around them. Both
carry feature WPs, so by item 2 their gates run no visual round and by item 6 they run text runs; M2's visual round is
the round of the polish cycle right before G-M2 (item 3).

| Cycle | When | Content | Gate |
|---|---|---|---|
| Harness cycle | Cycle 34 (*as built*: part 4 was built in cycle 34 by lane P34, not in cycle 35 as specified) | WP-S6.13 part 4 (one Opus lane) beside the cycle's other lanes | The calibration round (item 1) |
| **UI cycle 1** | When WP-S1.19 parts 1 to 5 and WP-S2.07 part 1 are delivered | WP-S2.07 parts 2 and 4 (the edit-cable table, the part selection dialog and part editor), WP-S1.19 part 6 (the library panes), what is left of W33's items (FT-557 to FT-567, FT-571, FT-581) after cycle 34, and the open window follow-ups FT-518, FT-520, FT-522 and FT-568; part 5 of this package beside them | Text runs (a feature cycle, item 6) |
| **UI cycle 2** | The last feature cycle before the R1 release candidate (WP-S2.12, six weeks before M2) | WP-S2.08 (palette modes, keymaps, accessibility names, Fluent with English and Dutch, pseudo-locale), WP-S2.07 part 3 (the PLC navigator and grid, after WP-S1.21) | Text runs (a feature cycle, item 6) |
| **M2 polish cycle** | Right before G-M2 | UI polish only, no feature WP | The polish round, which is G-M2's milestone round (items 2 and 3) |

## 10. Parts, estimates and routing

| Part | Content | Size | Routing |
|---|---|---|---|
| **4 (V1)** | The visual driver and `--visual` with the dispatch log and the moved offscreen renderer; FT-570's clocks onto the frame clock; the harness's `--senses visual` (filter, inputs, caps, frames, zoom, keeper and stateless); the friction command (both senses); the brief kind with `check-brief` and `report connections\|pages\|pdf`; `brief-relays-valves`; `PLAYTESTER.md`, `HELP-VISUAL.md`, `HELP.md` version 7, the `first-timer` persona; a visual self-test recording replayed in CI | **1.2 pw** | One Opus lane at `high` in one cycle (product determinism and a new driver; the harness patterns exist) |
| **5 (V2)** | The briefs `brief-24v-supply`, `brief-dol-starter`, `brief-pump-duty-standby` with their sanity runs; the personas `keyboard-expert` and `reviewer`; `PLAYCHECKER.md`; `report <run> inputs` with silent and refused-unseen inputs; `friction-report`; the ledger format with its rank | **0.6 pw** | Opus at `medium` (writing to this outline plus small harness code) |
| Runs | The calibration round, then the polish rounds of §9.1 | 0.2 to 0.3 pw a round | Orchestrator; the lead's triage on Opus; from the 20 % reserve, as part 3 |

**Part 4's build list, in order:** (1) move the offscreen renderer core into `ew-ui/src/offscreen.rs`, tests re-pointed,
pixel gate green; (2) the frame clock and FT-570's two rules; (3) `ShellModel`'s dispatch log; (4) `ew-ui/src/visual.rs`:
input parsing, `RawInput` building, settling, the title strip, the platform's fixed answers, frames and zoom, `--no-frames`;
(5) `bin/elektrawatt`: `--visual`, `--frames <dir>`, `--no-frames`, usage text; (6) the harness: `--senses`, the visual
filter and caps, `do` and `look` for frames, the keeper's and stateless child line, `run.json` members, `outcomes.jsonl`
with `dispatched`, R1v and R4 in `replay`; (7) `friction` and `ew.blindfriction/1`, recorded by `bless`; (8) the brief
kind: `task.toml`, `acceptance.toml`, `check-brief`, `report connections|pages|pdf`; (9) the files: `brief-relays-valves`,
`PLAYTESTER.md`, `HELP-VISUAL.md`, `HELP.md` v7, `personas/first-timer.md`; (10) `selftest-visual`: a scripted agent of
about 40 inputs, blessed `ci = pr`; (11) targeted checks.

**De-scope order** (a projected overrun above 20 % at the lane's midpoint): `look --zoom` (then the reading probe decides
whether frames must grow), `--stateless` for visual runs, the `--theme` and `--density` options, the replay on macOS and
Linux (Windows only); the lane reports which left.

*As built (cycle 34).* Part 4 was built in cycle 34 by one Opus lane (P34) in nine commits, in the build-list order:
the renderer move, FT-570's clocks, the dispatch log, the driver, the binary's flags, the dialogs' folder and the
frame line's `paint`, the harness with the friction log and the briefs, BV10, and the re-blessed self-test recordings
on the merged tip. Of the de-scope order only **`--theme` and `--density`** were left out; the zoom, `--stateless` for
visual runs and the visual replay in CI were built. Item (9) delivered **two briefs**, `brief-valve-switch` (runnable)
and `brief-relays-valves` (not runnable on this build, §6.3); the brief kind also gained `solution.uiscript`,
`requires` and `areas` (§6.1). `sha2` moved from a dev-dependency to a dependency of `ew-ui` (already in the graph
through `ew-api`; no new package).

## 11. Acceptance checks

| # | Check | Where | Proves |
|---|---|---|---|
| BV1 | `--visual` only with `--headless` (else exit 2); without it every existing headless test and transcript is byte-identical | `elektrawatt`, `ew-ui` tests | §5.2 |
| BV2 | Each input form parses with its bounds; an out-of-frame coordinate, an unknown key name, a text over 200 characters or with a control character, a `wait` out of range are refused with their texts | `ew-ui` and `xtask` unit tests | §4.1 |
| BV3 | A frame is 1536 × 994 with the title strip; a `click` at the frame pixels of a known button of a Golden A copy's window dispatches that button's action; a `right_click` opens the context menu; a `drag` on the canvas moves a selection; `type` into a field sets it | `ew-ui` test (Windows for the frame, every OS for the dispatch) | §3.1, §4 |
| BV4 | The same input script twice in fresh folders: equal dispatch logs and states on every OS, equal frame hashes on Windows; no wall-clock read in the visual frame path; the dwell and the job-field rule follow the frame clock | `ew-ui` tests | §5.1 |
| BV5 | Settling: an export started by clicks is finished in the batch's frame; the caret is drawn in every frame | `ew-ui` test | §5.1 |
| BV6 | The platform's fixed answers for new, save, open and PDF; a QET import cancelled; `open_url` false | `ew-ui` test | §4.3 |
| BV7 | The visual filter: text looks, `look --page`, action lines and a batch of 13 refused with their texts; caps end the run; every input of a batch is sent; no broken rule; `do` prints only the frame path | `xtask` tests with a fake product | §3.2, §3.3, §4.2 |
| BV8 | `friction`: required fields, enums, length and cap refusals; numbering, turn, step and frame filled in; works in a text run; append-only; `bless` records `friction.jsonl` | `xtask` tests | §7.1 |
| BV9 | `check-brief brief-relays-valves` passes; a brief with two hard items, a `drawing` prelude line or an action ID in its text fails. *As built:* `check-brief brief-valve-switch` passes with every hard item met on its replayed reference solution; `brief-relays-valves` fails only on its relay contacts (A1, A2, the `existing_function` target), with A3 to A8 met; a hard item marked `infeasible` or an unmet `requires` fails | `xtask` tests | §6.1 |
| BV10 | `report connections`, `pages` and `pdf` serve a check run and are refused to a tester | `xtask` tests | §6.2 |
| BV11 | `selftest-visual` runs through the keeper and the copied binaries, blessed `ci = pr`; `replay --twice` passes R1v and R2; `blind_replay` replays it in CI; R4 is reported. *As built:* the scripted agent sends 18 inputs in 7 turns (not about 40), with looks, a zoom, one friction entry, two refused calls (an action line and a text look) and the eight-heading hand-back; recording `tools/blind/recordings/selftest-visual` | lane; `elektrawatt` test | §5.3 |
| BV12 | Targeted nextest and clippy for `ew-ui`, `ew-viewmodel`, `elektrawatt`, `xtask`; `cargo fmt --all --check`; `check-deps` (no new edge); `reuse lint`; `check-licence-map`; `check-anonymisation`; `Cargo.lock` and `golden/` unchanged | lane | testing policy |
| BV13 | **Reading probe** (orchestrator, after part 4): one frame of a Golden A copy's window with 20 marked texts (device tags on the canvas, menu and command-bar labels, status bar, Properties rows); an Opus agent at `low` transcribes them; 18 or more exact keeps the default; fewer makes `PLAYTESTER.md` require a zoom before acting on tag-sized text, and the result is recorded | orchestrator | §3.1, §12 |
| BV14 | **Calibration round** (orchestrator, cycle 34): the reading probe and the runs of §9.1 item 1 valid by their tool-log audits; the checker's acceptance table for `brief-valve-switch`; the lead's triage, its findings parked except blockers in stable areas; the token use of each run recorded for §12. Result: recorded at the cycle-34 gate (ID-pending) | orchestrator, lead | §9.1 |
| BV15 | Part 5: three briefs pass `check-brief` and one sanity text run each; the two personas; `PLAYCHECKER.md` with the six headings; `report inputs` flags silent and refused-unseen inputs on a fixture; `friction-report` merges two runs; the ledger parses and ranks a fixture | lane; `xtask` tests | §6.3, §7.2, §7.3, §8 |

## 12. Risks

| Risk | Effect | Mitigation |
|---|---|---|
| **Token cost per run** | About 2,000 image tokens per frame; a brief run of 100 to 150 turns with looks reads 150 to 250 frames (0.3 to 0.5 M image tokens fresh), and the agent's growing context multiplies the total; not measured (owner's rule) | Caps (§4.2); one frame per batch of up to 12 inputs; looks capped per turn; the prompt tells the tester the newest frame is the only one it needs; the calibration round records the cost and the orchestrator sets the round caps from it |
| **Image size** | 80 to 200 KB per frame, tens of MB per run | Frames never committed; hashes only; `replay --frames` regenerates them |
| **Flaky renders** | WARP or driver updates shift pixels; a frame differs between hosts | Replay decides on the dispatch log (R1v), frames are R4, a report; one host class (Windows) paints for live runs and readers |
| **Layout differs across OSes** | R1v differs on Linux or macOS for the same inputs | Detected by BV4 and BV11; the de-scope keeps the visual replay on Windows only |
| **Misreading small text** | The tester reads `-1O1F1` for `-101F1` or clicks a neighbour; findings blame the product wrongly | 1.2 scale and the zoom; BV13's probe; U4 separates product causes (size, contrast) from the agent's vision by re-reading the zoom frame |
| **Wall-clock leaks** | A timer in `ew-ui` that is not on the frame clock makes a replay diverge | BV4's check over the frame path; FT-570 closed by part 4 |
| **Dialogs answered for the tester** | The system file dialogs' usability is untested | Out of scope by design; the window's reaction is in |
| **Brief scoring by judgement** | Two checkers disagree on a hard item | Each item names its deciding output; `unclear` is allowed and goes to the lead |

## 13. Files owned, out of scope and documents to update

**Part 4:** `crates/ew-ui/src/{visual.rs,offscreen.rs}` (new), `crates/ew-ui/src/{app.rs,canvas.rs,lib.rs}` (the frame
clock, the module lines), `crates/ew-ui/tests/offscreen/mod.rs` and the snapshot tests (re-pointed only),
`crates/ew-viewmodel/src/shell.rs` (the dispatch log), `bin/elektrawatt/src/main.rs` and its `tests/blind_replay.rs`,
`xtask/src/blind/**`, `tools/blind/{PLAYTESTER.md,HELP-VISUAL.md,HELP.md}`, `tools/blind/personas/first-timer.md`,
`tools/blind/tasks/brief-relays-valves/**`, `tools/blind/tasks/brief-valve-switch/**` (*as built*),
`tools/blind/tasks/selftest-visual/**`,
`tools/blind/recordings/selftest-visual/**`. **Part 5:** `tools/blind/tasks/brief-{24v-supply,dol-starter,pump-duty-standby}/**`,
`tools/blind/personas/{keyboard-expert,reviewer}.md`, `tools/blind/PLAYCHECKER.md`, `tools/blind/friction/ledger.toml`,
`xtask/src/blind/**` (`report inputs`, `friction-report`). Read-only: `golden/**`, `ci/**`, `deny.toml`,
`supply-chain/**`, `.github/**`, `docs/**`.

*As built (cycle 34).* Part 4 also changed `crates/ew-ui/src/{clock.rs (new),jobs.rs,page_view.rs,headless.rs}`,
`crates/ew-ui/Cargo.toml` (`sha2` as a dependency), `crates/ew-ui/tests/{visual.rs (new),jobs.rs,tools.rs}`, the
text self-test's `tools/blind/tasks/selftest/**` and its recording `tools/blind/recordings/selftest/**` (one friction
entry, re-blessed); `Cargo.lock` and `golden/` are unchanged.

**Out of scope:** the system's file dialogs; pixel comparison as a check; a model in CI; the moderated rounds
(WP-S2.12, WP-S2.16); visual runs on the owner's desktop or any real window (never); `brief-plc-io` until FT-593.

**Documents the design passes update.** After part 4: doc 18 §3.17 (the visual kind, the friction log, the brief kind
as built), doc 16 §3.9 (`--visual`), the base specification's status line, plan 02's WP-S6.13 status, FT-570 closed.
After the calibration round: plan 01 §3.8 (the round and its findings), the decision log. After part 5: doc 18 §3.17, gates §5.5
if the round's form changes.

---

## Sources

- [The base specification](WP-S6.13-blind-drafter-harness.md): D3, D4, D5, D10 to D16, §5.2, §5.3, §5.4, §5.7, §5.9,
  §5.10, §5.11, §12.7 to §12.9.
- [Decision log](../06-decision-log.md): ID-341, ID-474, ID-612, ID-613, ID-678, ID-700, ID-751, ID-766, ID-768,
  ID-776 to ID-779, ID-782 to ID-794.
- [Work breakdown](../02-work-packages.md): WP-S1.16, WP-S1.19, WP-S1.21, WP-S2.07, WP-S2.08, WP-S2.12, WP-S2.17,
  WP-S6.13; Appendix B (FT-518, FT-520, FT-522, FT-557 to FT-571, FT-581, FT-592, FT-593).
- [Gates](../03-gates.md) §5.4, §5.5; [quality](../../design/18-quality-security-release.md) §3.17;
  [design language](../../design/16a-design-language.md); ADR-0015 (the CPU canvas as the headless path), ADR-0031
  (the design language), both unchanged.
