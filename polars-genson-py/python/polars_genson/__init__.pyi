"""Type stubs for polars-genson."""

from __future__ import annotations

from typing import Any

import polars as pl

def infer_json_schema(
    expr: pl.Expr,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    schema_uri: str | None = "http://json-schema.org/schema#",
    merge_schemas: bool = True,
    debug: bool = False,
) -> pl.Expr: ...

class GensonNamespace:
    def __init__(self, df: pl.DataFrame) -> None: ...
    def infer_json_schema(
        self,
        column: str,
        *,
        ignore_outer_array: bool = True,
        ndjson: bool = False,
        schema_uri: str | None = "http://json-schema.org/schema#",
        merge_schemas: bool = True,
        debug: bool = False,
    ) -> dict[str, Any] | list[dict[str, Any]]: ...

# Augment DataFrame with genson attribute
class DataFrame(pl.DataFrame):
    genson: GensonNamespace
