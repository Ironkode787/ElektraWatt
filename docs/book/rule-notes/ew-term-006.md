## What it checks

A terminal whose own symbol draws a connection point on the path of a wire or cable core of the page (its stored
route, else the straight path between its ends), while that wire does not join the terminal. One finding per terminal
and wire, shown on the page. The points a strip segment draws for its terminals are not read.

## Why

The page shows the terminal sitting in the wire, so a reader takes it as wired through. In the model the wire passes
by, so the terminal diagram, the wire list and the potentials say otherwise.

## Standard

No standard: a project-hygiene rule.

## Example

In a new project, draw a wire across a page, then place a terminal on the wire without splitting it.

## How to fix

Draw the wire to the terminal's connection points, or move the terminal with the move tool. There is no quick fix.

## Waiving

Waive it only when the overlap is a drawing choice and the terminal is meant to stay unconnected.
