"""PROTOTYPE (measurement only): `ListStrategy::add_object` never folds items in parallel.

An array of 10 or more items is folded with rayon (`par_iter().fold(...).reduce_with`)
inside a worker that is itself one of `process_json_strings_parallel`'s parallel tasks;
the prototype raises the boundary so every array is added item by item on the worker.

Usage: python proto_list_seq.py <checkout>
"""

import sys
from pathlib import Path

p = Path(sys.argv[1]) / "genson-core/src/genson_rs/strategy/array.rs"
s = p.read_text()
old = "const PARALLEL_PROCESSING_BOUNDARY: usize = 10;"
assert s.count(old) == 1
p.write_text(s.replace(old, "const PARALLEL_PROCESSING_BOUNDARY: usize = usize::MAX;"))
print("proto_list_seq applied")
