## What it checks

A part copy used by a device whose description is empty or that names no manufacturer. One finding per device and part
copy. A missing connection-point pattern, the catalogue's third condition, is not checked.

## Why

The parts list prints the description and the manufacturer. Without them the list cannot be ordered from.

## Standard

No standard: a project-hygiene rule.

## Example

In a new project, use a part whose description is empty.

## How to fix

After the library item is complete, `library.update` refreshes the copy in the project. There is no quick fix.

## Waiving

Waive it for a part whose data the supplier completes on the order.
