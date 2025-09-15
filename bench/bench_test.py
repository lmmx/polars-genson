# polars-genson-py/tests/bench/bench_test.py
#
# Benchmarks for polars-genson using pytest-benchmark.
# Runs on the full dataset:
# - Normalisation (with/without decode)
# - Schema inference (Avro, JSON)

from pathlib import Path

import polars as pl
import polars_genson  # noqa: F401
import pytest

LABELS = Path(__file__).parent / "data" / "labels.parquet"
DF_LABELS = pl.read_parquet(LABELS)

RECORD_SCHEMA = pl.Struct({"language": pl.String, "value": pl.String})
LABELS_SCHEMA = {
    "labels": pl.List(pl.Struct({"key": pl.String, "value": RECORD_SCHEMA}))
}


@pytest.mark.parametrize(
    "decode", [False, True, None], ids=["no_decode", "infer_decode", "dtype_decode"]
)
def test_normalise(benchmark, decode):
    """Benchmark normalisation with and without decode on the full dataset."""

    def run():
        decode_param = False if decode is None else decode
        df = DF_LABELS.genson.normalise_json(
            "labels", wrap_root="labels", decode=decode_param
        )
        if decode is None:
            df.str.json_decode(dtype=pl.Struct(LABELS_SCHEMA))
        return

    benchmark.pedantic(run, rounds=1, iterations=1)


def test_infer_schema_json(benchmark):
    """Benchmark JSON Schema inference from the full dataset."""

    def run():
        _ = DF_LABELS.genson.infer_json_schema("labels", wrap_root="labels")

    benchmark.pedantic(run, rounds=1, iterations=1)


def test_infer_schema_avro(benchmark):
    """Benchmark Avro schema inference from the full dataset."""

    def run():
        _ = DF_LABELS.genson.infer_json_schema("labels", wrap_root="labels", avro=True)

    benchmark.pedantic(run, rounds=1, iterations=1)
