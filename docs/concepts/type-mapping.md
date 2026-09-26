# How JSON types map to Polars dtypes

JSON has six kinds of value: strings, numbers, booleans, null, arrays and objects. A
Polars column has exactly one dtype. When genson normalises a JSON column, it chooses
one dtype per field that can hold every row's value, and converts each value to it.

The table below shows what `normalise_json` produces for two rows of each kind of
input, with default options. It's generated from genson itself each time these docs are
built.

```python exec="on"
import json
import polars as pl
import polars_genson

cases = [
    ("String", ['{"x": "a"}', '{"x": "b"}'], {}),
    ("Integer", ['{"x": 1}', '{"x": 2}'], {}),
    ("Float", ['{"x": 1.5}', '{"x": 2.5}'], {}),
    ("Integer and float", ['{"x": 1}', '{"x": 1.5}'], {}),
    ("Boolean", ['{"x": true}', '{"x": false}'], {}),
    ("Always null", ['{"x": null}', '{"x": null}'], {}),
    ("Sometimes null", ['{"x": 1}', '{"x": null}'], {}),
    ("Sometimes missing", ['{"x": 1}', '{}'], {}),
    ("Array", ['{"x": ["a", "b"]}', '{"x": ["c"]}'], {}),
    ("Empty array", ['{"x": ["a"]}', '{"x": []}'], {}),
    ("Always-empty array", ['{"x": []}', '{"x": []}'], {}),
    ("String or array", ['{"x": "a"}', '{"x": ["b", "c"]}'], {}),
    ("Object (record)", ['{"x": {"a": 1}}', '{"x": {"a": 2, "b": "z"}}'], {}),
    ("Object with varying keys (map)", ['{"x": {"en": "hi"}}', '{"x": {"fr": "salut", "de": "hallo"}}'], {"map_threshold": 1}),
    ("Scalar or object", ['{"x": "a"}', '{"x": {"n": 1}}'], {}),
]

def cell(v):
    return "`" + json.dumps(v, ensure_ascii=False) + "`"

print("| Input | JSON rows | Polars dtype | Values |")
print("|---|---|---|---|")
for name, rows, opts in cases:
    out = pl.DataFrame({"j": rows}).genson.normalise_json("j", **opts)
    dtype = out.schema["x"]
    values = out["x"].to_list()
    rows_md = "<br>".join(f"`{r}`" for r in rows)
    opt_md = "".join(f"<br>(`{k}={v}`)" for k, v in opts.items())
    print(f"| {name}{opt_md} | {rows_md} | `{dtype}` | {'<br>'.join(cell(v) for v in values)} |")
```

## Reading the table

- **Scalars** map directly: strings to `String`, integers to `Int64`, floats to
  `Float64`, booleans to `Boolean`.
- **Integers and floats in the same field** become `Float64`, with the integers widened
  (`1` becomes `1.0`).
- **Null and missing are the same after normalising.** A key that is absent and a key
  whose value is `null` both give a null. See
  [Nulls, missing keys and empty values](nulls-and-empty-values.md).
- **Empty arrays become null** by default, and an array that is empty in every row gets
  the dtype `List(Null)`, since there is no item type to infer.
- **A value that is sometimes a string and sometimes an array** becomes a list: the
  string is wrapped into a one-item list.
- **Objects** become structs when their keys are a fixed set (a *record*), or lists of
  `{key, value}` structs when their keys vary (a *map*). See
  [Maps and records](maps-and-records.md).
- **A value that is sometimes a scalar and sometimes an object** becomes a struct, and
  the scalar is kept under a promoted field named after the field and its type (here
  `x__string`). See [Mixed types](mixed-types.md).
