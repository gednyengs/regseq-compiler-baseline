# Empirical Findings

Observations about MIR shape and rustc's desugar choices, captured
during dump inspection and hand-lowering. Append-only; ordered by
date of discovery.

Each finding documents:

- The construct or pattern observed.
- The MIR fragment that surfaces it (file:fn or line range).
- What it implies for the eventual extractor or backend.
- The owning layer (RegseqModel construction / C backend) the
  finding most affects.

## 2026-04-26 -- `?` Continue extraction is conditional on the OK type

When the OK type of a `Result<T, E>` is non-unit, rustc emits a Continue
extraction statement after the SwitchInt:

```
bb_continue: { _val = copy ((_cf as Continue).0: T); ... }
```

When the OK type is **unit** (i.e. `Result<(), E>`), the Continue
extraction is elided by `RemoveZsts` (and was already a structural
no-op even before that pass): the `val` binding becomes `debug val =>
const ();` at the scope level, with no MIR statement.

**Surfaced in**: `Pl011FullSpec::init` after cleanup-whitelist build,
`tests/fixtures/pl011-full-spec/dump/pl011_full_spec.mir` lines 433+
(see also commit a595989 message for before/after diff summary).

**Implication**: a recognizer for `?` desugar must tolerate the absent
Continue extraction when the OK type is unit. The dormant `try_folding`
recognizer in `crates/regseq-driver/src/construction/try_folding.rs`
has a positional dependency on this statement; it is the reason
enabling `RemoveZsts` in the wrapper would break that path.

**Owning layer**: RegseqModel construction (Rust-shape filtering
sub-category: pattern recognition for `?` desugar).

## 2026-04-26 -- `&self` vs `&mut self` carry meaningful constraints into C

Rust's reference rules are strictly stronger than C's defaults. The
correct C lowering preserves both mutability and (asymmetric) aliasing:

- `&T` -> `const T*`. No `restrict` -- multiple `&T` to the same data
  are allowed in Rust.
- `&mut T` -> `T* restrict`. Exclusive access in Rust corresponds to
  `noalias` at the LLVM level; emitting C's `restrict` is sound.

Caveats:

- `UnsafeCell<U>` (and types containing it, including atomics)
  invalidate the `const T*` lowering because interior mutation is
  permitted through `&T` over `UnsafeCell`. The extractor must check.
- Linux kernel C historically avoids `restrict`. Whether to emit it
  is a backend-policy axis. Default: emit. Kernel-mode targets may
  suppress it without losing semantic information (the IR carries the
  mutability either way).

**Surfaced in**: walkthrough of `Pl011FullSpec::init` parameter list
(`_1: &mut Pl011FullSpec`) and the receiver reborrow patterns
(`_5 = &(*_1)` for `&self` callees, `_12 = &mut (*_1)` for `&mut self`
callees).

**Implication**: RegseqModel preserves the distinction faithfully
(via `Ref(RegseqType, Mutability)` per spec.md Section 8); the C
backend is the layer that emits `const T*` / `T* restrict` and
decides on kernel-mode policy.

**Owning layer**: C backend.

## 2026-04-26 -- `panic = "abort"` makes `unwind continue` annotations inert

Every `Call` and `Drop` terminator in the dump carries an
`unwind continue` annotation. Driver specs are `panic = "abort"`, so
no unwind edge ever fires; the annotations exist because rustc emits
them unconditionally during MIR construction, before
`AbortUnwindingCalls` can rewrite calls in non-unwinding contexts.

The annotation has no semantic effect on the lowered C, but its
presence in MIR is uniform.

**Surfaced in**: every Call terminator in `pl011_full_spec.mir`.

**Implication**: drop the annotation uniformly during extraction.

**Owning layer**: RegseqModel construction (uniform-rule drop, no
per-call decision).
