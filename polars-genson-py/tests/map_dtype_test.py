"""Maps as `pl.Map` (the default "mapping" encoding) and as key/value lists ("kv")."""

import polars as pl
import pytest
from polars_genson import normalise_from_parquet, schema_to_dict

ROWS = ['{"id": 1, "m": {"a": 1, "b": 2}}', '{"id": 2, "m": {"c": 3}}', '{"id": 3}']
KV = pl.List(pl.Struct({"key": pl.String, "value": pl.Int64}))


@pytest.fixture
def df():
    """The sample rows as a frame."""
    return pl.DataFrame({"json_data": ROWS})


def test_json_output_is_mapping(df):
    """The JSON string output writes maps as plain objects by default."""
    out = df.genson.normalise_json("json_data", decode=False, map_threshold=1)
    assert out.to_list()[0] == '{"id":1,"m":{"a":1,"b":2}}'


def test_nested_maps():
    """Maps inside a record are `pl.Map` too."""
    rows = ['{"a": {"x": {"p": [1, 2]}, "y": {"p": [3]}}}', '{"a": {"x": {"q": [4]}}}']
    out = pl.DataFrame({"j": rows}).genson.normalise_json("j", map_threshold=1)
    assert out.schema["a"].fields[0].dtype == pl.Map(pl.String, pl.List(pl.Int64))
    assert out["a"].to_list()[0]["x"] == {"p": [1, 2]}


@pytest.mark.parametrize("dtype", [pl.Map(pl.String, pl.Int64), KV])
def test_decode_schema_sets_map_encoding(df, dtype):
    """A schema passed as `decode` is decoded in the encoding its maps take."""
    schema = pl.Schema({"id": pl.Int64, "m": dtype})
    out = df.genson.normalise_json("json_data", decode=schema, map_threshold=1)
    assert out.schema == schema


@pytest.mark.parametrize(
    "map_encoding, dtype, value",
    [
        ("mapping", pl.Map(pl.String, pl.Int64), {"c": 3}),
        ("kv", KV, [{"key": "c", "value": 3}]),
    ],
)
def test_typed_parquet_maps(df, tmp_path, map_encoding, dtype, value):
    """Typed Parquet stores "mapping" maps as Parquet maps and "kv" maps as lists."""
    src, out = tmp_path / "in.parquet", tmp_path / "out.parquet"
    df.write_parquet(src)
    opts = {"map_threshold": 1, "typed": True, "map_encoding": map_encoding}
    normalise_from_parquet(src, "json_data", out, **opts)
    m = pl.read_parquet(out).unnest("json_data")["m"]
    assert m.dtype == dtype
    assert m.to_list()[1] == value


def test_schema_to_dict_map():
    """`schema_to_dict` spells out a map's key and value dtypes."""
    schema = pl.Schema({"m": pl.Map(pl.String, pl.List(pl.Int64))})
    assert schema_to_dict(schema) == {
        "m": {"map": {"key": "String", "value": {"list": "Int64"}}}
    }
