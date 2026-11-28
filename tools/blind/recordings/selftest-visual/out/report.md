# Run selftest-visual

kind free, task selftest-visual, end handback

## Score

A free run holds no oracle: the protocol counts below are its score.

## Protocol

turns 7, steps 18, looks 2, harness refusals 2, refused 1, failed 0, broken 0

## Observations

- the project was clean after the agent's last step: yes
- the product's exit code: 0
- page texts of the epilogue: 1
- the PDFs of 1 and 4 threads are byte-identical: yes
- ew export pdf --threads 1: exit 0
- ew export pdf --threads 4: exit 0
- ew export bom --mode parts: exit 2 (error: the project has no report block of kind `parts_summary`: the bill of materials is the rows of that block (a project from the built-in template has none); `ew export report --kind parts_summary` exports the rows of the whole project instead)
- ew check --format text: exit 0
- ew check --format json: exit 0

## Prelude

3 lines

## Broken steps

none

## Harness refusals

- harness: in a visual run you act with the mouse and the keys
- harness: a visual run has no text view
