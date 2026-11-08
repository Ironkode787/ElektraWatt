# Brief RP2: a 24 V control circuit with a self-holding relay

A customer wants a small 24 V control circuit: a start pushbutton switches a relay on, and the relay holds itself on
with one of its own contacts; a lamp shows that the relay is on, and a second pushbutton tests the lamp. You are asked
to draw it in the new project that is already open, and to hand over the drawing.

Run `{run}` is a `{kind}` run with the caps {turns} turns, {steps} steps, {looks} looks and {refusals} refused steps.

## The situation

- The cabinet's mains arrive on a terminal strip: line L, neutral N and protective earth PE.
- The 24 V DC comes from a power supply unit in the cabinet, fed from the mains. Its part, the relay's, the
  pushbuttons' and the lamp's are among the program's generic starter parts.

## What to draw

- Folio 101: the mains terminals and the power supply unit fed from them. The supply's 24 V and 0 V outputs continue
  to the control page through interruption points.
- Folio 102, the control page: 24 V and 0 V arrive at interruption points. The start pushbutton energises the relay's
  coil from 24 V; the relay's normally open contact, wired across the start pushbutton, holds it on; the coil's other
  end goes to 0 V. The lamp lights through another contact of the relay, and the second pushbutton lights it
  directly; the lamp's other end goes to 0 V.
- Let the program draw the routes of the wires (Route page), and look at what it drew: say in your notes where a wire
  runs as you would draw it and where it does not.

## What to hand over

- The project, saved.
- A PDF of the drawing, exported by you.

Use placeholder data only: no real names, addresses or drawing numbers. Which tags you give is up to you, as you would
do it for a customer.

## History

- Version 1: the third candidate's persona round (WP-S2.19 §11.3, RP2).
