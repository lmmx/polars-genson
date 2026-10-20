<!-- genson-core-scorecard -->
## genson-core performance scorecard

base `5d22498d8169ad1c64597af9424b7b5885368598` → head `58ad87fb5598ceea2cc32aa2d21bffe1a32ab949`, measured 2026-10-20 04:09 UTC.

Instructions: one thread, callgrind, by stage. Times: release build, all threads, 6 runs each, interleaved ABBA, median and verdict against the runs' spread (inconclusive unless the instructions moved too).

| corpus | instructions | per byte | wall | CPU | allocations | output |
|---|---|---|---|---|---|---|
| claims | 2,205.5M → 2,205.7M (+0.0%) | 147.1 → 147.1 | 317.7 → 449.6 ms (+42%, inconclusive) | 483.0 → 554.4 ms (+15%, inconclusive) | +0.0% | identical |
| events | 332.0M → 331.8M (-0.1%) | 43.4 → 43.4 | 23.0 → 26.7 ms (+16%, inconclusive) | 38.0 → 37.7 ms (-1%, inconclusive) | +0.0% | identical |
| narrow | 573.7M → 572.7M (-0.2%) | 55.3 → 55.2 | 30.0 → 34.5 ms (+15%, inconclusive) | 57.6 → 55.4 ms (-4%, inconclusive) | +0.0% | identical |
| labels | 242.1M → 242.2M (+0.0%) | 83.8 → 83.8 | 22.3 → 26.2 ms (+17%, inconclusive) | 31.5 → 33.1 ms (+5%, inconclusive) | +0.0% | identical |
| entities | 642.2M → 642.2M (-0.0%) | 191.1 → 191.1 | 139.4 → 131.0 ms (-6%, inconclusive) | 148.3 → 148.6 ms (+0%, inconclusive) | +0.0% | identical |
| usual | 4,908.5M → 393.9M (-92.0%) | 642.2 → 51.5 | 543.6 → 37.8 ms (-93%, faster) | 996.6 → 53.4 ms (-95%, faster) | -98.5% | identical |
| normalise | 1,907.8M → 1,907.9M (+0.0%) | 127.3 → 127.3 | 435.2 → 430.9 ms (-1%, inconclusive) | 428.7 → 419.5 ms (-2%, inconclusive) | +0.0% | identical |

**Largest reductions:** usual: `rows` 4,583.3M fewer (-93.4%)
**Increases:** usual: `merge` +68.5M (new)
**Largest remaining costs:** normalise: `normalise` 1,907.9M (100% of it); events: `rows` 322.4M (97% of it); narrow: `rows` 550.0M (96% of it); usual: `rows` 322.4M (82% of it); labels: `rows` 177.2M (73% of it)

<details><summary>Stages by corpus</summary>

**claims** (2,000 rows, 15.0 MB)

| stage | base | head | change | share of head | allocations (head) | peak live (head) |
|---|---|---|---|---|---|---|
| rows | 848.4M | 848.4M | -0.0% | 38.5% | 1,055,269 (70.0 MB) | 47.7 MB |
| merge | 458.9M | 459.1M | +0.0% | 20.8% | 1,358,341 (163.5 MB) | 177.1 MB |
| write_schema | 180.5M | 180.5M | -0.0% | 8.2% | 92,356 (6.7 MB) | 154.7 MB |
| rewrite | 685.8M | 685.8M | +0.0% | 31.1% | 722,606 (79.1 MB) | 170.4 MB |
| materialise | 0.2M | 0.2M | -0.7% | 0.0% | 838 (0.1 MB) | 170.1 MB |
| reorder | 0.0M | 0.0M | -0.6% | 0.0% | 0 (0.0 MB) | 151.3 MB |
| avro | 8.1M | 8.1M | +0.1% | 0.4% | 28,248 (2.8 MB) | 151.9 MB |
| unattributed | 23.6M | 23.6M | -0.1% | 1.1% |  |  |
| total | 2,205.5M | 2,205.7M | +0.0% | 100.0% |  |  |

Peak RSS (head, whole process): 301 MB

**events** (20,000 rows, 7.6 MB)

| stage | base | head | change | share of head | allocations (head) | peak live (head) |
|---|---|---|---|---|---|---|
| rows | 322.4M | 322.4M | +0.0% | 97.2% | 16,289 (2.8 MB) | 11.6 MB |
| merge | 1.0M | 1.0M | +0.0% | 0.3% | 1,045 (0.2 MB) | 11.9 MB |
| write_schema | 0.1M | 0.1M | +0.0% | 0.0% | 286 (0.0 MB) | 10.5 MB |
| rewrite | 0.0M | 0.0M | +0.0% | 0.0% | 67 (0.0 MB) | 10.5 MB |
| materialise | 0.1M | 0.1M | -0.1% | 0.0% | 501 (0.0 MB) | 10.5 MB |
| reorder | 0.0M | 0.0M | -0.9% | 0.0% | 0 (0.0 MB) | 10.5 MB |
| avro | 3.5M | 3.5M | +0.2% | 1.1% | 10,573 (1.5 MB) | 10.9 MB |
| unattributed | 4.8M | 4.6M | -4.1% | 1.4% |  |  |
| total | 332.0M | 331.8M | -0.1% | 100.0% |  |  |

Peak RSS (head, whole process): 46 MB

**narrow** (100,000 rows, 10.4 MB)

