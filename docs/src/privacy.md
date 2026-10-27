# Privacy

tellegen parses dropped case files in your browser. The current public demo
does not upload those files to the tellegen backend.

## Current Demo

- Dropped `.m`, `.raw`, `.aux`, `.epc`, `.pwb`, `.dss`, `.pwd`, `.csv`,
  `.json`, and `.geojson` files stay on your device.
- The browser uses local file contents to draw the map and run DC solves.
- The tellegen backend receives ordinary page and API requests for the bundled
  demo cases.
- The public site (tellegen.dev and www.tellegen.dev) loads
  [Umami Cloud](https://umami.is/privacy) for aggregate usage analytics: page
  visits, actions, completion status, counts, calculation timings and page
  speed. Analytics exclude uploaded filenames and contents, study names and
  goals, equipment identities, coordinates, electrical values, agent prompts
  and error text, and page addresses are reduced to fixed public pages. Do
  Not Track and Global Privacy Control are respected, local and preview
  addresses load no analytics, and the in-app privacy page has a switch to
  turn them off in your browser.
- The map basemap comes from CARTO (`*.basemaps.cartocdn.com`) when the
  deployment has a CARTO basemap key, with OpenFreeMap
  (`tiles.openfreemap.org`) as the fallback. Without a key, OpenFreeMap leads
  and CARTO is the fallback. Your browser requests the style, tiles, fonts
  and sprites directly from whichever provider is on screen, so that provider
  receives your IP address, your user agent, and the map area you view. Your
  case file is never sent, but the map area shows where a locally parsed case
  sits. Apart from the analytics above, nothing else on the page loads from a
  third party.

## Future Opt In Sharing

If tellegen adds a sharing feature, it will be explicit. The action will say
what will be sent and will require a separate confirmation before upload.

Planned defaults for shared files:

- Raw shared files will be retained for 180 days.
- Derived aggregate statistics may be retained without a fixed end date.
- Each upload will return a share id that can be used to request deletion.
- Shared files will not be sold or used for third party advertising.

## Hosting And Legal Notes

The public demo is intended to run on Hetzner infrastructure. Before accepting
uploaded user files, the operator will conclude Hetzner's Data Processing
Agreement and publish controller contact details.

- [Hetzner data protection notes](https://docs.hetzner.com/general/company-and-policy/data-protection-at-hetzner/)
- [GDPR Article 5](https://gdpr-info.eu/art-5-gdpr/)
- [GDPR Article 6](https://gdpr-info.eu/art-6-gdpr/)
- [GDPR Article 13](https://gdpr-info.eu/art-13-gdpr/)
