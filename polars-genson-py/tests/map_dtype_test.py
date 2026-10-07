"""Maps decode to a native `pl.Map` on Polars 2+, and to key/value lists before that."""

import polars as pl
import pytest

from polars_genson import avro_to_polars_schema, normalise_from_parquet, schema_to_dict

HAS_MAP = hasattr(pl, "Map")
needs_map = pytest.mark.skipif(not HAS_MAP, reason="pl.Map needs Polars 2+")
KV_LIST = pl.List(pl.Struct({"key": pl.String, "value": pl.Int64}))
ROWS = ['{"id": 1, "m": {"a": 1, "b": 2}}', '{"id": 2, "m": {"c": 3}}', '{"id": 3}']


@pytest.fixture
def df():
    """The sample rows as a frame."""
    return pl.DataFrame({"json_data": ROWS})


def test_default_map_dtype(df):
    """Inferred and normalised maps share the version-appropriate dtype."""
    out = df.genson.normalise_json("json_data", map_threshold=1)
    expected = pl.Map(pl.String, pl.Int64) if HAS_MAP else KV_LIST
    assert out.schema == pl.Schema({"id": pl.Int64, "m": expected})
    assert out.schema == df.genson.infer_polars_schema("json_data", map_threshold=1)


@needs_map
def test_map_values(df):
    """Map values decode as dicts, with a null for a missing map."""
    out = df.genson.normalise_json("json_data", map_threshold=1)
    assert out["m"].to_list() == [{"a": 1, "b": 2}, {"c": 3}, None]


@needs_map
def test_explicit_kv_keeps_entries(df):
    """`map_encoding="kv"` still gives lists of key/value structs."""
    out = df.genson.normalise_json("json_data", map_threshold=1, map_encoding="kv")
    assert out.schema["m"] == KV_LIST
    assert out["m"].to_list()[1] == [{"key": "c", "value": 3}]


@needs_map
def test_nested_maps_decode_to_map():
    """Maps nested in a struct decode to `pl.Map` too."""
    rows = ['{"a": {"x": {"p": [1, 2]}, "y": {"p": [3]}}}', '{"a": {"z": {"q": []}}}']
    out = pl.DataFrame({"j": rows}).genson.normalise_json("j", map_threshold=1)
    assert out.schema["a"].fields[0].dtype == pl.Map(pl.String, pl.List(pl.Int64))
    assert out["a"].to_list()[0]["x"] == {"p": [1, 2]}


@needs_map
def test_known_schema_with_map(df):
    """A schema holding a `pl.Map` can be passed as `decode`."""
    schema = pl.Schema({"id": pl.Int64, "m": pl.Map(pl.String, pl.Int64)})
    out = df.genson.normalise_json("json_data", decode=schema, map_threshold=1)
    assert out.schema == schema
    assert out["m"].to_list() == [{"a": 1, "b": 2}, {"c": 3}, None]


def test_json_output_matches_map_encoding(df):
    """The JSON string output uses the encoding that matches the dtype."""
    out = df.genson.normalise_json("json_data", decode=False, map_threshold=1)
    first = out.to_list()[0]
    assert ('"m":{"a":1,"b":2}' in first) == HAS_MAP


def test_avro_schema_maps(df):
    """An Avro map converts to `pl.Map` (or entries on older Polars)."""
    avro = (
        '{"type":"record","name":"r","fields":[{"name":"m","type":'
        '{"type":"map","values":"long"}}]}'
    )
    expected = pl.Map(pl.String, pl.Int64) if HAS_MAP else KV_LIST
    assert avro_to_polars_schema(avro) == pl.Schema({"m": expected})


@pytest.mark.parametrize("map_encoding", [None, "kv", "mapping"])
def test_typed_parquet_maps(df, tmp_path, map_encoding):
    """Typed Parquet output reads back as the dtype `avro_to_polars_schema` gives.

    `"mapping"` writes a Parquet map, which Polars 1.x reads as the list of
    ``{key, value}`` structs, so it gives the same dtype as `"kv"` there.
    """
    src, out = tmp_path / "in.parquet", tmp_path / "out.parquet"
    df.write_parquet(src)
    normalise_from_parquet(
        src, "json_data", out, map_threshold=1, typed=True, map_encoding=map_encoding
    )
    m = pl.read_parquet(out).unnest("json_data")["m"]
    as_map = HAS_MAP and map_encoding != "kv"
    assert m.dtype == (pl.Map(pl.String, pl.Int64) if as_map else KV_LIST)
    assert m.to_list()[1] == ({"c": 3} if as_map else [{"key": "c", "value": 3}])


@needs_map
def test_schema_to_dict_map():
    """`schema_to_dict` spells out a map's key and value dtypes."""
    schema = pl.Schema({"m": pl.Map(pl.String, pl.List(pl.Int64))})
    assert schema_to_dict(schema) == {
        "m": {"map": {"key": "String", "value": {"list": "Int64"}}}
    }
