---
"@tellegen/analytics": minor
---

New package: the allowlisted, cookieless usage analytics the hosted demo and
MapToGrid share. A `Vocabulary` names the page paths, event names, property
keys and values a site may report; `createAnalytics` rebuilds every payload
from it and posts it to Umami Cloud's collection address from the site's own
code, so no third-party script runs on the page. Nothing is sent from an
unlisted host, over plain http, under Do Not Track or Global Privacy Control,
or after the visitor opts out.
