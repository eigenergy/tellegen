export {
	checkVocabulary,
	eventCredits,
	eventData,
	knownPage,
	DEFAULT_MAX_NUMBER,
	EVENT_NAME_LIMIT,
	PROPERTY_LIMIT,
	STRING_LIMIT
} from './vocabulary.js';
export type { CleanEventData, EventData, Vocabulary } from './vocabulary.js';
export { umamiSender, UMAMI_SEND } from './sender.js';
export type { FetchLike, Payload, Sender } from './sender.js';
export { createAnalytics, localStorageOptOut } from './client.js';
export type { AnalyticsClient, AnalyticsConfig, OptOutStore } from './client.js';
