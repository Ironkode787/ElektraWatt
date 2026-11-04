# Run r0-a7-qet-veteran-v

kind brief, task brief-ut-a7-move, end handback

## Score

Attempt: **unassisted**

| ID | Kind | Decision |
|---|---|---|
| A1 | hard | met |
| A2 | hard | met |
| A3 | hard | met |
| A4 | soft | met |

turns 3, steps 5, refused 0, failed 0, undo 0, hints 0; friction annoying 0, blocker 0, confusing 0, cosmetic 1

## Protocol

turns 3, steps 5, looks 2, harness refusals 0, refused 0, failed 0, broken 0

## Observations

- the project was clean after the agent's last step: yes
- the product's exit code: 0
- page texts of the epilogue: 16
- the PDFs of 1 and 4 threads are byte-identical: yes
- ew export pdf --threads 1: exit 0
- ew export pdf --threads 4: exit 0
- ew export bom --mode parts: exit 0
- ew check --format text: exit 1
- ew check --format json: exit 1
- golden/ unchanged: yes

## Prelude

5 lines

## Broken steps

none

## Harness refusals

none
