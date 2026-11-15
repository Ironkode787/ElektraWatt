## What it checks

A device that sits in the PLC as a module while nothing of it describes one: its part has no PLC data (a bus node
typed as a part with a bus port only), or it is a copy of a station that kept the station's own module. One finding
per device, a warning.

## Why

Its channels come from no part, and an export to the PLC software would list a module that does not exist.

## Standard

No standard: a project-hygiene rule.

## Example

Attach a device whose part has a bus port and no PLC data to a CPU, as an older version allowed.

## How to fix

The quick fix **Take it out of the rack** takes the device out of its station and removes the channels no part
gives. A bus device then joins a PLC as a station of its own: right-click it and choose **Add to PLC**.

## Waiving

Do not waive it: the module would be exported as it stands.
