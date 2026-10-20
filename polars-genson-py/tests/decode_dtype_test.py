# tests/decode_dtype_test.py
"""normalise_json with decode=<dtype> decodes the rows typed and projects them onto it.

It must give exactly what the JSON text route gives: rows normalised to JSON text (with the
map encoding the dtype decodes from) and decoded with str.json_decode against the dtype,
which keeps the dtype's fields by name, gives null for those the JSON lacks, and parses
leaves its own way. Where that could differ from a projection of the typed rows (a leaf
dtype other than the inferred one), normalise_json falls back to the text route.
"""

import json
import random
from pathlib import Path

import polars as pl
import polars_genson  # noqa: F401
from decode_typed_test import WIKIDATA, random_rows
from polars.expr.string import ExprStringNameSpace
from polars_genson.dtypes import contains_map, typed_route_makes
from pytest import fixture, mark, raises

DATA = Path(__file__).parent / "data"


@fixture
def routes(monkeypatch):
    """The route each decode=dtype call in `assert_same` takes, in order.

    "typed" (projected), or "text" (str.json_decode called on the normalised JSON).
    """
    decode = ExprStringNameSpace.json_decode

    def spy(self, *args, **kwargs):
        DECODES.append(1)
        return decode(self, *args, **kwargs)

    monkeypatch.setattr(ExprStringNameSpace, "json_decode", spy)
    taken = []
    ROUTES[:] = [taken]
    yield taken
    ROUTES.clear()


DECODES: list[int] = []
ROUTES: list[list[str]] = []


def text_route(df, dtype, unnest=True, map_encoding=None, **kw) -> pl.DataFrame:
    """The JSON text route: normalised text, decoded by str.json_decode against `dtype`."""
    if map_encoding is None:
        map_encoding = "mapping" if contains_map(dtype) else "kv"
    text = df.genson.normalise_json("j", decode=False, map_encoding=map_encoding, **kw)
    out = text.str.json_decode(dtype).to_frame()
    return out.unnest("j") if unnest else out


def assert_same(rows, dtype, unnest=True, **kw):
    """decode=dtype gives the text route's frame and schema, or raises where it raises."""
    df = pl.DataFrame({"j": rows}, schema={"j": pl.String})
    try:
        expected = text_route(df, dtype, unnest, **kw)
    except Exception as e:
        with raises(type(e)):
            df.genson.normalise_json("j", decode=dtype, unnest=unnest, **kw)
        return
    before = len(DECODES)
    got = df.genson.normalise_json("j", decode=dtype, unnest=unnest, **kw)
    for taken in ROUTES:
        taken.append("text" if len(DECODES) > before else "typed")
    assert got.schema == expected.schema
    assert got.equals(expected)
    # Stricter than equals: a null struct is not a struct of nulls
    assert got.to_dicts() == expected.to_dicts()
    if not unnest:
        assert got["j"].is_null().to_list() == expected["j"].is_null().to_list()


def inferred(rows, map_encoding="kv", **kw) -> pl.Struct:
    """The dtype the rows' inferred schema converts to."""
    df = pl.DataFrame({"j": rows}, schema={"j": pl.String})
    inference = {k: v for k, v in kw.items() if k != "empty_as_null"}
    return pl.Struct(
        df.genson.infer_polars_schema(
            "j", avro=True, map_encoding=map_encoding, **inference
        )
    )


ABSENT = [
    pl.String,
    pl.Int64,
    pl.Boolean,
    pl.List(pl.String),
    pl.Struct({"z": pl.Int64}),
    pl.List(pl.Struct({"z": pl.String, "y": pl.List(pl.Int64)})),
]


def reshaped(dtype: pl.DataType, rng: random.Random) -> pl.DataType:
    """`dtype` with fields dropped, added and reordered, at any depth, leaves kept.

    The fields added are absent from the data: a dtype the typed rows project onto.
    """
    if isinstance(dtype, pl.Struct):
        fields = [
            pl.Field(f.name, reshaped(f.dtype, rng))
            for f in dtype.fields
            if rng.random() < 0.7
        ]
        for i in range(rng.randrange(3)):
            fields.append(pl.Field(f"absent{i}", rng.choice(ABSENT)))
        rng.shuffle(fields)
        return pl.Struct(fields or [pl.Field("absent", pl.String)])
    if isinstance(dtype, pl.List):
        return pl.List(reshaped(dtype.inner, rng))
    return dtype


def leaf_changed(dtype: pl.DataType) -> pl.DataType | None:
    """`dtype` with its first scalar leaf given another dtype (a fallback), or None."""
    if isinstance(dtype, pl.Struct):
        for i, f in enumerate(dtype.fields):
            changed = leaf_changed(f.dtype)
            if changed is not None:
                fields = list(dtype.fields)
                fields[i] = pl.Field(f.name, changed)
                return pl.Struct(fields)
        return None
    if isinstance(dtype, pl.List):
        changed = leaf_changed(dtype.inner)
        return None if changed is None else pl.List(changed)
    if isinstance(dtype, (pl.Map, pl.Array, pl.Null)):
        return None
    return pl.String if dtype != pl.String else pl.Int64


