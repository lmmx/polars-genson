"""normalise_from_parquet(output_format="arrow"): the output as an Arrow C stream.

It holds what the Parquet output holds (the same rows, columns, types and schema
metadata), with and without a target schema and unnesting, is read once, and is refused
with an output path or without typed output.
"""

import orjson
import polars as pl
import pyarrow as pa
import pyarrow.parquet as pq
import pytest
from polars_genson import normalise_from_parquet

ROWS = [
    {"claims": {"P1": [{"mainsnak": {"value": "a", "n": 1}, "refs": [{"p": "x"}]}]}},
    {"claims": {"P2": [{"mainsnak": {"value": "b"}, "refs": []}]}},
    None,
    {"claims": {}},
]
STMT = pl.Struct(
    {
        "mainsnak": pl.Struct({"hash": pl.String, "value": pl.String, "n": pl.Int64}),
        "rank": pl.String,
        "refs": pl.List(pl.Struct({"p": pl.String, "q": pl.String})),
    }
)
TARGET = pl.Schema(
    {"claims": pl.List(pl.Struct({"key": pl.String, "value": pl.List(STMT)}))}
)
KW = dict(
    column="claims",
    output_column="claims",
    wrap_root="claims",
    force_field_types={"claims": "map"},
    map_encoding="kv",
    typed=True,
    keep_columns=["id"],
)


@pytest.fixture
def source(tmp_path):
    """The rows as JSON text in a Parquet column, with an id."""
    path = tmp_path / "in.parquet"
    text = [None if r is None else orjson.dumps(r["claims"]).decode() for r in ROWS]
    pl.DataFrame({"id": ["Q1", "Q2", "Q3", "Q4"], "claims": text}).write_parquet(path)
    return path


@pytest.mark.parametrize(
    "options",
    [{}, {"unnest": True}, {"schema": TARGET}, {"schema": TARGET, "unnest": True}],
    ids=["inferred", "unnest", "target", "target+unnest"],
)
def test_stream_holds_the_parquet_output(source, tmp_path, options):
    """The stream's table is the table the Parquet output reads back as."""
    out = tmp_path / "out.parquet"
    normalise_from_parquet(source, output_path=out, **KW, **options)
    want = pq.read_table(out)
    stream = normalise_from_parquet(
        source, output_path=None, output_format="arrow", **KW, **options
    )
    got = pa.RecordBatchReader.from_stream(stream).read_all()
    assert got.equals(want)
    assert got.schema.metadata == {
        k: v for k, v in want.schema.metadata.items() if k != b"ARROW:schema"
    }


def test_stream_is_read_once(source):
    """A second read is refused: the first moved the batches to its reader."""
    stream = normalise_from_parquet(
        source, output_path=None, output_format="arrow", **KW
    )
    assert pa.table(stream).num_rows == len(ROWS)
    with pytest.raises(ValueError, match="already been read"):
        pa.table(stream)


@pytest.mark.parametrize(
    "options, message",
    [
        ({"output_path": "out.parquet", "output_format": "arrow"}, "must be None"),
        ({"output_path": None}, "must be None"),
        ({"output_path": None, "output_format": "arrow", "typed": False}, "typed"),
        ({"output_path": "out.parquet", "output_format": "ipc"}, "output_format"),
    ],
    ids=["path with arrow", "no path for parquet", "untyped", "unknown format"],
)
def test_refused(source, options, message):
    """Arrow output takes no path and typed output; Parquet output needs a path."""
    with pytest.raises(ValueError, match=message):
        normalise_from_parquet(source, **{**KW, **options})
