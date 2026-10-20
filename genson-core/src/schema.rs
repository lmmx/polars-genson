use crate::genson_rs::{build_json_schema_into, get_builder, BuildConfig};
use crate::{debug, profile, profile_verbose};
use rayon::prelude::*;
use serde::de::Error as DeError;
use serde::Deserialize;
use serde_json::{json, Value};
use std::borrow::Cow;
use std::collections::HashSet;
use std::panic::{self, AssertUnwindSafe};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::genson_rs::{DataType, SchemaBuilder, SchemaNode};
use crate::value::object;

pub(crate) mod core;
pub use core::*;
mod map_inference;
use map_inference::*;

/// Maximum length of JSON string to include in error messages before truncating
const MAX_JSON_ERROR_LENGTH: usize = 100;
/// Threshold for switching to parallel processing. Below this, use sequential.
const PARALLEL_THRESHOLD: usize = 10;

/// Get current RSS memory usage in bytes
pub(crate) fn get_rss_bytes() -> Option<usize> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if line.starts_with("VmRSS:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let kb: usize = parts[1].parse().ok()?;
                return Some(kb * 1024); // Convert KB to bytes
            }
        }
    }
    None
}

/// Format bytes to human-readable string
pub(crate) fn format_bytes(bytes: usize) -> String {
    const MB: usize = 1024 * 1024;
    const GB: usize = 1024 * 1024 * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    }
}

fn current_time_hms() -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    let total_seconds = now.as_secs() % 86_400; // seconds in a day
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = (total_seconds % 60) as f64 + now.subsec_millis() as f64 / 1000.0; // add fractional part

    format!("{:02}:{:02}:{:04.1}", hours, minutes, seconds)
}

fn validate_json(s: &str) -> Result<(), serde_json::Error> {
    let mut de = serde_json::Deserializer::from_str(s);
    serde::de::IgnoredAny::deserialize(&mut de)?; // lightweight: ignores the parsed value
    de.end()
}

fn validate_ndjson(s: &str) -> Result<(), serde_json::Error> {
    for line in s.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        validate_json(trimmed)?; // propagate serde_json::Error
    }
    Ok(())
}

/// Recursively reorder union type arrays in a JSON Schema by canonical precedence.
///
/// Special case: preserves the common `["null", T]` pattern without reordering.
pub fn reorder_unions(schema: &mut Value) {
    match schema {
        Value::Object(obj) => {
            if let Some(Value::Array(types)) = obj.get_mut("type") {
                // sort by canonical precedence, but keep ["null", T] pattern intact
                if !(types.len() == 2 && types.iter().any(|t| t == "null")) {
                    types.sort_by_key(type_rank);
                }
            }
            // recurse into properties/items/etc.
            for v in obj.values_mut() {
                reorder_unions(v);
            }
        }
        Value::Array(arr) => {
            // Parallelize across array elements (if large enough)
            if arr.len() >= PARALLEL_THRESHOLD {
                arr.par_iter_mut().for_each(reorder_unions);
            } else {
                for v in arr {
                    reorder_unions(v);
                }
            }
        }
        _ => {}
    }
}

/// Assign a numeric precedence rank to a JSON Schema type.
///
/// Used by `reorder_unions` to sort union members deterministically.
/// - Null always first
/// - Containers before scalars (to enforce widening)
/// - Scalars ordered by narrowness
/// - Unknown types last
pub fn type_rank(val: &Value) -> usize {
    match val {
        Value::String(s) => type_string_rank(s),
        Value::Object(obj) => {
            if let Some(Value::String(t)) = obj.get("type") {
                type_string_rank(t)
            } else {
                100 // object with no "type" field
            }
        }
        _ => 100, // non-string/non-object
    }
}

/// Internal helper: rank by type string
pub(crate) fn type_string_rank(s: &str) -> usize {
    match s {
        // Null always first
        "null" => 0,

        // Containers before scalars: widening takes precedence
        "map" => 1,
        "array" => 2,
        "object" | "record" => 3,

        // Scalars (ordered by 'narrowness')
        "boolean" => 10,
        "integer" | "int" | "long" => 11,
        "number" | "float" | "double" => 12,
        "enum" => 13,
        "string" => 14,
        "fixed" => 15,
        "bytes" => 16,

        // Fallback
        _ => 99,
    }
}

