# Task A7: put a moved lamp back

You are working on an existing project for a control cabinet, which is already open on folio 101, the main supply.
Someone has pushed the cabinet lamp E56 out of its place: it now stands two columns to the right of where it belongs,
and the wire from the door position switch S56 to the lamp was lost on the way.

Run `{run}` is a `{kind}` run with the caps {turns} turns, {steps} steps, {looks} looks and {refusals} refused steps.

## The job

Put the lamp E56 back in its place and connect it as it was.

- Its place is cell E14 of folio 101, directly below the position switch S56.
- Its upper connection X1 was wired to S56:14; its lower connection X2 is still wired and must stay so.
- Do it the way you would in your own work; then say in your notes how you expected the program to treat the lamp's
  connections when you moved it.

Leave the project saved when you are done.

## History

- Version 1: the ADR-0007 probe A7 ("Validation before M2", autoconnect on move) as a usability task of the rounds R0
  and R1 (WP-S2.12): on Golden A, the prelude deletes the wire S56:14 to E56:X1 and moves E56 twelve grid steps to the
  right.
