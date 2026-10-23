# 2026-10-23: Output as an Arrow C stream

`normalise_from_parquet(output_format="arrow")` writes no file: it returns the output's
record batches as an Arrow C stream, the Arrow PyCapsule interface's
`__arrow_c_stream__` (polars-genson-py/src/arrow_stream.rs), read once with
`pyarrow.RecordBatchReader.from_stream` or `pyarrow.table`. The batches are those the
Parquet output writes (genson-core's `struct_batches`, which `write_struct_columns` now
calls), in the same schema and with the same metadata.

## Why

wikidata-pq wrote its chunks' claims with genson as nested Parquet (one row per entity),
then read them back to explode them into one row per statement and wrote them again
(its 2026-10-23 claims-stream journal entry). On 11 chunks, reading the file back,
exporting it from Polars to Arrow and writing it again took 46 s of CPU, against genson's
own write of 6.6 s. With the batches handed over in memory, wikidata-pq explodes them in
Arrow and writes Parquet once.

An Arrow IPC file was tried first, as a proof of concept: it took as long as the stream
but was 15 times the size of the Parquet (1.7 GB for the 11 chunks' claims), and is not
kept.

## Behaviour

- **No copy:** the batches' buffers move to the reader (pyarrow's memory pool grew 0 MB
  importing a 40.5 MB table).
- **Read once:** a second read raises `ValueError`.
- **Released if unread:** a stream that is dropped unread releases its batches (20
  dropped streams of 40 MB left the process's RSS unchanged).
- **Memory:** the batches are those genson holds before writing Parquet, so the peak is
  the same as the Parquet output's.
- **Refused:** an output path with `output_format="arrow"`, no path for Parquet, and
  `typed=False`.

Tests: `polars-genson-py/tests/arrow_output_test.py` (the stream holds the Parquet
output's table and metadata, inferred and target schema, unnested or not).
