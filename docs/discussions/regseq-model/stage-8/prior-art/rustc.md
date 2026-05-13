# Stage 8 -- Prior Art: rustc primary source

Primary-source rustc evidence for `mir::Statement<'tcx>` /
`mir::StatementKind<'tcx>` and the surrounding pipeline (cleanup
passes, runtime-lowering passes, codegen consumer, MIR validator),
scoped to what stage 8's candidate decision namespace
(`D-8-1` Statement variant set + per-variant payload shapes,
sub-tagged `D-8-1.1` cardinality / `D-8-1.2` Assign payload /
`D-8-1.3` SetDiscriminant payload / `D-8-1.4` Intrinsic payload;
`D-8-2` no-interning / inline storage; `D-8-3` Statement wrapper
struct (`source_info`, `debuginfos`); `D-8-4` site invariants +
rejection rules; `D-8-K` `render_statement` deferral) needs.
Fresh pass per `../../README.md` §2 methodology rule (do not
reuse stage-0 summaries).

All paths below are relative to the pinned-toolchain root:

```
~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/
```

(See `../../rustc-references.md` for the canonical statement of
this root path.)

Every factual claim cites a `[file:line]` in that pinned source.
Hypotheses are flagged HYPOTHESIS:. Inferences combining two
sources into a third are flagged INFERENCE:. The Regseq
compiler consumes `optimized_mir` outputs, so "what reaches the
bridge" means: the body returned by the `optimized_mir` query
after `run_analysis_to_runtime_passes` and `run_optimization_passes`
have completed. Stage 8's job is to specify the closed
RegseqModel `Statement` vocabulary the bridge fills from
`mir::StatementKind<'tcx>`.

---

## 1. `StatementKind<'tcx>` enum shape

`StatementKind<'tcx>` is a 13-variant closed enum
[`rustc_middle/src/mir/syntax.rs:309-472`]:

```rust
#[derive(Clone, Debug, PartialEq, TyEncodable, TyDecodable, Hash, HashStable)]
#[derive(TypeFoldable, TypeVisitable)]
pub enum StatementKind<'tcx> {
    Assign(Box<(Place<'tcx>, Rvalue<'tcx>)>),
    FakeRead(Box<(FakeReadCause, Place<'tcx>)>),
    SetDiscriminant { place: Box<Place<'tcx>>, variant_index: VariantIdx },
    StorageLive(Local),
    StorageDead(Local),
    Retag(RetagKind, Box<Place<'tcx>>),
    PlaceMention(Box<Place<'tcx>>),
    AscribeUserType(Box<(Place<'tcx>, UserTypeProjection)>, ty::Variance),
    Coverage(CoverageKind),
    Intrinsic(Box<NonDivergingIntrinsic<'tcx>>),
    ConstEvalCounter,
    Nop,
    BackwardIncompatibleDropHint {
        place: Box<Place<'tcx>>,
        reason: BackwardIncompatibleDropReason,
    },
}
```

Key facts:

- **13 variants, closed.** No `Other` / `Opaque` arm. Confirmed by
  the cardinality of the `name()` match
  [`rustc_middle/src/mir/statement.rs:48-64`].
- **Box indirection at six sites:** `Assign` (`Box<(Place,Rvalue)>`),
  `FakeRead` (`Box<(FakeReadCause,Place)>`), `SetDiscriminant`
  (`Box<Place>` for the place field), `Retag` (`Box<Place>`),
  `PlaceMention` (`Box<Place>`), `AscribeUserType`
  (`Box<(Place,UserTypeProjection)>`), and `BackwardIncompatibleDropHint`
  (`Box<Place>`); `Intrinsic` carries `Box<NonDivergingIntrinsic>`.
  These boxes are size-driven -- rustc pins
  [`syntax.rs:1753`]:

  ```rust
  static_assert_size!(StatementKind<'_>, 16);
  ```

  16 bytes: the discriminant tag plus a single pointer-width
  payload (the `Box`). `Place<'tcx>` is 16 bytes
  [`syntax.rs:1750`] and `Rvalue<'tcx>` is 40 bytes
  [`syntax.rs:1752`], so unboxing `Assign` would push
  `StatementKind` to ~64 bytes alone.

- **No `static_assert_size!` for `Statement<'tcx>` or
  `NonDivergingIntrinsic<'tcx>`** -- the size budget governs
  only the kind. INFERENCE: rustc tolerates a wider
  `Statement<'tcx>` because it carries `SourceInfo` (16 bytes)
  + `StmtDebugInfos` (a `Vec`, 24 bytes) on top of the kind,
  giving ~56 bytes total; the per-kind 16-byte budget is what
  matters for the variant-discrimination hot path.

- **Closed-surface candidate (post-pipeline) is 3 variants.**
  After the cleanup/lowering passes that the Regseq pipeline
  runs (analysed in §3), the only `StatementKind` arms that
  are simultaneously (a) reachable in `optimized_mir` outputs
  and (b) operationally meaningful (i.e. not validator-side
  no-ops or bridge-rejection candidates) are:
  - `Assign`
  - `SetDiscriminant`
  - `Intrinsic` (with two NonDivergingIntrinsic sub-variants)

  Every other arm is either NOPed by `CleanupPostBorrowck`
  (§3.1), removed by `RemoveStorageMarkers` / `RemovePlaceMention`
  (§3.2/§3.3), unreachable due to flag-gating (§3.5), or
  ICE-d by the post-codegen contract (§4.4).

Maps to `D-8-1.1` (variant cardinality) + `D-8-1.2` / `D-8-1.3` /
`D-8-1.4` (per-variant payload shapes).

---

## 2. Per-variant deep dive

The order below mirrors the source enum order. Each subsection
records: payload shape, where in the pipeline the variant is
emitted/discharged, what the codegen consumer does with it, and
the validator-side phase constraint (if any).

### 2.1 `Assign(Box<(Place, Rvalue)>)`

[`syntax.rs:341`].

Doc text [`syntax.rs:312-340`]: "Assign statements roughly
correspond to an assignment in Rust proper (`x = ...`) except
without the possibility of dropping the previous value (that
must be done separately, if at all). [...] **Assignments in
which the types of the place and rvalue differ are not
well-formed.**"

Type-equality invariant is enforced by the validator
[`rustc_mir_transform/src/validate.rs:1467-1482`]:

```rust
StatementKind::Assign(box (dest, rvalue)) => {
    // LHS and RHS of the assignment must have the same type.
    let left_ty = dest.ty(&self.body.local_decls, self.tcx).ty;
    let right_ty = rvalue.ty(&self.body.local_decls, self.tcx);
    if !self.mir_assign_valid_types(right_ty, left_ty) {
        self.fail(
            location,
            format!("encountered `{:?}` with incompatible types: ..."),
        );
    }
    // ... DerefTemp special case (deref temps must use Rvalue::CopyForDeref)
}
```

`mir_assign_valid_types` [`validate.rs:580+`] is a permissive
equivalence (handles `&T <: &mut T` lifetimes-erased, etc.) but
it requires structural type equality at the runtime phase.

**Payload shape -- `Box<(Place, Rvalue)>`:**

- `Place<'tcx>` = 16 bytes [`syntax.rs:1750`].
- `Rvalue<'tcx>` = 40 bytes [`syntax.rs:1752`].
- Tuple is 56 bytes; boxed to keep the enum at 16 bytes.

**DerefTemp special-case** [`validate.rs:1484-1490`]: if
`dest.as_local()` is a local with `LocalInfo::DerefTemp`,
the `rvalue` must be `Rvalue::CopyForDeref(_)`. But stage 7
already established that `EraseDerefTemps`
[`rustc_mir_transform/src/lib.rs:658`] runs in
`run_runtime_lowering_passes` and eliminates `CopyForDeref`
along with all DerefTemp locals
(see stage-7 prior-art §11). INFERENCE: this special case is
unreachable in `optimized_mir` outputs; the bridge does not
need to model it.

Codegen consumer [`rustc_codegen_ssa/src/mir/statement.rs:14-49`]:
distinguishes "dest is a local" vs "dest is a place":

```rust
mir::StatementKind::Assign(box (ref place, ref rvalue)) => {
    if let Some(index) = place.as_local() {
        match self.locals[index] {
            LocalRef::Place(cg_dest) => self.codegen_rvalue(bx, cg_dest, rvalue),
            LocalRef::PendingOperand => {
                let operand = self.codegen_rvalue_operand(bx, rvalue);
                self.overwrite_local(index, LocalRef::Operand(operand));
                ...
            }
            LocalRef::Operand(op) => {
                if !op.layout.is_zst() {
                    span_bug!(... "operand {:?} already assigned", rvalue);
                }
                // ZST: still codegen the rvalue for side effects.
                self.codegen_rvalue_operand(bx, rvalue);
            }
            ...
        }
    } else {
        let cg_dest = self.codegen_place(bx, place.as_ref());
        self.codegen_rvalue(bx, cg_dest, rvalue);
    }
}
```

The split into `codegen_rvalue` (place-style, dest-passed) vs
`codegen_rvalue_operand` (value-style, returns OperandRef) is
the same one stage 7 §14 flagged: it is an SSA-builder
optimization, not a semantic distinction.

INFERENCE: a RegseqModel C backend can fold both paths into a
single `lower_assign(Place dst, Rvalue rhs) -> CStatement` since
C lacks rustc's abstract-builder distinction. The variant set
and the type-equality invariant are what stage 8 fixes.

Maps to `D-8-1.1` + `D-8-1.2` (Assign payload shape: flat
`(Place, Rvalue)` after stage-5 / stage-7 close their surfaces;
boxing is a 64-bit-target size-budget choice rustc makes that
RegseqModel inherits if-and-only-if RegseqModel has comparable
inline-storage pressure -- see §5 below) + `D-8-4` (Assign
type-equality invariant).

