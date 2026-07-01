import { spawnSync } from "node:child_process";
import {
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import {
  dirname,
  isAbsolute,
  join,
  relative,
  sep,
} from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const repoRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const tmpRoot = await mkdtemp(join(tmpdir(), "tellegen-svelte-packed-"));
const packDir = join(tmpRoot, "packages");
const consumerDir = join(tmpRoot, "consumer");
const npmCacheDir = join(tmpRoot, "npm-cache");
const keepTmp = process.env.TELLEGEN_KEEP_SMOKE_TMP === "1";
const rootNodeModules = join(repoRoot, "node_modules");

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
    throw new Error(`${command} ${args.join(" ")} failed with ${result.status}`);
  }
  return result.stdout ?? "";
}

function parsePackedPath(stdout) {
  const [entry] = JSON.parse(stdout);
  if (!entry?.filename) throw new Error("npm pack did not report a tarball");
  return isAbsolute(entry.filename) ? entry.filename : join(packDir, entry.filename);
}

async function packWorkspace(workspace) {
  const stdout = run(
    "npm",
    [
      "--workspace",
      workspace,
      "pack",
      "--pack-destination",
      packDir,
      "--json",
    ],
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

async function exists(path) {
  try {
    await lstat(path);
    return true;
  } catch (e) {
    if (e?.code === "ENOENT") return false;
    throw e;
  }
}

async function linkInstalledDependencies() {
  if (!(await exists(rootNodeModules))) {
    throw new Error("missing node_modules; run npm ci before the packed smoke");
  }
  const consumerNodeModules = join(consumerDir, "node_modules");
  await mkdir(consumerNodeModules, { recursive: true });

  for (const entry of await readdir(rootNodeModules, { withFileTypes: true })) {
    if (entry.name === "@tellegen" || entry.name.startsWith(".")) continue;
    const source = join(rootNodeModules, entry.name);
    const target = join(consumerNodeModules, entry.name);
    if (entry.name.startsWith("@") && entry.isDirectory()) {
      await mkdir(target, { recursive: true });
      for (const scoped of await readdir(source, { withFileTypes: true })) {
        await symlink(
          join(source, scoped.name),
          join(target, scoped.name),
          scoped.isDirectory() ? "dir" : "file",
        );
      }
    } else {
      await symlink(source, target, entry.isDirectory() ? "dir" : "file");
    }
  }
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

async function writeConsumer() {
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
  import { TellegenViewer, formatOf } from "@tellegen/svelte";
  import type { TellegenMapProps } from "@tellegen/svelte/map";

  const mapProps: Partial<TellegenMapProps> = {};
  const matpowerFormat = formatOf("case14.m");
  if (matpowerFormat !== "m") throw new Error("format export failed");
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
    throw new Error("consumer build did not emit the core wasm asset");
  }
  if (!wasmFiles.some((file) => /tellegen_sens_bg.*\.wasm$/.test(file))) {
    throw new Error("consumer build did not emit the sensitivity wasm asset");
  }

  console.log(`packed Svelte consumer emitted ${wasmFiles.length} wasm assets`);
}

try {
  await mkdir(packDir, { recursive: true });
  const engineTarball = await packWorkspace("@tellegen/engine");
  const svelteTarball = await packWorkspace("@tellegen/svelte");
  await writeConsumer();
  await linkInstalledDependencies();
  await assertSveltePeer();
  run(
    "npm",
    [
      "install",
      "--no-audit",
      "--no-fund",
      "--ignore-scripts",
      "--legacy-peer-deps",
      "--package-lock=false",
      "--no-save",
      pathToFileURL(engineTarball).href,
      pathToFileURL(svelteTarball).href,
    ],
    { cwd: consumerDir },
  );
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
