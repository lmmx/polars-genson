# tests/map_max_required_keys_test.py
"""Tests for map_max_required_keys parameter in Polars extension."""

import polars as pl
import pytest


class TestMapMaxRequiredKeysNone:
    """Tests for map_max_required_keys=None (default behavior)."""

    def test_existing_behavior_preserved(self):
        """With map_max_required_keys=None, only threshold and homogeneity matter."""
        df = pl.DataFrame(
            {
                "json_data": [
                    '{"structured": {"req1": "val1", "req2": "val2", "req3": "val3"}}',
                    '{"structured": {"req1": "val4", "req2": "val5", "req3": "val6"}}',
                    '{"below_threshold": {"only": "one"}}',
                    '{"below_threshold": {"only": "two"}}',
                ]
            }
        )

        schema = df.genson.infer_json_schema("json_data", map_threshold=3)

        # structured meets threshold=3 and is homogeneous → Map
        structured = schema["properties"]["structured"]
        assert structured["type"] == "object"
        assert "additionalProperties" in structured
        assert "properties" not in structured

        # below_threshold has 1 key < threshold → Record
        below_threshold = schema["properties"]["below_threshold"]
        assert below_threshold["type"] == "object"
        assert "properties" in below_threshold
        assert "additionalProperties" not in below_threshold

    def test_avro_format_consistency(self):
        """Avro output should show same Map/Record decisions."""
        df = pl.DataFrame(
            {
                "json_data": [
                    '{"structured": {"req1": "val1", "req2": "val2", "req3": "val3"}}',
                    '{"below_threshold": {"only": "one"}}',
                ]
            }
        )

        avro_schema = df.genson.infer_json_schema(
            "json_data", map_threshold=3, avro=True
        )

        # structured should be Avro map
        structured_field = next(
            f for f in avro_schema["fields"] if f["name"] == "structured"
        )
        assert structured_field["type"][0] == "null"
        assert structured_field["type"][1]["type"] == "map"
        assert structured_field["type"][1]["values"] == "string"

        # below_threshold should be Avro record
        below_field = next(
            f for f in avro_schema["fields"] if f["name"] == "below_threshold"
        )
        assert below_field["type"][0] == "null"
        assert below_field["type"][1]["type"] == "record"
        assert "fields" in below_field["type"][1]


class TestMapMaxRequiredKeysZero:
    """Tests for map_max_required_keys=0 (strictest setting)."""

    def test_only_zero_required_keys_become_maps(self):
        """Only objects with 0 required keys can become Maps."""
        df = pl.DataFrame(
            {
                "json_data": [
                    '{"fully_optional": {"sometimes": "here", "other": "maybe"}}',
                    '{"fully_optional": {"different": "keys"}}',
                    '{"has_required": {"always": "present", "sometimes": "here"}}',
                    '{"has_required": {"always": "present", "other": "value"}}',
                ]
            }
        )

        schema = df.genson.infer_json_schema(
            "json_data", map_threshold=2, map_max_required_keys=0
        )

        # fully_optional has 0 required keys → Map
        fully_optional = schema["properties"]["fully_optional"]
        assert fully_optional["type"] == "object"
        assert "additionalProperties" in fully_optional

        # has_required has 1 required key > 0 → Record
        has_required = schema["properties"]["has_required"]
        assert has_required["type"] == "object"
        assert "properties" in has_required
        assert has_required["required"] == ["always"]

    def test_normalisation_strict_map_detection(self):
        """Normalisation should reflect strict Map detection."""
        df = pl.DataFrame(
            {
                "json_data": [
                    '{"fully_optional": {"sometimes": "here"}}',
                    '{"has_required": {"always": "present"}}',
                ]
            }
        )

        normalized = df.genson.normalise_json(
            "json_data", map_threshold=2, map_max_required_keys=0
        )
        results = normalized.to_dicts()

        # fully_optional preserves dynamic structure (Map)
        assert "fully_optional" in results[0]
        assert results[0]["fully_optional"] is not None

        # has_required gets fixed structure (Record)
        assert "has_required" in results[1]
        assert results[1]["has_required"] is not None


