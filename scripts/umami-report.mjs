#!/usr/bin/env node
// Pull aggregate usage from Umami Cloud's API into docs/evidence/usage/, so
// what people do with the hosted demo and MapToGrid (which regions they
// generate, which checks pass, where OpenStreetMap mode is reachable) becomes
// committed evidence beside the lab sweeps.
//
//   UMAMI_API_KEY=... node scripts/umami-report.mjs [--month YYYY-MM] [--base URL]
//
// The key comes from the environment only (an Umami Cloud API key, Pro plan
// and up; see docs/evidence/usage/README.md). The script reads, never writes,
// Umami. It writes docs/evidence/usage/<YYYY-MM>.json and refreshes the table
// between the markers in docs/evidence/usage/README.md. An endpoint that
// fails is recorded as an error in the JSON rather than stopping the run, so
// one missing feature does not hide the rest.

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const DEFAULT_BASE = 'https://api.umami.is/v1';
const USAGE_DIR = join(dirname(fileURLToPath(import.meta.url)), '..', 'docs', 'evidence', 'usage');
const README = join(USAGE_DIR, 'README.md');
const START_MARK = '<!-- usage:start -->';
const END_MARK = '<!-- usage:end -->';

// MapToGrid reports one composite property, `country_verdict` (`PRT:trusted`),
// so one call yields pass rates per country; the hosted demo reports the
// outcome of each solve.
const PROPERTIES = [
	{ event: 'generate.finish', property: 'country_verdict' },
	{ event: 'generate.finish', property: 'verdict' },
	{ event: 'osm.fetch', property: 'result' },
	{ event: 'osm.fetch', property: 'endpoint' },
	{ event: 'engine.load', property: 'result' },
	{ event: 'calculation.solve', property: 'result' },
	{ event: 'calculation.solve', property: 'calculation' },
	{ event: 'agent.call', property: 'tool' }
];

function argument(name, fallback) {
	const index = process.argv.indexOf(name);
	return index >= 0 && process.argv[index + 1] ? process.argv[index + 1] : fallback;
}

function monthRange(month) {
	const match = /^(\d{4})-(\d{2})$/.exec(month);
	if (!match) throw new Error(`--month takes YYYY-MM, not ${month}`);
	const year = Number(match[1]);
	const index = Number(match[2]) - 1;
	const start = Date.UTC(year, index, 1);
	const end = Math.min(Date.UTC(year, index + 1, 1) - 1, Date.now());
	if (start > Date.now()) throw new Error(`${month} has not started`);
	return { start, end };
}

async function main() {
	const key = process.env.UMAMI_API_KEY;
	if (!key) {
		console.error(
			'UMAMI_API_KEY is not set: create an API key in Umami Cloud (Settings, API keys) and export it.'
		);
		process.exit(2);
	}
	const base = argument('--base', process.env.UMAMI_API_BASE ?? DEFAULT_BASE).replace(/\/$/, '');
	const now = new Date();
	const month = argument(
		'--month',
		`${now.getUTCFullYear()}-${String(now.getUTCMonth() + 1).padStart(2, '0')}`
	);
	const { start, end } = monthRange(month);
	const errors = [];

	async function get(path, query = {}) {
		const url = new URL(base + path);
		for (const [name, value] of Object.entries(query)) url.searchParams.set(name, String(value));
		const response = await fetch(url, {
			headers: { authorization: `Bearer ${key}`, accept: 'application/json' }
		});
		if (!response.ok) throw new Error(`${response.status} ${response.statusText} for ${path}`);
		return response.json();
	}

	async function attempt(label, call) {
		try {
			return await call();
		} catch (error) {
			errors.push(`${label}: ${error instanceof Error ? error.message : String(error)}`);
			return null;
		}
	}

	const listing = await get('/websites');
	const websites = Array.isArray(listing) ? listing : (listing.data ?? []);
	const report = {
		schema: 'tellegen.usage-evidence/1',
		generated_at: now.toISOString(),
		month,
		range: { start: new Date(start).toISOString(), end: new Date(end).toISOString() },
		source: `${base} (Umami Cloud, read with an API key; aggregate counts only)`,
		websites: [],
		errors
	};
	const range = { startAt: start, endAt: end };

	for (const site of websites) {
		const id = site.id;
		const entry = {
			id,
			name: site.name,
			domain: site.domain,
			stats: null,
			events: {},
			properties: {}
		};
		const stats = await attempt(`${site.domain} stats`, () => get(`/websites/${id}/stats`, range));
		if (stats) {
			entry.stats = {};
			for (const metric of ['pageviews', 'visitors', 'visits', 'bounces', 'totaltime']) {
				const value = stats[metric];
				entry.stats[metric] =
					value && typeof value === 'object' && 'value' in value
						? Number(value.value)
						: Number(value ?? 0);
			}
		}
		const events = await attempt(`${site.domain} events`, () =>
			get(`/websites/${id}/metrics`, { ...range, type: 'event' })
		);
		for (const row of Array.isArray(events) ? events : []) entry.events[row.x] = Number(row.y);
		for (const { event, property } of PROPERTIES) {
			if (!(event in entry.events)) continue;
			const values = await attempt(`${site.domain} ${event}.${property}`, () =>
				get(`/websites/${id}/event-data/values`, {
					...range,
					eventName: event,
					propertyName: property
				})
			);
			if (!Array.isArray(values)) continue;
			entry.properties[`${event}.${property}`] = Object.fromEntries(
				values.map((row) => [String(row.value ?? row.x), Number(row.total ?? row.y ?? 0)])
			);
		}
		entry.countries = countryRates(entry.properties['generate.finish.country_verdict']);
		report.websites.push(entry);
	}

	const file = join(USAGE_DIR, `${month}.json`);
	writeFileSync(file, JSON.stringify(report, null, 2) + '\n');
	refreshReadme(report);
	console.log(`wrote ${file}${errors.length ? ` with ${errors.length} error(s)` : ''}`);
	for (const error of errors) console.error(`  ${error}`);
}

