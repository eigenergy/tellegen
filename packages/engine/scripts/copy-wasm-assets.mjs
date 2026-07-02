import { cpSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const packageDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const sourceDir = join(packageDir, "src");
const distDir = join(packageDir, "dist");

mkdirSync(distDir, { recursive: true });

const targetDir = join(distDir, "wasm-pkg");
cpSync(join(sourceDir, "wasm-pkg"), targetDir, { recursive: true });
writeFileSync(
  join(targetDir, ".npmignore"),
  "# Include wasm-pack output in the @tellegen/engine tarball.\n",
);
