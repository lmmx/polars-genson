#[cfg_attr(feature = "trace", crustrace::omni)]
mod _innermod {
    use crate::genson_rs::{build_json_schema, get_builder, BuildConfig};
    use serde::de::Error as DeError;
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    use std::borrow::Cow;
    use std::panic::{self, AssertUnwindSafe};

    /// Maximum length of JSON string to include in error messages before truncating
    const MAX_JSON_ERROR_LENGTH: usize = 100;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SchemaInferenceConfig {
        /// Whether to treat top-level arrays as streams of objects
        pub ignore_outer_array: bool,
        /// Delimiter for NDJSON format (None for regular JSON)
        pub delimiter: Option<u8>,
        /// Schema URI to use ("AUTO" for auto-detection)
        pub schema_uri: Option<String>,
        /// Threshold above which non-fixed keys are treated as a map
        pub map_threshold: usize,
        /// Maximum number of required keys a Map can have. If None, no gating based on required keys.
        /// If Some(n), objects with more than n required keys will be forced to Record type.
        pub map_max_required_keys: Option<usize>,
        /// Enable unification of compatible but non-homogeneous record schemas into maps
        pub unify_maps: bool,
        /// Force override of field treatment, e.g. {"labels": "map"}
        pub force_field_types: std::collections::HashMap<String, String>,
        /// Whether to promote scalar values to wrapped objects when they collide with record values
        /// during unification. If `true`, scalars are promoted under a synthetic property name derived from
        /// the parent field and the scalar type (e.g. "foo__string"). If `false`, don't unify on conflicts.
        pub wrap_scalars: bool,
        /// Wrap the inferred top-level schema under a single required field with this name.
        /// Example: wrap_root = Some("labels") turns `{...}` into
        /// `{"type":"object","properties":{"labels":{...}},"required":["labels"]}`.
        pub wrap_root: Option<String>,
        /// Whether to output Avro schema rather than regular JSON Schema.
        #[cfg(feature = "avro")]
        pub avro: bool,
        /// Enable debug output. When `true`, prints detailed information about schema inference
        /// processes including field unification, map detection, and scalar wrapping decisions.
        pub debug: bool,
    }

    impl Default for SchemaInferenceConfig {
        fn default() -> Self {
            Self {
                ignore_outer_array: true,
                delimiter: None,
                schema_uri: Some("AUTO".to_string()),
                map_threshold: 20,
                map_max_required_keys: None,
                unify_maps: false,
                force_field_types: std::collections::HashMap::new(),
                wrap_scalars: true,
                wrap_root: None,
                #[cfg(feature = "avro")]
                avro: false,
                debug: false,
            }
        }
    }

    impl SchemaInferenceConfig {
        pub fn debug(&self, args: std::fmt::Arguments) {
            if self.debug {
                eprintln!("{}", args);
            }
        }
    }

