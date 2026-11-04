## What it checks

A placement whose part is not one of its device's parts, for example after the part was removed or replaced. A
placement without a part realises the device's main part and is never reported.

## Why

The placement's footprint comes from its part. A part the device no longer uses gives a footprint that does not match
what is built.

## Standard

No standard: a project-hygiene rule.

## Example

In a new project, place a device with its part, then remove the part from the device.

## How to fix

Remove the placement and place the device again from the device navigator. There is no quick fix.

## Waiving

Do not waive it: the footprint is wrong.
