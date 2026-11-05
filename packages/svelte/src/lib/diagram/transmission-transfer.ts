import type { NetworkBranch, NetworkBus, SolutionFlow } from '@tellegen/engine';

export const TRANSFER_SCALES = [100, 500, 1500] as const;
const FLOW_TOLERANCE_MW = 1e-6;

function reportedMW(value: number | null | undefined): number | undefined {
	if (value == null || !Number.isFinite(value)) return undefined;
	return Math.abs(value) < FLOW_TOLERANCE_MW ? 0 : value;
}

export function transferReading(flow: SolutionFlow | undefined, dc: boolean) {
	const from = reportedMW(flow?.mw);
	// DC branch flows are lossless. Legacy and current compact DC results can omit the to end.
	const to = reportedMW(flow?.pt) ?? (dc && from !== undefined ? -from || 0 : undefined);
	const magnitude =
		from === undefined && to === undefined
			? undefined
			: Math.max(Math.abs(from ?? 0), Math.abs(to ?? 0));
	let direction = 0;
	if (from !== undefined && to !== undefined) {
		if (from > 0 && to < 0) direction = 1;
		else if (from < 0 && to > 0) direction = -1;
	}
	return { from, to, magnitude, direction };
}

export type TransferReading = ReturnType<typeof transferReading>;

export function transferDirectionText(
	reading: TransferReading,
	branch: Pick<NetworkBranch, 'from' | 'to'>
) {
	if (reading.from === undefined || reading.to === undefined) return 'Direction unavailable';
	if (reading.direction === 1) return `Bus ${branch.from} → bus ${branch.to}`;
	if (reading.direction === -1) return `Bus ${branch.to} → bus ${branch.from}`;
	if (reading.magnitude === 0) return 'Zero active flow';
	if (reading.from >= 0 && reading.to >= 0) return 'Active power enters the branch';
	return 'Active power leaves the branch';
}

/** Line width in pixels for an active-power magnitude: area-like (square root) and capped at
 * the chosen scale, so one large corridor cannot swamp the drawing. */
export function transferWidth(magnitude: number | undefined, scale: number) {
	return magnitude ? 1.2 + 5.3 * Math.sqrt(Math.min(magnitude / scale, 1)) : 1;
}

export function modelDemand(bus: NetworkBus, deltas: Readonly<Record<string, number>>) {
	const key = bus.uid != null && Object.hasOwn(deltas, bus.uid) ? bus.uid : bus.id;
	return bus.demand_mw + (Object.hasOwn(deltas, key) ? deltas[key] : 0);
}
