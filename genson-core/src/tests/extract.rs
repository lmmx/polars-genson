// genson-core/src/tests/extract.rs
use super::*;
use serde_json::json;

fn spec() -> Vec<(String, String)> {
    vec![
        ("labels".to_string(), "id".to_string()),
        ("property-labels".to_string(), "property".to_string()),
    ]
}

fn rows(values: &[Value]) -> Vec<Option<String>> {
    values.iter().map(|v| Some(v.to_string())).collect()
}

fn parse(row: &Option<String>) -> Value {
    serde_json::from_str(row.as_deref().unwrap()).unwrap()
}

#[test]
fn test_removes_fields_at_any_depth_and_dedups() {
    let input = rows(&[
        json!({"P31": [{"mainsnak": {"property": "P31", "property-labels": {"en": "instance of"},
            "datavalue": {"id": "Q5", "labels": {"en": "human"}}}}]}),
        json!({"P31": [{"mainsnak": {"property": "P31", "property-labels": {"en": "instance of"},
            "datavalue": {"id": "Q5", "labels": {"en": "human"}}}}]}),
    ]);
    let out = extract_invariants(input, &spec()).unwrap();

    let slim = json!({"P31": [{"mainsnak": {"property": "P31", "datavalue": {"id": "Q5"}}}]});
    assert_eq!(parse(&out.rows[0]), slim);
    assert_eq!(parse(&out.rows[1]), slim);
    assert_eq!(
        out.lookup,
        vec![
            LookupEntry {
                field: "property-labels".into(),
                key: "P31".into(),
                value: json!({"en": "instance of"}),
            },
            LookupEntry {
                field: "labels".into(),
                key: "Q5".into(),
                value: json!({"en": "human"}),
            },
        ]
    );
}

#[test]
fn test_field_without_key_is_left_in_place() {
    let input = rows(&[json!({"labels": {"en": "x"}, "other": 1})]);
    let out = extract_invariants(input, &spec()).unwrap();
    assert_eq!(parse(&out.rows[0]), json!({"labels": {"en": "x"}, "other": 1}));
    assert!(out.lookup.is_empty());
}

#[test]
fn test_conflicting_subtrees_error() {
    let input = rows(&[
        json!({"id": "Q5", "labels": {"en": "human"}}),
        json!({"id": "Q5", "labels": {"en": "person"}}),
    ]);
    let err = extract_invariants(input, &spec()).unwrap_err();
    assert!(
        err.contains("field 'labels' is not invariant for its determinant 'id' ('Q5'"),
        "{err}"
    );
}

#[test]
fn test_null_and_invalid_rows_pass_through() {
    let input = vec![None, Some("not json".to_string())];
    let out = extract_invariants(input, &spec()).unwrap();
    assert_eq!(out.rows, vec![None, Some("not json".to_string())]);
}

