# Brief: three solenoid valves, three relays, three door switches

A customer is adding three pneumatic functions to a machine. Each is a 24 V DC solenoid valve out on the machine. An
operator switches each one on and off with its own selector switch on the cabinet door; each switch commands a relay
in the cabinet, and the relay's contact switches the valve. You are asked to draw the circuit in the new project that
is already open, and to hand over the drawing.

Run `{run}` is a `{kind}` run with the caps {turns} turns, {steps} steps, {looks} looks and {refusals} refused steps.

## The situation

- The cabinet is fed with 230 V AC (line, neutral and protective earth) on terminals of a terminal strip.
- The 24 V DC for the controls comes from a power supply unit in the cabinet, fed from the 230 V. Its part is already
  in the project's parts.
- The three valves sit on the machine, outside the cabinet; each has a coil with two connections.
- The three selector switches sit on the cabinet door, which is wired to the cabinet through terminals as well.

## What the circuit must do

- The 230 V side of the power supply is protected by a circuit breaker; the 24 V side by a fuse.
- Each selector switch, when on, energises its own relay's coil from the 24 V; the coil's other end goes to 0 V.
- Each relay has a normally open contact that switches 24 V to its own valve; the valve's other end goes to 0 V.
- Everything that leaves the cabinet, to the door or to the machine, goes through terminals of the strip.

## What to hand over

- The project, saved.
- A PDF of the drawing, exported by you.
- A parts list as far as the program gives one.

Use placeholder data only: no real names, addresses or drawing numbers. How you lay out the pages, which tags you
give and how many pages you use is up to you, as you would do it for a customer.

## History

- Version 1: the owner's example brief (grade 2).
