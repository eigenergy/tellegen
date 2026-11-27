import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import Ajv2020 from "ajv/dist/2020.js";
import standaloneCode from "ajv/dist/standalone/index.js";
const root = new URL("../../../", import.meta.url);
const schema = JSON.parse(
  readFileSync(
    new URL("packages/engine/src/generated/api.schema.json", root),
    "utf8",
  ),
);
const ajv = new Ajv2020({
  strict: false,
  strictNumbers: true,
  validateFormats: false,
  inlineRefs: false,
  code: { source: true, esm: true, lines: false },
});
ajv.addSchema(schema, "tellegen-api");
const httpSchema = JSON.parse(
  readFileSync(
    new URL("packages/engine/src/generated/http.schema.json", root),
    "utf8",
  ),
);
// Older display responses lack solver context. Validate it when present while
// continuing to accept those existing servers and saved display results.
for (const name of ["SolutionPayload", "StreamSolution"])
  if (httpSchema.$defs[name]?.required)
    httpSchema.$defs[name].required = httpSchema.$defs[name].required.filter(
      (field) => !["formulation", "status"].includes(field),
    );
// Existing HTTP clients also accept these fields omitted by older servers.
// Their generated definitions still validate every field that is supplied.
const optionalLegacyFields = {
  CaseSummary: [
    "model",
    "unavailable_reason",
    "n_analysis_bus",
    "n_analysis_branch",
  ],
  NetworkPayload: ["equipment", "model_details", "substations"],
  NetworkBus: ["name", "area", "zone", "editable", "base_kv", "substation"],
  NetworkBranch: ["editable", "transformer"],
  Equipment: [
    "three_winding",
    "devices",
    "dc_topology",
    "dc_model",
    "dc_model_unavailable",
    "import_notes",
  ],
  Transformer: ["tap_changers"],
  Metadata: ["description", "source_url", "related_case_id", "pf_options"],
};
optionalLegacyFields.NetworkOutput = optionalLegacyFields.NetworkPayload;
optionalLegacyFields.EquipmentOutput = [...optionalLegacyFields.Equipment, "node_breaker"];
for (const [name, fields] of Object.entries(optionalLegacyFields))
  if (httpSchema.$defs[name]?.required)
    httpSchema.$defs[name].required = httpSchema.$defs[name].required.filter(
      (field) => !fields.includes(field),
    );
