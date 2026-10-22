| chunk | entities | process wall (s) | process CPU (s) | cores | peak RSS (MB) | imports (s) | minor faults |
|---|---|---|---|---|---|---|---|
| c0 | | 6.54 (6.14–9.51) | 20.3 | 3.1 | 2,072 | 0.45 | 108,238 |
| r10 | | 0.48 (0.48–0.72) | 1.3 | 2.7 | 443 | 0.37 | 12,704 |
| r30 | | 0.25 (0.24–0.25) | 0.6 | 2.3 | 262 | 0.41 | 5,718 |
| r50 | | 0.33 (0.33–0.53) | 0.8 | 2.5 | 309 | 0.40 | 11,065 |
| r70 | | 0.24 (0.22–0.40) | 0.5 | 1.9 | 253 | 0.34 | 6,651 |
| r90 | | 0.31 (0.26–0.35) | 0.4 | 1.3 | 187 | 0.38 | 5,465 |
| s10 | | 0.46 (0.44–0.50) | 1.3 | 2.8 | 533 | 0.42 | 10,512 |
| s30 | | 1.16 (1.09–1.78) | 3.8 | 3.3 | 1,186 | 0.39 | 23,299 |
| s50 | | 0.26 (0.26–0.39) | 0.6 | 2.4 | 333 | 0.37 | 6,976 |
| s70 | | 0.80 (0.75–0.89) | 2.3 | 2.9 | 808 | 0.34 | 17,053 |
| s90 | | 1.43 (1.33–2.27) | 4.5 | 3.2 | 1,418 | 0.35 | 25,052 |

sum of median process wall over chunks: 12.3 s

| step | wall (s) | share | CPU (s) | cores | minor faults | max peak over start (MB) |
|---|---|---|---|---|---|---|
| genson.normalise_from_parquet[claims] | 9.36 | 76.4% | 31.2 | 3.3 | 132,316 | 1,756 |
| process.normalise_sitelinks (genson.normalise_json) | 0.79 | 6.5% | 2.3 | 2.9 | 9,779 | 16 |
| genson.normalise_from_parquet[labels] | 0.58 | 4.7% | 0.9 | 1.6 | 6,514 | 130 |
| genson.normalise_from_parquet[descriptions] | 0.48 | 3.9% | 0.9 | 1.9 | 8,241 | 91 |
| polars.sink_parquet[links] | 0.30 | 2.4% | 0.3 | 1.0 | 12,848 | 38 |
| process.n_ids[check_ids] | 0.25 | 2.0% | 0.3 | 1.3 | 8,816 | 3 |
| polars.read_parquet[process] | 0.12 | 0.9% | 0.1 | 1.2 | 18,630 | 49 |
| genson.normalise_from_parquet[aliases] | 0.10 | 0.8% | 0.2 | 1.7 | 1,895 | 5 |
| process.check_ids[claims] | 0.06 | 0.5% | 0.1 | 1.3 | 1,809 | 2 |
| process.check_ids[labels] | 0.05 | 0.4% | 0.1 | 1.3 | 3,034 | 3 |

process() wall outside the wrapped steps (summed): 0.13 s (1.0%)

genson stages (summed over chunks, medians):
  genson.normalise_from_parquet[claims]: write_parquet 4.08, parse+normalise+decode 1.84, infer 1.40, extract_invariants 1.28, read_parquet 0.66, conform 0.34
  genson.normalise_from_parquet[labels]: infer 0.26, write_parquet 0.16, parse+normalise+decode 0.06, read_parquet 0.06, conform 0.00
  genson.normalise_from_parquet[descriptions]: write_parquet 0.20, infer 0.12, read_parquet 0.09, parse+normalise+decode 0.07, conform 0.01
  genson.normalise_from_parquet[aliases]: write_parquet 0.05, infer 0.03, read_parquet 0.01, parse+normalise+decode 0.01, conform 0.00
