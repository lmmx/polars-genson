// MEASUREMENT ONLY: the rewrite of a merged schema (`preprocess_force_field_types`,
// `rewrite_objects`, `reorder_unions`) with wikidata-pq's claims options, timed per run,
// and a hash of the result. The merged schema is `builder.to_schema()` in
// `infer_json_schema_from_strings`, written to a file. Paste into
// genson-core/src/tests/schema.rs and run:
// `MERGED=merged.json RUNS=10 cargo test --release --features avro --lib -- profile_rewrite_merged --ignored --nocapture`
#[test]
#[ignore]
fn profile_rewrite_merged() {
    let text = std::fs::read_to_string(std::env::var("MERGED").unwrap()).unwrap();
    let config = SchemaInferenceConfig {
        delimiter: Some(b'\n'),
        map_threshold: 0,
        unify_maps: true,
        force_field_types: [("mainsnak".to_string(), "record".to_string())].into(),
        force_scalar_promotion: ["mainsnak", "datavalue", "precision", "latitude", "longitude"].iter().map(|s| s.to_string()).collect(),
        no_unify: ["qualifiers".to_string()].into(),
        wrap_root: Some("claims".to_string()),
        max_builders: Some(100),
        avro: true,
        ..Default::default()
    };
    let runs: usize = std::env::var("RUNS").map_or(1, |r| r.parse().unwrap());
    let mut out = String::new();
    for _ in 0..runs {
        let mut schema: Value = serde_json::from_str(&text).unwrap();
        let t = std::time::Instant::now();
        preprocess_force_field_types(&mut schema, &config);
        rewrite_objects(&mut schema, None, &config, true);
        reorder_unions(&mut schema);
        eprintln!("rewrite {:?}", t.elapsed());
        out = schema.to_string();
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&out, &mut h);
    eprintln!("REWRITTEN {} bytes hash {:x}", out.len(), std::hash::Hasher::finish(&h));
}
