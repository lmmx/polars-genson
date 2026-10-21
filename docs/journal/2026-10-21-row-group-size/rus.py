"""Run a command; print its own stdout line plus its whole-process user, sys and minor faults."""
import os, subprocess, sys
p = subprocess.Popen(sys.argv[1:], stdout=subprocess.PIPE, text=True)
out = p.stdout.read().strip().splitlines()[-1]
_, _, ru = os.wait4(p.pid, 0)
print(f"{out} | proc user {ru.ru_utime:.2f} sys {ru.ru_stime:.2f} minflt {ru.ru_minflt}")
