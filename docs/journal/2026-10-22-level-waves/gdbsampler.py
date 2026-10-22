# Loaded with `gdb -p PID -batch -x gdbsampler.py` while the shell sends the inferior
# SIGINT every few ms (gdb is told to stop on it and not pass it on); at each stop
# every thread's backtrace goes to OUT, then the inferior continues, until it exits.
import gdb, os, signal, threading, time
OUT = os.environ["SAMPLE_OUT"]
pid = gdb.selected_inferior().pid
gdb.execute("set pagination off"); gdb.execute("set confirm off")
gdb.execute("handle SIGINT stop print nopass")  # noprint would imply nostop
for sig in ("SIGPIPE", "SIGCHLD", "SIGUSR1", "SIGUSR2"):
    gdb.execute(f"handle {sig} nostop noprint pass")
out = open(OUT, "w"); n = 0
while True:
    try:
        gdb.execute("continue", to_string=True)
    except gdb.error:
        break
    if not gdb.selected_inferior().pid:
        break
    n += 1
    out.write(f"=== SAMPLE {n} {time.monotonic()}\n")
    try:
        out.write(gdb.execute("thread apply all bt " + os.environ.get("SAMPLE_DEPTH", "40"), to_string=True))
    except gdb.error:
        break
out.close()
