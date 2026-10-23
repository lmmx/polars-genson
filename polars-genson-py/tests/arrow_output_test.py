"""normalise_from_parquet(output_format="arrow"): the output as an Arrow C stream.

It holds what the Parquet output holds (the same rows, columns, types and schema
metadata), with and without a target schema and unnesting, is read once, and is refused
with an output path or without typed output. A requested schema of the same data (its
own, or another representation of it) gives the stream in its own schema, and one of
other data raises, leaving the stream to be read.
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


def arrow_stream(source):
    """The unnested output as an Arrow C stream."""
    return normalise_from_parquet(
        source, output_path=None, output_format="arrow", unnest=True, **KW
    )


def test_requested_schema_of_the_same_data(source, tmp_path):
    """A request for the same data gives the stream in its own schema.

    The request is its own schema, or the same data in other representations (string for
    large_string, list for large_list).
    """
    out = tmp_path / "out.parquet"
    normalise_from_parquet(source, output_path=out, unnest=True, **KW)
    want = pq.read_table(out)
    own = pa.RecordBatchReader.from_stream(arrow_stream(source)).schema

    def plain(t: pa.DataType) -> pa.DataType:
        """The type with 32-bit offsets in place of 64-bit ones."""
        if pa.types.is_large_string(t):
            return pa.string()
        if pa.types.is_large_list(t):
            return pa.list_(plain(t.value_type))
        if pa.types.is_struct(t):
            return pa.struct([f.with_type(plain(f.type)) for f in t])
        return t

    other = pa.schema([f.with_type(plain(f.type)) for f in own])
    assert other != own
    for requested in (own, other):
        reader = pa.RecordBatchReader.from_stream(
            arrow_stream(source), schema=requested
        )
        assert reader.schema == own
        assert reader.read_all().equals(want)


@pytest.mark.parametrize(
    "change",
    ["rename a field", "another type", "drop a field"],
)
def test_requested_schema_of_other_data(source, change):
    """Other data raises ValueError, and the stream can still be read."""
    stream = arrow_stream(source)
    own = pa.RecordBatchReader.from_stream(arrow_stream(source)).schema
    if change == "rename a field":
        requested = own.set(0, own.field(0).with_name("other"))
    elif change == "another type":
        requested = own.set(0, own.field(0).with_type(pa.int64()))
    else:
        requested = own.remove(len(own) - 1)
    with pytest.raises(ValueError, match="requested schema"):
        pa.RecordBatchReader.from_stream(stream, schema=requested)
    assert pa.table(stream).num_rows == len(ROWS)


def test_requested_schema_not_a_schema(source):
    """A requested_schema that is not an arrow_schema capsule raises TypeError."""
    with pytest.raises(TypeError, match="arrow_schema"):
        arrow_stream(source).__arrow_c_stream__("not a schema")
