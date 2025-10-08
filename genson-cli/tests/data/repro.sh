#!/bin/bash
set -e

# Start fresh - restore the file with 7 P-codes already removed
jq '.' claims_fixture_x4_L1_min.jsonl > claims/x1818_L1_PENULTIMATE.json
jq 'del(.P8895, .P7704, .P735, .P734, .P7047, .P69, .P6839)' claims/x1818_L1_PENULTIMATE.json > claims/x1818_L1_test.json
git add claims/x1818_L1_test.json

# P-codes currently in test.json after removing 7
pcodes=(P10291 P106 P10757 P108 P1412 P1417 P1441 P1448 P166 P175 P18 P19 P21 P22 P2581 P26 P27 P31 P3373 P3417 P345 P3553 P40 P410 P451 P463 P4839 P512 P569 P570 P5800 P6262 P646)

for pcode in "${pcodes[@]}"; do
    echo "Testing individual removal of: $pcode"
    
    # Try removing JUST this one P-code from current state
    jq "del(.$pcode)" claims/x1818_L1_test.json > claims/x1818_L1_test_temp.json
    mv claims/x1818_L1_test_temp.json claims/x1818_L1_test.json
    
    if just test-repro 1 test; then
        echo "✓ Removed $pcode successfully"
        git add claims/x1818_L1_test.json
    else
        echo "✗ Cannot remove $pcode - needed for repro"
        git restore claims/x1818_L1_test.json
    fi
done

echo "Done! Final minimal set:"
jq 'keys | length' claims/x1818_L1_test.json
jq 'keys' claims/x1818_L1_test.json
