# polars_genson

A Polars plugin for JSON schema inference from string columns using genson-rs.

## GensonNamespace

```python
GensonNamespace(df: DataFrame)
```

Namespace for JSON schema inference operations.

Source code in `polars-genson-py/python/polars_genson/__init__.py`

```python
def __init__(self, df: pl.DataFrame):
    self._df = df
```

### schema_to_json

```python
schema_to_json() -> str
```

Convert the DataFrame's schema to JSON string representation.

Returns:

| Type  | Description                                          |
| ----- | ---------------------------------------------------- |
| `str` | JSON string representation of the DataFrame's schema |

Source code in `polars-genson-py/python/polars_genson/__init__.py`

```python
def schema_to_json(self) -> str:
    """Convert the DataFrame's schema to JSON string representation.

    Returns
    -------
    str
        JSON string representation of the DataFrame's schema
    """
    return _rust_schema_to_json(self._df)
```

### infer_polars_schema

```python
infer_polars_schema(
    column: str,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    merge_schemas: bool = True,
    debug: bool = False,
    profile: bool = False,
    verbosity: Literal["Normal", "Verbose"] = "Normal",
    map_threshold: int = 20,
    map_max_required_keys: int | None = None,
    unify_maps: bool = False,
    no_unify: set[str] | None = None,
    force_field_types: dict[str, str] | None = None,
    force_parent_field_types: dict[str, str] | None = None,
    force_scalar_promotion: set[str] | None = None,
    wrap_scalars: bool = True,
    coerce_strings: bool = False,
    avro: bool = True,
    map_encoding: Literal["entries", "mapping", "kv"] = "mapping",
    wrap_root: bool | str | None = None,
    no_root_map: bool = True,
    max_builders: int | None = None,
) -> pl.Schema
```

Infer Polars schema from a string column containing JSON data.

Parameters:

| Name                       | Type                           | Description                                                                                                                                                                                                                                                                                                                                                                                                                       | Default                                                                               |
| -------------------------- | ------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| `column`                   | `str`                          | Name of the column containing JSON strings                                                                                                                                                                                                                                                                                                                                                                                        | *required*                                                                            |
| `ignore_outer_array`       | `bool`                         | Whether to treat top-level arrays as streams of objects                                                                                                                                                                                                                                                                                                                                                                           | `True`                                                                                |
| `ndjson`                   | `bool`                         | Whether to treat input as newline-delimited JSON                                                                                                                                                                                                                                                                                                                                                                                  | `False`                                                                               |
| `merge_schemas`            | `bool`                         | Whether to merge schemas from all rows (True) or return individual schemas (False)                                                                                                                                                                                                                                                                                                                                                | `True`                                                                                |
| `debug`                    | `bool`                         | Whether to print debug information                                                                                                                                                                                                                                                                                                                                                                                                | `False`                                                                               |
| `profile`                  | `bool`                         | Whether to print profiling information                                                                                                                                                                                                                                                                                                                                                                                            | `False`                                                                               |
| `verbosity`                | `str`                          | Whether to print verbose debug information                                                                                                                                                                                                                                                                                                                                                                                        | `"Normal"`                                                                            |
| `map_threshold`            | `int`                          | Number of keys above which a heterogeneous object may be rewritten as a map (unless overridden).                                                                                                                                                                                                                                                                                                                                  | `20`                                                                                  |
| `map_max_required_keys`    | `int`                          | Maximum number of required keys allowed for Map inference. Objects with more required keys will be forced to Record type. If None, no gating based on required key count.                                                                                                                                                                                                                                                         | `None`                                                                                |
| `unify_maps`               | `bool`                         | Enable unification of compatible but non-homogeneous record schemas into maps. When True, record schemas with compatible field types can be merged into a single map schema with selective nullable fields.                                                                                                                                                                                                                       | `False`                                                                               |
| `no_unify`                 | \`set[str]                     | None\`                                                                                                                                                                                                                                                                                                                                                                                                                            | Prevent unification of keys under these field names with their sibling record fields. |
| `force_field_types`        | `dict[str, str]`               | Explicit overrides for specific fields. Values must be "map" or "record". Example: {"labels": "map", "claims": "record"}.                                                                                                                                                                                                                                                                                                         | `None`                                                                                |
| `force_parent_field_types` | `dict[str, str]`               | Explicit overrides for fields based on their parent field name. Values must be "map" or "record". Example: {"labels": "map", "claims": "record"}.                                                                                                                                                                                                                                                                                 | `None`                                                                                |
| `force_scalar_promotion`   | `set[str]`                     | Set of field names that should always be promoted to wrapped scalars, even when they appear as simple scalars. Ensures schema stability for fields known to have heterogeneous types across chunks. Example: {"precision", "datavalue"}.                                                                                                                                                                                          | `None`                                                                                |
| `wrap_scalars`             | `bool`                         | Whether to promote scalar values into singleton objects when they appear in contexts where other rows provide objects, or scalars of another kind (an integer and a string, not an integer and a number). This keeps every value with its type, where the union would keep one kind. The promoted field name defaults to the parent key with a \_\_{type} suffix, e.g. a string under "value" becomes {"value\_\_string": "..."}. | `True`                                                                                |
| `coerce_strings`           | `bool`                         | Infer the dtypes for rows that normalise_json(coerce_strings=True) normalises: strings beside numbers or booleans are to be coerced to them, so they are not given fields of their own.                                                                                                                                                                                                                                           | `False`                                                                               |
| `avro`                     | `bool`                         | Whether to infer and convert through an Avro schema (the default) or through JSON Schema. The Avro route reports the dtypes that normalise_json produces (maps as map_encoding decodes them, one branch of a union). The JSON Schema route falls back to String for unions and nullable fields.                                                                                                                                   | `True`                                                                                |
| `map_encoding`             | `('mapping', 'entries', 'kv')` | The map_encoding the JSON is normalised with: each map gets the dtype that encoding decodes to, a pl.Map for "mapping" and a list of {key, value} structs for "kv". "entries" has none, and raises ValueError if the schema holds a map.                                                                                                                                                                                          | `"mapping"`                                                                           |
| `wrap_root`                | \`str                          | bool                                                                                                                                                                                                                                                                                                                                                                                                                              | None\`                                                                                |
| `no_root_map`              | `bool`                         | Prevent document root from becoming a map type, even if it meets map inference criteria                                                                                                                                                                                                                                                                                                                                           | `True`                                                                                |
| `max_builders`             | `int`                          | Maximum number of schema builders to create in parallel at once. Lower values reduce peak memory usage during schema inference. If None, processes all strings at once. Default is None.                                                                                                                                                                                                                                          | `None`                                                                                |

Returns:

| Type     | Description    |
| -------- | -------------- |
| \`Schema | list[Schema]\` |

Source code in `polars-genson-py/python/polars_genson/__init__.py`

```python
def infer_polars_schema(
    self,
    column: str,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    merge_schemas: bool = True,
    debug: bool = False,
    profile: bool = False,
    verbosity: Literal["Normal", "Verbose"] = "Normal",
    map_threshold: int = 20,
    map_max_required_keys: int | None = None,
    unify_maps: bool = False,
    no_unify: set[str] | None = None,
    force_field_types: dict[str, str] | None = None,
    force_parent_field_types: dict[str, str] | None = None,
    force_scalar_promotion: set[str] | None = None,
    wrap_scalars: bool = True,
    coerce_strings: bool = False,
    avro: bool = True,
    map_encoding: Literal["entries", "mapping", "kv"] = "mapping",
    wrap_root: bool | str | None = None,
    no_root_map: bool = True,
    max_builders: int | None = None,
) -> pl.Schema:
    # ) -> pl.Schema | list[pl.Schema]:
    """Infer Polars schema from a string column containing JSON data.

    Parameters
    ----------
    column : str
        Name of the column containing JSON strings
    ignore_outer_array : bool, default True
        Whether to treat top-level arrays as streams of objects
    ndjson : bool, default False
        Whether to treat input as newline-delimited JSON
    merge_schemas : bool, default True
        Whether to merge schemas from all rows (True) or return individual schemas (False)
    debug : bool, default False
        Whether to print debug information
    profile : bool, default False
        Whether to print profiling information
    verbosity : str, default "Normal"
        Whether to print verbose debug information
    map_threshold : int, default 20
        Number of keys above which a heterogeneous object may be rewritten
        as a map (unless overridden).
    map_max_required_keys : int, optional
        Maximum number of required keys allowed for Map inference. Objects with more
        required keys will be forced to Record type. If None, no gating based on
        required key count.
    unify_maps : bool, default False
        Enable unification of compatible but non-homogeneous record schemas into maps.
        When True, record schemas with compatible field types can be merged into a single
        map schema with selective nullable fields.
    no_unify: set[str] | None, default None
        Prevent unification of keys under these field names with their sibling record fields.
    force_field_types : dict[str, str], optional
        Explicit overrides for specific fields. Values must be `"map"` or `"record"`.
        Example: ``{"labels": "map", "claims": "record"}``.
    force_parent_field_types : dict[str, str], optional
        Explicit overrides for fields based on their parent field name. Values must be `"map"` or `"record"`.
        Example: ``{"labels": "map", "claims": "record"}``.
    force_scalar_promotion : set[str], optional
        Set of field names that should always be promoted to wrapped scalars,
        even when they appear as simple scalars. Ensures schema stability for
        fields known to have heterogeneous types across chunks.
        Example: ``{"precision", "datavalue"}``.
    wrap_scalars : bool, default True
        Whether to promote scalar values into singleton objects when they appear
        in contexts where other rows provide objects, or scalars of another kind
        (an integer and a string, not an integer and a number). This keeps every
        value with its type, where the union would keep one kind. The promoted field name defaults
        to the parent key with a ``__{type}`` suffix, e.g. a string under
        ``"value"`` becomes ``{"value__string": "..."}``.
    coerce_strings : bool, default False
        Infer the dtypes for rows that ``normalise_json(coerce_strings=True)``
        normalises: strings beside numbers or booleans are to be coerced to them, so
        they are not given fields of their own.
    avro : bool, default True
        Whether to infer and convert through an Avro schema (the default) or through
        JSON Schema. The Avro route reports the dtypes that ``normalise_json`` produces
        (maps as ``map_encoding`` decodes them, one branch of a union). The JSON
        Schema route falls back to ``String`` for unions and nullable fields.
    map_encoding : {"mapping", "entries", "kv"}, default "mapping"
        The ``map_encoding`` the JSON is normalised with: each map gets the dtype
        that encoding decodes to, a ``pl.Map`` for "mapping" and a list of
        ``{key, value}`` structs for "kv". "entries" has none, and raises
        ``ValueError`` if the schema holds a map.
    wrap_root : str | bool | None, default None
        If a string, wrap each JSON row under that key before inference.
        If ``True``, wrap under the column name. If ``None``, leave rows unchanged.
    no_root_map : bool, default True
        Prevent document root from becoming a map type, even if it meets map inference criteria
    max_builders : int, optional
        Maximum number of schema builders to create in parallel at once.
        Lower values reduce peak memory usage during schema inference.
        If None, processes all strings at once. Default is None.

    Returns
    -------
    pl.Schema | list[pl.Schema]
        The inferred schema (if merge_schemas=True) or list of schemas (if merge_schemas=False)
    """
    if not merge_schemas:
        raise NotImplementedError("Only merge schemas is implemented")
    fft = (
        {}
        if force_field_types is None
        else {"force_field_types": force_field_types}
    )
    fpft = (
        {}
        if force_parent_field_types is None
        else {"force_parent_field_types": force_parent_field_types}
    )
    wrap_root_field = column if wrap_root is True else wrap_root
    result = self._df.select(
        infer_polars_schema(
            pl.col(column),
            ignore_outer_array=ignore_outer_array,
            ndjson=ndjson,
            merge_schemas=merge_schemas,
            debug=debug,
            profile=profile,
            verbosity=verbosity,
            map_threshold=map_threshold,
            map_max_required_keys=map_max_required_keys,
            unify_maps=unify_maps,
            no_unify=no_unify,
            **fft,
            **fpft,
            force_scalar_promotion=(
                list(force_scalar_promotion) if force_scalar_promotion else []
            ),
            wrap_scalars=wrap_scalars,
            coerce_strings=coerce_strings,
            avro=avro,
            wrap_root=wrap_root_field,
            no_root_map=no_root_map,
            max_builders=max_builders,
        ).first()
    )

    # Extract the schema from the first column, which is the struct
    schema_fields = result.to_series().item()
    return pl.Schema(
        {
            field["name"]: _parse_polars_dtype(field["dtype"], map_encoding)
            for field in schema_fields
        }
    )
