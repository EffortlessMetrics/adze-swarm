#!/bin/bash

# Script to clean up debug print statements in runtime code
# These should be converted to proper logging or removed

echo "Cleaning up debug prints in runtime source files..."

# Files to clean
FILES=(
    "runtime/src/decoder.rs"
    "runtime/src/parser_v4.rs"
    "runtime/src/pure_parser.rs"
    "runtime/src/scanner_registry.rs"
    "runtime/src/query/compiler.rs"
    "runtime/src/query/predicate_eval.rs"
)

for file in "${FILES[@]}"; do
    if [ -f "$file" ]; then
        echo "Processing $file..."
        # Comment out eprintln! and println! statements
        sed -i.bak 's/^\([[:space:]]*\)eprintln!/\1\/\/ eprintln!/g' "$file"
        sed -i.bak 's/^\([[:space:]]*\)println!/\1\/\/ println!/g' "$file"
        # Remove backup files
        rm "${file}.bak"
    fi
done

echo "Debug prints have been commented out."
echo "To re-enable for debugging, uncomment the relevant lines."