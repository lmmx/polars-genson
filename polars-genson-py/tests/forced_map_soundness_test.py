# tests/forced_map_soundness_test.py
"""Every value survives normalisation with fields forced to be maps.

Each case's rows are normalised (decode=False and decode=True), and every scalar in the
rows must be found in the output (as `soundness_cases.lost` in the round-3 journal checks
it): a value turned into null is lost. The cases are the shrunk failures of the journal's
fuzz_lossless.py, one per shape. Those polars-genson does not yet keep are marked xfail
(strict, so a fix shows as a pass to be moved up).
"""

import json

import polars as pl
import polars_genson  # noqa: F401
import pytest

FORCED = {"force_field_types": {"a": "map", "c": "map"}}


def leaves(value, out):
    """The scalars in `value`, as `(type, value)`, with numbers compared as numbers."""
    if isinstance(value, dict):
        for v in value.values():
            leaves(v, out)
    elif isinstance(value, list):
        for v in value:
            leaves(v, out)
    elif value is not None:
        out.append(
            ("number", float(value))
            if type(value) in (int, float)
            else (type(value).__name__, value)
        )
    return out


def lost(row_in, row_out):
    """The scalars of `row_in` not found in `row_out`."""
    have = leaves(row_out, [])
    missing = []
    for leaf in leaves(row_in, []):
        if leaf in have:
            have.remove(leaf)
        else:
            missing.append(leaf)
    return missing


def assert_kept(rows, **options):
    """Every row's scalars survive normalisation, on the text route and the typed one."""
    df = pl.DataFrame({"j": [json.dumps(r) for r in rows]})
    text = df.genson.normalise_json("j", decode=False, **options).to_list()
    typed = (
        df.genson.normalise_json("j", decode=True, unnest=False, **options)
        .to_series()
        .to_list()
    )
    for route, out in [("text", [json.loads(t) for t in text]), ("typed", typed)]:
        for row_in, row_out in zip(rows, out):
            assert not lost(row_in, row_out), f"{route}: {row_in} -> {row_out}"


KEPT = {
    # A field forced to be a map that holds a list: its values were `{"type": "array"}`,
    # so every item was written null
    "list": [{"a": [-7]}],
    "list of strings": [{"a": ["s"]}],
    "nested lists": [{"a": [[""]]}],
    "list of records": [{"a": [{"d": 1}]}],
    "list under a record": [{"b": {"a": [-7]}}],
    "list beside a map": [{"a": [-7]}, {"a": {"x": 1}}],
    # A forced map in a record in one row and in a list's records in another, with values
    # that differ: the two maps were left apart, and the list's maps written as the record's
    "record beside a list": [{"d": {"a": 1}}, {"d": [{"a": "s"}]}],
    "record beside a list, list values": [{"d": {"a": []}}, {"d": [{"a": True}]}],
    "record beside a list, nested": [{"b": [{"c": {"a": -7}}]}, {"b": {"c": ["42"]}}],
    # Values that are lists of kinds that clash: unified once rewritten, not text
    "lists of clashing kinds": [{"d": {"c": {"a": ["s"], "c": [-7, "s"]}}}],
    # A field forced within a forced map's values: forced before the values are unified
    # (the map's own keys are not fields, though they may share its name)
    "forced within values": [{"a": {"b": {"c": []}, "d": {"c": {"b": 1}}}}],
    "forced lists within values": [{"a": {"c": {"a": [False, 1]}, "d": {"a": ["42"]}}}],
    "forced within values, nested": [
        {"c": [{"c": {"c": -7}}]},
        {"c": {"c": {"c": []}}},
    ],
    "a key named as the field": [{"a": -7}, {"a": {"a": 0}}],
}

# Not yet kept: written as JSON text (no value is turned into null, but its type is lost)
KNOWN_GAPS = {
    # A forced map's values: a list of nulls in one row, a list of records in another
    "list of nulls beside a list of records": (
        [{"c": {"x": [None]}}, {"c": [{"x": [0]}]}],
        FORCED,
    ),
    # Not forced: inferred maps, a record beside a list of records whose maps differ
    "inferred maps beside a list": (
        [{"b": {"b": ["s"]}}, {"b": [{"b": ["", 2.5], "d": {}}]}],
        {"map_threshold": 0, "unify_maps": True},
    ),
}


@pytest.mark.parametrize("rows", KEPT.values(), ids=KEPT.keys())
def test_forced_maps_keep_every_value(rows):
    """No value is lost with fields forced to be maps."""
    assert_kept(rows, **FORCED)


@pytest.mark.xfail(strict=True, reason="values written as JSON text")
@pytest.mark.parametrize("rows,options", KNOWN_GAPS.values(), ids=KNOWN_GAPS.keys())
def test_known_gaps_keep_every_value(rows, options):
    """Shapes whose values are not yet all kept (each written as JSON text)."""
    assert_kept(rows, **options)
