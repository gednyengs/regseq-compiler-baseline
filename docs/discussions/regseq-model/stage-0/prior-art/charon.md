# Charon

**One-line summary**: Rustc driver that extracts whole-crate MIR (plus types,
traits, signatures) into an owned, serializable IR called ULLBC/LLBC for
downstream verification tools (Aeneas, Eurydice, Hax, ...).

**URLs**:
- Repo: https://github.com/AeneasVerif/charon
- Paper (TACAS 2025): https://arxiv.org/abs/2410.18042
- Docs: https://aeneasverif.github.io/charon/
- Sister project: https://github.com/AeneasVerif/aeneas

## Pipeline shape

- Source language: plain Rust (no embedded DSL of its own; consumers may add
  attributes via `#[charon::opaque]`, `#[charon::exclude]`).
- Extraction point in rustc: MIR. Configurable via `MirLevel`
  (`Built` / `Promoted` / `Elaborated` / `Optimized`) in
  `charon/src/bin/charon-driver/translate/get_mir.rs`. Default is post-borrowck
  but pre-optimization; optimized MIR is used as a fallback for foreign bodies.
- Output: JSON-serialized ULLBC or LLBC crate (one file per crate).
- How they invoke rustc: as a `RUSTC_WRAPPER` over Cargo. The `charon` binary
  shells out to `cargo`, intercepts compiler invocations, and runs
  `charon-driver` (a `rustc_driver::Callbacks` impl) instead of `rustc`. See
  `charon/src/bin/charon-driver/driver.rs`.

## IR design

- **Owned, not `'tcx`-borrowed**. ULLBC and LLBC are pure `serde`/`serde_state`
  data with no rustc lifetime parameters (`charon/src/ast/ullbc_ast.rs`,
  `llbc_ast.rs`). This is a deliberate split from rustc internals.
- Two IRs:
  - **ULLBC** (Unstructured Low-Level Borrow Calculus): a CFG of basic blocks,
    structurally close to MIR but cleaned up (constants reconstructed,
    panic desugarings unified, marker traits optionally stripped).
  - **LLBC** (Low-Level Borrow Calculus): structured-AST view produced by a
    Relooper-style pass over ULLBC; loops/`if`/`match` are materialized.
    Implemented in `charon/src/transform/control_flow/ullbc_to_llbc.rs`.
- Notable variants/types: `GenericParams { regions, types, const_generics,
  trait_clauses }`; `TraitRef` discriminates `TraitImpl` / `Clause` / `ItemClause`;
  `Body` can be `Missing` for unsupported items.
- Serialization: JSON via `serde_state` (custom variant of serde that supports
  shared interner state). OCaml bindings auto-generated in `charon-ml/`.

## Key design decisions

- **Monomorphization**: optional. Default is *generic-preserving* extraction
  (generics in the IR are kept, with substitutions). A `--monomorphize` flag
  invokes `hax::substitute` per item. This is unusual among MIR consumers --
  most verifiers either pre-mono or stay generic, Charon supports both.
- **`core` / `std` treatment**: handled by an opacity system
  (`docs/what_charon_translates.md`): items outside the current crate default
  to `Foreign` (signature-only translation), with `--include` / `--opaque` /
  `--extract-opaque-bodies` to widen the cone. Marker traits (`Sized`, `Send`,
  `Sync`, `Unpin`, `Tuple`, `Allocator`) are stripped by default.
- **Drop handling**: drops are explicit in ULLBC. There is dedicated logic in
  `translate_drops.rs` that synthesizes a `Destruct::drop_in_place` shim from
  rustc's `drop_glue_shim`, gated on `--precise-drops` because rustc's drop
  elaboration is known to ICE on polymorphic types. Issue 152 tracks gaps.
- **Panic strategy**: panic call sites are reconstructed via passes that
  collapse the many `panic!`/`assert!` desugarings into one statement
  (`Assert` variant); panic-as-abort is the abstract semantics.
- **Unsupported constructs**: graceful errors with spans via
  `raise_error!` / `ErrorCtx`. Bodies that fail translate to `Body::Missing`
  rather than aborting the whole crate. Translation wraps body extraction in
  `panic::catch_unwind` to survive rustc/hax panics.
- **Reference rules**: `&T` and `&mut T` survive into LLBC as borrow operations;
  reborrows are explicit (`mir_emit_retag` is enabled). Lifetime variables are
  abstracted but preserved in signatures.
- **Trait dispatch / vtables**: static dispatch is reconstructed as
  `(TraitImplRef, method)` pairs. `dyn Trait` has explicit vtable representation
  (`translate_trait_objects.rs`). An optional pass `--remove-associated-types`
  rewrites associated types into extra type parameters (dictionary-passing
  style) for backends that can't handle them.

## Test methodology

UI-style snapshot tests under `charon/tests/ui/`: each `<file>.rs` has a
matching `<file>.out` capturing the pretty-printed LLBC (or ULLBC). Magic
comments (`//@ charon-args=...`, `//@ known-failure`, `//@ known-panic`,
`//@ aux-crate=...`) configure each case. Driver: `charon/tests/ui.rs` using
`libtest_mimic` + `snapbox`. CI compares output and fails on drift.
A separate `cargo` integration test exercises the cargo-wrapper path. There is
also `popular-crates.rs` which fuzzes Charon over crates.io.

## What's worth stealing

- **Owned, serde-serializable IR with OCaml bindings auto-generated**: a clean
  separation from `'tcx` lifetimes is exactly what a long-running compiler IR
  wants if the backend may be a different process or language.
- **Configured rustc options**: `always_encode_mir = true`, `mir_opt_level = 0`,
  `mir_emit_retag = true`, `mir_preserve_ub = true`, plus disabling
  `CheckAlignment`. Same shape as our planned setup; their pin list is a
  useful checklist.
- **UI-snapshot test harness with magic comments**: drives both pretty-print
  and "known-failure" paths from one set of fixtures with `libtest_mimic`.
- **Two-tier IR (CFG then structured AST)**: ULLBC for analyses that want a
  CFG, LLBC for backends that want structured control flow. Our C backend
  needs structured output -- the relooper pass is a known pattern.

## What's specific to their constraints

- Charon must preserve generics and trait clauses for verification consumers
  (Aeneas needs them for functional translation). Regseq has no such consumer
  -- we monomorphize and discard generics aggressively.
- Charon keeps marker traits, lifetimes, and bounds that we can drop entirely
  for codegen.
- Charon supports an *opacity lattice* over the whole dependency graph because
  verification cares about specs of dependencies. Regseq only cares about the
  driver crate plus a small intrinsics surface.
- Charon's two-IR split (ULLBC + LLBC) is partly motivated by the need to
  feed *both* CFG- and AST-shaped consumers. We have only one consumer (C
  codegen), so we likely want only the structured form.

## Sources consulted

- https://github.com/AeneasVerif/charon (root README, Cargo.toml)
- `charon/src/bin/charon-driver/driver.rs`
- `charon/src/bin/charon-driver/translate/get_mir.rs`
- `charon/src/bin/charon-driver/translate/translate_drops.rs`
- `charon/src/transform/control_flow/ullbc_to_llbc.rs`
- `charon/src/ast/{ullbc_ast.rs, llbc_ast.rs, gast.rs}`
- `charon/tests/ui.rs`
- `docs/what_charon_translates.md`, `docs/transformations.md`,
  `docs/limitations.md`, `docs/usage.md`
- arXiv:2410.18042 ("Charon: An Analysis Framework for Rust", TACAS 2025)
