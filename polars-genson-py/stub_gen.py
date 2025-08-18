#!/usr/bin/env python3
"""Build script for creating portable virtual environments for CI/CD."""

import hashlib
import os
import re
import shutil
import subprocess
from contextlib import suppress
from pathlib import Path
from typing import TYPE_CHECKING

CI_DIR = Path(".stubs")
CHECKSUM_DIR = CI_DIR / "checksums"
COMPRESSED_ARCHIVE = CI_DIR / "venv.tar.gz"
TEMP_VENV = CI_DIR / "temp-venv"
PYTHON_BIN = TEMP_VENV / "bin" / "python"
PYVENV_CFG = TEMP_VENV / "pyvenv.cfg"
VENV_STATE_CHECKSUM = CI_DIR / "venv_state.checksum"
ORIGINAL_VENV_PATH = Path(".venv")
DRY_RUN = False

LIB_NAME = "polars_genson"
BUILT_SO = Path("python") / LIB_NAME / f"_{LIB_NAME}.abi3.so"

# tar -tzvf .stubs/venv.tar.gz | awk '{print $3, $6}' | grep -v 'libpython3.13.so' | sort -nr | head -100
extra_excludes = [
    f"--exclude={pattern}"
    for pattern in [
        "lib/python3.13/pydoc_data/",
        "lib/Tix*/",
        "lib/itcl*/",
        "lib/thread*/",
        "lib/tk*/demos/",
        "lib/tcl*/encoding/",
        "include/",
        "share/",
        "*.a",
        "lib/tcl*/",  # Tcl stuff if you don't need Tkinter
        *[
            f"lib/python3.13/{stdlib}"
            for stdlib in [
                "test/",
                "unittest/test/",
                "idlelib/",
                "tkinter/",
                "turtle.py",
                "pydoc.py",
                "config-*/Makefile",
                "mailbox.py",
                "imaplib.py",
                "http/server.py",
                "xmlrpc/",
            ]
        ],
    ]
]

DEBUG = os.getenv("DEBUG_PYSNOOPER", False)

if TYPE_CHECKING or not DEBUG:

    def snoop():
        """Dummy replacement to pysnooper.snoop when debugging is off."""

        def decorator(func):
            return func

        return decorator
else:
    from pysnooper import snoop


def handle_subprocess_error(error: subprocess.CalledProcessError, message: str):
    """Display details of a checked subprocess call that errored."""
    print(f"✗ {message}")
    print(f"Return code: {error.returncode}")
    print(f"stdout: {error.stdout}")
    print(f"stderr: {error.stderr}")


