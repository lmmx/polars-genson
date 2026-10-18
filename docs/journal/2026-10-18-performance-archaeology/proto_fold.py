"""PROTOTYPE (measurement only): fold strings into one builder per rayon worker.

Replaces the chunk loop of `process_json_strings_parallel` (a builder per string, hashed,
the distinct ones merged per chunk of `max_builders`) with one `try_fold` over all strings:
each worker adds its strings' documents straight into its own builder with `add_object`,
and the workers' builders are merged at the end. No per-string tree, no row hash, no
chunks. Forced map fields and `processed_count` are not handled (the corpora use neither).

Apply after acct_patch.py. Usage: python proto_fold.py <checkout>
"""

import sys

root = sys.argv[1] + "/genson-core/src"
p = f"{root}/schema.rs"
with open(p) as f:
    s = f.read()

start = s.index(
    "    for (chunk_idx, chunk) in json_strings.chunks(chunk_size).enumerate() {"
)
end = s.index('    profile!(config, "All chunks processed ({})", current_time_hms());')
fold = """    let _ = (chunk_size, forces_maps);
    seen_hashes.insert(0u64);
    let build_config = BuildConfig {
        delimiter: config.delimiter,
        ignore_outer_array: config.ignore_outer_array,
    };
    let ph = acct::Phase::start("chunk.parallel");
    let folded = json_strings
        .par_iter()
        .enumerate()
        .try_fold(
            || get_builder(config.schema_uri.as_deref()),
            |mut b, (i, json_str)| -> Result<SchemaBuilder, String> {
                let prepared =
                    acct::row(0, || prepare_json_bytes(json_str.as_ref().as_bytes(), i, config))?;
                if prepared.is_empty() {
                    return Ok(b);
                }
                let mut bytes = prepared.into_owned();
                acct::row(1, || crate::genson_rs::fold_documents(&mut b, &mut bytes, &build_config));
                acct::row(0, || drop(bytes));
                processed_one();
                Ok(b)
            },
        )
        .try_reduce_with(|mut a, b| {
            a.add_builder(b);
            Ok(a)
        });
    ph.end();
    let ph = acct::Phase::start("chunk.merge");
    if let Some(folded) = folded {
        builder.absorb(folded?);
    }
    ph.end();
    processed_count = PROCESSED.swap(0, std::sync::atomic::Ordering::Relaxed);
"""
s = s[:start] + fold + s[end:]
s = s.replace(
    "/// Process all JSON strings in parallel while maintaining order",
    """static PROCESSED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
fn processed_one() {
    PROCESSED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Process all JSON strings in parallel while maintaining order""",
)
with open(p, "w") as f:
    f.write(s)

g = f"{root}/genson_rs/mod.rs"
with open(g) as f:
    m = f.read()
m = m.replace(
    "/// trim the whitespace and non-JSON object characters",
    """/// PROTOTYPE: add each document of `json_slice` (split on the delimiter) to `builder`.
pub fn fold_documents(builder: &mut SchemaBuilder, json_slice: &mut [u8], config: &BuildConfig) {
    let delimiter = config.delimiter.unwrap_or(b'\\n');
    for document in json_slice.split_mut(|byte| *byte == delimiter) {
        let document = trim_to_object(document);
        if !document.is_empty() {
            build_single_json_object_schema(builder, document);
        }
    }
}

/// trim the whitespace and non-JSON object characters""",
)
with open(g, "w") as f:
    f.write(m)
print("proto_fold applied")
