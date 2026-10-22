#!/usr/bin/env bash
# One claims call (claims_step.py, runs=1) in a fresh process, sampled by gdbsampler.py
# (one attach, genson's symbols only). Usage: sample2.sh <chunk> <out> <python> <src>
chunk=$1; out=$2; py=$3; src=$4
cd "$(dirname "$0")"
$py -I claims_step.py "$chunk" "w_sample_$$" "$src" 1 > "ready_$$.txt" 2> "stderr_$$.txt" &
PID=$!
until grep -q READY "ready_$$.txt" 2>/dev/null; do sleep 0.05; done
( sleep 1; while kill -0 $PID 2>/dev/null; do kill -INT $PID 2>/dev/null; sleep 0.05; done ) &
SAMPLE_OUT="$out" gdb -p $PID -batch -nx -iex "set auto-solib-add off" -ex "sharedlibrary _polars_genson" -x gdbsampler.py > "gdb_$$.log" 2>&1
wait $PID
grep -E "^\[profile\]|^RUN" "stderr_$$.txt" >> "$out"
rm -rf "w_sample_$$" "ready_$$.txt" "stderr_$$.txt"; mv "gdb_$$.log" "$out.gdblog"
