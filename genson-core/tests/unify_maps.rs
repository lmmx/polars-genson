use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};

#[test]
fn test_unify_maps_compatible_records() {
    let json_strings = vec![
        r#"{"letter": {"a": {"alphabet": 0, "vowel": 0, "frequency": 0.0817}}}"#.to_string(),
        r#"{"letter": {"b": {"alphabet": 1, "consonant": 0, "frequency": 0.0150}}}"#.to_string(),
        r#"{"letter": {"c": {"alphabet": 2, "consonant": 1, "frequency": 0.0278}}}"#.to_string(),
    ];

    let config = SchemaInferenceConfig {
        map_threshold: 3,
        unify_maps: true,
        #[cfg(feature = "avro")]
        avro: true,
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&json_strings, config).unwrap();
    let schema = result.schema;

    println!(
        "Compatible records schema: {}",
        serde_json::to_string_pretty(&schema).unwrap()
    );

    #[cfg(feature = "avro")]
    {
        // Should create a map with unified record values
        let letter_field = &schema["fields"][0];
        assert_eq!(letter_field["name"], "letter");
        assert_eq!(letter_field["type"]["type"], "map");

        // Values should be unified record with all fields
        let values_schema = &letter_field["type"]["values"];
        assert_eq!(values_schema["type"], "record");

        let field_names: Vec<&str> = values_schema["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["name"].as_str().unwrap())
            .collect();

        // Should contain all unique fields from both record types
        assert!(field_names.contains(&"alphabet"));
        assert!(field_names.contains(&"frequency"));
        assert!(field_names.contains(&"vowel"));
        assert!(field_names.contains(&"consonant"));
    }

    #[cfg(not(feature = "avro"))]
    {
        // JSON Schema format
        let letter_field = &schema["properties"]["letter"];
        assert!(letter_field.get("additionalProperties").is_some());
        assert!(letter_field.get("properties").is_none());
    }
}

#[test]
fn test_unify_maps_disabled_by_default() {
    let json_strings = vec![
        r#"{"letter": {"a": {"alphabet": 0, "vowel": 0, "frequency": 0.0817}}}"#.to_string(),
        r#"{"letter": {"b": {"alphabet": 1, "consonant": 0, "frequency": 0.0150}}}"#.to_string(),
    ];

    let config = SchemaInferenceConfig {
        map_threshold: 2,
        unify_maps: false, // disabled
        #[cfg(feature = "avro")]
        avro: true,
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&json_strings, config).unwrap();
    let schema = result.schema;

    println!(
        "Disabled unify_maps schema: {}",
        serde_json::to_string_pretty(&schema).unwrap()
    );

    #[cfg(feature = "avro")]
    {
        // Should remain as fixed record with separate fields
        let letter_field = &schema["fields"][0];
        assert_eq!(letter_field["name"], "letter");
        assert_eq!(letter_field["type"]["type"], "record");

        // Should have separate fields for a and b
        let field_names: Vec<&str> = letter_field["type"]["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["name"].as_str().unwrap())
            .collect();

        assert!(field_names.contains(&"a"));
        assert!(field_names.contains(&"b"));
    }

    #[cfg(not(feature = "avro"))]
    {
        // JSON Schema format - should have properties for a, b
        let letter_field = &schema["properties"]["letter"];
        assert!(letter_field.get("properties").is_some());
        let properties = letter_field["properties"].as_object().unwrap();
        assert!(properties.contains_key("a"));
        assert!(properties.contains_key("b"));
    }
}

#[test]
fn test_unify_maps_incompatible_field_types() {
    let json_strings = vec![
        r#"{"data": {"a": {"name": "Alice", "age": 30}, "b": {"name": "Bob", "age": "twenty-five"}}}"#.to_string(),
    ];

    let config = SchemaInferenceConfig {
        map_threshold: 1,
        unify_maps: true,
        #[cfg(feature = "avro")]
        avro: true,
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&json_strings, config).unwrap();
    let schema = result.schema;

    println!(
        "Incompatible types schema: {}",
        serde_json::to_string_pretty(&schema).unwrap()
    );

    #[cfg(feature = "avro")]
    {
        let data_field = &schema["fields"][0];
        assert_eq!(data_field["name"], "data");

        // Should become a map due to threshold, but values should NOT be unified due to type conflict
        assert_eq!(data_field["type"]["type"], "map");

        // Values should be a record with separate fields for a and b (unification failed)
        let values_record = &data_field["type"]["values"];
        assert_eq!(values_record["type"], "record");

        let field_names: Vec<&str> = values_record["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["name"].as_str().unwrap())
            .collect();

        assert!(field_names.contains(&"a"));
        assert!(field_names.contains(&"b"));

        // Verify the age fields have different types (proving unification failed)
        let a_field = values_record["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == "a")
            .unwrap();
        let b_field = values_record["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == "b")
            .unwrap();

        let a_age_type = &a_field["type"]["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == "age")
            .unwrap()["type"];
        let b_age_type = &b_field["type"]["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == "age")
            .unwrap()["type"];

        assert_eq!(a_age_type, "int");
        assert_eq!(b_age_type, "string");
        assert_ne!(
            a_age_type, b_age_type,
            "Age types should differ, proving unification was rejected"
        );
    }
}

#[test]
fn test_unify_maps_below_threshold() {
    let json_strings = vec![
        r#"{"letter": {"a": {"alphabet": 0, "vowel": 0}}}"#.to_string(),
        r#"{"letter": {"b": {"alphabet": 1, "consonant": 0}}}"#.to_string(),
    ];

    let config = SchemaInferenceConfig {
        map_threshold: 10, // above the 2 keys we have
        unify_maps: true,
        #[cfg(feature = "avro")]
        avro: true,
        ..Default::default()
    };

    let result = infer_json_schema_from_strings(&json_strings, config).unwrap();
    let schema = result.schema;

    #[cfg(feature = "avro")]
    {
        // Should remain as record due to threshold not being met
        let letter_field = &schema["fields"][0];
        assert_eq!(letter_field["name"], "letter");
        assert_eq!(letter_field["type"]["type"], "record");
    }

    #[cfg(not(feature = "avro"))]
    {
        // JSON Schema - should remain as properties due to threshold
        let letter_field = &schema["properties"]["letter"];
        assert!(letter_field.get("properties").is_some());
        assert!(letter_field.get("additionalProperties").is_none());
    }
}
