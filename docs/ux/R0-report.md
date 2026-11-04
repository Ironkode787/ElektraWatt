# R0: the formative usability round (blind personas)

The first of the usability rounds of [doc 16 §19](../design/16-ux-ui.md#19-usability-test-plan), run as a
blind-persona round (ID-1331 (5)) by the method of [WP-S2.12](../plan/specs/WP-S2.12-usability-rounds.md). The method,
the file layout and the proxy statement are in the [README](README.md); the evidence of every run is in
`tools/blind/rounds/r0/<run>/`.

## 1. What this round is

**These are a model's attempts in a persona's role, not people's.** A completion below is an outcome the product's
exports decide (the acceptance list, decided mechanically at `stop`), so it is counted and read; a questionnaire
answer is a model's attitude without a norm, so its figure carries no threshold. Neither is NFR-UX-01's completion
rate or NFR-UX-02's SUS score. R0 is NFR-UX-09's formative round: two personas mirror the pilot user
(`qet-veteran`) and a colleague (`qet-user`) on T1, T2, T8 and the ADR-0007 probe A7. It runs after M-PP rather than
"as soon as the tools existed", as doc 16 §19.2 planned.

## 2. The build and the runs

The build is the working tip of cycle 51 with WP-S2.12's first sitting (ID-1361), the binaries reporting `0.0.0`.
The personas, play-testers, drafters and checkers ran on Opus at `low` (D18), the visual runs at the default frame
(1280 × 800 at 1.2, D12, ruling O4).

| Run | Senses | Attempt | Items | Hints | Turns | Steps | Refused | Friction | PQ |
|---|---|---|---|---|---|---|---|---|---|
| r0-t1-qet-user-v | visual | with help | A1-A3 met | 1 | 9 | 13 | 0 | 3 confusing, 1 annoying, 1 cosmetic | 72.5 |
| r0-t1-qet-veteran-v | visual | not completed | A2 not met | 0 | 9 | 15 | 0 | 2 confusing, 1 annoying, 1 cosmetic | 72.5 |
| r0-t1-qet-veteran-t | text | with help | A1-A3 met | 2 | 7 | 13 | 0 | 2 confusing, 1 annoying | 50.0 |
| r0-t2-qet-user-v | visual | unassisted | A1-A3 met | 0 | 7 | 18 | 0 | 1 confusing, 1 annoying | 72.5 |
| r0-t2-qet-veteran-v | visual | unassisted | A1-A3 met | 0 | 3 | 12 | 0 | 2 confusing, 1 cosmetic | 70.0 |
| r0-t2-qet-veteran-t | text | unassisted | A1-A3 met | 0 | 3 | 10 | 0 | 1 cosmetic | 77.5 |
| r0-t8-qet-user-v | visual | not completed | A2 not met | 0 | 2 | 2 | 0 | 1 annoying, 2 cosmetic | 80.0 |
| r0-t8-qet-veteran-v | visual | unassisted | A1-A3 met | 0 | 2 | 2 | 0 | 1 confusing, 1 annoying, 1 cosmetic | 72.5 |
| r0-t8-qet-veteran-t | text | unassisted | A1-A3 met | 0 | 2 | 2 | 0 | 1 confusing | 67.5 |
| r0-a7-qet-user-v | visual | unassisted | A1-A4 met | 0 | 7 | 11 | 0 | 3 confusing, 2 annoying | 50.0 |
| r0-a7-qet-veteran-v | visual | unassisted | A1-A4 met | 0 | 3 | 5 | 0 | 1 cosmetic | 75.0 |
| r0-a7-qet-veteran-t | text | unassisted | A1-A4 met | 0 | 6 | 18 | 2 | 2 confusing | 55.0 |

No run undid a step. Two checkers audited the two runs with a hard item unmet (D14, O8):
`r0-t8-qet-user-v-c` and `r0-t1-qet-veteran-v-c`; both confirm the mechanical decision.

**Validity.** The orchestrator audited every agent's tool log (D3). No run is voided. The only calls outside a run
were seven agents' first look for their own `prompt.md` under a wrong run root that the orchestrator had given them
(the file was not found, nothing was read), and one agent's listing of that root, which showed another run's folder
name and no content. The harness's own help output filtered by the agent's shell is not outside the run.

## 3. Per task

| Task | Unassisted | With help | Not completed | What the personas said |
|---|---|---|---|---|
| T1 Place | 0 | 2 | 1 | "Insert device: nothing matches; the Part field on the placed device offers A9F78116" (both senses); "connected into the wire it was dropped on, behind my back" |
| T2 Wire | 3 | 0 | 0 | "the new wires overlap the existing minus wires"; "33 errors project-wide scared me" |
| T8 Navigation | 2 | 0 | 1 | "the arrival is not highlighted"; "no visible back control; Alt+Left worked by guess"; text: "the page text gave L-1 no target folio" |
| A7 Move | 3 | 0 | 0 | "selecting by tag selects the device, Move refuses"; "the wire takes a rectangular detour"; "plain wheel zoomed instead of scrolling" |

The two attempts not completed:

- **r0-t1-qet-veteran-v** placed -102F1 at 102-C5, "just left of" the G1-F2 path by its own account, and the toast
  named the cell. A2 (row B to E, column 6 or 7) is not met; the checker would move it one column right. The persona
  placed it knowingly where the item does not allow it (see R0-F12).
- **r0-t8-qet-user-v** followed L-1 to 102-A1 and returned by double-clicking the arriving interruption point's
  back reference "101-H16", a second follow, not Back. A2 (`nav.follow` then `nav.back`) is not met; the checker
  confirms. **Ruling 3:** this is the item being too narrow, not a discoverability failure of this persona: following
  the back reference is QET's habit and returns to the same place on folio 101, correctly. The discoverability of
  Back is a separate finding that the other visual persona raised (R0-F6). The proposed change to A2 is row P-3.

## 4. Findings

Severity on Nielsen's 0 to 4 by D15. Evidence as `<run>#<friction entry>` (frame); "both" in the pairing column means
friction in a visual and a text run (a product finding), "visual" a window finding (D12).

| ID | Title | Severity | Personas | Evidence | Pairing | Owner | Route |
|---|---|---|---|---|---|---|---|
| R0-F1 | Insert device (Device from part…) finds no A9F78116, though the project holds its part copy and Choose part finds it; T1 met only with help by two personas | **3** | qet-user, qet-veteran | t1-qet-user-v#1, #3 (t0002, t0007); t1-qet-veteran-v#1, #2 (t0002, t0007); t1-qet-veteran-t#1, #2 | both | the picker's parts (WP-S2.18) and the part selection (WP-S2.07 part 4), both complete | fix lane (ID-1281), kittest flow below, before R1 |
| R0-F2 | The "N Pending" badge on the properties says nothing about what is pending | 2 | qet-user, qet-veteran | t1-qet-user-v#4 (t0009); t1-qet-veteran-v#3 (t0009); t2-qet-veteran-v#1 (t0002); a7-qet-user-v#1 (t0001) | visual | the properties' findings badge (WP-S2.09) | follow-up row P-5 |
| R0-F3 | Check project's toast counts the whole project's 33 errors, so a clean edit looks broken | 2 | qet-user, qet-veteran | t2-qet-user-v#2 (t0005); t2-qet-veteran-v#3 (t0003) | visual | the check toast (WP-S2.09) | follow-up row P-6 |
| R0-F4 | A device dropped on a wire connects into it without asking | 2 | qet-user | t1-qet-user-v#2 (t0005); B1 `ask` | visual | the place tool's autoconnect (WP-S2.04); ADR-0007's record | §6, row P-7 |
| R0-F5 | New wires route with jogs or rectangular detours and run over existing wires | 2 | qet-user, qet-veteran | t2-qet-user-v#1 (t0004); t2-qet-veteran-v#2 (t0002); a7-qet-user-v#5 (t0006); a7-qet-veteran-v#1 (t0002); a7-qet-veteran-t#2 | both | the router | follow-up row P-8 |
| R0-F6 | After a cross-reference jump the arrival is neither centred nor marked and Back has no visible control | 2 | qet-user, qet-veteran | t8-qet-user-v#1, #3 (t0001, t0002); t8-qet-veteran-v#1, #2, #3 (t0001, t0002) | visual | navigation (WP-S2.03) | follow-up row P-9 |
| R0-F7 | The two cross-reference labels "102-A1" of L-1 and N-1 overlap between the arrows | 2 | qet-user | t8-qet-user-v#2 (t0002) | visual | Golden A's 102 (the label placement) | follow-up row P-10 |
| R0-F8 | The page text of an interruption point names no continuation | 2 | qet-veteran | t8-qet-veteran-t#1 | text | the describe vocabulary (WP-S6.13's `page.describe`) | follow-up row P-11 |
| R0-F9 | Texts at the first zoom (69 to 88 %) too small: pin numbers, part labels, title block, cross-references | 2 | qet-user, qet-veteran | t1-qet-user-v#5 (t0009); t1-qet-veteran-v#4 (t0009); hand-backs' heading 8 of six visual runs | visual | FT-713 (WP-S2.11's O2) | §7 |
| R0-F10 | A device selected by its tag is not movable (Move refuses "nothing movable"), and a click on the lamp's centre selects the wire through it | 2 | qet-veteran | a7-qet-veteran-t#1; a7-qet-user-v#4 (t0005) | both | the Select tool (WP-S2.04) | follow-up row P-12 |
| R0-F11 | The plain mouse wheel zooms where a QET user expects it to scroll; opening the messages zoomed the sheet out to 35 % | 1 | qet-user | a7-qet-user-v#2, #3 (t0002, t0004) | visual | the canvas input (WP-S2.03) | follow-up row P-13 |
| R0-F12 | T1 placed one column left of the item's box (102-C5) | 0 | qet-veteran | t1-qet-veteran-v; its checker | visual | — | the persona's own choice; see the orchestrator's list |
| R0-F13 | Returning from a jump by the back reference counts as not returning | 0 | qet-user | t8-qet-user-v; its checker | visual | this package (T8's A2) | row P-3 |
| R0-F14 | The part actions and a finished wire answer a bare "done" in the text protocol | 1 | qet-veteran | t1-qet-veteran-t#3; t2-qet-veteran-t#1 | text | the describe vocabulary | follow-up row P-14 |

**To the orchestrator (§9.3 item 4).** R0-F12's severity rests on one persona's reading: (a) 0, the persona placed
the breaker beside the path on purpose, the window named the cell and the checker found no other miss; (b) 3, a wrong
output, a device off its supply path that a reviewer marks up, with a hard item unmet. This report takes (a). No
finding points at autoconnect as model semantics (D17): see §6.

## 5. The fix list

| Finding | Severity | Owner | Kittest flow it needs | State |
|---|---|---|---|---|
| R0-F1 | 3 | fix lane by ID-1281 (WP-S2.18's picker parts and WP-S2.07 part 4's part selection, both complete) | on GP-UT, folio 102: Device from part…, filter `A9F78116`: the project's part A9F78116 is a row and places -102F1 with its part (or, if the copy cannot be placed, the empty list names Choose part… for a placed symbol), asserted whole | open; closes before R1 (G-M2-22), re-run by R1's T1 runs |

## 6. ADR-0007's early evidence

| Question | What the personas did and answered |
|---|---|
| Autoconnect on move (question 2, B1) | In A7 every persona moved the lamp back and drew the lost wire itself; none tried Connect while moving (refused in this build, `reason-connect-on-place`). B1: `ask` 3 of 3 in A7, and `ask` once in T1 ("I want to decide myself when a dropped device connects into a wire"); none `connect`. |
| Autoconnect on placement (B2) | T1 placed the breaker without wiring; one visual persona saw it connect into the wire it was dropped on and called that "behind my back" (R0-F4). B2 `keep` once (text), else `not-seen`. |
| Boundaries across pages (question 3, B3) | T8: `yes` once, `partly` twice: the arrow names folio and cell, but the arrival is not marked and the way back is hidden (R0-F6). |
| Junction order (amendment 2, B4) | T2 and A7: `partly` three times, `yes` once; the comments concern the routes' jogs (R0-F5), not the order of the wires. |

**Limits.** A7 as built leaves a gap of about 13 mm between S56:14 and E56:X1, so the move back lines the points up
but never puts them on each other: it probes the expectation, not autoconnect onto coincident points. The personas are
QET personas; the EPLAN expectations come in R1. Nothing here asks for autoconnect as model semantics.

## 7. FT-713's friction 3

**A finding, Nielsen 2 (R0-F9).** At the first zoom of the default frame (69 % to 88 % of the sheet), six visual runs
report pin numbers, part labels, the title block or the cross-reference texts as too small to read, and no text run
does: a window finding. It slowed the personas (they zoomed or read the properties instead) and stopped none.

## 8. The PQ figures

Per attempt in §2. Per persona and sense: `qet-user` visual 68.75 (72.5, 72.5, 80.0, 50.0); `qet-veteran` visual
72.5 (72.5, 70.0, 72.5, 75.0); `qet-veteran` text 62.5 (50.0, 77.5, 67.5, 55.0). Round: 67.9 over 12 attempts. The
figures are SUS's arithmetic on this project's own ten statements, reported without a threshold and never as a SUS
score.

## 9. What the proxy cannot give

- **NFR-UX-01's completion rate** is a rate of people; R0 counts 8 unassisted, 2 with help and 2 not completed model
  attempts as a reading, and R0 has no rate to meet.
- **NFR-UX-02's SUS score** is a questionnaire of people normed on people; the PQ figure above is a model's answers.
- **NFR-UX-06** needs the pilot's own project and hour; it stays with the pilot log (§12 O5).
- **The pilot user's own judgement**: `qet-veteran` plays the pilot's role from a description, not from the pilot's
  practice.
- **Think-aloud**: the notes and friction entries are the persona's written account, made as it went, not speech.

## 10. The proposed follow-up rows

| Row | What | Owner |
|---|---|---|
| P-1 | T7 is not runnable: `check.waive` is HumanOnly and `check-brief` replays headless, so the waiver item cannot be proven; let `check-brief` replay a brief through the visual driver for a human-only step, or keep T7 out of R1 | WP-S2.12 (the harness), ruling for the orchestrator |
| P-2 | T9 to T11's task sheets (the panel page, the PLC binding and TIA export, the production package), left by §10's de-scope rule | WP-S2.16 |
| P-3 | T8's A2 accepts a return by the arriving cross-reference as well as by Back: `actions = ["nav.follow", "nav.back or nav.follow"]` with a new item that folio 101 shows again at the run's last step (needs the page in the decision, a harness addition); the sheet's version 2 | WP-S2.12, before R1 |
| P-4 | ID-1355's package set lacks `ew-library`, which names `golden/`; lane-check's nextest step needs it | the plan |
| P-5 | The "Pending" badge names what is pending (R0-F2) | WP-S2.09's area, design pass |
| P-6 | Check project's toast says what the last edit changed, not only the project's totals (R0-F3) | WP-S2.09's area |
| P-7 | Placing a device onto a wire shows its proposals and asks before connecting into the wire, or says so in the toast (R0-F4); ADR-0007's record in R1 | WP-S2.04's area, after R1's record |
| P-8 | The router's provisional routes avoid running over existing wires and the rectangular detour of a straight vertical connection (R0-F5) | the router |
| P-9 | A jump centres and marks its arrival, and Back has a visible control after a jump (R0-F6) | WP-S2.03's area |
| P-10 | The labels "102-A1" of L-1 and N-1 on folio 101 do not overlap (R0-F7) | Golden A's label placement |
| P-11 | `page.describe` names an interruption point's continuation (R0-F8) | the describe vocabulary |
| P-12 | Move takes a device selected by its tag as its representations on the page shown, as R and X do (R0-F10) | WP-S2.04's area |
| P-13 | The wheel's meaning (scroll or zoom) as a preference, the QET habit noted (R0-F11) | WP-S2.03's area |
| P-14 | The text protocol prints the result of a part choice and of a finished wire (R0-F14) | the describe vocabulary |
| P-15 | Inserting a terminal into -X1031 pushes its end bracket into -X1032 (EW-LAY-010), seen in T3's reference solution | the layout (WP-S4.02's area) |
| P-16 | A7 puts the moved lamp's connection point on the open end (no gap), so that it probes autoconnect onto coincident points | WP-S2.12, version 2 of the sheet, before R1 |

## 11. Token use

Not recorded in the run index; the orchestrator adds it.
