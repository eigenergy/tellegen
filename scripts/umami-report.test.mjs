import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

test('account-wide credentials cannot put unrelated websites in public evidence', async () => {
	const directory = await mkdtemp(join(tmpdir(), 'umami-report-'));
	const requests = [];
	const server = createServer((req, res) => {
		const url = new URL(req.url, 'http://localhost');
		const path = url.pathname;
		requests.push(url.pathname + url.search);
		res.setHeader('content-type', 'application/json');
		if (path.endsWith('/event-data/values')) {
			const event = url.searchParams.get('event');
			const property = url.searchParams.get('propertyName');
			const values = event === 'generate.finish' && property === 'country_verdict'
				? [{ value: 'PRT:trusted', total: 1 }, { value: 'PRT:failed', total: 1 }]
				: event === 'engine.load' && property === 'result'
					? [{ value: 'ok', total: 2 }]
					: event === 'generate.finish' && property === 'verdict'
						? [{ value: 'trusted', total: 1 }, { value: 'failed', total: 1 }]
						: [{ value: 'unrelated-event', total: 999 }];
			res.end(JSON.stringify(values));
			return;
		}
		if (path.includes('6e491a05-f119-4810-8e4e-64be7d9efb46') && path.endsWith('/metrics')) {
			res.end(JSON.stringify([{ x: 'generate.finish', y: 2 }, { x: 'engine.load', y: 2 }]));
			return;
		}
		res.end(JSON.stringify(path === '/websites' ? { data: [
			{ id: '6e491a05-f119-4810-8e4e-64be7d9efb46', domain: 'maptogrid.com', name: 'MapToGrid' },
			{ id: '41f22b57-b287-42b2-935c-037f82a80020', domain: 'tellegen.dev', name: 'tellegen' },
			{ id: 'private-id', domain: 'confidential.example', name: 'Private client' }
		] } : path.endsWith('/stats') ? { visitors: { value: 2 } } : []));
	});
	try {
		server.listen(0, '127.0.0.1');
		await once(server, 'listening');
		await mkdir(join(directory, 'scripts'));
		await mkdir(join(directory, 'docs/evidence/usage'), { recursive: true });
		await copyFile(new URL('./umami-report.mjs', import.meta.url), join(directory, 'scripts/umami-report.mjs'));
		await writeFile(join(directory, 'docs/evidence/usage/README.md'), '<!-- usage:start -->\n<!-- usage:end -->\n');
		const child = spawn(process.execPath, [join(directory, 'scripts/umami-report.mjs')], {
			env: { ...process.env, UMAMI_API_KEY: 'fixture-only', UMAMI_API_BASE: `http://127.0.0.1:${server.address().port}` },
			stdio: 'pipe'
		});
		let errors = '';
		child.stderr.on('data', (chunk) => { errors += chunk; });
		const [exit] = await once(child, 'close');
		assert.equal(exit, 0, errors);
		const month = new Date().toISOString().slice(0, 7);
		const raw = await readFile(join(directory, `docs/evidence/usage/${month}.json`), 'utf8');
		const report = JSON.parse(raw);
		assert.deepEqual(report.websites.map((s) => s.domain).sort(), ['maptogrid.com', 'tellegen.dev']);
		assert.ok(!requests.some((path) => path.includes('private-id')));
		assert.ok(!raw.includes('confidential.example'));
		assert.ok(!raw.includes('fixture-only'));
		const map = report.websites.find((site) => site.domain === 'maptogrid.com');
		assert.deepEqual(map.countries, { PRT: { runs: 2, trusted: 1, rate: 0.5 } });
		assert.deepEqual(map.properties['engine.load.result'], { ok: 2 });
		assert.ok(!requests.some((url) => url.includes('eventName=')));
		assert.ok(!raw.includes('unrelated-event'));
	} finally {
		server.close();
		await rm(directory, { recursive: true, force: true });
	}
});
