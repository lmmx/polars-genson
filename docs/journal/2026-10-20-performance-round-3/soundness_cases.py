"""The soundness gaps left open by campaign 2 (#267), each through every Python route:
the inferred JSON and Polars schemas, and normalise_json with decode False and True.
A row's scalars (its leaves, by value) must all survive normalisation: a leaf missing from
the normalised row is reported as LOST.

Usage: python soundness_cases.py [case ...]
"""

import json
import sys
import traceback

import polars as pl
import polars_genson  # noqa: F401

CASES = {
    "mixed scalars": ['{"a": 1}', '{"a": "x"}'],
    "mixed scalars in a list": ['{"a": [1, "x"]}', '{"a": [2]}'],
    "mixed scalars, float and int": ['{"a": 1}', '{"a": 1.5}'],
    "mixed scalars, bool and int": ['{"a": true}', '{"a": 3}'],
    "nullable untyped list": ['{"a": []}', '{"a": null}'],
    "untyped list only": ['{"a": []}', '{"a": []}'],
    "untyped list and missing": ['{"a": []}', "{}"],
    "maps typed differently": [
        '{"m": {"k%d": %d}}' % (i, i) for i in range(25)
    ] + ['{"m": {"s%d": "v"}}' % i for i in range(25)],
    "maps typed differently, forced": ['{"m": {"k": 1}}', '{"m": {"s": "v"}}'],
    "map with nullable values": ['{"m": {"k%d": %s}}' % (i, "null" if i % 2 else i) for i in range(30)],
    "map with nullable values, forced": ['{"m": {"k": 1, "j": null}}', '{"m": {"q": 2}}'],
    "record or scalar": ['{"a": {"b": 1}}', '{"a": 5}'],
    "list or scalar": ['{"a": [1, 2]}', '{"a": 5}'],
    "list of records or scalars": ['{"a": [{"b": 1}, 3]}'],
    "nested empty record": ['{"a": {}}', '{"a": {"b": 1}}'],
    "empty record only": ['{"a": {}}'],
    "null only": ['{"a": null}'],
    "deep mixed": ['{"a": [[1], ["x"], [{"b": null}]]}'],
    "keys needing escapes": ['{"a\\"b": 1, "c\\\\d": "e\\nf"}'],
    "unicode keys": ['{"ü": 1, "😀": [true]}'],
    "integer beyond i64": ['{"a": 18446744073709551615}', '{"a": 1}'],
    "float exponent": ['{"a": 1e300}', '{"a": -0.0}'],
    "empty record or scalar": ['{"c": {}}', '{"c": 3}'],
    "empty record or scalar in a list": ['{"a": [{}, 0]}'],
    "list of nulls or scalar": ['{"a": [null]}', '{"a": 3}'],
    "list of nulls or record": ['{"d": [null]}', '{"d": {"b": [0]}}'],
    "list of records or record of lists": ['{"b": [[{"a": 3}]]}', '{"b": {"a": [{"c": 0}]}}'],
    "scalar or list of empty records": ['{"a": -7}', '{"a": [{}]}'],
}
FORCED = {"maps typed differently, forced": {"m": "map"}, "map with nullable values, forced": {"m": "map"}}


def leaves(v, out):
    if isinstance(v, dict):
        for x in v.values():
            leaves(x, out)
    elif isinstance(v, list):
        for x in v:
            leaves(x, out)
    elif v is not None:
        out.append(json.dumps(v) if not isinstance(v, bool) else str(v).lower())
    return out


def norm_leaf(s):
    try:
        x = json.loads(s)
    except Exception:  # noqa: BLE001
        return s
    if isinstance(x, (int, float)) and not isinstance(x, bool):
        return repr(float(x))
    return json.dumps(x)


def lost(row_in, row_out):
    have = [norm_leaf(x) for x in leaves(row_out, [])]
    miss = []
    for x in leaves(row_in, []):
        k = norm_leaf(x)
        if k in have:
            have.remove(k)
        else:
            miss.append(x)
    return miss


def run(name, rows):
    kw = {}
    if name in FORCED:
        kw["force_field_types"] = FORCED[name]
    df = pl.DataFrame({"j": rows})
    print(f"== {name}: {rows[:3]}{' ...' if len(rows) > 3 else ''}")
    for label, f in [
        ("json schema", lambda: json.dumps(df.genson.infer_json_schema("j", **kw)["properties"])),
        ("polars schema", lambda: str(df.genson.infer_polars_schema("j", **kw))),
        ("avro schema", lambda: json.dumps(df.genson.infer_json_schema("j", avro=True, **kw).get("fields"))),
    ]:
        try:
            print(f"  {label}: {f()[:300]}")
        except Exception as e:  # noqa: BLE001
            print(f"  {label}: ERROR {type(e).__name__}: {str(e).splitlines()[0][:200]}")
    for decode in (False, True):
        try:
            out = df.genson.normalise_json("j", decode=decode, unnest=False, **kw)
            s = out if isinstance(out, pl.Series) else out.to_series()
            vals = [json.loads(v) if v is not None else None for v in s.to_list()] if not decode else s.to_list()
            losses = [(i, lost(json.loads(r), vals[i])) for i, r in enumerate(rows)]
            losses = [(i, m) for i, m in losses if m]
            tag = f"LOST {losses[:3]}" if losses else "ok"
            print(f"  decode={decode} dtype={s.dtype if decode else 'String'} {tag}")
            print(f"    row0 -> {json.dumps(vals[0], default=str)[:240]}")
        except Exception as e:  # noqa: BLE001
            msg = str(e).splitlines()
            print(f"  decode={decode}: ERROR {type(e).__name__}: {' | '.join(msg[:2])[:240]}")


if __name__ == "__main__":
    for name in sys.argv[1:] or CASES:
        try:
            run(name, CASES[name])
        except Exception:  # noqa: BLE001
            traceback.print_exc()
