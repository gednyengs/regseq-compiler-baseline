# Architecture

Architectural decisions for RegseqModel construction. These are
settled stage-0 calls that shape stages 1-13 of the bottom-up
specification work. Future sessions should treat the decisions
recorded here as binding unless explicitly revised; ambiguities
surfaced during stage work are resolved against the rationale in
this document.

Decisions are grounded in `prior-art/README.md`. Each section cites
the convergent patterns and per-project files that informed the
choice.

## Q1. Crate architecture

**Decision**: keep the existing two-crate split. The bridge
(rustc-internals -> RegseqModel translation) lives inside
`regseq-driver` as internal modules. RegseqModel itself lives in
the `regseq-model` crate, which has zero rustc dependencies.

### Rationale

Convergent pattern 1 (`rustc_codegen_c`,
`rustc_public` + `rustc_public_bridge`): the IR crate has no
`rustc_private`, no `'tcx`, no internal-rustc imports. The current
Cargo graph already enforces this:

- `regseq-driver` is the only crate with `rustc_private = true`.
- `regseq-model` depends only on `serde + toml + thiserror + regseq-config`. It cannot reference rustc internals.
- `regseq-analysis` and `regseq-backends` consume `regseq-model`
  and are also rustc-free.

The further `rustc_public` / `rustc_public_bridge` split exists to
keep `rustc_public` publishable on crates.io. We do not publish;
the forcing function does not apply.

### Concrete shape

```
regseq-driver/
  src/
    lib.rs            entry point
    args.rs           CLI / driver args
    callbacks.rs      rustc_driver::Callbacks impl
    bridge/           internal module
      mod.rs          top-level extract(tcx) -> RegseqModel
      tables.rs       transient remapping tables
      ty.rs           rustc Ty -> regseq_model::Ty
      body.rs         rustc Body -> regseq_model::Body
      ...

regseq-model/
  src/
    lib.rs            owned IR types only; no rustc deps
    ...
```

The bridge module is a candidate for promotion to a
`regseq-bridge` crate later if and only if a concrete forcing
function appears (e.g., a second consumer wants to drive extraction
independently of the full driver, or test isolation breaks down).
Until then, internal modules give the same separation without the
overhead.

### References

- `prior-art/rustc-codegen-c.md` -- two-crate split pattern.
- `prior-art/rustc-public.md` -- the further bridge-crate split,
  with the publishability rationale that does not apply to us.

## Q2. `'tcx` boundary policy

**Decision**: eager translation, fully owned RegseqModel. The
bridge consumes `TyCtxt<'tcx>` once, produces a fully-owned
`RegseqModel`, and `'tcx` does not survive past the bridge call.
None of the TLS / `RefCell<Tables<'tcx, _>>` /
`PhantomData<*const ()>` machinery from `rustc_public` is needed.

### Rationale

Regseq's posture is fundamentally different from `rustc_public`'s:

- `rustc_public`: IR is the *public API*. Callers may hold IR
  values past the compiler context, pass them across threads,
  query rustc state from them. The TLS/Tables/PhantomData
  machinery exists to support deferred lookups while keeping
  `'tcx` invisible.
- Regseq: bridge runs once in a `Callbacks` invocation; produces
  RegseqModel; consumers are downstream of the bridge and never
  query rustc state. There are no deferred lookups to support.

This extends convergent pattern 4 (bake monomorphization in the IR
builder, not deferred to consumers) from monomorphization to every
piece of rustc-side state: bake at the bridge, never defer.

### Subordinate decisions

Q2 cascades into several decisions that shape stage 1 directly.

#### a. Internal IDs

RegseqModel uses its own opaque integer IDs for everything:

- `ItemId`, `BodyId`, `TyId`, `ConstId`, `BlockId`, `LocalId`,
  `SpanId`, `StringId`, `SourceFileId`, ...

The bridge populates them from rustc-side handles (`DefId`,
`Ty<'tcx>`, `AllocId`, `Span`, `Symbol`, ...) via transient
remapping tables. RegseqModel never carries rustc-side handles.

This is convergent pattern 5 (lossy projection + side-channel
queries) applied *inside the bridge*: rustc-side queries recover
detail when populating RegseqModel fields, then both the queries
and the rustc handles are gone.

#### b. Self-describing RegseqModel

