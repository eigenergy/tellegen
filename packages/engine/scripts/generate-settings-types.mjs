import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
const root = new URL("../../../", import.meta.url);
const artifact = JSON.parse(
  readFileSync(
    new URL("packages/engine/src/generated/settings.schema.json", root),
    "utf8",
  ),
);
function type(node) {
  if (node === false) return "never";
  if (node === true || !node) return "unknown";
  if (node.$ref) return node.$ref.split("/").at(-1);
  if (node.const !== undefined) return JSON.stringify(node.const);
  if (node.enum) return node.enum.map((x) => JSON.stringify(x)).join(" | ");
  const union = node.oneOf ?? node.anyOf;
  if (union) return union.map((x) => `(${type(x)})`).join(" | ");
  if (Array.isArray(node.type))
    return node.type.map((x) => type({ ...node, type: x })).join(" | ");
  switch (node.type) {
    case "null":
      return "null";
    case "boolean":
      return "boolean";
    case "string":
      return "string";
    case "integer":
    case "number":
      return "number";
    case "array":
      return `Array<${type(node.items)}>`;
    case "object": {
      const fields = Object.entries(node.properties ?? {}).map(
        ([key, value]) =>
          `${JSON.stringify(key)}${node.required?.includes(key) ? "" : "?"}: ${type(value)};`,
      );
      if (node.additionalProperties && node.additionalProperties !== false)
        fields.push(`[key:string]:${type(node.additionalProperties)};`);
      return `{${fields.join("")}}`;
    }
    default:
      return "unknown";
  }
}
const defs = Object.entries(artifact.schema.$defs)
  .map(([key, value]) => `export type ${key} = ${type(value)};`)
  .join("\n");
const text = `// Generated from Rust settings_schema; do not edit.\n${defs}\nexport const settingsCatalog = ${JSON.stringify(artifact.catalog)} as const;\n`;
const output = new URL("packages/engine/src/generated/settings-types.ts", root);
if (process.argv.includes("--check")) {
  if (readFileSync(output, "utf8") !== text)
    throw new Error(`Stale generated settings types: ${fileURLToPath(output)}`);
} else writeFileSync(output, text);
