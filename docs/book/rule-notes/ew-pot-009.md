## What it checks

An electrical connection point of a placed function with no connection. Left out: interruption points, terminal
connection points, unplaced functions, non-physical and optional connection points, and connection points of another
discipline (fluid ports are EW-FL-001's). A connection point joined by a bridge, a rail or a declared link counts as
connected. A point marked unused is reported at info. Each function is checked on the first page it is drawn on.

## Why

An open pin is often a wire that was forgotten. Marking the pin unused records that it is open on purpose, which a
reviewer can then see.

## Standard

No standard: a project-hygiene rule.

## Example

Golden A's -102F2:13 and :14, the signal contact of the electronic breaker, are open.

## How to fix

The unused mark is on the connection point's row in the property grid. There is no quick fix.

## Waiving

Prefer marking the pin unused over a waiver; waive it only where the pin cannot be marked, and say why.