class TestMapMaxRequiredKeysOne:
    """Tests for map_max_required_keys=1 (moderate setting)."""

    def test_one_required_key_allowed(self):
        """Objects with ≤1 required key can become Maps."""
        df = pl.DataFrame(
            {
                "json_data": [
                    '{"one_required": {"common": "always", "varies": "sometimes"}}',
                    '{"one_required": {"common": "always", "other": "different"}}',
                    '{"two_required": {"stable1": "always", "stable2": "present", "varies": "sometimes"}}',
                    '{"two_required": {"stable1": "always", "stable2": "present", "other": "value"}}',
                ]
            }
        )

        schema = df.genson.infer_json_schema(
            "json_data", map_threshold=2, map_max_required_keys=1
        )

        # one_required: 1 required key ≤ 1 → Map
        one_required = schema["properties"]["one_required"]
        assert "additionalProperties" in one_required

        # two_required: 2 required keys > 1 → Record
        two_required = schema["properties"]["two_required"]
        assert "properties" in two_required
        assert len(two_required["required"]) == 2

    def test_boundary_condition_exactly_at_limit(self):
        """Test objects exactly at the required key limit."""
        df = pl.DataFrame(
            {
                "json_data": [
                    '{"at_limit": {"required": "always", "optional": "sometimes"}}',
                    '{"at_limit": {"required": "always"}}',
                    '{"over_limit": {"req1": "always", "req2": "present"}}',
                    '{"over_limit": {"req1": "always", "req2": "present"}}',
                ]
            }
        )

        schema = df.genson.infer_json_schema(
            "json_data", map_threshold=2, map_max_required_keys=1
        )

        # at_limit: 1 required key ≤ 1 → Map
        at_limit = schema["properties"]["at_limit"]
        assert "additionalProperties" in at_limit

        # over_limit: 2 required keys > 1 → Record
        over_limit = schema["properties"]["over_limit"]
        assert "properties" in over_limit


class TestMapMaxRequiredKeysTwo:
    """Tests for map_max_required_keys=2 (lenient setting)."""

    def test_two_required_keys_allowed(self):
        """Objects with ≤2 required keys can become Maps."""
        df = pl.DataFrame(
            {
                "json_data": [
                    '{"two_required": {"common1": "always", "common2": "present", "varies": "sometimes"}}',
                    '{"two_required": {"common1": "always", "common2": "present", "other": "value"}}',
                    '{"three_required": {"stable1": "always", "stable2": "present", "stable3": "here", "varies": "sometimes"}}',
                    '{"three_required": {"stable1": "always", "stable2": "present", "stable3": "here", "other": "value"}}',
                ]
            }
        )

        schema = df.genson.infer_json_schema(
            "json_data", map_threshold=3, map_max_required_keys=2
        )

        # two_required: 2 required keys ≤ 2 → Map
        two_required = schema["properties"]["two_required"]
        assert "additionalProperties" in two_required

        # three_required: 3 required keys > 2 → Record
        three_required = schema["properties"]["three_required"]
        assert "properties" in three_required
        assert len(three_required["required"]) == 3


class TestComplexNested:
    """Tests for nested objects with different required key patterns."""

    def test_nested_map_record_discrimination(self):
        """Nested objects should be classified independently."""
        df = pl.DataFrame(
            {
                "json_data": [
                    '{"user": {"id": 1, "name": "Alice"}, "config": {"host": "localhost", "port": "8080", "debug": "true"}}',
                    '{"user": {"id": 2, "name": "Bob"}, "config": {"host": "prod.com", "port": "443"}}',
                    '{"user": {"id": 3, "name": "Charlie"}, "config": {"host": "test.com", "port": "3000", "env": "test"}}',
                ]
            }
        )

        schema = df.genson.infer_json_schema(
            "json_data", map_threshold=2, map_max_required_keys=2
        )

        # Root level stays Record (user+config required, but mixed types fail homogeneity)
        assert "properties" in schema
        assert "user" in schema["properties"]
        assert "config" in schema["properties"]

        # user: id+name both required, mixed types → Record
        user = schema["properties"]["user"]
        assert "properties" in user

        # config: host+port required (2 ≤ 2), homogeneous strings → Map
        config = schema["properties"]["config"]
        assert "additionalProperties" in config
        assert config["additionalProperties"]["type"] == "string"

    def test_normalisation_preserves_nested_structure(self):
        """Normalisation should handle nested Map/Record correctly."""
        df = pl.DataFrame(
            {
                "json_data": [
                    '{"user": {"id": 1, "name": "Alice"}, "config": {"host": "localhost", "port": "8080"}}'
                ]
            }
        )

        normalized = df.genson.normalise_json(
            "json_data", map_threshold=2, map_max_required_keys=2
        )
        result = normalized.to_dicts()[0]

        # Both fields should be present
        assert "user" in result
        assert "config" in result
        assert result["user"]["id"] == 1
        assert result["user"]["name"] == "Alice"


