# Dynamic Loading Design Sketch

This guide records practical scenarios intended for adze's dynamic loading
feature.

> **Current status:** dynamic loading is not a supported parse-output path in
> the current CLI. `adze parse --dynamic ...` can be compiled behind the
> `dynamic` feature and can attempt to load a symbol, but it still exits with
> `dynamic parse mode is currently unimplemented`. Treat the examples below as
> design sketches until this surface is promoted with behavior tests.

## Prerequisites

Before using dynamic loading, ensure you have:

1. **CLI with Dynamic Support**: this is a release-surface command shape until
   `adze-cli` has a crates.io install receipt. For current repo proof, build
   the CLI from this checkout.
   ```bash
   cargo install adze-cli --features dynamic
   ```

2. **Tree-sitter Grammar Libraries**: Compiled `.so/.dylib/.dll` files
3. **Input Files**: Files to parse in the target language

## Finding Grammar Libraries

### System Package Managers

**Ubuntu/Debian:**
```bash
# Install common grammars
sudo apt install libtree-sitter-json-dev libtree-sitter-python-dev

# Find installed libraries
find /usr/lib -name "libtree-sitter-*.so" 2>/dev/null
```

**macOS (Homebrew):**
```bash
# Install grammars
brew install tree-sitter

# Install language-specific grammars
npm install tree-sitter-json tree-sitter-python

# Find libraries
find /opt/homebrew/lib -name "libtree-sitter-*.dylib" 2>/dev/null
```

**Fedora/CentOS:**
```bash
# Install development packages
sudo dnf install tree-sitter-devel

# Find libraries
find /usr/lib64 -name "libtree-sitter-*.so" 2>/dev/null
```

### Building from Source

```bash
# Clone a Tree-sitter grammar
git clone https://github.com/tree-sitter/tree-sitter-json
cd tree-sitter-json

# Build shared library
make
# Creates libtree-sitter-json.so

# Or use Node.js approach
npm install
node-gyp build
# Creates build/Release/tree_sitter_json_binding.node
```

### Node.js Installations

```bash
# Install via npm
npm install tree-sitter-json tree-sitter-python

# Find compiled libraries
find node_modules -name "*.node" -o -name "*.so" | grep tree_sitter
```

## Common Use Cases

### 1. Quick File Analysis

**Scenario**: You want to quickly analyze a JSON file's structure.

```bash
# Parse and get basic statistics
adze parse --dynamic /usr/lib/libtree-sitter-json.so data.json

# Output JSON for further processing
adze parse --dynamic /usr/lib/libtree-sitter-json.so data.json --format json > analysis.json

# Process the results
cat analysis.json | jq '.nodes' # Get node count
```

**Example Output:**
```
✓ Loaded language from: /usr/lib/libtree-sitter-json.so
Input size: 1024 bytes
Parsed successfully. Root symbol: document, nodes: 47
```

### 2. Batch Processing

**Scenario**: Process multiple Python files in a directory.

```bash
#!/bin/bash
# batch_parse.sh

PYTHON_LIB="/usr/lib/libtree-sitter-python.so"
OUTPUT_DIR="parse_results"

mkdir -p "$OUTPUT_DIR"

for py_file in *.py; do
    echo "Processing $py_file..."
    
    result_file="$OUTPUT_DIR/${py_file%.py}.json"
    adze parse --dynamic "$PYTHON_LIB" "$py_file" --format json > "$result_file"
    
    # Check if parsing was successful
    status=$(jq -r '.status' "$result_file" 2>/dev/null || echo "error")
    
    if [ "$status" = "ok" ]; then
        nodes=$(jq -r '.nodes' "$result_file")
        echo "  ✅ Success: $nodes nodes"
    else
        echo "  ❌ Failed to parse"
    fi
done

echo "Batch processing complete. Results in $OUTPUT_DIR/"
```

### 3. Language Detection

**Scenario**: Determine the language of unknown source files.