A RegseqModel value must be displayable, serializable, and
semantically complete with nothing else loaded. If any operation
requires "reach back to rustc," that is a design bug.

Concretely:

- **Spans** are expanded at the bridge into
  `(SourceFileId, byte_start, byte_end)`, indexing into a
  RegseqModel-owned `Vec<SourceFile>`. No live `SourceMap` is
  needed for display.
- **Strings** are interned through a RegseqModel-owned
  `StringInterner`. References are `StringId`.
- **Item metadata** (DefPath form, generic-args fingerprint, kind
  classification) is baked in. No DefId-side-channel lookups.
- **Body content** is monomorphization-resolved, const-evaluated,
  and normalized -- mirroring `rustc_public`'s `BodyBuilder`
  (`instantiate_mir_and_normalize_erasing_regions` plus the
  `ConstOperand -> Const::Val` rewrite).

#### c. Transient bridge Tables

Inside the bridge, the translation maintains:

```rust
struct Tables<'tcx> {
    items:   HashMap<DefId, ItemId>,
    types:   HashMap<Ty<'tcx>, TyId>,
    consts:  HashMap<AllocId, ConstId>,
    spans:   HashMap<Span, SpanId>,
    strings: HashMap<Symbol, StringId>,
    // ...
}
```

These exist only inside `bridge::extract(tcx) -> RegseqModel` and
are dropped when the function returns. They are not part of
RegseqModel's public surface.

#### d. No TLS / RefCell / PhantomData-ZST machinery

Skip. We get the benefits of `rustc_public`'s `'tcx`-confinement
pattern for free by virtue of eager translation; the complexity of
the three-mechanism scheme is a cost paid for a use case we do not
have.

If a future need for lazy queries appears (e.g., a JIT-style pass
that wants to call back into rustc), the machinery can be added
then. Do not pay for it speculatively.

### References

- `prior-art/rustc-public.md` -- the TLS / Tables / PhantomData
  scheme and what it solves.
- `prior-art/smir-history.md` -- PR #115934, the `'tcx`-leak
  history that motivated the scheme.
- `prior-art/charon.md`, `prior-art/kani.md` -- eager-translation
  precedents.

## Q3. IR layering: single-level

**Decision**: RegseqModel is a single-level IR. One hierarchy of
enums (`Ty`, `Operand`, `BasicBlock`, etc.), not a ladder of
distinct IR types per compiler stage. Successive compiler stages
*refine* the same IR rather than producing new ones.

### Rationale

