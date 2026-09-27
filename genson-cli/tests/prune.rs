use assert_cmd::Command;
use serde_json::{json, Value};
use std::fs;

const FIXTURE: &str = "tests/data/claims_prune.jsonl";

/// Snaks on deleted properties, in each form the Wikidata claims source has them
const PRUNE: &str = "mainsnak__string,P450__string,value,error";

/// Normalise the claims fixture with the Wikidata claims options, returning the rows and
/// (with `prune`) the pruned values
fn normalise(prune: bool) -> (Vec<Value>, Vec<Value>) {
    let out_dir = tempfile::tempdir().unwrap();
    let pruned_path = out_dir.path().join("pruned.jsonl");
    let mut args = vec![
        "--ndjson",
        "--map-threshold",
        "0",
        "--unify-maps",
        "--force-type",
        "mainsnak:record",
        "--force-scalar-promotion",
        "mainsnak,datavalue",
        "--no-unify",
        "qualifiers",
        "--wrap-root",
        "claims",
        "--map-encoding",
        "kv",
        "--normalise",
    ];
    let pruned_arg = pruned_path.to_str().unwrap();
    if prune {
        args.extend(["--prune", PRUNE, "--prune-output", pruned_arg]);
    }
    args.push(FIXTURE);
    let output = Command::cargo_bin("genson-cli")
        .unwrap()
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success());
    let parse = |s: &str| -> Vec<Value> {
        s.lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    };
    let rows = parse(&String::from_utf8(output.stdout).unwrap());
    let pruned = if prune {
        parse(&fs::read_to_string(pruned_path).unwrap())
    } else {
        Vec::new()
    };
    (rows, pruned)
}

fn has_field(v: &Value, name: &str) -> bool {
    match v {
        Value::Object(m) => m.contains_key(name) || m.values().any(|v| has_field(v, name)),
        Value::Array(a) => a.iter().any(|v| has_field(v, name)),
        _ => false,
    }
}

fn datavalues(v: &Value) -> Vec<&Value> {
    match v {
        Value::Object(m) => m
            .iter()
            .flat_map(|(k, v)| {
                let mut found = datavalues(v);
                if k == "datavalue" {
                    found.push(v);
                }
                found
            })
            .collect(),
        Value::Array(a) => a.iter().flat_map(datavalues).collect(),
        _ => Vec::new(),
    }
}

/// The claims map of a row as {property: statements}
fn claims(row: &Value) -> Vec<(&str, &Value)> {
    row["claims"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| (e["key"].as_str().unwrap(), &e["value"]))
        .collect()
}

#[test]
fn test_prune_claims_bad_snaks() {
    let (rows, pruned) = normalise(true);
    assert_eq!(rows.len(), 7);
    for name in ["mainsnak__string", "P450__string"] {
        assert!(
            rows.iter().all(|r| !has_field(r, name)),
            "{name} left in rows"
        );
    }
    // In the kv map encoding every map entry has a `value`, so look in the datavalues
    for name in ["value", "error"] {
        assert!(
            rows.iter()
                .all(|r| !datavalues(r).iter().any(|d| d.get(name).is_some())),
            "{name} left in a datavalue"
        );
    }
    let statement = |row: usize, prop: &str| {
        claims(&rows[row])
            .into_iter()
            .find(|(k, _)| *k == prop)
            .map(|(_, v)| v[0].clone())
    };

    // Row 0 has no bad snak: its qualifier and reference are kept
    let s = statement(0, "P31").unwrap();
    assert_eq!(s["qualifiers"][0]["key"], "P580");
    assert_eq!(s["references"][0][0]["key"], "P854");
    // Row 1: a bare string mainsnak takes its statement with it, the other is kept
    assert!(statement(1, "P450").is_none());
    assert!(statement(1, "P31").is_some());
    // Row 2: a qualifier snak with a datavalue error goes, its sibling group stays
    let quals = statement(2, "P793").unwrap()["qualifiers"].clone();
    assert_eq!(quals.as_array().unwrap().len(), 1);
    assert_eq!(quals[0]["key"], "P580");
    // Row 3: a bare string qualifier snak goes, leaving the qualifiers empty
    assert_eq!(statement(3, "P375").unwrap()["qualifiers"], Value::Null);
    // Row 4: a reference emptied by pruning goes, the other reference stays
    let refs = statement(4, "P31").unwrap()["references"].clone();
    assert_eq!(refs.as_array().unwrap().len(), 1);
    assert_eq!(refs[0][0]["key"], "P854");
    // Row 5: pruning the only statement empties the claims
    assert_eq!(rows[5]["claims"], Value::Null);
    // Row 6: a bare string reference snak goes, its sibling group stays
    let refs = statement(6, "P31").unwrap()["references"].clone();
    assert_eq!(refs[0].as_array().unwrap().len(), 1);
    assert_eq!(refs[0][0]["key"], "P854");

    // Each pruned value is recorded once, as it was in the input
    let error =
        json!({"property": "P450", "datavalue": {"value": "Q2", "error": "property not found"}});
    assert_eq!(
        pruned,
        vec![
            json!({"row": 1, "path": ["claims", "P450", 0], "value": {"mainsnak": "P450", "rank": "normal"}}),
            json!({"row": 2, "path": ["claims", "P793", 0, "qualifiers", "P450", 0], "value": error}),
            json!({"row": 3, "path": ["claims", "P375", 0, "qualifiers", "P450", 0], "value": "P450"}),
            json!({"row": 4, "path": ["claims", "P31", 0, "references", 0, "P450", 0], "value": {"property": "P450", "datavalue": {"value": "Q4", "error": "property not found"}}}),
            json!({"row": 5, "path": ["claims", "P450", 0], "value": {"mainsnak": {"property": "P450", "datavalue": {"value": "Q6", "error": "property not found"}}, "rank": "normal"}}),
            json!({"row": 6, "path": ["claims", "P31", 0, "references", 0, "P450", 0], "value": "P450"}),
        ]
    );
}

#[test]
fn test_without_prune_bad_snaks_kept() {
    let (rows, _) = normalise(false);
    assert!(rows.iter().any(|r| has_field(r, "mainsnak__string")));
    assert!(rows.iter().any(|r| has_field(r, "error")));
    assert_eq!(claims(&rows[5]).len(), 1);
}

/// Names are the schema's own: a scalar among records in an array goes by the name
/// inference gives it (`__string` for items of a root-level array), not one derived from
/// the enclosing field
#[test]
fn test_prune_by_inferred_names() {
    let output = Command::cargo_bin("genson-cli")
        .unwrap()
        .args([
            "--ndjson",
            "--normalise",
            "--prune",
            "error,calibration__string,__string",
            "tests/data/prune_sensors.jsonl",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let rows: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        rows,
        vec![
            json!({"sensor": "a", "readings": [{"t": 1, "value": 3.2, "calibration": {"offset": 0.1}}]}),
            json!({"sensor": "b", "readings": null}),
            json!({"sensor": "c", "readings": [{"t": 1, "value": 1.5, "calibration": null}]}),
        ]
    );
}
