#!/usr/bin/env python3
"""Simple demo for memory profiling."""

import threading
import time
from pathlib import Path

import polars as pl
import polars_genson


def get_rss_bytes():
    """Get current RSS memory usage in bytes."""
    try:
        with open("/proc/self/status", "r") as f:
            for line in f:
                if line.startswith("VmRSS:"):
                    # Format: "VmRSS:  123456 kB"
                    kb = int(line.split()[1])
                    return kb * 1024  # Convert to bytes
    except (FileNotFoundError, ValueError, IndexError):
        return None
    return None


def format_bytes(bytes_val):
    """Format bytes to human-readable string."""
    MB = 1024 * 1024
    GB = 1024 * 1024 * 1024

    if bytes_val >= GB:
        return f"{bytes_val / GB:.2f} GB"
    elif bytes_val >= MB:
        return f"{bytes_val / MB:.2f} MB"
    else:
        return f"{bytes_val / 1024:.2f} KB"


class MemoryTracker:
    """Track memory usage with continuous background monitoring."""

    def __init__(self):
        """Set up memory tracking."""
        time.sleep(0.1)  # Let process stabilize
        self.start_rss = get_rss_bytes() or 0
        self.peak_rss = self.start_rss
        self.monitoring = True

        print("📊 Memory tracking started")
        print(f"   Start RSS: {format_bytes(self.start_rss)}")

        # Start background monitoring thread
        self.monitor_thread = threading.Thread(target=self._monitor, daemon=True)
        self.monitor_thread.start()

    def _monitor(self):
        """Background thread to continuously monitor memory."""
        while self.monitoring:
            current = get_rss_bytes()
            if current and current > self.peak_rss:
                self.peak_rss = current
            time.sleep(0.01)  # Check every 10ms

    def report_final(self):
        """Stop monitoring and report final statistics."""
        self.monitoring = False
        self.monitor_thread.join(timeout=1.0)

        end_rss = get_rss_bytes() or 0

        print("\n📊 Memory Usage Summary:")
        print(f"   Start RSS: {format_bytes(self.start_rss)}")
        print(f"   Peak RSS:  {format_bytes(self.peak_rss)}")
        print(f"   End RSS:   {format_bytes(end_rss)}")
        print(f"   Delta:     {format_bytes(end_rss - self.start_rss)}")


def main():
    """Demo for memory profiling."""
    mem_tracker = MemoryTracker()

    # Hardcoded path to your fixture file
    path = (
        Path.home() / "dev/polars-genson/genson-cli/tests/data/claims_fixture_x30.jsonl"
    )
    n_rows = 400

    print(f"\nLoading first {n_rows} JSON rows from {path}")
    with open(path, "r") as f:
        json_lines = [line.strip() for line in f if line.strip()][:n_rows]

    df = pl.DataFrame({"claims": json_lines})
    print(f"DataFrame loaded with {df.height} rows")

    current = get_rss_bytes()
    if current:
        print(f"Current RSS after loading data: {format_bytes(current)}")

    print("\n=== Running genson.normalise_json ===")
    t0 = time.perf_counter()

    result = df.genson.normalise_json(
        "claims",
        wrap_root="claims",
        map_threshold=0,
        unify_maps=True,
        force_field_types={"mainsnak": "record"},
        no_unify={"qualifiers"},
        decode=True,
        profile=True,
    )

    t1 = time.perf_counter()
    print(f"\nCompleted in {t1 - t0:.2f} s")
    print(f"Result type: {type(result)}")

    if hasattr(result, "shape"):
        print(f"Result shape: {result.shape}")

    mem_tracker.report_final()


if __name__ == "__main__":
    main()
