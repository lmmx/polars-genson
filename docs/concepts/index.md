# How it works

polars-genson turns JSON into Polars columns in two steps:

1. **Infer a schema.** genson reads every row and builds one schema that fits them all:
   which fields exist, what type each has, which objects are records and which are maps.
2. **Normalise the rows.** Each row is rewritten to fit that schema: missing fields are
   filled with null, values are converted to the field's type, maps are encoded, and
   scalars are promoted where needed. The result decodes to one consistent set of typed
   columns.

The pages in this section explain the decisions genson makes along the way. Each shows
real output from genson for small inputs.

- [Type mapping](type-mapping.md): which Polars dtype each kind of JSON value becomes.
- [Nulls, missing keys and empty values](nulls-and-empty-values.md): what happens to
  absent, null and empty values, and to null rows.
- [Maps and records](maps-and-records.md): when an object becomes a struct and when a
  list of `{key, value}` structs, and how to choose.
- [Mixed types](mixed-types.md): what happens when one field holds different kinds of
  value in different rows.
- [Field order](key-order.md): how genson orders fields, and why that matters when
  combining output from several inputs.
