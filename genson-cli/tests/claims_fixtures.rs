use assert_cmd::Command;
use insta::{assert_snapshot, with_settings};
use std::fs;

/// Check if the current output matches the verified/blessed version
fn is_output_approved(snapshot_name: &str, output: &str) -> bool {
    let module_file = file!();
    let module_stem = std::path::Path::new(module_file)
        .file_stem()
        .unwrap()
        .to_string_lossy();
    let verified_path = format!("tests/verified/{}__{}.snap", module_stem, snapshot_name);

    if let Ok(verified_content) = fs::read_to_string(&verified_path) {
        if let Some(header_end) = verified_content.find("\n---\n") {
            let verified_output = &verified_content[header_end + 5..];
            return verified_output.trim() == output.trim();
        }
    }
    false
}

/// Run genson-cli with claims fixture from disk
fn run_genson_claims_fixture_from_disk(fixture_path: &str, name: &str, extra_args: &[&str]) {
    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    let mut args = vec![
        "--map-threshold",
        "0",
        "--unify-maps",
        "--wrap-root",
        "claims",
    ];
    // Add --ndjson flag if the fixture is a .jsonl file
    if fixture_path.ends_with(".jsonl") {
        args.push("--ndjson");
    }
    args.extend_from_slice(extra_args);
    args.push(fixture_path);
    let args_for_metadata = args.clone();
    cmd.args(args);

    let assert_output = cmd.assert().success();
    let output = assert_output.get_output();
    let stdout_str = String::from_utf8(output.stdout.clone()).unwrap();

    // Let stderr be visible if there's any debug output
    if !output.stderr.is_empty() {
        let stderr_str = String::from_utf8_lossy(&output.stderr);
        eprintln!("stderr from {}: {}", name, stderr_str);
    }

    let approved = is_output_approved(name, &stdout_str);

    with_settings!({
        info => &serde_json::json!({
            "approved": approved,
            "args": args_for_metadata[..args_for_metadata.len()-1], // exclude file path
            "fixture": fixture_path
        })
    }, {
        assert_snapshot!(name, stdout_str);
    });
}

#[test]
fn test_claims_fixture_l1_avro() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims_fixture_x4_L1.jsonl",
        "claims_fixture_l1__avro",
        &["--avro"],
    );
}

#[test]
fn test_claims_fixture_l1_jsonschema() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims_fixture_x4_L1.jsonl",
        "claims_fixture_l1__jsonschema",
        &[],
    );
}

#[test]
fn test_claims_fixture_l1_normalize() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims_fixture_x4_L1.jsonl",
        "claims_fixture_l1__normalize",
        &["--normalise"],
    );
}

#[test]
fn test_claims_fixture_l2_avro() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims_fixture_x4_L2.jsonl",
        "claims_fixture_l2__avro",
        &["--avro"],
    );
}

#[test]
fn test_claims_fixture_l2_jsonschema() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims_fixture_x4_L2.jsonl",
        "claims_fixture_l2__jsonschema",
        &[],
    );
}

#[test]
fn test_claims_fixture_l2_normalize() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims_fixture_x4_L2.jsonl",
        "claims_fixture_l2__normalize",
        &["--normalise"],
    );
}

#[test]
fn test_claims_fixture_l3_avro() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims_fixture_x4_L3.jsonl",
        "claims_fixture_l3__avro",
        &["--avro"],
    );
}

#[test]
fn test_claims_fixture_l3_jsonschema() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims_fixture_x4_L3.jsonl",
        "claims_fixture_l3__jsonschema",
        &[],
    );
}

#[test]
fn test_claims_fixture_l3_normalize() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims_fixture_x4_L3.jsonl",
        "claims_fixture_l3__normalize",
        &["--normalise"],
    );
}

#[test]
fn test_claims_fixture_l4_avro() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims_fixture_x4_L4.jsonl",
        "claims_fixture_l4__avro",
        &["--avro"],
    );
}

#[test]
fn test_claims_fixture_l4_jsonschema() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims_fixture_x4_L4.jsonl",
        "claims_fixture_l4__jsonschema",
        &[],
    );
}

