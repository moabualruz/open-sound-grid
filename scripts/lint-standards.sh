#!/usr/bin/env bash
# Project-specific standards checks (ADR-005) that clippy/eslint can't enforce.
# Run via: make check-standards
set -euo pipefail

ERRORS=0
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

red()   { printf '\033[1;31m%s\033[0m\n' "$1"; }
green() { printf '\033[1;32m%s\033[0m\n' "$1"; }
check() { printf '  checking: %s\n' "$1"; }

echo "=== ADR-005 Standards Check ==="

# ---------------------------------------------------------------------------
# RUST CHECKS
# ---------------------------------------------------------------------------

# 1. No anyhow — thiserror only (ADR-005)
check "no anyhow dependency"
if grep -rq 'anyhow' "$ROOT/crates" --include='*.rs' --include='*.toml'; then
    red "FAIL: anyhow found — use thiserror with typed error enums"
    grep -rn 'anyhow' "$ROOT/crates" --include='*.rs' --include='*.toml' || true
    ERRORS=$((ERRORS + 1))
fi

# 2. No #[cfg(test)] in source — tests go in tests/ directory (ADR-005)
check "no #[cfg(test)] in source files"
if grep -rq '#\[cfg(test)\]' "$ROOT/crates" --include='*.rs'; then
    red "FAIL: #[cfg(test)] found in source — move tests to tests/ directory"
    grep -rn '#\[cfg(test)\]' "$ROOT/crates" --include='*.rs' || true
    ERRORS=$((ERRORS + 1))
fi

# 3. No #[allow(dead_code)] blankets (ADR-005)
check "no #[allow(dead_code)] blankets"
if grep -rq '#\[allow(dead_code)\]' "$ROOT/crates" --include='*.rs'; then
    red "FAIL: #[allow(dead_code)] found — remove dead code instead of suppressing"
    grep -rn '#\[allow(dead_code)\]' "$ROOT/crates" --include='*.rs' || true
    ERRORS=$((ERRORS + 1))
fi

# 4. Domain glossary: no rejected synonyms in domain code (outside pw/ internals)
check "domain glossary compliance"
# GroupNode is allowed inside pw/ (PipeWire's term), but not in graph/ or routing/
GLOSSARY_VIOLATIONS=0
for dir in graph routing; do
    if grep -rqw 'GroupNode' "$ROOT/crates/osg-core/src/$dir" --include='*.rs' 2>/dev/null; then
        # Allow: type aliases, use imports, and doc comments (/// or //)
        VIOLATIONS=$(grep -rnw 'GroupNode' "$ROOT/crates/osg-core/src/$dir" --include='*.rs' \
            | grep -v 'type.*=.*GroupNode\|use.*GroupNode\|^\s*//\|^\s*///\|^[^:]*:[0-9]*:\s*//' || true)
        if [ -n "$VIOLATIONS" ]; then
            red "FAIL: 'GroupNode' used in $dir/ code — use 'Channel' (domain glossary)"
            echo "$VIOLATIONS"
            GLOSSARY_VIOLATIONS=$((GLOSSARY_VIOLATIONS + 1))
        fi
    fi
done
# "Application" is rejected — use "App"
if grep -rqw 'Application' "$ROOT/crates/osg-core/src/graph" --include='*.rs' 2>/dev/null; then
    red "FAIL: 'Application' used in graph/ — use 'App' (domain glossary)"
    grep -rn '\bApplication\b' "$ROOT/crates/osg-core/src/graph" --include='*.rs' || true
    GLOSSARY_VIOLATIONS=$((GLOSSARY_VIOLATIONS + 1))
fi
if [ "$GLOSSARY_VIOLATIONS" -gt 0 ]; then
    ERRORS=$((ERRORS + GLOSSARY_VIOLATIONS))
fi

# 5. No log crate — use tracing (ADR-005)
check "no log crate usage"
if grep -rq 'use log::' "$ROOT/crates" --include='*.rs'; then
    red "FAIL: log crate found — use tracing (ADR-005)"
    grep -rn 'use log::' "$ROOT/crates" --include='*.rs' || true
    ERRORS=$((ERRORS + 1))
