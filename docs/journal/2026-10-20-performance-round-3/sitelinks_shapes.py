import sys, json, collections
import polars as pl
def shape(v):
    if isinstance(v, dict): return ("o", tuple((k, shape(x)) for k, x in v.items()))
    if isinstance(v, list):
        s = sorted(set(shape(x) for x in v), key=repr)
        return ("a", tuple(s))
    if isinstance(v, bool): return "b"
    if isinstance(v, int): return "i"
    if isinstance(v, float): return "f"
    if v is None: return "n"
    return "s"
for path in sys.argv[1:]:
    col = pl.read_parquet(path, columns=["sitelinks"])["sitelinks"].to_list()

    rows = 0; links = 0; uniform = 0; distinct_per_row = []; allshapes = collections.Counter(); keysets=set(); rowshapes=set()
    for s in col:
        if not s: continue
        d = json.loads(s); rows += 1
        shapes = [shape(v) for v in d.values()]
        links += len(shapes)
        ds = set(shapes); distinct_per_row.append(len(ds))
        uniform += len(ds) <= 1
        allshapes.update(ds); keysets.add(tuple(d.keys())); rowshapes.add(shape(d))
    print(path.split("/")[-2:], rows, "rows", links, "links", round(links/max(rows,1),1), "per row;",
          uniform, "rows with one value shape;", len(allshapes), "distinct value shapes overall;",
          "max distinct/row", max(distinct_per_row or [0]), "; distinct row key sequences", len(keysets), "; distinct row shapes", len(rowshapes))
    for sh, c in allshapes.most_common(5): print("   ", c, str(sh)[:150])
