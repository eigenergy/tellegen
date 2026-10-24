import type { DistGraph, McPfResult } from '@tellegen/engine';

const polar = (m: number, deg: number) => ({
	re: m * Math.cos((deg * Math.PI) / 180),
	im: m * Math.sin((deg * Math.PI) / 180)
});

/** A deterministic radial four-wire feeder of `n` buses with laterals every 25 buses, declared
 * phase–neutral limits and line ampacities, and a solved-looking operating point: voltage falls with
 * depth, phase 3 falls fastest, and the neutral floats. Shaped like the solver's real output. */
export function syntheticFeeder(n: number): { graph: DistGraph; result: McPfResult } {
	const terminals = ['1', '2', '3', 'n'];
	const parentOf = (i: number) => (i === 0 ? 0 : i % 25 === 0 ? Math.max(0, i - 25) : i - 1);
	const depth = new Int32Array(n);
	for (let i = 1; i < n; i++) depth[i] = depth[parentOf(i)] + 1;
	const graph: DistGraph = {
		buses: Array.from({ length: n }, (_, i) => ({
			id: `bus${i}`,
			terminals,
			grounded: i === 0 ? ['n'] : [],
			neutral_terminal: 'n',
			load_kw: 1,
			gen_kw: 0,
			has_source: i === 0,
			voltage_limits: { vpn_min: [207, 207, 207], vpn_max: [253, 253, 253] }
		})),
		edges: Array.from({ length: n - 1 }, (_, k) => {
			const i = k + 1;
			return {
				kind: 'line' as const,
				id: `line${i}`,
				from: `bus${parentOf(i)}`,
				to: `bus${i}`,
				conductors: terminals.map((t) => [t, t] as [string, string]),
				closed: true,
				n_phases: 3,
				rating: { i_max: [200, 200, 200, 200] }
			};
		})
	};
	const voltage = (i: number, t: string) => {
		const d = depth[i] % 200;
		if (t === 'n') return polar(0.02 * d, 0);
		const drop = (t === '3' ? 0.18 : 0.1) * d;
		return polar(240 - drop, t === '1' ? 0 : t === '2' ? -120 : 120);
	};
	const result = {
		converged: true,
		iterations: 5,
		terminals: graph.buses.flatMap((b, i) =>
			terminals.map((t) => ({
				bus: b.id,
				terminal: t,
				voltage: voltage(i, t),
				current_into_network: { re: 0, im: 0 },
				power_into_network: { re: 0, im: 0 }
			}))
		),
		element_ports: graph.edges.flatMap((e, k) => {
			const amps = 60 + ((k * 37) % 160);
			return [e.from, e.to].flatMap((bus, side) =>
				terminals.map((t) => ({
					element: e.id,
					kind: 'line',
					branch: k,
					bus,
					terminal: t,
					current_into_element: { re: t === 'n' ? amps * 0.3 : amps, im: 0 },
					power_into_element: { re: (side ? -1 : 1) * amps * 230 * (side ? 0.99 : 1), im: 0 }
				}))
			);
		}),
		source_reactions: []
	} as unknown as McPfResult;
	return { graph, result };
}
