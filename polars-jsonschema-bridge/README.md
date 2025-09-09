# Polars JSON Schema Bridge

[![crates.io](https://img.shields.io/crates/v/polars-jsonschema-bridge.svg)](https://crates.io/crates/polars-jsonschema-bridge)
[![MIT/Apache-2.0 licensed](https://img.shields.io/crates/l/polars-jsonschema-bridge.svg)](https://github.com/lmmx/polars-genson/blob/master/LICENSE)
[![Documentation](https://docs.rs/polars-jsonschema-bridge/badge.svg)](https://docs.rs/polars-jsonschema-bridge)

A Rust library for bidirectional conversion between [JSON Schema](https://json-schema.org/) 
and [Polars](https://pola.rs/) data types.

This crate provides the core type conversion logic used by the [polars-genson](https://crates.io/crates/polars-genson-py) plugin, but can also be used independently for any application that needs to convert between JSON Schema and Polars types.

In addition, the crate supports **Avro → Polars** conversion, enabling Polars schema
inference from Avro record schemas (including maps, unions, and nested records).

## Features

- **JSON Schema → Polars**: Convert JSON Schema type definitions to Polars `DataType`
- **Avro → Polars**: Convert Avro record schemas to Polars `DataType`
- **Polars → JSON Schema**: Convert Polars schemas to valid JSON Schema documents
- **Complex Types**: Support for nested objects, arrays, and structs
- **Type Safety**: Comprehensive error handling for unsupported conversions
- **Standards Compliant**: Generates JSON Schema following Draft 2020-12 specification

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
polars-jsonschema-bridge = "0.1.1"
polars = "0.50"
serde_json = "1.0"
```

### JSON Schema to Polars Types

```rust
use polars_jsonschema_bridge::{schema_to_polars_fields, SchemaFormat};
use serde_json::json;

let json_schema = json!({
    "type": "object",
    "properties": {
        "name": {"type": "string"},
        "age": {"type": "integer"},
        "scores": {
            "type": "array",
            "items": {"type": "number"}
        },
        "profile": {
            "type": "object",
            "properties": {
                "active": {"type": "boolean"}
            }
        }
    }
});

let fields = schema_to_polars_fields(&json_schema, SchemaFormat::JsonSchema, false)?;
// Returns: [
//   ("name", "String"),
//   ("age", "Int64"), 
//   ("scores", "List[Float64]"),
//   ("profile", "Struct[active:Boolean]")
// ]
```

### Avro Schema to Polars Types

```rust
use polars_jsonschema_bridge::{schema_to_polars_fields, SchemaFormat};
use serde_json::json;

let avro_schema = json!({
    "type": "record",
    "name": "document",
    "fields": [
        {"name": "id", "type": "int"},
        {"name": "tags", "type": ["null", {"type": "array", "items": "string"}]},
        {"name": "labels", "type": {"type": "map", "values": "string"}},
        {"name": "active", "type": ["null", "boolean"]}
    ]
});

let fields = schema_to_polars_fields(&avro_schema, SchemaFormat::Avro, false)?;
// Returns: [
//   ("id", "Int64"),
//   ("tags", "List[String]"),
//   ("labels", "List[Struct[key:String,value:String]]"),
//   ("active", "Boolean")
// ]
```

### Polars Schema to JSON Schema

```rust
use polars_jsonschema_bridge::{polars_schema_to_json_schema, JsonSchemaOptions};
use polars::prelude::*;

let mut schema = Schema::default();
schema.with_column("name".into(), DataType::String);
schema.with_column("age".into(), DataType::Int64);
schema.with_column("scores".into(), DataType::List(Box::new(DataType::Float64)));

// Use default options
let json_schema = polars_schema_to_json_schema(&schema, &JsonSchemaOptions::new())?;

// Or customize the output with options
let options = JsonSchemaOptions::new()
    .with_title(Some("User Schema"))
    .with_description(Some("A simple user record"))
    .with_optional_fields(vec!["email"])
    .with_additional_properties(true);

let json_schema_custom = polars_schema_to_json_schema(&schema, &options)?;
````

This generates a JSON Schema document with the desired metadata and field requirements.

### Individual Type Conversions

```rust
use polars_jsonschema_bridge::{json_type_to_polars_type, polars_dtype_to_json_schema, JsonSchemaOptions};
use polars::prelude::DataType;
use serde_json::json;

// JSON Schema type → Polars type string
let polars_type = json_type_to_polars_type(&json!({"type": "string"}))?;
assert_eq!(polars_type, "String");

// Polars DataType → JSON Schema
let json_schema = polars_dtype_to_json_schema(
    &DataType::List(Box::new(DataType::Int64)),
    &JsonSchemaOptions::default()
)?;
assert_eq!(json_schema, json!({
    "type": "array",
    "items": {"type": "integer"}
}));
```

## Supported Type Mappings

### JSON Schema → Polars

| JSON Schema Type | Polars DataType | Notes |
|------------------|-----------------|-------|
| `string` | `String` | |
| `integer` | `Int64` | |
| `number` | `Float64` | |
| `boolean` | `Boolean` | |
| `null` | `Null` | |
| `array` | `List[T]` | Where T is the items' type |
| `object` | `Struct[...]` | Nested object properties |

- Note that we do not have JSON Schema `array` to Polars `Array` conversion (...yet?)

### Avro → Polars

| Avro Type                             | Polars DataType                    | Notes                                  |
| ------------------------------------- | ---------------------------------- | -------------------------------------- |
| `"string"`                            | `String`                           |                                        |
| `"int"`, `"long"`                     | `Int64`                            | All integers coerced to 64-bit         |
| `"float"`, `"double"`                 | `Float64`                          |                                        |
| `"boolean"`                           | `Boolean`                          |                                        |
| `"null"`                              | `Null`                             |                                        |
| `{"type": "array", "items": T}`       | `List[T]`                          | T converted recursively                |
| `{"type": "map", "values": T}`        | `List[Struct[key:String,value:T]]` | Encoded as key/value structs           |
| `{"type": "record", "fields": [...]}` | `Struct[...]`                      | Each field converted recursively       |
| `[ "null", T, ... ]`                  | `T`                                | Union: first non-null branch is chosen |

### Polars → JSON Schema

| Polars DataType | JSON Schema | Notes |
|-----------------|-------------|-------|
| `Boolean` | `{"type": "boolean"}` | |
| `Int8/16/32/64/128` | `{"type": "integer"}` | |
| `UInt8/16/32/64` | `{"type": "integer", "minimum": 0}` | |
| `Float32/64` | `{"type": "number"}` | |
| `String` | `{"type": "string"}` | |
| `Date` | `{"type": "string", "format": "date"}` | |
| `Datetime` | `{"type": "string", "format": "date-time"}` | |
| `List[T]` | `{"type": "array", "items": {...}}` | |
| `Array[T, N]` | `{"type": "array", "minItems": N, "maxItems": N}` | |
| `Struct[...]` | `{"type": "object", "properties": {...}}` | |

## Error Handling

The library provides comprehensive error handling through Polars' `PolarsError` type:

```rust
use polars_jsonschema_bridge::json_type_to_polars_type;
use serde_json::json;

// Unsupported types return errors
let result = json_type_to_polars_type(&json!({"type": "unsupported"}));
assert!(result.is_err());
```

## JSON Schema Generation Options

The `JsonSchemaOptions` struct lets you control aspects of the generated schema:

| Option | Default | Effect |
|--------|---------|--------|
| `schema_uri` | `Some("https://json-schema.org/draft/2020-12/schema")` | Controls the `$schema` field (or omit entirely with `None`) |
| `title` | `None` | Adds a `title` to the schema |
| `description` | `None` | Adds a `description` to the schema |
| `optional_fields` | empty set | By default all fields are required; use this to mark some as optional |
| `additional_properties` | `false` | Controls the `additionalProperties` flag |

Example:

```rust
use polars_jsonschema_bridge::JsonSchemaOptions;

let options = JsonSchemaOptions::new()
    .with_title(Some("Example"))
    .with_optional_fields(vec!["email"])
    .with_additional_properties(false);
````

## Debug Mode

Enable debug output to see the intermediate JSON Schema during conversion:

```rust
use polars_jsonschema_bridge::{schema_to_polars_fields, JsonSchema};

let fields = schema_to_polars_fields(&json_schema, SchemaFormat::JsonSchema, true)?; // debug = true
// Prints the generated JSON Schema to stderr
```

## Integration

This crate was designed for use in [polars-genson](https://pypi.org/project/polars-genson/), a Python plugin for JSON schema inference

See also: [genson-core](https://crates.io/crates/genson-core), a separate crate from the same monorepo for JSON schema inference (without the Polars part)

Please feel free to use this crate in your Rust applications working with Polars and JSON Schema and
submit feature requests/contributions.

## Contributing

This crate is part of the [polars-genson](https://github.com/lmmx/polars-genson) project. See the main repository for
the [contribution](https://github.com/lmmx/polars-genson/blob/master/CONTRIBUTION.md)
and [development](https://github.com/lmmx/polars-genson/blob/master/DEVELOPMENT.md) docs.

## License

Licensed under the MIT License. See [LICENSE](https://github.com/lmmx/polars-genson/blob/master/LICENSE) for details.
