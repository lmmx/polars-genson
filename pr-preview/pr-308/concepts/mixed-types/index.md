# Mixed types in one field

The same JSON field can hold different kinds of value in different rows. A Polars column has one dtype, so genson either finds a type that holds all of them or splits the field.

## A number, sometimes an integer and sometimes a float

Integers are widened to floats, and the column is `Float64`:

```python
import polars as pl
import polars_genson

df = pl.DataFrame({"j": ['{"x": 1}', '{"x": 1.5}']})
print(df.genson.normalise_json("j"))
```

```text
shape: (2, 1)
┌─────┐
│ x   │
│ --- │
│ f64 │
╞═════╡
│ 1.0 │
│ 1.5 │
└─────┘
```

## A value, sometimes a scalar and sometimes an array

The scalar is wrapped into a one-item list, and the column is a list:

```python
import polars as pl
import polars_genson

df = pl.DataFrame({"j": ['{"x": "a"}', '{"x": ["b", "c"]}']})
print(df.genson.normalise_json("j"))
```

```text
shape: (2, 1)
┌────────────┐
│ x          │
│ ---        │
│ list[str]  │
╞════════════╡
│ ["a"]      │
│ ["b", "c"] │
└────────────┘
```

## A value, sometimes a scalar and sometimes an object

The column becomes a struct, and the scalar is kept in an extra field named after the field and the scalar's type (`x__string` here). This is *scalar promotion*, controlled by `wrap_scalars` (on by default).

```python
import polars as pl
import polars_genson

df = pl.DataFrame({"j": ['{"x": "plain"}', '{"x": {"amount": 3, "unit": "kg"}}']})
out = df.genson.normalise_json("j")
print(out.schema)
print(out["x"].to_list())
```

```text
Schema({'x': Struct({'amount': Int64, 'unit': String, 'x__string': String})})
[{'amount': None, 'unit': None, 'x__string': 'plain'}, {'amount': 3, 'unit': 'kg', 'x__string': None}]
```

## Keeping a field's shape stable across files: `force_scalar_promotion`

If a field is a plain scalar in one file but an object in another, the two files get different schemas. `force_scalar_promotion` promotes the named fields even when every row is a scalar, so the field is always a struct:

```python
import polars as pl
import polars_genson

df = pl.DataFrame({"j": ['{"precision": 11}', '{"precision": 12}']})
out = df.genson.normalise_json("j", force_scalar_promotion={"precision"})
print(out.schema)
print(out["precision"].to_list())
```

```text
Schema({'precision': Struct({'precision__integer': Int64})})
[{'precision__integer': 11}, {'precision__integer': 12}]
```

## Numbers and booleans written as strings: `coerce_strings`

With `coerce_strings=True`, a string in a numeric or boolean field is parsed into that type (`"42"` becomes `42`). Strings that don't parse become null.

```python
import polars as pl
import polars_genson

df = pl.DataFrame({"j": ['{"x": 1}', '{"x": "42"}']})
print(df.genson.normalise_json("j", coerce_strings=True))
```

```text
shape: (2, 1)
┌─────┐
│ x   │
│ --- │
│ i64 │
╞═════╡
│ 1   │
│ 42  │
└─────┘
```

## Current limitation: different scalar types in one field

When a field holds different kinds of scalar, such as numbers in some rows and strings in others, genson keeps the first type in its precedence order (boolean, then integer, then number, then string), and values of the other types become null:

```python
import polars as pl
import polars_genson

df = pl.DataFrame({"j": ['{"x": 1}', '{"x": "a"}']})
print(df.genson.normalise_json("j"))
```

```text
shape: (2, 1)
┌────────────┐
│ x          │
│ ---        │
│ struct[2]  │
╞════════════╡
│ {1,null}   │
│ {null,"a"} │
└────────────┘
```

If a field can hold numbers written as strings, `coerce_strings=True` recovers them. Otherwise, check such fields in the raw JSON before normalising.
