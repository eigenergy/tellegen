# Workbench usability sessions (M0)

A short, repeatable protocol for testing whether the engineering workbench helps
distribution engineers find and explain problems. Run it before the Svelte port
(M3) is built out, so findings change the design while it is still cheap.

## Participants and setup

- **Who:** 3–5 engineers who read distribution power-flow results at work
  (network planning, connections or operations). Include at least one
  multiconductor/LV specialist. No one who built the workbench.
- **Duration:** 30 minutes each: 5 intro, 20 tasks, 5 debrief.
- **Setup:** the staged French feeder (`french-medium`, saved solution) in
  Chrome, 1440 × 900 or larger, dark theme. Open the start URL below. Use a fresh
  window per participant so URL state and browser storage are empty.
- **Start URL:** `/performance/?dataset=french-medium&layout=tidy-tree&routing=equipment&open=compact&results=loading&flows=static`
- **Roles:** one facilitator who reads the script and doesn't help; one note
  taker. Record the screen and audio only with the participant's consent.

## Script

> We're testing the tool, not you. Please think aloud: say what you're looking
> for and what you expect to happen. I can't answer questions about how to use
> it during the tasks, but I'll answer everything afterwards. You can stop at any
> time.

Read each task aloud and start the timer when you finish reading. A task ends
when the participant states an answer, or after its time limit. Don't confirm
answers until the debrief.

## Tasks

| # | Task (read aloud) | Success criterion | Limit |
|---|---|---|---|
| 1 | "Is anything wrong in this network? Tell me the most serious problem." | Names `75_LVBus0623137` undervoltage (3–n below its limit) | 3 min |
| 2 | "How far outside its limit is it, and on which phase?" | ≈ 0.03 % (207.78 V vs 207.85 V), phase 3 to neutral | 2 min |
| 3 | "Follow the voltage from the source to that bus. Where does it drop the most?" | Uses the profile; points to the LV section after the transformer | 3 min |
| 4 | "Which line is the most heavily loaded? Is it overloaded?" | `75_LVBranch0306208`, 94.2 %, a warning but not an overload | 2 min |
| 5 | "Why is that line loaded so heavily? Look at its phases." | Notices phase 3 ≈ 176 A vs 91/113 A, and the 72 A neutral current | 3 min |
| 6 | "Find bus `75_LVBus0622835` and tell me its lowest phase voltage." | Uses search; 215.21 V on 3–n | 2 min |
| 7 | "Show me where this feeder is on the map, then come back." | Uses Geographic / Schematic; keeps the selection | 2 min |
| 8 | "Send a colleague a link to exactly this view." | Copies the URL (or asks how); notes that it restores the view | 1 min |
| 9 | "Put a picture of the worst issue in a report." | Exports PNG or SVG with the title block | 2 min |

## What to record per task

- Completed (yes / partly / no), time to answer, and the first control the
  participant reached for.
- Errors and wrong turns, quoted verbatim where possible.
- Any value read wrongly (sign convention, units, phase–neutral vs
  phase–phase). These are the most important findings.

## Debrief questions

1. What did you expect to see first when the page opened?
2. Was anything ambiguous about the numbers: signs, units, which voltage is which?
3. What would you need before you trusted this for a real study?
4. What do you use today for this, and what does it do better?
5. On a scale of 1–7, how easy was it to find and explain the problem?

## Analysis

- Per task: completion rate and median time. A task that fewer than 3 of 5
  participants complete is a design problem, not a participant problem.
- Group observations into findings, each with a severity: **critical** (wrong
  engineering conclusion), **major** (task failed or took over twice the median),
  **minor** (friction).
- Every critical finding becomes an issue before the M3 components are built.
  Re-run tasks 1, 2 and 5 after fixing critical findings.
- Report results in the PR or the M3 tracking issue, without participant names.
