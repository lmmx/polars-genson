"""Where two tables of nested columns differ, walked in Arrow without copying: list
lengths and nulls, then the lists' items; struct nulls, then each field; leaves value by
value. Prints each differing path with counts and a few values.

Usage: python diff_claims.py <A.parquet> <B.parquet>
"""
import sys

import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.parquet as pq


def show(path, msg, a=None, b=None, idx=()):
    print(f"{path}: {msg}")
    for i in list(idx)[:4]:
        print("    ", str(a[i].as_py())[:200], "|", str(b[i].as_py())[:200])


def differ(a, b):
    eq = pc.equal(a, b)
    nulls_differ = pc.not_equal(pc.is_null(a), pc.is_null(b))
    return pc.indices_nonzero(pc.or_(pc.fill_null(pc.invert(eq), False), nulls_differ)).to_pylist()


def walk(path, a, b):
    if a.equals(b):
        return
    t = a.type
    if pa.types.is_struct(t):
        if (i := differ(pc.is_valid(a), pc.is_valid(b))):
            # On the side where such a row is valid, are all its fields null?
            ia, ib = pa.array(i, pa.int64()), None
            va, vb = pc.take(a, ia), pc.take(b, ia)
            valid = pc.if_else(pc.is_valid(va), 1, 0).to_pylist()
            empty = all(
                all(not v for v in pc.is_valid(pc.struct_field(side, [k])).to_pylist())
                for side in (va.filter(pc.is_valid(va)), vb.filter(pc.is_valid(vb)))
                for k in range(t.num_fields)
            )
            show(path, f"{len(i)} rows null on one side only: valid in A {sum(valid)}, in B {len(i) - sum(valid)}; "
                 f"every field null where valid: {empty}")
        for k in range(t.num_fields):
            walk(f"{path}.{t.field(k).name}", pc.struct_field(a, [k]), pc.struct_field(b, [k]))
    elif pa.types.is_large_list(t) or pa.types.is_list(t):
        la, lb = pc.list_value_length(a), pc.list_value_length(b)
        if (i := differ(la, lb)):
            show(path, f"{len(i)} rows with other lengths or nulls", a, b, i)
            return
        walk(f"{path}[]", a.flatten(), b.flatten())
    elif pa.types.is_null(t):
        if len(a) != len(b):
            show(path, f"{len(a)} vs {len(b)} values")
    else:
        i = differ(a, b)
        show(path, f"{len(i)} of {len(a)} values differ ({t})", a, b, i)


ta, tb = pq.read_table(sys.argv[1]), pq.read_table(sys.argv[2])
for col in ta.column_names:
    walk(col, ta[col].combine_chunks(), tb[col].combine_chunks())
