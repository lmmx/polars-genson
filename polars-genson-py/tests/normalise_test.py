# tests/normalise_test.py
"""Tests for JSON normalisation via genson-core integration."""

import polars as pl


def test_empty_array_becomes_null_by_default():
    """Empty arrays should become null unless keep_empty is requested."""
    df = pl.DataFrame({"json_data": ['{"labels": []}']})

    out = df.genson.normalise_json("json_data").to_list()

    assert out == ['{"labels":null}']


def test_keep_empty_preserves_arrays():
    """With empty_as_null disabled, empty arrays should be preserved."""
    df = pl.DataFrame({"json_data": ['{"labels": []}']})

    out = df.genson.normalise_json("json_data", empty_as_null=False).to_list()

    assert out == ['{"labels":[]}']


def test_string_coercion_disabled_by_default():
    """Numeric strings should remain strings unless coercion is enabled."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"id":"42", "active":"true"}',
                '{"id":7, "active":false}',
            ]
        }
    )

    out = df.genson.normalise_json("json_data").to_list()

    # String "42" is not coerced to int, "true" not coerced to bool
    assert '"id":"42"' not in out[0]
    assert '"id":null' in out[0]
    assert '"active":"true"' not in out[0]
    assert '"active":null' in out[0]


def test_string_coercion_enabled():
    """With coerce_strings=True, numeric/boolean strings should be converted."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"id": "42", "active": "true"}',
                '{"id": 7, "active": false}',
            ]
        }
    )

    out = df.genson.normalise_json("json_data", coerce_strings=True).to_list()

    # String "42" is coerced to int, "true" coerced to bool
    assert '"id":42' in out[0]
    assert '"active":true' in out[0]
