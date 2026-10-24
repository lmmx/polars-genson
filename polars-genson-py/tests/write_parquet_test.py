"""write_parquet: Arrow data written with genson's Parquet writer."""

import polars as pl
import pyarrow as pa
import pyarrow.parquet as pq
import pytest
from polars_genson import normalise_from_parquet, write_parquet

NESTED = pa.table(
    {
        "id": ["Q1", "Q2", "Q3", None],
        "refs": [
            [{"hash": "a", "snaks": [{"key": "P1", "value": ["x", "y"]}]}],
            None,
            [],
            [{"hash": "b", "snaks": None}, {"hash": None, "snaks": [{"key": "P2", "value": []}]}],
        ],
        "amount": [1.5, None, -2.0, 0.0],
    }
)


def test_round_trip_matches_pyarrow(tmp_path):
    """The file holds the same data in the same Arrow schema as pyarrow writes it."""
    ours, theirs = tmp_path / "genson.parquet", tmp_path / "pyarrow.parquet"
    write_parquet(NESTED, ours)
    pq.write_table(NESTED, theirs)
    a, b = pq.read_table(ours), pq.read_table(theirs)
    assert a.schema.remove_metadata().equals(b.schema.remove_metadata())
    assert a.equals(b)
    assert pl.read_parquet_schema(ours) == pl.read_parquet_schema(theirs)


def test_spec_names_and_arrow_schema(tmp_path):
    """List groups take the specification's names, and the Arrow schema is stored."""
    path = tmp_path / "out.parquet"
    write_parquet(NESTED, path)
    meta = pq.read_metadata(path)
    assert b"ARROW:schema" in meta.metadata
    paths = {meta.schema.column(i).path for i in range(meta.num_columns)}
    assert "refs.list.element.hash" in paths


def test_row_group_size(tmp_path):
    path = tmp_path / "out.parquet"
    table = pa.table({"n": list(range(10))})
    write_parquet(table, path, row_group_size=3)
    meta = pq.read_metadata(path)
    assert [meta.row_group(i).num_rows for i in range(meta.num_row_groups)] == [3, 3, 3, 1]
    assert pq.read_table(path).equals(table)


def test_row_group_size_must_be_positive(tmp_path):
    with pytest.raises(ValueError, match="row_group_size"):
        write_parquet(NESTED, tmp_path / "out.parquet", row_group_size=0)


def test_inputs_of_the_pycapsule_interface(tmp_path):
    """A record batch reader and a Polars frame are written like the table."""
    reader = pa.RecordBatchReader.from_batches(NESTED.schema, NESTED.to_batches(max_chunksize=2))
    write_parquet(reader, tmp_path / "reader.parquet")
    assert pq.read_table(tmp_path / "reader.parquet").equals(NESTED)

    df = pl.DataFrame({"a": [1, 2, None], "b": [["x"], None, []]})
    write_parquet(df, tmp_path / "polars.parquet")
    assert pl.read_parquet(tmp_path / "polars.parquet").equals(df)


def test_genson_arrow_stream(tmp_path):
    """The stream normalise_from_parquet returns is written as it would be read."""
    src = tmp_path / "in.parquet"
    pl.DataFrame({"data": ['{"a": 1, "b": ["x"]}', '{"a": 2}']}).write_parquet(src)
    out = tmp_path / "out.parquet"
    stream = normalise_from_parquet(src, column="data", output_path=None, output_format="arrow", typed=True)
    write_parquet(stream, out)
    expected = pa.table(
        normalise_from_parquet(src, column="data", output_path=None, output_format="arrow", typed=True)
    )
    assert pq.read_table(out).equals(expected)


def test_empty_table(tmp_path):
    path = tmp_path / "out.parquet"
    write_parquet(NESTED.slice(0, 0), path)
    back = pq.read_table(path)
    assert back.num_rows == 0
    assert back.schema.remove_metadata().equals(NESTED.schema)


def test_not_arrow_data(tmp_path):
    with pytest.raises(TypeError, match="__arrow_c_stream__"):
        write_parquet([1, 2, 3], tmp_path / "out.parquet")
