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

## Round-trajectory summary -- D-4-1

```
R1: 10 issues (7 BLOCK / 3 MINOR / 0 NIT)   [BLOCK]
R2:  5 issues (2 BLOCK / 3 MINOR / 0 NIT)   [BLOCK] -- all R1 closed
R3:  2 issues (1 BLOCK / 1 MINOR / 0 NIT)   [BLOCK] -- all R2 closed
R4:  1 issue  (0 BLOCK / 1 MINOR / 0 NIT)   [MINOR] -- all R3 closed
R5:  1 issue  (0 BLOCK / 1 MINOR / 0 NIT)   [MINOR] -- R4 closed; new pre-existing inconsistency surfaced
R6:  0 issues                                [APPROVE] -- R5 closed
```

20 distinct findings closed across the iteration. D-4-1 cluster
LOCKED in `discussions.md` and `status.md`. The cluster is the
first stage-4 spec contribution ready for the merger contract
(per `../README.md` §5.5); the merger will run after subsequent
stage-4 sub-decisions (D-4-2 onward) close, since stage-4 is
solo and the merger is end-of-stage, not end-of-cluster.

---

## Round 1 -- D-4-2 initial review

**Date.** 2026-05-03.

**Scope.** D-4-2 cluster (parent decision + sub-decisions D-4-2.1
through D-4-2.9) covering the per-`Const`-variant byte-encoding
tags subsection of §9.7 (the `ConstIdentityKey` byte form),
§12.3's concrete `ConstId` walk-rule row replacing the
stage-4-schema stub, and §3.3 Phase B step 9's added const-graph
acyclicity sub-bullet.

**Verdict.** BLOCK -- 6 issues (2 BLOCK / 4 MINOR / 0 NIT).

**Issues raised.**

1. **[BLOCK] args-key-byte-source.** §9.7's `args_key` byte
   form said "underlying 16 bytes from `ArgsKey.0`" without
   pinning a `u128`-to-bytes operation; native-endian /
   transmute interpretation possible.
2. **[BLOCK] acyclicity-algorithm.** §3.3 step 9 + D-4-2.6
   said "DFS visited-set" without distinguishing
   "currently-on-stack" from "already-validated"; could miss
   cycles or falsely reject shared DAG children.
3. **[MINOR] scalar-kind-discriminant-crossref.** §9.7
   pointed at "§12.3 assignments" for `IntKind` /
   `UintKind` / `FloatKind` discs, but the normative
   source is §10's `#[repr(u8)]` enum definitions.
4. **[MINOR] append-only-audit-wording.** D-4-2.1 said
   "245 reserved tag values" right after invoking the
   no-forward-looking-placeholders rule -- contradiction.
5. **[MINOR] stale-line-crossrefs.** D-4-2 audit text used
   line refs (`spec-snapshot.md:2680-2706`,
   `spec-snapshot.md:2716`, `spec-snapshot.md:2684-2686`,
   `spec-snapshot.md:2658-2660`, `spec-snapshot.md:2649-2650`)
   that drifted after §9.7's byte-encoding subsection
   shifted line numbers.
6. **[MINOR] prior-art-overclaim.** D-4-2 parent cited
   `prior-art/README.md` §3.1 as "cross-cluster framing of
   hash-based identity keys for content-addressed dedup,"
   but §3.1 actually documents the *negative* result
   (5-of-5 non-rustc projects fail) with only rustc as
   adjacent precedent.

**Corrections applied.**

- Issue 1: Replaced "underlying 16 bytes" wording in §9.7
  and D-4-2.8 with explicit `ArgsKey.0.to_be_bytes()`;
  forbid native memory layout / transmute; spelled out the
  documented BE exception to §12.3's general LE rule
  (anchored on §12.3's BE-load convention for
  `ArgsKey::EMPTY`).
- Issue 2: Replaced "DFS visited-set" wording in §3.3
  step 9 and D-4-2.6 with explicit three-state DFS
  (`Unseen` / `Visiting` / `Done`); back-edge to
  `Visiting` is corruption, edge to `Done` is skipped
  (legitimate DAG sharing).
- Issue 3: Updated §9.7 byte-encoding subsection and
  D-4-2.2 to cite §10's `#[repr(u8)]` enum definitions
  as the normative source for `IntKind` / `UintKind` /
  `FloatKind` discs; noted §12.3 reuses the same values.
