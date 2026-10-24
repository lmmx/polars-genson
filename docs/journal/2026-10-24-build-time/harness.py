# Applies the measurement harness to .github/workflows/CI.yml: no sccache (every build
# cold), cargo --timings, and the timing report uploaded per job.
import re, sys
p = sys.argv[1]
s = open(p).read()
s = re.sub(r"sccache: .*", "sccache: false", s)
s = s.replace("--find-interpreter --manifest-path polars-genson-py/Cargo.toml", "--find-interpreter --manifest-path polars-genson-py/Cargo.toml --timings")
for os_ in ["linux", "windows", "macos"]:
    old = f"""      - name: Upload wheels
        uses: actions/upload-artifact@v7
        if: matrix.platform.skip != true
        with:
          name: wheels-{os_}-${{{{ matrix.platform.target }}}}
          path: dist
"""
    assert old in s, os_
    new = old + f"""      - name: Upload timings
        uses: actions/upload-artifact@v7
        if: matrix.platform.skip != true
        with:
          name: timings-{os_}-${{{{ matrix.platform.target }}}}
          path: target/cargo-timings/cargo-timing.html
"""
    s = s.replace(old, new)
open(p, "w").write(s)
