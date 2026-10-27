// Copy the synthetic generator's populated places table (Natural Earth,
// public domain) from the crate that owns it into the package source, where
// `syntheticPlacesUrl()` resolves it beside the module. The crate's copy is
// the one source of truth; this one is a build artifact.
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const packageDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const source = join(packageDir, "..", "..", "crates", "tellegen-synth", "data", "populated_places.json");
const targetDir = join(packageDir, "src", "data");
mkdirSync(targetDir, { recursive: true });
copyFileSync(source, join(targetDir, "populated_places.json"));
