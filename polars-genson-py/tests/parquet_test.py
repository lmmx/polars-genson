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
        schema = orjson.loads(f.read())

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


def test_normalise_from_parquet_typed_matches_json_decode(tmp_path):
    """typed=True writes the same frame that decoding the JSON string output gives."""
    from polars_genson import avro_to_polars_schema, read_parquet_metadata

    src = tmp_path / "src.parquet"
    rows = pl.read_parquet("tests/data/claims_fixture_x4.parquet")["claims"].to_list()
    pl.DataFrame({"claims": rows + [None, "null"]}).write_parquet(src)
    opts = dict(map_threshold=0, unify_maps=True, wrap_root="claims")

    normalise_from_parquet(src, "claims", tmp_path / "str.parquet", **opts)
    str_out = tmp_path / "str.parquet"
    avro = read_parquet_metadata(str_out)["genson_avro_schema"]
    dtype = pl.Struct(avro_to_polars_schema(avro))
    expected = pl.read_parquet(str_out).select(pl.col("claims").str.json_decode(dtype))

    typed_out = tmp_path / "typed.parquet"
    normalise_from_parquet(src, "claims", typed_out, typed=True, **opts)
    typed = pl.read_parquet(typed_out)
    typed_avro = read_parquet_metadata(typed_out)["genson_avro_schema"]
    assert typed.schema["claims"] == pl.Struct(avro_to_polars_schema(typed_avro))
    # Key order can differ between the two inference runs, so compare as dicts
    assert typed.to_dicts() == expected.to_dicts()


@pytest.mark.parametrize("typed", [False, True])
def test_normalise_from_parquet_keeps_null_rows_and_columns(tmp_path, typed):
    """Null rows stay as null output rows, so kept columns line up with the output."""
    src = tmp_path / "src.parquet"
    rows = pl.read_parquet("tests/data/claims_fixture_x4.parquet")["claims"].to_list()
    rows = rows[:2] + [None] + rows[2:]
    ids = [f"Q{i}" for i in range(len(rows))]
    pl.DataFrame({"id": ids, "claims": rows}).write_parquet(src)
    out = tmp_path / "out.parquet"

    normalise_from_parquet(
        src, "claims", out, wrap_root="claims", typed=typed, keep_columns=["id"]
    )

    result = pl.read_parquet(out)
    assert result.columns == ["id", "claims"]
    assert result["id"].to_list() == ids
    assert result["claims"].is_null().to_list() == [r is None for r in rows]


def test_normalise_from_parquet_keep_column_name_clash(tmp_path):
    """A kept column may not share the output column's name."""
    src = tmp_path / "src.parquet"
    pl.DataFrame({"id": ["Q1"], "claims": ['{"a": 1}']}).write_parquet(src)
    with pytest.raises(OSError, match="same name as the output column"):
        normalise_from_parquet(
            src, "claims", tmp_path / "out.parquet", keep_columns=["claims"]
        )


def test_typed_output_with_float_column_is_readable(tmp_path):
    """Typed output with a float column reads back in Polars.

    parquet-rs 60 marks float columns with the IEEE 754 total column order, which
    Polars before 1.43.2 rejects as invalid thrift; hence the polars>=1.43.2 floor.
    """
    src = tmp_path / "src.parquet"
    pl.DataFrame({"data": ['{"x": 1.5, "n": 1}', '{"x": 2.5, "n": 2}']}).write_parquet(
        src
    )
    out = tmp_path / "out.parquet"

    normalise_from_parquet(src, "data", out, typed=True)

    result = pl.read_parquet(out).unnest("data")
    assert result.schema == pl.Schema({"x": pl.Float64, "n": pl.Int64})
    assert result["x"].to_list() == [1.5, 2.5]