// Per-country pass rates from the composite `ISO3:verdict` values: a grid is
// trusted when every check passed and every published range was met.
function countryRates(values) {
	if (!values) return null;
	const byCountry = new Map();
	for (const [value, count] of Object.entries(values)) {
		const [country, verdict] = value.split(':');
		if (!country || !verdict) continue;
		const row = byCountry.get(country) ?? { runs: 0, trusted: 0 };
		row.runs += count;
		if (verdict === 'trusted') row.trusted += count;
		byCountry.set(country, row);
	}
	return Object.fromEntries(
		[...byCountry.entries()]
			.sort((a, b) => b[1].runs - a[1].runs)
			.map(([country, row]) => [
				country,
				{ ...row, rate: row.runs ? row.trusted / row.runs : null }
			])
	);
}

function percent(value) {
	return value === null || value === undefined ? '–' : `${Math.round(value * 100)}%`;
}

function refreshReadme(report) {
	const lines = [
		START_MARK,
		'',
		`Last pull: ${report.month} (${report.generated_at.slice(0, 10)}).`,
		''
	];
	lines.push('| Site | Visitors | Page views | Top events |', '| --- | ---: | ---: | --- |');
	for (const site of report.websites) {
		const top = Object.entries(site.events)
			.sort((a, b) => b[1] - a[1])
			.slice(0, 5)
			.map(([name, count]) => `${name} ${count}`)
			.join(', ');
		lines.push(
			`| ${site.domain} | ${site.stats?.visitors ?? '–'} | ${site.stats?.pageviews ?? '–'} | ${top || '–'} |`
		);
	}
	for (const site of report.websites) {
		if (!site.countries || Object.keys(site.countries).length === 0) continue;
		lines.push(
			'',
			`Grids generated on ${site.domain}, by country (trusted = every check and published range met):`,
			''
		);
		lines.push('| Country | Runs | Trusted |', '| --- | ---: | ---: |');
		for (const [country, row] of Object.entries(site.countries).slice(0, 40))
			lines.push(`| ${country} | ${row.runs} | ${percent(row.rate)} |`);
	}
	if (report.errors.length)
		lines.push('', `Endpoints that failed: ${report.errors.length} (see the JSON).`);
	lines.push('', END_MARK);
	const readme = readFileSync(README, 'utf8');
	const begin = readme.indexOf(START_MARK);
	const finish = readme.indexOf(END_MARK);
	if (begin < 0 || finish < 0) throw new Error(`${README} lacks the usage markers`);
	writeFileSync(
		README,
		readme.slice(0, begin) + lines.join('\n') + readme.slice(finish + END_MARK.length)
	);
}

main().catch((error) => {
	console.error(error instanceof Error ? error.message : String(error));
	process.exit(1);
});
