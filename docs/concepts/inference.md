# How inference works

genson infers a schema in two passes. The first reads the data and builds one JSON
Schema that every row fits. The second rewrites that schema: it decides which objects are
maps, merges the different shapes a value takes, and puts union types in a fixed order.
This page describes both. The pages before it describe the outcome; this one describes
the algorithm, for when you need to know why a schema came out as it did, or what a
change to the data will do to it.

## Pass 1: build a schema from the data

### One tree per position in the data

genson parses each JSON string with simd-json and records what it sees in a tree with one
node per position in the data: the root, each field of an object, the items of an array,
the values of a map. A node records each kind of value seen at its position:

| Seen at the position | Recorded                                                                 |
|----------------------|--------------------------------------------------------------------------|
| an object            | its fields, in the order first seen; the keys present in every object    |
| an array             | one node for all of its items                                            |
| a number             | `integer`, until a number that is not an integer is seen: then `number`  |
| a string, boolean, null | the type                                                              |

A position that held more than one kind of value gets a schema for each: scalars become a
list of types (`{"type": ["null", "string"]}`), and an object or array alongside others
becomes an `anyOf`. A field is `required` when every object seen at that position held
it.

`wrap_root` writes each document inside an object of one field (`{"<field>": <document>}`)
before it is parsed.

### Many strings: one tree each, then merged

With 10 or more strings, each string gets a tree of its own on the thread pool. The trees
are then merged, in input order, into one: fields are united in the order first seen,
the `required` keys are those required in every tree, and a number position becomes
`number` if it is `number` in any tree. Merging two trees gives the same schema as
reading all of their data into one tree.

- **Repeated schemas are merged once.** Each string's tree is hashed as it is built, and a
  tree whose schema equals one already merged is skipped. Rows of the same shape are
  common, so this saves most of the merging. (The `processed_count` that inference
  reports is the number of distinct schemas merged.)
- **The merge runs in parallel across fields.** When the trees being merged are all
  objects, each field of the root is merged on its own thread, and so on down the tree
  while there are enough trees to make it worthwhile. Each field still takes its values
  in input order, so the result does not depend on the number of threads.
- **`max_builders` bounds memory.** Strings are processed in chunks of `max_builders`;
  each chunk's trees are merged before the next chunk is parsed (see
  [Process large inputs](../guides/large-inputs.md)).
- **Fields forced to be maps** (`force_field_types={"field": "map"}`) are turned into
  maps in each string's tree before the merge, so that their values are merged across
  strings as map values rather than as fields.

With fewer than 10 strings, each string is read into the same tree in turn. Within one
string, the documents of NDJSON (and arrays of 10 or more items anywhere) are read on
several threads and merged the same way.

## Pass 2: rewrite the schema

The merged tree is written out as a JSON Schema, and genson walks it from the root,
carrying each field's name down so that options naming fields apply. At each object,
the first of these rules that applies decides what happens:

1. **A field in `force_scalar_promotion`** holding a scalar becomes a record with one
   field, `<field>__<type>` (for example `datavalue__string`).
2. **A field in `force_field_types`** becomes a map (`"map"`), whose value schema is the
   values' common schema, unified as below if they differ, or else `string`; or stays a
   record (`"record"`).
3. **An `anyOf` of scalars and records**, with `wrap_scalars` on (the default), becomes
   one record: each scalar is promoted to a record of one field `<field>__<type>`, and
   the records are unified.
4. **An object that was empty in every row**, with `map_threshold=0`, becomes a map of
   null (a record with no fields cannot be written to Parquet).
5. **An object holding a field named in `force_parent_field_types`** as `"record"`, or a
   field in `force_scalar_promotion`, stays a record.
6. **An object becomes a map** when it has at least `map_threshold` keys, is not the
   root (unless `no_root_map=False`), and
   - all of its values are the same record; or else
   - it has at most `map_max_required_keys` required keys (if that is set), and its
     values' schemas are the same once nullability is set aside or, with `unify_maps`,
     they unify.

   The map's value schema is that common schema.

Then genson goes on into the object's fields, an array's items and a map's values.

### Unifying value schemas

Unification merges the schemas of a candidate map's values into one, when they are of
one kind:

| Values                    | Unified as                                                              |
|---------------------------|-------------------------------------------------------------------------|
| arrays                    | an array of the items unified; an array only ever seen empty fits any array |
| maps (and empty records)  | a map of the values unified                                             |
| records                   | a record of every field seen: unified field by field, required and non-null where every record has the field, nullable otherwise |
| scalars of one type       | that type, nullable                                                     |

Within a record field, two schemas that differ only in being nullable merge into the
nullable one, and with `wrap_scalars` a scalar that meets a record (or a scalar of
another type) is promoted under `<field>__<type>`. Array items unify the same way, and
there a scalar among records is promoted too. Values of different kinds that none of
this reconciles (an array and a record, say) do not unify, and the object stays a
record. Nor is an object unified when one of its keys is named in `no_unify`.

### Union order

Finally, every list of types is put in a fixed order: `null` first, then containers
(map, array, object), then scalars from the narrowest (`boolean`, `integer`, `number`,
`string`). A `["null", T]` pair keeps its order.
