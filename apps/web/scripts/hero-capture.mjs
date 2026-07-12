// Reproducible hi-DPI screenshot of the demo for papers and docs.
//
//   node scripts/hero-capture.mjs [options]
//
//   --url    app to capture (default https://tellegen.dev; use
//            http://127.0.0.1:4173 against `npm run preview`)
//   --case   case chip text to activate (default Texas7k)
//   --scale  deviceScaleFactor (default 2; 1600x842 viewport -> 3200x1684 png)
//   --drag   hold the rating slider this fraction of track width from its
//            current position and capture mid-drag (e.g. 0.25); omit to
//            capture the committed state
//   --clean  hide page chrome that fights a print figure: the grain overlay,
//            the header (which also names the group, so review-copy captures
//            need this), the footer, zoom buttons, and the bus lookup.
//            CARTO/OSM attribution stays visible
//   --out    output path (default hero-<case>-<scale>x.png)
//
// The flow mirrors line-rating.spec.ts: wait for the solve card, activate the
// case, select the binding line from the panel list, then screenshot the
// viewport. MapLibre sizes its backing store by devicePixelRatio, so the
// context's deviceScaleFactor is what buys print resolution (its 4096px canvas
// clamp allows up to 2x at this viewport).
import { chromium } from '@playwright/test';

const arg = (name, fallback) => {
	const i = process.argv.indexOf(`--${name}`);
	return i >= 0 && process.argv[i + 1] ? process.argv[i + 1] : fallback;
};

const url = arg('url', 'https://tellegen.dev');
const caseText = arg('case', 'Texas'); // matches the chip on its region row
const scale = Number(arg('scale', '2'));
const drag = arg('drag', null);
const clean = process.argv.includes('--clean');
const out = arg('out', `hero-${caseText.toLowerCase().replace(/[^a-z0-9]+/g, '')}-${scale}x.png`);

const browser = await chromium.launch();
const page = await browser.newPage({
	viewport: { width: 1600, height: 842 },
	deviceScaleFactor: scale
});

await page.goto(url, { waitUntil: 'domcontentloaded' });
// Hosted cases show the stats panel once solved; there is no solve card on
// this path (that belongs to dropped local cases).
const chip = page.locator('.case-activate').filter({ hasText: caseText }).first();
await chip.waitFor({ timeout: 60_000 });
await chip.click();

// Select the first binding line once the case solves, for the dLMP/drating
// view. Texas7k and ACTIVSg500 each have exactly one.
const bindingLine = page.locator('.binding-lines button').first();
await bindingLine.waitFor({ timeout: 120_000 });
await bindingLine.click();
await page.locator('.sensitivity-readout').waitFor();

// After the interactions: the header holds the case chips the script clicks,
// so it can only be hidden from here on.
if (clean) {
	await page.addStyleTag({
		content:
			'body::after, header, footer, .maplibregl-ctrl-group, .bus-lookup { display: none !important; }'
	});
}

// Basemap tiles and the network layer render asynchronously after the solve.
await page.waitForLoadState('networkidle', { timeout: 30_000 }).catch(() => {});
await page.waitForTimeout(4000);

if (drag) {
	const track = page.locator('input[type="range"]').first();
	const box = await track.boundingBox();
	if (!box) throw new Error('rating slider not visible');
	const y = box.y + box.height / 2;
	const from = box.x + box.width / 2;
	await page.mouse.move(from, y);
	await page.mouse.down();
	await page.mouse.move(from + Number(drag) * box.width, y, { steps: 12 });
	// Screenshot while held: the map shows the first order preview, not the
	// committed solve.
	await page.waitForTimeout(500);
	await page.screenshot({ path: out });
	await page.mouse.up();
} else {
	await page.screenshot({ path: out });
}

await browser.close();
console.log(`wrote ${out} (${1600 * scale}x${842 * scale})`);
