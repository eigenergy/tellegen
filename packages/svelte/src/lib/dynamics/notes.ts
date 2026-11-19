/** The engine's notes, grouped for reading. Grouping is a convenience: every
 * note keeps its text verbatim and each lands in exactly one group, in the
 * engine's order. The patterns are conservative; anything they do not match
 * is "Other". Pure.
 * @block Engine notes */

export interface NoteGroup {
	id: 'not_simulated' | 'held' | 'limit' | 'reactive' | 'other';
	title: string;
	notes: string[];
}

const GROUPS: { id: NoteGroup['id']; title: string; test: RegExp }[] = [
	{
		id: 'not_simulated',
		title: 'Not simulated',
		test: /not simulated|no model in tellegen|unsupported/i
	},
	{
		id: 'held',
		title: 'Held at power flow',
		test: /held at (its|their) power flow output|constant-current injection/i
	},
	{ id: 'limit', title: 'At a limit', test: /\blimit\b|beyond|ceiling|outside \[/i },
	{ id: 'reactive', title: 'Reactive power', test: /reactive|setpoint|Mvar/i }
];

export function groupNotes(notes: readonly string[]): NoteGroup[] {
	const out: NoteGroup[] = [
		...GROUPS.map((g) => ({ id: g.id, title: g.title, notes: [] as string[] })),
		{ id: 'other', title: 'Other', notes: [] }
	];
	for (const n of notes) {
		const k = GROUPS.findIndex((g) => g.test.test(n));
		out[k < 0 ? out.length - 1 : k].notes.push(n);
	}
	return out.filter((g) => g.notes.length > 0);
}
