# 2026-10-25: The typed decoder's columns grown to the batch's expected length (tried, not shipped)

The typed decoder's Arrow builders double their buffers as they fill, and the arrays they
finish keep that capacity. #317 (branch `perf/presize-builders`) grew the columns towards
the length each would have at the end of its batch instead. The output was the same, and
the arrays held a sixteenth or so more than their bytes rather than about half more. But
the decode's CPU did not fall: an exact pre-sizing would have saved at most a few
percent. On wikidata-pq's bench, peak RSS and process CPU fell beyond noise in one of two
runs and within noise in the other. That did not justify about 170 lines and a release,
so it is not shipped.

## The profile

wikidata-pq's claims call, decoding against `claims.avsc` (`to_arrow_strict`, batched as
`normalise_from_parquet` batches it: 16 batches on 4 threads), on the bench's chunks c0,
s30 and s90 with genson 1.5.0, in a genson-core example on a shared 4-core VM.

- **Capacity:** the decoded arrays held 1.42–1.46x their bytes:

  | chunk | slim rows' text | arrays' bytes | capacity |
  |---|---|---|---|
  | c0 | 374.8 MB | 355.9 MB | 519.7 MB (1.46x) |
  | s30 | 212.5 MB | 193.8 MB | 275.8 MB (1.42x) |
  | s90 | 255.5 MB | 237.0 MB | 337.2 MB (1.42x) |

- **Which spare survives `conform`:** `conform` writes new 64-bit string and list offsets,
  freeing the 32-bit ones and their spare capacity (58.9 MB on c0, 36.5 MB on s90). It
  shares the string values and the fixed-width values. So that spare stays in the arrays
  handed to wikidata-pq: 102.9 MB on c0, 57.3 MB on s30 and 62.6 MB on s90 (validity
  adds 1–2 MB).
- **Which columns:** the spare is spread across the string columns in proportion to their
  bytes. The largest were `value.id` (29.0 MB), the reference and qualifier `hash`es
  (14.2 and 10.5 MB), the references' `datavalue__string` and `datavalue_type`, and the
  mainsnak's `datatype`.
- **CPU (perf, s90):** `Decoder::put` 21% of samples, the lexer's strings and values 13%
  and 9%, `clear_page_erms` 10% (the kernel zeroing new pages), memmove 8%, slot lookup
  6%. The memmove includes the strings copied into the columns, which pre-sizing does not
  remove.

## The oracle bound

Each batch's builders were given its exact final sizes (from an earlier run of the same
batch): `StringBuilder::with_capacity`, `Int64Builder::with_capacity` and so on, for
every column at every depth. This bounds what any pre-sizing can save. Six interleaved
pairs per chunk:

- **User CPU:** 2–4% lower (c0 1.94 → 1.88 s, s30 1.08 → 1.03 s, s90 1.12 → 1.10 s on
  average), within noise.
- **Minor page faults:** about halved (c0 25–37K → 11–17K), and system time with them.
- **Capacity:** 1.03–1.09x.

So the doubling's copies are not where the decode's user time goes. What it costs is
memory, and page faults where they are dear.

## The change

- **The columns:** strings, Int64, Float64 and the list offsets were `Vec`s the decoder
  grew itself (`Strings`, `Primitive<T>` in `normalise/typed.rs`). They finished into the
  same arrays (`StringArray::new_unchecked`, `Int64Array::new`, `Float64Array::new`).
  Booleans and validity stayed with Arrow's builders.
- **The growth:** no count of a column's values is at hand before a batch is decoded. The
  root row count fixes almost nothing here, since every large column is nested under
  `claims`, and a counting pass would cost about as much as the decode. So a full column
  grew to the length it would reach at the end of the batch at its rate so far, measured
  by the batch's bytes read (set per row), plus a sixteenth. It grew by at least an eighth
  and never more than doubled.
- **The last-row fix:** the first version (a0d0776) grew to exactly the length needed
  once the batch's last row was being read, and likewise with no progress set. A column
  that was already full when that row began was then reallocated, and possibly copied, at
  every value the row added: quadratic in a large final entity. 127e4f1 applied the
  eighth (and a floor of 64 values) whatever the estimate. A test filled a full column
  with up to 200,000 values in the last row, with no progress set, and mid-batch. It grew
  a logarithmic number of times, each by at least an eighth and at most double. Under the
  first rule it grew 200,000 times.
- **Capacity:** 1.06–1.07x (c0 380.2 MB, s30 205.0 MB, s90 250.7 MB). The spare kept
  past `conform` fell to 14.9, 7.3 and 8.6 MB.

## The output was the same

- **Buffer for buffer:** on c0, s30 and s90, a hash of every buffer's bytes, child by
  child, and of the pruned values, was the same as 1.5.0's, before and after the
  last-row fix.
- **Tests:** genson-core's tests passed (`--features avro,parquet`), as did the Python
  suite on each CI wheel (622 passed, 3 skipped, 2 xfailed).
- **Downstream:** wikidata-pq's claims call on c0 and s90 gave the same claims table (as
  Arrow IPC), lookup and pruned files, byte for byte. The bench's parity check passed
  both times.

## Measured

wikidata-pq's bench, quick tier, 3 rounds, #317's CI wheels against the released 1.5.0
(lmmx/wikidata-pq#121):

| | a0d0776, on wikidata-pq d8f2826 | 127e4f1, on wikidata-pq 45de9b7 |
|---|---|---|
| `process.normalise_in_schema` | 5.8 → 5.7 s, -2.3% ±2.3% | 8.1 → 8.2 s, within noise |
| CPU, process | 11.0 → 10.8 s, **-1.9% ±0.9%** | 15.2 → 15.2 s, within noise |
| ETA: pipeline | 5h43m10s → 5h41m40s, within noise | 7h11m55s → 7h15m44s, within noise |
| ETA: a release | 13h12m46s → 13h15m51s, within noise | 17h00m43s → 16h55m36s, within noise |
| peak RSS, one worker (GB) | 1.759 → 1.729, **-1.7% ±1.3%** | 1.705 → 1.682, within noise |
| peak RSS, all processes (GB) | 4.015 → 3.888, **-3.0% ±0.7%** | 3.869 → 3.790, within noise |
| parity | passed | passed |

The last-row fix did not change the decode on these chunks, so the two runs differ by
noise; the second ran on slower runners. Locally the claims call's peak RSS was about
30 MB lower in 8 of 8 interleaved pairs on c0 and s90 (c0 1,322–1,363 → 1,299–1,327 MB,
s90 940–958 → 917–920 MB).

## Why it did not pay

- **Most of the spare capacity was never at the peak:** the arrays lost about 100 MB of
  it on c0. The worker's peak RSS fell by 23–30 MB, and in all processes by 79–127 MB,
  beyond noise in one run only.
- **Growth is not the decode's cost:** even exact sizes saved only 2–4% of user CPU. The
  time is in the walk (`put`, slot lookup) and the lexer.
