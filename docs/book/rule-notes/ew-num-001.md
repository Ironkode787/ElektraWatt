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

On the sample, the enclosure -UC1 and its mounting plate keep the frozen tags that Enclosure… gave them: these are its
two infos.

## How to fix

The frozen flag is on the device's tag row in the property grid. There is no quick fix.

## Waiving

Waive it when the frozen tag is intended; the finding is an info.
