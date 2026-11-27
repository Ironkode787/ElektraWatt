# Route the cabinet's wires and give them their lengths

Route lays each wire of the cabinet through the ducts of the mounting plate and gives it a length: its way, a reserve
at each end and, into the door, a loop across the hinge, rounded up to what the shop cuts to. The wire list, the cut
list and the consumables read that length.

You need the cabinet's panel layout page with its devices placed and its ducts drawn
([Draw a panel layout](panel-layout.md)), and the devices of the door in the door
([Place devices in the door](cabinet-surfaces.md)). The sample project's plate is page 902.

## Route the cabinet

1. With every device placed, the plate's HUD names the next step: "Next: Route gives each wire its way through the
   ducts and its length", with the button **Route**. Route is also **Draw ▸ Connect ▸ Route**: on the plate's page it
   routes the cabinet; on a schematic page the same button routes the drawing. With nothing selected it plans the
   wires that have no way and those whose way no longer holds; with devices, ducts or wires selected, the wires at
   them.
2. Nothing changes yet. The page shows the whole cabinet with each planned way, the wires to the door as one arrow to
   the hinge side ("… wires to the door") and on each duct a chip with its fill. The card says, under its lead, such
   as "Route: cabinet +CAB. 41 wires get a way through the ducts; 3 are made on site. Nothing has changed yet.":
   - **Ducts**: each duct with its wires, its fullest piece and the fill limit ("limit 60 %");
   - **Lengths**: each wire with its ends and its length;
   - **Typed by hand**: each wire that keeps the length typed for it, such as 101.1 with "keeps 400 mm; Route gives
     …";
   - **Made on site**: each wire with an end outside the cabinet, in the field or in another cabinet, such as "its
     end … is in +FLD";
   - **Without a way**: each wire Route cannot lay, with what to do, such as "-102S1 is not placed: place it, then
     route again" or "… meets no duct: draw a duct above or below …, or type the length". An unfinished wire stands
     here too, with its reason: "its end at the interruption point … is open: give the point its partner, or type the
     length", or "its end … is on a device without a location: give the device its location, then route again";
   - the tick **Route every wire again**, which plans the wires whose ways hold as well.

   Point at a row, or select it, to see that wire's way alone. Where the cabinet holds stored ways of wires it no
   longer lays (a wire deleted, or no longer the cabinet's), the card names them before you apply: "… ways of wires
   +CAB no longer lays are removed: …".
3. Choose **Apply**: every way is stored in one step of Undo, and the toast says "Routed 41 wires of +CAB.".
   **Cancel** leaves the project as it was. On a cabinet whose every wire has its way, Route says "Every wire of +CAB
   has its way." and its card plans them all again.

## See a wire's way and its length

4. Show **View ▸ Panels ▸ Navigators ▸ Connections** and choose its **Table** button. The Length column holds each
   wire's length, and Length from says where it comes from: routed, typed, cable (a cable's core, cut with its cable),
   stale, missing, or site.
5. Select a wire's row: the plate's page shows its way, a line from pin to pin with a chip of its number and length,
   and for a wire into the door, such as 104.3, "to -104E1 in the door".
6. The wire's Properties show **Length** with the words of where it comes from, such as "Routed: … of way, 50 mm
   reserve at each end and a 300 mm door loop, rounded up to …", and **Way**: the ducts it runs along, "door" where it
   crosses into the door, or "direct" over the fronts, with **Show on 902**.

## The duct fill

7. Select a duct: its Properties show its **Fill**, such as "8 % where fullest · the limit is 60 %", with "with
   default diameters (unverified)" where the wires' parts give no outer diameter, and its **Wires** with **Select
   them**.
8. Route lays a wire another way where a duct is full, if there is one. A duct over the limit is reported by **Check ▸
   Check project** (EW-LAY-008), such as "Duct 1 (40×60) is 72 % full where it is fullest; the limit is 60 %: widen it
   or add a duct".

## The reserve, the door loop and the fill limit

9. Choose **Output ▸ Workshop ▸ Workshop settings**. Among the Workshop rows are **Reserve per end** (50 mm when
   empty), **Door loop** (300 mm), **Duct fill limit** (60 %) and **Cut to** (5 mm), each one step of Undo. A change
   takes effect at once: every routed length and every fill follows it, and you need not route again.

## A wire made on site, and a length typed by hand

10. A wire with an end outside the cabinet, such as 104.4 to the sensor +FLD-104B1, or a wire between two cabinets,
    is made on site: Route gives it no way, the wire list prints "—" with Length from "site", and the fitter makes the
    end in this cabinet.
11. To give a wire a length of your own, type it into its Length cell in the Connections table, or into the Length
    row of its Properties, in millimetres or with its unit ("2 m"). Length from then says "typed". Route still lays
    the wire, so that its duct counts it, and its Properties say "Typed by hand. Route gives it …: clear the length
    to take it." Clear the cell to take Route's length.

## After a device moves

12. Move or turn a device on the plate, or change a duct: the ways of its wires no longer hold. The toast says so,
    such as "Moved -X1031. The ways of 5 wires no longer hold.", with **Undo** and **Route**; the HUD says "The ways of
    5 wires no longer hold: Route gives them new ones"; and Check project reports it (EW-LAY-011, an error when you
    release).
13. A way that no longer holds is **stale**. Its wire keeps its length, which the lists print after "≈", and Length
    from says "stale". Choose **Route**: with nothing selected it plans only those wires. **Apply** makes their lengths
    fresh again.

The rules are in the [rule reference](../reference/rules/ew-layout.md): EW-LAY-008, EW-LAY-011 and, for the wires
without a length, [EW-MFG-001](../reference/rules/ew-mfg-001.md).
