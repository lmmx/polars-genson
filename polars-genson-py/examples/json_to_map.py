"""From JSON to `pl.Map`: objects whose keys are data become Polars maps.

Run with `uv run json_to_map.py`.
"""

import io

import polars as pl
import polars_genson  # noqa: F401  (registers the .genson namespace)

# Wikidata-style entities: each has labels keyed by language, and the languages vary
rows = [
    '{"id": "Q42", "labels": {"en": "Douglas Adams", "fr": "Douglas Adams", "ja": "ダグラス・アダムズ"}}',
    '{"id": "Q64", "labels": {"en": "Berlin", "de": "Berlin", "pl": "Berlin"}}',
    '{"id": "Q90", "labels": {"en": "Paris", "es": "París", "zh": "巴黎"}}',
]

# Read by Polars alone, `labels` is a struct with a field for every language in any
# row, null wherever a row lacks it
plain = pl.read_ndjson(io.StringIO("\n".join(rows)))
print(plain["labels"].struct.unnest())

df = pl.DataFrame({"json": rows})

# genson infers `labels` as a map (its keys are data), so each row holds only its own
# languages. A map needs more than `map_threshold` (default 20) distinct keys, so with
# only a few rows here, it's named as one
entities = df.genson.normalise_json("json", force_field_types={"labels": "map"})
print(entities.schema)
for row in entities.iter_rows(named=True):
    print(row)

# Polars' `map` namespace works on it
labels = pl.col("labels")
print(
    entities.select(
        "id",
        labels.map.get("en").alias("en"),
        labels.map.len().alias("languages"),
        labels.map.contains_key("de").alias("has_de"),
    )
)