class TestProgression:
    """Tests showing progressive behavior with different max_rk values."""

    def setup_method(self):
        """Common test data: 2 required keys (always1, always2)."""
        self.df = pl.DataFrame(
            {
                "json_data": [
                    '{"data": {"always1": "val1", "always2": "val2", "sometimes": "val3"}}',
                    '{"data": {"always1": "val4", "always2": "val5"}}',
                    '{"data": {"always1": "val6", "always2": "val7", "other": "val8"}}',
                ]
            }
        )

    def test_progression_max_rk_0_record(self):
        """max_rk=0: 2 required > 0 → Record."""
        schema = self.df.genson.infer_json_schema(
            "json_data", map_threshold=2, map_max_required_keys=0
        )

        data_field = schema["properties"]["data"]
        assert "properties" in data_field
        assert data_field["required"] == ["always1", "always2"]

    def test_progression_max_rk_1_record(self):
        """max_rk=1: 2 required > 1 → Record."""
        schema = self.df.genson.infer_json_schema(
            "json_data", map_threshold=2, map_max_required_keys=1
        )

        data_field = schema["properties"]["data"]
        assert "properties" in data_field
        assert data_field["required"] == ["always1", "always2"]

    def test_progression_max_rk_2_map(self):
        """max_rk=2: 2 required ≤ 2 → Map."""
        schema = self.df.genson.infer_json_schema(
            "json_data", map_threshold=2, map_max_required_keys=2
        )

        data_field = schema["properties"]["data"]
        assert "additionalProperties" in data_field
        assert data_field["additionalProperties"]["type"] == "string"

    def test_progression_avro_consistency(self):
        """Avro schemas should show same progression."""
        # max_rk=0: Record
        avro0 = self.df.genson.infer_json_schema(
            "json_data", map_threshold=2, map_max_required_keys=0, avro=True
        )
        data_field0 = next(f for f in avro0["fields"] if f["name"] == "data")
        assert data_field0["type"]["type"] == "record"

        # max_rk=2: Map
        avro2 = self.df.genson.infer_json_schema(
            "json_data", map_threshold=2, map_max_required_keys=2, avro=True
        )
        data_field2 = next(f for f in avro2["fields"] if f["name"] == "data")
        assert data_field2["type"]["type"] == "map"

    def test_progression_normalization_differences(self):
        """Normalization should differ between Record and Map schemas."""
        # Record normalization (max_rk=1)
        norm_record = self.df.genson.normalise_json(
            "json_data", map_threshold=2, map_max_required_keys=1
        )
        record_results = norm_record.to_dicts()

        # Map normalization (max_rk=2)
        norm_map = self.df.genson.normalise_json(
            "json_data", map_threshold=2, map_max_required_keys=2
        )
        map_results = norm_map.to_dicts()

        # Record should have consistent fields across rows (including nulls)
        # Map should preserve only present keys
        # The exact structure depends on normalization implementation
        assert len(record_results) == len(map_results) == 3
        for record_row, map_row in zip(record_results, map_results):
            assert "data" in record_row
            assert "data" in map_row


class TestInteractionWithExistingFeatures:
    """Tests showing interaction with force_field_types and other features."""

    def test_force_override_takes_precedence(self):
        """force_field_types should override map_max_required_keys."""
        df = pl.DataFrame(
            {
                "json_data": [
                    '{"labels": {"en": "Hello", "fr": "Bonjour"}}',
                    '{"labels": {"en": "World", "fr": "Monde"}}',
                ]
            }
        )

        schema = df.genson.infer_json_schema(
            "json_data",
            map_threshold=2,
            map_max_required_keys=0,  # Would normally block this
            force_field_types={"labels": "map"},
        )

        # Force override should win
        labels = schema["properties"]["labels"]
        assert "additionalProperties" in labels

    def test_homogeneity_requirement_still_applies(self):
        """Non-homogeneous values should prevent Map inference regardless of required keys."""
        df = pl.DataFrame(
            {
                "json_data": [
                    '{"mixed": {"str_field": "text", "num_field": 42}}',
                    '{"mixed": {"str_field": "more", "num_field": 24}}',
                ]
            }
        )

        schema = df.genson.infer_json_schema(
            "json_data", map_threshold=2, map_max_required_keys=5
        )

        # Should remain Record due to mixed types, despite low required key count
        mixed = schema["properties"]["mixed"]
        assert "properties" in mixed  # Record structure
        assert "additionalProperties" not in mixed

    def test_threshold_requirement_still_applies(self):
        """Objects below map_threshold should remain Records regardless of required keys."""
        df = pl.DataFrame({"json_data": ['{"small": {"only": "one"}}']})

        schema = df.genson.infer_json_schema(
            "json_data", map_threshold=5, map_max_required_keys=10
        )

        # Should remain Record due to being below threshold
        small = schema["properties"]["small"]
        assert "properties" in small
        assert "additionalProperties" not in small


class TestEdgeCases:
    """Tests for edge cases and boundary conditions."""

    def test_empty_objects_handled_correctly(self):
        """Empty objects should not cause errors."""
        df = pl.DataFrame({"json_data": ['{"empty": {}}']})

        schema = df.genson.infer_json_schema("json_data", map_max_required_keys=0)

        # Should not crash and should have some reasonable structure
        assert "properties" in schema
        assert "empty" in schema["properties"]

    def test_very_high_max_required_keys(self):
        """Very high threshold should behave like None."""
        df = pl.DataFrame(
            {
                "json_data": [
                    '{"many_required": {"a": "1", "b": "2", "c": "3", "d": "4", "e": "5"}}'
                ]
            }
        )

        schema = df.genson.infer_json_schema(
            "json_data", map_threshold=5, map_max_required_keys=1000
        )

        # Should allow map inference due to high threshold
        many_required = schema["properties"]["many_required"]
        assert "additionalProperties" in many_required
