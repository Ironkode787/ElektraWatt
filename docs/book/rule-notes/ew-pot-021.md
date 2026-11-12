## What it checks

A load (a two-terminal function such as a coil or a lamp) fed on one side whose other side sits on a potential with no
source: no supply reaches it, its type is not PE or FE, no connection point on it has a potential role and everything
on it besides terminals and interruption points is such a load. Each load is reported once. A load with no source on
either side is left out, and an unconnected pin is EW-POT-009's. A black box (an imported element whose function the
importer could not read, such as a two-pin supply from QElectroTech) is neither a load nor a source: a potential that
holds one is not judged. Loads in series are returned through each other: the net between two coils in series from
+ to − is not reported, since its loads lead to two different potentials with a source.

## Why

A coil whose return is never wired to 0 V or N cannot work. The flow of potentials shows this as a load with an open
return, which is easy to miss on a busy page.

## Standard

No standard: a project-hygiene rule.

## Example

Golden A has none. In a new project, wire two valve coils' A1 to a PLC output and join their A2 pins to each other but
not to 0 V.

## How to fix

Follow the load's return side in the potentials navigator. There is no quick fix.

## Waiving

Waive it only when the return is made outside the project and an interruption point cannot show it.
