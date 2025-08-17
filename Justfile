default: lint

lint: ty ruff-check
fmt: ruff-fmt code-quality-fix

precommit:    lint fmt code-quality
precommit-ci: lint code-quality-ci
precommit-fix: fmt code-quality-fix

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
   ty check . {{args}} 2> >(grep -v "WARN ty is pre-release software" >&2)

t:
   just ty --output-format=concise

ty-ci:
    #!/usr/bin/env bash
    set -e  # Exit on any error
    
    echo "🔍 CI Environment Debug Information"
    echo "Current directory: $(pwd)"
    echo "Python available: $(which python3 || echo 'none')"
    
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
            fi
            
            echo "✅ Extraction complete"
            export PATH="$(pwd)/.venv/bin:$PATH"
            export VIRTUAL_ENV="$(pwd)/.venv"
            
            echo "🔍 Critical imports test:"
            python -c 'import polars as pl; print("✓ Polars import successful")' || echo "❌ Polars import failed"
            
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
    just ty

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
    uv build

# Develop Python plugin (release mode)  
[working-directory: 'polars-genson-py']
py-release:
    maturin develop --release

# Test Python plugin
[working-directory: 'polars-genson-py']
py-test:
    #!/usr/bin/env bash
    echo python -c "
    import polars as pl
    df = pl.DataFrame({
        'json_data': [
            '{\"name\": \"Alice\", \"age\": 30}',
            '{\"name\": \"Bob\", \"age\": 25, \"city\": \"NYC\"}'
        ]
    })
    
    schema = df.genson.infer_schema('json_data')
    print('Schema inference successful!')
    print(schema)
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

refresh-stubs *args="":
    #!/usr/bin/env bash
    rm -rf .stubs
    set -e  # Exit on any error
    
    # Check if --debug flag is passed 
    debug_flag=false
    echo "Args received: {{args}}"
    if [[ "{{args}}" == *"--debug"* ]]; then
        echo "DEBUG MODE: ON"
        debug_flag=true
    fi
    
    uv sync
    # Create compressed stubs for CI
    mkdir -p .stubs
    cp -r .venv venv
    # Fix pyvenv.cfg for CI 
    sed -i 's|home = .*|home = PLACEHOLDER_DIR/bin|g' venv/pyvenv.cfg
    tar -czf .stubs/venv.tar.gz venv
    rm -rf venv
    
    echo "✅ Stubs refreshed"
