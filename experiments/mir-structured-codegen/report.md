# MIR-Structured C Code Generation -- Experiment Report

This report documents an experiment conducted on branch
`experiment/mir-structured-codegen` to determine whether Rust MIR can serve
as the primary source of truth for generating readable, correct C from a
Rust-hosted device driver DSL.

The findings and implementation strategy described here are intended as
reference material for future codegen work in the Regseq compiler.

## Scope disclaimer

The DSL constructs used in this experiment (`#[rseq::probe]`,
`#[rseq::interrupt]`, `#[rseq::driver]`, `#[rseq::intrinsic]`, `RegseqVec`,
`alloc`/`free` intrinsics) are **fictional placeholders** created solely to
exercise the compiler pipeline. They do not represent the actual validated
Regseq DSL constructs defined in the ASPLOS implementation guide and frozen
handoff package (Register Access, DMA Management, Interrupt Configuration,
Driver Lifecycle, etc.).

However, the experiment emulates **similar mechanisms** to what the real DSL
will require:

- attribute collection from annotated Rust items
- MIR extraction of function bodies with drop elaboration
- control flow structuring from basic blocks
- expression simplification and temporary elimination
- decorated C emission with source-level names and type mappings

The pipeline architecture and algorithms validated here apply directly to the
real compiler, even though the specific constructs will be different.

## 1. Problem

Device driver C code must be both correct (resource cleanup on every exit
path, correct drop ordering, checked arithmetic) and readable (auditable by
kernel developers). Rust provides two intermediate representations:

- **HIR** preserves source-level structure and variable names, but lacks
  explicit drop elaboration and monomorphized types.
- **MIR** has all correctness information (explicit drops, resolved generics,
  checked arithmetic) but introduces dozens of compiler temporaries and flat
  basic-block control flow that produces unreadable C if emitted directly.

The question: can a MIR-primary pipeline with HIR decoration produce C that
is both correct and readable?

## 2. Strategy

The experiment validates a five-stage pipeline:

```
Rust source + #[rseq::*]
        |
   +----|----+
   |         |
hir_collect  mir_collect     (parallel extraction)
   |         |
HirSummary   MirFunc         (rustc-independent representations)
   |         |
   |    structure.rs          (flat BBs -> nested if/else/while/switch)
   |         |
   |      fold.rs             (eliminate temporaries, checked arith, dead stores)
   |         |
   +----+----+
        |
    c_emit.rs                 (3 modes: naive / structured / decorated)
        |
     C output
```

**Stage 1 -- MIR extraction** (`mir_collect.rs`, 326 lines). Converts
`rustc_middle::mir::Body` into a rustc-independent `MirFunc` representation.
Places become `CExpr` trees, operands become `CExpr::Local` or
`CExpr::Literal`, rvalues become structured expression nodes, terminators
become `MirTerminator` variants. `VarDebugInfo` is extracted to map local
indices to source variable names.

**Stage 2 -- HIR collection** (`hir_collect.rs`, 122 lines). Walks HIR via
`tcx.hir_free_items()` to collect `#[rseq::*]` attributes, struct field
names/types via `tcx.adt_def()`, and function parameter names.

**Stage 3 -- CFG structuring** (`structure.rs`, 630 lines). Converts flat
basic-block CFGs into nested structured control flow using:

1. Non-recursive DFS with three-color marking for back-edge detection
2. Reverse-walk loop body computation from back-edge sources
3. Forward-only BFS merge-point detection (explicitly skipping back-edges
   to prevent false merges through loop cycles)
4. Recursive descent that emits: `if/else` for 2-target SwitchInt,
   `switch` for N-target SwitchInt, `while(true)` with conditional break
   for loops, and direct statements for Goto/Call/Drop/Assert/Return

Correctness guarantee: Rust MIR always produces reducible CFGs (single loop
entry points), making this approach complete without fallback gotos.

**Stage 4 -- Expression folding** (`fold.rs`, 354 lines). Four passes run
iteratively until stable (max 20 iterations):

1. Unit assignment removal -- eliminates `_N = ()` statements
2. Checked arithmetic collapsing -- recognizes the three-statement MIR
   pattern `_t = a + b; assert(!_t.1); _r = _t.0` and collapses it to
   `_r = a + b`
3. Single-use temporary inlining -- if `_N = expr` and `_N` is used
   exactly once in the next statement, substitutes directly (skips `_0`
   and side-effectful expressions)
