<!-- genson-core-scorecard -->
## genson-core performance scorecard

base `c527f253a0aed7ad9f7cf78bf8ace3d564709fc7` → head `bc2cc3da217305c000aad70b3b0956061d5b32ba`, measured 2026-10-24 07:57 UTC ([run](https://github.com/lmmx/polars-genson/actions/runs/40075714329)).

Instructions: one thread, callgrind, by stage. Times: release build, all threads, 6 runs each, interleaved ABBA, median and verdict against the runs' spread (inconclusive unless the instructions moved too).

| corpus | instructions | per byte | wall | CPU | allocations | output |
|---|---|---|---|---|---|---|
| claims | 2,100.9M → 2,128.9M (+1.3%) | 140.1 → 142.0 | 229.2 → 229.1 ms (-0%, inconclusive) | 474.1 → 478.5 ms (+1%, inconclusive) | +0.0% | identical |
| events | 263.0M → 265.4M (+0.9%) | 34.4 → 34.7 | 13.8 → 12.0 ms (-13%, inconclusive) | 41.6 → 41.8 ms (+1%, inconclusive) | +0.0% | identical |
| narrow | 447.9M → 450.1M (+0.5%) | 43.2 → 43.4 | 17.3 → 17.3 ms (+0%, inconclusive) | 60.3 → 60.1 ms (-0%, inconclusive) | +0.0% | identical |
| labels | 195.3M → 196.7M (+0.7%) | 67.6 → 68.0 | 11.9 → 12.1 ms (+1%, inconclusive) | 31.1 → 31.6 ms (+2%, inconclusive) | +0.0% | identical |
| entities | 616.3M → 621.5M (+0.8%) | 183.4 → 185.0 | 69.8 → 71.8 ms (+3%, inconclusive) | 134.4 → 137.9 ms (+3%, inconclusive) | +0.0% | identical |
| usual | 327.6M → 331.4M (+1.1%) | 42.9 → 43.4 | 15.7 → 17.0 ms (+9%, inconclusive) | 54.0 → 50.3 ms (-7%, inconclusive) | +0.0% | identical |
| normalise | 1,902.3M → 1,981.5M (+4.2%) | 126.9 → 132.2 | 262.0 → 273.5 ms (+4%, slower) | 262.4 → 274.0 ms (+4%, slower) | +0.0% | identical |

**Increases:** normalise: `normalise` +79.2M (+4.2%)
**Largest remaining costs:** normalise: `normalise` 1,981.5M (100% of it); events: `rows` 258.7M (97% of it); narrow: `rows` 429.2M (95% of it); labels: `rows` 154.6M (79% of it); usual: `rows` 258.6M (78% of it)

<details><summary>Stages by corpus</summary>

**claims** (2,000 rows, 15.0 MB)

| stage | base | head | change | share of head | allocations (head) | peak live (head) |
|---|---|---|---|---|---|---|
| rows | 758.5M | 767.4M | +1.2% | 36.0% | 1,054,977 (69.4 MB) | 47.6 MB |
| merge | 457.5M | 460.8M | +0.7% | 21.6% | 1,358,341 (163.5 MB) | 177.1 MB |
| write_schema | 180.2M | 180.6M | +0.2% | 8.5% | 92,356 (6.7 MB) | 154.7 MB |
| rewrite | 678.8M | 692.6M | +2.0% | 32.5% | 722,606 (79.1 MB) | 170.4 MB |
| materialise | 0.2M | 0.2M | +1.4% | 0.0% | 838 (0.1 MB) | 170.1 MB |
| reorder | 0.0M | 0.0M | +1.7% | 0.0% | 0 (0.0 MB) | 151.3 MB |
| avro | 2.1M | 2.1M | +3.5% | 0.1% | 5,289 (0.4 MB) | 151.5 MB |
| unattributed | 23.5M | 25.3M | +7.5% | 1.2% |  |  |
| total | 2,100.9M | 2,128.9M | +1.3% | 100.0% |  |  |

Peak RSS (head, whole process): 316 MB

**events** (20,000 rows, 7.6 MB)

| stage | base | head | change | share of head | allocations (head) | peak live (head) |
|---|---|---|---|---|---|---|
| rows | 256.4M | 258.7M | +0.9% | 97.5% | 16,265 (2.8 MB) | 11.6 MB |
| merge | 1.0M | 1.0M | +0.1% | 0.4% | 1,045 (0.2 MB) | 11.9 MB |
| write_schema | 0.1M | 0.1M | +0.5% | 0.0% | 286 (0.0 MB) | 10.5 MB |
| rewrite | 0.0M | 0.0M | -0.4% | 0.0% | 67 (0.0 MB) | 10.5 MB |
| materialise | 0.1M | 0.1M | +0.7% | 0.0% | 501 (0.0 MB) | 10.5 MB |
| reorder | 0.0M | 0.0M | +1.0% | 0.0% | 0 (0.0 MB) | 10.5 MB |
| avro | 1.0M | 1.0M | +2.9% | 0.4% | 1,965 (0.1 MB) | 10.6 MB |
| unattributed | 4.4M | 4.5M | +0.5% | 1.7% |  |  |
| total | 263.0M | 265.4M | +0.9% | 100.0% |  |  |

Peak RSS (head, whole process): 38 MB

**narrow** (100,000 rows, 10.4 MB)

| stage | base | head | change | share of head | allocations (head) | peak live (head) |
|---|---|---|---|---|---|---|
| rows | 427.0M | 429.2M | +0.5% | 95.3% | 1,285 (6.4 MB) | 18.4 MB |
| merge | 0.1M | 0.1M | +0.5% | 0.0% | 129 (0.0 MB) | 20.0 MB |
| write_schema | 0.0M | 0.0M | +0.4% | 0.0% | 107 (0.0 MB) | 15.2 MB |
| rewrite | 0.0M | 0.0M | -0.2% | 0.0% | 20 (0.0 MB) | 15.2 MB |
| materialise | 0.0M | 0.0M | +0.7% | 0.0% | 74 (0.0 MB) | 15.2 MB |
| reorder | 0.0M | 0.0M | +2.0% | 0.0% | 0 (0.0 MB) | 15.2 MB |
| avro | 0.1M | 0.1M | +3.3% | 0.0% | 276 (0.0 MB) | 15.2 MB |
| unattributed | 20.7M | 20.7M | +0.0% | 4.6% |  |  |
| total | 447.9M | 450.1M | +0.5% | 100.0% |  |  |

Peak RSS (head, whole process): 42 MB

**labels** (1,000 rows, 2.9 MB)

| stage | base | head | change | share of head | allocations (head) | peak live (head) |
|---|---|---|---|---|---|---|
| rows | 153.7M | 154.6M | +0.6% | 78.6% | 26,330 (12.3 MB) | 6.8 MB |
| merge | 22.6M | 22.5M | -0.3% | 11.5% | 4,550 (4.5 MB) | 8.5 MB |
| write_schema | 0.2M | 0.2M | +1.0% | 0.1% | 715 (0.1 MB) | 4.1 MB |
| rewrite | 0.5M | 0.5M | -0.4% | 0.3% | 654 (0.0 MB) | 4.2 MB |
| materialise | 1.7M | 1.8M | +1.1% | 0.9% | 7,027 (0.4 MB) | 4.6 MB |
| reorder | 0.3M | 0.3M | +1.0% | 0.2% | 0 (0.0 MB) | 4.5 MB |
| avro | 15.2M | 15.6M | +2.9% | 7.9% | 28,769 (1.6 MB) | 5.8 MB |
| unattributed | 1.0M | 1.1M | +3.6% | 0.5% |  |  |
| total | 195.3M | 196.7M | +0.7% | 100.0% |  |  |

Peak RSS (head, whole process): 40 MB

**entities** (150 rows, 3.4 MB)

| stage | base | head | change | share of head | allocations (head) | peak live (head) |
|---|---|---|---|---|---|---|
| rows | 176.1M | 178.2M | +1.2% | 28.7% | 236,000 (16.6 MB) | 10.7 MB |
| merge | 146.4M | 147.5M | +0.8% | 23.7% | 467,977 (56.1 MB) | 58.3 MB |
| write_schema | 67.1M | 67.2M | +0.2% | 10.8% | 38,636 (2.9 MB) | 53.6 MB |
| rewrite | 7.6M | 7.6M | -0.7% | 1.2% | 9,220 (0.6 MB) | 53.9 MB |
| materialise | 184.3M | 185.4M | +0.6% | 29.8% | 734,240 (43.1 MB) | 96.9 MB |
| reorder | 30.0M | 30.4M | +1.3% | 4.9% | 0 (0.0 MB) | 95.2 MB |
| unattributed | 4.7M | 5.1M | +8.5% | 0.8% |  |  |
| total | 616.3M | 621.5M | +0.8% | 100.0% |  |  |

Peak RSS (head, whole process): 144 MB

**usual** (20,000 rows, 7.6 MB)

| stage | base | head | change | share of head | allocations (head) | peak live (head) |
|---|---|---|---|---|---|---|
| rows | 256.3M | 258.6M | +0.9% | 78.0% | 16,265 (2.8 MB) | 11.6 MB |
| merge | 69.9M | 71.3M | +2.0% | 21.5% | 143,415 (18.9 MB) | 16.3 MB |
| write_schema | 0.1M | 0.1M | +0.6% | 0.0% | 349 (0.0 MB) | 10.5 MB |
| rewrite | 0.2M | 0.2M | +1.0% | 0.1% | 394 (0.0 MB) | 10.5 MB |
| materialise | 0.1M | 0.1M | +0.6% | 0.0% | 432 (0.0 MB) | 10.5 MB |
| reorder | 0.0M | 0.0M | +1.6% | 0.0% | 0 (0.0 MB) | 10.5 MB |
| avro | 0.7M | 0.7M | +3.1% | 0.2% | 1,619 (0.1 MB) | 10.6 MB |
| unattributed | 0.3M | 0.3M | +9.4% | 0.1% |  |  |
| total | 327.6M | 331.4M | +1.1% | 100.0% |  |  |

Peak RSS (head, whole process): 50 MB

**normalise** (2,000 rows, 15.0 MB)

| stage | base | head | change | share of head | allocations (head) | peak live (head) |
|---|---|---|---|---|---|---|
| normalise | 1,902.3M | 1,981.5M | +4.2% | 100.0% | 2,933,109 (298.1 MB) | 484.3 MB |
| unattributed | 0.0M | 0.0M | +0.5% | 0.0% |  |  |
| total | 1,902.3M | 1,981.5M | +4.2% | 100.0% |  |  |

Peak RSS (head, whole process): 1091 MB

</details>
