# Moreau comparison measurements

Recorded on 2026-09-06. Medians below use milliseconds; [provenance](provenance.json) records the machine, inputs, source hashes, toolchain, artifact sizes, and measurement limits. Raw JSON files retain the cold sample, three warmups by protocol, and ten sorted timed samples.

| Native case | Solver | Derivatives | Solve | Demand column | Weighted rating adjoint |
| --- | --- | --- | ---: | ---: | ---: |
| [case3](native-case3.json) | clarabel | tellegen | 0.091 | 0.020 | 0.020 |
| [case3](native-case3.json) | clarabel | moreau_selected | 0.104 | 0.028 | 0.026 |
| [case3](native-case3.json) | moreau | tellegen | 0.086 | 0.020 | 0.020 |
| [case3](native-case3.json) | moreau | moreau_selected | 0.087 | 0.021 | 0.020 |
| [case200](native-case200.json) | clarabel | tellegen | 5.898 | 2.917 | 3.754 |
| [case200](native-case200.json) | clarabel | moreau_selected | 5.233 | 1.871 | 1.921 |
| [case200](native-case200.json) | moreau | tellegen | 5.712 | 2.135 | 3.795 |
| [case200](native-case200.json) | moreau | moreau_selected | 5.720 | 1.920 | 1.874 |

| Browser case | Solver | Derivatives | Demand preview | Commit with column |
| --- | --- | --- | ---: | ---: |
| [case14](browser-case14.json) | clarabel | tellegen | 0.900 | 3.150 |
| [case14](browser-case14.json) | clarabel | moreau_selected | 0.500 | 1.850 |
| [case14](browser-case14.json) | moreau | tellegen | 0.450 | 2.550 |
| [case14](browser-case14.json) | moreau | moreau_selected | 0.400 | 2.700 |
| [case200](browser-case200.json) | clarabel | tellegen | 8.350 | 18.250 |
| [case200](browser-case200.json) | clarabel | moreau_selected | 4.100 | 20.150 |
| [case200](browser-case200.json) | moreau | tellegen | 4.000 | 14.450 |
| [case200](browser-case200.json) | moreau | moreau_selected | 3.800 | 14.500 |

Optimized WASM size: default 9,243,630 bytes, Moreau 9,534,154 bytes, an increase of 290,524 bytes. Gzip sizes and hashes appear in the provenance file.

The 200-bus native selected derivatives have lower measured medians. The three-bus results do not show a consistent benefit. Browser results show ordering and warmup effects even where the derivative implementation is unchanged. These are exploratory measurements, with no claim of a general speedup and no change to the default backends.

Browser numerical errors use `norm(actual - reference) / max(1, norm(reference))`. The reference uses Clarabel and Tellegen derivatives. The smoke test checks objective agreement at `1e-6`, price/dispatch at `1e-5`, and previews/demand columns at `1e-4`. The case200 demand-column discrepancy is below `2.4e-6`. Native regression tests additionally compare finite differences and forward/adjoint derivatives at differentiable points.
