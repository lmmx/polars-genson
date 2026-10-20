"""The official dump's first chunk, made as wikidata-pq's split-dump makes it: the first
CHUNK_ENTITIES entity lines of the latest release's bz2 dump (streamed, not downloaded
whole), reshaped by dump.write_chunk."""
import bz2, sys, time
from pathlib import Path
sys.path.insert(0, "/home/user/lmmx/wikidata-pq/src")
from wikidata import dump
N = 2 * dump.CHUNK_ENTITIES
release = dump.latest_release()
url = f"{dump.DUMPS_URL}/{release}/{dump.dump_name(release)}"
print("release", release, url, flush=True)
dec, lines, buf, read = bz2.BZ2Decompressor(), [], b"", 0
t0 = time.time()
with dump._open(url, timeout=120) as r:
    while len(lines) < N:
        block = r.read(1 << 20); read += len(block)
        buf += dec.decompress(block)
        *done, buf = buf.split(b"\n")
        for line in done:
            line = line.rstrip(b",\r")
            if line.startswith(b"{"):
                lines.append(line)
lines = lines[:N]
print(f"read {read/1e6:.1f} MB compressed, {sum(map(len, lines))/1e6:.1f} MB of JSON, {len(lines)} entities in {time.time()-t0:.0f}s", flush=True)
print(dump.write_chunk(0, lines[:dump.CHUNK_ENTITIES], Path(".")))
print(dump.write_chunk(1, lines[dump.CHUNK_ENTITIES:], Path(".")))
