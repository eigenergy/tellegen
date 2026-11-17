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
| Saved-study storage and controls | #409 open; migration and browser tests pass locally | CI, merge and integration checks |
| AC OPF speed and sensitivities | #411 and viewer #415 open; native and WASI measurements recorded | CI, integration and final browser checks |
| Neutral reusable viewer and result labels | #414 open | CI, merge and installed-package check |
| General OpenDSS models, #310 | PowerIO #884 merged; #885 releases the prerequisite; Tellegen implementation and #417 pass local checks | Dependency publication; published-version qualification |
| CLI preparation costs, #363 | #412 merged; profile and 180 cold-start measurements recorded | Final release |
| Numerical backend comparison, #153 | #420 reproduces the Austin discrepancy and compares actual Tellegen reduced systems | Regional import is unsupported; further bounded checks underway |
| MC OPF review, #338 | #416 open; formulation review and independent OpenDSS comparisons recorded | CI and integration |
| Synchronous areas and HVDC, #263 | #418 merged; all four countries converge within voltage limits for three seeds | Complete viewer and deployed checks |
| Drawing editing and persistence, #198 | #421 open; radial, meshed and substation browser checks and performance measurements recorded | Fix review findings, CI and integration |
| Node-breaker diagrams, #388 | #421 includes detailed projection and removes the old canvas | Mixed-topology review fixes, CI and integration |
| Shared result validation and MCP argument types | #419 includes engine, HTTP stream and Python checks generated from Rust; local tests pass | CI and integration |
| Controller organization | Extraction underway behind the same public API | Complete tests, review and integration |
| Dynamics PR #399 | Private review completed | Findings remain private; PR remains unmerged |
| Release and local cleanup | Pending implementation | Patch releases, installed packages, production checks |

The interrupted AC OPF files and agent changes were backed up before editing.
Existing unrelated files and dirty worktrees are retained. Original study data
is preserved during migration; deleting a migrated study does not restore it
on the next launch.
