"""How much of genson's per-row work is repeated: occurrences vs distinct values per level.

Builds each row's node tree the way a per-row builder does (properties in first-seen order,
list items merged into one node, scalar kinds merged with integer→number promotion,
`required` as the intersection of keys), then hash-conses the nodes bottom-up so equal
subtrees get equal ids. Reports, per level: key occurrences, leaves, nodes, rows.

Usage: python entropy.py <file.ndjson>
"""

import json
import sys
from collections import Counter


class Node:
    __slots__ = ("items", "kinds", "props", "required")

    def __init__(self):
        self.props = None
        self.items = None
        self.kinds = set()
        self.required = None


def kind(v):
    if v is None:
        return "null"
    if isinstance(v, bool):
        return "boolean"
    if isinstance(v, int):
        return "integer"
    if isinstance(v, float):
        return "number"
    return "string"


def visit(node, v, key_occ):
    if isinstance(v, dict):
        if node.props is None:
            node.props = {}
        keys = set(v)
        node.required = keys if node.required is None else node.required & keys
        for k, sub in v.items():
            key_occ[k] += 1
            child = node.props.get(k)
            if child is None:
                child = node.props[k] = Node()
            visit(child, sub, key_occ)
    elif isinstance(v, list):
        if node.items is None:
            node.items = Node()
        for x in v:
            visit(node.items, x, key_occ)
    else:
        node.kinds.add(kind(v))


table = {}


def shape(node, stats):
    """Hash-cons id of the node's schema (bottom-up)."""
    stats["nodes"] += 1
    kinds = set(node.kinds)
    if {"integer", "number"} <= kinds:
        kinds.discard("integer")
    if node.props is None and node.items is None:
        stats["leaves"] += 1
        stats["leaf_states"][tuple(sorted(kinds))] += 1
    props = (
        None
        if node.props is None
        else tuple((k, shape(c, stats)) for k, c in node.props.items())
    )
    items = None if node.items is None else shape(node.items, stats)
    req = None if node.required is None else tuple(sorted(node.required))
    key = (tuple(sorted(kinds)), props, items, req)
    sid = table.setdefault(key, len(table))
    stats["row_ids"].add(sid)
    return sid


def main(path):
    key_occ = Counter()
    stats = Counter()
    stats["leaf_states"] = Counter()
    row_ids_total = 0
    row_shapes = set()
    rows = 0
    with open(path) as f:
        for line in f:
            rows += 1
            root = Node()
            visit(root, {"claims": json.loads(line)}, key_occ)
            stats["row_ids"] = set()
            row_shapes.add(shape(root, stats))
            row_ids_total += len(stats["row_ids"])
    print(f"rows: {rows:,} occurrences, {len(row_shapes):,} distinct row schemas")
    print(f"keys: {sum(key_occ.values()):,} occurrences, {len(key_occ):,} distinct")
    print(
        f"nodes (row builders): {stats['nodes']:,}; distinct subtrees within a row, summed: {row_ids_total:,}; distinct across corpus: {len(table):,}"
    )
    print(
        f"leaves: {stats['leaves']:,}; distinct leaf states: {len(stats['leaf_states'])} -> {dict(stats['leaf_states'].most_common())}"
    )
    top = key_occ.most_common(1)[0]
    print(f"most frequent key: {top[0]!r} x {top[1]:,}")


if __name__ == "__main__":
    main(sys.argv[1])
