default: lint

lint:    ty ruff-check
lint-ci: ty-ci ruff-check

fmt:     ruff-fmt code-quality-fix

precommit:     lint fmt code-quality
precommit-ci:  lint-ci  code-quality-ci
precommit-fix: fmt      code-quality-fix

prepush: clippy py-test py-dev

ci: precommit prepush docs

# Full development workflow
dev-test: check test py-dev py-test

# CI workflow
ci-full: precommit-ci prepush py-dev py-test docs

e:
    $EDITOR Justfile

# -------------------------------------


clippy-all:
    cargo clippy --workspace --all-targets --all-features --target-dir target/clippy-all-features -- -D warnings

clippy:
    cargo clippy --workspace --all-targets --target-dir target/clippy -- -D warnings

# Fast clippy for individual packages
clippy-core:
    cargo clippy -p genson-core -- -D warnings

clippy-cli:
    cargo clippy -p genson-cli -- -D warnings

clippy-py:
    cargo clippy -p polars-genson-py -- -D warnings

# -------------------------------------

test *args:
    cargo test {{args}}

test-ci *args:
    #!/usr/bin/env -S bash -euo pipefail
    echo -e "\033[1;33m🏃 Running Rust tests...\033[0m"
    cargo test {{args}}
    
    echo -e "\033[1;36m📚 Running documentation tests...\033[0m"
    cargo test --doc {{args}}

# -------------------------------------

[working-directory: 'polars-genson-py']
ruff-check mode="":
   ruff check . {{mode}}

[working-directory: 'polars-genson-py']
ruff-fix:
   just ruff-check --fix

[working-directory: 'polars-genson-py']
ruff-fmt:
   ruff format .

# Type checking
[working-directory: 'polars-genson-py']
ty *args:
   #!/usr/bin/env bash
   ty check . --exit-zero {{args}} 2> >(grep -v "WARN ty is pre-release software" >&2)

t:
   just ty --output-format=concise

[working-directory: 'polars-genson-py']
ty-ci:
    #!/usr/bin/env bash
    set -e  # Exit on any error
    
    echo "🔍 CI Environment Debug Information"
    echo "Current directory: $(pwd)"
    echo "Python available: $(which python3 || echo 'none')"
    echo "UV available: $(which uv || echo 'none')"
    
    # Check if .venv exists, if not extract from compressed CI venv
    if [ ! -d ".venv" ]; then
        echo "📦 Extracting compressed stubs for CI..."
        if [ -f ".stubs/venv.tar.gz" ]; then
            echo "Found compressed stubs, extracting..."
            tar -xzf .stubs/venv.tar.gz
            mv venv .venv
            
            # Fix pyvenv.cfg with current absolute path
            if [ -f ".venv/pyvenv.cfg" ]; then
                CURRENT_DIR=$(pwd)
                sed -i "s|PLACEHOLDER_DIR|${CURRENT_DIR}/.venv|g" ".venv/pyvenv.cfg"
                echo "✓ pyvenv.cfg updated with current directory: $CURRENT_DIR"
                echo "Updated pyvenv.cfg contents:"
                cat ".venv/pyvenv.cfg"
            fi
            
            echo "✅ Extraction complete, running diagnostics..."
            
            # Diagnostic checks
            echo "🔍 Venv structure check:"
            ls -la .venv/ | head -5
            echo ""
            
            echo "🔍 Python interpreter check:"
            if [ -f ".venv/bin/python" ]; then
                echo "Python executable exists"
                .venv/bin/python --version || echo "❌ Python version check failed"
            else
                echo "❌ No Python executable found"
                ls -la .venv/bin/ | head -5
            fi
            
            echo "🔍 Site-packages check:"
            SITE_PACKAGES=".venv/lib/python*/site-packages"
            if ls $SITE_PACKAGES >/dev/null 2>&1; then
                echo "Site-packages directory exists:"
                ls $SITE_PACKAGES | grep -E "(polars|polars_genson)" || echo "❌ Key packages not found"
            else
                echo "❌ No site-packages directory found"
            fi
            
            echo "🔍 Environment activation test:"
            export PATH="$(pwd)/.venv/bin:$PATH"
            export VIRTUAL_ENV="$(pwd)/.venv"
            export PYTHONPATH=""  # Clear any existing PYTHONPATH
            
            echo "Active Python: $(which python)"
            python --version || echo "❌ Python activation failed"
            
            echo "🔍 Critical imports test:"
            python -c 'import sys; print("✓ Python sys module working"); print("Python executable:", sys.executable)' || echo "❌ Basic Python test failed"
            python -c 'import polars as pl; print("✓ Polars import successful, version:", pl.__version__)' || echo "❌ Polars import failed"
            python -c 'import polars_genson; print("✓ Polars Genson import successful")' || echo "❌ Polars Genson import failed"
            python -c 'import pytest; print("✓ Pytest import successful")' || echo "❌ Pytest import failed"
            
        else
            echo "❌ No stubs found, running regular setup..."
            just setup
        fi
    else
        echo "✅ .venv already exists, activating..."
        export PATH="$(pwd)/.venv/bin:$PATH"
        export VIRTUAL_ENV="$(pwd)/.venv"
    fi
    
    echo "🚀 Running ty check..."
    just ty .