/// Prepare JSON bytes for schema building (validation + wrap_root transformation)
fn prepare_json_bytes<'a>(
    json_bytes: &'a [u8],
    index: usize,
    config: &SchemaInferenceConfig,
) -> Result<Cow<'a, [u8]>, String> {
    // Early return for empty input
    let json_str = std::str::from_utf8(json_bytes)
        .map_err(|e| format!("Invalid UTF-8 at index {}: {}", index + 1, e))?;

    if json_str.trim().is_empty() {
        return Ok(Cow::Borrowed(&[]));
    }

    // Choose validation strategy based on delimiter
    let validation_result = if let Some(delim) = config.delimiter {
        if delim == b'\n' {
            validate_ndjson(json_str)
        } else {
            Err(serde_json::Error::custom(format!(
                "Unsupported delimiter: {:?}",
                delim
            )))
        }
    } else {
        validate_json(json_str)
    };

    if let Err(parse_error) = validation_result {
        let truncated_json = if json_str.len() > MAX_JSON_ERROR_LENGTH {
            format!(
                "{}... [truncated {} chars]",
                &json_str[..MAX_JSON_ERROR_LENGTH],
                json_str.len() - MAX_JSON_ERROR_LENGTH
            )
        } else {
            json_str.to_string()
        };

        return Err(format!(
            "Invalid JSON input at index {}: {} - JSON: {}",
            index + 1,
            parse_error,
            truncated_json
        ));
    }

    match &config.wrap_root {
        Some(field) => Ok(Cow::Owned(wrap_documents(
            json_str,
            field,
            config.delimiter.is_some(),
        ))),
        None => Ok(Cow::Borrowed(json_bytes)),
    }
}

/// `{"<field>": <document>}` for the input (each line of it, when `ndjson`), written around
/// the document's own bytes: the input has already been validated, so there is no need to
/// parse it into a value and serialise that again.
fn wrap_documents(json_str: &str, field: &str, ndjson: bool) -> Vec<u8> {
    let key = serde_json::to_string(field).expect("a string always serialises");
    let documents: Vec<&str> = if ndjson {
        json_str
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect()
    } else {
        vec![json_str]
    };
    let wrapping = key.len() + 3; // `{`, `:`, `}`
    let mut wrapped = Vec::with_capacity(json_str.len() + documents.len() * (wrapping + 1));
    for (i, document) in documents.into_iter().enumerate() {
        if i > 0 {
            wrapped.push(b'\n');
        }
        wrapped.push(b'{');
        wrapped.extend_from_slice(key.as_bytes());
        wrapped.push(b':');
        wrapped.extend_from_slice(document.as_bytes());
        wrapped.push(b'}');
    }
    wrapped
}

/// Process all JSON strings sequentially and build schemas
fn process_json_strings_sequential<S: AsRef<str>>(
    json_strings: &[S],
    config: &SchemaInferenceConfig,
    builder: &mut crate::genson_rs::SchemaBuilder,
) -> Result<usize, String> {
    let build_config = BuildConfig {
        delimiter: config.delimiter,
        ignore_outer_array: config.ignore_outer_array,
    };

    let mut processed_count = 0;

    // Process each JSON string
    for (i, json_str) in json_strings.iter().enumerate() {
        profile_verbose!(config, "PROCESSING JSON STRING {}", i);

        let prep_start = std::time::Instant::now();
        let prepared_json = prepare_json_bytes(json_str.as_ref().as_bytes(), i, config)?;
        let prep_elapsed = prep_start.elapsed();
        profile_verbose!(config, "  Preparation took: {:?}", prep_elapsed);

        if prepared_json.is_empty() {
            continue;
        }

        let mut bytes = prepared_json.into_owned(); // Only allocate if Cow::Owned

        // Build schema incrementally - this is where panics happen
        let build_start = std::time::Instant::now();
        build_json_schema_into(builder, &mut bytes, &build_config);
        let build_elapsed = build_start.elapsed();
        profile_verbose!(config, "  Schema building took: {:?}", build_elapsed);

        processed_count += 1;
    }

    Ok(processed_count)
}

