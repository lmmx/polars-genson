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
| As a struct    | written |         | Stored as a struct, with a field for every language, most of the values are null, and every new language adds a field to the type. |
| As a struct    | pause   |     0.8 |  |
| As a struct    | say     |     3.7 | Stored as a struct, with a field for every language, |
| As a struct    | pause   |     0.3 |  |
| As a struct    | say     |     2.2 | most of the values are null, |
| As a struct    | pause   |     0.3 |  |
| As a struct    | say     |     3.7 | and every new language adds a field to the type. |
| As a struct    | pause   |     2.5 |  |
| As a map       | written |         | Stored as a map, Paris has five labels and Rome has two, with no nulls. The column's type is Map(String, String), however many languages there are. |
| As a map       | pause   |     0.8 |  |
| As a map       | say     |     1.5 | Stored as a map, |
| As a map       | pause   |     0.3 |  |
| As a map       | say     |     3.0 | Paris has five labels and Rome has two, |
| As a map       | pause   |     0.3 |  |
| As a map       | say     |     1.1 | with no nulls. |
| As a map       | pause   |     0.3 |  |
| As a map       | say     |     2.2 | The column's type is Map(String, String), |
| As a map       | pause   |     0.3 |  |
| As a map       | say     |     1.9 | however many languages there are. |
| As a map       | pause   |     2.5 |  |
| Map keys       | written |         | In Polars, map keys can be any type that can be row-encoded, which is anything except Object. JSON object keys are always strings, so polars-genson's map keys are strings. |
| Map keys       | pause   |     0.8 |  |
| Map keys       | say     |     3.0 | In Polars, map keys can be any type |
| Map keys       | pause   |     0.3 |  |
| Map keys       | say     |     1.5 | that can be row-encoded, |
| Map keys       | pause   |     0.3 |  |
| Map keys       | say     |     1.9 | which is anything except Object. |
| Map keys       | pause   |     0.3 |  |
| Map keys       | say     |     2.2 | JSON object keys are always strings, |
| Map keys       | pause   |     0.3 |  |
| Map keys       | say     |     2.2 | so polars-genson's map keys are strings. |
| Map keys       | pause   |     2.5 |  |
| Naming maps    | written |         | To get Map columns from JSON, name the fields that are maps with force_field_types. |
| Naming maps    | pause   |     0.8 |  |
| Naming maps    | say     |     2.2 | To get Map columns from JSON, |
| Naming maps    | pause   |     0.3 |  |
| Naming maps    | say     |     2.2 | name the fields that are maps |
| Naming maps    | pause   |     0.3 |  |
| Naming maps    | say     |     3.0 | with force_field_types. |
| Naming maps    | pause   |     2.5 |  |
| Inferring maps | written |         | polars-genson can also infer maps. An object with one type of value and more distinct keys than map_threshold becomes a map. The default threshold is 20, so these labels, with 8 keys, stay a struct. Lower it to 5, and they become a map. |
| Inferring maps | pause   |     0.8 |  |
| Inferring maps | say     |     1.9 | polars-genson can also infer maps. |
| Inferring maps | pause   |     0.3 |  |
| Inferring maps | say     |     2.6 | An object with one type of value |
| Inferring maps | pause   |     0.3 |  |
| Inferring maps | say     |     4.0 | and more distinct keys than map_threshold becomes a map. |
| Inferring maps | pause   |     0.3 |  |
| Inferring maps | say     |     1.9 | The default threshold is 20, |
| Inferring maps | pause   |     0.3 |  |
| Inferring maps | say     |     3.3 | so these labels, with 8 keys, stay a struct. |
| Inferring maps | pause   |     0.3 |  |
| Inferring maps | say     |     3.0 | Lower it to 5, and they become a map. |
| Inferring maps | pause   |     3.5 |  |
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
| Map functions  | written |         | The map namespace works on Map columns. map.get looks up a key: here, English, and here, Spanish, which two cities don't have. map.len counts each city's labels. |
| Map functions  | pause   |     0.8 |  |
| Map functions  | say     |     2.6 | The map namespace works on Map columns. |
| Map functions  | pause   |     0.3 |  |
| Map functions  | say     |     2.6 | map.get looks up a key: here, English, |
| Map functions  | pause   |     0.3 |  |
| Map functions  | say     |     3.0 | and here, Spanish, which two cities don't have. |
| Map functions  | pause   |     0.3 |  |
| Map functions  | say     |     1.9 | map.len counts each city's labels. |
| Map functions  | pause   |     2.5 |  |
| End card       | written |         | polars-genson 1.0 supports Polars 2.0, and you can install it from PyPI. See the docs to get started. |
| End card       | pause   |     0.8 |  |
| End card       | say     |     1.9 | polars-genson 1.0 supports Polars 2.0, |
| End card       | pause   |     0.3 |  |
| End card       | say     |     2.6 | and you can install it from PyPI. |
| End card       | pause   |     0.3 |  |
| End card       | say     |     2.2 | See the docs to get started. |
| End card       | pause   |     2.5 |  |
