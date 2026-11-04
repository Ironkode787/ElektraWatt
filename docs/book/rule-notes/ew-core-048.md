## What it checks

An assembly whose parts list uses the assembly's order codes, where those order codes do not cover each child with
parts exactly once. Only an order code with a configured part covers; covers are not transitive. One finding per child
that is not covered or is covered twice. A cover outside the assembly is not judged here.

## Why

When an assembly is ordered by its order codes, its children are not ordered separately. A child no code covers is
never ordered; a child two codes cover is ordered twice.

## Standard

No standard: a project-hygiene rule.

## Example

In a new project, give an assembly the order-code basis and leave one child with a part out of every order code.

## How to fix

The order codes are in the assembly's properties. There is no quick fix.

## Waiving

Waive it when the child is ordered by other means and the parts list notes it.
