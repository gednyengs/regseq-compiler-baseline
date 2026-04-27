# rustc_public / rustc_public_bridge

**One-line summary**: In-tree rustc crates that expose an owned, serializable mirror of MIR/Ty for external tools, using a Tables-based `'tcx` boundary and a `with(|cx| ...)` thread-local API.

**URLs**:
- Source (nightly-2026-02-01): `compiler/rustc_public/`, `compiler/rustc_public_bridge/`
- Project hub: https://github.com/rust-lang/project-stable-mir

## Pipeline shape

- Source language: Rust.
- Extraction point in rustc: post-analysis MIR. `BodyBuilder` calls `tcx.instance_mir(instance.def)` then `instance.instantiate_mir_and_normalize_erasing_regions(...)` (`rustc_public_bridge/src/builder.rs:31-44`); a `MutVisitor` rewrites `ConstOperand` to `mir::Const::Val` so all constants are pre-evaluated (lines 55-71).
- Output: owned, `Serialize`-derived types (`Body`, `BasicBlock`, `Terminator`, `Statement`, `Ty`, `Instance`, `MirConst`, `Allocation`).
- How they invoke rustc: tools link `rustc_driver` + `rustc_interface`, register a callback (the `run!` macro), and inside it call `rustc_public::*` free functions that dereference a thread-local `CompilerInterface` via `compiler_interface::with` (`src/compiler_interface.rs:864-871`).

## IR design

- Owned, not `'tcx`-borrowed. All public types are `Clone + Debug + Serialize`. The `'tcx` lifetime is confined to `rustc_public_bridge::Tables<'tcx, B>` and `CompilerCtxt<'tcx, B>` inside the thread-local `CompilerInterface`.
- Shape relative to MIR: close mirror with deliberate simplifications. `Body` keeps `blocks`, `locals`, `arg_count`, `var_debug_info`, `spread_arg`, `span` (`mir/body.rs:14-38`). `TerminatorKind` mirrors MIR but flattens unwind into one `UnwindAction` and reduces `InlineAsm` operands to a partial form with `raw_rpr: String` (lines 153-193, 244-250).
- Notable variant collapses:
  - `InstanceKind` reduced from rustc's ~14 variants to 4: `Item`, `Intrinsic`, `Virtual { idx }`, `Shim` (`mir/mono.rs:29-40`). All shim flavors (`VTableShim`, `ReifyShim`, `FnPtrShim`, `DropGlue`, `CloneShim`, `AsyncDropGlue`, `ThreadLocalShim`, `ClosureOnceShim`, ...) collapse to `Shim` at `unstable/convert/stable/ty.rs:947-963`.
  - `Coverage` and `LocalDefId` are aliased to `Opaque(String)` -- intentionally non-introspectable.
- Serialization: every public type derives `serde::Serialize`. Indexed handles (`Ty`, `DefId`, `Span`, `AllocId`, `Layout`, `MirConstId`, `TyConstId`, `VariantIdx`) serialize as bare integers via `serialize_index_impl!`.
- ID-table / sidecar: `Tables<'tcx, B>` holds eight `IndexMap`s -- one per kind of interned rustc handle (`def_ids`, `alloc_ids`, `spans`, `types`, `instances`, `ty_consts`, `mir_consts`, `layouts`) -- mapping each `'tcx` value to a small `Cacheable` index (`rustc_public_bridge/src/lib.rs:45-69`). Public types carry only the index; resolution goes through `Tables[index]`.
- The bridge is generic in a `Bridge` trait (`rustc_public_bridge/src/lib.rs:206-240`); `rustc_public` provides `BridgeTys` (`compiler_interface.rs:31-67`) instantiating every associated type.

## Key design decisions