This is the LLVM model (one IR, many passes that refine or
transform) rather than the CompCert model (separate IR types per
layer with total-function lowerings between them) or the
verification-tool model (Charon ULLBC/LLBC, Verus VIR-AST/SST/AIR,
Prusti's 5-tier ladder). The per-layer model imposes high
boilerplate costs (parallel enum trees, per-layer visitors,
between-layer lowerings) for benefits that do not apply when
stages can be characterized by which fields are populated and
which invariants hold.

This decision was reaffirmed from prior iterations of the design;
not relitigated against the synthesis.

### Subordinate notes

These follow from the decision; they are not separate decisions.

- **ID tables are single-namespace.** One `TyId`, one `BodyId`,
  etc. used uniformly across all stages. No per-stage namespacing.
- **Refinement mechanism is per-stage.** A stage may refine by
  populating optional fields, by inserting tags on existing
  variants, or by rewriting parts of the IR. The mechanism is
  chosen when each stage is specified; the IR design does not
  prescribe one.
- **Domain extensions are part of the same IR.** Stage 13's
  intrinsics, ordered regions, register access, ISR classification,
  etc. are additional enum variants or tags on existing variants
  -- not a separate layer above the MIR-mirror core. The
  variant-vs-tag choice is per-construct and made during stage 13's
  specification.

### References

- `prior-art/llvm-ir.md` -- single-IR model with intrinsics +
  metadata.
- `prior-art/compcert.md`, `prior-art/charon.md`,
  `prior-art/verus.md`, `prior-art/prusti.md` -- counterexamples
  (multi-tier IRs).

## Q4. rustc integration point: `Callbacks`

**Decision**: keep `rustc_driver::Callbacks` as the integration
mechanism for the bridge. Do not switch to `CodegenBackend`.

### Rationale

The convergent prior-art precedent (`rustc_codegen_c`,
`rustc_codegen_clr`, Kani all use `CodegenBackend`) is informative
but not directly transferable: those projects are *literal codegen
replacements* whose entire purpose is to be drop-in replacements for
rustc's LLVM backend, which is exactly what `CodegenBackend` is
designed for. Regseq is more than a codegen backend -- the asplos
plan calls for `model` / `analysis` / `driver` as distinct emit
kinds, and the primary outputs include analysis artifacts alongside
C code.

A requirements-driven matrix surfaced no hard constraints that force
the choice. On soft tradeoffs:

- `CodegenBackend` saves a query call for mono-item collection and
  pre-triggers const evaluation. Both reachable from `Callbacks`
  via TyCtxt.
- `Callbacks` is the natural fit for the multiple-emit-spec story:
  full control after `after_analysis()` to emit whatever shape,
  including `Compilation::Stop` to skip codegen entirely.
  `CodegenBackend` can emit non-object artifacts (Kani does this
  for GOTO programs) but its per-CGU + link API is shaped against
  this use case.
- `Callbacks` ships as a standalone binary; `CodegenBackend` ships
  as a `.so` loaded via `-Zcodegen-backend`. Standalone is simpler
  for distribution.
- `Callbacks` is the current state. Migration cost to
  `CodegenBackend` is non-trivial (test infrastructure switches to
  backend invocation).

Decisive factor: Regseq's primary job is not codegen replacement;
it is producing a domain-specific IR (RegseqModel) and multiple
downstream artifacts. The integration point should fit that posture.

### Subordinate notes

These follow from the decision; they are not separate decisions.

- **Bridge access path is `after_analysis`.** The bridge is invoked
  from the `Callbacks::after_analysis` hook with `TyCtxt` in hand;
  it produces `RegseqModel` and the driver dispatches to whichever
  emit spec is selected.
- **Mono-item collection is a TyCtxt query.** The bridge calls
  `tcx.collect_and_partition_mono_items` (or equivalent) to
  enumerate reachable items.
- **rustc options are set via `Config` override.** The
  Charon-validated flag set (`always_encode_mir`, `mir_opt_level=0`,
  `mir_emit_retag`, `mir_preserve_ub`, disabling `CheckAlignment`)
  plus the cleanup-pass whitelist live in the `Callbacks::config()`
  override.
- **Multiple emit specs dispatch after extraction.** `model`,
  `analysis`, `driver` are selected by driver args; the bridge
  produces RegseqModel uniformly, and emit-spec dispatch decides
  what to do with it downstream.
- **`Compilation::Stop` is returned after extraction.** No need to
  run rustc's codegen; we explicitly stop after analysis.

### Reconsidering this decision

The two paths are not mutually exclusive forever. Re-open the
question if any of the following appears on the roadmap:

- Integration with rustc's per-CGU output infrastructure
  (incremental compilation hooks, parallel CGU codegen).
- A custom `--target` triple (e.g., `cargo build --target regseq-linux`)
  that should "just work" without `cargo regseq`.
- A use case for shipping Regseq as a drop-in `rustc` codegen
  backend rather than a separate compiler.

Migration cost is bounded by the test infrastructure on the
`Callbacks` path; the longer we wait, the more accumulates.

### References

- `prior-art/kani.md`, `prior-art/rustc-codegen-c.md`,
  `prior-art/rustc-codegen-clr.md` -- the `CodegenBackend`
  precedents and why their primary-job framing differs from
  Regseq's.
- `prior-art/README.md` -- the integration-point fork in the road
  and the prior-art convergent signal.
- `crates/regseq-driver/src/callbacks.rs`,
  `crates/regseq-driver/src/lib.rs` -- current `Callbacks`-based
  implementation.

## Q5. Test machinery and serialization format

**Decision**:

- *Test machinery*: `insta` snapshot tests via `cargo insta review`.
- *Primary serialization format*: custom textual pretty-printer
  (one-way, output-only). MIR-inspired rendering used for the
  `model` emit-spec output, insta snapshots, and visual
  inspection.
- *Secondary serialization format*: serde-derived JSON. Used for
  external tooling, archival, and round-trip.

These decisions are coupled. The test-machinery choice constrains
the snapshot format; the primary-serialization choice determines
what humans read. Recorded as one Q because they were decided
together and depend on each other.

### Rationale (test machinery)

The convergent prior-art signal is FileCheck/lit (MLIR, LLVM,
`rustc_codegen_c`). We are consciously not following it:

- Our test pattern is snapshot-of-full-RegseqModel for synthetic
  linguistic snippets, not pattern-match-against-textual-IR.
  FileCheck's pattern-match strength does not pay off at our test
  volume.
- `insta` integrates with `#[test]` and `cargo test`. FileCheck/lit
  requires a Python-driven runner that is separate from Cargo.
- `cargo insta review` is the equivalent of
  `update_test_checks.py` -- the "regenerate expectations" piece
  that keeps snapshot tests fresh under iteration. Free with
  insta; non-trivial to build on FileCheck.

Going against the prior-art signal is reasonable: those projects
have orders of magnitude more tests, deal with backward-compat we
do not have, and are general-purpose IRs.

### Rationale (serialization format)

The dominant *reading* workload for RegseqModel is golden-C
authoring against pl011 and subsequent device-pressure work in
phase 2. Every body, every device, requires a human to read
RegseqModel and produce equivalent C. Human readability is a
load-bearing requirement, not a nice-to-have.

Structured-format options (RON, JSON, YAML) are all tractable but
verbose and cognitively heavy for sustained reading. A custom
textual pretty-printer, MIR-inspired, gives a format that:

- Mirrors what humans already read from rustc's `--emit=mir`.
- Resolves ID indirection inline (e.g., `TyId(7)` rendered as
  `Ty/u32`) so reading does not require cross-referencing.
- Renders bb-by-bb so control flow is visible at a glance.

The cost is writing the pretty-printer up front. It is bounded: a
`Display` impl (or dedicated `pretty.rs` module) per major IR
variant, plus a top-level renderer for the bb-by-bb structure. It
grows organically with the bottom-up staging -- stage 1 needs only
table-and-id rendering; stage 11 needs full body rendering.

JSON serves the secondary role: structured, serde-derived (free
off `Serialize` / `Deserialize`), used for tooling and archival.
JSON over RON because external tooling universally speaks JSON;
RON is Rust-niche.

### Subordinate notes

- **Textual pretty-printer is one-way.** No parser is needed.
  RegseqModel is produced only by the bridge; the textual format
  is a rendering for human consumption.
- **JSON is round-trippable via serde.** If round-trip is ever
  needed (e.g., loading a saved RegseqModel into an external tool
  for analysis), JSON is the format that supports it.
- **insta snapshots use the textual format.** `assert_snapshot!`
  with the pretty-printed text. Same format users read for
  golden-C authoring; same format reviewers see in test diffs.
- **Pretty-printer is treated as a stable interface.** Intentional
  changes trigger a single regenerate-all step (`cargo insta
  review` then accept). Snapshot churn from rendering changes is
  rare and visible.
- **Stricter-assertion escape hatch.** If a particular test wants
  to assert exact IR shape independent of rendering,
  `assert_json_snapshot!` is available off the same `Serialize`
  derive.
- **FileCheck-compatible format.** The textual pretty-printer is
  line-oriented and stable; if a specific test would benefit from
  pattern-match assertions over a subset of the output (rather
  than full-snapshot), FileCheck-style matching can be applied to
  the same textual format. We do not invest in FileCheck
  infrastructure up front, but the format choice deliberately
  preserves the option.
- **Multiple output formats from the `model` emit spec.** Default:
  textual pretty-print. Opt-in: `--format=json` for structured
  output.

### Reconsidering this decision

Re-open the question if any of the following appears:

- Pretty-printer maintenance becomes painful at scale -- fall back
  to JSON snapshots and keep the pretty-printer for human emit
  only.
- A use case for parsing the textual format (round-trip from
  human-edited dumps) -- revisit; the current textual format is
  output-only by design.
- Test volume grows to where pattern-match assertions save real
  effort over snapshots -- revisit FileCheck-style or `insta` with
  custom matchers.

### References

- `prior-art/mlir.md`, `prior-art/llvm-ir.md`,
  `prior-art/rustc-codegen-c.md` -- FileCheck/lit precedent we are
  consciously not following, with reasons.
- `prior-art/README.md` -- fork D (test machinery) and fork E
  (serialization format) sources.
- rustc's `--emit=mir` output format -- the reference for the
  textual pretty-printer's shape.

## Q6. Multi-crate strategy: separate `regseq` facade crate

**Decision**: restore the `regseq` facade as a separate crate
(reverting the proof-of-concept-stage collapse to a `crate::regseq`
submodule). Bring back the three-tier wrapper dispatch
(`PassthroughCallbacks` / `RegseqCallbacks` / `RegseqAwareCallbacks`)
with cargo-metadata-based recognition of regseq-aware crates.

### Rationale

The proof-of-concept-stage collapse moved `regseq` into a
`crate::regseq` submodule of each spec to eliminate cross-crate
infrastructure complexity (three-tier dispatch, cargo-metadata
routing, `RegseqAwareCallbacks`, cross-crate MIR encoding). At the
time, hand-lowering against cross-crate MIR seemed challenging and
the bridge layer that would normalize cross-crate-ness did not
exist.

With Q1-Q5 settled, two structural shifts make cross-crate handling
tractable:

1. **RegseqModel as a normalizing layer** (Q2). The bridge produces
   a unified, monomorphized graph regardless of source crate;
   `'tcx` terminates at the bridge boundary; downstream consumers
   see one shape.
2. **`core` opaque + everything else walked** methodology. Walking
   a facade body via `tcx.optimized_mir(def_id)` is the same
   operation as walking a spec body; cross-crate-ness is invisible
   post-monomorphization.

Benefits of the separate crate:

- The facade can be developed independently from any specific
  spec.
- Facade work does not have to be replicated across driver crates.
- The architecture matches the original intent (DSL primitives in
  a shared library).

### Subordinate notes

- **Three callbacks tier:**
  - `PassthroughCallbacks`: empty `rustc_driver::Callbacks` impl.
    For cargo probes, dependencies that are not aware, anything
    else.
  - `RegseqCallbacks`: full extraction pipeline. Sets
    `mir_opt_level = Some(0)`, `always_encode_mir = true`,
    `mir_enable_passes = regseq_mir_passes()`,
    `overflow_checks = Some(false)`,
    `debug_assertions = Some(false)`. Sets `no_codegen` (extraction
    stops after analysis). For the primary spec.
  - `RegseqAwareCallbacks`: same MIR shape contract as
    `RegseqCallbacks`, sourced from the shared
    `regseq_mir_passes()` so they cannot drift. Does NOT set
    `no_codegen` and does NOT override `after_analysis` -- rustc
    must produce normal `rmeta` / `rlib` outputs because downstream
    specs link against them.
- **Recognition via cargo metadata.** Aware crates declare
  `[package.metadata.regseq]` in their `Cargo.toml`. `cargo-regseq`
  runs `cargo metadata`, walks packages, and injects names as
  `REGSEQ_AWARE_CRATES` (comma-separated env var) into the build
  environment. Presence of the table is the marker; the `role`
  field is informational.
- **Dispatch precedence:** Passthrough (cargo probes / no
  `--target`) -> Regseq (primary package + `REGSEQ_OPTIONS`) ->
  RegseqAware (matches `REGSEQ_AWARE_CRATES`) -> Passthrough.
  Primary check beats aware check; deterministic for crates that
  are both.
- **Symmetric `register_tool` rule.** The aware crate's source
  declares its own `#![feature(register_tool)]` /
  `#![register_tool(rseq)]`. Neither callback injects this.
  Discipline lets `cargo check` work directly on aware crates
  without the wrapper being active.
- **Same-compiler-options requirement.** Both `RegseqCallbacks` and
  `RegseqAwareCallbacks` use `regseq_mir_passes()` to ensure the
  cleanup whitelist is identical. Any future divergence requires a
  justification.
- **`always_encode_mir = true`** ensures upstream-crate MIR is in
  the `.rmeta` and consumable by the bridge via
  `tcx.optimized_mir(def_id)` for cross-crate `DefId`s.

### Implementation plan

The facade collapse is reverted in a single commit (drops the
three-tier code, deletes the facade crate, updates fixtures and
dump script). The rebuild proceeds in subsequent sessions with
these modifications relative to the historical implementation in
commit `f0b2db1`:

1. **Updated `regseq_mir_passes()` whitelist.** The historical pass
   list is conservative (UB-check force-offs only); the rebuild
   aligns with the dump-script whitelist
   (`+RemoveStorageMarkers,+RemoveZsts,+SimplifyLocals-final,+ReorderBasicBlocks,+ReorderLocals`
   plus the UB-check force-offs).
2. **Bridge integration aligned with Q1-Q5.**
   `RegseqCallbacks::after_analysis` invokes the bridge to produce
   RegseqModel (per Q2 eager translation), not the older
   construction pipeline.
3. **Tests rebuilt against the new pipeline.** `aware_routing.rs`
   and `aware_compile.rs` (the latter `--ignored` by default)
   verify cargo-metadata recognition and end-to-end facade
   compilation under `RegseqAwareCallbacks`.
4. **Facade contents -- start empty, populate incrementally.**
   The new `regseq` crate is created empty: just the package
   manifest with `[package.metadata.regseq]`, a minimal `lib.rs`,
   and whatever `register_tool` declarations the symmetric rule
   requires. Items currently inlined into each spec's
   `crate::regseq` submodule stay there during the rebuild; each
   one is promoted to the shared `regseq` crate on its own
   schedule, as part of the redesign work that touches it. This
   avoids a bulk copy of `f0b2db1`'s facade contents that would
   not match the new architecture, and lets per-item redesign
   happen against the new architecture without churn.
5. **Document the symmetric `register_tool` rule in code.** Add
   explanatory comment blocks at the points where an agent might
   be tempted to violate the rule:
   - `regseq` crate `lib.rs` -- explain that
     `#![feature(register_tool)]` and `#![register_tool(rseq)]`
     are deliberately source-side and must not be removed; without
     them `cargo check` on the facade fails outside the wrapper.
   - `RegseqAwareCallbacks::config` -- comment block listing what
     is intentionally NOT injected (no `register_tool`, no
     `no_codegen`, no `after_analysis`) and why.
   - `RegseqCallbacks::config` -- reciprocal comment that
     `register_tool` is also not injected here, with a pointer to
     the methodology rule in `memory.md`.

### Re-open trigger

If the rebuilt three-tier dispatch leaks cross-crate complexity
into the bridge or into RegseqModel construction in ways that the
normalizing-layer abstraction does not actually contain,
reconsider. The Q6 expectation is that cross-crate-ness terminates
at the bridge boundary and is invisible to consumers; failure of
that expectation would be the signal to revisit.

### References

- Commit `f0b2db1` "feat: three-tier wrapper dispatch for
  regseq-aware crates" -- structural reference for the rebuild.
- `prior-art/charon.md` -- Charon validates the rustc-options set
  used by both callbacks (`always_encode_mir`, `mir_opt_level=0`,
  `mir_emit_retag`, `mir_preserve_ub`, disabling `CheckAlignment`).
- Q1, Q2, Q3 in this document -- the architectural decisions that
  make cross-crate handling tractable.

## Q7. Intrinsic recognition: by attribute name, not by path

**Decision**: the bridge recognizes domain intrinsics by the
`NAME` argument of `#[rseq::intrinsic("NAME")]` on the callee, not
by the callee's canonical path or `DefId`. Intrinsic names form a
*closed* vocabulary; a function attributed with a name not in the
vocabulary is rejected by the bridge, per the no-defensive-fallbacks
methodology.

### Rationale

Path-based recognition couples the recognition mechanism to a
specific crate layout. The bridge would need a fixed list of
`(crate, path)` pairs that designate intrinsics. This breaks down
when:

- An item is moved from one module to another (path changes;
  recognition rule must update).
- An item is moved between crates (e.g., from a spec's
  `crate::regseq` submodule to the shared `regseq` crate per Q6's
  incremental promotion plan -- the move would invalidate
  path-based rules).
- A spec wants to define an intrinsic locally during development
  (path will not match the canonical pattern).

Attribute-based recognition is invariant under all of these. The
intrinsic identity travels with the item. The `regseq` crate
becomes a *convenient home* for shared intrinsic stubs, not a
recognition oracle: it just so happens that intrinsic stubs live
there for reusability.

### Subordinate notes

- **Closed vocabulary.** The bridge maintains a registry of valid
  intrinsic names (likely a `const &[&str]` or an `enum
  IntrinsicKind` with a `from_name` method). Items declaring an
  intrinsic name not in the registry are rejected at the bridge
  with a diagnostic, not silently treated as regular function
  calls. Vocabulary updates are deliberate code changes in the
  bridge.
- **Recognition site is the callee, not the call site.** The
  bridge looks up `#[rseq::intrinsic(...)]` on the callee's
  `DefId` via `tcx.get_attr` (or equivalent for the `rseq` tool
  attribute namespace). The attribute travels with the function
  definition.
- **Cross-crate compatibility.** `tcx.get_attr` works on any
  `DefId`, including upstream-crate items. Combined with
  `always_encode_mir = true` (per Q6), this means a stub in the
  shared `regseq` crate is recognized identically whether the
  call is intra-crate or cross-crate.
- **Downstream stages reference intrinsics by `IntrinsicKind`.**
  Once the bridge identifies an intrinsic and tags the call site
  with its `IntrinsicKind`, subsequent passes (lowering, error
  analysis, etc.) reference the intrinsic by enum value, not by
  path string. Path is captured only for diagnostics.
- **No path coupling anywhere in the recognition rule.** A
  consequence worth restating: the bridge does not maintain
  "regseq::busy_wait is the busy_wait intrinsic." It maintains
  "any function with `#[rseq::intrinsic('busy_wait')]` is the
  busy_wait intrinsic, and the vocabulary has `busy_wait` in it."

### Re-open trigger

If we ever want to permit *additional* intrinsics defined per-spec
(i.e., the vocabulary becomes open per-driver), reconsider. The
current methodology (closed vocabulary, central registry)
explicitly rejects this.

### References

- Q6 -- intrinsic items live in the shared `regseq` crate by
  convention but the recognition rule does not depend on their
  location.
- Methodology rule (`memory.md`): `core` is opaque (lang-item
  recognition); domain intrinsics are name-recognized via
  attribute; items outside the recognized vocabulary are rejected.

## Implications for stages 1-13

### Stage 1 (vocabulary) is heavier than originally sketched

Stage 1 now specs the entire table-and-id infrastructure that the
rest of RegseqModel rides on:

- All ID types (`ItemId`, `TyId`, `BodyId`, `BlockId`, `LocalId`,
  `SpanId`, `StringId`, `SourceFileId`, `ConstId`, ...). Uniform
  convention to be picked (probably `NonZeroU32` newtypes).
- `StringInterner` shape.
- `SourceFile` shape: path, optional content, line table for
  `Span -> (line, col)` resolution.
- DefPath form for human-readable identifiers, including
  generic-args fingerprint.
- The top-level `RegseqModel` struct: a record of tables (items,
  types, bodies, source files, ...) plus a root index into them.

### Stage 2 (serialization) is decided at the format level

Format choice settled by Q5: a custom textual MIR-inspired
pretty-printer (primary, output-only, used for the `model` emit
spec, insta snapshots, and human reading for golden-C authoring),
plus serde-derived JSON (secondary, structured, for tooling and
archival). Schema-level work continues per-stage as variants gain
`Serialize`/`Deserialize` derives and pretty-printer output.

### Stages 3-11 are unaffected at the architectural level

Each variant in types, places, operands, rvalues, statements,
terminators, blocks, bodies is owned. No `'tcx` references appear.
Per-stage specifications proceed against the stage-1 vocabulary
without re-litigating the architecture.

### Stage 12 (items) inherits convergent pattern 5 and Q7

The bridge collapses MIR's many shim/coroutine variants into a
small RegseqModel-side set, with rustc-side queries recovering
detail during translation. Item-kind classification is populated
at the bridge: lang-item identity for `core` recognition (per the
opaque-`core` methodology), drop-glue handling, and -- per Q7 --
domain-intrinsic recognition by `#[rseq::intrinsic("NAME")]`
attribute lookup against the bridge's closed name vocabulary.

### Stage 13 (domain extensions) is independent of Q2

Domain extensions ride on top of stages 1-12 and do not interact
with the `'tcx` boundary directly.

## Open architectural decisions

No open architectural decisions at present. All current decisions
(Q1-Q7) are closed above. Future architectural decisions encountered
during stage work should be appended above as new Q sections, with
references to the prior-art evidence and rationale.
