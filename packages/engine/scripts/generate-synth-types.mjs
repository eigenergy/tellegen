// TypeScript types for synthetic grid generation, generated from the
// JSON Schema that `crates/tellegen-synth/examples/synth_schema.rs` emits.
// `--check` fails when the schema is older than the Rust records or the
// TypeScript is older than the schema.
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const generatedDir = join(root, "packages/engine/src/generated");
const schema = JSON.parse(readFileSync(join(generatedDir, "synth.schema.json"), "utf8"));
const sources = ["request.rs", "report.rs", "error.rs"];
const hash = createHash("sha256");
for (const source of sources) hash.update(readFileSync(join(root, "crates/tellegen-synth/src", source)));
if (hash.digest("hex") !== schema["x-rust-source-sha256"]) {
  throw new Error(
    "Synth schema is stale. Run cargo run -p tellegen-synth --example synth_schema --features schema > packages/engine/src/generated/synth.schema.json",
  );
}

// A field's Rust doc comment as JSDoc, so editors show it on hover.
function doc(node) {
  const text = typeof node === "object" ? node.description?.replace(/\s+/g, " ").trim().replaceAll("*/", "*\\/") : undefined;
  return text ? `/** ${text} */ ` : "";
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
      // A fixed-length array (a Rust `[f64; 2]` range) is a tuple.
      if (node.items && node.minItems !== undefined && node.minItems === node.maxItems && node.minItems <= 4) {
        return `[${Array(node.minItems).fill(type(node.items)).join(", ")}]`;
      }
      return `Array<${type(node.items ?? true)}>`;
    }
    case "object": {
      const fields = Object.entries(node.properties ?? {}).map(
        ([key, value]) => `${doc(value)}${JSON.stringify(key)}${node.required?.includes(key) ? "" : "?"}: ${type(value)};`,
      );
      if (node.additionalProperties && node.additionalProperties !== false) fields.push(`[key: string]: ${type(node.additionalProperties)};`);
      return `{ ${fields.join(" ")} }`;
    }
    default:
      if (Object.keys(node).every((key) => ["description", "title", "default"].includes(key))) return "unknown";
      throw new Error(`Unsupported synth schema node: ${JSON.stringify(node)}`);
  }
}

const definitions = Object.entries(schema.$defs).map(([name, definition]) => `export type ${name} = ${type(definition)};`);
const generated = `// Generated from Rust by synth_schema and generate-synth-types.mjs.\n// Source SHA256: ${schema["x-rust-source-sha256"]}\n\n${definitions.join("\n\n")}\n`;
const typesOutput = join(generatedDir, "synth-types.ts");
const schemaOutput = join(generatedDir, "synth-schema.ts");
const schemaSource = `// Generated from the Rust synthetic generation types.\nexport const synthSchema: Record<string, unknown> = ${JSON.stringify(schema)};\n`;
if (process.argv.includes("--check")) {
  if (readFileSync(typesOutput, "utf8") !== generated) throw new Error("Synth TypeScript types are stale; run npm run types");
  if (readFileSync(schemaOutput, "utf8") !== schemaSource) throw new Error("Synth runtime schema is stale; run npm run types");
} else {
  writeFileSync(typesOutput, generated);
  writeFileSync(schemaOutput, schemaSource);
}
