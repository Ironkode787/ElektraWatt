## What it checks

A committed computation that a release output uses and that is stale: its inputs changed since it was computed. It
runs only in the gate. At this release no committed computation exists yet, so it reports nothing.

## Why

A stored result, such as a routed length, must match the project it is released with.

## Standard

No standard: a project-hygiene rule.

## Example

No project shows it yet: committed computations arrive with later releases.

## How to fix

Nothing to recompute at this release. There is no quick fix.

## Waiving

Release-gate findings cannot be waived.