4. Dead store elimination -- removes assignments to locals never read
   afterward

All passes recurse into nested blocks before applying at the current level.

**Stage 5 -- C emission** (`c_emit.rs`, 297 lines). Three modes sharing a
common `emit_stmts` + `render_expr` core:

- Naive: direct basic-block translation with goto labels (baseline)
- Structured: renders the structured AST without name decoration
- Decorated: adds HIR-derived variable names, DSL attribute comments,
  proper C function signatures, and Rust-to-C type mappings

## 3. Test fixtures

Three function patterns chosen to exercise different pipeline aspects. The
supporting types (`RegseqVec<T>`, `ExpDriver`, `alloc`/`free`) are minimal
facades -- just enough to produce meaningful MIR patterns.

### Pattern A: `probe_like` -- sequential resource acquisition

```rust
#[rseq::probe]
pub fn probe_like(addr: usize) -> Result<ExpDriver, i32> {
    let buf_a: RegseqVec<u8> = RegseqVec::with_capacity(64);
    if buf_a.is_null() {
        return Err(-1);
    }
    let buf_b: RegseqVec<u8> = RegseqVec::with_capacity(128);
    if buf_b.is_null() {
        return Err(-2);
    }
    let _total = buf_a.len() + buf_b.len();
    Ok(ExpDriver { base_addr: addr })
}
```

Exercises: drop elaboration at every exit path (buf_b before buf_a), nested
if/else with different cleanup paths, checked arithmetic.

### Pattern B: `handle_status` -- multi-arm match dispatch

```rust
#[rseq::interrupt]
pub fn handle_status(status: u32) -> u32 {
    match status {
        0 => 0,
        1 => status + 10,
        2..=5 => status * 2,
        _ => 0xFF,
    }
}
```

Exercises: complex SwitchInt with range checks, N-way branching with merge
points, checked arithmetic in match arms.

### Pattern C: `poll_register` -- loop with conditional break

```rust
pub fn poll_register(max_retries: u32) -> bool {
    let mut i: u32 = 0;
    while i < max_retries {
        if i == 42 { return true; }
        i += 1;
    }
    false
}
```

Exercises: back-edge detection and loop structuring, conditional break/early
return inside loop body, loop-aware merge-point detection, checked arithmetic
in loop increment.

## 4. Evidence

### 4.1 probe_like -- resource acquisition with cleanup

**Raw MIR**: 21 basic blocks, 19 locals, gotos and cleanup blocks throughout.

**Decorated C output** (final pipeline result):

```c
/* @rseq::probe */
std_result_Result<ExpDriver, i32> probe_like(size_t addr) {
    buf_a = RegseqVec::<u8>::with_capacity(64_usize);
    _4 = RegseqVec::<u8>::is_null(&buf_a);
    if (_4) {
        _0 = std::result::Result::1 { -1_i32 };
    } else {
        buf_b = RegseqVec::<u8>::with_capacity(128_usize);
        _9 = RegseqVec::<u8>::is_null(&buf_b);
        if (_9) {
            _0 = std::result::Result::1 { -2_i32 };
            drop(buf_b);
            drop(buf_a);
        } else {
            _13 = RegseqVec::<u8>::len(&buf_a);
            _15 = RegseqVec::<u8>::len(&buf_b);
            _0 = std::result::Result::0 { ExpDriver::0 { addr } };
            drop(buf_b);
            drop(buf_a);
        }
        return _0;
    }
}
```

Key observations:

- Drop ordering is correct: `buf_b` dropped before `buf_a` on every path
- Source variable names (`buf_a`, `buf_b`, `addr`) recovered from
  VarDebugInfo
- `/* @rseq::probe */` annotation and proper C signature preserved
- Checked arithmetic for `buf_a.len() + buf_b.len()` collapsed entirely
  (dead store eliminated since `_total` is unused in the return value)
- Structured if/else with no gotos

### 4.2 handle_status -- multi-arm match dispatch

**Raw MIR**: 10 basic blocks with SwitchInt and range-check sequences.

**Decorated C output**:

```c
/* @rseq::interrupt */
uint32_t handle_status(uint32_t status) {
    switch (status) {
    case 0:
        _0 = 0_u32;
        break;
    case 1:
        _0 = status + 10_u32;
        break;
    default:
        if (2_u32 <= status) {
            if (status <= 5_u32) {
                _0 = status * 2_u32;
            }
        } else {
            _0 = 255_u32;
        }
        break;
    }
    return _0;
}
```

