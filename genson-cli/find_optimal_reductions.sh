#!/bin/bash

# Automated reduction parameter finder
# Systematically finds optimal settings for each reduction step

set -e

LINES=(4 5 12 14 16 26)

# Test if a configuration preserves the bug (returns 0 if bug present, 1 if not)
test_configuration() {
    local line=$1
    local temp_suffix=$2
    
    ../target/debug/genson-cli --avro --map-threshold 0 --unify-maps --wrap-root claims \
        "tests/data/claims/x1818_L${line}_${temp_suffix}.json" \
        > "outL${line}_${temp_suffix}.json" 2> "errL${line}_${temp_suffix}.txt"
    
    if rg -q 'name": "P' "outL${line}_${temp_suffix}.json"; then
        return 0  # Bug present
    else
        return 1  # Bug absent
    fi
}

# Binary search to find maximum working value
binary_search_max() {
    local line=$1
    local temp_suffix=$2
    local min_val=$3
    local max_val=$4
    local apply_func=$5
    
    while [ $((max_val - min_val)) -gt 1 ]; do
        local mid=$(( (min_val + max_val) / 2 ))
        
        # Restore and apply reduction
        cp "tests/data/claims/x1818_L${line}_PENULTIMATE.json" \
           "tests/data/claims/x1818_L${line}_${temp_suffix}.json"
        
        # Apply the reduction function
        case "$apply_func" in
            "reduce_pcodes")
                reduce_pcodes "$line" "$mid" "$temp_suffix"
                ;;
            "reduce_claims_per_pcode")
                reduce_claims_per_pcode "$line" "$mid" "$temp_suffix"
                ;;
            "skip_first_pcodes")
                skip_first_pcodes "$line" "$mid" "$temp_suffix"
                ;;
            "skip_last_pcodes")
                skip_last_pcodes "$line" "$mid" "$temp_suffix"
                ;;
        esac
        
        if test_configuration "$line" "$temp_suffix"; then
            min_val=$mid
        else
            max_val=$mid
        fi
        
        echo "  L$line: tested $mid, range now [$min_val, $max_val]" >&2
    done
    
    echo $min_val
}

# Reduction functions
reduce_pcodes() {
    local line=$1
    local count=$2
    local suffix=$3
    
    if [ "$count" -eq 0 ]; then
        # If count is 0, create empty object
        echo '{}' > "tests/data/claims/x1818_L${line}_${suffix}.json"
    else
        jq "to_entries[0:$count] | from_entries" \
            "tests/data/claims/x1818_L${line}_${suffix}.json" > temp.json && \
            mv temp.json "tests/data/claims/x1818_L${line}_${suffix}.json"
    fi
}

reduce_claims_per_pcode() {
    local line=$1
    local count=$2
    local suffix=$3
    
    if [ "$count" -eq 0 ]; then
        # If count is 0, make all arrays empty
        jq 'with_entries(.value = [])' \
            "tests/data/claims/x1818_L${line}_${suffix}.json" > temp.json && \
            mv temp.json "tests/data/claims/x1818_L${line}_${suffix}.json"
    else
        jq "with_entries(.value |= .[0:$count])" \
            "tests/data/claims/x1818_L${line}_${suffix}.json" > temp.json && \
            mv temp.json "tests/data/claims/x1818_L${line}_${suffix}.json"
    fi
}

skip_first_pcodes() {
    local line=$1
    local count=$2
    local suffix=$3
    
    if [ "$count" -eq 0 ]; then
        # If count is 0, don't skip anything (no-op)
        return 0
    else
        jq "to_entries[$count:] | from_entries" \
            "tests/data/claims/x1818_L${line}_${suffix}.json" > temp.json && \
            mv temp.json "tests/data/claims/x1818_L${line}_${suffix}.json"
    fi
}

skip_last_pcodes() {
    local line=$1
    local count=$2
    local suffix=$3
    
    if [ "$count" -eq 0 ]; then
        # If count is 0, don't skip anything (no-op)
        return 0
    else
        jq "to_entries[:-$count] | from_entries" \
            "tests/data/claims/x1818_L${line}_${suffix}.json" > temp.json && \
            mv temp.json "tests/data/claims/x1818_L${line}_${suffix}.json"
    fi
}

