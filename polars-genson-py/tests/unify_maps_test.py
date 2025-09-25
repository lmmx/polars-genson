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
    assert field_types["alphabet"] == "int"
    assert field_types["frequency"] == "float"

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


def test_unify_maps_with_scalar_promotion():
    """Records with conflicting field types can now be unified via scalar promotion."""
    df = pl.DataFrame(
        {
            "json_data": [
                '{"data": {"a": {"name": "Alice", "age": 30}, "b": {"name": "Bob", "age": "twenty-five"}}}'
            ]
        }
    )

    avro_schema = df.genson.infer_json_schema(
        "json_data",
        avro=True,
        map_threshold=1,
        unify_maps=True,
        no_root_map=False,
    )

    # The Python integration wraps everything under "document"
    document_field = next(f for f in avro_schema["fields"] if f["name"] == "document")

    # Document becomes a map (due to threshold being met)
    assert document_field["type"]["type"] == "map"

    # Now the values should be unified into a map (unification succeeded due to scalar promotion)
    data_map = document_field["type"]["values"]
    assert data_map["type"] == "map"

    # The unified record should have name (string) and age (promoted scalar)
    unified_record = data_map["values"]
    assert unified_record["type"] == "record"

    field_names = {f["name"] for f in unified_record["fields"]}
    assert field_names == {"name", "age"}

    # Get field types
    name_field = next(f for f in unified_record["fields"] if f["name"] == "name")
    age_field = next(f for f in unified_record["fields"] if f["name"] == "age")

    # name should be simple string
    assert name_field["type"] == "string"

    # age should be promoted scalar record with age__integer and age__string
    assert age_field["type"]["type"] == "record"

    age_field_names = {f["name"] for f in age_field["type"]["fields"]}
    assert age_field_names == {"age__integer", "age__string"}

    # Verify the promoted fields are nullable
    age_int_field = next(
        f for f in age_field["type"]["fields"] if f["name"] == "age__integer"
    )
    age_str_field = next(
        f for f in age_field["type"]["fields"] if f["name"] == "age__string"
    )

    assert age_int_field["type"] == ["null", "int"]
    assert age_str_field["type"] == ["null", "string"]


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


def test_wrap_scalars_promotes_scalar_to_record():
    """Schema inference: scalar values should be promoted into record fields when wrap_scalars is enabled."""
    df = pl.DataFrame(
        {
            "json_data": [
                # Carefully designed to prevent the value field from becoming a map:
                # - 4 rows create 4 letter keys (A,B,C,D) in the letters map (0 required keys)
                # - value field has only 3 distinct properties: hello, foo, value__string (promoted scalar)
                # - 3 properties < map_threshold=4, so value stays as record with scalar promotion
                '{"letters": {"A": {"id": 1, "value": {"hello": "world"}}}}',
                '{"letters": {"B": {"id": 2, "value": {"foo": "bar"}}}}',
                '{"letters": {"C": {"id": 3, "value": {"foo": "baz"}}}}',  # Different foo value to ensure 2 foo keys
                '{"letters": {"D": {"id": 4, "value": "scalar-string"}}}',  # Scalar → promotes to value__string
            ]
        }
    )

    avro_schema = df.genson.infer_json_schema(
        "json_data",
        avro=True,
        map_threshold=4,  # Prevents value field (3 props) from becoming map
        map_max_required_keys=0,  # Allows letters map (0 required) to become map
        unify_maps=True,
        wrap_scalars=True,
    )

    letters_field = next(f for f in avro_schema["fields"] if f["name"] == "letters")
    assert letters_field["type"]["type"] == "map"

    values_schema = letters_field["type"]["values"]
    assert values_schema["type"] == "record"

    field_names = {f["name"] for f in values_schema["fields"]}
    assert "id" in field_names
    assert "value" in field_names

    value_field = next(f for f in values_schema["fields"] if f["name"] == "value")
    assert value_field["type"]["type"] == "record"  # Should be record, not map

    inner_field_names = {f["name"] for f in value_field["type"]["fields"]}
    assert "hello" in inner_field_names
    assert "foo" in inner_field_names
    assert "value__string" in inner_field_names, (
        f"Expected promoted scalar key 'value__string', got {inner_field_names}"
    )


