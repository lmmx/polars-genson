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

### Forced maps on the interned path (perf/forced-maps-interned)

- A field forced to a map sent every row to the usual path (above). The interned path now
  takes it: rows are lexed and interned as for any other options, a schema is built from
  each distinct row's states (by the row's identity hash) and forced, and the forced
  schemas are merged in order by chunks of `max_builders` rows, each distinct schema once,
  as the usual path merges them. Each chunk's new distinct rows are forced in parallel and
  dropped once merged (distinct rows are numbered in the order they first appear, so a
  chunk's new ones are a run)
- What the per-row forcing reads must be the tree a row's builder gives. Two differences
  had to go (`ObjectStrategy::unintern`): a property `absorb_deferred` leaves empty, as the
  same states as an earlier sibling (#254), is given a copy of it, since the forcing
  rewrites it; and the `properties`/`required` placeholders the interned absorb keeps are
  dropped
- The schema is the usual path's up to the order of an object's keywords: `type` comes
  before `properties` from a row's builder but after it once a node has absorbed another,
  which a row's builder does for a list of ten or more items (`ListStrategy::add_object`
  folds them in parallel). Interned states keep no item count, so this order cannot be
  reproduced from them. It shows only where a schema is copied whole into another's
  keywords (a forced value beside `null`). The usual path's order did not change with the
  thread count (1, 3, 4) on the test's 442 compared sets. Properties keep their order
- `test_interned_forced_maps_match_the_builders`: 600 generated sets, forced at the top,
  nested, under lists, beside other kinds, rows repeating and differing only in their
  maps' keys, each delimiter, `wrap_root`, `$schema` and `max_builders`. The processed
  count, the merged schema and the rewritten schema all match the usual path's, with
  keywords sorted and properties kept in order. Before the placeholders were dropped, 23
  of the 442 compared sets differed in keyword order and none otherwise
- Sitelinks, wikidata-pq's options, prof3 (logs/e17, forced/), release builds, 6 runs each
  interleaved ABBA:

  | | parent 5d22498 | this | change |
  |---|---|---|---|
  | rowsw inference, wall | 0.920 s | 0.093 s | −90% |
  | rowsw inference, CPU | 3.43 s | 0.33 s | −90% |
  | rowsw inference + normalise to JSON, wall | 1.091 s | 0.209 s | −81% |
  | rowsw instructions (one thread) | 24,263M | 2,485M | −89.8% |
  | rowsw allocations (one thread) | 47.58M (7.26 GB) | 1.50M (0.23 GB) | −96.8% |
  | rowsw peak RSS (whole process) | 197 MB | 202 MB | +2.5% |
  | chunk_0 inference, wall | 0.036 s | 0.031 s | within noise |
  | chunk_0 instructions (one thread) | 666M | 712M | +6.9% |
  | chunk_0 allocations (one thread) | 1.30M | 1.49M | +14% |

- rowsw's 2,185 rows are 60 shapes (`sitelinks_shapes.py`) and fewer distinct rows still,
  so the forcing (about 70% of the usual path's instructions, `apply_force_field_types_to_node`)
  runs once per distinct row. chunk_0's 60 rows are all distinct, so the forcing costs the
  same on both paths (477M against 493M) and lexing, interning and building from states
  add the rest. The small-chunk cost is about 46M instructions (a few milliseconds), and
  no change in wall time could be measured. The extra 5 MB of RSS is the interned row
  states, held through the merge
- Schema text, JSON Schema and Avro, is byte-identical between the parent and this on both
  chunks (hashes in logs/e17). wikidata-pq's five columns keep the same Avro schemas and
  normalised text on chunk_0 and rowsw as on 1.0.4 (prod_schemas.py, logs/e19)
- genson-core's tests pass with default features, `avro`, and `avro,parquet`, as do
  genson-cli's, polars-jsonschema-bridge's and polars-genson-py's (317, 3 skipped)
- Scorecard (logs/e18): `usual` (event logs with `payload` forced to a map)
  4,908.5M → 393.9M instructions (−92.0%), 543.6 → 37.8 ms wall, allocations −98.5%, the
  output identical. The other corpora are unchanged in instructions (±0.2%) and output;
  their times were taken while builds ran and are not used. The `usual` corpus keeps its
  name; scorecard.md and the workload say what it now measures
- A first timing showed no change: the two sides' prof3 had been built into one target
  directory and the child's copied binary was the parent's. Each side is now built into a
  directory of its own, and each binary is checked for a string only the change has

### decode=<dtype> through the typed decoder (perf/decode-dtype-typed)

- `normalise_json(column, decode=<pl.Schema | pl.Struct>)` took the JSON text route: rows
  normalised to JSON text by the plugin expression, then parsed again by Polars
  `str.json_decode(dtype)`. Only `decode=True` used the typed decoder
  (`_rust_normalise_json_typed`, `ArrowPlan`). Now both call the typed decoder. With a
  dtype, the typed Series (cast to the inferred dtype as before) is projected onto the
  caller's dtype by `dtypes.project_dtype` (Python only, no Rust change):
  - a struct is built field by field in the dtype's order, by name, with `struct.field`
    and `pl.struct`. A field the data lacks is `pl.lit(None, dtype)`, and a field the
    dtype lacks is dropped. It is wrapped in `when(is_not_null)`, so a null struct stays
    null rather than becoming a struct of nulls. `json_decode` gives a missing struct
    field as a null struct, which I checked on Polars 2.0.0;
  - a list is projected with `list.eval(pl.element() ...)`. A null list stays null, and
    an empty list stays empty;
  - a dtype equal to the inferred one passes through unchanged. This covers maps and
    `pl.Map`, which both routes decode the same way, as the decode=True tests show;
  - an inferred `pl.Null` (only nulls seen) is cast to a scalar dtype;
  - no `cast` is used to drop a struct field. That cast once took 16 minutes on a large
    List(Struct).
- Checked upfront (`dtypes.typed_route_makes`, before the typed call): a dtype holding
  any dtype the typed route never makes goes straight to the text route.
  `avro_to_polars_schema` only gives String, Int64, Float64, Boolean and Null leaves, in
  lists, structs and String-keyed maps (`pl.Map` or kv lists). So Categorical, Enum,
  Int8-32, UInt*, Float32, Date, Datetime, Time, Duration, Decimal, Binary, Array and
  Object never run the typed decode.
- After the typed decode, the text route is still the fallback whenever the result could
  differ:
  - a leaf that is not the inferred one (String asked as Int64, Int64 as String), since a
    cast and `json_decode`'s parse can disagree;
  - a nested dtype (struct, list, array or map) where the data holds only nulls;
  - a projection whose output dtype is not exactly the dtype asked for;
  - the typed decoder returning `None`, as before: the root is not a record, inference
    fails, or a batch is declined.

  The typed run uses the same `map_encoding` as the text route ("kv" unless the dtype
  holds a `pl.Map`, or what the caller passes).
- What a fallback costs: only the post-hoc fallbacks pay for both routes. The typed decode
  has already run when the dtypes are compared, so they pay for it and then for the whole
  text route.
  - Before the upfront check (5750d0f), rowsw sitelinks with title as Categorical took
    1.98-3.40 s on master and 3.52-5.71 s on the child, 3 runs under load
    (the prototype's log, not kept).
  - With the check, 8 interleaved runs, median wall [range]: master 2.82 s [2.13-3.59],
    child 2.66 s [2.16-3.39]. CPU was 5.35 against 5.25 s, maxrss 1217 against 1227 MB
    (logs/e21). That is master's cost. Both of wikidata-pq's `decode=` calls take the typed
  route: sitelinks, and the labels `KV_SCHEMA`, which equals the inferred dtype.
- Tests:
  - `tests/decode_dtype_test.py` (223 tests) compares `normalise_json(decode=dtype)`
    with the text route called explicitly (`decode=False` with that `map_encoding`, then
    `str.json_decode(dtype)`). It checks the schema, `equals`, `to_dicts()` (a null
    struct is not a struct of nulls) and the top-level null mask. If the text route
    raises, the new path must raise the same exception type. The cases:
    - the 8 case sets (decode_typed's 7, plus a field that is only null), under both
      map encodings, onto the inferred dtype, onto dtypes reshaped at every depth
      (fields dropped, absent fields added, from String up to List(Struct), fields
      reordered), and onto a dtype with one leaf changed;
    - the claims fixture onto reshaped dtypes;
    - 60 random seeds (decode_typed's generator; all 60 reach the comparisons), each
      onto the inferred, a reshaped and a leaf-changed dtype;
    - sitelinks-shaped rows (forced map, `wrap_root`, values `{site,title,badges,url}`,
      null rows), decoded to List(Struct{key, value}) with values `{site,title,badges}`,
      `{site,title}`, and `{title,site,extra: Struct}`, with `url` dropped;
    - a sitelink title asked for as Int64 (a post-hoc fallback);
    - 11 dtypes the typed route never makes (Int32, Categorical, Datetime, Date, Float32,
      UInt8, Decimal, Enum, Binary, Array, and Int16 nested in List(Struct)). For these,
      `_rust_normalise_json_typed` is patched to raise, and the output must still equal
      the text route's, with `unnest` on and off;
    - `typed_route_makes` itself, at depth.
  - A spy on `ExprStringNameSpace.json_decode` checks which route each call took: the
    typed route where expected, the text route for every leaf change.
  - `decode_typed_test.via_text` now calls the text route explicitly, since `decode=schema`
    no longer takes it.
  - Mutants all caught: no `when` wrapper (98 tests fail), an absent struct as a struct
    of nulls (47), leaf cast allowed (29), list declined (57), all declined (154), fields
    in name order (102), Null-to-nested cast (1), Null-to-scalar declined (1); the upfront check always true (11) or always false (194).
  - polars-genson-py: 540 passed, 3 skipped (master 317 passed, 3 skipped).
  - Restacked on the soundness stack, 10 of the coerced-strings cases took the text route
    where the test expected the typed one: the test inferred their dtype without
    `coerce_strings`, which `infer_polars_schema` did not take on master and takes since
    fix/mixed-scalar-kinds-promote, so the dtype had the promoted fields that coercion does
    not write. The test now passes it, as decode_typed_test's `via_text` does: 540 passed,
    3 skipped
- Measurements (logs/e20, decode_dtype/): the parent is perf/forced-maps-interned and the
  child this, both on the same compiled extension (only Python differs). wikidata-pq's
  `normalise_sitelinks` call as written (`decode=raw`), one fresh process per run, ABBA, a
  warm-up of each, then 8 runs each; medians [range]. Peak RSS is VmHWM, reset by
  `/proc/self/clear_refs` just before the call; "over base" subtracts the RSS at the reset

  | chunk, dtype | wall s | CPU s | user s | peak RSS MB (over base) |
  |---|---|---|---|---|
  | rowsw `{site,title}` parent | 1.365 [1.330-1.499] | 1.91 | 1.35 | 1233 (1163) |
  | rowsw `{site,title}` child | 0.310 [0.234-1.101] | 0.72 | 0.61 | 373 (299) |
  | rowsw `{site,title,badges}` parent | 1.355 [1.318-1.501] | 1.92 | 1.32 | 1209 (1140) |
  | rowsw `{site,title,badges}` child | 0.343 [0.249-1.122] | 0.82 | 0.61 | 370 (295) |
  | release chunk_0 `{site,title,badges}` parent | 0.091 [0.076-0.143] | 0.192 | 0.152 | 149.7 (80.1) |
  | release chunk_0 child | 0.062 [0.054-0.091] | 0.153 | 0.131 | 134.8 (60.7) |

  - On rowsw the call is 77% faster at the median, and peak memory over base falls by
    865 MB (-74%): the 597,759 links' JSON text and its `json_decode` are gone. With
    forced maps now on the interned path, that round trip was most of what was left. One
    child run in 8 took 1.1 s, all of it system time (page faulting, transparent huge
    pages); user time is steady at 0.51-0.76 s
  - Instructions (callgrind, the whole process, minus a run of the same driver that only
    imports and reads, 620M): rowsw 9,315M to 5,316M (-42.9%), chunk_0 984.6M to 872.9M
    (-11.3%). Callgrind runs threads one at a time, so these are total work
  - Outputs equal to the parent's on all three (schema, `to_dicts`, NDJSON bytes)
  - Measured first against master d38635e, before forced maps took the interned path
    (logs/e20's last part, 10 runs): rowsw 1.986 to 1.242 s, 1226 to 356 MB; chunk_0
    0.072 to 0.039 s, 144.5 to 122.0 MB; instructions rowsw 30.65G to 26.64G, chunk_0
    914.5M to 803.1M. The time saved is about the same; the forced inference then
    dominated what was left
- Found on the way: Polars 2.0.0's `DataFrame.equals` on
  List(Struct{.., List(String)}) took 27 s for 50 rows of rowsw sitelinks. The
  comparison script compares `to_dicts()` and NDJSON instead. The tests' small frames
  are unaffected.

### Extraction validates each removed text once (perf/extract-validate-once)

- With sitelinks fixed, rowsw's claims spend most of genson's time in
  `extract_invariants` (the label maps moved out of each row into a lookup, before
  inference): 0.79 s of 0.93 s on one thread, 0.42 s of 0.49 s on four, at 16.0
  instructions per byte over 322.5 MB. Inference takes 0.10 s and the typed decode 0.04 s,
  since they read the slim rows
- The removed fields are 188.2 MB of the 322.5 MB (58%), and 184.8 MB of that (98%) is
  text identical to what an earlier occurrence of the same field and key held
  (`extract/repeats.py`): the same `property-labels` for P31 in every row. The scan
  validated every occurrence in full, as `serde_json` would, so that a row it accepts is
  one the `Value` path accepts too
- Now a removed value is validated once for each text. Where the object has its
  determinant before the field (all 9,213 such objects in rowsw's first 300 rows do) and
  that field and key already hold a validated text, from an earlier row of the chunk (the
  lookup's held text) or earlier in this row, and the row has those bytes there, the scan
  steps over them with one comparison. Validating the same bytes would end at the same
  place with the same result: the text opens with `{`, `[` or `"` (so its end does not
  depend on what follows; a number such as `12` beside `123` does), and its nesting,
  recorded when it was validated, is checked against the 127-level limit at this depth
  (a text validated shallow can be too deep elsewhere: the row is then validated, and the
  `Value` path keeps it whole). Anything else is validated as before
- The line-break check before the scan (any `\n` or `\r` sends a row to the `Value` path)
  took two passes of `slice::contains` (core's generic search, 2.25 instructions per
  byte); it is one `memchr::memchr2` pass now (memchr is already a dependency)
- Not taken: skipping the scan where a chunk has no field to extract (release-format
  claims, chunk_0). A row the scan gives up on is re-serialised by `serde_json` (a
  repeated key, an escaped key), so the slim rows would differ, and no byte search finds
  a repeated key. On chunk_0 extraction is 0.025 s
- prof3 claims (logs/e22, `extract/`), release builds, 6 runs each interleaved ABBA:

  | | parent | this | change |
  |---|---|---|---|
  | rowsw extraction, wall | 0.420 s | 0.222 s | −47% |
  | rowsw extraction, CPU | 0.961 s | 0.407 s | −58% |
  | rowsw claims (extract, infer, typed), wall | 0.486 s | 0.277 s | −43% |
  | rowsw extraction instructions (one thread) | 5,154M | 1,031M | −80.0% |
  | rowsw extraction allocations (one thread) | 589,822 | 597,207 | +1.3% |
  | rowsw extraction bytes allocated | 106.0 MB | 117.0 MB | +10% |
  | rowsw peak RSS (whole process) | 581 MB | 576 MB | within noise |
  | chunk_0 extraction, wall | 0.025 s | 0.024 s | unchanged |

  The line-break check accounts for 650M of the 4,122M instructions saved, the rest is
  validation not repeated. The added allocations are each row's map of the values it has
  removed (for repeats within the row)
- The output is the parent's exactly: the slim rows' text and the lookup hash the same on
  rowsw and chunk_0 (`extract/ex_check.rs`), and both equal the `Value` path's (2,185 of
  2,185 rows, the lookup equal)
- Tests: the stepping over a known value (a contrived "validated" text that is not valid
  JSON, so that stepping shows as the row accepted; not when the determinant comes after,
  for another key or field, or other text; an escaped determinant read as the same key),
  the depth limit for a value known from a shallower place (in another row and in the
  same row), and a known number never stepped over. Mutants caught: no depth check, no
  known values, numbers stepped over, nesting not recorded. test_scan_matches_the_value_path
  (3,000 random sets against the `Value` path) and the rest pass unchanged

### A forced map over a list, and forced maps that meet (fix/forced-map-over-list)

- Of fuzz_lossless.py's 125 failing sets at seed 4 after fix/forced-map-values-keep-kinds,
  110 were under forced maps, nearly all one shape: a field forced to a map holding a list
  (`{"a": [-7]}` with `a` forced), at the top or at any depth. Its values were written as
  `{"type": "array"}` and its `items` left on the map: `forced_map_value_schema` (on both
  rewrites) took the field's `array` type for a scalar, `{"type": "array"}`, without its
  items, and `convert_to_map` and the final rewrite removed `properties` and `required`
  but not `items`. Avro then made the values an "anything" union, and every item was
  written null (`{"a": {"a__unknown": [null]}}`). A list's values now keep its items
  (`{"type": "array", "items": {"type": "integer"}}`), and the map holds no `items`; the
  list is wrapped under `a__unknown` as a list beside a map already was
- With that fixed, three more causes, each a few of the remaining sets:
  - A forced map in a record in one row and in a list's records in another
    (`{"d": {"a": 1}}`, `{"d": [{"a": "s"}]}`): fix/record-field-list-beside-others makes
    the field one list where its record unifies with the list's, but a record field's two
    maps unify only where alike (`unify_maps_beside_null_values`), and `map<int>` beside
    `map<string>` is not. The field stayed a union, and normalisation wrote the list's maps
    as the record's: null. Two maps of a field forced to be a map are now unified as one
    forced map's values are (`unify_forced_maps`: `map_value_schema` of both values,
    nullable where either map is). A field with 50 schemas or more is unified by halves,
    whose map unification keeps every value too (tested with 56 distinct maps, one of
    strings beside records)
  - A forced map's values that are lists of kinds that clash (`["s"]` beside
    `[-7, "s"]`): `map_value_schema` unifies them before they are rewritten, while the
    list's items are still the type list `["integer", "string"]`, which only the rewrite
    makes a field each, so it fell back to text. Values that do not unify as they stand
    are now unified once rewritten (the rewrite they get after anyway), before text
  - A field forced within a forced map's values (`{"a": {"b": {"c": []}, "d": {"c": {"b":
    1}}}}`, `a` and `c` forced): `a` was made a map, and its values unified, before the
    `c` within them were forced, so `[]` and `{"b": 1}` unified as a list of records and
    `1` was lost. The fields within a map's values are now forced first (in its
    properties' schemas, a list's items and a union's branches, on both rewrites and in
    each row's forcing), but not its properties themselves: they are the map's keys, not
    fields. The final rewrite's forced-map arm, which a list's values now reach before
    `convert_to_map` (in the values rewritten for unifying), types the map an object, as
    `convert_to_map` does: it removed the list's `items` but left `"type": "array"`, which
    the interned path's builders could not read back (an error where the tip wrote
    text). Forcing every field bottom up instead (a first attempt) forced keys that share
    the field's name (`{"a": {"a": 0}}`), and 792 sets of 6,400 lost a value
- fuzz_lossless.py, every option set and both decodes (logs/e24), the tip against this:

  | seed (sets) | tip | this |
  |---|---|---|
  | 1 (600) | 59 | 0 |
  | 2, integers (800) | 86 | 1 |
  | 3 (2,000) | 179 | 0 |
  | 4 (1,500) | 125 | 1 |
  | 5 (1,500) | 140 | 0 |

  The two left are written as JSON text, no value made null: a forced map's values that
  are a list of nulls in one row and a list of records in another
  (`{"c": {"x": [null]}}`, `{"c": [{"x": [0]}]}`), and, without forcing, inferred maps
  (`map_threshold: 0`, `unify_maps`) in a record beside a list of records whose maps
  differ. The tip writes both as text too
- soundness_cases.py: no case loses a value. wikidata-pq's five columns keep the same
  Avro schemas and normalised text on chunk_0 and rowsw as on 1.0.4 (prod_schemas.py)
- Tests: forced_map.rs (a list's items in the values and none on the map, nested lists, a
  list of records; a record's and a list's forced maps unified into one list; lists of
  clashing kinds unified); polars-genson-py's forced_map_soundness_test.py checks that
  every scalar survives both decodes on 14 shapes from the fuzz (12 fail on the tip), and
  holds the two left as strict xfails
- test_interned_forced_maps_match_the_builders now also writes the Avro schema from each
  path's schema and compares the text: of its 442 compared sets, 7 have keywords in
  another order, and all 442 give the same Avro text. polars-jsonschema-bridge reads a
  schema's keywords by name (`get("type")`, `get("properties")`) and iterates only
  properties, whose order is the same, so the Polars types do not see keyword order
  either
- Mutants (each guard removed in turn, against genson-core's library, forced_map,
  forced_map_soundness and integration tests): caught for the list's items kept, the
  unification of forced maps, the rewritten values, and the map values forced first (each
  rewrite and each row's forcing). The four removals of `items` (in `convert_to_map` and
  in the final rewrite, on both rewrites) survive one at a time: each backs up the other,
  as the removals of `properties` and `required` there already did. A guard that unified a
  forced field's 50 or more schemas pairwise was taken out: the 56-map case reached the
  halves and kept every value without it
- Cost, scorecard workload, instructions on one thread against the parent: `usual` (a
  forced map) 567.6M to 569.1M (+0.25%), `claims` 2,523.0M to 2,525.1M (+0.08%), `labels`
  and `events` unchanged (±0.02%); the same schema hashes
- `coerce_strings` (fix/mixed-scalar-kinds-promote) now reads as `false` from a
  configuration serialised without it (`#[serde(default)]`): `SchemaInferenceConfig`
  derives `Deserialize` with no defaults, so a 1.0.3 configuration failed to read

### The round-3 stack end to end (logs/e23, tip/)

- wikidata-pq's per-column calls (`profile/e2e_one.py`) on the published 1.0.4 and on
  the stack's tip (this branch), sides alternated in each round, 3 measured processes
  each after a warm-up; the warm run's medians:

  | | 1.0.4 | tip |
  |---|---|---|
  | rowsw, the five columns | 4.943 s | 2.596 s (−47%) |
  | rowsw sitelinks (`normalise_json` + Polars' sink) | 2.636 s | 0.553 s |
  | rowsw sitelinks `normalise_json`, peak over base | +205 MB | +21 MB |
  | rowsw claims | 1.534 s | 1.444 s |
  | rowsw claims `normalise_from_parquet`, peak over base | +351 MB | +71 MB |
  | chunk_0, the five columns | 2.159 s | 2.190 s |

- Polars' sinks are now the largest part of every column: on rowsw 0.66 s of claims'
  1.44 s and 0.30 s of sitelinks' 0.55 s; on chunk_0 1.19 s of claims' 2.05 s (the
  conformed claims table; pyarrow writes it in 0.27-0.41 s, the 2026-10-18 journal). That
  write is wikidata-pq's, not polars-genson's
- In Python the extraction's gain is smaller than prof3 shows and the call's is lost in
  the noise: claims on rowsw, perf/decode-dtype-typed against this, 8 rounds,
  `extract_invariants` 0.328 → 0.218 s at the median, `normalise_from_parquet` 0.628 →
  0.593 s; its `read_parquet` stage varies from 0.12 to 0.60 s on both (page faults:
  see the first-touch note above), more than the saving
- chunk_0 is unchanged within its spread: its claims have nothing to extract, its
  sitelinks are 60 rows (0.090 → 0.069 s), and the rest is Polars

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
- `MIMALLOC_ALLOW_LARGE_OS_PAGES=0` (logs/read_firsttouch.log): a claims read takes a
  steady 0.06-0.07 s against 0.03-0.05 s warm with large pages, but large pages make the
  first read 0.33 s (system time): a trade between the first read and the others, not a
  saving, so the allocator's options are left as they are
- Skipping extraction where a chunk has nothing to extract (release-format claims): not
  exact, as the extraction section says (a repeated key's row is re-serialised), and
  0.025 s on chunk_0
- Restoring JSON Schema keyword order exactly on the interned forced-map path: the usual
  path's order depends on whether a row's list held ten or more items, which interned
  states do not keep; the schemas are compared up to keyword order instead
- Measurement hygiene: two worktrees built into one target directory share genson-core's
  build unit (cargo's metadata hash leaves the checkout's path out), so the second build
  can reuse the first's binary when its sources are older; every comparison here builds
  each side into a directory of its own and checks the binaries differ

## Missing

- Whether a 16-minute cast of the typed sitelinks struct to a narrower struct is a Polars
  issue or our dtype
