// genson-core/tests/comprehensive_example.rs

use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use serde_json::json;

/// This test demonstrates the motivating example from the GitHub issue:
/// distinguishing between user records and dynamic attribute maps.
#[test]
fn test_motivating_example_user_records_vs_attribute_maps() {
    println!("=== Testing the motivating example ===");

    let json_strings = vec![
        json!({
            "user_id": 1,
            "profile": {
                "name": "Alice",
                "email": "alice@example.com"
            },
            "attributes": {
                "source": "web",
                "campaign": "summer2024",
                "region": "US"
            }
        })
        .to_string(),
        json!({
            "user_id": 2,
            "profile": {
                "name": "Bob",
                "email": "bob@example.com"
            },
            "attributes": {
                "source": "mobile",
                "utm_medium": "social"
            }
        })
        .to_string(),
        json!({
            "user_id": 3,
            "profile": {
                "name": "Charlie",
                "email": "charlie@example.com"
            },
            "attributes": {
                "source": "email"
            }
        })
        .to_string(),
    ];

    println!("Input data represents:");
    println!("- user_id: always present (required)");
    println!("- profile: always present with name+email (both required) → should be RECORD");
    println!("- attributes: 'source' usually present, other keys vary → should be MAP");
    println!();

    // Configuration that allows us to distinguish maps from records
    let config = SchemaInferenceConfig {
        map_threshold: 2,               // Low enough to consider both profile and attributes
        map_max_required_keys: Some(1), // Maps can have ≤1 required key, records typically have more
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&json_strings, config)
        .expect("Schema inference should succeed");

    let schema_str = serde_json::to_string_pretty(&result.schema).unwrap();
    println!("Generated schema:\n{}", schema_str);
    println!();

    // Analyze the results
    println!("=== Analysis ===");

    // Root level should be a record (user_id, profile, attributes all required)
    assert_eq!(result.schema["type"], "object");
    assert!(result.schema.get("properties").is_some());
    assert!(result.schema.get("additionalProperties").is_none());
    println!("✅ Root level: RECORD (user_id, profile, attributes all required)");

    // Profile should be a record (name and email both required)
    let profile = &result.schema["properties"]["profile"];
    assert_eq!(profile["type"], "object");
    assert!(profile.get("properties").is_some());
    assert!(profile.get("additionalProperties").is_none());

    // Verify profile has the expected structure
    assert!(profile["properties"]["name"].is_object());
    assert!(profile["properties"]["email"].is_object());
    println!("✅ profile: RECORD (name, email both required)");

    // Attributes should be a map (only 'source' is consistently required)
    let attributes = &result.schema["properties"]["attributes"];
    assert_eq!(attributes["type"], "object");
    assert!(attributes.get("additionalProperties").is_some());
    assert!(attributes.get("properties").is_none());

    // Verify it's a string map
    assert_eq!(attributes["additionalProperties"]["type"], "string");
    println!("✅ attributes: MAP (only 'source' required, other keys vary)");

    println!();
    println!("=== Summary ===");
    println!("Successfully distinguished:");
    println!("- Structured records (profile) with stable, required fields");
    println!("- Dynamic maps (attributes) with mostly optional, varying keys");
    println!("- This was achieved using map_threshold=2 + map_max_required_keys=1");
}

/// Test the exact example from the GitHub issue description
#[test]
fn test_github_issue_example() {
    println!("=== Testing exact GitHub issue example ===");

    let json_strings = vec![
        r#"{"id": 1, "labels": {"en": "hello", "fr": "bonjour"}}"#.to_string(),
        r#"{"id": 2, "labels": {"en": "world"}}"#.to_string(),
    ];

    println!("Data analysis:");
    println!("- Top level: 'id' and 'labels' always present → 2 required keys");
    println!("- labels: 'en' often present, 'fr' sometimes → 1 required key typically");
    println!();

    let config = SchemaInferenceConfig {
        map_threshold: 2,
        map_max_required_keys: Some(1), // Key insight: distinguishes based on required keys
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&json_strings, config)
        .expect("Schema inference should succeed");

    let schema_str = serde_json::to_string_pretty(&result.schema).unwrap();
    println!("Generated schema:\n{}", schema_str);

    // Document level: id and labels both required (2 > 1) → Record
    assert_eq!(result.schema["type"], "object");
    assert!(result.schema.get("properties").is_some());
    println!("✅ Document: RECORD (id, labels both required: 2 > 1)");

    // Labels: en is required, fr is optional (1 ≤ 1) → Map
    let labels = &result.schema["properties"]["labels"];
    assert_eq!(labels["type"], "object");
    assert!(labels.get("additionalProperties").is_some());
    assert!(labels.get("properties").is_none());
    println!("✅ labels: MAP (en required, fr optional: 1 ≤ 1)");

    println!();
    println!("🎯 SUCCESS: Correctly distinguished document (record) from labels (map)!");
}

