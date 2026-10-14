# Tellegen diagram reference gallery

This local gallery implements the first visual specimen from the
[diagram projection design](../design.md): a four-wire feeder in single-line and
multi-wire representations. The geometry is manually authored over the existing
projection data. It is a visual reference, not a general automatic renderer.

## Open the gallery

Open `index.html` directly in a browser. Scripts, data and fonts are local; no
installation, server, account or network connection is needed.

For a local development preview, from the repository root:

```sh
python3 -m http.server 8765 --bind 127.0.0.1 --directory docs/drafts/diagram-projection
```

Then visit `http://127.0.0.1:8765/gallery/`.

Use the view controls to switch representations, Paper/Dark for the drawing
theme, and Topology/Phases for wire colouring. Select equipment or a wire to
inspect its source mapping; selection persists between representations. Keyboard
users can focus the same objects and select them with Enter or Space. On a small
screen, scroll the drawing horizontally. Export SVG saves the current diagram.

## Files and rebuilding

- `index.html`, `gallery.css` and `gallery.js` provide the local interface and
  reference geometry.
- `gallery-data.js` is generated from `../feeder.connectivity.json` and the
  `singleLine` / `multiWire` exports in `../feeder.ts`.
- `assets/` contains the two existing Tellegen font families and their licenses.

After changing the fixture or projections, regenerate the data with Node 22.18
or newer (native TypeScript type stripping):

```sh
node docs/drafts/diagram-projection/gallery/build-gallery.mjs
node docs/drafts/diagram-projection/gallery/verify-gallery.mjs
```

The verification checks generated-data freshness, visual references, electrical
endpoint membership, full conductor coverage and grounding. It does not prove
layout quality; inspect both themes and representations in a browser after
geometry or style changes.

## Interpretation and limits

The fixture has no solved operating values. Phase colours are a display
convention. Source internals, coil polarity and individual magnetic leg
associations are unspecified and remain identified as unavailable. A shared
transformer annotation is not a wire. The generic load blocks do not imply a
resistive numerical model.

SVG exports retain the current selection and resolved styles. They include a
paper or charcoal background and font-family references; the fonts themselves
are not embedded in the SVG, so another computer may use the fallback fonts.

The restrained canvas, terminal hierarchy and conductor routing use Tellegen's
typography and palette.

## Reviewed reference images

Browser captures from this specimen, before engineer approval:

- [Paper single-line](reference-images/feeder-single-line.jpg)
- [Dark multi-wire](reference-images/feeder-multi-wire.jpg)

Checked both representations and themes, conductor selection across views,
keyboard coil inspection, a 390-pixel viewport with internal canvas scrolling,
and an exported SVG parsed as XML. These are review references, not yet approved
visual regression baselines.
