---
"@tellegen/engine": minor
"@tellegen/svelte": patch
"@tellegen/webmcp": patch
---

The engine reads and writes cases with PowerIO 0.12 (release candidate 0.12.0-rc.1), up from 0.11.11. This is a new PowerIO release series, so it is a minor release of the engine.

- Saved cases, solutions and Study documents now state `"version": "0.12.0"` in their `pio-ir` header instead of `2`. PowerIO 0.11 readers, including the 0.11 Python wheel and older Tellegen releases, refuse these files; read them with PowerIO 0.12. Files saved by earlier releases still open, with their original meaning.
- A branch or winding with no thermal limit is now stated as a rating of positive infinity, and a rating of `0` is a zero limit. Case files keep their own convention: a MATPOWER `rateA` of `0` still means no limit. The display payloads still show an unrated branch's rating as `0`.
- An OpenDSS import that passes the snapshot checks can run a multiconductor power flow instead of being view-only.
- The SOCWR solution module names its branch flow columns `branch_from_active_flow`, `branch_from_reactive_flow`, `branch_to_active_flow` and `branch_to_reactive_flow`, as PowerIO 0.12 does; the values are unchanged.
