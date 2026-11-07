import { type Sender, type Payload, umamiSender } from './sender.js';
import {
	type CleanEventData,
	type EventData,
	type Vocabulary,
	DEFAULT_MAX_NUMBER,
	eventData,
	knownPage
} from './vocabulary.js';

/** Where the visitor's choice to turn analytics off is kept. */
export interface OptOutStore {
	get(): boolean;
	set(disabled: boolean): void;
}

type StorageLike = {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
	removeItem(key: string): void;
};

/**
 * An opt-out kept in `localStorage` under `key` (Umami's own tracker honours
 * `umami.disabled`). Storage that cannot be read or written counts as opted
 * out: a choice that cannot be kept is not asked for.
 */
export function localStorageOptOut(key: string, win?: { localStorage: StorageLike }): OptOutStore {
	const storage = (): StorageLike | null => {
		try {
			const host = win ?? (typeof window === 'undefined' ? undefined : window);
			return host?.localStorage ?? null;
		} catch {
			return null;
		}
	};
	return {
		get() {
			try {
				const store = storage();
				return store === null || store.getItem(key) !== null;
			} catch {
				return true;
			}
		},
		set(disabled) {
			try {
				const store = storage();
				if (!store) return;
				if (disabled) store.setItem(key, '1');
				else store.removeItem(key);
			} catch {
				/* Storage denied: the in-memory flag in the client still applies. */
			}
		}
	};
}

export interface AnalyticsConfig extends Vocabulary {
	/** Umami website id. */
	website: string;
	/** Hostnames that report; every other host (previews, local servers) sends nothing. */
	hosts: readonly string[];
	/** Collection endpoint; defaults to Umami Cloud's. */
	endpoint?: string;
	/** Replaces the default sender (tests). */
	sender?: Sender;
	/** Where the opt-out is kept; defaults to nothing kept, so the choice lasts the page. */
	optOut?: OptOutStore;
	/** The page's window; defaults to the global one. */
	window?: Window;
	/** Ceiling for measurements, default one day in milliseconds. */
	maxNumber?: number;
	/** A site rule applied after sanitising, such as dropping a property that only makes sense with another. */
	refine?: (name: string, data: CleanEventData) => CleanEventData;
}

export interface AnalyticsClient {
	/** Whether anything may be sent right now. */
	readonly allowed: boolean;
	/** Do Not Track or Global Privacy Control is on in this browser. */
	readonly blockedByBrowser: boolean;
	/** The visitor turned analytics off, or their choice cannot be kept. */
	readonly optedOut: boolean;
	/** Report a view of a listed page. */
	page(path: string): void;
	/** Report a listed event with sanitised data. Unlisted names are ignored. */
	track(name: string, data?: EventData): void;
	/** Turn analytics off or on for this browser. */
	setOptOut(disabled: boolean): void;
	/** Be told when the opt-out changes. */
	subscribe(listener: () => void): () => void;
	/** The payload an event or page view would send, or null when nothing may be sent. */
	sanitize(name: string | undefined, data: Record<string, unknown>, path: string): Payload | null;
}

type PrivacyNavigator = Navigator & { msDoNotTrack?: string; globalPrivacyControl?: boolean };

/**
 * A client that sends only what the vocabulary lists, only from the listed
 * hosts over https, never under Do Not Track or Global Privacy Control, and
 * never after the visitor opted out. It stores nothing itself: the opt-out
 * store is the only thing written, and only when the visitor asks.
 */
export function createAnalytics(config: AnalyticsConfig): AnalyticsClient {
	const win = config.window ?? (typeof window === 'undefined' ? undefined : window);
	const hosts = new Set(config.hosts);
	const sender = config.sender ?? umamiSender(config.endpoint);
	const maxNumber = config.maxNumber ?? DEFAULT_MAX_NUMBER;
	const listeners = new Set<() => void>();
	let disabled = false;

	const client: AnalyticsClient = {
		get blockedByBrowser() {
			if (!win) return true;
			try {
				const nav = win.navigator as PrivacyNavigator;
				const signal = nav.doNotTrack ?? nav.msDoNotTrack ?? '';
				return signal === '1' || signal === 'yes' || nav.globalPrivacyControl === true;
			} catch {
				return true;
			}
		},
		get optedOut() {
			if (disabled) return true;
			try {
				return config.optOut ? config.optOut.get() : false;
			} catch {
				return true;
			}
		},
		get allowed() {
			if (!win) return false;
			try {
				return (
					win.location.protocol === 'https:' &&
					hosts.has(win.location.hostname) &&
					!client.blockedByBrowser &&
					!client.optedOut
				);
			} catch {
				return false;
			}
		},
		sanitize(name, data, path) {
			if (!client.allowed || !win) return null;
			const url = knownPage(config.pages, path);
			const payload: Payload = {
				website: config.website,
				hostname: win.location.hostname,
				url,
				title: config.pages[url] ?? '',
				referrer: ''
			};
			if (name === undefined) return payload;
			if (!(name in config.events)) return null;
			payload.name = name;
			let clean = eventData(config, name, data, maxNumber);
			if (config.refine) clean = config.refine(name, clean);
			payload.data = clean;
			return payload;
		},
		page(path) {
			try {
				const payload = client.sanitize(undefined, {}, path);
				if (payload) sender.send(payload);
			} catch {
				/* Measurement never interrupts the page. */
			}
		},
		track(name, data = {}) {
			try {
				const payload = client.sanitize(name, data, win?.location.pathname ?? '/');
				if (payload) sender.send(payload);
			} catch {
				/* Measurement never interrupts the page. */
			}
		},
		setOptOut(next) {
			disabled = next;
			try {
				config.optOut?.set(next);
			} catch {
				disabled = true;
			}
			for (const listener of listeners) listener();
		},
		subscribe(listener) {
			listeners.add(listener);
			return () => {
				listeners.delete(listener);
			};
		}
	};
	return client;
}
