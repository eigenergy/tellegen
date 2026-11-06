---
'@tellegen/engine': patch
---

The engine reads cases with PowerIO 0.11.9, up from 0.11.7. A dropped JSON document that carries PowerModelsDistribution's `data_model` marker now opens as a distribution case even when its tables are shaped like PowerModels', where it used to be reported as ambiguous. PSS/E two-winding transformers stated with CZ=2 or CZ=3 keep the impedance 0.11.7 read; 0.11.8, which this release skips, rescaled it. Two-winding transformers record `psse_magnetizing_referral` and `psse_winding2_ratio` in their extras where they apply, and splitting a three-winding transformer into its star legs keeps each winding's impedance correction table.
