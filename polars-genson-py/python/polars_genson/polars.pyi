"""Extend polars DataFrame with genson namespace."""

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import polars as pl
    from . import GensonNamespace

    # This extends DataFrame with the genson attribute
    class DataFrameNamespaceExtension:
        genson: "GensonNamespace"

    # Monkey patch DataFrame for type checking
    class DataFrame(pl.DataFrame, DataFrameNamespaceExtension):
        pass

    # Replace DataFrame in polars module
    pl.DataFrame = DataFrame  # type: ignore[assignment]
