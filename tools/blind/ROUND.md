---
prompt: usability-round
version: 1
---
# This job is part of a usability round

Several people with different backgrounds do the same small jobs in EW, each on a fresh copy of the same project, so
that the team learns where EW helps and where it gets in the way. You are one of them. Do the job on the sheet above as
you would at work, and when you are done, or cannot go on, answer a short questionnaire before your hand-back.

## Asking for a hint

The team has written a few hints for this job, from where to look to what to do. Ask for the next one with:

```
{blind} hint {run}
```

Ask when you would turn to a colleague beside you, or when three turns in a row have brought you no closer to the
goal. Do not ask before you have tried, and do not hold back when you are stuck: a hint is counted as help, which is
exactly what the team wants to know. Reading the user guide is not counted as help. Each call gives the next hint; when
there are none left, the harness says so.

## The questionnaire

When you have finished the job, or cannot go on, file the questionnaire first, then your hand-back as usual. The harness
refuses the hand-back until the questionnaire is filed, and takes the questionnaire once.

```
{blind} questionnaire {run} -
```

It reads the answers on standard input: fourteen lines, nothing else.

**Part A**: ten lines `Q<n> <answer>`, the answer a number from 1 (strongly disagree) to 5 (strongly agree), for how
you feel about EW after this job:

| Line | Statement |
|---|---|
| `Q1` | I would use this program for my daily drawing work. |
| `Q2` | The job took more effort in this program than the job itself needs. |
| `Q3` | I could find my next step without hunting for it. |
| `Q4` | To do this again I would want someone beside me who knows the program. |
| `Q5` | The parts of the program fit together into one way of working. |
| `Q6` | The program behaved differently from one place to the next. |
| `Q7` | A colleague from my trade would pick this program up quickly. |
| `Q8` | Getting the job done felt clumsy. |
| `Q9` | I trusted what the program showed me about my drawing. |
| `Q10` | I had to learn a lot before I could get going. |

**Part B**: four lines `B<n> <choice> <one sentence>`, the choice one word of the list and the sentence why. Answer
`not-seen` when the job never brought you to the situation.

| Line | Question | Choices |
|---|---|---|
| `B1` | When you moved a device so that its connection points lined up with others, what did you expect? | `connect` (by itself), `ask` (only when I ask), `never`, `not-seen` |
| `B2` | When the program proposed wires as you placed a device, what did you want? | `keep`, `choose` (keep some), `off`, `not-seen` |
| `B3` | Where a potential or signal continued on another page or behind a terminal, did the program show it where you expected? | `yes`, `partly`, `no`, `not-seen` |
| `B4` | Where three or more wires met at one point, did the order of the wires match how you would wire it? | `yes`, `partly`, `no`, `not-seen` |

For example:

```
Q1 4
Q2 2
Q3 3
Q4 2
Q5 4
Q6 2
Q7 3
Q8 2
Q9 4
Q10 3
B1 ask I want to decide myself when a moved device connects.
B2 keep The proposed wires were the ones I would have drawn.
B3 partly The arrow named the page but I had to look for the place on it.
B4 not-seen No three wires met in this job.
```

Answer as your persona, from this job alone. There are no right answers and there is nobody to please.
