import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  classifyExpectedPreparationFailure,
  CommandError,
  resolvePowerioProvenance,
  summarizeSolutionModule,
  validatePlanResponse,
  validateSpec,
} from "./run.mjs";

const cats = JSON.parse(
  await readFile(new URL("./specs/cats.json", import.meta.url), "utf8"),
);
const texas = JSON.parse(
  await readFile(new URL("./specs/texas7k.json", import.meta.url), "utf8"),
);

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function validPlanResponse() {
  const [first, second, rejected] = cats.request.candidates;
  return {
    plan: {
      baseline: { phi: 10, declared_objective: 100, exact_solve: 1 },
      exact_verified_result: {
        phi: 8,
        declared_objective: 90,
        exact_solve: 4,
      },
      baseline_phi: 10,
      final_phi: 8,
      proposal: [
        { branch: first, delta_mw: 50 },
        { branch: second, delta_mw: 50 },
      ],
      spent_budget_mw: 100,
      iterations: [
        {
          delta_mw: [{ branch: first, delta_mw: 50 }],
          accepted: true,
        },
        {
          delta_mw: [{ branch: rejected, delta_mw: 50 }],
          accepted: false,
        },
        {
          delta_mw: [{ branch: second, delta_mw: 50 }],
          accepted: true,
        },
        { delta_mw: [], accepted: false },
      ],
      exact_solves: 4,
    },
    solution_module: {
      schema: "powerio.module",
      version: 1,
      value: { kind: "dc_opf_solution" },
    },
  };
}

test("checked in CATS invocation has the complete bounded planning request", () => {
  assert.equal(validateSpec(cats), cats);
  assert.equal(cats.request.objective.kind, "weighted_lmp");
  assert.ok(cats.request.objective.weights.length > 0);
  assert.ok(cats.request.candidates.length > 0);
  assert.ok(cats.request.exact_solve_budget >= 1);
});

test("checked in Texas7k invocation uses stable source identities", () => {
  assert.equal(validateSpec(texas), texas);
  assert.deepEqual(texas.expected_outcome, {
    kind: "failure",
    code: "nonconvex_piecewise_generator_cost",
  });
  assert.ok(
    texas.request.objective.weights.every(({ bus }) => Number.isInteger(bus)),
  );
  assert.ok(
    texas.request.candidates.every((identity) =>
      /^branches:\d+$/.test(identity),
    ),
  );
});

test("invocation refuses unknown framing and absolute source paths", () => {
  assert.throws(
    () => validateSpec({ ...cats, commentary: "trust me" }),
    /unknown spec field/,
  );
  assert.throws(
    () => validateSpec({ ...cats, source: "/tmp/case.m" }),
    /repository relative/,
  );
});

test("invocation requires every capacity and solve bound", () => {
  const request = { ...cats.request };
  delete request.exact_solve_budget;
  assert.throws(
    () => validateSpec({ ...cats, request }),
    /missing "exact_solve_budget"/,
  );
});

test("invocation rejects request fields the Rust contract does not state", () => {
  assert.throws(
    () =>
      validateSpec({
        ...cats,
        request: { ...cats.request, narrative: "trust me" },
      }),
    /unknown request field/,
  );
  assert.throws(
    () =>
      validateSpec({
        ...cats,
        request: { ...cats.request, budget_mw: Number.NaN },
      }),
    /budget_mw must be positive and finite/,
  );
});

test("only a named preparation error becomes a zero solve failure", () => {
  const expected = {
    kind: "failure",
    code: "nonconvex_piecewise_generator_cost",
  };
  assert.equal(
    validateSpec({ ...texas, expected_outcome: expected }).expected_outcome,
    expected,
  );
  const matched = classifyExpectedPreparationFailure(
    expected,
    new CommandError(
      "tellegen",
      ["plan"],
      "tellegen: BUILD.INSTANCE.PIECEWISE_COST_NONCONVEX: generator 0",
    ),
  );
  assert.deepEqual(matched, {
    code: expected.code,
    stage: "instance_preparation",
    message: "tellegen: BUILD.INSTANCE.PIECEWISE_COST_NONCONVEX: generator 0",
    exact_solves_completed: 0,
  });
  assert.equal(
    classifyExpectedPreparationFailure(
      expected,
      new CommandError("tellegen", ["plan"], "tellegen: solver diverged"),
    ),
    null,
  );
});

