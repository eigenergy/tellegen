# @tellegen/analytics

## 0.1.0

### Minor Changes

- [#320](https://github.com/eigenergy/tellegen/pull/320) [`94b3427`](https://github.com/eigenergy/tellegen/commit/94b3427b5e9411da25be83c5f8dabb095693eaeb) Thanks [@samtalki](https://github.com/samtalki)! - New package: the allowlisted, cookieless usage analytics the hosted demo and
  MapToGrid share. A `Vocabulary` names the page paths, event names, property
  keys and values a site may report; `createAnalytics` rebuilds every payload
  from it and posts it to Umami Cloud's collection address from the site's own
  code, so no third-party script runs on the page. Nothing is sent from an
  unlisted host, over plain http, under Do Not Track or Global Privacy Control,
  or after the visitor opts out.
