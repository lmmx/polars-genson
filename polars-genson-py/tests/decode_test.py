# tests/decode_test.py
"""Tests for normalise_json with decode=True (Avro → Polars schema + dtype decode)."""

import polars as pl
import polars_genson as pg
from pytest import mark, raises


def test_decode_basic_record_schema_and_values():
    """Verify decoding of a simple record into Polars Schema and row values."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"id": 1, "active": true}',
                '{"id": 2, "active": false}',
            ]
        }
    )
    out = df.genson.normalise_json("json_data", decode=True)

    # Schema should reflect inferred types
    assert out.schema == {"id": pl.Int64, "active": pl.Boolean}

    # Values should decode correctly
    assert out.to_dicts() == [
        {"id": 1, "active": True},
        {"id": 2, "active": False},
    ]


def test_decode_map_to_kv_struct():
    """Decode a map field into a list of {key,value} structs (default kv encoding)."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"labels": {"en": "Hello", "fr": "Bonjour"}}',
                '{"labels": {"es": "Hola"}}',
            ]
        }
    )
    out = df.genson.normalise_json("json_data", decode=True, map_threshold=2)

    # Schema should encode map as list of {key,value}
    assert out.schema == {
        "labels": pl.List(pl.Struct({"key": pl.String, "value": pl.String}))
    }

    # Values should decode into proper list-of-structs
    assert out.to_dicts() == [
        {
            "labels": [
                {"key": "en", "value": "Hello"},
                {"key": "fr", "value": "Bonjour"},
            ]
        },
        {"labels": [{"key": "es", "value": "Hola"}]},
    ]


def test_decode_with_empty_as_null_disabled():
    """Ensure empty collections remain instead of becoming null when empty_as_null=False."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"tags": []}',
                '{"tags": ["a", "b"]}',
            ]
        }
    )
    out = df.genson.normalise_json("json_data", decode=True, empty_as_null=False)

    assert out.schema == {"tags": pl.List(pl.String)}
    assert out.to_dicts() == [
        {"tags": []},
        {"tags": ["a", "b"]},
    ]


@mark.skip(reason="Not implemented string coercion for infer polars schema")
def test_decode_with_coerce_strings_enabled():
    """Ensure numeric-like strings are coerced into numbers when coerce_strings=True."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"id": "123"}',
                '{"id": "456"}',
            ]
        }
    )
    out = df.genson.normalise_json("json_data", decode=True, coerce_strings=True)

    assert out.schema == {"id": pl.Int64}
    assert out.to_dicts() == [{"id": 123}, {"id": 456}]


@mark.skip(reason="Not implemented non-kv encodings for infer polars schema")
def test_decode_map_encoding_mapping():
    """Decode JSON objects as plain Polars Structs when map_encoding='mapping'."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"labels": {"en": "Hello", "fr": "Bonjour"}}',
            ]
        }
    )
    out = df.genson.normalise_json("json_data", decode=True, map_encoding="mapping")

    assert out.schema == {"labels": pl.Struct({"en": pl.String, "fr": pl.String})}
    assert out.to_dicts() == [{"labels": {"en": "Hello", "fr": "Bonjour"}}]


@mark.skip(reason="Not implemented non-kv encodings for infer polars schema")
def test_decode_map_encoding_entries():
    """Decode JSON objects as a list of single-entry Structs when map_encoding='entries'."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"labels": {"en": "Hello", "es": "Hola"}}',
            ]
        }
    )
    out = df.genson.normalise_json("json_data", decode=True, map_encoding="entries")

    assert out.schema == {
        "labels": pl.List(pl.Struct({"en": pl.String, "es": pl.String}))
    }
    # Each entry is a separate struct, one field filled
    row = out.to_dicts()[0]
    assert isinstance(row["labels"], list)
    assert {"en": "Hello"} in row["labels"]
    assert {"es": "Hola"} in row["labels"]


def test_decode_with_wrap_root_string():
    """Ensure wrap_root='<field>' nests JSON under that field before decoding."""
    df = pl.DataFrame(
        {"json_data": ['{"id": 1, "name": "Alice"}', '{"id": 2, "name": "Bob"}']}
    )
    out = df.genson.normalise_json("json_data", decode=True, wrap_root="payload")

    assert out.schema == {"payload": pl.Struct({"id": pl.Int64, "name": pl.String})}
    assert out.to_dicts() == [
        {"payload": {"id": 1, "name": "Alice"}},
        {"payload": {"id": 2, "name": "Bob"}},
    ]


def test_decode_with_wrap_root_true_uses_column_name():
    """Ensure wrap_root=True wraps using the column name as field name."""
    df = pl.DataFrame({"json_data": ['{"x": 1}', '{"x": 2}']})
    out = df.genson.normalise_json("json_data", decode=True, wrap_root=True)

    assert out.schema == {"json_data": pl.Struct({"x": pl.Int64})}
    assert out.to_dicts() == [{"json_data": {"x": 1}}, {"json_data": {"x": 2}}]


def test_decode_with_explicit_schema():
    """Verify that passing a schema to decode skips inference and uses dtype directly."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"id": 1, "name": "Alice"}',
                '{"id": 2, "name": "Bob"}',
            ]
        }
    )

    schema = pl.Struct({"id": pl.Int64, "name": pl.String})

    out = df.genson.normalise_json("json_data", decode=schema)

    # Should match the schema exactly
    assert out.schema == pl.Schema(schema)

    # Values should decode according to the provided schema
    assert out.to_dicts() == [
        {"id": 1, "name": "Alice"},
        {"id": 2, "name": "Bob"},
    ]


def test_decode_with_invalid_schema():
    """Verify that passing a schema to decode skips inference and uses dtype directly."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"id": 1, "name": "Alice"}',
                '{"id": 2, "name": "Bob"}',
            ]
        }
    )

    schema = pl.Struct({"id": pl.Int64, "name": pl.Null})

    with raises(pl.exceptions.ComputeError):
        out = df.genson.normalise_json("json_data", decode=schema)
