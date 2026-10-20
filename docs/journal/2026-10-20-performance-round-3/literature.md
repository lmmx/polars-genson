# Research ledger: published techniques against polars-genson's measured costs

2026-10-20. Code read at `wt-stack` (v1.0.4 tip, d38635e). Measured costs are from
docs/journal/2026-10-18-performance-archaeology.md (PA), 2026-10-20-performance-round-3/logs
(R3), and the callgrind files left in the scratchpad (`cg_rsl4_*.out`, one thread, at the
row-state-lookups step). I measured two new things myself (marked **[new]**). Everything
else under "hypothesis" is my expectation, not a measurement. No repository was changed.

Citation check: **[F]** = I fetched and read the full text; **[A]** = I fetched the
abstract/landing page or the HTML text; **[C]** = I checked the bibliographic record
(Crossref metadata: DOI, venue, pages) but did not read the text.

## New measurements made for this ledger

- **[new] Per-token cost of the row phase** (`cg_rsl4_narrow200k.out`, exclusive cost
  grouped by function). narrow200k is 25.1 MB, 200,000 rows and 3,780,539 tokens (keys,
  values and containers, counted with a Python walk). Of 1.217G instructions, the
  `tape::Lexer` functions take 546M: **144 instructions per token, 21.7 per byte**.
  `RowStates` interning takes 497M: 131 per token. memcmp is 30M. On events20k the
  split is lexer 187M and interning 178M of 458M. On rel7k it is lexer 233M, interning
  243M and allocation 522M of 1.39G at that step. For comparison, simdjson's DOM
  parser averages 5.1 instructions per byte in Keiser & Lemire's Table 3, and the
  simdjson paper reports 8.3 on average.
- **[new] Sitelinks shapes** (script: `sitelinks_shapes.py` here). `prod/rowsw.parquet`
  has 2,185 rows and 597,759 links (274 per row). The rows have **60 distinct shapes**
  (the 60 well-known items repeated). The link values have **2 distinct shapes in the
  whole column**: `{site,title,badges:[],url}` and `{site,title,badges:[string],url}`.
  2,119 rows hold both. `rel7k/chunk_0.parquet` has 7,128 rows, about 1 link per row,
  2,291 distinct row shapes and the same 2 value shapes.

---

## L1. Forced maps on the interned path, converted once per distinct state (H2): try first

- **Sources.** Spoth, Kennedy, Lu, Hammerschmidt, Liu, "Reducing Ambiguity in Json Schema
  Discovery", SIGMOD 2021, doi:10.1145/3448016.3452801,
  https://odin.cse.buffalo.edu/papers/2021/SIGMOD-JsonSchemas.pdf **[F]**. Filliâtre &
  Conchon, "Type-safe modular hash-consing", ML Workshop 2006, doi:10.1145/1159876.1159880
  **[C]**. Baazizi, Ben Lahmar, Colazzo, Ghelli, Sartiani, "Schema Inference for Massive
  JSON Datasets", EDBT 2017, https://openproceedings.org/2017/conf/edbt/paper-62.pdf
  **[F]**; journal version: Baazizi, Colazzo, Ghelli, Sartiani, VLDB J. 28(4):497–521,
  2019, doi:10.1007/s00778-018-0532-7 **[C]**.
- **Insight.** Jxplain decides which paths are collections (maps) in its own pass. A
  later pass then merges each collection's nested types into one type: "If the elements
  of the input bag are determined to be collections, nested types are merged together"
  (§4.1–4.2). Baazizi et al. need fusion to be associative (and commutative) so that it
  can be distributed (EDBT §5, Theorems 5.4–5.5). Hash-consing shares equal subterms, so
  a function of a term can be memoised by the term's id.
