# Test reduction parameters for genson bug reproduction
# Usage: just find-all-limits
# L26 currently has skip-first 135 applied, let's find the real skip-last limit
# just find-skip-last-limit 26 225 1 135

# Configuration
CLI := "../target/release/genson-cli"
WORKING_LINES := "1"
DATA_DIR := "tests/data/claims"

# Test if a file exhibits non-determinism AND maintains unification (no P-codes in output)
[working-directory: 'genson-cli']
test-repro line suffix="PENULTIMATE":
    #!/bin/bash
    set -euo pipefail
    
    max_runs=10
    found_diff=0
    
    # First run
    {{CLI}} --avro --map-threshold 0 --unify-maps --wrap-root claims --force-type mainsnak:record --no-unify qualifiers "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}.json" > "outL{{line}}_{{suffix}}_run1.json" 2> "errL{{line}}_{{suffix}}_run1.txt"
    
    # Check first run has no P-codes (successful unification)
    if rg -q '"name": "P' "outL{{line}}_{{suffix}}_run1.json"; then
        echo "    ✗ L{{line}} unification broken (P-codes found in output)"
        exit 1
    fi
    
    # Keep running until we find a difference or hit max_runs
    for i in $(seq 2 $max_runs); do
        {{CLI}} --avro --map-threshold 0 --unify-maps --wrap-root claims --force-type mainsnak:record --no-unify qualifiers "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}.json" > "outL{{line}}_{{suffix}}_run$i.json" 2> "errL{{line}}_{{suffix}}_run$i.txt"
        
        # Check this run also has no P-codes
        if rg -q '"name": "P' "outL{{line}}_{{suffix}}_run$i.json"; then
            echo "    ✗ L{{line}} unification broken (P-codes found in output)"
            exit 1
        fi
        
        # Compare with first run
        if ! diff -q "outL{{line}}_{{suffix}}_run1.json" "outL{{line}}_{{suffix}}_run$i.json" > /dev/null 2>&1; then
            echo "    ✓ L{{line}} non-determinism preserved with unification intact (difference found on run $i, no P-codes)"
            found_diff=1
            break
        fi
    done
    
    if [ $found_diff -eq 0 ]; then
        echo "    ✗ L{{line}} non-determinism lost (all $max_runs runs identical)"
        exit 1
    fi

# Apply P-code reduction with safety check
[working-directory: 'genson-cli']
reduce-pcodes line count suffix:
    #!/bin/bash
    set -euo pipefail
    if [ "{{count}}" -eq 0 ]; then
        echo '{}' > "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json"
    else
        jq "to_entries[0:{{count}}] | from_entries" "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}.json" > "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json"
    fi
    
    # Test temp file first
    {{CLI}} --avro --map-threshold 0 --unify-maps --wrap-root claims --force-type mainsnak:record --no-unify qualifiers "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json" > "outL{{line}}_{{suffix}}_test.json" 2> "errL{{line}}_{{suffix}}_test.txt"
    
    if rg -q 'name": "P' "outL{{line}}_{{suffix}}_test.json"; then
        mv "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json" "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}.json"
        rm -f "outL{{line}}_{{suffix}}_test.json" "errL{{line}}_{{suffix}}_test.txt"
        echo "    ✓ P-code reduction to {{count}} successful"
    else
        echo "    ✗ P-code reduction to {{count}} broke reproduction"
        rm -f "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json" "outL{{line}}_{{suffix}}_test.json" "errL{{line}}_{{suffix}}_test.txt"
        exit 1
    fi

# Apply claims reduction with safety check  
[working-directory: 'genson-cli']
reduce-claims line count suffix:
    #!/bin/bash
    set -euo pipefail
    if [ "{{count}}" -eq 0 ]; then
        jq 'with_entries(.value = [])' "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}.json" > "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json"
    else
        jq "with_entries(.value |= .[0:{{count}}])" "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}.json" > "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json"
    fi
    
    # Test temp file first
    {{CLI}} --avro --map-threshold 0 --unify-maps --wrap-root claims --force-type mainsnak:record --no-unify qualifiers "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json" > "outL{{line}}_{{suffix}}_test.json" 2> "errL{{line}}_{{suffix}}_test.txt"
    
    if rg -q 'name": "P' "outL{{line}}_{{suffix}}_test.json"; then
        mv "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json" "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}.json"
        rm -f "outL{{line}}_{{suffix}}_test.json" "errL{{line}}_{{suffix}}_test.txt"
        echo "    ✓ Claims reduction to {{count}} successful"
    else
        echo "    ✗ Claims reduction to {{count}} broke reproduction"
        rm -f "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json" "outL{{line}}_{{suffix}}_test.json" "errL{{line}}_{{suffix}}_test.txt"
        exit 1
    fi