# Load or initialize results
load_results() {
    declare -gA PCODE_LIMITS CLAIMS_LIMITS SKIP_FIRST_LIMITS SKIP_LAST_LIMITS
    
    if [ -f "pcode_limits.txt" ]; then
        echo "Loading P-code limits from pcode_limits.txt"
        while IFS='=' read -r key value; do
            PCODE_LIMITS[$key]=$value
        done < pcode_limits.txt
    fi
    
    if [ -f "claims_limits.txt" ]; then
        echo "Loading claims limits from claims_limits.txt"
        while IFS='=' read -r key value; do
            CLAIMS_LIMITS[$key]=$value
        done < claims_limits.txt
    fi
    
    if [ -f "skip_first_limits.txt" ]; then
        echo "Loading skip-first limits from skip_first_limits.txt"
        while IFS='=' read -r key value; do
            SKIP_FIRST_LIMITS[$key]=$value
        done < skip_first_limits.txt
    fi
    
    if [ -f "skip_last_limits.txt" ]; then
        echo "Loading skip-last limits from skip_last_limits.txt"
        while IFS='=' read -r key value; do
            SKIP_LAST_LIMITS[$key]=$value
        done < skip_last_limits.txt
    fi
}

save_pcode_limits() {
    echo "Saving P-code limits to pcode_limits.txt"
    for line in "${WORKING_LINES[@]}"; do
        echo "$line=${PCODE_LIMITS[$line]}"
    done > pcode_limits.txt
}

save_claims_limits() {
    echo "Saving claims limits to claims_limits.txt"
    for line in "${WORKING_LINES[@]}"; do
        echo "$line=${CLAIMS_LIMITS[$line]}"
    done > claims_limits.txt
}

save_skip_first_limits() {
    echo "Saving skip-first limits to skip_first_limits.txt"
    for line in "${WORKING_LINES[@]}"; do
        echo "$line=${SKIP_FIRST_LIMITS[$line]}"
    done > skip_first_limits.txt
}

save_skip_last_limits() {
    echo "Saving skip-last limits to skip_last_limits.txt"
    for line in "${WORKING_LINES[@]}"; do
        echo "$line=${SKIP_LAST_LIMITS[$line]}"
    done > skip_last_limits.txt
}

# Main optimization process
echo "=== AUTOMATED REDUCTION PARAMETER FINDING ==="

# Skip L16 since it doesn't have the bug in baseline
WORKING_LINES=(4 5 12 14 26)

load_results

if [ ! -f "pcode_limits.txt" ]; then
    echo "Step 1: Finding P-code count limits..."
    for line in "${WORKING_LINES[@]}"; do
        echo "Finding P-code limit for L$line..."
        # Get current P-code count
        current_count=$(jq 'keys | length' "tests/data/claims/x1818_L${line}_PENULTIMATE.json")
        max_limit=$(binary_search_max "$line" "temp_pcode" 1 "$current_count" reduce_pcodes)
        PCODE_LIMITS[$line]=$max_limit
        echo "L$line P-code limit: $max_limit"
    done
    save_pcode_limits
else
    echo "Step 1: P-code limits already found, skipping..."
fi

if [ ! -f "claims_limits.txt" ]; then
    echo -e "\nStep 2: Finding claims-per-P-code limits..."
    for line in "${WORKING_LINES[@]}"; do
        echo "Finding claims limit for L$line..."
        # Apply P-code limit first, then find claims limit
        cp "tests/data/claims/x1818_L${line}_PENULTIMATE.json" \
           "tests/data/claims/x1818_L${line}_temp_claims.json"
        reduce_pcodes "$line" "${PCODE_LIMITS[$line]}" "temp_claims"
        
        # Get max claims count from any P-code to set upper bound
        max_claims=$(jq '[.[] | length] | max' "tests/data/claims/x1818_L${line}_temp_claims.json")
        max_limit=$(binary_search_max "$line" "temp_claims" 1 "$max_claims" reduce_claims_per_pcode)
        CLAIMS_LIMITS[$line]=$max_limit
        echo "L$line claims-per-P-code limit: $max_limit"
    done
    save_claims_limits
else
    echo "Step 2: Claims limits already found, skipping..."