- **Correspondence.** A field forced to be a map is a collection path known before any
  row is read. Today that one fact disables the whole interned path:
  `process_json_strings_interned` returns `None` when `force_field_types` has a "map"
  (schema.rs:513). The fallback runs serde validation, builds each row's full
  `SchemaNode` tree, runs `apply_force_field_types_to_node` (`to_schema` of the property,
  then `forced_map_value_schema` → `map_value_schema` → `check_unifiable_schemas`, then
  read it back), and merges. The conversion is a function of the row's joined record
  state at that property. The usual path also forces the joined row node, not each
  object, since `visit_properties_mut` descends lists through their items. Interned
  states are hash-consed, so the conversion can be memoised by `StateId`.
- **Adaptation (small).** Keep `RowStates::row` as it is. After the row's state is
  built, map it through `force(state)`, memoised per state. It rewrites `Strat::Object`
  properties whose key is forced to be a map into a new `Strat::Forced(fid)`. `fid`
  indexes a per-block table of forced schemas (a `Value`, or an `Id` in a per-block
  `Dag`). Compute each entry exactly as the usual path does: absorb the property's state
  into a fresh `SchemaNode` (`absorb_interned`, which the interned tests already check
  against the builders), `to_schema`, `apply_force_field_types` on the holder object,
  then read it back. Leave an object with no keys unchanged, as `has_keys` does. In
  `absorb_interned_strategy*`, a `Forced(fid)` absorbs that node, built once per fid
  (`absorb` of a clone). `hash()` takes the node's `hash_schema` (or xxh64 of the
  schema's text) for a `Forced` part. Remove the early `return None`.
- **Hypothesis.** On rowsw sitelinks, conversions fall from 2,185 rows × 274 values
  to 60 (one per distinct row state). Validation, per-row trees and their hashing go,
  as they did for unforced inference. I expect 0.717 s → about 0.05 s, close to the
  unforced 0.041 s (R3 e1), with no extra peak memory: states only. On rel7k (one link
  per row) the gain is the serde pass and the tree building, not the unification.
- **Invariants.** The schema text (key order included) and `processed_count` must equal
  the usual path's. The usual path dedups rows after forcing, so hash equality must
  still match schema equality. Absorbing a `Forced` node twice must leave the node
  unchanged, as the skip-by-identity rule needs. Rows the interned path declines must
  still decline.
- **Trade-offs.** A third kind of interned strategy, holding an arbitrary schema.
  `Forced` must be leaf-like for `mixed()` (a list of 10 or more items that holds a
  forced map still needs the long-list decline check).
- **Falsification.** (a) A stage split of the forced run on rowsw sitelinks: if
  `apply_force_field_types_to_node` is not the main cost, the gain is mostly the
  validation and the trees, and still large. (b) Extend `test_interned_rows_match_the_builders`
  with random `force_field_types` maps over random keys, at depth, under lists, nullable
  and mixed. Any schema-text difference rejects the design. (c) Benchmark the R3
  `sitelinks_split.py` forced run.
- **Against it.** The archaeology journal shows unification is not idempotent in field
  order, but this design never skips a repeat inside a row: it converts whole row
  states. The production value is smaller than on rowsw: on rel7k the whole of
  `normalise_sitelinks` is 0.1 s (PA, "data path").
- **Confidence:** high that it is exact; high that the gain is large on rowsw, modest on
  release chunks.

## L2. decode=<dtype> through the typed decoder, projected onto the target (H3): try first

- **Sources.** Li, Katsipoulakis, Chandramouli, Goldstein, Kossmann, "Mison: A Fast JSON
  Parser for Data Analytics", PVLDB 10(10), 2017,
  https://www.vldb.org/pvldb/vol10/p1118-li.pdf **[F]**. Keiser & Lemire, "On-Demand
  JSON: A Better Way to Parse Documents?", Softw. Pract. Exp. 54(6), 2024,
  arXiv:2312.17149 **[A]**.
- **Insight.** Push the projection into the parser: decode only the fields the consumer
  asks for (Mison, abstract and §1). On-Demand "only materializ[es] the results" the
  consumer reads.
