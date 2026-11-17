import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import Ajv2020 from "ajv/dist/2020.js";
import standaloneCode from "ajv/dist/standalone/index.js";
const root = new URL("../../../", import.meta.url);
const schema = JSON.parse(
  readFileSync(
    new URL("packages/engine/src/generated/api.schema.json", root),
    "utf8",
  ),
);
for (const [file, expected] of Object.entries(schema["x-rust-source-files"])) {
  const actual = createHash("sha256")
    .update(readFileSync(new URL(`crates/tellegen/${file}`, root)))
    .digest("hex");
  if (actual !== expected)
    throw new Error(
      `API schema is stale for ${file}; regenerate with cargo run -p tellegen --example api_schema --features schema,conic,mc-pf`,
    );
}
const ajv = new Ajv2020({
  strict: false,
  strictNumbers: true,
  validateFormats: false,
  inlineRefs: false,
  code: { source: true, esm: true, lines: true },
});
ajv.addSchema(schema, "tellegen-api");
const exports = Object.fromEntries(
  Object.keys(schema.properties).map((name) => [
    name,
    `tellegen-api#/properties/${name}`,
  ]),
);
const code =
  "// @ts-nocheck\n// Generated from Rust API schemas by generate-api-validators.mjs.\n" +
  standaloneCode(ajv, exports).replaceAll(
    'require("ajv/dist/runtime/ucs2length").default',
    "(text => { let size = 0; for (const character of text) size++; return size; })",
  );
if (/require\(/.test(code))
  throw new Error(
    `Standalone helper: ${[...code.matchAll(/require\([^)]*\)/g)].map((m) => m[0]).join(", ")}`,
  );
function output(path, content) {
  const url = new URL(path, root);
  if (process.argv.includes("--check")) {
    if (readFileSync(url, "utf8") !== content)
      throw new Error(`${path} is stale; run npm run types`);
  } else writeFileSync(url, content);
}
output("packages/engine/src/generated/api-validators.ts", code);
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
