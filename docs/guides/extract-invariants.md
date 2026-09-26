# Factor out repeated embedded data

JSON documents often embed a full copy of the things they reference. An order carries
its customer's details, and a Wikidata claim carries every language's label for the
property and entity it points to. Such a field is **invariant per key**: every object
with the same key carries the same value, so the key (its **determinant**) fixes it.

Every copy after the first is redundant, and genson would otherwise infer a schema for
it, normalise it and write it out again for every row. `extract_invariants` on
`normalise_from_parquet` removes these fields from the rows before inference and writes
each distinct value once to a lookup table.

```python exec="on" session="invariants"
import os, tempfile
os.chdir(tempfile.mkdtemp())  # keep the example files out of the way
```

## Example: orders that embed their customer

```python exec="on" source="above" session="invariants"
import polars as pl

pl.DataFrame({"order": [
    '{"order": 1, "customer_id": 7, "customer": {"name": "Ada", "city": "London"}}',
    '{"order": 2, "customer_id": 8, "customer": {"name": "Bo", "city": "Oslo"}}',
    '{"order": 3, "customer_id": 7, "customer": {"name": "Ada", "city": "London"}}',
]}).write_parquet("orders.parquet")
```

`customer` is determined by `customer_id`, so extract it:

```python exec="on" source="above" result="text" session="invariants"
from polars_genson import normalise_from_parquet

normalise_from_parquet(
    "orders.parquet",
    column="order",
    output_path="slim.parquet",
    typed=True,
    extract_invariants={"customer": "customer_id"},
    lookup_output_path="customers.parquet",
)
print(pl.read_parquet("slim.parquet").unnest("order"))
print(pl.read_parquet("customers.parquet"))
```

The rows keep `customer_id` and lose `customer`. The lookup table has one row per
distinct customer, with string columns `field` (which extracted field), `key` (the
determinant's value) and `value` (the field as JSON), in the order first seen.

## Join the values back

The `value` column is JSON, so decode it with the dtype you want, then join on the key:

```python exec="on" source="above" result="text" session="invariants"
customers = pl.read_parquet("customers.parquet").select(
    pl.col("key").cast(pl.Int64).alias("customer_id"),
    pl.col("value").str.json_decode(pl.Struct({"name": pl.String, "city": pl.String})),
).unnest("value")
orders = pl.read_parquet("slim.parquet").unnest("order")
print(orders.join(customers, on="customer_id", how="left"))
```

The lookup key is always a string, so cast it to the determinant's type before joining.

## Several fields at once

Pass one entry per field. Each field is found wherever it sits next to its determinant,
at any depth, and the `field` column tells the lookup rows apart:

```python
extract_invariants={"labels": "id", "property-labels": "property", "unit-labels": "unit"}
```

## A field that isn't invariant is an error

If one determinant value turns up with two different values of the field, extracting it
would lose one of them, so genson raises an error instead:

```python exec="on" source="above" result="text" session="invariants"
pl.DataFrame({"order": [
    '{"customer_id": 7, "customer": {"name": "Ada"}}',
    '{"customer_id": 7, "customer": {"name": "Ada Lovelace"}}',
]}).write_parquet("conflict.parquet")

try:
    normalise_from_parquet(
        "conflict.parquet", column="order", output_path="out.parquet", typed=True,
        extract_invariants={"customer": "customer_id"}, lookup_output_path="lookup.parquet",
    )
except ValueError as e:
    print(e)
```

## When it's worth it

The more often each key repeats, and the larger its value, the more work extraction
saves. On a 615 MB chunk of Wikidata claims, where each entity's multilingual label maps
are embedded in every claim that mentions it, extraction took normalisation from 22 s to
4.2 s and peak memory from 9.6 GB to 4.2 GB.
