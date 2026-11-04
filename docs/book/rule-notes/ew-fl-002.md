## What it checks

The channels bound to a valve's coils against the solenoids its part declares: a solenoid coil bound by more than one
channel, and a channel bound to a coil the part does not declare, are errors; on a used valve, a declared solenoid
that no channel binds is a warning. Spare, reserved and not-fitted valves give no warning.

## Why

Each solenoid is driven by one output. An extra or missing binding means a valve that never switches, or two outputs
fighting over one coil.

## Standard

No standard: a project-hygiene rule.

## Example

In a new project, bind two PLC outputs to the one solenoid of a monostable valve.

## How to fix

A binding is changed with `plc.bind_channel`. There is no quick fix.

## Waiving

Waive the warning for a solenoid that is driven by hand or by another system.
