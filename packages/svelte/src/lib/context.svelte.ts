import { getContext, setContext } from 'svelte';
import type { AppState } from './state.svelte.js';
import type { Controller } from './controller.svelte.js';

const APP = Symbol('tellegen.app');
const CTRL = Symbol('tellegen.controller');
const CONFIG = Symbol('tellegen.config');

export interface TellegenUiConfig {
	apiBase: string;
	loadDefaultCases: boolean;
	docsHref: string;
	orgHref: string;
	orgLabel: string;
	showFooter: boolean;
}

export type TellegenUiOptions = Partial<TellegenUiConfig>;

export const DEFAULT_TELLEGEN_UI_CONFIG: TellegenUiConfig = {
	apiBase: '/api',
	loadDefaultCases: true,
	docsHref: 'https://eigenergy.github.io/tellegen/',
	orgHref: 'https://github.com/eigenergy',
	orgLabel: 'eigenergy group @ michigan ece',
	showFooter: true
};

export function resolveTellegenUiConfig(options: TellegenUiOptions = {}): TellegenUiConfig {
	return {
		apiBase: options.apiBase ?? DEFAULT_TELLEGEN_UI_CONFIG.apiBase,
		loadDefaultCases:
			options.loadDefaultCases ?? DEFAULT_TELLEGEN_UI_CONFIG.loadDefaultCases,
		docsHref: options.docsHref ?? DEFAULT_TELLEGEN_UI_CONFIG.docsHref,
		orgHref: options.orgHref ?? DEFAULT_TELLEGEN_UI_CONFIG.orgHref,
		orgLabel: options.orgLabel ?? DEFAULT_TELLEGEN_UI_CONFIG.orgLabel,
		showFooter: options.showFooter ?? DEFAULT_TELLEGEN_UI_CONFIG.showFooter
	};
}

export const setAppState = (a: AppState): AppState => setContext(APP, a);
export const setController = (c: Controller): Controller => setContext(CTRL, c);
export const setUiConfig = (config: TellegenUiConfig): TellegenUiConfig =>
	setContext(CONFIG, config);

export const getAppState = (): AppState => {
	const app = getContext<AppState | undefined>(APP);
	if (!app)
		throw new Error('getAppState() called outside a tellegen provider');
	return app;
};

export const getController = (): Controller => {
	const ctrl = getContext<Controller | undefined>(CTRL);
	if (!ctrl)
		throw new Error('getController() called outside a tellegen provider');
	return ctrl;
};

export const getUiConfig = (): TellegenUiConfig => {
	const config = getContext<TellegenUiConfig | undefined>(CONFIG);
	if (!config) throw new Error('getUiConfig() called outside a tellegen provider');
	return config;
};
