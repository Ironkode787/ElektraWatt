## What it checks

Over the PROFINET ports of a logical net: a device name that is not DNS-compatible, a device name or an IP address that
two ports share, and an IP address outside the subnet of the net's master.

## Why

PROFINET finds a device by its name and talks to it by its IP address; neither may be shared, and every device needs the master's subnet.

## Standard

- IEC 61158-6-10 (unverified)

## Example

Give two stations the address 192.168.0.10, or name a device io_device_1.

## How to fix

Correct the name or the address on the port's row. There is no quick fix.

## Waiving

Do not waive it: the network does not start.
