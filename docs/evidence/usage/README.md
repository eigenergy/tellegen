# Usage Evidence

What people do with the hosted demo (tellegen.dev) and MapToGrid
(maptogrid.com), pulled from Umami Cloud as aggregate counts and committed
here beside the lab sweeps in `docs/evidence/synth/`. The lab sweeps say how
often generation passes its checks on a fixed set of regions and seeds; this
record says how often it passes for the regions people actually choose, and
where OpenStreetMap mode is reachable for them.

Both sites report through `@tellegen/analytics` (`packages/analytics`): a
fixed vocabulary of event names and enumerated values, sent from each site's
own code, with no cookie or identifier stored on the visitor's device, and
no drawn-region coordinates, seed or files. MapToGrid's `generate.finish` event carries one composite
property, `country_verdict` (for example `PRT:trusted`), so one API call gives
pass rates per country.

## Pulling a month

```sh
UMAMI_API_KEY=... npm run report:usage -- --month 2026-11
```

The key is an Umami Cloud API key, which the Pro plan and up provide (Umami
Cloud: Settings, API keys, Create key). It lives in the environment only and
is never written to this repository; the script reads Umami and never writes
to it. It fetches only the configured tellegen.dev and maptogrid.com websites,
their summary statistics,
event counts and the values of the properties it knows, writes
`docs/evidence/usage/<YYYY-MM>.json`, and rewrites the table below. An
endpoint that fails is recorded in the JSON's `errors` list.

## Asking the data directly

Umami Cloud serves the same account through a read-only Model Context
Protocol endpoint. Claude Code can be given it at user scope, so the key
stays out of every repository:

```sh
export UMAMI_API_KEY=...
claude mcp add --transport http -s user umami https://cloud.umami.is/mcp --header "Authorization: Bearer ${UMAMI_API_KEY}"
```

Then ask, for example, how many `generate.finish` events carried
`country_verdict` values ending in `:trusted` this month. The tools
(`list_websites`, `get_website_stats`, `get_event_properties`,
`get_event_series` and the rest) only read.

## Latest pull

<!-- usage:start -->

Last pull: 2026-11 (2026-11-07).

| Site | Visitors | Page views | Top events |
| --- | ---: | ---: | --- |
| maptogrid.com | 0 | 0 | – |
| tellegen.dev | 87 | 216 | calculation.solve 380, case.select 333, view.change 285, agent.availability 212, case.import 77 |

<!-- usage:end -->
