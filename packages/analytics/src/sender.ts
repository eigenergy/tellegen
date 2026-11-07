/**
 * The one request this package makes: a POST of a sanitised payload to Umami
 * Cloud's collection endpoint, the same request Umami's own tracker script
 * sends. No script is loaded from a third party. The address is one literal,
 * never assembled, so a site's content security policy can name its origin.
 */
export const UMAMI_SEND = 'https://gateway.umami.is/api/send';

/** What an event or page view carries. Nothing else is ever sent. */
export interface Payload {
	/** Umami website id. */
	website: string;
	/** The page's hostname. */
	hostname: string;
	/** A listed page path, never a query or a fragment. */
	url: string;
	/** That page's listed title. */
	title: string;
	/** Always empty: referring addresses are not reported. */
	referrer: '';
	/** The event name, absent for a page view. */
	name?: string;
	/** Sanitised event data. */
	data?: Record<string, string | number>;
}

export interface Sender {
	send(payload: Payload): void;
}

/** The subset of `fetch` the sender needs; a test passes a stub. */
export type FetchLike = (
	input: string,
	init: {
		method: 'POST';
		headers: Record<string, string>;
		body: string;
		keepalive: boolean;
		credentials: 'omit';
		referrerPolicy: 'no-referrer';
		mode: 'cors';
	}
) => Promise<{ ok: boolean; json(): Promise<unknown> } | undefined | void> | void;

/**
 * A sender for Umami's collection endpoint. The response carries a `cache`
 * token that lets the server skip recomputing the visitor's session; it is
 * kept in memory only and resent as the `x-umami-cache` header. Every
 * failure is swallowed: analytics never changes what the page does.
 */
export function umamiSender(endpoint: string = UMAMI_SEND, fetchFn?: FetchLike): Sender {
	let cache: string | undefined;
	return {
		send(payload) {
			try {
				const call: FetchLike | undefined =
					fetchFn ?? (typeof fetch === 'function' ? (fetch as unknown as FetchLike) : undefined);
				if (!call) return;
				const headers: Record<string, string> = { 'content-type': 'application/json' };
				if (cache) headers['x-umami-cache'] = cache;
				const request = call(endpoint, {
					method: 'POST',
					headers,
					body: JSON.stringify({ type: 'event', payload }),
					keepalive: true,
					credentials: 'omit',
					referrerPolicy: 'no-referrer',
					mode: 'cors'
				});
				Promise.resolve(request)
					.then(async (response) => {
						if (!response || !response.ok) return;
						const body: unknown = await response.json();
						if (
							body &&
							typeof body === 'object' &&
							typeof (body as { cache?: unknown }).cache === 'string'
						)
							cache = (body as { cache: string }).cache;
					})
					.catch(() => {});
			} catch {
				/* A blocked or failing collector must not reach the page. */
			}
		}
	};
}