/// Test demonstrating the boundary conditions and edge cases
#[test]
fn test_boundary_conditions() {
    println!("=== Testing boundary conditions ===");

    // Test case 1: Exactly at the threshold
    let case1 = vec![
        r#"{"data": {"always": "present", "sometimes": "here"}}"#.to_string(),
        r#"{"data": {"always": "present"}}"#.to_string(),
    ];

    let config1 = SchemaInferenceConfig {
        map_threshold: 2,
        map_max_required_keys: Some(1), // Exactly 1 required key allowed
        ..Default::default()
    };

    let result1 = infer_json_schema_from_strings(&case1, config1).unwrap();
    let data1 = &result1.schema["properties"]["data"];

    // Should be map (1 required key ≤ 1)
    assert!(data1.get("additionalProperties").is_some());
    println!("✅ Boundary case: exactly 1 required key → MAP");

    // Test case 2: Just over the threshold
    let case2 = vec![
        r#"{"data": {"req1": "always", "req2": "present"}}"#.to_string(),
        r#"{"data": {"req1": "always", "req2": "present"}}"#.to_string(),
    ];

    let config2 = SchemaInferenceConfig {
        map_threshold: 2,
        map_max_required_keys: Some(1), // 2 required keys > 1
        ..Default::default()
    };

    let result2 = infer_json_schema_from_strings(&case2, config2).unwrap();
    let data2 = &result2.schema["properties"]["data"];

    // Should be record (2 required keys > 1)
    assert!(data2.get("properties").is_some());
    println!("✅ Boundary case: 2 required keys > 1 → RECORD");

    // Test case 3: Zero threshold (strictest)
    let case3 = vec![
        r#"{"data": {"opt1": "maybe", "opt2": "perhaps"}}"#.to_string(),
        r#"{"data": {"opt3": "different"}}"#.to_string(),
    ];

    let config3 = SchemaInferenceConfig {
        map_threshold: 2,
        map_max_required_keys: Some(0), // Only allow fully optional maps
        ..Default::default()
    };

    let result3 = infer_json_schema_from_strings(&case3, config3).unwrap();
    let data3 = &result3.schema["properties"]["data"];

    // Should be map (0 required keys ≤ 0)
    assert!(data3.get("additionalProperties").is_some());
    println!("✅ Zero threshold: 0 required keys → MAP");

    println!("All boundary conditions working correctly! 🎯");
}

/// Performance and scalability test with larger datasets
#[test]
fn test_performance_with_larger_dataset() {
    println!("=== Testing performance with larger dataset ===");

    // Generate a realistic dataset with varying key patterns
    let mut json_strings = Vec::new();

    // Create 100 user records with varying attribute patterns
    for i in 1..=100 {
        let user_data = json!({
            "user_id": i,
            "profile": {
                "name": format!("User{}", i),
                "email": format!("user{}@example.com", i),
                "created_at": "2024-01-01"
            },
            "attributes": {
                "source": if i % 3 == 0 { "web" } else if i % 3 == 1 { "mobile" } else { "email" },
                // Add varying optional attributes
                "campaign": if i % 5 == 0 { Some(format!("campaign{}", i / 5)) } else { None },
                "region": if i % 7 == 0 { Some("EU") } else if i % 7 == 1 { Some("US") } else { None },
                "utm_source": if i % 11 == 0 { Some("google") } else { None },
            }
        });

        // Clean up None values
        let mut cleaned = serde_json::Map::new();
        if let Some(obj) = user_data.as_object() {
            for (k, v) in obj {
                if k == "attributes" {
                    let mut attr_map = serde_json::Map::new();
                    if let Some(attrs) = v.as_object() {
                        for (ak, av) in attrs {
                            if !av.is_null() {
                                attr_map.insert(ak.clone(), av.clone());
                            }
                        }
                    }
                    cleaned.insert(k.clone(), serde_json::Value::Object(attr_map));
                } else {
                    cleaned.insert(k.clone(), v.clone());
                }
            }
        }

        json_strings.push(serde_json::Value::Object(cleaned).to_string());
    }

    let start = std::time::Instant::now();

    let config = SchemaInferenceConfig {
        map_threshold: 3,
        map_max_required_keys: Some(2),
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&json_strings, config)
        .expect("Large dataset schema inference should succeed");

    let duration = start.elapsed();

    println!("Processed {} records in {:?}", json_strings.len(), duration);
    println!("Processed {} objects total", result.processed_count);

    // Verify the schema structure is still correct
    let profile = &result.schema["properties"]["profile"];
    assert!(profile.get("properties").is_some()); // Record

    let attributes = &result.schema["properties"]["attributes"];
    assert!(attributes.get("additionalProperties").is_some()); // Map

    println!("✅ Performance test passed - schema structure maintained at scale");

    // Basic performance assertion (should complete in reasonable time)
    assert!(duration.as_secs() < 5, "Should complete within 5 seconds");
}

