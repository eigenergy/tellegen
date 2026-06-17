# Privacy

tellegen parses dropped case files in your browser. Those files are not
uploaded to the tellegen backend by the current public demo.

## Current Demo

- Dropped `.m`, `.raw`, `.aux`, `.pwd`, `.csv`, `.json`, and `.geojson` files
  stay on your device.
- The browser uses local file contents to draw the map and run DC solves.
- The tellegen backend receives ordinary page and API requests for the bundled
  demo cases.
- There is no analytics product wired to uploaded case contents.

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
