import { cpSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const packageDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const sourceDir = join(packageDir, "src");
const distDir = join(packageDir, "dist");

mkdirSync(distDir, { recursive: true });

for (const dir of ["wasm-pkg", "wasm-sens-pkg"]) {
  cpSync(join(sourceDir, dir), join(distDir, dir), { recursive: true });
}
