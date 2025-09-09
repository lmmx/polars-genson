"""Test that union reordering makes schema inference invariant to data order."""

import polars as pl
import polars_genson  # ignore: F401


def test_union_reordering_gives_stable_schema():
    """Unions should be stable and ordered canonically regardless of data order."""
    df_a = pl.DataFrame(
        {
            "json_data": [
                '{"field": null}',
                '{"field": "hello"}',
                '{"field": 42}',
            ]
        }
    )
    df_b = pl.DataFrame(
        {
            "json_data": [
                '{"field": 42}',
                '{"field": "hello"}',
                '{"field": null}',
            ]
        }
    )

    schema_a = df_a.genson.infer_json_schema("json_data")
    schema_b = df_b.genson.infer_json_schema("json_data")

    # Schema must be identical regardless of input order
    assert schema_a == schema_b

    # Union order should be canonical: null, integer, string
    union = schema_a["properties"]["field"]["type"]
    assert union == ["null", "integer", "string"]