ajv.addSchema(httpSchema, "tellegen-http");
// These schemas already generate the public TypeScript records. Reuse those
// same records for runtime results rather than maintaining another model.
const extraResults = [];
for (const [family, names] of [
  ["synth", ["report", "step"]],
  ["rms", ["inspection"]],
]) {
  const extraSchema = JSON.parse(
    readFileSync(
      new URL(`packages/engine/src/generated/${family}.schema.json`, root),
      "utf8",
    ),
  );
  ajv.addSchema(extraSchema, `tellegen-${family}`);
  for (const name of names)
    extraResults.push([
      `${family}_${name}`,
      `tellegen-${family}#/properties/${name}`,
    ]);
}
function adapterResults(family, prefix) {
  const adapter = JSON.parse(
    readFileSync(
      new URL(
        `packages/engine/src/generated/${family}-adapter.schema.json`,
        root,
      ),
      "utf8",
    ),
  );
  // Drawing detail was added after the original browser response format.
  // Omission stays compatible; supplied detail must satisfy its Rust record.
  if (family === "wasm" && adapter.$defs.EquipmentOutput?.required)
    adapter.$defs.EquipmentOutput.required = adapter.$defs.EquipmentOutput.required.filter(field => field !== "node_breaker");
  ajv.addSchema(adapter, `tellegen-${family}-adapter`);
  for (const name of Object.keys(adapter.properties))
    extraResults.push([
      `${prefix}${name}`,
      `tellegen-${family}-adapter#/properties/${name}`,
    ]);
}
adapterResults("rms", "rms_");
adapterResults("wasm", "");
const exports = Object.fromEntries([
  ...extraResults,
  ...Object.keys(schema.properties).map((name) => [
    name,
    `tellegen-api#/properties/${name}`,
  ]),
  ...Object.keys(httpSchema.properties).map((name) => [
    name,
    `tellegen-http#/properties/${name}`,
  ]),
]);
// Each family is one module, loaded the first time one of its results is
// validated. Results in a family share most of their $defs, so grouping them
// emits that code once.
const familyRules = [
  ["synth", (name) => name.startsWith("synth_")],
  ["rms", (name) => name.startsWith("rms_")],
  ["mc", (name) => name.startsWith("mc_")],
  ["collection", (name) => name.startsWith("collection_")],
  ["http", (name) => name.startsWith("http_")],
  [
    "ingest",
    (name) =>
      [
        "json_drop",
        "ingest_balanced",
        "ingest_balanced_view",
        "ingest_distribution",
        "classifier",
        "parsed_geo",
        "geo_report",
        "tx_layout",
        "dc_grid",
        "capabilities",
        "memory_stats",
      ].includes(name),
  ],
  ["study", () => true],
];
const families = {};
for (const name of Object.keys(exports).sort()) {
  const [family] = familyRules.find(([, member]) => member(name));
  (families[family] ??= {})[name] = exports[name];
}
function standalone(members) {
  const code =
    "// @ts-nocheck\n// Generated from Rust API schemas by generate-api-validators.mjs.\n" +
    standaloneCode(ajv, members).replaceAll(
      'require("ajv/dist/runtime/ucs2length").default',
      "(text => { let size = 0; for (const character of text) size++; return size; })",
    );
  if (/require\(/.test(code))
    throw new Error(
      `Standalone helper: ${[...code.matchAll(/require\([^)]*\)/g)].map((m) => m[0]).join(", ")}`,
    );
  return code;
}
const check = process.argv.includes("--check");
function output(path, content) {
  const url = new URL(path, root);
  if (check) {
    if (!existsSync(url) || readFileSync(url, "utf8") !== content)
      throw new Error(`${path} is stale; run npm run types`);
  } else {
    mkdirSync(new URL(".", url), { recursive: true });
    writeFileSync(url, content);
  }
}
const validatorDir = "packages/engine/src/generated/validators/";
const familyNames = Object.keys(families).sort();
const stale = existsSync(new URL(validatorDir, root))
  ? readdirSync(new URL(validatorDir, root)).filter(
      (file) => !familyNames.includes(file.replace(/\.ts$/, "")),
    )
  : [];
if (stale.length) {
  if (check)
    throw new Error(`${validatorDir}${stale[0]} is stale; run npm run types`);
  for (const file of stale) rmSync(new URL(validatorDir + file, root));
}
for (const family of familyNames)
  output(`${validatorDir}${family}.ts`, standalone(families[family]));
const resultFamilies = Object.fromEntries(
  familyNames.flatMap((family) =>
    Object.keys(families[family]).map((name) => [name, family]),
  ),
);
output(
  "packages/engine/src/generated/validator-index.ts",
  `// Generated by generate-api-validators.mjs. Maps each result to the module
// that validates it; every module is imported only when first needed.

export const resultFamilies = ${JSON.stringify(
    Object.fromEntries(Object.entries(resultFamilies).sort()),
    null,
    2,
  )} as const;

export type ResultName = keyof typeof resultFamilies;
export type ResultFamily = (typeof resultFamilies)[ResultName];

export const loadFamily: Record<ResultFamily, () => Promise<object>> = {
${familyNames.map((family) => `  ${family}: () => import("./validators/${family}.js"),`).join("\n")}
};
`,
);
output(
  "packages/engine/src/generated/validator-eager.ts",
  `// Generated by generate-api-validators.mjs. Every validator, imported
// statically, for the synchronous \`@tellegen/engine/validators\` entry.

${familyNames.map((family) => `import * as ${family} from "./validators/${family}.js";`).join("\n")}

export const validators: Record<string, unknown> = {
${familyNames.map((family) => `  ...${family},`).join("\n")}
};
`,
);
// The MCP adapter reads the same input definitions; wrapper summaries remain MCP-owned.
const names = ["edits", "sensitivity", "plan_spec"];
const definitions = {};
function visit(node) {
  if (!node || typeof node !== "object") return;
  if (node.$ref) {
    const name = node.$ref.slice("#/$defs/".length);
    if (!(name in definitions)) {
      definitions[name] = schema.$defs[name];
      visit(definitions[name]);
    }
  }
  for (const value of Object.values(node)) visit(value);
}
const properties = Object.fromEntries(
  names.map((name) => [name, schema.properties[name]]),
);
visit(properties);
output(
  "python/tellegen/mcp/_engine_schema.json",
  JSON.stringify(
    { $schema: schema.$schema, $defs: definitions, properties },
    null,
    2,
  ) + "\n",
);