- Issue 4: Reworded D-4-2.1 from "245 reserved tag values"
  to "245 currently-unassigned tag values available for
  append-only future variants"; clarified
  no-forward-looking-placeholders means values are not
  pre-reserved for any specific extension.
- Issue 5: Replaced specific line refs in D-4-2 audit text
  (D-4-2 parent reasoning, D-4-2.2's discriminant cite,
  D-4-2.3's `GenericArgs` cite, D-4-2.8's BE-load
  convention cite) with section-only refs that don't drift
  with line numbers.
- Issue 6: Reframed D-4-2's `README.md` §3.1 cite to
  match D-4-1.5's accurate framing (5-of-5 negative;
  rustc adjacent); reframed D-4-2.1 / D-4-2.3's cites
  to use per-project files (LLVM, MLIR, charon, mrustc,
  regseq-model-old) instead of overclaiming via §3.1;
  reframed D-4-2.8's cite to inherit from D-4-1.5's
  framing.

Round-2 prompt focus: verify each correction; surface any
new issues introduced by the corrections; check residual
soundness across the cluster.

---

## Round 2 -- D-4-2 re-review after round-1 fixes

**Date.** 2026-05-03.

**Scope.** Same as round 1. Round-1 corrections summarised
above.

**Verdict.** MINOR -- 4 issues (0 BLOCK / 4 MINOR / 0 NIT).
All round-1 issues verified closed. Findings are residual
cleanup of the round-1 fixes (incomplete propagation across
parallel mentions of the same concept).

**Issues raised.**

1. **[MINOR] args-key-underlying-bytes-residual.** D-4-2.8's
   normative text correctly pinned `to_be_bytes()`, but a
   downstream paragraph still said the stream "emits the
   underlying bytes directly, making endianness a
   non-question" -- partial regression on the R1 fix.
2. **[MINOR] acyclicity-visited-set-residual.** §3.3 step 9
   and D-4-2.6's main text correctly say "three-state DFS
   marks," but §9.7's recursion paragraph and D-4-2.6's
   first option text still summarised as "DFS visited-set."
3. **[MINOR] scalar-kind-crossref-residual.** D-4-2.2's
   normative text correctly cites §10's enum definitions,
   but the option text still said "reusing the §12.3
   assignments" and the cross-ref still located the disc
   assignments in §12.3.
4. **[MINOR] stale-line-crossrefs-residual.** R1 cleanup
   missed several stale `spec-snapshot.md:NNNN` refs:
   D-4-2 parent's "lines 2680-2706," D-4-2.4's
   `spec-snapshot.md:162`, D-4-2.4's
   `spec-snapshot.md:1689-1692`, D-4-2.5's
   `spec-snapshot.md:1741-1758`, D-4-2.7's
   `spec-snapshot.md:2669`, D-4-2.8's
   `spec-snapshot.md:1767-1803`.

**Corrections applied.**

- Issue 1: Replaced D-4-2.8's lingering "underlying bytes
  directly, making endianness a non-question" paragraph
  with explicit `ArgsKey.0.to_be_bytes()` wording, anchored
  on §12.3's BE-load convention.
- Issue 2: Updated §9.7's recursion paragraph from
  "DFS visited-set" to "three-state DFS marks --
  Unseen / Visiting / Done -- ... back-edge to Visiting
  is corruption, edge to Done is skipped as a legitimate
  shared DAG child." Updated D-4-2.6's first option text
  from "visited-set" to "three-state DFS marks
  (Unseen / Visiting / Done)" with back-edge-to-Visiting
  rejection.
- Issue 3: Updated D-4-2.2's option 1 text and Cross-refs
  to cite §10's `#[repr(u8)]` enum definitions as the
  normative source; §12.3 noted only as downstream reuse.
- Issue 4: Replaced all six remaining stale line refs in
  discussions.md with section-only cross-references
  (`§12.3's per-Ty byte-encoding-tags table`,
  `§3 VariantIdx type-alias declaration`,
  `§9.7's Const::EnumValue declaration`,
  `§9.7's Byte enum "Byte-sequence well-formedness
  invariants" subsection`,
  `§12.3's per-IR-identifier termination rule for TyId`,
  `§9.7's InstanceRef struct-declaration order`).

Round-3 prompt focus: verify R2 corrections; confirm no
further residual ambiguity; check whether the cluster is
ready for APPROVE.

---

## Round 3 -- D-4-2 re-review after round-2 fixes

