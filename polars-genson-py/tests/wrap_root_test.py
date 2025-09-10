# tests/test_wrap_root.py
"""Tests for wrap_root behavior in schema inference."""

import polars as pl


def test_no_wrap_root():
    """If wrap_root is disabled, schema should directly infer the inner object."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"a": 10, "b": 20}',
                '{"a": 30, "b": 40}',
            ]
        }
    )

    avro_schema = df.genson.infer_json_schema("json_data", avro=True, wrap_root=None)
    assert avro_schema == {
        "type": "record",
        "name": "document",
        "namespace": "genson",
        "fields": [{"name": "a", "type": "int"}, {"name": "b", "type": "int"}],
    }


def test_wrap_root_defaults_to_column_name():
    """If wrap_root=True, the schema should be wrapped under the column name."""
    df = pl.DataFrame(
        {
            "payload": [
                '{"x": 1, "y": 2}',
                '{"x": 3, "y": 4}',
            ]
        }
    )

    avro_schema = df.genson.infer_json_schema("payload", avro=True, wrap_root=True)
    assert avro_schema == {
        "type": "record",
        "name": "document",
        "namespace": "genson",
        "fields": [
            {
                "name": "payload",
                "type": {
                    "type": "record",
                    "name": "payload",
                    "namespace": "genson.document_types",
                    "fields": [
                        {"name": "x", "type": "int"},
                        {"name": "y", "type": "int"},
                    ],
                },
            }
        ],
    }


def test_wrap_root_custom_name():
    """If wrap_root is a string, it should override the root record name."""
    df = pl.DataFrame(
        {
            "data": [
                '{"foo": "bar"}',
                '{"foo": "baz"}',
            ]
        }
    )

    avro_schema = df.genson.infer_json_schema("data", avro=True, wrap_root="RootDoc")
    assert avro_schema == {
        "type": "record",
        "name": "document",
        "namespace": "genson",
        "fields": [
            {
                "name": "RootDoc",
                "type": {
                    "type": "record",
                    "name": "RootDoc",
                    "namespace": "genson.document_types",
                    "fields": [{"name": "foo", "type": "string"}],
                },
            }
        ],
    }
