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
    // In a real Polars context, we'd have multiple rows
    let json_strings = vec![input];

    // Infer schema
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
