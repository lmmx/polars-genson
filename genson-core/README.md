# Genson Core

[![crates.io](https://img.shields.io/crates/v/genson-core.svg)](https://crates.io/crates/genson-core)
[![MIT/Apache-2.0 licensed](https://img.shields.io/crates/l/genson-core.svg)](https://github.com/lmmx/polars-genson/blob/master/LICENSE)
[![Documentation](https://docs.rs/genson-core/badge.svg)](https://docs.rs/genson-core)

Fast and robust Rust library for JSON schema inference: pre-validates JSON to avoid panics, handles errors properly.
Adapts the `genson-rs` library's SIMD parallelism after first checking the string with `serde_json`
in a streaming pass without allocating values.

This is the core library that powers both the [genson-cli](https://crates.io/crates/genson-cli) command-line tool and the [polars-genson](https://pypi.org/project/polars-genson/) Python plugin. It includes a vendored and enhanced version of the genson-rs library with added safety features and comprehensive error handling.

## Features

- **Robust JSON Schema Inference**: Generate JSON schemas from JSON data with comprehensive type detection
- **Parallel Processing**: Efficient processing of large JSON datasets using Rayon
- **Enhanced Error Handling**: Proper error propagation instead of panics for invalid JSON
- **Multiple Input Formats**: Support for regular JSON, NDJSON, and arrays of JSON objects
- **Field Order Preservation**: Maintains original field ordering using IndexMap
- **Memory Efficient**: Uses mimalloc for optimized memory allocation
- **SIMD Acceleration**: Fast JSON parsing with simd-json

## Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
genson-core = "0.1.2"
```

- ⚠️  **Caution**: if you include `serde_json` in your dependencies but don't activate its `preserve_order` feature,
  `genson-core` schema properties will not be in insertion order. This may be an unwelcome surprise!

```toml
serde_json = { version = "1.0", features = ["preserve_order"] }
```

### Basic Usage

```rust
use genson_core::{infer_json_schema, SchemaInferenceConfig};

fn main() -> Result<(), String> {
    let json_strings = vec![
        r#"{"name": "Alice", "age": 30, "scores": [95, 87]}"#.to_string(),
        r#"{"name": "Bob", "age": 25, "city": "NYC", "active": true}"#.to_string(),
    ];

    let result = infer_json_schema(&json_strings, None)?;
    
    println!("Processed {} JSON objects", result.processed_count);
    println!("Schema: {}", serde_json::to_string_pretty(&result.schema)?);
    
    Ok(())
}
```

### Configuration Options

```rust
use genson_core::{infer_json_schema, SchemaInferenceConfig};

let config = SchemaInferenceConfig {
    ignore_outer_array: true,           // Treat top-level arrays as streams of objects
    delimiter: Some(b'\n'),             // Enable NDJSON processing
    schema_uri: Some("AUTO".to_string()), // Auto-detect schema URI
};

let result = infer_json_schema(&json_strings, Some(config))?;
```

### NDJSON Processing

```rust
let ndjson_data = vec![
    r#"
    {"user": "alice", "action": "login"}
    {"user": "bob", "action": "logout"}
    {"user": "charlie", "action": "login", "ip": "192.168.1.1"}
    "#.to_string()
];

let config = SchemaInferenceConfig {
    delimiter: Some(b'\n'),  // Enable NDJSON mode
    ..Default::default()
};

let result = infer_json_schema(&ndjson_data, Some(config))?;
```

### Advanced Schema Building

For more control over the schema building process:

```rust
use genson_core::genson_rs::{get_builder, build_json_schema, BuildConfig};

let mut builder = get_builder(Some("https://json-schema.org/draft/2020-12/schema"));

let build_config = BuildConfig {
    delimiter: None,
    ignore_outer_array: true,
};

let mut json_bytes = br#"{"field": "value"}"#.to_vec();
let schema = build_json_schema(&mut builder, &mut json_bytes, &build_config);

let final_schema = builder.to_schema();
```

## Performance Features

**Parallel Processing**

The library automatically uses parallel processing for:

- Large JSON arrays (when items > 10)
- NDJSON files with delimiter-based splitting
- Multiple JSON objects in a single input

**Memory Optimisation**

- **mimalloc**: Fast memory allocation
- **SIMD JSON**: Hardware-accelerated parsing where available
- **Streaming**: Processes large files without loading everything into memory

## Error Handling

The library has been put together so as to avoid panics. That said, if a panic does occur, it will
be caught. This was left in after solving the initial panic problem, and should not be seen in
practice, since the JSON is always pre-validated with `serde_json` and panics only occurred when the
JSON was invalid. Please report any examples you find that panic along with the JSON that caused it
if possible.

The library provides comprehensive error handling that catches and converts internal panics into proper error messages:

```rust
let invalid_json = vec![r#"{"invalid": json}"#.to_string()];

match infer_json_schema(&invalid_json, None) {
    Ok(result) => println!("Success: {:?}", result),
    Err(error) => {
        // Will contain a descriptive error message instead of panicking
        eprintln!("JSON parsing failed: {}", error);
    }
}
```

Error messages include:
- Position information for syntax errors
- Truncated JSON content for context (prevents huge error messages)
- Clear descriptions of what went wrong

## Schema Features

### Type Inference

The library accurately infers:
- Basic types: `string`, `number`, `integer`, `boolean`, `null`
- Complex types: `object`, `array`
- Nested structures with proper schema merging
- Optional vs required fields based on data presence

### Field Order Preservation

This library uses [OrderMap](https://github.com/indexmap-rs/ordermap) to preserve the original field ordering from JSON input:

```rust
// Input: {"z": 1, "b": 2, "a": 3}
// Output schema will maintain z -> b -> a ordering
```

### Schema Merging

When processing multiple JSON objects, schemas are intelligently merged:

```rust
// Object 1: {"name": "Alice", "age": 30}
// Object 2: {"name": "Bob", "city": "NYC"}
// Merged schema: name (required), age (optional), city (optional)
```

## Integration

This crate is designed as the foundation for:

- **[polars-genson](https://pypi.org/project/polars-genson/)**: Python plugin for Polars DataFrames
- **[genson-cli](https://crates.io/crates/genson-cli)**: Command-line JSON schema inference tool
  (mainly for testing)
- **Your code!**: Any Rust application needing fast and reliable JSON schema inference can use this crate!

## Safety & Reliability

- **Panic Safety**: All genson-rs panics are caught and converted to errors
- **Memory Safety**: Comprehensive bounds checking and safe parsing
- **Input Validation**: JSON validation before processing
- **Graceful Degradation**: Handles malformed input gracefully

## Contributing

This crate is part of the [polars-genson](https://github.com/lmmx/polars-genson) project. See the main repository for
the [contribution](https://github.com/lmmx/polars-genson/blob/master/CONTRIBUTION.md)
and [development](https://github.com/lmmx/polars-genson/blob/master/DEVELOPMENT.md) docs.

## License

Licensed under the MIT License. See [LICENSE](https://img.shields.io/crates/l/genson-core.svg)](https://github.com/lmmx/polars-genson/blob/master/LICENSE) for details.

Contains vendored and adapted code from the Apache 2.0 licensed genson-rs crate.
