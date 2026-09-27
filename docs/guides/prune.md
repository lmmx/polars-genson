# Remove malformed records

Real JSON sometimes holds records that are broken in a recognisable way: an entry that
carries an `error` field instead of its data, or a bare string where every other row has
an object. genson normalises these like any other record, so they reach the output, and
the fields only they have (`error`, a promoted `…__string`) widen the schema for every
row.

`prune` on `normalise_from_parquet` names those fields. Every record holding one is
removed during normalisation and written to a separate file, and the fields are left
out of the output schema.

```python exec="on" session="prune"
import os, tempfile
os.chdir(tempfile.mkdtemp())  # keep the example files out of the way
```

## Example: sensor readings with failures

```python exec="on" source="above" session="prune"
import polars as pl

pl.DataFrame({
    "sensor": ["a", "b", "c"],
    "data": [
        '{"readings": [{"t": 1, "value": 3.2, "calibration": {"offset": 0.1}}, {"t": 2, "error": "timeout"}]}',
        '{"readings": [{"t": 1, "value": 2.8, "calibration": "pending"}]}',
        '{"readings": [{"t": 1, "value": 1.5}]}',
    ],
}).write_parquet("sensors.parquet")
```

Sensor `a` has a failed reading holding `error`. Sensor `b`'s calibration is the string
`"pending"` where the others have an object, so genson promotes it to the field
`calibration__string`. Prune both:

```python exec="on" source="above" result="text" session="prune"
from polars_genson import normalise_from_parquet

normalise_from_parquet(
    "sensors.parquet",
    column="data",
    output_path="clean.parquet",
    typed=True,
    keep_columns=["sensor"],
    prune={"error", "calibration__string"},
    prune_output_path="pruned.parquet",
)
print(pl.read_parquet("clean.parquet").unnest("data"))
print(pl.read_parquet("pruned.parquet"))
```

## What gets removed

- A record is pruned if it holds a non-null value for a named field, at any depth.
- Names are the fields of the inferred schema, so a scalar promoted to a record goes by
  its promoted name, as it appears in the schema (`calibration__string`).
- A pruned record that is a field of another record takes that record with it, since the
  record is incomplete without it. This goes on up to an array element or map entry,
  which is removed on its own. Above, the pending calibration takes its reading with it.
- An array or map left empty by pruning is removed from its array or map, or is null as a
  record field. Sensor `b`'s readings are null for this reason, while the row itself is
  kept. An array or map already empty in the input is normalised as usual.
- The named fields are left out of the output schema.

## The pruned values

Nothing is dropped silently. `prune_output_path` gets one row per pruned value:

- the `keep_columns` of its row, to tell which row it came from;
- `path`: the keys and array indices from the row's root to the value, as a JSON array;
- `value`: the value as it was in the input, as JSON.

`prune` and `prune_output_path` must be given together.

## Finding the names

Infer the schema first and look for fields that only the malformed records have: an
`error` or `message`, or a `…__string` beside a record's usual fields. See
[Mixed types](../concepts/mixed-types.md) for how promoted names are formed.