```

### infer_json_schema

```python
infer_json_schema(
    column: str,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    schema_uri: str | None = "http://json-schema.org/schema#",
    merge_schemas: bool = True,
    debug: bool = False,
    profile: bool = False,
    verbosity: Literal["Normal", "Verbose"] = "Normal",
    map_threshold: int = 20,
    map_max_required_keys: int | None = None,
    unify_maps: bool = False,
    no_unify: set[str] | None = None,
    force_field_types: dict[str, str] | None = None,
    force_parent_field_types: dict[str, str] | None = None,
    force_scalar_promotion: set[str] | None = None,
    wrap_scalars: bool = True,
    avro: bool = False,
    wrap_root: bool | str | None = None,
    no_root_map: bool = True,
    max_builders: int | None = None,
) -> dict[str, Any] | list[dict[str, Any]]
```

Infer JSON schema from a string column containing JSON data.

Parameters:

| Name                       | Type             | Description                                                                                                                                                                                                                                                                                                                                                                                                                       | Default                                                                               |
| -------------------------- | ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| `column`                   | `str`            | Name of the column containing JSON strings                                                                                                                                                                                                                                                                                                                                                                                        | *required*                                                                            |
| `ignore_outer_array`       | `bool`           | Whether to treat top-level arrays as streams of objects                                                                                                                                                                                                                                                                                                                                                                           | `True`                                                                                |
| `ndjson`                   | `bool`           | Whether to treat input as newline-delimited JSON                                                                                                                                                                                                                                                                                                                                                                                  | `False`                                                                               |
| `schema_uri`               | `str or None`    | Schema URI to use for the generated schema                                                                                                                                                                                                                                                                                                                                                                                        | `"http://json-schema.org/schema#"`                                                    |
| `merge_schemas`            | `bool`           | Whether to merge schemas from all rows (True) or return individual schemas (False)                                                                                                                                                                                                                                                                                                                                                | `True`                                                                                |
| `debug`                    | `bool`           | Whether to print debug information                                                                                                                                                                                                                                                                                                                                                                                                | `False`                                                                               |
| `profile`                  | `bool`           | Whether to print profiling information                                                                                                                                                                                                                                                                                                                                                                                            | `False`                                                                               |
| `verbosity`                | `str`            | Whether to print verbose debug information                                                                                                                                                                                                                                                                                                                                                                                        | `"Normal"`                                                                            |
| `map_threshold`            | `int`            | Number of keys above which a heterogeneous object may be rewritten as a map (unless overridden).                                                                                                                                                                                                                                                                                                                                  | `20`                                                                                  |
| `map_max_required_keys`    | `int`            | Maximum number of required keys allowed for Map inference. Objects with more required keys will be forced to Record type. If None, no gating based on required key count.                                                                                                                                                                                                                                                         | `None`                                                                                |
| `unify_maps`               | `bool`           | Enable unification of compatible but non-homogeneous record schemas into maps. When True, record schemas with compatible field types can be merged into a single map schema with selective nullable fields.                                                                                                                                                                                                                       | `False`                                                                               |
| `no_unify`                 | \`set[str]       | None\`                                                                                                                                                                                                                                                                                                                                                                                                                            | Prevent unification of keys under these field names with their sibling record fields. |
| `force_field_types`        | `dict[str, str]` | Explicit overrides for specific fields. Values must be "map" or "record". Example: {"labels": "map", "claims": "record"}.                                                                                                                                                                                                                                                                                                         | `None`                                                                                |
| `force_parent_field_types` | `dict[str, str]` | Explicit overrides for fields based on their parent field name. Values must be "map" or "record". Example: {"labels": "map", "claims": "record"}.                                                                                                                                                                                                                                                                                 | `None`                                                                                |
| `force_scalar_promotion`   | `set[str]`       | Set of field names that should always be promoted to wrapped scalars, even when they appear as simple scalars. Ensures schema stability for fields known to have heterogeneous types across chunks. Example: {"precision", "datavalue"}.                                                                                                                                                                                          | `None`                                                                                |
| `wrap_scalars`             | `bool`           | Whether to promote scalar values into singleton objects when they appear in contexts where other rows provide objects, or scalars of another kind (an integer and a string, not an integer and a number). This keeps every value with its type, where the union would keep one kind. The promoted field name defaults to the parent key with a \_\_{type} suffix, e.g. a string under "value" becomes {"value\_\_string": "..."}. | `True`                                                                                |
| `avro`                     | `bool`           | Whether to output an Avro schema instead of a JSON Schema.                                                                                                                                                                                                                                                                                                                                                                        | `False`                                                                               |
| `wrap_root`                | \`str            | bool                                                                                                                                                                                                                                                                                                                                                                                                                              | None\`                                                                                |
| `no_root_map`              | `bool`           | Prevent document root from becoming a map type, even if it meets map inference criteria                                                                                                                                                                                                                                                                                                                                           | `True`                                                                                |
| `max_builders`             | `int`            | Maximum number of schema builders to create in parallel at once. Lower values reduce peak memory usage during schema inference. If None, processes all strings at once. Default is None.                                                                                                                                                                                                                                          | `None`                                                                                |

Returns:

| Type   | Description  |
| ------ | ------------ |
| \`dict | list[dict]\` |

Source code in `polars-genson-py/python/polars_genson/__init__.py`

```python
def infer_json_schema(
    self,
    column: str,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    schema_uri: str | None = "http://json-schema.org/schema#",
    merge_schemas: bool = True,
    debug: bool = False,
    profile: bool = False,
    verbosity: Literal["Normal", "Verbose"] = "Normal",
    map_threshold: int = 20,
    map_max_required_keys: int | None = None,
    unify_maps: bool = False,
    no_unify: set[str] | None = None,
    force_field_types: dict[str, str] | None = None,
    force_parent_field_types: dict[str, str] | None = None,
    force_scalar_promotion: set[str] | None = None,
    wrap_scalars: bool = True,
    avro: bool = False,
    wrap_root: bool | str | None = None,
    no_root_map: bool = True,
    max_builders: int | None = None,
) -> dict | list[dict]:
    """Infer JSON schema from a string column containing JSON data.

    Parameters
    ----------
    column : str
        Name of the column containing JSON strings
    ignore_outer_array : bool, default True
        Whether to treat top-level arrays as streams of objects
    ndjson : bool, default False
        Whether to treat input as newline-delimited JSON
    schema_uri : str or None, default "http://json-schema.org/schema#"
        Schema URI to use for the generated schema
    merge_schemas : bool, default True
        Whether to merge schemas from all rows (True) or return individual schemas (False)
    debug : bool, default False
        Whether to print debug information
    profile : bool, default False
        Whether to print profiling information
    verbosity : str, default "Normal"
        Whether to print verbose debug information
    map_threshold : int, default 20
        Number of keys above which a heterogeneous object may be rewritten
        as a map (unless overridden).
    map_max_required_keys : int, optional
        Maximum number of required keys allowed for Map inference. Objects with more
        required keys will be forced to Record type. If None, no gating based on
        required key count.
    unify_maps : bool, default False
        Enable unification of compatible but non-homogeneous record schemas into maps.
        When True, record schemas with compatible field types can be merged into a single
        map schema with selective nullable fields.
    no_unify: set[str] | None, default None
        Prevent unification of keys under these field names with their sibling record fields.
    force_field_types : dict[str, str], optional
        Explicit overrides for specific fields. Values must be `"map"` or `"record"`.
        Example: ``{"labels": "map", "claims": "record"}``.
    force_parent_field_types : dict[str, str], optional
        Explicit overrides for fields based on their parent field name. Values must be `"map"` or `"record"`.
        Example: ``{"labels": "map", "claims": "record"}``.
    force_scalar_promotion : set[str], optional
        Set of field names that should always be promoted to wrapped scalars,
        even when they appear as simple scalars. Ensures schema stability for
        fields known to have heterogeneous types across chunks.
        Example: ``{"precision", "datavalue"}``.
    wrap_scalars : bool, default True
        Whether to promote scalar values into singleton objects when they appear
        in contexts where other rows provide objects, or scalars of another kind
        (an integer and a string, not an integer and a number). This keeps every
        value with its type, where the union would keep one kind. The promoted field name defaults
        to the parent key with a ``__{type}`` suffix, e.g. a string under
        ``"value"`` becomes ``{"value__string": "..."}``.
    avro : bool, default False
        Whether to output an Avro schema instead of a JSON Schema.
    wrap_root : str | bool | None, default None
        If a string, wrap each JSON row under that key before inference.
        If ``True``, wrap under the column name. If ``None``, leave rows unchanged.
    no_root_map : bool, default True
        Prevent document root from becoming a map type, even if it meets map inference criteria
    max_builders : int, optional
        Maximum number of schema builders to create in parallel at once.
        Lower values reduce peak memory usage during schema inference.
        If None, processes all strings at once. Default is None.

    Returns
    -------
    dict | list[dict]
        The inferred JSON schema as a dictionary (if merge_schemas=True) or
        list of schemas (if merge_schemas=False)
    """
    wrap_root_field = column if wrap_root is True else wrap_root
    result = self._df.select(
        infer_json_schema(
            pl.col(column),
            ignore_outer_array=ignore_outer_array,
            ndjson=ndjson,
            schema_uri=schema_uri,
            merge_schemas=merge_schemas,
            debug=debug,
            profile=profile,
            verbosity=verbosity,
            map_threshold=map_threshold,
            map_max_required_keys=map_max_required_keys,
            unify_maps=unify_maps,
            no_unify=no_unify,
            force_field_types=force_field_types,
            force_parent_field_types=force_parent_field_types,
            force_scalar_promotion=(
                list(force_scalar_promotion) if force_scalar_promotion else []
            ),
            wrap_scalars=wrap_scalars,
            avro=avro,
            wrap_root=wrap_root_field,
            no_root_map=no_root_map,
            max_builders=max_builders,
        ).first()
    )

    # Extract the schema from the first column (whatever it's named)
    schema_json = result.to_series().item()
    if not isinstance(schema_json, str):
        raise ValueError(f"Expected string schema, got {type(schema_json)}")

    try:
        return orjson.loads(schema_json)
    except orjson.JSONDecodeError as e:
        raise ValueError(f"Failed to parse schema JSON: {e}") from e
```

### normalise_json

```python
normalise_json(
    column: str,
    *,
    decode: bool | Schema = True,
    unnest: bool = True,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    empty_as_null: bool = True,
    coerce_strings: bool = False,
    map_encoding: Literal["entries", "mapping", "kv"] | None = None,
    profile: bool = False,
    map_threshold: int = 20,
    map_max_required_keys: int | None = None,
    unify_maps: bool = False,
    no_unify: set[str] | None = None,
    force_field_types: dict[str, str] | None = None,
    force_parent_field_types: dict[str, str] | None = None,
    force_scalar_promotion: set[str] | None = None,
    wrap_scalars: bool = True,
    wrap_root: bool | str | None = None,
    no_root_map: bool = True,
    max_builders: int | None = None,
    avro_schema: str | dict | None = None,
) -> pl.Series
```

Normalise a JSON string column to conform to an inferred Avro schema.

This is a higher-level wrapper around :func:`normalise_json`, returning the results as a Polars Series instead of an expression.

Parameters:

| Name                       | Type                           | Description                                                                                                                                                                                                                                                                                                                                                                                                                                 | Default                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| -------------------------- | ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `column`                   | `str`                          | Name of the column containing JSON strings.                                                                                                                                                                                                                                                                                                                                                                                                 | *required*                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `decode`                   | \`bool                         | Schema\`                                                                                                                                                                                                                                                                                                                                                                                                                                    | Controls how the normalised JSON strings are decoded after normalisation: If False leave values as raw JSON strings. If True (default), decode into native Polars datatypes, with the schema inferred from the data (may be slower). If a polars.Schema (or a pl.Struct), decode to that dtype as str.json_decode decodes the normalised JSON: only its fields, taken by name, null where the data lacks them. The rows are decoded as for True and projected onto it, unless one of its leaf dtypes differs from the inferred one, when str.json_decode decodes the JSON text. |
| `unnest`                   | `bool`                         | Whether to expand the decoded struct into a column per field, in place of a single column of structs. Only applies if decode=True.                                                                                                                                                                                                                                                                                                          | `True`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `ignore_outer_array`       | `bool`                         | Whether to treat a top-level JSON array as a stream of objects instead of a single array value.                                                                                                                                                                                                                                                                                                                                             | `True`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `ndjson`                   | `bool`                         | Whether the input column contains newline-delimited JSON (NDJSON).                                                                                                                                                                                                                                                                                                                                                                          | `False`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `empty_as_null`            | `bool`                         | If True, normalise empty arrays and empty maps to null. If False, preserve them as empty collections.                                                                                                                                                                                                                                                                                                                                       | `True`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `coerce_strings`           | `bool`                         | If True, attempt to parse numeric/boolean values from strings (e.g. "42" → 42, "true" → true). If False, leave them as strings.                                                                                                                                                                                                                                                                                                             | `False`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `map_encoding`             | `('mapping', 'entries', 'kv')` | Encoding to use for Avro maps: - "mapping": plain JSON object ({"en":"Hello"}), decoded as a pl.Map - "entries": list of single-entry objects ([{"en":"Hello"}]), which cannot be decoded - "kv": list of {key,value} dicts ([{"key":"en","value":"Hello"}]), decoded as a list of {key, value} structs Defaults to "mapping", or when decode is a schema, to the encoding its maps decode from: "mapping" if it holds a pl.Map, else "kv". | `"mapping"`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `profile`                  | `bool`                         | Whether to display timing profile information                                                                                                                                                                                                                                                                                                                                                                                               | `False`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `map_threshold`            | `int`                          | Threshold above which objects with many varying keys are normalised as Avro maps instead of records.                                                                                                                                                                                                                                                                                                                                        | `20`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `map_max_required_keys`    | `int`                          | Maximum number of required keys allowed for Map inference during schema inference. Objects with more required keys will be forced to Record type. If None, no gating based on required key count.                                                                                                                                                                                                                                           | `None`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `unify_maps`               | `bool`                         | Enable unification of compatible but non-homogeneous record schemas into maps. When True, record schemas with compatible field types can be merged into a single map schema with selective nullable fields.                                                                                                                                                                                                                                 | `False`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `no_unify`                 | \`set[str]                     | None\`                                                                                                                                                                                                                                                                                                                                                                                                                                      | Prevent unification of keys under these field names with their sibling record fields.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `force_field_types`        | `dict[str, str]`               | Per-field overrides for schema inference (e.g. {"labels": "map"}).                                                                                                                                                                                                                                                                                                                                                                          | `None`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `force_parent_field_types` | `dict[str, str]`               | Per-field overrides for schema inference based on their parent field name (e.g. {"labels": "map"}).                                                                                                                                                                                                                                                                                                                                         | `None`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `force_scalar_promotion`   | `set[str]`                     | Set of field names that should always be promoted to wrapped scalars, even when they appear as simple scalars. Ensures schema stability for fields known to have heterogeneous types across chunks. Example: {"precision", "datavalue"}.                                                                                                                                                                                                    | `None`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `wrap_scalars`             | `bool`                         | Whether to promote scalar values into singleton objects when they appear in contexts where other rows provide objects, or scalars of another kind (an integer and a string, not an integer and a number). This keeps every value with its type, where the union would keep one kind. The promoted field name defaults to the parent key with a \_\_{type} suffix, e.g. a string under "value" becomes {"value\_\_string": "..."}.           | `True`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `wrap_root`                | \`str                          | bool                                                                                                                                                                                                                                                                                                                                                                                                                                        | None\`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `no_root_map`              | `bool`                         | Prevent document root from becoming a map type, even if it meets map inference criteria                                                                                                                                                                                                                                                                                                                                                     | `True`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `max_builders`             | `int`                          | Maximum number of schema builders to create in parallel at once. Lower values reduce peak memory usage during schema inference. If None, processes all strings at once. Default is None.                                                                                                                                                                                                                                                    | `None`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `avro_schema`              | \`str                          | dict\`                                                                                                                                                                                                                                                                                                                                                                                                                                      | An Avro schema (as JSON text or a dict) to normalise against in place of inferring one, for data whose schema is known: the inference pass is skipped, and the inference options have no effect. Rows are normalised to it as to an inferred one (fields it lacks dropped, those the rows lack null, empty arrays and maps null under empty_as_null).                                                                                                                                                                                                                           |

Returns:

| Type     | Description                                                                                                                                                                                                                  |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Series` | A Series of normalised JSON data. Each row is rewritten to match the same Avro schema, with consistent shape across the column. If unnest=True, the Series is expanded into multiple columns corresponding to schema fields. |

Source code in `polars-genson-py/python/polars_genson/__init__.py`