**Date.** 2026-05-03.

**Scope.** Same as round 2. Round-2 corrections summarised
above.

**Verdict.** BLOCK -- 1 issue (1 BLOCK / 0 MINOR / 0 NIT).
All round-2 issues verified closed (R2.1 / R2.2 / R2.3 / R2.4
all confirmed). The BLOCK is a deeper architectural finding
about validation ordering that the cumulative-consistency
pass surfaced after R2's residual cleanup completed.

**Issue raised.**

1. **[BLOCK] validation-order-before-acyclicity.** §3.3 step 4
   said it validates `ConstInterner.lookup` injectivity over
   canonical-content keys "for all variants" before step 9's
   acyclicity sub-bullet. But computing canonical keys for
   `Aggregate` / `EnumValue` requires recursive
   `canonical_const(ConstId)` calls, and step 9's three-state
   DFS is what makes such recursion safe (prevents infinite
   loops on tampered cyclic models). The validator was ordered
   to compute recursive value-level keys BEFORE the cycle
   check that makes those computations safe.

**Correction applied.**

- Restructured the §3.3 phase split so step 4 is type-level
  only: (a) recompute canonical-content keys for type-level
  scalar entries (no recursion; bounded), (b) verify they
  match stored `ConstInterner.lookup` keys, (c) injectivity
  over the type-level subset of `ConstInterner.lookup`,
  (d) every `GenericArg::Const(_)` reference resolves to a
  type-level variant. Value-level entries are NOT recomputed
  at step 4.
- Added a new value-level sub-bullet to §3.3 step 9 ordered
  AFTER the const-graph acyclicity sub-bullet:
  "Value-level lookup injectivity and canonical-form
  well-formedness." Recomputes canonical keys for value-level
  entries (now safe to recurse — DAG verified), checks they
  match stored keys, verifies injectivity over the value-level
  subset. Combined with step 4's type-level pass, this
  completes injectivity over the full `ConstInterner.lookup`
  key set.
- Updated §9.7's "Const validation phase split" subsection to
  reflect the refined split and the step-9 ordering.
- Updated D-4-2.6 audit-shape entry with a "Validation-order
  refinement" paragraph spelling out the four sub-steps of
  step 9 in order: resolve → content checks → acyclicity →
  value-level key recomputation + injectivity.

Round-4 prompt focus: verify R3 correction; confirm no
further validation-order issues; check whether the cluster is
ready for APPROVE.

---

## Round 4 -- D-4-2 re-review after round-3 fixes

**Date.** 2026-05-03.

**Scope.** Same as round 3. Round-3 correction summarised
above.

**Verdict.** BLOCK -- 2 issues (1 BLOCK / 1 MINOR / 0 NIT).
Both are residual cleanup of the round-3 phase-split
restructuring that did not propagate to two parallel
mentions.

**Issues raised.**

1. **[BLOCK] phase-split-summary-still-claims-full-step4-uniqueness.**
   §3.3's "Phase B (continued) — late value-level const
   validation" preface still said "item 4 validates
   `ConstInterner` content uniqueness (no duplicate canonical-
   content lookup-map keys)" without the type-level qualifier
   — partial regression on the R3 fix. As written, the summary
   could be read as reintroducing the round-3 defect: full
   lookup-key uniqueness before value-level recursive keys are
   safe to compute.
2. **[MINOR] phase-split-order-wording-ambiguous.** §9.7's
   "Const validation phase split" subsection said
   "step-9 sub-bullets are ordered so const-graph acyclicity
   verifies first," but §3.3 step 9 actually orders sub-
   reference / content checks BEFORE acyclicity, then value-
   level key recomputation AFTER acyclicity. The §9.7 wording
   contradicted the §3.3 step ordering and D-4-2.6's audit-
   shape sub-step description.

**Corrections applied.**

