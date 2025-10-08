"""Test round-trip normalization and decoding from Parquet."""

import json
import tempfile
from pathlib import Path

import polars as pl
import pytest
from polars_genson import (
    avro_to_polars_schema,
    normalise_from_parquet,
    read_parquet_metadata,
)


@pytest.fixture
def sample_json_parquet(tmp_path):
    """Create a Parquet file with JSON strings."""
    data = pl.DataFrame(
        {
            "claims": [
                '{"P31":[{"mainsnak":{"property":"P31","datavalue":{"id":"Q5"},"datatype":"wikibase-item"},"rank":"normal"}]}',
                '{"P734":[{"mainsnak":{"property":"P734","datavalue":{"text":"Smith","language":"en"},"datatype":"monolingualtext"},"rank":"normal"}]}',
            ]
        }
    )
    path = tmp_path / "input.parquet"
    data.write_parquet(path)
    return path


def test_normalize_writes_metadata(sample_json_parquet, tmp_path):
    """Test that normalization writes schema metadata to Parquet."""
    output_path = tmp_path / "normalized.parquet"

    normalise_from_parquet(
        input_path=sample_json_parquet,
        column="claims",
        output_path=output_path,
        output_column="claims",
        ndjson=True,
        map_threshold=0,
        unify_maps=True,
    )

    # Check metadata exists
    metadata = read_parquet_metadata(output_path)

    assert "genson_avro_schema" in metadata
    assert "genson_normalise_config" in metadata

    # Verify it's valid JSON
    avro_schema = json.loads(metadata["genson_avro_schema"])
    norm_config = json.loads(metadata["genson_normalise_config"])

    assert avro_schema["type"] == "record"
    assert norm_config["wrap_root"] == None


def test_roundtrip_normalize_and_decode(sample_json_parquet, tmp_path):
    """Test full round-trip: normalize → read metadata → decode."""
    output_path = tmp_path / "normalized.parquet"

    # Step 1: Normalize
    normalise_from_parquet(
        input_path=sample_json_parquet,
        column="claims",
        output_path=output_path,
        output_column="claims",
        wrap_root="claims",
        ndjson=True,
        map_threshold=0,
        unify_maps=True,
    )

    # Step 2: Read back the normalized data
    result = pl.read_parquet(output_path)
    assert result.shape[0] == 2
    assert result.columns == ["claims"]

    # Step 3: Extract schema from metadata
    metadata = read_parquet_metadata(output_path)
    avro_schema_json = metadata["genson_avro_schema"]

    # Step 4: Convert Avro schema to Polars schema
    # TODO: This is where we need the conversion
    # For now, just verify the metadata is there
    avro_schema = json.loads(avro_schema_json)

    # The schema should have a claims field
    claims_field = next(f for f in avro_schema["fields"] if f["name"] == "claims")
    assert claims_field == {
        "name": "claims",
        "type": {
            "name": "claims",
            "type": "map",
            "values": {
                "type": "array",
                "items": {
                    "type": "record",
                    "name": "claims_values",
                    "namespace": "genson.document_types",
                    "fields": [
                        {
                            "name": "mainsnak",
                            "type": {
                                "type": "record",
                                "name": "mainsnak",
                                "namespace": "genson.document_types.claims_values_types",
                                "fields": [
                                    {"name": "property", "type": "string"},
                                    {
                                        "name": "datavalue",
                                        "type": {
                                            "name": "datavalue",
                                            "type": "map",
                                            "values": "string",
                                        },
                                    },
                                    {"name": "datatype", "type": "string"},
                                ],
                            },
                        },
                        {"name": "rank", "type": "string"},
                    ],
                },
            },
        },
    }

    # Step 5: Decode (once we have the schema conversion working)
    schema = avro_to_polars_schema(avro_schema_json)
    dtype = pl.Struct(schema)
    decoded = result.select(pl.col("claims").str.json_decode(dtype=dtype))
    assert decoded.unnest("claims").to_dict(as_series=False) == {
        "claims": [
            [
                {
                    "key": "P31",
                    "value": [
                        {
                            "mainsnak": {
                                "property": "P31",
                                "datavalue": [{"key": "id", "value": "Q5"}],
                                "datatype": "wikibase-item",
                            },
                            "rank": "normal",
                        }
                    ],
                }
            ],
            [
                {
                    "key": "P734",
                    "value": [
                        {
                            "mainsnak": {
                                "property": "P734",
                                "datavalue": [
                                    {"key": "text", "value": "Smith"},
                                    {"key": "language", "value": "en"},
                                ],
                                "datatype": "monolingualtext",
                            },
                            "rank": "normal",
                        }
                    ],
                }
            ],
        ]
    }


def test_metadata_helper_function(tmp_path):
    """Test the metadata reading helper."""
    # Create a simple parquet file with metadata
    df = pl.DataFrame({"data": ["test"]})
    path = tmp_path / "test.parquet"
    df.write_parquet(path)

    metadata = read_parquet_metadata(path)
    assert isinstance(metadata, dict)
    # Empty parquet has no custom metadata
    assert len(metadata) == 0
