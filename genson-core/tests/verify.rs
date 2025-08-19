use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};

#[test]
fn test_single_invalid_json_honest() {
    println!("Testing invalid JSON: {{\"invalid\": json}}");

    let json_strings = vec![r#"{"invalid": json}"#.to_string()];
    let result = infer_json_schema_from_strings(&json_strings, SchemaInferenceConfig::default());

    match result {
        Ok(schema_result) => {
            println!("RESULT: Got success (THIS IS BAD)");
            println!("Schema: {:?}", schema_result);
            panic!("This should have failed but got success instead");
        }
        Err(error_msg) => {
            println!("RESULT: Got error");
            println!("Error message: '{}'", error_msg);

            // Let's see what kind of error this actually is
            if error_msg.contains("panic") {
                println!("ERROR TYPE: This error came from a caught panic");
            } else {
                println!("ERROR TYPE: This appears to be a proper error return");
            }

            // Don't assert anything - just show what happened
        }
    }

    println!("Test completed");
}