    #[macro_export]
    macro_rules! debug {
        ($cfg:expr, $($arg:tt)*) => {
            $cfg.debug(format_args!($($arg)*))
        };
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SchemaInferenceResult {
        pub schema: Value,
        pub processed_count: usize,
    }

    #[cfg(feature = "avro")]
    impl SchemaInferenceResult {
        pub fn to_avro_schema(
            &self,
            namespace: &str,
            utility_namespace: Option<&str>,
            base_uri: Option<&str>,
            split_top_level: bool,
        ) -> Value {
            avrotize::converter::jsons_to_avro(
                &self.schema,
                namespace,
                utility_namespace.unwrap_or(""),
                base_uri.unwrap_or("genson-core"),
                split_top_level,
            )
        }
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

    /// Normalize a schema that may be wrapped in one or more layers of
    /// `["null", <type>]` union arrays.
    ///
    /// During inference, schemas often get wrapped in a nullable-union
    /// more than once (e.g. `["null", ["null", {"type": "string"}]]`).
    /// This helper strips away *all* redundant layers of `["null", ...]`
    /// until only the innermost non-null schema remains.
    ///
    /// This ensures that equality checks and recursive unification don’t
    /// spuriously fail due to extra layers of null-wrapping.
    fn normalise_nullable(v: &serde_json::Value) -> &serde_json::Value {
        let mut current = v;
        loop {
            if let Some(arr) = current.as_array() {
                if arr.len() == 2 && arr.contains(&serde_json::Value::String("null".to_string())) {
                    // peel off the non-null element
                    current = arr
                        .iter()
                        .find(|x| *x != &serde_json::Value::String("null".to_string()))
                        .unwrap();
                    continue;
                }
            }
            return current;
        }
    }

    /// Return a string representation of a JSON Schema type.
    /// If it’s a union, pick the first non-"null" type.
    fn schema_type_str(schema: &Value) -> String {
        if let Some(t) = schema.get("type").and_then(|v| v.as_str()) {
            return t.to_string();
        }

        // handle union case: ["null", {"type": "string"}]
        if let Some(arr) = schema.as_array() {
            for v in arr {
                if v != "null" {
                    if let Some(t) = v.get("type").and_then(|x| x.as_str()) {
                        return t.to_string();
                    }
                }
            }
        }

        "unknown".to_string()
    }

    /// Check if a collection of record schemas can be unified into a single schema with selective nullable fields.
    ///
    /// This function determines whether heterogeneous record schemas are "unifiable" - meaning they
    /// can be merged into a single schema where only missing fields become nullable. This enables
    /// map inference for cases where record values have compatible but non-identical structures.
    ///
    /// Schemas are considered unifiable if:
    /// 1. All schemas represent record types (`"type": "object"` with `"properties"`)
    /// 2. Field names are either disjoint OR have identical types when they overlap
    /// 3. No field has conflicting type definitions across schemas
    ///
    /// Fields present in all schemas remain required, while fields missing from some schemas
    /// become nullable unions (e.g., `["null", {"type": "string"}]`).
    ///
    /// When `wrap_scalars` is enabled, scalar types that collide with object types are promoted
    /// to singleton objects under a synthetic key (e.g., `value__string`), allowing unification
    /// to succeed instead of failing.
    ///
    /// # Returns
    ///
    /// - `Some(unified_schema)` if schemas can be unified - contains all unique fields with selective nullability
    /// - `None` if schemas cannot be unified due to:
    ///   - Non-record types in the collection
    ///   - Conflicting field types (same field name, different types)
    ///   - Empty schema collection
    fn check_unifiable_schemas(
        schemas: &[Value],
        path: &str,
        config: &SchemaInferenceConfig,
    ) -> Option<Value> {
        if schemas.is_empty() {
            debug!(config, "{path}: failed (empty schema list)");
            return None;
        }

        // Only unify record schemas
        if !schemas
            .iter()
            .all(|s| s.get("type") == Some(&Value::String("object".into())))
        {
            // debug!(config, "{path}: failed (non-object schema): {schemas:?}");
            return None;
        }

        let mut all_fields = ordermap::OrderMap::new();
        let mut field_counts = std::collections::HashMap::new();

        // Helper function to check if two schemas are compatible (handling nullable vs non-nullable)
        let schemas_compatible = |existing: &Value, new: &Value| -> Option<Value> {
            if existing == new {
                return Some(existing.clone());
            }

            // Handle new JSON Schema nullable format: {"type": ["null", "string"]}
            let extract_nullable_info = |schema: &Value| -> (bool, Value) {
                if let Some(Value::Array(type_arr)) = schema.get("type") {
                    if type_arr.len() == 2 && type_arr.contains(&Value::String("null".into())) {
                        let non_null_type = type_arr
                            .iter()
                            .find(|t| *t != &Value::String("null".into()))
                            .unwrap();
                        (true, serde_json::json!({"type": non_null_type}))
                    } else {
                        (false, schema.clone())
                    }
                } else {
                    (false, schema.clone())
                }
            };

            let (existing_nullable, existing_inner) = extract_nullable_info(existing);
            let (new_nullable, new_inner) = extract_nullable_info(new);

            // If the inner types match, return the nullable version
            if existing_inner == new_inner {
                if existing_nullable || new_nullable {
                    let inner_type = existing_inner.get("type").unwrap();
                    return Some(serde_json::json!({
                        "type": ["null", inner_type]
                    }));
                } else {
                    return Some(existing_inner);
                }
            }

            None
        };

        // Collect all field types and count occurrences
        for (i, schema) in schemas.iter().enumerate() {
            if let Some(Value::Object(props)) = schema.get("properties") {
                for (field_name, field_schema) in props {
                    *field_counts.entry(field_name.clone()).or_insert(0) += 1;

                    match all_fields.entry(field_name.clone()) {
                        ordermap::map::Entry::Vacant(e) => {
                            debug!(config, "Schema[{i}] introduces new field `{field_name}`");

                            // Normalise before storing
                            e.insert(normalise_nullable(field_schema).clone());
                        }
                        ordermap::map::Entry::Occupied(mut e) => {
                            // Normalise both sides before comparison
                            let existing = normalise_nullable(e.get()).clone();
                            let new = normalise_nullable(field_schema).clone();

                            // First try the compatibility check for nullable/non-nullable
                            if let Some(compatible_schema) = schemas_compatible(&existing, &new) {
                                debug!(config, "Field `{field_name}` compatible (nullable/non-nullable unification)");
                                e.insert(compatible_schema);
                            } else if existing.get("type") == Some(&Value::String("object".into()))
                                && new.get("type") == Some(&Value::String("object".into()))
                            {
                                // Try recursive unify if both are objects
                                debug!(config,
                                    "Field `{field_name}` has conflicting object schemas, attempting recursive unify"
                                );
                                if let Some(unified) = check_unifiable_schemas(
                                    &[existing.clone(), new.clone()],
                                    &format!("{path}.{}", field_name),
                                    config,
                                ) {
                                    debug!(
                                        config,
                                        "Field `{field_name}` unified successfully after recursion"
                                    );
                                    e.insert(unified);
                                } else {
                                    debug!(config, "{path}.{}: failed to unify", field_name);
                                    return None;
                                }
                            } else {
                                // Handle scalar vs object promotion if wrap_scalars is enabled
                                if config.wrap_scalars {
                                    let existing_is_obj = existing.get("type")
                                        == Some(&Value::String("object".into()));
                                    let new_is_obj = field_schema.get("type")
                                        == Some(&Value::String("object".into()));

                                    if existing_is_obj ^ new_is_obj {
                                        // One is object, other is scalar → wrap scalar
                                        let (obj_schema, scalar_schema, scalar_side) =
                                            if existing_is_obj {
                                                (existing.clone(), field_schema.clone(), "new")
                                            } else {
                                                (field_schema.clone(), existing.clone(), "existing")
                                            };

                                        let type_suffix = schema_type_str(&scalar_schema);
                                        let wrapped_key =
                                            format!("{}__{}", field_name, type_suffix);

                                        debug!(config,
                                            "Promoting scalar on {} side: wrapping into object under key `{}`",
                                            scalar_side, wrapped_key
                                        );

                                        let mut wrapped_props = serde_json::Map::new();
                                        wrapped_props.insert(wrapped_key, scalar_schema.clone());

                                        let promoted = serde_json::json!({
                                            "type": "object",
                                            "properties": wrapped_props
                                        });

                                        // Recursively unify with the object schema
                                        if let Some(unified) = check_unifiable_schemas(
                                            &[obj_schema.clone(), promoted.clone()],
                                            &format!("{path}.{}", field_name),
                                            config,
                                        ) {
                                            debug!(config,
                                                "Field `{field_name}` unified successfully after scalar promotion"
                                            );
                                            e.insert(unified);
                                            continue;
                                        }
                                    }
                                }

                                // If we didn’t handle it, it’s a true conflict
                                debug!(config,
                                    "{path}.{field_name}: incompatible types:\n  existing={:#?}\n  new={:#?}",
                                    existing, field_schema
                                );
                                return None; // fundamentally incompatible types
                            }
                        }
                    }
                }
            } else {
                debug!(config, "Schema[{i}] has no properties object");
                return None;
            }
        }

        let total_schemas = schemas.len();
        let mut unified_properties = serde_json::Map::new();

        // Required in all -> non-nullable
        for (field_name, field_type) in &all_fields {
            let count = field_counts.get(field_name).unwrap_or(&0);
            if *count == total_schemas {
                debug!(
                    config,
                    "Field `{field_name}` present in all schemas → keeping non-nullable"
                );
                unified_properties.insert(field_name.clone(), field_type.clone());
            }
        }

        // Missing in some -> nullable
        for (field_name, field_type) in &all_fields {
            let count = field_counts.get(field_name).unwrap_or(&0);
            if *count < total_schemas {
                debug!(
                    config,
                    "Field `{field_name}` missing in {}/{} schemas → making nullable",
                    total_schemas - count,
                    total_schemas
                );

                // Create proper JSON Schema nullable syntax
                if let Some(type_str) = field_type.get("type").and_then(|t| t.as_str()) {
                    // Create a copy of the field_type and modify its type to be a union
                    let mut nullable_field = field_type.clone();
                    nullable_field["type"] = serde_json::json!(["null", type_str]);
                    unified_properties.insert(field_name.clone(), nullable_field);
                } else {
                    // Fallback for schemas without explicit type
                    unified_properties
                        .insert(field_name.clone(), serde_json::json!(["null", field_type]));
                }
            }
        }

        debug!(config, "Schemas unified successfully");
        Some(serde_json::json!({
            "type": "object",
            "properties": unified_properties
        }))
    }

    /// Post-process an inferred JSON Schema to rewrite certain object shapes as maps.
    ///
    /// This mutates the schema in place, applying user overrides and heuristics.
    ///
    /// # Rules
    /// - If the current field name matches a `force_field_types` override, that wins
    ///   (`"map"` rewrites to `additionalProperties`, `"record"` leaves as-is).
    /// - Otherwise, applies map inference heuristics based on:
    ///   - Total key cardinality (`map_threshold`)
    ///   - Required key cardinality (`map_max_required_keys`)
    ///   - Value homogeneity (all values must be homogeneous) OR
    ///   - Value unifiability (compatible record schemas when `unify_maps` enabled)
    /// - Recurses into nested objects/arrays, carrying field names down so overrides apply.
    fn rewrite_objects(
        schema: &mut Value,
        field_name: Option<&str>,
        config: &SchemaInferenceConfig,
    ) {
        if let Value::Object(obj) = schema {
            // --- Forced overrides by field name ---
            if let Some(name) = field_name {
                if let Some(forced) = config.force_field_types.get(name) {
                    match forced.as_str() {
                        "map" => {
                            obj.remove("properties");
                            obj.remove("required");
                            obj.insert(
                                "additionalProperties".to_string(),
                                serde_json::json!({ "type": "string" }),
                            );
                            return; // no need to apply heuristics or recurse
                        }
                        "record" => {
                            if let Some(props) =
                                obj.get_mut("properties").and_then(|p| p.as_object_mut())
                            {
                                for (k, v) in props {
                                    rewrite_objects(v, Some(k), config);
                                }
                            }
                            if let Some(items) = obj.get_mut("items") {
                                rewrite_objects(items, None, config);
                            }
                            return;
                        }
                        _ => {}
                    }
                }
            }

            // --- Heuristic rewrite ---
            if let Some(props) = obj.get("properties").and_then(|p| p.as_object()) {
                let key_count = props.len(); // |UK| - total keys observed
                let above_threshold = key_count >= config.map_threshold;

                // Copy out child schema shapes
                let child_schemas: Vec<Value> = props.values().cloned().collect();

                // Detect map-of-records only if:
                // - all children are identical
                // - and that child is itself an object with "properties" (i.e. a proper record)
                if above_threshold {
                    if let Some(first) = child_schemas.first() {
                        if first.get("type") == Some(&Value::String("object".into()))
                            && first.get("properties").is_some()
                            && child_schemas.len() > 1
                        {
                            let all_same = child_schemas.iter().all(|other| other == first);
                            if all_same {
                                obj.remove("properties");
                                obj.remove("required");
                                obj.insert("additionalProperties".to_string(), first.clone());
                                return;
                            }
                        }
                    }
                }

                // Calculate required key count |RK|
                let required_key_count = obj
                    .get("required")
                    .and_then(|r| r.as_array())
                    .map(|r| r.len())
                    .unwrap_or(0);

                // Check for unifiable schemas
                let mut unified_schema: Option<Value> = None;
                if let Some(first_schema) = props.values().next() {
                    if props.values().all(|schema| schema == first_schema) {
                        // Handle union types properly - extract the non-null type for additionalProperties
                        if let Value::Array(arr) = first_schema {
                            if arr.len() == 2 && arr.contains(&Value::String("null".to_string())) {
                                // This is a nullable union - extract the non-null type
                                let non_null_type = arr
                                    .iter()
                                    .find(|v| *v != &Value::String("null".to_string()))
                                    .unwrap();
                                unified_schema = Some(non_null_type.clone());
                            } else {
                                unified_schema = Some(first_schema.clone());
                            }
                        } else {
                            unified_schema = Some(first_schema.clone());
                        }
                    } else if config.unify_maps {
                        // Detect if these are all arrays of records
                        if child_schemas
                            .iter()
                            .all(|s| s.get("type") == Some(&Value::String("array".into())))
                        {
                            // Collect item schemas, short-circuit if any missing
                            let mut item_schemas = Vec::with_capacity(child_schemas.len());
                            let mut all_items_ok = true;
                            for s in &child_schemas {
                                if let Some(items) = s.get("items") {
                                    item_schemas.push(items.clone());
                                } else {
                                    all_items_ok = false;
                                    break;
                                }
                            }
                            if all_items_ok {
                                if let Some(unified_items) = check_unifiable_schemas(
                                    &item_schemas,
                                    field_name.unwrap_or(""),
                                    config,
                                ) {
                                    unified_schema = Some(serde_json::json!({
                                        "type": "array",
                                        "items": unified_items
                                    }));
                                }
                            }
                        } else {
                            unified_schema = check_unifiable_schemas(
                                &child_schemas,
                                field_name.unwrap_or(""),
                                config,
                            );
                        }
                    }
                }

                // Apply map inference logic
                let should_be_map = if above_threshold && unified_schema.is_some() {
                    if let Some(max_required) = config.map_max_required_keys {
                        required_key_count <= max_required
                    } else {
                        true
                    }
                } else {
                    false
                };

                if should_be_map {
                    if let Some(schema) = unified_schema {
                        obj.remove("properties");
                        obj.remove("required");
                        obj.insert("type".to_string(), Value::String("object".to_string()));
                        obj.insert("additionalProperties".to_string(), schema);
                        return;
                    }
                }
            }

            // --- Recurse into nested values ---
            if let Some(props) = obj.get_mut("properties").and_then(|p| p.as_object_mut()) {
                for (k, v) in props {
                    rewrite_objects(v, Some(k), config);
                }
            }
            if let Some(items) = obj.get_mut("items") {
                rewrite_objects(items, None, config);
            }
            for v in obj.values_mut() {
                rewrite_objects(v, None, config);
            }
        } else if let Value::Array(arr) = schema {
            for v in arr {
                rewrite_objects(v, None, config);
            }
        }
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
                for v in arr {
                    reorder_unions(v);
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
    fn type_string_rank(s: &str) -> usize {
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

    /// Infer JSON schema from a collection of JSON strings
    pub fn infer_json_schema_from_strings(
        json_strings: &[String],
        config: SchemaInferenceConfig,
    ) -> Result<SchemaInferenceResult, String> {
        debug!(config, "Schema inference config: {:#?}", config);
        if json_strings.is_empty() {
            return Err("No JSON strings provided".to_string());
        }

        // Wrap the entire genson-rs interaction in panic handling
        let result = panic::catch_unwind(AssertUnwindSafe(
            || -> Result<SchemaInferenceResult, String> {
                // Create schema builder
                let mut builder = get_builder(config.schema_uri.as_deref());

                // Build config for genson-rs
                let build_config = BuildConfig {
                    delimiter: config.delimiter,
                    ignore_outer_array: config.ignore_outer_array,
                };

                let mut processed_count = 0;

                // Process each JSON string
                for (i, json_str) in json_strings.iter().enumerate() {
                    if json_str.trim().is_empty() {
                        continue;
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
                            json_str.clone()
                        };

                        return Err(format!(
                            "Invalid JSON input at index {}: {} - JSON: {}",
                            i + 1,
                            parse_error,
                            truncated_json
                        ));
                    }

                    // Safe: JSON is valid, now hand off to genson-rs
                    let prepared_json: Cow<str> = if let Some(ref field) = config.wrap_root {
                        if config.delimiter == Some(b'\n') {
                            // NDJSON: wrap each line separately
                            let mut wrapped_lines = Vec::new();
                            for line in json_str.lines() {
                                let trimmed = line.trim();
                                if trimmed.is_empty() {
                                    continue;
                                }
                                let inner_val: Value =
                                    serde_json::from_str(trimmed).map_err(|e| {
                                        format!(
                                            "Failed to parse NDJSON line before wrap_root: {}",
                                            e
                                        )
                                    })?;
                                wrapped_lines
                                    .push(serde_json::json!({ field: inner_val }).to_string());
                            }
                            Cow::Owned(wrapped_lines.join("\n"))
                        } else {
                            // Single JSON doc
                            let inner_val: Value = serde_json::from_str(json_str).map_err(|e| {
                                format!("Failed to parse JSON before wrap_root: {}", e)
                            })?;
                            Cow::Owned(serde_json::json!({ field: inner_val }).to_string())
                        }
                    } else {
                        Cow::Borrowed(json_str)
                    };

                    let mut bytes = prepared_json.as_bytes().to_vec();

                    // Build schema incrementally - this is where panics happen
                    let _schema = build_json_schema(&mut builder, &mut bytes, &build_config);
                    processed_count += 1;
                }

                // Get final schema
                let mut final_schema = builder.to_schema();
                rewrite_objects(&mut final_schema, None, &config);
                reorder_unions(&mut final_schema);

                #[cfg(feature = "avro")]
                if config.avro {
                    let avro_schema = SchemaInferenceResult {
                        schema: final_schema.clone(),
                        processed_count,
                    }
                    .to_avro_schema(
                        "genson", // namespace
                        Some(""),
                        Some(""), // base_uri
                        false,    // don't split top-level
                    );
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
            Err(_panic) => {
                Err("JSON schema inference failed due to invalid JSON input".to_string())
            }
        }
    }

    #[cfg(test)]
    #[path = "../../tests/schema.rs"]
    mod tests;
}
pub use _innermod::*;
