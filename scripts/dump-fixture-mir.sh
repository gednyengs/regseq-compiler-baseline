#!/usr/bin/env bash
#
# Dump per-crate MIR plus HIR plus monomorphization listing for a driver-spec
# fixture, using rustc's built-in --emit=mir / -Zunpretty=hir / -Zprint-mono-items.
#
# Output lands under <FIXTURE>/dump/.
#
# This script is intentionally a sandbox decoupled from the wrapper. It runs
# AHEAD of regseq_mir_passes() in callbacks.rs: the cleanup-pass whitelist is
# enabled here for hand-lowering readability, while the wrapper stays
# conservative until extraction is rewritten to expect the cleaned shape.
# The decisions captured during hand-lowering inform what the wrapper's
# eventual whitelist and the new extractor's recognition strategy should be.
#
# Usage: scripts/dump-fixture-mir.sh <fixture-dir>
# Example: scripts/dump-fixture-mir.sh tests/fixtures/pl011-full-spec

set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "usage: $0 <fixture-dir>" >&2
    exit 2
fi

FIXTURE="$1"
if [[ ! -d "$FIXTURE" ]]; then
    echo "error: $FIXTURE is not a directory" >&2
    exit 1
fi
if [[ ! -f "$FIXTURE/Cargo.toml" ]]; then
    echo "error: $FIXTURE/Cargo.toml missing" >&2
    exit 1
fi

# Resolve to absolute paths before we cd into the fixture so OUT lands where
# the caller expects.
FIXTURE_ABS="$(cd "$FIXTURE" && pwd)"
OUT="$FIXTURE_ABS/dump"
mkdir -p "$OUT"
rm -f "$OUT"/*.mir "$OUT"/spec.hir.txt "$OUT"/mono-items.txt

PKG_NAME="$(awk -F'"' '/^name = / { print $2; exit }' "$FIXTURE_ABS/Cargo.toml")"
PKG_UNDERSCORE="${PKG_NAME//-/_}"

cd "$FIXTURE_ABS"

# Sandbox flag set: opt level 0, encode MIR, UB checks off, full cleanup
# whitelist on. The wrapper's regseq_mir_passes() is currently a strict
# subset of this (UB-check force-offs only); when extraction is rewritten
# to expect the cleaned shape these will converge. See script header.
#
# Cleanup passes (+):
#   RemoveStorageMarkers   strip StorageLive/StorageDead
#   RemoveZsts             elide zero-sized-type writes (incl. unit-payload)
#   SimplifyLocals-final   compact unused local decls
#   ReorderBasicBlocks     reverse-postorder block IDs (RPO)
#   ReorderLocals          renumber locals in encounter order
# UB-check passes (-):
#   CheckAlignment, CheckNull, CheckEnums  no driver-spec instrumentation
export RUSTFLAGS="--emit=mir -Zmir-opt-level=0 -Zalways-encode-mir \
-Zmir-enable-passes=+RemoveStorageMarkers,+RemoveZsts,+SimplifyLocals-final,+ReorderBasicBlocks,+ReorderLocals,-CheckAlignment,-CheckNull,-CheckEnums"

rm -rf target
echo "[1/3] cargo build (--emit=mir, whole graph)" >&2
cargo build >&2

# Copy the primary crate's MIR. The regseq facade now lives inside each
# spec crate as a `crate::regseq` submodule, so its bodies are inlined
# into the spec's `.mir` -- no separate facade file is produced.
# Everything else (proc-macro deps, build scripts) is irrelevant for
# hand-lowering.
cp "target/debug/deps/${PKG_UNDERSCORE}"-*.mir "$OUT/${PKG_UNDERSCORE}.mir"

echo "[2/3] cargo rustc -- -Zunpretty=hir > spec.hir.txt" >&2
cargo rustc -- -Zunpretty=hir 2>/dev/null > "$OUT/spec.hir.txt"

echo "[3/3] cargo rustc -- -Zprint-mono-items > mono-items.txt" >&2
# touch lib.rs to force recompile; -Zprint-mono-items only emits during
# actual code generation, which cargo skips on cache hits.
touch src/lib.rs 2>/dev/null || true
cargo rustc -- -Zprint-mono-items 2>/dev/null \
    | grep "^MONO_ITEM" > "$OUT/mono-items.txt" || true

unset RUSTFLAGS

cd - >/dev/null

echo "" >&2
echo "Dump produced under $OUT/" >&2
ls -1 "$OUT/" | sed 's/^/  /' >&2
echo "" >&2
echo "Sizes:" >&2
wc -l "$OUT"/*.mir "$OUT"/spec.hir.txt "$OUT"/mono-items.txt 2>/dev/null \
    | awk 'NR>0 {printf "  %7d  %s\n", $1, $2}' >&2