# Apply skip-first reduction with safety check
[working-directory: 'genson-cli']
skip-first-pcodes line count suffix:
    #!/bin/bash
    set -euo pipefail
    if [ "{{count}}" -eq 0 ]; then
        echo "No P-codes to skip"
        exit 0
    fi
    
    jq "to_entries[{{count}}:] | from_entries" "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}.json" > "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json"
    
    # Test temp file first
    {{CLI}} --avro --map-threshold 0 --unify-maps --wrap-root claims --force-type mainsnak:record --no-unify qualifiers "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json" > "outL{{line}}_{{suffix}}_test.json" 2> "errL{{line}}_{{suffix}}_test.txt"
    
    if rg -q 'name": "P' "outL{{line}}_{{suffix}}_test.json"; then
        mv "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json" "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}.json"
        rm -f "outL{{line}}_{{suffix}}_test.json" "errL{{line}}_{{suffix}}_test.txt"
        echo "    ✓ Skip-first {{count}} successful"
    else
        echo "    ✗ Skip-first {{count}} broke reproduction"
        rm -f "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json" "outL{{line}}_{{suffix}}_test.json" "errL{{line}}_{{suffix}}_test.txt"
        exit 1
    fi

# Apply skip-last reduction with safety check
[working-directory: 'genson-cli']
skip-last-pcodes line count suffix:
    #!/bin/bash
    set -euo pipefail
    if [ "{{count}}" -eq 0 ]; then
        echo "No P-codes to skip"
        exit 0
    fi
    
    jq "to_entries[:-{{count}}] | from_entries" "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}.json" > "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json"
    
    # Test temp file first
    {{CLI}} --avro --map-threshold 0 --unify-maps --wrap-root claims --force-type mainsnak:record --no-unify qualifiers "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json" > "outL{{line}}_{{suffix}}_test.json" 2> "errL{{line}}_{{suffix}}_test.txt"
    
    if rg -q 'name": "P' "outL{{line}}_{{suffix}}_test.json"; then
        mv "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json" "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}.json"
        rm -f "outL{{line}}_{{suffix}}_test.json" "errL{{line}}_{{suffix}}_test.txt"
        echo "    ✓ Skip-last {{count}} successful"
    else
        echo "    ✗ Skip-last {{count}} broke reproduction"
        rm -f "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json" "outL{{line}}_{{suffix}}_test.json" "errL{{line}}_{{suffix}}_test.txt"
        exit 1
    fi

# Apply label reduction with safety check
[working-directory: 'genson-cli']
reduce-labels line count suffix:
    #!/bin/bash
    set -euo pipefail
    jq --argjson count {{count}} 'walk(if type == "object" and has("property-labels") and (."property-labels" | type == "object") then ."property-labels" = (."property-labels" | to_entries | .[0:$count] | from_entries) else . end) | walk(if type == "object" and has("labels") and (.labels | type == "object") then .labels = (.labels | to_entries | .[0:$count] | from_entries) else . end)' "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}.json" > "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json"
    
    # Test temp file first
    {{CLI}} --avro --map-threshold 0 --unify-maps --wrap-root claims --force-type mainsnak:record --no-unify qualifiers "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json" > "outL{{line}}_{{suffix}}_test.json" 2> "errL{{line}}_{{suffix}}_test.txt"
    
    if rg -q 'name": "P' "outL{{line}}_{{suffix}}_test.json"; then
        mv "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json" "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}.json"
        rm -f "outL{{line}}_{{suffix}}_test.json" "errL{{line}}_{{suffix}}_test.txt"
        echo "    ✓ Label reduction to {{count}} successful"
    else
        echo "    ✗ Label reduction to {{count}} broke reproduction"
        rm -f "{{DATA_DIR}}/x1818_L{{line}}_{{suffix}}_temp.json" "outL{{line}}_{{suffix}}_test.json" "errL{{line}}_{{suffix}}_test.txt"
        exit 1
    fi

