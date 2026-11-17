# Cleanup progress

This records unfinished work as well as completed checks. A passing PR is not
proof that its changes are on the deployed site.

| Work | Current state | Remaining checks or work |
| --- | --- | --- |
| Repository folders and contribution rules | #401 merged | Require owner review after cleanup |
| WebMCP clarity | #402 merged; attached Brave agent checked local and earlier deployed build | Final deployed browser check |
| Engine errors and result fields | #404 and saved-study metadata #413 merged | Final integration and package checks |
| Generated-grid handoff, #261 | #405 merged | Final deployed handoff check |
| Shared frontend rules and messages | #406 merged; all PR checks passed | Final deployed checks |
| Documentation | #407 open; 20 chapters, links and Rust examples checked | Final behavior updates, CI and package examples |
| Collections and nested ZIPs, #279 | #408 merged; all PR checks passed | Final deployed import check |
| Saved-study storage and controls | #409 merged; migration and browser checks passed | Final deployed checks |
| AC OPF speed and sensitivities | #411 and viewer #415 merged with all checks passed | Final browser checks |
| Neutral reusable viewer and result labels | #414 merged with all checks passed | Installed-package check |
| General OpenDSS models, #310 | PowerIO #884 merged; #885 merged the prerequisite release source; Tellegen implementation and #417 pass local checks | Dependency publication; published-version qualification |
| CLI preparation costs, #363 | #412 merged; profile and 180 cold-start measurements recorded | Final release |
| Numerical backend comparison, #153 | #420 merged with all checks passed; Austin reproduction and 316,000-unknown prepared comparison recorded | PowerIO 0.12.2 recheck still refuses all three original imports before preparation |
| MC OPF review, #338 | #416 merged with all checks passed; formulation review and OpenDSS comparisons recorded | Final release |
| Synchronous areas and HVDC, #263 | #418 and viewer #422 merged; four-country/three-seed and browser checks pass | Final deployed checks |
| Drawing editing and persistence, #198 | #421 merged with all CI checks passed; radial, meshed and substation browser checks and performance measurements recorded | 38 combined browser checks passed; final deployed checks remain |
| Node-breaker diagrams, #388 | #421 merged with all CI checks passed, detailed projection and replacement of the old canvas | 38 combined browser checks passed; final deployed checks remain |
| Shared result validation and MCP argument types | #419 merged with all CI checks passed; 173 engine and 628 frontend tests, native compatibility and Rust checks pass | Final installed-package checks |
| Controller organization | #423 merged; public API preserved, review and combined browser checks pass | Final deployed checks |
| Dynamics PR #399 | Private review completed | Findings remain private; PR remains unmerged |
| Release and local cleanup | Pending implementation | Patch releases, installed packages, production checks |

The interrupted AC OPF files and agent changes were backed up before editing.
Existing unrelated files and dirty worktrees are retained. Original study data
is preserved during migration; deleting a migrated study does not restore it
on the next launch.
