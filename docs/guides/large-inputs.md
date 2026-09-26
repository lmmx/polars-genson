# Process large inputs

For large JSON data, three things make the biggest difference to speed and memory: the
route the data takes, how much redundant data it carries, and how many row schemas are
held at once during inference.

## Use `normalise_from_parquet` with `typed=True`

The DataFrame methods (`df.genson.normalise_json` and friends) work on data already
loaded into Polars. For data in a Parquet file, `normalise_from_parquet` does the whole
job in Rust: it reads the column, infers, normalises and writes the result, without a
DataFrame in between. With `typed=True` it writes typed columns directly, instead of
writing JSON strings that then have to be parsed again. See
[Normalise JSON stored in Parquet](parquet.md).

## Remove repeated embedded data

If the JSON carries the same nested value in many rows (a customer embedded in every
order, a label map in every reference to an entity), extract it with
`extract_invariants`. The work then scales with the number of distinct values rather
than the number of copies. See [Factor out repeated embedded data](extract-invariants.md).

## Limit memory during inference: `max_builders`

Schema inference builds a schema for every row in parallel (on a thread pool the size of
your CPU core count), then merges them into one. By default every row's schema is held
in memory until that single merge, so peak memory grows with the number of rows.

`max_builders` caps this: rows are processed in chunks of `max_builders`, and each
chunk's schemas are merged before the next chunk starts. With 1,000,000 rows and
`max_builders=1000`, at most 1000 row schemas are held at once.

```python
schema = df.genson.infer_polars_schema("json", max_builders=1000)
```

`max_builders` is accepted by `infer_json_schema`, `infer_polars_schema`,
`normalise_json`, `infer_from_parquet` and `normalise_from_parquet`. Smaller values lower
peak memory at some cost in speed.

## See where the time goes: `profile=True`

Pass `profile=True` to print how long each step takes (reading, inference,
normalisation, writing) to standard error. It's the quickest way to tell which of the
steps above will help.