```python
def normalise_json(
    self,
    column: str,
    *,
    decode: bool | pl.Schema = True,
    unnest: bool = True,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    empty_as_null: bool = True,
    coerce_strings: bool = False,
    map_encoding: Literal["entries", "mapping", "kv"] | None = None,
    profile: bool = False,
    map_threshold: int = 20,
    map_max_required_keys: int | None = None,
    unify_maps: bool = False,
    no_unify: set[str] | None = None,
    force_field_types: dict[str, str] | None = None,
    force_parent_field_types: dict[str, str] | None = None,
    force_scalar_promotion: set[str] | None = None,
    wrap_scalars: bool = True,
    wrap_root: bool | str | None = None,
    no_root_map: bool = True,
    max_builders: int | None = None,
    avro_schema: str | dict | None = None,
) -> pl.Series:
    """Normalise a JSON string column to conform to an inferred Avro schema.

    This is a higher-level wrapper around :func:`normalise_json`, returning the
    results as a Polars Series instead of an expression.

    Parameters
    ----------
    column : str
        Name of the column containing JSON strings.
    decode : bool | pl.Schema, default True
        Controls how the normalised JSON strings are decoded after
        normalisation:

        - If False leave values as raw JSON strings.
        - If True (default), decode into native Polars datatypes,
          with the schema inferred from the data (may be slower).
        - If a polars.Schema (or a ``pl.Struct``), decode to that dtype as
          ``str.json_decode`` decodes the normalised JSON: only its fields, taken
          by name, null where the data lacks them. The rows are decoded as for
          ``True`` and projected onto it, unless one of its leaf dtypes differs
          from the inferred one, when ``str.json_decode`` decodes the JSON text.
    unnest : bool, default True
        Whether to expand the decoded struct into a column per field, in place
        of a single column of structs. Only applies if `decode=True`.
    ignore_outer_array : bool, default True
        Whether to treat a top-level JSON array as a stream of objects instead
        of a single array value.
    ndjson : bool, default False
        Whether the input column contains newline-delimited JSON (NDJSON).
    empty_as_null : bool, default True
        If True, normalise empty arrays and empty maps to ``null``.
        If False, preserve them as empty collections.
    coerce_strings : bool, default False
        If True, attempt to parse numeric/boolean values from strings
        (e.g. ``"42" → 42``, ``"true" → true``). If False, leave them as strings.
    map_encoding : {"mapping", "entries", "kv"}, optional
        Encoding to use for Avro maps:
        - "mapping": plain JSON object ({"en":"Hello"}), decoded as a ``pl.Map``
        - "entries": list of single-entry objects ([{"en":"Hello"}]), which
          cannot be decoded
        - "kv":      list of {key,value} dicts ([{"key":"en","value":"Hello"}]),
          decoded as a list of ``{key, value}`` structs
        Defaults to "mapping", or when ``decode`` is a schema, to the encoding its
        maps decode from: "mapping" if it holds a ``pl.Map``, else "kv".
    profile : bool, default False
        Whether to display timing profile information
    map_threshold : int, default 20
        Threshold above which objects with many varying keys are normalised
        as Avro maps instead of records.
    map_max_required_keys : int, optional
        Maximum number of required keys allowed for Map inference during schema
        inference. Objects with more required keys will be forced to Record type.
        If None, no gating based on required key count.
    unify_maps : bool, default False
        Enable unification of compatible but non-homogeneous record schemas into maps.
        When True, record schemas with compatible field types can be merged into a single
        map schema with selective nullable fields.
    no_unify: set[str] | None, default None
        Prevent unification of keys under these field names with their sibling record fields.
    force_field_types : dict[str, str], optional
        Per-field overrides for schema inference (e.g. ``{"labels": "map"}``).
    force_parent_field_types : dict[str, str], optional
        Per-field overrides for schema inference based on their parent field name (e.g. ``{"labels": "map"}``).
    force_scalar_promotion : set[str], optional
        Set of field names that should always be promoted to wrapped scalars,
        even when they appear as simple scalars. Ensures schema stability for
        fields known to have heterogeneous types across chunks.
        Example: ``{"precision", "datavalue"}``.
    wrap_scalars : bool, default True
        Whether to promote scalar values into singleton objects when they appear
        in contexts where other rows provide objects, or scalars of another kind
        (an integer and a string, not an integer and a number). This keeps every
        value with its type, where the union would keep one kind. The promoted field name defaults
        to the parent key with a ``__{type}`` suffix, e.g. a string under
        ``"value"`` becomes ``{"value__string": "..."}``.
    wrap_root : str | bool | None, default None
        If a string, wrap each JSON row under that key before normalisation.
        If ``True``, wrap under the column name. If ``None``, leave rows unchanged.
    no_root_map : bool, default True
        Prevent document root from becoming a map type, even if it meets map inference criteria
    max_builders : int, optional
        Maximum number of schema builders to create in parallel at once.
        Lower values reduce peak memory usage during schema inference.
        If None, processes all strings at once. Default is None.
    avro_schema : str | dict, optional
        An Avro schema (as JSON text or a dict) to normalise against in place of
        inferring one, for data whose schema is known: the inference pass is
        skipped, and the inference options have no effect. Rows are normalised to
        it as to an inferred one (fields it lacks dropped, those the rows lack
        null, empty arrays and maps null under ``empty_as_null``).

    Returns
    -------
    pl.Series
        A Series of normalised JSON data. Each row is rewritten to match the
        same Avro schema, with consistent shape across the column.
        If ``unnest=True``, the Series is expanded into multiple columns
        corresponding to schema fields.
    """
    wrap_root_field = column if wrap_root is True else wrap_root
    if not isinstance(decode, bool):
        # decode was passed as a Polars Schema (or a Struct dtype) directly
        dtype = decode if isinstance(decode, pl.Struct) else pl.Struct(decode)
        if map_encoding is None:
            map_encoding = "mapping" if contains_map(dtype) else "kv"
    elif map_encoding is None:
        map_encoding = "mapping"
    expr = normalise_json(
        pl.col(column),
        ignore_outer_array=ignore_outer_array,
        ndjson=ndjson,
        empty_as_null=empty_as_null,
        coerce_strings=coerce_strings,
        map_encoding=map_encoding,
        profile=profile,
        map_threshold=map_threshold,
        map_max_required_keys=map_max_required_keys,
        unify_maps=unify_maps,
        no_unify=no_unify,
        force_field_types=force_field_types,
        force_parent_field_types=force_parent_field_types,
        force_scalar_promotion=(
            list(force_scalar_promotion) if force_scalar_promotion else []
        ),
        wrap_scalars=wrap_scalars,
        wrap_root=wrap_root_field,
        no_root_map=no_root_map,
        max_builders=max_builders,
        avro_schema=avro_schema,
    )
    if decode:
        if map_encoding == "entries":
            # This could be done, it would always make record fields, ...but why?
            raise NotImplementedError(
                "map_encoding entries cannot decode to Polars"
            )

        # Inferred once, and the rows normalised straight into Polars dtypes: for
        # decode=True those of the inferred schema, for a dtype given those projected
        # onto it where that is exactly what json_decode makes of the JSON text. A
        # dtype holding one the typed route never makes goes to the text route
        typed = None
        if decode is True or typed_route_makes(dtype):
            typed = _rust_normalise_json_typed(
                self._df.get_column(column),
                orjson.dumps(
                    _normalise_kwargs(
                        ignore_outer_array=ignore_outer_array,
                        ndjson=ndjson,
                        empty_as_null=empty_as_null,
                        coerce_strings=coerce_strings,
                        map_encoding=map_encoding,
                        profile=profile,
                        map_threshold=map_threshold,
                        map_max_required_keys=map_max_required_keys,
                        unify_maps=unify_maps,
                        no_unify=no_unify,
                        force_field_types=force_field_types,
                        force_parent_field_types=force_parent_field_types,
                        force_scalar_promotion=force_scalar_promotion,
                        wrap_scalars=wrap_scalars,
                        wrap_root=wrap_root_field,
                        no_root_map=no_root_map,
                        max_builders=max_builders,
                        avro_schema=avro_schema,
                    )
                ).decode(),
            )
        if typed is not None:
            series, normalised_schema = typed
            inferred = pl.Struct(
                avro_to_polars_schema(normalised_schema, map_encoding=map_encoding)
            )
            result = series.cast(inferred).to_frame()
            if decode is True:
                return result.unnest(column) if unnest else result
            projected = project_dtype(pl.col(column), inferred, dtype)
            if projected is not None:
                result = result.select(projected.alias(column))
                if result.schema[column] == dtype:
                    return result.unnest(column) if unnest else result
            # Otherwise the JSON text route, decoded by json_decode itself
        if decode is True and avro_schema is not None:
            dtype = pl.Struct(
                avro_to_polars_schema(
                    avro_schema
                    if isinstance(avro_schema, str)
                    else orjson.dumps(avro_schema).decode(),
                    map_encoding=map_encoding,
                )
            )
        elif decode is True:
            # Infer Avro schema and convert it to Polars Schema
            schema = self.infer_polars_schema(
                column,
                ignore_outer_array=ignore_outer_array,
                ndjson=ndjson,
                merge_schemas=True,
                profile=profile,
                map_threshold=map_threshold,
                map_max_required_keys=map_max_required_keys,
                unify_maps=unify_maps,
                no_unify=no_unify,
                force_field_types=force_field_types,
                force_parent_field_types=force_parent_field_types,
                force_scalar_promotion=(
                    list(force_scalar_promotion) if force_scalar_promotion else []
                ),
                wrap_scalars=wrap_scalars,
                coerce_strings=coerce_strings,
                avro=True,
                map_encoding=map_encoding,
                wrap_root=wrap_root_field,
                no_root_map=no_root_map,
                max_builders=max_builders,
            )
            dtype = pl.Struct(schema)

        result = self._df.select(expr.str.json_decode(dtype=dtype))
        if unnest:
            result = result.unnest(expr.meta.output_name())
    else:
        result = self._df.select(expr).to_series()
    return result
```

## schema_to_json

```python
schema_to_json(schema: Schema, *, debug: bool = False) -> str
```

Convert a Polars schema to JSON string representation.

A `pl.Map` is written as its Arrow storage, a list of `{key, value}` structs, as the Rust polars crate has no map dtype, so `json_to_schema` reads it back as that list.

Parameters:

| Name     | Type     | Description                          | Default    |
| -------- | -------- | ------------------------------------ | ---------- |
| `schema` | `Schema` | The Polars schema to convert to JSON | *required* |
| `debug`  | `bool`   | Whether to print debug information   | `False`    |

Returns:

| Type  | Description                              |
| ----- | ---------------------------------------- |
| `str` | JSON string representation of the schema |

Source code in `polars-genson-py/python/polars_genson/__init__.py`

```python
def schema_to_json(schema: pl.Schema, *, debug: bool = False) -> str:
    """Convert a Polars schema to JSON string representation.

    A ``pl.Map`` is written as its Arrow storage, a list of ``{key, value}`` structs,
    as the Rust polars crate has no map dtype, so `json_to_schema` reads it back as
    that list.

    Parameters
    ----------
    schema : pl.Schema
        The Polars schema to convert to JSON
    debug : bool, default False
        Whether to print debug information

    Returns
    -------
    str
        JSON string representation of the schema
    """
    assert isinstance(schema, pl.Schema), (
        f"Expected Schema, got {type(schema)}: {schema}"
    )
    empty_df = schema.to_frame()
    return _rust_schema_to_json(empty_df, debug)
```

## json_to_schema

```python
json_to_schema(json_str: str, *, debug: bool = False) -> pl.Schema
```

Convert a JSON string to Polars schema.

Parameters:

| Name       | Type   | Description                             | Default    |
| ---------- | ------ | --------------------------------------- | ---------- |
| `json_str` | `str`  | JSON string to convert to Polars schema | *required* |
| `debug`    | `bool` | Whether to print debug information      | `False`    |

Returns:

| Name     | Type     | Description                                  |
| -------- | -------- | -------------------------------------------- |
| `schema` | `Schema` | The Polars schema representation of the JSON |

Source code in `polars-genson-py/python/polars_genson/__init__.py`

```python
def json_to_schema(json_str: str, *, debug: bool = False) -> pl.Schema:
    """Convert a JSON string to Polars schema.

    Parameters
    ----------
    json_str : str
        JSON string to convert to Polars schema
    debug : bool, default False
        Whether to print debug information

    Returns
    -------
    schema : pl.Schema
        The Polars schema representation of the JSON
    """
    df = _rust_json_to_schema(json_str, debug)
    schema = df.schema
    return schema
```

## plug

```python
plug(expr: Expr, changes_length: bool, **kwargs) -> pl.Expr
```

Wrap Polars' `register_plugin_function` helper to always pass the same `lib`.

