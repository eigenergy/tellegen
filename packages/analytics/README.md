# `@tellegen/analytics`

Cookieless usage analytics for Tellegen sites, built so that only an
allowlisted vocabulary ever leaves the browser. The package has no
dependencies and no UI framework. It does two things:

- **Sanitise.** A `Vocabulary` lists the page paths, event names, property
  keys and property values a site may report. Every payload is rebuilt from
  it: an unlisted key or value is dropped, numbers become clamped
  non-negative integers, and any other path is reported as the root. Queries,
  fragments, titles, referrers, screen sizes and languages are never sent.
- **Send.** A sender posts each payload straight to Umami Cloud's collection
  address (`https://gateway.umami.is/api/send`) from the site's own code. No
  third-party script runs on the page, so a content security policy keeps
  `script-src 'self'` and adds one host to `connect-src`.

Nothing is sent unless the page is served over https from a listed host, Do
Not Track and Global Privacy Control are off, and the visitor has not opted
out. The opt-out is the only thing written to storage, and only when asked.

```sh
npm install @tellegen/analytics
```

```ts
import { createAnalytics, localStorageOptOut, checkVocabulary } from '@tellegen/analytics';

const vocabulary = {
	pages: { '/': 'Site', '/privacy': 'Privacy' },
	events: { 'calculation.solve': ['calculation', 'result', 'duration_ms'] },
	enums: {
		calculation: new Set(['dcopf', 'acpf']),
		result: new Set(['completed', 'failed', 'cancelled'])
	},
	numbers: new Set(['duration_ms'])
};
if (checkVocabulary(vocabulary).length) throw new Error('fix the vocabulary');

const analytics = createAnalytics({
	...vocabulary,
	website: 'your-umami-website-id',
	hosts: ['example.org', 'www.example.org'],
	optOut: localStorageOptOut('umami.disabled')
});

analytics.page(location.pathname);
analytics.track('calculation.solve', {
	calculation: 'dcopf',
	result: 'completed',
	duration_ms: 812
});
analytics.setOptOut(true); // a privacy page's switch
```

`eventCredits(vocabulary, name)` gives what one occurrence costs against an
Umami quota (the event plus each stored property), so a site can pin its
budget in a test. `sanitize` returns the exact payload an event would send,
for tests that assert nothing private is in it.
