# Changelog

Release notes for earlier versions are on
[GitHub Releases](https://github.com/lmmx/polars-genson/releases).

## Unreleased

- **`DataFrame.genson.normalise_json` decodes rows straight into Polars dtypes.** With
  `decode=True` (the default) it inferred the schema twice, once to normalise and once for
  the dtypes, then wrote every row out as JSON text for `str.json_decode` to parse again.
  It now infers once and normalises the rows straight into Arrow, as typed
  `normalise_from_parquet` output does: 2.3–8.8× faster on 1,000 label maps, 20,000 event
  logs, 200,000 small records and a chunk of Wikidata claims, with less than half the peak
  memory on most, and the same frames. Rows the direct decoder cannot take, and
  `decode=<schema>`, still go by way of JSON text.

- **A key that is a list in some places and a record or scalar in others keeps its
  values.** Such a field inferred as a union of the list and the other types, and both
  `avro_to_polars_schema` and normalisation take a union's first branch: normalisation
  wraps a record or scalar into the list, but the list's items did not hold its fields or
  type, so its values were dropped (`{"d": 9}` beside `[{"d": null}]` gave `d: null`, as
  in #238), or, with the record first, every list was. The union is now one list whose
  items unify the list's items with the other branches (records fieldwise, scalars
  promoted to `__integer`-style fields as among a list's records), so every value is
  kept, whichever comes first, beside `null` or not. Branches the items cannot unify
  with (different scalars, a branch of several types), and every union with
  `wrap_scalars=False`, stay as they were.
- **A field that is a scalar in some rows and an empty list in the rest decodes.** Its
  Avro schema typed the list's items `null` (the list was empty in every row), but
  normalisation wraps the scalar into the list, so `normalise_json`'s output (`["x"]`
  under `List(Null)`) failed to decode, whatever `empty_as_null`. Such a list's items now
  take the scalars' types (`List(String)`, holding `["x"]`). Schemas change only there:
  a list that is ever non-empty is as before.
- **A value where the schema types `null` normalises to `null`, so the rows decode.** A
  record field or a list's items typed `null` (as the Avro conversion types those only
  ever null or empty) kept any value given there as it was, though the column is
  `Null` and cannot hold it: `normalise_json`'s output failed to decode. Such a value
  reaches there when a union's other branch is wrapped into its first. It is now
  `null`, as a value a field's type cannot hold is anywhere else, and the typed decoder
  takes these rows rather than leaving them to the slower path. Output changes only
  there: with `decode=False`, the JSON held the value; decoded, it failed.
- **A field forced to a map keeps its values' type when it is a scalar in some rows.**
  `force_field_types={"labels": "map"}` on a field that held `{"en": "x"}` in some rows
  and `"s"` in others (or only ever `"s"`) gave a map of `null` values, as the union, the
  type list (`["object", "string"]`) or the scalar type left it no properties to read its
  values from; normalisation kept the strings regardless only because a `null`-typed
  value passed through. Such a map now takes its records' values with the scalar, which
  normalisation wraps into the map as a value (`labels__string`): `map<string>` here,
  `map<int>` for integers, strings where they differ. On Wikidata claims fixtures, the
  forced `labels` maps hold strings where they held `null`.
- **A map only ever empty, or holding nulls alone, unifies with one holding values.** Its
  values are typed `null`, and unifying it with a map of typed values failed: the
  records holding them were not unified, so a field of such records was not inferred as
  a map (a map with `{}` in one row beside `{"en": "x"}` in another). Values typed `null`
  now join typed ones as `null` does: the map holds the type, nullable (`null` and
  `string`), so the nulls a map may hold stay possible; two maps of `null` values stay
  `null`. In a record's field, two maps whose values differ only in being nullable now
  unify too. Schemas change only where such maps failed to unify.
