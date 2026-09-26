# Nulls, missing keys and empty values

JSON has several ways to say "nothing here": a key can be missing, a key can be `null`,
an array or object can be empty, and a whole row can be null. A Polars column only has
null, so genson has to decide which of these become null.

## What each one becomes

With default options, a missing key, a `null` value and an empty array all become null.
An empty object in a field that holds records becomes a struct whose fields are null.

```python exec="on"
import json
import polars as pl
import polars_genson

rows = [
    ('`{"a": 1, "tags": ["x"], "meta": {"k": 1}}`', '{"a": 1, "tags": ["x"], "meta": {"k": 1}}'),
    ("key missing: `{}`", "{}"),
    ('key is null: `{"a": null, "tags": null, "meta": null}`', '{"a": null, "tags": null, "meta": null}'),
    ('empty values: `{"tags": [], "meta": {}}`', '{"tags": [], "meta": {}}'),
]
def show(v):
    return "`" + json.dumps(v) + "`"

for empty_as_null in (True, False):
    df = pl.DataFrame({"j": [r for _, r in rows]})
    out = df.genson.normalise_json("j", empty_as_null=empty_as_null)
    print(f"**`empty_as_null={empty_as_null}`**" + (" (the default)" if empty_as_null else ""))
    print()
    print("| Row | `a` | `tags` | `meta` |")
    print("|---|---|---|---|")
    for (label, _), rec in zip(rows, out.to_dicts()):
        print(f"| {label} | {show(rec['a'])} | {show(rec['tags'])} | {show(rec['meta'])} |")
    print()
```

- **A missing key and a `null` value give the same result.** After normalising, you
  can't tell whether a field was absent or explicitly null. If that distinction matters,
  check the raw JSON before normalising.
- **Empty arrays become null by default.** Pass `empty_as_null=False` to keep them as
  empty lists. The option also covers empty maps (objects whose keys vary, see
  [Maps and records](maps-and-records.md)).
- **An empty object in a record field is not null.** It becomes a struct with every field
  null (`{"k": null}` above), with or without `empty_as_null`, because a record's fields
  are fixed and each one is simply absent.
- **The dtype isn't affected.** `tags` is `List(String)` either way. Only when a field is
  empty in *every* row does genson have no item type to infer, and then it uses
  `List(Null)` (see [Type mapping](type-mapping.md)).

## Null rows

A row that is null, rather than a JSON object, gives a row of nulls, so the output has
exactly one row per input row and still lines up with the other columns:

```python exec="on" source="above" result="text"
import polars as pl
import polars_genson

df = pl.DataFrame({"id": [1, 2, 3], "j": ['{"a": 1}', None, '{"a": 3}']})
print(df.select("id").hstack(df.genson.normalise_json("j")))
```

`normalise_from_parquet` does the same, and its `keep_columns` option copies other
columns (such as an `id`) into the output file alongside the normalised one. See
[JSON in Parquet files](../parquet.md).
