Outline: the MapToGrid app's 1:110m layer (regions.py --outline app-110m), the app's default request. Seeds 1, 7, 2026: 51 runs over 17 regions.

- DC OPF optimal in 51 of 51 runs; AC power flow converged inside 0.9–1.1 pu in 47 of 51 (47 converged at all).
- 15 of 17 regions are valid at every seed.
- Peak within ±25% of the reference for 13 of the 13 regions whose reference scope matches (demand does not depend on the seed).

Levels are the national ladder of the region's dominant country, highest first; a `+` marks a higher level in service there that the corridor plan raised buses to at some seed. Counts are out of the seeds run; AC in band means converged inside 0.9–1.1 pu; Vm is the range over the runs that converged; engine ms is the median.

| Region | Levels (kV) | Buses | DC optimal | AC converged | AC in band | Vm pu | Planning incomplete | Engine ms |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| PRT | 400/220/150 | 95 | 3/3 | 3/3 | 3/3 | 0.992–1.033 | 0/3 | 7 |
| ESP | 400/220 | 254 | 3/3 | 3/3 | 3/3 | 1.015–1.028 | 3/3 | 20 |
| DEU | 380/220 | 524 | 3/3 | 3/3 | 3/3 | 0.980–1.024 | 1/3 | 45 |
| GBR | 400/275 | 315 | 3/3 | 3/3 | 3/3 | 1.011–1.054 | 0/3 | 25 |
| TEXAS-BOX | +765/500/345/230 | 462 | 3/3 | 3/3 | 3/3 | 1.001–1.046 | 1/3 | 63 |
| USA | +765/500/345/230 | 3000 | 3/3 | 0/3 | 0/3 |  | 3/3 | 1,454 |
| IND | +765/400/220 | 2014 | 3/3 | 3/3 | 3/3 | 1.008–1.024 | 3/3 | 486 |
| CHN | +1000/500/220 | 3000 | 3/3 | 3/3 | 3/3 | 1.018–1.079 | 3/3 | 1,000 |
| RUS | +750/500/220 | 1181 | 3/3 | 3/3 | 3/3 | 1.008–1.058 | 3/3 | 276 |
| BRA | +765/500/230 | 750 | 3/3 | 3/3 | 3/3 | 1.013–1.037 | 3/3 | 146 |
| JPN | 500/275 | 996 | 3/3 | 3/3 | 3/3 | 1.016–1.037 | 3/3 | 154 |
| AUS | +500/330/275/220 | 312 | 3/3 | 2/3 | 2/3 | 1.008–1.027 | 3/3 | 40 |
| ZAF | +765/400/275 | 312 | 3/3 | 3/3 | 3/3 | 1.012–1.039 | 0/3 | 30 |
| NGA | 330/132 | 174 | 3/3 | 3/3 | 3/3 | 0.956–1.034 | 2/3 | 12 |
| EGY | 500/220 | 312 | 3/3 | 3/3 | 3/3 | 1.015–1.021 | 2/3 | 28 |
| CHL | 500/220 | 282 | 3/3 | 3/3 | 3/3 | 1.019–1.037 | 2/3 | 25 |
| IDN | 500/150 | 356 | 3/3 | 3/3 | 3/3 | 1.015–1.025 | 2/3 | 38 |

Peak demand and structure (demand, the bus count and the levels do not depend on the seed; branch/bus is the range over the seeds):

| Region | Peak MW | Reference MW (year) | Ratio | Peak verdict | Branch/bus | Max circuits |
| --- | ---: | --- | ---: | --- | ---: | ---: |
| PRT | 8,162 | 9,395 (2025) | 0.87 | within | 1.49–1.52 | 2 |
| ESP | 37,988 | 39,957 (2025) | 0.95 | within | 1.60–1.67 | 4 |
| DEU | 78,508 | 75,635 (2025) | 1.04 | within | 1.53–1.60 | 4 |
| GBR | 47,219 | 48,141 (2025) | 0.98 | within | 1.50–1.59 | 4 |
| TEXAS-BOX | 69,266 | 83,679 (2025) | 0.83 | not comparable | 1.60 | 4 |
| USA | 649,906 | 759,180 (2025) | 0.86 | within | 1.60–1.61 | 4 |
| IND | 302,004 | 249,854 (2024) | 1.21 | within | 1.60 | 4 |
| CHN | 1,501,472 | 1,508,000 (2025) | 1.00 | within | 1.64–1.67 | 4 |
| RUS | 175,954 | 166,155 (2025) | 1.06 | within | 1.60 | 4 |
| BRA | 112,433 | 106,532 (2025) | 1.06 | within | 1.60 | 4 |
| JPN | 149,312 | 162,600 (2025) | 0.92 | within | 1.60–1.61 | 4 |
| AUS | 41,796 | 37,949 (2024) | 1.10 | flagged | 1.60 | 4 |
| ZAF | 36,390 | 33,485 (2024) | 1.09 | within | 1.60 | 4 |
| NGA | 5,236 | 5,802 (2025) | 0.90 | not comparable | 1.60 | 4 |
| EGY | 34,828 | 39,800 (2025) | 0.88 | within | 1.60–1.63 | 4 |
| CHL | 13,156 | 13,093 (2025) | 1.00 | within | 1.60 | 4 |
| IDN | 44,810 | 61,288 (2024) | 0.73 | flagged | 1.57–1.60 | 4 |

Not in this outline layer, so not run: FRA.

