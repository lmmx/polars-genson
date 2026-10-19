//! Normalised rows as JSON text by way of the typed decoder: rows decoded straight into
//! Arrow ([`ArrowPlan`]) and written back as JSON, rather than parsed into `Value`s,
//! normalised and serialised. The text is the same.

use super::{ArrowPlan, NormalisePlan};
use arrow::array::{Array, AsArray, PrimitiveArray};
use arrow::datatypes::{DataType, Field, FieldRef, Fields, Float64Type};
use arrow::error::ArrowError;
use arrow::json::writer::{make_encoder, Encoder, EncoderFactory, EncoderOptions, NullableEncoder};
use rayon::prelude::*;
use std::sync::Arc;

/// Each row normalised against `plan`'s schema, as JSON text: what `serde_json::to_string`
/// gives for [`NormalisePlan::normalise`] of the row parsed (`null` where it is missing or
/// not valid JSON). The rows go a batch at a time through the typed decoder, against
/// `fields` (the schema's, by `avro_record_fields`, with native maps under the mapping
/// encoding), and are written from Arrow with nulls given; a batch the decoder declines,
/// and all of them without `fields` or where they are too few for the schema's columns to
/// pay for building (a row for each 8), through `Value`s. `plan` prunes nothing.
pub fn normalise_to_json(
    plan: &NormalisePlan<'_>,
    fields: Option<&Fields>,
    rows: &[Option<&str>],
) -> Vec<String> {
    let by_value = |row: &Option<&str>| {
        let value = row
            .and_then(|row| serde_json::from_str(row).ok())
            .unwrap_or(serde_json::Value::Null);
        serde_json::to_string(&plan.normalise(value).0).expect("a Value serialises")
    };
    // A record without fields is `{}` for any input, which an Arrow struct without children
    // does not keep apart from null
    let fields = fields.filter(|fields| !fields.iter().any(|f| holds_empty_struct(f.data_type())));
    // A batch builds every column of the schema: it holds a row for each 8 columns at least,
    // and rows too few to fill one go through `Value`s
    let columns: usize = fields.map_or(0, |f| f.iter().map(|f| leaf_columns(f.data_type())).sum());
    let least = columns.div_ceil(8);
    let typed = fields
        .filter(|_| rows.len() >= least)
        .and_then(|fields| plan.to_arrow(fields));
    let Some(typed) = typed else {
        return rows.par_iter().map(by_value).collect();
    };
    // A missing row, which the typed decoder keeps null, is normalised as null is
    let missing = by_value(&None);
    let batch = rows
        .len()
        .div_ceil(rayon::current_num_threads() * 4)
        .max(least)
        .max(1);
    rows.par_chunks(batch)
        .enumerate()
        .flat_map_iter(|(b, chunk)| {
            written(&typed, chunk, b * batch, &missing)
                .unwrap_or_else(|| chunk.iter().map(by_value).collect())
        })
        .collect()
}

/// The columns `data_type` decodes into: its leaves.
fn leaf_columns(data_type: &DataType) -> usize {
    match data_type {
        DataType::Struct(fields) => fields
            .iter()
            .map(|f| leaf_columns(f.data_type()))
            .sum::<usize>()
            .max(1),
        DataType::List(item) | DataType::Map(item, _) => leaf_columns(item.data_type()),
        _ => 1,
    }
}

/// Whether `data_type` is or holds a struct without fields.
fn holds_empty_struct(data_type: &DataType) -> bool {
    match data_type {
        DataType::Struct(fields) => {
            fields.is_empty() || fields.iter().any(|f| holds_empty_struct(f.data_type()))
        }
        DataType::List(item) | DataType::Map(item, _) => holds_empty_struct(item.data_type()),
        _ => false,
    }
}

/// `chunk`'s rows decoded typed and written as JSON (a missing row as `missing`), unless
/// the decoder declines them.
fn written(
    typed: &ArrowPlan<'_>,
    chunk: &[Option<&str>],
    first_row: usize,
    missing: &str,
) -> Option<Vec<String>> {
    let (array, _) = typed.normalise(chunk, first_row)?;
    let field: FieldRef = Arc::new(Field::new("", array.data_type().clone(), true));
    let options = EncoderOptions::default()
        .with_explicit_nulls(true)
        .with_encoder_factory(Arc::new(SerdeFloats));
    let mut encoder = make_encoder(&field, &array, &options).ok()?;
    let mut buf = Vec::new();
    (0..array.len())
        .map(|i| {
            if chunk[i].is_none() {
                return Some(missing.to_string());
            }
            buf.clear();
            encoder.encode(i, &mut buf);
            String::from_utf8(buf.clone()).ok()
        })
        .collect()
}

/// Floats written as `serde_json` writes them (`ryu`), where `arrow-json`'s own differ.
#[derive(Debug)]
struct SerdeFloats;

impl EncoderFactory for SerdeFloats {
    fn make_default_encoder<'a>(
        &self,
        _field: &'a FieldRef,
        array: &'a dyn Array,
        _options: &'a EncoderOptions,
    ) -> Result<Option<NullableEncoder<'a>>, ArrowError> {
        if array.data_type() != &DataType::Float64 {
            return Ok(None);
        }
        let floats = array.as_primitive::<Float64Type>();
        let nulls = floats.nulls().cloned();
        Ok(Some(NullableEncoder::new(Box::new(Floats(floats)), nulls)))
    }
}

struct Floats<'a>(&'a PrimitiveArray<Float64Type>);

impl Encoder for Floats<'_> {
    fn encode(&mut self, idx: usize, out: &mut Vec<u8>) {
        serde_json::to_writer(&mut *out, &self.0.value(idx)).expect("a float serialises");
    }
}
