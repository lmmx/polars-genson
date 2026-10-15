# 2026-10-15 Unification algebra

Chats with Claude Fable on formalising the merge operation behind map unification, and
on which algebraic properties it holds, are stored in `2026-10-15-unification-algebra/`
next to this entry. This entry records the operation as implemented at 96ef9d5.

## Current State

- `check_unifiable_schemas` merges a list of schemas by dispatching on their kind — all
  arrays, all maps or empty records, all records or empty records, all scalars — and
  returns `None` for any mix of kinds
  (genson-core/src/schema/map_inference/unification.rs:678-797)
- `check_unifiable_schemas` returns `{"type": "object", "additionalProperties":
  {"type": "string"}}` when every schema is an empty record, so an empty map's value type
  comes from no data (genson-core/src/schema/map_inference/unification.rs:725-734)
- `unify_field_schemas` folds one field's schemas left to right with `unify_field_pair`
  when there are fewer than 50 of them or `wrap_scalars` mixes scalars with records, and
  otherwise splits them in halves on rayon
  (genson-core/src/schema/map_inference/unification.rs:453-469)
- `unify_field_pair` tries `schemas_compatible` and scalar promotion before
  `check_unifiable_schemas`, while the halves of `unify_field_schemas_parallel` are
  joined by `check_unifiable_schemas` alone — the left fold and the halves combine
  schemas with different functions
  (genson-core/src/schema/map_inference/unification.rs:402-431, 434-450)
- `check_unifiable_item_schemas` falls back to `unify_anyof_schemas` for arrays whose
  items mix scalars and records when `wrap_scalars` is set; record fields get no such
  fallback (genson-core/src/schema/map_inference/unification.rs:802-817)
- `unified_values` decides a candidate map's value schema: the shared schema when every
  value is equal once nulls are set aside, `None` without `unify_maps` or when a key is
  in `no_unify`, and `check_unifiable_schemas` (or `check_unifiable_item_schemas` for
  arrays) otherwise (genson-core/src/schema/map_inference.rs:451-512)
- Tree merging during the build phase goes through `SchemaNode::absorb` and
  `absorb_all`, recorded in docs/journal/2026-10-08-inference-speed.md

## Missing

- No test checks that unification gives the same schema whatever the order or grouping
  of its inputs, and no crate in the workspace depends on proptest or quickcheck
- No document in docs/concepts states the merge as an operation with an identity or
  algebraic laws; docs/concepts/inference.md and maps-and-records.md give its rules as
  prose
