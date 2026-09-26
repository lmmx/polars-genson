# Mixed types in one field

The same JSON field can hold different kinds of value in different rows. A Polars column
has one dtype, so genson either finds a type that holds all of them or splits the field.

## A number, sometimes an integer and sometimes a float

Integers are widened to floats, and the column is `Float64`:

```python exec="on" source="above" result="text"
import polars as pl
import polars_genson

df = pl.DataFrame({"j": ['{"x": 1}', '{"x": 1.5}']})
print(df.genson.normalise_json("j"))
```

## A value, sometimes a scalar and sometimes an array

The scalar is wrapped into a one-item list, and the column is a list:

```python exec="on" source="above" result="text"
import polars as pl
import polars_genson

df = pl.DataFrame({"j": ['{"x": "a"}', '{"x": ["b", "c"]}']})
print(df.genson.normalise_json("j"))
```

## A value, sometimes a scalar and sometimes an object

The column becomes a struct, and the scalar is kept in an extra field named after the
field and the scalar's type (`x__string` here). This is *scalar promotion*, controlled
by `wrap_scalars` (on by default).

```python exec="on" source="above" result="text"
import polars as pl
import polars_genson

df = pl.DataFrame({"j": ['{"x": "plain"}', '{"x": {"amount": 3, "unit": "kg"}}']})
out = df.genson.normalise_json("j")
print(out.schema)
print(out["x"].to_list())
```

## Keeping a field's shape stable across files: `force_scalar_promotion`

If a field is a plain scalar in one file but an object in another, the two files get
different schemas. `force_scalar_promotion` promotes the named fields even when every
row is a scalar, so the field is always a struct:

```python exec="on" source="above" result="text"
import polars as pl
import polars_genson

df = pl.DataFrame({"j": ['{"precision": 11}', '{"precision": 12}']})
out = df.genson.normalise_json("j", force_scalar_promotion={"precision"})
print(out.schema)
print(out["precision"].to_list())
```

## Numbers and booleans written as strings: `coerce_strings`

With `coerce_strings=True`, a string in a numeric or boolean field is parsed into that
type (`"42"` becomes `42`). Strings that don't parse become null.

```python exec="on" source="above" result="text"
import polars as pl
import polars_genson

df = pl.DataFrame({"j": ['{"x": 1}', '{"x": "42"}']})
print(df.genson.normalise_json("j", coerce_strings=True))
```

## Current limitation: different scalar types in one field

When a field holds different kinds of scalar, such as numbers in some rows and strings in
others, genson keeps the first type in its precedence order (boolean, then integer,
then number, then string), and values of the other types become null:

```python exec="on" source="above" result="text"
import polars as pl
import polars_genson

df = pl.DataFrame({"j": ['{"x": 1}', '{"x": "a"}']})
print(df.genson.normalise_json("j"))
```

If a field can hold numbers written as strings, `coerce_strings=True` recovers them.
Otherwise, check such fields in the raw JSON before normalising.
