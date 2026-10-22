"""Touch `GB` gigabytes of fresh memory, then exit, so that the next process starts from the
same memory history whatever ran before it. Usage: primer.py <GB>"""
import sys
n = int(float(sys.argv[1]) * (1 << 30)); b = bytearray(n)
for i in range(0, n, 4096): b[i] = 1
