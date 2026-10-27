# Run selftest

kind free, task selftest, end handback

## Score

A free run holds no oracle: the protocol counts below are its score.

## Protocol

turns 12, steps 38, looks 6, harness refusals 1, refused 1, failed 0, broken 0

## Observations

- the project was clean after the agent's last step: yes
- the product's exit code: 1
- page texts of the epilogue: 1
- the PDFs of 1 and 4 threads are byte-identical: yes
- ew export pdf --threads 1: exit 0
- ew export pdf --threads 4: exit 0
- ew export bom --mode parts: exit 2 (error: the project has no report block of kind `parts_summary`: the bill of materials is the rows of that block (the company-standard wizard creates it as folio 901; a project from the built-in template has none), so there is nothing to export)
- ew check --format text: exit 0
- ew check --format json: exit 0
- golden/ unchanged: yes

## Prelude

3 lines

## Broken steps

none

## Harness refusals

- harness: help.tutorials is not available to the drafter
