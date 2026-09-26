# Field order

genson keeps fields in the order they first appear in the JSON. The order is part of the
output: it's the order of the columns and of the fields inside each struct.

## Within one object: document order

An object's fields keep the order of its keys, however many keys it has:

```python exec="on" source="above" result="text"
import polars as pl
import polars_genson

keys = ["zeta", "alpha", "mu", "beta"]
row = "{" + ", ".join(f'"{k}": 1' for k in keys) + "}"
print(pl.DataFrame({"j": [row]}).genson.infer_polars_schema("j"))
```

## Across rows: first-seen order

When rows have different keys, a field's position is where it was *first* seen, reading
the rows in order:

```python exec="on" source="above" result="text"
import polars as pl
import polars_genson

df = pl.DataFrame({"j": ['{"b": 1}', '{"a": 1, "b": 2}', '{"c": 3, "a": 4}']})
print(df.genson.infer_polars_schema("j"))
```

`b` comes first because the first row has it; `a` appears in the second row and `c` in
the third. The same holds for the fields of merged records, such as the values of a map
under `unify_maps` (see [Maps and records](maps-and-records.md)).

## Determinism

The same input always gives the same field order, on every run. (Before 0.9.0, objects
with more than 32 keys could come out in a different order on each run.)

## Combining output from several inputs

Because the order depends on which rows are seen first, two inputs with the same fields
can still produce different field orders: another file may simply list its keys or rows
in a different order. When you combine output from several inputs, put their fields
into one fixed order first, for example the order of a schema you keep for the dataset,
so the combined columns and structs line up.
