# Nulls, missing keys and empty values

JSON has several ways to say "nothing here": a key can be missing, a key can be `null`, an array or object can be empty, and a whole row can be null. A Polars column only has null, so genson has to decide which of these become null.

## What each one becomes

With default options, a missing key, a `null` value and an empty array all become null. An empty object in a field that holds records becomes a struct whose fields are null.

**`empty_as_null=True`** (the default)

| Row                                                    | `a`    | `tags`  | `meta`        |
| ------------------------------------------------------ | ------ | ------- | ------------- |
| `{"a": 1, "tags": ["x"], "meta": {"k": 1}}`            | `1`    | `["x"]` | `{"k": 1}`    |
| key missing: `{}`                                      | `null` | `null`  | `null`        |
| key is null: `{"a": null, "tags": null, "meta": null}` | `null` | `null`  | `null`        |
| empty values: `{"tags": [], "meta": {}}`               | `null` | `null`  | `{"k": null}` |

**`empty_as_null=False`**

| Row                                                    | `a`    | `tags`  | `meta`        |
| ------------------------------------------------------ | ------ | ------- | ------------- |
| `{"a": 1, "tags": ["x"], "meta": {"k": 1}}`            | `1`    | `["x"]` | `{"k": 1}`    |
| key missing: `{}`                                      | `null` | `null`  | `null`        |
| key is null: `{"a": null, "tags": null, "meta": null}` | `null` | `null`  | `null`        |
| empty values: `{"tags": [], "meta": {}}`               | `null` | `[]`    | `{"k": null}` |

- **A missing key and a `null` value give the same result.** After normalising, you can't tell whether a field was absent or explicitly null. If that distinction matters, check the raw JSON before normalising.
- **Empty arrays become null by default.** Pass `empty_as_null=False` to keep them as empty lists. The option also covers empty maps (objects whose keys vary, see [Maps and records](https://lmmx.github.io/polars-genson/pr-preview/pr-319/concepts/maps-and-records/index.md)).
- **An empty object in a record field is not null.** It becomes a struct with every field null (`{"k": null}` above), with or without `empty_as_null`, because a record's fields are fixed and each one is simply absent.
- **The dtype isn't affected.** `tags` is `List(String)` either way. Only when a field is empty in *every* row does genson have no item type to infer, and then it uses `List(Null)` (see [Type mapping](https://lmmx.github.io/polars-genson/pr-preview/pr-319/concepts/type-mapping/index.md)).

## Null rows

A row that is null, rather than a JSON object, gives a row of nulls, so the output has exactly one row per input row and still lines up with the other columns:

```python
import polars as pl
import polars_genson

df = pl.DataFrame({"id": [1, 2, 3], "j": ['{"a": 1}', None, '{"a": 3}']})
print(df.select("id").hstack(df.genson.normalise_json("j")))
```

```text
shape: (3, 2)
┌─────┬──────┐
│ id  ┆ a    │
│ --- ┆ ---  │
│ i64 ┆ i64  │
╞═════╪══════╡
│ 1   ┆ 1    │
│ 2   ┆ null │
│ 3   ┆ 3    │
└─────┴──────┘
```

`normalise_from_parquet` does the same, and its `keep_columns` option copies other columns (such as an `id`) into the output file alongside the normalised one. See [Normalise JSON stored in Parquet](https://lmmx.github.io/polars-genson/pr-preview/pr-319/guides/parquet/index.md).
