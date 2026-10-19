//! MEASUREMENT ONLY: which texts simd-json (as genson-core configures it) accepts, and as
//! what kind of number, against serde_json's `IgnoredAny` validation and `Value` parse; and
//! the nesting depth each accepts.
//!
//! Copy to genson-core/examples/ and run with `--features parquet,avro`: `probe_simd`
use serde::Deserialize;
use simd_json::prelude::*;
fn main() {
    let cases = [
        "-0", "0", "1e2", "1E2", "1.0", "-0.0", "123456789012345678901234567890", "18446744073709551615",
        "18446744073709551616", "-9223372036854775808", "-9223372036854775809", "1e999", "-1e999", "1e-999",
        "[\"\\ud800\"]", "[\"\\udc00\"]", "[\"\\ud83d\\ude00\"]", "[\"\\u0000\"]", "[\"a\tb\"]", "[\"\\x\"]",
        "01", "1.", ".5", "[1,]", "{\"a\":1,}", "{\"a\":1} x", "{\"a\":1}}", "[1] ", " [1]", "[1]\u{a0}", "\u{feff}[1]",
        "[1e400]", "[1.7976931348623157e308]", "[1.7976931348623159e308]", "[4.9e-324]", "[2e-324]",
    ];
    for c in cases {
        let mut b = c.as_bytes().to_vec();
        let simd = match simd_json::to_tape(&mut b) {
            Ok(tape) => {
                let v = tape.as_value();
                let kind = |v: simd_json::tape::Value| if v.is_f64() { "f64" } else if v.is_i64() { "i64" } else if v.is_u64() { "u64" } else if v.is_str() { "str" } else { "other" };
                let k = match v.as_array() { Some(a) => a.iter().next().map(kind).unwrap_or("empty"), None => kind(v) };
                format!("ok {k}")
            }
            Err(e) => format!("err {e}"),
        };
        let mut de = serde_json::Deserializer::from_str(c);
        let ignored = serde::de::IgnoredAny::deserialize(&mut de).and_then(|_| de.end()).is_ok();
        let value = serde_json::from_str::<serde_json::Value>(c).is_ok();
        println!("{c:45} simd {simd:40} ignored_any {ignored:5} value {value}");
    }
    for depth in [126, 127, 128, 129, 200, 1100] {
        let s = format!("{}1{}", "[".repeat(depth), "]".repeat(depth));
        let mut b = s.clone().into_bytes();
        let simd = simd_json::to_tape(&mut b).is_ok();
        let mut de = serde_json::Deserializer::from_str(&s);
        let ignored = serde::de::IgnoredAny::deserialize(&mut de).and_then(|_| de.end()).is_ok();
        println!("depth {depth}: simd {simd} ignored_any {ignored} value {}", serde_json::from_str::<serde_json::Value>(&s).is_ok());
    }
}
