# Synchronous areas and synthetic HVDC links

The four countries from issue #263 are generated at seeds 1, 7 and 2026 with
the existing Natural Earth 1:50m outlines, demand tables and default request.
All twelve DC optimizations are optimal. All twelve AC power flows converge
with every bus inside the request's 0.95–1.05 pu planning band.

| Country | AC areas | DC links | Voltage range over the three seeds |
| --- | ---: | ---: | --- |
| USA | 3 | 2 | 1.0105–1.0395 pu |
| China | 6 | 5 | 1.0136–1.0263 pu |
| Indonesia | 6 | 5 | 1.0160–1.0239 pu |
| Australia | 4 | 3 | 1.0108–1.0245 pu |

`results.json` records each run, input/output hashes, area sizes, every DC
transfer and rating, AC residuals, timings and remaining planning warnings.
`check.py` verifies the saved networks independently of the generator: every
bus belongs to exactly one area, each area is connected by AC branches, no AC
branch crosses an area boundary, DC transfers conserve active power and stay
within their ratings, and all reported voltages meet the band. Runtime varies
with hardware and other builds on the same machine.

## Geographic assumptions

These are approximate divisions for synthetic networks. They are not surveyed
operating boundaries, and the generated DC links are not claims about installed
equipment. The code states every coordinate threshold in `synchronous.rs`.
The country is determined from the populated places; if one country accounts
for at least 95% of their demand it applies to the region, so a small border
town does not create an extra electrical area. Otherwise each site's nearest
place selects the table. The supplied region label does not select a table.

- USA: Western, Eastern and Texas, following the three divisions described by
  [EIA](https://www.eia.gov/energyexplained/electricity/delivery-to-consumers.php).
  The coordinate thresholds approximate the Rockies and Texas; they do not
  reproduce the detailed ERCOT boundary.
- China: Northeast, Northwest, Northern, Central, Eastern and Southern, using
  the regional planning divisions discussed by
  [Xu, Dong and Huang (2015)](https://doi.org/10.1049/iet-gtd.2014.0281).
  This is a stated six-region modelling choice, not a claim that all six
  boundaries match today's synchronous connections.
- Australia: mainland NEM, Tasmania, Western Australia and Northern Territory.
  [AEMO](https://www.aemo.com.au/energy-systems/electricity/national-electricity-market-nem/about-the-national-electricity-market-nem)
  describes the NEM and the separate western and northern systems;
  [Hydro Tasmania](https://www.hydro.com.au/our-energy-system/how-energy-works)
  describes the Basslink connection to Victoria.
- Indonesia: Sumatra, Java/Bali, Kalimantan, Sulawesi, Papua and eastern islands.
  These approximate the island systems described by the
  [IEA](https://www.iea.org/reports/enhancing-indonesias-power-system).
  Combining the smaller eastern islands is a modelling simplification.

Other regions retain the existing landmass/distance grouping. The request's
far-island exclusions still apply. Co-located voltage levels stay in one area.

## Electrical assumptions and remaining limits

AC topology and generator placement run separately in each area. A tree joins
the nearest highest-voltage buses of adjacent areas. A system-wide economic
dispatch supplies desired net exports, and conservation gives the tree flows.
One common scale limits transfers to 5% of the smaller area's peak demand,
divided by its number of neighbours. Ratings are transfer/0.8, with a 1 MW
minimum. These percentages are explicit modelling choices, not fitted
equipment ratings. The corridor and reactive plans include the same transfer
injections that DC and AC validation use. Stored loads keep their original
values.

The links carry fixed, lossless active-power transfers with no reactive-power
control. [PowerModelsACDC.jl](https://github.com/Electa-Git/PowerModelsACDC.jl)
also models DC voltages, converter controls and losses. Those additional
equations are outside this fixed-transfer model; convergence here does not
validate a converter design or establish equivalence to its full AC/DC OPF.

The existing corridor-planning warning remains in ten runs: the angle-spread
target is not met, and USA seed 7 also has one corridor above its rating in the
unconstrained planning dispatch. The constrained DC dispatch is feasible in
every run. These warnings are retained, not reclassified as success. Narrower
capacity promotion and further compensation remain distinct planning work.

## Reproduce

Use Rust 1.96.0, the repository's required toolchain. Download the public-domain
[Natural Earth 5.1.2 country file](https://raw.githubusercontent.com/nvkelso/natural-earth-vector/v5.1.2/geojson/ne_50m_admin_0_countries.geojson);
`regions.py` verifies its SHA-256 before producing requests.

```sh
cargo build -p tellegen-cli --profile release-py
python3 -I validation/evidence/synth/regions.py /tmp/countries.geojson --out /tmp/synth-specs
python3 -I validation/evidence/synth/run.py sweep /tmp/synth-specs/usa.json /tmp/synth-specs/chn.json /tmp/synth-specs/idn.json /tmp/synth-specs/aus.json --tellegen target/release-py/tellegen --output /tmp/synth-sweep --keep
python3 -I validation/evidence/synth/synchronous-areas/check.py /tmp/synth-sweep --output /tmp/synth-results.json
```

The generation report exposes `synchronous_areas` and `planned_hvdc_links`.
Saved modules retain the same lists under
`extensions["tellegen.synchronous_areas"]`, with bus identifiers for the viewer.
Old reports and the existing Rust report constructors remain accepted.
