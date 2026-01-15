use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use genson_core::genson_rs::{build_json_schema, get_builder, BuildConfig};
use std::hint::black_box;

// Small inline test data
const SMALL_OBJECT: &str = r#"{"name": "test", "age": 30, "active": true}"#;

const ARRAY_OF_OBJECTS: &str = r#"[
    {"id": 1, "name": "alice", "score": 95.5},
    {"id": 2, "name": "bob", "score": 87.3},
    {"id": 3, "name": "charlie", "score": 92.1}
]"#;

const NESTED_OBJECT: &str = r#"{
    "user": {"name": "test", "email": "test@example.com"},
    "metadata": {"created": 1234567890, "tags": ["a", "b", "c"]},
    "scores": [1, 2, 3, 4, 5]
}"#;

fn bench_small_object(c: &mut Criterion) {
    let mut group = c.benchmark_group("small_object");
    group.throughput(Throughput::Bytes(SMALL_OBJECT.len() as u64));

    group.bench_function("infer_schema", |b| {
        b.iter(|| {
            let mut data = SMALL_OBJECT.as_bytes().to_vec();
            let mut builder = get_builder(Some("AUTO"));
            let config = BuildConfig {
                delimiter: None,
                ignore_outer_array: false,
            };
            build_json_schema(&mut builder, black_box(&mut data), &config)
        })
    });

    group.finish();
}

fn bench_array_of_objects(c: &mut Criterion) {
    let mut group = c.benchmark_group("array_of_objects");
    group.throughput(Throughput::Bytes(ARRAY_OF_OBJECTS.len() as u64));

    group.bench_function("infer_schema", |b| {
        b.iter(|| {
            let mut data = ARRAY_OF_OBJECTS.as_bytes().to_vec();
            let mut builder = get_builder(Some("AUTO"));
            let config = BuildConfig {
                delimiter: None,
                ignore_outer_array: false,
            };
            build_json_schema(&mut builder, black_box(&mut data), &config)
        })
    });

    group.finish();
}

fn bench_nested_object(c: &mut Criterion) {
    let mut group = c.benchmark_group("nested_object");
    group.throughput(Throughput::Bytes(NESTED_OBJECT.len() as u64));

    group.bench_function("infer_schema", |b| {
        b.iter(|| {
            let mut data = NESTED_OBJECT.as_bytes().to_vec();
            let mut builder = get_builder(Some("AUTO"));
            let config = BuildConfig {
                delimiter: None,
                ignore_outer_array: false,
            };
            build_json_schema(&mut builder, black_box(&mut data), &config)
        })
    });

    group.finish();
}

// Optional: bench with larger synthetic data
fn bench_many_objects(c: &mut Criterion) {
    // Generate 1000 similar objects
    let objects: String = (0..1000)
        .map(|i| {
            format!(
                r#"{{"id":{},"value":"item_{}","flag":{}}}"#,
                i,
                i,
                i % 2 == 0
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut group = c.benchmark_group("many_objects_1k");
    group.throughput(Throughput::Bytes(objects.len() as u64));

    group.bench_function("infer_schema_newline_delim", |b| {
        b.iter(|| {
            let mut data = objects.as_bytes().to_vec();
            let mut builder = get_builder(Some("AUTO"));
            let config = BuildConfig {
                delimiter: Some(b'\n'),
                ignore_outer_array: false,
            };
            build_json_schema(&mut builder, black_box(&mut data), &config)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_small_object,
    bench_array_of_objects,
    bench_nested_object,
    bench_many_objects,
);

criterion_main!(benches);
