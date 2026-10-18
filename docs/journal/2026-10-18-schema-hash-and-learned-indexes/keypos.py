"""Simulate genson-core's per-row builder walk and measure how predictable property lookups are.

For each row a fresh tree of nodes is built (as `process_json_strings_parallel` gives each
string its own builder). Every object visit looks each key up in the node's insertion-ordered
property map. For each lookup of a key the node already holds, we record whether simple
"models" of the key's position would have predicted it:

- cursor: index of the previous key in this object visit + 1 (first key: index 0)
- same:   the key's position within this object
Then we report hit rates, the node-size distribution of lookups, and the cost of `required`.

Usage: python keypos.py <file.ndjson> [rows]
"""

import json
import sys
from collections import Counter


class Node:
    __slots__ = ("items", "keys", "props", "scalar")

    def __init__(self):
        self.props = None  # dict key -> (index, Node)
        self.keys = None  # list of keys in insertion order
        self.items = None
        self.scalar = False


stats = Counter()
size_hist = Counter()
width_hist = Counter()


def bucket(n):
    b = 1
    while b < n:
        b *= 4
    return b


def visit(node, v):
    if isinstance(v, dict):
        if node.props is None:
            node.props, node.keys = {}, []
        stats["objects"] += 1
        width_hist[bucket(len(v))] += 1
        prev = -1
        for j, (k, sub) in enumerate(v.items()):
            stats["lookups"] += 1
            found = node.props.get(k)
            if found is None:
                stats["inserts"] += 1
                idx = len(node.keys)
                child = Node()
                node.props[k] = (idx, child)
                node.keys.append(k)
            else:
                idx, child = found
                stats["hits"] += 1
                size_hist[bucket(len(node.keys))] += 1
                if idx == prev + 1:
                    stats["cursor_hit"] += 1
                elif idx > prev:
                    stats["forward_skip"] += 1
                    stats["forward_skip_dist"] += idx - prev - 1
                else:
                    stats["backward"] += 1
                if idx == j:
                    stats["same_hit"] += 1
            prev = idx
            visit(child, sub)
    elif isinstance(v, list):
        if node.items is None:
            node.items = Node()
        for x in v:
            visit(node.items, x)
    elif node.props is None and node.items is None and not node.scalar:
        node.scalar = True


def count_nodes(node):
    """Count nodes, and those that only ever held scalars (scalar leaves)."""
    stats["nodes"] += 1
    stats["scalar_leaves"] += node.scalar and node.props is None and node.items is None
    for _, child in (node.props or {}).values():
        count_nodes(child)
    if node.items is not None:
        count_nodes(node.items)


def main(path, limit):
    with open(path) as f:
        for i, line in enumerate(f):
            if i >= limit:
                break
            root = Node()
            visit(root, {"claims": json.loads(line)})
            count_nodes(root)
            stats["rows"] += 1
    s = stats
    print(f"rows={s['rows']} objects={s['objects']:,} lookups={s['lookups']:,}")
    print(
        f"  inserts (new key at node): {s['inserts']:,} ({s['inserts'] / s['lookups']:.1%})"
    )
    print(
        f"  hits (key already held):   {s['hits']:,} ({s['hits'] / s['lookups']:.1%})"
    )
    h = s["hits"]
    print(
        f"  of hits: cursor model exact {s['cursor_hit'] / h:.2%}, same-position {s['same_hit'] / h:.2%}"
    )
    print(
        f"           forward skip {s['forward_skip'] / h:.2%} (mean dist {s['forward_skip_dist'] / max(1, s['forward_skip']):.1f}), backward {s['backward'] / h:.2%}"
    )
    print("  lookups (hits) by node size (<= bucket):", dict(sorted(size_hist.items())))
    print(
        f"  nodes in the row builders: {s['nodes']:,}, scalar leaves {s['scalar_leaves']:,} ({s['scalar_leaves'] / s['nodes']:.1%})"
    )
    print("  objects by width (<= bucket):", dict(sorted(width_hist.items())))


if __name__ == "__main__":
    main(sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else 10**9)