| stage | base | head | change | share of head | allocations (head) | peak live (head) |
|---|---|---|---|---|---|---|
| rows | 550.0M | 550.0M | -0.0% | 96.1% | 1,301 (6.4 MB) | 18.4 MB |
| merge | 0.1M | 0.1M | +0.1% | 0.0% | 129 (0.0 MB) | 20.0 MB |
| write_schema | 0.0M | 0.0M | +0.0% | 0.0% | 107 (0.0 MB) | 15.2 MB |
| rewrite | 0.0M | 0.0M | +0.0% | 0.0% | 20 (0.0 MB) | 15.2 MB |
| materialise | 0.0M | 0.0M | -0.2% | 0.0% | 74 (0.0 MB) | 15.2 MB |
| reorder | 0.0M | 0.0M | -0.3% | 0.0% | 0 (0.0 MB) | 15.2 MB |
| avro | 0.9M | 0.9M | +0.1% | 0.2% | 1,368 (0.8 MB) | 15.6 MB |
| unattributed | 22.6M | 21.6M | -4.4% | 3.8% |  |  |
| total | 573.7M | 572.7M | -0.2% | 100.0% |  |  |

Peak RSS (head, whole process): 46 MB

**labels** (1,000 rows, 2.9 MB)

| stage | base | head | change | share of head | allocations (head) | peak live (head) |
|---|---|---|---|---|---|---|
| rows | 177.2M | 177.2M | +0.0% | 73.2% | 27,329 (14.3 MB) | 6.8 MB |
| merge | 22.6M | 22.6M | +0.0% | 9.3% | 4,550 (4.5 MB) | 8.5 MB |
| write_schema | 0.2M | 0.2M | +0.1% | 0.1% | 715 (0.1 MB) | 4.1 MB |
| rewrite | 0.5M | 0.5M | -0.0% | 0.2% | 654 (0.0 MB) | 4.2 MB |
| materialise | 1.8M | 1.8M | +0.0% | 0.7% | 7,027 (0.4 MB) | 4.6 MB |
| reorder | 0.3M | 0.3M | -0.3% | 0.1% | 0 (0.0 MB) | 4.5 MB |
| avro | 38.4M | 38.5M | +0.3% | 15.9% | 125,903 (10.0 MB) | 9.7 MB |
| unattributed | 1.1M | 1.0M | -0.9% | 0.4% |  |  |
| total | 242.1M | 242.2M | +0.0% | 100.0% |  |  |

Peak RSS (head, whole process): 46 MB

**entities** (150 rows, 3.4 MB)

| stage | base | head | change | share of head | allocations (head) | peak live (head) |
|---|---|---|---|---|---|---|
| rows | 200.0M | 200.0M | -0.0% | 31.1% | 236,303 (17.6 MB) | 10.8 MB |
| merge | 146.8M | 146.9M | +0.1% | 22.9% | 467,977 (56.1 MB) | 58.3 MB |
| write_schema | 67.2M | 67.2M | +0.0% | 10.5% | 38,636 (2.9 MB) | 53.6 MB |
| rewrite | 7.6M | 7.6M | +0.0% | 1.2% | 9,220 (0.6 MB) | 53.9 MB |
| materialise | 185.6M | 185.6M | -0.0% | 28.9% | 734,240 (43.1 MB) | 96.9 MB |
| reorder | 30.2M | 30.1M | -0.3% | 4.7% | 0 (0.0 MB) | 95.2 MB |
| unattributed | 4.7M | 4.7M | -0.0% | 0.7% |  |  |
| total | 642.2M | 642.2M | -0.0% | 100.0% |  |  |

Peak RSS (head, whole process): 149 MB

**usual** (20,000 rows, 7.6 MB)

| stage | base | head | change | share of head | allocations (head) | peak live (head) |
|---|---|---|---|---|---|---|
| rows | 4,905.7M | 322.4M | -93.4% | 81.8% | 16,289 (2.8 MB) | 11.6 MB |
| merge | 0.0M | 68.5M | new | 17.4% | 143,415 (18.9 MB) | 16.3 MB |
| write_schema | 0.1M | 0.1M | +2.2% | 0.0% | 349 (0.0 MB) | 10.5 MB |
| rewrite | 0.2M | 0.2M | +0.2% | 0.1% | 394 (0.0 MB) | 10.5 MB |
| materialise | 0.1M | 0.1M | -0.2% | 0.0% | 432 (0.0 MB) | 10.5 MB |
| reorder | 0.0M | 0.0M | -0.2% | 0.0% | 0 (0.0 MB) | 10.5 MB |
| avro | 2.3M | 2.3M | +0.2% | 0.6% | 6,501 (1.2 MB) | 10.9 MB |
| unattributed | 0.0M | 0.3M | +569.6% | 0.1% |  |  |
| total | 4,908.5M | 393.9M | -92.0% | 100.0% |  |  |

Peak RSS (head, whole process): 56 MB

**normalise** (2,000 rows, 15.0 MB)

| stage | base | head | change | share of head | allocations (head) | peak live (head) |
|---|---|---|---|---|---|---|
| normalise | 1,907.8M | 1,907.9M | +0.0% | 100.0% | 2,933,097 (298.1 MB) | 484.3 MB |
| unattributed | 0.0M | 0.0M | +0.0% | 0.0% |  |  |
| total | 1,907.8M | 1,907.9M | +0.0% | 100.0% |  |  |

Peak RSS (head, whole process): 1084 MB

</details>
