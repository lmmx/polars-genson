# tests/decode_typed_test.py
"""normalise_json with decode=True decodes the rows straight into Polars dtypes.

It must give exactly what the JSON text route gives: rows normalised to JSON text and
decoded with str.json_decode against the inferred schema, which `decode=<that schema>` still
takes.
"""

import json
import random
from pathlib import Path

import polars as pl
import polars_genson  # noqa: F401
from pytest import mark

DATA = Path(__file__).parent / "data"

WIKIDATA = {
    "map_threshold": 0,
    "unify_maps": True,
    "force_field_types": {"mainsnak": "record"},
    "force_scalar_promotion": {"mainsnak", "datavalue", "precision", "latitude", "longitude"},
    "wrap_root": "claims",
}


def via_text(df: pl.DataFrame, map_encoding: str, unnest: bool, **kw) -> pl.DataFrame:
    """The JSON text route: the schema inferred, then normalised text decoded against it."""
    inference = {k: v for k, v in kw.items() if k not in ("coerce_strings", "empty_as_null")}
    schema = df.genson.infer_polars_schema(
        "j", avro=True, map_encoding=map_encoding, **inference
    )
    return df.genson.normalise_json(
        "j", decode=schema, map_encoding=map_encoding, unnest=unnest, **kw
    )


def assert_routes_agree(rows, map_encoding="mapping", unnest=True, **kw):
    df = pl.DataFrame({"j": rows}, schema={"j": pl.String})
    typed = df.genson.normalise_json(
        "j", map_encoding=map_encoding, unnest=unnest, **kw
    )
    text = via_text(df, map_encoding, unnest, **kw)
    assert typed.schema == text.schema
    assert typed.equals(text)


def random_rows(rng: random.Random, n: int) -> list[str | None]:
    """Rows whose keys recur at several depths with different shapes."""
    def value(depth):
        kind = rng.randrange(8 if depth < 3 else 5)
        if kind == 0:
            return rng.randrange(-5, 5)
        if kind == 1:
            return rng.choice(["a", "b", ""])
        if kind == 2:
            return rng.random() < 0.5
        if kind == 3:
            return None
        if kind == 4:
            return rng.random()
        if kind == 5:
            return [value(depth + 1) for _ in range(rng.randrange(3))]
        return {rng.choice("pqrs"): value(depth + 1) for _ in range(rng.randrange(4))}

    rows = []
    for _ in range(n):
        if rng.random() < 0.05:
            rows.append(None)
        else:
            rows.append(json.dumps({rng.choice("abcd"): value(0) for _ in range(rng.randrange(1, 5))}))
    return rows


@mark.parametrize("map_encoding", ["mapping", "kv"])
@mark.parametrize("unnest", [True, False])
def test_claims_fixture(map_encoding, unnest):
    rows = (DATA / "claims_fixture_x4.jsonl").read_text().splitlines()
    assert_routes_agree(rows, map_encoding, unnest, **WIKIDATA)


@mark.parametrize(
    "rows,kw",
    [
        (['{"m": {"k%d": %d}}' % (i, i) for i in range(30)] + ['{"m": {}}', '{"m": null}', "{}"], {"map_threshold": 5}),
        (['{"v": 1}', '{"v": "s"}', '{"v": {"x": 1}}', '{"v": [1, 2]}', '{"v": null}', '{"v": 1.5}'], {}),
        (['{"n": "42", "b": "true"}', '{"n": 7, "b": false}'], {"coerce_strings": True}),
        (['{"s": ""}', '{"s": "x"}', '{"t": {}}', '{"t": {"u": 1}}'], {"empty_as_null": False}),
        (['{"x": 123456789012345678901234567890}', '{"x": 1}'], {}),
        (['{"a": 1}', '{"a": 2, "c": "z"}'], {"wrap_root": "root"}),
        (['{"a": 1, "b": {"x": "y"}}', None, "", '{"a": 2}', "   ", '{"b": {"x": "z", "w": [1, 2]}}'], {}),
    ],
    ids=["maps", "promoted scalars", "coerced strings", "empty kept", "big integer", "wrap_root", "null and empty rows"],
)
def test_cases(rows, kw):
    assert_routes_agree(rows, **kw)
    assert_routes_agree(rows, "kv", **kw)


@mark.parametrize("seed", range(40))
def test_random_rows(seed):
    rng = random.Random(seed)
    rows = random_rows(rng, rng.randrange(1, 60))
    kw = {"map_threshold": rng.choice([0, 2, 20]), "unify_maps": rng.random() < 0.5}
    map_encoding, unnest = rng.choice(["mapping", "kv"]), rng.random() < 0.5
    try:
        via_text(pl.DataFrame({"j": rows}, schema={"j": pl.String}), map_encoding, unnest, **kw)
    except Exception:
        # Rows the text route itself cannot decode (#238, a root with no fields) are out of
        # scope here
        return
    assert_routes_agree(rows, map_encoding, unnest, **kw)
