import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { gzipSync } from "node:zlib";

const packageDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const exampleDir = join(packageDir, "..", "..", "examples", "browser-minimal");
const distDir = join(packageDir, "dist");
// The main entry's static import graph, gzipped as one file. The generated
// validators load on first use and are not part of it.
const MAIN_ENTRY_GZIP_BUDGET = 80_000;

const result = spawnSync("npm", ["run", "build"], {
  cwd: exampleDir,
  stdio: "inherit",
});
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);

/** Every module reached from `entry` through static imports and re-exports;
 * `import()` expressions are not followed. tsc writes each import or
 * re-export as a statement at the start of a line, ending in a semicolon. */
const STATIC_IMPORT =
  /^(?:import|export)\b[^;]*?\bfrom\s*["']([^"']+)["']|^import\s*["']([^"']+)["']/gm;
function staticGraph(entry) {
  const seen = new Set();
  const queue = [entry];
  while (queue.length) {
    const file = queue.pop();
    if (seen.has(file)) continue;
    seen.add(file);
    for (const match of readFileSync(file, "utf8").matchAll(STATIC_IMPORT)) {
      const specifier = match[1] ?? match[2];
      if (specifier.startsWith(".")) queue.push(join(dirname(file), specifier));
    }
  }
  return [...seen].sort();
}

const graph = staticGraph(join(distDir, "index.js"));
const eager = graph.filter((file) =>
  /generated[\\/](validators[\\/]|validator-eager|api-validators)/.test(
    relative(distDir, file),
  ),
);
if (eager.length)
  throw new Error(
    `The main entry statically imports generated validators: ${eager.map((file) => relative(distDir, file)).join(", ")}`,
  );
const gzip = gzipSync(Buffer.concat(graph.map((file) => readFileSync(file))), {
  level: 9,
}).length;
console.log(
  `Main entry static graph: ${graph.length} modules, ${gzip} bytes gzipped (budget ${MAIN_ENTRY_GZIP_BUDGET}).`,
);
if (gzip > MAIN_ENTRY_GZIP_BUDGET)
  throw new Error(
    `The main entry's static graph is ${gzip} bytes gzipped, over its ${MAIN_ENTRY_GZIP_BUDGET} byte budget`,
  );

// Validation still works through the lazy and the synchronous entries.
const engine = await import(pathToFileURL(join(distDir, "index.js")).href);
const sync = await import(pathToFileURL(join(distDir, "validators.js")).href);
const expectRejects = async (promise, text) => {
  const outcome = await promise.then(
    () => null,
    (error) => error,
  );
  if (!(outcome instanceof Error) || !outcome.message.includes(text))
    throw new Error(`Expected a rejection containing "${text}"`);
};
if (!Array.isArray(await engine.parseApiResult("capabilities", "[]")))
  throw new Error("parseApiResult did not return the validated capabilities");
await expectRejects(
  engine.parseApiResult("capabilities", "{}"),
  "Invalid engine capabilities result",
);
await engine.preloadValidators(["solve", "http_cases"]);
if (!Array.isArray(sync.parseApiResultSync("capabilities", "[]")))
  throw new Error(
    "parseApiResultSync did not return the validated capabilities",
  );
let syncError = null;
try {
  sync.parseApiResultSync("capabilities", "{}");
} catch (error) {
  syncError = error;
}
if (!(syncError instanceof Error))
  throw new Error("parseApiResultSync accepted an invalid result");
console.log("Lazy and synchronous result validation work from the build.");
