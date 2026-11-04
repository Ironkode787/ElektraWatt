## What it checks

The project pins a derivation profile that this build does not ship, from a newer build or a hand edit. The pin is
kept; the newest shipped profile is used for display only, and the commands that depend on the profile are
refused.

## Why

The profile decides how tags, wire numbers and cross-references are derived. Deriving with another one would
change them without your asking.

## Standard

No standard: a project-hygiene rule.

## Example

To see it, change `derivation_profile` in `project.jsonl` of a copy of Golden A to a version that does not exist,
such as `"2099.1"`, and open the copy.

## How to fix

Open the project in a build that ships the profile. There is no quick fix.

## Waiving

An integrity finding cannot be waived: the stored project itself is wrong, and it is repaired, never accepted.
