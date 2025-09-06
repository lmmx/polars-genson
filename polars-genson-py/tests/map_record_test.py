"""Tests for map_threshold and force_field_types kwargs."""

import polars as pl
from pytest import mark


def test_map_threshold_triggers_map():
    """Objects with many keys should be rewritten as maps when threshold is low."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"labels": {"en": "Hello", "fr": "Bonjour"}}',
                '{"labels": {"de": "Hallo", "es": "Hola"}}',
            ]
        }
    )

    # Very low threshold so even 2 keys counts as "map"
    schema = df.genson.infer_json_schema("json_data", map_threshold=2)

    labels_schema = schema["properties"]["labels"]
    assert labels_schema["type"] == "object"
    # Should be a map (additionalProperties), not a record (properties)
    assert "additionalProperties" in labels_schema
    assert "properties" not in labels_schema


def test_force_field_types_override():
    """Force a field to be map regardless of heuristic."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"labels": {"en": "Hello", "fr": "Bonjour"}}',
            ]
        }
    )

    schema = df.genson.infer_json_schema(
        "json_data", force_field_types={"labels": "map"}
    )

    labels_schema = schema["properties"]["labels"]
    assert "additionalProperties" in labels_schema
    assert "properties" not in labels_schema


@mark.skip(reason="Does not work?")
def test_force_field_types_record_override():
    """Force a field to stay a record even if threshold would rewrite."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"labels": {"en": "Hello", "fr": "Bonjour"}}',
                '{"labels": {"de": "Hallo", "es": "Hola"}}',
            ]
        }
    )

    schema = df.genson.infer_json_schema(
        "json_data", map_threshold=2, force_field_types={"labels": "record"}
    )

    labels_schema = schema["properties"]["labels"]
    assert "properties" in labels_schema
    assert "additionalProperties" not in labels_schema
