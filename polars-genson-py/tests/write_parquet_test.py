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


def _statements(rows: int) -> pa.Table:
    """Rows of claims-like statement ids: each its row's id, `$`, then 32 hex digits."""
    ids = [f"Q{i // 5}" for i in range(rows)]
    return pa.table(
        {
            "id": pa.array(ids, pa.large_string()),
            "statement_id": pa.array(
                [f"{q}${(i * 0x9E3779B97F4A7C15F39CC0605CEDC835) % 2**128:032X}" for i, q in enumerate(ids)],
                pa.large_string(),
            ),
            "refs": pa.array([[f"{i % 40:040x}"] for i in range(rows)], pa.large_list(pa.large_string())),
        }
    )


def test_column_encoding(tmp_path):
    """A column asked for in DELTA_BYTE_ARRAY is written in it, and only the bytes change.

    It has no dictionary; the file has the same values, schema, row groups and
    statistics, in fewer bytes. A nested leaf takes its path.
    """
    t = _statements(20_000)
    plain, delta = tmp_path / "plain.parquet", tmp_path / "delta.parquet"
    write_parquet(t, plain, row_group_size=6_000)
    write_parquet(
        t,
        delta,
        row_group_size=6_000,
        column_encoding={"statement_id": "DELTA_BYTE_ARRAY", "refs.list.element": "delta_byte_array"},
    )
    mp, md = pq.read_metadata(plain), pq.read_metadata(delta)
    assert md.num_row_groups == mp.num_row_groups == 4
    for g in range(md.num_row_groups):
        for i in range(md.num_columns):
            a, b = mp.row_group(g).column(i), md.row_group(g).column(i)
            assert b.statistics == a.statistics
            if b.path_in_schema in ("statement_id", "refs.list.element"):
                assert "DELTA_BYTE_ARRAY" in b.encodings and not b.has_dictionary_page
            else:
                assert b.encodings == a.encodings
    a, b = pq.read_table(plain), pq.read_table(delta)
    assert b.schema.equals(a.schema, check_metadata=True) and b.equals(a) and b.equals(t)
    assert pl.read_parquet(delta).equals(pl.read_parquet(plain))
    size = lambda m: sum(m.row_group(g).column(1).total_compressed_size for g in range(m.num_row_groups))
    assert size(md) < size(mp)


@pytest.mark.parametrize(
    "encoding,says",
    [
        ({"nope": "DELTA_BYTE_ARRAY"}, "no leaf column"),
        ({"statement_id": "NOPE"}, "no encoding"),
        ({"statement_id": "DELTA_BINARY_PACKED"}, "does not encode"),
    ],
)
def test_column_encoding_checked(tmp_path, encoding, says):
    """A column or encoding not taken raises ValueError, and no file is written.

    An unknown column or encoding, or one the column's type does not take.
    """
    path = tmp_path / "out.parquet"
    with pytest.raises(ValueError, match=says):
        write_parquet(_statements(100), path, column_encoding=encoding)
    assert not path.exists()
