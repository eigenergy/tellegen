import type { JsonValue, TellegenToolActivityEvent } from '@tellegen/webmcp';

type Finished = Extract<TellegenToolActivityEvent, { type: 'finished' }>;
type Data = Record<string, JsonValue>;

function record(value: JsonValue | undefined): Data | null {
	return value && typeof value === 'object' && !Array.isArray(value) ? (value as Data) : null;
}

function number(value: JsonValue | undefined): number | null {
	return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function text(value: JsonValue | undefined): string | null {
	return typeof value === 'string' && value ? value : null;
}

function format(value: number, digits = 2): string {
	return value.toLocaleString('en-US', { maximumFractionDigits: digits });
}

/** Decimals for a value of this size: fewer as it grows, so a change reads at
 * a glance. */
function decimals(magnitude: number): number {
	return magnitude >= 10_000 ? 0 : magnitude >= 1_000 ? 1 : 2;
}

/** A before → after pair at one precision, chosen from the larger magnitude,
 * so `626.95 → 876.5` reads `626.95 → 876.50`. */
export function formatChange(from: number, to: number): string {
	const digits = decimals(Math.max(Math.abs(from), Math.abs(to)));
	const fixed = (value: number) =>
		value.toLocaleString('en-US', {
			minimumFractionDigits: digits,
			maximumFractionDigits: digits
		});
	return `${fixed(from)} → ${fixed(to)}`;
}

function signed(value: number): string {
	return `${value > 0 ? '+' : value < 0 ? '−' : ''}${format(Math.abs(value))}`;
}

function plural(count: number, one: string, many: string): string {
	return `${format(count, 0)} ${count === 1 ? one : many}`;
}

function element(kind: JsonValue | undefined, id: JsonValue | undefined): string | null {
	const name = text(id);
	if (!name) return null;
	return `${kind === 'branch' ? 'line' : 'bus'} ${name}`;
}

/** How long a call took, or has taken so far. */
export function formatElapsed(ms: number): string {
	const clamped = Math.max(0, ms);
	return clamped < 1_000 ? `${Math.round(clamped)} ms` : `${(clamped / 1_000).toFixed(1)} s`;
}

/** A few words for a finished call: what it found or changed, or why it failed. */
export function activitySummary(event: Finished): string {
	const { response } = event;
	if (!response.ok) return response.error.message;
	if (event.callerCancelled) return 'Cancelled after completing';
	const data = response.data;
	if (data.output_truncated) return 'Done';
	switch (event.toolName) {
		case 'list_cases': {
			const total = number(data.total);
			return total === null ? 'Done' : plural(total, 'case', 'cases');
		}
		case 'select_case':
		case 'inspect_case': {
			const id = text(data.case_id);
			const state = text(data.state_id);
			if (state) return `Saved state ${state}`;
			return id ? `Case ${id}` : 'Done';
		}
		case 'query_network': {
			const returned = number(data.returned);
			const matched = number(data.total) ?? number(data.matched);
			const branch = data.element_kind === 'branch';
			if (returned === null) return 'Done';
			const noun = branch ? (returned === 1 ? 'line' : 'lines') : returned === 1 ? 'bus' : 'buses';
			return matched !== null && matched > returned
				? `${format(returned, 0)} of ${format(matched, 0)} ${branch ? 'lines' : 'buses'}`
				: `${format(returned, 0)} ${noun}`;
		}
		case 'focus_network': {
			const focused = record(data.focused);
			const name = element(focused?.kind, focused?.element_id);
			return name ? `Focused ${name}` : 'Done';
		}
		case 'analyze_sensitivity': {
			const target = record(data.target);
			const name = element(target?.kind, target?.element_id);
			return name ? `Sensitivity: ${name}` : 'Done';
		}
		case 'preview_case_update': {
			const delta = number(record(data.prediction)?.objective_delta);
			return delta === null ? 'Preview only' : `Predicted ${signed(delta)} $/h`;
		}
		case 'update_case':
		case 'reset_case': {
			const before = record(data.before);
			const after = record(data.after);
			const from = number(before?.objective);
			const to = number(after?.objective);
			const lines = number(after?.binding_branches);
			const parts: string[] = [];
			if (from !== null && to !== null) parts.push(`Cost ${formatChange(from, to)} $/h`);
			if (lines !== null) parts.push(`${plural(lines, 'line', 'lines')} at limit`);
			if (!parts.length) return event.toolName === 'reset_case' ? 'Reset' : 'Solved';
			return parts.join(', ');
		}
		case 'run_calculation':
			return 'Done';
		case 'propose_capacity_plan':
			return 'Needs approval';
		case 'apply_capacity_plan': {
			const count = number(data.applied_count);
			return count === null ? 'Applied' : `Applied ${format(count, 0)}`;
		}
		case 'solve_multiconductor_pf':
			return data.converged === false ? 'Did not converge' : 'Converged';
		case 'export_case': {
			const name = text(data.file_name);
			return name ? `Exported ${name}` : 'Exported';
		}
		case 'import_case': {
			const opened = Array.isArray(data.opened) ? data.opened.length : null;
			return opened === null ? 'Opened' : `Opened ${format(opened, 0)}`;
		}
		default:
			return `Done, ${formatElapsed(event.finishedAt - event.startedAt)}`;
	}
}
