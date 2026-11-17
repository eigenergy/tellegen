#!/usr/bin/env node
// Write the two tables in docs/src/webmcp.md that come from code: what the
// Connect tab says (apps/web/src/lib/webmcp/connect-status.ts) and the tool
// error codes (packages/webmcp/src/error-codes.ts).
//
//   node --experimental-strip-types scripts/webmcp-docs.mjs          rewrite the page
//   node --experimental-strip-types scripts/webmcp-docs.mjs --check  fail if it is out of date
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { CONNECT_STATUSES } from "../apps/web/src/lib/webmcp/connect-status.ts";
import { TELLEGEN_ERROR_CODES } from "../packages/webmcp/src/error-codes.ts";

const PAGE = fileURLToPath(new URL("../docs/src/webmcp.md", import.meta.url));

const cell = (text) => text.replaceAll("|", "\\|");
const table = (head, rows) =>
  [head, head.map(() => "---"), ...rows]
    .map((row) => `| ${row.map(cell).join(" | ")} |`)
    .join("\n");

export const TABLES = {
  status: table(
    ["The Connect tab says", "What to do"],
    CONNECT_STATUSES.map((status) => [
      status.label.replace(
        "{browser} {version}+",
        "Chrome 149+ (or Edge 150+)",
      ),
      status.docs,
    ]),
  ),
  errors: table(
    ["Code", "When", "What to do next"],
    Object.entries(TELLEGEN_ERROR_CODES).map(([code, entry]) => [
      `\`${code}\``,
      entry.when,
      entry.next,
    ]),
  ),
};

/** Replace what lies between each pair of `<!-- webmcp:<name>:start/end -->` markers. */
export function render(page) {
  for (const [name, body] of Object.entries(TABLES)) {
    const start = `<!-- webmcp:${name}:start -->`;
    const end = `<!-- webmcp:${name}:end -->`;
    const from = page.indexOf(start);
    const to = page.indexOf(end);
    if (from === -1 || to === -1 || to < from)
      throw new Error(`missing ${start} … ${end}`);
    page = `${page.slice(0, from + start.length)}\n${body}\n${page.slice(to)}`;
  }
  return page;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const current = readFileSync(PAGE, "utf8");
  const next = render(current);
  if (process.argv.includes("--check")) {
    if (next !== current) {
      console.error(
        "docs/src/webmcp.md is out of date; run `npm run docs:webmcp` and commit the result",
      );
      process.exit(1);
    }
  } else if (next !== current) {
    writeFileSync(PAGE, next);
  }
}