#[test]
fn test_key_order_of_remaining_fields_is_kept() {
    let input = vec![Some(r#"{"b": 1, "labels": {"en": "x"}, "id": "Q1", "a": 2}"#.to_string())];
    let out = extract_invariants(input.clone(), &spec()).unwrap();
    // The scan keeps the row's own text, the `Value` path writes it as serde_json does
    assert_eq!(out.rows[0].as_deref(), Some(r#"{"b": 1, "id": "Q1", "a": 2}"#));
    let by_value = extract_invariants_by_value(&input, &spec()).unwrap();
    assert_eq!(by_value.rows[0].as_deref(), Some(r#"{"b":1,"id":"Q1","a":2}"#));
}

#[test]
fn test_extract_from_borrowed_rows_matches_owned() {
    let owned: Vec<Option<String>> = vec![
        Some(r#"{"x": {"id": "Q1", "labels": {"en": "one"}}}"#.to_string()),
        None,
        Some("not json".to_string()),
        Some(r#"{"x": [{"id": "Q1", "labels": {"en": "one"}}, {"id": "Q2"}]}"#.to_string()),
    ];
    let borrowed: Vec<Option<&str>> = owned.iter().map(Option::as_deref).collect();
    let from_borrowed = extract_invariants_from(&borrowed, &spec()).unwrap();
    let from_owned = extract_invariants(owned, &spec()).unwrap();
    assert_eq!(from_borrowed.rows, from_owned.rows);
    assert_eq!(from_borrowed.lookup, from_owned.lookup);
}

/// A small deterministic generator for the differential tests below.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len() as u64) as usize]
    }
}

/// Space or tab between tokens, sometimes.
fn ws(rng: &mut Rng) -> &'static str {
    ["", "", "", " ", "  ", "\t"][rng.below(6) as usize]
}

/// A label map for `id` in one of a few versions, written with escapes and spacing.
fn label_map(rng: &mut Rng, id: &str) -> String {
    let version = rng.below(8);
    let text = |s: &str| format!(r#""{s}""#);
    let mut members = vec![
        format!("{}:{}{}", text("en"), ws(rng), text(&format!("name of {id}"))),
        format!(r#""fr":"café {id}""#),
        format!(r#""zh":"😀 {id}""#),
    ];
    if version == 0 {
        // A different value for the same id: not invariant
        members.push(r#""de":"anders""#.to_string());
    }
    if version == 1 {
        // The same value written differently: equal once parsed
        members.swap(0, 1);
    }
    format!("{{{}}}", members.join(&format!(",{}", ws(rng))))
}

fn scalar(rng: &mut Rng) -> String {
    rng.pick(&[
        "1",
        "-0",
        "2.5",
        "1e3",
        "1E-2",
        "12345678901234567890123",
        "true",
        "false",
        "null",
        r#""s""#,
        r#""tab\tquote\" slash\/""#,
        r#""\u0000""#,
    ])
    .to_string()
}

/// A random row: objects holding `labels` with or without their `id` (null, a number, an
/// escaped string), `property-labels` with `property`, and other keys, nested in objects
/// and arrays, with an occasional line break, repeated key or escaped key.
fn random_value(rng: &mut Rng, depth: u32) -> String {
    let pick = if depth == 0 { 3 } else { rng.below(4) };
    match pick {
        0 | 1 => {
            let mut members: Vec<String> = Vec::new();
            for _ in 0..rng.below(4) {
                let key = rng.pick(&["a", "b", "claims", "x"]);
                members.push(format!(r#""{key}":{}{}"#, ws(rng), random_value(rng, depth - 1)));
            }
            let id = ["Q1", "Q2", "Q3"][rng.below(3) as usize];
            match rng.below(9) {
                0 => {}
                1 => members.push(format!(r#""labels":{}"#, label_map(rng, id))),
                2 => members.push(r#""id":null"#.to_string()),
                3 => members.push(format!(r#""id":7,"labels":{}"#, label_map(rng, "7"))),
                4 => members.push(format!(r#""id":"Q1","labels":{}"#, label_map(rng, "Q1"))),
                5 => members.push(format!(r#""property":"P{}""#, rng.below(2))),
                6 => {
                    let p = format!("P{}", rng.below(2));
                    members.push(format!(r#""property-labels":{}"#, label_map(rng, &p)));
                    members.push(format!(r#""property":"{p}""#));
                }
                _ => {
                    members.push(format!(r#""id":"{id}""#));
                    members.push(format!(r#""labels":{}"#, label_map(rng, id)));
                }
            }
            let n = members.len() as u64;
            if n > 1 {
                // Removed fields first, last or between the others
                let i = rng.below(n) as usize;
                let j = rng.below(n) as usize;
                members.swap(i, j);
            }
            match rng.below(40) {
                0 => members.push(r#""a":1"#.to_string()),
                1 => members.push(r#""ab":1"#.to_string()),
                _ => {}
            }
            let sep = if rng.below(60) == 0 { ",\n" } else { "," };
            format!("{{{}{}{}}}", ws(rng), members.join(sep), ws(rng))
        }
        2 => {
            let items: Vec<String> = (0..rng.below(4)).map(|_| random_value(rng, depth - 1)).collect();
            format!("[{}]", items.join(&format!(",{}", ws(rng))))
        }
        _ => scalar(rng),
    }
}

/// The scan and the `Value` path give the same slim rows (equal values, keys in the same
/// order), the same lookup entries in the same order, and the same errors.
#[test]
fn test_scan_matches_the_value_path() {
    let spec = spec();
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let (mut scanned_rows, mut fallbacks, mut errors) = (0, 0, 0);
    for _ in 0..3000 {
        let rows: Vec<Option<String>> = (0..1 + rng.below(5))
            .map(|_| match rng.below(30) {
                0 => None,
                1 => Some(r#"{"a": 1e400}"#.to_string()),
                _ => Some(random_value(&mut rng, 4)),
            })
            .collect();
        for row in rows.iter().flatten() {
            match scan::scan(row, &spec) {
                Ok(_) => scanned_rows += 1,
                Err(_) => fallbacks += 1,
            }
        }
        let fast = extract_invariants_from(&rows, &spec);
        let slow = extract_invariants_by_value(&rows, &spec);
        let (fast, slow) = match (fast, slow) {
            (Err(a), Err(b)) => {
                assert_eq!(a, b, "{rows:?}");
                errors += 1;
                continue;
            }
            (Ok(a), Ok(b)) => (a, b),
            (a, b) => panic!("{rows:?}: {:?} against {:?}", a.err(), b.err()),
        };
        assert_eq!(fast.lookup, slow.lookup, "{rows:?}");
        for (f, s) in fast.rows.iter().zip(&slow.rows) {
            match (f, s) {
                (Some(f), Some(s)) => {
                    let parsed = |t: &str| serde_json::from_str::<Value>(t).ok().map(|v| v.to_string());
                    assert_eq!(parsed(f), parsed(s), "{f} against {s}");
                    if parsed(s).is_none() {
                        assert_eq!(f, s, "an invalid row passes through");
                    }
                    assert!(!f.contains('\n') || f == s, "{f}");
                }
                (f, s) => assert_eq!(f, s),
            }
        }
    }
    assert!(scanned_rows > 3000 && fallbacks > 300 && errors > 50, "{scanned_rows} {fallbacks} {errors}");
}

/// `Scan::Keyed` gives the slim rows `Scan::All` gives as `Value`s (an invalid row passing
/// through as it is), the same lookup entries in the same order, and the same errors,
/// with the rows in which no key can be a field kept as they are.
#[test]
fn test_keyed_scan_matches_scanning_every_row() {
    let spec = spec();
    let mut rng = Rng(0x6c07_8965_2a3b_91d7);
    let (mut kept, mut errors) = (0, 0);
    for _ in 0..3000 {
        let rows: Vec<Option<String>> = (0..1 + rng.below(5))
            .map(|_| match rng.below(30) {
                0 => None,
                1 => Some(r#"{"a": 1e400}"#.to_string()),
                // A field's name spelt with an escape, and a repeated key in a row without one
                2 => Some(r#"{"id":"Q1","l\u0061bels":{"en":"x"}}"#.to_string()),
                3 => Some(r#"{"a":1,"b":[2],"a":"x"}"#.to_string()),
                _ => Some(random_value(&mut rng, 4)),
            })
            .collect();
        let all = extract_invariants_rows(&rows, &spec, Scan::All);
        let keyed = extract_invariants_rows(&rows, &spec, Scan::Keyed);
        let (all, keyed) = match (all, keyed) {
            (Err(a), Err(b)) => {
                assert_eq!(a, b, "{rows:?}");
                errors += 1;
                continue;
            }
            (Ok(a), Ok(b)) => (a, b),
            (a, b) => panic!("{rows:?}: {:?} against {:?}", a.err(), b.err()),
        };
        assert!(!all.unscanned);
        assert_eq!(all.lookup, keyed.lookup, "{rows:?}");
        for ((a, k), row) in all.rows.iter().zip(&keyed.rows).zip(&rows) {
            match (a, k, row) {
                (Some(a), Some(k), Some(row)) => {
                    let parsed = |t: &str| serde_json::from_str::<Value>(t).ok().map(|v| v.to_string());
                    assert_eq!(parsed(a), parsed(k), "{a} against {k}");
                    if parsed(row).is_none() {
                        assert_eq!(k, row, "an invalid row passes through");
                    }
                    // No name in the spec has a character only a short escape spells
                    let keyed_row = row.contains("\\u")
                        || spec.iter().any(|(f, _)| row.contains(&format!("\"{f}\"")));
                    if !keyed_row {
                        assert!(matches!(k, Cow::Borrowed(_)) && k == row, "{row}");
                        kept += 1;
                    }
                }
                (a, k, _) => assert_eq!(a, k),
            }
        }
    }
    assert!(kept > 2000 && errors > 50, "{kept} {errors}");
}

#[test]
fn test_keyed_scan_extracts_a_field_spelt_with_an_escape() {
    let rows = vec![
        Some(r#"{"id":"Q1","l\u0061bels":{"en":"x"}}"#),
        Some(r#"{"a":1,"a":2}"#),
    ];
    let out = extract_invariants_rows(&rows, &spec(), Scan::Keyed).unwrap();
    assert_eq!(out.lookup.len(), 1);
    assert_eq!(parse(&out.rows[0].as_deref().map(String::from)), json!({"id": "Q1"}));
    // Kept as it is, where the scan would give up on the repeated key and write it again
    assert_eq!(out.rows[1].as_deref(), Some(r#"{"a":1,"a":2}"#));
    assert!(out.unscanned);
    let all = extract_invariants_rows(&rows, &spec(), Scan::All).unwrap();
    assert_eq!(all.rows[1].as_deref(), Some(r#"{"a":2}"#));
}

/// Whatever the scan accepts, serde_json accepts too: valid rows with one byte changed,
/// removed or inserted.
#[test]
fn test_scan_accepts_only_valid_json() {
    let spec = spec();
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let alphabet = b"{}[]:,\"\\/ \t0123456789-+.eEtrufalsnubdu\xc3\xa9";
    let mut accepted = 0;
    for _ in 0..20000 {
        let mut bytes = random_value(&mut rng, 3).into_bytes();
        for _ in 0..1 + rng.below(3) {
            let at = rng.below(bytes.len() as u64 + 1) as usize;
            let c = alphabet[rng.below(alphabet.len() as u64) as usize];
            match rng.below(3) {
                0 if at < bytes.len() => bytes[at] = c,
                1 if at < bytes.len() => {
                    bytes.remove(at);
                }
                _ => bytes.insert(at, c),
            }
        }
        let Ok(row) = String::from_utf8(bytes) else {
            continue;
        };
        if scan::scan(&row, &spec).is_ok() {
            accepted += 1;
            assert!(
                serde_json::from_str::<Value>(&row).is_ok(),
                "the scan accepts what serde_json rejects: {row}"
            );
        }
    }
    assert!(accepted > 1000, "{accepted}");
}

#[test]
fn test_scan_rejects_what_serde_json_rejects() {
    let spec = spec();
    for row in [
        r#""\ud800""#,
        r#""\ud800A""#,
        r#""\udc00""#,
        "\"a\u{1}b\"",
        r#""\x""#,
        "1e400",
        "1.8e308",
        "01",
        "1.",
        "[1,]",
        r#"{"a":1,}"#,
        "{} x",
        "\u{feff}{}",
        "nul",
        "",
    ] {
        assert!(serde_json::from_str::<Value>(row).is_err(), "{row}");
        assert!(scan::scan(row, &spec).is_err(), "{row}");
    }
    let deep = "[".repeat(128) + &"]".repeat(128);
    assert!(scan::scan(&deep, &spec).is_err());
    let deep = "[".repeat(127) + &"]".repeat(127);
    assert!(scan::scan(&deep, &spec).is_ok());
}

#[test]
fn test_scan_cuts_removed_fields_wherever_they_are() {
    let spec = spec();
    for (row, slim) in [
        (r#"{"labels":{"en":"x"},"id":"Q1"}"#, r#"{"id":"Q1"}"#),
        (r#"{"id":"Q1","labels":{"en":"x"}}"#, r#"{"id":"Q1"}"#),
        (r#"{"a":1, "labels":{"en":"x"} ,"id":"Q1","b":2}"#, r#"{"a":1, "id":"Q1","b":2}"#),
        (
            r#"{"labels":{},"property-labels":{},"id":"Q1","property":"P1"}"#,
            r#"{"id":"Q1","property":"P1"}"#,
        ),
        (
            r#"{"id":"Q1","property":"P1","labels":{},"property-labels":{}}"#,
            r#"{"id":"Q1","property":"P1"}"#,
        ),
        (r#"[{"id":"Q1","labels":1},{"x":{"labels":2,"id":"Q2"}}]"#, r#"[{"id":"Q1"},{"x":{"id":"Q2"}}]"#),
    ] {
        let scanned = scan::scan(row, &spec).unwrap();
        assert_eq!(scanned.slim, slim);
    }
    // Only fields to extract in an object
    let spec_one = vec![("labels".to_string(), "id".to_string()), ("id".to_string(), "z".to_string())];
    let scanned = scan::scan(r#"{"z":1,"id":"Q1","labels":{}}"#, &spec_one).unwrap();
    assert_eq!(scanned.slim, r#"{"z":1}"#);
}

/// A value a `known` text gives (as validated) is stepped over where the object has its
/// determinant before it and the row has that text there, and validated otherwise: here a
/// text that is not valid at all, so that stepping over it shows as the row accepted.
#[test]
fn test_scan_steps_over_known_values_after_their_determinant() {
    let spec = spec();
    let invalid = r#"{"en":"\ud800"}"#;
    let known = |field: usize, key: &str| (field == 0 && key == "Q1").then_some((invalid, 1));
    let row = format!(r#"{{"id":"Q1","labels":{invalid}}}"#);
    assert!(scan::scan(&row, &spec).is_err());
    let scanned = scan::scan_known(&row, &spec, &known).unwrap();
    assert_eq!(scanned.slim, r#"{"id":"Q1"}"#);
    assert_eq!(scanned.removed[0].nesting, 1);
    for row in [
        // The determinant after the value, another key, another field, other text
        format!(r#"{{"labels":{invalid},"id":"Q1"}}"#),
        format!(r#"{{"id":"Q2","labels":{invalid}}}"#),
        format!(r#"{{"property":"Q1","property-labels":{invalid}}}"#),
        r#"{"id":"Q1","labels":{"en":"\ud800" }}"#.to_string(),
        // An escaped determinant is read as the same key
        format!(r#"{{"id":"\u0051\u0031","labels":{invalid}}}"#),
    ] {
        let stepped = scan::scan_known(&row, &spec, &known).is_ok();
        assert_eq!(stepped, row.contains(r#""\u0051\u0031""#), "{row}");
    }
}

/// A value seen earlier (in this row or another) is stepped over only where its nesting
/// fits under the depth limit at this depth: deeper, it is validated, and the row then
/// read as a `Value` (which `serde_json` rejects, so it is kept as it is).
#[test]
fn test_known_values_too_deep_here_are_validated() {
    let spec = spec();
    let labels = "[".repeat(20) + &"]".repeat(20);
    let wrap = |depth: usize, inner: &str| "[".repeat(depth) + inner + &"]".repeat(depth);
    let object = format!(r#"{{"id":"Q1","labels":{labels}}}"#);
    for (depth, fits) in [(100, true), (106, true), (107, false), (120, false)] {
        // The object at `depth` lists: 1 + depth + 20 levels for the labels' innermost
        let deep = wrap(depth, &object);
        let in_one_row = format!(r#"[{object},{deep}]"#);
        let rows = vec![Some(object.clone()), Some(deep.clone()), Some(in_one_row)];
        let fast = extract_invariants_from(&rows, &spec).unwrap();
        let slow = extract_invariants_by_value(&rows, &spec).unwrap();
        assert_eq!(fast.lookup, slow.lookup);
        for (f, s) in fast.rows.iter().zip(&slow.rows) {
            let parsed = |t: &Option<String>| serde_json::from_str::<Value>(t.as_deref().unwrap()).ok();
            assert_eq!(parsed(f), parsed(s), "depth {depth}");
        }
        assert_eq!(fast.rows[1].as_deref() == Some(deep.as_str()), !fits, "depth {depth}");
    }
}

/// A number is never stepped over (its end depends on what follows it): a longer one with
/// the same start is a different value.
#[test]
fn test_known_numbers_are_validated() {
    let spec = spec();
    let rows = vec![
        Some(r#"{"id":"Q1","labels":12}"#.to_string()),
        Some(r#"{"id":"Q1","labels":123}"#.to_string()),
    ];
    let fast = extract_invariants_from(&rows, &spec);
    let slow = extract_invariants_by_value(&rows, &spec);
    assert!(fast.is_err());
    assert_eq!(fast.err(), slow.err());
    let known = |_: usize, _: &str| Some(("12", 0));
    assert!(scan::scan_known(r#"{"id":"Q1","labels":123}"#, &spec, &known).is_ok());
}
