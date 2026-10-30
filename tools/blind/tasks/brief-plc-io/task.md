# Brief: three door switches and three solenoid valves on a small PLC

A customer is adding three pneumatic functions to a machine, controlled by a small PLC in the cabinet. Each function is
a 24 V DC solenoid valve out on the machine. An operator asks for each one with its own selector switch on the cabinet
door; the PLC reads the three switches on a digital input card and switches the three valves from a digital output
card. You are asked to draw the circuit in the new project that is already open, and to hand over the drawing.

Run `{run}` is a `{kind}` run with the caps {turns} turns, {steps} steps, {looks} looks and {refusals} refused steps.

## The situation

- The cabinet is fed with 230 V AC (line, neutral and protective earth) on terminals of a terminal strip.
- The 24 V DC for the controls comes from a power supply unit in the cabinet, fed from the 230 V. Its part is among
  the program's generic starter parts.
- The PLC is a CPU with one digital input card and one digital output card, each with eight channels. Their parts are
  among the program's generic starter parts as well.
- The three valves sit on the machine, outside the cabinet; each has a coil with two connections. Each coil draws
  well under the output card's 0.5 A per channel, so the output card switches the valves directly, without relays in
  between. This is the customer's choice; draw it that way.
- The three selector switches sit on the cabinet door, which is wired to the cabinet through terminals as well.

## What the circuit must do

- The 230 V side of the power supply is protected by a circuit breaker; the 24 V side by a fuse.
- Each selector switch, when on, applies the fused 24 V to its own input channel of the input card.
- Each valve is switched by its own output channel of the output card; the valve's other end goes to 0 V.
- The cards' 0 V connections go to 0 V; the output card's 24 V supply comes from the fused 24 V.
- Every channel used shows its PLC address on the drawing.
- Everything that leaves the cabinet, to the door or to the machine, goes through terminals of the strip.

## What to hand over

- The project, saved.
- A PDF of the drawing, exported by you.
- A parts list as far as the program gives one; the PLC's cards must be in it.

Use placeholder data only: no real names, addresses or drawing numbers. How you lay out the pages, which tags you
give, which channels you use and how many pages you use is up to you, as you would do it for a customer.

## History

- Version 1: the PLC variant of `brief-relays-valves` (grade 3; WP-S2.18 part 4, FT-593).
