"""PROTOTYPE (measurement only): debug macros evaluate their arguments only when debugging.

`debug!` and `debug_verbose!` expand to `$cfg.debug(format_args!(...))`; the arguments of
`format_args!` are evaluated at the call site, before `debug` checks `self.debug`, so a
`serde_json::to_string(schema)` argument serialises the schema on every call even with
debugging off. The prototype checks `$cfg.debug` before building the arguments.

Usage: python proto_lazy_debug.py <checkout>
"""

import sys

p = sys.argv[1] + "/genson-core/src/schema/core.rs"
with open(p) as f:
    s = f.read()
for name in ("debug", "debug_verbose"):
    old = f"""        $cfg.{name}(format_args!($($arg)*))
"""
    new = f"""        if $cfg.debug {{
            $cfg.{name}(format_args!($($arg)*))
        }}
"""
    assert s.count(old) == 1, name
    s = s.replace(old, new)
with open(p, "w") as f:
    f.write(s)
print("proto_lazy_debug applied")
