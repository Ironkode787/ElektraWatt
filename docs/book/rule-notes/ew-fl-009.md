## What it checks

A valve whose part gives a coil voltage that differs from the voltage of a potential feeding the coil: at a coil with
pins, the potentials at its pins; at an internal coil of a valve terminal, the potential at the load supply pin of the
module of the channel bound to it, or of the first device above that module with such a pin. One finding per valve.

## Why

A 230 V coil on a 24 V output does not switch; a 24 V coil on 230 V burns.

## Standard

No standard: a project-hygiene rule.

## Example

A valve part with a 230 V coil whose channel's module takes its valve supply from a 24 V DC potential.

## How to fix

Choose a valve with the right coil, or feed it from a potential of that voltage. There is no quick fix.

## Waiving

Waive it when the part data give the wrong voltage and the coil is known to match.
