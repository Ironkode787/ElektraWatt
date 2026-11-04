## What it checks

A terminal with rail bonding set that has no PE connection point. One finding per terminal; online only.

## Why

Rail bonding means the terminal's PE contact is clamped to the mounting rail, which joins it to the location's
protective earth. A terminal without a PE connection point cannot do that, so the setting is wrong and the PE
potentials it implies are wrong too.

## Standard

No standard: a project-hygiene rule.

## Example

In a copy of Golden A, set rail bonding on a feed-through terminal of -X1031.

## How to fix

Rail bonding is on the terminal's row in the strip editor. There is no quick fix.

## Waiving

Do not waive it; the setting has no meaning on a terminal without a PE connection point.
