## What it checks

A frozen tag that differs from the tag the numbering scheme would give, reported at info as "frozen, deviates from
scheme" with the scheme's value. A tag that is not frozen is the scheme's own value, so it cannot deviate. One finding
per device; no tag text enters the fingerprint, so a renumber keeps a waiver.

## Why

A frozen tag keeps its text while everything else renumbers. That is often intended, for an imported or a customer
tag, but it should be visible.

## Standard

No standard: a project-hygiene rule.

## Example

On Golden A, ten devices keep frozen tags, among them S51, F55, T55, E55 and 1X1.

## How to fix

The frozen flag is on the device's tag row in the property grid. There is no quick fix.

## Waiving

Waive it when the frozen tag is intended; the finding is an info.
