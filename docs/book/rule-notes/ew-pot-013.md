## What it checks

A contact or other opening pass-through that opens the neutral (N or PEN) without opening the associated line
conductor. A device whose definition switches N together with L, and that also opens a line pair, is accepted; a
neutral-isolating terminal is reported at info. The finding names the loads that stay connected to the line. One
finding per opening pair; there is no quick fix.

## Why

Opening the neutral alone leaves the load connected to the line: it looks switched off but is live. Anyone who works
on it trusts a dead load that is not dead.

## Standard

- IEC 60364-4-43:2008, 431.3 (unverified)

## Example

Golden A's thermostat T55 and switch S56 open N-55 while E55 and E56 stay on L-55.

## How to fix

The correct wiring is a decision for the designer, so there is no quick fix.

## Waiving

This is a safety rule. Do not waive it; correct the wiring.