```bash
#!/bin/bash
# detect_language.sh

detect_language() {
    local file="$1"
    local best_score=0
    local best_lang=""
    
    # Try different grammar libraries
    for lib_path in /usr/lib/libtree-sitter-*.so; do
        if [ -f "$lib_path" ]; then
            lang_name=$(basename "$lib_path" .so | sed 's/libtree-sitter-//')
            
            # Parse with this grammar
            result=$(adze parse --dynamic "$lib_path" "$file" --format json 2>/dev/null)
            
            if echo "$result" | jq -e '.status == "ok"' >/dev/null 2>&1; then
                nodes=$(echo "$result" | jq -r '.nodes // 0')
                
                # Higher node count often indicates better parse
                if [ "$nodes" -gt "$best_score" ]; then
                    best_score="$nodes"
                    best_lang="$lang_name"
                fi
            fi
        fi
    done
    
    if [ -n "$best_lang" ]; then
        echo "Best match: $best_lang ($best_score nodes)"
    else
        echo "No grammar successfully parsed this file"
    fi
}

detect_language "$1"
```

### 4. Syntax Validation

**Scenario**: Validate syntax of configuration files in CI/CD.

```bash
#!/bin/bash
# validate_syntax.sh - CI/CD script

set -e

validate_json_files() {
    local json_lib="/usr/lib/libtree-sitter-json.so"
    local failed=0
    
    for json_file in config/*.json; do
        if [ -f "$json_file" ]; then
            echo "Validating $json_file..."
            
            result=$(adze parse --dynamic "$json_lib" "$json_file" --format json)
            status=$(echo "$result" | jq -r '.status')
            
            if [ "$status" != "ok" ]; then
                echo "❌ $json_file has syntax errors"
                failed=1
            else
                echo "✅ $json_file is valid"
            fi
        fi
    done
    
    return $failed
}

validate_python_files() {
    local python_lib="/usr/lib/libtree-sitter-python.so"
    local failed=0
    
    for py_file in scripts/*.py; do
        if [ -f "$py_file" ]; then
            echo "Validating $py_file..."
            
            result=$(adze parse --dynamic "$python_lib" "$py_file" --format json)
            status=$(echo "$result" | jq -r '.status')
            
            if [ "$status" != "ok" ]; then
                echo "❌ $py_file has syntax errors"
                failed=1
            else
                echo "✅ $py_file is valid"
            fi
        fi
    done
    
    return $failed
}

echo "🔍 Starting syntax validation..."

if validate_json_files && validate_python_files; then
    echo "✅ All files passed validation"
    exit 0
else
    echo "❌ Some files failed validation"
    exit 1
fi
```

### 5. Code Metrics Collection

**Scenario**: Collect metrics about codebase complexity.

```bash
#!/bin/bash
# code_metrics.sh

collect_metrics() {
    local grammar_lib="$1"
    local file_pattern="$2"
    local language="$3"
    
    local total_files=0
    local total_nodes=0
    local total_size=0
    local failed_files=0
    
    echo "Analyzing $language files..."
    
    for file in $file_pattern; do
        if [ -f "$file" ]; then
            total_files=$((total_files + 1))
            file_size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null || echo 0)
            total_size=$((total_size + file_size))
            
            result=$(adze parse --dynamic "$grammar_lib" "$file" --format json 2>/dev/null)
            
            if echo "$result" | jq -e '.status == "ok"' >/dev/null 2>&1; then
                nodes=$(echo "$result" | jq -r '.nodes // 0')
                total_nodes=$((total_nodes + nodes))
            else
                failed_files=$((failed_files + 1))
            fi
        fi
    done
    
    if [ $total_files -gt 0 ]; then
        avg_nodes=$((total_nodes / total_files))
        avg_size=$((total_size / total_files))
        success_rate=$(((total_files - failed_files) * 100 / total_files))
        
        echo "📊 $language Metrics:"
        echo "  Files: $total_files"
        echo "  Total nodes: $total_nodes"
        echo "  Average nodes per file: $avg_nodes"
        echo "  Average file size: $avg_size bytes"
        echo "  Parse success rate: $success_rate%"
        echo
    fi
}

# Collect metrics for different languages
collect_metrics "/usr/lib/libtree-sitter-python.so" "src/**/*.py" "Python"
collect_metrics "/usr/lib/libtree-sitter-javascript.so" "src/**/*.js" "JavaScript"
collect_metrics "/usr/lib/libtree-sitter-json.so" "config/*.json" "JSON"
```