- Monomorphization handling: bodies are built per `Instance` via `BodyBuilder::build`, which monomorphizes and erases regions before stabilization. `instance_body` returns a `Body` with all generics substituted and constants evaluated. Polymorphic per-`DefId` bodies remain available through `mir_body`.
- `core` / `std` treatment: no special-casing. `external_crates()` and `find_crates(name)` enumerate all loaded crates uniformly.
- `'tcx` boundary: `'tcx` never escapes `CompilerInterface`. Public IR types are `'static`. Enforced by (a) `RefCell<Tables<'tcx, _>>` and `RefCell<CompilerCtxt<'tcx, _>>` inside `CompilerInterface<'tcx>`, (b) a thread-local raw pointer (`scoped_tls`) that hands back `&CompilerInterface<'_>` only inside `with`, and (c) `ThreadLocalIndex` (a `PhantomData<*const ()>` ZST embedded in `CrateNum`) making public types `!Send`/`!Sync` so indices cannot leak across `run!` invocations (`src/lib.rs:299-313`).
- InstanceKind: only the four user-visible distinctions are kept; orthogonal queries (`is_empty_drop_shim`, `intrinsic_name`, `mangled_name`) recover finer information.
- Stability story: crate version `0.1.0-preview`; module docs say "API is still completely unstable and subject to change." A `rustc_internal` Cargo feature gates raw `TyCtxt` interop and is excluded from semver.
- Unsupported constructs: instead of failing, conversion uses opaque strings or partial structs (`InlineAsmOperand.raw_rpr: String`, `Coverage = Opaque`). Explicit "ship now, model later" mechanism.

## Test methodology

Unit tests at `compiler/rustc_public/src/tests.rs` assert serde round-trips for index types. Integration tests live under `tests/ui-fulldeps/stable-mir/`: a custom driver runs `rustc_public` queries on fixtures and snapshot-compares. `-Zunpretty=stable-mir` prints `Body::dump` output.

## What's worth stealing

- Tables-as-bridge: keep public IR `'static` and owned; route every internal handle through an `IndexMap<Internal, PublicIndex>` accessed only inside a scoped context. Lets RegseqModel stay free of `'tcx`.
- Variant collapse with side-channel queries: shrinking enums (14 -> 4 InstanceKinds) and exposing the rest as predicates simplifies backend pattern matching without losing useful information.
- Eager monomorphization + const evaluation in the IR builder: keeps consumers from re-implementing evaluation; matches what RegseqModel construction will want.
- Opaque escape hatches (`Opaque(String)`, `raw_rpr: String`) let the IR ship before every corner is designed.

## What's specific to their constraints

- crates.io publication target: hence the `Bridge` trait, `rustc_internal` feature gate, `0.1.0-preview` version, and the split between translation logic (bridge crate) and data types (public crate). RegseqModel is internal-only; we can keep both in one crate.
- Tools cannot hold `TyCtxt`: the thread-local `with(|cx| ...)` and `run!` macro exist for this. We own our driver entry and can pass `tcx` directly.
- Cross-version compat (per PR #115934 rationale) drove the bridge abstraction; our toolchain is pinned, so unnecessary.
- Generality across consumers (Kani, verifiers, codegen) forces breadth -- coroutines, async drop, inline asm, full trait machinery. Regseq has a narrow source dialect and can reject most at MIR ingestion.

## Sources consulted

- `compiler/rustc_public/src/lib.rs:1-90, 220-320`
- `compiler/rustc_public/src/compiler_interface.rs:31-67, 75-110, 600-640, 843-871`
- `compiler/rustc_public/src/mir/body.rs:14-100, 130-250, 420-440`
- `compiler/rustc_public/src/mir/mono.rs:13-180`
- `compiler/rustc_public/src/unstable/convert/stable/ty.rs:947-963`
- `compiler/rustc_public/src/rustc_internal/mod.rs:30-80`
- `compiler/rustc_public/src/tests.rs:1-60`; `Cargo.toml`; `README.md`
- `compiler/rustc_public_bridge/src/lib.rs:1-295`; `src/builder.rs:1-77`; `src/context/mod.rs:1-80`
- https://github.com/rust-lang/project-stable-mir
