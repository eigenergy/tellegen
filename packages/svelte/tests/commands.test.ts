import { describe, expect, it, vi } from 'vitest';
import { CommandRegistry, commandHint, commandTitle } from '../src/lib/commands.svelte.js';

const noop = () => {};

describe('command registry', () => {
	it('replaces a command by id and unregisters it', () => {
		const registry = new CommandRegistry();
		registry.register({ id: 'panels', title: 'Hide panels', run: noop });
		const unregister = registry.register({
			id: 'panels',
			title: 'Show panels',
			run: noop
		});
		expect(registry.commands.map(commandTitle)).toEqual(['Show panels']);
		unregister();
		expect(registry.commands).toHaveLength(0);
	});
	it('matches every query token against the title and keywords, ignoring case', () => {
		const registry = new CommandRegistry();
		registry.register({
			id: 'panels',
			title: 'Hide panels',
			keywords: ['focus', 'present'],
			run: noop
		});
		registry.register({ id: 'reset', title: 'Reset panel layout', run: noop });
		expect(registry.matches().map((c) => c.id)).toEqual(['panels', 'reset']);
		registry.query = 'FOCUS';
		expect(registry.matches().map((c) => c.id)).toEqual(['panels']);
		registry.query = 'hide pan';
		expect(registry.matches().map((c) => c.id)).toEqual(['panels']);
		registry.query = 'panel';
		expect(registry.matches().map((c) => c.id)).toEqual(['panels', 'reset']);
		registry.query = 'panel reset';
		expect(registry.matches().map((c) => c.id)).toEqual(['reset']);
		registry.query = 'nothing';
		expect(registry.matches()).toEqual([]);
	});
	it('resolves dynamic titles and hints', () => {
		let hidden = false;
		const command = {
			id: 'panels',
			title: () => (hidden ? 'Exit' : 'Enter'),
			hint: () => (hidden ? 'Restore' : 'Hide'),
			run: noop
		};
		expect(commandTitle(command)).toBe('Enter');
		expect(commandHint(command)).toBe('Hide');
		hidden = true;
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
	it('lists sources after static commands, grouped Actions, Cases, Panels', () => {
		const registry = new CommandRegistry();
		const cases = ['IEEE 14', 'IEEE 118'];
		registry.register({ id: 'panels', title: 'Hide panels', run: noop });
		registry.registerSource(() => [
			{ id: 'panel:agent', title: 'Open Agent panel', group: 'Panels', run: noop }
		]);
		const remove = registry.registerSource(() =>
			cases.map((name) => ({
				id: `case:${name}`,
				title: `Switch to ${name}`,
				group: 'Cases' as const,
				run: noop
			}))
		);
		registry.register({ id: 'run', title: 'Run calculation', run: noop });
		expect(registry.matches().map((c) => c.id)).toEqual([
			'panels',
			'run',
			'case:IEEE 14',
			'case:IEEE 118',
			'panel:agent'
		]);
		// Sources are read on demand, so a new case appears without re-registering.
		cases.push('Texas 7k');
		registry.query = 'texas';
		expect(registry.matches().map((c) => c.id)).toEqual(['case:Texas 7k']);
		remove();
		expect(registry.matches()).toEqual([]);
	});
	it('ignores a stale unregister after a command is replaced', () => {
		const registry = new CommandRegistry();
		const stale = registry.register({ id: 'panels', title: 'Old', run: noop });
		registry.register({ id: 'panels', title: 'New', run: noop });
		stale();
		expect(registry.commands.map(commandTitle)).toEqual(['New']);
	});
});