- **Normalisation no longer panics on a field whose type is `null` alone.** The Avro
  conversion can give a record the fields of a differently shaped one that shares its name
  (#238, avrotize-rs#28), leaving a field typed `["null"]` where the data holds values.
  Normalising such a value panicked; it is now `null`, as a value a field's type cannot
  hold is anywhere else. The record's lost fields are a separate bug, in the conversion.
- **A key missing from some objects is no longer required.** When a field's objects
  were merged, those holding no key in common were not counted, so a key could be
  marked required (and its Avro field non-nullable) although some rows lack it, and
  which keys were required could depend on the order of the rows or vary from run to
  run. Schemas change only by keys dropped from `required`.
- **A field some map values lack is no longer required.** Map unification made a
  record field required (and its Avro field non-nullable) whenever every record it
  unified listed the field, though a record that was itself unified lists every field
  seen so far, required or not. A record field missing from some values could then be
  required, depending on the order of the values, and normalised output held a record of
  nulls where the field was missing; it now holds `null`. Schemas change only by fields
  dropped from `required` and made nullable.
- **Parquet output is zstd-compressed.** genson wrote every Parquet file uncompressed;
  typed output, string output, lookup tables and pruned values are now written with
  zstd (level 1). On 200 rows of Wikidata claims the typed output is 5 MB where it was
  16 MB, in the same time, and the normalised JSON 27 MB where it was 212 MB.
- **`normalise_from_parquet` and `infer_from_parquet` decode only their column.** Reading
  one column decoded every column of the file, and each row was then copied into a
  `String`; the column is now decoded alone and its rows borrowed from the Arrow arrays.
  On a chunk of wikidata-pq's shape (3,000 rows, 332 MB of claims beside the labels,
  descriptions, aliases and sitelinks), its three map steps take 0.7–0.8 s where they
  took 3.5–4.6 s.
- **`extract_invariants` reads each row's text once.** Each row was parsed into a tree,
  the fields removed and the row written out again; rows are now scanned as text, with
  the removed fields cut out, and only rows the scan cannot read exactly as before
  (invalid JSON, a line break, a repeated or escaped key in an object with fields
  removed) take the old path. The lookup table, the inferred schema and the normalised
  output are unchanged. On 3,000 rows of wikidata-pq's claims (332 MB, 98% of it the
  label maps removed), extraction takes 0.2–0.3 s where it took 0.7–0.8 s. With two
  values for one key, the error now names the first in row order. In genson-core's
  `extract_invariants`, the slim rows keep their own spacing and escapes instead of
  serde_json's compact form.
- **Parquet string columns are decoded in parallel.** Where the file has an offset index
  (Polars and arrow-rs write one) or several row groups, the column is split into row
  ranges of about equal size, each decoded by its own reader. Decoding the claims of a
  chunk written as wikidata-pq writes them (Polars, zstd level 9) takes 0.05–0.06 s on
  4 cores where it took 0.3–0.4 s.
- **Less copying in map unification and normalisation.** `json!` copies each value it is
  given by serialising it; map unification built each unified record, array and map
  that way, so every level of a unified schema copied everything beneath it again, and
  normalisation built every map entry of the key/value encodings that way. The values
  are now moved in. On a release-format chunk of wikidata-pq's claims (7,128 rows),
  inference takes 17% fewer instructions, with the same schemas and rows.
- **Faster merging and schema writing in inference.** Merging each row's schema into the
  running one moved every node's strategies through two new vectors to order them, and
  rebuilt every object's recorded keywords, though nearly always there was one strategy
  and no keyword new; those cases are now merged as they are. An object of 16 or more
  properties writes their schemas in parallel. On the release-format chunk, inference
  takes 0.39–0.43 s on 4 cores where it took 0.45–0.46 s, and 0.53–0.54 s before the
  entry above; the same schemas.
- **Normalisation reads the schema once.** It looked up each schema node's kind, fields,
  their names and the fields `prune` names for every value; the schema is now compiled
  once into a plan (`NormalisePlan` in genson-core, for normalising many values against
  one schema), and `normalise_from_parquet` and the `normalise_json` expression compile
  it once for all their rows, which the expression now normalises in parallel. On the
  release-format chunk, normalisation takes 60% fewer instructions, with the same rows
  and pruned values.
- **An integer beyond the 64-bit range no longer panics.** Inference read it with
  simd-json, which failed on it after serde_json had accepted the input; it is now a
  `number`, as serde_json reads it when normalising.
- **`wrap_root` wraps each document as written.** Duplicate keys and number formatting
  now reach inference unchanged, as without `wrap_root`.
- **Faster inference.** About twice as fast on Wikidata claims with `unify_maps`, and
  on large rows with default options. See [How inference works](concepts/inference.md).
- **Less time spent deduplicating rows.** Given 10 or more strings, inference hashes each
  string's schema to skip duplicates; a scalar field is now hashed without building its
  schema, for 6–17% less CPU time on Wikidata claims and on narrow rows.
  `SchemaBuilder::schema_hash` gives the same values as before.
- **`SchemaBuilder::add_schemas` keeps what the builder already held** (genson-core
  Rust API). Given schemas with more than 3 properties between them, it replaced the
  builder's schema for each of those properties instead of merging into it. Inference,
  genson-cli and the Python package do not call it.

