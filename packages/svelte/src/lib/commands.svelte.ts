import { getContext, setContext } from 'svelte';

/** One entry in the command palette. `title` may be a function so a command
 * can describe its next effect ("Enter zen mode" / "Exit zen mode"). */
export interface Command {
	id: string;
	title: string | (() => string);
	/** Secondary line shown under the title; a function when it depends on state. */
	hint?: string | (() => string);
	/** Extra terms the query matches against, in addition to the title. */
	keywords?: string[];
	/** Section the palette lists the command under; defaults to "Actions". */
	group?: CommandGroup;
	run: () => void;
}

export type CommandGroup = 'Actions' | 'Cases' | 'Panels';
const GROUP_ORDER: CommandGroup[] = ['Actions', 'Cases', 'Panels'];
export const commandGroup = (command: Command): CommandGroup => command.group ?? 'Actions';

const KEY = Symbol('tellegen.commands');

export const commandTitle = (command: Command): string =>
	typeof command.title === 'function' ? command.title() : command.title;
export const commandHint = (command: Command): string | undefined =>
	typeof command.hint === 'function' ? command.hint() : command.hint;

/** Commands register from any component under the provider; the palette
 * lists whatever is registered when it opens. */
export class CommandRegistry {
	commands = $state.raw<Command[]>([]);
	open = $state(false);
	query = $state('');

	/** Command lists computed on demand, such as one command per open case.
	 * The palette reads them inside its own `$derived`, so they track live
	 * state without any component syncing them through an effect. */
	sources = $state.raw<(() => Command[])[]>([]);

	register(command: Command) {
		this.commands = [...this.commands.filter((c) => c.id !== command.id), command];
		return () => {
			// Compare by identity: a stale unregister must not remove a replacement.
			this.commands = this.commands.filter((c) => c !== command);
		};
	}
	/** Register a function that lists commands; returns its unregister function. */
	registerSource(source: () => Command[]) {
		this.sources = [...this.sources, source];
		return () => {
			this.sources = this.sources.filter((s) => s !== source);
		};
	}
	/** Static commands followed by every source's current commands. */
	all(): Command[] {
		return [...this.commands, ...this.sources.flatMap((source) => source())];
	}
	/** Every query token must start a word of the title or a keyword, so "two"
	 * finds "Case Two" but not "network". Results
	 * keep registration order within each group, and groups keep a fixed order. */
	matches(): Command[] {
		const tokens = this.query.toLowerCase().split(/[^\p{L}\p{N}]+/u).filter(Boolean);
		const found =
			tokens.length === 0
				? this.all()
				: this.all().filter((command) => {
						const words = [commandTitle(command), ...(command.keywords ?? [])]
							.join(' ')
							.toLowerCase()
							.split(/[^\p{L}\p{N}]+/u);
						return tokens.every((token) => words.some((word) => word.startsWith(token)));
					});
		return GROUP_ORDER.flatMap((group) => found.filter((c) => commandGroup(c) === group));
	}
	show() {
		this.query = '';
		this.open = true;
	}
	hide() {
		this.open = false;
		this.query = '';
	}
	toggle() {
		if (this.open) this.hide();
		else this.show();
	}
	run(command: Command) {
		this.hide();
		command.run();
	}
}

export const setCommandRegistry = (registry: CommandRegistry): CommandRegistry =>
	setContext(KEY, registry);
export const getCommandRegistry = (): CommandRegistry => {
	const registry = getContext<CommandRegistry | undefined>(KEY);
	if (!registry) throw new Error('The command palette requires a TellegenProvider');
	return registry;
};
