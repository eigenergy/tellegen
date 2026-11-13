---
"@tellegen/svelte": minor
---

Lead each result with one plain sentence. The Solver panel opens with what the calculation found and how long it took, for example "AC power flow converged in 4 iterations (38 ms).", or what failed and what to try next. The network summary states the OPF objective and how many lines are at their limit. The convergence plot, raw solver status, solver backend and fallback reason move under a "Solver details" disclosure. `statusWords()`, `solveVerdict()` and `outcomeVerdict()` in `diagram/verdict.ts` build these sentences.

With no `theme` prop, no `data-theme` in the host template and no saved viewer choice, the provider now follows the operating system's `prefers-color-scheme`, and `styles.css` paints the dark tokens from the first frame.

Notices are full sentences that say what happened and what to do; repeated ones are shared as `NOTICES`. The transmission result panels use the type and spacing tokens, set numbers in tabular mono, and read counts as "Exceptions (3)" rather than "Exceptions · 3".
