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

A relay -101K1 whose tag is frozen as K1 reports "K1: frozen, deviates from scheme (rule-derived value 101K1)". A tag
the program gives (Enclosure…'s -UC1 and its plate -UC1-U1) is the scheme's own and is not frozen.

## How to fix

The frozen flag is on the device's tag row in the property grid. There is no quick fix.

## Waiving

Waive it when the frozen tag is intended; the finding is an info.
