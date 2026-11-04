## What it checks

A waiver whose fingerprint matches no finding. It is reported only when the waiver's rule ran in the scheme and mode;
a waiver whose rule did not run is reported as not evaluated. One finding per waiver.

## Why

A waiver that matches nothing is a leftover: the finding it accepted was fixed, or its target changed. Left in place
it could hide a later finding.

## Standard

No standard: a project-hygiene rule.

## Example

In a copy of Golden A, assign a part to -XPE: its waiver W-1 then matches nothing.

## How to fix

Waivers are listed in the Messages pane under their rule. There is no quick fix.

## Waiving

Waiving a waiver's finding makes no sense; remove or correct the waiver.
