"""normalise_from_parquet(schema=...): typed output in a target schema's shape.

It is checked against the inferred schema, and holds the same rows as Polars makes of
the untargeted output when it scans it with that schema (missing struct fields inserted)
and unnests it.
"""

import orjson
import polars as pl
import pyarrow.parquet as pq
import pytest
from polars_genson import (
    avro_to_polars_schema,
    check_schema_compatibility,
    normalise_from_parquet,
    read_parquet_metadata,
)

ROWS = [
    {"claims": {"P1": [{"mainsnak": {"value": "a", "n": 1}, "refs": [{"p": "x"}]}]}},
    {"claims": {"P2": [{"mainsnak": {"value": "b"}, "refs": []}]}},
    None,
    {"claims": {}},
]
# Fields the rows never have (`mainsnak.hash`, `refs[].q`, `rank`), as a chunk lacks
# fields that another chunk had
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


def conformed_by_polars(source, tmp_path) -> pl.DataFrame:
    """Conform as wikidata-pq does today.

    The untargeted output, scanned with the target schema (missing struct fields
    inserted, extra ones ignored) and unnested.
    """
    out = tmp_path / "plain.parquet"
    normalise_from_parquet(input_path=source, output_path=out, **KW)
    return (
        pl.scan_parquet(
            out,
            schema={"id": pl.String, "claims": pl.Struct(TARGET)},
            cast_options=pl.ScanCastOptions(
                missing_struct_fields="insert", extra_struct_fields="ignore"
            ),
        )
        .unnest("claims")
        .collect()
    )


def test_target_schema_unnested_equals_polars_conformance(source, tmp_path):
    """The target-schema output, unnested, is the frame Polars makes of the plain output."""
    out = tmp_path / "target.parquet"
    normalise_from_parquet(
        input_path=source, output_path=out, schema=TARGET, unnest=True, **KW
    )
    got = pl.read_parquet(out)
    expected = conformed_by_polars(source, tmp_path)
    assert got.schema == expected.schema
    assert got.equals(expected)
    assert got.schema == pl.Schema({"id": pl.String, **TARGET})


def test_target_schema_file_is_laid_out_as_polars_writes_it(source, tmp_path):
    """The file's Parquet columns and Arrow schema are those Polars writes for the frame.

    Strings and lists have 64-bit offsets, and list items are named `element`.
    """
    out, by_polars = tmp_path / "target.parquet", tmp_path / "polars.parquet"
    normalise_from_parquet(
        input_path=source, output_path=out, schema=TARGET, unnest=True, **KW
    )
    conformed_by_polars(source, tmp_path).write_parquet(by_polars)
    a, b = pq.ParquetFile(out), pq.ParquetFile(by_polars)
    assert a.schema_arrow.remove_metadata() == b.schema_arrow.remove_metadata()

    def columns(f):
        return [
            (
                c.path,
                c.physical_type,
                str(c.logical_type),
                c.max_definition_level,
                c.max_repetition_level,
            )
            for c in (f.schema.column(i) for i in range(len(f.schema)))
        ]

    assert columns(a) == columns(b)
    assert "claims.list.element.value.list.element.mainsnak.hash" in {
        c[0] for c in columns(a)
    }


def test_target_schema_struct_column(source, tmp_path):
    """Without unnest, the output column is one struct of the target's dtype."""
    out = tmp_path / "target.parquet"
    normalise_from_parquet(input_path=source, output_path=out, schema=TARGET, **KW)
    got = pl.read_parquet(out)
    assert got.schema == pl.Schema({"id": pl.String, "claims": pl.Struct(TARGET)})
    assert got.unnest("claims").equals(conformed_by_polars(source, tmp_path))


def test_metadata_keeps_the_inferred_schema(source, tmp_path):
    """The file's metadata is the inferred schema, not the target's.

    So drift can still be checked from it.
    """
    out, plain = tmp_path / "target.parquet", tmp_path / "plain.parquet"
    normalise_from_parquet(input_path=source, output_path=out, schema=TARGET, **KW)
    normalise_from_parquet(input_path=source, output_path=plain, **KW)
    meta = read_parquet_metadata(out)["genson_avro_schema"]
    assert meta == read_parquet_metadata(plain)["genson_avro_schema"]
    inferred = pl.Schema(avro_to_polars_schema(meta, map_encoding="kv"))
    assert inferred == pl.read_parquet_schema(plain)["claims"].to_schema()
    assert inferred != TARGET


