## What it checks

A used channel that has no address: its address or its module's start is excluded, or no address can be derived, for
example a module without a station or configuration project. A TBD address is EW-PLC-015's. One finding per channel, a
warning; the error in export mode waits for the PLC export's check.

## Why

A used channel without an address cannot be programmed or exported.

## Standard

No standard: a project-hygiene rule.

## Example

In a new project, wire a channel of a module that belongs to no configuration project.

## How to fix

Place the module in a station of a configuration project to derive addresses. There is no quick fix.

## Waiving

Waive it for a channel that is wired for later use and is not programmed yet.
