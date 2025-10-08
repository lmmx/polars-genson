# polars-genson-py/tests/test_parquet_io.py
"""Tests for Parquet I/O functionality."""

import json
from pathlib import Path

import orjson
import polars as pl
import pytest
from polars_genson import infer_from_parquet, normalise_from_parquet


@pytest.fixture
def claims_parquet_path():
    """Path to the claims fixture parquet file."""
    return "tests/data/claims_fixture_x4.parquet"


@pytest.fixture
def temp_parquet(tmp_path):
    """Temporary parquet file path."""
    return tmp_path / "output.parquet"


@pytest.fixture
def temp_json(tmp_path):
    """Temporary JSON file path."""
    return tmp_path / "schema.json"


def test_infer_from_parquet(claims_parquet_path):
    """Test schema inference from Parquet file."""
    # Infer schema - same args as CLI command
    schema = infer_from_parquet(
        claims_parquet_path,
        column="claims",
        map_threshold=0,
        unify_maps=True,
        wrap_root="claims",
    )

    # Verify it's a dict with expected structure
    assert isinstance(schema, dict)
    assert "type" in schema


def test_infer_from_parquet_to_file(claims_parquet_path, temp_json):
    """Test schema inference with file output."""
    # Write schema to file
    result = infer_from_parquet(
        claims_parquet_path,
        column="claims",
        output_path=str(temp_json),
        map_threshold=0,
        unify_maps=True,
        wrap_root="claims",
    )

    # Verify result message
    assert "Schema written to" in result

    # Verify file was written
    assert temp_json.exists()

    # Read and verify content
    with open(temp_json) as f:
        schema = orjson.load(f)

    assert isinstance(schema, dict)
    assert "type" in schema


def test_normalise_from_parquet(claims_parquet_path, temp_parquet):
    """Test normalization from Parquet to Parquet."""
    # Normalize the data
    normalise_from_parquet(
        claims_parquet_path,
        column="claims",
        output_path=str(temp_parquet),
        map_threshold=0,
        unify_maps=True,
        wrap_root="claims",
    )

    # Verify output file exists
    assert temp_parquet.exists()

    # Read the normalized data
    df = pl.read_parquet(temp_parquet)

    # Verify it has the claims column
    assert "claims" in df.columns
    assert df.shape[0] == 4  # Same number of rows as input

    # Check that the data is valid JSON
    for row in df["claims"]:
        parsed = orjson.loads(row)
        assert isinstance(parsed, dict)


def test_normalise_parquet_custom_output_column(claims_parquet_path, temp_parquet):
    """Test normalization with custom output column name."""
    # Normalize with custom column name
    normalise_from_parquet(
        claims_parquet_path,
        column="claims",
        output_path=str(temp_parquet),
        output_column="claims_normalized",
        map_threshold=0,
        unify_maps=True,
        wrap_root="claims",
    )

    df = pl.read_parquet(temp_parquet)

    # Verify custom column name
    assert "claims_normalized" in df.columns
    assert df.shape[0] == 4


def test_infer_from_parquet_avro_mode(claims_parquet_path):
    """Test schema inference in Avro mode."""
    schema = infer_from_parquet(
        claims_parquet_path,
        column="claims",
        map_threshold=0,
        unify_maps=True,
        wrap_root="claims",
        avro=True,
    )

    assert isinstance(schema, dict)
    # Avro schemas have different structure
    assert "type" in schema


def test_normalise_from_parquet_different_encodings(claims_parquet_path, tmp_path):
    """Test normalization with different map encodings."""
    encodings = ["mapping", "entries", "kv"]

    for encoding in encodings:
        output_path = tmp_path / f"output_{encoding}.parquet"

        normalise_from_parquet(
            claims_parquet_path,
            column="claims",
            output_path=str(output_path),
            map_encoding=encoding,
            map_threshold=0,
            unify_maps=True,
            wrap_root="claims",
        )

        assert output_path.exists()
        df = pl.read_parquet(output_path)
        assert df.shape[0] == 4


def test_infer_from_parquet_with_debug(claims_parquet_path, capfd):
    """Test that debug output appears in stderr."""
    infer_from_parquet(
        claims_parquet_path,
        column="claims",
        map_threshold=0,
        unify_maps=True,
        wrap_root="claims",
        debug=True,
    )

    captured = capfd.readouterr()
    # Check that debug output appeared
    assert "Processed 4 JSON object(s)" in captured.err


def test_normalise_from_parquet_empty_as_null(claims_parquet_path, temp_parquet):
    """Test normalization with empty_as_null setting."""
    normalise_from_parquet(
        claims_parquet_path,
        column="claims",
        output_path=str(temp_parquet),
        empty_as_null=True,
        map_threshold=0,
        unify_maps=True,
        wrap_root="claims",
    )

    df = pl.read_parquet(temp_parquet)
    assert df.shape[0] == 4

    # Verify all rows are valid JSON
    for row in df["claims"]:
        orjson.loads(row)  # Should not raise


def test_normalise_parquet_in_place(claims_parquet_path, tmp_path):
    """Test in-place normalization by overwriting the source file."""
    # Copy the original file to tmp so we don't modify the test fixture
    import shutil

    temp_input = tmp_path / "claims_for_overwrite.parquet"
    shutil.copy(claims_parquet_path, temp_input)

    # Read original data
    df_before = pl.read_parquet(temp_input)
    original_first_row = df_before["claims"][0]

    # Normalize in-place (output_path == input_path)
    normalise_from_parquet(
        str(temp_input),
        column="claims",
        output_path=str(temp_input),  # Same as input!
        map_threshold=0,
        unify_maps=True,
        wrap_root="claims",
    )

    # Read the overwritten file
    df_after = pl.read_parquet(temp_input)

    # Verify it was modified
    assert df_after.shape[0] == 4
    assert "claims" in df_after.columns

    # The data should be normalized (different from original)
    normalized_first_row = df_after["claims"][0]
    # Both should be valid JSON
    orjson.loads(original_first_row)
    orjson.loads(normalized_first_row)
