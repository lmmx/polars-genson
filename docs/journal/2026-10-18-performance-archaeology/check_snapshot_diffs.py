"""Check that changed insta snapshots differ from HEAD only in `required` and nullability.

For each modified `.snap` under the given directory, parses the snapshot body at HEAD and
in the working tree (one JSON document, or one per line) and walks both together. A
difference is explained when it is one of:

- `required`: entries removed (the rest in the same order), or the list removed;
- a type made nullable: `T` -> `["null", T]` (JSON Schema `type`, or an Avro union), or
  a schema -> `{"anyOf": [{"type": "null"}, schema]}`, with `"default": null` allowed
  beside a type made nullable; an Avro `"null"` made optional is written `["null"]`, as
  the Avro conversion writes any optional field of type null;
- normalised data: an object whose leaves are all null -> null.

Anything else (other values, keys added, removed or reordered, list lengths) is printed as
unexplained. Exits 1 if any difference is unexplained.

Usage: python check_snapshot_diffs.py <snapshots dir>
"""

import collections
import json
import subprocess
import sys

snap_dir = sys.argv[1]
changed = subprocess.run(
    ["git", "diff", "--name-only", "HEAD", "--", snap_dir],
    capture_output=True,
    text=True,
    check=True,
).stdout.split()


def body(text):
    """A snapshot's body: what follows its `---` header."""
    parts = text.split("---\n", 2)
    return parts[2] if len(parts) == 3 and parts[0] == "" else text


def parse(text):
    try:
        return [json.loads(text)]
    except json.JSONDecodeError:
        lines = [line for line in text.splitlines() if line.strip()]
        return [json.loads(line) for line in lines]


def all_null(v):
    if v is None:
        return True
    return isinstance(v, dict) and all(all_null(x) for x in v.values())


def nullable_of(new):
    """The schema `new` makes nullable, if it is a nullable form of one."""
    if isinstance(new, list) and len(new) == 2 and new[0] == "null":
        return True, new[1]
    if isinstance(new, dict) and list(new) == ["anyOf"]:
        branches = new["anyOf"]
        if len(branches) == 2 and branches[0] == {"type": "null"}:
            return True, branches[1]
    return False, None


def compare(old, new, path, kinds, bad):
    if old == new and json.dumps(old) == json.dumps(new):
        return
    if old == "null" and new == ["null"]:
        kinds["null type made optional"] += 1
        return
    made_nullable, inner = nullable_of(new)
    if made_nullable and not nullable_of(old)[0]:
        kinds["made nullable"] += 1
        compare(old, inner, path + "<nullable>", kinds, bad)
        return
    if isinstance(old, dict) and new is None and all_null(old):
        kinds["record of nulls -> null"] += 1
        return
    if isinstance(old, dict) and isinstance(new, dict):
        extra = [k for k in new if k not in old]
        if extra == ["default"] and new["default"] is None:
            kinds["default null added"] += 1
        elif extra:
            bad.append(f"{path}: keys added {extra}")
        missing = [k for k in old if k not in new and k != "required"]
        if missing:
            bad.append(f"{path}: keys removed {missing}")
        order_old = [k for k in old if k in new]
        order_new = [k for k in new if k in old]
        if order_old != order_new:
            bad.append(f"{path}: keys reordered {order_old} -> {order_new}")
        for k in order_old:
            if k == "required":
                compare_required(old[k], new[k], f"{path}.required", kinds, bad)
            else:
                compare(old[k], new[k], f"{path}.{k}", kinds, bad)
        if "required" in old and "required" not in new:
            kinds["required removed"] += 1
        return
    if isinstance(old, list) and isinstance(new, list) and len(old) == len(new):
        for i, (a, b) in enumerate(zip(old, new)):
            compare(a, b, f"{path}[{i}]", kinds, bad)
        return
    bad.append(f"{path}: {json.dumps(old)[:120]} -> {json.dumps(new)[:120]}")


def compare_required(old, new, path, kinds, bad):
    it = iter(old)
    if all(any(name == o for o in it) for name in new):
        kinds["required entries removed"] += len(old) - len(new)
    else:
        bad.append(f"{path}: {old} -> {new}")


total_bad = 0
summary = collections.Counter()
for name in changed:
    head = subprocess.run(
        ["git", "show", f"HEAD:{name}"], capture_output=True, text=True, check=True
    ).stdout
    with open(name) as f:
        now = f.read()
    kinds, bad = collections.Counter(), []
    old_docs, new_docs = parse(body(head)), parse(body(now))
    if len(old_docs) != len(new_docs):
        bad.append(f"documents {len(old_docs)} -> {len(new_docs)}")
    for i, (a, b) in enumerate(zip(old_docs, new_docs)):
        compare(a, b, f"doc{i}", kinds, bad)
    summary.update(kinds)
    total_bad += len(bad)
    print(
        f"{name.rsplit('/', 1)[-1]}: {dict(kinds)}"
        + (f" UNEXPLAINED {len(bad)}" if bad else "")
    )
    for line in bad[:10]:
        print("   ", line)
print(
    f"\n{len(changed)} snapshots; explained: {dict(summary)}; unexplained: {total_bad}"
)
sys.exit(1 if total_bad else 0)