# Binary search for maximum P-codes that preserve the bug
[working-directory: 'genson-cli']
find-pcode-limit line:
    #!/bin/bash
    set -euo pipefail
    echo "Finding P-code limit for L{{line}}..."
    
    # Get current P-code count
    current_count=$(jq 'keys | length' "{{DATA_DIR}}/x1818_L{{line}}_PENULTIMATE.json")
    low=1
    high=$current_count
    last_working=$current_count
    
    # Binary search for the minimum working value
    while [ $low -le $high ]; do
        mid=$(( (low + high) / 2 ))
        echo "  Testing $mid P-codes..."
        
        # Test on temp copy
        cp "{{DATA_DIR}}/x1818_L{{line}}_PENULTIMATE.json" "{{DATA_DIR}}/x1818_L{{line}}_temp.json"
        if just reduce-pcodes {{line}} $mid temp >/dev/null 2>&1; then
            last_working=$mid
            high=$((mid - 1))  # Try smaller values
        else
            low=$((mid + 1))   # This failed, try larger values
        fi
    done
    
    echo "{{line}}=$last_working" >> pcode_limits.txt
    echo "L{{line}} P-code limit: $last_working"
    
    # Apply the found limit to the PENULTIMATE file (skip for L26)
    if [ "{{line}}" != "26" ]; then
        just reduce-pcodes {{line}} $last_working PENULTIMATE
    else
        echo "  Skipping P-code application for L26 (collect limits only)"
    fi

# Binary search for maximum claims per P-code
[working-directory: 'genson-cli']
find-claims-limit line pcode_limit:
    #!/bin/bash
    set -euo pipefail
    echo "Finding claims limit for L{{line}}..."
    
    # Get max claims count from current state
    max_claims=$(jq '[.[] | length] | max' "{{DATA_DIR}}/x1818_L{{line}}_PENULTIMATE.json")
    low=1
    high=$max_claims
    last_working=$max_claims
    
    # Binary search for the minimum working value
    while [ $low -le $high ]; do
        mid=$(( (low + high) / 2 ))
        echo "  Testing $mid claims per P-code..."
        
        # Test on temp copy
        cp "{{DATA_DIR}}/x1818_L{{line}}_PENULTIMATE.json" "{{DATA_DIR}}/x1818_L{{line}}_temp.json"
        if just reduce-claims {{line}} $mid temp >/dev/null 2>&1; then
            last_working=$mid
            high=$((mid - 1))  # Try smaller values
        else
            low=$((mid + 1))   # This failed, try larger values
        fi
    done
    
    echo "{{line}}=$last_working" >> claims_limits.txt
    echo "L{{line}} claims limit: $last_working"
    
    # Apply the found limit to the PENULTIMATE file (skip for L26)
    if [ "{{line}}" != "26" ]; then
        just reduce-claims {{line}} $last_working PENULTIMATE
    else
        echo "  Skipping claims application for L26 (collect limits only)"
    fi

# Binary search for skip-first limit
[working-directory: 'genson-cli']
find-skip-first-limit line pcode_limit claims_limit:
    #!/bin/bash
    set -euo pipefail
    echo "Finding skip-first limit for L{{line}}..."
    
    current_count=$(jq 'keys | length' "{{DATA_DIR}}/x1818_L{{line}}_PENULTIMATE.json")
    low=0
    high=$((current_count - 1))
    last_working=0
    
    # Binary search for the maximum working value (we want to skip as many as possible)
    while [ $low -le $high ]; do
        mid=$(( (low + high) / 2 ))
        echo "  Testing skip-first $mid..."
        
        # Test on temp copy
        cp "{{DATA_DIR}}/x1818_L{{line}}_PENULTIMATE.json" "{{DATA_DIR}}/x1818_L{{line}}_temp.json"
        if just skip-first-pcodes {{line}} $mid temp >/dev/null 2>&1; then
            last_working=$mid
            low=$((mid + 1))   # Try skipping more
        else
            high=$((mid - 1))  # This failed, try skipping fewer
        fi
    done
    
    echo "{{line}}=$last_working" >> skip_first_limits.txt
    echo "L{{line}} skip-first limit: $last_working"
    
    # Apply the found limit to the PENULTIMATE file (skip for L26)
    if [ "{{line}}" != "26" ]; then
        just skip-first-pcodes {{line}} $last_working PENULTIMATE
    else
        echo "  Skipping skip-first application for L26 (collect limits only)"
    fi

