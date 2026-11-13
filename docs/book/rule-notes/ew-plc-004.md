## What it checks

An output channel (DO, AO) that drives a contact, or an input channel (DI, AI, counter) that reads a coil or a valve
solenoid. The connected function is the channel's target: its manual target, the coil it is bound to, or the function
its pin's signal reaches through wires and terminals.

## Why

A proximity sensor on an output, or a coil on an input, does not work and can damage the card.

## Standard

No standard: a wiring-consistency rule.

## Example

Wire a DO channel's pin through a terminal to a proximity switch.

## How to fix

Rewire the channel, or change its kind on the channel's row. There is no quick fix.

## Waiving

Waive it only for a deliberate use, such as an output that feeds a sensor supply, with the reason.
