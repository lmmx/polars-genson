# How JSON types map to Polars dtypes

JSON has six kinds of value: strings, numbers, booleans, null, arrays and objects. A Polars column has exactly one dtype. When genson normalises a JSON column, it chooses one dtype per field that can hold every row's value, and converts each value to it.

The table below shows what `normalise_json` produces for two rows of each kind of input, with default options. It's generated from genson itself each time these docs are built.

| Input                                              | JSON rows                                                     | Polars dtype                                | Values                                                        |
| -------------------------------------------------- | ------------------------------------------------------------- | ------------------------------------------- | ------------------------------------------------------------- |
| String                                             | `{"x": "a"}` `{"x": "b"}`                                     | `String`                                    | `"a"` `"b"`                                                   |
| Integer                                            | `{"x": 1}` `{"x": 2}`                                         | `Int64`                                     | `1` `2`                                                       |
| Float                                              | `{"x": 1.5}` `{"x": 2.5}`                                     | `Float64`                                   | `1.5` `2.5`                                                   |
| Integer and float                                  | `{"x": 1}` `{"x": 1.5}`                                       | `Float64`                                   | `1.0` `1.5`                                                   |
| Boolean                                            | `{"x": true}` `{"x": false}`                                  | `Boolean`                                   | `true` `false`                                                |
| Always null                                        | `{"x": null}` `{"x": null}`                                   | `Null`                                      | `null` `null`                                                 |
| Sometimes null                                     | `{"x": 1}` `{"x": null}`                                      | `Int64`                                     | `1` `null`                                                    |
| Sometimes missing                                  | `{"x": 1}` `{}`                                               | `Int64`                                     | `1` `null`                                                    |
| Array                                              | `{"x": ["a", "b"]}` `{"x": ["c"]}`                            | `List(String)`                              | `["a", "b"]` `["c"]`                                          |
| Empty array                                        | `{"x": ["a"]}` `{"x": []}`                                    | `List(String)`                              | `["a"]` `null`                                                |
| Always-empty array                                 | `{"x": []}` `{"x": []}`                                       | `List(Null)`                                | `null` `null`                                                 |
| String or array                                    | `{"x": "a"}` `{"x": ["b", "c"]}`                              | `List(String)`                              | `["a"]` `["b", "c"]`                                          |
| Object (record)                                    | `{"x": {"a": 1}}` `{"x": {"a": 2, "b": "z"}}`                 | `Struct({'a': Int64, 'b': String})`         | `{"a": 1, "b": null}` `{"a": 2, "b": "z"}`                    |
| Object with varying keys (map) (`map_threshold=1`) | `{"x": {"en": "hi"}}` `{"x": {"fr": "salut", "de": "hallo"}}` | `Map(String, String)`                       | `{"en": "hi"}` `{"fr": "salut", "de": "hallo"}`               |
| Scalar or object                                   | `{"x": "a"}` `{"x": {"n": 1}}`                                | `Struct({'n': Int64, 'x__string': String})` | `{"n": null, "x__string": "a"}` `{"n": 1, "x__string": null}` |

## Reading the table

- **Scalars** map directly: strings to `String`, integers to `Int64`, floats to `Float64`, booleans to `Boolean`.
- **Integers and floats in the same field** become `Float64`, with the integers widened (`1` becomes `1.0`).
- **Null and missing are the same after normalising.** A key that is absent and a key whose value is `null` both give a null. See [Nulls, missing keys and empty values](https://lmmx.github.io/polars-genson/pr-preview/pr-315/concepts/nulls-and-empty-values/index.md).
- **Empty arrays become null** by default, and an array that is empty in every row gets the dtype `List(Null)`, since there is no item type to infer.
- **A value that is sometimes a string and sometimes an array** becomes a list: the string is wrapped into a one-item list.
- **Objects** become structs when their keys are a fixed set (a *record*), or `pl.Map` columns when their keys vary (a *map*). See [Maps and records](https://lmmx.github.io/polars-genson/pr-preview/pr-315/concepts/maps-and-records/index.md) and [Map types](https://lmmx.github.io/polars-genson/pr-preview/pr-315/concepts/map-types/index.md).
- **A value that is sometimes a scalar and sometimes an object** becomes a struct, and the scalar is kept under a promoted field named after the field and its type (here `x__string`). See [Mixed types](https://lmmx.github.io/polars-genson/pr-preview/pr-315/concepts/mixed-types/index.md).
