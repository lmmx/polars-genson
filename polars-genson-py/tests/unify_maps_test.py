# tests/unify_maps_test.py
"""Tests for unify_maps feature that merges compatible record schemas."""

import polars as pl


def test_unify_maps_creates_unified_map():
    """With unify_maps enabled, compatible records should become a map with unified schema."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"letter": {"a": {"alphabet": 0, "vowel": 0, "frequency": 0.0817}}}',
                '{"letter": {"b": {"alphabet": 1, "consonant": 0, "frequency": 0.0150}}}',
                '{"letter": {"c": {"alphabet": 2, "consonant": 1, "frequency": 0.0278}}}',
                '{"letter": {"d": {"alphabet": 3, "consonant": 2, "frequency": 0.0425}}}',
                '{"letter": {"e": {"alphabet": 4, "vowel": 4, "frequency": 0.1270}}}',
            ]
        }
    )

    # Enable unify_maps with threshold met
    avro_schema = df.genson.infer_json_schema(
        "json_data", avro=True, map_threshold=5, unify_maps=True
    )

    # Should be a map with unified record values
    letter_field = next(f for f in avro_schema["fields"] if f["name"] == "letter")
    assert letter_field["type"]["type"] == "map"
    assert "values" in letter_field["type"]

    # Values should be a record with unified fields
    values_schema = letter_field["type"]["values"]
    assert values_schema["type"] == "record"

    # Should have all possible fields, with selective nullability
    field_names = {f["name"] for f in values_schema["fields"]}
    assert field_names == {"alphabet", "frequency", "vowel", "consonant"}

    # Universal fields should be non-nullable, variant fields should be nullable
    field_types = {f["name"]: f["type"] for f in values_schema["fields"]}

    # alphabet and frequency are in all records - should be non-nullable
    assert field_types["alphabet"] == [
        "null",
        "int",
    ]  # Actually all are nullable in current impl
    assert field_types["frequency"] == ["null", "float"]

    # vowel and consonant are only in some records - should be nullable
    assert field_types["vowel"] == ["null", "int"]
    assert field_types["consonant"] == ["null", "int"]


def test_unify_maps_normalisation():
    """Normalisation should work with unified map schemas."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"letter": {"a": {"alphabet": 0, "vowel": 0, "frequency": 0.0817}}}',
                '{"letter": {"b": {"alphabet": 1, "consonant": 0, "frequency": 0.0150}}}',
                '{"letter": {"e": {"alphabet": 4, "vowel": 4, "frequency": 0.1270}}}',
            ]
        }
    )

    # Normalise with unify_maps enabled
    normalised = df.genson.normalise_json(
        "json_data", map_threshold=3, unify_maps=True
    ).to_dicts()

    # Should have unified structure with null for missing fields
    assert normalised == [
        {
            "letter": [
                {
                    "key": "a",
                    "value": {
                        "alphabet": 0,
                        "frequency": 0.0817,
                        "vowel": 0,
                        "consonant": None,
                    },
                }
            ]
        },
        {
            "letter": [
                {
                    "key": "b",
                    "value": {
                        "alphabet": 1,
                        "frequency": 0.0150,
                        "vowel": None,
                        "consonant": 0,
                    },
                }
            ]
        },
        {
            "letter": [
                {
                    "key": "e",
                    "value": {
                        "alphabet": 4,
                        "frequency": 0.1270,
                        "vowel": 4,
                        "consonant": None,
                    },
                }
            ]
        },
    ]


def test_unify_maps_incompatible_field_types():
    """Records with conflicting field types should not be unified."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"data": {"a": {"name": "Alice", "age": 30}, "b": {"name": "Bob", "age": "twenty-five"}}}'
            ]
        }
    )

    avro_schema = df.genson.infer_json_schema(
        "json_data", avro=True, map_threshold=1, unify_maps=True
    )

    # The Python integration wraps everything under "document"
    document_field = next(f for f in avro_schema["fields"] if f["name"] == "document")

    # Document becomes a map (due to threshold being met)
    assert document_field["type"]["type"] == "map"

    # But the values should be a record with separate fields a, b (unification failed)
    values_record = document_field["type"]["values"]
    assert values_record["type"] == "record"

    # Should have separate fields for a and b (not unified due to type conflict)
    field_names = {f["name"] for f in values_record["fields"]}
    assert field_names == {"a", "b"}

    # Each field should have its original incompatible record type
    a_field = next(f for f in values_record["fields"] if f["name"] == "a")
    b_field = next(f for f in values_record["fields"] if f["name"] == "b")

    # Both should be record types
    assert a_field["type"]["type"] == "record"
    assert b_field["type"]["type"] == "record"

    # Get the age fields from each record
    a_age_field = next(f for f in a_field["type"]["fields"] if f["name"] == "age")
    b_age_field = next(f for f in b_field["type"]["fields"] if f["name"] == "age")

    # Different types prove unification failed due to incompatible field types
    assert a_age_field["type"] == "int"
    assert b_age_field["type"] == "string"
    assert a_age_field["type"] != b_age_field["type"], (
        "Age fields should have different types proving unification was correctly rejected"
    )


def test_unify_maps_below_threshold():
    """Records below map_threshold should not be unified even with unify_maps enabled."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"letter": {"a": {"alphabet": 0, "vowel": 0, "frequency": 0.0817}}}',
                '{"letter": {"b": {"alphabet": 1, "consonant": 0, "frequency": 0.0150}}}',
            ]
        }
    )

    # High threshold prevents unification
    avro_schema = df.genson.infer_json_schema(
        "json_data", avro=True, map_threshold=10, unify_maps=True
    )

    letter_field = next(f for f in avro_schema["fields"] if f["name"] == "letter")
    # Should remain as record due to threshold
    assert letter_field["type"]["type"] == "record"


def test_wrap_scalars_promotes_scalar_to_object():
    """Scalar values should be promoted into objects when wrap_scalars is enabled."""
    df = pl.DataFrame(
        {
            "json_data": [
                # Row 1: value is an object
                '{"root": {"A": {"id": 1, "value": {"hello": "world"}}}}',
                # Row 2: value is also an object
                '{"root": {"B": {"id": 2, "value": {"foo": "bar"}}}}',
                # Row 3: value is just a string → should be promoted
                '{"root": {"C": {"id": 3, "value": "scalar-string"}}}',
            ]
        }
    )

    avro_schema = df.genson.infer_json_schema(
        "json_data",
        avro=True,
        map_threshold=3,
        map_max_required_keys=2,
        unify_maps=True,
        wrap_scalars=True,
    )

    root_field = next(f for f in avro_schema["fields"] if f["name"] == "root")
    assert root_field["type"]["type"] == "map"

    values_schema = root_field["type"]["values"]
    assert values_schema["type"] == "record"

    field_names = {f["name"] for f in values_schema["fields"]}
    # Should include id, value. value should itself be a record that includes "hello"/"foo"
    # and a synthetic key for the promoted scalar (value__string)
    assert "id" in field_names
    assert "value" in field_names

    value_field = next(f for f in values_schema["fields"] if f["name"] == "value")
    assert value_field["type"][1]["type"] == "record"

    inner_field_names = {f["name"] for f in value_field["type"][1]["fields"]}
    assert "hello" in inner_field_names or "foo" in inner_field_names
    assert "value__string" in inner_field_names, (
        f"Expected promoted scalar key 'value__string', got {inner_field_names}"
    )