- Issue 1: Updated §3.3's late-value-level preface to qualify
  step 4 as the type-level subset only ("type-level scalar
  entries: canonical-content lookup-map injectivity and
  canonical-form well-formedness for `Bool` / `Char` / `Int`
  / `Uint` / `Float`"); explicitly noted that "full
  `ConstInterner.lookup` injectivity over the entire key set
  is completed by the value-level pass at step 9 after
  acyclicity verifies." Updated step 9's preface to say
  "sub-reference resolution and value-level lookup injectivity
  are validated late."
- Issue 2: Replaced §9.7's "acyclicity verifies first" wording
  with the explicit four-sub-step ordering: (1) sub-reference
  resolution; (2) content checks; (3) acyclicity via three-
  state DFS; (4) value-level key recomputation + lookup
  injectivity. Noted that steps (1) and (2) are recursion-
  free (resolution, not key recomputation) so they can run
  before acyclicity; step (4) runs after acyclicity so
  recursive `canonical_const(ConstId)` calls terminate.

Round-5 prompt focus: verify R4 corrections; confirm no
further parallel mentions of validation order need updating;
check whether the cluster is ready for APPROVE.

---

## Round 5 -- D-4-2 re-review after round-4 fixes

**Date.** 2026-05-03.

**Scope.** Same as round 4. Round-4 corrections summarised
above.

**Verdict.** MINOR -- 1 issue (0 BLOCK / 1 MINOR / 0 NIT).
All round-4 issues verified closed. The one finding is a
naming inconsistency surfaced by the cumulative-consistency
lens.

**Issue raised.**

1. **[MINOR] stale-canonical-const-repr-name.** §9.7's
   identity contract still said `ArgsKey` traversal queries
   `canonical_const_repr(ConstId)`, but the D-4-2 byte-form
   subsection defined the recursion as
   `canonical_const(ConstId)`, leaving one stale, undefined
   helper name in the cluster. Pre-existing wording from
   D-4-1's draft phase; surfaced now because R4's cleanup
   pass made naming consistency tractable to verify.

**Correction applied.**

- Replaced §9.7's `canonical_const_repr(ConstId)` reference
  with `canonical_const(ConstId)` and added "per the per-
  `Const`-variant byte-encoding tags subsection below" so
  readers can locate the helper's definition.
- Replaced one further `canonical_const_repr` reference in
  D-4-1.3's locked audit-shape entry (at `discussions.md:331`)
  with `canonical_const` plus a note that this is D-4-2's
  per-`Const`-variant byte-encoding helper. D-4-1.3 is
  LOCKED but the corrected naming is normative now, and the
  prior inconsistent name was a placeholder.

Round-6 prompt focus: verify R5 correction; check whether the
cluster is ready for APPROVE.

---

## Round 6 -- D-4-2 re-review after round-5 fix

**Date.** 2026-05-03.

**Scope.** Same as round 5. Round-5 correction summarised
above.

**Verdict.** MINOR -- 2 issues (0 BLOCK / 2 MINOR / 0 NIT).
R5 fix verified closed. Both findings are residual cross-
reference inaccuracies surfaced by the cumulative-consistency
lens.

**Issues raised.**

1. **[MINOR] stale-genericarg-const-phase-label.** §12.3's
   `GenericArg::Const` encoding said "the value-level
   identity-key rules pinned in D-4-2," but
   `GenericArg::Const(_)` positions resolve to type-level
   scalar variants only per §3.3 step 4 and §9.7's partition.
   Pre-existing wording from before D-4-2 closed; surfaced
   now because R5's naming-consistency pass made phase-label
   verification tractable.
2. **[MINOR] d-4-2-array-constid-crossref.** D-4-2 parent's
   reasoning said §12.3's `ConstId` walk-rule calls
   `canonical_const(ConstId)` from the per-`Ty` table for
   "(`Array`, `GenericArg::Const`)," but `Ty::Array` carries
   `(TyId, u64)` (a literal `u64` length, not a `ConstId`),
   so `Array` is not actually a `ConstId` call site. The
   only `ConstId` call site in the per-`Ty` walk is the
   `GenericArgs` container's `GenericArg::Const(ConstId)`
   arm.

**Corrections applied.**

- Issue 1: Replaced §12.3's `disc = 1` payload note from
  "the value-level identity-key rules pinned in D-4-2" to
  "the per-`Const`-variant byte-encoding rules pinned in
  D-4-2 / §9.7" plus "GenericArg::Const(_) positions resolve
  to type-level scalar variants only per §3.3 step 4 and
  §9.7's type-level / value-level partition."
- Issue 2: Updated D-4-2 parent's reasoning (iii) to drop
  `Array` from the call-site list and clarify that the only
  `ConstId` call site is the `GenericArgs` container's
  `GenericArg::Const(ConstId)` arm; noted that
  `Ty::Array(elem, n: u64)` carries a literal `u64`, not a
  `ConstId`.

Round-7 prompt focus: verify R6 corrections; final
cumulative-consistency check; expected to APPROVE.

---

## Round 7 -- D-4-2 final close-out re-review after round-6 fixes

**Date.** 2026-05-03.

**Scope.** Same as round 6. Round-6 corrections summarised
above.

**Verdict.** APPROVE -- 0 issues (0 BLOCK / 0 MINOR / 0 NIT).
All round-6 issues verified closed. Cumulative-consistency
pass found no further issues. D-4-2 cluster closed.

**Corrections applied.** None required.

---

## Round-trajectory summary -- D-4-2

```
R1: 6 issues (2 BLOCK / 4 MINOR / 0 NIT)   [BLOCK]
R2: 4 issues (0 BLOCK / 4 MINOR / 0 NIT)   [MINOR] -- all R1 closed; residual cleanup
R3: 1 issue  (1 BLOCK / 0 MINOR / 0 NIT)   [BLOCK] -- all R2 closed; deeper validation-order finding surfaced
R4: 2 issues (1 BLOCK / 1 MINOR / 0 NIT)   [BLOCK] -- R3 closed; phase-split parallel mentions not propagated
R5: 1 issue  (0 BLOCK / 1 MINOR / 0 NIT)   [MINOR] -- all R4 closed; naming inconsistency surfaced
R6: 2 issues (0 BLOCK / 2 MINOR / 0 NIT)   [MINOR] -- R5 closed; cross-reference inaccuracies surfaced
R7: 0 issues                                [APPROVE] -- all R6 closed
```

16 distinct findings closed across the iteration. D-4-2
cluster LOCKED in `discussions.md` and `status.md`. The
trajectory pattern (alternating BLOCK / MINOR rounds with
deeper findings surfacing as cumulative-consistency tightened)
mirrors D-4-1's R1=10 → R2=5 → R3=2 → R4=1 → R5=1 → R6=0
trajectory; both clusters required ~6-7 rounds to converge,
which is the established stage-4 spec-review depth.

---

## Round 1 -- D-4-3 initial review

**Date.** 2026-05-03.

**Scope.** D-4-3 cluster (parent decision + sub-decisions
D-4-3.1 through D-4-3.10) covering reference-typed const
handling via the new `SyntheticItemKind::ConstMemoryStatic`
family. Whole-family scope: stage-4 owns bridge dispatch +
const composition + Phase B validation; stage-12 territory
(§13.3 enum extension + §13.4 segments / DefPathHash) closed
in stage 4 since the changes mirror the vtable family
precedent. Spec under review: `stage-4/spec-snapshot.md` §9.7
(new "Reference-typed const handling (D-4-3)" subsection
replacing the prior deferred subsection; new dispatch table
rows for `Slice`, `Indirect`-over-Memory, and `Memory(memory)`
GlobalAlloc projection); §13.3 (`ContentHash` type;
`ConstMemoryStatic` variant + canonical encoding subsection);
§13.4 (const-memory family segments + DefPathHash + ownership
rule); §3.3 Phase B step 9 (`ConstMemoryStatic dedup` sub-
bullet). Audit shape: `stage-4/discussions.md` D-4-3 parent +
D-4-3.1 through D-4-3.10. Outbound: `stage-12/expectations.md`
"From stage 4" flipped to `[CLOSED by D-4-3]`.

**Verdict.** BLOCK -- 6 issues (2 BLOCK / 3 MINOR / 1 NIT).

**Issues raised.**

1. **[BLOCK] static-init-shape.** `InstanceKind::Static`
   stores `init: Option<ConstId>`, but the synthesis rule
   said `init: ConstId` (without the `Option` wrapper).
   Schema mismatch.
2. **[BLOCK] phase-b-family-validation.** §3.3 Phase B's
   `SyntheticItemKind ↔ ItemKind` consistency list omitted
   `ConstMemoryStatic → ItemKind::Static`. Step 9 had a
   dedup check but no immutable-statics validation parallel
   to the existing `VTableInstance` immutable-statics rule.
3. **[MINOR] stale-deferred-const-text.** §9.7 still had
   inline doc-comments on `Const::Bytes` and `Const::NullPtr`
   saying reference-typed handling "is deferred" even though
   D-4-3 closes it. The append-only `Const`-tag note also
   mentioned a future `ConstMemoryStatic`-backed family
   appending at tag `11+`, contradicting D-4-3's "no new
   `Const` variant" decision.
4. **[MINOR] thin-pointer-type-side.** The synthesised Static
   is type-erased to `Ty::Array(U8, N)`. Wide-pointer (Slice)
   dispatch documented how the typed-pointer side is carried
   by the wrapping `Aggregate`, but thin-pointer (Indirect)
   dispatch had no equivalent statement about how the
   pointee type is recovered at the use site.
5. **[MINOR] forward-placeholder-d4k.** Non-zero offset
   rejection text named a "future D-4-K extension" — a
   forward-looking placeholder that contradicts D-4-3.8's
   audit-shape claim of "no slot reserved."
6. **[NIT] methodology-mislabel.** D-4-3.10 (audit-trail
   bookkeeping) cited "Cross-build canonicalisation via
   stable identity" as its methodology rule, which is a
   substantive methodology rule that doesn't apply to
   bookkeeping.

**Corrections applied.**

- Issue 1: Updated §9.7 synthesis rule and D-4-3.6 audit-shape
  to write `init: Some(c)` with the full `Static { ty, init,
  mutability }` shape; explicitly noted that `init: None` is
  invalid for `ConstMemoryStatic` Items. Updated §3.3 step 9
  dedup sub-bullet to read `init: Some(c)` and reference the
  immutable-statics rule for `init: None` rejection.
- Issue 2: Added `ConstMemoryStatic { .. } → ItemKind::Static`
  to the `SyntheticItemKind ↔ ItemKind` consistency list in
  §3.3 Phase B. Added a new `ConstMemoryStatic` immutable-
  statics validation bullet parallel to the
  `VTableInstance` rule, requiring `mutability: Not`,
  `init: Some(c)`, byte-array `Ty` shape, and `Const::Bytes`
  init shape.
- Issue 3: Updated §9.7's `Const::Bytes` doc-comment to
  reference `ConstMemoryStatic` synthesised statics directly
  (no longer "deferred"). Updated `Const::NullPtr` doc-comment
  to clarify that reference-typed consts are handled by D-4-3,
  not as `NullPtr`. Updated the append-only `Const`-tag note
  to remove the `ConstMemoryStatic`-backed-family example
  (since D-4-3 confirmed no new `Const` variant) and replaced
  with a generic future-variant statement.
- Issue 4: Added a "Pointee type at the use site" paragraph
  after the bridge dispatch arms, explicitly stating that the
  consumer's context supplies the pointee type for both Slice
  (via the wrapping `Aggregate`'s position type) and
  Indirect-thin (via the `Reloc`'s position type). Noted
  C codegen casts from the byte static at the use site.
- Issue 5: Removed "future D-4-K extension" wording from §9.7
  and from the D-4-3 parent + D-4-3.8 audit entries. Replaced
  with "rejected unless a future spec revision admits them"
  (no specific decision name reserved).
- Issue 6: D-4-3.10 methodology rules updated to "None
  applicable — audit-trail bookkeeping," referencing the
  project-wide outbound-expectations convention rather than
  a stage-4 methodology rule.

Round-2 prompt focus: verify each correction; surface any
new issues introduced by the corrections; check residual
soundness across the cluster.

---

## Round 2 -- D-4-3 re-review after round-1 fixes

**Date.** 2026-05-03.

**Scope.** Same as round 1. Round-1 corrections summarised
above.

**Verdict.** BLOCK -- 3 issues (1 BLOCK / 2 MINOR / 0 NIT).
All round-1 issues verified closed. The findings are
schema-shape and provenance / consistency cleanups surfaced
by the cumulative-consistency lens.

**Issues raised.**

1. **[BLOCK] item-ty-schema.** D-4-3 synthesis said
   `item.ty = TyId for Ty::Array(...)`, but the stage-1 `Item`
   schema does not have a `ty` field -- per-kind rich data
   (including the Static's `ty`) lives on the `Instance`, not
   the `Item`. `Item` carries only `def_path`, `kind`, `span`,
   flags, and `opaque_handle_eligible`.
2. **[MINOR] stale-stage4-status.** `stage-12/expectations.md`
   was already flipped to `[CLOSED by D-4-3]`, but
   `stage-4/status.md` still said D-4-3 was "next / open" with
   the deferred-items section flagging `&str` / `&[T]` as
   open. Inconsistent in-flight state.
3. **[MINOR] wrong-lock-provenance.** D-4-3's non-zero-offset
   rationale said adding an offset would change a "stage-3-
   locked structure" twice, but `InstanceRef` is the
   stage-4-locked const-relocation shape from D-4-1.5 / §9.7.

**Corrections applied.**

- Issue 1: Removed `item.ty = TyId for Ty::Array(...)` from
  the §9.7 synthesis rule and from D-4-3.6's audit-shape
  decision text. Updated the synthesised `Instance` shape to
  carry the `ty` (`InstanceKind::Static { ty: array_ty, init:
  Some(c), mutability: Not }`) and added an explanatory note
  in both places that per-kind rich data lives on the
  `Instance`, not the `Item`, per the stage-1 schema. The
  immutable-statics validation rule was already correct
  (reads `array_ty` from the Instance, not the Item) -- no
  change needed there.
- Issue 2: Updated `stage-4/status.md`'s top-of-file Status
  line to "D-4-3 IN CODEX REVIEW"; reworked the inbound-
  expectations bullet to mark `&str` / `&[T]` closed by D-4-3
  (in active review); added an "In-flight decisions" section
  for D-4-3; renamed "Open decisions" to "(post-D-4-3)";
  rewrote the "Stage-4-specific deferred items" section to
  state no items are currently active; rewrote "Next concrete
  unit of work" to focus on completing the D-4-3 review loop;
  updated the cluster-split discussion to reference D-4-K
  (not D-4-3+) as the only remaining decision after D-4-3.
- Issue 3: Replaced "stage-3-locked structure" with
  "stage-4-locked `InstanceRef` shape (D-4-1.5 / §9.7)" in
  both occurrences in `discussions.md` (D-4-3 parent reasoning
  + D-4-3.8 reasoning).

Round-3 prompt focus: verify R2 corrections; check whether
the cluster is ready for APPROVE.

---

## Round 3 -- D-4-3 re-review after round-2 fixes

**Date.** 2026-05-03.

**Scope.** Same as round 2. Round-2 corrections summarised
above.

**Verdict.** MINOR -- 1 issue (0 BLOCK / 1 MINOR / 0 NIT).
All round-2 issues verified closed. The single finding is a
cross-reference inaccuracy in the D-4-3 parent's audit-shape
entry that did not propagate when status.md was restructured.

**Issue raised.**

1. **[MINOR] stale-d4-3-crossrefs.** D-4-3 parent's "Cross-refs"
   section pointed at `status.md` "Open decisions / Locked
   decisions" but D-4-3 now lives under "In-flight decisions"
   (with "Open decisions (post-D-4-3)" containing only D-4-K).
   Same cross-ref said `review-log.md` rounds were "none yet,"
   but D-4-3 had Round 1 and Round 2 entries.

**Correction applied.**

- Updated D-4-3 parent's cross-ref to point at `status.md`
  "In-flight decisions" / "Open decisions (post-D-4-3)" and
  `review-log.md` "D-4-3 rounds" (replacing "rounds (none
  yet)").

Round-4 prompt focus: verify R3 correction; final cumulative-
consistency check; expected to APPROVE.

---

## Round 4 -- D-4-3 final close-out re-review after round-3 fix

**Date.** 2026-05-03.

**Scope.** Same as round 3.

**Verdict.** APPROVE -- 0 issues (0 BLOCK / 0 MINOR / 0 NIT).
All round-3 issues verified closed. Cumulative-consistency
pass found no further issues. D-4-3 cluster closed.

**Corrections applied.** None required.

---

## Round-trajectory summary -- D-4-3

```
R1: 6 issues (2 BLOCK / 3 MINOR / 1 NIT)   [BLOCK]
R2: 3 issues (1 BLOCK / 2 MINOR / 0 NIT)   [BLOCK] -- all R1 closed; schema + provenance findings
R3: 1 issue  (0 BLOCK / 1 MINOR / 0 NIT)   [MINOR] -- all R2 closed; cross-ref propagation
R4: 0 issues                                [APPROVE] -- R3 closed
```

10 distinct findings closed across the iteration. D-4-3
cluster LOCKED in `discussions.md` and `status.md`. The
trajectory was tighter than D-4-1 (R1=10, 6 rounds) and
D-4-2 (R1=6, 7 rounds) -- 4 rounds total -- because most
D-4-3 design decisions were mechanical mirror of the
vtable-family precedent (§13.2 / §13.3 / §13.4) rather than
new design space. The deepest finding (R2 item-ty-schema)
was a real schema-shape error caught by the cumulative-
consistency lens after the surface-level wording errors
had been cleared.
