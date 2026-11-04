## What it checks

A cable core assigned to two or more physical wires. The pieces of one physical wire (both legs of an
interruption-point pair, the arm of a star) may share a core. One finding per core. An edit that assigns a core a
second time is refused; a double assignment in a file or an import is reported online.

## Why

A core carries one conductor. Two wires on one core is a wiring error, and the cable list shows the core twice.

## Standard

No standard: a project-hygiene rule.

## Example

Import a QElectroTech project in which two wires name the same core of one cable.

## How to fix

The assignment is on the connection's row of the property grid. There is no quick fix.

## Waiving

Do not waive it: one core cannot carry two wires.
