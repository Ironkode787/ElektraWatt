1. Keep or drop
6 - placing a breaker with an automatic tag and wire was quick, but getting the real part on it took a second hunt that QET users would not expect.
2. Where it lost me
- Device from part picker: filter A9F78116 and A9F gave rows 0; the project part is not in that picker, only generic EW-GEN-MCB parts.
- look canvas does not list picker rows until the filter is narrow, so I could not browse the 34 rows.
- Finding how to set the part: part.choose / part.search are hidden actions I found only by searching the help; I needed hint 2.
- part.choose, part.search, part.pick print bare "done" with no change, so I had to look each time.
- The place ghost kept floating after placing; I ended it with the Select tool.
3. What the tool is
A desktop ECAD program for control cabinets, close to QET in folio/cell layout, but it keeps devices, parts, potentials and numbering in a model: tags come from the numbering scheme, wires get proposed as you place, and lists like the parts list are generated.
4. Broken
Nothing broke or was refused. The only failure was the device picker finding nothing for the part number A9F78116.
5. The drawing set against the design language
Folio 102 now reads -102G1 (B6), -102F1 1P MCB (C7), -102F2 (E7): a supply, a breaker, then the loads, in the usual top-to-bottom order with QET grid cells and the folio-based tag -102F1. Pin 1 is on +24V; pin 2 is open, as the task allowed. A QET veteran would need about 1-2 minutes (drag the breaker, type the tag, fill the reference). It took me 7 turns plus 2 hints, roughly 10 minutes, mostly spent finding where the part goes.
6. Flow chart
`mermaid
flowchart TD
  A[Look at folio 102] --> B[Device from part picker]
  B --> C{Filter A9F78116}
  C -->|0 rows| D[Stuck: try A9F, breaker]
  D --> E[Hints 1 and 2]
  E --> F[Choose generic 1P MCB, click C7]
  F --> G[Tag -102F1 and wire proposed automatically - smooth]
  G --> H[Hunt the help for part.choose]
  H --> I[Search A9F78116 in the Choose part dialog - found]
  I --> J[Pick, then Select tool, then Save]
  J --> K[Verify page: done]
`
7. Good
The automatic tag -102F1 from the folio numbering, the proposed wire with a clear toast that names the connection, the part dialog listing the Schneider part with "in this project", and look --page giving cells and potentials, which was enough to work blind.
