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

The first MapToGrid observation in the November 2026 snapshot is the production
release check: Portugal completed with every check and published range met.
That one observation is verification of the reporting path, not an independent
sample of visitor outcomes. Automated headless requests were not included in
the observed counts. Event-property requests use the Cloud API's `event` filter
([API changelog](https://docs.umami.is/docs/cloud/changelog)).

A subsequent production check on 7 November completed Portugal generation
with DC OPF optimal and AC power flow converged. All five collector requests
(page view, engine load, generation start/finish and agent availability)
returned HTTP 200. The aggregate totals below did not increase after the
automated check, so it proves transport only and is not counted as new visitor
evidence. A separate normal browser session also completed without collector
errors. The test machine's VPN DNS returned NXDOMAIN for the Umami hosts;
the isolated transport test used their verified public DNS address without
changing the machine's network settings or the site's privacy behavior.

<!-- usage:start -->

Last pull: 2026-11 (2026-11-07).

| Site | Visitors | Page views | Top events |
| --- | ---: | ---: | --- |
| maptogrid.com | 1 | 1 | engine.load 1, generate.start 1, generate.finish 1, agent.availability 1 |
| tellegen.dev | 90 | 222 | calculation.solve 386, case.select 339, view.change 291, agent.availability 218, case.import 77 |

Grids generated on maptogrid.com, by country (trusted = every check and published range met):

| Country | Runs | Trusted |
| --- | ---: | ---: |
| PRT | 1 | 100% |

<!-- usage:end -->