/// Apply force_field_types to a schema before merging.
/// This ensures structural consistency across schemas that will be unified.
fn apply_force_field_types(schema: &mut Value, config: &SchemaInferenceConfig) {
    match schema {
        Value::Object(obj) => {
            // Check if this object has properties
            if let Some(props) = obj.get_mut("properties") {
                if let Some(props_obj) = props.as_object_mut() {
                    for (field_name, field_schema) in props_obj.iter_mut() {
                        // Apply force_field_types
                        if let Some(forced) = config.force_field_types.get(field_name) {
                            if forced == "map" {
                                if let Some(field_obj) = field_schema.as_object_mut() {
                                    // Convert to map schema, keeping its values' schema (as
                                    // the final rewrite does) so the merge unifies them
                                    // across strings. An object with no keys in this string
                                    // is left as it is: it has no values to type, and
                                    // merges with the other strings' maps as it stands.
                                    let has_keys = field_obj
                                        .get("properties")
                                        .and_then(|p| p.as_object())
                                        .is_some_and(|p| !p.is_empty());
                                    if has_keys {
                                        // The fields forced within its values first (its
                                        // keys are not fields), as `preprocess_map_values`
                                        if let Some(Value::Object(props)) =
                                            field_obj.get_mut("properties")
                                        {
                                            for value in props.values_mut() {
                                                apply_force_field_types(value, config);
                                            }
                                        }
                                        let values = forced_map_value_schema(
                                            field_obj,
                                            Some(field_name.as_str()),
                                            config,
                                        );
                                        field_obj.shift_remove("properties");
                                        field_obj.shift_remove("required");
                                        field_obj.insert("type".to_string(), json!("object"));
                                        field_obj
                                            .insert("additionalProperties".to_string(), values);
                                    }
                                }
                            }
                        }
                        // Recurse
                        apply_force_field_types(field_schema, config);
                    }
                }
            }
            // Also recurse into items, additionalProperties, anyOf, etc.
            if let Some(items) = obj.get_mut("items") {
                apply_force_field_types(items, config);
            }
            if let Some(additional) = obj.get_mut("additionalProperties") {
                apply_force_field_types(additional, config);
            }
            if let Some(Value::Array(any_of)) = obj.get_mut("anyOf") {
                for item in any_of {
                    apply_force_field_types(item, config);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                apply_force_field_types(item, config);
            }
        }
        _ => {}
    }
}

/// `apply_force_field_types` on schema nodes: each property forced to be a map is written
/// out as a schema, rewritten, and read back. The rest of the tree is left as it is.
fn apply_force_field_types_to_node(node: &mut SchemaNode, config: &SchemaInferenceConfig) {
    node.visit_properties_mut(&mut |name, property| {
        if config.force_field_types.get(name).map(String::as_str) != Some("map") {
            return true;
        }
        // Rewritten within an object holding just this property, so that it is
        // recognised by name; that also rewrites the forced fields within it
        let properties = [(name.to_string(), property.to_schema())]
            .into_iter()
            .collect();
        let mut holder = object([("properties", Value::Object(properties))]);
        apply_force_field_types(&mut holder, config);
        let mut forced = SchemaNode::new();
        forced.add_schema(DataType::Schema(&holder["properties"][name]));
        *property = forced;
        false
    });
}

/// Process all JSON strings in parallel while maintaining order
fn process_json_strings_parallel<S: AsRef<str> + Sync>(
    json_strings: &[S],
    config: &SchemaInferenceConfig,
    builder: &mut SchemaBuilder,
) -> Result<usize, String> {
    profile!(
        config,
        "Starting parallel preparation and building ({})",
        current_time_hms()
    );

    // Process in chunks to limit peak memory
    let chunk_size = config.max_builders.unwrap_or(json_strings.len());

    if config.profile {
        if let Some(rss) = get_rss_bytes() {
            anstream::eprintln!("📊 RSS before parallel processing: {}", format_bytes(rss));
        }
    }

    let mut processed_count = 0;
    let mut seen_hashes = HashSet::new();
    // A field forced to be a map is rewritten in each string's schema before the merge
    let forces_maps = config.force_field_types.values().any(|t| t == "map");

    for (chunk_idx, chunk) in json_strings.chunks(chunk_size).enumerate() {
        profile!(
            config,
            "Processing chunk {} ({} strings)",
            chunk_idx,
            chunk.len()
        );

        if config.profile {
            if let Some(rss) = get_rss_bytes() {
                anstream::eprintln!("📊 RSS before chunk {}: {}", chunk_idx, format_bytes(rss));
            }
        }

        let t_par = std::time::Instant::now();
        let rows: Vec<Option<(SchemaBuilder, u64)>> = chunk
            .par_iter()
            .enumerate()
            .map(
                |(i, json_str)| -> Result<Option<(SchemaBuilder, u64)>, String> {
                    profile_verbose!(config, "Thread processing JSON STRING {}", i);

                    let prep_start = std::time::Instant::now();
                    let prepared = prepare_json_bytes(json_str.as_ref().as_bytes(), i, config)?;
                    let prep_elapsed = prep_start.elapsed();
                    profile_verbose!(
                        config,
                        "  String {} preparation took: {:?}",
                        i,
                        prep_elapsed
                    );

                    if prepared.is_empty() {
                        return Ok(None);
                    }

                    let mut chunk_builder = get_builder(config.schema_uri.as_deref());
                    let mut bytes = prepared.into_owned();
                    let chunk_build_config = BuildConfig {
                        delimiter: config.delimiter,
                        ignore_outer_array: config.ignore_outer_array,
                    };

                    let build_start = std::time::Instant::now();
                    build_json_schema_into(&mut chunk_builder, &mut bytes, &chunk_build_config);
                    let build_elapsed = build_start.elapsed();
                    profile_verbose!(
                        config,
                        "  String {} schema building took: {:?}",
                        i,
                        build_elapsed
                    );

                    if forces_maps {
                        apply_force_field_types_to_node(chunk_builder.root_node_mut(), config);
                    }
                    let hash = chunk_builder.schema_hash();
                    Ok(Some((chunk_builder, hash)))
                },
            )
            .collect::<Result<Vec<_>, String>>()?;

        if config.profile {
            if let Some(rss) = get_rss_bytes() {
                anstream::eprintln!("📊 RSS after collecting chunk: {}", format_bytes(rss));
            }
        }

        let par_el = t_par.elapsed();
        let t_mrg = std::time::Instant::now();
        // Merge each distinct schema of this chunk, in order
        let mut builders = Vec::with_capacity(rows.len());
        let mut duplicates = Vec::new();
        for (row_builder, hash) in rows.into_iter().flatten() {
            if seen_hashes.insert(hash) {
                processed_count += 1;
                builders.push(row_builder);
            } else {
                duplicates.push(row_builder);
            }
        }
        builder.add_builders(builders);
        // Freeing the duplicates is a large share of the serial merge, so spread it over
        // the rayon pool.
        duplicates.into_par_iter().for_each(drop);

        profile!(
            config,
            "chunk {} parallel {:?} serial-merge {:?}",
            chunk_idx,
            par_el,
            t_mrg.elapsed()
        );
        if config.profile {
            if let Some(rss) = get_rss_bytes() {
                anstream::eprintln!("📊 RSS after merging chunk: {}", format_bytes(rss));
            }
        }
    }

    profile!(config, "All chunks processed ({})", current_time_hms());

    Ok(processed_count)
}

/// `process_json_strings_parallel` by way of interned row states: each row's schema is
/// built from its token tape into states its block of rows shares, then the distinct rows
/// are merged in order, as there, a repeated subtree merged once. `None`, having changed
/// nothing, where a row or the options might be read differently: the rows then take the
/// usual path, which also raises their errors.
fn process_json_strings_interned<S: AsRef<str> + Sync>(
    json_strings: &[S],
    config: &SchemaInferenceConfig,
    builder: &mut SchemaBuilder,
) -> Option<usize> {
    use crate::genson_rs::interned::{RowState, RowStates};
    let ndjson = match config.delimiter {
        Some(b'\n') => true,
        None => false,
        Some(_) => return None,
    };
    let t_rows = std::time::Instant::now();
    let block_len = json_strings
        .len()
        .div_ceil(rayon::current_num_threads() * 4)
        .max(1);
    let blocks: Vec<(RowStates, Vec<RowState>)> = crate::phase::rows(|| {
        json_strings
            .par_chunks(block_len)
            .map(|rows| {
                let mut states = RowStates::new();
                let rows = rows
                    .iter()
                    .map(|row| states.row(row.as_ref(), ndjson, config.wrap_root.as_deref()))
                    .collect::<Result<Vec<_>, _>>()
                    .ok()?;
                Some((states, rows))
            })
            .collect::<Option<_>>()
    })?;
    profile!(config, "Interned row states took {:?}", t_rows.elapsed());

    // A field forced to be a map is rewritten in each row's schema before the merge
    if config.force_field_types.values().any(|t| t == "map") {
        let t_merge = std::time::Instant::now();
        let processed_count =
            crate::phase::merge(|| merge_forced_rows(&blocks, json_strings.len(), config, builder));
        profile!(config, "Forced rows' merge took {:?}", t_merge.elapsed());
        return Some(processed_count);
    }

    // The distinct rows of each chunk of `max_builders`, in order, as the usual path merges
    let t_merge = std::time::Instant::now();
    let chunk_size = config.max_builders.unwrap_or(json_strings.len()).max(1);
    let rows = blocks
        .iter()
        .flat_map(|(states, rows)| rows.iter().map(move |row| (states, *row)));
    let mut seen_hashes = HashSet::new();
    let mut processed_count = 0;
    let mut chunks = Vec::new();
    let mut rows = rows.peekable();
    while rows.peek().is_some() {
        let mut chunk = Vec::with_capacity(chunk_size);
        for (states, row) in rows.by_ref().take(chunk_size) {
            if let Some((state, hash)) = row {
                if seen_hashes.insert(hash) {
                    processed_count += 1;
                    chunk.push((states, state));
                }
            }
        }
        chunks.push(chunk);
    }
    crate::phase::merge(|| builder.add_interned_row_chunks(chunks, config.schema_uri.as_deref()));
    profile!(config, "Interned merge took {:?}", t_merge.elapsed());
    Some(processed_count)
}

/// The interned rows of `process_json_strings_interned` merged as
/// `process_json_strings_parallel` merges its rows where a field is forced to be a map:
/// each row's schema forced before the merge. A row's schema is built from its states, and
/// forced, once for each distinct row rather than once for each row; the forced schemas
/// are merged in order by chunks of `max_builders` rows, each distinct one once, as there.
/// Each chunk's new distinct rows are forced in parallel and dropped once merged, so only
/// one chunk's are held. Returns the number merged.
fn merge_forced_rows(
    blocks: &[(
        crate::genson_rs::interned::RowStates,
        Vec<crate::genson_rs::interned::RowState>,
    )],
    rows_len: usize,
    config: &SchemaInferenceConfig,
    builder: &mut SchemaBuilder,
) -> usize {
    // Each row's distinct row (by its identity), numbered in the order they first appear:
    // so those a chunk of rows is the first to hold are numbered in a run
    let mut first_of: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
    let mut distinct = Vec::new();
    let mut row_index: Vec<Option<usize>> = Vec::with_capacity(rows_len);
    for (states, rows) in blocks {
        for row in rows {
            row_index.push(row.map(|(state, hash)| {
                *first_of.entry(hash).or_insert_with(|| {
                    distinct.push((states, state));
                    distinct.len() - 1
                })
            }));
        }
    }
    let row_uri = config.schema_uri.as_deref();
    let force = |&(states, state): &(&_, _)| {
        let mut row = SchemaBuilder::new(None);
        row.add_interned_row_chunks([vec![(states, state)]], row_uri);
        // The tree the row's own builder gives, which the forcing reads and the merge
        // absorbs
        row.root_node_mut().unintern();
        apply_force_field_types_to_node(row.root_node_mut(), config);
        let hash = row.schema_hash();
        (Some(row), hash)
    };
    let chunk_size = config.max_builders.unwrap_or(rows_len).max(1);
    let (mut seen_hashes, mut processed_count, mut next) = (HashSet::new(), 0, 0);
    for chunk in row_index.chunks(chunk_size) {
        let end = chunk.iter().flatten().map(|&i| i + 1).max().unwrap_or(0);
        let new = next..end.max(next);
        let mut forced: Vec<(Option<SchemaBuilder>, u64)> =
            distinct[new.clone()].par_iter().map(force).collect();
        let mut builders = Vec::new();
        for &i in chunk.iter().flatten().filter(|i| new.contains(i)) {
            // A distinct row's forced schema is taken where its hash is first seen, once
            let (row, hash) = &mut forced[i - new.start];
            if seen_hashes.insert(*hash) {
                processed_count += 1;
                builders.extend(row.take());
            }
        }
        builder.add_builders(builders);
        next = new.end;
    }
    processed_count
}

/// Recursively convert fields matching force_field_types BEFORE unification runs.
/// This ensures structural consistency so that unification can succeed.
pub(crate) fn preprocess_force_field_types(schema: &mut Value, config: &SchemaInferenceConfig) {
    match schema {
        Value::Object(obj) => {
            // Process properties
            if let Some(props) = obj.get_mut("properties") {
                if let Some(props_obj) = props.as_object_mut() {
                    for (field_name, field_schema) in props_obj.iter_mut() {
                        // Check if this field should be forced to a type
                        if let Some(forced) = config.force_field_types.get(field_name) {
                            if forced == "map" {
                                convert_to_map(field_schema, field_name, config);
                            }
                        }
                        // Recurse into the field schema
                        preprocess_force_field_types(field_schema, config);
                    }
                }
            }
            // Recurse into items (for arrays)
            if let Some(items) = obj.get_mut("items") {
                preprocess_force_field_types(items, config);
            }
            // Recurse into additionalProperties (for maps)
            if let Some(additional) = obj.get_mut("additionalProperties") {
                preprocess_force_field_types(additional, config);
            }
            // Recurse into anyOf branches
            if let Some(Value::Array(any_of)) = obj.get_mut("anyOf") {
                for item in any_of {
                    preprocess_force_field_types(item, config);
                }
            }
        }
        Value::Array(arr) => {
            // Handle union types like ["null", {...}]
            for item in arr {
                preprocess_force_field_types(item, config);
            }
        }
        _ => {}
    }
}

/// The fields forced within the values of an object about to become a map, forced first,
/// so that its values are unified with them as maps already: within its properties' schemas,
/// a list's items and a union's branches, but not its properties themselves, which are the
/// map's keys rather than fields (`a` in `{"a": {"a": 0}}` forced at `a`).
fn preprocess_map_values(obj: &mut serde_json::Map<String, Value>, config: &SchemaInferenceConfig) {
    if let Some(Value::Object(props)) = obj.get_mut("properties") {
        for value in props.values_mut() {
            preprocess_force_field_types(value, config);
        }
    }
    if let Some(items) = obj.get_mut("items") {
        preprocess_force_field_types(items, config);
    }
    if let Some(Value::Array(branches)) = obj.get_mut("anyOf") {
        for branch in branches {
            match branch.as_object_mut() {
                Some(b) if b.contains_key("properties") => preprocess_map_values(b, config),
                _ => preprocess_force_field_types(branch, config),
            }
        }
    }
}

/// Convert any schema to a Map<string, string> schema
fn convert_to_map(schema: &mut Value, field_name: &str, config: &SchemaInferenceConfig) {
    // Handle union types first: ["null", {...}] or [Record, "string"]
    if let Value::Array(arr) = schema {
        // Check if it's a nullable union
        let has_null = arr.iter().any(|v| {
            v == &Value::String("null".into())
                || v.get("type") == Some(&Value::String("null".into()))
        });
        if has_null {
            // Create nullable map
            *schema = serde_json::json!({
                "type": ["null", "object"],
                "additionalProperties": {"type": "string"}
            });
        } else {
            // Non-nullable map
            *schema = serde_json::json!({
                "type": "object",
                "additionalProperties": {"type": "string"}
            });
        }
        return;
    }

    if let Value::Object(obj) = schema {
        // Check if already a map
        if obj.contains_key("additionalProperties") {
            return;
        }
        preprocess_map_values(obj, config);

        // Check for nullable type: {"type": ["null", "object"]}
        let is_nullable = obj
            .get("type")
            .and_then(|t| t.as_array())
            .map(|arr| arr.contains(&Value::String("null".into())))
            .unwrap_or(false);

        // The values' common schema (string if they have none), before properties go
        let values = forced_map_value_schema(obj, Some(field_name), config);

        // Convert to map (a list's items are in the values now)
        obj.shift_remove("properties");
        obj.shift_remove("required");
        obj.shift_remove("anyOf");
        obj.shift_remove("items");

        if is_nullable {
            obj.insert("type".to_string(), serde_json::json!(["null", "object"]));
        } else {
            obj.insert("type".to_string(), serde_json::json!("object"));
        }
        obj.insert("additionalProperties".to_string(), values);
    }
}

/// How the merged schema is rewritten (map inference and unification).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Rewrite {
    /// Written out as `Value`s and rewritten in place: the way debugging takes, so that it
    /// reports as it did
    Values,
    /// Written out as a DAG, each distinct part once, and rewritten there: only the
    /// rewritten schema is written out as `Value`s
    Dag,
}

