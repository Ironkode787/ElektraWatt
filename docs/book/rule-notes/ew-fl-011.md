## What it checks

A valve whose Drives link names a device other than the one its working-port lines reach, when the lines do reach a
device. One finding per valve; online only.

## Why

The Drives link says which cylinder or actuator the valve moves. When the lines go elsewhere, the drawing and the link
disagree.

## Standard

No standard: a project-hygiene rule.

## Example

In a new project, link a valve to cylinder 1A1 while its lines run to 2A1.

## How to fix

The Drives link is on the valve's row in the property grid. There is no quick fix.

## Waiving

Waive it when the lines reach the actuator through a device EW does not follow.
