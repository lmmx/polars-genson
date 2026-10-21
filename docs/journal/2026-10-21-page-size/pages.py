"""Summarise pagedump output per writer and column: pages, values per page, uncompressed
and compressed bytes, compression ratio, page kinds and encodings."""

import json
import statistics
import sys

for path in sys.argv[1:]:
    lines = [json.loads(line.replace('""', '"')) for line in open(path)]
    meta = lines[-1]
    pages = [entry for entry in lines if not entry.get("file")]
    print(
        path,
        {
            k: meta[k]
            for k in (
                "created_by",
                "version",
                "row_groups",
                "compression",
                "has_column_index",
                "stats",
            )
        },
    )
    for col in sorted({p["col"] for p in pages}):
        ps = [p for p in pages if p["col"] == col]
        data = [p for p in ps if p["kind"] != "dict"]
        dicts = [p for p in ps if p["kind"] == "dict"]
        unc = sum(p["uncompressed"] for p in data)
        comp = sum(p["compressed"] for p in data)
        vals = [p["values"] for p in data]
        print(
            f"  {col:16} data pages {len(data):5} ({','.join(sorted({p['kind'] for p in data}))}, {','.join(sorted({p['encoding'] for p in data}))})"
            f"  values/page med {statistics.median(vals):>8.0f}  uncompressed/page med {statistics.median(p['uncompressed'] for p in data) / 1e3:7.1f} kB"
            f"  data unc {unc / 1e6:7.2f} MB  comp {comp / 1e6:7.2f} MB  ratio {unc / comp:5.2f}"
            + (
                f"  dict pages {len(dicts)} {sum(p['compressed'] for p in dicts) / 1e6:.2f} MB"
                if dicts
                else ""
            )
        )