CASES = [
    (
        ['{"m": {"k%d": %d}}' % (i, i) for i in range(30)]
        + ['{"m": {}}', '{"m": null}', "{}"],
        {"map_threshold": 5},
    ),
    (
        [
            '{"v": 1}',
            '{"v": "s"}',
            '{"v": {"x": 1}}',
            '{"v": [1, 2]}',
            '{"v": null}',
            '{"v": 1.5}',
        ],
        {},
    ),
    (['{"n": "42", "b": "true"}', '{"n": 7, "b": false}'], {"coerce_strings": True}),
    (
        ['{"s": ""}', '{"s": "x"}', '{"t": {}}', '{"t": {"u": 1}}'],
        {"empty_as_null": False},
    ),
    (['{"x": 123456789012345678901234567890}', '{"x": 1}'], {}),
    (['{"a": 1}', '{"a": 2, "c": "z"}'], {"wrap_root": "root"}),
    (
        [
            '{"a": 1, "b": {"x": "y"}}',
            None,
            "",
            '{"a": 2}',
            "   ",
            '{"b": {"x": "z", "w": [1, 2]}}',
        ],
        {},
    ),
    (['{"a": 1, "n": null}', '{"a": 2, "n": null}', "{}"], {}),
]
CASE_IDS = [
    "maps",
    "promoted scalars",
    "coerced strings",
    "empty kept",
    "big integer",
    "wrap_root",
    "null and empty rows",
    "null-only field",
]


@mark.parametrize("map_encoding", ["mapping", "kv"])
@mark.parametrize("unnest", [True, False])
@mark.parametrize("rows,kw", CASES, ids=CASE_IDS)
def test_cases_inferred_dtype(rows, kw, map_encoding, unnest, routes):
    """The inferred dtype itself: the typed rows as they are."""
    assert_same(rows, inferred(rows, map_encoding, **kw), unnest, **kw)
    assert routes == ["typed"]


@mark.parametrize("seed", range(5))
@mark.parametrize("map_encoding", ["mapping", "kv"])
@mark.parametrize("rows,kw", CASES, ids=CASE_IDS)
def test_cases_reshaped_dtype(rows, kw, map_encoding, seed, routes):
    """Fields dropped, absent from the data, and reordered: projected."""
    dtype = reshaped(inferred(rows, map_encoding, **kw), random.Random(seed))
    assert_same(rows, dtype, seed % 2 == 0, **kw)
    assert routes == ["typed"]


@mark.parametrize("map_encoding", ["mapping", "kv"])
@mark.parametrize("rows,kw", CASES, ids=CASE_IDS)
def test_cases_leaf_changed(rows, kw, map_encoding, routes):
    """A leaf dtype other than the inferred one: decoded from JSON text instead."""
    dtype = leaf_changed(inferred(rows, map_encoding, **kw))
    if dtype is None:
        return
    assert_same(rows, dtype, **kw)
    assert routes in (["text"], [])


def test_null_only_field_as_scalar(routes):
    """A field only ever null: cast to a scalar dtype, from JSON text to a nested one."""
    rows = ['{"a": 1, "n": null}', '{"a": 2}']
    assert_same(rows, pl.Struct({"a": pl.Int64, "n": pl.String}))
    assert_same(rows, pl.Struct({"a": pl.Int64, "n": pl.Struct({"x": pl.Int64})}))
    assert routes == ["typed", "text"]


@mark.parametrize("seed", range(3))
def test_claims_fixture(seed, routes):
    """Wikidata claims, onto reshaped dtypes."""
    rows = (DATA / "claims_fixture_x4.jsonl").read_text().splitlines()
    rng = random.Random(seed)
    dtype = reshaped(inferred(rows, "kv", **WIKIDATA), rng)
    assert_same(rows, dtype, **WIKIDATA)
    assert routes == ["typed"]


@mark.parametrize("seed", range(60))
def test_random_rows(seed, routes):
    """Random rows onto their inferred dtype, a reshaped one and a leaf-changed one."""
    rng = random.Random(seed)
    rows = random_rows(rng, rng.randrange(1, 60))
    kw = {"map_threshold": rng.choice([0, 2, 20]), "unify_maps": rng.random() < 0.5}
    map_encoding = rng.choice(["mapping", "kv"])
    try:
        dtype = inferred(rows, map_encoding, **kw)
        text_route(pl.DataFrame({"j": rows}, schema={"j": pl.String}), dtype, **kw)
    except Exception:
        # Rows the text route itself cannot decode (#238, a root with no fields) are out of
        # scope here
        return
    unnest = rng.random() < 0.5
    assert_same(rows, dtype, unnest, **kw)
    assert_same(rows, reshaped(dtype, rng), unnest, **kw)
    assert routes == ["typed", "typed"]
    if (changed := leaf_changed(dtype)) is not None:
        assert_same(rows, changed, unnest, **kw)
        assert routes in (["typed", "typed", "text"], ["typed", "typed"])