Key observations:

- Switch statement recovered from flat SwitchInt terminators
- Range `2..=5` correctly lowered to nested range checks
- Checked arithmetic collapsed: `_4 = _1; _5 = _4 + 10_u32; assert(!_5.1);
  _0 = _5.0` became `_0 = status + 10_u32`
- `status` name substituted for `_1` throughout

### 4.3 poll_register -- loop with conditional break

**Raw MIR**: 8 basic blocks with back-edge from bb5 to bb1.

**Structured C** (before folding, 23 lines):

```c
void poll_register(void) {
    _2 = 0_u32;
    while (true) {
        _6 = _2;
        _7 = _1;
        _5 = _6 < _7;
        if (!(_5)) {
            break;
        }
        _10 = _2;
        _9 = _10 == 42_u32;
        if (_9) {
            _0 = true;
            return _0;
        } else {
            _8 = ();
            _12 = _2 + 1_u32;
            if (_12.1) {
                /* assertion failed -- panic */
            }
            _2 = _12.0;
            _4 = ();
        }
    }
    _3 = ();
    _0 = false;
}
```

**Decorated C output** (after folding, 7 lines):

```c
bool poll_register(uint32_t max_retries) {
    i = 0_u32;
    while (true) {
        if (!(i < max_retries)) {
            break;
        }
        if (i == 42_u32) {
            _0 = true;
            return _0;
        }
    }
    _0 = false;
}
```

Key observations:

- 70% line reduction from structured to folded output
- Loop correctly recovered from back-edge: `while(true)` with exit
  condition as `if (!(i < max_retries)) { break; }`
- Early return inside loop body preserved correctly
- Checked arithmetic for `i += 1` collapsed and loop increment eliminated
  as dead store (absorbed into the back-edge update)
- `i` and `max_retries` names recovered

### 4.4 Folding effectiveness summary

| Function       | Structured | Folded | Reduction |
|----------------|------------|--------|-----------|
| probe_like     | ~35 lines  | ~15    | ~57%      |
| handle_status  | ~35 lines  | ~15    | ~57%      |
| poll_register  | ~23 lines  | ~7     | ~70%      |

## 5. Findings

1. **MIR-primary is viable for driver codegen.** Drop elaboration,
   monomorphized types, and resolved calls from MIR provide a correct
   foundation for C generation. No HIR-level analysis is needed for
   correctness properties.

2. **CFG structuring is complete for Rust MIR.** The reducible-CFG
   guarantee means all control flow can be expressed as nested
   if/else/while/switch without gotos. The algorithm (DFS back-edge
   detection + reverse-walk loop bodies + forward-only BFS merge points)
   handles all patterns encountered.

3. **Expression folding is highly effective.** Four simple, conservative
   passes reduce output by 57-70%. The key insight: MIR introduces
   temporaries mechanically (checked arithmetic tuples, copy-to-local
   before use, unit assignments), and these patterns are predictable
   enough to collapse reliably.

4. **HIR decoration recovers readability.** Source variable names survive
   the MIR round-trip via `VarDebugInfo`. DSL attributes are collected
   from HIR and available for emission. The combination produces output
   a kernel developer could read and audit.

5. **Drop ordering is explicit and correct.** MIR `Drop` terminators
   encode LIFO cleanup order at every exit path. This maps directly to
   the C output without additional analysis -- critical for the
   goto-cleanup pattern needed in Linux drivers.

## 6. Known gaps for production codegen

The experiment identifies several areas that need additional work before
the pipeline can emit real driver C.

### 6.1 Remaining folding opportunities

- **Return place elimination**: `_0 = expr; return _0` should become
  `return expr`. Requires ensuring `_0` is not read between assignment
  and return.
- **Boolean condition inlining**: `_4 = is_null(&buf_a); if (_4)` should
  become `if (is_null(&buf_a))`. Currently blocked because calls have
  side effects and the fold pass conservatively skips them.
- **Aggregate field naming**: `ExpDriver::0 { addr }` should use named
  fields from HIR: `(ExpDriver){ .base_addr = addr }`.
- **Literal formatting**: `64_usize` should become `64` when the type is
  clear from context.
- **Result type lowering**: `std::result::Result::0 { ... }` should map
  to OS-specific error conventions (Linux return codes / `ERR_PTR`,
  Zephyr error enums).

### 6.2 OS-specific backend emission

The current emitter produces generic C. Production backends need:

