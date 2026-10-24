# tests/avro_schema_test.py
"""normalise_json with avro_schema normalises against that schema instead of inferring one.

Given the schema inference would infer, it gives exactly the inferred route's result, on
every route (typed decode, a dtype, and the JSON text). Given a wider schema, the rows are
normalised to it: fields the rows lack are null, and empty arrays and maps null.
"""

import json

import polars as pl
import polars_genson  # noqa: F401
from polars_genson import normalise_json
from pytest import mark, raises

SITELINK = pl.Struct(
    {"site": pl.String, "title": pl.String, "badges": pl.List(pl.String)}
)
LINKS = pl.Struct({"links": pl.List(pl.Struct({"key": pl.String, "value": SITELINK}))})
OPTIONS = dict(
    ndjson=True,
    wrap_root="links",
    force_field_types={"links": "map"},
    map_encoding="kv",
)
ROWS = [
    '{"enwiki": {"site": "enwiki", "title": "A", "badges": ["Q17437796"]},'
    ' "frwiki": {"site": "frwiki", "title": "B", "badges": []}}',
    "{}",
    None,
    '{"dewiki": {"site": "dewiki", "title": "C", "badges": []}}',
]
# The schema inference gives these rows, and one wider than it (title nullable)
INFERRED = {
    "type": "record",
    "name": "document",
    "namespace": "genson",
    "fields": [
        {
            "name": "links",
            "type": {
                "type": "map",
                "name": "links",
                "values": {
                    "type": "record",
                    "name": "links_values",
                    "namespace": "genson.document_types",
                    "fields": [
                        {"name": "site", "type": "string"},
                        {"name": "title", "type": "string"},
                        {"name": "badges", "type": {"type": "array", "items": "string"}},
                    ],
                },
            },
        }
    ],
}
WIDER = json.loads(
    json.dumps(INFERRED).replace(
        '{"name": "title", "type": "string"}',
        '{"name": "title", "type": ["null", "string"]}',
    )
)


def frame(rows=ROWS) -> pl.DataFrame:
    return pl.DataFrame({"links": rows}, schema={"links": pl.String})


def test_inferred_schema_is_the_one_given():
    """INFERRED is what inference makes of ROWS, so the tests below compare like with like."""
    got = frame().genson.infer_json_schema(
        "links",
        ndjson=True,
        wrap_root="links",
        force_field_types={"links": "map"},
        avro=True,
    )
    assert json.dumps(got, sort_keys=True) == json.dumps(INFERRED, sort_keys=True)


@mark.parametrize("decode", [True, LINKS, False])
@mark.parametrize("given", [INFERRED, json.dumps(INFERRED)])
def test_given_the_inferred_schema_the_same(decode, given):
    df = frame()
    inferred = df.genson.normalise_json("links", decode=decode, **OPTIONS)
    skipped = df.genson.normalise_json(
        "links", decode=decode, avro_schema=given, **OPTIONS
    )
    assert skipped.equals(inferred)


@mark.parametrize("decode", [LINKS, False])
def test_a_wider_schema_gives_the_same_rows(decode):
    df = frame()
    inferred = df.genson.normalise_json("links", decode=decode, **OPTIONS)
    wider = df.genson.normalise_json(
        "links", decode=decode, avro_schema=WIDER, **OPTIONS
    )
    assert wider.equals(inferred)


def test_inference_options_do_not_apply():
    """With a schema given, map_threshold (which would make `links` a record) is unused."""
    df = frame()
    out = df.genson.normalise_json(
        "links", decode=LINKS, avro_schema=INFERRED, **{**OPTIONS, "force_field_types": None}
    )
    assert out.equals(df.genson.normalise_json("links", decode=LINKS, **OPTIONS))


def test_fields_the_rows_lack_are_null():
    schema = json.loads(json.dumps(INFERRED))
    values = schema["fields"][0]["type"]["values"]["fields"]
    values.append({"name": "url", "type": ["null", "string"]})
    out = frame().genson.normalise_json(
        "links", decode=True, avro_schema=schema, **OPTIONS
    )
    entries = out["links"][0]
    assert [e["value"]["url"] for e in entries] == [None, None]


def test_expression_takes_it():
    df = frame()
    exprs = [
        normalise_json(pl.col("links"), avro_schema=schema, **OPTIONS)
        for schema in (None, INFERRED)
    ]
    a, b = (df.select(e).to_series() for e in exprs)
    assert a.equals(b)


def test_invalid_schema_raises():
    with raises(Exception, match="avro_schema"):
        frame().genson.normalise_json(
            "links", decode=False, avro_schema="{not json", **OPTIONS
        )