- **Correspondence.** With `decode=` given a dtype, `normalise_json` writes each
  normalised row as JSON text, and Polars `str.json_decode(dtype)` parses it again
  (`__init__.py`, lines 1279–1344). In production (`normalise_sitelinks`, decode=RAW) on
  rowsw that is 1.864 s, of which `json_decode` is 1.206 s (R3 e1). The `decode=True`
  route already decodes straight from the text (0.885 s, which includes the 0.717 s
  forced inference). `RecordNode::new` declines unless the target fields equal the plan's
  fields one for one (typed.rs:200–207). RAW lacks the plan's `url` field.
- **Adaptation.** Add `NormalisePlan::to_arrow_projected(&Fields)`.
  - A record compiles against the target's fields by name.
  - A plan field absent from the target becomes `Node::Skip`: still read, so validation
    and its prune checks hold, but nothing is appended.
  - A target field absent from the plan becomes an all-null child.
  - Any leaf type mismatch declines.
  - `_rust_normalise_json_typed` takes the target dtype, and Python uses it when `decode`
    is a schema. It keeps the JSON route as the fallback when the plan declines.
- **Hypothesis.** rowsw sitelinks go from 1.864 s to about 0.885 s now, and to about
  0.2 s with L1. This removes one serialise and one parse per row, plus the per-row
  strings in between.
- **Invariants.** The frame must equal `json_decode(normalise_json text, dtype)` exactly,
  including null struct versus struct of nulls, and empty list versus null. Pruned
  values must stay the same.
- **Trade-offs.** We would be emulating `json_decode`'s projection semantics. If they
  change in Polars, the differential test catches it.
- **Falsification.** A differential test of the two routes on the R3 fuzz corpora
  (`fuzz_lossless.py`, `soundness_cases.py`) and on rowsw and rel7k sitelinks with
  RAW. One mismatch class that cannot be expressed as a decline rejects the projected
  route for that class. Then time it with `sitelinks_split.py`.
- **Against it.** This is mostly an engineering fix, cited for its mechanism. Its
  production value is again bounded by `normalise_sitelinks` being 0.1 s on rel7k.
- **Confidence:** high.

## L3. A two-stage lexer: SIMD structural index plus serde-rule scalar checks (H1, H3): try first, staged

- **Sources.** Langdale & Lemire, "Parsing Gigabytes of JSON per Second", VLDB J.
  28(6), 2019, arXiv:1902.08318 **[A, HTML read]**. Keiser & Lemire 2024 (above).
  Jiang, Qiu, Zhao, "Scalable Structural Index Construction for JSON Analytics" (Pison),
  PVLDB 14(4):694–707, 2020, https://github.com/AutomataLab/Pison and
  https://par.nsf.gov/biblio/10222359 **[A]**. Mison (above) **[F]**.
- **Insight.**
  - Stage 1 classifies 64-byte blocks without branches. Odd-length backslash runs give
    the escaped quotes. A carry-less multiply turns the unescaped quotes into a mask of
    the string spans. Two `vpshufb` lookups classify structural characters and
    whitespace, and the bit positions are extracted into an index.
  - Stage 2 walks that index, checking atoms, numbers and escapes.
  - Mison: building the index "converts control flow into data flow".
  - simdjson averages 8.3 instructions per byte; RapidJSON 18.7.
- **Correspondence.** `tape::Lexer` is one byte at a time. `peek()` is bounds-checked,
  and `ws()` loops per byte before every token. `plain_prefix` is an 8-byte SWAR scan.
  **[new]** It takes 144 instructions per token, 21.7 per byte (narrow200k). The same
  lexer runs again in the typed decode, where it is "about a quarter" of decode (PA,
  "Normalisation straight into Arrow"). So it is paid twice per row.
- **Adaptation, staged.**
  - (a) Scalar tightening first: unchecked indexing behind proven bounds, a 256-entry
    class table for whitespace and dispatch, number class reported by the lexer (L15).
  - (b) SSE2/NEON 16-byte, or AVX2 32-byte, string scanning in `plain_prefix`: quote,
    backslash and bytes below 0x20 in one compare-movemask.
  - (c) A full stage 1 per row, with a quote mask, an in-string mask and a
    structural/whitespace bitmap. `Lexer::value` then reads the next structural position
    instead of `ws()` loops.
  - Validation stays in our own code (serde rules: number range, surrogates, depth 127).
    This is not simd-json validation, the known dead end.