test("plan response accounts for the baseline and every nonempty trial", () => {
  const response = validPlanResponse();
  assert.equal(validatePlanResponse(response, cats.request), response);

  const wrongCount = structuredClone(response);
  wrongCount.plan.exact_solves = 5;
  assert.throws(
    () => validatePlanResponse(wrongCount, cats.request),
    /baseline plus every nonempty trial/,
  );
});

test("proposal and trial changes must use request candidate identities", () => {
  const unknownProposal = validPlanResponse();
  unknownProposal.plan.proposal[0].branch = "branches:999999";
  assert.throws(
    () => validatePlanResponse(unknownProposal, cats.request),
    /is not a request candidate/,
  );

  const unknownTrial = validPlanResponse();
  unknownTrial.plan.iterations[0].delta_mw[0].branch = "branches:999999";
  assert.throws(
    () => validatePlanResponse(unknownTrial, cats.request),
    /positive request-candidate increment/,
  );
});

test("proposal enforces cardinality and discrete increments", () => {
  const response = validPlanResponse();
  assert.throws(
    () =>
      validatePlanResponse(response, {
        ...cats.request,
        max_changed_lines: 1,
      }),
    /exceeds max_changed_lines/,
  );

  const offIncrement = validPlanResponse();
  offIncrement.plan.proposal[0].delta_mw = 25;
  offIncrement.plan.iterations[0].delta_mw[0].delta_mw = 25;
  offIncrement.plan.spent_budget_mw = 75;
  assert.throws(
    () => validatePlanResponse(offIncrement, cats.request),
    /not a multiple of increment_mw/,
  );
});

test("proposal enforces per branch and total MW budgets", () => {
  const overBranch = validPlanResponse();
  const branch = cats.request.candidates[0];
  overBranch.plan.proposal = [{ branch, delta_mw: 150 }];
  overBranch.plan.spent_budget_mw = 150;
  overBranch.plan.iterations = [
    { delta_mw: [{ branch, delta_mw: 50 }], accepted: true },
    { delta_mw: [{ branch, delta_mw: 50 }], accepted: true },
    { delta_mw: [{ branch, delta_mw: 50 }], accepted: true },
  ];
  assert.throws(
    () =>
      validatePlanResponse(overBranch, {
        ...cats.request,
        budget_mw: 200,
      }),
    /exceeds max_increase_per_branch_mw/,
  );

  assert.throws(
    () =>
      validatePlanResponse(validPlanResponse(), {
        ...cats.request,
        budget_mw: 50,
      }),
    /exceeds budget_mw/,
  );
});

test("proposal reconstructs exactly from accepted trials", () => {
  const response = validPlanResponse();
  response.plan.proposal[0].delta_mw = 100;
  response.plan.spent_budget_mw = 150;
  assert.throws(
    () =>
      validatePlanResponse(response, {
        ...cats.request,
        budget_mw: 150,
      }),
    /does not reconstruct from accepted trial changes/,
  );
});

test("the JSON Schemas parse and name the checked in contracts", async () => {
  const specSchema = JSON.parse(
    await readFile(new URL("./spec.schema.json", import.meta.url), "utf8"),
  );
  const resultSchema = JSON.parse(
    await readFile(new URL("./result.schema.json", import.meta.url), "utf8"),
  );
  assert.equal(specSchema.properties.schema.const, cats.schema);
  assert.equal(
    resultSchema.properties.schema.const,
    "tellegen.webmcp-challenge-result/1",
  );
  assert.deepEqual(resultSchema.properties.status.enum, ["success", "failure"]);
  assert.deepEqual(resultSchema.properties.solution.properties.version, {
    const: 1,
  });

  const releases = JSON.parse(
    await readFile(new URL("./powerio-releases.json", import.meta.url), "utf8"),
  );
  assert.equal(releases.schema, "tellegen.powerio-release-revisions/1");
  assert.ok(Array.isArray(releases.releases));
});

test("solution evidence is compact and independent of object key order", () => {
  const first = {
    schema: "powerio.module",
    version: 1,
    value: {
      kind: "dc_opf_solution",
      data: {
        termination: { kind: "converged" },
        objective: 90,
        instance: { network: { buses: [{ uid: "bus-1" }] } },
      },
    },
  };
  const second = {
    value: {
      data: {
        instance: { network: { buses: [{ uid: "bus-1" }] } },
        objective: 90,
        termination: { kind: "converged" },
      },
      kind: "dc_opf_solution",
    },
    version: 1,
    schema: "powerio.module",
  };
  assert.deepEqual(
    summarizeSolutionModule(first),
    summarizeSolutionModule(second),
  );
});

