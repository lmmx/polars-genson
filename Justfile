import ".just/commit.just"
import ".just/bless.just"

default: clippy

ci_opt := if env("PRE_COMMIT_HOME", "") != "" { "-ci" } else { "" }

precommit:
    just pc{{ci_opt}}

pc:     fmt code-quality lint 
pc-fix: fmt code-quality-fix
pc-ci:      code-quality

prepush: check clippy docs py

prepush-rs:
    #!/usr/bin/env -S bash -euo pipefail
    just check-core
    just check-cli
    just clippy-core
    just clippy-cli
    just docs-core
    just docs-cli
    

# (Not running ty in lint recipe)
lint: ruff-check # lint-action

fmt:     ruff-fmt code-quality-fix

full:    pc prepush build test py
full-ci: pc-ci prepush         py

# usage:
#   just e                -> open Justfile normally
#   just e foo            -> search for "foo" and open Justfile at that line
#   just e @bar           -> search for "^bar" (recipe name) and open Justfile at that line
#
e target="":
    #!/usr/bin/env -S echo-comment --color bold-red
    if [[ "{{target}}" == "" ]]; then
      $EDITOR Justfile
    else
      pat="{{target}}"
      if [[ "$pat" == @* ]]; then
        pat="^${pat:1}"   # strip @ and prefix with ^
      fi
      line=$(rg -n "$pat" Justfile | head -n1 | cut -d: -f1)
      if [[ -n "$line" ]]; then
        $EDITOR +$line Justfile
      else
        # No match for: $pat
        exit 1
      fi
    fi

lint-action:
    actionlint .github/workflows/CI.yml

# -------------------------------------

build:
    cargo build --workspace

check:
    cargo check --workspace

check-core:
    cargo check -p genson-core

check-cli:
    cargo check -p genson-cli

check-py:
    cargo check -p polars-genson-py

# -------------------------------------

clippy: clippy-all

clippy-all:
    cargo clippy --workspace --all-targets --all-features --target-dir target/clippy-all-features -- -D warnings

clippy-core:
    cargo clippy -p genson-core -- -D warnings

clippy-cli:
    cargo clippy -p genson-cli -- -D warnings

clippy-py:
    cargo clippy -p polars-genson-py -- -D warnings

# -------------------------------------

test *args:
    just test-core {{args}} -F avro
    just test-cli {{args}}
    just test-js {{args}}

test-ci *args:
    #!/usr/bin/env -S echo-comment --color bright-green
    # 🏃 Running Rust tests...
    cargo test {{args}}
    
    # 📚 Running documentation tests...
    cargo test --doc {{args}}

[working-directory: 'genson-core']
test-core *args:
    cargo nextest run {{args}}
    
[working-directory: 'genson-cli']
test-cli *args:
    cargo nextest run {{args}}
    
test-pl *args:
    just py-test {{args}}

[working-directory: 'polars-jsonschema-bridge']
test-js *args:
    cargo nextest run {{args}}

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

py: py-dev py-test

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
py-test *args:
    #!/usr/bin/env bash
    $(uv python find) -m pytest tests/ {{args}}

[working-directory: 'polars-genson-py']
py-schema:
    $(uv python find) schema_demo.py

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
          --exclude ".git/|target/|dist/|\.swp|\.so$|.json$|.lock$|.parquet$|.venv/|.stubs/|\..*cache/" \
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

docs-core:
    cargo doc -p genson-core --all-features --no-deps --document-private-items --keep-going

docs-cli:
    cargo doc -p genson-cli --all-features --no-deps --document-private-items --keep-going

# -------------------------------------

mkdocs command="build":
    $(uv python find --directory polars-genson-py) -m mkdocs {{command}}

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

# The fmt flag is just for debugging
check-no-fmt-feat:
    #!/usr/bin/env echo-comment
    NO_RELEASE='"fmt"'
    HINT="# ✋🛑 Remove the $NO_RELEASE feature flag before release!"
    features=$(tq -r -f Cargo.toml workspace.dependencies.polars.features | grep -q $NO_RELEASE; echo $?)
    [ "$features" -eq 1 ] && true || { echo "$HINT" | echo-comment /dev/stdin --color=bold-red; false; }

demo-release:
     just check-no-fmt-feat
     echo "RELEASE IS GO"

# Release a new version, pass --help for options to `uv version --bump`
[working-directory: 'polars-genson-py']
release bump_level="patch":
    #!/usr/bin/env -S echo-comment --shell-flags="-e" --color bright-green
    
    ## Exit early if help was requested
    if [[ "{{bump_level}}" == "--help" ]]; then
        uv version --help
        exit 0
    fi

    just check-no-fmt-feat
    
    # 📈 Bump the version in pyproject.toml (patch/minor/major: {{bump_level}})
    uv version --bump {{bump_level}}
    
    # 📦 Stage all changes (including the version bump)
    git add --all
    
    # 🔄 Create a temporary commit to capture the new version
    git commit -m "chore(temp): version check"
     
    # ✂️  Extract the new version number that was just set, undo the commit
    new_version=$(uv version --short)
    git reset --soft HEAD~1
     
    # ✅ Stage everything again and create the real release commit
    git add --all
    git commit -m  "chore(release): bump 🐍 -> v$new_version"
     
    # 🏷️ Create the git tag for this release
    git tag -a "py-$new_version" -m "Python Release $new_version"
    
    branch_name=$(git rev-parse --abbrev-ref HEAD);
    # 🚀 Push the release commit to $branch_name
    git push origin $branch_name
    
    # 🚀 Push the commit tag to the remote
    git push origin "py-$new_version"
    
    # ⏳ Wait for CI to build wheels, then download and publish them
    test -z "$(compgen -G 'wheel*/')" || {
      # 🛡️ Safety first: halt if there are leftover wheel* directories from previous runs
      echo "Please delete the wheel*/ dirs:" >&2
      ls wheel*/ -1d >&2
      false
    }
    
    # 🎊 Wait for the current wheel build to finish then fetch and publish
    just ship-wheels
    
