# Define a supply potential

This guide gives a drawn potential its name, its type and its voltage in the application window, so that its wires
take the colours of the project's profile and the reports print its name. A potential that you have only drawn is
known by its connection points and has a fallback name until you define it.

## What you need

- A project open for writing, with a schematic page shown.
- A wire, a junction or a terminal on the potential.

## Steps

1. Select one item on the potential:
   - a wire: the potential is defined at its start, a connection point or a junction;
   - a junction;
   - a terminal: the potential is defined at its first connection point.
2. Open **Define potential…** in one of these ways:
   - the **Draw** menu, under **Connect**, beside **Bond**;
   - a right-click on the item, then **Define potential…**;
   - the command palette (`Ctrl+K`): type `potential`, `supply` or `voltage`, then press Enter.
3. In the form, type the **Name** (`24VDC-1`). The **Type** and the **Voltage** may stay empty; type them as the
   pages print them: `L+`, `L-`, `L1`, `N`, `PE` for the type, and `24 V DC` or `230 V AC` for the voltage. `DC` or
   `AC` in the voltage also sets the current.
4. Press Enter. The potential is defined in one step: **Undo** takes it back.

When nothing, or more than one item, is selected, **Define potential…** is greyed out and its tooltip says what to
select. A name left empty is refused with "Name the potential first".

## Check the result

Open the **Potentials** navigator: the potential's row now carries the name you typed, with its type and voltage. In
the navigator's table view you can also change them later in the **Potential**, **Type** and **Voltage** cells.

The command behind the form is [`potential.define`](../reference/commands/potential.define.md).
