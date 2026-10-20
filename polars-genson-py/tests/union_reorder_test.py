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

    # By default the string and the integer each keep a field of their own, in canonical
    # order; without wrap_scalars the field stays their union
    for wrap_scalars in (True, False):
        schema_a = df_a.genson.infer_json_schema("json_data", wrap_scalars=wrap_scalars)
        schema_b = df_b.genson.infer_json_schema("json_data", wrap_scalars=wrap_scalars)

        # Schema must be identical regardless of input order
        assert schema_a == schema_b

        field = schema_a["properties"]["field"]
        if wrap_scalars:
            assert list(field["properties"]) == ["field__integer", "field__string"]
        else:
            # Union order should be canonical: null, integer, string
            assert field["type"] == ["null", "integer", "string"]
