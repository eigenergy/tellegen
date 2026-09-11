import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { chromium } from "@playwright/test";

const browser = await chromium.launch();
try {
  const page = await browser.newPage();
  const workers = [];
  page.on("worker", (worker) => workers.push(worker));
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto(process.env.TELLEGEN_COMPARE_URL ?? "http://127.0.0.1:5175/moreau.html");
  if (process.env.TELLEGEN_COMPARE_CASE) await page.locator("#case").setInputFiles(process.env.TELLEGEN_COMPARE_CASE);
  await page.getByRole("button", { name: "Compare backends" }).click();
  await page.waitForFunction(() => document.querySelector("#status").textContent !== "Running", null, { timeout: 180000 });
  assert.equal(await page.locator("#status").textContent(), "Complete");
  const result = JSON.parse(await page.locator("#results").textContent());
  assert.equal(result.records.length, 4);
  assert.ok(workers.length >= 4, "comparison must execute in isolated workers");
  assert.deepEqual(errors, []);
  const reference = result.records[0];
  function close(actual, expected, tolerance) {
    assert.equal(actual.length, expected.length);
    const error = Math.hypot(...actual.map((value, i) => value - expected[i]));
    const scaledError = error / Math.max(1, Math.hypot(...expected));
    assert.ok(scaledError <= tolerance, `numerical discrepancy ${scaledError}`);
    return scaledError;
  }
  for (const record of result.records) {
    assert.equal(record.error, undefined, JSON.stringify(record));
    assert.ok(Math.abs(record.objective - reference.objective) < 1e-6 * Math.max(1, Math.abs(reference.objective)));
    record.numerical_error = {
      lmp: close(record.lmp.map((v) => v.value), reference.lmp.map((v) => v.value), 1e-5),
      dispatch: close(record.dispatch.map((v) => v.mw), reference.dispatch.map((v) => v.mw), 1e-5),
      preview: close(record.preview_prices.map((v) => v.value), reference.preview_prices.map((v) => v.value), 1e-4),
      demand_column: close(record.demand_column.map((v) => v.value), reference.demand_column.map((v) => v.value), 1e-4),
    };
    assert.ok(record.exact_solves >= 1);
    assert.equal(record.demand_preview.samples_ms.length, 10);
    assert.equal(record.commit_with_column.samples_ms.length, 10);
  }
  result.records = result.records.map(({ lmp, dispatch, preview_prices, demand_column, ...record }) => record);
  await writeFile(process.env.TELLEGEN_COMPARE_OUT ?? "target/moreau-browser.json", JSON.stringify(result, null, 2) + "\n");
  console.log("Four browser backend combinations passed, including previews, commits, and planning.");
} finally { await browser.close(); }
