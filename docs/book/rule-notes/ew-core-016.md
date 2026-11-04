## What it checks

Data from a newer format: a member, a variant or a file that this build does not know. It is kept unchanged and
written back as it was. Records of a reserved kind are kept and not reported. Information only.

## Why

A newer build of EW wrote data that this build cannot show. Keeping it unchanged lets the newer build read it back,
but outputs that would use it may be incomplete.

## Standard

No standard: a project-hygiene rule.

## Example

To see it, add a member `"x_future": 1` to one line of `project.jsonl` in a copy of Golden A and open the copy.

## How to fix

Open the project in the newer build of EW that wrote it, or upgrade EW. There is no quick fix.

## Waiving

An integrity finding cannot be waived. The data is not wrong: open the project in a build that knows it.