def test_a_field_the_target_lacks_is_rejected_and_nothing_written(source, tmp_path):
    """A field the target schema lacks raises, naming it, before any file is written."""
    narrow = pl.Schema(
        {
            "claims": pl.List(
                pl.Struct(
                    {
                        "key": pl.String,
                        "value": pl.List(
                            pl.Struct(
                                {
                                    "mainsnak": pl.Struct({"value": pl.String}),
                                    "refs": STMT.to_schema()["refs"],
                                }
                            )
                        ),
                    }
                )
            )
        }
    )
    out = tmp_path / "target.parquet"
    with pytest.raises(ValueError, match=r"claims\[\]\.value\[\]\.mainsnak\.n"):
        normalise_from_parquet(input_path=source, output_path=out, schema=narrow, **KW)
    assert not out.exists()


def test_schema_check_none_drops_fields_the_target_lacks(source, tmp_path):
    """Under schema_check="none" the target's fields are written whatever the rows hold."""
    narrow = pl.Schema(
        {"claims": pl.List(pl.Struct({"key": pl.String, "value": pl.Null}))}
    )
    out = tmp_path / "target.parquet"
    normalise_from_parquet(
        input_path=source,
        output_path=out,
        schema=narrow,
        schema_check="none",
        unnest=True,
        **KW,
    )
    got = pl.read_parquet(out)
    assert got.schema == pl.Schema({"id": pl.String, **narrow})
    assert got["claims"].to_list()[0] == [{"key": "P1", "value": None}]


def test_schema_check_strict_rejects_missing_fields(source, tmp_path):
    """Under schema_check="strict" a field the rows lack is an error."""
    out = tmp_path / "target.parquet"
    with pytest.raises(ValueError, match="strict"):
        normalise_from_parquet(
            input_path=source,
            output_path=out,
            schema=TARGET,
            schema_check="strict",
            **KW,
        )
    assert not out.exists()


def test_schema_needs_typed_output(source, tmp_path):
    """A target schema is for typed output only."""
    kw = {**KW, "typed": False}
    with pytest.raises(ValueError, match="typed=True"):
        normalise_from_parquet(
            input_path=source, output_path=tmp_path / "o.parquet", schema=TARGET, **kw
        )


def test_a_field_never_seen_with_a_value_takes_the_target_type(tmp_path):
    """A field null in every row (inferred Null) is written with the target's dtype."""
    path = tmp_path / "in.parquet"
    pl.DataFrame({"j": ['{"a": null, "b": 1}', '{"a": null, "b": 2}']}).write_parquet(
        path
    )
    out = tmp_path / "o.parquet"
    target = pl.Schema({"a": pl.String, "b": pl.Int64})
    normalise_from_parquet(
        input_path=path,
        column="j",
        output_path=out,
        typed=True,
        schema=target,
        unnest=True,
    )
    got = pl.read_parquet(out)
    assert got.schema == target
    assert got["b"].to_list() == [1, 2]


# The cases of wikidata-pq's is_acceptable_diff (DeepDiff of schema_to_dict outputs)
STORED = pl.Schema(
    {
        "r": pl.Struct({"x": pl.String, "y": pl.Int64}),
        "l": pl.List(pl.Struct({"p": pl.String, "q": pl.String})),
    }
)


@pytest.mark.parametrize(
    "inferred",
    [
        pl.Schema({"r": pl.Struct({"x": pl.String})}),
        pl.Schema({"r": pl.Null}),
        pl.Schema({"l": pl.List(pl.Struct({"p": pl.String}))}),
        pl.Schema({"l": pl.List(pl.Null)}),
        pl.Schema({"l": pl.Null}),
        STORED,
    ],
)
def test_compatible_as_is_acceptable_diff_accepts(inferred):
    """Missing fields and nulls fit, as wikidata-pq's is_acceptable_diff accepts them."""
    assert check_schema_compatibility(inferred, STORED) == []


