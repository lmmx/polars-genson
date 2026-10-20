# Performance round 4: the row phase, from the research ledger

2026-10-20. Starts from fix/forced-map-over-list (#281), the top of the round-3 stack. Logs
and scripts are in `2026-10-20-performance-round-4/`. The techniques come from round 3's
research ledger (`2026-10-20-performance-round-3/literature.md`: L3, L4, L5, L15), with
what this round read added to it below.

Each change is measured against its exact parent, each revision built into a build
directory of its own, the binaries checked to differ. The workload is the scorecard's
(`genson-core/examples/scorecard.rs`):

- instructions by callgrind on one thread, the row phase being `genson_core::phase::rows`
  (inclusive);
- allocations by the `alloc-stats` debug build on one thread;
- time by the release build on all four threads, interleaved ABBA.

Most corpora run for 10–30 ms on a shared machine. An unchanged stage moved by up to 8%
between runs while anything else ran, so times were taken with nothing else running, and
instructions decide where they disagree.

## Baseline at #281 (logs/a1_instructions.txt, the `fl` rows)

| corpus | rows | bytes | row phase | whole run | row phase per byte |
|---|---|---|---|---|---|
| narrow | 100,000 | 10.4 MB | 551.7M | 933.4M | 53.1 |
| events | 20,000 | 7.6 MB | 321.8M | 505.6M | 42.1 |
| usual | 20,000 | 7.6 MB | 321.7M | 569.1M | 42.1 |
| labels | 1,000 | 2.9 MB | 176.8M | 373.6M | 61.2 |
| claims | 2,000 | 15.0 MB | 844.6M | 2,527.6M | 56.3 |
| entities | 150 | 3.4 MB | 199.1M | 987.0M | 59.3 |

- On narrow the lexer's own functions are 253.8M of the row phase (46%), and
  `RowStates`' 238.3M (43%), as round 3's ledger found (45% and 41%). The rest is the
  brace scan (memchr2), key hashing and the row-hash set

## Lexing straight into interned states (perf/lex-into-states)

- **The work.** Inference read each row twice.
  - The lexer wrote a token tape: a `Tok` per key and value, each container's token
    back-patched with its length and end.
  - Then `RowStates::value` walked the tape, dispatching on each token again.
  - Deforestation (Wadler 1990): a producer composed with its consumer needs no
    intermediate structure. Inference consumes every token, so this is the case
    On-Demand JSON (Keiser & Lemire 2024) gains least from; the ledger (L4) expected
    10–20% of the row phase and set the bar for rejection at 8%.
- **The change.** The lexer hands what it reads to a `Sink` as it reads it, in the text's
  order: a value's parts before the value, and a member's key before its value.
  - `Tape` is the sink that writes the token tape, exactly as before, for the typed
    decoder.
  - `States` is inference's sink. It joins a list's items as they come, looks a key up
    as it is read, and interns an object when it closes.
  - The lexer's checks (serde_json's rules) are the same code for both sinks, so both
    accept and reject the same text.
- **What the sink learns as it goes**, where the tape walk knew it up front:
  - an object finds its keys by hash from its 17th distinct member on, where the walk
    decided from the token count; both give a key given again the first one's position;
  - a list's length, for the long-list decline, is counted;
  - whether the row is an object is its first byte after whitespace, which the lexer
    then checks;
  - an integer of 18 digits or fewer is an `integer` from the lexer's digit count,
    without being parsed again.
- **Allocation.** The positions maps of wide objects are kept for the next to reuse,
  rather than allocated per object.
- **Inlining.** The sink's methods and the lexer's small helpers (`peek`, `ws`,
  `expect`, `literal`, `string`) are `#[inline]`. Without it, callgrind's line costs put
  about a tenth of the fused row phase on entering and leaving the sink's methods.
- **Instructions**, one thread (logs/a1_instructions.txt):

  | corpus | row phase, #281 | fused | fused and inlined | row phase | whole run |
  |---|---|---|---|---|---|
  | narrow | 551.7M | 480.4M | 438.8M | −20.5% | −12.1% |
  | events | 321.8M | 283.3M | 263.8M | −18.0% | −11.5% |
  | usual | 321.7M | 283.2M | 263.8M | −18.0% | −10.2% |
  | labels | 176.8M | 165.5M | 156.8M | −11.3% | −5.3% |
  | claims | 844.6M | 786.8M | 756.5M | −10.4% | −3.0% |
  | entities | 199.1M | 185.8M | 178.8M | −10.2% | −1.1% |

  - Per byte, the row phase goes from 53.1 to 42.3 instructions on narrow, 42.1 to 34.5
    on events, and 56.3 to 50.5 on claims.
  - The decoder's tape path is the same code monomorphised for `Tape`. The normalise
    corpus's decode stage is 1,908.7M against 1,908.8M (0.00%).
- **Allocations**, one thread (logs/a2_allocations.txt): equal or fewer on every corpus,
  peak live heap within 0.002%.
  - The rows stage on labels: 27,329 to 26,330 allocations (−3.7%), 14.3 to 12.3 MB.
  - The first prototype allocated a positions map per wide object (grown from 32
    entries), which had labels at 28,029 allocations and 15.1 MB: above the parent until
    the maps were reused.
- **Times**, 8 rounds of 15 measured runs each, nothing else running (logs/a3_times.txt),
  median wall then CPU:

  | corpus | #281 | this | wall | CPU |
  |---|---|---|---|---|
  | narrow | 22.2 ms | 17.9 ms | −19.6% | −24.1% |
  | events | 16.1 ms | 13.4 ms | −16.5% | −18.0% |
  | usual | 24.7 ms | 21.9 ms | −11.4% | −15.6% |
  | labels | 19.6 ms | 19.1 ms | −2.9% | −3.9% |
  | claims | 301.4 ms | 304.3 ms | +1.0% | −2.2% |
  | entities | 93.8 ms | 95.5 ms | +1.8% | +1.7% |

  Claims and entities move within the spread of their runs, as their instructions
  predict: the row phase is a third of claims and a fifth of entities.
- **Parity** (logs/a4_parity.txt):
  - every corpus has the same schema hash;
  - round 3's fuzz_lossless.py (seeds 1–5, 6,400 sets) finds the same two sets as
    #281, both written as JSON text;
  - soundness_cases.py loses nothing;
  - wikidata-pq's five columns keep the same Avro schemas and normalised text on
    chunk_0 and rowsw.
- **Tests.** The interned-row equivalence tests (interned states against each row's
  builder, 1,200 sets) and the forced-map ones (442 sets against the builders) pass
  unchanged. So do the typed decoder's tests on the tape path.
