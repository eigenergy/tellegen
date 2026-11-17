# Cleanup progress

This records unfinished work as well as completed checks. A passing PR is not
proof that its changes are on the deployed site.

| Work | Current state | Remaining checks or work |
| --- | --- | --- |
| Repository folders and contribution rules | #401 merged | Require owner review after cleanup |
| WebMCP clarity | #402 merged | Main CI and deployment; final browser check |
| Engine errors and result fields | #404 updated | CI; saved-study result metadata |
| Generated-grid handoff, #261 | #405 reviewed, CI passed | Merge, main CI, deploy, issue closure |
| Shared frontend rules and messages | #406 reviewed, CI passed | Merge and integration checks |
| Documentation | #407 open | CI, final behavior updates and package examples |
| Collections and nested ZIPs, #279 | #408 open | CI and deployed import check |
| Saved-study storage and controls | #409 open | CI and integration checks |
| AC OPF speed and sensitivities | Separate branch under test | Native/browser measurements, PR, integration |
| General OpenDSS models, #310 | PowerIO changes under test | Reviewed dependency release; Tellegen qualification |
| CLI preparation costs, #363 | Existing output options verified | Repeated measurements and profile, PR |
| Numerical backend comparison, #153 | Not yet implemented | Reduced-system study and accuracy checks |
| MC OPF review, #338 | Not yet implemented | All issue criteria and independent comparisons |
| Synchronous areas and HVDC, #263 | Not yet implemented | Generation, seed sweeps, reports and viewer |
| Drawing editing and persistence, #198 | Not yet implemented | Complete drawing and export workflows |
| Node-breaker diagrams, #388 | Not yet implemented | Source fixtures, shared renderer and detailed views |
| Dynamics PR #399 | Private review in progress | Keep findings private; do not merge |
| Release and local cleanup | Pending implementation | Patch releases, installed packages, production checks |

The interrupted AC OPF files and agent changes were backed up before editing.
Existing unrelated files and worktrees are retained.
