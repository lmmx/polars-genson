"""PROTOTYPE (measurement only): pick the Parquet writers' compression codec from the env.

genson-core's `open_writer` builds `WriterProperties::builder().build()`, whose codec is
UNCOMPRESSED; with the prototype, COMPRESSION=zstd1|zstd3|snappy|lz4 sets one (anything
else keeps UNCOMPRESSED). Used with bench_parquet_io.rs.

Usage: python proto_parquet_codec.py <checkout>
"""

import sys
from pathlib import Path

p = Path(sys.argv[1]) / "genson-core/src/parquet.rs"
s = p.read_text()
old = "    let props = WriterProperties::builder().build();\n"
new = """    let codec = match std::env::var("COMPRESSION").as_deref() {
        Ok("zstd1") => parquet::basic::Compression::ZSTD(parquet::basic::ZstdLevel::try_new(1).unwrap()),
        Ok("zstd3") => parquet::basic::Compression::ZSTD(parquet::basic::ZstdLevel::try_new(3).unwrap()),
        Ok("snappy") => parquet::basic::Compression::SNAPPY,
        Ok("lz4") => parquet::basic::Compression::LZ4_RAW,
        _ => parquet::basic::Compression::UNCOMPRESSED,
    };
    let props = WriterProperties::builder().set_compression(codec).build();
"""
assert s.count(old) == 1
p.write_text(s.replace(old, new))
print("proto_parquet_codec applied")
