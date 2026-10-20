"""A callgrind output's functions, inclusive (largest entry per name, recursive '2 entries
skipped) and exclusive, as compact text. Usage: python3 cgt.py OUT [N] [regex-filter]"""
import re
import subprocess
import sys

out, n = sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else 40
flt = re.compile(sys.argv[3]) if len(sys.argv) > 3 else None


def table(inclusive):
    txt = subprocess.run(["callgrind_annotate", f"--inclusive={'yes' if inclusive else 'no'}", "--threshold=100", out],
                         capture_output=True, text=True).stdout
    total = int(re.search(r"([\d,]+) \(100\.0%\)\s+PROGRAM TOTALS", txt).group(1).replace(",", ""))
    costs = {}
    for line in txt.splitlines():
        m = re.match(r"\s*([\d,]+) \([ \d.]+%\)\s+(?:\S+?:)?(\S.*?)(?: \[.*\])?$", line)
        if not m or "PROGRAM TOTALS" in line:
            continue
        name = m.group(2)
        if "'2" in name or name.startswith("=>"):
            continue
        name = re.sub(r"<[^<>]*(?:<[^<>]*(?:<[^<>]*>[^<>]*)*>[^<>]*)*>", lambda mm: mm.group(0) if len(mm.group(0)) < 60 else "<..>", name)
        v = int(m.group(1).replace(",", ""))
        costs[name] = max(costs.get(name, 0), v)
    return total, costs


def show(inclusive):
    total, costs = table(inclusive)
    print(f"== {'inclusive' if inclusive else 'exclusive'} (total {total / 1e6:,.1f}M Ir)")
    k = 0
    for name, v in sorted(costs.items(), key=lambda kv: -kv[1]):
        if flt and not flt.search(name):
            continue
        if inclusive and not flt and not re.search(r"genson|prof3|serde|simd|avrotize|arrow|regex", name):
            continue
        print(f"{v / 1e6:12,.1f}M {v / total * 100:5.1f}%  {name[:170]}")
        k += 1
        if k >= n:
            break


show(True)
show(False)
