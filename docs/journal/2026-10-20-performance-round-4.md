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

## Polymorphic and successor caches for keys (measured, not kept)

- **The idea.** A row's keys and objects are found by comparing them with the previous
  row's at the same position in the row (`last_keys`, `last_objects`), and on a miss by
  hash. Round 3's ledger (L5) proposed keeping 2–4 recent entries per position
  (polymorphic inline caches, Hölzle, Chambers & Ungar 1991), to be rejected if four
  entries added under 10 points of hit rate.
- **Hit rates, measured first** (logs/b1_key_successors.txt; an instrumented build,
  simulated): four entries add 15–40 points on narrow's objects, events, claims and
  entities. Even so, claims' keys hit only 23.8% at four entries (7.3% at one), and its
  objects 14.8%.
  - The positions are the problem. They count keys from the start of the row, so one
    member more early in a row moves every key after it.
  - Mison (Li et al. 2017) predicts a field's logical position instead. V8's JSON parser
    predicts a key from the object's shape so far (its transition tree), and from the
    previous array element's map (Verwaest, "[json] More feedback-guided json parsing",
    V8 commit fdde8945, 2019).
  - The analogue here predicts a key from the key before it in its object (and an
    object's first key from its parent's key). Simulated, that hits 81.5% of claims'
    keys (7.3% by position), 79.6% of entities' and 78.0% of events'. Labels' map keys
    (languages) are unpredictable either way (66%). Narrow's already hit 100%.
- **Built three ways**, each checked by one comparison of the key's text, on the fused
  row phase. The best, the previous key alone, tried after the position cache misses,
  with no parent context (scripts/key_successors_rejected.diff), measured:

  | corpus | row phase | whole run |
  |---|---|---|
  | claims | −4.4% | −1.9% |
  | events | −3.8% | −2.3% |
  | entities | −4.1% | −0.75% |
  | narrow | +0.7% | +0.35% |
  | labels | +2.4% | +1.0% |

  The variant with a parent context saved more on claims (−6.6%) but cost narrow and
  labels 4.3% each. Moving the miss path out of line lost most of the gain.
- **Why it was not kept.** The hash lookup a prediction saves costs about 132
  instructions on claims (102.4M over some 770,000 lookups that miss the position
  cache). A prediction saves about 53 of them: it still compares the key's text, and
  keeps its table. A gain of about 2% of three corpora's runs, against a loss on two,
  does not pay for the state it adds.
- **Objects.** Predicting an object's state from its parent key would hit claims'
  objects 27.9% of the time (4.1% by position). That was not built: an object found by
  its state still compares its members, which is most of what the hash lookup costs.

## The distinct rows' hashes in an FxHashSet (measured, not kept)

- **The change** (scripts/row_hash_set_not_kept.diff). Each row's schema hash (xxh64)
  was checked against the rows seen before in std's `HashSet`, which hashes each `u64`
  again with SipHash-1-3. `FxHashSet` hashes it with one multiply.
- **Instructions**, whole run, one thread (logs/b2_row_hash_set.txt and
  logs/b4_repeats_and_times.txt): narrow −1.79%, events −0.66%, usual −0.59%, labels
  −0.08%, claims −0.06% and entities 0.00%, each within the same mode of std's random
  seeds. No regression, and the row phase is unchanged.
- **Memory**: unchanged, since the table grows as std's does.
- **Times**: within this machine's noise (ABBA: −3.1% to +4.6%, with the same binary
  drifting 9% between sets).
- **Hash flooding.** Fx is not keyed. This would have added no exposure the row phase
  lacks, since the builder's `Absorbed` already keeps the same hashes in an
  `FxHashSet<u64>`, and key interning hashes the key strings with Fx.
- **Why it was not kept.** The gain is real, but at most 0.1% of the whole run on the
  Wikidata-shaped corpora (labels, claims, entities). It saves no memory and is too
  small to show in time. That is not worth a PR of its own to review and release.

## A fraction's kind from the lexer (measured, not kept)

- **The change** (scripts/number_class_not_kept.diff). The lexer reported a number with a
  point, no exponent and 300 characters or fewer as a fraction, which inference took as
  `FLOAT` without parsing it. Its magnitude is below 10^300, so it is finite and below
  `number_kind`'s 1e308 decline. Numbers with an exponent, and longer ones, were still
  classed by `number_kind` behind the lexer's `serde_json` range check.
- **Test.** The equivalence test it came with (numbers either side of each bound, `i64`'s
  limits, 300- and 301-character fractions, values past `f64`'s range, exponents)
  matched the builders, and caught classing long integers as fractions.
- **Instructions**, one thread (logs/b3_number_class.txt and
  logs/b4_repeats_and_times.txt): row phase −0.14% to −2.00%. Whole run: narrow −1.09%,
  events −0.15%, usual −0.12%, labels −0.10%, claims −0.27% and entities −0.14% (within
  a mode). The schema hashes, fuzz, soundness cases and wikidata-pq's schemas and
  normalised text are all the same.
- **Times**: within noise.
- **Why it was not kept.** It costs a new `Sink` argument and a numeric bound whose
  soundness has to be argued, for at most 0.3% of the whole run on the Wikidata-shaped
  corpora.

## Round 4 closed

Round 4 keeps one change: lexing straight into interned states (#282), at 10–20% of the
row phase and 1–12% of the whole run.
- Every candidate measured after it was below the bar where its review costs less than
  it gains: polymorphic and successor caches, the row-hash set, and the fraction class.
- The ledger's remaining ideas (SIMD-assisted lexing, property tests of the merge
  algebra) wait for a later round.
