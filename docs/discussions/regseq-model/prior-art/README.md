# Prior-Art Synthesis

This folder collects research into existing projects that compile Rust
to C, extract Rust into a custom IR for a different domain, or design
layered/serializable IRs more broadly. The goal: avoid re-deriving
solutions that are already well-explored, and identify the places
where deliberate decisions are required because precedent disagrees
or doesn't apply.

This README synthesizes across the per-project files. The per-project
files are factual descriptions; this README is a load-bearing reading
of what the body of prior art tells us. Treat the per-project files
as evidence and this README as conclusions drawn from the evidence.

## Convergent patterns (don't reinvent)

Places where multiple tracks landed on the same answer. Treat as
settled-by-precedent rather than open questions.

### 1. Two-crate architecture: extractor (rustc-facing) + IR crate (no rustc deps)

`rustc_codegen_c` and `rustc_public`/`rustc_public_bridge` both use
this shape. The IR crate carries no `'tcx`, no `rustc_private`, no
internal-rustc imports. The extractor crate carries the bridge logic.
The split is the cleanest structural answer to the `'tcx` boundary
problem.

See: `rustc-codegen-c.md`, `rustc-public.md`.

### 2. The `'tcx` boundary is a *central* design problem

`rustc_public`'s PR #115934 was specifically driven by repeated `'tcx`
leak bugs. The current architecture confines `'tcx` entirely to the
bridge crate via three layered mechanisms: scoped TLS, `RefCell<Tables<'tcx, _>>`,
and a `PhantomData<*const ()>` ZST (`ThreadLocalIndex`) that makes
public indices `!Send`/`!Sync`. Plan for this structurally up front;
discovering it later is painful.

See: `smir-history.md`, `rustc-public.md`.

### 3. `CodegenBackend` is the rustc-supported integration point

`rustc_codegen_c`, `rustc_codegen_clr`, and Kani all plug in via
`CodegenBackend` / `-Zcodegen-backend`, not `rustc_driver::Callbacks`.
The codegen-backend route gives free access to `rustc_codegen_ssa`'s
mono-item walking and CGU partitioning -- which is most of what
extraction has to do anyway.

See: `rustc-codegen-c.md`, `rustc-codegen-clr.md`, `kani.md`.

### 4. Bake monomorphization in the IR builder, not deferred

`rustc_public`'s `BodyBuilder` runs `instantiate_mir_and_normalize_erasing_regions`
and rewrites `ConstOperand` -> `Const::Val` eagerly. Kani similar.
Charon supports both modes but defaults to baking via `--monomorphize`.
Consumers see fully resolved bodies; they don't re-run substitution.

See: `rustc-public.md`, `kani.md`, `charon.md`.

### 5. Lossy projection + side-channel queries

For things with too-many-variants-to-carry-faithfully. `rustc_public`'s
`InstanceKind` collapses 14 rustc-internal variants to 4 public ones,
with side-channel queries (`is_empty_drop_shim`, `intrinsic_name`)
recovering the lost detail when actually needed. Directly transferable
to RegseqModel collapsing MIR's many shim/coroutine variants while
preserving access to detail when a downstream pass actually wants it.

See: `rustc-public.md`.

### 6. Opaque escape hatches enable incremental rollout

`rustc_public` uses `Opaque(String)` for `Coverage`/`LocalDefId` and
`raw_rpr: String` for `InlineAsmOperand`. The IR ships and exercises
its consumers before every corner is fully designed. Right pattern for
our bottom-up rollout: not-yet-spec'd parts ship as opaque-tagged
stubs while consumers iterate against the parts that *are* nailed
down.

See: `rustc-public.md`.

### 7. Closed-vocabulary Rust enums beat open-vocabulary dialects

CompCert's separate-IR-per-layer (Csyntax / Clight / Cminor / RTL),
Verus's VIR-AST / VIR-SST / AIR, Charon's ULLBC/LLBC, Prusti's
5-tier ladder -- all use closed enums. MLIR's dialect/op framework
is open-vocabulary and is overkill at our scale. The macro boilerplate
for closed-enum layered IRs is real but tractable.

See: `compcert.md`, `verus.md`, `charon.md`, `prusti.md`, `mlir.md`.

## Real forks in the road (require explicit decision)

Places where prior art disagrees, or where the convergent pattern
diverges from current Regseq state and requires a conscious call.

### A. `CodegenBackend` or `Callbacks`? (RESOLVED)

Stay with `Callbacks`. The convergent precedent (codegen_c,
codegen_clr, Kani) comes from projects whose primary job is codegen
replacement; Regseq's primary job is producing RegseqModel and
multiple downstream artifacts (`model` / `analysis` / `driver`),
which fits `Callbacks`'s post-`after_analysis` flexibility better
than `CodegenBackend`'s per-CGU + link API. See `../architecture.md`
(Q4) for the full requirements matrix and rationale, plus the
trigger conditions for re-opening the question.

### B. Extraction point: `optimized_mir` + `mir_opt_level=0` + cleanup whitelist (RESOLVED)

Maintain the current setup. The configuration is principled
(post-mono so generics are resolved; `mir_opt_level=0` so
optimization does not obscure driver intent; cleanup whitelist for
hand-readable shape) and externally validated: Charon uses a
closely-matching rustc-options set (`always_encode_mir`,
`mir_opt_level=0`, `mir_emit_retag`, `mir_preserve_ub`, disabling
`CheckAlignment`), differing only in extraction level. The
unusualness flagged by the survey is an observation, not a problem.
Operational commitment lives in `scripts/dump-fixture-mir.sh` and
`../../manual-lowering/README.md`.

