# Changelog

Release notes for earlier versions are on [GitHub Releases](https://github.com/lmmx/polars-genson/releases).

## Unreleased

- **`write_parquet` writes Arrow data with genson's Parquet writer.** Any object with `__arrow_c_stream__` (a pyarrow table or record batch reader, a Polars frame, genson's Arrow stream) is written as genson writes its own output, each row group's leaf columns encoded in parallel with the GIL released, its buffers not copied. Wikidata's claims statements (1.56 million deeply nested rows) write in 2.3 s on 4 threads against pyarrow's 8.5 s, 5% smaller, with the same schema and data as pyarrow writes. In Rust, `genson_core::parquet::write_arrow_batches`.

- **Tall flat Parquet output is smaller.** Data pages end at 1 MiB, as Polars' do, rather than every 20,000 rows (arrow-rs's default). zstd compresses each page on its own, so larger pages compress better: a 12.5-million-row table of a string and an Int64 column is 7.7% smaller (67.6 MB to 62.4 MB, under Polars' 62.6 MB), and a 94-column table 3.0% smaller. Tables of fewer than 20,000 rows, and nested columns whose pages already reached 1 MiB first, are written byte for byte as before.

- **Parquet output dictionary-encodes only columns whose values repeat.** A leaf column whose first values are mostly distinct (more than three quarters, as Polars decides) is written without a dictionary, which cost a hash per value and grew the file until arrow-rs gave the dictionary up: a table of 94 near-unique columns writes 2.5 times faster and 25% smaller. No file is larger; Wikidata claims are 0.5% smaller.

- **Narrow tables write faster.** genson's Parquet writer encodes several row groups at once, each leaf column of each a task of its own, where it encoded one row group at a time: a table of a few columns and many rows uses every thread rather than one (a 25-million-row string column: 2.4 s to 0.8 s on 4 threads). The file is unchanged, byte for byte.

- **Typed Parquet output can be written in a stored schema.** `normalise_from_parquet` takes `schema` (a `pl.Schema` or `pl.Struct`), checked against the inferred schema before anything is written (`schema_check="compatible"`, `"strict"` or `"none"`), and `unnest=True` to write the struct's fields as top-level columns. Fields the file lacks are written as nulls and the file is laid out as Polars writes the same frame, so chunks inferred separately share one schema without reading the output back to conform it. `check_schema_compatibility` gives the same verdict for two schemas. On a 10,000-entity chunk of the Wikidata dump, writing the claims this way took 3.0 s where genson's output read back, conformed and written again by Polars took 30 s.

- **`normalise_json(decode=False)` is faster.** It parsed every row into a `Value`, normalised it and serialised it; rows now go through the typed decoder into Arrow and are written back as JSON, with the same text. Rows the typed decoder declines, and inputs too few rows for the schema's width to pay for its columns, take the old path.

- **`no_unify` works through the `DataFrame.genson` methods.** `infer_json_schema`, `infer_polars_schema` and `normalise_json` on `df.genson` accepted `no_unify` but never passed it on, so a map candidate holding a key it names was still unified into a map. The expression functions (`polars_genson.infer_json_schema` and the rest) were not affected.

- **`DataFrame.genson.normalise_json` decodes rows straight into Polars dtypes.** With `decode=True` (the default) it inferred the schema twice, once to normalise and once for the dtypes, then wrote every row out as JSON text for `str.json_decode` to parse again. It now infers once and normalises the rows straight into Arrow, as typed `normalise_from_parquet` output does: 2.3–8.8× faster on 1,000 label maps, 20,000 event logs, 200,000 small records and a chunk of Wikidata claims, with less than half the peak memory on most, and the same frames. Rows the direct decoder cannot take, and `decode=<schema>`, still go by way of JSON text.

- **A key that is a list in some places and a record or scalar in others keeps its values.** Such a field inferred as a union of the list and the other types, and both `avro_to_polars_schema` and normalisation take a union's first branch: normalisation wraps a record or scalar into the list, but the list's items did not hold its fields or type, so its values were dropped (`{"d": 9}` beside `[{"d": null}]` gave `d: null`, as in #238), or, with the record first, every list was. The union is now one list whose items unify the list's items with the other branches (records fieldwise, scalars promoted to `__integer`-style fields as among a list's records), so every value is kept, whichever comes first, beside `null` or not. Branches the items cannot unify with (different scalars, a branch of several types), and every union with `wrap_scalars=False`, stay as they were.

- **A field that is a scalar in some rows and an empty list in the rest decodes.** Its Avro schema typed the list's items `null` (the list was empty in every row), but normalisation wraps the scalar into the list, so `normalise_json`'s output (`["x"]` under `List(Null)`) failed to decode, whatever `empty_as_null`. Such a list's items now take the scalars' types (`List(String)`, holding `["x"]`). Schemas change only there: a list that is ever non-empty is as before.

- **A value where the schema types `null` normalises to `null`, so the rows decode.** A record field or a list's items typed `null` (as the Avro conversion types those only ever null or empty) kept any value given there as it was, though the column is `Null` and cannot hold it: `normalise_json`'s output failed to decode. Such a value reaches there when a union's other branch is wrapped into its first. It is now `null`, as a value a field's type cannot hold is anywhere else, and the typed decoder takes these rows rather than leaving them to the slower path. Output changes only there: with `decode=False`, the JSON held the value; decoded, it failed.

- **A field forced to a map keeps its values' type when it is a scalar in some rows.** `force_field_types={"labels": "map"}` on a field that held `{"en": "x"}` in some rows and `"s"` in others (or only ever `"s"`) gave a map of `null` values, as the union, the type list (`["object", "string"]`) or the scalar type left it no properties to read its values from; normalisation kept the strings regardless only because a `null`-typed value passed through. Such a map now takes its records' values with the scalar, which normalisation wraps into the map as a value (`labels__string`): `map<string>` here, `map<int>` for integers, strings where they differ. On Wikidata claims fixtures, the forced `labels` maps hold strings where they held `null`.

- **A map only ever empty, or holding nulls alone, unifies with one holding values.** Its values are typed `null`, and unifying it with a map of typed values failed: the records holding them were not unified, so a field of such records was not inferred as a map (a map with `{}` in one row beside `{"en": "x"}` in another). Values typed `null` now join typed ones as `null` does: the map holds the type, nullable (`null` and `string`), so the nulls a map may hold stay possible; two maps of `null` values stay `null`. In a record's field, two maps whose values differ only in being nullable now unify too. Schemas change only where such maps failed to unify.

- **Normalisation no longer panics on a field whose type is `null` alone.** The Avro conversion can give a record the fields of a differently shaped one that shares its name (#238, avrotize-rs#28), leaving a field typed `["null"]` where the data holds values. Normalising such a value panicked; it is now `null`, as a value a field's type cannot hold is anywhere else. The record's lost fields are a separate bug, in the conversion.

## 1.0.3

- **A field some map values lack is no longer required.** Map unification made a record field required (and its Avro field non-nullable) whenever every record it unified listed the field, though a record that was itself unified lists every field seen so far, required or not. A record field missing from some values could then be required, depending on the order of the values, and normalised output held a record of nulls where the field was missing; it now holds `null`. Schemas change only by fields dropped from `required` and made nullable.

- **Inference, normalisation and Parquet I/O do far less work** ([#256](https://github.com/lmmx/polars-genson/issues/256)). Inference reads each row once as interned states and rewrites the merged schema as a hash-consed DAG, never writing it out. Normalisation compiles the schema into a plan once (`NormalisePlan` in genson-core) and writes rows straight into Arrow, and the `normalise_json` expression normalises its rows in parallel. Values are moved rather than copied. Inferred schemas and normalised output are unchanged, except by the two fixes in this release. On genson-core's scorecard corpora (`scripts/scorecard.py`: instructions on one thread, wall time on 4 threads), from 1.0.2 with the row-hashing change below to 1.0.3:

  | corpus                                                   | instructions | wall time      |
  | -------------------------------------------------------- | ------------ | -------------- |
  | claims (2,000 Wikidata-like rows, wikidata-pq's options) | −91%         | 1,762 → 233 ms |
  | events (20,000 event logs)                               | −77%         | 90 → 17 ms     |
  | narrow (100,000 small records)                           | −81%         | 186 → 23 ms    |
  | labels (1,000 label maps)                                | −54%         | 171 → 83 ms    |
  | entities (150 wide entities)                             | −59%         | 201 → 81 ms    |
  | the claims, normalised                                   | −78%         | 1,109 → 423 ms |

  A field forced to `"map"` takes the slower per-row path: 20% fewer instructions there, with no measurable change in time. These are genson-core's workloads; the Python package runs the same code.

  - **Parquet output is zstd-compressed.** genson wrote every Parquet file uncompressed; typed output, string output, lookup tables and pruned values are now written with zstd (level 1). On 200 rows of Wikidata claims the typed output is 5 MB where it was 16 MB, in the same time, and the normalised JSON 27 MB where it was 212 MB.
  - **`normalise_from_parquet` and `infer_from_parquet` decode only their column.** Reading one column decoded every column of the file, and each row was then copied into a `String`; the column is now decoded alone and its rows borrowed from the Arrow arrays. On a chunk of wikidata-pq's shape (3,000 rows, 332 MB of claims beside the labels, descriptions, aliases and sitelinks), its three map steps take 0.7–0.8 s where they took 3.5–4.6 s.
  - **`extract_invariants` reads each row's text once.** Each row was parsed into a tree, the fields removed and the row written out again; rows are now scanned as text, with the removed fields cut out, and only rows the scan cannot read exactly as before (invalid JSON, a line break, a repeated or escaped key in an object with fields removed) take the old path. The lookup table, the inferred schema and the normalised output are unchanged. On 3,000 rows of wikidata-pq's claims (332 MB, 98% of it the label maps removed), extraction takes 0.2–0.3 s where it took 0.7–0.8 s. With two values for one key, the error now names the first in row order. In genson-core's `extract_invariants`, the slim rows keep their own spacing and escapes instead of serde_json's compact form.
  - **Parquet string columns are decoded in parallel.** Where the file has an offset index (Polars and arrow-rs write one) or several row groups, the column is split into row ranges of about equal size, each decoded by its own reader. Decoding the claims of a chunk written as wikidata-pq writes them (Polars, zstd level 9) takes 0.05–0.06 s on 4 cores where it took 0.3–0.4 s.
  - **An integer beyond the 64-bit range no longer panics.** Inference read it with simd-json, which failed on it after serde_json had accepted the input; it is now a `number`, as serde_json reads it when normalising.
  - **Less time spent deduplicating rows.** Given 10 or more strings, inference hashes each string's schema to skip duplicates; a scalar field is now hashed without building its schema, for 6–17% less CPU time on Wikidata claims and on narrow rows. `SchemaBuilder::schema_hash` gives the same values as before.

## 1.0.2

- **`SchemaBuilder::add_schemas` keeps what the builder already held** (genson-core Rust API). Given schemas with more than 3 properties between them, it replaced the builder's schema for each of those properties instead of merging into it. Inference, genson-cli and the Python package do not call it.

## 1.0.1

- **A key missing from some objects is no longer required.** When a field's objects were merged, those holding no key in common were not counted, so a key could be marked required (and its Avro field non-nullable) although some rows lack it, and which keys were required could depend on the order of the rows or vary from run to run. Schemas change only by keys dropped from `required`.
- **`wrap_root` wraps each document as written.** Duplicate keys and number formatting now reach inference unchanged, as without `wrap_root`.
- **Faster inference.** About twice as fast on Wikidata claims with `unify_maps`, and on large rows with default options. See [How inference works](https://lmmx.github.io/polars-genson/pr-preview/pr-309/concepts/inference/index.md).

## 1.0.0

A JSON object genson infers as a map is now a Polars map, `pl.Map`, wherever genson gives Polars data or dtypes, and a Parquet `MAP` in typed output. See [Map types](https://lmmx.github.io/polars-genson/pr-preview/pr-309/concepts/map-types/index.md).

- **Polars ≥ 2 is required.** `import polars_genson` raises `ImportError` on Polars 1.x.
- **Maps are `pl.Map` by default.** `map_encoding` defaults to `"mapping"` throughout the Python package, as in genson-core and genson-cli: `infer_polars_schema`, `avro_to_polars_schema` and `normalise_json(decode=True)` give `pl.Map(pl.String, V)`, JSON string output writes each map as an object (`{"en": "Hello"}`), and `normalise_from_parquet(typed=True)` writes Parquet maps. Pass `map_encoding="kv"` for lists of `{key, value}` structs, as before.
- **`normalise_json(decode=schema)` decodes maps in the schema's shape:** a `pl.Map` from objects, a list of `{key, value}` structs from key/value lists, unless `map_encoding` is given.
- **`schema_to_dict` renders a `pl.Map`** as `{"map": {"key": ..., "value": ...}}`.
- **genson-core's `avro_to_arrow_type` and `avro_record_fields` take `native_map`,** which writes an Avro map as an Arrow `Map` in place of a list of `{key, value}` structs.

## 0.9.7

- **`infer_polars_schema` and `avro_to_polars_schema` take `map_encoding`.** Each map gets the dtype its encoding decodes to, so a schema can be asked for in the shape the data was normalised in. `"kv"`, the default, gives the list of `{key, value}` structs as before. polars-jsonschema-bridge now names a map `Map[String,V]` in place of `List[Struct[key:String,value:V]]`, which the `infer_polars_schema` expression shows.

## 0.9.3

- **`normalise_from_parquet(prune=...)` removes malformed records.** Every record holding one of the named fields is removed during normalisation and written to `prune_output_path`, and the fields are left out of the output schema. See [Remove malformed records](https://lmmx.github.io/polars-genson/pr-preview/pr-309/guides/prune/index.md). genson-cli has `--prune` and `--prune-output`.

## 0.9.1

- **`normalise_json(decode=schema)` accepts a `pl.Schema`,** such as one from `infer_polars_schema`, as its docstring says. It previously raised `TypeError`.
- **Fields forced to `"map"` keep their values' type.** `force_field_types={"x": "map"}` used to make every value a string; now a map of integers stays a map of integers, and values fall back to strings only when they have no common type. See [Maps and records](https://lmmx.github.io/polars-genson/pr-preview/pr-309/concepts/maps-and-records/index.md).

## 0.9.0

This release fixes several ways genson silently lost or mistyped data, makes inferred field order deterministic, and adds `extract_invariants` for JSON that embeds repeated data.

### Breaking changes

- **Polars ≥ 1.43.2 is required.** genson writes Parquet with `parquet` 60, which marks float columns with the IEEE 754 total column order, and Polars before 1.43.2 can't read typed output that has a float column.
- **Python ≥ 3.10 is required,** because Polars 1.43.2 requires it.
- **The `polars-lts-cpu` extra is replaced by `rtcompat`**, following Polars: `pip install polars-genson[rtcompat]`.
- **`infer_polars_schema` defaults to `avro=True`,** so it reports the dtypes that `normalise_json` and typed Parquet output produce. Pass `avro=False` for the old route.

### Output changes

- **A field that is sometimes a scalar and sometimes an object** keeps its scalars under a promoted `field__string`-style key by default. See [Mixed types](https://lmmx.github.io/polars-genson/pr-preview/pr-309/concepts/mixed-types/index.md).
- **Integers in a float field** become floats (`1` → `1.0`) instead of null.
- **Arrays that are empty in every row** get the dtype `List(Null)` instead of `List(Boolean)`.
- **Field order follows the order keys appear in the JSON,** for objects of any size. Objects of more than 32 keys previously gave a different order on each run. See [Field order](https://lmmx.github.io/polars-genson/pr-preview/pr-309/concepts/key-order/index.md).
- **JSON Schema to Polars conversion** maps objects with `additionalProperties` to `List(Struct{key, value})`, and nullable types to their non-null type.

### New

- **`extract_invariants`** on `normalise_from_parquet`: moves fields that are invariant per key into a lookup table before inference. See [Factor out repeated embedded data](https://lmmx.github.io/polars-genson/pr-preview/pr-309/guides/extract-invariants/index.md).

### Dependencies

- polars / polars-arrow 0.55.2, pyo3 0.29, pyo3-polars 0.28, arrow / parquet 60.
