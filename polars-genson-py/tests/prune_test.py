"""Tests for pruning records by named fields in normalise_from_parquet."""

import json
from pathlib import Path

import polars as pl
import pytest
from polars_genson import normalise_from_parquet

# One row per kind of snak on a deleted property, shared with genson-cli's prune tests
FIXTURE = Path(__file__).parents[2] / "genson-cli/tests/data/claims_prune.jsonl"

PRUNE = {"mainsnak__string", "P450__string", "value", "error"}

CLAIMS_OPTIONS = {
    "ndjson": True,
    "map_threshold": 0,
    "unify_maps": True,
    "force_field_types": {"mainsnak": "record"},
    "force_scalar_promotion": {"mainsnak", "datavalue"},
    "no_unify": {"qualifiers"},
    "wrap_root": "claims",
    "keep_columns": ["id"],
    # A kv entry's `value` is not a record field, so pruning "value" must leave it
    "map_encoding": "kv",
}


@pytest.fixture
def claims_parquet(tmp_path):
    """The claims fixture as a Parquet file with an `id` per row."""
    lines = FIXTURE.read_text().splitlines()
    path = tmp_path / "claims.parquet"
    pl.DataFrame(
        {"id": [f"Q{i}" for i in range(len(lines))], "claims": lines}
    ).write_parquet(path)
    return path


def normalise(src, tmp_path, *, typed, prune=True):
    """Normalise the claims, returning the rows (as dicts) and the pruned table."""
    out = tmp_path / "out.parquet"
    pruned = tmp_path / "pruned.parquet"
    normalise_from_parquet(
        src,
        "claims",
        out,
        typed=typed,
        **CLAIMS_OPTIONS,
        **({"prune": PRUNE, "prune_output_path": pruned} if prune else {}),
    )
    df = pl.read_parquet(out)
    if typed:
        # The typed column is the wrap_root record, {"claims": ...}
        rows = [
            {"id": r["id"], "claims": r["claims"]["claims"] if r["claims"] else None}
            for r in df.to_dicts()
        ]
    else:
        rows = [
            {"id": id_, "claims": json.loads(s)["claims"]} for id_, s in df.iter_rows()
        ]
    return rows, (pl.read_parquet(pruned) if prune else None)


def has_field(v, name):
    """Whether a field `name` is anywhere in `v`."""
    if isinstance(v, dict):
        return name in v or any(has_field(x, name) for x in v.values())
    if isinstance(v, list):
        return any(has_field(x, name) for x in v)
    return False


def datavalues(v):
    """Every datavalue in `v`."""
    if isinstance(v, dict):
        found = [v["datavalue"]] if v.get("datavalue") is not None else []
        return found + [d for x in v.values() for d in datavalues(x)]
    if isinstance(v, list):
        return [d for x in v for d in datavalues(x)]
    return []


def statement(row, prop):
    """The first statement of the claim on `prop`, or None."""
    for entry in row["claims"] or []:
        if entry["key"] == prop:
            return entry["value"][0]
    return None


@pytest.mark.parametrize("typed", [False, True])
def test_prune_claims_bad_snaks(claims_parquet, tmp_path, typed):
    """Each kind of bad snak is pruned as the Wikidata claims card describes."""
    rows, _ = normalise(claims_parquet, tmp_path, typed=typed)
    assert len(rows) == 7
    for name in ["mainsnak__string", "P450__string"]:
        assert not any(has_field(r, name) for r in rows), name
    # In the kv map encoding every map entry has a `value`, so look in the datavalues
    for name in ["value", "error"]:
        assert not any(d.get(name) is not None for r in rows for d in datavalues(r))

    # Row 0 has no bad snak: its qualifier and reference are kept
    s = statement(rows[0], "P31")
    assert s["qualifiers"][0]["key"] == "P580"
    assert s["references"][0][0]["key"] == "P854"
    # Row 1: a bare string mainsnak takes its statement with it, the other is kept
    assert statement(rows[1], "P450") is None
    assert statement(rows[1], "P31") is not None
    # Row 2: a qualifier snak with a datavalue error goes, its sibling group stays
    assert [q["key"] for q in statement(rows[2], "P793")["qualifiers"]] == ["P580"]
    # Row 3: a bare string qualifier snak goes, leaving the qualifiers empty
    assert statement(rows[3], "P375")["qualifiers"] is None
    # Row 4: a reference emptied by pruning goes, the other reference stays
    refs = statement(rows[4], "P31")["references"]
    assert [[g["key"] for g in ref] for ref in refs] == [["P854"]]
    # Row 5: pruning the only statement empties the claims, the row is kept
    assert rows[5]["id"] == "Q5"
    assert rows[5]["claims"] is None
    # Row 6: a bare string reference snak goes, its sibling group stays
    refs = statement(rows[6], "P31")["references"]
    assert [[g["key"] for g in ref] for ref in refs] == [["P854"]]


@pytest.mark.parametrize("typed", [False, True])
def test_prune_output(claims_parquet, tmp_path, typed):
    """Each pruned value is recorded once, with its row's id, its path and its input."""
    _, pruned = normalise(claims_parquet, tmp_path, typed=typed)
    assert pruned.columns == ["id", "path", "value"]
    error = {"value": "Q2", "error": "property not found"}
    assert [
        (id_, json.loads(path), json.loads(value))
        for id_, path, value in pruned.iter_rows()
    ] == [
        ("Q1", ["claims", "P450", 0], {"mainsnak": "P450", "rank": "normal"}),
        (
            "Q2",
            ["claims", "P793", 0, "qualifiers", "P450", 0],
            {"property": "P450", "datavalue": error},
        ),
        ("Q3", ["claims", "P375", 0, "qualifiers", "P450", 0], "P450"),
        (
            "Q4",
            ["claims", "P31", 0, "references", 0, "P450", 0],
            {
                "property": "P450",
                "datavalue": {"value": "Q4", "error": "property not found"},
            },
        ),
        (
            "Q5",
            ["claims", "P450", 0],
            {
                "mainsnak": {
                    "property": "P450",
                    "datavalue": {"value": "Q6", "error": "property not found"},
                },
                "rank": "normal",
            },
        ),
        ("Q6", ["claims", "P31", 0, "references", 0, "P450", 0], "P450"),
    ]


def test_prune_schema(claims_parquet, tmp_path):
    """The pruned fields are left out of the typed output's schema."""
    out = tmp_path / "out.parquet"
    normalise_from_parquet(
        claims_parquet,
        "claims",
        out,
        typed=True,
        prune=PRUNE,
        prune_output_path=tmp_path / "pruned.parquet",
        **CLAIMS_OPTIONS,
    )
    schema = str(pl.read_parquet_schema(out)["claims"])
    for name in PRUNE - {"value"}:
        assert f"'{name}'" not in schema, name


def test_without_prune_bad_snaks_kept(claims_parquet, tmp_path):
    """Without prune the bad snaks are normalised like any other."""
    rows, _ = normalise(claims_parquet, tmp_path, typed=False, prune=False)
    assert any(has_field(r, "mainsnak__string") for r in rows)
    assert any(d.get("error") is not None for r in rows for d in datavalues(r))
    assert statement(rows[5], "P450") is not None


def test_prune_requires_output_path(claims_parquet, tmp_path):
    """Prune and prune_output_path must be given together."""
    with pytest.raises(ValueError, match="prune_output_path"):
        normalise_from_parquet(
            claims_parquet,
            "claims",
            tmp_path / "out.parquet",
            prune=PRUNE,
            **CLAIMS_OPTIONS,
        )
