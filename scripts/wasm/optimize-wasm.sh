#!/usr/bin/env bash
set -euo pipefail

# This script assumes wasm-opt is installed and on PATH.
# It builds release WASM for the specified crate and optimizes it.

CRATE=${1:-}
if [[ -z "$CRATE" ]]; then
  echo "Usage: $0 <crate-name>"
  exit 1
fi

TARGET_DIR="target/wasm32-unknown-unknown/release"
INPUT_WASM="${TARGET_DIR}/${CRATE}.wasm"
OUTPUT_WASM="${TARGET_DIR}/${CRATE}.opt.wasm"
SIZE_REPORT="target/wasm-size-report.txt"

if [[ ! -f "$INPUT_WASM" ]]; then
  echo "Input WASM not found: $INPUT_WASM"
  exit 2
fi

mkdir -p "$(dirname "$OUTPUT_WASM")"

PRE_SIZE=$(wc -c < "$INPUT_WASM")

if ! command -v wasm-opt >/dev/null 2>&1; then
  echo "wasm-opt is required but not installed. Please install Binaryen." >&2
  exit 3
fi

wasm-opt -Oz "$INPUT_WASM" -o "$OUTPUT_WASM"
POST_SIZE=$(wc -c < "$OUTPUT_WASM")

cat > "$SIZE_REPORT" <<EOF
crate: $CRATE
before: $PRE_SIZE
after: $POST_SIZE
EOF

echo "WASM optimization complete for $CRATE"
echo "Size before: ${PRE_SIZE} bytes"
echo "Size after: ${POST_SIZE} bytes"
echo "Optimized artifact: $OUTPUT_WASM"