### 2.2 `FakeRead(Box<(FakeReadCause, Place)>)`

[`syntax.rs:343-358`].

Doc text [`syntax.rs:343-356`]: "When executed at runtime, this
is a nop. During static analysis, a fake read: requires that the
value being read is initialized [...] but, unlike an actual
read, does *not* invalidate any exclusive borrows. **Disallowed
after drop elaboration.**"

`FakeReadCause` [`syntax.rs:526-629`] has 5 variants
(`ForMatchGuard`, `ForMatchedPlace`, `ForGuardBinding`, `ForLet`,
`ForIndex`) -- borrowck-internal tags.

**Discharge.** `CleanupPostBorrowck::run_pass` NOPs every
`FakeRead`
[`rustc_mir_transform/src/cleanup_post_borrowck.rs:33-43`]:

```rust
StatementKind::AscribeUserType(..)
| StatementKind::Assign(box (_, Rvalue::Ref(_, BorrowKind::Fake(_), _)))
| StatementKind::Coverage(
    CoverageKind::BlockMarker { .. } | CoverageKind::SpanMarker { .. },
)
| StatementKind::FakeRead(..)
| StatementKind::BackwardIncompatibleDropHint { .. } => {
    statement.make_nop(true)
}
```

`CleanupPostBorrowck::is_required(): true`
[`cleanup_post_borrowck.rs:88-90`]. The pass runs in
`run_analysis_cleanup_passes` [`rustc_mir_transform/src/lib.rs:621`]
which runs unconditionally (no opt-level gate) before any of
the runtime-lowering / optimization passes. By the time the
bridge sees `optimized_mir`, every `FakeRead` is a `Nop`.

The validator confirms: at `MirPhase::Runtime(RuntimePhase::Initial)`
or later, `FakeRead` is a hard error
[`rustc_mir_transform/src/validate.rs:309-315, 1500-1506`]:

```rust
StatementKind::FakeRead(..) => {
    if self.body.phase >= MirPhase::Runtime(RuntimePhase::Initial) {
        self.fail(
            location,
            "`FakeRead` should have been removed after drop lowering phase",
        );
    }
}
```

`optimized_mir` is `MirPhase::Runtime(RuntimePhase::PostCleanup)`
[`rustc_mir_transform/src/lib.rs:612`] which is greater than
`Runtime(Initial)`, so the validator categorically prohibits
`FakeRead` in `optimized_mir` bodies.

Codegen consumer [`rustc_codegen_ssa/src/mir/statement.rs:91-97`]
treats `FakeRead` as a no-op (silent ignore, not bug!).
INFERENCE: this is defence-in-depth -- the consumer does not
ICE because there are non-codegen consumers (miri, MIR pretty
printer) that can reach `FakeRead` at earlier phases, but in
the `optimized_mir` -> codegen path it is unreachable.

**Bridge implication.** `FakeRead` is not reachable in
well-formed `optimized_mir`. The bridge can either:
1. Drop the `Nop`s (since `RemoveStorageMarkers` already does
   so when enabled -- see §3.2), folding the resulting empty
   slots out of `BasicBlock::statements`.
2. Reject `FakeRead` if seen, treating it as a defence-in-depth
   "unreachable per CleanupPostBorrowck" assertion.

