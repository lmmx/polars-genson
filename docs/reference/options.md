# Options

Every optional parameter across the main functions, grouped by what it affects. The
defaults, the functions that accept each option, and the descriptions are read from
polars-genson itself when these docs are built, so they match the released package.

Function key: **ij** `infer_json_schema`, **ip** `infer_polars_schema`, **nj**
`normalise_json` (the `df.genson` methods), **ifp** `infer_from_parquet`, **nfp**
`normalise_from_parquet`.

```python exec="on"
import inspect
import re
import polars_genson
from polars_genson import GensonNamespace

FUNCTIONS = {
    "ij": GensonNamespace.infer_json_schema,
    "ip": GensonNamespace.infer_polars_schema,
    "nj": GensonNamespace.normalise_json,
    "ifp": polars_genson.infer_from_parquet,
    "nfp": polars_genson.normalise_from_parquet,
}
GROUPS = [
    ("Input framing", ["ndjson", "ignore_outer_array", "wrap_root"], None),
    ("Maps and records", ["map_threshold", "map_max_required_keys", "force_field_types",
      "force_parent_field_types", "unify_maps", "no_unify", "no_root_map"],
     ("Maps and records", "../concepts/maps-and-records.md")),
    ("Mixed types", ["wrap_scalars", "force_scalar_promotion", "coerce_strings"],
     ("Mixed types", "../concepts/mixed-types.md")),
    ("Normalised output", ["empty_as_null", "map_encoding", "decode", "unnest"],
     ("Normalise a JSON column", "../guides/normalise.md")),
    ("Schema output", ["avro", "merge_schemas", "schema_uri"],
     ("Infer a schema", "../guides/infer-schema.md")),
    ("Parquet output", ["output_path", "output_column", "typed", "keep_columns",
      "extract_invariants", "lookup_output_path"],
     ("Normalise JSON stored in Parquet", "../guides/parquet.md")),
    ("Performance and diagnostics", ["max_builders", "profile", "debug", "verbosity"],
     ("Process large inputs", "../guides/large-inputs.md")),
]

def param_docs(fn):
    """Map each parameter name to the first sentence of its numpy docstring entry."""
    doc = inspect.getdoc(fn) or ""
    out, name, lines = {}, None, []
    section = doc.split("Parameters\n----------\n", 1)[-1].split("\n\nReturns", 1)[0]
    section = section.split("\n\nExamples", 1)[0]
    for line in section.splitlines():
        m = re.match(r"^(\w+)\s*:", line)
        if m and not line.startswith(" "):
            if name:
                out[name] = " ".join(lines)
            name, lines = m.group(1), []
        elif name and line.strip():
            lines.append(line.strip())
    if name:
        out[name] = " ".join(lines)
    def first_sentence(text):
        text = text.split(" - ", 1)[0]  # stop where a bulleted list starts
        return re.split(r"(?<!e\.g\.)(?<!i\.e\.)(?<=\.)\s", text, maxsplit=1)[0]
    return {k: first_sentence(v) for k, v in out.items() if v}

defaults, where, docs = {}, {}, {}
for key, fn in FUNCTIONS.items():
    pd = param_docs(fn)
    for p in inspect.signature(fn).parameters.values():
        if p.name == "self" or p.default is inspect.Parameter.empty:
            continue
        defaults.setdefault(p.name, {})[key] = p.default
        where.setdefault(p.name, []).append(key)
        if p.name in pd and p.name not in docs:
            docs[p.name] = pd[p.name]

grouped = {name for _, names, _ in GROUPS for name in names}
missing = sorted(set(defaults) - grouped)
assert not missing, f"options not placed in a group: {missing}"

def fmt_default(name):
    values = defaults[name]
    distinct = {repr(v) for v in values.values()}
    if len(distinct) == 1:
        return f"`{next(iter(distinct))}`"
    return "<br>".join(f"`{v!r}` ({k})" for k, v in values.items())

for title, names, link in GROUPS:
    print(f"## {title}\n")
    if link:
        print(f"See [{link[0]}]({link[1]}).\n")
    print("| Option | Default | Functions | Description |")
    print("|---|---|---|---|")
    for name in names:
        desc = docs.get(name, "").replace("|", "\\|")
        print(f"| `{name}` | {fmt_default(name)} | {', '.join(where[name])} | {desc} |")
    print()
```
