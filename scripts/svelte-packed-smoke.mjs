import { spawnSync } from "node:child_process";
import {
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, isAbsolute, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const tmpRoot = await mkdtemp(join(tmpdir(), "tellegen-svelte-packed-"));
const packDir = join(tmpRoot, "packages");
const consumerDir = join(tmpRoot, "consumer");
const npmCacheDir = join(tmpRoot, "npm-cache");
const keepTmp = process.env.TELLEGEN_KEEP_SMOKE_TMP === "1";

function run(command, args, options = {}) {
  const cwd = options.cwd ?? repoRoot;
  console.log(`$ ${command} ${args.join(" ")}`);
  const result = spawnSync(command, args, {
    cwd,
    encoding: "utf8",
    stdio: options.capture ? ["ignore", "pipe", "inherit"] : "inherit",
    env: {
      ...process.env,
      CI: process.env.CI ?? "1",
      npm_config_cache: npmCacheDir,
      npm_config_fetch_retries: "0",
      npm_config_fetch_timeout: "15000",
    },
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed with ${result.status}`,
    );
  }
  return result.stdout ?? "";
}

function parsePackedPath(stdout) {
  const [entry] = JSON.parse(stdout);
  if (!entry?.filename) throw new Error("npm pack did not report a tarball");
  return isAbsolute(entry.filename)
    ? entry.filename
    : join(packDir, entry.filename);
}

function packageFileSpec(path) {
  return `file:${relative(consumerDir, path).split(sep).join("/")}`;
}

async function packWorkspace(workspace) {
  const stdout = run(
    "npm",
    ["--workspace", workspace, "pack", "--pack-destination", packDir, "--json"],
    { capture: true },
  );
  return parsePackedPath(stdout);
}

async function listFiles(root) {
  const entries = await readdir(root, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await listFiles(path)));
    } else if (entry.isFile()) {
      files.push(path);
    }
  }
  return files;
}

async function assertSveltePeer() {
  const sveltePackage = JSON.parse(
    await readFile(
      join(consumerDir, "node_modules/svelte/package.json"),
      "utf8",
    ),
  );
  const [major, minor] = String(sveltePackage.version)
    .split(".")
    .map((part) => Number(part));
  if (major !== 5 || minor < 30) {
    throw new Error(
      `consumer has svelte ${sveltePackage.version}; expected >=5.30 <6`,
    );
  }
}

async function assertTypeScriptSupported() {
  const [typescript, svelteCheck] = await Promise.all(
    ["typescript", "svelte-check"].map(async (name) =>
      JSON.parse(
        await readFile(
          join(consumerDir, "node_modules", name, "package.json"),
          "utf8",
        ),
      ),
    ),
  );
  // svelte-check loads the TypeScript *JavaScript* compiler API through
  // `require("typescript")`. TypeScript 7 is the native port: its only root
  // export is a version stub, so a consumer that resolves it dies with an
  // opaque TypeError instead of a type error. The install below passes
  // `--legacy-peer-deps`, which means npm will not quietly add a supported
  // copy to rescue it, so assert on the version this repository propagated out
  // of packages/svelte.
  const supported = String(svelteCheck.peerDependencies?.typescript ?? "");
  const major = String(typescript.version).split(".")[0];
  if (!supported.includes(`^${major}.`)) {
    throw new Error(
      `consumer resolved typescript ${typescript.version}, which svelte-check ` +
        `${svelteCheck.version} does not support (peer range "${supported}"). ` +
        `Change the typescript devDependency in packages/svelte.`,
    );
  }
}

async function writeConsumer(engineTarball, svelteTarball) {
  const sveltePackage = JSON.parse(
    await readFile(join(repoRoot, "packages/svelte/package.json"), "utf8"),
  );
  const devDependencies = Object.fromEntries(
    [
      "@sveltejs/vite-plugin-svelte",
      "svelte",
      "svelte-check",
      "typescript",
      "vite",
    ].map((name) => [name, sveltePackage.devDependencies[name]]),
  );

  await mkdir(join(consumerDir, "src"), { recursive: true });
  await writeFile(
    join(consumerDir, "package.json"),
    `${JSON.stringify(
      {
        name: "tellegen-svelte-packed-smoke",
        private: true,
        type: "module",
        scripts: {
          build: "svelte-check --tsconfig ./tsconfig.json && vite build",
        },
        dependencies: {
          "@tellegen/engine": packageFileSpec(engineTarball),
          "@tellegen/svelte": packageFileSpec(svelteTarball),
        },
        devDependencies,
      },
      null,
      2,
    )}\n`,
  );
  await writeFile(
    join(consumerDir, "svelte.config.js"),
    `import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

export default {
  preprocess: vitePreprocess()
};
`,
  );
  await writeFile(
    join(consumerDir, "vite.config.ts"),
    `import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [svelte()],
  build: {
    chunkSizeWarningLimit: 1200
  }
});
`,
  );
  await writeFile(
    join(consumerDir, "tsconfig.json"),
    `${JSON.stringify(
      {
        compilerOptions: {
          allowArbitraryExtensions: true,
          esModuleInterop: true,
          forceConsistentCasingInFileNames: true,
          isolatedModules: true,
          module: "ESNext",
          moduleDetection: "force",
          moduleResolution: "bundler",
          noEmit: true,
          skipLibCheck: true,
          strict: true,
          target: "ES2022",
          verbatimModuleSyntax: true,
        },
        include: ["src/**/*"],
      },
      null,
      2,
    )}\n`,
  );
  await writeFile(
    join(consumerDir, "index.html"),
    `<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>tellegen packed package smoke</title>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
`,
  );
  await writeFile(
    join(consumerDir, "src/App.svelte"),
    `<script lang="ts">
  import {
    TellegenViewer,
    classifyJson,
    formatOf,
    ingestCase,
    ingestJsonDrop
  } from "@tellegen/svelte";
  import type {
    IngestedCase,
    IngestedJsonDrop,
    JsonDropClassification
  } from "@tellegen/svelte";
  import type { TellegenMapProps } from "@tellegen/svelte/map";

  const mapProps: Partial<TellegenMapProps> = {};
  const matpowerFormat = formatOf("case14.m");
  if (matpowerFormat !== "m") throw new Error("format export failed");
  const classify: (bytes: Uint8Array) => Promise<JsonDropClassification> = classifyJson;
  const ingest: (bytes: Uint8Array, format: string) => Promise<IngestedCase> = ingestCase;
  const route: (bytes: Uint8Array) => Promise<IngestedJsonDrop> = ingestJsonDrop;
  void classify;
  void ingest;
  void route;
</script>

<TellegenViewer
  {...mapProps}
  loadDefaultCases={false}
  showFooter={false}
  docsHref="https://eigenergy.github.io/tellegen/"
  orgHref="https://github.com/eigenergy/tellegen"
  orgLabel="tellegen"
/>
`,
  );
  await writeFile(
    join(consumerDir, "src/main.ts"),
    `import { mount } from "svelte";
import { AppFooter } from "@tellegen/svelte/components";
import App from "./App.svelte";
import "@tellegen/svelte/styles.css";

void AppFooter;

mount(App, {
  target: document.getElementById("app")!
});
`,
  );
  // The consumer imports a stylesheet for its side effect, exactly as the demo
  // does. Without the ambient declaration that is TS2882 under a TypeScript
  // that defaults `noUncheckedSideEffectImports` on, so ship it here too and
  // keep this smoke test an honest model of a downstream project.
  await writeFile(
    join(consumerDir, "src/vite-env.d.ts"),
    '/// <reference types="vite/client" />\n',
  );
}

async function assertTarballInstall() {
  const svelteInstall = join(consumerDir, "node_modules/@tellegen/svelte");
  const engineInstall = join(consumerDir, "node_modules/@tellegen/engine");
  for (const path of [svelteInstall, engineInstall]) {
    const stat = await lstat(path);
    if (stat.isSymbolicLink()) {
      throw new Error(`${path} is a symlink, not an installed package tarball`);
    }
  }
}

async function assertBuildOutput() {
  const distDir = join(consumerDir, "dist");
  const files = (await listFiles(distDir)).map((file) =>
    relative(distDir, file).split(sep).join("/"),
  );
  const wasmFiles = files.filter((file) => file.endsWith(".wasm"));

  if (!files.some((file) => file.endsWith(".css"))) {
    throw new Error("consumer build did not emit a CSS asset");
  }
  if (!wasmFiles.some((file) => /tellegen_bg.*\.wasm$/.test(file))) {
    throw new Error("consumer build did not emit the engine wasm asset");
  }
  if (!files.some((file) => /worker/.test(file) && file.endsWith(".js"))) {
    throw new Error("consumer build did not emit the engine worker chunk");
  }

  console.log(`packed Svelte consumer emitted ${wasmFiles.length} wasm assets`);
}

try {
  await mkdir(packDir, { recursive: true });
  const engineTarball = await packWorkspace("@tellegen/engine");
  const svelteTarball = await packWorkspace("@tellegen/svelte");
  await writeConsumer(engineTarball, svelteTarball);
  run(
    "npm",
    [
      "install",
      "--no-audit",
      "--no-fund",
      "--ignore-scripts",
      "--legacy-peer-deps",
      "--package-lock=false",
    ],
    { cwd: consumerDir },
  );
  await assertSveltePeer();
  await assertTypeScriptSupported();
  await assertTarballInstall();
  run("npm", ["run", "build"], { cwd: consumerDir });
  await assertBuildOutput();
} finally {
  if (keepTmp) {
    console.log(`kept smoke test workspace at ${tmpRoot}`);
  } else {
    await rm(tmpRoot, { recursive: true, force: true });
  }
}
