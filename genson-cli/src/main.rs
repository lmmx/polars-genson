use std::env;
use std::fs;
use std::io::{self, Read};

use genson_core::{infer_json_schema, SchemaInferenceConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_cli()
}

// Extract the main logic into a separate function so we can call it from tests
fn run_cli() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    // Handle command line options
    let mut config = SchemaInferenceConfig::default();
    let mut input_file = None;

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
    let json_strings = vec![input];

    // Infer schema - genson-core should handle any panics and return proper errors
    let result = infer_json_schema(&json_strings, Some(config))
        .map_err(|e| format!("Schema inference failed: {}", e))?;

    // Pretty-print the schema
    println!("{}", serde_json::to_string_pretty(&result.schema)?);

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
    println!("    -h, --help           Print this help message");
    println!("    --no-ignore-array    Don't treat top-level arrays as object streams");
    println!("    --ndjson            Treat input as newline-delimited JSON");
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
}
