use std::env;
use std::fs;
use std::io::{self, Read};

use genson_core::{
    infer_json_schema,
    normalise::{normalise_values, NormaliseConfig},
    SchemaInferenceConfig,
};
use serde_json::Value;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_cli()
}

// Extract the main logic into a separate function so we can call it from tests
fn run_cli() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    // Handle command line options
    let mut config = SchemaInferenceConfig::default();
    let mut input_file = None;

    // Normalisation config
    let mut do_normalise = false;
    let mut empty_as_null = true; // default ON
    let mut coerce_string = false; // default OFF

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            "--no-ignore-array" => {
                config.ignore_outer_array = false;
            }
            "--ndjson" => {
                config.delimiter = Some(b'\n');
            }
            "--avro" => {
                config.avro = true;
            }
            "--normalise" => {
                do_normalise = true;
                config.avro = true;
            }
            "--coerce-strings" => {
                coerce_string = true;
            }
            "--keep-empty" => {
                empty_as_null = false; // override default
            }
            "--map-threshold" => {
                if i + 1 < args.len() {
                    config.map_threshold = args[i + 1].parse::<usize>().map_err(|_| {
                        format!("Invalid value for --map-threshold: {}", args[i + 1])
                    })?;
                    i += 1;
                } else {
                    return Err("Missing value for --map-threshold".into());
                }
            }
            "--force-type" => {
                if i + 1 < args.len() {
                    for pair in args[i + 1].split(',') {
                        if let Some((field, typ)) = pair.split_once(':') {
                            config
                                .force_field_types
                                .insert(field.to_string(), typ.to_string());
                        }
                    }
                    i += 1;
                } else {
                    return Err("Missing value for --force-type".into());
                }
            }
            _ => {
                if !args[i].starts_with('-') && input_file.is_none() {
                    input_file = Some(args[i].clone());
                }
            }
        }
        i += 1;
    }

    // Read input from file or stdin
    let input = if let Some(path) = input_file {
        fs::read_to_string(path)?
    } else {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        buffer
    };

    // For CLI, we treat the entire input as one JSON string
    let json_strings = vec![input.clone()];

    // Infer schema - genson-core should handle any panics and return proper errors
    let result = infer_json_schema(&json_strings, Some(config.clone()))
        .map_err(|e| format!("Schema inference failed: {}", e))?;

    if do_normalise {
        let schema = &result.schema;

        // Parse input again for actual values
        let values: Vec<Value> = if config.delimiter == Some(b'\n') {
            input
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| serde_json::from_str::<Value>(l).unwrap_or(Value::Null))
                .collect()
        } else {
            vec![serde_json::from_str::<Value>(&input).unwrap_or(Value::Null)]
        };

        let cfg = NormaliseConfig {
            empty_as_null,
            coerce_string,
        };
        let normalised = normalise_values(values, schema, &cfg);

        if config.delimiter == Some(b'\n') {
            // print one line per row
            for v in normalised {
                println!("{}", serde_json::to_string(&v)?);
            }
        } else {
            println!("{}", serde_json::to_string_pretty(&normalised)?);
        }
    } else {
        // Pretty-print the schema
        println!("{}", serde_json::to_string_pretty(&result.schema)?);
    }

    eprintln!("Processed {} JSON object(s)", result.processed_count);
    Ok(())
}

