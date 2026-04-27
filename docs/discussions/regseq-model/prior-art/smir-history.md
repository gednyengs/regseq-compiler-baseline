# SMIR design history (project-stable-mir / librarification)

**One-line summary**: A multi-year effort to produce a SemVer-compliant, owned mirror of rustc's MIR for external tools, evolving from "MIR librarification" (2019) -> single `smir` crate (2023) -> `stable_mir` + `rustc_smir` split (2023) -> `rustc_public` + `rustc_public_bridge` rename (2024-2025).

**URLs**:
- Project hub: https://github.com/rust-lang/project-stable-mir
- Original MCP: https://github.com/rust-lang/compiler-team/issues/233 ("MIR librarification", oli-obk, 2019-12-22)
- Steering meeting compiler-team#498 ("Exposing a versioned, semi-stable MIR interface")
- Design notes: https://hackmd.io/@celinaval/Skgj_k1Ti, https://hackmd.io/@rust-compiler-team/rJlG-DfMc
- Crate-split PR: https://github.com/rust-lang/rust/pull/115934 (oli-obk, 2023)
- Other key PRs: #113412 (basic types), #118012 (allocations), #120135 (privacy), #125967 (TyConst/MirConst split), #126410 (ConstOperand merge)

## Pipeline shape

- Source language: Rust.
- Extraction point in rustc: post-analysis MIR via `tcx.instance_mir`; this anchor has been stable across the entire history.
- Output: progressively more complete owned IR (initially basic types and a `Body` skeleton; allocations, layouts, ABI, coroutines, async drop glue added incrementally).
- How they invoke rustc: from day one, a callback registered with `rustc_driver`, holding `TyCtxt` via thread-local on the host side and exposing only owned types to the tool.

## IR design

- Owned vs `'tcx`-borrowed: the original librarification MCP proposed "moving MIR datastructures into a separate crate that does not depend on librustc, with dependencies on Ty broken by introducing a generic parameter." Early notes (hackmd `Skgj_k1Ti`) committed to "no interning, everything is using `Box<T>` instead of `&'tcx T`". This decision held; the current `rustc_public` IR is fully owned and `'static`.
- Shape relative to MIR: close mirror with simplifications. PR #115934 stated the original 1:1 goal: "initial 0.1 releases would provide 1:1 conversions between unstable rustc versions and stable versions." Subsequent PRs simplified specific corners (PR #126410 merged duplicated `Constant` and `ConstOperand`).
- Notable type evolution:
  - `Const` -> `TyConst` + `MirConst` (PR #125967), mirroring rustc's own split.
  - Type representation went from opaque display strings (`struct Ty(String)` per the early hackmd) to indexed handles backed by `IndexMap<Ty<'tcx>, B::Ty>` in the Tables.
  - PR #120135 made remaining "private" fields actually private after the structural split.
- Serialization: serde Serialize on every type became default after the rename to `rustc_public`.
- ID-table / sidecar / bridge: the central pattern, present from the first split. Bridge holds `'tcx` data; public crate holds owned indices; conversion gated by a scoped context.

## Key design decisions

- Monomorphization handling: per-`Instance` monomorphization with eager const evaluation, in `rustc_public_bridge::builder::BodyBuilder`. Allocation support landed in PR #118012.
- `core` / `std` treatment: never special-cased. `find_crates`/`external_crates` are uniform across local and upstream crates.
- `'tcx` boundary: the load-bearing decision and the one that drove the crate split. The original `smir` crate mixed stable types with rustc-internal references; PR #115934 split it precisely "to prevent accidental usage of forever unstable things." The boundary now: `rustc_public_bridge` is the only crate that names `'tcx`; `rustc_public` is `'static` and accesses `'tcx` only through scoped TLS. The `ThreadLocalIndex` ZST further makes public indices `!Send`/`!Sync` so they cannot leak across `run!` calls.
- InstanceKind variants: 4 (`Item`, `Intrinsic`, `Virtual { idx }`, `Shim`); ~10 shim flavors merged into `Shim`. Auxiliary boolean queries recover information when needed -- a deliberate "lossy projection plus side-channel queries" pattern.
- Stability story: rebrand from `stable_mir` to `rustc_public` reframed the promise to "public, SemVer compliant" rather than "stable forever". Cargo manifest still reads `0.1.0-preview`. The `rustc_internal` feature gates raw-`TyCtxt` interop, excluded from semver. PR #115934 outlined a forward-compat plan: future breaking changes ship new versions with compatibility layers translating through the newer one.
- Unsupported constructs: deferred via `Opaque(String)` (`Coverage`, `LocalDefId`) and partial structs with `raw_rpr: String` (`InlineAsmOperand`).

## Rejected / deferred ideas

- "Stable MIR as on-disk format": both emit and accept-as-input were deferred. Emit is partially available (`-Zunpretty=stable-mir`); accept-as-input is explicitly out of scope.
- Interning / `&'tcx`-style sharing inside the public IR: rejected up front in favor of `Box<T>` + indices, to avoid bringing rustc's allocator and lifetime model along.
- "Forever stable" semantics: superseded by the `rustc_public` rebrand.
- Single-crate design: the original `smir` was rejected (PR #115934) because it allowed unstable internals to leak into stable surface area.

## Test methodology

In-tree integration tests at `tests/ui-fulldeps/stable-mir/` (later `rustc_public/`) build a custom driver, run queries against fixtures, and snapshot-compare. `-Zunpretty=stable-mir` provides a textual oracle. Downstream consumers (Kani, MIRI-related tools) act as integration tests; project-stable-mir issues are frequently filed against gaps surfaced by Kani.

## What's worth stealing

- Crate-split rationale: keep "IR data types" and "translation from rustc internals" in separate compilation units (or modules with a clear API boundary) so rustc-side churn cannot leak into RegseqModel consumers.
- Scoped-TLS context + indexed handles is a clean way to avoid threading `tcx` through every backend function while keeping the IR `'static`.
- Lossy variant collapse with side-channel queries: pick the few distinctions that matter to the backend, expose the rest as predicates.
- Historical lesson: every place where the original `smir` crate let `'tcx` references leak became a bug. Designing the boundary up front saves work.

## What's specific to their constraints

- Supporting arbitrary downstream tools (Kani, verifiers, alternate codegens) forces breadth -- coroutines, async drop, inline asm, full trait machinery. Regseq has a narrow source dialect and can reject most at MIR ingestion.
- crates.io publication is why the bridge is generic over a `Bridge` trait and `rustc_internal` is feature-gated. RegseqModel is private to the compiler.
- The `0.1.0-preview` version and the rebrand reflect a multi-year process of negotiating "stable" semantics; RegseqModel has no external stability obligations.
- The deprecated `pub mod rustc_internal {}` re-export at `rustc_public_bridge/src/lib.rs:39-40` is a maintenance tax to keep downstream tools compiling across the rename. We will not have that tax.

## Sources consulted

- https://github.com/rust-lang/project-stable-mir (README)
- https://github.com/rust-lang/compiler-team/issues/233 (MCP, oli-obk, 2019-12-22)
- https://github.com/rust-lang/rust/pull/115934 (split, 2023); #113412, #118012, #120135, #125967, #126410
- https://hackmd.io/@celinaval/Skgj_k1Ti, https://hackmd.io/@rust-compiler-team/rJlG-DfMc
- https://rust-lang.github.io/rfcs/1211-mir.html (RFC 1211)
- https://model-checking.github.io/kani/stable-mir.html
- `compiler/rustc_public/Cargo.toml` (`0.1.0-preview`); `compiler/rustc_public_bridge/src/lib.rs:39-40`
