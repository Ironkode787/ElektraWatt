## What it checks

A drawn wire whose route passes through a connection point or a junction it does not join. One finding per wire and
point, shown on the page. A terminal's connection point is left to EW-TERM-006, and a wire without a route to
EW-CORE-073.

## Why

A line through a pin or a junction dot reads as connected to it. The model says otherwise, so the page, the wire
list and the potentials disagree.

## Standard

No standard: a project-hygiene rule.

## Example

Route a wire straight across a page so that it runs over the lower pin of a coil it is not drawn to.

## How to fix

Select the wire and use Route selection, or change its route with Edit route so that it keeps clear of the point.
There is no quick fix.

## Waiving

Waive it only when the drawing must keep the line where it is and a note on the page explains it.