# Binary search for skip-last limit (works on current cumulative state)
[working-directory: 'genson-cli']
find-skip-last-limit line pcode_limit claims_limit skip_first_limit:
    #!/bin/bash
    set -euo pipefail
    echo "Finding skip-last limit for L{{line}} (on current cumulative state)..."
    
    # Work on the current PENULTIMATE file which already has previous reductions
    current_count=$(jq 'keys | length' "{{DATA_DIR}}/x1818_L{{line}}_PENULTIMATE.json")
    
    if [ $current_count -le 1 ]; then
        echo "Only $current_count P-codes remaining, cannot skip any more"
        echo "{{line}}=0" >> skip_last_limits.txt
        echo "L{{line}} skip-last limit: 0"
        return 0
    fi
    
    low=0
    high=$((current_count - 1))
    last_working=0
    
    # Binary search for the maximum working value
    while [ $low -le $high ]; do
        mid=$(( (low + high) / 2 ))
        echo "  Testing skip-last $mid (on $current_count P-codes)..."
        
        # Create backup of current state
        cp "{{DATA_DIR}}/x1818_L{{line}}_PENULTIMATE.json" "{{DATA_DIR}}/x1818_L{{line}}_backup.json"
        
        # Test the reduction directly on PENULTIMATE
        if just skip-last-pcodes {{line}} $mid PENULTIMATE >/dev/null 2>&1; then
            last_working=$mid
            low=$((mid + 1))   # Try skipping more
        else
            # Restore backup and try fewer
            cp "{{DATA_DIR}}/x1818_L{{line}}_backup.json" "{{DATA_DIR}}/x1818_L{{line}}_PENULTIMATE.json"
            high=$((mid - 1))
        fi
    done
    
    # Apply the final working limit
    if [ $last_working -gt 0 ]; then
        cp "{{DATA_DIR}}/x1818_L{{line}}_backup.json" "{{DATA_DIR}}/x1818_L{{line}}_PENULTIMATE.json"
        just skip-last-pcodes {{line}} $last_working PENULTIMATE
    fi
    
    rm -f "{{DATA_DIR}}/x1818_L{{line}}_backup.json"
    
    echo "{{line}}=$last_working" >> skip_last_limits.txt
    echo "L{{line}} skip-last limit: $last_working"
    
    # Skip application for L26 (it's already applied above)
    if [ "{{line}}" = "26" ]; then
        echo "  L26 skip-last limit applied: $last_working"
    fi

# Find label reduction limit
[working-directory: 'genson-cli']
find-label-limit line pcode_limit claims_limit skip_first_limit skip_last_limit:
    #!/bin/bash
    set -euo pipefail
    echo "Finding label limit for L{{line}}..."
    
    # Try different label counts
    for count in 2 10 20 40 100; do
        echo "  Testing $count label keys..."
        
        # Test on temp copy
        cp "{{DATA_DIR}}/x1818_L{{line}}_PENULTIMATE.json" "{{DATA_DIR}}/x1818_L{{line}}_temp.json"
        if just reduce-labels {{line}} $count temp >/dev/null 2>&1; then
            echo "{{line}}=$count" >> label_limits.txt
            echo "L{{line}} label limit: $count"
            
            # Apply the found limit to the PENULTIMATE file (skip for L26)
            if [ "{{line}}" != "26" ]; then
                just reduce-labels {{line}} $count PENULTIMATE
            else
                echo "  Skipping label application for L26 (collect limits only)"
            fi
            exit 0
        fi
    done
    
    echo "{{line}}=NONE" >> label_limits.txt
    echo "L{{line}} label limit: NONE"

