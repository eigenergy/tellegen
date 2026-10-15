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
	run: () => void;
}

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

	register(command: Command) {
		this.commands = [...this.commands.filter((c) => c.id !== command.id), command];
		return () => {
			this.commands = this.commands.filter((c) => c.id !== command.id);
		};
	}
	/** Every token of the query must appear in the title or a keyword. */
	matches(): Command[] {
		const tokens = this.query.toLowerCase().split(/\s+/).filter(Boolean);
		if (tokens.length === 0) return this.commands;
		return this.commands.filter((command) => {
			const haystack = [commandTitle(command), ...(command.keywords ?? [])].join(' ').toLowerCase();
			return tokens.every((token) => haystack.includes(token));
		});
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
