# tests/normalise_test.py
"""Tests for JSON normalisation via genson-core integration."""

import orjson
import polars as pl
import polars_genson


def test_empty_array_becomes_null_by_default():
    """Empty arrays should become null unless keep_empty is requested."""
    df = pl.DataFrame({"json_data": ['{"labels": []}']})

    out = df.genson.normalise_json("json_data", decode=False).to_list()

    assert out == ['{"labels":null}']


def test_keep_empty_preserves_arrays():
    """With empty_as_null disabled, empty arrays should be preserved."""
    df = pl.DataFrame({"json_data": ['{"labels": []}']})

    out = df.genson.normalise_json(
        "json_data", empty_as_null=False, decode=False
    ).to_list()

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

    out = df.genson.normalise_json("json_data", decode=False).to_list()

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

    out = df.genson.normalise_json(
        "json_data", coerce_strings=True, decode=False
    ).to_list()

    # String "42" is coerced to int, "true" coerced to bool
    assert '"id":42' in out[0]
    assert '"active":true' in out[0]


def run_norm(
    rows,
    *,
    empty_as_null=True,
    coerce_strings=False,
    map_threshold=None,
    map_encoding=None,
):
    """Helper: run normalisation on a list of JSON strings."""
    df = pl.DataFrame({"json_data": rows})
    kwargs = {"empty_as_null": empty_as_null, "coerce_strings": coerce_strings}
    if map_threshold is not None:
        kwargs["map_threshold"] = map_threshold
    if map_encoding is not None:
        kwargs["map_encoding"] = map_encoding
    return df.genson.normalise_json("json_data", **kwargs, decode=False).to_list()


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


def test_normalise_map_threshold_forces_map_mapping():
    """Low map_threshold forces heterogeneous objects into map type."""
    rows = [
        '{"id":"A","labels":{"en":"Hello"}}',
        '{"id":"B","labels":{"fr":"Bonjour"}}',
    ]
    out = run_norm(rows, map_threshold=1, map_encoding="mapping")
    # Labels stabilised as a map
    assert '"labels":{"en":"Hello"}' in out[0] or '"labels":{"fr":"Bonjour"}' in out[1]


def test_normalise_map_threshold_forces_map_kv():
    """Low map_threshold forces heterogeneous objects into map type."""
    rows = [
        '{"id":"A","labels":{"en":"Hello"}}',
        '{"id":"B","labels":{"fr":"Bonjour"}}',
    ]
    out = run_norm(rows, map_threshold=1)
    # Labels stabilised as a map
    assert '"labels":[{"key":"en","value":"Hello"}]' in out[0]
    assert '"labels":[{"key":"fr","value":"Bonjour"}]' in out[1]


def test_normalise_scalar_to_map_kv():
    """Scalar values are widened into maps with a 'default' key."""
    rows = [
        '{"id":"A","labels":"foo"}',
        '{"id":"B","labels":{"en":"Hello"}}',
    ]
    out = run_norm(rows, map_threshold=0)
    # Scalar widened into {"default": ...}
    assert out == [
        '{"id":"A","labels":[{"key":"labels__string","value":"foo"}]}',
        '{"id":"B","labels":[{"key":"en","value":"Hello"}]}',
    ]


def test_normalise_scalar_to_map_mapping():
    """Scalar values are widened into maps with a 'default' key."""
    rows = [
        '{"id":"A","labels":"foo"}',
        '{"id":"B","labels":{"en":"Hello"}}',
    ]
    out = run_norm(rows, map_threshold=0, map_encoding="mapping")
    # Scalar widened into {"default": ...}
    assert '"labels":{"labels__string":"foo"}' in out[0]
    assert '"labels":{"en":"Hello"}}' in out[1]


def test_normalise_record_expands_to_struct():
    """Records ('labels' field) with partial/empty keys should always get null keys."""
    rows = [
        '{"id": "123", "tags": [], "labels": {}, "active": "true"}',
        '{"id": 456, "tags": ["x","y"], "labels": {"en":"Hello"}, "active": false}',
        '{"id": null, "labels": {"es": "Hola", "fr":"Bonjour"}}',
    ]

    df = pl.DataFrame({"json_data": rows})
    out_strs = df.genson.normalise_json("json_data", decode=False).to_list()
    out = [orjson.loads(out_st) for out_st in out_strs]

    # Full output snapshot, not partial assertion
    assert out == [
        {
            "id": None,
            "tags": None,
            "labels": {"es": None, "fr": None, "en": None},
            "active": None,
        },
        {
            "id": 456,
            "tags": ["x", "y"],
            "labels": {"es": None, "fr": None, "en": "Hello"},
            "active": False,
        },
        {
            "id": None,
            "tags": None,
            "labels": {"es": "Hola", "fr": "Bonjour", "en": None},
            "active": None,
        },
    ]


def test_normalise_map_currently_expands_to_struct():
    """Maps with different keys should NOT explode to struct with nulls.

    This test locks in current behaviour until the bug is fixed.
    """
    rows = [
        '{"id": "123", "tags": [], "labels": {}, "active": "true"}',
        '{"id": 456, "tags": ["x","y"], "labels": {"en":"Hello"}, "active": false}',
        '{"id": null, "labels": {"es": "Hola", "fr":"Bonjour"}}',
    ]

    df = pl.DataFrame({"json_data": rows})
    out = df.genson.normalise_json("json_data", map_threshold=1).to_dicts()

    # Full output snapshot, not partial assertion
    assert out == [
        {"id": None, "tags": None, "labels": None, "active": None},
        {
            "id": 456,
            "tags": ["x", "y"],
            "labels": [
                {"key": "en", "value": "Hello"},
            ],
            "active": False,
        },
        {
            "id": None,
            "tags": None,
            "labels": [
                {"key": "es", "value": "Hola"},
                {"key": "fr", "value": "Bonjour"},
            ],
            "active": None,
        },
    ]


def test_normalise_map_readme_demo():
    """README demo: ids int, tags empty/missing→null, labels as list-of-key/value."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"id": 123, "tags": [], "labels": {}, "active": true}',
                '{"id": 456, "tags": ["x","y"], "labels": {"fr":"Bonjour"}, "active": false}',
                '{"id": 789, "labels": {"en": "Hi", "es": "Hola"}}',
            ]
        }
    )

    out = df.genson.normalise_json("json_data", map_threshold=0).to_dicts()

    assert out == [
        {"id": 123, "tags": None, "labels": None, "active": True},
        {
            "id": 456,
            "tags": ["x", "y"],
            "labels": [{"key": "fr", "value": "Bonjour"}],
            "active": False,
        },
        {
            "id": 789,
            "tags": None,
            "labels": [
                {"key": "en", "value": "Hi"},
                {"key": "es", "value": "Hola"},
            ],
            "active": None,
        },
    ]


def test_normalise_with_wrap_root_string():
    """Normalisation should respect wrap_root='<field>' by nesting input."""
    df = pl.DataFrame({"json_data": ['{"foo": "bar"}']})
    out = df.genson.normalise_json(
        "json_data", decode=False, wrap_root="root"
    ).to_list()

    assert out == ['{"root":{"foo":"bar"}}']


def test_normalise_with_wrap_root_true_uses_column_name():
    """Normalisation with wrap_root=True uses the column name."""
    df = pl.DataFrame({"json_data": ['{"foo": "bar"}']})
    out = df.genson.normalise_json("json_data", decode=False, wrap_root=True).to_list()

    assert out == ['{"json_data":{"foo":"bar"}}']
