# Stage 4 -- review log

Iterative Codex adversarial-review rounds for the stage-4 const-
vocabulary spec at `spec-snapshot.md` (primary, §9.7) plus the
audit-shape decision record at `discussions.md` (D-4-1 entries).
Each entry records the round's scope, verdict, issue list, and
the corrections applied before the next round.

Reviews are invoked via `scripts/python/codex_runner.py` per the
project's CLAUDE.md contract. Each round uses a fresh prompt
(Codex has no session continuation); re-reviews include a brief
summary of what was changed since the prior round.

The structured output contract for every round:
- First line: `VERDICT: <BLOCK | MINOR | APPROVE>`
- Numbered issues list, each tagged `[BLOCK | MINOR | NIT]` with
  a decision reference, file:line citation, 1-3-sentence
  description, and a 1-sentence suggested fix.
- Last line: `TOTAL: <N> issues (<count of BLOCK> BLOCK /
  <count of MINOR> MINOR / <count of NIT> NIT)`.

Prompt template skeleton: `memory.md` §4.

---

## Round 1 -- D-4-1 initial review

**Date.** 2026-05-02.

**Scope.** D-4-1 cluster (parent decision + sub-decisions D-4-1.1
through D-4-1.9) covering the 11-variant `Const` enum, the `Byte`
three-state, the `InstanceRef` 4-tuple, the type-level vs value-
level partition, the bridge construction dispatch, and the
const-validation phase split. Spec under review:
`stage-4/spec-snapshot.md` §9.7. Audit shape under review:
`stage-4/discussions.md` D-4-1 entries.

Out of scope: the per-variant `ConstIdentityKey` byte-level
serialisation order (D-4-2 territory); `pretty.rs::render_const`
renderer style (post-D-4-1); stage-1/3 sections of
`spec-snapshot.md` outside §9.7; the existing top-level
`spec.md` §9.7 stub (which stage 4 supersedes via the merger).

**Codex run.** `--effort high`, `--timeout 1500`, output to
`/tmp/codex_d4_1_review_r1.txt`.

**Verdict.** `BLOCK` -- 7 BLOCK + 3 MINOR + 0 NIT (10 total
issues).

**Issues.**

1. *[BLOCK] D-4-1.5 / §9.7 (`spec-snapshot.md`).* `GlobalAlloc::Function`
   projection dropped the rustc-shim-flavor disambiguator
   (`shim_disambig: None`); §9.3's `RegseqInstanceKey` requires
   `fn_shim_disambig` to disambiguate shims sharing
   `(DefId, args)` (e.g., `Item(foo)` vs `ReifyShim(foo)`).
   D-4-1.5's claim that `kind_rank` separates `Function` from
   `DropGlue` was wrong — `DropGlue` is a
   `FunctionInstanceFlavor`, not a separate Regseq InstanceKind.
2. *[BLOCK] D-4-1.5 / §9.7.* `GlobalAlloc::VTable` projection
   used `kind_rank: VTableInstance-rank`, but §9.3's pinned
   table has only six ranks (Function/Static/Const/Struct/Enum/
   Union); vtable instances are `Static` Items synthesised per
   §13.2.
3. *[BLOCK] §9.7 bridge dispatch.* The slice and anonymous-
   memory paths synthesised data Static Items, but §13.3's
   `SyntheticItemKind` universe lists six families and none for
   memory/string backings.
4. *[BLOCK] §9.7 Byte invariants.* "For every byte at offset
   `o` with `Byte::Reloc(r)`: bytes at `[o..o+ptr_size]` ..."
   reads literally as requiring the second byte of any pointer
   slot to start its own aligned run.
5. *[BLOCK] §9.7 validation split.* Wording "aggregate children
   are not type-level" contradicts the partition table that
   admits scalars at value-level; the slice dispatch produces
   `Aggregate([Reloc(...), Uint(Usize, ...)])` with a type-
   level scalar child.
6. *[BLOCK] §9.7 Slice / NullPtr dispatch.* `NullPtr(TyId)`
   description claimed to cover the empty-string fallback `&str`
   data pointer, but the Slice dispatch always emits an
   `Aggregate`, never a `NullPtr` at the data field.