/// Integration test showing before/after behavior
#[test]
fn test_before_and_after_comparison() {
    println!("=== Before/After Comparison ===");

    let json_strings = vec![
        r#"{"config": {"host": "localhost", "port": "8080", "debug": "true"}}"#.to_string(),
        r#"{"config": {"host": "prod.example.com", "port": "443"}}"#.to_string(),
    ];

    println!("Data: config objects where 'host' and 'port' are common, 'debug' is optional");
    println!();

    // OLD BEHAVIOR: Without map_max_required_keys (None = no gating)
    let old_config = SchemaInferenceConfig {
        map_threshold: 2,
        map_max_required_keys: None, // Old behavior
        ..Default::default()
    };

    let old_result = infer_json_schema_from_strings(&json_strings, old_config).unwrap();
    let old_config_field = &old_result.schema["properties"]["config"];

    println!("OLD BEHAVIOR (map_max_required_keys = None):");
    if old_config_field.get("additionalProperties").is_some() {
        println!("  config → MAP (because it meets map_threshold)");
    } else {
        println!("  config → RECORD");
    }

    // NEW BEHAVIOR: With map_max_required_keys gating
    let new_config = SchemaInferenceConfig {
        map_threshold: 2,
        map_max_required_keys: Some(1), // Gate based on required keys
        ..Default::default()
    };

    let new_result = infer_json_schema_from_strings(&json_strings, new_config).unwrap();
    let new_config_field = &new_result.schema["properties"]["config"];

    println!("NEW BEHAVIOR (map_max_required_keys = 1):");
    if new_config_field.get("additionalProperties").is_some() {
        println!("  config → MAP (≤1 required key)");
    } else {
        println!("  config → RECORD (>1 required key)");
    }

    println!();
    println!("This demonstrates how the new parameter provides finer control");
    println!("over map vs record inference based on key stability patterns.");

    // Verify that we now have more nuanced control
    assert_ne!(
        old_config_field.get("additionalProperties").is_some(),
        new_config_field.get("additionalProperties").is_some(),
        "The new parameter should provide different behavior options"
    );
}

#[cfg(feature = "avro")]
#[test]
fn test_avro_output_with_map_max_required_keys() {
    println!("=== Testing Avro output with map_max_required_keys ===");

    let json_strings = vec![
        r#"{"user": {"id": 1, "name": "Alice"}, "metadata": {"source": "web", "campaign": "2024"}}"#.to_string(),
        r#"{"user": {"id": 2, "name": "Bob"}, "metadata": {"source": "mobile"}}"#.to_string(),
    ];

    let config = SchemaInferenceConfig {
        avro: true,
        map_threshold: 2,
        map_max_required_keys: Some(1),
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&json_strings, config)
        .expect("Avro schema inference should succeed");

    let avro_str = serde_json::to_string_pretty(&result.schema).unwrap();
    println!("Avro schema:\n{}", avro_str);

    // Should contain Avro record structure
    assert!(avro_str.contains(r#""type": "record""#));

    // Should contain both record fields (user) and map fields (metadata)
    assert!(avro_str.contains(r#""name": "user""#));
    assert!(avro_str.contains(r#""name": "metadata""#));

    // The metadata field should be an Avro map
    assert!(avro_str.contains(r#""type": "map""#));

    println!("✅ Avro output correctly applies map_max_required_keys logic");
}
