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
		const path = new URL(req.url, 'http://localhost').pathname;
		requests.push(path);
		res.setHeader('content-type', 'application/json');
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
	} finally {
		server.close();
		await rm(directory, { recursive: true, force: true });
	}
});
