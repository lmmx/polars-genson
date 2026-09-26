# Guides

Each guide covers one task, with examples you can run. For why genson behaves as it
does, see [How it works](../concepts/index.md).

- [Infer a schema from a JSON column](infer-schema.md): as a Polars schema, a JSON Schema
  or an Avro schema, per row or merged.
- [Normalise a JSON column](normalise.md): into typed columns, one struct column, or JSON
  strings.
- [Normalise JSON stored in Parquet](parquet.md): straight from a Parquet column to typed
  Parquet output, keeping other columns alongside.
- [Factor out repeated embedded data](extract-invariants.md): store data that repeats
  per key once, in a lookup table.
- [Process large inputs](large-inputs.md): speed and memory for big files.
