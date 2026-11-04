## What it checks

A rule that could not be evaluated. The shipped rules use no scripts, SQL or formulas, so the cause is a defect in the
rule: the engine catches it, and the finding replaces that rule's findings in that scope, while every other rule's
findings stay. In prevent mode such a rule refuses the edit, since it could not be checked.

## Why

A rule that did not run reports nothing, which looks like a clean result. The finding makes the gap visible.

## Standard

No standard: a project-hygiene rule.

## Example

No project shows it by design: it appears only when a rule has a defect.

## How to fix

Nothing in the project causes it. There is no quick fix.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
