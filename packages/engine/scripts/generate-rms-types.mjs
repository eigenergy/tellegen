// TypeScript types of the RMS records the browser engine exchanges, from the
// JSON Schema the Rust types derive (`cargo run -p tellegen --example
// rms_schema --features schema,rms-experimental`). `--check` fails when the
// types are stale against the schema; `scripts/schemas.sh --check` compares
// the schema with the Rust records.
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const output = join(root, "packages/engine/src/generated/rms-types.ts");
const schemaPath = join(root, "packages/engine/src/generated/rms.schema.json");
const schema = JSON.parse(readFileSync(schemaPath, "utf8"));

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
  if (union) {
    const parts = union.map((x) => `(${type(x)})`);
    // A tagged variant's own properties sit beside its oneOf.
    if (node.properties) return `(${parts.join(" | ")}) & ${type({ ...node, oneOf: undefined, anyOf: undefined, type: "object" })}`;
    return parts.join(" | ");
  }
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
      const fields = Object.entries(node.properties ?? {}).map(
        ([key, value]) => `${JSON.stringify(key)}${node.required?.includes(key) ? "" : "?"}: ${type(value)};`,
      );
      if (node.additionalProperties && node.additionalProperties !== false) fields.push(`[key: string]: ${type(node.additionalProperties)};`);
      return `{ ${fields.join(" ")} }`;
    }
    default:
      if (Object.keys(node).every((key) => ["description", "title", "default"].includes(key))) return "unknown";
      throw new Error(`Unsupported RMS schema node: ${JSON.stringify(node)}`);
  }
}

const definitions = Object.entries(schema.$defs).map(([name, definition]) => {
  const doc = definition.description ? `/** ${definition.description.replace(/\*\//g, "*\\/").split("\n").join("\n * ")} */\n` : "";
  return `${doc}export type ${name} = ${type(definition)};`;
});
const generated = `// Generated from Rust by rms_schema and generate-rms-types.mjs.\n\n${definitions.join("\n\n")}\n`;
if (process.argv.includes("--check")) {
  if (readFileSync(output, "utf8") !== generated) throw new Error("RMS TypeScript types are stale; run npm run types");
} else writeFileSync(output, generated);
