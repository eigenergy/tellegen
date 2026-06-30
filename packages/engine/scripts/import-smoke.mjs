import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const packageDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const exampleDir = join(packageDir, "..", "..", "examples", "browser-minimal");

const result = spawnSync("npm", ["run", "build"], {
  cwd: exampleDir,
  stdio: "inherit",
});

if (result.error) throw result.error;
process.exit(result.status ?? 1);
