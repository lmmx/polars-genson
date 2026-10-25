# 2026-10-25: Where the claims call's CPU goes (profiled; no cut found in genson)

`process.normalise_in_schema` is a release's largest cost (about 42% of the process
step's CPU on wikidata-pq's bench). This profiles it at genson 1.5.0 on the bench's
chunks c0, s30 and s90 (a shared 4-core VM). The time is spread over a single pass that is
already lean. No part of it is work whose result never reaches the output, and none of
the ideas tested would cut more than a few percent. The structural cut is upstream, in
how the claims reach genson.

## The calls

wikidata-pq's four calls, timed from Python (`calls.py`, user CPU, the median of three
passes after a first):

| chunk | claims | labels | descriptions | aliases |
|---|---|---|---|---|
| c0 | 2.61 s | 0.19 s | 0.16 s | 0.09 s |
| s30 | 1.57 s | 0.02 s | 0.11 s | 0.01 s |
| s90 | 1.60 s | 0.03 s | 0.14 s | <0.01 s |

The claims call is 86–92% of the step. A Rust replica of it (`bench_claims_call.rs`:
read, `extract_invariants_rows`, the strict decode against `claims.avsc`, the lookup
write, `conform` to the target, `struct_batches`) used the same CPU within noise
(2.2–2.7 s on c0, 1.1–1.3 s on s30, 1.5 s on s90). So the Python side around the call
is not measurable here: the Arrow stream is handed over without a copy.

## The claims call, by stage

perf with call graphs (frame pointers), samples of the replica's six runs, user CPU only:

| | c0 | s30 | s90 |
|---|---|---|---|
| decode: the walk (`put`, slots, `field_input`, numbers) | 39.6% | 41.9% | 41.0% |
| decode: lexing (`tape::lex`) | 19.7% | 20.7% | 20.4% |
| decode: appends and allocation | 15.6% | 15.4% | 15.3% |
| read (`read_string_column_arrays`) | 12.5% | 9.8% | 10.6% |
| `extract_invariants` (the keyed scan, the rows copied) | 4.8% | 4.4% | 4.5% |
| `conform` | 3.4% | 3.6% | 3.8% |
| other (rayon, drops) | 4.4% | 4.2% | 4.4% |

- **Unescaping:** not visible. After reshape, the claims' strings hold almost no escapes.
- **Kernel time:** 20–30% of all samples on this VM. It is page faults, mostly `clear_page`
  for the read's buffers, the decoder's columns and `conform`'s 64-bit offsets. CI's
  runners pay page faults cheaply.
- **Inside the decode (s90, leaf functions):** `Decoder::put` 23%, the lexer's strings
  and values 14% and 10%, memmove 7% (strings copied into their columns), `slots_of`
  7%, `field_input` 5%, `number` 4%, string appends 4%, memcmp 4% (keys against field
  names). `put`'s own samples are spread across its instructions, with no hot spot: it
  is a dispatch-bound walk of the plan over the tape.
- **Inside the read:** memmove 35%, zstd 22%, page clearing 17%, UTF-8 validation 5%.

## Ideas tested

- **Lexed twice?** No. The bench's claims carry no label maps (like an official dump's),
  so `Scan::Keyed` scans each row with `memmem` and keeps every row unread: 10,000 of
  10,000 on c0, 7,301 of 7,301 on s30, 8,919 of 8,919 on s90. Copying them into owned
  strings (`Extracted::from`) took 0.011–0.017 s. The decoder's lex is the only parse
  in genson.
- **SIMD structural lexing:** unlikely to pay. The claims text holds 94 tokens per KB,
  and its strings average 9.8 bytes (91% under 16), so the cost is per token, not per
  byte. On one thread our lexer read 804–891 MB/s on s90 and 708–851 MB/s on c0.
  simd-json's `to_tape` (stage 1 and stage 2, its copy of each row taken out) read
  625–680 and 571–633 MB/s. A SIMD stage 1 does not beat the lexer as it is on these rows.
- **Skipping what is pruned:** the prune checks cost nothing measurable. Decoding with
  no prune set used the same user CPU within noise (s90 1.13–1.28 s against 1.18–1.28 s,
  s30 0.91–0.98 against 0.93–1.07, c0 1.77–1.94 against 1.77–1.83). The bench's chunks
  have no values to prune.
- **Interning keys once per batch:** members mostly come in the schema's order, so a key
  is matched by one comparison with the next field's name (`object_slots`). Key matching
  is within the 7% of `slots_of` and the 4% of memcmp, so interning could save only part
  of that.
- **Smaller ones, not pursued:**
  - The strict decode parses an integer twice (`fits`, then `put`), within `number`'s 4%.
  - A record's fields are visited, not its members (`field_input` 5%).
  - Each would be a few percent of the decode at most.
- **Buffer growth:** tried and not shipped (#317, `2026-10-25-presized-builders.md`).

## Where a structural cut is

The claims reach genson as JSON text that wikidata-pq's reshape wrote. Reshape parses each
entity with sonic-rs (`entity_row`) and writes the claims back out as text
(`write_statement`, `write_snak`), and the chunk's Parquet compresses them. genson then
reads the column back (10–12% of the call) and lexes it (about 20%). Then it walks the
tape against the same schema reshape knows (40%) and appends to columns (15%).

If reshape appended the claims it has already parsed straight into the claims schema's
Arrow columns, these would go:
- reshape's text writing of the claims;
- the claims text's Parquet round trip;
- genson's read and lex: about a third of the call.

The walk and the appends would move into reshape. That is a change to reshape and to the
chunk files, which another workstream owns, so it is left to be weighed there.

## Scripts

In `2026-10-25-claims-call-profile/`:
- `calls.py` times wikidata-pq's four calls from Python.
- `bench_claims_call.rs` is the call's replica (a genson-core example). Build it with
  `--features avro,parquet` and `RUSTFLAGS="-C force-frame-pointers=yes"`, and record it
  with `perf record -g`.
- `cat.py` sorts `perf script` samples by stage.
- `bench_lex.rs` compares the lexer with simd-json. It needs a helper not on master, in
  `lib.rs`: `#[doc(hidden)] pub fn bench_lex(rows: &[&str]) -> usize`, which runs
  `tape::lex` over the rows and counts the tokens.