- **Hypothesis.** (a)+(b) cut lexing by 20–40% on narrow and event rows (short strings,
  so (b) gains little there), and more on rel7k and labels strings. (c) gets lexing
  toward 5–10 instructions per byte, about 0.3–0.4G off narrow200k's 1.2G. Every corpus
  gains, in both inference and decode. These are hypotheses from the paper's figures,
  not from our data.
- **Invariants.** Accept and reject exactly what `serde_json::from_str::<Value>` does.
  `test_unescape_matches_serde_json`, the typed-decoder tests (2,500 batches) and the
  interned-row tests (1,200 sets) are the regression net.
- **Trade-offs.** Unsafe or arch-specific code, with a portable fallback. Stage 1 on rows
  of about 126 bytes amortises over only 2 blocks per row. Short rows are the regime
  where simdjson's gain is smallest (its stage-2 cost is per structural).
- **Falsification.** Build (a) alone and count instructions per token on narrow200k,
  events20k and rel7k. If (a) already reaches about 50 instructions per token, stage 1
  is worth less. Then microbench (b) on strings from rel7k against narrow200k. Prototype
  (c) only if the lexer is still over 40% of the row phase after (a) and (b).
- **Confidence:** high that there is room (the per-token cost is 3–4 times simdjson's);
  medium on how much (c) adds for short rows.

## L4. Fusing lexer and interner: no tape on the inference path (H1): try first, after L3a

- **Sources.** Wadler, "Deforestation: transforming programs to eliminate trees", TCS
  73:231–248, 1990, doi:10.1016/0304-3975(90)90147-A **[C]**. Keiser & Lemire 2024 **[A]**.
- **Insight.** Composing a producer with a consumer removes the intermediate structure.
  On-Demand drives parsing from the consumer over the structural index, without a DOM:
  60% of DOM simdjson's instructions (3.1 against 5.1 per byte). But on json2msgpack,
  which consumes the whole document, the gain is only 2.3–2.5 against 1.7–1.8 GiB/s.
- **Correspondence.** Inference lexes each row into a `Vec<Tok>`, back-patching each
  container's `len` and `next`, then `RowStates::value` walks it again, dispatching on
  every token. Inference reads every token, so this is the json2msgpack case.
- **Adaptation.** Make the lexer generic over a sink trait (`TapeSink` for the decoder,
  `StateSink` for inference), monomorphised, so that the validation rules stay in one
  place.
  - `StateSink` returns `StateId`s from the recursion.
  - An object switches to a positions hash map once it reaches 17 members (today it
    decides from `len > 16` up front).
  - A list counts its items for the `LONG_LIST`/`mixed` decline.
  - `row()` checks the first non-space byte for `{`.
- **Hypothesis.** It removes the token pushes and back-patching, about 25M instructions
  on narrow200k (`vec/mod.rs` lines), and the second dispatch and bounds checks per
  token in `RowStates::value`. I expect 10–20% of the row phase. No memory change: the
  tape is reused per block anyway.
- **Invariants.** Same as L3, plus the interned-row equivalence tests.
- **Trade-offs.** Generic code; the decoder's tape path is unchanged.
- **Falsification.** Prototype `StateSink` and count instructions on narrow200k and
  events20k. Below 8% gain, reject: On-Demand's full-consumption result already
  predicts a modest gain.
- **Confidence:** medium.

## L5. Polymorphic inline caches for row positions (H1, event logs): try later

- **Sources.** Hölzle, Chambers, Ungar, "Optimizing dynamically-typed object-oriented
  languages with polymorphic inline caches", ECOOP 1991, LNCS 512:21–38,
  doi:10.1007/BFb0057013 **[C]**. Chambers, Ungar, Lee, "An efficient implementation of
  SELF…", OOPSLA 1989, doi:10.1145/74877.74884 **[C]** (maps and hidden classes).
- **Insight.** A call site that sees a few receiver shapes keeps a small list of them
  instead of one, so alternating shapes stop missing.
- **Correspondence.** `row_key` and `row_object` compare only with the previous row's key
  and object at the same position: a monomorphic cache. Event logs interleave event
  types (456 distinct schemas in 20,000 rows), so the payload positions miss whenever
  the type changes. The row phase is 92–97% of event-log inference (PA, "Siblings").
- **Adaptation.** Make `last_keys` and `last_objects` small arrays of k = 2–4 entries per
  position, ordered by most recent use.
- **Hypothesis.** It removes hash lookups on the payload parts of event rows. Gain is
  0–15% of event-log interning; nothing for narrow rows, a small cost for rel7k.
- **Falsification.** Instrument hit and miss counts per position for k = 1, 2 and 4 on
  events20k and narrow200k first. If k = 4 adds under 10 points of hit rate, reject.
  The journal already found that extra bookkeeping in `value` changed inlining for the
  worse ("The row phase's lookups").
- **Confidence:** low to medium.

## L6. Inference as a validity certificate for decode (H3): try later

- **Sources.** Keiser & Lemire 2024 **[A]**: On-Demand validates "only the components
  that the user parses". JSONSki: Jiang & Zhao, "JSONSki: streaming semi-structured data
  with bit-parallel fast-forwarding", ASPLOS 2022, doi:10.1145/3503222.3507719 **[A]**.
- **Insight.** Validation is separable from tokenisation. Once a text is known to be
  valid, a cheaper scanner can be used, including bit-parallel skipping.
- **Correspondence.** `normalise_from_parquet` and the typed expression pass the very same
  `&str` rows to inference and then to the decoder (`parquet_io.rs` 284 and 370;
  `typed.rs` 67 and 86). When the interned path succeeds, every row has already been
  lexed under serde's rules.
- **Adaptation.** `SchemaInferenceResult` reports `rows_validated: bool`. The decoder
  then uses a non-validating lex mode: no number-grammar or range checks, no surrogate
  or control-character checks. Escapes still need unescaping.
- **Hypothesis.** Validation is maybe 30–40% of lexer work, so this saves a fraction of
  decode's lexing quarter: about 5–10% of decode.
- **Invariants.** Only when the same buffers are used and the interned path covered
  every row. Any decline sets the flag false.
- **Falsification.** Time the decoder with the checks stubbed out on rel7k and narrow200k.
  Below 5%, reject.
- **Confidence:** medium that it is correct; low that it is worth the plumbing.

## L7. Decode specialised per distinct row state (H3): try later

- **Sources.** Futamura, "Partial Evaluation of Computation Process—An Approach to a
  Compiler-Compiler", HOSC 12:381–391, 1999 (reprint of 1971),
  doi:10.1023/A:1010095604496 **[C]**. Mison's speculative pattern trees (§5) **[F]**.
- **Insight.** Specialise an interpreter (the `ArrowPlan` walk) for a static input: here,
  the row's shape. Mison guesses field positions from previously seen patterns and
  verifies each guess. We would know the shape, not guess it.
- **Adaptation.** Keep the per-block `RowStates` and a `StateId` per row from inference
  (4 bytes per row; the states are only the distinct subtrees). Compile the plan per
  distinct object state into member→slot arrays and known number kinds, so members go
  straight to their builders with no name compare.
- **Hypothesis.** It removes the key→slot work, which is only part of decode's
  dispatch. 5–15% of decode.
- **Falsification.** Count the instructions in `object_slots`/`slots_of` on rel7k's
  decode. Under 10% of decode, reject.
- **Against it.** The "next field tried first" heuristic already makes most lookups one
  compare. Inference's states merge repeated keys, so the decoder must still handle
  `has_repeat`.
- **Confidence:** low.

## L8. Shredding during inference, then casting columns (H3): reject for now

- **Sources.** Melnik et al., "Dremel: interactive analysis of web-scale datasets", PVLDB
  3:330–339, 2010, doi:10.14778/1920841.1920886 **[C]**. Durner, Leis, Neumann, "JSON
  Tiles", SIGMOD 2021, doi:10.1145/3448016.3452809 **[F]**: frequent key paths per tile
  are mined and extracted into columns.
- **Idea.** Write leaf values into per-path columns while interning, and once the schema
  is known, assemble or cast them without a second lex.
- **Reject.** The columns hold about 1× the input in extra memory. That is the same
  objection as retained tapes, the known dead end, only smaller. Normalisation
  (promotion, map encoding, record-of-nulls rules) would also have to be re-expressed
  over columns. Revisit only if a memory budget allows +1× input.

## L9. Speculative decode against a schema inferred from a sample (H3): reject

- **Sources.** Jxplain's sampling and validation loop (§4.2) **[F]**. Spark's JSON
  `samplingRatio` ("fraction of input JSON objects used for schema inferring"),
  https://spark.apache.org/docs/latest/sql-data-sources-json.html **[A]**.
- **Idea.** Infer from a sample, decode every row while checking that its state fits, and
  re-decode only if schema(all) differs from schema(sample).
- **Reject.** Our output must equal the schema inferred from all rows. The journal shows
  `required` and nullability vary from one 20-row chunk to the next (PA, "wikidata-pq's
  ingestion"), so the speculation would usually fail and the work would be paid twice.
  Falsification, if anyone wants it: the fraction of rel7k blocks whose schema equals
  the full schema.

## L10. Field order as a left-regular band, so a fold can skip repeats (H4): reject

- **Source.** Brown, "Semigroups, Rings, and Markov Chains", J. Theor. Probab. 13:871–938,
  2000, doi:10.1023/A:1007822931408 **[C]**. "Concatenate, keep the first occurrence" is
  idempotent and satisfies xyx = xy, so a fold can skip any repeated input.
- **Correspondence.** The journal: unification "is not idempotent in field order", so
  "repeated schemas cannot be skipped in a fold".
- **Reject.** Making it so changes output (snapshots), and the rewrite is now 96M of
  rel7k's 899M instructions. L1 converts per state and never needs the property.

## L11. Memoised lattice joins, the BDD computed table (H4): already implemented

- **Source.** Bryant, "Graph-Based Algorithms for Boolean Function Manipulation", IEEE TC
  C-35(8):677–691, 1986, doi:10.1109/TC.1986.1676819 **[C]**; Filliâtre & Conchon (above).
- `dag.rs` together with `on_dag.rs`'s `pairs`/`rewritten` memo tables already hash-cons
  the merged schema and memoise unification per pair and path. Nothing to add. Remaining
  H4 cost is small (rel7k: writing the DAG 39M, rewrite about 96M).

## L12. Perfect hashing or static dictionaries for keys (H1): reject

- Keys are discovered online, per block, so there is no static set to build a perfect
  hash over; hash lookups happen only on position-cache misses (key memcmp is 2.5% on
  narrow200k), and the external interner was reverted (#133).

## L13. Raw-text dedup before lexing, and Sparser-style prefiltering (H1): try later, niche

- **Source.** Palkar, Abuzaid, Bailis, Zaharia, "Filter Before You Parse: Faster Analytics
  on Raw Data with Sparser", PVLDB 11(11):1576–1589, 2018, doi:10.14778/3236187.3236207
  **[A]**. It filters on raw bytes before parsing; the filters may give false positives,
  never false negatives.
- **Correspondence.** Inference has no predicate to filter on. What does apply is that
  byte-identical rows have identical states, so a row whose text was already seen can
  skip both lex and intern (an exact shortcut). Parquet dictionary pages give that
  identity for free.
- **Verdict.** rowsw sitelinks have 60 distinct shapes, and the texts repeat too, since
  the well-known items' sitelinks are repeated. But release chunks hold distinct
  entities, so production gains nothing. Keep only for users with repetitive logs.

## L14. Avro written from the DAG, memoised per node and name context (H5): try later

- avrotize names records after their keys, so the same subschema at two paths gets two
  names. That limits sharing to subtrees under the same name context. Measure avrotize's
  share after the upstream regex fix before doing anything (PA: rows200 0.45–0.55 s with
  the fix). No literature beyond hash-consing applies.

## L15. Small fixes inside the row phase, bundled with L3a

- **Number class.** The lexer already tracks `.`/`e` (`plain`) and the digit count.
  `number_kind` scans the text again and runs `parse::<i64>`/`parse::<u64>`/`parse::<f64>`
  (about 3% on narrow200k: `num/mod.rs` and `str` lines). An integer of 18 digits or
  fewer always fits in `i64`, so only longer ones need checking.
- **Row hash set.** `seen_hashes` is a SipHash `HashSet<u64>`, about 1.2% on narrow200k.
  The hashes are already xxh64, so a pass-through hasher would do.
- **Brace scan.** `read_whole`'s memchr2 pass (about 2.5% without a delimiter) could be
  folded into the lexer, with brace counting inside string scans.
- Falsification: an instruction count per change on narrow200k. Each is exact by
  construction.

## L16. Considered and not applicable

- DiScala & Abadi, "Automatic Generation of Normalized Relational Schemas from Nested
  Key-Value Data", SIGMOD 2016, doi:10.1145/2882903.2882924 **[C]**: it mines functional
  dependencies for schema design, not a speed technique for our passes.
- Klettke, Störl, Scherzinger, "Schema Extraction and Structural Outlier Detection for
  JSON-based NoSQL Data Stores", BTW 2015, LNI P-241:425–444,
  https://dl.gi.de/handle/20.500.12116/2420 **[A]**: a path-structured summary graph
  that our builder already is.
- Cousot & Cousot, "Abstract interpretation…", POPL 1977, doi:10.1145/512950.512973
  **[C]**: map forcing is a widening applied at a known path. It frames L1, but every
  exact version has to stay per row, because map unification differs from the builder
  join (the collapsed-corpus experiment in PA gave a different schema).
- JSONSki fast-forwarding: the schema comes from the same rows, so the decoder has almost
  nothing to skip (pruned values must be recorded). FlatBuffers / Cap'n Proto: the input
  is JSON text, so they do not apply.

---

## Ranking: expected end-to-end value per unit of complexity

| rank | entry | hotspot | complexity | expected value | where it pays |
|---|---|---|---|---|---|
| 1 | L2 projected typed decode for decode=dtype | H3 | small | about 1.0 s per rowsw sitelinks chunk (1.86 s → about 0.89 s); about 0.05 s on rel7k | sitelinks, any decode=schema caller |
| 2 | L1 forced maps on interned states | H2 | small to medium | 0.717 s → about 0.05 s on rowsw sitelinks (hypothesis) | forced-map users; with L2, sitelinks about 1.86 s → about 0.2 s |
| 3 | L3 lexer (a: scalar, b: SIMD strings, then c: stage 1) | H1+H3 | a, b small; c large | 20–40% of lexing from a+b; lexing is about 45% of the row phase on narrow and events, and a quarter of decode | every corpus, paid twice per row |
| 4 | L4 fused lex→intern | H1 | medium | 10–20% of the row phase | every corpus |
| 5 | L5 polymorphic position caches | H1 | small | 0–15% of event-log interning | heterogeneous logs |

Next in line: L6 and L7 (decode certificate and specialisation), then L15 (bundle it with
L3a; small, certain gains).

Caveat that applies to ranks 1 and 2: their large numbers are on rowsw, a stress corpus
of well-known items. On a release chunk (rel7k) the whole of `normalise_sitelinks` takes
0.1 s of `process()`'s roughly 1.5 s, while ranks 3–5 apply to the claims column of every
chunk.
