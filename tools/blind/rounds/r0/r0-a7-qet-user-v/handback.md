1. Keep or drop
5 - Moving and wiring went fast once I found my way, but the wheel zoom, the unexplained 'Pending' and the odd wire shape make me doubt it.
2. Where it lost me
- t0001: '3 Pending' badge on the lamp, no idea what was pending.
- t0002: clicking it opened messages about a tag scheme and zoomed the sheet out on its own.
- t0004: plain mouse wheel zoomed out to 18% instead of scrolling.
- t0005/t0006: could not tell if S56:14 to lamp was already wired before I drew my wire.
3. What the tool is
A schematic CAD for control cabinets, like QET with folios and a cell grid, but with built-in checks, a devices list and a properties panel that tells you which cell a device sits in.
4. Broken
- t0006: my wire from S56:14 to E56:X1 came out as a box-shaped jog beside a straight line, looks like a double wire.
- t0002: sheet zoom changed to 35% when I opened messages.
5. My design against the brief
Lamp E56 dragged back to 101-E14 under S56 (status bar confirmed), wire drawn from S56:14 to X1, X2 wire kept, project saved. Still to do: make sure there is no doubled wire and straighten the route. In QET about 1 minute; here about 7 turns, a few minutes.
6. Flow chart
flowchart TD
  A[Open folio 101] --> B[Drag lamp left two columns - smooth]
  B --> C[3 Pending badge - confused]
  C --> D[Messages panel, sheet zoomed out - detour]
  D --> E[Wheel zooms instead of scroll - detour]
  E --> F[Draw wire S56:14 to X1]
  F --> G[Odd jog in wire - unsure]
  G --> H[Ctrl+S saved]
7. Good
Drag to move worked first time and the status bar showed the cell 101-E14 straight away. QET grid and folio cells feel familiar. Ctrl+S and the Saved indicator are clear. X2 wire followed the lamp.
8. What I could not read
- t0000/t0001: terminal numbers (13, 14, X1, X2) on S56 and E56 are tiny at 69%, region 1200,470,140,90.
- t0006: at 88% the pin numbers next to the lamp are still too small, region 1040,540,40,120.
