import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const output = join(root, "packages/engine/src/generated/study-contracts.ts");
const schema = JSON.parse(readFileSync(join(root, "packages/engine/src/generated/study.schema.json"), "utf8"));
const sources = ["document.rs", "objective.rs", "exploration.rs", "study_ops.rs", "api.rs", "sens/contract.rs", "solve.rs"];
const hash = createHash("sha256");
for (const source of sources) hash.update(readFileSync(join(root, "crates/tellegen/src", source)));
if (hash.digest("hex") !== schema["x-rust-source-sha256"]) {
  throw new Error("Study schema is stale. Run cargo run -p tellegen --example study_contract --features schema > packages/engine/src/generated/study.schema.json");
}

function type(node) {
  if (node === true) return "unknown";
  if (node === false) return "never";
  if (node.$ref) {
    if (!node.$ref.startsWith("#/$defs/")) throw new Error(`Unsupported schema reference ${node.$ref}`);
    return node.$ref.slice(8);
  }
  if ("const" in node) return JSON.stringify(node.const);
  if (node.enum) return node.enum.map((x) => JSON.stringify(x)).join(" | ");
  const union = node.oneOf ?? node.anyOf;
  if (union) return union.map((x) => `(${type(x)})`).join(" | ");
  if (node.allOf) return node.allOf.map((x) => `(${type(x)})`).join(" & ");
  if (Array.isArray(node.type)) return node.type.map((t) => type({ ...node, type: t })).join(" | ");
  switch (node.type) {
    case "null": return "null";
    case "boolean": return "boolean";
    case "integer": case "number": return "number";
    case "string": return "string";
    case "array": {
      if (node.prefixItems) return `[${node.prefixItems.map(type).join(", ")}]`;
      return `Array<${type(node.items ?? true)}>`;
    }
    case "object": {
      const fields = Object.entries(node.properties ?? {}).map(([key, value]) => `${JSON.stringify(key)}${node.required?.includes(key) ? "" : "?"}: ${type(value)};`);
      if (node.additionalProperties && node.additionalProperties !== false) fields.push(`[key: string]: ${type(node.additionalProperties)};`);
      return `{ ${fields.join(" ")} }`;
    }
    default:
      if (Object.keys(node).every((key) => ["description", "title", "default"].includes(key))) return "unknown";
      throw new Error(`Unsupported Study schema node: ${JSON.stringify(node)}`);
  }
}

const definitions = Object.entries(schema.$defs).map(([name, definition]) => `export type ${name} = ${type(definition)};`);
const generated = `// Generated from Rust by study_contract and generate-study-contracts.mjs.\n// Source SHA256: ${schema["x-rust-source-sha256"]}\n\n${definitions.join("\n\n")}\n`;
if (process.argv.includes("--check")) {
  if (readFileSync(output, "utf8") !== generated) throw new Error("Study TypeScript contracts are stale; run npm run contracts");
} else writeFileSync(output, generated);

const schemaOutput = new URL('../src/generated/study-schema.ts', import.meta.url);
const schemaSource = `// Generated from the Rust Study contract.\nexport const studySchema: Record<string, unknown> = ${JSON.stringify(schema)};\n`;
if (process.argv.includes('--check')) {
  if (readFileSync(schemaOutput, 'utf8') !== schemaSource) throw new Error('Study runtime schema is stale; run npm run contracts');
} else writeFileSync(schemaOutput, schemaSource);
