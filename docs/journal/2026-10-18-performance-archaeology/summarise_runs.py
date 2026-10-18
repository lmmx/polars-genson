"""Medians and ranges of the JSON lines `bench_wpq.py`, `bench_wpq_maps.py` and
`bench_process.py` print, by label: wall, CPU, peak RSS, and each step and stage.

Usage: python summarise_runs.py <runs.txt>
"""
import json, statistics as st, collections, sys
d = collections.defaultdict(list)
for l in open(sys.argv[1]):
    if l.startswith('{'):
        j = json.loads(l); d[j['label']].append(j)
    else:
        print(l.rstrip()[:300])
for k, v in d.items():
    print(k, len(v), 'runs: wall med %.3f [%.3f-%.3f]' % (st.median(x['wall'] for x in v), min(x['wall'] for x in v), max(x['wall'] for x in v)), 'cpu med %.2f' % st.median(x['cpu'] for x in v), 'rss %.0f' % st.median(x['maxrss_mb'] for x in v))
    for group in ('steps', 'stages'):
        keys = sorted({s for x in v for s in x.get(group, {})})
        print('   ', group + ':', ', '.join('%s %.3f' % (s, st.median(x[group].get(s, 0) for x in v)) for s in keys))
