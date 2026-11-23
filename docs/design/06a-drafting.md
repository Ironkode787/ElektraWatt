# Drafting: routing wires, arranging parts and settling texts on a schematic page

**Status:** Accepted · **Date:** 2026-11-19 · **Decided by** [ADR-0034](../adr/0034-schematic-drafting.md) ·
**Supersedes** the routing parts of [06 Schematic editor](06-schematic-editor.md) §5.4 and of
[ADR-0033](../adr/0033-schematic-routing-model.md)

**Purpose.** This document says how ElektraWatt lays out a schematic page: where wires and tubes run, where parts
stand when the person asks the program to arrange them, and where the texts of parts and wires stand. It replaces
the page router of ADR-0033 and the scene's text rules with one design. It has two grounds. The research is the note `docs/research/schematic-layout.md`; a rule taken from there is cited by its number (D-7, R-3). The second is the pilot's reference set: drawings that the pilot user named as neat and tidy. It is not in the
repository and nothing of it is reproduced here, but it was measured, and **where it and the research differ,
the reference set gives this program's defaults** (§1.3 marks those rules with an E).

**What Arrange and route is, and is not (owner, 2026-11-23).** The placer is one general way of putting a
page in order, and it is not perfect: a page it has arranged still needs the eye of a trained draughtsman, and
that is accepted. So it is not made to handle one more shape of page by a rule for that shape; a page it serves
badly is kept as a case (the backlog's section on the placer and the router) and is worked only as part of a
change that holds on every page. The program is not judged by whether a page comes out ready to sign, but by
whether what it offers is a fair start and never worse than what stood there. A model trained on laying out
drawings may one day do what rules do not; that is a thought for the future and no plan.

**Where each section lives.** The section numbers are the ones the code and the other documents cite.

| Section | File |
|---|---|
| [§1. What a good page is](06a-measure-model.md#1-what-a-good-page-is) | [what a good page is, and the model of a page](06a-measure-model.md) |
| [§2. The model of a page for layout](06a-measure-model.md#2-the-model-of-a-page-for-layout) | [what a good page is, and the model of a page](06a-measure-model.md) |
| [§3. Routing](06a-routing.md#3-routing) | [routing](06a-routing.md) |
| [§4. Placing](06a-placing.md#4-placing) | [placing](06a-placing.md) |
| [§5. Texts](06a-texts-window.md#5-texts) | [texts, placing by hand, the window and proof](06a-texts-window.md) |
| [§6. Placing by hand, helped](06a-texts-window.md#6-placing-by-hand-helped) | [texts, placing by hand, the window and proof](06a-texts-window.md) |
| [§7. The window](06a-texts-window.md#7-the-window) | [texts, placing by hand, the window and proof](06a-texts-window.md) |
| [§8. What goes](06a-texts-window.md#8-what-goes) | [texts, placing by hand, the window and proof](06a-texts-window.md) |
| [§9. Proof](06a-texts-window.md#9-proof) | [texts, placing by hand, the window and proof](06a-texts-window.md) |
| [§10. Libraries](06a-texts-window.md#10-libraries) | [texts, placing by hand, the window and proof](06a-texts-window.md) |
| [§11. Where this design is unsure](06a-texts-window.md#11-where-this-design-is-unsure) | [texts, placing by hand, the window and proof](06a-texts-window.md) |
| [Summary](06a-measure-model.md#summary) | [what a good page is, and the model of a page](06a-measure-model.md) |
