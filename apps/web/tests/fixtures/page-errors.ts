import { test as base } from '@playwright/test';

// An exception thrown inside a render loop does not fail a Playwright spec on
// its own. The page keeps serving DOM, so selectors resolve and geometry
// assertions still hold while nothing draws. maplibre-gl 6 shipped exactly
// that shape: deck.gl threw once per frame out of the interleaved render, the
// canvas stayed blank, and every spec here stayed green.
//
// So fail a spec on any uncaught exception. This is deliberately narrower than
// "no console errors": several specs drive the app into 4xx paths on purpose
// and the browser logs those as console errors, which says nothing about
// whether the page still works.
//
// Import `test` and `expect` from here rather than from `@playwright/test` and
// the check applies automatically.

export const test = base.extend<{ failOnPageError: void }>({
	failOnPageError: [
		async ({ page }, use) => {
			const errors: Error[] = [];
			page.on('pageerror', (error) => errors.push(error));
			await use();
			if (errors.length === 0) return;
			// Repeats are one fault per frame, so report the shapes and the count
			// rather than several hundred identical stacks.
			const byMessage = new Map<string, { count: number; stack: string }>();
			for (const error of errors) {
				const seen = byMessage.get(error.message);
				if (seen) seen.count += 1;
				else byMessage.set(error.message, { count: 1, stack: error.stack ?? error.message });
			}
			const detail = [...byMessage.values()]
				.map(({ count, stack }) => `  (x${count}) ${stack}`)
				.join('\n');
			throw new Error(`page threw ${errors.length} uncaught error(s):\n${detail}`);
		},
		{ auto: true }
	]
});

export { devices, expect } from '@playwright/test';
