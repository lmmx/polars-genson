# Benchmarks

`compare.py` times polars-genson builds against each other through the Python API, each
run in a fresh process:

```sh
just bench                  # the two newest releases on PyPI
just bench 1.2.1 ..         # a release against this checkout
just bench --rounds 9 --workloads claims_infer,claims_parquet 1.1.0 1.2.1
```

A build is a release on PyPI or a checkout of this repository, whose `polars-genson-py`
is built into the environment. The first build named is the baseline. Environments are
kept under `.envs/` (Python 3.13, `polars-genson[polars]` and the Polars release it
resolves to), and the claims and flat inputs are generated into `data/` the first time.

A release and a checkout differ in how they were built as well as in their code: the
release wheels are built in manylinux containers by CI. A checkout of the code of 1.2.1
built here took 7% more user time on `claims_parquet` than the 1.2.1 wheel (3 rounds,
ranges 3.54–3.62 s and 3.35–3.41 s). To measure a change in the code, compare two
checkouts (a worktree of its parent, and the change).

## Workloads

`workloads.py` runs one workload once and times the operation alone:

| workload | operation | input |
|---|---|---|
| `labels_infer` | `infer_json_schema(avro=True)` | `data/labels.parquet` 30 times over: 30,000 rows of Wikidata labels maps, 184 MB of JSON |
| `labels_normalise` | `normalise_json` | the same |
| `claims_infer` | `infer_from_parquet(avro=True)` with wikidata-pq's claims options | `data/claims_200.parquet`: 200 rows of Wikidata-like claims, 217 MB of JSON (`claims_rows.py 200`, seed 0) |
| `claims_parquet` | `normalise_from_parquet(typed=True)`, the same options, to a Parquet file | the same |
| `flat_parquet` | `normalise_from_parquet(typed=True)` to a Parquet file | `data/flat_2000000.parquet`: 2,000,000 rows of an id, a label, a count and a score, 151 MB of JSON |

Each takes 0.2 to 2 s of wall time, long enough that the process's start-up, imports and
reading the input (left out of the timing) do not dominate.

## Method

For each workload, every build makes one warm-up run (discarded), then 5 rounds of one
run each, the builds' order rotated from round to round. Each run reports:

- **user CPU time** of all threads: the work the code does;
- **wall time**: what a caller waits, and the measure that a change in parallelism moves;
- system CPU time and the process's peak memory, reported beside them.

On a virtual machine, wall and system time also carry the host's cost of backing the
guest's memory. On the 4-CPU machine of the results below, eight processes of
`labels_normalise` took 0.12 to 2.9 s of system time and 0.61 to 2.6 s of wall time, the
slow ones with half as many page faults again, while their user time stayed within 1.5
to 1.8 s. One thread instead of four did not narrow the spread (1.46 to 2.50 s of wall
time).

User CPU and wall time are judged separately: a build is called faster or slower than
the baseline only if its range and the baseline's do not overlap; otherwise it is "within
noise". Wall time, with system time in it, resolves only large differences. Two sets of
5 runs from the same distribution are disjoint with probability 2/252 (0.8%). More
rounds (`--rounds`) resolve smaller differences.

Every comparison is appended to `history/runs.jsonl`, one line per build and workload
with each round's measurements, the Polars version, the CPU count and the load average.

## Latest results

1.2.0 against 1.2.1 on Polars 2.0.0, 4 CPUs, load average 1.3; seconds (memory in MB),
median (min–max). Every verdict, of user CPU and of wall time, was "within noise":

| workload | polars-genson | user CPU | wall | system CPU | peak memory |
|---|---|---|---|---|---|
| labels_infer | 1.2.0 | 0.72 (0.67–0.79) | 0.24 (0.22–0.29) | 0.03 (0.02–0.12) | 173 |
| labels_infer | 1.2.1 | 0.74 (0.66–0.76) | 0.25 (0.20–0.36) | 0.03 (0.01–0.30) | 173 |
| labels_normalise | 1.2.0 | 1.61 (1.56–1.86) | 0.84 (0.72–1.73) | 0.17 (0.12–1.09) | 605 |
| labels_normalise | 1.2.1 | 1.66 (1.50–1.86) | 0.77 (0.67–1.31) | 0.16 (0.14–0.69) | 604 |
| claims_infer | 1.2.0 | 1.02 (0.94–1.06) | 0.55 (0.42–0.58) | 0.22 (0.10–0.29) | 531 |
| claims_infer | 1.2.1 | 0.96 (0.93–1.06) | 0.46 (0.40–0.50) | 0.14 (0.11–0.24) | 532 |
| claims_parquet | 1.2.0 | 3.53 (3.31–3.93) | 1.89 (1.68–2.66) | 0.65 (0.56–2.85) | 2007 |
| claims_parquet | 1.2.1 | 3.18 (3.16–3.38) | 1.82 (1.56–2.54) | 0.74 (0.47–2.72) | 2013 |
| flat_parquet | 1.2.0 | 1.47 (1.40–1.68) | 0.56 (0.55–0.60) | 0.15 (0.11–0.17) | 570 |
| flat_parquet | 1.2.1 | 1.47 (1.26–1.57) | 0.55 (0.46–0.97) | 0.13 (0.12–1.03) | 574 |

1.2.1 encodes a Parquet file's row groups side by side (#288). Claims are written as one
row group, and in `flat_parquet` the write took 0.13 s of a profiled run's 0.81 s
(inference 0.47 s), so neither shows it: the writer alone is measured by
`docs/journal/2026-10-21-row-group-parallel-write/scripts/`.

## History

`history/pytest_benchmark.csv` holds the earlier benchmark's results for releases 0.2.2
to 1.2.0: pytest-benchmark over the first 10 rows of `data/labels.parquet` (about 60 kB
of JSON), 20 rounds of 10 iterations in one process, one row per release and test, in
milliseconds. At 2 to 4 ms an operation, its differences between recent releases were
within its run-to-run variation: 1.2.0's medians are 20 to 31% above 1.1.0's, and
the two releases run alternately in one session were as fast as each other or 1.2.0
faster.