@pytest.mark.parametrize(
    "inferred, problem",
    [
        (
            pl.Schema({"r": pl.Struct({"x": pl.String, "z": pl.Int64})}),
            "r.z: a field the target schema lacks (Int64)",
        ),
        (
            pl.Schema({"l": pl.List(pl.Struct({"p": pl.String, "z": pl.Int64}))}),
            "l[].z: a field the target schema lacks (Int64)",
        ),
        (
            pl.Schema({"r": pl.Struct({"y": pl.Float64})}),
            "r.y: Float64, the target has Int64",
        ),
        (pl.Schema({"r": pl.String}), "r: String, the target has Struct"),
    ],
)
def test_compatible_as_is_acceptable_diff_rejects(inferred, problem):
    """Extra fields and other dtypes do not, as is_acceptable_diff rejects them."""
    assert check_schema_compatibility(inferred, STORED) == [problem]


def test_a_null_stored_field_takes_only_null():
    """A stored field typed Null (never seen with a value) takes no value."""
    stored = pl.Schema({"altitude": pl.Null})
    assert check_schema_compatibility(pl.Schema({"altitude": pl.Null}), stored) == []
    assert check_schema_compatibility(pl.Schema({"altitude": pl.Float64}), stored) == [
        "altitude: Float64, the target has Null"
    ]


MAP_ROWS = [
    {
        "labels": {"en": "Alice", "fr": "Alice"},
        "props": {"p1": {"v": 1}, "p2": {"v": 2}},
    },
    {"labels": {"de": "Bob"}, "props": {"p3": {"v": 3}}},
    None,
    {"labels": {}, "props": {}},
]


@pytest.mark.parametrize("unnest", [True, False])
@pytest.mark.parametrize(
    "target",
    [
        pl.Schema(
            {
                "labels": pl.Map(pl.String, pl.String),
                "props": pl.Map(pl.String, pl.Struct({"v": pl.Int64})),
            }
        ),
        pl.Schema(
            {
                "labels": pl.Map(pl.String, pl.String),
                "props": pl.Map(pl.String, pl.Struct({"v": pl.Int64, "w": pl.String})),
            }
        ),
    ],
    ids=["same values", "value struct with a field the rows lack"],
)
def test_map_targets_are_laid_out_as_polars_writes_them(tmp_path, target, unnest):
    """A target schema with Map dtypes is written as Polars writes the frame.

    The output has the target's dtypes and the rows' values (fields the map values lack
    as nulls), and the Parquet layout Polars writes for the same frame.
    """
    path = tmp_path / "in.parquet"
    text = [None if r is None else orjson.dumps(r).decode() for r in MAP_ROWS]
    pl.DataFrame({"id": ["a", "b", "c", "d"], "j": text}).write_parquet(path)
    out, by_polars = tmp_path / "out.parquet", tmp_path / "polars.parquet"
    normalise_from_parquet(
        input_path=path,
        column="j",
        output_path=out,
        force_field_types={"labels": "map", "props": "map"},
        typed=True,
        keep_columns=["id"],
        schema=target,
        unnest=unnest,
    )
    got = pl.read_parquet(out)
    frame = got if unnest else got.unnest("j")
    assert frame.schema == pl.Schema({"id": pl.String, **target})
    fill = {f: None for f in target["props"].value.to_schema()}
    expected = [
        {
            k: None
            if r is None
            else (
                r[k] if k == "labels" else {p: {**fill, **v} for p, v in r[k].items()}
            )
            for k in target
        }
        for r in MAP_ROWS
    ]
    rows = frame.drop("id").to_dicts()
    assert [
        {k: (dict(sorted(v.items())) if v else v) for k, v in r.items()}
        for r in rows[:2]
    ] == expected[:2]
    assert rows[2] == {"labels": None, "props": None}
    got.write_parquet(by_polars)
    a, b = pq.ParquetFile(out), pq.ParquetFile(by_polars)
    assert a.schema_arrow.remove_metadata() == b.schema_arrow.remove_metadata()

    def columns(f):
        return [
            (
                c.path,
                c.physical_type,
                str(c.logical_type),
                c.max_definition_level,
                c.max_repetition_level,
            )
            for c in (f.schema.column(i) for i in range(len(f.schema)))
        ]

    assert columns(a) == columns(b)
