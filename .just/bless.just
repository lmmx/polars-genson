# Bless a snapshot by copying it to the verified directory with optional reason
# Usage: just bless map_of_records__infer__jsonschema.snap
# Usage: just bless map_of_records__infer__jsonschema.snap "Empty object gets normalised to null"
[no-cd]
bless snapshot_file *reason:
    #!/usr/bin/env -S echo-comment --color bold-green --shell-flags="-euo pipefail"
    
    ## Find the crate root using cargo
    CARGO_TOML=$(cargo locate-project --message-format=plain)
    CRATE_ROOT=$(dirname "$CARGO_TOML")
    
    ## Set paths relative to crate root
    SNAPSHOTS_DIR="$CRATE_ROOT/tests/snapshots"
    VERIFIED_DIR="$CRATE_ROOT/tests/verified"
    
    ## Check if snapshots directory exists
    if [[ ! -d "$SNAPSHOTS_DIR" ]]; then
        # Error: $SNAPSHOTS_DIR directory not found
        exit 1
    fi
    
    ## Check if file exists
    if [[ ! -f "$SNAPSHOTS_DIR/{{snapshot_file}}" ]]; then
        # Error: {{snapshot_file}} not found in $SNAPSHOTS_DIR
        exit 1
    fi
    
    ## Create verified directory if it doesn't exist
    mkdir -p "$VERIFIED_DIR"
    
    ## Copy the snapshot to verified directory
    cp "$SNAPSHOTS_DIR/{{snapshot_file}}" "$VERIFIED_DIR/{{snapshot_file}}"
    sed -i '1,/^---$/ { /^[[:space:]]*approved:/d }' "$VERIFIED_DIR/{{snapshot_file}}"
    
    ## If reason provided, add it to the blessed snapshot header
    if [[ "{{reason}}" != "" ]]; then
        # Add blessing metadata after the first line (---)
        sed -i '2i blessing:' "$VERIFIED_DIR/{{snapshot_file}}"
        sed -i '3i   reason: "{{reason}}"' "$VERIFIED_DIR/{{snapshot_file}}"
    fi
    
    ## Show relative paths for cleaner output
    SNAPSHOTS_REL=$(realpath --relative-to="$PWD" "$SNAPSHOTS_DIR/{{snapshot_file}}")
    VERIFIED_REL=$(realpath --relative-to="$PWD" "$VERIFIED_DIR/{{snapshot_file}}")
    
    # ✓ Blessed $SNAPSHOTS_REL → $VERIFIED_REL
    if [[ "{{reason}}" != "" ]]; then
        # Reason: {{reason}}
    fi
    
    ## Extract module name from snapshot filename (everything before first __)
    MODULE_NAME=$(echo "{{snapshot_file}}" | sed 's/__.*$//')
    
    ## Delete original snapshot and regenerate with approval
    rm "$SNAPSHOTS_DIR/{{snapshot_file}}"
    
    ## Run tests for just this module and accept the regenerated snapshots
    echo "Running tests for module: $MODULE_NAME"
    cargo insta test --accept --test "$MODULE_NAME"

# -------- Summarisation recipes ---------

# Show summary of snapshot blessing status
[no-cd]
snapshot-status:
    #!/usr/bin/env -S echo-comment --color bold-yellow --shell-flags="-euo pipefail"
    
    ## Find the crate root using cargo
    CARGO_TOML=$(cargo locate-project --message-format=plain)
    CRATE_ROOT=$(dirname "$CARGO_TOML")
    SNAPSHOTS_DIR="$CRATE_ROOT/tests/snapshots"
    
    ## Check if snapshots directory exists
    if [[ ! -d "$SNAPSHOTS_DIR" ]]; then
        # Error: $SNAPSHOTS_DIR directory not found
        exit 1
    fi
    
    # === Snapshot Blessing Status ===
    
    total=0
    approved=0
    
    for snap in "$SNAPSHOTS_DIR"/*.snap; do
        if [[ -f "$snap" ]]; then
            total=$((total + 1))
            filename=$(basename "$snap")
            if grep -q 'approved: true' "$snap"; then
                approved=$((approved + 1))
                # ✓ $filename
            else
                # ✗ $filename
            fi
        fi
    done
    
    echo ""
    # Summary: $approved/$total snapshots approved

# List only unblessed snapshots needing attention
[no-cd]
unblessed:
    #!/usr/bin/env -S echo-comment --color bold-yellow --shell-flags="-euo pipefail"
    
    # === Snapshots needing blessing ===
    just snapshot-status | rg "✗"

# Show which snapshots are already blessed
[no-cd]
blessed:
    #!/usr/bin/env -S echo-comment --color bold-green --shell-flags="-euo pipefail"
    
    # === Blessed snapshots ===
    just snapshot-status | rg "✓"
