"""Type stubs for polars-genson."""

from __future__ import annotations

from typing import Any

import polars as pl

def infer_json_schema(
    expr: pl.Expr,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    schema_uri: str | None = "AUTO",
    merge_schemas: bool = True,
    debug: bool = False,
) -> pl.Expr: ...
def infer_polars_schema(
    expr: pl.Expr,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    merge_schemas: bool = True,
    debug: bool = False,
) -> pl.Expr: ...
def serialize_polars_schema(
    expr: pl.Expr,
    *,
    schema_uri: str | None = "https://json-schema.org/draft/2020-12/schema",
    title: str | None = None,
    description: str | None = None,
    optional_fields: list[str] | None = None,
    additional_properties: bool = False,
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
        schema_uri: str | None = "AUTO",
        merge_schemas: bool = True,
        debug: bool = False,
    ) -> dict[str, Any] | list[dict[str, Any]]: ...
    def infer_polars_schema(
        self,
        column: str,
        *,
        ignore_outer_array: bool = True,
        ndjson: bool = False,
        merge_schemas: bool = True,
        debug: bool = False,
    ) -> pl.Schema: ...
    def serialize_schema_to_json(
        self,
        *,
        schema_uri: str | None = "https://json-schema.org/draft/2020-12/schema",
        title: str | None = None,
        description: str | None = None,
        optional_fields: list[str] | None = None,
        additional_properties: bool = False,
        debug: bool = False,
    ) -> dict[str, Any]: ...

# Augment DataFrame with genson attribute
class DataFrame(pl.DataFrame):
    genson: GensonNamespace
