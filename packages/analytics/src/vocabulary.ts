/**
 * A site's analytics vocabulary: the only page paths, event names, property
 * keys and property values that may ever leave the browser. Every outgoing
 * payload is rebuilt from it, so a value that is not listed here is dropped
 * before it is sent.
 */
export interface Vocabulary {
	/** Page paths that may be reported, each with its title. Any other path is reported as the root. */
	pages: Record<string, string>;
	/** Event names and the property keys each may carry. */
	events: Record<string, readonly string[]>;
	/** The values each string property may take. */
	enums: Record<string, ReadonlySet<string>>;
	/** Properties that carry a non-negative integer measurement. */
	numbers: ReadonlySet<string>;
}

/** What a caller passes to `track`: anything, since only listed keys survive. */
export type EventData = Record<string, string | number | undefined>;

/** Event data after sanitising: listed enum values and clamped integers only. */
export type CleanEventData = Record<string, string | number>;

/** Umami's limits on what it stores (docs.umami.is, tracker functions). */
export const EVENT_NAME_LIMIT = 50;
export const STRING_LIMIT = 500;
export const PROPERTY_LIMIT = 50;
/** Default ceiling for measurements: one day in milliseconds. */
export const DEFAULT_MAX_NUMBER = 86_400_000;

/** The listed page for a URL or path, or the root when the path is not listed. */
export function knownPage(pages: Vocabulary['pages'], value: unknown): string {
	const root = '/' in pages ? '/' : (Object.keys(pages)[0] ?? '/');
	if (typeof value !== 'string') return root;
	try {
		const path = new URL(value, 'https://vocabulary.invalid').pathname;
		return path in pages ? path : root;
	} catch {
		return root;
	}
}

/**
 * The event data that may be sent for an event: listed keys only, string
 * values that the property's enum lists, numbers clamped to non-negative
 * integers no larger than `maxNumber`. Unlisted keys and values vanish.
 */
export function eventData(
	vocabulary: Vocabulary,
	name: string,
	input: Record<string, unknown>,
	maxNumber = DEFAULT_MAX_NUMBER
): CleanEventData {
	const output: CleanEventData = {};
	for (const key of vocabulary.events[name] ?? []) {
		const value = input[key];
		if (typeof value === 'string') {
			if (vocabulary.enums[key]?.has(value)) output[key] = value;
		} else if (
			vocabulary.numbers.has(key) &&
			typeof value === 'number' &&
			Number.isFinite(value) &&
			value >= 0
		) {
			output[key] = Math.min(maxNumber, Math.round(value));
		}
	}
	return output;
}

/**
 * How many units of an Umami quota one occurrence of the event costs: the
 * event itself plus each stored property.
 */
export function eventCredits(vocabulary: Vocabulary, name: string): number {
	const properties = vocabulary.events[name];
	if (!properties) return 0;
	return 1 + properties.length;
}

/**
 * Problems with a vocabulary, as sentences; an empty list means every name
 * and value fits Umami's limits and every property is either an enum or a
 * number. Sites assert this in a unit test so a vocabulary change is checked
 * before it ships.
 */
export function checkVocabulary(vocabulary: Vocabulary): string[] {
	const problems: string[] = [];
	const pageEntries = Object.entries(vocabulary.pages);
	if (pageEntries.length === 0) problems.push('no pages are listed');
	for (const [path, title] of pageEntries) {
		if (!path.startsWith('/')) problems.push(`page ${path} is not an absolute path`);
		if (title.length === 0 || title.length > STRING_LIMIT)
			problems.push(`page ${path} has a bad title`);
	}
	for (const [name, properties] of Object.entries(vocabulary.events)) {
		if (name.length === 0 || name.length > EVENT_NAME_LIMIT)
			problems.push(`event ${name} has a name longer than ${EVENT_NAME_LIMIT}`);
		if (properties.length > PROPERTY_LIMIT)
			problems.push(`event ${name} has more than ${PROPERTY_LIMIT} properties`);
		if (new Set(properties).size !== properties.length)
			problems.push(`event ${name} lists a property twice`);
		for (const key of properties) {
			const isEnum = key in vocabulary.enums;
			const isNumber = vocabulary.numbers.has(key);
			if (isEnum === isNumber)
				problems.push(`property ${key} of ${name} must be exactly one of an enum or a number`);
		}
	}
	for (const [key, values] of Object.entries(vocabulary.enums)) {
		if (values.size === 0) problems.push(`enum ${key} has no values`);
		for (const value of values) {
			if (value.length === 0 || value.length > STRING_LIMIT)
				problems.push(`enum ${key} has a value outside 1 to ${STRING_LIMIT} characters`);
		}
	}
	return problems;
}
