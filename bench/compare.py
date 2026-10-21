#!/usr/bin/env python3
"""Compare polars-genson builds on the workloads of `workloads.py`, in fresh processes.

    compare.py [--rounds N] [--workloads W,W,...] [SPEC ...]

A SPEC is a release on PyPI (`1.2.1`) or a checkout of this repository (`..`), whose
`polars-genson-py` is built into the environment. With no SPEC, the two newest releases
are compared. The first SPEC is the baseline the others are compared with.

Each SPEC gets its own environment under `.envs/` (Python 3.13, `polars-genson[polars]`),
kept between invocations; a checkout is rebuilt every time. Then, for each workload:

- one warm-up run per SPEC, discarded (the first run of a sequence reads its input from
  disk and was up to twice as slow on the machine these were made on);
- N rounds of one run per SPEC, each a fresh process, the SPECs' order rotated from round
  to round so that neither always runs first.

Each run times the operation alone: wall time, user and system CPU time of all threads,
and the process's peak memory. User CPU time is the work the code does; wall time is what
a user waits, and the one that shows a change in parallelism. On a virtual machine, wall
and system time also carry the host's cost of backing the guest's memory: on the one
these were made on, the same normalisation of labels took 0.12 to 2.9 s of system time
(and 0.61 to 2.6 s of wall time) from one process to the next, while its user time stayed
within 1.5 to 1.8 s.

The report gives each SPEC's median and range, and judges user CPU and wall time
separately: a SPEC is called faster or slower than the baseline only if its range and the
baseline's do not overlap, otherwise "within noise". Two sets of 5 runs from the same
distribution are disjoint with probability 2/252 (0.8%).

Every comparison is appended to `history/runs.jsonl`, one line per SPEC and workload.

Standard library only: run it with any Python 3.
"""

import argparse
import datetime
import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
ENVS = HERE / ".envs"
DATA = HERE / "data"
HISTORY = HERE / "history" / "runs.jsonl"
PYTHON = "3.13"

sys.path.insert(0, str(HERE))
from workloads import CLAIMS_ROWS, FLAT_ROWS, WORKLOADS  # noqa: E402

CLAIMS = DATA / f"claims_{CLAIMS_ROWS}.parquet"
FLAT = DATA / f"flat_{FLAT_ROWS}.parquet"
# uv's exclude-newer (if configured) would hide a release made in the last few days
UV_ENV = {**os.environ, "UV_EXCLUDE_NEWER": "false"}
UV_FRESH = [
    "--exclude-newer-package",
    "polars=P0D",
    "--exclude-newer-package",
    "polars-genson=P0D",
]


def sh(cmd, **kw):
    return subprocess.run(
        cmd, check=True, text=True, capture_output=True, env=UV_ENV, **kw
    ).stdout


def releases(n: int) -> list[str]:
    """The `n` newest releases on PyPI, oldest first."""
    url = "https://pypi.org/simple/polars-genson/"
    req = urllib.request.Request(
        url, headers={"Accept": "application/vnd.pypi.simple.v1+json"}
    )
    with urllib.request.urlopen(req) as response:
        versions = json.load(response)["versions"]

    def key(v):
        return tuple(int(p) for p in v.split("."))

    stable = [v for v in versions if all(p.isdigit() for p in v.split("."))]
    return sorted(stable, key=key)[-n:]


class Spec:
    def __init__(self, text: str):
        self.text = text
        path = Path(text)
        self.checkout = (path / "polars-genson-py").is_dir() and path.resolve()
        if self.checkout:
            sha = sh(["git", "rev-parse", "--short", "HEAD"], cwd=self.checkout).strip()
            dirty = sh(
                ["git", "status", "--porcelain", "--untracked-files=no"],
                cwd=self.checkout,
            ).strip()
            self.label = f"{sha}{'+dirty' if dirty else ''}"
            self.env = ENVS / f"checkout-{self.checkout.name}"
        else:
            self.label = text
            self.env = ENVS / text
        self.python = self.env / "bin" / "python"

    def install(self):
        if self.checkout:
            if not self.python.exists():
                sh(["uv", "venv", "-q", "--python", PYTHON, str(self.env)])
            print(
                f"building {self.text} ({self.label}) into {self.env.name}",
                file=sys.stderr,
            )
            sh(
                [
                    "uv",
                    "pip",
                    "install",
                    "-q",
                    "--python",
                    str(self.python),
                    "--reinstall-package",
                    "polars-genson",
                    f"{self.checkout / 'polars-genson-py'}[polars]",
                ]
            )
        elif not self.python.exists():
            print(
                f"installing polars-genson {self.text} into {self.env.name}",
                file=sys.stderr,
            )
            sh(["uv", "venv", "-q", "--python", PYTHON, str(self.env)])
            try:
                sh(
                    [
                        "uv",
                        "pip",
                        "install",
                        "-q",
                        "--python",
                        str(self.python),
                        *UV_FRESH,
                        f"polars-genson[polars]=={self.text}",
                    ]
                )
            except subprocess.CalledProcessError:
                shutil.rmtree(self.env)
                raise
        code = "import importlib.metadata as m; print(m.version('polars-genson'), m.version('polars'))"
        self.genson, self.polars = sh([str(self.python), "-c", code]).split()

    def run(self, workload: str) -> dict | None:
        p = subprocess.run(
            [str(self.python), str(HERE / "workloads.py"), workload, str(DATA)],
            text=True,
            capture_output=True,
        )
        if p.returncode:
            # an older release without the API the workload calls
            self.error = p.stderr.strip().splitlines()[-1]
            return None
        return json.loads(p.stdout.splitlines()[-1])


