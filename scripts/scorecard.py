#!/usr/bin/env python3
"""genson-core's performance scorecard: where inference spends its instructions, time and
memory, stage by stage, and how a revision changes that against another.

    scorecard.py compare --base REV --head REV [--cumulative REV] [--out DIR]
    scorecard.py measure --rev REV [--out DIR]
    scorecard.py report BASE.json HEAD.json [--cumulative ROOT.json] [--md OUT.md]

`compare` measures both revisions (and a third to compare the head against, say the stack's
root) and writes `scorecard-<sha>.json` for each and `report.md`. Each revision is built
in its own worktree, with the workload of this checkout (`genson-core/examples/scorecard.rs`)
so that both run the same thing, and measured on the corpora that the workload generates:

- instructions: one callgrind run per corpus on one thread, by stage. A stage is the
  inclusive cost of `genson_core::phase::<stage>`; a revision without those markers is
  attributed by the functions the stages run (`LEGACY`), and the report says so. What no
  stage holds is shown as `unattributed`;
- time: the release build on all threads, base and head interleaved ABBA, each process
  timing a warm-up run and a measured one;
- allocations, bytes and peak live heap by stage: a debug build with the `alloc-stats`
  feature, on one thread (the counts do not depend on optimisation), where it exists;
- peak RSS from the timing runs, and a hash of the output (the schema, or the normalised
  rows), which must not change unless the change means it to.

Needs git, cargo, valgrind and callgrind_annotate. Standard library only.
"""

import argparse
import datetime
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKLOAD = ROOT / "genson-core" / "examples" / "scorecard.rs"
STAGES = ["rows", "merge", "write_schema", "rewrite", "materialise", "reorder", "avro", "normalise"]
# A revision without stage markers: the functions each stage runs (the largest inclusive
# cost among them), which inlining can hide, so the attribution is approximate
LEGACY = {
    "rows": [r"<genson_core::genson_rs::interned::RowStates>::row$",
             r"genson_core::schema::process_json_strings_parallel", r"genson_core::schema::process_json_strings_sequential"],
    "merge": [r"<genson_core::genson_rs::builder::SchemaBuilder>::add_interned_row_chunks"],
    "write_schema": [r"<genson_core::genson_rs::builder::SchemaBuilder>::to_dag$", r"<genson_core::genson_rs::node::SchemaNode>::to_dag$",
                     r"<genson_core::genson_rs::builder::SchemaBuilder>::to_schema$"],
    "rewrite": [r"<genson_core::schema::map_inference::on_dag::OnDag>::rewrite_objects$", r"genson_core::schema::map_inference::rewrite_objects$"],
    "materialise": [r"<genson_core::dag::Dag>::to_value$"],
    "reorder": [r"genson_core::schema::reorder_unions$"],
    "avro": [r"<genson_core::schema::core::SchemaInferenceResult>::to_avro_schema$"],
    "normalise": [r"genson_core::normalise::normalise_values_pruned"],
}
BUILD_ENV = {"CARGO_PROFILE_RELEASE_DEBUG": "line-tables-only", "CARGO_PROFILE_RELEASE_STRIP": "none"}
MARKER = "<!-- genson-core-scorecard -->"


def run(cmd, **kw):
    return subprocess.run(cmd, check=True, text=True, capture_output=True, **kw).stdout


def sha_of(rev):
    return run(["git", "rev-parse", rev], cwd=ROOT).strip()


def worktree(cache, sha):
    """A worktree of `sha` with this checkout's workload in it."""
    tree = cache / f"tree-{sha[:12]}"
    if not tree.exists():
        run(["git", "worktree", "add", "--detach", str(tree), sha], cwd=ROOT)
    shutil.copy(WORKLOAD, tree / "genson-core" / "examples" / "scorecard.rs")
    return tree


def build(tree, target, release, features):
    cmd = ["cargo", "build", "-q", "-p", "genson-core", "--example", "scorecard", "--features", ",".join(features)]
    if release:
        cmd.insert(2, "--release")
    env = dict(os.environ, CARGO_TARGET_DIR=str(target), **(BUILD_ENV if release else {}))
    subprocess.run(cmd, cwd=tree, env=env, check=True)
    return target / ("release" if release else "debug") / "examples" / "scorecard"


def workload(binary, corpus, runs, threads=None):
    env = dict(os.environ)
    if threads:
        env["RAYON_NUM_THREADS"] = str(threads)
    return json.loads(run([str(binary), corpus, "--runs", str(runs)], env=env))


