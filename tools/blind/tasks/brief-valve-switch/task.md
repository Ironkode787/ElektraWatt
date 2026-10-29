# Brief: a solenoid valve switched from the cabinet door

A customer wants a small extension to a machine: one 24 V DC solenoid valve out in the field that an operator opens
and closes with a switch on the cabinet door. You are asked to draw the circuit in the new project that is already
open, and to hand over the drawing.

Run `{run}` is a `{kind}` run with the caps {turns} turns, {steps} steps, {looks} looks and {refusals} refused steps.

## The situation

- The cabinet already has a 24 V DC control supply. Its +24 V and its 0 V come into your circuit on two terminals of a
  terminal strip in the cabinet; you do not draw the supply itself.
- The solenoid valve sits on the machine, outside the cabinet. Its coil has two connections.
- The switch is a simple on/off switch mounted on the cabinet door.

## What the circuit must do

- With the door switch on, the valve's coil gets 24 V; with the switch off, it gets none.
- The circuit is protected by a fuse right after the incoming +24 V.
- Everything that leaves the cabinet for the field goes through terminals of the strip: the valve is never wired
  straight to the switch or the fuse.

## What to hand over

- The project, saved.
- A PDF of the drawing, exported by you.

Use placeholder data only: no real names, addresses or drawing numbers. How you lay out the page, which tags you give
and how many pages you use is up to you, as you would do it for a customer.

## History

- Version 1: the calibration brief of the first visual round (grade 1).
- Version 2: the reference solution takes the fuse, the switch and the valve from the generic starter parts, so the
  parts item is no longer infeasible (WP-S2.18 D8, FT-592).
