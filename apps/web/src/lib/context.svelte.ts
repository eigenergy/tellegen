import { getContext, setContext } from 'svelte';
import type { AppState } from './state.svelte';
import type { Controller } from './controller.svelte';

const APP = Symbol('tellegen.app');
const CTRL = Symbol('tellegen.controller');

export const setAppState = (a: AppState): AppState => setContext(APP, a);
export const getAppState = (): AppState => getContext(APP);
export const setController = (c: Controller): Controller => setContext(CTRL, c);
export const getController = (): Controller => getContext(CTRL);
