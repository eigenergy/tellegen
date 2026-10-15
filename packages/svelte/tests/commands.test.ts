import { describe, expect, it, vi } from 'vitest';
import { CommandRegistry, commandHint, commandTitle } from '../src/lib/commands.svelte.js';

const noop = () => {};

describe('command registry', () => {
	it('replaces a command by id and unregisters it', () => {
		const registry = new CommandRegistry();
		registry.register({ id: 'zen', title: 'Enter zen mode', run: noop });
		const unregister = registry.register({
			id: 'zen',
			title: 'Exit zen mode',
			run: noop
		});
		expect(registry.commands.map(commandTitle)).toEqual(['Exit zen mode']);
		unregister();
		expect(registry.commands).toHaveLength(0);
	});
	it('matches every query token against the title and keywords, ignoring case', () => {
		const registry = new CommandRegistry();
		registry.register({
			id: 'zen',
			title: 'Enter zen mode',
			keywords: ['focus', 'hide panels'],
			run: noop
		});
		registry.register({ id: 'reset', title: 'Reset panel layout', run: noop });
		expect(registry.matches().map((c) => c.id)).toEqual(['zen', 'reset']);
		registry.query = 'ZEN';
		expect(registry.matches().map((c) => c.id)).toEqual(['zen']);
		registry.query = 'hide pan';
		expect(registry.matches().map((c) => c.id)).toEqual(['zen']);
		registry.query = 'panel';
		expect(registry.matches().map((c) => c.id)).toEqual(['zen', 'reset']);
		registry.query = 'panel reset';
		expect(registry.matches().map((c) => c.id)).toEqual(['reset']);
		registry.query = 'nothing';
		expect(registry.matches()).toEqual([]);
	});
	it('resolves dynamic titles and hints', () => {
		let zen = false;
		const command = {
			id: 'zen',
			title: () => (zen ? 'Exit' : 'Enter'),
			hint: () => (zen ? 'Restore' : 'Hide'),
			run: noop
		};
		expect(commandTitle(command)).toBe('Enter');
		expect(commandHint(command)).toBe('Hide');
		zen = true;
		expect(commandTitle(command)).toBe('Exit');
		expect(commandHint(command)).toBe('Restore');
		expect(commandHint({ id: 'x', title: 'X', run: noop })).toBeUndefined();
	});
	it('hides the palette and clears the query before running a command', () => {
		const registry = new CommandRegistry();
		const run = vi.fn(() => {
			expect(registry.open).toBe(false);
			expect(registry.query).toBe('');
		});
		registry.register({ id: 'x', title: 'X', run });
		registry.toggle();
		expect(registry.open).toBe(true);
		registry.query = 'x';
		registry.run(registry.commands[0]);
		expect(run).toHaveBeenCalledOnce();
		registry.toggle();
		expect(registry.query).toBe('');
		registry.toggle();
		expect(registry.open).toBe(false);
	});
});