7. *[BLOCK] §9.7 validation order.* §9.7 appended a new value-
   level const validation step after Phase B step 8, but §3.3
   Phase B / Phase C numbering and the dependency-arrow summary
   were not updated.
8. *[MINOR] D-4-1.9 (`discussions.md`).* Bridge dispatch's
   `ConstValue::Scalar(Scalar::Int(_))` arm listed `ZST` as a
   possible target; the spec dispatch table correctly gives
   `ZeroSized → ZST` as a separate arm.
9. *[MINOR] D-4-1.3 (`discussions.md`).* Wording said the
   aggregate "key" is `Vec<ConstId>`, conflating storage shape
   with cross-build canonical key (which recursively uses each
   child's canonical content per §12.3).
10. *[MINOR] D-4-1.5 / §9.7.* `shim_disambig: Option<ShimDisambig>`
    pinned partly as a v2 "structural hedge" even though §13.4
    already encodes vtable method/drop distinctions in
    synthesised `DefPathHash` — violates README §5.9 "no
    forward-looking placeholders."

Full Codex output: `/tmp/codex_d4_1_review_r1.txt`.

**Corrections applied to `spec-snapshot.md` / `discussions.md`.**

- *Issues 1, 2, 10.* Replaced `Option<ShimDisambig>` enum with
  `fn_shim_disambig: u8` (matching §9.3's existing field). The
  4-tuple is now `(regseq_kind_rank, def_path_hash, args_key,
  fn_shim_disambig)` — same shape as `RegseqInstanceKey` minus
  the local `ItemId`. `Function` projection populates
  `fn_shim_disambig` from `rustc_instance_kind_sort_rank(flavor)`;
  `VTable` projection uses `regseq_kind_rank: 1` (Static),
  marked in `item_synthetic_kinds` as `VTableInstance` per §13.2.
  `discussions.md` D-4-1.5 prose updated to match: dropped the
  v2-hedge rationale; added "why no separate ShimDisambig enum"
  explicitly grounded in §13.4 disambiguation.
- *Issue 3.* Dropped the slice / anonymous-memory dispatch
  arms from §9.7. Added "Stage-4 deferred: reference-typed
  const handling" subsection; filed deferral in
  `stage-4/status.md` "Stage-4-specific deferred items"; filed
  outbound forward expectation in
  `stage-12/expectations.md` for the
  `SyntheticItemKind::ConstMemoryStatic` family extension.
- *Issue 4.* Rephrased Byte well-formedness invariant in terms
  of *maximal relocation runs* (length-`ptr_size` runs of
  same-`InstanceRef` bytes starting on `ptr_align`-aligned
  offsets), replacing the per-byte rule.
- *Issue 5.* Refactored value-level validation prose: type-
  level scalar children ARE admissible inside value-level
  aggregates (the type-level / value-level boundary is variant-
  intrinsic and non-transitive — a value-level aggregate may
  carry type-level scalar children without becoming type-level
  itself).
- *Issue 6.* Dropped the empty-string-fallback claim from
  `NullPtr` description; updated D-4-1.6 reasoning to use
  in-scope examples (vacant vtable slots vs source-level
  `core::ptr::null::<T>()`).
- *Issue 7.* Edited §3.3 Phase B to insert the value-level
  const validation step as new step 9 (between step 8 and
  Phase C); renumbered Phase C step 9 → step 10; updated the
  dependency-arrow summary to `consts (type-level) ← sigs ←
  types ← (layout, instances) ← consts (value-level)`. Updated
  step 4 wording to acknowledge the type-level phase split.
  §9.7's "Const validation phase split" subsection now
  references the concrete §3.3 step numbers (step 4 / step 9).
- *Issue 8.* Removed `ZST` from D-4-1.9's `Scalar::Int` arm in
  `discussions.md`.
- *Issue 9.* Refactored D-4-1.3 reasoning to clarify
  `Vec<ConstId>` is the storage shape; the cross-build canonical
  identity key recursively uses each child's canonical content
  per §12.3.

---

## Round 2 -- D-4-1 re-review after round-1 fixes

**Date.** 2026-05-02.

**Scope.** Same as round 1 (D-4-1 cluster: parent + sub-decisions
D-4-1.1 through D-4-1.9; spec under review §9.7 +
discussions.md). Round-1 corrections include: `InstanceRef`
shape realignment with §9.3; vtable rank fix; slice/memory
dispatch deferral; Byte invariant rephrasing; aggregate-child
type-level admissibility clarification; §3.3 step 9 insertion;
trivial fixes in `discussions.md`.

**Codex run.** `--effort high`, `--timeout 1500`, output to
`/tmp/codex_d4_1_review_r2.txt`.

**Verdict.** `BLOCK` -- 2 BLOCK + 3 MINOR + 0 NIT (5 total
issues). All round-1 issues closed; round-2 findings are
residual / corrections-of-corrections.

**Issues.**

1. *[BLOCK] §9.7 / D-4-1.5.* `Function` projection set
   `fn_shim_disambig: rustc_instance_kind_sort_rank(instance.def)`,
   but §9.3 / §9.4 say the public discriminator is
   `InstanceKind::Function.flavor` (`FunctionInstanceFlavor`)
   and the rank table maps that flavor to the `u8`. Wording
   conflated rustc-side `instance.def` with the Regseq-side
   flavor wrapper.
2. *[BLOCK] §9.7 / §12.3.* `ArgsKey::EMPTY` referenced by
   `Static` and `VTable` projections, but §12.3 defines only
   `pub struct ArgsKey(pub u128)` — no `EMPTY` constant. The
   projection depended on an undefined sentinel.
3. *[MINOR] §9.7 deferral consistency.* `Bytes` variant
   doc-comment listed `&str` / `&[u8]` backing as a use case
   while D-4-1.9 explicitly defers `&str` / `&[T]` reference-
   typed const handling.
4. *[MINOR] D-4-1 parent methodology (`discussions.md`).*
   Parent-decision compositional-behavior bullet still claimed
   "`&str` backing falls out of `Aggregate` over a `Reloc` and
   a `Uint`" without acknowledging the `ConstMemoryStatic`
   deferral.
5. *[MINOR] `status.md` stale D-4-1.5 shape.* "Open decisions"
   catalogue still listed D-4-1.5 as
   `(kind_rank, def_path_hash, args_key, shim_disambig:
   Option<ShimDisambig>)` — pre-correction shape. Could mislead
   the next pass.

Full Codex output: `/tmp/codex_d4_1_review_r2.txt`.

**Corrections applied.**

- *Issue 1.* Rewrote the `Function` GlobalAlloc projection row
  to extract `def_path_hash` from `instance.def_id()` and
  `fn_shim_disambig` from `rustc_instance_kind_sort_rank(flavor)`
  where `flavor: FunctionInstanceFlavor` is the §9.4 wrapper.
  Added explicit examples (`Item(foo)` rank 0, `ReifyShim(foo)`
  rank 3, `DropGlue(foo)` rank 10).
- *Issue 2.* Added `ArgsKey::EMPTY` constant definition to
  §12.3 ("ArgsKey value type" subsection) — the canonical
  `ArgsKey` for an empty `GenericArgs`, computed as the
  leading 16 bytes of `SHA-256(0x00 0x00 0x00 0x00)` (the
  4-byte LE u32 length prefix per §12.3's canonical encoding)
  loaded as a big-endian u128. Referenced from `Static` /
  `VTable` projections in §9.7 unchanged.
- *Issue 3.* Updated `Bytes` variant doc-comment in §9.7 to
  drop `&str` / `&[u8]` from listed use cases and added a
  pointer to the deferral subsection.
- *Issue 4.* Reworded D-4-1 parent decision's compositional-
  behavior bullet: explicitly noted that `&str` / `&[T]`
  reference-typed const handling is stage-4-deferred until
  `ConstMemoryStatic` lands; the `Aggregate([Reloc, Uint])`
  shape is the *expected post-deferral* compositional form,
  not an in-scope D-4-1 path.
- *Issue 5.* Updated `status.md` "Open decisions" D-4-1.5
  entry to the corrected 4-tuple
  `(regseq_kind_rank, def_path_hash, args_key,
  fn_shim_disambig)`.

---

## Round 3 -- D-4-1 re-review after round-2 fixes

**Date.** 2026-05-02.

**Scope.** Same as round 2. Round-2 corrections include:
`Function` projection rephrased via `FunctionInstanceFlavor`;
`ArgsKey::EMPTY` defined in §12.3; `Bytes` doc-comment scope
narrowed; D-4-1 parent decision &str example reframed as
post-deferral; `status.md` D-4-1.5 catalogue entry refreshed.

**Codex run.** `--effort high`, `--timeout 1500`, output to
`/tmp/codex_d4_1_review_r3.txt`.

**Verdict.** `BLOCK` -- 1 BLOCK + 1 MINOR + 0 NIT (2 total
issues). All round-2 issues closed; round-3 findings are
deeper-residual.

**Issues.**

1. *[BLOCK] §12.3 / `ArgsKey::EMPTY`.* The new `ArgsKey::EMPTY`
   definition relies on a "GenericArgs encodes as 4-byte
   length-zero" claim, but §12.3 never defines the
   `GenericArgs` container grammar (length prefix + N entries
   with `GenericArg::{Type, Const}` per-entry tags). The
   per-`Ty` encodings call `canonical_args(GenericArgs)`
   without the container grammar pinned. This leaves
   non-empty `argskey_of(instance.args)` under-specified.
2. *[MINOR] §9.3 / §9.7 function shim rank.* `rustc_instance_kind_sort_rank`
   in §9.3 is typed as taking `&rustc_ty::InstanceKind<'_>`,
   but the §9.7 corrected projection passes
   `flavor: FunctionInstanceFlavor`. Cosmetic typed-function-
   call mismatch.

Full Codex output: `/tmp/codex_d4_1_review_r3.txt`.

**Corrections applied.**

- *Issue 1.* Added `GenericArgs container encoding` subsection
  to §12.3 between the per-`Ty` tag table and the cross-build
  determinism subsection. Pins the grammar:
  `u32 LE arg_count` followed by N `GenericArg` entries; per
  entry `u8 disc` (0=Type, 1=Const) + canonical payload bytes.
  Empty case (N=0) yields exactly the 4 bytes
  `0x00 0x00 0x00 0x00`. `ArgsKey::EMPTY`'s
  prior-paragraph reference to "encoding rules below" now
  resolves to a concrete spec.
- *Issue 2.* Rephrased the §9.7 `Function` projection's
  `fn_shim_disambig` clause from a typed function call
  (`rustc_instance_kind_sort_rank(flavor)`) to a table-lookup
  reference: "the `u8` rank looked up in §9.3's
  `rustc_instance_kind_sort_rank` table for the
  `FunctionInstanceFlavor` corresponding to `instance.def`."
  Avoids the type-signature mismatch without redefining the
  §9.3 helper.

---

## Round 4 -- D-4-1 re-review after round-3 fixes

**Date.** 2026-05-02.

**Scope.** Same as round 3. Round-3 corrections include: `GenericArgs`
container grammar pinned in §12.3; `ArgsKey::EMPTY`'s
encoding-byte composition now resolved; §9.7 `Function`
projection rephrased to table-lookup form.

**Codex run.** `--effort high`, `--timeout 1500`, output to
`/tmp/codex_d4_1_review_r4.txt`.

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total).
All BLOCKs cleared; sole remaining issue is a cross-reference
correctness fix.

**Issues.**

1. *[MINOR] D-4-1 / §12.3 (`spec-snapshot.md`).* The new
   "GenericArgs container encoding" subsection's introductory
   parenthetical claimed `canonical_args(GenericArgs)` appears
   in the `Tuple` per-field rule, but the per-`Ty` table
   entry for `Tuple` actually uses `canonical_args(TyId)`
   (per-field, not the `GenericArgs` container). Cross-
   reference error in the parenthetical, not a grammar
   failure.

Full Codex output: `/tmp/codex_d4_1_review_r4.txt`.

**Corrections applied.**

- *Issue 1.* Removed "and the `Tuple` per-field rule" from
  the introductory parenthetical of the new `GenericArgs
  container encoding` subsection. The remaining list (`FnDef`,
  `Closure`, `Struct`, `Enum`, `Union`) matches the per-`Ty`
  table entries that actually invoke `canonical_args(GenericArgs)`.

---

## Round 5 -- D-4-1 final re-review after round-4 fix

**Date.** 2026-05-02.

**Scope.** Same as round 4. Round-4 correction is the
single-line removal of "and the `Tuple` per-field rule" from
§12.3's `GenericArgs container encoding` subsection
introductory parenthetical.

**Codex run.** `--effort high`, `--timeout 1500`, output to
`/tmp/codex_d4_1_review_r5.txt`.

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total).
Round-4 fix verified correct; one **new** MINOR found
(longstanding inconsistency that prior rounds did not flag).

**Issues.**

1. *[MINOR] D-4-1.1 / §9.7 (`spec-snapshot.md`).* Float-padding
   wording inconsistency between `discussions.md` and
   `spec-snapshot.md`. `discussions.md` D-4-1.1 says smaller
   floats "zero-pad into the high bits" with worked example
   `Float(F32, 0x3f800000)` (which has the upper 96 bits of
   the `u128` zero). §9.7's `Float` doc-comment said
   "zero-pad into the low-order bits" and the bridge dispatch
   table said "low-order zero-pad" — both read as "low-order
   bits are zero," contradicting the example.

Full Codex output: `/tmp/codex_d4_1_review_r5.txt`.

**Corrections applied.**

- *Issue 1.* Rewrote both §9.7 references to use unambiguous
  phrasing: "the bit-pattern occupies the low-order bits of
  `u128`; the high-order bits are zero for narrow widths,"
  with the same `1.0f32 → 0x3f800000` example as
  `discussions.md`. Both files now agree on the bit-pattern
  layout.

---

## Round 6 -- D-4-1 final close-out re-review after round-5 fix

**Date.** 2026-05-02.

**Scope.** Same as round 5. Round-5 correction is the float-
padding wording fix in §9.7's `Float` doc-comment and bridge
dispatch table.

**Codex run.** `--effort high`, `--timeout 1500`, output to
`/tmp/codex_d4_1_review_r6.txt`.

**Verdict.** `APPROVE` -- 0 BLOCK + 0 MINOR + 0 NIT (0 total).
D-4-1 cluster closed. Codex confirms cumulative scoped
consistency holds across §3.3 (content-keyed uniqueness), §9.7
(validation phase split), §12.3 (canonical const recursion).
Audit-shape coverage retained for D-4-1 parent and all 9
sub-decisions.

**Issues.** None.

**Corrections applied.** None required.

---

## Round-trajectory summary

```
R1: 10 issues (7 BLOCK / 3 MINOR / 0 NIT)   [BLOCK]
R2:  5 issues (2 BLOCK / 3 MINOR / 0 NIT)   [BLOCK] -- all R1 closed
R3:  2 issues (1 BLOCK / 1 MINOR / 0 NIT)   [BLOCK] -- all R2 closed
R4:  1 issue  (0 BLOCK / 1 MINOR / 0 NIT)   [MINOR] -- all R3 closed
R5:  1 issue  (0 BLOCK / 1 MINOR / 0 NIT)   [MINOR] -- R4 closed; new pre-existing inconsistency surfaced
R6:  0 issues                                [APPROVE] -- R5 closed
```

20 distinct findings closed across the iteration. D-4-1 cluster
ready to be marked `[LOCKED]` in `discussions.md` and
`status.md`. The cluster is the first stage-4 spec contribution
ready for the merger contract (per `../README.md` §5.5); the
merger will run after subsequent stage-4 sub-decisions
(D-4-2 onward) close, since stage-4 is solo and the merger is
end-of-stage, not end-of-cluster.