def prepare(spec: Spec):
    if not CLAIMS.exists():
        print(f"writing {CLAIMS.relative_to(HERE)}", file=sys.stderr)
        sh(
            [
                str(spec.python),
                str(HERE / "claims_rows.py"),
                str(CLAIMS_ROWS),
                str(CLAIMS),
                "0",
            ]
        )
    if not FLAT.exists():
        print(f"writing {FLAT.relative_to(HERE)}", file=sys.stderr)
        sh([str(spec.python), str(HERE / "workloads.py"), "flat", str(DATA)])


METRICS = ["user", "wall", "sys", "peak_mb"]


def summary(xs: list[float]) -> str:
    return f"{statistics.median(xs):.2f} ({min(xs):.2f}–{max(xs):.2f})"


def verdict(xs: list[float], base: list[float]) -> str:
    if max(xs) < min(base):
        return f"faster ×{statistics.median(base) / statistics.median(xs):.2f}"
    if min(xs) > max(base):
        return f"slower ×{statistics.median(xs) / statistics.median(base):.2f}"
    return "within noise"


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument(
        "specs",
        nargs="*",
        help="PyPI versions or checkout paths (default: the two newest releases)",
    )
    ap.add_argument("--rounds", type=int, default=5)
    ap.add_argument(
        "--workloads",
        type=lambda s: s.split(","),
        default=WORKLOADS,
        help=f"comma-separated, of {','.join(WORKLOADS)} (default: all)",
    )
    args = ap.parse_args()
    if unknown := set(args.workloads) - set(WORKLOADS):
        ap.error(f"unknown workloads: {', '.join(sorted(unknown))}")

    specs = [Spec(s) for s in (args.specs or releases(2))]
    for spec in specs:
        spec.install()
    prepare(specs[0])
    if len({s.polars for s in specs}) > 1:
        print(
            "warning: the SPECs run different Polars versions:",
            ", ".join(f"{s.label} {s.polars}" for s in specs),
            file=sys.stderr,
        )

    cpus = len(os.sched_getaffinity(0))
    load = os.getloadavg()[0]
    if load > cpus / 4:
        print(
            f"warning: load average {load:.1f} on {cpus} CPUs: other work will add noise",
            file=sys.stderr,
        )

    date = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds")
    host = {
        "cpus": cpus,
        "load_before": round(load, 2),
        "machine": platform.machine(),
        "python": PYTHON,
    }
    results = {}  # (spec index, workload) -> {metric: [one value per round]}
    for w in args.workloads:
        print(f"{w}: ", end="", file=sys.stderr, flush=True)
        for spec in specs:
            spec.run(w)
        for r in range(args.rounds):
            order = specs[r % len(specs) :] + specs[: r % len(specs)]
            for spec in order:
                t = spec.run(w)
                if t is not None:
                    times = results.setdefault(
                        (specs.index(spec), w), {m: [] for m in METRICS}
                    )
                    for m in METRICS:
                        times[m].append(round(t[m], 3))
            print(".", end="", file=sys.stderr, flush=True)
        print(file=sys.stderr)

    print(
        f"\n{args.rounds} rounds, {cpus} CPUs, load average {load:.1f} before; "
        f"seconds (memory in MB), median (min–max); against {specs[0].label}\n"
    )
    print(
        "| workload | polars-genson | Polars | user CPU | wall | system CPU | peak memory "
        "| user CPU against baseline | wall against baseline |"
    )
    print("|---|---|---|---|---|---|---|---|---|")
    HISTORY.parent.mkdir(exist_ok=True)
    with HISTORY.open("a") as history:
        for w in args.workloads:
            base = results.get((0, w))
            for i, spec in enumerate(specs):
                t = results.get((i, w))
                if t is None:
                    print(
                        f"| {w} | {spec.label} | {spec.polars} | n/a | n/a | n/a | n/a | {spec.error} | |"
                    )
                    continue
                if i == 0:
                    against = "baseline | baseline"
                elif base:
                    against = " | ".join(
                        verdict(t[m], base[m]) for m in ("user", "wall")
                    )
                else:
                    against = " | "
                cells = " | ".join(
                    summary(t[m])
                    if m != "peak_mb"
                    else f"{statistics.median(t[m]):.0f}"
                    for m in METRICS
                )
                print(f"| {w} | {spec.label} | {spec.polars} | {cells} | {against} |")
                record = {
                    "date": date,
                    "workload": w,
                    "spec": spec.text,
                    "genson": spec.genson,
                    "build": spec.label,
                    "polars": spec.polars,
                    "baseline": specs[0].label,
                    **t,
                    **host,
                }
                history.write(json.dumps(record) + "\n")


if __name__ == "__main__":
    main()