## 1.0.0

A JSON object genson infers as a map is now a Polars map, `pl.Map`, wherever genson
gives Polars data or dtypes, and a Parquet `MAP` in typed output. See
[Map types](concepts/map-types.md).

- **Polars ≥ 2 is required.** `import polars_genson` raises `ImportError` on Polars 1.x.
- **Maps are `pl.Map` by default.** `map_encoding` defaults to `"mapping"` throughout
  the Python package, as in genson-core and genson-cli: `infer_polars_schema`,
  `avro_to_polars_schema` and `normalise_json(decode=True)` give
  `pl.Map(pl.String, V)`, JSON string output writes each map as an object
  (`{"en": "Hello"}`), and `normalise_from_parquet(typed=True)` writes Parquet maps.
  Pass `map_encoding="kv"` for lists of `{key, value}` structs, as before.
- **`normalise_json(decode=schema)` decodes maps in the schema's shape:** a `pl.Map`
  from objects, a list of `{key, value}` structs from key/value lists, unless
  `map_encoding` is given.
- **`schema_to_dict` renders a `pl.Map`** as `{"map": {"key": ..., "value": ...}}`.
- **genson-core's `avro_to_arrow_type` and `avro_record_fields` take `native_map`,**
  which writes an Avro map as an Arrow `Map` in place of a list of `{key, value}`
  structs.

## 0.9.7

- **`infer_polars_schema` and `avro_to_polars_schema` take `map_encoding`.** Each map
  gets the dtype its encoding decodes to, so a schema can be asked for in the shape the
  data was normalised in. `"kv"`, the default, gives the list of `{key, value}` structs
  as before. polars-jsonschema-bridge now names a map `Map[String,V]` in place of
  `List[Struct[key:String,value:V]]`, which the `infer_polars_schema` expression shows.

## 0.9.3

- **`normalise_from_parquet(prune=...)` removes malformed records.** Every record holding
  one of the named fields is removed during normalisation and written to
  `prune_output_path`, and the fields are left out of the output schema. See
  [Remove malformed records](guides/prune.md). genson-cli has `--prune` and
  `--prune-output`.

## 0.9.1

- **`normalise_json(decode=schema)` accepts a `pl.Schema`,** such as one from
  `infer_polars_schema`, as its docstring says. It previously raised `TypeError`.
- **Fields forced to `"map"` keep their values' type.** `force_field_types={"x": "map"}`
  used to make every value a string; now a map of integers stays a map of integers, and
  values fall back to strings only when they have no common type. See
  [Maps and records](concepts/maps-and-records.md).

## 0.9.0

This release fixes several ways genson silently lost or mistyped data, makes inferred
field order deterministic, and adds `extract_invariants` for JSON that embeds repeated
data.

### Breaking changes

- **Polars ≥ 1.43.2 is required.** genson writes Parquet with `parquet` 60, which marks
  float columns with the IEEE 754 total column order, and Polars before 1.43.2 can't read
  typed output that has a float column.
- **Python ≥ 3.10 is required,** because Polars 1.43.2 requires it.
- **The `polars-lts-cpu` extra is replaced by `rtcompat`**, following Polars:
  `pip install polars-genson[rtcompat]`.
- **`infer_polars_schema` defaults to `avro=True`,** so it reports the dtypes that
  `normalise_json` and typed Parquet output produce. Pass `avro=False` for the old route.

### Output changes

- **A field that is sometimes a scalar and sometimes an object** keeps its scalars under a
  promoted `field__string`-style key by default. See [Mixed types](concepts/mixed-types.md).
- **Integers in a float field** become floats (`1` → `1.0`) instead of null.
- **Arrays that are empty in every row** get the dtype `List(Null)` instead of
  `List(Boolean)`.
- **Field order follows the order keys appear in the JSON,** for objects of any size.
  Objects of more than 32 keys previously gave a different order on each run. See
  [Field order](concepts/key-order.md).
- **JSON Schema to Polars conversion** maps objects with `additionalProperties` to
  `List(Struct{key, value})`, and nullable types to their non-null type.

### New

- **`extract_invariants`** on `normalise_from_parquet`: moves fields that are invariant per
  key into a lookup table before inference. See
  [Factor out repeated embedded data](guides/extract-invariants.md).

### Dependencies

- polars / polars-arrow 0.55.2, pyo3 0.29, pyo3-polars 0.28, arrow / parquet 60.
