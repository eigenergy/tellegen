#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const RESULT_SCHEMA = "tellegen.webmcp-challenge-result/1";
const SPEC_SCHEMA = "tellegen.webmcp-challenge-spec/1";
const POWERIO_PACKAGES = new Set([
  "powerio",
  "powerio-core",
  "powerio-dist",
  "powerio-matrix",
  "powerio-prob",
  "powerio-tx",
]);
const EXPECTED_PREPARATION_FAILURES = new Map([
  [
    "nonconvex_piecewise_generator_cost",
    /BUILD\.INSTANCE\.PIECEWISE_COST_NONCONVEX/,
  ],
]);

export class CommandError extends Error {
  constructor(commandName, args, detail) {
    super(`${commandName} ${args.join(" ")}: ${detail}`);
    this.name = "CommandError";
    this.detail = detail;
  }
}

function fail(message) {
  throw new Error(message);
}

function command(commandName, args, options = {}) {
  const result = spawnSync(commandName, args, {
    cwd: options.cwd,
    encoding: "utf8",
    input: options.input,
    maxBuffer: 512 * 1024 * 1024,
  });
  if (result.error) fail(`${commandName}: ${result.error.message}`);
  if (result.status !== 0) {
    const detail =
      result.stderr.trim() || result.stdout.trim() || `exit ${result.status}`;
    throw new CommandError(commandName, args, detail);
  }
  return result.stdout;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function canonicalJsonValue(value) {
  if (Array.isArray(value)) return value.map(canonicalJsonValue);
  if (!plainObject(value)) return value;
  return Object.fromEntries(
    Object.keys(value)
      .sort()
      .map((key) => [key, canonicalJsonValue(value[key])]),
  );
}

export function summarizeSolutionModule(module) {
  const data = module.value?.data;
  if (!plainObject(data) || data.termination?.kind !== "converged") {
    fail("solution_module does not contain a converged result");
  }
  if (!Number.isFinite(data.objective)) {
    fail("solution_module does not contain a finite declared objective");
  }
  return {
    schema: module.schema,
    version: module.version,
    kind: module.value.kind,
    termination: data.termination,
    declared_objective: data.objective,
    canonical_json_sha256: sha256(
      Buffer.from(JSON.stringify(canonicalJsonValue(module))),
    ),
  };
}

function parseJson(text, label) {
  try {
    return JSON.parse(text);
  } catch (error) {
    fail(`${label} is not JSON: ${error.message}`);
  }
}

function plainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

export function validateSpec(spec) {
  if (!plainObject(spec)) fail("spec must be an object");
  const allowed = new Set([
    "schema",
    "case_id",
    "source",
    "request",
    "expected_outcome",
  ]);
  for (const key of Object.keys(spec)) {
    if (!allowed.has(key)) fail(`unknown spec field ${JSON.stringify(key)}`);
  }
  if (spec.schema !== SPEC_SCHEMA) fail(`spec schema must be ${SPEC_SCHEMA}`);
  if (
    typeof spec.case_id !== "string" ||
    !/^[a-z0-9][a-z0-9_-]*$/.test(spec.case_id)
  ) {
    fail(
      "case_id must contain only lower case ASCII letters, numbers, '_' or '-'",
    );
  }
  if (
    typeof spec.source !== "string" ||
    spec.source.length === 0 ||
    isAbsolute(spec.source)
  ) {
    fail("source must be a nonempty repository relative path");
  }
  if (!plainObject(spec.request))
    fail("request must be a CapacityPlanSpec object");
  const required = [
    "objective",
    "candidates",
    "max_increase_per_branch_mw",
    "budget_mw",
    "increment_mw",
    "max_changed_lines",
    "exact_solve_budget",
  ];
  const requestAllowed = new Set(required);
  for (const key of Object.keys(spec.request)) {
    if (!requestAllowed.has(key))
      fail(`unknown request field ${JSON.stringify(key)}`);
  }
  for (const key of required) {
    if (!(key in spec.request))
      fail(`request is missing ${JSON.stringify(key)}`);
  }
  const objective = spec.request.objective;
  if (!plainObject(objective) || objective.kind !== "weighted_lmp") {
    fail("request.objective must be weighted_lmp");
  }
  if (
    Object.keys(objective).some((key) => key !== "kind" && key !== "weights")
  ) {
    fail("request.objective has an unknown field");
  }
  if (!Array.isArray(objective.weights) || objective.weights.length === 0) {
    fail("request.objective.weights must be a nonempty array");
  }
  const buses = new Set();
  for (const term of objective.weights) {
    if (
      !plainObject(term) ||
      Object.keys(term).some((key) => key !== "bus" && key !== "weight") ||
      !Number.isInteger(term.bus) ||
      !Number.isFinite(term.weight)
    ) {
      fail(
        "each objective weight must contain one integer bus and finite weight",
      );
    }
    if (buses.has(term.bus)) fail(`objective bus ${term.bus} appears twice`);
    buses.add(term.bus);
  }
  if (
    !Array.isArray(spec.request.candidates) ||
    spec.request.candidates.length === 0
  ) {
    fail("request.candidates must be a nonempty array");
  }
  if (
    spec.request.candidates.some(
      (identity) => typeof identity !== "string" || identity.length === 0,
    )
  ) {
    fail("every request candidate must be a nonempty stable identity");
  }
  if (
    new Set(spec.request.candidates).size !== spec.request.candidates.length
  ) {
    fail("request candidates must be unique");
  }
  for (const key of [
    "max_increase_per_branch_mw",
    "budget_mw",
    "increment_mw",
  ]) {
    if (!Number.isFinite(spec.request[key]) || spec.request[key] <= 0) {
      fail(`request.${key} must be positive and finite`);
    }
  }
  for (const key of ["max_changed_lines", "exact_solve_budget"]) {
    if (!Number.isInteger(spec.request[key]) || spec.request[key] < 1) {
      fail(`request.${key} must be a positive integer`);
    }
  }
  if (spec.request.max_changed_lines > spec.request.candidates.length) {
    fail("request.max_changed_lines exceeds the candidate count");
  }
  if (spec.expected_outcome !== undefined) {
    const expected = spec.expected_outcome;
    if (
      !plainObject(expected) ||
      Object.keys(expected).some((key) => key !== "kind" && key !== "code") ||
      expected.kind !== "failure" ||
      !EXPECTED_PREPARATION_FAILURES.has(expected.code)
    ) {
      fail("expected_outcome must name one supported preparation failure code");
    }
  }
  return spec;
}

export function classifyExpectedPreparationFailure(expected, error) {
  if (
    !expected ||
    expected.kind !== "failure" ||
    !(error instanceof CommandError)
  ) {
    return null;
  }
  const pattern = EXPECTED_PREPARATION_FAILURES.get(expected.code);
  if (!pattern || !pattern.test(error.detail)) return null;
  return {
    code: expected.code,
    stage: "instance_preparation",
    message: error.detail,
    exact_solves_completed: 0,
  };
}

function approximatelyEqual(left, right) {
  const scale = 1 + Math.max(Math.abs(left), Math.abs(right));
  return Math.abs(left - right) <= 1e-9 * scale;
}

function isIncrementMultiple(value, increment) {
  return approximatelyEqual(value / increment, Math.round(value / increment));
}

export function validatePlanResponse(response, request) {
  if (
    !plainObject(response) ||
    !plainObject(response.plan) ||
    !plainObject(response.solution_module)
  ) {
    fail("tellegen plan did not return plan and solution_module objects");
  }
  const plan = response.plan;
  if (!plainObject(plan.baseline) || !plainObject(plan.exact_verified_result)) {
    fail("plan is missing exact baseline or proposed result summaries");
  }
  if (!Array.isArray(plan.iterations) || !Array.isArray(plan.proposal)) {
    fail("plan is missing iterations or proposal arrays");
  }
  if (!Number.isInteger(plan.exact_solves) || plan.exact_solves < 1) {
    fail("plan exact_solves must be a positive integer");
  }
  if (plan.exact_solves > request.exact_solve_budget) {
    fail("plan exceeded request.exact_solve_budget");
  }
  if (plan.baseline.exact_solve !== 1) fail("baseline must be exact solve 1");
  if (plan.exact_verified_result.exact_solve > plan.exact_solves) {
    fail("proposed result names an uncounted exact solve");
  }
  const candidates = new Set(request.candidates);
  const proposalByBranch = new Map();
  for (const change of plan.proposal) {
    if (
      !plainObject(change) ||
      typeof change.branch !== "string" ||
      !Number.isFinite(change.delta_mw) ||
      change.delta_mw <= 0
    ) {
      fail(
        "every proposal change must name one branch and a positive finite MW increase",
      );
    }
    if (!candidates.has(change.branch)) {
      fail(
        `proposal branch ${JSON.stringify(change.branch)} is not a request candidate`,
      );
    }
    if (proposalByBranch.has(change.branch)) {
      fail(`proposal branch ${JSON.stringify(change.branch)} appears twice`);
    }
    if (!isIncrementMultiple(change.delta_mw, request.increment_mw)) {
      fail(
        `proposal change for ${change.branch} is not a multiple of increment_mw`,
      );
    }
    if (change.delta_mw > request.max_increase_per_branch_mw + 1e-9) {
      fail(
        `proposal change for ${change.branch} exceeds max_increase_per_branch_mw`,
      );
    }
    proposalByBranch.set(change.branch, change.delta_mw);
  }
  if (proposalByBranch.size > request.max_changed_lines) {
    fail("proposal exceeds max_changed_lines");
  }

  const proposalTotal = [...proposalByBranch.values()].reduce(
    (sum, value) => sum + value,
    0,
  );
  if (proposalTotal > request.budget_mw + 1e-9) {
    fail("proposal exceeds budget_mw");
  }
  if (
    !Number.isFinite(plan.spent_budget_mw) ||
    !approximatelyEqual(plan.spent_budget_mw, proposalTotal)
  ) {
    fail("spent_budget_mw does not equal the proposal total");
  }

  const acceptedByBranch = new Map();
  let trialCount = 0;
  let lastAcceptedSolve = 1;
  for (const iteration of plan.iterations) {
    if (!plainObject(iteration) || !Array.isArray(iteration.delta_mw)) {
      fail("every plan iteration must contain a delta_mw array");
    }
    if (typeof iteration.accepted !== "boolean") {
      fail("every plan iteration must state whether it was accepted");
    }
    if (iteration.delta_mw.length === 0) {
      if (iteration.accepted)
        fail("an empty planning iteration cannot be accepted");
      continue;
    }
    trialCount += 1;
    const trialSolve = 1 + trialCount;
    for (const change of iteration.delta_mw) {
      if (
        !plainObject(change) ||
        typeof change.branch !== "string" ||
        !candidates.has(change.branch) ||
        !Number.isFinite(change.delta_mw) ||
        change.delta_mw <= 0 ||
        !isIncrementMultiple(change.delta_mw, request.increment_mw)
      ) {
        fail(
          "every trial change must be a positive request-candidate increment",
        );
      }
      if (iteration.accepted) {
        acceptedByBranch.set(
          change.branch,
          (acceptedByBranch.get(change.branch) ?? 0) + change.delta_mw,
        );
      }
    }
    if (iteration.accepted) lastAcceptedSolve = trialSolve;
  }
  if (plan.exact_solves !== 1 + trialCount) {
    fail("exact_solves must equal the baseline plus every nonempty trial");
  }
  if (plan.exact_verified_result.exact_solve !== lastAcceptedSolve) {
    fail("exact verified result does not name the last accepted exact solve");
  }
  if (acceptedByBranch.size !== proposalByBranch.size) {
    fail("proposal does not reconstruct from accepted trial changes");
  }
  for (const [branch, delta] of proposalByBranch) {
    if (
      !approximatelyEqual(acceptedByBranch.get(branch) ?? Number.NaN, delta)
    ) {
      fail("proposal does not reconstruct from accepted trial changes");
    }
  }
  if (
    response.solution_module.schema !== "powerio.module" ||
    response.solution_module.version !== 1
  ) {
    fail("solution_module is not powerio.module/1");
  }
  if (response.solution_module.value?.kind !== "dc_opf_solution") {
    fail("solution_module does not hold dc_opf_solution");
  }
  return response;
}

export function resolvePowerioProvenance(packages, releases) {
  if (packages.length !== POWERIO_PACKAGES.size) {
    fail("cargo metadata does not contain the complete PowerIO dependency set");
  }
  const names = new Set(packages.map((pkg) => pkg.name));
  if (
    names.size !== POWERIO_PACKAGES.size ||
    [...POWERIO_PACKAGES].some((name) => !names.has(name))
  ) {
    fail("cargo metadata does not contain each PowerIO component exactly once");
  }
  const sources = new Set(packages.map((pkg) => pkg.source));
  const versions = new Set(packages.map((pkg) => pkg.version));
  if (sources.size !== 1 || versions.size !== 1) {
    fail("PowerIO component crates do not resolve to one source and version");
  }
  const powerioSource = [...sources][0];
  const powerioVersion = [...versions][0];
  const gitMatch =
    typeof powerioSource === "string" &&
    powerioSource.match(/#([0-9a-f]{40})$/);
  let powerioRevision;
  let powerioReleaseTag = null;
  let powerioLockChecksums = {};
  if (gitMatch) {
    powerioRevision = gitMatch[1];
  } else if (
    typeof powerioSource === "string" &&
    powerioSource.startsWith("registry+")
  ) {
    powerioLockChecksums = Object.fromEntries(
      packages
        .map((pkg) => {
          if (
            typeof pkg.checksum !== "string" ||
            !/^[0-9a-f]{64}$/.test(pkg.checksum)
          ) {
            fail(
              `Cargo metadata has no SHA256 checksum for ${pkg.name} ${pkg.version}`,
            );
          }
          return [pkg.name, pkg.checksum];
        })
        .sort(([left], [right]) => left.localeCompare(right)),
    );
    if (
      releases.schema !== "tellegen.powerio-release-revisions/1" ||
      !Array.isArray(releases.releases)
    ) {
      fail("powerio-releases.json has the wrong schema");
    }
    const release = releases.releases.find(
      (entry) => entry.version === powerioVersion,
    );
    if (
      !release ||
      release.tag !== `v${powerioVersion}` ||
      typeof release.revision !== "string" ||
      !/^[0-9a-f]{40}$/.test(release.revision)
    ) {
      fail(
        `powerio-releases.json does not map v${powerioVersion} to its release commit`,
      );
    }
    powerioRevision = release.revision;
    powerioReleaseTag = release.tag;
  } else {
    fail(
      "PowerIO must resolve from one exact Git revision or one checksummed registry release",
    );
  }
  return {
    powerio_revision: powerioRevision,
    powerio_version: powerioVersion,
    powerio_source: powerioSource,
    powerio_release_tag: powerioReleaseTag,
    powerio_lock_checksums: powerioLockChecksums,
  };
}

function exactRevisions(repoRoot) {
  const tellegenRevision = command("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
  }).trim();
  const tellegenTree = command("git", ["rev-parse", "HEAD^{tree}"], {
    cwd: repoRoot,
  }).trim();
  const metadata = parseJson(
    command("cargo", ["metadata", "--locked", "--format-version", "1"], {
      cwd: repoRoot,
    }),
    "cargo metadata",
  );
  const packages = metadata.packages.filter((pkg) =>
    POWERIO_PACKAGES.has(pkg.name),
  );
  const releases = parseJson(
    readFileSync(
      resolve(repoRoot, "evidence/webmcp/powerio-releases.json"),
      "utf8",
    ),
    "PowerIO release revisions",
  );
  return {
    tellegen_revision: tellegenRevision,
    tellegen_tree: tellegenTree,
    ...resolvePowerioProvenance(packages, releases),
  };
}

function trackedTreeIsClean(repoRoot) {
  const status = spawnSync(
    "git",
    ["status", "--porcelain=v1", "--untracked-files=all"],
    {
      cwd: repoRoot,
      encoding: "utf8",
    },
  );
  if (status.error || status.status !== 0)
    fail("could not inspect the Git worktree");
  const relevant = status.stdout
    .split("\n")
    .filter(Boolean)
    .filter(
      (line) =>
        !(
          line.startsWith("?? evidence/webmcp/results/") &&
          line.endsWith(".json")
        ),
    );
  return relevant.length === 0;
}

function injectSourceDigest(module, digest, byteLength) {
  if (!Array.isArray(module.sources) || module.sources.length !== 1) {
    fail("MATPOWER conversion did not retain exactly one source descriptor");
  }
  const descriptor = module.sources[0];
  if (descriptor.byte_length !== byteLength)
    fail("PowerIO source byte length changed during parse");
  descriptor.digest = { algorithm: "sha256", value: digest };
}

export async function runEvidence(
  specPath,
  outputPath,
  { allowDirty = false } = {},
) {
  const harnessDir = dirname(fileURLToPath(import.meta.url));
  const repoRoot = resolve(harnessDir, "../..");
  const absoluteSpec = resolve(repoRoot, specPath);
  const specRelativeToRoot = relative(repoRoot, absoluteSpec);
  if (
    specRelativeToRoot === "" ||
    specRelativeToRoot === ".." ||
    specRelativeToRoot.startsWith(`..${sep}`)
  ) {
    fail("spec resolves outside the repository");
  }
  const clean = trackedTreeIsClean(repoRoot);
  if (!clean && !allowDirty) {
    fail(
      "tracked files are modified; commit the implementation before generating evidence",
    );
  }

  const specBytes = await readFile(absoluteSpec);
  const spec = validateSpec(parseJson(specBytes.toString("utf8"), specPath));
  const absoluteOutput = resolve(repoRoot, outputPath);
  const expectedOutput = resolve(
    repoRoot,
    `evidence/webmcp/results/${spec.case_id}.json`,
  );
  if (!allowDirty && absoluteOutput !== expectedOutput) {
    fail(
      `clean evidence for ${spec.case_id} must be written to ${relative(repoRoot, expectedOutput)}`,
    );
  }
  const absoluteSource = resolve(repoRoot, spec.source);
  const sourceRelative = relative(repoRoot, absoluteSource);
  if (
    sourceRelative === "" ||
    sourceRelative === ".." ||
    sourceRelative.startsWith(`..${sep}`)
  ) {
    fail("source resolves outside the repository");
  }
  const sourceBytes = await readFile(absoluteSource);
  const sourceDigest = sha256(sourceBytes);
  const revisions = exactRevisions(repoRoot);
  const cargoLockDigest = sha256(
    await readFile(resolve(repoRoot, "Cargo.lock")),
  );

  const moduleText = command(
    "cargo",
    [
      "run",
      "--quiet",
      "--locked",
      "--release",
      "-p",
      "benchmarks",
      "--bin",
      "challenge-module",
      "--",
      absoluteSource,
      spec.source,
    ],
    { cwd: repoRoot },
  );
  const module = parseJson(moduleText, "PowerIO module");
  injectSourceDigest(module, sourceDigest, sourceBytes.byteLength);

  const outputRelative = relative(repoRoot, resolve(repoRoot, outputPath))
    .split(sep)
    .join("/");
  const specRelative = relative(repoRoot, absoluteSpec).split(sep).join("/");
  const common = {
    schema: RESULT_SCHEMA,
    case_id: spec.case_id,
    reproducible: clean && !allowDirty,
    invocation: `node evidence/webmcp/run.mjs ${specRelative} ${outputRelative}`,
    software: {
      ...revisions,
      cargo_lock_sha256: cargoLockDigest,
    },
    source: {
      path: spec.source,
      byte_length: sourceBytes.byteLength,
      sha256: sourceDigest,
    },
    spec_sha256: sha256(specBytes),
    request: spec.request,
    expected_outcome: spec.expected_outcome ?? { kind: "success" },
  };
  const requestEnvelope = JSON.stringify({ module, spec: spec.request });
  let artifact;
  try {
    const response = validatePlanResponse(
      parseJson(
        command(
          "cargo",
          [
            "run",
            "--quiet",
            "--locked",
            "--release",
            "-p",
            "tellegen-cli",
            "--",
            "plan",
          ],
          { cwd: repoRoot, input: requestEnvelope },
        ),
        "tellegen plan response",
      ),
      spec.request,
    );
    if (spec.expected_outcome?.kind === "failure") {
      fail(
        `planning succeeded but the spec expected ${spec.expected_outcome.code}`,
      );
    }
    const retainedDigest = response.solution_module.sources?.[0]?.digest;
    if (
      retainedDigest?.algorithm !== "sha256" ||
      retainedDigest.value !== sourceDigest
    ) {
      fail("final solution module did not retain the PowerIO source digest");
    }
    artifact = {
      ...common,
      status: "success",
      plan: response.plan,
      solution: summarizeSolutionModule(response.solution_module),
    };
  } catch (error) {
    const failure = classifyExpectedPreparationFailure(
      spec.expected_outcome,
      error,
    );
    if (!failure) throw error;
    artifact = {
      ...common,
      status: "failure",
      failure,
    };
  }
  await mkdir(dirname(absoluteOutput), { recursive: true });
  await writeFile(absoluteOutput, `${JSON.stringify(artifact, null, 2)}\n`, {
    flag: "wx",
  });
  return artifact;
}

async function main() {
  const args = process.argv.slice(2);
  const allowDirtyIndex = args.indexOf("--allow-dirty");
  const allowDirty = allowDirtyIndex >= 0;
  if (allowDirty) args.splice(allowDirtyIndex, 1);
  if (args.length !== 2) {
    fail(
      "usage: node evidence/webmcp/run.mjs [--allow-dirty] <spec.json> <result.json>",
    );
  }
  const artifact = await runEvidence(args[0], args[1], { allowDirty });
  if (artifact.status === "success") {
    process.stdout.write(
      `${artifact.case_id}: ${artifact.plan.exact_solves} exact solves; ${artifact.plan.proposal.length} changed branches\n`,
    );
  } else {
    process.stdout.write(
      `${artifact.case_id}: expected ${artifact.failure.code}; 0 exact solves\n`,
    );
  }
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  main().catch((error) => {
    process.stderr.write(`challenge evidence: ${error.message}\n`);
    process.exitCode = 1;
  });
}
