import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
import { initSync, ingest_case, Study, study_document_create, study_document_import, study_document_run } from "../src/wasm-pkg/tellegen.js";
import type { StudyBundle, StudyOperationResult } from "../src/generated/study-types.js";

initSync({module: readFileSync(new URL("../src/wasm-pkg/tellegen_bg.wasm", import.meta.url))});

it("preserves declared objective kind through saved-study browser operations", async () => {
  const source = readFileSync(new URL("../../../validation/evidence/studies/case3.m", import.meta.url));
  const input = JSON.parse(ingest_case(source, "matpower")) as {module_json: string};
  const study = new Study(input.module_json, "dcopf");
  try {
    const view = JSON.parse(study.solution()) as {objective_kind?: string};
    delete view.objective_kind; // A result saved by an older version.
    const created = study_document_create(JSON.stringify({
      id: "kind-test", title: "Saved case", input: study.save_instance_module(),
      formulation: "dcopf", solution: study.save_solution_module(), view,
    }));
    const imported = study_document_import(created);
    const bundle = JSON.parse(imported) as StudyBundle;
    const state = bundle.document.inspected_state!;
    const response = JSON.parse(await study_document_run(imported, JSON.stringify({
      expected_revision: bundle.document.revision,
      operation: {kind: "compare", left: state, right: state},
    }), () => false)) as {result: StudyOperationResult};
    expect(response.result.inspected_view?.objective_kind).toBe("cost");
    expect(response.result.comparison?.left_view?.objective_kind).toBe("cost");
    expect(response.result.comparison?.right_view?.objective_kind).toBe("cost");
  } finally {
    study.free();
  }
});
