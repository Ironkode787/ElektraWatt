## What it checks

References to records the project does not hold. The registered rule, run by `ew check` and the release gate, reports
three cases: a page whose frame defaults name a form copy the project does not hold (discriminator `form`); a PLC
channel whose module is no longer a PLC module (`channel`); and a channel that shares its number with another channel
of its module (`duplicate_number`). The other dangling references are found when the project is opened.

## Why

A reference to nothing makes a page print with the wrong frame, or leaves channels that no module owns. Every output
that follows the reference is then wrong or incomplete.

## Standard

No standard: a project-hygiene rule.

## Example

To see it, open a project file whose frame defaults name a form copy that is not in the project, for example after
deleting the form copy's record by hand.

## How to fix

Embed the missing form again from the library, or change the frame defaults in the project's settings. A duplicate
channel number has no quick fix yet: delete the extra channel record.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