/// The builder's schema, rewritten.
fn rewrite_merged(builder: SchemaBuilder, config: &SchemaInferenceConfig, way: Rewrite) -> Value {
    // It changes only a field forced to be a map
    let forced_maps = config.force_field_types.values().any(|t| t == "map");
    match way {
        Rewrite::Values => {
            let mut schema = crate::phase::write_schema(|| builder.to_schema());
            // The builder's tree is freed on the rayon pool while the schema is rewritten
            rayon::spawn(move || drop(builder));
            crate::phase::rewrite(|| {
                profile!(
                    config,
                    "Applying force field types ({})",
                    current_time_hms()
                );
                if forced_maps {
                    preprocess_force_field_types(&mut schema, config);
                }
                profile!(config, "Rewriting objects ({})", current_time_hms());
                rewrite_objects(&mut schema, None, config, true);
            });
            schema
        }
        Rewrite::Dag => {
            let mut on = OnDag::new(crate::dag::Dag::new(), config);
            let schema = crate::phase::write_schema(|| builder.to_dag(&mut on.dag));
            rayon::spawn(move || drop(builder));
            let schema = crate::phase::rewrite(|| {
                profile!(
                    config,
                    "Applying force field types ({})",
                    current_time_hms()
                );
                let schema = match forced_maps {
                    true => on.preprocess_force_field_types(schema),
                    false => schema,
                };
                profile!(config, "Rewriting objects ({})", current_time_hms());
                on.rewrite_objects(schema, None, true)
            });
            crate::phase::materialise(|| on.dag.to_value(schema))
        }
    }
}

