## What it checks

A process tag that does not match the project's process-tag scheme, and one process tag that two or more devices
carry. The scheme is a regular expression with named parts, set with project.settings.set.

## Why

A process tag names one field device in the plant; two devices with one tag, or a tag outside the scheme, break the instrument list.

## Standard

No standard: a project-hygiene rule.

## Example

Give a valve and the field device it drives the same process tag.

## How to fix

Correct the tag on the device's row, or keep it on the field device only. There is no quick fix.

## Waiving

Waive it for a device that the plant names outside the scheme, with the reason.
