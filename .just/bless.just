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

# Show summary of snapshot blessing status with fallen/unblessed distinction
[no-cd]
[no-exit-message]
snapshot-status:
    #!/usr/bin/env -S echo-comment --color bold-yellow --shell-flags="-euo pipefail"
    
    ## Find the crate root using cargo
    CARGO_TOML=$(cargo locate-project --message-format=plain)
    CRATE_ROOT=$(dirname "$CARGO_TOML")
    SNAPSHOTS_DIR="$CRATE_ROOT/tests/snapshots"
    VERIFIED_DIR="$CRATE_ROOT/tests/verified"
    
    ## Check if snapshots directory exists
    if [[ ! -d "$SNAPSHOTS_DIR" ]]; then
        # Error: $SNAPSHOTS_DIR directory not found
        exit 1
    fi
    
    # === Snapshot Blessing Status ===
    
    total=0
    approved=0
    fallen=0
    unblessed=0
    
    for snap in "$SNAPSHOTS_DIR"/*.snap; do
        if [[ -f "$snap" ]]; then
            total=$((total + 1))
            filename=$(basename "$snap")
            if grep -q 'approved: true' "$snap"; then
                approved=$((approved + 1))
                # ✓ $filename
            else
                ## Check if verified version exists to distinguish fallen from unblessed
                if [[ -f "$VERIFIED_DIR/$filename" ]]; then
                    fallen=$((fallen + 1))
                    # ↯ $filename
                else
                    unblessed=$((unblessed + 1))
                    # ✗ $filename
                fi
            fi
        fi
    done
    
    echo ""
    # Summary: $approved approved, $fallen fallen, $unblessed unblessed ($total total)

# Show which snapshots are already blessed
[no-cd]
[no-exit-message]
blessed:
    #!/usr/bin/env -S echo-comment --color bold-green --shell-flags="-euo pipefail"
    
    # === Blessed snapshots ===
    just snapshot-status | rg "✓"

# List unblessed snapshots that have never been blessed
[no-cd]
[no-exit-message]
profane:
    #!/usr/bin/env -S echo-comment --color bold-yellow --shell-flags="-euo pipefail"
    
    # === Snapshots needing blessing ===
    just snapshot-status | rg "✗"

# List unblessed snapshots that have previously blessed but are now stale
[no-cd]
[no-exit-message]
fallen:
    #!/usr/bin/env -S echo-comment --color bold-yellow --shell-flags="-euo pipefail"
    
    # === Snapshots previously blessed but now stale ===
    just snapshot-status | rg --colors 'match:fg:208' "↯"

[no-cd]
[no-exit-message]
unblessed:
    -just profane
    -just fallen

# Rebless a snapshot by preserving the existing blessing reason from verified version
# Usage: just rebless map_of_records__infer__jsonschema.snap
[no-cd]
rebless snapshot_file:
    #!/usr/bin/env -S echo-comment --color bold-blue --shell-flags="-euo pipefail"
    
    ## Find the crate root using cargo
    CARGO_TOML=$(cargo locate-project --message-format=plain)
    CRATE_ROOT=$(dirname "$CARGO_TOML")
    VERIFIED_DIR="$CRATE_ROOT/tests/verified"
    
    ## Check if verified version exists
    if [[ ! -f "$VERIFIED_DIR/{{snapshot_file}}" ]]; then
        # Error: No verified version of {{snapshot_file}} found - use 'just bless' instead
        exit 1
    fi
    
    ## Extract existing blessing reason from verified version
    BLESSING_REASON=""
    if grep -q "reason:" "$VERIFIED_DIR/{{snapshot_file}}"; then
        BLESSING_REASON=$(awk '/reason:/ { gsub(/^[[:space:]]*reason:[[:space:]]*"/, ""); gsub(/"[[:space:]]*$/, ""); print; exit }' "$VERIFIED_DIR/{{snapshot_file}}")
    fi
    
    ## Require blessing reason - exit if none found
    if [[ "$BLESSING_REASON" == "" ]]; then
        # Error: No blessing reason found in verified version - cannot rebless safely
        exit 1
    fi
    
    ## Use existing bless recipe with preserved reason
    # Reblessing with preserved reason: $BLESSING_REASON
    just bless "{{snapshot_file}}" "$BLESSING_REASON"

# Confess what changed in a fallen snapshot using difft
# Usage: just confess map_of_records__infer__jsonschema.snap
[no-cd]
confess snapshot_file:
    #!/usr/bin/env -S echo-comment --color bold-magenta --shell-flags="-euo pipefail"
    
    ## Find the crate root using cargo
    CARGO_TOML=$(cargo locate-project --message-format=plain)
    CRATE_ROOT=$(dirname "$CARGO_TOML")
    
    ## Set paths relative to crate root
    SNAPSHOTS_DIR="$CRATE_ROOT/tests/snapshots"
    VERIFIED_DIR="$CRATE_ROOT/tests/verified"
    
    ## Check if snapshot is actually fallen
    if ! just fallen | rg -q "↯ {{snapshot_file}}"; then
        # Error: {{snapshot_file}} is not fallen - use this only for fallen snapshots
        exit 1
    fi
    
    ## Show the confession
    # === Confessing changes in {{snapshot_file}} ===
    difft "$SNAPSHOTS_DIR/{{snapshot_file}}" "$VERIFIED_DIR/{{snapshot_file}}"