fi

if [ ! -f "skip_first_limits.txt" ]; then
    echo -e "\nStep 3: Finding skip-first limits..."
    for line in "${WORKING_LINES[@]}"; do
        echo "Finding skip-first limit for L$line..."
        # Apply previous limits first
        cp "tests/data/claims/x1818_L${line}_PENULTIMATE.json" \
           "tests/data/claims/x1818_L${line}_temp_skip_first.json"
        reduce_pcodes "$line" "${PCODE_LIMITS[$line]}" "temp_skip_first"
        reduce_claims_per_pcode "$line" "${CLAIMS_LIMITS[$line]}" "temp_skip_first"
        
        max_limit=$(binary_search_max "$line" "temp_skip_first" 0 $((${PCODE_LIMITS[$line]} - 1)) skip_first_pcodes)
        SKIP_FIRST_LIMITS[$line]=$max_limit
        echo "L$line skip-first limit: $max_limit"
    done
    save_skip_first_limits
else
    echo "Step 3: Skip-first limits already found, skipping..."
fi

if [ ! -f "skip_last_limits.txt" ]; then
    echo -e "\nStep 4: Finding skip-last limits..."
    for line in "${WORKING_LINES[@]}"; do
        echo "Finding skip-last limit for L$line..."
        # Apply all previous limits first
        cp "tests/data/claims/x1818_L${line}_PENULTIMATE.json" \
           "tests/data/claims/x1818_L${line}_temp_skip_last.json"
        reduce_pcodes "$line" "${PCODE_LIMITS[$line]}" "temp_skip_last"
        reduce_claims_per_pcode "$line" "${CLAIMS_LIMITS[$line]}" "temp_skip_last"
        skip_first_pcodes "$line" "${SKIP_FIRST_LIMITS[$line]}" "temp_skip_last"
        
        remaining_count=$(jq 'keys | length' "tests/data/claims/x1818_L${line}_temp_skip_last.json")
        max_limit=$(binary_search_max "$line" "temp_skip_last" 0 $((remaining_count - 1)) skip_last_pcodes)
        SKIP_LAST_LIMITS[$line]=$max_limit
        echo "L$line skip-last limit: $max_limit"
    done
    save_skip_last_limits
else
    echo "Step 4: Skip-last limits already found, skipping..."
fi

echo -e "\n=== OPTIMAL PARAMETERS FOUND ==="
echo "declare -A PCODE_LIMITS"
for line in "${WORKING_LINES[@]}"; do
    echo "PCODE_LIMITS[$line]=${PCODE_LIMITS[$line]}"
done

echo -e "\ndeclare -A CLAIMS_LIMITS"  
for line in "${WORKING_LINES[@]}"; do
    echo "CLAIMS_LIMITS[$line]=${CLAIMS_LIMITS[$line]}"
done

echo -e "\ndeclare -A SKIP_FIRST_LIMITS"
for line in "${WORKING_LINES[@]}"; do
    echo "SKIP_FIRST_LIMITS[$line]=${SKIP_FIRST_LIMITS[$line]}"
done

echo -e "\ndeclare -A SKIP_LAST_LIMITS"
for line in "${WORKING_LINES[@]}"; do
    echo "SKIP_LAST_LIMITS[$line]=${SKIP_LAST_LIMITS[$line]}"
done

echo -e "\n=== APPLYING FINAL REDUCTIONS ==="

for line in "${WORKING_LINES[@]}"; do
    echo "Overwriting x1818_L${line}_PENULTIMATE.json with reduced version..."
    reduce_pcodes "$line" "${PCODE_LIMITS[$line]}" "PENULTIMATE"
    reduce_claims_per_pcode "$line" "${CLAIMS_LIMITS[$line]}" "PENULTIMATE"
    skip_first_pcodes "$line" "${SKIP_FIRST_LIMITS[$line]}" "PENULTIMATE"
    skip_last_pcodes "$line" "${SKIP_LAST_LIMITS[$line]}" "PENULTIMATE"
done

# Cleanup temp files
rm -f tests/data/claims/x1818_L*_temp*.json
rm -f outL*_temp*.json errL*_temp*.txt

echo -e "\nAutomated parameter finding complete!"