Maps to `D-8-1.1` (variant cardinality -- discharged) +
`D-8-4` Part A (rejection-table entry: `FakeRead` ->
"NOPed by CleanupPostBorrowck (is_required: true) and
validator-rejected at Runtime(Initial)+; no bridge variant
needed").

### 2.3 `SetDiscriminant { place: Box<Place>, variant_index: VariantIdx }`

[`syntax.rs:360-365`].

Doc text [`syntax.rs:360-364`]: "Write the discriminant for a
variant to the enum Place. **This is permitted for both
coroutines and ADTs.** This does not necessarily write to the
entire place; instead, it writes to the minimum set of bytes as
required by the layout for the type."

**Phase constraint.** Validator
[`validate.rs:317-321, 1551-1567`]:

```rust
StatementKind::SetDiscriminant { place, .. } => {
    if self.body.phase < MirPhase::Runtime(RuntimePhase::Initial) {
        self.fail(location, "`SetDiscriminant`is not allowed until deaggregation");
    }
    let pty = place.ty(&self.body.local_decls, self.tcx).ty;
    if !matches!(
        pty.kind(),
        ty::Adt(..) | ty::Coroutine(..) | ty::Alias(ty::Opaque, ..)
    ) {
        self.fail(
            location,
            format!("`SetDiscriminant` is only allowed on ADTs and coroutines, not {pty}"),
        );
    }
}
```

So `SetDiscriminant` is the *opposite* of `FakeRead`: it is
*forbidden* before `Runtime(Initial)` and *required* after, as
the runtime form of enum-variant assignment that drop
elaboration / aggregate lowering produces.

**Target-type constraint.** `place.ty()` must be `ty::Adt`,
`ty::Coroutine`, or `ty::Alias(Opaque, ..)`. Coroutines are
discharged by `coroutine::StateTransform`
[`rustc_mir_transform/src/lib.rs:660`] which is `is_required: true`
[`rustc_mir_transform/src/coroutine.rs:1683-1685`] and runs in
`run_runtime_lowering_passes`. INFERENCE: in `optimized_mir`
outputs, coroutine bodies have already had their
`SetDiscriminant { place: <coroutine state>, ... }` rewritten
to `Assign` of an integer-typed discriminant local (the
`StateTransform` lowers coroutines into a state-machine ADT).

Stage 3 already rejects coroutine types
(`ty::Coroutine` / `ty::CoroutineWitness` are not in the
RegseqModel `Ty` surface; see `../../spec.md` §3 Type closure).
INFERENCE: `SetDiscriminant` reaching the bridge will only
target `ty::Adt(_, _)` enum places. `ty::Alias(Opaque, _)`
should be normalized away by `PostAnalysisNormalize`
[`rustc_mir_transform/src/lib.rs:642`] which runs in
`run_runtime_lowering_passes`; HYPOTHESIS: opaque aliases on
the LHS of `SetDiscriminant` cannot reach `optimized_mir` --
needs verification by reading
`post_analysis_normalize.rs`, deferred to stage-8 spec close.

**Payload shape -- struct (named fields, not tuple):**

```rust
SetDiscriminant { place: Box<Place<'tcx>>, variant_index: VariantIdx }
```

`VariantIdx` is an `IndexVec` newtype-index, 32 bits in this
nightly (search the rustc-index newtype crate for sizeof if
load-bearing). `place` is `Box<Place>` (16 bytes for `Place`
boxed to fit the 16-byte StatementKind budget).

rustc_public flattens this payload (see §6 below) -- it has
the freedom to skip the box because rustc_public's
`StatementKind` carries no `static_assert_size!`.

Codegen consumer [`rustc_codegen_ssa/src/mir/statement.rs:50-52`]:

```rust
mir::StatementKind::SetDiscriminant { box ref place, variant_index } => {
    self.codegen_place(bx, place.as_ref()).codegen_set_discr(bx, variant_index);
}
```

`codegen_set_discr` consults the layout to write either the
tag field (for `Multiple`-niche layouts) or compute the
niche-encoded value (for `Niche` layouts). C-backend implication
for stage 8: `SetDiscriminant` lowers to a memory write into
the discriminant byte(s) of an enum place at the offset
determined by the `Ty::Adt` layout -- this is a stage-14
backend concern, but stage 8 must preserve the `(place,
variant_index)` payload to enable that lowering.

Maps to `D-8-1.1` + `D-8-1.3` (SetDiscriminant payload: struct-named
field convention from rustc; Box the place or flatten? rustc's
choice is size-budget-driven; RegseqModel's choice depends on
whether `Statement` has a comparable inline-storage budget --
parallel question to D-7-8 BinaryOp pair-storage) + `D-8-4`
(SetDiscriminant target-type-must-be-Adt invariant; variant_index
in-range invariant).

### 2.4 `StorageLive(Local)` and `StorageDead(Local)`

[`syntax.rs:367-388`].

Doc text [`syntax.rs:367-384`]: "`StorageLive` and `StorageDead`
statements mark the live range of a local. [...] act like the
heap operations `allocate`/`deallocate`, but for stack-allocated
local variables. Using a local in any way (not only
reading/writing from it) while it is unallocated is UB. [...]
Some locals have no `StorageLive` or `StorageDead` statements
within the entire MIR body. These locals are implicitly
allocated for the full duration of the function."

**Discharge.** `RemoveStorageMarkers`
[`rustc_mir_transform/src/remove_storage_markers.rs`]:

```rust
impl<'tcx> crate::MirPass<'tcx> for RemoveStorageMarkers {
    fn is_enabled(&self, sess: &rustc_session::Session) -> bool {
        sess.mir_opt_level() > 0 && !sess.emit_lifetime_markers()
    }
    fn run_pass(&self, _tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        for data in body.basic_blocks.as_mut_preserves_cfg() {
            data.retain_statements(|statement| match statement.kind {
                StatementKind::StorageLive(..)
                | StatementKind::StorageDead(..)
                | StatementKind::Nop => false,
                _ => true,
            })
        }
    }
    fn is_required(&self) -> bool { true }
}
```

The `is_required(): true` is a pass-manager hint that this pass
must run when enabled (i.e., it cannot be turned off via
`#[optimize(none)]`). The pass-manager rule for enablement is
[`rustc_mir_transform/src/pass_manager.rs:213-239`]:

```rust
pub(super) fn should_run_pass<'tcx, P>(
    tcx: TyCtxt<'tcx>,
    pass: &P,
    optimizations: Optimizations,
) -> bool { ... }
// ...
let suppressed = !pass.is_required() && matches!(optimizations, Optimizations::Suppressed);
overridden.unwrap_or_else(|| !suppressed && pass.is_enabled(tcx.sess))
```

INFERENCE (CRITICAL): `is_required` only matters in the
`Optimizations::Suppressed` case (e.g., `#[optimize(none)]` on
the function). Otherwise the pass runs IFF `is_enabled` returns
true. `RemoveStorageMarkers::is_enabled` returns
`mir_opt_level() > 0 && !emit_lifetime_markers()`. So:

- At `mir_opt_level >= 1` (the default; see
  `rustc_session/src/session.rs:573-578`: when unset, defaults
  to 2 if `OptLevel != No`, else 1) AND no sanitizers / not
  optimizing -> the pass runs and removes
  `StorageLive` / `StorageDead` / `Nop`.
- At explicit `mir_opt_level = 0` -> the pass does NOT run;
  `StorageLive`/`StorageDead`/`Nop` survive.
- At `OptLevel != No` (i.e., `-O` or higher) AND
  `emit_lifetime_markers() == true` -> the pass does NOT run;
  storage markers are preserved for LLVM lifetime intrinsics
  [`rustc_session/src/session.rs:510-516`].

The Regseq pipeline pins `mir_opt_level = 0` per the top-level
README. **At that setting, `RemoveStorageMarkers` does NOT
run.** This contradicts the README's claim that
`RemoveStorageMarkers` is "in the whitelist". Two possible
resolutions:

1. The Regseq pipeline currently *plans* for
   `mir_opt_level = 0` plus `RemoveStorageMarkers` enabled via
   `-Z mir-enable-passes=+RemoveStorageMarkers`
   [`rustc_session/src/options.rs:2476-2479`] which lets the
   user force-enable a pass against its `is_enabled` check (see
   `should_run_pass` line 227-236 -- `overridden.unwrap_or_else`).
2. The pipeline implicitly assumes `mir_opt_level = 1` (the
   no-`-O` default) and the README's "0" is shorthand for
   "Rust source -O0" rather than the rustc-internal
   `mir_opt_level` knob.

NOTE: Stage 8's design must clarify which discipline applies.
Either way, the bridge cannot rely on `StorageLive` /
`StorageDead` being absent from `optimized_mir` -- they may
or may not be present depending on session flags.

**Bridge implication.** Stage 8 has three options for storage
markers:

1. **Reject:** treat `StorageLive` / `StorageDead` as
   "should not occur in well-formed pinned-pipeline MIR" and
   raise a hard diagnostic if seen. Brittle if option (2) above
   is the actual pipeline state.
2. **Drop:** lower both to `// ignored` C comments. Codegen
   already treats them as effective no-ops at the C level
   (driver C code does not need explicit storage liveness;
   the C compiler manages stack frames).
3. **Preserve:** carry `StorageLive(LocalId)` /
   `StorageDead(LocalId)` as RegseqModel `Statement` variants
   for round-trip fidelity. Preserves miri-style aliasing
   semantics if RegseqModel ever feeds back into a Rust
   verification tool.

Codegen consumer [`rustc_codegen_ssa/src/mir/statement.rs:53-66`]:
emits `cg_place.storage_live(bx)` / `storage_dead(bx)` which
become LLVM `lifetime.start` / `lifetime.end` intrinsics. C
output has no equivalent.

Cranelift consumer
[`rustc_codegen_cranelift/src/base.rs:603, 907-908`]:
silently drops both ("Those are not very useful").

Maps to `D-8-1.1` (cardinality -- need to decide preserve / drop /
reject) + `D-8-4` Part A (rejection-table entry conditional on
pipeline-state contract).

### 2.5 `Retag(RetagKind, Box<Place>)`

[`syntax.rs:390-401`].

Doc text [`syntax.rs:390-400`]: "Retag references in the given
place, ensuring they got fresh tags. This is part of the
Stacked Borrows model. **These statements are currently only
interpreted by miri and only generated when `-Z mir-emit-retag`
is passed.** [...] Only `RetagKind::Default` and
`RetagKind::FnEntry` are permitted."

`RetagKind` [`syntax.rs:511-522`]: 4 variants -- `FnEntry`,
`TwoPhase`, `Raw`, `Default`. The validator forbids `TwoPhase`
[`validate.rs:322-329, 1568-1574`]:

```rust
StatementKind::Retag(kind, _) => {
    if matches!(kind, RetagKind::TwoPhase) {
        self.fail(location, format!("explicit `{kind:?}` is forbidden"));
    }
}
```

**Discharge / emission.** `AddRetag::is_enabled`
[`rustc_mir_transform/src/add_retag.rs:50-52`]:

```rust
fn is_enabled(&self, sess: &rustc_session::Session) -> bool {
    sess.opts.unstable_opts.mir_emit_retag
}
```

`mir_emit_retag` defaults to `false`
[`rustc_session/src/options.rs:2473-2475`]. INFERENCE: `Retag`
statements never appear in `optimized_mir` outputs unless the
user passes `-Z mir-emit-retag`. The Regseq pipeline does not
opt into this flag (per the README's flag inventory), so
`Retag` is unreachable.

Codegen consumer [`rustc_codegen_ssa/src/mir/statement.rs:91-97`]:
treats `Retag` as a no-op alongside `FakeRead`, `AscribeUserType`,
etc. Cranelift consumer
[`rustc_codegen_cranelift/src/base.rs:907-916`]: same.

Maps to `D-8-1.1` (cardinality -- discharged by absent flag) +
`D-8-4` Part A (rejection-table entry: `Retag` ->
"unreachable when `-Z mir-emit-retag` is off (default); reject
with diagnostic if seen").

### 2.6 `PlaceMention(Box<Place>)`

[`syntax.rs:403-409`].

Doc text [`syntax.rs:403-408`]: "This statement exists to
preserve a trace of a scrutinee matched against a wildcard
binding. This is especially useful for `let _ = PLACE;` bindings
that desugar to a single `PlaceMention(PLACE)`. When executed at
runtime, this computes the given place, but then discards it
without doing a load. `let _ = *ptr;` is fine even if the
pointer is dangling."

**Discharge.** `RemovePlaceMention`
[`rustc_mir_transform/src/remove_place_mention.rs`]:

```rust
impl<'tcx> crate::MirPass<'tcx> for RemovePlaceMention {
    fn is_enabled(&self, sess: &rustc_session::Session) -> bool {
        !sess.opts.unstable_opts.mir_preserve_ub
    }

    fn run_pass(&self, _: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        for data in body.basic_blocks.as_mut_preserves_cfg() {
            data.retain_statements(|statement| match statement.kind {
                StatementKind::PlaceMention(..) | StatementKind::Nop => false,
                _ => true,
            })
        }
    }

    fn is_required(&self) -> bool { true }
}
```

`mir_preserve_ub` defaults to `false`
[`rustc_session/src/options.rs:2487-2489`]. INFERENCE:
`RemovePlaceMention::is_enabled` is `true` by default. The pass
runs in `run_runtime_cleanup_passes`
[`rustc_mir_transform/src/lib.rs:670`] which executes
unconditionally as part of `optimized_mir`'s pipeline (no
opt-level gate on the cleanup-pass dispatch -- only on the
optimization-pass dispatch in `run_optimization_passes`).

So `PlaceMention` is removed from `optimized_mir` outputs
*unless* the user passes `-Z mir-preserve-ub`. The Regseq
pipeline does not opt into this flag.

Codegen consumer [`rustc_codegen_ssa/src/mir/statement.rs:91-97`]:
no-op. Validator [`validate.rs:340-347, 1576-1582`]: allowed at
all phases (no rejection arm).

Maps to `D-8-1.1` (cardinality -- discharged by RemovePlaceMention)
+ `D-8-4` Part A.

### 2.7 `AscribeUserType(Box<(Place, UserTypeProjection)>, ty::Variance)`

[`syntax.rs:411-427`].

Doc text [`syntax.rs:411-426`]: "Encodes a user's type
ascription. These need to be preserved intact so that NLL can
respect them. [...] When executed at runtime this is a nop.
**Disallowed after drop elaboration.**"

**Discharge.** `CleanupPostBorrowck`
[`cleanup_post_borrowck.rs:33-43`] NOPs every `AscribeUserType`
(the same arm as `FakeRead`, see §2.2 above) and additionally
clears the user-type-annotation tables
[`cleanup_post_borrowck.rs:81-85`]:

```rust
body.user_type_annotations.raw.clear();

for decl in &mut body.local_decls {
    decl.user_ty = None;
}
```

Validator forbids `AscribeUserType` at `Runtime(Initial)` or
later [`validate.rs:301-307, 1492-1498`].

Maps to `D-8-1.1` + `D-8-4` Part A (rejection-table entry: same
as FakeRead -- discharged by CleanupPostBorrowck, validator-
forbidden at Runtime+).

### 2.8 `Coverage(CoverageKind)`

[`syntax.rs:429-446`].

`CoverageKind` is a 3-variant enum
[`rustc_middle/src/mir/coverage.rs:73-93`]:

```rust
pub enum CoverageKind {
    SpanMarker,
    BlockMarker { id: BlockMarkerId },
    VirtualCounter { bcb: BasicCoverageBlock },
}
```

**Discharge of two of the three.** `CleanupPostBorrowck`
[`cleanup_post_borrowck.rs:33-43`] NOPs `BlockMarker` and
`SpanMarker` -- the markers used during coverage instrumentation
that are not needed past borrowck. The third variant,
`VirtualCounter`, survives if and only if `-C instrument-coverage`
is enabled.

Validator confirms [`validate.rs:330-339`]:

```rust
StatementKind::Coverage(kind) => {
    if self.body.phase >= MirPhase::Analysis(AnalysisPhase::PostCleanup)
        && let CoverageKind::BlockMarker { .. } | CoverageKind::SpanMarker { .. } = kind
    {
        self.fail(location, format!("{kind:?} should have been removed after analysis"));
    }
}
```

So `BlockMarker` / `SpanMarker` are categorically forbidden in
`optimized_mir`; only `VirtualCounter` may appear, and only
when coverage is on.

**Pipeline state.** Driver builds for Linux/Zephyr do not enable
`-C instrument-coverage`. INFERENCE: in the Regseq pipeline,
`Coverage` is unreachable. The bridge can reject if seen.

Codegen consumer [`rustc_codegen_ssa/src/mir/statement.rs:67-69`]:
calls `self.codegen_coverage(bx, kind, source_info.scope)`
which for `VirtualCounter` emits an `llvm.instrprof.increment`
or a no-op depending on counter-creation outcome
[`rustc_middle/src/mir/coverage.rs:87-92`].

Cranelift consumer [`rustc_codegen_cranelift/src/base.rs:918`]:

```rust
StatementKind::Coverage { .. } => unreachable!(),
```

Cranelift hard-rejects coverage. INFERENCE: this confirms
coverage is not part of the standard codegen path -- the
LLVM consumer treats it specially, and cranelift doesn't
support it at all.

Maps to `D-8-1.1` + `D-8-4` Part A (rejection: Coverage
unreachable in driver builds; if encountered, treat as
unsupported).

### 2.9 `Intrinsic(Box<NonDivergingIntrinsic>)` and friends

[`syntax.rs:448-450`]. Doc: "Denotes a call to an intrinsic that
does not require an unwind path and always returns. This avoids
adding a new block and a terminator for simple intrinsics."

`NonDivergingIntrinsic<'tcx>` is a 2-variant enum
[`syntax.rs:485-508`]:

```rust
pub enum NonDivergingIntrinsic<'tcx> {
    Assume(Operand<'tcx>),
    CopyNonOverlapping(CopyNonOverlapping<'tcx>),
}
```

`CopyNonOverlapping<'tcx>` [`syntax.rs:631-638`]:

```rust
pub struct CopyNonOverlapping<'tcx> {
    pub src: Operand<'tcx>,
    pub dst: Operand<'tcx>,
    pub count: Operand<'tcx>,
}
```

**Emission.** `lower_intrinsics::LowerIntrinsics` (in
`run_runtime_cleanup_passes` [`lib.rs:669`]) rewrites direct
calls to `core::intrinsics::assume` and
`core::intrinsics::copy_nonoverlapping` from
`TerminatorKind::Call` into in-block `StatementKind::Intrinsic`
statements followed by `Goto`
[`rustc_mir_transform/src/lower_intrinsics.rs:55-89`]:

```rust
sym::copy_nonoverlapping => {
    block.statements.push(Statement::new(
        terminator.source_info,
        StatementKind::Intrinsic(Box::new(
            NonDivergingIntrinsic::CopyNonOverlapping(
                rustc_middle::mir::CopyNonOverlapping {
                    src: src.node, dst: dst.node, count: count.node,
                },
            ),
        )),
    ));
    terminator.kind = TerminatorKind::Goto { target };
}
sym::assume => { /* analogous */ }
```

`LowerIntrinsics::is_required(): true` (line 361). It runs
unconditionally in `run_runtime_cleanup_passes`. INFERENCE:
both `Assume` and `CopyNonOverlapping` reach `optimized_mir`
when the user calls these intrinsics.

`large_enums::EnumSizeOpt` also emits `CopyNonOverlapping`
[`rustc_mir_transform/src/large_enums.rs:129-130`] but its
`is_enabled` requires `mir_opt_level() >= 3` or
`-Z unsound-mir-opts` [`large_enums.rs:32-37`], so this
secondary emission point is not reached at the Regseq pipeline's
opt level.

**Validator.** Per-variant operand-type checks
[`validate.rs:1508-1549`]:

```rust
StatementKind::Intrinsic(box NonDivergingIntrinsic::Assume(op)) => {
    let ty = op.ty(&self.body.local_decls, self.tcx);
    if !ty.is_bool() {
        self.fail(location, format!("`assume` argument must be `bool`, but got: `{ty}`"));
    }
}
StatementKind::Intrinsic(box NonDivergingIntrinsic::CopyNonOverlapping(
    CopyNonOverlapping { src, dst, count },
)) => {
    let src_ty = src.ty(...);
    let op_src_ty = if let Some(src_deref) = src_ty.builtin_deref(true) { src_deref } else {
        self.fail(... "Expected src to be ptr in copy_nonoverlapping");
        return;
    };
    let dst_ty = dst.ty(...);
    let op_dst_ty = if let Some(dst_deref) = dst_ty.builtin_deref(true) { dst_deref } else {
        self.fail(... "Expected dst to be ptr in copy_nonoverlapping");
        return;
    };
    if !self.mir_assign_valid_types(op_src_ty, op_dst_ty) {
        self.fail(location, format!("bad arg ({op_src_ty} != {op_dst_ty})"));
    }

    let op_cnt_ty = count.ty(...);
    if op_cnt_ty != self.tcx.types.usize {
        self.fail(location, format!("bad arg ({op_cnt_ty} != usize)"));
    }
}
```

So:
- `Assume(op)`: `op.ty()` must be `bool`.
- `CopyNonOverlapping { src, dst, count }`: `src.ty()` and
  `dst.ty()` must both be pointer-or-reference-or-Box types
  (`builtin_deref(true)` is `Some`); their pointee types must
  satisfy `mir_assign_valid_types` (i.e., be the same up to
  lifetime erasure); `count.ty()` must be exactly `usize`.

Codegen consumer [`rustc_codegen_ssa/src/mir/statement.rs:70-90`]:

```rust
mir::StatementKind::Intrinsic(box NonDivergingIntrinsic::Assume(ref op)) => {
    let op_val = self.codegen_operand(bx, op);
    bx.assume(op_val.immediate());
}
mir::StatementKind::Intrinsic(box NonDivergingIntrinsic::CopyNonOverlapping(
    mir::CopyNonOverlapping { ref count, ref src, ref dst },
)) => {
    let dst_val = self.codegen_operand(bx, dst);
    let src_val = self.codegen_operand(bx, src);
    let count = self.codegen_operand(bx, count).immediate();
    let pointee_layout = dst_val.layout.pointee_info_at(bx, rustc_abi::Size::ZERO).expect("Expected pointer");
    let bytes = bx.mul(count, bx.const_usize(pointee_layout.size.bytes()));
    let align = pointee_layout.align;
    let dst = dst_val.immediate();
    let src = src_val.immediate();
    bx.memcpy(dst, align, src, align, bytes, crate::MemFlags::empty(), None);
}
```

Cranelift consumer
[`rustc_codegen_cranelift/src/base.rs:920-942`]: same
shape -- `Assume` is a hint that cranelift "ignores" (no codegen
for it), `CopyNonOverlapping` lowers to a `memcpy` call.

INFERENCE: For RegseqModel's C backend, `Assume` lowers to a
no-op (or, if compatible C compiler, `__builtin_assume`); and
`CopyNonOverlapping` lowers to `memcpy(dst, src, count *
sizeof(*src))`. The intrinsic statement vocabulary is small
and stable.

**Payload-shape question (D-8-1.4).** rustc nests:
`Intrinsic(Box<NonDivergingIntrinsic>)` ->
`NonDivergingIntrinsic::Assume(Operand)` /
`CopyNonOverlapping(CopyNonOverlapping { src, dst, count })`.
Stage 8 has two reasonable shapes:

1. **Mirror rustc:** preserve the `Intrinsic` arm with a nested
   `NonDivergingIntrinsic` enum. Faithful but adds an enum
   layer.
2. **Flatten:** lift `Assume` and `CopyNonOverlapping` to
   top-level `Statement` variants:
   ```
   Statement::Assume { cond: Operand }
   Statement::CopyNonOverlapping { src: Operand, dst: Operand, count: Operand }
   ```
   This shrinks the variant count but loses the "non-diverging
   intrinsic" grouping. Driver C code is a small enough
   universe that this grouping carries little information.
   rustc_public takes option (1) [`rustc_public/src/mir/body.rs:487`].

Maps to `D-8-1.1` (cardinality) + `D-8-1.4` (Intrinsic payload
flattening choice) + `D-8-4` Part B (Intrinsic per-variant
operand-type invariants).

### 2.10 `ConstEvalCounter`

[`syntax.rs:452-456`]. Doc: "Instructs the const eval interpreter
to increment a counter; this counter is used to track how many
steps the interpreter has taken. [...] Other than in the const
eval interpreter, this is a no-op."

**Emission.** `CtfeLimit::run_pass`
[`rustc_mir_transform/src/ctfe_limit.rs:14-43`] inserts
`ConstEvalCounter` into blocks with back edges or `Call`
terminators. Pipeline placement: `CtfeLimit` is applied in the
`mir_for_ctfe` query
[`rustc_mir_transform/src/lib.rs:519`]:

```rust
pm::run_passes(tcx, &mut body, &[&ctfe_limit::CtfeLimit], None, pm::Optimizations::Allowed);
```

But the CTFE branch is *only* for const-context bodies
[`rustc_mir_transform/src/lib.rs:494-515`]. INFERENCE: the
`optimized_mir` query never invokes `CtfeLimit`, so
`ConstEvalCounter` cannot appear in `optimized_mir` outputs
(it can appear in `mir_for_ctfe` outputs, which the Regseq
bridge does not consume per `../../spec.md` §15 bridge entry
contract).

Codegen consumer [`rustc_codegen_ssa/src/mir/statement.rs:91-97`]:
no-op. Validator [`validate.rs:340-347, 1576-1582`]: allowed
at all phases.

Maps to `D-8-1.1` + `D-8-4` Part A (rejection-table entry:
`ConstEvalCounter` -> "unreachable in optimized_mir; injected
only for CTFE bodies").

### 2.11 `Nop`

[`syntax.rs:458-459`]. Doc: "No-op. Useful for deleting
instructions without affecting statement indices."

**Emission.** Various passes via `Statement::make_nop`
[`rustc_middle/src/mir/statement.rs:24-43`]:

```rust
pub fn make_nop(&mut self, drop_debuginfo: bool) {
    if self.kind == StatementKind::Nop { return; }
    let replaced_stmt = std::mem::replace(&mut self.kind, StatementKind::Nop);
    if !drop_debuginfo {
        let Some(debuginfo) = replaced_stmt.as_debuginfo() else {
            bug!("debuginfo is not yet supported.")
        };
        self.debuginfos.push(debuginfo);
    }
}
```

Note the side-effect: when `drop_debuginfo: false`, `make_nop`
salvages the replaced statement's `as_debuginfo()` output
[`statement.rs:79-89`]:

```rust
pub fn as_debuginfo(&self) -> Option<StmtDebugInfo<'tcx>> {
    match self {
        StatementKind::Assign(box (place, Rvalue::Ref(_, _, ref_place)))
            if let Some(local) = place.as_local() =>
        {
            Some(StmtDebugInfo::AssignRef(local, *ref_place))
        }
        _ => None,
    }
}
```

Only `Assign(local, Ref(...))` salvages debuginfo -- all other
arms either get dropped or, if `drop_debuginfo: false` is passed,
trigger the `bug!`.

`CleanupPostBorrowck` always passes `drop_debuginfo: true`
[`cleanup_post_borrowck.rs:42`]: `statement.make_nop(true)`.
So the borrowck-discharge path drops the debuginfo unconditionally.

**Discharge.** `RemoveStorageMarkers` and `RemovePlaceMention`
both retain only non-`Nop` statements (see §2.4, §2.6 source
quotes). At default `mir_opt_level >= 1` and default
`!mir_preserve_ub`, both passes run, and the outputs of
`optimized_mir` are `Nop`-free.

Note: `RemoveStorageMarkers` only runs when
`mir_opt_level() > 0`. As discussed in §2.4, the Regseq pipeline's
"`mir_opt_level = 0`" claim is in tension with the README's
"`RemoveStorageMarkers` in whitelist" claim. **At
`mir_opt_level = 0` with no overrides, `Nop` survives** (only
`RemovePlaceMention` removes it, and that pass also retains
`Nop` in some races -- actually re-reading the source,
`RemovePlaceMention` ALSO removes `Nop`
[`remove_place_mention.rs:18`]). So even at
`mir_opt_level = 0`, `RemovePlaceMention` (which runs in
`run_runtime_cleanup_passes` unconditionally except for
`mir_preserve_ub`) sweeps `Nop`s.

INFERENCE: `Nop` is therefore unreachable in the Regseq pipeline's
`optimized_mir` outputs unless `-Z mir-preserve-ub` is set.

Codegen consumer [`rustc_codegen_ssa/src/mir/statement.rs:91-97`]:
no-op. Validator [`validate.rs:340-347, 1576-1582`]: allowed.

Maps to `D-8-1.1` + `D-8-4` Part A.

### 2.12 `BackwardIncompatibleDropHint { place: Box<Place>, reason: BackwardIncompatibleDropReason }`

[`syntax.rs:461-471`]. Doc: "Marker statement indicating where
`place` would be dropped. This is semantically equivalent to
`Nop`, so codegen and MIRI should interpret this statement as
such. The only use case of this statement is for linting in MIR
to detect temporary lifetime changes."

`BackwardIncompatibleDropReason`
[`syntax.rs:1000-1002`]: 1 variant -- `Edition2024`.

**Discharge.** `CleanupPostBorrowck`
[`cleanup_post_borrowck.rs:33-43`] NOPs every
`BackwardIncompatibleDropHint`:

```rust
| StatementKind::BackwardIncompatibleDropHint { .. } => {
    statement.make_nop(true)
}
```

So this statement is unreachable in `optimized_mir`. The
codegen consumer [`statement.rs:91-97`] treats it as a no-op
defence-in-depth. The validator [`validate.rs:340-347,
1576-1582`] allows it at all phases (no rejection arm) -- but
since `CleanupPostBorrowck::is_required(): true` runs in
`run_analysis_cleanup_passes` before any phase transition to
`Runtime`, it cannot survive.

Maps to `D-8-1.1` + `D-8-4` Part A (rejection-table entry:
discharged by CleanupPostBorrowck).

---

## 3. Cleanup-pass discharge mechanics + required/enabled taxonomy

The pinned RegseqModel cleanup-pass whitelist (per top-level
README) is:

```
- RemoveStorageMarkers
- RemoveZsts
- SimplifyLocals
- ReorderBasicBlocks
- ReorderLocals
- CheckAlignment
- CheckNull
- CheckEnums
```

None of these are *Statement*-shape-altering except
`RemoveStorageMarkers` (which removes `StorageLive` /
`StorageDead` / `Nop`). However, the *load-bearing* discharge
for stage 8 happens earlier in the pipeline -- in the
unconditionally-applied analysis-cleanup and runtime-cleanup
phases. This section documents the full set of statement-shape-
altering passes that run before `optimized_mir` returns.

### 3.1 `CleanupPostBorrowck` (analysis-cleanup, always runs)

- Source: `rustc_mir_transform/src/cleanup_post_borrowck.rs`.
- Pipeline placement:
  `run_analysis_cleanup_passes` [`lib.rs:618-634`], runs
  before any runtime-phase lowering, before any opt-level gate.
- `is_required(): true` [`cleanup_post_borrowck.rs:88-90`].
- `is_enabled`: not overridden -> default `true`
  [`pass_manager.rs:94-96`]. INFERENCE: this pass runs
  unconditionally on every `optimized_mir` invocation.
- Statement-shape effect: NOPs `AscribeUserType`, `FakeRead`,
  `Assign(_, Rvalue::Ref(_, BorrowKind::Fake(_), _))`,
  `Coverage(BlockMarker | SpanMarker)`,
  `BackwardIncompatibleDropHint`. Rewrites
  `CastKind::PointerCoercion(ArrayToPointer | MutToConstPointer)`
  to `CastKind::PtrToPtr` (stage-7 evidence -- carried over
  here for completeness). Clears `body.user_type_annotations`
  and `local_decls[*].user_ty`. Rewrites `FalseEdge` /
  `FalseUnwind` terminators to `Goto` (stage-9 concern).

### 3.2 `RemoveStorageMarkers` (optimization-pass, conditional)

- Source: `rustc_mir_transform/src/remove_storage_markers.rs`.
- Pipeline placement: in the optimization-pass list invoked by
  `run_optimization_passes` [`lib.rs:728`].
- `is_required(): true`, `is_enabled: mir_opt_level() > 0 &&
  !emit_lifetime_markers()`.
- Statement-shape effect: removes `StorageLive`, `StorageDead`,
  `Nop` from every basic block.
- **Critical constraint for the Regseq pipeline.** At the
  pipeline-pinned `mir_opt_level = 0`, this pass is NOT
  enabled by `is_enabled` (returns `false`). The pass-manager
  rule [`pass_manager.rs:213-239`] is:

  ```
  let suppressed = !pass.is_required() && matches!(optimizations, Optimizations::Suppressed);
  overridden.unwrap_or_else(|| !suppressed && pass.is_enabled(tcx.sess))
  ```

  `is_required(): true` only matters when `optimizations ==
  Suppressed` (e.g., `#[optimize(none)]`). Otherwise the pass
  runs IFF `is_enabled` returns true. So `mir_opt_level = 0`
  blocks `RemoveStorageMarkers`. The pipeline must clarify this
  contradiction (see §2.4 above).

  HYPOTHESIS: the pipeline's intended state is either (a)
  `mir_opt_level = 1` plus a curated whitelist that excludes
  most opt passes via `-Z mir-enable-passes=-PassName,...`, or
  (b) `mir_opt_level = 0` plus
  `-Z mir-enable-passes=+RemoveStorageMarkers` to force-
  enable the storage-marker stripping. Resolution deferred to
  spec close.

### 3.3 `RemovePlaceMention` (runtime-cleanup, near-unconditional)

- Source: `rustc_mir_transform/src/remove_place_mention.rs`.
- Pipeline placement: `run_runtime_cleanup_passes`
  [`lib.rs:670`]. Unconditional w.r.t. opt-level.
- `is_required(): true`, `is_enabled: !mir_preserve_ub`.
- `mir_preserve_ub` defaults to `false`
  [`rustc_session/src/options.rs:2487-2489`].
- Statement-shape effect: removes `PlaceMention` and `Nop`.
- INFERENCE: this pass runs by default in the Regseq pipeline,
  so `PlaceMention` and `Nop` are gone from `optimized_mir`
  (modulo the `mir_preserve_ub` opt-out).

### 3.4 `LowerIntrinsics` (runtime-cleanup, always runs)

- Source: `rustc_mir_transform/src/lower_intrinsics.rs`.
- Pipeline placement: `run_runtime_cleanup_passes`
  [`lib.rs:669`]. Unconditional.
- `is_required(): true`, default `is_enabled`.
- Statement-shape effect: emits `StatementKind::Intrinsic`
  statements (Assume, CopyNonOverlapping) by rewriting calls
  to corresponding intrinsics into in-block statements.
  This is the *source* of `Intrinsic` statements in
  `optimized_mir`.

### 3.5 `AddRetag` (runtime-lowering, flag-gated; default off)

- Source: `rustc_mir_transform/src/add_retag.rs`.
- Pipeline placement: `run_runtime_lowering_passes`
  [`lib.rs:657`].
- `is_required(): true`, `is_enabled:
  sess.opts.unstable_opts.mir_emit_retag` -- defaults to
  `false`.
- Statement-shape effect: emits `StatementKind::Retag` (only
  when flag is on).
- INFERENCE: in the Regseq pipeline (no `-Z mir-emit-retag`),
  no `Retag` statements are produced.

### 3.6 `coroutine::StateTransform` (runtime-lowering, always runs)

- Source: `rustc_mir_transform/src/coroutine.rs`.
- Pipeline placement: `run_runtime_lowering_passes`
  [`lib.rs:660`].
- `is_required(): true`
  [`coroutine.rs:1683-1685`].
- Statement-shape effect: rewrites coroutine bodies, lowering
  `Yield` / `CoroutineDrop` terminators (stage-9 concern) and
  generating `SetDiscriminant` statements on the synthesised
  state-machine ADT [`coroutine.rs:366`].
- Cross-stage anchor: stage 3 rejects coroutine types entirely,
  so the only `SetDiscriminant` statements reaching the bridge
  target user-written enum types -- not coroutine-state-machines.

### 3.7 Other runtime-lowering passes (statement-neutral)

The full `run_runtime_lowering_passes` list
[`lib.rs:637-664`]:

```
- add_call_guards::CriticalCallEdges
- post_analysis_normalize::PostAnalysisNormalize
- add_subtyping_projections::Subtyper
- elaborate_drops::ElaborateDrops
- Lint(check_call_recursion::CheckDropRecursion)
- abort_unwinding_calls::AbortUnwindingCalls
- add_moves_for_packed_drops::AddMovesForPackedDrops
- add_retag::AddRetag                               (gated)
- erase_deref_temps::EraseDerefTemps                (rvalue-shape effect)
- elaborate_box_derefs::ElaborateBoxDerefs          (rvalue-shape effect)
- coroutine::StateTransform                         (statement-shape effect)
- Lint(known_panics_lint::KnownPanicsLint)
```

INFERENCE: `ElaborateDrops` and `AddMovesForPackedDrops`
introduce `Drop` terminators (stage 9) and may inject
`Assign` / `SetDiscriminant` statements as part of drop
elaboration, but they do not introduce new statement
*kinds* -- only new instances of existing kinds.

### 3.8 The `optimized_mir` -> bridge contract

After `run_analysis_to_runtime_passes` (always)
[`lib.rs:583-613`] and `run_optimization_passes` (always at
default opt levels) [`lib.rs:689-789`], the body is at
`MirPhase::Runtime(RuntimePhase::Optimized)` and is what the
Regseq bridge consumes via `tcx.optimized_mir(def_id)`.

INFERENCE: in this body, the closed `StatementKind` surface is:

| Variant | Reachable? | Source |
|-|-|-|
| `Assign` | YES | universal |
| `FakeRead` | NO | NOPed by CleanupPostBorrowck |
| `SetDiscriminant` | YES (post-Runtime-Initial only) | drop elaboration / aggregate lowering |
| `StorageLive` | CONDITIONAL | depends on `mir_opt_level` + `emit_lifetime_markers` |
| `StorageDead` | CONDITIONAL | same |
| `Retag` | NO (default flags) | only with `-Z mir-emit-retag` |
| `PlaceMention` | NO (default flags) | removed by RemovePlaceMention |
| `AscribeUserType` | NO | NOPed by CleanupPostBorrowck |
| `Coverage` | NO (no `-C instrument-coverage`) | gated; markers NOPed regardless |
| `Intrinsic` | YES | emitted by LowerIntrinsics |
| `ConstEvalCounter` | NO | only injected for CTFE bodies |
| `Nop` | NO (default flags) | removed by RemoveStorageMarkers / RemovePlaceMention |
| `BackwardIncompatibleDropHint` | NO | NOPed by CleanupPostBorrowck |

So the closed-surface candidate is **3 variants** (Assign,
SetDiscriminant, Intrinsic), with `StorageLive` /
`StorageDead` as a conditional fourth depending on the
pipeline's exact `mir_opt_level` setting.

Maps to `D-8-1.1` (variant cardinality) + `D-8-4` Part A
(rejection table).

---

## 4. `Statement<'tcx>` wrapper struct

`Statement<'tcx>` [`rustc_middle/src/mir/statement.rs:14-43`]:

```rust
#[derive(Clone, TyEncodable, TyDecodable, HashStable, TypeFoldable, TypeVisitable)]
#[non_exhaustive]
pub struct Statement<'tcx> {
    pub source_info: SourceInfo,
    pub kind: StatementKind<'tcx>,
    /// Some debuginfos appearing before the primary statement.
    pub debuginfos: StmtDebugInfos<'tcx>,
}
```

Three fields:

### 4.1 `source_info: SourceInfo`

`SourceInfo` [`rustc_middle/src/mir/mod.rs:822-830`]:

```rust
#[derive(Copy, Clone, Debug, Eq, PartialEq, TyEncodable, TyDecodable, Hash, HashStable)]
pub struct SourceInfo {
    pub span: Span,
    pub scope: SourceScope,
}
```

`Span` (16 bytes typical, depends on interning) + `SourceScope`
(an `IndexVec` newtype, 32 bits). Total ~16 bytes (with
alignment). The `scope` carries lexical-scope information used
by debuginfo emission and lint level resolution.

**Stage 8 design choice (D-8-3).** Two reasonable shapes:

1. **Mirror rustc:** carry a `SourceInfo` struct on each
   `Statement` so `(span, scope)` are co-located.
2. **Flatten:** drop `scope` (RegseqModel does not emit Rust
   debuginfo to LLVM/DWARF; the C output's debug info comes
   from `#line` directives or backend-inserted debug
   annotations) and carry only `span: Span`. rustc_public takes
   this approach [`rustc_public/src/mir/body.rs:471-474`]:

   ```rust
   pub struct Statement {
       pub kind: StatementKind,
       pub span: Span,
   }
   ```

INFERENCE: RegseqModel C-backend `#line` directives need only
the file/line info from `Span`; `SourceScope` is dead weight in
this consumer. Flat `span: Span` is the natural choice unless a
later stage (e.g., a Regseq linter that wants to consult lint
levels) demands the scope.

### 4.2 `kind: StatementKind<'tcx>`

The 13-variant enum analysed in §1-2. Stage-8-locked variant set
goes here. 16-byte size pin governs payload-shape choices.

### 4.3 `debuginfos: StmtDebugInfos<'tcx>`

`StmtDebugInfos<'tcx>` [`statement.rs:1014-1058`] is a newtype
wrapper around `Vec<StmtDebugInfo<'tcx>>`. `StmtDebugInfo`
[`statement.rs:1076-1080`]:

```rust
pub enum StmtDebugInfo<'tcx> {
    AssignRef(Local, Place<'tcx>),
    InvalidAssign(Local),
}
```

Origin: `Statement::make_nop` [`statement.rs:24-43`] preserves
debuginfo from the *replaced* statement when `drop_debuginfo:
false`, calling `as_debuginfo()` which only returns `Some` for
`Assign(local, Ref(...))`. So `debuginfos` is a salvaged-Ref
trail that survives across NOP rewrites.

Codegen consumer
[`rustc_codegen_ssa/src/mir/statement.rs:11`]:

```rust
self.codegen_stmt_debuginfos(bx, &statement.debuginfos);
```

It runs `codegen_stmt_debuginfo` for each entry, which calls
`debug_new_val_to_local` or `debug_poison_to_local` on the
LLVM/cranelift debuginfo channel
[`rustc_codegen_ssa/src/mir/statement.rs:101-144`].

INFERENCE: `debuginfos` is a Rust-debuginfo carrier. The C
backend emits debug info via `#line` directives and (for some
toolchains) DWARF location lists generated by the C compiler;
it does not consume rustc's `StmtDebugInfo`. Stage 8 can drop
this field.

**Stage 8 design choice (D-8-3 continued).** Drop
`debuginfos`. rustc_public agrees: its `Statement` has no
`debuginfos` field [`rustc_public/src/mir/body.rs:471-474`].

Maps to `D-8-3` (Statement wrapper: `source_info` flatten to
`span`, drop `debuginfos`).

---

## 5. Storage of statements in basic blocks (D-8-2)

`BasicBlockData<'tcx>` [`rustc_middle/src/mir/mod.rs:1308-1310`]:

```rust
pub struct BasicBlockData<'tcx> {
    pub statements: Vec<Statement<'tcx>>,
    ...
}
```

A flat `Vec<Statement>` -- no interning, no compaction. This
parallels stage-5 (`Place::projection: &'tcx List<PlaceElem>`)
where the *projection* is interned but the place vector is not,
and stage-7 where `Rvalue` is inline in the `Assign` payload
without interning.

**Stage 8 storage decision (D-8-2).** Stage 10 (`BasicBlock`)
will pick between `Vec<Statement>` and `Box<[Statement]>` for
the body's storage. Stage 8 commits to "no `Statement`-level
interning" -- statements are not deduplicated, not arena-
allocated by ID, not reference-counted. This is compositional
with `D-5-2` (no Place interning -- Place is value-typed),
`D-6-2` (no Operand interning -- Operand is value-typed), and
`D-7-2` (no Rvalue interning -- Rvalue is value-typed).

INFERENCE: rustc's choice to inline `Vec<Statement>` rather
than intern `Statement` reflects (a) statements are highly
unique per-site (so interning rarely deduplicates) and (b)
linear traversal during codegen / dataflow is the dominant
access pattern, which `Vec` serves directly.

Maps to `D-8-2` (no Statement interning).

---

## 6. `rustc_public` Statement (peer evidence, drift)

`rustc_public/src/mir/body.rs:470-490` defines an owned analog:

```rust
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

pub enum StatementKind {
    Assign(Place, Rvalue),
    FakeRead(FakeReadCause, Place),
    SetDiscriminant { place: Place, variant_index: VariantIdx },
    StorageLive(Local),
    StorageDead(Local),
    Retag(RetagKind, Place),
    PlaceMention(Place),
    AscribeUserType { place: Place, projections: UserTypeProjection, variance: Variance },
    Coverage(Coverage),
    Intrinsic(NonDivergingIntrinsic),
    ConstEvalCounter,
    Nop,
}
```

Notable drift from rustc_middle (this nightly):

- **12 variants vs 13.** `BackwardIncompatibleDropHint` is
  absent from rustc_public. INFERENCE: rustc_public has not
  been updated to track the rustc_middle addition (this
  variant entered nightly with the Edition 2024 lints).
- **`Statement { kind, span }`, not `{ source_info, kind,
  debuginfos }`.** Confirms the §4 simplification path: the
  owned-IR design space tolerates dropping `SourceScope` and
  `debuginfos`.
- **`Assign(Place, Rvalue)` flat, not `Box<(Place, Rvalue)>`.**
  rustc_public is not bound by the 16-byte
  `static_assert_size!(StatementKind<'_>, 16)` budget.
- **`SetDiscriminant { place: Place, variant_index }` flat,
  not `Box<Place>`.** Same.
- **`Retag(RetagKind, Place)` flat, not `Box<Place>`.** Same.
- **`PlaceMention(Place)` flat, not `Box<Place>`.** Same.
- **`AscribeUserType { place, projections, variance }`** -- a
  named-field struct rather than rustc_middle's
  `(Box<(Place, UserTypeProjection)>, Variance)` tuple. Same
  three pieces of information, different surface.
- **`FakeRead(FakeReadCause, Place)` flat, not
  `Box<(FakeReadCause, Place)>`.** Same.
- **`Intrinsic(NonDivergingIntrinsic)` flat, not
  `Box<NonDivergingIntrinsic>`.** Same.

INFERENCE: this drift confirms the same lesson stage 7 §12
extracted -- the owned-IR design space tolerates flatter
shapes than `mir::StatementKind<'tcx>` because it is not
under rustc_middle's `static_assert_size!` discipline. Stage 8
is free to choose flat vs boxed payloads based on RegseqModel's
own size profile.

But: rustc_public is NOT a current-truth peer for the bridge.
The bridge consumes `mir::StatementKind<'tcx>` from rustc_middle
(per `../../spec.md` §15 bridge translation rules) -- not
rustc_public's narrowed surface. Citations from rustc_middle
take precedence in stage 8's spec.

Maps to `D-8-1.2` / `D-8-1.3` / `D-8-1.4` (payload-shape choice
flat vs Box) + `D-8-3` (Statement wrapper simplification).

---

## 7. Codegen consumer pattern

Two codegen backends in-tree consume `StatementKind`:

### 7.1 `rustc_codegen_ssa` (LLVM, GCC, etc.)

`codegen_statement` [`rustc_codegen_ssa/src/mir/statement.rs:8-99`]:

```rust
pub(crate) fn codegen_statement(&mut self, bx: &mut Bx, statement: &mir::Statement<'tcx>) {
    self.codegen_stmt_debuginfos(bx, &statement.debuginfos);
    self.set_debug_loc(bx, statement.source_info);
    match statement.kind {
        mir::StatementKind::Assign(box (ref place, ref rvalue)) => { ... }
        mir::StatementKind::SetDiscriminant { box ref place, variant_index } => { ... }
        mir::StatementKind::StorageLive(local) => { ... }
        mir::StatementKind::StorageDead(local) => { ... }
        mir::StatementKind::Coverage(ref kind) => {
            self.codegen_coverage(bx, kind, statement.source_info.scope);
        }
        mir::StatementKind::Intrinsic(box NonDivergingIntrinsic::Assume(ref op)) => { ... }
        mir::StatementKind::Intrinsic(box NonDivergingIntrinsic::CopyNonOverlapping(...)) => { ... }
        mir::StatementKind::FakeRead(..)
        | mir::StatementKind::Retag { .. }
        | mir::StatementKind::AscribeUserType(..)
        | mir::StatementKind::ConstEvalCounter
        | mir::StatementKind::PlaceMention(..)
        | mir::StatementKind::BackwardIncompatibleDropHint { .. }
        | mir::StatementKind::Nop => {}
    }
}
```

Six "active" arms (Assign, SetDiscriminant, StorageLive,
StorageDead, Coverage, Intrinsic) and seven "no-op" arms
(FakeRead, Retag, AscribeUserType, ConstEvalCounter,
PlaceMention, BackwardIncompatibleDropHint, Nop).

The seven no-op arms are defence-in-depth: by the time the body
reaches codegen, all seven have been NOPed or removed by the
pipeline (per §3 above), but the codegen consumer tolerates
their presence rather than ICE-ing.

### 7.2 `rustc_codegen_cranelift` (cranelift)

`codegen_stmt` [`rustc_codegen_cranelift/src/base.rs:597-944`]:

```rust
match &stmt.kind {
    StatementKind::SetDiscriminant { place, variant_index } => { ... }
    StatementKind::Assign(to_place_and_rval) => { ... }   // big match on rvalue
    StatementKind::StorageLive(_)
    | StatementKind::StorageDead(_)
    | StatementKind::ConstEvalCounter
    | StatementKind::Nop
    | StatementKind::FakeRead(..)
    | StatementKind::Retag { .. }
    | StatementKind::PlaceMention(..)
    | StatementKind::BackwardIncompatibleDropHint { .. }
    | StatementKind::AscribeUserType(..) => {}

    StatementKind::Coverage { .. } => unreachable!(),
    StatementKind::Intrinsic(intrinsic) => match &**intrinsic {
        NonDivergingIntrinsic::Assume(_) => {}    // ignored, optimization-only
        NonDivergingIntrinsic::CopyNonOverlapping(...) => { ... }   // memcpy
    },
}
```

Differences from ssa:
- Cranelift hard-rejects `Coverage` with `unreachable!()`
  rather than calling a `codegen_coverage` shim.
- Cranelift treats `Assume` as a dropped hint (not even an
  LLVM-style `bx.assume`).
- Cranelift treats `StorageLive`/`StorageDead` as no-ops with
  no LLVM lifetime emission.

INFERENCE: For the C backend stage 8 informs:
- `Assign` and `SetDiscriminant` are the two non-trivial active
  arms.
- `Intrinsic::Assume` is a no-op (or `__builtin_assume(cond)`
  if the C compiler supports it).
- `Intrinsic::CopyNonOverlapping` is a `memcpy(dst, src, count
  * sizeof(*dst))`.
- The other 7 arms are bridge-rejection candidates (per §3
  pipeline analysis -- they are unreachable in a well-formed
  pinned-pipeline `optimized_mir`).

Maps to `D-8-1.1` (cardinality) + `D-8-K` (`render_statement`
deferral -- the C-backend lowering of these 3 active arms is a
stage-11 / stage-14 concern, not stage 8's).

---

## 8. Statement-site invariants (D-8-4 Part B)

The MIR validator [`rustc_mir_transform/src/validate.rs`] runs
two passes: a bare `MirVisitor` pass (lines 299-348) and a typed
pass (lines 1465-1583). The typed pass enforces:

- **`Assign`:** `mir_assign_valid_types(rvalue.ty(),
  place.ty())` must hold. Place and rvalue are
  structurally type-equal modulo lifetime erasure
  [`validate.rs:1467-1482`]. DerefTemp special case: assignment
  to a `LocalInfo::DerefTemp` local *must* use
  `Rvalue::CopyForDeref` -- but stage 7 already established
  `CopyForDeref` is gone post-`EraseDerefTemps`, so this case
  is unreachable in `optimized_mir`.

- **`SetDiscriminant`:** `place.ty()` must be `ty::Adt`,
  `ty::Coroutine`, or `ty::Alias(Opaque, ..)`
  [`validate.rs:1551-1567`]. Coroutines are stage-3-rejected;
  opaque aliases should be normalized away by
  `PostAnalysisNormalize` (HYPOTHESIS). The variant_index must
  index into the place's enum's variant list (not enforced by
  validator -- a malformed variant_index is UB at codegen).

- **`Intrinsic::Assume(op)`:** `op.ty()` must be `bool`
  [`validate.rs:1508-1515`].

- **`Intrinsic::CopyNonOverlapping { src, dst, count }`:**
  `src.ty()` and `dst.ty()` must satisfy `builtin_deref(true)`
  (be ptr/ref/Box); pointee types must satisfy
  `mir_assign_valid_types`; `count.ty()` must be exactly
  `usize` [`validate.rs:1517-1549`].

- **`Retag(kind, _)`:** `kind` must NOT be `RetagKind::TwoPhase`
  [`validate.rs:1568-1574`].

These five clusters of invariants form the positive part of
`D-8-4`. The negative part (rejection rules) is captured by §3.8's
table.

Visitor template informs the invariant-walker shape (relevant to
the deferred `D-8-4` walk-impl). `super_statement` recurses into:
- `visit_source_info(source_info)`
- `visit_statement_debuginfo(...)` for each entry in `debuginfos`
- per-kind: `visit_assign`, `visit_place`, `visit_local`,
  `visit_retag`, `visit_ascribe_user_ty`, `visit_coverage`,
  `visit_operand`, etc.
[`rustc_middle/src/mir/visit.rs:425-509`].

INFERENCE: A RegseqModel statement-walker can mirror this
shape, but with the closed 3-variant kind, the walker collapses
to (a) `visit_assign(place, rvalue)`; (b)
`visit_set_discriminant(place, variant_index)`; (c) the two
intrinsic sub-walks (`assume(operand)`,
`copy_nonoverlapping(src, dst, count)`).

Maps to `D-8-4` (positive invariants).

---

## 9. Size-budget evidence (D-8-1 payload-shape arithmetic)

Restated from §1: rustc pins
[`rustc_middle/src/mir/syntax.rs:1741-1755`]:

```rust
#[cfg(target_pointer_width = "64")]
mod size_asserts {
    use rustc_data_structures::static_assert_size;

    use super::*;
    // tidy-alphabetical-start
    static_assert_size!(AggregateKind<'_>, 32);
    static_assert_size!(Operand<'_>, 24);
    static_assert_size!(Place<'_>, 16);
    static_assert_size!(PlaceElem<'_>, 24);
    static_assert_size!(Rvalue<'_>, 40);
    static_assert_size!(StatementKind<'_>, 16);
    static_assert_size!(TerminatorKind<'_>, 80);
    // tidy-alphabetical-end
}
```

Stage 8 implications:

- `StatementKind` is pinned at 16 bytes (1 tag word + 1 pointer
  payload on 64-bit). All payloads are boxed when they exceed
  pointer-width.
- `(Place, Rvalue) = 56 bytes` -> Box for `Assign`.
- `(FakeReadCause, Place) = ~24 bytes` -> Box for `FakeRead`
  (drives the box even though the tuple is only ~24 bytes;
  size-budget is strict).
- `(Place, UserTypeProjection)` -> Box for `AscribeUserType`.
- `Place = 16 bytes` -> Box for `SetDiscriminant`, `Retag`,
  `PlaceMention`, `BackwardIncompatibleDropHint` (all
  Place-bearing kinds box the Place to keep the kind at 16
  bytes since Place + variant tag would exceed).
- `NonDivergingIntrinsic` is also boxed (its largest variant
  `CopyNonOverlapping` is `3 * 24 = 72` bytes).
- `Local` (4 bytes), `RetagKind` (1 byte), `CoverageKind` (16
  bytes -- 4-byte tag + 12-byte payload for `BlockMarker`/etc.,
  fits without Box) all live inline.

INFERENCE: RegseqModel's `Statement` does not need to mirror
this size discipline. The Regseq compiler is a one-pass
build-time tool; runtime memory pressure is a non-issue
relative to driver-corpus sizes. Stage 8 can flatten `Assign`
to `(Place, Rvalue)` directly (parallel to rustc_public's
choice) without size penalty -- but it should re-evaluate this
once RegseqModel's `Place`, `Rvalue`, `Operand` final sizes are
known (post stages 5-7-impl).

Maps to `D-8-1.2` / `D-8-1.3` / `D-8-1.4` (Box vs flat payload
storage justification).

---

## 10. Rejection-point evidence summary

Three layers of rejection / discharge fire on stage-8 statement
kinds:

1. **Pipeline pass discharge.** Statement is NOPed or removed by
   a pre-`optimized_mir` pass. (§3.1, §3.2, §3.3.)
2. **Validator phase rejection.** The validator
   [`rustc_mir_transform/src/validate.rs`] `fail()`s if the
   statement appears at the wrong `MirPhase`. (§2.2, §2.3, §2.7,
   §2.8.)
3. **Codegen `bug!()` rejection.** The codegen consumer ICE-s.
   In stage 8, this only applies to cranelift's
   `Coverage { .. } => unreachable!()`
   [`rustc_codegen_cranelift/src/base.rs:918`]. The ssa
   consumer does not `bug!()` on any statement kind -- it
   silently no-ops the seven defence-in-depth arms.

Per-variant rejection table (assembled from §3.8):

| Variant | Layer | Mechanism |
|-|-|-|
| `Assign` | None | Reaches codegen always |
| `FakeRead` | (1)+(2) | CleanupPostBorrowck NOPs; validator forbids at Runtime+ |
| `SetDiscriminant` | None | Reaches codegen post-Runtime-Initial |
| `StorageLive` | (1) conditional | RemoveStorageMarkers if mir_opt_level>0 && !emit_lifetime_markers |
| `StorageDead` | (1) conditional | same |
| `Retag` | (1) gated | AddRetag never runs without `-Z mir-emit-retag` |
| `PlaceMention` | (1) | RemovePlaceMention runs by default |
| `AscribeUserType` | (1)+(2) | CleanupPostBorrowck NOPs; validator forbids at Runtime+ |
| `Coverage` | (1) partial + (2) | CleanupPostBorrowck NOPs BlockMarker/SpanMarker; validator forbids those at PostCleanup+ |
| `Intrinsic` | None | Reaches codegen always |
| `ConstEvalCounter` | pipeline branch | Only injected for CTFE, not optimized_mir |
| `Nop` | (1) | RemoveStorageMarkers + RemovePlaceMention sweep |
| `BackwardIncompatibleDropHint` | (1) | CleanupPostBorrowck NOPs |

Maps to `D-8-4` Part A (rejection table).

---

## 11. `render_statement` deferral (D-8-K)

Stage 5 (`Place`), stage 6 (`Operand`), and stage 7 (`Rvalue`)
all locked their `render_*` C-backend lowering as deferred
to stage 11 (`pretty.rs::render_*`). Stage 8 follows the same
discipline: the C-form rendering of `Statement` (the textual
shape `(Place) = (Rvalue);`, `(Place).discr = N;`,
`memcpy(dst, src, count * sizeof(*dst));`,
`__builtin_assume(cond);`, etc.) is a stage-11 concern, not
stage 8's.

Stage 8 fixes:
- The closed `Statement` variant set (D-8-1).
- The per-variant payload shapes (D-8-1.x).
- The wrapper struct (D-8-3).
- The site-invariants and rejection rules (D-8-4).

Stage 11 will compose `render_place` + `render_rvalue` +
`render_operand` to produce `render_statement`. Stage 8's spec
must NOT pre-decide that composition.

Maps to `D-8-K`.

---

## 12. Summary mapping to `D-8-K` candidate decisions

| Tag | Decision | Evidence anchor |
|-|-|-|
| `D-8-1.1` | Statement variant cardinality (3 active: Assign, SetDiscriminant, Intrinsic; conditional StorageLive/Dead; rest rejected) | §1, §2, §3.8 |
| `D-8-1.2` | Assign payload (`(Place, Rvalue)` flat or boxed) | §2.1, §6, §9 |
| `D-8-1.3` | SetDiscriminant payload (`(Place, VariantIdx)` struct or tuple; flat or boxed) | §2.3, §6 |
| `D-8-1.4` | Intrinsic payload (nested NonDivergingIntrinsic vs flattened top-level Assume / CopyNonOverlapping) | §2.9, §6 |
| `D-8-2` | No Statement interning; inline in BB storage | §5 |
| `D-8-3` | Statement wrapper (drop SourceScope -> bare Span; drop debuginfos) | §4 |
| `D-8-4` Part A | Rejection table (12 of 13 variants discharged or unreachable) | §2 (each subsection) + §3 + §10 |
| `D-8-4` Part B | Positive invariants (Assign type-equality, SetDiscriminant target-Adt, Intrinsic operand types) | §8 |
| `D-8-K` | render_statement defer to stage 11 | §11 |

---

## 13. Open D-8-K design questions where rustc's choices are one option among peers

The following stage 8 sub-decisions are NOT pinned by rustc
primary source -- they are design-space points where Charon /
mrustc / rustc_public may differ. These will be cross-checked in
the sibling prior-art files (`charon.md`, `mrustc.md`,
`rustc_public.md` -- the latter only as drift evidence per §6).

1. **D-8-1.2 (Assign payload Box vs flat).** rustc_middle:
   boxed (size-budget). rustc_public: flat. Stage 8 inherits
   rustc_middle's `static_assert_size!` only if RegseqModel
   adopts a comparable inline-storage budget; otherwise flat is
   the natural choice.

2. **D-8-1.3 (SetDiscriminant payload).** rustc_middle: struct
   `{ place: Box<Place>, variant_index: VariantIdx }`.
   rustc_public: struct `{ place: Place, variant_index:
   VariantIdx }`. The named-struct shape is preferable to
   `(Place, VariantIdx)` tuple for clarity; the box is a
   size-budget artefact.

3. **D-8-1.4 (Intrinsic payload).** rustc_middle:
   `Intrinsic(Box<NonDivergingIntrinsic>)` with nested 2-variant
   enum. rustc_public: same nesting, unboxed. A flatter design
   lifts `Assume` and `CopyNonOverlapping` to top-level
   `Statement` variants:
   ```
   Statement::Assume(Operand)
   Statement::CopyNonOverlapping { src: Operand, dst: Operand, count: Operand }
   ```
   Charon and mrustc may take this flatter path -- to be
   verified by the sibling prior-art files.

4. **D-8-3 (Statement wrapper -- drop SourceScope?).** Driver
   C-output `#line` directives need only `Span`. rustc_public
   drops `scope` and `debuginfos`. This is the natural choice
   unless a downstream Regseq lint pass demands lint-level
   resolution.

5. **D-8-1.1 conditional fourth variant (StorageLive/Dead).**
   The pipeline contradiction noted in §2.4 / §3.2 must be
   resolved before locking. Three sub-options:
   - Reject on sight (assume `RemoveStorageMarkers` always runs).
   - Drop silently (lower to no-op).
   - Preserve as RegseqModel statements for round-trip fidelity.

6. **D-8-4 Part A: should the bridge reject or silently drop the
   12 unreachable variants?** Defence-in-depth (reject with
   diagnostic) catches future rustc regressions but adds
   compiler surface area. Silent drop matches the codegen
   consumer's behaviour (which silently no-ops the seven
   "should-not-reach" arms) but loses early-warning signal.

These are the points where `charon.md` / `mrustc.md` will
provide alternative evidence; resolution lands in
`spec-snapshot.md` after cross-source synthesis.

---

This rustc.md is the kickoff evidence pass. Mid-stage deeper
reads on specific D-tags will land additional citations
(e.g., for `D-8-1.1` storage-marker contradiction resolution
under the `mir_opt_level=0` pipeline contract; for `D-8-4` Part B
Place / Rvalue / Operand type-equality propagation through
`mir_assign_valid_types` which has its own substructure not
fully exhausted here).
