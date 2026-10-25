"""As rus.py, with transparent huge pages disabled for the child (PR_SET_THP_DISABLE, inherited across exec)."""
import ctypes, os, subprocess, sys
assert ctypes.CDLL(None, use_errno=True).prctl(41, 1, 0, 0, 0) == 0
p = subprocess.Popen(sys.argv[1:], stdout=subprocess.PIPE, text=True)
out = p.stdout.read().strip().splitlines()[-1]
_, _, ru = os.wait4(p.pid, 0)
print(f"{out} | proc user {ru.ru_utime:.2f} sys {ru.ru_stime:.2f} minflt {ru.ru_minflt}")
