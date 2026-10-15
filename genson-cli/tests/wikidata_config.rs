//! Snapshots of the claims fixtures under the options wikidata-pq infers its claims with
//! (`CLAIMS_INFERENCE_OPTIONS` and `max_builders=100` in wikidata-pq's `process.py`): each
//! fixture file read whole, which genson-core takes as one string (the sequential path),
//! and 16 of their rows read from a Parquet column, one string per row (the parallel path,
//! where rows are built in chunks and duplicate row schemas dropped). The parallel path is
//! also checked to give the same output whatever the chunk size and thread count.
use assert_cmd::Command;
use genson_core::parquet::write_string_column;
use insta::{assert_snapshot, with_settings};
use std::fs;
use tempfile::TempDir;

const WIKIDATA_ARGS: &[&str] = &[
    "--ndjson",
    "--map-threshold",
    "0",
    "--unify-maps",
    "--wrap-root",
    "claims",
    "--force-type",
    "mainsnak:record",
    "--force-scalar-promotion",
    "mainsnak,datavalue,precision,latitude,longitude",
    "--no-unify",
    "qualifiers",
];

/// Run genson-cli on `fixture` with the wikidata options, `--max-builders max_builders`,
/// and `extra_args`, with `threads` rayon threads if given, and return its stdout.
fn run(fixture: &str, max_builders: usize, extra_args: &[&str], threads: Option<usize>) -> String {
    let max_builders = max_builders.to_string();
    let mut cmd = Command::cargo_bin("genson-cli").unwrap();
    cmd.args(WIKIDATA_ARGS)
        .args(["--max-builders", &max_builders])
        .args(extra_args)
        .arg(fixture);
    if let Some(threads) = threads {
        cmd.env("RAYON_NUM_THREADS", threads.to_string());
    }
    let output = cmd.assert().success().get_output().clone();
    String::from_utf8(output.stdout).unwrap()
}

fn snapshot(fixture: &str, name: &str, extra_args: &[&str]) {
    let stdout = run(fixture, 100, extra_args, None);
    let args = [WIKIDATA_ARGS, &["--max-builders", "100"], extra_args].concat();
    // The tests sit in one module per fixture, so the snapshot names carry the file stem
    // themselves rather than the module path
    with_settings!({
        prepend_module_to_snapshot => false,
        info => &serde_json::json!({
            "args": args,
            "fixture": fixture,
        })
    }, {
        assert_snapshot!(format!("wikidata_config__{name}"), stdout);
    });
}

macro_rules! snapshot_tests {
    ($($name:ident => $fixture:literal),* $(,)?) => {
        mod snapshots {
            use super::snapshot;
            $(
                mod $name {
                    use super::snapshot;

                    #[test]
                    fn jsonschema() {
                        snapshot($fixture, concat!(stringify!($name), "__jsonschema"), &[]);
                    }

                    #[test]
                    fn avro() {
                        snapshot($fixture, concat!(stringify!($name), "__avro"), &["--avro"]);
                    }
                }
            )*
        }
    };
}

snapshot_tests! {
    x4 => "tests/data/claims_fixture_x4.jsonl",
    x30 => "tests/data/claims_fixture_x30.jsonl",
    x1818_l4 => "tests/data/claims/x1818_L4.jsonl",
    x1818_l5 => "tests/data/claims/x1818_L5.jsonl",
    x1818_l12 => "tests/data/claims/x1818_L12.jsonl",
    x1818_l14 => "tests/data/claims/x1818_L14.jsonl",
    x1818_l16 => "tests/data/claims/x1818_L16.jsonl",
    x1818_l26 => "tests/data/claims/x1818_L26.jsonl",
    x1818_l29 => "tests/data/claims/x1818_L29.jsonl",
}

/// The rows of the input to the parallel-path tests: 16 rows, enough for the parallel path
/// (10 or more strings), with the 4 rows of `claims_fixture_x4.jsonl` given twice so the
/// duplicate row schemas are dropped. The rows go in a Parquet string column read with
/// `--pq-column`, one string per row as wikidata-pq passes them; a file read without
/// `--pq-column` is one string, which takes the sequential path.
const MIXED_SOURCES: &[&str] = &[
    "tests/data/claims_fixture_x4.jsonl",
    "tests/data/claims_fixture_x4.jsonl",
    "tests/data/claims_fixture_x4_L1.jsonl",
    "tests/data/claims_fixture_x4_L2.jsonl",
    "tests/data/claims_fixture_x4_L3.jsonl",
    "tests/data/claims_fixture_x4_L4.jsonl",
    "tests/data/claims/chunk_0-00024-of-00546_ddminv2_REDUCED.jsonl",
    "tests/data/claims/chunk_0-00024-of-00546_x750_t30_REDUCED_REWRITTEN.jsonl",
];

/// The rows of `MIXED_SOURCES` as a Parquet file with one string column, `claims`.
fn mixed_rows() -> (TempDir, String) {
    let rows: Vec<String> = MIXED_SOURCES
        .iter()
        .flat_map(|source| {
            let text = fs::read_to_string(source).unwrap();
            text.lines().map(str::to_string).collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(rows.len(), 16);
    let dir = TempDir::new().unwrap();
    let path = dir
        .path()
        .join("mixed.parquet")
        .to_str()
        .unwrap()
        .to_string();
    write_string_column(&path, "claims", rows, None).unwrap();
    (dir, path)
}

const PQ_COLUMN: &[&str] = &["--pq-column", "claims"];

fn mixed_snapshot(name: &str, extra_args: &[&str]) {
    let (_dir, rows) = mixed_rows();
    let extra_args = [PQ_COLUMN, extra_args].concat();
    let stdout = run(&rows, 100, &extra_args, None);
    let args = [WIKIDATA_ARGS, &["--max-builders", "100"], &extra_args].concat();
    with_settings!({
        prepend_module_to_snapshot => false,
        info => &serde_json::json!({
            "args": args,
            "sources": MIXED_SOURCES,
        })
    }, {
        assert_snapshot!(format!("wikidata_config__mixed__{name}"), stdout);
    });
}

#[test]
fn mixed_jsonschema() {
    mixed_snapshot("jsonschema", &[]);
}

#[test]
fn mixed_avro() {
    mixed_snapshot("avro", &["--avro"]);
}

#[test]
fn max_builders_does_not_change_output() {
    let (_dir, rows) = mixed_rows();
    for extra_args in [PQ_COLUMN.to_vec(), [PQ_COLUMN, &["--avro"]].concat()] {
        let expected = run(&rows, 100, &extra_args, None);
        for max_builders in [1, 3, 7] {
            assert_eq!(
                run(&rows, max_builders, &extra_args, None),
                expected,
                "{extra_args:?} with --max-builders {max_builders}"
            );
        }
    }
}

#[test]
fn thread_count_does_not_change_output() {
    let (_dir, rows) = mixed_rows();
    for extra_args in [PQ_COLUMN.to_vec(), [PQ_COLUMN, &["--avro"]].concat()] {
        let expected = run(&rows, 100, &extra_args, None);
        for threads in [1, 4] {
            assert_eq!(
                run(&rows, 100, &extra_args, Some(threads)),
                expected,
                "{extra_args:?} with {threads} threads"
            );
        }
    }
}