fn print_help() {
    println!("genson-cli - JSON schema inference tool");
    println!();
    println!("USAGE:");
    println!("    genson-cli [OPTIONS] [FILE]");
    println!();
    println!("ARGS:");
    println!("    <FILE>    Input JSON file (reads from stdin if not provided)");
    println!();
    println!("OPTIONS:");
    println!("    -h, --help            Print this help message");
    println!("    --no-ignore-array     Don't treat top-level arrays as object streams");
    println!("    --ndjson              Treat input as newline-delimited JSON");
    println!("    --avro                Output Avro schema instead of JSON Schema");
    println!("    --normalise           Normalise the input data against the inferred schema");
    println!("    --coerce-strings      Coerce numeric/boolean strings to schema type during normalisation");
    println!("    --keep-empty          Keep empty arrays/maps instead of turning them into nulls");
    println!("    --map-threshold <N>   Treat objects with >N keys as map candidates (default 20)");
    println!("    --force-type k:v,...  Force field(s) to 'map' or 'record'");
    println!("                          Example: --force-type labels:map,claims:record");
    println!();
    println!("EXAMPLES:");
    println!("    genson-cli data.json");
    println!("    echo '{{\"name\": \"test\"}}' | genson-cli");
    println!("    genson-cli --ndjson multi-line.jsonl");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_cli_with_invalid_json_unit() {
        println!("=== Unit test calling CLI logic directly ===");

        // Create a temp file with invalid JSON
        let invalid_json = r#"{"invalid": json}"#;
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        temp_file
            .write_all(invalid_json.as_bytes())
            .expect("Failed to write to temp file");

        // Mock command line args to point to our temp file
        // let _original_args = env::args().collect::<Vec<_>>();

        // We can't easily mock env::args(), so let's call the genson-core function directly
        // This bypasses the CLI argument parsing but tests the core logic
        let json_strings = vec![invalid_json.to_string()];
        let result = infer_json_schema(&json_strings, Some(SchemaInferenceConfig::default()));

        println!("Result: {:?}", result);

        match result {
            Ok(schema_result) => {
                panic!(
                    "Expected error for invalid JSON but got success: {:?}",
                    schema_result
                );
            }
            Err(error_msg) => {
                println!("✅ Got error in unit test: {}", error_msg);
                // Check for the key parts of the error message instead of exact match
                assert!(error_msg.contains("Invalid JSON input"));
                assert!(error_msg.contains("line"));
            }
        }
    }

    #[test]
    fn test_genson_core_directly() {
        println!("=== Direct test of genson-core function ===");

        let json_strings = vec![r#"{"invalid": json}"#.to_string()];
        let result = infer_json_schema(&json_strings, Some(SchemaInferenceConfig::default()));

        println!("Direct result: {:?}", result);

        match result {
            Ok(schema_result) => {
                panic!("Expected error but got success: {:?}", schema_result);
            }
            Err(error_msg) => {
                println!("✅ Got expected error: {}", error_msg);
                // Check for the key parts of the error message instead of exact match
                assert!(error_msg.contains("Invalid JSON input"));
                assert!(error_msg.contains("line"));
            }
        }
    }

    #[test]
    fn test_cli_normalise_with_empty_as_null() {
        // Empty array should become null when --normalise is used (default behaviour)
        let input = r#"{"labels": []}"#;
        let json_strings = vec![input.to_string()];

        let config = SchemaInferenceConfig {
            avro: true,
            ..SchemaInferenceConfig::default()
        };

        let result = infer_json_schema(&json_strings, Some(config))
            .expect("Schema inference should succeed");

        let values: Vec<serde_json::Value> = json_strings
            .iter()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();

        let norm_cfg = NormaliseConfig {
            empty_as_null: true,
            ..NormaliseConfig::default()
        };
        let normalised = normalise_values(values, &result.schema, &norm_cfg);

        println!(
            "Normalised with empty_as_null: {}",
            serde_json::to_string(&normalised).unwrap()
        );
        assert_eq!(normalised[0]["labels"], serde_json::Value::Null);
    }

    #[test]
    fn test_cli_normalise_with_keep_empty() {
        // Empty array should be kept when --keep-empty is used
        let input = r#"{"labels": []}"#;
        let json_strings = vec![input.to_string()];

        let config = SchemaInferenceConfig {
            avro: true,
            ..SchemaInferenceConfig::default()
        };

        let result = infer_json_schema(&json_strings, Some(config))
            .expect("Schema inference should succeed");

        let values: Vec<serde_json::Value> = json_strings
            .iter()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();

        let norm_cfg = NormaliseConfig {
            empty_as_null: false,
            ..NormaliseConfig::default()
        };
        let normalised = normalise_values(values, &result.schema, &norm_cfg);

        println!(
            "Normalised with keep_empty: {}",
            serde_json::to_string(&normalised).unwrap()
        );
        assert_eq!(normalised[0]["labels"], serde_json::json!([]));
    }
}
