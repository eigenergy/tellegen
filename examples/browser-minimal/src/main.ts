import {
  CONTRACT_VERSION,
  browserWasmTransport,
  formatOf,
  type DemandDeltas,
  type SensitivityColumn,
} from "@tellegen/engine";
import { CASE14 } from "./case14";
import "./style.css";

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) throw new Error("missing #app");
const appRoot = app;

function fmt(value: number | null | undefined, digits = 2): string {
  return value == null || !Number.isFinite(value) ? "n/a" : value.toFixed(digits);
}

function sensitivityRows(column: SensitivityColumn | null): string {
  const rows = [...(column?.values ?? [])]
    .sort((a, b) => Math.abs(b.value) - Math.abs(a.value))
    .slice(0, 5);
  return rows
    .map(
      (row) => `<tr><td>${row.bus}</td><td>${fmt(row.value, 5)}</td></tr>`,
    )
    .join("");
}

async function main() {
  appRoot.innerHTML = `<p class="status">loading wasm...</p>`;

  const format = formatOf("case14synthetic.m");
  if (!format) throw new Error("case format was not detected");

  const [parsed, capabilities] = await Promise.all([
    browserWasmTransport.ingestCase(CASE14, format),
    browserWasmTransport.capabilities(),
  ]);
  const study = await browserWasmTransport.createStudy(parsed.network_json, "dcopf");

  try {
    const editBus = 3;
    const edit: DemandDeltas = { [editBus]: 25 };
    const base = study.currentSolution();
    const preview = study.preview(edit);
    const committed = study.commit(parsed.name, edit, editBus);
    const selfSensitivity =
      committed.sensitivity?.values.find((row) => row.bus === editBus)?.value ?? null;

    appRoot.innerHTML = `
      <section class="shell">
        <header>
          <div>
            <p class="label">contract ${CONTRACT_VERSION}</p>
            <h1>${parsed.name}</h1>
          </div>
          <p class="status ok">browser wasm</p>
        </header>

        <div class="grid">
          <article>
            <h2>case</h2>
            <dl>
              <div><dt>buses</dt><dd>${parsed.n_bus}</dd></div>
              <div><dt>branches</dt><dd>${parsed.n_branch}</dd></div>
              <div><dt>generators</dt><dd>${parsed.n_gen}</dd></div>
              <div><dt>base objective</dt><dd>${fmt(base.objective)}</dd></div>
            </dl>
          </article>

          <article>
            <h2>edit</h2>
            <dl>
              <div><dt>demand bus</dt><dd>${editBus}</dd></div>
              <div><dt>delta</dt><dd>${edit[editBus]} MW</dd></div>
              <div><dt>preview objective</dt><dd>${fmt(preview.objectiveDelta)}</dd></div>
              <div><dt>committed objective</dt><dd>${fmt(committed.solution.objective)}</dd></div>
            </dl>
          </article>

          <article>
            <h2>capabilities</h2>
            <ul>
              ${capabilities
                .map(
                  (cap) =>
                    `<li>${cap.formulation}: ${cap.available ? "available" : "unavailable"}</li>`,
                )
                .join("")}
            </ul>
          </article>

          <article>
            <h2>sensitivity</h2>
            <p class="metric">bus ${editBus}: ${fmt(selfSensitivity, 5)} ${committed.sensitivity?.units ?? ""}</p>
            <table>
              <thead><tr><th>bus</th><th>dLMP/dd</th></tr></thead>
              <tbody>${sensitivityRows(committed.sensitivity)}</tbody>
            </table>
          </article>
        </div>
      </section>
    `;
  } finally {
    study.free();
  }
}

main().catch((error) => {
  appRoot.innerHTML = `<p class="status error">${error instanceof Error ? error.message : String(error)}</p>`;
});
