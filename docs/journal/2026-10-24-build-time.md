# 2026-10-24: Wheel build time

The wheel jobs took 10 to 20 minutes, and the slowest job sets how long CI takes
(polars-genson#300). #301 gives the Polars crates 16 codegen units. #303 (stacked on it)
builds Polars without the `object` feature, and takes avrotize without its default
features so that reqwest can be left out once avrotize allows it. This entry records the
measurements behind both, and the ideas that did not help. Scripts and logs are in
`2026-10-24-build-time/`.

## Current State

- **Profile:** `[profile.release]` has `codegen-units = 1`, `lto = "thin"` and
  `opt-level = 2` for every crate. #301 overrides the Polars crates to 16 codegen units.
- **Critical path:** polars-core, then polars-ops, polars-plan and polars-expr, then
  polars-genson-py. polars-genson-py's 90-270 s is thin LTO over the whole dependency
  graph, not its own code (see "polars-genson-py on 16 codegen units" below).
- **Host builds:** pyo3-polars-derive, a proc-macro, depends on polars-plan. So
  polars-core, polars-ops, polars-compute, polars-plan and polars-arrow are built a second
  time for the host, at the build-override profile's opt-level 0. That is 120 s of CPU on
  4 cores (155 duplicated units), alongside the critical path rather than on it.
- **avrotize:** genson-core calls only `jsons_to_avro`, but avrotize depends on reqwest
  (blocking, default TLS) unconditionally, to fetch schemas and `$ref`s over http(s).
  That brings tokio, hyper, h2 and native-tls, which uses OpenSSL on Linux: 55 of the
  101 crates avrotize builds. aarch64 and armv7 build OpenSSL from source
  (`--features vendor-openssl`), 140-230 s of CPU.
- **Windows:** every crate takes about twice as long as on the Linux runner (the runners
  are 4 CPUs each), and Windows is the slowest job in every run.
- **Duplicate versions:** hashbrown 0.15/0.16/0.17, foldhash, getrandom, base64,
  num-bigint and zstd 0.13/0.14 all come from upstream crates (planus, halfbrown, ahash,
  uuid, serde-pickle, polars-arrow). None of them is genson's to choose, and there is one
  zstd-sys.

## Measured

- **Setup:** `harness.py` applies the same changes to `CI.yml` on each experiment
  branch: sccache off, `--timings` passed to maturin, and cargo's timing report uploaded
  for each job. Each branch is dispatched with `workflow_dispatch`, which publishes
  nothing. `collect.py` reads the reports.
- **Noise:** identical runs differed by up to 1.5x in wall time (Windows 606-915 s,
  macOS x86_64 588-650 s). `norm.py` scales each run's cargo total by the runner's
  speed. The speed is the summed duration of crates that no variant changes
  (arrow-cast, arrow-ord, arrow-select, arrow-arith, parquet), relative to that
  platform's median. This brought repeats within a few percent of each other. The
  scaling does not hold for no-LTO, where those crates themselves change.
- **Results** (`logs/normalised_wall.txt`; seconds of cargo build, each run, scaled):

| variant | linux x86_64 | linux aarch64 | linux armv7 | windows x64 | macos aarch64 | macos x86_64 |
|---|---|---|---|---|---|---|
| master | 425 / 429 / 434 | 537 / 588 / 590 | 580 / 607 / 610 | 985 / 1010 / 1064 | 721 / 753 / 768 | 720 / 749 / 784 |
| #301 | 365 / 366 / 370 | 484 / 493 / 525 | 489 / 508 / 530 | 909 / 923 / 940 | 608 / 625 / 633 | 660 / 666 / 730 |
| #301, avrotize without reqwest | 345 / 348 / 362 | 425 / 446 / 449 | 446 / 448 / 451 | 902 / 907 / 926 | 613 / 628 / 642 | 668 / 674 / 710 |
| #301, Polars without `object` (#303) | 347 / 347 | 493 / 494 | 476 / 511 | 857 / 912 | 621 / 626 | 614 / 660 |
| avrotize without reqwest, polars-genson-py on 16 codegen units | 360 | 438 | 451 | 877 | 639 | 648 |
| #301, Polars above polars-compute at opt-level 1 | 392 | 501 / 552 | 580 / 580 | 982 / 1011 | 617 / 684 | 684 / 739 |
| #301, Windows on a Dev Drive | - | - | - | 938 / 954 | - | - |

- **#301:** 9-18% shorter than master on every platform.
- **avrotize without reqwest:** 5-12% shorter again on Linux, from 50 fewer crates
  (595 to 545 units) and no OpenSSL source build. Windows and macOS are unchanged:
  reqwest uses the system TLS there, and the crates it adds are off the critical path.
  This was measured with avrotize patched in from a branch
  (`avrotize-optional-http.patch`: reqwest behind a default `http` feature; off, an
  http(s) URL is an error). avrotize's tests pass with and without the feature. genson
  needs an avrotize release with the patch before it applies.
- **Polars without `object`:** about 3% shorter. Linux x86_64 is about 5% faster,
  Windows and macOS x86_64 about 3%, and the other platforms are within noise.
  `schema_to_json` sees an Object column as Binary with or without the feature, and the
  Python suite passes on the CI wheel (600 passed, 3 skipped, 2 xfailed).

## Ruled out

- **polars-genson-py on 16 codegen units:** no change. Its own time was 132-266 s,
  within the 92-275 s it took without the override. Its time is thin LTO, which these settings do not
  change.
- **No LTO** (`lto = false`): polars-genson-py drops to 9-29 s, but every rlib then
  emits machine code as well as bitcode. Total CPU rose (Windows 3,851-4,002 s against
  2,489-2,809 s), and raw wall time was no better (Windows 1,025-1,073 s against
  833-937 s; armv7 377-585 s against 443-467 s). Runtime also got worse: the genson-core
  scorecard (`logs/scorecard_no_lto.md`) shows inference instructions up 0.5-1.3% and
  normalise up 4.2% in instructions and time ("slower"). On the CI wheels
  (`logs/runtime_wheels.txt`, bench/workloads.py, 7 rotated rounds) every workload's user
  CPU was within noise.
- **Polars at opt-level 1:** no shorter (polars-core takes as long; polars-expr and
  polars-plan are shorter, but the total is the same or longer), and it risks the speed
  of the conversion from Arrow into Polars. Not kept.
- **Dev Drive on Windows** (`samypr100/setup-dev-drive`, target directory on ReFS): 938
  and 954 s against 909-940 s.
- **Defender real-time scanning off on Windows:** not tried. Turning it off was refused
  in this environment as weakening security.
- **sccache on Windows:** on a second identical run, Linux x86_64 missed 2 compiles and
  took 176 s for the build step, against 376-552 s cold. Windows missed 84 (polars-core, polars-plan and
  parquet among them) and took 762 s, against 649-981 s cold. `/Brepro` in `CFLAGS`,
  `CXXFLAGS`, `ARFLAGS` and `ASMFLAGS` (no timestamps in MSVC objects) brought the misses
  to 56, but polars-core still missed, and the time was 787-813 s. Why Windows misses
  is not found. linux aarch64 also missed 35, polars-core among them.
- **zstd-sys build script** (25-47 s): it runs at the start, beside other crates, off
  the critical path. Linking a system zstd instead would need it bundled into every wheel.
- **Duplicate versions:** see "Current State". None of them is genson's to choose.

## Missing

- An avrotize release with reqwest behind a default `http` feature. With it, #303's
  `default-features = false` takes effect, and CI's `--features vendor-openssl` (aarch64,
  armv7) and `yum install -y openssl-devel` (x86_64) can go.
- Why sccache misses on Windows on an unchanged rerun. `SCCACHE_LOG=debug` on two runs
  would show which compile's hash changes first.
- Windows is still the slowest job (about 900 s scaled, against 350-650 s elsewhere).
  Nothing tried here closes that gap.
