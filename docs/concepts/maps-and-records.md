# Maps and records: objects with fixed or varying keys

A JSON object can play two different roles:

- A **record** has a fixed set of fields, like `{"name": ..., "age": ...}`. Each field is
  its own column, so it becomes a Polars **struct**.
- A **map** has keys that are data, like `{"en": "hello", "fr": "bonjour"}` or scores
  keyed by subject. The keys vary from row to row, so making each key a struct field
  would give one field per distinct key. genson instead encodes a map as a **list of
  `{key, value}` structs**, since Polars has no map type.

genson decides which role an object plays from what it sees across all rows.

## How genson decides

An object becomes a map when it has more than `map_threshold` distinct keys across all
rows (default 20) and its values all have the same type. Otherwise it stays a record.

```python exec="on" source="above" result="text" session="maps"
import polars as pl
import polars_genson

df = pl.DataFrame({"j": [
    '{"scores": {"maths": 90, "art": 75}}',
    '{"scores": {"history": 60}}',
]})
print(df.genson.infer_polars_schema("j"))                   # 3 distinct keys: a record
print(df.genson.infer_polars_schema("j", map_threshold=1))  # threshold lowered: a map
```

As a record, `scores` has one field per subject seen in any row, and each row has nulls
for the subjects it doesn't have. As a map, each row lists only its own subjects:

```python exec="on" source="above" result="text" session="maps"
print(df.genson.normalise_json("j", map_threshold=1))
```

Use a map when the keys are data (IDs, languages, dates, names) and a record when they
are a known set of fields.

## Choosing per field

`map_threshold` applies to every object. To set the role of particular fields, use
`force_field_types`:

```python exec="on" source="above" result="text" session="maps"
print(df.genson.infer_polars_schema("j", force_field_types={"scores": "record"}, map_threshold=1))
```

Forcing a field to `"map"` makes it a map whatever its number of keys, and its values
keep their type:

```python exec="on" source="above" result="text" session="maps"
print(df.genson.infer_polars_schema("j", force_field_types={"scores": "map"}))
```

If the values differ, genson unifies them the same way as for `unify_maps` below. Only
values with no common type, such as numbers in some keys and strings in others, fall back
to strings, so that no value is lost.

`map_max_required_keys` adds a second condition: an object with more than that many keys
present in *every* row stays a record, however many keys it has in total.

## Maps of records that differ: `unify_maps`

When a map's values are themselves objects with different fields, they can't share one
value type, so by default genson keeps the whole object as a record:

```python exec="on" source="above" result="text" session="maps"
letters = pl.DataFrame({"j": [
    '{"letters": {"a": {"vowel": true, "n": 3}, "b": {"consonant": true, "n": 1}}}'
]})
print(letters.genson.infer_polars_schema("j", map_threshold=1))
```

With `unify_maps=True`, genson merges the values into one record type, with a field for
every field seen and nulls where a value lacks one, and the object becomes a map:

```python exec="on" source="above" result="text" session="maps"
print(letters.genson.infer_polars_schema("j", map_threshold=1, unify_maps=True))
print(letters.genson.normalise_json("j", map_threshold=1, unify_maps=True)["letters"].to_list())
```

To keep particular fields out of this merging, list them in `no_unify`.

## Map encodings

Typed output always uses the list-of-`{key, value}` encoding. When you ask for JSON
strings instead (`decode=False`), `map_encoding` chooses how maps are written:

```python exec="on" source="above" result="text" session="maps"
for encoding in ("kv", "mapping", "entries"):
    out = df.genson.normalise_json("j", map_threshold=1, map_encoding=encoding, decode=False)
    print(f"{encoding:8s}", out[0])
```

Empty maps become null by default, like empty arrays; see
[Nulls, missing keys and empty values](nulls-and-empty-values.md).