@snoop()
def main():
    """Main build function for creating portable virtual environments."""
    for dir_path in (CI_DIR, CHECKSUM_DIR):
        dir_path.mkdir(parents=True, exist_ok=True)

    # Check if venv state has changed
    print("Checking venv state...")
    pattern = r"^.*\.py$|^.*\.so$|^.*\.so\..*$|^pyvenv\.cfg$"  # "*.py", "*.so", "*.so.*", "pyvenv.cfg"

    def file_matcher(path):
        rel_path = path.relative_to(ORIGINAL_VENV_PATH)
        return path.is_file() and re.match(pattern, str(rel_path))

    file_hits = filter(file_matcher, ORIGINAL_VENV_PATH.rglob("*"))

    # Checksums of all files
    file_hashes = (
        f"{hashlib.sha256(f.read_bytes()).hexdigest()}  {f}" for f in file_hits
    )

    # Checksum of newline conjoined sorted file checksums
    current_venv_checksum = hashlib.sha256(
        "\n".join(sorted(file_hashes)).encode()
    ).hexdigest()

    def human_size(size_bytes):
        """Get human-readable byte size."""
        for unit in ["B", "K", "M", "G"]:
            if size_bytes < 1024:
                return (
                    f"{size_bytes:.1f}{unit}"
                    if unit != "B"
                    else f"{int(size_bytes)}{unit}"
                )
            size_bytes /= 1024
        return f"{size_bytes:.1f}T"

    if (
        VENV_STATE_CHECKSUM.exists()
        and COMPRESSED_ARCHIVE.exists()
        and VENV_STATE_CHECKSUM.read_text().strip() == current_venv_checksum
    ):
        print("✓ Venv unchanged, using existing archive")
        compressed_size = human_size(COMPRESSED_ARCHIVE.stat().st_size)
        print(f"Archive size: {compressed_size}")
        exit(0)

    # Always start with a fresh copy of the current .venv
    print("Creating temp venv from current .venv...")
    if TEMP_VENV.exists():
        shutil.rmtree(TEMP_VENV)
    TEMP_VENV.mkdir(parents=True)

    # Use rsync for the exclude functionality
    subprocess.run(
        [
            "rsync",
            "-a",
            "--exclude=**/polars.abi3.so",
            "--exclude=**/_xxhash.cpython-*.so",
            f"{ORIGINAL_VENV_PATH}/",
            f"{TEMP_VENV}/",
        ],
        check=True,
    )

    # Build and copy the compiled polars-genson extension
    print("Building polars-genson extension...")
    try:
        subprocess.run(
            ["uv", "run", "--with", "maturin", "maturin", "develop"],
            check=True,
            cwd=Path.cwd(),
        )
        print("✓ Built polars-genson extension")
    except subprocess.CalledProcessError as e:
        handle_subprocess_error(
            error=e, message="Failed to build polars-genson extension"
        )
        exit(1)

    if BUILT_SO.exists():
        target_so = (
            TEMP_VENV
            / "lib"
            / "python3.13"
            / "site-packages"
            / LIB_NAME
            / BUILT_SO.name
        )
        target_so.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(BUILT_SO, target_so)
        print(f"✓ Copied {BUILT_SO.name} for compression")
    else:
        print(f"✗ {BUILT_SO} not found even after build - check maturin configuration")
        exit(1)

    # Fix the Python symlinks by copying the ENTIRE Python installation
    print("Making venv relocatable with Python 3.13...")

    if PYTHON_BIN.is_symlink():
        print("Converting Python symlinks and copying full Python installation...")

        # Find the real Python executable and its installation directory
        real_python = PYTHON_BIN.resolve()
        PYTHON_INSTALL_DIR = real_python.parent.parent

        print(f"Python installation at: {PYTHON_INSTALL_DIR}")

        if PYTHON_INSTALL_DIR.is_dir():
            # Remove symlinks
            for python_file in PYTHON_BIN.parent.glob("python*"):
                python_file.unlink(missing_ok=True)

            # Copy the entire Python installation into the venv (EXCLUDING site-packages)
            PYTHON_INSTALL_TARGET = TEMP_VENV / "python-install"
            PYTHON_INSTALL_TARGET.mkdir(parents=True, exist_ok=True)

            # Copy everything except site-packages directories at any level
            subprocess.run(
                [
                    "rsync",
                    "-a",
                    "--exclude=**/polars.abi3.so",
                    "--exclude=**/_xxhash.cpython-*.so",
                    "--exclude=site-packages/",
                    "--exclude=*/site-packages/",
                    "--exclude=**/site-packages/",
                    "--exclude=lib/python*/site-packages/",
                    "--exclude=lib/python3.13/ensurepip/_bundled/*.whl",
                    "--exclude=lib/python3.13/ensurepip/",
                    *extra_excludes,
                    f"{PYTHON_INSTALL_DIR}/",
                    f"{PYTHON_INSTALL_TARGET}/",
                ],
                check=True,
            )

            # Verify no site-packages were copied
            site_packages_found = list(PYTHON_INSTALL_TARGET.rglob("site-packages"))
            if site_packages_found:
                print("ERROR: site-packages found in python-install, cleaning up...")
                for site_pkg in site_packages_found:
                    if site_pkg.is_dir():
                        shutil.rmtree(site_pkg)

            # Create new executable that uses the copied Python with proper venv setup
            python_wrapper_content = """#!/bin/bash
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    VENV_DIR="$(dirname "$SCRIPT_DIR")"
    VIRTUAL_ENV="$VENV_DIR" PYTHONPATH="$VENV_DIR/lib/python3.13/site-packages" exec "$VENV_DIR/python-install/bin/python3.13" "$@"
    """

            PYTHON_BIN.write_text(python_wrapper_content)
            PYTHON_BIN.chmod(0o755)

            # Create symlinks
            bin_dir = TEMP_VENV / "bin"
            (bin_dir / "python3").symlink_to("python")
            (bin_dir / "python3.13").symlink_to("python")

            print("✓ Python 3.13 installation copied and made relocatable")
        else:
            print("✗ Could not find Python installation directory")
            exit(1)

    # Update pyvenv.cfg with placeholder paths (will be fixed during extraction)
    pyvenv_content = """home = PLACEHOLDER_DIR/python-install/bin
    include-system-site-packages = false
    version = 3.13.3
    executable = PLACEHOLDER_DIR/python-install/bin/python3.13
    command = PLACEHOLDER_DIR/python-install/bin/python3.13 -m venv PLACEHOLDER_DIR
    """
    PYVENV_CFG.write_text(pyvenv_content)
    print("✓ pyvenv.cfg created with placeholders")

    # Test before compression
    print("Testing venv before compression...")
    try:
        result = subprocess.run(
            [
                str(TEMP_VENV / "bin" / "python"),
                "-c",
                "import sys; print('Python version:', sys.version)",
            ],
            capture_output=True,
            text=True,
            check=True,
        )
        if "Python version:" in result.stdout:
            print("✓ Python wrapper works")
        else:
            print("✗ Python wrapper broken - unexpected output")
            exit(1)
    except subprocess.CalledProcessError as e:
        handle_subprocess_error(error=e, message="Python wrapper broken")
        exit(1)

    # Now compress .so files only (skip Python core libraries that UPX can't handle)
    print("Compressing shared libraries...")

    # Find all .so files
    so_files = list(TEMP_VENV.rglob("*.so")) + list(TEMP_VENV.rglob("*.so.*"))

    for file_path in so_files:
        if not file_path.is_file():
            continue

        # Skip Python core libraries that UPX can't compress AND site-packages in python-install
        if re.search(
            r"(libpython|python-install/bin/|python-install/lib/python3\.13/site-packages)",
            str(file_path),
        ):
            print(f"Skipping Python core file: {file_path.name}")
            continue

        # Skip if it's already been processed
        rel_path = file_path.relative_to(TEMP_VENV)
        safe_filename = str(rel_path).replace("/", "_").replace(".", "_")
        checksum_file = CHECKSUM_DIR / f"{safe_filename}.checksum"
        current_checksum = hashlib.sha256(file_path.read_bytes()).hexdigest()

        if (
            checksum_file.exists()
            and checksum_file.read_text().strip() == current_checksum
        ):
            print(f"✓ {rel_path} unchanged, skipping compression")
            continue

        print(f"Compressing {rel_path}...")
        backup_file = file_path.with_suffix(file_path.suffix + ".backup")
        shutil.copy2(file_path, backup_file)

        if DRY_RUN:
            print(f"DRY RUN: Would run: upx --best {file_path}")
            continue

        try:
            # Run upx compression
            subprocess.run(
                ["upx", "--best", str(file_path)], capture_output=True, check=True
            )
            # Test the compressed file
            subprocess.run(
                ["upx", "-t", str(file_path)], capture_output=True, check=True
            )

            print(f"✓ {rel_path} compressed successfully")
            checksum_file.write_text(current_checksum)
            backup_file.unlink()

        except subprocess.CalledProcessError:
            print(f"✗ {rel_path} compression failed, reverting...")
            shutil.move(backup_file, file_path)

    if DRY_RUN:
        print("DRY RUN: Skipping compression test and archive creation")
        exit(0)

    # Final test
    print("Testing compressed venv...")
    try:
        result = subprocess.run(
            [
                str(TEMP_VENV / "bin" / "python"),
                "-c",
                "import polars, polars_genson, pytest; print('All imports OK')",
            ],
            capture_output=True,
            text=True,
            check=True,
        )

        if "All imports OK" in result.stdout:
            print("✓ Compressed venv works, creating archive...")

            subprocess.run(
                [
                    "tar",
                    "-czf",
                    str(COMPRESSED_ARCHIVE),
                    "-C",
                    str(CI_DIR),
                    "temp-venv/",
                    "--transform",
                    "s/^temp-venv/venv/",
                ],
                check=True,
            )

            print(f"✓ Created {COMPRESSED_ARCHIVE}")

            # Save the venv state checksum for future runs
            VENV_STATE_CHECKSUM.write_text(current_venv_checksum)

            # Show size savings
            original_size = human_size(
                sum(
                    f.stat().st_size
                    for f in ORIGINAL_VENV_PATH.rglob("*")
                    if f.is_file()
                )
            )
            compressed_size = human_size(COMPRESSED_ARCHIVE.stat().st_size)
            print(f"Size: {original_size} -> {compressed_size}")

        else:
            print("✗ Compressed venv broken - unexpected output")
            print(f"stdout: {result.stdout}")
            print(f"stderr: {result.stderr}")
            exit(1)

    except subprocess.CalledProcessError as e:
        handle_subprocess_error(error=e, message="Compressed venv broken")

        # Show what went wrong
        with suppress(Exception):
            debug_result = subprocess.run(
                [
                    str(TEMP_VENV / "bin" / "python"),
                    "-c",
                    "import sys; print('Python works, version:', sys.version)",
                ],
                capture_output=True,
                text=True,
            )
            print(debug_result.stdout)

        exit(1)

    # Cleanup
    shutil.rmtree(TEMP_VENV)


if __name__ == "__main__":
    main()