### 6. Documentation Generation

**Scenario**: Extract structure information for documentation tools.

```bash
#!/bin/bash
# extract_structure.sh

extract_python_classes() {
    local python_file="$1"
    local python_lib="/usr/lib/libtree-sitter-python.so"
    
    echo "Analyzing $python_file..."
    
    # Parse the file and output detailed structure
    adze parse --dynamic "$python_lib" "$python_file" --format sexp > /tmp/parse_tree.sexp
    
    # Extract class and function names (would need additional processing)
    echo "Parse tree saved to /tmp/parse_tree.sexp"
    
    # Basic statistics
    result=$(adze parse --dynamic "$python_lib" "$python_file" --format json)
    nodes=$(echo "$result" | jq -r '.nodes // 0')
    
    echo "  Structure complexity: $nodes nodes"
}

# Process all Python files
for py_file in src/*.py; do
    extract_python_classes "$py_file"
done
```

### 7. Editor Integration

**Scenario**: VS Code task for quick syntax checking.

```json
// .vscode/tasks.json
{
    "version": "2.0.0",
    "tasks": [
        {
            "label": "Parse Current File",
            "type": "shell",
            "command": "adze",
            "args": [
                "parse",
                "--dynamic",
                "${config:adze.grammarPath}",
                "${file}",
                "--format",
                "json"
            ],
            "group": "build",
            "presentation": {
                "echo": true,
                "reveal": "always",
                "focus": false,
                "panel": "shared"
            },
            "problemMatcher": []
        },
        {
            "label": "Validate Project Files",
            "type": "shell",
            "command": "bash",
            "args": [
                "-c",
                "find src -name '*.py' -exec adze parse --dynamic /usr/lib/libtree-sitter-python.so {} --format json \\; | jq -r 'select(.status != \"ok\") | \"Error in file\"'"
            ],
            "group": "build"
        }
    ]
}
```

```json
// .vscode/settings.json
{
    "adze.grammarPath": "/usr/lib/libtree-sitter-python.so"
}
```

## Advanced Techniques

### Custom Symbol Names

Some grammars use non-standard symbol names:

```bash
# Find symbol names in a library
nm -D /path/to/library.so | grep -i tree_sitter

# Try common alternatives
adze parse --dynamic lib.so input.txt --symbol language
adze parse --dynamic lib.so input.txt --symbol get_language
adze parse --dynamic lib.so input.txt --symbol tree_sitter_lang_language
```

### Performance Monitoring

Track parsing performance across large codebases:

```bash
#!/bin/bash
# performance_test.sh

GRAMMAR_LIB="$1"
TEST_DIR="$2"

echo "timestamp,file,size_bytes,nodes,parse_time_ms" > performance.csv

for file in "$TEST_DIR"/*; do
    if [ -f "$file" ]; then
        size=$(stat -c%s "$file" 2>/dev/null || stat -f%z "$file")
        start_time=$(date +%s%3N)
        
        result=$(adze parse --dynamic "$GRAMMAR_LIB" "$file" --format json)
        
        end_time=$(date +%s%3N)
        parse_time=$((end_time - start_time))
        
        nodes=$(echo "$result" | jq -r '.nodes // 0')
        timestamp=$(date -Iseconds)
        
        echo "$timestamp,$(basename "$file"),$size,$nodes,$parse_time" >> performance.csv
    fi
done

echo "Performance data saved to performance.csv"
```

### Error Analysis

Analyze parsing failures to improve grammar coverage:

```bash
#!/bin/bash
# error_analysis.sh

GRAMMAR_LIB="$1"
FILE_PATTERN="$2"

echo "file,status,error_details" > errors.csv

for file in $FILE_PATTERN; do
    if [ -f "$file" ]; then
        result=$(adze parse --dynamic "$GRAMMAR_LIB" "$file" --format json 2>&1)
        
        if echo "$result" | jq -e '.status == "ok"' >/dev/null 2>&1; then
            echo "$(basename "$file"),ok," >> errors.csv
        else
            # Extract error information
            error_msg=$(echo "$result" | jq -r '.message // "Parse failed"' | tr ',' ';')
            echo "$(basename "$file"),error,$error_msg" >> errors.csv
        fi
    fi
done

echo "Error analysis saved to errors.csv"

# Show summary
echo -e "\n📊 Error Summary:"
awk -F, '$2=="error" {print $3}' errors.csv | sort | uniq -c | sort -nr
```

