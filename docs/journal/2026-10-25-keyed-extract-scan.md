# 2026-10-25: Rows decoded against a schema scanned only where a key can be a field

`normalise_from_parquet(avro_schema=..., extract_invariants=...)` scans a row for the
fields to extract only where one of its keys can be one of them (`extract::Scan::Keyed`,
#311). The other rows are kept as they are, unread, and go to the typed decoder.

## Why

wikidata-pq's claims call for a release passes both `avro_schema` and
`extract_invariants` (the label maps, `labels`, `property-labels` and `unit-labels`,
of docs/journal/2026-09-26-claims-label-maps.md). A release's claims carry no label maps,
so the scan of every row removed nothing:

- **The cost:** about a third of the call's CPU went on validating the column's text
  (375 MB on chunk 0) for fields that are not there, before the decoder read the same
  text again.
- **Locally** (4-core VM, fresh processes, chunk 0): the extraction stage took
  0.89–0.93 s of the call's 3.09–3.12 s of user CPU.

## The change

- **`Scan::Keyed`** (`extract_invariants_rows`): a row is scanned only where it holds a
  field's name in quotes (`"labels"`, found with `memmem`), or an escape that can spell
  one of a name's characters. That is `\u`, or any backslash where a name has a quote,
  backslash, slash or control character (no name in wikidata-pq's spec has one).
- **`normalise_from_parquet`:** with `avro_schema` the rows are extracted with
  `Scan::Keyed` and decoded against the schema. Where they do not all fit it, the slim
  rows are scanned again with `Scan::All` and inferred, as before.
- **Borrowed when unchanged:** a scanned row with nothing removed is borrowed by the scan
  (`Scanned::slim` is a `Cow`) rather than built again.

## Why it is exact

- **No key, no field:** a key spells a name either as the name's own characters in
  quotes or through an escape, and a row with neither holds no key that is a field. It
  has nothing to remove, so its slim row is its text, and the lookup gets nothing from
  it. A name that appears only in a value is scanned anyway, which costs time, not
  correctness.
- **The same `Value`s:** each slim row has the `Value` that scanning every row gives it,
  and the lookup is the same, entries in the same order.
- **Where the text differs:** only in a row the scan gives up on, with a key given twice
  or a line break, which `Scan::All` writes again with `serde_json` and `Keyed` keeps as it
  is. The typed decoder reads either as the same `Value`.
- **Inference never sees a `Keyed` row:** it reads a repeated key's values and can split
  rows at line breaks, so where the rows do not fit the schema they are scanned again
  first. A row already scanned is left as it is, so the second scan gives what scanning
  every row once gave.

## The memory pitfall

- **The first version (61253e6)** kept the unscanned rows borrowed from the read column
  through the decode. That kept the whole read column alive, and the bench's worker peak
  RSS rose by 68 MB (+4.0% ±1.3%).
- **The fix (b1bcb49):** the slim rows are owned (`Extracted::from` the borrowed rows),
  and the read rows are released after extraction, as before. The worker's peak is
  unchanged.

## Measured

wikidata-pq's bench, quick tier (4 chunks, 33,220 entities), 3 rounds, #311's CI wheel
(b1bcb49) against the released 1.4.0 (lmmx/wikidata-pq#92, closed, measurement only):

| | 1.4.0 | #311 | change |
|---|---|---|---|
| `process.normalise_in_schema` | 9.8 s (48%) | 7.5 s (41%) | **-23.6% ±3.5%** |
| CPU, process | 16.1 s ± 2.4 s | 13.9 s ± 2.0 s | **-2.3 s (-13.6% ±1.0%)** |
| ETA: pipeline | 7h33m41s | 6h58m53s | **-34m48s (-7.6% ±1.1%)** |
| peak RSS, one worker (GB) | 1.710 ± 0.039 | 1.689 ± 0.013 | within noise |
| parity | | | passed |

- **The baseline:** genson master before #311 (2b883ff) against 1.4.0 changes nothing
  beyond noise in the pipeline (lmmx/wikidata-pq#91), so the gain above is #311's.
- **Locally:** the claims call's user CPU on chunk 0 went 3.09–3.12 s → 2.17–2.42 s,
  and its extraction stage 0.89–0.93 s → 0.11 s. Every output table of wikidata-pq's
  process step is the same, on chunks c0 and s90.

Tests: `test_keyed_scan_matches_scanning_every_row` (3,000 random sets of rows, with
label maps with and without their ids, repeated and escaped keys, line breaks and invalid
rows: the same `Value`s, lookup entries and errors as `Scan::All`),
`test_keyed_scan_extracts_a_field_spelt_with_an_escape`, and, in Python,
`test_extracted_rows_decode_as_inferred`.

## Missing

- The rest of the claims call is the decode itself, about 70% of it after #311.
