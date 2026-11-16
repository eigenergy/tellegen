# US State Demand and Fuel Mix

Two tables for the generator's sub-national (US state) basis, built from US
government data only:

- `crates/tellegen-synth/data/subnational_demand.json`
  (`tellegen.synth-subnational-demand/1`), from
  `scripts/derive-subnational-demand.py`: for the 50 states and DC, keyed by
  ISO 3166-2 code, `[year, TWh, population, peak_factor or null]`, with the
  balancing-authority (BA) basis of every factor in `peak_basis`.
- `crates/tellegen-synth/data/subnational_fuel_mix.json`
  (`tellegen.synth-subnational-fuel-mix/1`), from
  `scripts/derive-subnational-fuel-mix.py`: each state's operating nameplate
  MW and its shares in the generator's fuels.

Both files record every input's URL and SHA-256 and a method note. EIA and
Census Bureau data are works of the US government and in the public domain.

## Data year: 2025

2025 is the latest year with final data in every source as of November
2026: EIA-861 final 2025 data were released on 7 October 2026, EIA-860 final
2025 data on 10 September 2026, the Census Bureau's Vintage 2025 estimates
give 1 July 2025 populations, and EIA-930 has the whole of 2025.

## Rebuilding

The scripts are standard-library Python and read third-party files as data:
xlsx through `zipfile` and `xml.etree`, CSV through `csv`. Download into a
new, empty directory and pass the paths in:

```sh
mkdir /path/to/new-empty-directory && cd /path/to/new-empty-directory
curl -LO https://www.eia.gov/electricity/data/eia861/zip/f8612025.zip
curl -LO https://www.eia.gov/electricity/data/eia860/xls/eia8602025.zip
curl -LO https://www2.census.gov/programs-surveys/popest/datasets/2020-2025/state/totals/NST-EST2025-ALLDATA.csv
curl -LO https://www.eia.gov/electricity/gridmonitor/sixMonthFiles/EIA930_BALANCE_2025_Jan_Jun.csv
curl -LO https://www.eia.gov/electricity/gridmonitor/sixMonthFiles/EIA930_BALANCE_2025_Jul_Dec.csv
shasum -a 256 *   # compare with the sources in the two JSON files
cd /path/to/tellegen
D=/path/to/new-empty-directory
python3 -I scripts/derive-subnational-demand.py --sales $D/f8612025.zip \
  --population $D/NST-EST2025-ALLDATA.csv \
  --hourly $D/EIA930_BALANCE_2025_Jan_Jun.csv $D/EIA930_BALANCE_2025_Jul_Dec.csv \
  > crates/tellegen-synth/data/subnational_demand.json
python3 -I scripts/derive-subnational-fuel-mix.py $D/eia8602025.zip \
  > crates/tellegen-synth/data/subnational_fuel_mix.json
```

The demand script prints each state's in-state BAs and coverage to stderr,
the fuel-mix script the MW it put in each fuel by EIA technology. From the
pinned inputs both outputs are byte-identical across runs and between
Python 3.9 and 3.13.

## Method