## Troubleshooting

### Common Issues and Solutions

**Library Not Found:**
```bash
# Check if file exists and is readable
ls -la /path/to/library.so

# Check library dependencies
ldd /path/to/library.so

# Try alternative paths
find /usr -name "*tree-sitter*" -name "*.so" 2>/dev/null
```

**Symbol Not Found:**
```bash
# List all exported symbols
nm -D library.so | grep -v "^$"

# Look for tree-sitter related symbols
nm -D library.so | grep -i tree

# Common symbol name patterns to try:
adze parse --dynamic lib.so input.txt --symbol language
adze parse --dynamic lib.so input.txt --symbol tree_sitter_LANG
adze parse --dynamic lib.so input.txt --symbol get_language
```

**Parse Failures:**
```bash
# Enable verbose output for debugging
adze --verbose parse --dynamic lib.so input.txt

# Try with smaller input files to isolate issues
head -n 10 large_file.txt > small_test.txt
adze parse --dynamic lib.so small_test.txt
```

**Permission Issues:**
```bash
# Make sure library is executable
chmod +x /path/to/library.so

# Check SELinux context (on Red Hat systems)
ls -Z /path/to/library.so
```

### Performance Optimization

**For Large Files:**
- Use streaming approaches for files >100MB
- Enable performance monitoring: `ADZE_LOG_PERFORMANCE=true`
- Consider breaking large files into smaller chunks

**For Batch Processing:**
- Use parallel processing with GNU parallel:
  ```bash
  find . -name "*.py" | parallel adze parse --dynamic lib.so {} --format json
  ```
- Cache library loading when processing many files
- Use JSON output format for machine processing

**Memory Usage:**
- Monitor memory usage with system tools
- Use compact output formats when possible
- Process files in batches rather than all at once

## Integration Examples

### Makefile Integration

```makefile
# Makefile for syntax validation

PYTHON_LIB := /usr/lib/libtree-sitter-python.so
JSON_LIB := /usr/lib/libtree-sitter-json.so

.PHONY: validate validate-python validate-json

validate: validate-python validate-json

validate-python:
	@echo "Validating Python files..."
	@for file in src/*.py; do \
		if adze parse --dynamic $(PYTHON_LIB) "$$file" --format json | \
		   jq -e '.status == "ok"' >/dev/null; then \
			echo "✅ $$file"; \
		else \
			echo "❌ $$file"; \
			exit 1; \
		fi; \
	done

validate-json:
	@echo "Validating JSON files..."
	@for file in config/*.json; do \
		if adze parse --dynamic $(JSON_LIB) "$$file" --format json | \
		   jq -e '.status == "ok"' >/dev/null; then \
			echo "✅ $$file"; \
		else \
			echo "❌ $$file"; \
			exit 1; \
		fi; \
	done
```

### GitHub Actions Workflow

```yaml
# .github/workflows/syntax-validation.yml
name: Syntax Validation

on: [push, pull_request]

jobs:
  validate:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v3
    
    - name: Install adze CLI
      run: |
        # Release-surface command shape; requires a crates.io install receipt
        # before this guide can be treated as supported workflow proof.
        cargo install adze-cli --features dynamic
    
    - name: Install Tree-sitter grammars
      run: |
        sudo apt-get update
        sudo apt-get install -y libtree-sitter-json-dev libtree-sitter-python-dev
    
    - name: Validate JSON files
      run: |
        for file in config/*.json; do
          if [ -f "$file" ]; then
            adze parse --dynamic /usr/lib/libtree-sitter-json.so "$file" --format json
          fi
        done
    
    - name: Validate Python files  
      run: |
        for file in src/*.py; do
          if [ -f "$file" ]; then
            adze parse --dynamic /usr/lib/libtree-sitter-python.so "$file" --format json
          fi
        done
```

This guide covers the most common dynamic loading scenarios. The key benefits are immediate parsing without compilation, support for existing Tree-sitter ecosystems, and excellent integration with shell scripting and automation tools.
