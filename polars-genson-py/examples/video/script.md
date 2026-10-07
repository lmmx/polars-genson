# Script and timing for the json_to_map video

The video runs down this table, top to bottom. Edit any `seconds` to re-time a row, and
add or remove `pause` rows wherever you want a gap. Both videos and the captions are
timed from it, so render them together after an edit (`render.py`), and use
`render.py timings` to see when each row starts.

- `written`: the scene's paragraph as written, for the captions. Takes no time.
- `say`: one teleprompter line, and how long it takes to say. Leave `seconds` blank to
  use the default speaking rate (`WORDS_PER_SECOND` in voiceover.py).
- `pause`: silence. The pause at the end of each scene is how long its last visuals
  stay on screen before the next scene.

The explainer's visuals appear on cue with a scene's `say` rows, counted from 0, so
adding or removing a `say` row in a scene shifts which line a later visual comes in on.

| scene          | kind    | seconds | text |
|----------------|---------|---------|------|
| Title          | written |         | Polars 2.0 introduces a Map data type, for objects whose keys are data rather than part of the type. |
| Title          | pause   |     0.8 |  |
| Title          | say     |     2.6 | Polars 2.0 introduces a Map data type, |
| Title          | pause   |     0.3 |  |
| Title          | say     |     2.2 | for objects whose keys are data, |
| Title          | pause   |     0.3 |  |
| Title          | say     |     2.2 | rather than part of the type. |
| Title          | pause   |     2.5 |  |
| Records        | written |         | Some keys are part of the type. A person's name and date of birth are fields: every person has them, and they hold different types of value, a string and a date, so they can't share one value type. That's a struct. |
| Records        | pause   |     0.8 |  |
| Records        | say     |     2.6 | Some keys are part of the type. |
| Records        | pause   |     0.3 |  |
| Records        | say     |     3.3 | A person's name and date of birth are fields: |
| Records        | pause   |     0.3 |  |
| Records        | say     |     1.5 | every person has them, |
| Records        | pause   |     0.3 |  |
| Records        | say     |     2.6 | and they hold different types of value, |
| Records        | pause   |     0.3 |  |
| Records        | say     |     1.9 | a string and a date, |
| Records        | pause   |     0.3 |  |
| Records        | say     |     2.6 | so they can't share one value type. |
| Records        | pause   |     0.3 |  |
| Records        | say     |     1.1 | That's a struct. |
| Records        | pause   |     2.5 |  |
| Maps           | written |         | Other keys are data. These are labels from Wikidata, keyed by language code. There are thousands of languages, each city has labels in only some of them, and every label is a string. That's a map. |
| Maps           | pause   |     0.8 |  |
| Maps           | say     |     1.5 | Other keys are data. |
| Maps           | pause   |     0.3 |  |
| Maps           | say     |     1.9 | These are labels from Wikidata, |
| Maps           | pause   |     0.3 |  |
| Maps           | say     |     1.5 | keyed by language code. |
| Maps           | pause   |     0.3 |  |
| Maps           | say     |     1.9 | There are thousands of languages, |
| Maps           | pause   |     0.3 |  |
| Maps           | say     |     3.3 | each city has labels in only some of them, |
| Maps           | pause   |     0.3 |  |
| Maps           | say     |     2.2 | and every label is a string. |
| Maps           | pause   |     0.3 |  |
| Maps           | say     |     1.1 | That's a map. |
| Maps           | pause   |     2.5 |  |
| As a struct    | written |         | You could store the labels as a struct, with a field per language, but most of the fields would be null, and every new language would change the type. |
| As a struct    | pause   |     0.8 |  |
| As a struct    | say     |     3.0 | You could store the labels as a struct, |
| As a struct    | pause   |     0.3 |  |
| As a struct    | say     |     1.9 | with a field per language, |
| As a struct    | pause   |     0.3 |  |
| As a struct    | say     |     3.0 | but most of the fields would be null, |
| As a struct    | pause   |     0.3 |  |
| As a struct    | say     |     3.0 | and every new language would change the type. |
| As a struct    | pause   |     2.5 |  |
| As a map       | written |         | As a Map column, each row contains just the labels it has, and the type stays the same however many languages turn up. |
| As a map       | pause   |     0.8 |  |
| As a map       | say     |     1.5 | As a Map column, |
| As a map       | pause   |     0.3 |  |
| As a map       | say     |     3.0 | each row contains just the labels it has, |
| As a map       | pause   |     0.3 |  |
| As a map       | say     |     2.2 | and the type stays the same, |
| As a map       | pause   |     0.3 |  |
| As a map       | say     |     1.9 | however many languages turn up. |
| As a map       | pause   |     2.5 |  |
| Map keys       | written |         | Polars allows map keys of any type that can be row-encoded: anything but Object. JSON keys are always strings, so polars-genson's map keys are strings, for now. |
| Map keys       | pause   |     0.8 |  |
| Map keys       | say     |     2.6 | Polars allows map keys of any type |
| Map keys       | pause   |     0.3 |  |
| Map keys       | say     |     1.5 | that can be row-encoded: |
| Map keys       | pause   |     0.3 |  |
| Map keys       | say     |     1.1 | anything but Object. |
| Map keys       | pause   |     0.3 |  |
| Map keys       | say     |     1.9 | JSON keys are always strings, |
| Map keys       | pause   |     0.3 |  |
| Map keys       | say     |     3.0 | so polars-genson's map keys are strings, for now. |
| Map keys       | pause   |     2.5 |  |
| Inferring      | written |         | polars-genson reads a column of JSON strings and infers one schema for all of them. Maps are inferred with a heuristic: more distinct keys than a threshold, and one value type. The threshold is a rule of thumb, 20 by default, so with only four cities, the labels stay a struct until you lower it. |
| Inferring      | pause   |     0.8 |  |
| Inferring      | say     |     2.6 | polars-genson reads a column of JSON strings |
| Inferring      | pause   |     0.3 |  |
| Inferring      | say     |     3.0 | and infers one schema for all of them. |
| Inferring      | pause   |     0.3 |  |
| Inferring      | say     |     2.2 | Maps are inferred with a heuristic: |
| Inferring      | pause   |     0.3 |  |
| Inferring      | say     |     2.2 | more distinct keys than a threshold, |
| Inferring      | pause   |     0.3 |  |
| Inferring      | say     |     1.5 | and one value type. |
| Inferring      | pause   |     0.3 |  |
| Inferring      | say     |     2.6 | The threshold is a rule of thumb, |
| Inferring      | pause   |     0.3 |  |
| Inferring      | say     |     1.1 | 20 by default, |
| Inferring      | pause   |     0.3 |  |
| Inferring      | say     |     1.9 | so with only four cities, |
| Inferring      | pause   |     0.3 |  |
| Inferring      | say     |     1.9 | the labels stay a struct |
| Inferring      | pause   |     0.3 |  |
| Inferring      | say     |     1.5 | until you lower it. |
| Inferring      | pause   |     2.5 |  |
| Other ways     | written |         | If you know a field is a map, you can say so with force_field_types. Other options control how maps are inferred, such as unify_maps and map_max_required_keys. |
| Other ways     | pause   |     0.8 |  |
| Other ways     | say     |     3.0 | If you know a field is a map, |
| Other ways     | pause   |     0.3 |  |
| Other ways     | say     |     3.5 | you can say so with force_field_types. |
| Other ways     | pause   |     0.3 |  |
| Other ways     | say     |     2.6 | Other options control how maps are inferred, |
| Other ways     | pause   |     0.3 |  |
| Other ways     | say     |     4.0 | such as unify_maps and map_max_required_keys. |
| Other ways     | pause   |     2.5 |  |
| Why a Map type | written |         | Before Polars 2.0, polars-genson stored maps as lists of key-value structs. To look up one key, you had to filter the list. With a Map column, it's one call. |
| Why a Map type | pause   |     0.8 |  |
| Why a Map type | say     |     1.1 | Before Polars 2.0, |
| Why a Map type | pause   |     0.3 |  |
| Why a Map type | say     |     3.0 | polars-genson stored maps as lists of key-value structs. |
| Why a Map type | pause   |     0.3 |  |
| Why a Map type | say     |     4.1 | To look up one key, you had to filter the list. |
| Why a Map type | pause   |     0.3 |  |
| Why a Map type | say     |     2.6 | With a Map column, it's one call. |
| Why a Map type | pause   |     2.5 |  |
| Map functions  | written |         | The map namespace looks up keys, lists keys and values, and counts entries: here, how many labels each city has, and whether one is in German. |
| Map functions  | pause   |     0.8 |  |
| Map functions  | say     |     2.2 | The map namespace looks up keys, |
| Map functions  | pause   |     0.3 |  |
| Map functions  | say     |     1.5 | lists keys and values, |
| Map functions  | pause   |     0.3 |  |
| Map functions  | say     |     1.1 | and counts entries: |
| Map functions  | pause   |     0.3 |  |
| Map functions  | say     |     2.6 | here, how many labels each city has, |
| Map functions  | pause   |     0.3 |  |
| Map functions  | say     |     2.2 | and whether one is in German. |
| Map functions  | pause   |     2.5 |  |
| Parquet        | written |         | For files, normalise_from_parquet writes typed Parquet. Maps are written as Parquet map columns. |
| Parquet        | pause   |     0.8 |  |
| Parquet        | say     |     2.5 | For files, normalise_from_parquet |
| Parquet        | pause   |     0.3 |  |
| Parquet        | say     |     1.1 | writes typed Parquet. |
| Parquet        | pause   |     0.3 |  |
| Parquet        | say     |     2.6 | Maps are written as Parquet map columns. |
| Parquet        | pause   |     2.5 |  |
| End card       | written |         | polars-genson 1.0 supports Polars 2.0, and you can install it from PyPI. See the docs to get started. |
| End card       | pause   |     0.8 |  |
| End card       | say     |     1.9 | polars-genson 1.0 supports Polars 2.0, |
| End card       | pause   |     0.3 |  |
| End card       | say     |     2.6 | and you can install it from PyPI. |
| End card       | pause   |     0.3 |  |
| End card       | say     |     2.2 | See the docs to get started. |
| End card       | pause   |     2.5 |  |