#[test]
fn test_claims_fixture_l4_normalize() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims_fixture_x4_L4.jsonl",
        "claims_fixture_l4__normalize",
        &["--normalise"],
    );
}

// Following are numbered from the full x1818 fixture

#[test]
fn test_claims_fixture_x1818_l4_avro() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims/x1818_L4_MINIMAL.json",
        "claims_fixture_x1818_l4__avro",
        &["--avro"],
    );
}

#[test]
fn test_claims_fixture_x1818_l4_jsonschema() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims/x1818_L4_MINIMAL.json",
        "claims_fixture_x1818_l4__jsonschema",
        &[],
    );
}

#[test]
fn test_claims_fixture_x1818_l4_normalize() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims/x1818_L4_MINIMAL.json",
        "claims_fixture_x1818_l4__normalize",
        &["--normalise"],
    );
}

#[test]
fn test_claims_fixture_x1818_l5_avro() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims/x1818_L5_MINIMAL.json",
        "claims_fixture_x1818_l5__avro",
        &["--avro"],
    );
}

#[test]
fn test_claims_fixture_x1818_l5_jsonschema() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims/x1818_L5_MINIMAL.json",
        "claims_fixture_x1818_l5__jsonschema",
        &[],
    );
}

#[test]
fn test_claims_fixture_x1818_l5_normalize() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims/x1818_L5_MINIMAL.json",
        "claims_fixture_x1818_l5__normalize",
        &["--normalise"],
    );
}

#[test]
fn test_claims_fixture_x1818_l12_avro() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims/x1818_L12_MINIMAL.json",
        "claims_fixture_x1818_l12__avro",
        &["--avro"],
    );
}

#[test]
fn test_claims_fixture_x1818_l12_jsonschema() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims/x1818_L12_MINIMAL.json",
        "claims_fixture_x1818_l12__jsonschema",
        &[],
    );
}

#[test]
fn test_claims_fixture_x1818_l12_normalize() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims/x1818_L12_MINIMAL.json",
        "claims_fixture_x1818_l12__normalize",
        &["--normalise"],
    );
}

#[test]
fn test_claims_fixture_x1818_l14_avro() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims/x1818_L14_MINIMAL.json",
        "claims_fixture_x1818_l14__avro",
        &["--avro"],
    );
}

#[test]
fn test_claims_fixture_x1818_l14_jsonschema() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims/x1818_L14_MINIMAL.json",
        "claims_fixture_x1818_l14__jsonschema",
        &[],
    );
}

#[test]
fn test_claims_fixture_x1818_l14_normalize() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims/x1818_L14_MINIMAL.json",
        "claims_fixture_x1818_l14__normalize",
        &["--normalise"],
    );
}

#[test]
fn test_claims_fixture_x1818_l16_avro() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims/x1818_L16_MINIMAL.json",
        "claims_fixture_x1818_l16__avro",
        &["--avro"],
    );
}

#[test]
fn test_claims_fixture_x1818_l16_jsonschema() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims/x1818_L16_MINIMAL.json",
        "claims_fixture_x1818_l16__jsonschema",
        &[],
    );
}

#[test]
fn test_claims_fixture_x1818_l16_normalize() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims/x1818_L16_MINIMAL.json",
        "claims_fixture_x1818_l16__normalize",
        &["--normalise"],
    );
}

#[test]
fn test_claims_fixture_x1818_l26_avro() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims/x1818_L26_MINIMAL.json",
        "claims_fixture_x1818_l26__avro",
        &["--avro"],
    );
}

#[test]
fn test_claims_fixture_x1818_l26_jsonschema() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims/x1818_L26_MINIMAL.json",
        "claims_fixture_x1818_l26__jsonschema",
        &[],
    );
}

#[test]
fn test_claims_fixture_x1818_l26_normalize() {
    run_genson_claims_fixture_from_disk(
        "tests/data/claims/x1818_L26_MINIMAL.json",
        "claims_fixture_x1818_l26__normalize",
        &["--normalise"],
    );
}