# Test all final files
[working-directory: 'genson-cli']
test-all:
    #!/bin/bash
    set -euo pipefail
    echo "Testing all final reductions..."
    for line in {{WORKING_LINES}}; do
        echo "Testing L$line..."
        just test-repro $line
    done

# Clean temporary files
[working-directory: 'genson-cli']
clean-tmp:
    rm -f {{DATA_DIR}}/x1818_L*_temp*.json
    rm -f outL*_temp*.json errL*_temp*.txt
    rm -f outL*_test.json errL*_test.txt

# Restore original files
[working-directory: 'genson-cli']
restore:
    #!/bin/bash
    set -euo pipefail
    for line in {{WORKING_LINES}}; do
        echo "Restoring L$line..."
        cp "{{DATA_DIR}}/x1818_L${line}.json" "{{DATA_DIR}}/x1818_L${line}_PENULTIMATE.json"
    done

# Main workflow - find all limits and apply them cumulatively
[working-directory: 'genson-cli']
find-all-limits:
    #!/bin/bash
    set -euo pipefail
    echo "=== FINDING OPTIMAL REDUCTION PARAMETERS ==="
    
    # Clear existing limit files
    rm -f *_limits.txt
    
    # Step 1: Find P-code limits
    echo -e "\nStep 1: Finding P-code limits..."
    for line in {{WORKING_LINES}}; do
        just find-pcode-limit $line
    done
    
    # Step 2: Find claims limits  
    echo -e "\nStep 2: Finding claims limits..."
    for line in {{WORKING_LINES}}; do
        pcode_limit=$(grep "^${line}=" pcode_limits.txt | cut -d'=' -f2)
        just find-claims-limit $line $pcode_limit
    done
    
    # Step 3: Find skip-first limits
    echo -e "\nStep 3: Finding skip-first limits..."
    for line in {{WORKING_LINES}}; do
        pcode_limit=$(grep "^${line}=" pcode_limits.txt | cut -d'=' -f2)
        claims_limit=$(grep "^${line}=" claims_limits.txt | cut -d'=' -f2)
        just find-skip-first-limit $line $pcode_limit $claims_limit
    done
    
    # Step 4: Find skip-last limits
    echo -e "\nStep 4: Finding skip-last limits..."
    for line in {{WORKING_LINES}}; do
        pcode_limit=$(grep "^${line}=" pcode_limits.txt | cut -d'=' -f2)
        claims_limit=$(grep "^${line}=" claims_limits.txt | cut -d'=' -f2)
        skip_first_limit=$(grep "^${line}=" skip_first_limits.txt | cut -d'=' -f2)
        just find-skip-last-limit $line $pcode_limit $claims_limit $skip_first_limit
    done
    
    # Step 5: Find label limits
    echo -e "\nStep 5: Finding label limits..."
    for line in {{WORKING_LINES}}; do
        pcode_limit=$(grep "^${line}=" pcode_limits.txt | cut -d'=' -f2)
        claims_limit=$(grep "^${line}=" claims_limits.txt | cut -d'=' -f2)
        skip_first_limit=$(grep "^${line}=" skip_first_limits.txt | cut -d'=' -f2)
        skip_last_limit=$(grep "^${line}=" skip_last_limits.txt | cut -d'=' -f2)
        just find-label-limit $line $pcode_limit $claims_limit $skip_first_limit $skip_last_limit
    done
    
    just clean-tmp
    echo -e "\n=== VERIFICATION ==="
    just test-all
    echo -e "\nAll limits found and applied successfully!"

# Show current limits
[working-directory: 'genson-cli']
show-limits:
    @echo "Current limits:"
    @echo "P-code limits:"; cat pcode_limits.txt 2>/dev/null || echo "  None found"
    @echo "Claims limits:"; cat claims_limits.txt 2>/dev/null || echo "  None found" 
    @echo "Skip-first limits:"; cat skip_first_limits.txt 2>/dev/null || echo "  None found"
    @echo "Skip-last limits:"; cat skip_last_limits.txt 2>/dev/null || echo "  None found"
    @echo "Label limits:"; cat label_limits.txt 2>/dev/null || echo "  None found"