Pass `changes_length` when using the `merge_schemas` (per-row) inference, as we only build a single schema in that case (so it'd be a waste to make more than one row).

Source code in `polars-genson-py/python/polars_genson/__init__.py`

```python
def plug(expr: pl.Expr, changes_length: bool, **kwargs) -> pl.Expr:
    """Wrap Polars' `register_plugin_function` helper to always pass the same `lib`.

    Pass `changes_length` when using the `merge_schemas` (per-row) inference, as we only
    build a single schema in that case (so it'd be a waste to make more than one row).
    """
    func_name = inspect.stack()[1].function
    return register_plugin_function(
        plugin_path=lib,
        function_name=func_name,
        args=expr,
        is_elementwise=False,  # This is an aggregation across rows
        changes_length=changes_length,
        kwargs=kwargs,
    )
```

## infer_json_schema

```python
infer_json_schema(
    expr: Expr,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    schema_uri: str | None = "http://json-schema.org/schema#",
    merge_schemas: bool = True,
    debug: bool = False,
    profile: bool = False,
    verbosity: Literal["Normal", "Verbose"] = "Normal",
    map_threshold: int = 20,
    map_max_required_keys: int | None = None,
    unify_maps: bool = False,
    no_unify: set[str] | None = None,
    force_field_types: dict[str, str] | None = None,
    force_parent_field_types: dict[str, str] | None = None,
    force_scalar_promotion: set[str] | None = None,
    wrap_scalars: bool = True,
    avro: bool = False,
    wrap_root: str | None = None,
    no_root_map: bool = True,
    max_builders: int | None = None,
) -> pl.Expr
```

Infer JSON schema from a string column containing JSON data.

Parameters:

| Name                       | Type             | Description                                                                                                                                                                                                                                                                                                                                                                                                                       | Default                                                                                         |
| -------------------------- | ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `expr`                     | `Expr`           | Expression representing a string column containing JSON data                                                                                                                                                                                                                                                                                                                                                                      | *required*                                                                                      |
| `ignore_outer_array`       | `bool`           | Whether to treat top-level arrays as streams of objects                                                                                                                                                                                                                                                                                                                                                                           | `True`                                                                                          |
| `ndjson`                   | `bool`           | Whether to treat input as newline-delimited JSON                                                                                                                                                                                                                                                                                                                                                                                  | `False`                                                                                         |
| `schema_uri`               | `str or None`    | Schema URI to use for the generated schema                                                                                                                                                                                                                                                                                                                                                                                        | `"http://json-schema.org/schema#"`                                                              |
| `merge_schemas`            | `bool`           | Whether to merge schemas from all rows (True) or return individual schemas (False)                                                                                                                                                                                                                                                                                                                                                | `True`                                                                                          |
| `debug`                    | `bool`           | Whether to print debug information                                                                                                                                                                                                                                                                                                                                                                                                | `False`                                                                                         |
| `profile`                  | `bool`           | Whether to print profiling information                                                                                                                                                                                                                                                                                                                                                                                            | `False`                                                                                         |
| `verbosity`                | `str`            | Whether to print verbose debug information                                                                                                                                                                                                                                                                                                                                                                                        | `"Normal"`                                                                                      |
| `map_threshold`            | `int`            | Number of keys above which a heterogeneous object may be rewritten as a map (unless overridden).                                                                                                                                                                                                                                                                                                                                  | `20`                                                                                            |
| `map_max_required_keys`    | `int`            | Maximum number of required keys allowed for Map inference. Objects with more required keys will be forced to Record type. If None, no gating based on required key count (preserves existing behavior).                                                                                                                                                                                                                           | `None`                                                                                          |
| `unify_maps`               | `bool`           | Enable unification of compatible but non-homogeneous record schemas into maps. When True, record schemas with compatible field types can be merged into a single map schema with selective nullable fields.                                                                                                                                                                                                                       | `False`                                                                                         |
| `no_unify`                 | \`set[str]       | None\`                                                                                                                                                                                                                                                                                                                                                                                                                            | Prevent unification of keys under these field names with their sibling record fields.           |
| `force_field_types`        | `dict[str, str]` | Explicit overrides for specific fields. Values must be "map" or "record". Example: {"labels": "map", "claims": "record"}.                                                                                                                                                                                                                                                                                                         | `None`                                                                                          |
| `force_parent_field_types` | `dict[str, str]` | Explicit overrides for fields based on their parent field name. Values must be "map" or "record". Example: {"labels": "map", "claims": "record"}.                                                                                                                                                                                                                                                                                 | `None`                                                                                          |
| `force_scalar_promotion`   | `set[str]`       | Set of field names that should always be promoted to wrapped scalars, even when they appear as simple scalars. Ensures schema stability for fields known to have heterogeneous types across chunks. Example: {"precision", "datavalue"}.                                                                                                                                                                                          | `None`                                                                                          |
| `wrap_scalars`             | `bool`           | Whether to promote scalar values into singleton objects when they appear in contexts where other rows provide objects, or scalars of another kind (an integer and a string, not an integer and a number). This keeps every value with its type, where the union would keep one kind. The promoted field name defaults to the parent key with a \_\_{type} suffix, e.g. a string under "value" becomes {"value\_\_string": "..."}. | `True`                                                                                          |
| `avro`                     | `bool`           | Whether to output an Avro schema instead of a JSON Schema.                                                                                                                                                                                                                                                                                                                                                                        | `False`                                                                                         |
| `wrap_root`                | \`str            | None\`                                                                                                                                                                                                                                                                                                                                                                                                                            | If a string, wrap each JSON row under that key before inference. If None, leave rows unchanged. |
| `no_root_map`              | `bool`           | Prevent document root from becoming a map type, even if it meets map inference criteria                                                                                                                                                                                                                                                                                                                                           | `True`                                                                                          |
| `max_builders`             | `int`            | Maximum number of schema builders to create in parallel at once. Lower values reduce peak memory usage during schema inference. If None, processes all strings at once. Default is None.                                                                                                                                                                                                                                          | `None`                                                                                          |

Returns:

| Type   | Description                                      |
| ------ | ------------------------------------------------ |
| `Expr` | Expression representing the inferred JSON schema |

Source code in `polars-genson-py/python/polars_genson/__init__.py`

```python
def infer_json_schema(
    expr: pl.Expr,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    schema_uri: str | None = "http://json-schema.org/schema#",
    merge_schemas: bool = True,
    debug: bool = False,
    profile: bool = False,
    verbosity: Literal["Normal", "Verbose"] = "Normal",
    map_threshold: int = 20,
    map_max_required_keys: int | None = None,
    unify_maps: bool = False,
    no_unify: set[str] | None = None,
    force_field_types: dict[str, str] | None = None,
    force_parent_field_types: dict[str, str] | None = None,
    force_scalar_promotion: set[str] | None = None,
    wrap_scalars: bool = True,
    avro: bool = False,
    wrap_root: str | None = None,
    no_root_map: bool = True,
    max_builders: int | None = None,
) -> pl.Expr:
    """Infer JSON schema from a string column containing JSON data.

    Parameters
    ----------
    expr : pl.Expr
        Expression representing a string column containing JSON data
    ignore_outer_array : bool, default True
        Whether to treat top-level arrays as streams of objects
    ndjson : bool, default False
        Whether to treat input as newline-delimited JSON
    schema_uri : str or None, default "http://json-schema.org/schema#"
        Schema URI to use for the generated schema
    merge_schemas : bool, default True
        Whether to merge schemas from all rows (True) or return individual schemas (False)
    debug : bool, default False
        Whether to print debug information
    profile : bool, default False
        Whether to print profiling information
    verbosity : str, default "Normal"
        Whether to print verbose debug information
    map_threshold : int, default 20
        Number of keys above which a heterogeneous object may be rewritten
        as a map (unless overridden).
    map_max_required_keys : int, optional
        Maximum number of required keys allowed for Map inference. Objects with more
        required keys will be forced to Record type. If None, no gating based on
        required key count (preserves existing behavior).
    unify_maps : bool, default False
        Enable unification of compatible but non-homogeneous record schemas into maps.
        When True, record schemas with compatible field types can be merged into a single
        map schema with selective nullable fields.
    no_unify: set[str] | None, default None
        Prevent unification of keys under these field names with their sibling record fields.
    force_field_types : dict[str, str], optional
        Explicit overrides for specific fields. Values must be `"map"` or `"record"`.
        Example: ``{"labels": "map", "claims": "record"}``.
    force_parent_field_types : dict[str, str], optional
        Explicit overrides for fields based on their parent field name. Values must be `"map"` or `"record"`.
        Example: ``{"labels": "map", "claims": "record"}``.
    force_scalar_promotion : set[str], optional
        Set of field names that should always be promoted to wrapped scalars,
        even when they appear as simple scalars. Ensures schema stability for
        fields known to have heterogeneous types across chunks.
        Example: ``{"precision", "datavalue"}``.
    wrap_scalars : bool, default True
        Whether to promote scalar values into singleton objects when they appear
        in contexts where other rows provide objects, or scalars of another kind
        (an integer and a string, not an integer and a number). This keeps every
        value with its type, where the union would keep one kind. The promoted field name defaults
        to the parent key with a ``__{type}`` suffix, e.g. a string under
        ``"value"`` becomes ``{"value__string": "..."}``.
    avro : bool, default False
        Whether to output an Avro schema instead of a JSON Schema.
    wrap_root : str | None, default None
        If a string, wrap each JSON row under that key before inference.
        If ``None``, leave rows unchanged.
    no_root_map : bool, default True
        Prevent document root from becoming a map type, even if it meets map inference criteria
    max_builders : int, optional
        Maximum number of schema builders to create in parallel at once.
        Lower values reduce peak memory usage during schema inference.
        If None, processes all strings at once. Default is None.

    Returns
    -------
    pl.Expr
        Expression representing the inferred JSON schema
    """
    kwargs = {
        "ignore_outer_array": ignore_outer_array,
        "ndjson": ndjson,
        "merge_schemas": merge_schemas,
        "debug": debug,
        "profile": profile,
        "verbosity": verbosity,
        "map_threshold": map_threshold,
        "map_max_required_keys": map_max_required_keys,
        "unify_maps": unify_maps,
        "no_unify": list(no_unify) if no_unify else [],
        "force_scalar_promotion": (
            list(force_scalar_promotion) if force_scalar_promotion else []
        ),
        "wrap_scalars": wrap_scalars,
        "avro": avro,
        "wrap_root": wrap_root,
        "no_root_map": no_root_map,
        "max_builders": max_builders,
    }
    if schema_uri is not None:
        kwargs["schema_uri"] = schema_uri
    if force_field_types is not None:
        kwargs["force_field_types"] = force_field_types
    if force_parent_field_types is not None:
        kwargs["force_parent_field_types"] = force_parent_field_types

    return plug(expr, changes_length=merge_schemas, **kwargs)
```

## infer_polars_schema

```python
infer_polars_schema(
    expr: Expr,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    merge_schemas: bool = True,
    debug: bool = False,
    profile: bool = False,
    verbosity: Literal["Normal", "Verbose"] = "Normal",
    map_threshold: int = 20,
    map_max_required_keys: int | None = None,
    unify_maps: bool = False,
    no_unify: set[str] | None = None,
    force_field_types: dict[str, str] | None = None,
    force_parent_field_types: dict[str, str] | None = None,
    force_scalar_promotion: set[str] | None = None,
    wrap_scalars: bool = True,
    coerce_strings: bool = False,
    avro: bool = True,
    wrap_root: str | None = None,
    no_root_map: bool = True,
    max_builders: int | None = None,
) -> pl.Expr
```

Infer Polars schema from a string column containing JSON data.

Parameters:

| Name                       | Type             | Description                                                                                                                                                                                                                                                                                                                                                                                                                       | Default                                                                                         |
| -------------------------- | ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `expr`                     | `Expr`           | Expression representing a string column containing JSON data                                                                                                                                                                                                                                                                                                                                                                      | *required*                                                                                      |
| `ignore_outer_array`       | `bool`           | Whether to treat top-level arrays as streams of objects                                                                                                                                                                                                                                                                                                                                                                           | `True`                                                                                          |
| `ndjson`                   | `bool`           | Whether to treat input as newline-delimited JSON                                                                                                                                                                                                                                                                                                                                                                                  | `False`                                                                                         |
| `merge_schemas`            | `bool`           | Whether to merge schemas from all rows (True) or return individual schemas (False)                                                                                                                                                                                                                                                                                                                                                | `True`                                                                                          |
| `debug`                    | `bool`           | Whether to print debug information                                                                                                                                                                                                                                                                                                                                                                                                | `False`                                                                                         |
| `profile`                  | `bool`           | Whether to print profiling information                                                                                                                                                                                                                                                                                                                                                                                            | `False`                                                                                         |
| `verbosity`                | `str`            | Whether to print verbose debug information                                                                                                                                                                                                                                                                                                                                                                                        | `"Normal"`                                                                                      |
| `map_threshold`            | `int`            | Number of keys above which a heterogeneous object may be rewritten as a map (unless overridden).                                                                                                                                                                                                                                                                                                                                  | `20`                                                                                            |
| `map_max_required_keys`    | `int`            | Maximum number of required keys allowed for Map inference. Objects with more required keys will be forced to Record type. If None, no gating based on required key count.                                                                                                                                                                                                                                                         | `None`                                                                                          |
| `unify_maps`               | `bool`           | Enable unification of compatible but non-homogeneous record schemas into maps. When True, record schemas with compatible field types can be merged into a single map schema with selective nullable fields.                                                                                                                                                                                                                       | `False`                                                                                         |
| `no_unify`                 | \`set[str]       | None\`                                                                                                                                                                                                                                                                                                                                                                                                                            | Prevent unification of keys under these field names with their sibling record fields.           |
| `force_field_types`        | `dict[str, str]` | Explicit overrides for specific fields. Values must be "map" or "record". Example: {"labels": "map", "claims": "record"}.                                                                                                                                                                                                                                                                                                         | `None`                                                                                          |
| `force_parent_field_types` | `dict[str, str]` | Explicit overrides for fields based on their parent field name. Values must be "map" or "record". Example: {"labels": "map", "claims": "record"}.                                                                                                                                                                                                                                                                                 | `None`                                                                                          |
| `force_scalar_promotion`   | `set[str]`       | Set of field names that should always be promoted to wrapped scalars, even when they appear as simple scalars. Ensures schema stability for fields known to have heterogeneous types across chunks. Example: {"precision", "datavalue"}.                                                                                                                                                                                          | `None`                                                                                          |
| `wrap_scalars`             | `bool`           | Whether to promote scalar values into singleton objects when they appear in contexts where other rows provide objects, or scalars of another kind (an integer and a string, not an integer and a number). This keeps every value with its type, where the union would keep one kind. The promoted field name defaults to the parent key with a \_\_{type} suffix, e.g. a string under "value" becomes {"value\_\_string": "..."}. | `True`                                                                                          |
| `coerce_strings`           | `bool`           | Infer the dtypes for rows that normalise_json(coerce_strings=True) normalises: strings beside numbers or booleans are to be coerced to them, so they are not given fields of their own.                                                                                                                                                                                                                                           | `False`                                                                                         |
| `avro`                     | `bool`           | Whether to infer and convert through an Avro schema (the default) or through JSON Schema. The Avro route reports the dtypes that normalise_json produces (maps as Map[String,V], one branch of a union). The JSON Schema route falls back to String for unions and nullable fields.                                                                                                                                               | `True`                                                                                          |
| `wrap_root`                | \`str            | None\`                                                                                                                                                                                                                                                                                                                                                                                                                            | If a string, wrap each JSON row under that key before inference. If None, leave rows unchanged. |
| `no_root_map`              | `bool`           | Prevent document root from becoming a map type, even if it meets map inference criteria                                                                                                                                                                                                                                                                                                                                           | `True`                                                                                          |
| `max_builders`             | `int`            | Maximum number of schema builders to create in parallel at once. Lower values reduce peak memory usage during schema inference. If None, processes all strings at once. Default is None.                                                                                                                                                                                                                                          | `None`                                                                                          |

Returns:

| Type   | Description                                                                           |
| ------ | ------------------------------------------------------------------------------------- |
| `Expr` | Expression yielding the inferred Polars schema (as a struct of {name, dtype} fields). |

Source code in `polars-genson-py/python/polars_genson/__init__.py`

```python
def infer_polars_schema(
    expr: pl.Expr,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    merge_schemas: bool = True,
    debug: bool = False,
    profile: bool = False,
    verbosity: Literal["Normal", "Verbose"] = "Normal",
    map_threshold: int = 20,
    map_max_required_keys: int | None = None,
    unify_maps: bool = False,
    no_unify: set[str] | None = None,
    force_field_types: dict[str, str] | None = None,
    force_parent_field_types: dict[str, str] | None = None,
    force_scalar_promotion: set[str] | None = None,
    wrap_scalars: bool = True,
    coerce_strings: bool = False,
    avro: bool = True,
    wrap_root: str | None = None,
    no_root_map: bool = True,
    max_builders: int | None = None,
) -> pl.Expr:
    """Infer Polars schema from a string column containing JSON data.

    Parameters
    ----------
    expr : pl.Expr
        Expression representing a string column containing JSON data
    ignore_outer_array : bool, default True
        Whether to treat top-level arrays as streams of objects
    ndjson : bool, default False
        Whether to treat input as newline-delimited JSON
    merge_schemas : bool, default True
        Whether to merge schemas from all rows (True) or return individual schemas (False)
    debug : bool, default False
        Whether to print debug information
    profile : bool, default False
        Whether to print profiling information
    verbosity : str, default "Normal"
        Whether to print verbose debug information
    map_threshold : int, default 20
        Number of keys above which a heterogeneous object may be rewritten
        as a map (unless overridden).
    map_max_required_keys : int, optional
        Maximum number of required keys allowed for Map inference. Objects with more
        required keys will be forced to Record type. If None, no gating based on
        required key count.
    unify_maps : bool, default False
        Enable unification of compatible but non-homogeneous record schemas into maps.
        When True, record schemas with compatible field types can be merged into a single
        map schema with selective nullable fields.
    no_unify: set[str] | None, default None
        Prevent unification of keys under these field names with their sibling record fields.
    force_field_types : dict[str, str], optional
        Explicit overrides for specific fields. Values must be `"map"` or `"record"`.
        Example: ``{"labels": "map", "claims": "record"}``.
    force_parent_field_types : dict[str, str], optional
        Explicit overrides for fields based on their parent field name. Values must be `"map"` or `"record"`.
        Example: ``{"labels": "map", "claims": "record"}``.
    force_scalar_promotion : set[str], optional
        Set of field names that should always be promoted to wrapped scalars,
        even when they appear as simple scalars. Ensures schema stability for
        fields known to have heterogeneous types across chunks.
        Example: ``{"precision", "datavalue"}``.
    wrap_scalars : bool, default True
        Whether to promote scalar values into singleton objects when they appear
        in contexts where other rows provide objects, or scalars of another kind
        (an integer and a string, not an integer and a number). This keeps every
        value with its type, where the union would keep one kind. The promoted field name defaults
        to the parent key with a ``__{type}`` suffix, e.g. a string under
        ``"value"`` becomes ``{"value__string": "..."}``.
    coerce_strings : bool, default False
        Infer the dtypes for rows that ``normalise_json(coerce_strings=True)`` normalises:
        strings beside numbers or booleans are to be coerced to them, so they are not given
        fields of their own.
    avro : bool, default True
        Whether to infer and convert through an Avro schema (the default) or through
        JSON Schema. The Avro route reports the dtypes that ``normalise_json`` produces
        (maps as ``Map[String,V]``, one branch of a union). The JSON Schema
        route falls back to ``String`` for unions and nullable fields.
    wrap_root : str | None, default None
        If a string, wrap each JSON row under that key before inference.
        If ``None``, leave rows unchanged.
    no_root_map : bool, default True
        Prevent document root from becoming a map type, even if it meets map inference criteria
    max_builders : int, optional
        Maximum number of schema builders to create in parallel at once.
        Lower values reduce peak memory usage during schema inference.
        If None, processes all strings at once. Default is None.

    Returns
    -------
    pl.Expr
        Expression yielding the inferred Polars schema (as a struct of {name, dtype} fields).
    """
    kwargs = {
        "ignore_outer_array": ignore_outer_array,
        "ndjson": ndjson,
        "merge_schemas": merge_schemas,
        "debug": debug,
        "profile": profile,
        "verbosity": verbosity,
        "map_threshold": map_threshold,
        "map_max_required_keys": map_max_required_keys,
        "unify_maps": unify_maps,
        "coerce_string": coerce_strings,
        "no_unify": list(no_unify) if no_unify else [],
        "force_scalar_promotion": (
            list(force_scalar_promotion) if force_scalar_promotion else []
        ),
        "wrap_scalars": wrap_scalars,
        "avro": avro,
        "wrap_root": wrap_root,
        "no_root_map": no_root_map,
        "max_builders": max_builders,
    }
    if not merge_schemas:
        url = "https://github.com/lmmx/polars-genson/issues/37"
        raise NotImplementedError("Merge schemas for Polars schemas is TODO: see {url}")
    if force_field_types is not None:
        kwargs["force_field_types"] = force_field_types
    if force_parent_field_types is not None:
        kwargs["force_parent_field_types"] = force_parent_field_types

    return plug(expr, changes_length=merge_schemas, **kwargs)
```

## normalise_json

```python
normalise_json(
    expr: Expr,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    empty_as_null: bool = True,
    coerce_strings: bool = False,
    map_encoding: Literal["entries", "mapping", "kv"] = "mapping",
    profile: bool = False,
    map_threshold: int = 20,
    map_max_required_keys: int | None = None,
    unify_maps: bool = False,
    no_unify: set[str] | None = None,
    force_field_types: dict[str, str] | None = None,
    force_parent_field_types: dict[str, str] | None = None,
    force_scalar_promotion: set[str] | None = None,
    wrap_scalars: bool = True,
    wrap_root: str | None = None,
    no_root_map: bool = True,
    max_builders: int | None = None,
    avro_schema: str | dict | None = None,
) -> pl.Expr
```

Normalise a JSON string column against an inferred Avro schema.

This performs schema inference once across all rows, then rewrites each row to conform to that schema. The output is a new column of JSON strings with consistent structure and datatypes.

Parameters:

| Name                       | Type                           | Description                                                                                                                                                                                                                                                                                                                                                                                                                       | Default                                                                                                                                                                                          |
| -------------------------- | ------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `expr`                     | `Expr`                         | Expression representing a string column of JSON data.                                                                                                                                                                                                                                                                                                                                                                             | *required*                                                                                                                                                                                       |
| `ignore_outer_array`       | `bool`                         | Treat a top-level JSON array as a stream of objects (like NDJSON).                                                                                                                                                                                                                                                                                                                                                                | `True`                                                                                                                                                                                           |
| `ndjson`                   | `bool`                         | Treat input as newline-delimited JSON rather than a single JSON document.                                                                                                                                                                                                                                                                                                                                                         | `False`                                                                                                                                                                                          |
| `empty_as_null`            | `bool`                         | Convert empty arrays/maps into null to preserve row count when exploding. Disable with False to keep empty collections.                                                                                                                                                                                                                                                                                                           | `True`                                                                                                                                                                                           |
| `coerce_strings`           | `bool`                         | If True, attempt to coerce string values into numeric/boolean types where the schema expects them. If False, unmatched strings become null.                                                                                                                                                                                                                                                                                       | `False`                                                                                                                                                                                          |
| `map_encoding`             | `('mapping', 'entries', 'kv')` | Encoding to use for Avro maps: - "mapping": plain JSON object ({"en":"Hello"}), decoded as a pl.Map - "entries": list of single-entry objects ([{"en":"Hello"}]) - "kv": list of {key,value} dicts ([{"key":"en","value":"Hello"}]), decoded as a list of {key, value} structs                                                                                                                                                    | `"mapping"`                                                                                                                                                                                      |
| `profile`                  | `bool`                         | Whether to show timing profile output                                                                                                                                                                                                                                                                                                                                                                                             | `False`                                                                                                                                                                                          |
| `map_threshold`            | `int`                          | Maximum number of keys before an object is treated as a map (unless overridden).                                                                                                                                                                                                                                                                                                                                                  | `20`                                                                                                                                                                                             |
| `map_max_required_keys`    | `int`                          | Maximum number of required keys allowed for Map inference during schema inference. Objects with more required keys will be forced to Record type. If None, no gating based on required key count.                                                                                                                                                                                                                                 | `None`                                                                                                                                                                                           |
| `unify_maps`               | `bool`                         | Enable unification of compatible but non-homogeneous record schemas into maps. When True, record schemas with compatible field types can be merged into a single map schema with selective nullable fields.                                                                                                                                                                                                                       | `False`                                                                                                                                                                                          |
| `no_unify`                 | \`set[str]                     | None\`                                                                                                                                                                                                                                                                                                                                                                                                                            | Prevent unification of keys under these field names with their sibling record fields.                                                                                                            |
| `force_field_types`        | `dict[str, str]`               | Override the inferred type for specific fields. Keys are field names, values must be either "map" or "record".                                                                                                                                                                                                                                                                                                                    | `None`                                                                                                                                                                                           |
| `force_parent_field_types` | `dict[str, str]`               | Override the inferred type for specific fields based on their parent field name. Keys are field names, values must be either "map" or "record".                                                                                                                                                                                                                                                                                   | `None`                                                                                                                                                                                           |
| `force_scalar_promotion`   | `set[str]`                     | Set of field names that should always be promoted to wrapped scalars, even when they appear as simple scalars. Ensures schema stability for fields known to have heterogeneous types across chunks. Example: {"precision", "datavalue"}.                                                                                                                                                                                          | `None`                                                                                                                                                                                           |
| `wrap_scalars`             | `bool`                         | Whether to promote scalar values into singleton objects when they appear in contexts where other rows provide objects, or scalars of another kind (an integer and a string, not an integer and a number). This keeps every value with its type, where the union would keep one kind. The promoted field name defaults to the parent key with a \_\_{type} suffix, e.g. a string under "value" becomes {"value\_\_string": "..."}. | `True`                                                                                                                                                                                           |
| `wrap_root`                | \`str                          | None\`                                                                                                                                                                                                                                                                                                                                                                                                                            | Wrap each JSON row under that key before normalisation. If None, leave rows unchanged.                                                                                                           |
| `no_root_map`              | `bool`                         | Prevent document root from becoming a map type, even if it meets map inference criteria                                                                                                                                                                                                                                                                                                                                           | `True`                                                                                                                                                                                           |
| `max_builders`             | `int`                          | Maximum number of schema builders to create in parallel at once. Lower values reduce peak memory usage during schema inference. If None, processes all strings at once. Default is None.                                                                                                                                                                                                                                          | `None`                                                                                                                                                                                           |
| `avro_schema`              | \`str                          | dict\`                                                                                                                                                                                                                                                                                                                                                                                                                            | An Avro schema (as JSON text or a dict) to normalise against in place of inferring one, for data whose schema is known: the inference pass is skipped, and the inference options have no effect. |

Returns:

| Type   | Description                                                                                                                |
| ------ | -------------------------------------------------------------------------------------------------------------------------- |
| `Expr` | An expression producing a new string column, where each row is a normalised JSON object matching the inferred Avro schema. |

Examples:

```pycon
>>> df = pl.DataFrame({
...     "json_data": [
...         '{"id": "1", "labels": {}}',
...         '{"id": 2, "labels": {"en": "Hello"}}',
...     ]
... })
>>> df.select(normalise_json(pl.col("json_data")))
shape: (2, 1)
┌──────────────────────────────────────┐
│ normalised                           │
│ ---                                  │
│ str                                  │
╞══════════════════════════════════════╡
│ {"id": "1", "labels": null}          │
│ {"id": "2", "labels": {"en":"Hello"}}│
└──────────────────────────────────────┘
```

Source code in `polars-genson-py/python/polars_genson/__init__.py`

```python
def normalise_json(
    expr: pl.Expr,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    empty_as_null: bool = True,
    coerce_strings: bool = False,
    map_encoding: Literal["entries", "mapping", "kv"] = "mapping",
    profile: bool = False,
    map_threshold: int = 20,
    map_max_required_keys: int | None = None,
    unify_maps: bool = False,
    no_unify: set[str] | None = None,
    force_field_types: dict[str, str] | None = None,
    force_parent_field_types: dict[str, str] | None = None,
    force_scalar_promotion: set[str] | None = None,
    wrap_scalars: bool = True,
    wrap_root: str | None = None,
    no_root_map: bool = True,
    max_builders: int | None = None,
    avro_schema: str | dict | None = None,
) -> pl.Expr:
    """Normalise a JSON string column against an inferred Avro schema.

    This performs schema inference once across all rows, then rewrites each row
    to conform to that schema. The output is a new column of JSON strings with
    consistent structure and datatypes.

    Parameters
    ----------
    expr : pl.Expr
        Expression representing a string column of JSON data.
    ignore_outer_array : bool, default True
        Treat a top-level JSON array as a stream of objects (like NDJSON).
    ndjson : bool, default False
        Treat input as newline-delimited JSON rather than a single JSON document.
    empty_as_null : bool, default True
        Convert empty arrays/maps into `null` to preserve row count when exploding.
        Disable with ``False`` to keep empty collections.
    coerce_strings : bool, default False
        If True, attempt to coerce string values into numeric/boolean types
        where the schema expects them. If False, unmatched strings become null.
    map_encoding : {"mapping", "entries", "kv"}, default "mapping"
        Encoding to use for Avro maps:
        - "mapping": plain JSON object ({"en":"Hello"}), decoded as a ``pl.Map``
        - "entries": list of single-entry objects ([{"en":"Hello"}])
        - "kv":      list of {key,value} dicts ([{"key":"en","value":"Hello"}]),
          decoded as a list of ``{key, value}`` structs
    profile : bool, default False
        Whether to show timing profile output
    map_threshold : int, default 20
        Maximum number of keys before an object is treated as a map
        (unless overridden).
    map_max_required_keys : int, optional
        Maximum number of required keys allowed for Map inference during schema
        inference. Objects with more required keys will be forced to Record type.
        If None, no gating based on required key count.
    unify_maps : bool, default False
        Enable unification of compatible but non-homogeneous record schemas into maps.
        When True, record schemas with compatible field types can be merged into a single
        map schema with selective nullable fields.
    no_unify: set[str] | None, default None
        Prevent unification of keys under these field names with their sibling record fields.
    force_field_types : dict[str, str], optional
        Override the inferred type for specific fields. Keys are field names,
        values must be either ``"map"`` or ``"record"``.
    force_parent_field_types : dict[str, str], optional
        Override the inferred type for specific fields based on their parent field name.
        Keys are field names, values must be either ``"map"`` or ``"record"``.
    force_scalar_promotion : set[str], optional
        Set of field names that should always be promoted to wrapped scalars,
        even when they appear as simple scalars. Ensures schema stability for
        fields known to have heterogeneous types across chunks.
        Example: ``{"precision", "datavalue"}``.
    wrap_scalars : bool, default True
        Whether to promote scalar values into singleton objects when they appear
        in contexts where other rows provide objects, or scalars of another kind
        (an integer and a string, not an integer and a number). This keeps every
        value with its type, where the union would keep one kind. The promoted field name defaults
        to the parent key with a ``__{type}`` suffix, e.g. a string under
        ``"value"`` becomes ``{"value__string": "..."}``.
    wrap_root : str | None, default None
        Wrap each JSON row under that key before normalisation.
        If ``None``, leave rows unchanged.
    no_root_map : bool, default True
        Prevent document root from becoming a map type, even if it meets map inference criteria
    max_builders : int, optional
        Maximum number of schema builders to create in parallel at once.
        Lower values reduce peak memory usage during schema inference.
        If None, processes all strings at once. Default is None.
    avro_schema : str | dict, optional
        An Avro schema (as JSON text or a dict) to normalise against in place of
        inferring one, for data whose schema is known: the inference pass is skipped,
        and the inference options have no effect.

    Returns
    -------
    pl.Expr
        An expression producing a new string column, where each row is a
        normalised JSON object matching the inferred Avro schema.

    Examples
    --------
    >>> df = pl.DataFrame({
    ...     "json_data": [
    ...         '{"id": "1", "labels": {}}',
    ...         '{"id": 2, "labels": {"en": "Hello"}}',
    ...     ]
    ... })
    >>> df.select(normalise_json(pl.col("json_data")))
    shape: (2, 1)
    ┌──────────────────────────────────────┐
    │ normalised                           │
    │ ---                                  │
    │ str                                  │
    ╞══════════════════════════════════════╡
    │ {"id": "1", "labels": null}          │
    │ {"id": "2", "labels": {"en":"Hello"}}│
    └──────────────────────────────────────┘
    """
    kwargs = _normalise_kwargs(
        ignore_outer_array=ignore_outer_array,
        ndjson=ndjson,
        empty_as_null=empty_as_null,
        coerce_strings=coerce_strings,
        map_encoding=map_encoding,
        profile=profile,
        map_threshold=map_threshold,
        map_max_required_keys=map_max_required_keys,
        unify_maps=unify_maps,
        no_unify=no_unify,
        force_field_types=force_field_types,
        force_parent_field_types=force_parent_field_types,
        force_scalar_promotion=force_scalar_promotion,
        wrap_scalars=wrap_scalars,
        wrap_root=wrap_root,
        no_root_map=no_root_map,
        max_builders=max_builders,
        avro_schema=avro_schema,
    )
    return plug(expr, changes_length=True, **kwargs)
```

## infer_from_parquet

```python
infer_from_parquet(
    input_path: str | Path,
    column: str,
    output_path: str | Path | None = None,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    schema_uri: str | None = "http://json-schema.org/schema#",
    debug: bool = False,
    profile: bool = False,
    verbosity: Literal["Normal", "Verbose"] = "Normal",
    map_threshold: int = 20,
    map_max_required_keys: int | None = None,
    unify_maps: bool = False,
    no_unify: set[str] | None = None,
    force_field_types: dict[str, str] | None = None,
    force_parent_field_types: dict[str, str] | None = None,
    force_scalar_promotion: set[str] | None = None,
    wrap_scalars: bool = True,
    avro: bool = False,
    wrap_root: str | None = None,
    no_root_map: bool = True,
    max_builders: int | None = None,
) -> str | dict
```

Infer JSON schema from a Parquet column.

Parameters:

| Name                       | Type             | Description                                                                                                                                                                                                                              | Default                                                                                         |
| -------------------------- | ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `input_path`               | \`str            | Path\`                                                                                                                                                                                                                                   | Path to input Parquet file                                                                      |
| `column`                   | `str`            | Name of column containing JSON strings                                                                                                                                                                                                   | *required*                                                                                      |
| `output_path`              | \`str            | Path\`                                                                                                                                                                                                                                   | Path to write schema JSON. If None, returns schema as dict                                      |
| `ignore_outer_array`       | `bool`           | Whether to treat top-level arrays as streams of objects                                                                                                                                                                                  | `True`                                                                                          |
| `ndjson`                   | `bool`           | Whether to treat input as newline-delimited JSON                                                                                                                                                                                         | `False`                                                                                         |
| `schema_uri`               | `str or None`    | Schema URI to use for the generated schema                                                                                                                                                                                               | `"http://json-schema.org/schema#"`                                                              |
| `debug`                    | `bool`           | Whether to print debug information                                                                                                                                                                                                       | `False`                                                                                         |
| `profile`                  | `bool`           | Whether to print profiling information                                                                                                                                                                                                   | `False`                                                                                         |
| `verbosity`                | `str`            | Whether to print verbose debug information                                                                                                                                                                                               | `"Normal"`                                                                                      |
| `map_threshold`            | `int`            | Number of keys above which a heterogeneous object may be rewritten as a map (unless overridden).                                                                                                                                         | `20`                                                                                            |
| `map_max_required_keys`    | `int`            | Maximum number of required keys allowed for Map inference. Objects with more required keys will be forced to Record type. If None, no gating based on required key count.                                                                | `None`                                                                                          |
| `unify_maps`               | `bool`           | Enable unification of compatible but non-homogeneous record schemas into maps. When True, record schemas with compatible field types can be merged into a single map schema with selective nullable fields.                              | `False`                                                                                         |
| `no_unify`                 | \`set[str]       | None\`                                                                                                                                                                                                                                   | Prevent unification of keys under these field names with their sibling record fields.           |
| `force_field_types`        | `dict[str, str]` | Explicit overrides for specific fields. Values must be "map" or "record". Example: {"labels": "map", "claims": "record"}.                                                                                                                | `None`                                                                                          |
| `force_parent_field_types` | `dict[str, str]` | Explicit overrides for fields based on their parent field name. Values must be "map" or "record". Example: {"labels": "map", "claims": "record"}.                                                                                        | `None`                                                                                          |
| `force_scalar_promotion`   | `set[str]`       | Set of field names that should always be promoted to wrapped scalars, even when they appear as simple scalars. Ensures schema stability for fields known to have heterogeneous types across chunks. Example: {"precision", "datavalue"}. | `None`                                                                                          |
| `wrap_scalars`             | `bool`           | Whether to promote scalar values into singleton objects when they appear in contexts where other rows provide objects, or scalars of another kind (an integer and a string, not an integer and a number).                                | `True`                                                                                          |
| `avro`                     | `bool`           | Whether to output an Avro schema instead of a JSON Schema.                                                                                                                                                                               | `False`                                                                                         |
| `wrap_root`                | \`str            | None\`                                                                                                                                                                                                                                   | If a string, wrap each JSON row under that key before inference. If None, leave rows unchanged. |
| `no_root_map`              | `bool`           | Prevent document root from becoming a map type, even if it meets map inference criteria                                                                                                                                                  | `True`                                                                                          |
| `max_builders`             | `int`            | Maximum number of schema builders to create in parallel at once. Lower values reduce peak memory usage during schema inference. If None, processes all strings at once. Default is None.                                                 | `None`                                                                                          |

Returns:

| Type  | Description |
| ----- | ----------- |
| \`str | dict\`      |

Examples:

```pycon
>>> # Infer schema and return as dict
>>> schema = infer_from_parquet("data.parquet", "claims")
>>> # Infer schema and write to file
>>> infer_from_parquet("data.parquet", "claims", "schema.json")
```

Source code in `polars-genson-py/python/polars_genson/__init__.py`

```python
def infer_from_parquet(
    input_path: str | Path,
    column: str,
    output_path: str | Path | None = None,
    *,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    schema_uri: str | None = "http://json-schema.org/schema#",
    debug: bool = False,
    profile: bool = False,
    verbosity: Literal["Normal", "Verbose"] = "Normal",
    map_threshold: int = 20,
    map_max_required_keys: int | None = None,
    unify_maps: bool = False,
    no_unify: set[str] | None = None,
    force_field_types: dict[str, str] | None = None,
    force_parent_field_types: dict[str, str] | None = None,
    force_scalar_promotion: set[str] | None = None,
    wrap_scalars: bool = True,
    avro: bool = False,
    wrap_root: str | None = None,
    no_root_map: bool = True,
    max_builders: int | None = None,
) -> str | dict:
    """Infer JSON schema from a Parquet column.

    Parameters
    ----------
    input_path : str | Path
        Path to input Parquet file
    column : str
        Name of column containing JSON strings
    output_path : str | Path, optional
        Path to write schema JSON. If None, returns schema as dict
    ignore_outer_array : bool, default True
        Whether to treat top-level arrays as streams of objects
    ndjson : bool, default False
        Whether to treat input as newline-delimited JSON
    schema_uri : str or None, default "http://json-schema.org/schema#"
        Schema URI to use for the generated schema
    debug : bool, default False
        Whether to print debug information
    profile : bool, default False
        Whether to print profiling information
    verbosity : str, default "Normal"
        Whether to print verbose debug information
    map_threshold : int, default 20
        Number of keys above which a heterogeneous object may be rewritten
        as a map (unless overridden).
    map_max_required_keys : int, optional
        Maximum number of required keys allowed for Map inference. Objects with more
        required keys will be forced to Record type. If None, no gating based on
        required key count.
    unify_maps : bool, default False
        Enable unification of compatible but non-homogeneous record schemas into maps.
        When True, record schemas with compatible field types can be merged into a single
        map schema with selective nullable fields.
    no_unify: set[str] | None, default None
        Prevent unification of keys under these field names with their sibling record fields.
    force_field_types : dict[str, str], optional
        Explicit overrides for specific fields. Values must be `"map"` or `"record"`.
        Example: ``{"labels": "map", "claims": "record"}``.
    force_parent_field_types : dict[str, str], optional
        Explicit overrides for fields based on their parent field name. Values must be `"map"` or `"record"`.
        Example: ``{"labels": "map", "claims": "record"}``.
    force_scalar_promotion : set[str], optional
        Set of field names that should always be promoted to wrapped scalars,
        even when they appear as simple scalars. Ensures schema stability for
        fields known to have heterogeneous types across chunks.
        Example: ``{"precision", "datavalue"}``.
    wrap_scalars : bool, default True
        Whether to promote scalar values into singleton objects when they appear
        in contexts where other rows provide objects, or scalars of another kind
        (an integer and a string, not an integer and a number).
    avro : bool, default False
        Whether to output an Avro schema instead of a JSON Schema.
    wrap_root : str | None, default None
        If a string, wrap each JSON row under that key before inference.
        If ``None``, leave rows unchanged.
    no_root_map : bool, default True
        Prevent document root from becoming a map type, even if it meets map inference criteria
    max_builders : int, optional
        Maximum number of schema builders to create in parallel at once.
        Lower values reduce peak memory usage during schema inference.
        If None, processes all strings at once. Default is None.

    Returns
    -------
    str | dict
        If output_path is given, returns success message.
        If output_path is None, returns schema as dict.

    Examples
    --------
    >>> # Infer schema and return as dict
    >>> schema = infer_from_parquet("data.parquet", "claims")
    >>> # Infer schema and write to file
    >>> infer_from_parquet("data.parquet", "claims", "schema.json")
    """
    result = _rust_infer_from_parquet(
        input_path=str(input_path),
        column=column,
        output_path=str(output_path) if output_path else None,
        ignore_outer_array=ignore_outer_array,
        ndjson=ndjson,
        schema_uri=schema_uri,
        debug=debug,
        profile=profile,
        verbosity=verbosity,
        map_threshold=map_threshold,
        map_max_required_keys=map_max_required_keys,
        unify_maps=unify_maps,
        no_unify=list(no_unify) if no_unify else None,
        force_scalar_promotion=(
            list(force_scalar_promotion) if force_scalar_promotion else []
        ),
        force_field_types=force_field_types,
        force_parent_field_types=force_parent_field_types,
        wrap_scalars=wrap_scalars,
        avro=avro,
        wrap_root=wrap_root,
        no_root_map=no_root_map,
        max_builders=max_builders,
    )

    if output_path:
        return result  # Success message
    else:
        return orjson.loads(result)  # Parse and return dict
```

## normalise_from_parquet

```python
normalise_from_parquet(
    input_path: str | Path,
    column: str,
    output_path: str | Path | None,
    *,
    output_column: str | None = None,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    empty_as_null: bool = True,
    coerce_strings: bool = False,
    map_encoding: Literal["entries", "mapping", "kv"] = "mapping",
    debug: bool = False,
    profile: bool = False,
    map_threshold: int = 20,
    map_max_required_keys: int | None = None,
    unify_maps: bool = False,
    no_unify: set[str] | None = None,
    force_field_types: dict[str, str] | None = None,
    force_parent_field_types: dict[str, str] | None = None,
    force_scalar_promotion: set[str] | None = None,
    wrap_scalars: bool = True,
    wrap_root: str | None = None,
    no_root_map: bool = True,
    max_builders: int | None = None,
    typed: bool = False,
    keep_columns: list[str] | None = None,
    extract_invariants: dict[str, str] | None = None,
    lookup_output_path: str | Path | None = None,
    prune: set[str] | None = None,
    prune_output_path: str | Path | None = None,
    schema: Schema | Struct | None = None,
    schema_check: Literal["compatible", "strict", "none"] = "compatible",
    unnest: bool = False,
    output_format: Literal["parquet", "arrow"] = "parquet",
    avro_schema: str | dict | None = None,
) -> Any
```

Normalise JSON data from a Parquet column and write back to Parquet.

Parameters:

| Name                       | Type                               | Description                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | Default                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| -------------------------- | ---------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `input_path`               | \`str                              | Path\`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | Path to input Parquet file                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `column`                   | `str`                              | Name of column containing JSON strings                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | *required*                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `output_path`              | \`str                              | Path                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | None\`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `output_column`            | `str`                              | Name for output column. Defaults to same as input column name                                                                                                                                                                                                                                                                                                                                                                                                                                                         | `None`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `ignore_outer_array`       | `bool`                             | Whether to treat a top-level JSON array as a stream of objects instead of a single array value.                                                                                                                                                                                                                                                                                                                                                                                                                       | `True`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `ndjson`                   | `bool`                             | Whether the input column contains newline-delimited JSON (NDJSON).                                                                                                                                                                                                                                                                                                                                                                                                                                                    | `False`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `empty_as_null`            | `bool`                             | If True, normalise empty arrays and empty maps to null. If False, preserve them as empty collections.                                                                                                                                                                                                                                                                                                                                                                                                                 | `True`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `coerce_strings`           | `bool`                             | If True, attempt to parse numeric/boolean values from strings (e.g. "42" → 42, "true" → true). If False, leave them as strings.                                                                                                                                                                                                                                                                                                                                                                                       | `False`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `map_encoding`             | `('mapping', 'entries', 'kv')`     | Encoding to use for Avro maps: - "mapping": plain JSON object ({"en":"Hello"}), decoded as a pl.Map - "entries": list of single-entry objects ([{"en":"Hello"}]) - "kv": list of {key,value} dicts ([{"key":"en","value":"Hello"}]), decoded as a list of {key, value} structs                                                                                                                                                                                                                                        | `"mapping"`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `debug`                    | `bool`                             | Whether to print debug information                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | `False`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `profile`                  | `bool`                             | Whether to display timing profile information                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | `False`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `map_threshold`            | `int`                              | Threshold above which objects with many varying keys are normalised as Avro maps instead of records.                                                                                                                                                                                                                                                                                                                                                                                                                  | `20`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `map_max_required_keys`    | `int`                              | Maximum number of required keys allowed for Map inference during schema inference. Objects with more required keys will be forced to Record type. If None, no gating based on required key count.                                                                                                                                                                                                                                                                                                                     | `None`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `unify_maps`               | `bool`                             | Enable unification of compatible but non-homogeneous record schemas into maps. When True, record schemas with compatible field types can be merged into a single map schema with selective nullable fields.                                                                                                                                                                                                                                                                                                           | `False`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `no_unify`                 | \`set[str]                         | None\`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | Prevent unification of keys under these field names with their sibling record fields.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `force_field_types`        | `dict[str, str]`                   | Per-field overrides for schema inference (e.g. {"labels": "map"}).                                                                                                                                                                                                                                                                                                                                                                                                                                                    | `None`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `force_parent_field_types` | `dict[str, str]`                   | Per-field overrides for schema inference based on their parent field name (e.g. {"labels": "map"}).                                                                                                                                                                                                                                                                                                                                                                                                                   | `None`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `force_scalar_promotion`   | `set[str]`                         | Set of field names that should always be promoted to wrapped scalars, even when they appear as simple scalars. Ensures schema stability for fields known to have heterogeneous types across chunks. Example: {"precision", "datavalue"}.                                                                                                                                                                                                                                                                              | `None`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `wrap_scalars`             | `bool`                             | Whether to promote scalar values into singleton objects when they appear in contexts where other rows provide objects, or scalars of another kind (an integer and a string, not an integer and a number).                                                                                                                                                                                                                                                                                                             | `True`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `wrap_root`                | \`str                              | None\`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | If a string, wrap each JSON row under that key before normalisation. If None, leave rows unchanged.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `no_root_map`              | `bool`                             | Prevent document root from becoming a map type, even if it meets map inference criteria                                                                                                                                                                                                                                                                                                                                                                                                                               | `True`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `max_builders`             | `int`                              | Maximum number of schema builders to create in parallel at once. Lower values reduce peak memory usage during schema inference. If None, processes all strings at once. Default is None.                                                                                                                                                                                                                                                                                                                              | `None`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `typed`                    | `bool`                             | If True, write the normalised rows as a typed struct column (the dtype avro_to_polars_schema gives for the inferred schema) instead of JSON strings, so no str.json_decode is needed after reading it back. Requires map_encoding="mapping", which writes maps as Parquet maps, or "kv", which writes them as lists of {key, value} structs.                                                                                                                                                                          | `False`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `keep_columns`             | `list[str]`                        | Input columns (e.g. an id) to copy unchanged into the output file, before the normalised column. The output has one row per input row, with a null normalised value for each null input row.                                                                                                                                                                                                                                                                                                                          | `None`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `extract_invariants`       | `dict[str, str]`                   | Fields that are invariant per key, each mapped to its determinant: the sibling field whose value determines it, e.g. {"labels": "id"} when every object with the same id carries the same labels. Wherever an object holds both, the field is removed before inference, so the output schema and rows do not contain it, and each distinct value is written once to lookup_output_path. A field that is not invariant (one determinant value with two different values) is an error. Requires lookup_output_path.     | `None`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `lookup_output_path`       | \`str                              | Path\`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | Parquet file for the lookup table, with string columns field, key and value (the subtree as JSON), in first-seen order.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `prune`                    | `set[str]`                         | Record fields, at any depth, whose records are pruned: wherever a record holds a non-null value for one, the record is removed, and each removed value is written to prune_output_path. A scalar promoted to a record goes by its promoted field (e.g. calibration\_\_string). A pruned record that is a field of another record takes that record with it, up to an array element or map entry; an array or map left empty is removed too. The fields are left out of the output schema. Requires prune_output_path. | `None`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `prune_output_path`        | \`str                              | Path\`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | Parquet file for the pruned values, one row each: the keep_columns of its row, path (the keys and indices from the row's root, as a JSON array) and value (the value as it was in the input, as JSON).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `schema`                   | \`Schema                           | Struct\`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | A target schema for the output column (the fields of its struct), for output that has the same schema whatever the rows held, such as chunks of one dataset normalised one at a time. The rows are normalised against the inferred schema, which is checked against the target (see schema_check), and written in the target's shape: its fields in its order, those no row had as nulls, at any depth. The inferred schema is still written to the file's metadata (genson_avro_schema). Requires typed=True.                                                                                                                                                                                                                |
| `schema_check`             | `('compatible', 'strict', 'none')` | How the inferred schema is checked against schema before anything is written (see check_schema_compatibility). A schema that fails raises a ValueError naming each offending field, and no output, lookup or pruned file is written. "none" writes the target's fields whatever the rows held: fields it lacks are dropped, and other types cast.                                                                                                                                                                     | `"compatible"`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `unnest`                   | `bool`                             | Write the output struct's fields as top-level columns (after keep_columns) in place of the one struct column, as DataFrame.unnest would. Requires typed=True.                                                                                                                                                                                                                                                                                                                                                         | `False`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `output_format`            | `('parquet', 'arrow')`             | "arrow" writes no output file and returns the output's record batches as an Arrow C stream (the Arrow PyCapsule interface, __arrow_c_stream__), read once with pyarrow.RecordBatchReader.from_stream or pyarrow.table. Its buffers are moved to the reader, not copied, for a caller that transforms the arrays before writing them itself. Requires typed=True and output_path=None.                                                                                                                                 | `"parquet"`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `avro_schema`              | \`str                              | dict\`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | An Avro schema (JSON text or a dict) expected to hold the rows, such as one inferred before from data of the same shape with the same options. Where every row fits it strictly (its own inferred schema adds nothing to it: no member it lacks, no value of a type its node does not hold, no union of two or more types in it) and it fits schema, the rows are decoded against it and nothing is inferred. Otherwise they are normalised as without it, against the schema inferred from them, checked as schema_check says. It changes how fast the rows are normalised, not what they are normalised to, but for the genson_avro_schema metadata, which is the schema they were normalised against. Requires typed=True. |

Returns:

| Type                                                                  | Description |
| --------------------------------------------------------------------- | ----------- |
| ``` The Arrow C stream with ``output_format="arrow"``, else None. ``` |             |

Examples:

```pycon
>>> # Normalize and write to new file
>>> normalise_from_parquet(
...     "input.parquet",
...     "claims",
...     "output.parquet",
...     map_threshold=0,
...     unify_maps=True
... )
>>> # In-place normalization (overwrites source)
>>> normalise_from_parquet(
...     "data.parquet",
...     "claims",
...     "data.parquet"
... )
```

Source code in `polars-genson-py/python/polars_genson/__init__.py`

```python
def normalise_from_parquet(
    input_path: str | Path,
    column: str,
    output_path: str | Path | None,
    *,
    output_column: str | None = None,
    ignore_outer_array: bool = True,
    ndjson: bool = False,
    empty_as_null: bool = True,
    coerce_strings: bool = False,
    map_encoding: Literal["entries", "mapping", "kv"] = "mapping",
    debug: bool = False,
    profile: bool = False,
    map_threshold: int = 20,
    map_max_required_keys: int | None = None,
    unify_maps: bool = False,
    no_unify: set[str] | None = None,
    force_field_types: dict[str, str] | None = None,
    force_parent_field_types: dict[str, str] | None = None,
    force_scalar_promotion: set[str] | None = None,
    wrap_scalars: bool = True,
    wrap_root: str | None = None,
    no_root_map: bool = True,
    max_builders: int | None = None,
    typed: bool = False,
    keep_columns: list[str] | None = None,
    extract_invariants: dict[str, str] | None = None,
    lookup_output_path: str | Path | None = None,
    prune: set[str] | None = None,
    prune_output_path: str | Path | None = None,
    schema: pl.Schema | pl.Struct | None = None,
    schema_check: Literal["compatible", "strict", "none"] = "compatible",
    unnest: bool = False,
    output_format: Literal["parquet", "arrow"] = "parquet",
    avro_schema: str | dict | None = None,
) -> Any:
    """Normalise JSON data from a Parquet column and write back to Parquet.

    Parameters
    ----------
    input_path : str | Path
        Path to input Parquet file
    column : str
        Name of column containing JSON strings
    output_path : str | Path | None
        Path to write normalised Parquet file (can be same as input_path for in-place).
        None with ``output_format="arrow"``, which writes no file
    output_column : str, optional
        Name for output column. Defaults to same as input column name
    ignore_outer_array : bool, default True
        Whether to treat a top-level JSON array as a stream of objects instead
        of a single array value.
    ndjson : bool, default False
        Whether the input column contains newline-delimited JSON (NDJSON).
    empty_as_null : bool, default True
        If True, normalise empty arrays and empty maps to ``null``.
        If False, preserve them as empty collections.
    coerce_strings : bool, default False
        If True, attempt to parse numeric/boolean values from strings
        (e.g. ``"42" → 42``, ``"true" → true``). If False, leave them as strings.
    map_encoding : {"mapping", "entries", "kv"}, default "mapping"
        Encoding to use for Avro maps:
        - "mapping": plain JSON object ({"en":"Hello"}), decoded as a ``pl.Map``
        - "entries": list of single-entry objects ([{"en":"Hello"}])
        - "kv":      list of {key,value} dicts ([{"key":"en","value":"Hello"}]),
          decoded as a list of ``{key, value}`` structs
    debug : bool, default False
        Whether to print debug information
    profile : bool, default False
        Whether to display timing profile information
    map_threshold : int, default 20
        Threshold above which objects with many varying keys are normalised
        as Avro maps instead of records.
    map_max_required_keys : int, optional
        Maximum number of required keys allowed for Map inference during schema
        inference. Objects with more required keys will be forced to Record type.
        If None, no gating based on required key count.
    unify_maps : bool, default False
        Enable unification of compatible but non-homogeneous record schemas into maps.
        When True, record schemas with compatible field types can be merged into a single
        map schema with selective nullable fields.
    no_unify: set[str] | None, default None
        Prevent unification of keys under these field names with their sibling record fields.
    force_field_types : dict[str, str], optional
        Per-field overrides for schema inference (e.g. ``{"labels": "map"}``).
    force_parent_field_types : dict[str, str], optional
        Per-field overrides for schema inference based on their parent field name (e.g. ``{"labels": "map"}``).
    force_scalar_promotion : set[str], optional
        Set of field names that should always be promoted to wrapped scalars,
        even when they appear as simple scalars. Ensures schema stability for
        fields known to have heterogeneous types across chunks.
        Example: ``{"precision", "datavalue"}``.
    wrap_scalars : bool, default True
        Whether to promote scalar values into singleton objects when they appear
        in contexts where other rows provide objects, or scalars of another kind
        (an integer and a string, not an integer and a number).
    wrap_root : str | None, default None
        If a string, wrap each JSON row under that key before normalisation.
        If ``None``, leave rows unchanged.
    no_root_map : bool, default True
        Prevent document root from becoming a map type, even if it meets map inference criteria
    max_builders : int, optional
        Maximum number of schema builders to create in parallel at once.
        Lower values reduce peak memory usage during schema inference.
        If None, processes all strings at once. Default is None.
    typed : bool, default False
        If True, write the normalised rows as a typed struct column (the dtype
        ``avro_to_polars_schema`` gives for the inferred schema) instead of JSON
        strings, so no ``str.json_decode`` is needed after reading it back.
        Requires ``map_encoding="mapping"``, which writes maps as Parquet maps, or
        ``"kv"``, which writes them as lists of ``{key, value}`` structs.
    keep_columns : list[str], optional
        Input columns (e.g. an ``id``) to copy unchanged into the output file,
        before the normalised column. The output has one row per input row, with
        a null normalised value for each null input row.
    extract_invariants : dict[str, str], optional
        Fields that are invariant per key, each mapped to its determinant: the sibling
        field whose value determines it, e.g. ``{"labels": "id"}`` when every object with
        the same ``id`` carries the same ``labels``. Wherever an object holds both, the
        field is removed before inference, so the output schema and rows do not contain
        it, and each distinct value is written once to ``lookup_output_path``. A field
        that is not invariant (one determinant value with two different values) is an
        error. Requires ``lookup_output_path``.
    lookup_output_path : str | Path, optional
        Parquet file for the lookup table, with string columns ``field``, ``key`` and
        ``value`` (the subtree as JSON), in first-seen order.
    prune : set[str], optional
        Record fields, at any depth, whose records are pruned: wherever a record holds a
        non-null value for one, the record is removed, and each removed value is written
        to ``prune_output_path``. A scalar promoted to a record goes by its promoted
        field (e.g. ``calibration__string``). A pruned record that is a field of another
        record takes that record with it, up to an array element or map entry; an array
        or map left empty is removed too. The fields are left out of the output schema.
        Requires ``prune_output_path``.
    prune_output_path : str | Path, optional
        Parquet file for the pruned values, one row each: the ``keep_columns`` of its
        row, ``path`` (the keys and indices from the row's root, as a JSON array) and
        ``value`` (the value as it was in the input, as JSON).
    schema : pl.Schema | pl.Struct, optional
        A target schema for the output column (the fields of its struct), for output
        that has the same schema whatever the rows held, such as chunks of one dataset
        normalised one at a time. The rows are normalised against the inferred schema,
        which is checked against the target (see `schema_check`), and written in the
        target's shape: its fields in its order, those no row had as nulls, at any
        depth. The inferred schema is still written to the file's metadata
        (``genson_avro_schema``). Requires ``typed=True``.
    schema_check : {"compatible", "strict", "none"}, default "compatible"
        How the inferred schema is checked against `schema` before anything is written
        (see `check_schema_compatibility`). A schema that fails raises a ``ValueError``
        naming each offending field, and no output, lookup or pruned file is written.
        "none" writes the target's fields whatever the rows held: fields it lacks are
        dropped, and other types cast.
    unnest : bool, default False
        Write the output struct's fields as top-level columns (after `keep_columns`) in
        place of the one struct column, as ``DataFrame.unnest`` would. Requires
        ``typed=True``.
    output_format : {"parquet", "arrow"}, default "parquet"
        "arrow" writes no output file and returns the output's record batches as an
        Arrow C stream (the Arrow PyCapsule interface, ``__arrow_c_stream__``), read
        once with ``pyarrow.RecordBatchReader.from_stream`` or ``pyarrow.table``. Its
        buffers are moved to the reader, not copied, for a caller that transforms the
        arrays before writing them itself. Requires ``typed=True`` and
        ``output_path=None``.
    avro_schema : str | dict, optional
        An Avro schema (JSON text or a dict) expected to hold the rows, such as one
        inferred before from data of the same shape with the same options. Where every
        row fits it strictly (its own inferred schema adds nothing to it: no member it
        lacks, no value of a type its node does not hold, no union of two or more types
        in it) and it fits ``schema``, the rows are decoded against it and nothing is
        inferred. Otherwise they are normalised as without it, against the schema
        inferred from them, checked as ``schema_check`` says. It changes how fast the
        rows are normalised, not what they are normalised to, but for the
        ``genson_avro_schema`` metadata, which is the schema they were normalised
        against. Requires ``typed=True``.

    Returns
    -------
    The Arrow C stream with ``output_format="arrow"``, else None.

    Examples
    --------
    >>> # Normalize and write to new file
    >>> normalise_from_parquet(
    ...     "input.parquet",
    ...     "claims",
    ...     "output.parquet",
    ...     map_threshold=0,
    ...     unify_maps=True
    ... )
    >>> # In-place normalization (overwrites source)
    >>> normalise_from_parquet(
    ...     "data.parquet",
    ...     "claims",
    ...     "data.parquet"
    ... )
    """
    if (output_format == "arrow") != (output_path is None):
        raise ValueError(
            'output_path must be None with output_format="arrow", and a path otherwise'
        )
    return _rust_normalise_from_parquet(
        input_path=str(input_path),
        column=column,
        output_path="" if output_path is None else str(output_path),
        output_column=output_column,
        ignore_outer_array=ignore_outer_array,
        ndjson=ndjson,
        empty_as_null=empty_as_null,
        coerce_strings=coerce_strings,
        map_encoding=map_encoding,
        debug=debug,
        profile=profile,
        map_threshold=map_threshold,
        map_max_required_keys=map_max_required_keys,
        unify_maps=unify_maps,
        no_unify=list(no_unify) if no_unify else None,
        force_field_types=force_field_types,
        force_parent_field_types=force_parent_field_types,
        force_scalar_promotion=(
            list(force_scalar_promotion) if force_scalar_promotion else []
        ),
        wrap_scalars=wrap_scalars,
        wrap_root=wrap_root,
        no_root_map=no_root_map,
        max_builders=max_builders,
        typed=typed,
        keep_columns=keep_columns,
        extract_invariants=(
            list(extract_invariants.items()) if extract_invariants else None
        ),
        lookup_output_path=str(lookup_output_path) if lookup_output_path else None,
        prune=list(prune) if prune else None,
        prune_output_path=str(prune_output_path) if prune_output_path else None,
        schema=None if schema is None else orjson.dumps(dtype_spec(schema)).decode(),
        schema_check=schema_check,
        unnest=unnest,
        output_format=output_format,
        avro_schema=(
            avro_schema
            if avro_schema is None or isinstance(avro_schema, str)
            else orjson.dumps(avro_schema).decode()
        ),
    )
```

## check_schema_compatibility

```python
check_schema_compatibility(
    inferred: Schema | Struct,
    target: Schema | Struct,
    schema_check: Literal["compatible", "strict"] = "compatible",
) -> list[str]
```

Report where an inferred schema does not fit a target schema.

One line per offending field: its path (`a.b[].c`, `[]` for list items, `{}` for map values) and why. An empty list means it fits.

Under "compatible", `inferred` fits `target` when, recursively, it is `Null` (no value was seen), which fits any dtype; it is the same dtype, with no widening (an `Int64` does not fit a `Float64`); or both are structs, lists, arrays of one size or maps whose parts fit, a struct having only fields the target has (the target may have more, written as nulls by `normalise_from_parquet(schema=...)`). A target field typed `Null` takes only `Null`. Under "strict", `inferred` must be `target`, field for field.

Source code in `polars-genson-py/python/polars_genson/__init__.py`

```python
def check_schema_compatibility(
    inferred: pl.Schema | pl.Struct,
    target: pl.Schema | pl.Struct,
    schema_check: Literal["compatible", "strict"] = "compatible",
) -> list[str]:
    """Report where an inferred schema does not fit a target schema.

    One line per offending field: its path (``a.b[].c``, ``[]`` for list items, ``{}``
    for map values) and why. An empty list means it fits.

    Under "compatible", `inferred` fits `target` when, recursively, it is ``Null`` (no
    value was seen), which fits any dtype; it is the same dtype, with no widening (an
    ``Int64`` does not fit a ``Float64``); or both are structs, lists, arrays of one size
    or maps whose parts fit, a struct having only fields the target has (the target may
    have more, written as nulls by ``normalise_from_parquet(schema=...)``). A target
    field typed ``Null`` takes only ``Null``. Under "strict", `inferred` must be `target`,
    field for field.
    """
    return _rust_schema_incompatibilities(
        orjson.dumps(dtype_spec(inferred)).decode(),
        orjson.dumps(dtype_spec(target)).decode(),
        schema_check,
    )
```

## read_parquet_metadata

```python
read_parquet_metadata(path: str | Path) -> dict[str, str]
```

Read metadata from a Parquet file.

Parameters:

| Name   | Type  | Description              | Default    |
| ------ | ----- | ------------------------ | ---------- |
| `path` | `str` | Path to the Parquet file | *required* |

Returns:

| Type             | Description                            |
| ---------------- | -------------------------------------- |
| `dict[str, str]` | Dictionary of metadata key-value pairs |

Source code in `polars-genson-py/python/polars_genson/__init__.py`

```python
def read_parquet_metadata(path: str | Path) -> dict[str, str]:
    """Read metadata from a Parquet file.

    Parameters
    ----------
    path : str
        Path to the Parquet file

    Returns
    -------
    dict[str, str]
        Dictionary of metadata key-value pairs
    """
    return _rust_read_parquet_metadata(str(path))
```

## write_parquet

```python
write_parquet(
    data: Any, path: str | Path, *, row_group_size: int | None = None
) -> None
```

Write Arrow data to a Parquet file with genson's Parquet writer.

The writer genson writes its own output with, for any Arrow data: its row groups' leaf columns are encoded in parallel, so a deeply nested table (lists of structs of lists) writes in a fraction of the time of a writer that encodes them one at a time. The data is taken through the Arrow PyCapsule interface, its buffers not copied, and the GIL is released while it is encoded.

The file holds the same data in the same Arrow schema as `pyarrow.parquet` writes it: list and map groups take the Parquet specification's names (`element`, `key_value`), as pyarrow names them, and the Arrow schema is stored in the file's metadata. It is compressed with zstd (level 1). Each leaf column is dictionary encoded but one whose first values (up to 16,384, at least 128) are more than three quarters distinct, for which a dictionary costs a hash per value and grows the file.

Parameters:

| Name             | Type  | Description                                                                                                                                                                                                            | Default                                                          |
| ---------------- | ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| `data`           | `Any` | Arrow data with an __arrow_c_stream__ method: a pyarrow.Table or pyarrow.RecordBatchReader, a Polars DataFrame, or the stream normalise_from_parquet(..., output_format="arrow") returns. A stream is read to its end. | *required*                                                       |
| `path`           | \`str | Path\`                                                                                                                                                                                                                 | Path to write the Parquet file to. An existing file is replaced. |
| `row_group_size` | `int` | Rows to a row group. Defaults to 1,048,576 (arrow-rs's default, and pyarrow's).                                                                                                                                        | `None`                                                           |

Raises:

| Type        | Description                               |
| ----------- | ----------------------------------------- |
| `TypeError` | If data has no __arrow_c_stream__ method. |
| `OSError`   | If the file cannot be written.            |

Source code in `polars-genson-py/python/polars_genson/__init__.py`

```python
def write_parquet(data: Any, path: str | Path, *, row_group_size: int | None = None) -> None:
    """Write Arrow data to a Parquet file with genson's Parquet writer.

    The writer genson writes its own output with, for any Arrow data: its row groups'
    leaf columns are encoded in parallel, so a deeply nested table (lists of structs of
    lists) writes in a fraction of the time of a writer that encodes them one at a time.
    The data is taken through the Arrow PyCapsule interface, its buffers not copied, and
    the GIL is released while it is encoded.

    The file holds the same data in the same Arrow schema as ``pyarrow.parquet`` writes
    it: list and map groups take the Parquet specification's names (``element``,
    ``key_value``), as pyarrow names them, and the Arrow schema is stored in the file's
    metadata. It is compressed with zstd (level 1). Each leaf column is dictionary
    encoded but one whose first values (up to 16,384, at least 128) are more than three
    quarters distinct, for which a dictionary costs a hash per value and grows the file.

    Parameters
    ----------
    data : Any
        Arrow data with an ``__arrow_c_stream__`` method: a ``pyarrow.Table`` or
        ``pyarrow.RecordBatchReader``, a Polars ``DataFrame``, or the stream
        ``normalise_from_parquet(..., output_format="arrow")`` returns. A stream is read
        to its end.
    path : str | Path
        Path to write the Parquet file to. An existing file is replaced.
    row_group_size : int, optional
        Rows to a row group. Defaults to 1,048,576 (arrow-rs's default, and pyarrow's).

    Raises
    ------
    TypeError
        If ``data`` has no ``__arrow_c_stream__`` method.
    OSError
        If the file cannot be written.
    """
    if row_group_size is not None and row_group_size < 1:
        raise ValueError(f"row_group_size must be at least 1, not {row_group_size}")
    _rust_write_parquet(data, str(path), row_group_size)
```

## avro_to_polars_schema

```python
avro_to_polars_schema(
    avro_schema_json: str,
    debug: bool = False,
    *,
    map_encoding: Literal["entries", "mapping", "kv"] = "mapping",
) -> pl.Schema
```

Convert an Avro schema to a Polars Schema.

Parameters:

| Name               | Type                           | Description                                                                                                                                                                                                                                   | Default     |
| ------------------ | ------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------- |
| `avro_schema_json` | `str`                          | JSON string containing Avro schema                                                                                                                                                                                                            | *required*  |
| `debug`            | `bool`                         | Whether to print debug information                                                                                                                                                                                                            | `False`     |
| `map_encoding`     | `('mapping', 'entries', 'kv')` | The map_encoding the data is normalised with: each Avro map gets the dtype that encoding decodes to, a pl.Map for "mapping" and a list of {key, value} structs for "kv". "entries" has none, and raises ValueError if the schema holds a map. | `"mapping"` |

Returns:

| Type     | Description                  |
| -------- | ---------------------------- |
| `Schema` | Polars schema representation |

Source code in `polars-genson-py/python/polars_genson/__init__.py`

```python
def avro_to_polars_schema(
    avro_schema_json: str,
    debug: bool = False,
    *,
    map_encoding: Literal["entries", "mapping", "kv"] = "mapping",
) -> pl.Schema:
    """Convert an Avro schema to a Polars Schema.

    Parameters
    ----------
    avro_schema_json : str
        JSON string containing Avro schema
    debug : bool, default False
        Whether to print debug information
    map_encoding : {"mapping", "entries", "kv"}, default "mapping"
        The ``map_encoding`` the data is normalised with: each Avro map gets the dtype
        that encoding decodes to, a ``pl.Map`` for "mapping" and a list of
        ``{key, value}`` structs for "kv". "entries" has none, and raises
        ``ValueError`` if the schema holds a map.

    Returns
    -------
    pl.Schema
        Polars schema representation
    """
    # Get field name/type string pairs from Rust
    fields = _rust_avro_to_polars_fields(avro_schema_json, debug)

    # Convert type strings to actual DataType objects
    return pl.Schema(
        {
            name: _parse_polars_dtype(dtype_str, map_encoding)
            for name, dtype_str in fields
        }
    )
```

## schema_to_dict

```python
schema_to_dict(schema: Schema)
```

Convert a Polars Schema into a nested Python dict.

Source code in `polars-genson-py/python/polars_genson/__init__.py`

```python
def schema_to_dict(schema: pl.Schema):
    """Convert a Polars Schema into a nested Python dict."""
    if not isinstance(schema, pl.Schema):
        raise TypeError(f"Expected Polars Schema, got {type(schema)}")
    return {name: _dtype_to_dict(dtype) for name, dtype in schema.items()}
```

## dtypes

Dtype parsing from concise string format used to serialise across Rust to Python.

### contains_map

```python
contains_map(dtype: DataType) -> bool
```

Whether a dtype is, or holds, a `pl.Map`.

Source code in `polars-genson-py/python/polars_genson/dtypes.py`

```python
def contains_map(dtype: pl.DataType) -> bool:
    """Whether a dtype is, or holds, a `pl.Map`."""
    if isinstance(dtype, pl.Map):
        return True
    if isinstance(dtype, pl.Struct):
        return any(contains_map(field.dtype) for field in dtype.fields)
    if isinstance(dtype, (pl.List, pl.Array)):
        return contains_map(dtype.inner)
    return False
```

### project_dtype

```python
project_dtype(expr: Expr, have: DataType, want: DataType) -> pl.Expr | None
```

`expr`, of dtype `have`, as `str.json_decode` decodes the same values to `want`.

`want`'s struct fields are taken by name (a field `have` lacks is null, one `want` lacks is dropped) and lists are projected element by element. `None` where the result could differ from `json_decode`'s: a leaf, map or array dtype other than `have`'s (where a cast and `json_decode`'s parse can disagree), or a nested `want` where `have` holds only nulls. Structs are built field by field, never cast, so no cast drops a struct field.

Source code in `polars-genson-py/python/polars_genson/dtypes.py`

```python
def project_dtype(
    expr: pl.Expr, have: pl.DataType, want: pl.DataType
) -> pl.Expr | None:
    """`expr`, of dtype `have`, as `str.json_decode` decodes the same values to `want`.

    `want`'s struct fields are taken by name (a field `have` lacks is null, one `want`
    lacks is dropped) and lists are projected element by element. ``None`` where the
    result could differ from `json_decode`'s: a leaf, map or array dtype other than
    `have`'s (where a cast and `json_decode`'s parse can disagree), or a nested `want`
    where `have` holds only nulls. Structs are built field by field, never cast, so
    no cast drops a struct field.
    """
    if have == want:
        return expr
    if isinstance(have, pl.Null):
        # Only nulls were seen there, and json_decode makes a null of any leaf dtype
        nested = (pl.Struct, pl.List, pl.Array, pl.Map)
        return None if isinstance(want, nested) else expr.cast(want)
    if isinstance(have, pl.Struct) and isinstance(want, pl.Struct):
        fields = {field.name: field.dtype for field in have.fields}
        parts = []
        for field in want.fields:
            if field.name in fields:
                part = project_dtype(
                    expr.struct.field(field.name), fields[field.name], field.dtype
                )
                if part is None:
                    return None
            else:
                # json_decode gives a field the JSON lacks as null (a null struct, not
                # a struct of nulls)
                part = pl.lit(None, dtype=field.dtype)
            parts.append(part.alias(field.name))
        # A null struct stays null, not a struct of null fields
        return pl.when(expr.is_not_null()).then(pl.struct(parts))
    if isinstance(have, pl.List) and isinstance(want, pl.List):
        inner = project_dtype(pl.element(), have.inner, want.inner)
        return None if inner is None else expr.list.eval(inner)
    return None
```

### typed_route_makes

```python
typed_route_makes(dtype: DataType) -> bool
```

Whether `dtype` holds only dtypes the typed route can decode to.

Those are the ones `avro_to_polars_schema` gives an Avro schema: String, Int64, Float64, Boolean and Null leaves, in lists, structs and maps keyed by String. A dtype holding any other (Categorical, Int32, Datetime, Decimal, Array, ...) never projects, so it is decoded from JSON text without running the typed route first.

Source code in `polars-genson-py/python/polars_genson/dtypes.py`

```python
def typed_route_makes(dtype: pl.DataType) -> bool:
    """Whether `dtype` holds only dtypes the typed route can decode to.

    Those are the ones ``avro_to_polars_schema`` gives an Avro schema: String, Int64,
    Float64, Boolean and Null leaves, in lists, structs and maps keyed by String. A dtype
    holding any other (Categorical, Int32, Datetime, Decimal, Array, ...) never projects,
    so it is decoded from JSON text without running the typed route first.
    """
    # A field's dtype may be the class itself (pl.String) or an instance
    if any(
        dtype == leaf for leaf in (pl.String, pl.Int64, pl.Float64, pl.Boolean, pl.Null)
    ):
        return True
    if isinstance(dtype, pl.List):
        return typed_route_makes(dtype.inner)
    if isinstance(dtype, pl.Struct):
        return all(typed_route_makes(field.dtype) for field in dtype.fields)
    if isinstance(dtype, pl.Map):
        return dtype.key == pl.String and typed_route_makes(dtype.value)
    return False
```

### dtype_spec

```python
dtype_spec(dtype: DataType | Schema) -> object
```

Describe a dtype (or a schema, as a struct of its columns) for a target schema.

The JSON-ready description the Rust side reads (genson_core::conform:: data_type_from_spec): a dtype name such as `"String"`, or `{"Struct": [[name, dtype], ...]}`, `{"List": dtype}`, `{"Array": [dtype, size]}` or `{"Map": [key, value]}`.

Source code in `polars-genson-py/python/polars_genson/dtypes.py`

```python
def dtype_spec(dtype: pl.DataType | pl.Schema) -> object:
    """Describe a dtype (or a schema, as a struct of its columns) for a target schema.

    The JSON-ready description the Rust side reads (genson_core::conform::
    data_type_from_spec): a dtype name such as ``"String"``, or ``{"Struct": [[name,
    dtype], ...]}``, ``{"List": dtype}``, ``{"Array": [dtype, size]}`` or ``{"Map":
    [key, value]}``.
    """
    if isinstance(dtype, pl.Schema):
        dtype = pl.Struct(dtype)
    if isinstance(dtype, pl.Struct):
        return {"Struct": [[f.name, dtype_spec(f.dtype)] for f in dtype.fields]}
    if isinstance(dtype, pl.List):
        return {"List": dtype_spec(dtype.inner)}
    if isinstance(dtype, pl.Array):
        return {"Array": [dtype_spec(dtype.inner), dtype.size]}
    if isinstance(dtype, pl.Map):
        return {"Map": [dtype_spec(dtype.key), dtype_spec(dtype.value)]}
    return str(dtype.base_type() if not isinstance(dtype, type) else dtype)
```

## polars

Extend polars DataFrame with genson namespace.

## utils

Utility functions for polars-genson plugin.

### parse_into_expr

```python
parse_into_expr(
    expr: IntoExpr,
    *,
    str_as_lit: bool = False,
    list_as_lit: bool = True,
    dtype: PolarsDataType | None = None,
) -> pl.Expr
```

Convert the user input into a polars.Expr.

- If `expr` is already an `pl.Expr`, we return it as-is.
- If `expr` is a string and `str_as_lit=False`, interpret as `pl.col(expr)`.
- Otherwise, treat it as a literal (possibly typed by `dtype`).

Source code in `polars-genson-py/python/polars_genson/utils.py`

```python
def parse_into_expr(
    expr: IntoExpr,
    *,
    str_as_lit: bool = False,
    list_as_lit: bool = True,
    dtype: PolarsDataType | None = None,
) -> pl.Expr:
    """Convert the user input into a polars.Expr.

    - If `expr` is already an `pl.Expr`, we return it as-is.
    - If `expr` is a string and `str_as_lit=False`, interpret as `pl.col(expr)`.
    - Otherwise, treat it as a literal (possibly typed by `dtype`).
    """
    if isinstance(expr, pl.Expr):
        return expr
    elif isinstance(expr, str) and not str_as_lit:
        return pl.col(expr)
    elif isinstance(expr, list) and not list_as_lit:
        return pl.lit(pl.Series(expr), dtype=dtype)
    else:
        return pl.lit(expr, dtype=dtype)
```

### parse_version

```python
parse_version(version: Sequence[str | int]) -> tuple[int, ...]
```

Split a version string like "0.20.16" into a tuple of ints, e.g. (0, 20, 16).

Source code in `polars-genson-py/python/polars_genson/utils.py`

```python
def parse_version(version: Sequence[str | int]) -> tuple[int, ...]:
    """Split a version string like "0.20.16" into a tuple of ints, e.g. (0, 20, 16)."""
    if isinstance(version, str):
        version = version.split(".")
    return tuple(int(re.sub(r"\D", "", str(v))) for v in version)
```
