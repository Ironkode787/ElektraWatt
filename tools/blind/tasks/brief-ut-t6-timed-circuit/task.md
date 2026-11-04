# Task T6: draw the control supply

Your shop has set up its company standard in ElektraWatt, and a new, empty project made from it is open. The three
parts of a 24 V control supply are already in the project's parts: the power supply 2904602 (Phoenix Contact), the
miniature circuit breaker A9F78116 (Schneider Electric) and the electronic load distribution MICO 4.10
(Murrelektronik). You have no manual for this job.

Run `{run}` is a `{kind}` run with the caps {turns} turns, {steps} steps, {looks} looks and {refusals} refused steps.

## The job

From the company basic project, draw the folio 102 circuit and export a PDF.

- Folio 102 is a multi-line schematic page of the control supply.
- The power supply -102G1 (part 2904602) feeds the miniature circuit breaker -102F1 (part A9F78116): both + outputs of
  the supply go to input 1 of the breaker.
- The breaker's output 2 feeds both 24VDC inputs of the load distribution -102F2 (part MICO 4.10).
- Every device carries its part; the tags are the ones the project's numbering gives.
- Export the drawing as a PDF next to the project, and leave the project saved.

Use placeholder data only: no real names, addresses or drawing numbers.

## History

- Version 1: doc 16 §19.4's task T6 (NFR-UX-04) as a usability task of the round R1 for the QET personas (WP-S2.12),
  from the wizard's basic project with three golden part copies embedded in the prelude, as `gpp-101-103` embeds its
  parts (the parts come from `ew-sample-parts` once the picker offers them, ID-1332 O2). Run with `--no-book`.
