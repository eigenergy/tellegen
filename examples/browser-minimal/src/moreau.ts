import { createStudy, executionCapabilities, ingestCase, type ExecutionOptions } from "@tellegen/engine";
import { CASE14 } from "./case14";

const status = document.querySelector<HTMLElement>("#status")!;
const results = document.querySelector<HTMLElement>("#results")!;
const run = document.querySelector<HTMLButtonElement>("#run")!;
const file = document.querySelector<HTMLInputElement>("#case")!;

async function sample(operation: () => Promise<unknown>) {
  let start = performance.now();
  await operation();
  const cold_ms = performance.now() - start;
  for (let i = 0; i < 3; i++) await operation();
  const samples_ms = [];
  for (let i = 0; i < 10; i++) {
    start = performance.now();
    await operation();
    samples_ms.push(performance.now() - start);
  }
  samples_ms.sort((a, b) => a - b);
  return { cold_ms, median_ms: (samples_ms[4] + samples_ms[5]) / 2, samples_ms };
}

run.addEventListener("click", async () => {
  run.disabled = true;
  status.textContent = "Running";
  const records: unknown[] = [];
  try {
    if (!(await executionCapabilities()).moreau) throw new Error("Build the engine with npm --workspace @tellegen/engine run wasm:moreau");
    const bytes = file.files?.[0] ? new Uint8Array(await file.files[0].arrayBuffer()) : new TextEncoder().encode(CASE14);
    const parsed = await ingestCase(bytes, "matpower");
    if (!parsed.module_json) throw new Error("Case has no solvable PowerIO module");
    for (const dc_solver of ["clarabel", "moreau"] as const) {
      for (const dc_derivatives of ["tellegen", "moreau_selected"] as const) {
        const execution: ExecutionOptions = { dc_solver, dc_derivatives };
        const start = performance.now();
        let study: Awaited<ReturnType<typeof createStudy>> | undefined;
        try {
          study = await createStudy(parsed.module_json, "dcopf", { isolated: true, execution });
          const setup_and_solve_ms = performance.now() - start;
          const baseline = await study.currentSolution();
          const bus = baseline.prices?.[1]?.bus ?? baseline.prices?.[0]?.bus;
          if (bus === undefined) throw new Error("Case has no nodal price axis");
          const preview = await sample(() => study!.preview({ [bus]: 0.01 }));
          const commit = await sample(() => study!.commit(parsed.name, {}, {}, { bus }));
          const previewResult = await study.preview({ [bus]: 0.01 });
          const column = await study.sensitivity(parsed.name, {}, {}, { bus });
          const branches = parsed.topology.branches.filter((branch) => branch.status !== 0 && branch.editable !== false).slice(0, 3).map((branch) => branch.uid ?? `${branch.from}-${branch.to}`);
          const planStart = performance.now();
          const plan = await study.plan({ objective: { kind: "weighted_lmp", weights: [{ bus, weight: 1 }] }, candidates: branches, max_increase_per_branch_mw: 1, budget_mw: 1, increment_mw: 1, max_changed_lines: 1, exact_solve_budget: 2 });
          records.push({ execution, setup_and_solve_ms, demand_preview: preview, commit_with_column: commit, planning_ms: performance.now() - planStart, objective: baseline.objective, lmp: baseline.prices, dispatch: baseline.dispatch, preview_prices: previewResult.prices, demand_column: column?.values, exact_solves: plan.exact_solves });
        } catch (error) {
          records.push({ execution, error: String(error) });
        } finally { study?.free(); }
      }
    }
    results.textContent = JSON.stringify({ case: parsed.name, browser: navigator.userAgent, records }, null, 2);
    status.textContent = "Complete";
  } catch (error) {
    status.textContent = String(error);
  } finally { run.disabled = false; }
});