SITELINK = {"site": pl.String, "title": pl.String, "badges": pl.List(pl.String)}
SITELINKS = {
    "ndjson": True,
    "wrap_root": "sitelinks",
    "force_field_types": {"sitelinks": "map"},
    "max_builders": 100,
}


def sitelinks_rows(rng: random.Random, n: int) -> list[str | None]:
    """Rows of sitelinks: a map of site to {site, title, badges, url}."""
    rows = []
    for _ in range(n):
        if rng.random() < 0.05:
            rows.append(None)
            continue
        links = {}
        for site in rng.sample(["enwiki", "dewiki", "frwiki", "jawiki", "eswiki"], 3):
            if rng.random() < 0.6:
                badges = rng.sample(["Q17437796", "Q17437798"], rng.randrange(3))
                links[site] = {
                    "site": site,
                    "title": f"T{rng.randrange(100)}",
                    "badges": badges,
                    "url": f"https://{site}.example/{rng.randrange(100)}",
                }
        rows.append(json.dumps(links))
    return rows


@mark.parametrize("seed", range(6))
@mark.parametrize(
    "value",
    [
        SITELINK,
        {"site": pl.String, "title": pl.String},
        {"title": pl.String, "site": pl.String, "extra": pl.Struct({"z": pl.String})},
    ],
    ids=["release", "no badges", "absent field"],
)
def test_sitelinks(seed, value, routes):
    """wikidata-pq's sitelinks: a forced map, decoded to {key, value} entries.

    The values keep some of the data's fields (`url` dropped), or ones the data lacks.
    """
    rows = sitelinks_rows(random.Random(seed), 40)
    dtype = pl.Struct(
        {"sitelinks": pl.List(pl.Struct({"key": pl.String, "value": pl.Struct(value)}))}
    )
    assert_same(rows, dtype, **SITELINKS)
    assert_same(rows, dtype, False, **SITELINKS)
    assert routes == ["typed", "typed"]


def test_sitelinks_leaf_changed(routes):
    """A sitelink's title asked for as an integer: decoded from JSON text."""
    rows = sitelinks_rows(random.Random(0), 20)
    value = {"site": pl.String, "title": pl.Int64}
    dtype = pl.Struct(
        {"sitelinks": pl.List(pl.Struct({"key": pl.String, "value": pl.Struct(value)}))}
    )
    assert_same(rows, dtype, **SITELINKS)
    assert routes in (["text"], [])


NEVER_TYPED_ROWS = [
    '{"a": 1, "s": "x", "t": "2024-01-02T03:04:05", "l": [1, 2], "n": [{"a": 3}]}',
    '{"a": 2, "s": "y", "t": "2024-05-06T07:08:09", "l": [3, 4], "n": []}',
    None,
]


@mark.parametrize(
    "dtype",
    [
        pl.Struct({"a": pl.Int32, "s": pl.String}),
        pl.Struct({"s": pl.Categorical()}),
        pl.Struct({"t": pl.Datetime("us")}),
        pl.Struct({"t": pl.Date}),
        pl.Struct({"a": pl.Float32}),
        pl.Struct({"a": pl.UInt8}),
        pl.Struct({"a": pl.Decimal(10, 2)}),
        pl.Struct({"s": pl.Enum(["x", "y"])}),
        pl.Struct({"s": pl.Binary}),
        pl.Struct({"l": pl.Array(pl.Int64, 2)}),
        pl.Struct({"n": pl.List(pl.Struct({"a": pl.Int16}))}),
    ],
    ids=lambda d: str(d),
)
def test_never_typed_dtype_skips_typed_route(dtype, monkeypatch):
    """A dtype holding one the typed route never makes: from JSON text, typed route unrun.

    Run first, the typed route would only be declined after it, then paid for twice.
    """
    assert not typed_route_makes(dtype)

    def typed(*args):
        raise AssertionError("the typed route ran")

    monkeypatch.setattr(polars_genson, "_rust_normalise_json_typed", typed)
    assert_same(NEVER_TYPED_ROWS, dtype)
    assert_same(NEVER_TYPED_ROWS, dtype, False)


def test_typed_route_makes():
    """The dtypes avro_to_polars_schema gives, at any depth, and no others."""
    kv = pl.List(pl.Struct({"key": pl.String, "value": pl.Int64}))
    assert typed_route_makes(
        pl.Struct(
            {
                "s": pl.String,
                "i": pl.Int64,
                "f": pl.Float64,
                "b": pl.Boolean,
                "n": pl.Null,
                "l": pl.List(pl.Struct({"x": pl.List(pl.String)})),
                "kv": kv,
                "m": pl.Map(pl.String, pl.Struct({"y": pl.Float64})),
            }
        )
    )
    assert not typed_route_makes(pl.Struct({"m": pl.Map(pl.Int64, pl.String)}))
    assert not typed_route_makes(pl.Struct({"l": pl.List(pl.List(pl.Int32))}))
    assert not typed_route_makes(pl.Struct({"o": pl.Object}))
