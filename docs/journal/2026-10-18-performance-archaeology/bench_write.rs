//! MEASUREMENT ONLY: writing a chunk's typed claims (inferred and decoded by `ArrowPlan`
//! with wikidata-pq's claims options) as Parquet, as `write_struct_column` writes them, `N`
//! times (3 by default): the time and size of each write, to `$OUT/written.parquet`.
//!
//! Copy to genson-core/examples/ and run with `--features avro,parquet`:
//! `OUT=dir N=8 bench_write chunk.parquet`
use genson_core::normalise::{prune_schema, MapEncoding, NormaliseConfig, NormalisePlan};
use genson_core::parquet::avro_record_fields;
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let column = genson_core::parquet::read_string_column_arrays(&args[1], "claims").unwrap();
    let rows = column.rows();
    let set = |names: &[&str]| names.iter().map(|s| s.to_string()).collect();
    let present: Vec<&str> = rows.iter().flatten().copied().collect();
    let config = SchemaInferenceConfig {
        delimiter: Some(b'\n'),
        map_threshold: 0,
        unify_maps: true,
        force_field_types: [("mainsnak".to_string(), "record".to_string())].into(),
        force_scalar_promotion: set(&[
            "mainsnak",
            "datavalue",
            "precision",
            "latitude",
            "longitude",
        ]),
        no_unify: set(&["qualifiers"]),
        wrap_root: Some("claims".to_string()),
        max_builders: Some(100),
        avro: true,
        ..Default::default()
    };
    let schema = infer_json_schema_from_strings(&present, config)
        .unwrap()
        .schema;
    let prune = set(&["mainsnak__string", "value__string", "value", "error"]);
    let mut out_schema = schema.clone();
    prune_schema(&mut out_schema, &prune);
    let fields = avro_record_fields(&out_schema, false).unwrap();
    let cfg = NormaliseConfig {
        empty_as_null: true,
        coerce_string: false,
        map_encoding: MapEncoding::KeyValueEntries,
        wrap_root: Some("claims".to_string()),
    };
    let plan = NormalisePlan::new(&schema, &cfg, &prune);
    let arrow = plan.to_arrow(&fields).unwrap();
    let batch = rows.len().div_ceil(rayon::current_num_threads() * 4).max(1);
    let arrays: Vec<_> = rows
        .chunks(batch)
        .enumerate()
        .map(|(b, c)| arrow.normalise(c, b * batch).unwrap().0)
        .collect();
    let out = format!(
        "{}/written.parquet",
        std::env::var("OUT").unwrap_or("/tmp".into())
    );
    for _ in 0..std::env::var("N").map_or(3, |n| n.parse().unwrap()) {
        let t = Instant::now();
        genson_core::parquet::write_struct_column(&out, "claims", arrays.clone(), &fields, None)
            .unwrap();
        println!(
            "write_struct_column {:.3}s, {} bytes",
            t.elapsed().as_secs_f64(),
            std::fs::metadata(&out).unwrap().len()
        );
    }
}
