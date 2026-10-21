import sys, pyarrow.parquet as pq
a, b = (pq.read_table(p).replace_schema_metadata(None) for p in sys.argv[1:3])
f = pq.ParquetFile(sys.argv[2])
print(f"equal={a.equals(b) if a.schema.equals(b.schema) else 'cast:' + str(a.equals(b.cast(a.schema)))} rgs={f.metadata.num_row_groups} mb={round(f.metadata.serialized_size/1e6,2) if False else ''}")
