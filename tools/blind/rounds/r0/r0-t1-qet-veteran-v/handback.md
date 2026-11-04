1. Keep or drop
7 - placing a device by search and getting a folio-cell tag automatically is quicker than QET, and I would keep trying it.
2. Where it lost me
t0002: Insert device found nothing for part number A9F78116, although the catalogue has it (shown later in the Part field, t0007).
t0009: the folio shows a '7 Pending' badge after saving; I do not know what is pending.
3. What the tool is
An electrical schematic CAD for control cabinets, like QET but device-centred: you insert a device (with a part) instead of a bare symbol, and it tags it per folio and cell.
4. Broken
t0002 vs t0007: the part search of Insert device and the Part field disagree on what parts exist.
5. My design against the brief
Placed a 1-pole MCB -102F1 at 102-C5 between -102G1 and -102F2 with part A9F78116 (Schneider Electric), derived tag, saved. Not wired, as allowed. Ideally it would sit right on the G1-F2 path; I put it just left of it. About 2 minutes in QET; here 9 turns, comparable.
6. Flow chart
flowchart TD
  A[Folio 102 open] --> B[Device button]
  B --> C[Search A9F78116: nothing]
  C --> D[Search breaker: generic MCB]
  D --> E[Click on sheet: -102F1 at C5]
  E --> F[Part field: A9F78116 found]
  F --> G[Ctrl+S saved]
7. Good
Insert device search; status bar hints in place mode; toast 'Placed -102F1 at 102-C5' with Undo; automatic tag; part dropdown filter; clear Saved state.
8. What I could not read
t0005/t0009 region 755,400,130,40: breaker symbol and its part labels tiny and overlapping. t0000 title block text at 69 % unreadable.
