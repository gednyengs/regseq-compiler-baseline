#!/usr/bin/env bash
# Run the MIR-structured codegen experiment.
#
# Usage: ./scripts/run-codegen-experiment.sh
#
# Output: /tmp/codegen-experiment/codegen_experiment_report.txt

set -euo pipefail

WORKSPACE_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIXTURE="$WORKSPACE_ROOT/tests/fixtures/codegen-experiment"
OUTPUT_DIR="/tmp/codegen-experiment"

mkdir -p "$OUTPUT_DIR"

echo "Building regseq-driver..."
cargo build -p regseq-driver --manifest-path "$WORKSPACE_ROOT/Cargo.toml" 2>&1

DRIVER="$WORKSPACE_ROOT/target/debug/regseq-driver"
SYSROOT=$(rustc --print sysroot)
HOST=$(rustc -vV | grep 'host:' | awk '{print $2}')

OPTIONS=$(python3 -c "
import json
print(json.dumps({
    'target_os': 'linux',
    'platform_file': '$FIXTURE/platform.toml',
    'output_dir': '$OUTPUT_DIR',
    'emit': [{'type': 'driver', 'match_id': None}],
    'check': False,
    'add_analysis': [],
    'skip_analysis': [],
    'deny': [],
    'allow': [],
    'params': [],
    'log_level': 'warn',
    'dump_stage': None,
    'color': 'auto'
}))
")

echo "Running experiment against codegen-experiment fixture..."
echo "  Driver:  $DRIVER"
echo "  Fixture: $FIXTURE"
echo "  Output:  $OUTPUT_DIR"
echo

RUSTC_WRAPPER="$DRIVER" \
REGSEQ_OPTIONS="$OPTIONS" \
REGSEQ_SYSROOT="$SYSROOT" \
CARGO_TARGET_DIR="$FIXTURE/target" \
  cargo check \
    --target "$HOST" \
    --color never \
    --manifest-path "$FIXTURE/Cargo.toml" \
    2>"$OUTPUT_DIR/full_stderr.txt"

echo
echo "Done. Report written to: $OUTPUT_DIR/codegen_experiment_report.txt"
echo
echo "To view the report:"
echo "  cat $OUTPUT_DIR/codegen_experiment_report.txt"
echo
echo "To view only the three key functions:"
echo "  grep -A 100 'FUNCTION: probe_like' $OUTPUT_DIR/codegen_experiment_report.txt | head -360"
echo "  grep -A 100 'FUNCTION: handle_status' $OUTPUT_DIR/codegen_experiment_report.txt | head -190"
echo "  grep -A 100 'FUNCTION: poll_register' $OUTPUT_DIR/codegen_experiment_report.txt | head -240"
