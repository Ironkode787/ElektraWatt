## What it checks

A connection of a placed device that has no wire. Left out: interruption points, terminal connection points, unplaced
functions, non-physical and optional connection points, and connection points of another discipline (fluid ports are
EW-FL-001's). A connection joined by a bridge, a rail or a declared link counts as connected. A connection marked not
used is reported at info. Each function is checked on the first page it is drawn on.

## Why

An open pin is often a wire that was forgotten. Marking the pin unused records that it is open on purpose, which a
reviewer can then see.

## Standard

No standard: a project-hygiene rule.

## Example

Golden A's -102F2:13 and :14, the signal contact of the electronic breaker, are open.

## How to fix

Open the message: its page opens with the device selected. Draw a wire to the connection, or right-click it and choose
Mark not used when it stays free on purpose. There is no quick fix.

## Waiving

Prefer marking the pin unused over a waiver; waive it only where the pin cannot be marked, and say why.
