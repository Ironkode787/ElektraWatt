## What it checks

A binding or an address that differs from the part's address map. Binding: a valve's solenoid bound to a channel other
than the one the module part's solenoid map assigns, with the quick fix "Rebind to the map's channel". Manual: a
channel whose manual address differs from the one EW derives. Both read the module's part, and the finding says when
the part's PLC data is not verified.

## Why

The part's address map is how the hardware is wired inside: a valve terminal connects each position's solenoids to
fixed bits. A binding against the map drives the wrong valve.

## Standard

No standard: a project-hygiene rule.

## Example

In a copy of the sample project, bind the solenoid of the valve -201K1-Q1 to another channel of the valve terminal
-201K1 than its part's map assigns.

## How to fix

The quick fix "Rebind to the map's channel" binds the map's channel to the coil and unbinds the other, as a preview
first.

## Waiving

Waive it only when the part's map is known to be wrong and cannot be corrected yet; name the source.
