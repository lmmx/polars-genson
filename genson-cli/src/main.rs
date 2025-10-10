use std::env;
use std::fs;
use std::io::{self, Read};

use genson_core::{
    infer_json_schema,
    normalise::{normalise_values, MapEncoding, NormaliseConfig},
    DebugVerbosity, SchemaInferenceConfig,
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
    let mut pq_column: Option<String> = None;

    // Normalisation config
    let mut do_normalise = false;
    let mut empty_as_null = true; // default ON
    let mut coerce_string = false; // default OFF
    let mut map_encoding = genson_core::normalise::MapEncoding::Mapping; // default

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
            "--pq-column" => {
                if i + 1 < args.len() {
                    pq_column = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    return Err("Missing value for --pq-column".into());
                }
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
            "--map-max-rk" | "--map-max-required-keys" => {
                if i + 1 < args.len() {
                    config.map_max_required_keys =
                        Some(args[i + 1].parse::<usize>().map_err(|_| {
                            format!("Invalid value for --map-max-required-keys: {}", args[i + 1])
                        })?);
                    i += 1;
                } else {
                    return Err("Missing value for --map-max-required-keys".into());
                }
            }
            "--unify-maps" => {
                config.unify_maps = true;
            }
            "--no-unify" => {
                if i + 1 < args.len() {
                    for field in args[i + 1].split(',') {
                        config.no_unify.insert(field.to_string());
                    }
                    i += 1;
                } else {
                    return Err("Missing value for --no-unify".into());
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
            "--force-scalar-promotion" => {
                if i + 1 < args.len() {
                    for field in args[i + 1].split(',') {
                        config.force_scalar_promotion.insert(field.to_string());
                    }
                    i += 1;
                } else {
                    return Err("Missing value for --force-scalar-promotion".into());
                }
            }
            "--map-encoding" => {
                if i + 1 < args.len() {
                    map_encoding = match args[i + 1].as_str() {
                        "mapping" => MapEncoding::Mapping,
                        "entries" => MapEncoding::Entries,
                        "kv" => MapEncoding::KeyValueEntries,
                        other => {
                            return Err(format!(
                            "Invalid value for --map-encoding: {} (expected mapping|entries|kv)",
                            other
                        )
                            .into())
                        }
                    };
                    i += 1;
                } else {
                    return Err("Missing value for --map-encoding".into());
                }
            }
            "--no-wrap-scalars" => {
                config.wrap_scalars = false;
            }
            "--wrap-root" => {
                if i + 1 < args.len() {
                    config.wrap_root = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    return Err("Missing value for --wrap-root".into());
                }
            }
            "--root-map" => {
                config.no_root_map = false;
            }
            "--max-builders" => {
                if i + 1 < args.len() {
                    config.max_builders = Some(args[i + 1].parse::<usize>().map_err(|_| {
                        format!("Invalid value for --max-builders: {}", args[i + 1])
                    })?);
                    i += 1;
                } else {
                    return Err("Missing value for --max-builders".into());
                }
            }
            "--debug" => {
                config.debug = true;
            }
            "--profile" => {
                config.profile = true;
            }
            "--verbose" => {
                config.verbosity = DebugVerbosity::Verbose;
            }
            _ => {
                if !args[i].starts_with('-') && input_file.is_none() {
                    input_file = Some(args[i].clone());
                }
            }
        }
        i += 1;
    }

    // For CLI, we treat the entire input as one JSON string
    let json_strings = if let Some(ref col_name) = pq_column {
        // Parquet mode
        let path = input_file.ok_or("--pq-column requires an input file path")?;

        let strings = genson_core::parquet::read_string_column(&path, col_name)?;

        // If --ndjson, split each string by newlines
        if config.delimiter == Some(b'\n') {
            strings
                .into_iter()
                .flat_map(|s| s.lines().map(|l| l.to_string()).collect::<Vec<_>>())
                .collect()
        } else {
            strings
        }
    } else {
        // Original JSON/JSONL mode - pass as single string, let core handle delimiter
        let input = if let Some(path) = input_file {
            fs::read_to_string(path)?
        } else {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            buffer
        };
        vec![input] // Don't clone, just move
    };

    // Infer schema - genson-core should handle any panics and return proper errors
    let result = infer_json_schema(&json_strings, Some(config.clone()))
        .map_err(|e| format!("Schema inference failed: {}", e))?;

    if do_normalise {
        let schema = &result.schema;

        let values: Vec<Value> = if pq_column.is_some() {
            // Parquet mode: json_strings is already split correctly
            json_strings
                .iter()
                .map(|s| serde_json::from_str::<Value>(s).unwrap_or(Value::Null))
                .collect()
        } else if config.delimiter == Some(b'\n') {
            // NDJSON mode: split the single string by lines
            json_strings[0]
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| serde_json::from_str::<Value>(l).unwrap_or(Value::Null))
                .collect()
        } else {
            // Regular JSON: parse the single string
            vec![serde_json::from_str::<Value>(&json_strings[0]).unwrap_or(Value::Null)]
        };

        let cfg = NormaliseConfig {
            empty_as_null,
            coerce_string,
            map_encoding,
            wrap_root: config.wrap_root,
        };
        let normalised = normalise_values(values, schema, &cfg);

        if config.delimiter == Some(b'\n') {
            // print one line per row
            for v in normalised {
                anstream::println!("{}", serde_json::to_string(&v)?);
            }
        } else {
            anstream::println!("{}", serde_json::to_string_pretty(&normalised)?);
        }
    } else {
        // Pretty-print the schema
        anstream::println!("{}", serde_json::to_string_pretty(&result.schema)?);
    }

    anstream::eprintln!("Processed {} JSON object(s)", result.processed_count);
    Ok(())
}

