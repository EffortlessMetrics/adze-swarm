#!/bin/bash
# CI validation script for adze
# Run this before publishing or in CI to ensure code quality

set -e

echo "=== Adze CI Validation ==="
echo

# Format check
echo "→ Checking code formatting..."
./scripts/fmt-workspace.sh --check || {
    echo "❌ Format check failed. Run 'cargo fmt' to fix."
    exit 1
}
echo "✅ Format check passed"
echo

# Clippy lints
echo "→ Running clippy on tablegen (strict)..."
cargo clippy -p adze-tablegen -- -D warnings || {
    echo "❌ Clippy found issues. Fix the warnings above."
    exit 1
}
echo "✅ Clippy passed for tablegen (strict mode)"
echo

echo "→ Running clippy on ir (permissive)..."
cargo clippy -p adze-ir 2>&1 | tee /tmp/clippy-ir.log
if grep -q "error:" /tmp/clippy-ir.log; then
    echo "❌ Clippy found errors in ir. Fix these before release."
    exit 1
else
    echo "✅ Clippy check complete for ir (warnings allowed)"
fi
echo

echo "→ Running clippy on glr-core (permissive)..."
cargo clippy -p adze-glr-core 2>&1 | tee /tmp/clippy-glr.log
if grep -q "error:" /tmp/clippy-glr.log; then
    echo "❌ Clippy found errors in glr-core. Fix these before release."
    exit 1
else
    echo "✅ Clippy check complete for glr-core (warnings allowed)"
fi
echo

# Tests
echo "→ Running all tests..."
cargo test --workspace || {
    echo "❌ Tests failed. Fix the failing tests above."
    exit 1
}
echo "✅ All tests passed"
echo

# Documentation build
echo "→ Building documentation..."
cargo doc -p adze-tablegen --no-deps || {
    echo "❌ Documentation build failed."
    exit 1
}
cargo doc -p adze-ir --no-deps || {
    echo "❌ Documentation build failed."
    exit 1
}
cargo doc -p adze-glr-core --no-deps || {
    echo "❌ Documentation build failed."
    exit 1
}
echo "✅ Documentation builds successfully"
echo

# Package validation
echo "→ Validating package contents..."
echo "  Checking adze-ir..."
cargo package -p adze-ir --list > /dev/null || {
    echo "❌ Package validation failed for adze-ir"
    exit 1
}

echo "  Checking adze-glr-core..."
cargo package -p adze-glr-core --list > /dev/null || {
    echo "❌ Package validation failed for adze-glr-core"
    exit 1
}

echo "  Checking adze-tablegen..."
cargo package -p adze-tablegen --list > /dev/null || {
    echo "❌ Package validation failed for adze-tablegen"
    exit 1
}
echo "✅ Package contents validated"
echo

# Check for TODO/FIXME comments in release crates
echo "→ Checking for TODO/FIXME comments..."
TODO_COUNT=$(grep -r "TODO\|FIXME" ir/src glr-core/src tablegen/src 2>/dev/null | wc -l || echo 0)
if [ "$TODO_COUNT" -gt 0 ]; then
    echo "⚠️  Found $TODO_COUNT TODO/FIXME comments in release crates:"
    grep -r "TODO\|FIXME" ir/src glr-core/src tablegen/src 2>/dev/null | head -5 || true
    echo "   Consider addressing these before release."
else
    echo "✅ No TODO/FIXME comments found"
fi
echo

echo "=== ✅ All CI checks passed! ==="
echo "Ready for release or merge."
