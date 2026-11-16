## What it checks

A PLC module that is drawn on a page while none of its channels is bound to its pin. One finding per module, a
warning.

## Why

Its addresses stand at no pin, and a device wired to one of its pins is no channel's target: the PLC overview, the
PLC grid and the export to the PLC software then show the channels as spare although they are wired.

## Standard

No standard: a project-hygiene rule.

## Example

A project in which an earlier version drew a station and its modules from the rack dialog: that version drew the
boxes and left the channels unbound.

## How to fix

The quick fix **Bind the channels to their pins** binds every channel of the module to its pin. The addresses then stand at the
pins, and a wired device becomes its channel's target.

## Waiving

Do not waive it: the lists and the export would stay wrong.
