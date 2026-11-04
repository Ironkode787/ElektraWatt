## What it checks

Two definitions or member roles of one potential that disagree on type, voltage, environment or circuit class, and a
pass-through function (a contact, a fuse) that joins two potentials of different types. One finding per potential for
its attribute conflicts, naming the definitions and connection points in their printed form, and one per pass-through
pair. When 0 V and PE meet in one potential, the finding offers the quick fix "Declare 0 V–PE bond (PELV)"; without a
wire to carry the bond, its message points to Draw ▸ Bond.

## Why

A potential is one electrical node: everything on it has one type and one voltage. A conflict means two circuits that
must stay apart are joined, or a definition is wrong; both make the potential's colour, the reports and the safety
rules wrong.

## Standard

No standard: a project-hygiene rule.

## Example

In a copy of Golden A, replace the 0 V–PE bond B1 by a plain wire, so that 0V and PE become one potential.

## How to fix

When 0 V and PE meet, the quick fix "Declare 0 V–PE bond (PELV)" declares the bond on the wire that joins them;
without such a wire, use Draw ▸ Bond.

## Waiving

Waive it only when two definitions describe the same potential on purpose and the reports show the right value; a real
type conflict is a wiring error.
