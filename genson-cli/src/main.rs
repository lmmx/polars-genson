use std::env;
use std::fs;
use std::io::{self, Read};

use genson_core::{infer_json_schema, SchemaInferenceConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    use std::process::Command;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_valid_json() {
        let valid_json = r#"{"name": "Alice", "age": 30}"#;
        
        // Create a temporary file with valid JSON
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        temp_file.write_all(valid_json.as_bytes()).expect("Failed to write to temp file");
        
        // Use relative path to the binary
        let binary_path = "../target/debug/genson-cli";
        
        // Run the CLI binary directly
        let output = Command::new(binary_path)
            .arg(temp_file.path().to_str().unwrap())
            .output()
            .expect("Failed to execute CLI");
        
        // Should succeed
        assert!(output.status.success(), "CLI should succeed with valid JSON");
        
        // Should contain schema output
        let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8 in stdout");
        assert!(stdout.contains("\"type\""), "Output should contain JSON schema");
        assert!(stdout.contains("\"properties\""), "Output should contain properties");
    }

    #[test]
    fn test_invalid_json() {
        let invalid_json = r#"{"hello":"world}"#; // Missing closing quote
        
        // Create a temporary file with invalid JSON
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        temp_file.write_all(invalid_json.as_bytes()).expect("Failed to write to temp file");
        
        // Use relative path to the binary
        let binary_path = "../target/debug/genson-cli";
        
        // Run the CLI binary directly
        let output = Command::new(binary_path)
            .arg(temp_file.path().to_str().unwrap())
            .output()
            .expect("Failed to execute CLI");
        
        // Should fail gracefully, not panic
        assert!(!output.status.success(), "CLI should fail with invalid JSON");
        
        // Should contain error message, not panic output
        let stderr = String::from_utf8(output.stderr).expect("Invalid UTF-8 in stderr");
        assert!(stderr.contains("Schema inference failed"), "Should contain error message");
        assert!(!stderr.contains("panicked"), "Should not contain panic message");
        assert!(!stderr.contains("SIGABRT"), "Should not segfault");
    }

    #[test]
    fn test_malformed_json_variants() {
        let test_cases = vec![
            (r#"{"invalid": json}"#, "unquoted value"),
            (r#"{"incomplete":"#, "incomplete string"),
            (r#"{"trailing":,"#, "trailing comma"),
            (r#"{invalid: "json"}"#, "unquoted key"),
            (r#"{"nested": {"broken": json}}"#, "nested broken JSON"),
        ];

        // Use relative path to the binary
        let binary_path = "../target/debug/genson-cli";

        for (invalid_json, description) in test_cases {
            println!("Testing: {}", description);
            
            // Create a temporary file with invalid JSON
            let mut temp_file = NamedTempFile::new()
                .expect(&format!("Failed to create temp file for {}", description));
            temp_file.write_all(invalid_json.as_bytes())
                .expect(&format!("Failed to write to temp file for {}", description));
            
            // Run the CLI binary directly
            let output = Command::new(binary_path)
                .arg(temp_file.path().to_str().unwrap())
                .output()
                .expect(&format!("Failed to execute CLI for {}", description));
            
            // Should fail gracefully, not panic
            assert!(!output.status.success(), "CLI should fail with invalid JSON: {}", description);
            
            // Should not panic or segfault
            let stderr = String::from_utf8(output.stderr).expect("Invalid UTF-8 in stderr");
            assert!(!stderr.contains("panicked"), "Should not panic for {}: {}", description, stderr);
            assert!(!stderr.contains("SIGABRT"), "Should not segfault for {}: {}", description, stderr);
            assert!(!stderr.contains("Aborted"), "Should not abort for {}: {}", description, stderr);
        }
    }
}