| Region | Reference scope | Source |
| --- | --- | --- |
| PRT | Mainland public-grid demand, 15-minute (REN); excludes the Azores and Madeira and behind-the-meter self-consumption. Model: Mainland Portugal; the Azores and Madeira are cut as far islands or left out as too small for a bus. | REN, Dados Técnicos 2025: <https://www.ren.pt/media/2cbfm5p4/ren-dados-te-cnicos-2025.pdf> |
| ESP | Peninsular hourly demand at power-station busbars (REE); excludes the Balearic and Canary Islands. Model: Peninsular Spain and the Balearic Islands; the Canaries are cut as far islands. | REE, annual maximums: <https://www.sistemaelectrico-ree.es/en/spanish-electricity-system/demand/annual-maximums> |
| DEU | Country hourly total load (ENTSO-E, the SMARD Netzlast series). Model: Germany. | ENTSO-E Statistical Factsheet 2025: <https://eepublicdownloads.entsoe.eu/clean-documents/Publications/Statistics/Factsheet/entsoe_sfs2025_web.pdf> |
| GBR | Great Britain: transmission National Demand plus NESO's estimates of embedded wind and solar, half-hourly maximum (9 Jan 2025); Northern Ireland, about 1.8 GW, is not included. Model: The United Kingdom polygon, Northern Ireland included. | NESO historic demand data: <https://www.neso.energy/data-portal/historic-demand-data> |
| TEXAS-BOX | ERCOT region hourly integrated native load. Model: A bounding box around Texas that also takes in parts of Mexico, New Mexico, Oklahoma and Louisiana, and El Paso, which is outside ERCOT. | ERCOT hourly load archive: <https://www.ercot.com/gridinfo/load/load_hist> |
| USA | Lower-48 coincident hourly demand over the balancing authorities (EIA-930); 2025 value preliminary. Model: The United States after the far-island policy (Alaska and Hawaii cut). | EIA, Today in Energy: <https://www.eia.gov/todayinenergy/detail.php?id=65864> |
| IND | All-India peak demand met (30 May 2024). No 2025 annual figure is published; ministerial statements put 2025 near 241-243 GW. Model: India. | Ministry of Power via PIB: <https://www.pib.gov.in/PressReleaseIframePage.aspx?PRID=2022257> |
| CHN | National total maximum load, all grids (NEA, 17 Jul 2025). Model: China (Natural Earth's CHN polygon, which leaves out Taiwan, Hong Kong and Macao). | National Energy Administration, as reported: <https://news.10jqka.com.cn/20260711/c678109972.shtml> <https://www.stdaily.com/web/gdxw/2025-07/06/content_365859.html> |
| RUS | UES of Russia; excludes the technologically isolated systems. Model: Russia with its isolated eastern systems; one far polygon is cut. | SO UPS, 2025 operating report: <https://www.so-ups.ru/fileadmin/files/company/reports/disclosure/2026/ups_rep2025.pdf> |
| BRA | SIN instantaneous load (ONS), including estimated behind-the-meter distributed generation. Model: Brazil. | ONS via eixos.com.br: <https://eixos.com.br/energia-eletrica/consumo-de-energia-no-brasil-bate-setimo-recorde-do-ano-atingindo-106-532-mw/> |
| JPN | National coincident peak of the 10 areas, hourly, sending end (OCCTO draft verification report, 22 Oct 2025). Model: Japan. | OCCTO: <https://www.occto.or.jp/iinkai/chouseiryoku/2025/files/chousei_112_02.pdf> |
| AUS | No national figure exists. NEM operational demand 33,716 MW (16 Dec 2024) plus WEM 4,233 MW (18 Feb 2024): a non-coincident sum that leaves out rooftop PV and the off-grid systems. Model: Australia with Tasmania as one AC island; the NEM and Western Australia are joined. | AEMO Quarterly Energy Dynamics Q4 2024 and Q1 2024: <https://aemo.com.au/-/media/files/major-publications/qed/2024/qed-q4-2024.pdf> <https://aemo.com.au/-/media/files/major-publications/qed/2024/qed-q1-2024.pdf> |
| ZAF | Eskom total peak demand on the integrated power system, financial year April 2024 to March 2025; no 2025 figure is published. Model: South Africa. | Eskom Integrated Report 2025: <https://www.eskom.co.za/wp-content/uploads/2025/10/Eskom-integrated-report-2025.pdf> |
| NGA | National grid peak generation (4 Mar 2025): supply-limited, not demand. Model: Nigeria. | TCN via Nairametrics: <https://nairametrics.com/2025/03/06/nigerias-power-generation-hits-record-5801-84mw-tcn/> |
| EGY | Unified national grid peak load (13 Aug 2025). Model: Egypt. | Ministry of Electricity via Amwal Al Ghad: <https://en.amwalalghad.com/?p=214097> |
| CHL | SEN gross hourly demand (CEN, 29 Dec 2025). Model: Chile; small islands are left out. | Coordinador Eléctrico Nacional, Reporte Energético enero 2026: <https://www.coordinador.cl/wp-content/uploads/2026/01/CEN_Reporte_Energetico_SEN_Ene26.pdf> |
| IDN | Sum of the individual PLN system peaks, not coincident (Java-Bali alone 42,636 MW); no 2025 figure was retrievable. Model: Indonesia as one AC island over 26 landmasses; one far polygon is cut. | PLN Statistik 2024: <https://www.pln.co.id/webapi/media/file/webkorp/asset/Statistik2024.pdf> |
