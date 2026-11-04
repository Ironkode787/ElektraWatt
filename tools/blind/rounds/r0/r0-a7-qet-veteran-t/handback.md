1. Keep or drop
6 - Once I found that Move follows the grid cursor, the fix took a few steps, but finding that cost me three turns.
2. Where it lost me
- select.set E56 then tool.move: "Nothing movable is selected"; the tag selects the device, not its symbol.
- Pressing the lamp centre selected the X2 wire (101.16) instead of the lamp.
- A press/release drag in the Move tool left the ghost at dx 0; only cursor.set plus Enter moved it.
3. What the tool is
An ECAD program for control cabinets with folios, cells, tags and terminals like QET. You work it by keyboard: put a grid cursor on a point and press Enter. Snap labels name the terminal you are on.
4. Broken
Nothing was marked broken. Two Move refusals happened after selecting by tag and after hitting the wire.
5. The drawing set against the design language
Folio 101 reads like QET: cells such as 101-E14 and terminal tags such as S56:14. Wire potentials (N-55S) came across by themselves. The new straight wire is stored with 6 bends, and the X2 wire is now listed at 242/92 mm, which might cross S56. A QET veteran would do this in about 30 seconds with a drag and a wire. It took me 6 turns.
6. Flow chart
flowchart TD
A[Read page 101] --> B[select.set E56]
B --> C{Move refused}
C --> D[Click lamp centre: got X2 wire]
D --> E[Click body offset: lamp selected]
E --> F[Move, drag gives dx 0]
F --> G[cursor.set + Enter: moved to E14]
G --> H[Wire tool S56:14 to E56:X1]
H --> I[Save]
7. Good
The page text gives exact positions and wire counts for every terminal. The snap labels and the moving-ghost readout of steps were helpful. The X2 wire followed the lamp, and Save was clean.
