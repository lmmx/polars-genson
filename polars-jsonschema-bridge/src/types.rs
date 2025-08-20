//! Common types and error definitions for JSON Schema ↔ Polars conversions.

use polars::prelude::PolarsError;

/// Create a PolarsError::ComputeError with the given message.
pub fn conversion_error(msg: impl Into<String>) -> PolarsError {
    PolarsError::ComputeError(msg.into().into())
}
