---
"@tellegen/engine": patch
---

AC OPF solves reuse one worker and its compiled solver module instead of starting and compiling a new one each time, and their results carry nodal prices. `releaseAcOpfWorker()` frees the idle worker.

Sensitivity requests can be passed as the optional fifth argument to `solveAcOpfModule`. The separate WASI solver includes demand, line-limit and generator-bound derivatives, and worker failures retain the engine error code.