def test_wrap_scalars_promotes_scalar_to_record_normalisation():
    """Normalisation: scalar values should be promoted into record fields when wrap_scalars is enabled."""
    df = pl.DataFrame(
        {
            "json_data": [
                # Carefully designed to prevent the value field from becoming a map:
                # - 4 rows create 4 letter keys (A,B,C,D) in the letters map (0 required keys)
                # - value field has only 3 distinct properties: hello, foo, value__string (promoted scalar)
                # - 3 properties < map_threshold=4, so value stays as record with scalar promotion
                '{"letters": {"A": {"id": 1, "value": {"hello": "world"}}}}',
                '{"letters": {"B": {"id": 2, "value": {"foo": "bar"}}}}',
                '{"letters": {"C": {"id": 3, "value": {"foo": "baz"}}}}',  # Different foo value to ensure 2 foo keys
                '{"letters": {"D": {"id": 4, "value": "scalar-string"}}}',  # Scalar → promotes to value__string
            ]
        }
    )

    # Normalise with wrap_scalars enabled
    normalised = df.genson.normalise_json(
        "json_data",
        map_threshold=4,  # Prevents value field (3 props) from becoming map
        map_max_required_keys=0,  # Allows letters map (0 required) to become map
        unify_maps=True,
        wrap_scalars=True,
    ).to_dicts()

    # Should have unified structure with promoted scalar
    assert normalised == [
        {
            "letters": [
                {
                    "key": "A",
                    "value": {
                        "id": 1,
                        "value": {
                            "hello": "world",
                            "foo": None,
                            "value__string": None,
                        },
                    },
                }
            ]
        },
        {
            "letters": [
                {
                    "key": "B",
                    "value": {
                        "id": 2,
                        "value": {
                            "hello": None,
                            "foo": "bar",
                            "value__string": None,
                        },
                    },
                }
            ]
        },
        {
            "letters": [
                {
                    "key": "C",
                    "value": {
                        "id": 3,
                        "value": {
                            "hello": None,
                            "foo": "baz",
                            "value__string": None,
                        },
                    },
                }
            ]
        },
        {
            "letters": [
                {
                    "key": "D",
                    "value": {
                        "id": 4,
                        "value": {
                            "hello": None,
                            "foo": None,
                            "value__string": "scalar-string",
                        },
                    },
                }
            ]
        },
    ]


def test_wrap_scalars_promotes_scalar_to_map():
    """Schema inference: when value becomes a map, scalar promotion should use consistent naming."""
    df = pl.DataFrame(
        {
            "json_data": [
                # Original 3-row case that triggers value field becoming a map
                # - 3 letters keys (A,B,C) in letters map
                # - value field has 2 object properties + 1 promoted scalar = 3 properties
                # - With map_threshold=3, value field becomes a map (meets threshold exactly)
                '{"letters": {"A": {"id": 1, "value": {"hello": "world"}}}}',
                '{"letters": {"B": {"id": 2, "value": {"foo": "bar"}}}}',
                '{"letters": {"C": {"id": 3, "value": "scalar-string"}}}',
            ]
        }
    )

    avro_schema = df.genson.infer_json_schema(
        "json_data",
        avro=True,
        map_threshold=3,  # value field (3 props) meets threshold → becomes map
        map_max_required_keys=2,  # Allows maps with ≤2 required keys
        unify_maps=True,
        wrap_scalars=True,
    )

    letters_field = next(f for f in avro_schema["fields"] if f["name"] == "letters")
    assert letters_field["type"]["type"] == "map"

    values_schema = letters_field["type"]["values"]
    assert values_schema["type"] == "record"

    field_names = {f["name"] for f in values_schema["fields"]}
    assert "id" in field_names
    assert "value" in field_names

    value_field = next(f for f in values_schema["fields"] if f["name"] == "value")
    assert value_field["type"]["type"] == "map"  # Should be map, not record

    # Map should handle strings (including promoted scalars)
    assert value_field["type"]["values"] == "string"


def test_wrap_scalars_promotes_scalar_to_map_normalisation():
    """Normalisation: when value becomes a map, scalar promotion should use consistent naming."""
    df = pl.DataFrame(
        {
            "json_data": [
                # Original 3-row case that triggers value field becoming a map
                # - 3 letters keys (A,B,C) in letters map
                # - value field has 2 object properties + 1 promoted scalar = 3 properties
                # - With map_threshold=3, value field becomes a map (meets threshold exactly)
                '{"letters": {"A": {"id": 1, "value": {"hello": "world"}}}}',
                '{"letters": {"B": {"id": 2, "value": {"foo": "bar"}}}}',
                '{"letters": {"C": {"id": 3, "value": "scalar-string"}}}',
            ]
        }
    )

    # Normalise with settings that cause value to become a map
    normalised = df.genson.normalise_json(
        "json_data",
        map_threshold=3,  # value field (3 props) meets threshold → becomes map
        map_max_required_keys=2,  # Allows maps with ≤2 required keys
        unify_maps=True,
        wrap_scalars=True,
    ).to_dicts()

    # Should have map structure but with consistent key naming
    # Currently produces "default" but should produce "value__string"
    assert normalised == [
        {
            "letters": [
                {
                    "key": "A",
                    "value": {
                        "id": 1,
                        "value": [{"key": "hello", "value": "world"}],
                    },
                }
            ]
        },
        {
            "letters": [
                {
                    "key": "B",
                    "value": {
                        "id": 2,
                        "value": [{"key": "foo", "value": "bar"}],
                    },
                }
            ]
        },
        {
            "letters": [
                {
                    "key": "C",
                    "value": {
                        "id": 3,
                        "value": [
                            {"key": "value__string", "value": "scalar-string"}
                        ],  # Should be value__string, not default
                    },
                }
            ]
        },
    ]


# def test_claims_fixture_parquet_placeholder():
#     df = pl.read_parquet("tests/data/claims_x4.parquet")
#     ...
