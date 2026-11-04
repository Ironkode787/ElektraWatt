## What it checks

The positions of a valve terminal: a second valve at a position and a valve outside the module's positions are errors,
an empty position is a warning, and two modules at one position of their parent are an error. An edit that puts a
valve at a held position or outside the module is refused.

## Why

A valve terminal has a fixed number of positions, each holding one valve or blanking plate. A double or out-of-range
position cannot be built.

## Standard

No standard: a project-hygiene rule.

## Example

In a new project, place two valves at position 1 of one valve terminal in a file edit.

## How to fix

Positions are on the valve's row in the property grid. There is no quick fix.

## Waiving

Do not waive the errors; waive an empty position only when it is fitted on site.
