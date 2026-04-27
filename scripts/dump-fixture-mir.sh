#!/usr/bin/env bash
#
# Dump per-crate MIR plus HIR plus monomorphization listing for a driver-spec
# fixture, using rustc's built-in --emit=mir / -Zunpretty=hir / -Zprint-mono-items.
#
# Output lands under <FIXTURE>/dump/. The flag set mirrors what RegseqCallbacks
# applies, so the MIR we read here matches what the wrapper consumes when
# extraction eventually runs.
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

# Match RegseqCallbacks::config: opt level 0, encode MIR, force off UB checks.
# When the cleanup whitelist lands in regseq_mir_passes(), mirror it here.
export RUSTFLAGS="--emit=mir -Zmir-opt-level=0 -Zalways-encode-mir \
-Zmir-enable-passes=-CheckAlignment,-CheckNull,-CheckEnums"

rm -rf target
echo "[1/3] cargo build (--emit=mir, whole graph)" >&2
cargo build >&2

# Copy the primary crate's MIR plus the regseq facade. Everything else
# (proc-macro deps, build scripts) is irrelevant for hand-lowering.
cp "target/debug/deps/${PKG_UNDERSCORE}"-*.mir "$OUT/${PKG_UNDERSCORE}.mir"
cp target/debug/deps/regseq-*.mir "$OUT/regseq.mir"

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