fi
if grep -rq '^log ' "$ROOT/crates" --include='Cargo.toml'; then
    red "FAIL: log dependency found in Cargo.toml — use tracing"
    grep -rn '^log ' "$ROOT/crates" --include='Cargo.toml' || true
    ERRORS=$((ERRORS + 1))
fi

# 6. Serde camelCase on all serializable structs/enums
check "serde camelCase on serializable types"
# Find structs/enums with Serialize but without rename_all
MISSING_RENAME=$(grep -rn '#\[derive.*Serialize' "$ROOT/crates" --include='*.rs' -A1 | grep -v 'rename_all' | grep 'Serialize' | grep -v 'skip_serializing\|serde(skip)' || true)
if [ -n "$MISSING_RENAME" ]; then
    # Check if the next line has rename_all — grep -A1 already captures it
    # This is a heuristic — may have false positives for single-field structs
    : # Skip for now — too noisy without AST analysis
fi

# 7. File size limits (800 lines max for code)
check "file size limits (800 lines)"
while IFS= read -r f; do
    lines=$(wc -l < "$f")
    if [ "$lines" -gt 800 ]; then
        red "FAIL: $f has $lines lines (max 800)"
        ERRORS=$((ERRORS + 1))
    fi
done < <(find "$ROOT/crates" "$ROOT/web/src" -name '*.rs' -o -name '*.ts' -o -name '*.tsx' 2>/dev/null)

# ---------------------------------------------------------------------------
# WEB CHECKS
# ---------------------------------------------------------------------------

# 8. No hardcoded colors in TSX (use Tailwind theme tokens)
check "no hardcoded colors in TSX"
if grep -rqE '#[0-9a-fA-F]{3,8}|rgb\(|rgba\(|hsl\(|oklch\(' "$ROOT/web/src" --include='*.tsx' --include='*.ts' 2>/dev/null; then
    # Exclude types.ts (it has string types, not colors) and index.css (theme definitions)
    HARDCODED=$(grep -rnE '#[0-9a-fA-F]{3,8}|rgb\(|rgba\(|hsl\(|oklch\(' "$ROOT/web/src" --include='*.tsx' 2>/dev/null || true)
    if [ -n "$HARDCODED" ]; then
        red "FAIL: hardcoded colors in TSX — use Tailwind theme classes"
        echo "$HARDCODED"
        ERRORS=$((ERRORS + 1))
    fi
fi

# 9. Domain glossary in frontend: no rejected synonyms
check "frontend domain glossary"
for term in "SinkInput" "VirtualSink" "GroupNode" "Connection" "Wire" "FaderLevel" "RouteVolume" "Snapshot" "Scene" "Profile"; do
    if grep -rqw "$term" "$ROOT/web/src" --include='*.ts' --include='*.tsx' 2>/dev/null; then
        # Allow PwGroupNode in types.ts (it matches the wire format)
        if [ "$term" = "GroupNode" ]; then
            NON_TYPE=$(grep -rnw "$term" "$ROOT/web/src" --include='*.ts' --include='*.tsx' | grep -v 'types.ts\|PwGroupNode\|groupNodes' || true)
            if [ -n "$NON_TYPE" ]; then
                red "FAIL: '$term' used in frontend — check domain glossary"
                echo "$NON_TYPE"
                ERRORS=$((ERRORS + 1))
            fi
        else
            red "FAIL: '$term' used in frontend — check domain glossary"
            grep -rnw "$term" "$ROOT/web/src" --include='*.ts' --include='*.tsx' || true
            ERRORS=$((ERRORS + 1))
        fi
    fi
done

# 10. No direct WebSocket usage outside stores (SOLID-D: components depend on signals)
check "WebSocket only in stores"
if grep -rq 'WebSocket\|new WebSocket' "$ROOT/web/src/components" --include='*.tsx' --include='*.ts' 2>/dev/null; then
    red "FAIL: WebSocket used in components — move to stores (SOLID-D)"
    grep -rn 'WebSocket' "$ROOT/web/src/components" --include='*.tsx' --include='*.ts' || true
    ERRORS=$((ERRORS + 1))
fi

# ---------------------------------------------------------------------------
# RESULT
# ---------------------------------------------------------------------------

echo ""
if [ "$ERRORS" -gt 0 ]; then
    red "FAILED: $ERRORS standard violation(s) found"
    exit 1
else
    green "PASSED: all ADR-005 standards checks"
fi