# -------------------------------------

[working-directory: 'polars-genson-py']
pf:
    pyrefly check . --output-format=min-text

# -------------------------------------

# Check all projects
check:
    cargo check --workspace

# Fast individual package checks
check-core:
    cargo check -p genson-core

check-cli:
    cargo check -p genson-cli

check-py:
    cargo check -p polars-genson-py

# -------------------------------------

# Test CLI with example input
test-cli input="'{\"name\": \"test\", \"value\": 42}'":
    echo '{{input}}' | cargo run -p genson-cli

# Run CLI with file
run-cli *args:
    cargo run -p genson-cli -- {{args}}

# -------------------------------------

# Develop Python plugin (debug mode)
[working-directory: 'polars-genson-py']
py-dev:
    $(uv python find) -m maturin develop

# Develop Python plugin (release mode)  
[working-directory: 'polars-genson-py']
py-release:
    $(uv python find) -m maturin develop --release

# Test Python plugin with pytest
[working-directory: 'polars-genson-py']
py-test:
    #!/usr/bin/env bash
    $(uv python find) -m pytest tests/

# Quick test to verify basic functionality  
[working-directory: 'polars-genson-py']
py-quick:
    #!/usr/bin/env bash
    python -c "
    import polars as pl
    import polars_genson
    import json
    
    print('Testing polars-genson plugin...')
    
    df = pl.DataFrame({
        'json_data': [
            '{\"name\": \"Alice\", \"age\": 30}',
            '{\"name\": \"Bob\", \"age\": 25, \"city\": \"NYC\"}',
            '{\"name\": \"Charlie\", \"age\": 35, \"email\": \"charlie@example.com\"}'
        ]
    })
    
    print('Input DataFrame:')
    print(df)
    
    schema = df.genson.infer_schema('json_data')
    print('\nInferred schema:')
    print(json.dumps(schema, indent=2))
    
    # Verify schema structure
    assert 'type' in schema
    assert 'properties' in schema
    props = schema['properties']
    assert 'name' in props
    assert 'age' in props
    
    print('\n✅ Schema inference successful!')
    print(f'Found properties: {list(props.keys())}')
    "

# -------------------------------------

install-hooks:
   pre-commit install

run-pc:
   pre-commit run --all-files

[working-directory: 'polars-genson-py']
setup:
   #!/usr/bin/env bash
   uv venv
   source .venv/bin/activate
   uv sync

[working-directory: 'polars-genson-py']
sync:
   uv sync

# -------------------------------------

fix-eof-ws mode="":
    #!/usr/bin/env sh
    ARGS=''
    if [ "{{mode}}" = "check" ]; then
        ARGS="--check-only"
    fi
    whitespace-format --add-new-line-marker-at-end-of-file \
          --new-line-marker=linux \
          --normalize-new-line-markers \
          --exclude ".git/|target/|.json$|.lock$|.parquet$|.venv/|.stubs/|\..*cache/" \
          $ARGS \
          .

code-quality:
    just ty
    taplo lint
    taplo format --check
    just fix-eof-ws check
    cargo machete
    cargo fmt --check --all

code-quality-ci:
    just ty-ci
    taplo lint
    taplo format --check
    just fix-eof-ws check
    cargo machete
    cargo fmt --check --all

code-quality-fix:
    taplo lint
    taplo format
    just fix-eof-ws
    cargo machete
    cargo fmt --all

# -------------------------------------

docs:
    cargo doc --workspace --all-features --no-deps --document-private-items --keep-going

# -------------------------------------

clean:
    cargo clean
    rm -rf polars-genson-py/target

# -------------------------------------

# Example: JSON schema inference
example-basic:
    just test-cli '{"name": "Alice", "age": 30}'

example-array:
    just test-cli '[{"name": "Alice", "age": 30}, {"name": "Bob", "age": 25, "city": "NYC"}]'

example-complex:
    echo '{"users": [{"name": "Alice", "profile": {"age": 30, "active": true}}, {"name": "Bob", "profile": {"age": 25, "premium": false}}]}' | just run-cli

# -------------------------------------

[working-directory: 'polars-genson-py']
refresh-stubs *args="":
    #!/usr/bin/env bash
    rm -rf .stubs
    set -e  # Exit on any error
    
    # Check if --debug flag is passed and export DEBUG_PYSNOOPER
    debug_flag=false
    uv_args="--no-group debug"
    echo "Args received: {{args}}"
    if [[ "{{args}}" == *"--debug"* ]]; then
        export DEBUG_PYSNOOPER=true
        echo "DEBUG MODE: ON"
        debug_flag=true
        uv_args=""  # Remove --no-group debug when in debug mode
    fi
    
    uv sync --no-group build $uv_args
    ./stub_gen.py
    deactivate
    mv .venv/ offvenv
    just run-pc
    rm -rf .venv
    mv offvenv .venv
    
    # Unset DEBUG_PYSNOOPER if it was set
    if [[ "$debug_flag" == "true" ]]; then
        unset DEBUG_PYSNOOPER
    fi