### C. Single-level enum or layered IR? (RESOLVED)

Single-level. RegseqModel is one hierarchy of enums; successive
compiler stages refine the same IR rather than producing new ones.
LLVM-style, not CompCert-style. See `../architecture.md` (Q3) for
the decision and rationale, including how domain extensions ride
inside the same IR rather than as a separate layer.

### D. Test machinery: FileCheck or `insta`? (RESOLVED)

`insta` snapshot tests via `cargo insta review`. Going against the
FileCheck/lit prior-art signal because our test pattern is
snapshot-of-full-RegseqModel for synthetic linguistic snippets
(not pattern-match-against-textual-IR), our test volume does not
warrant FileCheck's complexity, and `insta` is Rust-native (no
Python/lit overhead). See `../architecture.md` (Q5) for the
decision and rationale.

### E. Serialization format (RESOLVED)

Two formats: a custom textual pretty-printer (primary, MIR-inspired,
output-only, used for the `model` emit spec, insta snapshots, and
human reading for golden-C authoring) plus serde-derived JSON
(secondary, structured, used for tooling and archival). The
primary-format choice is driven by phase-2 device-pressure work,
where reading RegseqModel to author golden C is the dominant
workload. See `../architecture.md` (Q5) for the decision and
rationale.

## What's specific to us (no direct precedent)

- **`#![no_std]` driver-spec surface is small.** Every prior-art
  project handles whole-language Rust + std + every construct. We
  can be more aggressive about rejection at the IR level.
- **Pinned toolchain.** `rustc_public`'s stability machinery (the
  `Bridge` trait, `0.1.0-preview` versioning, `rustc_internal`
  re-export, AutoUpgrade-style cross-version migration) does not
  apply. We get most of the patterns without the stability cost.
- **Domain-specific extensions** (intrinsics, ordered regions,
  register access, ISR classification, error-cleanup analysis) are
  unique to Regseq. Best-fit pattern: layered IR with domain
  extensions riding above MIR-mirror primitives -- which interacts
  with fork C.

## Implications for the bottom-up staging plan

The 13-stage bottom-up plan stays sound. Three additions that emerge
from the prior art:

### Stage 0a: crate architecture (RESOLVED)

Two-crate split: bridge inside `regseq-driver` as internal modules,
RegseqModel in `regseq-model` (zero rustc deps). See
`../architecture.md` (Q1) for the decision and rationale.

### Stage 0b: `'tcx` boundary policy (RESOLVED)

Eager translation, fully owned RegseqModel. Bridge holds transient
internal Tables; no TLS / RefCell-Tables / PhantomData-ZST machinery.
See `../architecture.md` (Q2) for the decision and rationale,
including the cascade into stage 1 (internal IDs, StringInterner,
SourceFile table, self-describing IR).

### Cross-cutting decision around stage 3: layered-vs-single-IR

Fork C above. Not a stage in itself. The answer affects whether the
type system in stage 3 is shared across layers or layer-specific,
and whether stages 5-11 produce per-layer variants. Worth deciding
no later than stage 3.

## Per-project file index

Files in this folder, grouped by research track:

### Rust to C compilers

- `mrustc.md` -- standalone C++ Rust compiler that bootstraps rustc;
  owns its own AST/HIR/MIR; explicit named MIR cleanup passes; rich
  drop-scope model.
- `c2rust.md` -- C-to-Rust translator; `Translation` context-object
  pattern, `ExprContext` per-expression carrier, comment/source-location
  preservation, bitfield-derive design.
- `rustc-codegen-c.md` -- rust-lang Rust-to-C backend; the closest
  single precedent we have. Two-crate split (rustc-facing extractor
  + `rustc_codegen_c_ast` arena+pretty-printer with no rustc deps).
- `rustc-codegen-clr.md` -- Rust-to-CIL backend; "one IR, two printers"
  pattern (~2-3K LOC for the second backend) directly precedented
  for our Linux/Zephyr split.

### Rust to custom-IR for verification

- `charon.md` -- Aeneas/Charon; emits ULLBC/LLBC; validates rustc
  options similar to Regseq's; supports both polymorphic and
  monomorphic modes.
- `kani.md` -- consumes MIR, emits CBMC GOTO; uses `CodegenBackend`
  + Stable MIR.
- `creusot.md` -- emits Why3 logic; `cleanup_spec_closures` in an
  overridden `mir_built` provider for excising DSL-only constructs.
- `prusti.md` -- emits Viper; 5-tier IR ladder.
- `verus.md` -- verification tool with a Rust dialect; VIR-AST /
  VIR-SST / AIR layers.
- `rust-stainless.md` -- extracts to Stainless verification IR from
  THIR.
- `mirai.md` -- abstract interpretation over raw MIR.

### rustc's own stable / owned IR extraction

- `rustc-public.md` -- current state of `rustc_public` /
  `rustc_public_bridge`; the bridge pattern, ID-table design,
  `'tcx` confinement mechanisms.
- `smir-history.md` -- SMIR design history; PR #115934 split; what
  was deferred or rejected and why.

### IR design patterns (general)

- `mlir.md` -- multi-level IR with dialects; textual `.mlir` + binary
  bytecode; lit + FileCheck testing.
- `compcert.md` -- verified C compiler; separate IR types per layer
  with total-function lowerings; no serialization, Coq-only.
- `llvm-ir.md` -- textual `.ll` + bitcode; `AutoUpgrade.cpp` for
  cross-version compat; `update_test_checks.py` makes FileCheck
  testing scale.
