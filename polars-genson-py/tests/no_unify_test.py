# tests/no_unify_test.py
"""The DataFrame.genson methods pass `no_unify` on, as the expression functions do."""

import json

import polars as pl
import polars_genson as pg

# `r` is a map candidate holding a key in `no_unify`, so it must stay a record
ROWS = [json.dumps({"r": {"qualifiers": {"a": i}, "x": {"b": str(i)}}}) for i in range(5)]
OPTIONS = {"map_threshold": 0, "unify_maps": True}


def frame():
    """A frame of the rows in one String column `j`."""
    return pl.DataFrame({"j": ROWS})


def test_infer_json_schema_method_matches_the_expression():
    """The method infers the schema the expression does, with and without `no_unify`."""
    df = frame()
    for no_unify in [None, {"qualifiers"}]:
        kw = {**OPTIONS, **({"no_unify": no_unify} if no_unify else {})}
        expression = json.loads(df.select(pg.infer_json_schema(pl.col("j"), **kw)).item())
        assert df.genson.infer_json_schema("j", **kw) == expression


def test_no_unify_keeps_the_record():
    """A record holding a `no_unify` key is not unified into a map."""
    df = frame()
    unified = df.genson.infer_json_schema("j", **OPTIONS)["properties"]["r"]
    kept = df.genson.infer_json_schema("j", no_unify={"qualifiers"}, **OPTIONS)["properties"]["r"]
    assert "additionalProperties" in unified
    assert set(kept["properties"]) == {"qualifiers", "x"}


def test_infer_polars_schema_honours_no_unify():
    """infer_polars_schema keeps the record as a struct of its fields."""
    df = frame()
    unified = df.genson.infer_polars_schema("j", **OPTIONS)
    kept = df.genson.infer_polars_schema("j", no_unify={"qualifiers"}, **OPTIONS)
    assert unified != kept
    assert [f.name for f in kept["r"].fields] == ["qualifiers", "x"]


def test_normalise_json_honours_no_unify():
    """normalise_json keeps the record by the typed, schema and text routes alike."""
    df = frame()
    kept = df.genson.normalise_json("j", no_unify={"qualifiers"}, **OPTIONS)
    assert [f.name for f in kept.schema["r"].fields] == ["qualifiers", "x"]
    # The JSON text route, given the same schema, agrees
    schema = df.genson.infer_polars_schema("j", avro=True, no_unify={"qualifiers"}, **OPTIONS)
    assert kept.equals(df.genson.normalise_json("j", decode=schema, no_unify={"qualifiers"}, **OPTIONS))
    text = df.genson.normalise_json("j", decode=False, no_unify={"qualifiers"}, **OPTIONS)
    assert set(json.loads(text[0])["r"]) == {"qualifiers", "x"}
