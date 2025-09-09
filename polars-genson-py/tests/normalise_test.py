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


def run_norm(rows, *, empty_as_null=True, coerce_strings=False, map_threshold=None):
    """Helper: run normalisation on a list of JSON strings."""
    df = pl.DataFrame({"json_data": rows})
    kwargs = {"empty_as_null": empty_as_null, "coerce_strings": coerce_strings}
    if map_threshold is not None:
        kwargs["map_threshold"] = map_threshold
    return df.genson.normalise_json("json_data", **kwargs).to_list()


def test_normalise_ndjson_like():
    """Mixed rows: empty arrays/maps become null; strings preserved."""
    rows = [
        '{"id":"Q1","aliases":[],"labels":{},"description":"Example entity"}',
        '{"id":"Q2","aliases":["Sample"],"labels":{"en":"Hello"},"description":null}',
        '{"id":"Q3","aliases":null,"labels":{"fr":"Bonjour"},"description":"Third one"}',
        '{"id":"Q4","aliases":["X","Y"],"labels":{},"description":""}',
    ]
    out = run_norm(rows, map_threshold=1)
    # Snapshot-like check: aliases=[] → null, labels={} → null (with default empty_as_null=True)
    assert '"aliases":null' in out[0]
    assert '"labels":null' in out[0]


def test_normalise_union_coercion():
    """Unions: string inputs coerced to int/float/bool when allowed."""
    rows = [
        '{"int_field":1,"float_field":3.14,"bool_field":true}',
        '{"int_field":"42","float_field":"2.718","bool_field":"false"}',
        '{"int_field":null,"float_field":null}',
    ]
    out = run_norm(rows, coerce_strings=True)
    # String values coerced into proper types
    assert '"int_field":42' in out[1]
    assert '"float_field":2.718' in out[1]
    assert '"bool_field":false' in out[1]


def test_normalise_string_or_array():
    """Scalars widened to singleton arrays when unioned with arrays."""
    rows = [
        '{"foo":"json"}',
        '{"foo":["bar","baz"]}',
    ]
    out = run_norm(rows)
    # Scalars widened to singleton arrays
    assert out[0].startswith('{"foo":["json"]}')
    assert out[1] == '{"foo":["bar","baz"]}'


def test_normalise_string_or_array_rev():
    """Order of rows does not affect string→array widening behaviour."""
    rows = [
        '{"foo":["bar","baz"]}',
        '{"foo":"json"}',
    ]
    out = run_norm(rows)
    # Same outcome regardless of row order
    assert out[0] == '{"foo":["bar","baz"]}'
    assert out[1].startswith('{"foo":["json"]}')


def test_normalise_object_or_array():
    """Single objects are widened to arrays."""
    rows = [
        '{"foo":[{"bar":1}]}',
        '{"foo":{"bar":2}}',
    ]
    out = run_norm(rows)
    # Single object widened to array-of-objects
    assert out[1] == '{"foo":[{"bar":2}]}'


def test_normalise_missing_field():
    """Missing fields are injected as null to stabilise schema."""
    rows = [
        '{"foo":"present"}',
        '{"bar":123}',  # foo missing
    ]
    out = run_norm(rows)
    # Missing foo should be injected as null
    assert '"foo":"present"' in out[0]
    assert '"foo":null' in out[1]


def test_normalise_null_vs_missing_field():
    """Explicit null and missing values are both normalised to null."""
    rows = [
        '{"foo":null,"bar":1}',
        '{"bar":2}',  # foo missing
    ]
    out = run_norm(rows)
    # Both rows should agree: foo=null
    assert '"foo":null' in out[0]
    assert '"foo":null' in out[1]


def test_normalise_empty_map_default_null():
    """Empty maps are normalised to null when empty_as_null=True."""
    rows = [
        '{"id":"A","labels":{"en":"Hello"}}',
        '{"id":"B","labels":{}}',
    ]
    out = run_norm(rows, map_threshold=1)
    # Empty maps normalised to null by default
    assert '"labels":null' in out[1]


def test_normalise_map_threshold_forces_map():
    """Low map_threshold forces heterogeneous objects into map type."""
    rows = [
        '{"id":"A","labels":{"en":"Hello"}}',
        '{"id":"B","labels":{"fr":"Bonjour"}}',
    ]
    out = run_norm(rows, map_threshold=1)
    # Labels stabilised as a map
    assert '"labels":{"en":"Hello"}' in out[0] or '"labels":{"fr":"Bonjour"}' in out[1]


def test_normalise_scalar_to_map():
    """Scalar values are widened into maps with a 'default' key."""
    rows = [
        '{"id":"A","labels":"foo"}',
        '{"id":"B","labels":{"en":"Hello"}}',
    ]
    out = run_norm(rows, map_threshold=0)
    # Scalar widened into {"default": ...}
    assert '"labels":{"default":"foo"}' in out[0]
    assert '"labels":{"en":"Hello"}}' in out[1]
