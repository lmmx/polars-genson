# 2026-10-20 Performance round 3: soundness first, then the forced map and the text round trip

The third round of work on genson-core and polars-genson after the 1.0.4 release: campaign
2's findings recorded here (they were only in #267), an audit of the values normalisation
drops, and the measurements and literature that choose the next performance work. Scripts,
logs and the research ledger are in `2026-10-20-performance-round-3/` next to this entry.
Measurements are on the published polars-genson 1.0.4 wheel (Python 3.12, Polars 2.0.0) on a
4-core Intel Xeon @ 2.10GHz VM unless stated.

## Current State

### Campaign 2 (#258–#271), recorded after the fact

- Shipped in genson-core and genson-cli 1.0.3 and polars-genson 1.0.4 as one linear stack:
  #270, #260, #263, #264, #265, #261, #262, #258, #259, #266, #268, #269, #271. Each PR
  was compared with its immediate parent, built in its own worktree and target directory
- `decode=True` decodes typed rows straight into Arrow over the C data interface (#258):
  with avrotize-rs#29 (its regexes compiled once), labels 604 → 118 ms, events 284 → 47
  ms, rel7k 701 → 116 ms against PyPI 1.0.3; 51 differential tests against the text route
- Decode instructions: unescaping strings directly rather than through
  `serde_json::from_str` (#266), labels −8.8%, events −7.8%, narrow −1.2%, rel7k −1.3%;
  padding a wide record's absent fields with nulls in bulk when next written (#268),
  labels −19%, rel7k −16%, events −6%, narrow +1.4%. Together labels 468.5M → 346.1M,
  rel7k 774.8M → 645.6M
- `decode=False` writes its JSON from the typed decoder through arrow-json, byte-identical
  (#269): Python wall, median of 11, events 62.3 → 42.3 ms, narrow 156.3 → 96.2 ms, rel7k
  141.3 → 106.2 ms, labels 150.5 → 136.0 ms; 20 whole entities within noise
- Labels' decode at #268, 346M instructions: lexing each row again 41% of instructions
  (about 19% of decode time), the per-row slot table (`slots_of`, per-key hash lookups)
  16%, unescaping 14%, each of the 473 fields' input (`field_input`) 7%
- Correctness found on the way, each with a test failing on its parent: `no_unify` through
  the DataFrame namespace (#259); a union of `null` alone normalises to null, not a panic
  (#260, #238); an empty list beside scalars takes their type as its items (#261); a union
  of a list and other types is one list holding their values (#262, #238's `{"d": 9}`);
  map values `null ⊔ T` is nullable `T` (#263); a forced map keeps its values' type when
  it is a scalar in some rows (#264); a value at a position typed `"null"` normalises to
  null (#265, which needs #264 or it nulls the ddminv2 fixture's label strings)
- avrotize 0.1.2 (#270) carries avrotize-rs#29 and #30 (distinct names for differently
  shaped records). #238's reproduction passes on the published 1.0.4 wheel in all four
  `decode` × `empty_as_null` combinations; on 1.0.3 the same rows panic

### Sitelinks: a forced map and a JSON text round trip (logs/e1_sitelinks_rowsw_104.txt)

- wikidata-pq's `normalise_sitelinks` (src/wikidata/process.py at 5ad5f96) calls
  `normalise_json("sitelinks", ndjson=True, wrap_root="sitelinks",
  force_field_types={"sitelinks": "map"}, decode=<a List(Struct{key, value}) dtype>,
  max_builders=100)`; its claims force `mainsnak` to `"record"`, which the interned path
  handles
- A field forced to `"map"` sends the whole run off the interned path
  (`process_json_strings_interned` returns `None` at `schema.rs:513` when any
  `force_field_types` value is `"map"`): every row then takes serde validation, a
  `SchemaBuilder` tree of its own, and a `Value` round trip of the forced subtree
  (`apply_force_field_types_to_node`)
- `decode=<dtype>` normalises rows to JSON text and has Polars' `json_decode` parse it
  again; only `decode=True` takes the typed decoder
- `sitelinks_split.py` on rowsw (2,185 rows, 83.2 MB of sitelinks, 274 links per row),
  median of 3 after a warm-up: inference forced to a map 0.717 s, not forced 0.041 s (a
  different schema, so not a like-for-like comparison); the expression to JSON text 0.793
  s; `json_decode` of that text alone 1.206 s; production `normalise_json(decode=dtype)`
  1.864 s; the typed route `normalise_json(decode=True, map_encoding="kv")` 0.885 s, whose
  struct also has the links' `url` (the production dtype leaves it out)
- Casting the typed result to the production dtype (the `url` field dropped) did not
  finish in 16 minutes of one core, twice; not investigated further here
- rowsw's rows have 60 distinct shapes and the column's link values 2
  (`sitelinks_shapes.py`); on a release chunk (rel7k's 7,128 rows, about one link each)
  `normalise_sitelinks` takes about 0.1 s in the 2026-10-18 journal's `process()` runs

### Where wikidata-pq's chunks spend their time at master d38635e (profile/)

- `profile/e2e_*.py` replays `process.py`'s per-column calls (with `map_encoding="kv"`,
  as wikidata-pq's pinned 0.9.6 decodes maps), one fresh process per measurement, a
  warm-up and 5 runs; peak RSS is VmHWM over the step; `profile/prof3.rs` replays the same
  calls in Rust with each step in a function of its own for callgrind. The machine was
  shared with builds at times: the times are medians with real spread, the instruction
  counts exact
- rowsw, warm (logs/e2e_summary.txt): labels 0.211 s, descriptions 0.133, aliases 0.121,
  sitelinks 2.042 (cold 2.542; `normalise_json` 1.860 of it, peak +1,168 MB), claims 1.473
  (`normalise_from_parquet` 0.711, Polars' sink 0.521, `lookup_to_long` 0.155, the
  conforming scan 0.089): 3.98 s, sitelinks 51%, claims 37%
- chunk_0 (60 release entities): the map columns 0.014–0.018 s each, sitelinks 0.059,
  claims 1.470 (Polars' sink of the conformed claims 0.884, `normalise_from_parquet`
  0.350, the conforming scan 0.173): 1.58 s, claims 93%
- The forced sitelinks inference on rowsw (logs/p3_*, one thread): 23.58G instructions,
  283 per byte. `apply_force_field_types_to_node` 16.73G (71%: the property's node to a
  `Value` 7.71G, forcing on the `Value` 8.16G of which `check_unifiable_schemas` 6.73G),
  the rows' `SchemaBuilder` trees 5.57G, serde's validation 0.97G, the `wrap_root` copy
  0.18G; 47.6M allocations and 7.25 GB requested, against 74k and 64 MB on the interned
  path. Unforced, inference is 1.84G and the typed decode 2.25G
- `decode=<dtype>`: Polars' `json_decode` adds about 1.0 s and +900 MB of peak RSS on
  rowsw's sitelinks; the typed decoder's arrays, their structs projected to the dtype's
  fields by name, equal the `json_decode` output on rowsw and chunk_0
- Forcing the map per interned state (`prof3_measurement.diff`, a switch off by default):
  inference 0.944 → 0.062 s on 4 threads, and the same Avro schema on rowsw and chunk_0;
  but on the scorecard's `usual` corpus `payload` becomes `map<string>` where master
  gives `map<record>`, so a sound version must keep the per-row unification
- `map_threshold=0, unify_maps=True` in place of the forced map gives the same Avro schema
  and typed output on both chunks in 0.046 s, without the guarantee of a map on every chunk
- The scorecard at master (logs/sc_*.json): claims 2,062M instructions (rows 845M,
  rewrite 546M, merge 459M; 177 MB live in the merge), events 331M (rows 97%), narrow
  574M (rows 96%), labels 242M (rows 177M, avro 38M), entities 641M (materialise 186M),
  the usual path (a forced map) 2,487M and 4.89M allocations, 97 bytes per input byte
- Row phase, instructions per byte (lexer + interning): chunk_0 claims 32.7 (14.1 +
  18.6), rowsw labels 35.6, descriptions 15.8, aliases 27.6, sitelinks interned 22.0
- chunk_0's claims core (2.67G): extraction 0.70G (26%) though release claims carry no
  label maps to extract, inference 1.06G (rows 0.78G, merge 0.12G, rewrite 0.11G, Avro
  6.4M), the typed decode 0.91G (its lexing 0.34G)
- Peak RSS over the resting process, normalising claims (logs/rss_claims.txt):
  `normalise_from_parquet` +338 MB on 322.5 MB of rows (0.79 MB per MB beyond about 110
  MB), `normalise_json` +744 MB as text and +758 MB typed (about 2 MB per MB)
- A cold claims read takes 0.18–0.43 s, mostly system time, against 0.022 s warm;
  `MIMALLOC_ALLOW_LARGE_OS_PAGES=0` gives 0.06–0.07 s steadily (logs/read_firsttouch.log)
- Small: the Avro conversion 0.4–0.6% of claims inference; merge, rewrite and DAG
  together 8% of chunk_0's claims core; the typed normalisation of claims 12–30 ms;
  `schema_hash` 25M; the forced path's validation 4.1% and its copy 0.7%

### Values normalisation drops (logs/e2_soundness_104.txt, e3, e4)

- `soundness_cases.py` runs each case through `infer_json_schema`, `infer_polars_schema`
  and `normalise_json` with `decode` False and True, and reports a row's scalar that is
  missing from its normalised row as LOST
- On 1.0.4 every route drops: the second kind of a field holding scalars of two kinds
  (`{"a": 1}`, `{"a": "x"}` → `x` null; a boolean and an integer, the integer null); the
  same inside a list (`[1, "x"]`) and inside nested lists beside records
  (`[[1], ["x"], [{"b": null}]]` keeps only the records); a scalar beside a record with no
  fields (`{"c": {}}`, `{"c": 3}` → `{"c": {}}` twice); a scalar or record beside a list
  of nulls (`[null]`, `3`); a scalar beside a list of empty records; an integer above
  `i64::MAX` (`18446744073709551615` infers as `integer`, normalises to null); a forced
  map whose values are integers and nulls types its values as strings
- The Avro schema of a mixed scalar field is right (`["int", "string"]`); the union is
  normalised against its first branch, and the Polars dtype is that branch's
- `fuzz_lossless.py` (random rows, default and map options, both decodes, each failure
  shrunk): 334 of 600 sets lose a value with every scalar kind; 191 of 800 with integers
  and null only, so not only mixed kinds: a scalar beside an empty record, a list of nulls
  beside a scalar or record, a list of records beside a record of lists
- The scalar/object promotion (`a__integer` beside a record's fields) already keeps such
  values, and the normalisers already route a scalar into its `__`-suffixed field; it ran
  only for an `anyOf` of a record with fields and one scalar. A field of several scalar
  kinds reaches the rewrite as `{"type": ["integer", "string"]}`, a record with no fields
  beside a scalar as `{"type": ["object", "integer"]}`
- 2026-09-26-mixed-scalar-types-dropped.md left the representation open; the rule is now
  the existing promotion: scalars of kinds that do not widen to one (anything but integers
  and numbers together) each keep a `__`-suffixed field, with the existing naming
  (`a__integer`, `__string` among a list's items); integers and numbers alone still widen
  to a number; `coerce_strings` keeps coercing a string beside numbers or booleans

### Scalars of kinds that clash keep a field each (fix/mixed-scalar-kinds-promote)

- The promotion that keeps a scalar beside a record (`unify_anyof_schemas`) now also
  takes: scalars of kinds that clash with no record among them; a record with no fields
  as a record; a branch of several clashing types, split into one branch per type. A
  schema of several clashing types (`{"type": ["integer", "string"]}`) is rewritten as an
  `anyOf` of them in `type_rank`'s order; `null` among the branches adds nothing (a
  promoted field holds a value in some rows only); a list's items seen only as `null`
  make the absorbed items nullable. The Value rewrite and the DAG rewrite alike
- Integers and numbers alone still widen to a number; `wrap_scalars=False` leaves every
  union as before; with `coerce_strings` a string beside numbers or booleans does not
  clash with them (`SchemaInferenceConfig::coerce_strings`, set by the CLI and
  polars-genson's normalisation, and `infer_polars_schema(coerce_strings=)`), so the
  `normalise_union_coercion` snapshot is unchanged
- On the same seeds as 1.0.4 (logs/e5, e6): 63 of 600 sets lose a value with every scalar
  kind (334 before), 62 of 800 with integers and null (191 before). What is left is a
  list beside a record whose fields clash (`[{"a": -7}]` and `{"a": []}`): record
  unification gives up on a field whose types differ
- soundness_cases.py (logs/e7): every case keeps its values but the forced map's (its
  values typed string), the integer above `i64::MAX`, and the list of records beside a
  record of lists
- Tests that held the dropped values are updated to the kept ones: genson-core's
  reorder.rs and the array-union test, polars-genson-py's union_reorder_test.py and three
  normalise_test.py snapshots (`"123"` and `"true"` were nulls); decode_typed_test.py's
  text route now passes `coerce_strings` to `infer_polars_schema`
- genson-core 153 tests (default features) and 233 (avro, parquet), genson-cli 181,
  polars-jsonschema-bridge 37, polars-genson-py 317 (3 skipped); clippy and precommit clean

### A list beside a record or scalar under a record's field (fix/record-field-list-beside-others)

- Unifying records gave up on a field that is a list in some records and a record or
  scalar in others (`unify_field_pair` had no case for it), and so on the whole record;
  a union nested in another (a list's nullable records, `anyOf[record, null]`) was not
  looked into, and records beside `null` without scalars stayed a union
- Now: such a field is one list holding them (`unify_anyof_schemas`, which absorbs into
  the array), sequentially even past 50 schemas; array items likewise
  (`check_unifiable_item_schemas`, the owned way taking the borrowed one for these
  pairs); a branch that is a union stands for its branches; `null` beside two or more
  schemas that unify makes their unification nullable; a promoted record beside `null`
  is nullable; integers and numbers alone widen to a number
- Fuzz on the same seeds (logs/e8, e9, e10): seed 1, every kind, 0 of 600 lose a value
  (334 on 1.0.4, 63 after the scalar kinds); seed 2, integers and null, 0 of 800 (191,
  62); seed 3, 2 of 2,000 (1,166 on 1.0.4, logs/e11), both under map options with scalars of two kinds in a list's
  records inside a unified map
- genson-core 153 tests (default features) and 237 (avro, parquet), genson-cli 181,
  polars-genson-py 317 (3 skipped); test_owned_unification_matches_the_borrowed and the
  Value/DAG comparisons pass; scalar_kinds.rs gains the fuzz's shrunk rows

### A forced map's values keep their kinds (fix/forced-map-values-keep-kinds)

- `map_value_schema` (the values of a field forced to a map, on both rewrites and in each
  row's forcing on the usual path) unified `null` values with the rest as a clash, and
  gave up on values of kinds that clash: both became `{"type": "string"}`, so integers and
  nulls came out as text (`{"k": 1, "j": null}` → `"1"`)
- Now `null` values make the values nullable (`null ⊔ T` is nullable `T`, as #263 has it
  for maps that are not forced), and values of kinds that clash keep a field each through
  `unify_anyof_schemas` (`m__integer`, `m__string`) before the string fallback; `no_unify`
  keeps its string fallback
- soundness_cases.py (logs/e13): every case keeps its values but the integer above
  `i64::MAX`
- wikidata-pq's columns are unchanged by the stack so far (prod_schemas.py,
  logs/e12_production_unchanged.txt): on chunk_0 and rowsw, each of claims, sitelinks,
  labels, descriptions and aliases has the same Avro schema and the same normalised text
  on 1.0.4 and here
- A forced map's values unified as records could make a field typed `["boolean",
  "integer"]` nullable as `["null", ["boolean", "integer"]]` (`try_make_nullable_union`,
  `schemas_compatible_*`, the DAG's `null_and`: a list of types made the second of two),
  which no schema reader takes: the usual path panicked reading it back, on 10 random rows
  that 1.0.4 inferred (found by the interned forced-map comparison below). `null` is now
  put first in a list of types instead (`["null", "boolean", "integer"]`, which promotes)
- fuzz_lossless.py now also forces fields to maps; seed 4 (logs/e15, e16): 125 of 1,500
  sets lose a value (947 on 1.0.4), 110 of them under forced maps (339 on 1.0.4), mostly a
  field forced to a map that holds a list (`{"a": [-7]}`): 1.0.4 wrote it as text
  (`{"a__unknown": "[-7]"}`), now as the list, though not every value lands; a forced map
  over a list is left as it is here
- genson-core 154 tests (default features) and 238 (avro, parquet), genson-cli 181,
  polars-genson-py 317 (3 skipped); forced_map.rs's string fallback for an integer and a
  string is now the two promoted fields, and a test of null values is added

### An integer above `i64::MAX` is a number (fix/integers-above-i64-are-numbers)

- #235 made an integer beyond 64 bits a `number`; one between `i64::MAX` and `u64::MAX`
  still inferred as `integer`, whose Avro `long` and Arrow `Int64` cannot hold it, so it
  normalised to null (`18446744073709551615` → `null`)
- Both inference paths now take an integer that does not fit an `i64` as a number: the
  interned rows' `number_kind`, and `NumberStrategy::add_object` on the usual path. The
  value is kept as a float (`1.8446744073709552e+19`), as #235 keeps larger ones
- soundness_cases.py (logs/e14): every case keeps every value on every route
- wikidata-pq's columns still unchanged on chunk_0 and rowsw (prod_schemas.py)
- A JSON `number` is an Avro `float` (32-bit) in the Avro schema, while polars-genson
  decodes it as `Float64`: no value is narrowed on the Polars routes; not changed here
- genson-core 155 tests (default features) and 240 (avro, parquet), genson-cli 181,
  polars-genson-py 317 (3 skipped); big_integers.rs's `u64::MAX`-is-an-integer test is
  now at the signed limits, with the value checked through normalisation

### Research ledger (literature.md)

- Sixteen candidate techniques against the measured costs, each with its citation (read in
  full, abstract read, or the bibliographic record only, marked), the mechanism, the
  adaptation, a falsification experiment and a verdict
- Its measurements: the row phase's lexer takes 144 instructions per token and 21.7 per
  byte on narrow200k (546M of 1.217G, one thread), interning 131 per token; simdjson's DOM
  parser averages 5.1 instructions per byte (Keiser & Lemire, Table 3)
- First to try: the typed decoder for `decode=<dtype>` (projection pushed into the parser,
  after Mison, PVLDB 10(10) 2017), and forced maps on the interned path through a forcing
  step memoised per interned state (Jxplain, SIGMOD 2021; hash-consing, Filliâtre &
  Conchon, ML 2006): rowsw's 2,185 rows have 60 shapes
- Rejected there: shredding during inference (memory on the order of the input, as
  retained tapes), speculative decoding against a sampled schema (`required` and
  nullability vary between chunks), perfect hashing of keys

## Measured and not kept

- Widening scalars of different kinds to a string: it keeps the values but not their types
  (`1`, `"1"` and `true` become `"1"`, `"1"` and `"true"`), so it was not taken

## Missing

- The interned path for forced maps, and `decode=<dtype>` through the typed decoder, are
  not yet measured as implementations
- Whether a 16-minute cast of the typed sitelinks struct to a narrower struct is a Polars
  issue or our dtype