/// Infer JSON schema from a collection of JSON strings
pub fn infer_json_schema_from_strings<S: AsRef<str> + Sync>(
    json_strings: &[S],
    config: SchemaInferenceConfig,
) -> Result<SchemaInferenceResult, String> {
    let way = if config.debug {
        Rewrite::Values
    } else {
        Rewrite::Dag
    };
    infer_json_schema_from_strings_by(json_strings, config, way)
}

/// `infer_json_schema_from_strings`, its merged schema rewritten the given way.
pub(crate) fn infer_json_schema_from_strings_by<S: AsRef<str> + Sync>(
    json_strings: &[S],
    config: SchemaInferenceConfig,
    way: Rewrite,
) -> Result<SchemaInferenceResult, String> {
    profile!(
        config,
        "Processing {} strings ({})",
        json_strings.len(),
        current_time_hms()
    );
    debug!(config, "Schema inference config: {:#?}", config);
    if json_strings.is_empty() {
        return Err("No JSON strings provided".to_string());
    }

    // Wrap the entire genson-rs interaction in panic handling
    let result = panic::catch_unwind(AssertUnwindSafe(
        || -> Result<SchemaInferenceResult, String> {
            // Create schema builder
            let mut builder = get_builder(config.schema_uri.as_deref());

            profile!(config, "Starting preparation loop ({})", current_time_hms());

            let use_parallel = std::env::var("GENSON_PARALLEL")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or_else(|_| json_strings.len() >= PARALLEL_THRESHOLD);

            let processed_count = if use_parallel {
                match process_json_strings_interned(json_strings, &config, &mut builder) {
                    Some(processed_count) => processed_count,
                    None => crate::phase::rows(|| {
                        process_json_strings_parallel(json_strings, &config, &mut builder)
                    })?,
                }
            } else {
                crate::phase::rows(|| {
                    process_json_strings_sequential(json_strings, &config, &mut builder)
                })?
            };

            // Get final schema
            let mut final_schema = rewrite_merged(builder, &config, way);
            profile!(config, "Reordering unions ({})", current_time_hms());
            crate::phase::reorder(|| reorder_unions(&mut final_schema));

            #[cfg(feature = "avro")]
            if config.avro {
                let avro_schema = crate::phase::avro(|| {
                    SchemaInferenceResult {
                        schema: final_schema,
                        processed_count,
                    }
                    .to_avro_schema(
                        "genson", // namespace
                        Some(""),
                        Some(""), // base_uri
                        false,    // don't split top-level
                    )
                });
                return Ok(SchemaInferenceResult {
                    schema: avro_schema,
                    processed_count,
                });
            }

            Ok(SchemaInferenceResult {
                schema: final_schema,
                processed_count,
            })
        },
    ));

    // Handle the result of panic::catch_unwind
    match result {
        Ok(Ok(schema_result)) => Ok(schema_result),
        Ok(Err(e)) => Err(e),
        Err(_panic) => Err("JSON schema inference failed due to invalid JSON input".to_string()),
    }
}

#[cfg(test)]
mod tests {
    include!("tests/schema.rs");
}
