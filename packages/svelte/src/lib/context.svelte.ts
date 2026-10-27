import { getContext, setContext } from 'svelte';
import { normalizeCartoApiKey } from './basemap.js';
import type { AppState } from './state.svelte.js';
import type { Controller } from './controller.svelte.js';

const APP = Symbol('tellegen.app');
const CTRL = Symbol('tellegen.controller');
const CONFIG = Symbol('tellegen.config');

export interface TellegenUiConfig {
	/** Backend base path; `null` for a host with no backend at all. */
	apiBase: string | null;
	/** URL of the separately built, development-only AC OPF WASI asset. */
	acOpfWasmUrl: string | null;
	loadDefaultCases: boolean;
	docsHref: string;
	orgHref: string;
	orgLabel: string;
	showFooter: boolean;
	/** `'dark'` sets `data-theme="dark"` on the document root while the
	 * provider is mounted, which switches the `styles.css` tokens. The
	 * provider keeps it in step with its prop. */
	theme: 'light' | 'dark';
	/** CARTO basemap key for the default shell's map, trimmed; empty for
	 * none. See `TellegenMap`'s `cartoApiKey`. The provider keeps it in step
	 * with its prop. */
	cartoApiKey: string;
}

export type TellegenUiOptions = Partial<TellegenUiConfig>;

export const DEFAULT_TELLEGEN_UI_CONFIG: TellegenUiConfig = {
	apiBase: '/api',
	acOpfWasmUrl: null,
	loadDefaultCases: true,
	docsHref: 'https://eigenergy.github.io/tellegen/',
	orgHref: 'https://github.com/eigenergy',
	orgLabel: 'eigenergy group @ michigan ece',
	showFooter: true,
	theme: 'light',
	cartoApiKey: ''
};

export function resolveTellegenUiConfig(options: TellegenUiOptions = {}): TellegenUiConfig {
	return {
		apiBase: options.apiBase === undefined ? DEFAULT_TELLEGEN_UI_CONFIG.apiBase : options.apiBase,
		loadDefaultCases:
			options.loadDefaultCases ?? DEFAULT_TELLEGEN_UI_CONFIG.loadDefaultCases,
		acOpfWasmUrl: options.acOpfWasmUrl ?? DEFAULT_TELLEGEN_UI_CONFIG.acOpfWasmUrl,
		docsHref: options.docsHref ?? DEFAULT_TELLEGEN_UI_CONFIG.docsHref,
		orgHref: options.orgHref ?? DEFAULT_TELLEGEN_UI_CONFIG.orgHref,
		orgLabel: options.orgLabel ?? DEFAULT_TELLEGEN_UI_CONFIG.orgLabel,
		showFooter: options.showFooter ?? DEFAULT_TELLEGEN_UI_CONFIG.showFooter,
		theme: resolveTheme(options.theme),
		cartoApiKey: resolveCartoApiKey(options.cartoApiKey)
	};
}

export const resolveTheme = (theme: TellegenUiOptions['theme']): TellegenUiConfig['theme'] =>
	theme ?? DEFAULT_TELLEGEN_UI_CONFIG.theme;

export const resolveCartoApiKey = (key: TellegenUiOptions['cartoApiKey']): string =>
	normalizeCartoApiKey(key) ?? DEFAULT_TELLEGEN_UI_CONFIG.cartoApiKey;

export const setAppState = (a: AppState): AppState => setContext(APP, a);
export const setController = (c: Controller): Controller => setContext(CTRL, c);
export const setUiConfig = (config: TellegenUiConfig): TellegenUiConfig =>
	setContext(CONFIG, config);

export const getAppState = (): AppState => {
	const app = getContext<AppState | undefined>(APP);
	if (!app) throw new Error('getAppState() called outside a tellegen provider');
	return app;
};

export const getController = (): Controller => {
	const ctrl = getContext<Controller | undefined>(CTRL);
	if (!ctrl) throw new Error('getController() called outside a tellegen provider');
	return ctrl;
};

export const getUiConfig = (): TellegenUiConfig => {
	const config = getContext<TellegenUiConfig | undefined>(CONFIG);
	if (!config) throw new Error('getUiConfig() called outside a tellegen provider');
	return config;
};