# Ship a new version as the final step of the release process (idempotent)
[working-directory: 'polars-genson-py']
ship-wheels mode="":
    just check-no-fmt-feat

    # 📥 Download wheel artifacts from the completed CI run
    ## -p wheel* downloads only artifacts matching the "wheel*" pattern
    gh run watch "$(gh run list -L 1 --json databaseId --jq .[0].databaseId)" {{mode}} --exit-status
    gh run download "$(gh run list -L 1 --json databaseId --jq .[0].databaseId)" -p wheel*
    
    # 🧹 Clean up any existing dist directory and create a fresh one
    rm -rf dist/
    mkdir dist/
    
    # 🎯 Move all wheel-* artifacts into dist/ and delete their temporary directories
    mv wheel*/* dist/
    rm -rf wheel*/
    
    # 🎊 Publish the CI-built wheels to PyPI
    uv publish -u __token__ -p $(keyring get PYPIRC_TOKEN "")

# --------------------------------------------------------------------------------------------------

# Rust release workflow using release-plz
ship-rust bump_level="auto":
    #!/usr/bin/env -S echo-comment --shell-flags="-euo pipefail" --color="\\033[38;5;202m"

    just check-no-fmt-feat

    # 🔍 Refuse to run if not on master branch or not up to date with origin/master
    branch="$(git rev-parse --abbrev-ref HEAD)"
    if [[ "$branch" != "master" ]]; then
        # ❌ Refusing to run: not on 'master' branch (current: $branch)
        exit 1
    fi
    # 🔍 Fetch master branch
    git fetch origin master
    local_rev="$(git rev-parse HEAD)"
    remote_rev="$(git rev-parse origin/master)"
    # 🔍 Local: $local_rev\n🔍 Remote: $remote_rev
    if [[ "$local_rev" != "$remote_rev" ]]; then
        # ❌ Refusing to run: local master branch is not up to date with origin/master
        # Local HEAD:  $local_rev
        # Origin HEAD: $remote_rev
        # Please pull/rebase to update.
        exit 1
    fi

    # 🔍 Dry-run release...
    just publish-rust --dry-run
    # ✅ Dry-run went OK, proceeding to real release

    if [[ "{{bump_level}}" != "auto" ]]; then
        if [[ -n "$(git status --porcelain)" ]]; then
            # ❌ Working directory must be clean for manual version bump
            git status --short
            exit 1
        fi
        cargo set-version -p genson-core --bump {{bump_level}}
        cargo set-version -p polars-jsonschema-bridge --bump {{bump_level}}
        cargo set-version -p genson-cli --bump {{bump_level}}
    fi
    
    # 🦀 Update Cargo.toml versions and changelogs
    release-plz update
    git add .
    # Run a pre-precommit lint pass to avoid the linter halting our release!
    just precommit || true
    git commit -m "chore(release): 🦀 Upgrades"
    # Note: if already pushed you would just need to revert the additions (delete changelogs)

    # 🦀 Run prepush only for the Rust crates we are releasing
    just prepush-rs
    # 🚀 Push the version bump commit
    git push --no-verify

    # 📦 Create releases and tags
    just publish-rust

publish-rust mode="":
    #!/usr/bin/env -S bash -euo pipefail
    git_token=$(gh auth token 2>/dev/null) || git_token=$PUBLISH_GITHUB_TOKEN

    ## 🦀 Let release-plz handle workspace crate tagging
    ## It will create tags like: genson-core-v0.2.1, genson-cli-v0.1.5, etc.
    release-plz release --backend github --git-token $git_token {{mode}}

# For when release-plz isn't working
ship-rust-manual:
    #!/usr/bin/env -S echo-comment --shell-flags="-euo pipefail" --color="\\033[38;5;202m"

    # Bump patch versions
    cargo set-version -p genson-core --bump patch
    cargo set-version -p polars-jsonschema-bridge --bump patch
    cargo set-version -p genson-cli --bump patch
    
    # Commit and tag (using the new CLI version as tag)
    git add .
    git commit -m "chore(release): 🦀 Upgrades"
    
    git tag genson-core-v$(cargo metadata --no-deps --format-version 1 \
      | jq -r '.packages[] | select(.name=="genson-core") | .version')
    
    git tag polars-jsonschema-bridge-v$(cargo metadata --no-deps --format-version 1 \
      | jq -r '.packages[] | select(.name=="polars-jsonschema-bridge") | .version')
    
    git tag genson-cli-v$(cargo metadata --no-deps --format-version 1 \
      | jq -r '.packages[] | select(.name=="genson-cli") | .version')
    
    # Push to remote
    git push
    git push --tags
    
    # Publish in dependency order
    cargo publish -p genson-core
    cargo publish -p polars-jsonschema-bridge
    cargo publish -p genson-cli