def inclusive(callgrind_out):
    """Each function's inclusive instructions (the largest of its entries), and the total."""
    out = run(["callgrind_annotate", "--inclusive=yes", "--threshold=100", str(callgrind_out)])
    total = int(re.search(r"([\d,]+) \(100\.0%\)\s+PROGRAM TOTALS", out).group(1).replace(",", ""))
    costs = {}
    for line in out.splitlines():
        if "'2" in line:  # a recursive call's entry counts its callers again
            continue
        m = re.match(r"\s*([\d,]+) \([ \d.]+%\)\s+(?:\S+?:)?(\S.*?)(?: \[.*\])?$", line)
        if m:
            name, n = m.group(2), int(m.group(1).replace(",", ""))
            costs[name] = max(costs.get(name, 0), n)
    return total, costs


def stages_of(total, costs):
    """Instructions by stage: by marker where the revision has them, else `LEGACY`."""
    marked = {s: max([n for f, n in costs.items() if re.match(rf"genson_core::phase::{s}(::<|$)", f)] or [0]) for s in STAGES}
    if any(marked.values()):
        found, attribution = marked, "markers"
    else:
        found = {s: max([n for f, n in costs.items() for p in pats if re.search(p, f)] or [0]) for s, pats in LEGACY.items()}
        attribution = "legacy"
    stages = {s: n for s, n in found.items() if n}
    return {"total": total, "stages": stages, "unattributed": total - sum(stages.values())}, attribution


def instructions(binary, corpus, cache, sha):
    out = cache / f"callgrind.{sha[:12]}.{corpus}.out"
    subprocess.run(["valgrind", "--tool=callgrind", "--toggle-collect=scorecard::measured::on_the_pool*",
                    f"--callgrind-out-file={out}", str(binary), corpus, "--runs", "1"],
                   env=dict(os.environ, RAYON_NUM_THREADS="1"), check=True, capture_output=True)
    return stages_of(*inclusive(out))


def measure(rev, cache, timing_rounds=0):
    """A revision's builds, instructions and allocations (timings come from `compare`)."""
    sha = sha_of(rev)
    tree = worktree(cache, sha)
    has_stats = "alloc-stats" in (tree / "genson-core" / "Cargo.toml").read_text()
    release = build(tree, cache / f"target-{sha[:12]}", True, ["avro"])
    debug = build(tree, cache / f"target-{sha[:12]}", False, ["avro", "alloc-stats"]) if has_stats else None
    corpora = run([str(release), "--list"]).split()
    result = {"rev": rev, "sha": sha, "corpora": {}, "binary": str(release)}
    for corpus in corpora:
        print(f"[{sha[:8]}] {corpus}", file=sys.stderr)
        counted, attribution = instructions(release, corpus, cache, sha)
        one = workload(release, corpus, 1)
        entry = {k: one[k] for k in ("rows", "bytes", "processed", "schema_hash")}
        entry.update(instructions=counted, attribution=attribution, timing={"wall": [], "cpu": [], "peak_rss_kb": []})
        if debug:
            entry["allocations"] = workload(debug, corpus, 1, threads=1).get("stages", [])
        result["corpora"][corpus] = entry
    return result


def time_abba(results, rounds):
    """Each revision's timings, interleaved base-head-head-base, so that neither always runs
    second (which runs faster)."""
    for r in range(rounds):
        order = results if r % 2 == 0 else results[::-1]
        for res in order:
            for corpus, entry in res["corpora"].items():
                one = workload(res["binary"], corpus, 2)
                entry["timing"]["wall"].append(one["runs"][-1]["wall"])
                entry["timing"]["cpu"].append(one["runs"][-1]["cpu"])
                entry["timing"]["peak_rss_kb"].append(one["peak_rss_kb"])


def millions(n):
    return f"{n / 1e6:,.1f}M"


def change(a, b):
    if not a:
        return "new" if b else "–"
    return f"{(b - a) / a * 100:+.1f}%"


def quartiles(xs):
    xs = sorted(xs)
    if len(xs) < 4:
        return xs[0], statistics.median(xs), xs[-1]
    q = statistics.quantiles(xs, n=4)
    return q[0], q[1], q[2]


