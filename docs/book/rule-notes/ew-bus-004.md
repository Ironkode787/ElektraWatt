## What it checks

Over each segment of a physical bus net: at each end the nearest port with a termination switch must be On, and every
other one Off. Reported: an end that is Off, a switch inside the segment that is On, an end without any termination
left, and, at info, an external end whose partner is outside the project, and a switch whose state is of a newer
format.

## Why

A bus segment needs termination at exactly its two ends. Missing or extra termination causes reflections and
communication errors that are hard to find on site.

## Standard

No standard: a project-hygiene rule.

## Example

In a copy of the sample project, switch off the termination of the valve terminal -201K1's port, an end of the PROFIBUS
segment on page 105.

## How to fix

Set each port's termination switch in the property grid. There is no quick fix.

## Waiving

Waive the info for an external end once the partner confirms its termination.
