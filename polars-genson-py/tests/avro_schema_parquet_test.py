"""normalise_from_parquet(avro_schema=...) decodes against a known schema where it can.

Every row that fits the schema strictly is decoded against it, nothing inferred;
otherwise the rows are normalised as without it. Either way the output is the same.
"""

import json

import orjson
import polars as pl
import pyarrow as pa
import pytest
from polars_genson import normalise_from_parquet

ROWS = [
    {"P1": [{"mainsnak": {"value": "a", "n": 1}, "refs": [{"p": "x"}]}]},
    {"P2": [{"mainsnak": {"value": "b"}, "refs": []}]},
    None,
    {},
]
# Another chunk's rows: more fields, an integer where the first rows had none
WIDER = [
    {"P3": [{"mainsnak": {"value": "c", "n": 2, "hash": "h"}, "refs": [{"p": "y"}]}]},
    {"P4": [{"mainsnak": {"value": "d"}, "rank": "normal"}]},
]
TARGET = pl.Schema(
    {
        "claims": pl.List(
            pl.Struct(
                {
                    "key": pl.String,
                    "value": pl.List(
                        pl.Struct(
                            {
                                "mainsnak": pl.Struct(
                                    {"hash": pl.String, "value": pl.String, "n": pl.Int64}
                                ),
                                "rank": pl.String,
                                "refs": pl.List(pl.Struct({"p": pl.String})),
                            }
                        )
                    ),
                }
            )
        )
    }
)
KW = dict(
    column="claims",
    output_path=None,
    output_format="arrow",
    output_column="claims",
    wrap_root="claims",
    force_field_types={"claims": "map"},
    map_encoding="kv",
    typed=True,
    keep_columns=["id"],
    schema=TARGET,
    profile=True,
)


def parquet(tmp_path, rows, name):
    path = tmp_path / f"{name}.parquet"
    text = [None if r is None else orjson.dumps(r).decode() for r in rows]
    ids = [f"Q{i}" for i in range(len(rows))]
    pl.DataFrame({"id": ids, "claims": text}).write_parquet(path)
    return path


def normalised(path, capfd, **kw) -> tuple[pa.Table, str]:
    table = pa.table(normalise_from_parquet(path, **{**KW, **kw}))
    return table, capfd.readouterr().err


def schema_of(path, capfd) -> str:
    """The Avro schema inferred from a file's rows, as genson records it."""
    table, _ = normalised(path, capfd)
    return table.schema.metadata[b"genson_avro_schema"].decode()


@pytest.fixture
def rows(tmp_path):
    return parquet(tmp_path, ROWS, "rows")


@pytest.fixture
def wider(tmp_path):
    return parquet(tmp_path, ROWS + WIDER, "wider")


def test_the_inferred_schema_decodes_without_inference(rows, capfd):
    inferred, log = normalised(rows, capfd)
    assert "infer" in log
    known, log = normalised(rows, capfd, avro_schema=schema_of(rows, capfd))
    assert "infer" not in log and "decode (avro_schema)" in log
    assert known.equals(inferred)


@pytest.mark.parametrize("as_dict", [False, True])
def test_a_wider_schema_decodes_the_same_rows(rows, wider, capfd, as_dict):
    """A schema inferred from more rows, with fields these lack."""
    inferred, _ = normalised(rows, capfd)
    schema = schema_of(wider, capfd)
    known, log = normalised(
        rows, capfd, avro_schema=json.loads(schema) if as_dict else schema
    )
    assert "infer" not in log
    assert known.equals(inferred)


def test_rows_that_do_not_fit_are_inferred(rows, wider, capfd):
    """A schema inferred from fewer rows: the rest do not fit it, so they are inferred."""
    inferred, _ = normalised(wider, capfd)
    known, log = normalised(wider, capfd, avro_schema=schema_of(rows, capfd))
    assert "do not all fit" in log and "infer" in log
    assert known.equals(inferred)


def test_a_schema_not_fitting_the_target_is_not_used(rows, tmp_path, capfd):
    """Rows of another shape than the target's: the schema check rejects them as before."""
    other = parquet(tmp_path, [{"P1": [{"mainsnak": {"value": 1}}]}], "other")
    with pytest.raises(ValueError, match="does not fit the target schema"):
        normalised(other, capfd)
    with pytest.raises(ValueError, match="does not fit the target schema"):
        normalised(other, capfd, avro_schema=schema_of(rows, capfd).replace('"string"', '"long"'))


def test_needs_typed_output(rows):
    with pytest.raises(ValueError, match="typed"):
        normalise_from_parquet(
            rows, "claims", rows.with_name("out.parquet"), avro_schema="{}"
        )


def test_invalid_schema_raises(rows, capfd):
    with pytest.raises(ValueError, match="avro_schema"):
        normalised(rows, capfd, avro_schema="{not json")


# Rows with label maps to extract (one with its key spelt with an escape), and rows without:
# one with a key given twice, which the extraction's scan would write again
LABELLED = [
    '{"P1": [{"mainsnak": {"value": "a", "id": "Q5", "labels": {"en": "five"}}}]}',
    '{"P2": [{"mainsnak": {"value": "b", "id": "Q5", "l\\u0061bels": {"en": "five"}}}]}',
    '{"P3": [{"mainsnak": {"value": "c", "n": 1, "n": 2}}]}',
    None,
    '{"P4": [{"mainsnak": {"value": "d"}, "rank": "normal"}]}',
]


@pytest.mark.parametrize("fits", [True, False])
def test_extracted_rows_decode_as_inferred(tmp_path, capfd, fits):
    """Rows the extraction leaves unscanned (no key can be a label map) are decoded as they
    are, with the same output and lookup as inference gives; where they do not all fit,
    they are scanned and inferred as before."""
    src = tmp_path / "labelled.parquet"
    ids = [f"Q{i}" for i in range(len(LABELLED))]
    pl.DataFrame({"id": ids, "claims": LABELLED}).write_parquet(src)
    kw = dict(extract_invariants={"labels": "id"}, force_field_types={"claims": "map"})

    def run(name, **extra):
        lookup = tmp_path / f"lookup_{name}.parquet"
        out, log = normalised(src, capfd, **kw, lookup_output_path=lookup, **extra)
        return out, pl.read_parquet(lookup), log

    inferred, lookup, _ = run("inferred")
    schema = inferred.schema.metadata[b"genson_avro_schema"].decode()
    if not fits:
        schema = schema.replace('{"name":"rank","type":["null","string"]},', "")
        assert '"rank"' not in schema
    known, known_lookup, log = run("known", avro_schema=schema)
    assert ("infer" in log) != fits
    assert known.equals(inferred)
    assert known_lookup.equals(lookup) and lookup.height == 1