def timed(a, b, instructions_moved=True):
    """The change in a median time, and whether it is clear of the runs' spread. A time that
    moves while the instructions do not is noise, so it is inconclusive too."""
    if not a or not b:
        return "–"
    (a25, am, a75), (b25, bm, b75) = quartiles(a), quartiles(b)
    verdict = "faster" if b75 < a25 else "slower" if b25 > a75 else "inconclusive"
    pct = (bm - am) / am * 100
    if abs(pct) < 3 or not instructions_moved:
        verdict = "inconclusive"
    return f"{am * 1e3:.1f} → {bm * 1e3:.1f} ms ({pct:+.0f}%, {verdict})"


def alloc_total(entry, key):
    return sum(s[key] for s in entry.get("allocations", []))


def allocations(b, h):
    """The allocations cell: the change, or what was not collected and why."""
    if b.get("allocations") and h.get("allocations"):
        return change(alloc_total(b, "allocations"), alloc_total(h, "allocations"))
    if h.get("allocations"):
        return f"{alloc_total(h, 'allocations'):,} (base not collected: it predates `alloc-stats`)"
    return "not collected (predates `alloc-stats`)"


def report(base, head, cumulative=None, run_url=None):
    lines = [MARKER, "## genson-core performance scorecard", ""]
    when = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
    lines.append(f"base `{base['sha']}` → head `{head['sha']}`, measured {when}" + (f" ([run]({run_url}))" if run_url else "") + ".")
    lines.append("")
    attributions = {e["attribution"] for res in (base, head) for e in res["corpora"].values()}
    lines.append(
        "Instructions: one thread, callgrind, by stage"
        + (" (a revision without stage markers is attributed by function name, approximately)" if "legacy" in attributions else "")
        + f". Times: release build, all threads, {len(next(iter(head['corpora'].values()))['timing']['wall'])} runs each, interleaved ABBA, median and verdict against the runs' spread (inconclusive unless the instructions moved too)."
    )
    lines += ["", "| corpus | instructions | per byte | wall | CPU | allocations | output |", "|---|---|---|---|---|---|---|"]
    gains, losses, remaining = [], [], []
    for corpus, h in head["corpora"].items():
        b = base["corpora"].get(corpus)
        if not b:
            continue
        bt, ht = b["instructions"]["total"], h["instructions"]["total"]
        per_byte = f"{bt / b['bytes']:.1f} → {ht / h['bytes']:.1f}"
        allocs = allocations(b, h)
        same = b["schema_hash"] == h["schema_hash"] and b["processed"] == h["processed"]
        moved = abs(ht - bt) > 0.005 * bt
        wall, cpu = timed(b["timing"]["wall"], h["timing"]["wall"], moved), timed(b["timing"]["cpu"], h["timing"]["cpu"], moved)
        lines.append(f"| {corpus} | {millions(bt)} → {millions(ht)} ({change(bt, ht)}) | {per_byte} | {wall} | {cpu} | {allocs} | {'identical' if same else '**changed**'} |")
        # Stages are compared only when both revisions mark them: one attributed by function
        # name puts what inlining hides under `unattributed`
        if b["attribution"] != "markers" or h["attribution"] != "markers":
            remaining += [(h["instructions"]["stages"].get(s, 0) / ht if ht else 0, corpus, s, h["instructions"]["stages"].get(s, 0)) for s in STAGES]
            continue
        for stage in STAGES + ["unattributed"]:
            bs = b["instructions"]["stages"].get(stage, 0) if stage != "unattributed" else b["instructions"]["unattributed"]
            hs = h["instructions"]["stages"].get(stage, 0) if stage != "unattributed" else h["instructions"]["unattributed"]
            delta = hs - bs
            if abs(delta) > max(1e6, 0.01 * bt):
                (gains if delta < 0 else losses).append((delta, corpus, stage, bs, hs))
            remaining.append((hs / ht if ht else 0, corpus, stage, hs))
    lines.append("")
    if "legacy" in attributions:
        lines.append("**Stage changes:** not compared, as a revision predates the stage markers and is attributed by function name; the totals are exact.")
    if gains:
        lines.append("**Largest reductions:** " + "; ".join(f"{c} {s} {millions(-d)} fewer ({change(a, b)})" for d, c, s, a, b in sorted(gains)[:5]))
    if "legacy" not in attributions:
        lines.append("**Increases:** " + ("; ".join(f"{c} {s} +{millions(d)} ({change(a, b)})" for d, c, s, a, b in sorted(losses, reverse=True)[:5]) if losses else "none above 1% of a corpus"))
    top = sorted([r for r in remaining if r[2] != "unattributed"], reverse=True)[:5]
    lines.append("**Largest remaining costs:** " + "; ".join(f"{c} {s} {millions(n)} ({share * 100:.0f}% of it)" for share, c, s, n in top))
    lines += ["", "<details><summary>Stages by corpus</summary>", ""]
    for corpus, h in head["corpora"].items():
        b = base["corpora"].get(corpus)
        if not b:
            continue
        ht = h["instructions"]["total"]
        lines += [f"**{corpus}** ({h['rows']:,} rows, {h['bytes'] / 1e6:.1f} MB)", "",
                  "| stage | base | head | change | share of head | allocations (head) | peak live (head) |", "|---|---|---|---|---|---|---|"]
        allocs = {s["stage"]: s for s in h.get("allocations", [])}
        missing = "not collected" if not h.get("allocations") else "0"
        for stage in STAGES + ["unattributed", "total"]:
            if stage == "unattributed":
                bs, hs = b["instructions"]["unattributed"], h["instructions"]["unattributed"]
            elif stage == "total":
                bs, hs = b["instructions"]["total"], ht
            else:
                bs, hs = b["instructions"]["stages"].get(stage, 0), h["instructions"]["stages"].get(stage, 0)
            if not bs and not hs:
                continue
            a = allocs.get(stage)
            alloc = f"{a['allocations']:,} ({a['bytes'] / 1e6:.1f} MB)" if a else (missing if stage not in ("unattributed", "total") else "")
            peak = f"{a['peak_live'] / 1e6:.1f} MB" if a else ""
            lines.append(f"| {stage} | {millions(bs)} | {millions(hs)} | {change(bs, hs)} | {hs / ht * 100:.1f}% | {alloc} | {peak} |")
        rss = h["timing"]["peak_rss_kb"]
        if rss:
            lines.append(f"\nPeak RSS (head, whole process): {max(rss) / 1024:.0f} MB")
        lines.append("")
    lines.append("</details>")
    if cumulative:
        lines += ["", f"<details><summary>Cumulative, against `{cumulative['sha'][:8]}`</summary>", "",
                  "| corpus | instructions | wall |", "|---|---|---|"]
        for corpus, h in head["corpora"].items():
            c = cumulative["corpora"].get(corpus)
            if c:
                ct, ht = c["instructions"]["total"], h["instructions"]["total"]
                lines.append(f"| {corpus} | {millions(ct)} → {millions(ht)} ({change(ct, ht)}) | {timed(c['timing']['wall'], h['timing']['wall'])} |")
        lines += ["", "</details>"]
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    c = sub.add_parser("compare")
    c.add_argument("--base", required=True)
    c.add_argument("--head", required=True)
    c.add_argument("--cumulative")
    c.add_argument("--rounds", type=int, default=6, help="ABBA timing rounds (each revision runs once per round)")
    c.add_argument("--run-url", help="the CI run, linked from the report")
    m = sub.add_parser("measure")
    m.add_argument("--rev", required=True)
    m.add_argument("--rounds", type=int, default=3)
    for p in (c, m):
        p.add_argument("--out", default="scorecard-out")
        p.add_argument("--cache", default=str(ROOT / ".scorecard"))
    r = sub.add_parser("report")
    r.add_argument("base")
    r.add_argument("head")
    r.add_argument("--cumulative")
    r.add_argument("--md")
    args = parser.parse_args()

    if args.command == "report":
        load = lambda p: json.loads(Path(p).read_text())
        md = report(load(args.base), load(args.head), load(args.cumulative) if args.cumulative else None)
        Path(args.md).write_text(md) if args.md else print(md)
        return
    cache, out = Path(args.cache), Path(args.out)
    cache.mkdir(parents=True, exist_ok=True)
    out.mkdir(parents=True, exist_ok=True)
    revs = [args.rev] if args.command == "measure" else [args.base, args.head] + ([args.cumulative] if args.cumulative else [])
    results = [measure(rev, cache) for rev in revs]
    time_abba(results, args.rounds)
    for res in results:
        (out / f"scorecard-{res['sha'][:12]}.json").write_text(json.dumps(res, indent=1))
    if args.command == "compare":
        md = report(results[0], results[1], results[2] if args.cumulative else None, args.run_url)
        (out / "report.md").write_text(md)
        (out / "compared.json").write_text(json.dumps({"base": results[0]["sha"], "head": results[1]["sha"]}))
        print(md)


if __name__ == "__main__":
    main()
