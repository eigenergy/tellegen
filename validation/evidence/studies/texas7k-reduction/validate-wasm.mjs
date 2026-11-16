// Validate through the browser's actual WASM ingest and solver entry points.
// Each case runs in a fresh process; only compact comparison evidence is kept.
// node validate-wasm.mjs WASM_DIRECTORY ORIGINAL REDUCED OPTIONS OUTPUT_DIRECTORY
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL, fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import assert from 'node:assert/strict';

const sha = (value) => createHash('sha256').update(value).digest('hex');
const args = process.argv.slice(2);
if (args[0] === '--case') {
  const [, directory, input, optionsPath, reportPath, modulePath] = args;
  const engine = await import(pathToFileURL(path.join(path.resolve(directory), 'tellegen.js')));
  const wasm = fs.readFileSync(path.join(directory, 'tellegen_bg.wasm'));
  engine.initSync({ module: wasm });
  const source = fs.readFileSync(input, 'utf8');
  let start = performance.now();
  const payload = JSON.parse(engine.ingest_dist_case(source, 'bmopf-json'));
  const ingestSeconds = (performance.now() - start) / 1000;
  const { module_json, ...view } = payload;
  const geometry = JSON.parse(payload.geo_layer);
  let pointSourceTags = 0;
  for (const feature of geometry.features) {
    if (feature.geometry.type === 'Point' && feature.properties.kind === 'source') {
      pointSourceTags++;
      delete feature.properties.kind;
    }
  }
  // Only the optional per-point Source marker is allowed to differ. Keep
  // global provenance, coordinates, identifiers, routes and every other field.
  const displayWithoutPointSourceTags = { ...view, geo_layer: JSON.stringify(geometry) };
  const original = JSON.parse(source);
  assert.equal(payload.mc_pf_supported, true, payload.mc_pf_reason);
  assert.equal(payload.coords_kind, 'geographic');
  assert.equal(payload.coords_space, 'geographic');
  assert.equal(payload.placed_buses, Object.keys(original.bus).length);
  assert.deepEqual(payload.diagnostics, []);
  for (const bus of payload.graph.buses) {
    assert.deepEqual(bus.xy, [original.bus[bus.id].longitude, original.bus[bus.id].latitude]);
  }
  if (modulePath) fs.writeFileSync(modulePath, module_json);
  start = performance.now();
  // Exercise the module path used by hosted cases, including its capability gate.
  const options = fs.readFileSync(optionsPath, 'utf8');
  const session = new engine.McPfSession(module_json, options);
  let result;
  try { result = JSON.parse(session.result()); } finally { session.free(); }
  const solveSeconds = (performance.now() - start) / 1000;
  assert.equal(result.converged, true);
  const report = {
    input_sha256: sha(source), wasm_sha256: sha(wasm),
    wasm_loader_sha256: sha(fs.readFileSync(path.join(directory, 'tellegen.js'))),
    view_payload_sha256: sha(JSON.stringify(view)),
    display_without_point_source_tags_sha256: sha(JSON.stringify(displayWithoutPointSourceTags)),
    point_source_tags: pointSourceTags,
    graph_sha256: sha(JSON.stringify(payload.graph)),
    geo_layer_sha256: sha(payload.geo_layer),
    module_bytes: Buffer.byteLength(module_json),
    n_bus: payload.n_bus, placed_buses: payload.placed_buses,
    coords_space: payload.coords_space, coords_kind: payload.coords_kind,
    mc_pf_supported: payload.mc_pf_supported, diagnostic_count: payload.diagnostics.length,
    all_coordinates_equal_to_input: true,
    ingest_seconds: ingestSeconds, module_session_solve_seconds: solveSeconds,
    peak_process_rss_bytes: process.resourceUsage().maxRSS * 1024,
    result_sha256: sha(JSON.stringify(result)),
    converged: result.converged, iterations: result.iterations,
    physical_kcl_residual: result.physical_kcl_residual,
    terminal_count: result.terminals.length,
    element_port_count: result.element_ports.length,
  };
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2) + '\n');
} else {
  if (args.length !== 5) throw new Error('Usage: node validate-wasm.mjs WASM_DIRECTORY ORIGINAL REDUCED OPTIONS OUTPUT_DIRECTORY');
  const [directory, original, reduced, optionsPath, output] = args;
  fs.mkdirSync(output, { recursive: true });
  const reports = {};
  for (const [label, input] of [['original', original], ['reduced', reduced]]) {
    const reportPath = path.join(output, `wasm-${label}.json`);
    const modulePath = label === 'reduced' ? path.join(path.dirname(reduced), 'p1uhs0_1247.reduced.pio.json') : null;
    const child = spawnSync(process.execPath, [fileURLToPath(import.meta.url), '--case', directory, input, optionsPath, reportPath, ...(modulePath ? [modulePath] : [])], { encoding: 'utf8' });
    if (child.status !== 0) throw new Error(child.stderr || `Child failed: ${child.signal}`);
    reports[label] = JSON.parse(fs.readFileSync(reportPath, 'utf8'));
  }
  const report = {
    ...reports,
    graph_exactly_equal: reports.original.graph_sha256 === reports.reduced.graph_sha256,
    display_payload_exactly_equal: reports.original.view_payload_sha256 === reports.reduced.view_payload_sha256,
    display_equal_except_optional_point_source_tags: reports.original.display_without_point_source_tags_sha256 === reports.reduced.display_without_point_source_tags_sha256,
    all_power_flow_result_fields_exactly_equal: reports.original.result_sha256 === reports.reduced.result_sha256,
    timing_note: 'One fresh Node process per input using browser WASM, original first. Includes JSON and result serialization; not a browser UI benchmark.'
  };
  fs.writeFileSync(path.join(output, 'wasm-validation.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify(report, null, 2));
  assert.equal(report.graph_exactly_equal, true);
  assert.equal(report.display_equal_except_optional_point_source_tags, true);
  assert.equal(report.all_power_flow_result_fields_exactly_equal, true);
}