fn print_help() {
    anstream::println!("genson-cli - JSON schema inference tool");
    anstream::println!();
    anstream::println!("USAGE:");
    anstream::println!("    genson-cli [OPTIONS] [FILE]");
    anstream::println!();
    anstream::println!("ARGS:");
    anstream::println!("    <FILE>    Input JSON file (reads from stdin if not provided)");
    anstream::println!();
    anstream::println!("OPTIONS:");
    anstream::println!("    -h, --help            Print this help message");
    anstream::println!("    --no-ignore-array     Don't treat top-level arrays as object streams");
    anstream::println!("    --ndjson              Treat input as newline-delimited JSON");
    anstream::println!("    --avro                Output Avro schema instead of JSON Schema");
    anstream::println!(
        "    --normalise           Normalise the input data against the inferred schema"
    );
    anstream::println!("    --coerce-strings      Coerce numeric/boolean strings to schema type during normalisation");
    anstream::println!(
        "    --keep-empty          Keep empty arrays/maps instead of turning them into nulls"
    );
    anstream::println!(
        "    --map-threshold <N>   Treat objects with >N keys as map candidates (default 20)"
    );
    anstream::println!(
        "    --map-max-rk <N>      Maximum required keys for Map inference (default: no limit)"
    );
    anstream::println!("    --map-max-required-keys <N>");
    anstream::println!(
        "    --unify-maps          Enable unification of compatible record schemas into maps"
    );
    anstream::println!("                          Same as --map-max-rk");
    anstream::println!(
        "    --no-unify <fields>   Exclude fields from record unification (comma-separated)"
    );
    anstream::println!("                          Example: --no-unify qualifiers,references");
    anstream::println!("    --force-type k:v,...  Force field(s) to 'map' or 'record'");
    anstream::println!("                          Example: --force-type labels:map,claims:record");
    anstream::println!("    --force-scalar-promotion <fields>");
    anstream::println!("                          Always promote these fields to wrapped scalars (comma-separated)");
    anstream::println!(
        "                          Example: --force-scalar-promotion precision,datavalue"
    );
    anstream::println!("    --map-encoding <mode> Choose map encoding (mapping|entries|kv)");
    anstream::println!("                          mapping = Avro/JSON object (shared dict)");
    anstream::println!(
        "                          entries = list of single-entry objects (individual dicts)"
    );
    anstream::println!("                          kv      = list of {{key,value}} objects");
    anstream::println!(
        "    --no-wrap-scalars     Disable scalar promotion (keep raw scalar types)"
    );
    anstream::println!("    --wrap-root <field>   Wrap top-level schema under this required field");
    anstream::println!("    --root-map            Allow document root to become a map");
    anstream::println!(
        "    --max-builders <N>    Maximum schema builders to create in parallel at once"
    );
    anstream::println!(
        "                          Lower values reduce peak memory (default: unlimited)"
    );
    anstream::println!("    --debug               Enable debug output during schema inference");
    anstream::println!("    --profile             Enable profiling output during schema inference");
    anstream::println!();
    anstream::println!("EXAMPLES:");
    anstream::println!("    genson-cli data.json");
    anstream::println!("    echo '{{\"name\": \"test\"}}' | genson-cli");
    anstream::println!("    genson-cli --ndjson multi-line.jsonl");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_cli_with_invalid_json_unit() {
        anstream::println!("=== Unit test calling CLI logic directly ===");

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

        anstream::println!("Result: {:?}", result);

        match result {
            Ok(schema_result) => {
                panic!(
                    "Expected error for invalid JSON but got success: {:?}",
                    schema_result
                );
            }
            Err(error_msg) => {
                anstream::println!("✅ Got error in unit test: {}", error_msg);
                // Check for the key parts of the error message instead of exact match
                assert!(error_msg.contains("Invalid JSON input"));
                assert!(error_msg.contains("line"));
            }
        }
    }

    #[test]
    fn test_genson_core_directly() {
        anstream::println!("=== Direct test of genson-core function ===");

        let json_strings = vec![r#"{"invalid": json}"#.to_string()];
        let result = infer_json_schema(&json_strings, Some(SchemaInferenceConfig::default()));

        anstream::println!("Direct result: {:?}", result);

        match result {
            Ok(schema_result) => {
                panic!("Expected error but got success: {:?}", schema_result);
            }
            Err(error_msg) => {
                anstream::println!("✅ Got expected error: {}", error_msg);
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

        anstream::println!(
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

        anstream::println!(
            "Normalised with keep_empty: {}",
            serde_json::to_string(&normalised).unwrap()
        );
        assert_eq!(normalised[0]["labels"], serde_json::json!([]));
    }
}
