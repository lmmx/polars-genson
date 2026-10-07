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
| Title          | written |         | Polars 2.0 introduces a Map data type, for objects whose keys are data rather than part of the type. Polars-genson infers the schema of JSON data, including Map types. |
| Title          | pause   |    0.80 |  |
| Title          | say     |    2.84 | Polars 2.0 introduces a Map data type, |
| Title          | pause   |    0.00 |  |
| Title          | say     |    1.94 | for objects whose keys are data, |
| Title          | pause   |    0.00 |  |
| Title          | say     |    1.46 | rather than part of the type. |
| Title          | pause   |    1.62 |  |
| Title          | say     |    4.18 | Polars-genson infers the schema of JSON data, including Map types. |
| Title          | pause   |    1.28 |  |
| Records        | written |         | In some objects, the keys are part of the type. A person's name and date of birth are fields: every person has them, and they hold different types of value: one's a string and one's a date. They can't share one value type, so you'd store them in a struct. |
| Records        | pause   |    0.80 |  |
| Records        | say     |    2.76 | In some objects, the keys are part of the type. |
| Records        | pause   |    1.08 |  |
| Records        | say     |    2.66 | A person's name and date of birth are fields: |
| Records        | pause   |    0.52 |  |
| Records        | say     |    1.32 | every person has them, |
| Records        | pause   |    0.00 |  |
| Records        | say     |    1.82 | and they hold different types of value: |
| Records        | pause   |    1.00 |  |
| Records        | say     |    2.10 | one's a string and one's a date. |
| Records        | pause   |    0.58 |  |
| Records        | say     |    1.76 | They can't share one value type, |
| Records        | pause   |    0.00 |  |
| Records        | say     |    1.46 | so you'd store them in a struct. |
| Records        | pause   |    2.68 |  |
| Maps           | written |         | In other objects, the keys are data, like these labels from Wikidata, with one label for each language. Wikidata has labels in hundreds of languages, and each city has labels in only some of them, and every label is a string. So that's a map. |
| Maps           | pause   |    0.80 |  |
| Maps           | say     |    2.00 | In other objects, the keys are data, |
| Maps           | pause   |    0.50 |  |
| Maps           | say     |    1.98 | like these labels from Wikidata, |
| Maps           | pause   |    0.00 |  |
| Maps           | say     |    1.60 | with one label for each language. |
| Maps           | pause   |    1.80 |  |
| Maps           | say     |    1.96 | Wikidata has labels in hundreds of languages, |
| Maps           | pause   |    0.00 |  |
| Maps           | say     |    3.26 | and each city has labels in only some of them, |
| Maps           | pause   |    0.96 |  |
| Maps           | say     |    1.40 | and every label is a string. |
| Maps           | pause   |    0.00 |  |
| Maps           | say     |    2.02 | So that's a map. |
| Maps           | pause   |    3.30 |  |
| As a struct    | written |         | If you stored these as a struct, with a field for every language, most of the values would be null, and every new language would add another field to the type. |
| As a struct    | pause   |    0.80 |  |
| As a struct    | say     |    3.34 | If you stored these as a struct, with a field for every language, |
| As a struct    | pause   |    0.00 |  |
| As a struct    | say     |    2.66 | most of the values would be null, |
| As a struct    | pause   |    1.02 |  |
| As a struct    | say     |    4.08 | and every new language would add another field to the type. |
| As a struct    | pause   |    3.42 |  |
| As a map       | written |         | If you store them as a map instead, Paris has five labels and Rome has just two, and there are no nulls anywhere. The column is a map with string keys and string values, however many languages there are. |
| As a map       | pause   |    0.80 |  |
| As a map       | say     |    2.50 | If you store them as a map instead, |
| As a map       | pause   |    1.18 |  |
| As a map       | say     |    3.42 | Paris has five labels and Rome has just two, |
| As a map       | pause   |    0.00 |  |
| As a map       | say     |    2.20 | and there are no nulls anywhere. |
| As a map       | pause   |    0.74 |  |
| As a map       | say     |    3.28 | The column is a map with string keys and string values, |
| As a map       | pause   |    0.00 |  |
| As a map       | say     |    1.96 | however many languages there are. |
| As a map       | pause   |    3.18 |  |
| Map keys       | written |         | In Polars, a map's keys can be of any type that's row-encodable, which means anything except Object. JSON object keys are always strings, so polars-genson's maps have string keys. |
| Map keys       | pause   |    0.80 |  |
| Map keys       | say     |    3.30 | In Polars, a map's keys can be of any type |
| Map keys       | pause   |    0.00 |  |
| Map keys       | say     |    1.22 | that's row-encodable, |
| Map keys       | pause   |    0.00 |  |
| Map keys       | say     |    2.08 | which means anything except Object. |
| Map keys       | pause   |    1.14 |  |
| Map keys       | say     |    1.80 | JSON object keys are always strings, |
| Map keys       | pause   |    0.00 |  |
| Map keys       | say     |    2.76 | so polars-genson's maps have string keys. |
| Map keys       | pause   |    3.32 |  |
| Naming maps    | written |         | polars-genson infers a schema from your JSON. You can choose which fields become maps with force_field_types. |
| Naming maps    | pause   |    0.80 |  |
| Naming maps    | say     |    2.94 | polars-genson infers a schema from your JSON. |
| Naming maps    | pause   |    0.54 |  |
| Naming maps    | say     |    1.58 | You can choose which fields become maps |
| Naming maps    | pause   |    0.00 |  |
| Naming maps    | say     |    1.58 | with force_field_types. |
| Naming maps    | pause   |    4.26 |  |
| Inferring maps | written |         | Or you can let it decide from the data. An object whose values all share one type and that has more distinct keys than map_threshold becomes a map. The default threshold is 20, so these labels, with 8 keys, stay a struct. Lower it to 5, and they become a map. |
| Inferring maps | pause   |    0.80 |  |
| Inferring maps | say     |    3.72 | Or you can let it decide from the data. |
| Inferring maps | pause   |    0.52 |  |
| Inferring maps | say     |    2.36 | An object whose values all share one type |
| Inferring maps | pause   |    0.00 |  |
| Inferring maps | say     |    3.92 | and that has more distinct keys than map_threshold becomes a map. |
| Inferring maps | pause   |    1.94 |  |
| Inferring maps | say     |    1.54 | The default threshold is 20, |
| Inferring maps | pause   |    0.00 |  |
| Inferring maps | say     |    3.38 | so these labels, with 8 keys, stay a struct. |
| Inferring maps | pause   |    1.22 |  |
| Inferring maps | say     |    2.12 | Lower it to 5, and they become a map. |
| Inferring maps | pause   |    4.60 |  |
| Map functions  | written |         | Polars has functions for working with Map columns. map.get looks up a key: here, English, and here, Spanish, which two of the cities don't have. And map.len counts each city's labels. |
| Map functions  | pause   |    0.80 |  |
| Map functions  | say     |    2.26 | Polars has functions for working with Map columns. |
| Map functions  | pause   |    0.58 |  |
| Map functions  | say     |    2.56 | map.get looks up a key: here, English, |
| Map functions  | pause   |    0.00 |  |
| Map functions  | say     |    3.72 | and here, Spanish, which two of the cities don't have. |
| Map functions  | pause   |    0.84 |  |
| Map functions  | say     |    1.96 | And map.len counts each city's labels. |
| Map functions  | pause   |    2.28 |  |
| End card       | written |         | polars-genson 1.0 supports Polars 2.0, and you can install it from PyPI. See the docs to get started. |
| End card       | pause   |    0.80 |  |
| End card       | say     |    2.96 | polars-genson 1.0 supports Polars 2.0, |
| End card       | pause   |    0.00 |  |
| End card       | say     |    1.74 | and you can install it from PyPI. |
| End card       | pause   |    0.44 |  |
| End card       | say     |    1.30 | See the docs to get started. |
| End card       | pause   |    2.50 |  |
