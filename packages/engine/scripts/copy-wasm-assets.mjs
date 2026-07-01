import { cpSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const packageDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const sourceDir = join(packageDir, "src");
const distDir = join(packageDir, "dist");

mkdirSync(distDir, { recursive: true });

for (const dir of ["wasm-pkg", "wasm-sens-pkg"]) {
  const targetDir = join(distDir, dir);
  cpSync(join(sourceDir, dir), targetDir, { recursive: true });
  writeFileSync(
    join(targetDir, ".npmignore"),
    "# Include wasm-pack output in the @tellegen/engine tarball.\n",
  );
}