const powerioNames = [
  "powerio",
  "powerio-core",
  "powerio-dist",
  "powerio-matrix",
  "powerio-prob",
  "powerio-tx",
];

test("PowerIO provenance accepts one exact Git revision", () => {
  const revision = "a".repeat(40);
  const packages = powerioNames.map((name) => ({
    name,
    version: "1.0.0",
    source: `git+https://github.com/eigenergy/powerio.git?rev=${revision}#${revision}`,
    checksum: null,
  }));
  const resolved = resolvePowerioProvenance(packages, {
    schema: "tellegen.powerio-release-revisions/1",
    releases: [],
  });
  assert.equal(resolved.powerio_revision, revision);
  assert.equal(resolved.powerio_release_tag, null);
  assert.deepEqual(resolved.powerio_lock_checksums, {});
});

test("PowerIO provenance ties a checksummed registry release to its commit", () => {
  const revision = "b".repeat(40);
  const packages = powerioNames.map((name, index) => ({
    name,
    version: "1.0.0",
    source: "registry+https://github.com/rust-lang/crates.io-index",
    checksum: index.toString(16).repeat(64),
  }));
  const resolved = resolvePowerioProvenance(packages, {
    schema: "tellegen.powerio-release-revisions/1",
    releases: [{ version: "1.0.0", tag: "v1.0.0", revision }],
  });
  assert.equal(resolved.powerio_revision, revision);
  assert.equal(resolved.powerio_release_tag, "v1.0.0");
  assert.equal(
    Object.keys(resolved.powerio_lock_checksums).length,
    powerioNames.length,
  );
});

test("native browser evidence matches its checked in media", async () => {
  const native = JSON.parse(
    await readFile(
      new URL("./native/in-app-browser.json", import.meta.url),
      "utf8",
    ),
  );
  const videoRun = JSON.parse(
    await readFile(new URL("./native/video-run.json", import.meta.url), "utf8"),
  );
  const repoRoot = new URL("../../", import.meta.url);

  assert.equal(native.schema, "tellegen.native-webmcp-evidence/1");
  assert.equal(native.software.powerio_version, "1.0.0");
  assert.equal(native.checks.missing_callback_options.passed, true);
  assert.equal(
    native.checks.application_before_approval.error.code,
    "APPROVAL_REQUIRED",
  );
  assert.equal(native.checks.application.ok, true);
  assert.ok(
    native.checks.application.data.after.objective <
      native.checks.application.data.before.objective,
  );
  assert.equal(native.checks.stale_mutation.error.code, "STALE_REVISION");
  assert.equal(native.checks.invalid_mutation.error.code, "EDIT_OUT_OF_RANGE");
  assert.equal(native.checks.rejected_mutation_rollback.passed, true);
  assert.equal(
    native.checks.rejected_mutation_rollback.error.code,
    "EDIT_OUT_OF_RANGE",
  );
  assert.equal(
    native.checks.rejected_mutation_rollback.before.data.revision,
    native.checks.rejected_mutation_rollback.after.data.revision,
  );
  assert.equal(native.checks.navigation_invalidation.in_flight, true);
  assert.equal(native.checks.navigation_invalidation.new_session, true);
  assert.equal(native.checks.navigation_invalidation.proposal_cleared, true);

  for (const artifact of [...native.screenshots, native.video]) {
    const bytes = await readFile(new URL(artifact.path, repoRoot));
    assert.equal(sha256(bytes), artifact.sha256, artifact.path);
    if (artifact.path.endsWith(".png")) {
      assert.deepEqual(
        [...bytes.subarray(0, 8)],
        [137, 80, 78, 71, 13, 10, 26, 10],
        artifact.path,
      );
    } else if (artifact.path.endsWith(".mp4")) {
      assert.equal(bytes.subarray(4, 8).toString("ascii"), "ftyp");
    }
  }

  assert.equal(videoRun.schema, "tellegen.native-webmcp-video-run/1");
  assert.equal(videoRun.video.sha256, native.video.sha256);
  assert.equal(
    videoRun.application_before_approval.error.code,
    "APPROVAL_REQUIRED",
  );
  assert.equal(videoRun.application.ok, true);
  assert.equal(videoRun.final_proposal_status, "expired");
  assert.ok(videoRun.frames > 0);
});
