# tests/avro_test.py
"""Tests for Avro schema output via genson-core integration."""

import polars as pl


def test_simple_object_to_avro():
    """Basic object should become an Avro record with fields."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"name": "Alice", "age": 30}',
                '{"name": "Bob", "age": 25}',
            ]
        }
    )

    avro_schema = df.genson.infer_json_schema("json_data", avro=True)

    # Top-level record
    assert avro_schema["type"] == "record"
    assert avro_schema["name"] == "document"

    # Fields should include "name" and "age"
    field_names = {f["name"] for f in avro_schema["fields"]}
    assert "name" in field_names
    assert "age" in field_names


def test_map_field_to_avro():
    """Map-like object should become an Avro map, not a record."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"labels": {"en": "Hello", "fr": "Bonjour"}}',
                '{"labels": {"de": "Hallo", "es": "Hola"}}',
            ]
        }
    )

    # Low threshold → force detection as map
    avro_schema = df.genson.infer_json_schema("json_data", avro=True, map_threshold=2)

    # Grab "labels" field
    labels_field = next(f for f in avro_schema["fields"] if f["name"] == "labels")
    assert labels_field["type"]["type"] == "map"
    assert labels_field["type"]["values"] == "string"