- **TWh**: EIA-861 `Sales_Ult_Cust_2025.xlsx`, sheet `States`, the TOTAL
  Sales (MWh) column, summed over Parts A, B and D as the file's own footnote
  prescribes (Part C, delivery only, repeats Part B's energy). That includes
  the `Adjustment 2025` rows standing for the short-form respondents. These
  are retail sales: direct use (on-site industrial and commercial
  generation, customer-owned rooftop solar) and network losses are not in
  them. Third-party-owned behind-the-meter solar sold to the host customer
  is in them (11.8 TWh nationally). The 51 totals equal the Electric Power
  Annual 2025, [Table 2.8](https://www.eia.gov/electricity/annual/html/epa_02_08.html),
  to the thousand MWh; the US total is 4,079.9 TWh.
- **Population**: `POPESTIMATE2025` in the Vintage 2025 state file.
- **Peak factor**: a BA is in a state when at least 80% of its EIA-861
  retail sales (by `BA Code`) lie there. If the state's in-state BAs that
  report EIA-930 demand for at least 95% of the year carry at least 80% of
  its sales, the factor is the maximum of their summed hourly
  `Demand (MW) (Adjusted)` over its mean, over the UTC hours all of them
  report. Before summing, each BA's non-positive hours and hours above twice
  its 99.9th percentile are dropped (IID 122 zero hours, TIDC 25, NEVP 17,
  FMPP, NYIS and SEC 2 each). Otherwise the factor is null and the generator
  uses its default, 1.3.
- **Fuel mix**: EIA-860 `3_1_Generator_Y2025.xlsx`, sheet `Operable`,
  status OP, nameplate MW. A storage prime mover (BA, CE, FW, PS, ES) is
  storage. Otherwise the fuel follows Energy Source 1 and EIA's own groups
  in Table 28 of the EIA-860 layout: gas is NG, BFG, OG and H2; coal is
  ANT, BIT, LIG, SUB, WC, RC and SGC; WAT at an HY turbine is hydro (WAT
  hydrokinetic is other); WND is wind; SUN is solar (photovoltaic and
  thermal); NUC is nuclear. Petroleum, geothermal, biomass and waste,
  landfill gas, MSW and the rest are other. Every Table 28 code is listed
  in the script, so a new code stops it. Shares are rounded to 6 decimals
  by largest remainder and sum to exactly 1.

## The largest states, and Washington

The ten largest states by retail sales, plus Washington. Implied peak is
TWh·10⁶/8760 times the factor, or times 1.3 where the factor is null. BA
basis is the BAs and their share of the state's sales.

| State | TWh | Population | Peak factor | Implied peak (MW) | BA basis | Top 3 fuels (nameplate share) |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| Texas | 533.3 | 31,709,821 | 1.500 | 91,312 | ERCO (88.2%) | gas 42.1%, wind 22.9%, solar 16.0% |
| Florida | 259.1 | 23,462,518 | 1.785 | 52,791 | FMPP, FPC, FPL, GVL, HST, JEA, SEC, TAL, TEC (99.0%) | gas 68.9%, solar 15.6%, nuclear 5.0% |
| California | 243.0 | 39,355,309 | 1.751 | 48,567 | BANC, CISO, IID, LDWP, TIDC (98.0%) | gas 37.6%, solar 23.6%, storage 17.5% |
| Ohio | 161.9 | 11,900,510 | null | 24,028 | none | gas 54.6%, coal 19.8%, solar 13.1% |
| Georgia | 153.2 | 11,302,748 | null | 22,730 | none | gas 43.9%, nuclear 15.4%, coal 14.6% |
| Virginia | 147.6 | 8,880,107 | null | 21,903 | none | gas 47.7%, solar 16.8%, nuclear 11.9% |
| New York | 143.8 | 20,002,427 | 1.841 | 30,213 | NYIS (99.7%) | gas 55.9%, hydro 10.4%, other 8.5% |
| Pennsylvania | 143.4 | 13,059,432 | null | 21,279 | none | gas 57.2%, nuclear 18.5%, coal 10.2% |
| North Carolina | 140.4 | 11,197,968 | null | 20,841 | none | gas 47.2%, solar 18.1%, nuclear 13.9% |
| Illinois | 136.3 | 12,719,141 | null | 20,231 | none | gas 35.4%, nuclear 24.0%, wind 16.4% |
| Washington | 90.1 | 8,001,020 | null | 13,373 | none (52.7%) | hydro 66.8%, gas 12.4%, wind 11.4% |

Six states have a factor: these four and Arizona (2.009; AZPS, SRP and TEPC,
90.6%) and Nevada (2.000; NEVP, 99.1%).

## California check

California's implied peak is 48.6 GW: 242.974 TWh is an average of
27,737 MW, times 1.751. The five in-state BAs' coincident hourly peak was
54,517 MW in the hour ending 19:00 PDT on 21 August 2025, of which CISO
carried 43,665 MW. CAISO's own figure for 2025 is an instantaneous
44,506 MW at 18:07 that day
([CAISO peak load history](https://www.caiso.com/documents/californiaisopeakloadhistory.pdf)),
so the EIA-930 hour agrees with it to within the difference between an
hourly mean and an instantaneous peak. 2025 was a mild year: CAISO's peak
was 48,323 MW in 2024 and 52,061 MW in 2022.

The implied peak is 11% below the measured 54.5 GW, by the same margin as
the energy: the five BAs' hourly demand averages 31,139 MW, 12% more than
California's retail sales. Retail sales leave out network losses and
wholesale loads such as the State Water Project's pumping (the Department of
Water Resources has no retail sales in EIA-861). The factor itself, a ratio,
is unaffected, so 48.6 GW is a peak at retail-sales level. It falls inside
the 45–60 GW range of public statewide figures; nothing in the inputs or
the method was adjusted to reach it.

## States with a null factor

45 of the 51 have no factor:

- 32 have no BA with 80% of its sales inside them. They are served by
  multi-state BAs: PJM, MISO, SPP, Southern, TVA, ISO New England,
  Bonneville, PacifiCorp and others.
- Nine have in-state BAs that carry too little of their sales: Colorado
  (PSCO, 68.8%), Idaho (IPCO, 58.3%), Kentucky (LGEE, 41.8%), Montana
  (NWMT, 69.8%), New Mexico (PNM, 41.8%), North Carolina (CPLE, 42.1%),
  Oregon (PGE, 34.7%), South Carolina (SC and SCEG, 58.4%) and Washington
  (CHPD, DOPD, GCPD, PSEI, SCL and TPWR, 52.7%).
- Four have in-state BAs with no EIA-930 demand: Alaska (AMPL, CEA) and
  Hawaii (HECO), absent from the EIA-930 files; Maine (NBSO, the New
  Brunswick system operator serving northern Maine); Georgia (SEPA, which
  reports no demand).

## Caveats

- **Basis.** TWh is retail sales, which exclude losses and direct use. The
  BA demand behind the factors includes losses: its mean is 1.6–12% above
  retail sales in the four states whose BAs cover 98% or more of sales. The
  US total, 4,079.9 TWh, is 7.4% below the national table's Ember figure
  for the United States (4,404.8 TWh, 2024). A generator that mixes the two
  tables mixes the two bases.
- **The 1.3 default is low for the US.** Every measured factor lies
  between 1.500 (Texas) and 2.009 (Arizona), and the null states hold 68%
  of US sales. Summing every state's implied peak over the lower 48 gives
  662 GW, below EIA's preliminary 2025 lower-48 coincident peak of 759 GW
  (`peak_references.json`), even though a sum of state peaks should exceed
  a coincident one.
- **Snapshots.** EIA regenerates the EIA-930 six-month files (both pinned
  files were last modified on 2 November 2026) and sometimes updates the
  861 and 860 archives in place. A new download can hash differently; the
  hashes in the tables identify the snapshot used.
- **Outliers.** The twice-the-99.9th-percentile rule removes gross spikes,
  not moderate ones. Three survive in EIA's adjusted series (AZPS
  10,529 MW on 13 February, LDWP 5,823 MW on 1 February, SEC 1,560 MW in
  January) and none of them sets a state peak; all six peaks are summer
  hours. Computing the factors from reported `Demand (MW)` under the same
  rule moves California to 1.749 and Nevada to 2.001 and leaves the other
  four unchanged.
- **One year.** A factor carries its year's weather, and 2025 was mild in
  California.
- **Fuel mix.** These are nameplate shares, not summer capacity or
  generation, of utility-scale plants (1 MW or more); customer rooftop
  solar is not in EIA-860. Standby and out-of-service units are left out.
  The 110 MW compressed-air plant (prime mover CE, burning gas) counts as
  storage, and 8 MW of hydrogen fuel cells counts as gas.
- **Not yet wired in.** No Rust code reads these files yet. When the
  generator bundles them, `docs/src/third-party-notices.md` should list
  them with the other data tables.