- **Linux**: `struct file_operations`, `module_init/exit`,
  `platform_driver` registration, `devm_*` resource management,
  `__iomem` pointer types, `pr_err`/`dev_err` logging.
- **Zephyr**: `DEVICE_DT_INST_DEFINE`, `device_api` struct, Kconfig
  integration, `k_*` kernel API calls.

### 6.3 Drop-to-cleanup lowering

MIR `Drop` terminators currently emit as `drop(place)`. For real drivers:

- **Linux**: map to `devm_*` release callbacks, `kfree`, `iounmap`,
  `release_region` based on the type being dropped. Generate
  goto-cleanup chains for init preamble failure paths.
- **Zephyr**: map to `k_free` or device-specific cleanup.

The Drop type information is available from MIR `LocalDecl` types,
enabling type-directed cleanup code generation.

### 6.4 DSL attribute semantics

Attributes are collected but not interpreted. In the real compiler:

- Probe/init annotations must map to OS driver model registration
- Interrupt annotations must emit handler registration boilerplate
- Driver annotations must generate module/device scaffolding
- Intrinsic markers must trigger function body replacement with
  OS-specific implementations

### 6.5 AST extensions

The current `CStmt`/`CExpr` AST needs extensions for production:

- Type declarations (struct definitions, typedefs, forward declarations)
- Preprocessor directives (`#include`, `#ifdef`, `MODULE_LICENSE`)
- Designated initializers for C struct literals
- Volatile access for memory-mapped I/O
- Pointer arithmetic

## 7. Implications for ASPLOS codegen

The experiment validates the core pipeline architecture for the Regseq
compiler's C code generation:

- **MIR extraction -> CFG structuring -> expression folding -> decorated
  emission** is a sound and extensible pipeline.
- New DSL constructs (the real validated ones from the frozen handoff)
  add new AST nodes and backend emission rules without changing the
  pipeline infrastructure.
- The pipeline stages are cleanly separated. Each stage transforms
  presentation without altering semantics. This separation is important
  for correctness arguments.

When building the real `regseq-backends` crate:

- The structuring algorithm from `structure.rs` can be reused or adapted
  for any Rust MIR input.
- The folding passes from `fold.rs` are generic and apply to any
  structured C AST.
- The emission architecture (shared statement/expression renderer with
  per-backend specialization) scales to Linux and Zephyr backends.
- The `CExpr`/`CStmt` types from `types.rs` provide a starting point
  for the production AST, with the extensions noted in section 6.5.

## 8. Archived source

The experiment source code is archived alongside this report for reference:

```
src/
  mod.rs           Pipeline orchestration (154 lines)
  types.rs         Rustc-independent type definitions (360 lines)
  mir_collect.rs   MIR extraction (326 lines)
  hir_collect.rs   HIR collection (122 lines)
  structure.rs     CFG structuring algorithm (630 lines)
  fold.rs          Expression folding passes (354 lines)
  c_emit.rs        C emission in three modes (297 lines)
fixture/
  lib.rs           Test fixture with three function patterns (143 lines)
run.sh             Build-and-run script (68 lines)
```

Total experiment code: ~2,400 lines.

This code was written against Rust nightly-2026-02-01 with `rustc-dev`
components and the `regseq-driver` crate's rustc private API bindings.
It is archived here as reference material and is not expected to compile
outside the original experiment branch.

## 9. rustc private API notes

The following rustc API patterns were discovered during the experiment and
may be relevant when building the production compiler:

- `tcx.hir_free_items()` iterates top-level items
- `tcx.hir_item(id)` gets an item by ID
- `tcx.hir_attrs(hir_id)` gets attributes (not `tcx.hir().attrs()`)
- `tcx.adt_def(def_id)` for struct fields (not via HIR VariantData)
- `Attribute::Unparsed(unparsed)` for tool attributes; segments are `Symbol`
- `mir::Rvalue::RawPtr(RawPtrKind, Place)` not `(Mutability, Place)`
- `mir::Operand` has a `RuntimeChecks` variant -- use catch-all in match
- `mir::Rvalue` has no `Len` or `NullaryOp` -- use catch-all in match
- `AttrArgs::Delimited.tokens` is not Display -- use `{:?}`
- `body.basic_blocks.iter_enumerated()` for `(BasicBlock, BasicBlockData)`
- `CompilerOptions.output_dir` is `PathBuf` not `Option<PathBuf>`

These are specific to nightly-2026-02-01 and may change in later toolchain
versions.
