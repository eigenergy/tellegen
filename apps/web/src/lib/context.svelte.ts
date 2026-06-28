import { getContext, setContext } from 'svelte';
import type { AppState } from './state.svelte';
import type { Controller } from './controller.svelte';

const APP = Symbol('tellegen.app');
const CTRL = Symbol('tellegen.controller');

export const setAppState = (a: AppState): AppState => setContext(APP, a);
export const setController = (c: Controller): Controller => setContext(CTRL, c);

export const getAppState = (): AppState => {
	const app = getContext<AppState | undefined>(APP);
	if (!app)
		throw new Error('getAppState() called outside the tellegen app provider (root +layout)');
	return app;
};

export const getController = (): Controller => {
	const ctrl = getContext<Controller | undefined>(CTRL);
	if (!ctrl)
		throw new Error('getController() called outside the tellegen app provider (root +layout)');
	return ctrl;
};
