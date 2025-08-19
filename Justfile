default: clippy

# lint:    ty ruff-check
lint: ruff-check
lint-ci: clippy-ci
# lint-ci: ty-ci ruff-check

fmt:     ruff-fmt code-quality-fix

precommit:     lint fmt code-quality
precommit-ci:  lint-ci  code-quality
precommit-fix: fmt      code-quality-fix

prepush: clippy py-test py-dev

ci: precommit prepush docs

# Full development workflow
full: check clippy-all build test py-dev py-test

# CI workflow
ci-full: precommit-ci prepush py-dev py-test docs

e:
    $EDITOR Justfile

# -------------------------------------

build:
    cargo build --workspace

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

clippy-all:
    cargo clippy --workspace --all-targets --all-features --target-dir target/clippy-all-features -- -D warnings

clippy:
    cargo clippy --workspace --all-targets --target-dir target/clippy -- -D warnings

clippy-ci:
    cargo clippy --offline --workspace --all-targets --target-dir target/clippy -- -D warnings

# Fast clippy for individual packages
clippy-core:
    cargo clippy -p genson-core -- -D warnings

clippy-cli:
    cargo clippy -p genson-cli -- -D warnings

clippy-py:
    cargo clippy -p polars-genson-py -- -D warnings

# -------------------------------------

test *args:
    just test-core {{args}}
    just test-cli {{args}}
    just test-js {{args}}

[working-directory: 'genson-core']
test-core *args:
    cargo nextest run {{args}}
    
[working-directory: 'genson-cli']
test-cli *args:
    cargo nextest run {{args}}
    
test-pl *args:
    just test-py {{args}}

[working-directory: 'polars-jsonschema-bridge']
test-js *args:
    cargo nextest run {{args}}


test-ci *args:
    #!/usr/bin/env -S echo-comment --color bright-green
    # 🏃 Running Rust tests...
    cargo test {{args}}
    
    # 📚 Running documentation tests...
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
    #!/usr/bin/env -S echo-comment --shell-flags="-e" --color blue
    # 🔍 CI Environment Debug Information
    # Current directory: $(pwd)
    # Python available: $(which python3 || echo 'none')
    # UV available: $(which uv || echo 'none')
    
    ## Check if .venv exists, if not extract from compressed CI venv
    if [ ! -d ".venv" ]; then
        # 📦 Extracting compressed stubs for CI...
        if [ -f ".stubs/venv.tar.gz" ]; then
            # Found compressed stubs, extracting...
            tar -xzf .stubs/venv.tar.gz
            mv venv .venv
            
            ## Fix pyvenv.cfg with current absolute path
            if [ -f ".venv/pyvenv.cfg" ]; then
                CURRENT_DIR=$(pwd)
                sed -i "s|PLACEHOLDER_DIR|${CURRENT_DIR}/.venv|g" ".venv/pyvenv.cfg"
                # ✓ pyvenv.cfg updated with current directory: $CURRENT_DIR
                # Updated pyvenv.cfg contents:
                cat ".venv/pyvenv.cfg"
            fi
            
            # ✅ Extraction complete, running diagnostics...
            
            ## Diagnostic checks
            # 🔍 Venv structure check:
            ls -la .venv/ | head -5
            #
            
            # 🔍 Python interpreter check:
            if [ -f ".venv/bin/python" ]; then
                # Python executable exists
                .venv/bin/python --version || echo "❌ Python version check failed"
            else
                # ❌ No Python executable found
                ls -la .venv/bin/ | head -5
            fi
            
            # 🔍 Site-packages check:
            SITE_PACKAGES=".venv/lib/python*/site-packages"
            if ls $SITE_PACKAGES >/dev/null 2>&1; then
                # Site-packages directory exists:
                ls $SITE_PACKAGES | grep -E "(polars|polars_genson)" || echo "❌ Key packages not found"
            else
                # ❌ No site-packages directory found
            fi
            
            # 🔍 Environment activation test:
            export PATH="$(pwd)/.venv/bin:$PATH"
            export VIRTUAL_ENV="$(pwd)/.venv"
            export PYTHONPATH=""  # Clear any existing PYTHONPATH
            
            # Active Python: $(which python)
            python --version || echo "❌ Python activation failed"
            
            # 🔍 Critical imports test:
            python -c 'import sys; print("✓ Python sys module working"); print("Python executable:", sys.executable)' || echo "❌ Basic Python test failed"
            python -c 'import polars as pl; print("✓ Polars import successful, version:", pl.__version__)' || echo "❌ Polars import failed"
            python -c 'import polars_genson; print("✓ Polars Genson import successful")' || echo "❌ Polars Genson import failed"
            python -c 'import pytest; print("✓ Pytest import successful")' || echo "❌ Pytest import failed"
            
        else
            # ❌ No stubs found, running regular setup...
            just setup
        fi
    else
        # ✅ .venv already exists, activating...
        export PATH="$(pwd)/.venv/bin:$PATH"
        export VIRTUAL_ENV="$(pwd)/.venv"
    fi
    
    # 🚀 Running ty check...
    just t

# -------------------------------------

[working-directory: 'polars-genson-py']
pf:
    pyrefly check . --output-format=min-text

# -------------------------------------

# Test CLI with example input
run-cli input="'{\"name\": \"test\", \"value\": 42}'":
    echo '{{input}}' | cargo run -p genson-cli

# Run CLI with file
run-cli-on *args:
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
    
    schema = df.genson.infer_json_schema('json_data')
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
          --exclude ".git/|target/|dist/|\.so$|.json$|.lock$|.parquet$|.venv/|.stubs/|\..*cache/" \
          $ARGS \
          .

code-quality:
    # just ty-ci
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
    #!/usr/bin/env -S echo-comment --shell-flags="-e" --color bright-green
    rm -rf .stubs
    set -e  # Exit on any error
    
    ## Check if --debug flag is passed and export DEBUG_PYSNOOPER
    debug_flag=false
    uv_args="--no-group debug"
    # Args received: {{args}}
    if [[ "{{args}}" == *"--debug"* ]]; then
        export DEBUG_PYSNOOPER=true
        # DEBUG MODE: ON
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
    
    ## Unset DEBUG_PYSNOOPER if it was set
    if [[ "$debug_flag" == "true" ]]; then
        unset DEBUG_PYSNOOPER
    fi


# Release a new version, pass --help for options to `uv version --bump`
[working-directory: 'polars-genson-py']
release bump_level="patch":
    #!/usr/bin/env -S echo-comment --shell-flags="-e" --color blue

    ## Exit early if help was requested
    if [[ "{{bump_level}}" == "--help" ]]; then
        uv version --help
        exit 0
    fi

    uv version --bump {{bump_level}}
    
    git add --all
    git commit -m "chore(temp): version check"
    new_version=$(uv version --short)
    git reset --soft HEAD~1
    git add --all
    git commit -m  "chore(release): bump -> v$new_version"
    branch_name=$(git rev-parse --abbrev-ref HEAD);
    git push origin $branch_name
    uv build
    uv publish -u __token__ -p $(keyring get PYPIRC_TOKEN "")
