# Stage 3 SESSION-C -- Discussion Record (addendum)

This file is a session-local addendum to `../../discussions.md`. It
captures the distilled discussion records for the four decisions
owned by SESSION-C:

- **D9** -- Volatile semantics (per-op vs per-type) -- LOCKED
- **D10** -- Atomics (operation-level vs type-level) -- LOCKED
- **D13** -- Discriminant level (typeck vs layout; per-variant values) -- LOCKED
- **D16** -- `c_void` recognition (rejected at bridge in v1) -- LOCKED

Format mirrors `../../discussions.md` Section 1: each entry carries
**Question / Key options / Decision (or Lean) / Reasoning /
Cross-refs**. Entries here are written so that the post-parallel
merge session can lift them directly into Section 1 of the shared
`discussions.md`.

Status of each decision in this file is *session-local*:
SESSION-C marks D9 and D10 LOCKED here pending the merge
session's integration into the shared docs. The live shared
status -- recorded in `../../stage-3-status.md` and in the
Section 5 open-decisions table of `../../vocabulary.md` --
will continue to show D9 and D10 as open until the merge
session lands SESSION-C's content (the merge-session
replacement S5 rows are in `vocabulary-SESSION-C.md` S5).

---

## 1. Decision discussions

### D9 -- Volatile semantics: no `Ty` bit; volatile lives on the intrinsic `InstanceId` [LOCKED]

**Question.** Does our `Ty` carry a volatile bit (CompCert-style),
does a generic load/store `Statement` carry a `volatile: bool`
flag (CLR-style per-op), or is volatility purely a property of
specific intrinsic `InstanceId`s with no IR-level flag at any
level?

The framing extends to a meta-question: do we have a
QualType-style type-level qualifier wrapper at all? `const`,
`static`, and `restrict` were already handled by other locked
decisions but had not been articulated as a unified pattern.

**Key options.**
1. *Per-type* (CompCert-style `attr_volatile: bool` on `Ty`).
2. *Per-op flag on a generic load/store `Statement`* (CLR-style
   `LdInd { volatile: bool }`).
3. *Per-intrinsic-`InstanceId`* (no IR-level volatile flag at
   any level; volatility is a codegen-emission fact about
   specific MMIO intrinsic `InstanceId`s).

**Decision.** Option 3, plus articulation of the implicit "no
QualType layer" pattern that already governs `const`, `static`,
and `restrict`. The bridge rejects raw
`core::ptr::read_volatile` / `write_volatile` calls
(Call-terminator-level rejection in stage 9 / stage 13
enforcement); MMIO access flows through
`#[rseq::intrinsic("mmio_*")]` exclusively per the DSL
three-mechanism convention.

**Reasoning.**

The "no QualType layer" pattern: regseq-model's `Ty`
represents the unqualified Rust type; C-side qualifiers are
emitted at codegen time from per-decl / per-op information
elsewhere in the IR. Each qualifier already maps to a distinct
site:

- `const` on `Ty::Ref` → `Provenance` on `Ty::Ref(TyId, Provenance)` (locked D7/D8 2-arg form) + transitive `is_unsafe_cell` flag on `Item` (the flag drops `const` when the referent transitively contains `UnsafeCell`; D1.1 + D1.5 + D7/D8 LOCKED).
- `const` on `Ty::Ptr` → `Mutability` on `Ty::Ptr(TyId, Mutability)` only (`*const T` vs `*mut T`; locked D7/D8 renames `RawPtr` -> `Ptr`); the `is_unsafe_cell` flag does *not* apply to raw pointers per locked D1.5 (D1.1 + D7/D8 LOCKED).
- `static`    → `Item` decl (linkage / storage class; stage-12 territory).
- `restrict`  → `Provenance::UniqueBorrow` on `Ty::Ref` (D1.1 + D7/D8 LOCKED).
- `volatile`  → intrinsic `InstanceId` identity (D9, this entry).

Locating volatile on the intrinsic `InstanceId` follows the
same shape as the other three. Adopting Option 1 (CompCert
per-type) would be a one-off departure from the implicit
pattern. Option 2 puts a flag on a generic `Statement` that
no driver code in the closed DSL surface ever produces (raw
`*_volatile` is rejected; MMIO is intrinsic-only), so the
flag would be dead weight.

CompCert is the lone per-type prior because its trace
semantics must observe volatile operations as part of the
verified semantics; we emit C and have no trace-semantics
requirement. Among surveyed projects with explicit volatile
evidence: LLVM, mrustc, rustc_codegen_clr, and rustc-codegen-c
all keep volatile per-op or off the type entirely; MLIR's
general "special semantics off type" pattern is consistent;
Charon's Theme D does not specifically address volatile (its
no-volatile-marker-in-TyKind state is suggestive but not
load-bearing). Rustc itself routes volatile through intrinsics.

The DSL three-mechanism convention
(`../../../regseq-background.md` lines 14-27: Intrinsics
operational / Properties declarative / Traits integration)
places MMIO unambiguously in intrinsic territory. The pl011
fixture's `mmio.rs` (lines 35-71) confirms intrinsic-only
usage in practice -- no `core::ptr::read_volatile` call
sites. Q7 already locks the closed-intrinsic-vocabulary
mechanism. Option 3 is the fixed point of those existing
commitments.

The bridge-level rejection of raw
`core::ptr::{read,write}_volatile` is the analog of D1.4
Category A applied to function calls instead of types: rigid
Rust constructs that exist in `core` but lie outside the
regseq DSL surface are rejected with a clear diagnostic, not
handled defensively. The DSL surface rule ("Regseq DSL uses a
limited subset of `core`. No other crates.") plus the
methodology rule "no defensive fallbacks" jointly imply this
posture.

**Cross-refs.** `vocabulary-SESSION-C.md` S0 (no-QualType-layer
framing principle), S1 (full audit-shape decision body), S1.1
(worked example: pl011-style `mmio_read` invocation);
`../../vocabulary.md` Section 1.6 (D1.5 `is_unsafe_cell` flag --
the existing `const`-handling precedent), Section 4.1
(reference / raw-pointer worked C lowerings showing
`const`/`restrict` projection), Section 4.5 (UnsafeCell
aliasing example);
`../../../regseq-background.md` lines 14-27 (DSL
three-mechanism convention);
`tests/fixtures/pl011-full-spec/src/regseq/mmio.rs:35-71`
(pl011 intrinsic usage);
`../../../stage-1/vocabulary.md` lines 869-899 (intrinsic call
representation); `../../../architecture.md` Q7 (intrinsic
recognition mechanism); `../../../stage-13/expectations.md`
lines 12-17 and 50-58 (currently records the closed-vocabulary
recognition mechanism per Q7; the MMIO-specific intrinsic
vocabulary entries and Call-translation rejection enforcement
of D9 are outbound forward refs the merge session should land
into stage-13 expectations when SESSION-C is integrated);
`../../prior-art/llvm-ir.md` Theme D (lines 311-336);
`../../prior-art/mrustc.md` Theme D (lines 516-518, 520-536);
`../../prior-art/charon.md` Theme D (lines 609 and 664; D9 scoped
to atomic / marker types only -- volatile not specifically
addressed in Charon's Theme D);
`../../prior-art/rustc-codegen-clr.md` Theme D (lines 328-382);
`../../prior-art/rustc-codegen-c.md` Theme D (lines 224-242);
`../../prior-art/mlir.md` Theme D (lines 365-405);
`../../prior-art/compcert.md` Theme D (lines 415-434 covering
volatile, citing Ctypes.v lines 100-107 and CompCert manual
lines 729-746);
`../../findings-rustc-draft.md` lines 570-572 (rustc's
intrinsic-level volatile).

### D10 -- Atomics: regseq-DSL `Atomic<T>` with intrinsic operations; no `Ty` bit [LOCKED]

**Question.** Three sub-questions, sharpened by the S0 framing
principle established in D9:

1. *Substantive lock* -- does `Ty` carry an atomic bit? Are
   atomic types (`AtomicU32`, `AtomicBool`, ...) flagged on
   `Item` or carved out as a separate `Ty` variant?
2. *Ingest policy* -- analog of D9's bridge-side rejection of
   `core::ptr::{read,write}_volatile`. Should the bridge admit
   the `core::sync::atomic` operational API (the
   `Atomic*` types and their methods, plus the rustc-internal
   `core::intrinsics::atomic_*` family that `core::sync::atomic`
   inlines into), reject it (forcing a DSL-defined
   `Atomic<T>`), or admit both?
3. *Memory ordering* -- does ordering need any stage-3
   representation, or is it entirely operation/InstanceId
   metadata (stage-13 territory)?

**Key options (Q2, the genuine choice).** (Each option below
is the *broad alternative* that was on the table; the
locked decision narrows the rejection scope structurally as
detailed in the Decision row.)

1. *Reject the `core::sync::atomic` operational API; provide
   regseq-DSL `Atomic<T>` with `#[rseq::intrinsic("atomic_*")]`
   operations* (D9-symmetric; single DSL surface for
   operational verbs).
2. *Admit the `core::sync::atomic` operational API*; bridge
   recognises the rustc-internal `core::intrinsics::atomic_*`
   calls inside their MIR and translates them to stage-13
   atomic-intrinsic InstanceIds.
3. *Admit both* (`core::sync::atomic` operational API *and*
   `#[rseq::intrinsic("atomic_*")]` accepted); duplicate
   surface for the same semantics.

**Decision.** Option 1, locked with structural decomposition:
the bridge rejects (a) the `core::sync::atomic::Atomic*` type
family at type translation (`AtomicU8`, `AtomicI8`,
`AtomicU16`, `AtomicI16`, `AtomicU32`, `AtomicI32`,
`AtomicU64`, `AtomicI64`, `AtomicU128`, `AtomicI128` -- the
128-bit variants are cfg-gated on
`target_has_atomic_load_store = "128"` per
`library/core/src/sync/atomic.rs:3884-3920` in the pinned
toolchain; rejection scope is the family, not gated by the
cfg --, `AtomicUsize`, `AtomicIsize`, `AtomicBool`,
`AtomicPtr<T>`, plus the unstable generic alias
`core::sync::atomic::Atomic<T>` -- declared with
`#[unstable(feature = "generic_atomic", issue = "130539")]`
at `library/core/src/sync/atomic.rs:340-345` in the pinned
toolchain; rejection scope is the family, not conditional on
the unstable feature gate (the alias is in scope even
though source-side use of it requires
`#![feature(generic_atomic)]`); the core alias is distinct
from the regseq-DSL `regseq::Atomic<T>` facade);
(b) calls to methods on those
types and to the rustc-internal `core::intrinsics::atomic_*`
family at Call translation; with (c)
`core::sync::atomic::Ordering` (the value-level enum)
explicitly *not* in the rejection scope. The regseq DSL
provides `Atomic<T>` with `#[rseq::intrinsic("atomic_*")]`
operations as the canonical surface. The `core::sync::atomic`
module's free functions for memory barriers and compiler
fences (notably `fence` and `compiler_fence`) are out of
D10's scope -- they are not part of the regseq Atomic
construct (which is value-keyed; see
`docs/compiler-collateral/constructs/atomic-memory-access-and-ordering.md`
for the construct-level exclusion of barrier/fence elements
from Atomic) and their positive classification (reject /
admit via a barrier-specific intrinsic / something else)
is deferred to a future construct-level decision. D10 adds
no accepted intrinsic-vocabulary entry for these; what
happens at the bridge for source-level use is governed by
a separate mechanism (future construct decision or
stage-13 unsupported-call policy), not by Q7 -- Q7's
recognition mechanism applies to
`#[rseq::intrinsic("NAME")]`-attributed callees, not to
raw `core::*` free-function calls.
Q1 resolves to ordinary `Ty::Struct` (no `is_atomic` flag, no
`Ty::Atomic<T>` variant) -- the existing `is_unsafe_cell` flag
(D1.5) on the inner `UnsafeCell<T>` field handles
`const`-dropping. Q3 resolves to no stage-3 representation --
ordering survives in the IR at the operation/InstanceId level,
where stage 13 picks the encoding (per-(op × ordering)
InstanceIds vs const-generic ordering on a single InstanceId per
op).

**Reasoning.**

The substantive lock (Q1) is a direct application of S0: atomic
types have the same shape as any other `UnsafeCell`-bearing
struct, and the `is_unsafe_cell` flag already produces the
correct C lowering (drop `const` on enclosing shared
references). No atomic-specific machinery is needed at the type
level; adding one would inflate the variant set or the flag set
without semantic gain.

The ingest policy (Q2) was the genuine choice. The Linux/Zephyr
research surfaced that atomic-operation ordering must survive
into stage-13 codegen for both targets to produce idiomatic
output, which initially recommended admitting `core::sync::atomic`
(Option 2) for ergonomic reasons. On reflection, Option 1
dominates on five architectural strands:

- *Architectural symmetry with D9.* D9 rejected raw
  `core::ptr::{read,write}_volatile` and required
  `#[rseq::intrinsic("mmio_*")]`. Admitting `core::sync::atomic`
  would have given the DSL two different ingest patterns for
  what are structurally the same thing (operational verbs).
  Option 1 collapses both to one rule: operational primitives
  are DSL intrinsics; nothing operational comes from `core::*`
  directly.
- *DSL surface coherence.* Regseq drivers are DSL-encoded
  device specs, not generic Rust programs. Using
  `regseq::Atomic<T>` instead of `core::sync::atomic::AtomicU32`
  is a one-import-line change, not an ergonomic loss.
- *DSL controls vocabulary instead of inheriting it.* With
  `regseq::Atomic<T>`, the DSL pins exactly which atomic
  operations exist; rustc evolution is decoupled from DSL
  surface. Admitting `core::sync::atomic` would have left the
  DSL at the mercy of stdlib changes.
- *Single bridge recognition mechanism.* Q7's
  `#[rseq::intrinsic("NAME")]` attribute remains the only
  admission path. Admitting `core::sync::atomic` would have
  required a parallel path-based recognizer for
  `core::intrinsics::atomic_*` (rustc-internal, no
  `#[rseq::intrinsic]` attribute). Cleaner architecture without
  it.
- *Per-target codegen latitude is unaffected.* The
  Linux/Zephyr divergence -- Linux exposes
  `_relaxed`/`_acquire`/`_release` suffixes on `atomic_*`
  (covering Relaxed/Acquire/Release directly) plus an
  unsuffixed fully-ordered form (covering SeqCst, and used
  conservatively for AcqRel since Linux has no AcqRel
  suffix); Zephyr's public API is SeqCst-only and codegen
  must use GCC `__atomic_*` builtins to preserve non-SeqCst
  orderings -- is handled the same way regardless: stage-13
  codegen for the regseq atomic-intrinsic InstanceIds picks
  per target, preserving Rust ordering where the target
  supports it directly and conservatively strengthening
  otherwise. The IR carries ordering at the operation /
  InstanceId level either way.

Memory ordering (Q3) is operation-level metadata that survives
in the IR at the operation/InstanceId level (either as an
operand on the `Call` to a single `atomic_<op>(_, order)`
intrinsic, or via per-(op × ordering) `InstanceId` identity --
the encoding choice is stage-13 territory). Stage 3 records
that no `Ty`-level representation is needed; both encoding
shapes keep ordering off the type.

The Linux/Zephyr C-side evidence further reinforces S0: both
v1 codegen targets reject C11 `_Atomic` by convention. Linux
uses `atomic_t`/`atomic64_t`/`atomic_long_t`/`refcount_t`
(Documentation/atomic_t.txt; Linus 2017 LKML position) and
exposes four ordering surfaces via suffix:
`_relaxed`/`_acquire`/`_release` plus an unsuffixed
fully-ordered form (covering Relaxed/Acquire/Release directly
and SeqCst via the unsuffixed form; AcqRel must be
conservatively strengthened to unsuffixed since Linux has no
dedicated AcqRel suffix). Zephyr uses `atomic_t` (long)
with a SeqCst-only public API backed by GCC `__atomic_*`
builtins (`include/zephyr/sys/atomic_builtin.h`). Neither
target can or should consume a type-level atomic marker --
both keep atomicity at the operation/function level. This is
hard external corroboration of the operation-level posture.

**Cross-refs.** `vocabulary-SESSION-C.md` S0 (no-QualType-layer
framing principle), S1 (D9 -- the architecturally-symmetric
volatile decision), S2 (full audit-shape decision body), S2.1
(worked example: regseq `Atomic<u32>` with acquire-load and
target-specific Linux/Zephyr lowerings);
`../../vocabulary.md` Section 1.6 (D1.5 `is_unsafe_cell` flag --
the existing UnsafeCell-handling precedent);
`../../../regseq-background.md` lines 14-27 (DSL three-mechanism
convention);
`tests/fixtures/pl011-full-spec/src/regseq/mmio.rs:35-71`
(pl011 facade pattern that the regseq `Atomic<T>` shape
mirrors); `../../../stage-1/vocabulary.md` lines 869-899
(intrinsic call representation); `../../../architecture.md` Q7
(intrinsic recognition mechanism); `../../../stage-13/expectations.md`
lines 12-17 and 50-58 (currently records the closed-vocabulary
recognition mechanism per Q7; the atomic-specific intrinsic
vocabulary entries, the ordering encoding choice, and D10's
bridge-side rejection enforcement (type-translation rejection
of the `core::sync::atomic::Atomic*` type family;
Call-translation rejection of methods on those types and of
`core::intrinsics::atomic_*`; with `core::sync::atomic::Ordering`
explicitly *not* in the rejection scope; and
`core::sync::atomic::{fence, compiler_fence}` and related
barrier/fence free functions explicitly *out of D10's scope* --
not atomic intrinsic vocabulary entries; positive classification
deferred to a future construct-level decision per
`docs/compiler-collateral/constructs/atomic-memory-access-and-ordering.md`)
are outbound forward refs the merge session should land into
stage-13 expectations when SESSION-C is integrated);
`../../prior-art/llvm-ir.md` Theme D (lines 311-336);
`../../prior-art/mrustc.md` Theme D (lines 520-536);
`../../prior-art/charon.md` Theme D (lines 609 and 664);
`../../prior-art/rustc-codegen-clr.md` Theme D (lines 328-382);
`../../prior-art/rustc-codegen-c.md` Theme D (lines 224-242);
`../../prior-art/mlir.md` Theme D (lines 365-405; general
"special semantics off type" pattern -- no atomic-specific
evidence in this file);
`../../prior-art/compcert.md` Theme D (lines 436-451 covering
atomic / `_Atomic` context; volatile-specific lines are 415-434,
separate);
`../../findings-rustc-draft.md` Theme D (rustc's intrinsic-
level atomics; library/core/src/sync/atomic.rs for atomic
type shapes);
external: Linux `Documentation/atomic_t.txt`,
`include/linux/atomic.h` and friends; Zephyr
`include/zephyr/sys/atomic.h`,
`include/zephyr/sys/atomic_builtin.h`,
`include/zephyr/sys/atomic_types.h`.

### D13 -- Discriminant level: per-variant resolved tag value on `VariantLayout` [LOCKED]

**Question.** The shared `../../discussions.md` D13 entry
asks: "Preserve typeck-level discriminant type (the
`repr(<int>)` choice) AND layout-level tag (the in-memory
bytes), or only one?" with lean "layout-level tag only --
the typeck-level discriminant is recoverable from the
layout's tag scalar type."

That framing collapses two distinct things called
"discriminant":

1. *Discriminant **type*** -- the `repr(<int>)` choice or
   rustc's default tag-size selection (e.g., `u32` for
   `repr(u32) enum E { ... }`).
2. *Per-variant discriminant **values*** -- the concrete tag
   bytes each variant maps to (e.g., `0`, `1`, `0xFF`,
   `0xDEAD` for a `repr(u32) enum Status { Idle = 0,
   Active = 1, Error = 0xFF, Reset = 0xDEAD }`).

Each needs a separate answer. The shared lean's "recoverable
from the layout's tag scalar type" claim only applies to (1).

**Key options for (2) -- per-variant discriminant values.**

1. *On `VariantLayout`* (two fields) -- add
   `discriminant: u128` (always present; semantic discriminant
   from `adt_def.discriminants(tcx)`) and
   `tag_value: Option<u128>` (layout-level tag bytes for
   `SetDiscriminant`; `None` when no tag write occurs).
2. *On `EnumShape::Multiple` as parallel `Vec<u128>` and
   `Vec<Option<u128>>`* -- same information, separate
   collections.
3. *On the enum's `Item` decl* (stage-12 territory) --
   typeck-level lookup at codegen time.
4. *Recover from variant index + tag encoding* (the original
   shared-spec lean) -- fails for user-assigned `repr(<int>)`
   where variant index ≠ discriminant value.

**Decision.**

- (1) -- *Discriminant type*: settled by **D11**. The
  `TagScalar.scalar_ty: TyId` field on `EnumShape::Multiple`
  carries the resolved tag scalar (`Ty::Int(_)` /
  `Ty::Uint(_)` / `Ty::Ptr(_, _)` / `Ty::Ref(_, _)` per
  locked D7/D8),
  which encodes everything `repr(<int>)` controls. D11's
  schema-mapping table at `../../vocabulary.md` Section 2.2
  drops `ReprOptions.int` because the resolved scalar is
  authoritative. D13 carries this lock forward without
  modification.
- (2) -- *Per-variant discriminant values*: **Option 1**.
  `VariantLayout` is refined with two fields:
  - `discriminant: u128` -- the Rust-level semantic
    discriminant (what `Discriminant(place)` returns; from
    `adt_def.discriminants(tcx)`'s `Discr.val: u128`). Always
    present for any `VariantLayout` in any `EnumShape::Single`
    or `EnumShape::Multiple`. `EnumShape::Empty` carries no
    `VariantLayout`, so the field does not apply there.
  - `tag_value: Option<u128>` -- the layout-level tag bytes
    for `SetDiscriminant`:
    * `Some(v)` for direct-encoded variants
      (`v == discriminant`).
    * `Some(v)` for niche-encoded niched variants
      (`v = niche_start + (variant_idx - niche_variants.start)`,
      resolved per D12's formula).
    * `None` for niche-encoded untagged variants (the inner
      data's bit pattern IS the tag; no separate write).
    * `None` for `EnumShape::Single` variants (no tag scalar
      exists in the layout; no tag write).

This refines D11's locked Section-2.2 schema by *two* fields;
the merge session lands both additions into the shared
`../../vocabulary.md` D11 declaration when integrating
SESSION-C.

**Reasoning.**

For `Direct` tag encoding with user-assigned discriminants
(common in driver code -- status codes, error codes,
register values), variant index and discriminant value are
*independent*. Codegen for `SetDiscriminant(Status::Error)`
on `repr(u32) enum Status { Idle = 0, Active = 1, Error =
0xFF, Reset = 0xDEAD }` must write `0xFF` to the tag
location, not `2` (the variant index). Without
per-variant tag-value storage, codegen has no source for
`0xFF` -- the value lives only in rustc's typeck-level
`adt_def.discriminants(tcx)`, which is post-bridge
unavailable. The shared D13 lean
("recoverable from variant index + tag encoding") fails
this case.

The Option 1 placement (on `VariantLayout`, two fields) is
selected over the alternatives because:

- *Methodology -- "store the resolved result, not the
  input."* Per-variant discriminants are a resolved-by-rustc
  decision (auto-incrementing where omitted, signed/unsigned
  conversion, constant folding all done by rustc); we copy
  the answer.
- *Two senses, two fields.* The Rust-level semantic
  discriminant (returned by `Discriminant(place)`, always
  present per `adt_def.discriminants(tcx)`) and the
  layout-level tag bytes (written by `SetDiscriminant`,
  absent for niche-untagged variants and Single-variant
  enums) diverge for niche-encoded enums; conflating them
  into a single field cannot honestly represent both.
- *Cohesion.* Discriminant data is intrinsic to the variant's
  layout (alongside its fields' offsets and the `uninhabited`
  flag). Splitting into parallel collections (Option 2) or
  onto the `Item` decl (Option 3) fragments the layout-level
  data without semantic gain.
- *Stage-3 / stage-12 boundary respected.* Per-variant
  discriminant data is layout-level (resolved tag bytes plus
  resolved Rust-level semantic discriminant, not AST-level
  user-source `= EXPR` clauses). It belongs with the rest of
  the layout-level facts.
- *Niche-encoded enums fit the two-field schema cleanly.*
  `discriminant: u128` is always present (rustc gives every
  variant one); `tag_value: Option<u128>` distinguishes
  "tag write occurs" (Some) from "no tag write" (None,
  for niche-untagged and `EnumShape::Single`).
- *`u128` width* matches rustc's resolved `Discr.val: u128`
  (`rustc_middle/src/ty/util.rs:36-40`); sufficient for any
  ≤128-bit tag scalar; the bit pattern is unsigned at
  storage with signed interpretation supplied by
  `TagScalar.scalar_ty` (D11) at codegen time.

Modest redundancy with D12: for niche-encoded enums, both
`niche_variants`/`niche_start` (D12) *and* per-variant
`tag_value` are stored. They are not strictly redundant:

- `niche_variants`/`niche_start` (D12) tell codegen *where*
  to write the tag (into the niched field, not a separate
  tag location) and *how* the variant index maps to the bit
  pattern via the formula.
- `tag_value: Some(v)` (D13) tells codegen *what value*
  to write, baked in for direct lookup; `discriminant: u128`
  (D13) tells codegen *what semantic discriminant to
  produce* in `Discriminant(place)` lowering.

Each is load-bearing for a different `SetDiscriminant`
codegen aspect on niche-encoded enums.

**Cross-refs.** `vocabulary-SESSION-C.md` S3 (full audit
shape; the refined `VariantLayout` schema; the
discriminant-type-vs-discriminant-values disambiguation),
S3.1 (worked example: `repr(u32)` enum with user-assigned
tags, Direct encoding), S3.2 (worked example:
`Option<&u32>` niche-encoded with the untagged-variant
`None` case); S0 (this is a layout-level qualifier-style
question; orthogonal to S0's "no QualType layer" framing
but consistent with its methodology); merge note item 4
(extends D11's locked schema; merge session lands the
addition into `../../vocabulary.md` Section 2.2);
`../../vocabulary.md` Section 2.1 (D2 -- layout position),
Section 2.2 (D11 -- layout content subset; the schema D13
extends), Section 4.4 (existing niche-encoded `Option<&u32>`
worked example; D13's S3.2 builds on this);
`../../findings-rustc-draft.md` Theme B.4 (TagEncoding
covers Direct/Niche encoding shape, not per-variant
values), Theme B.6 (Scalar / valid_range), Theme W.8
(typeck vs layout for discriminant *type*);
`rustc_middle/src/ty/adt.rs:159` (`VariantDef.discr`,
typeck-level per-variant value);
`rustc_abi/src/lib.rs:1858-1880` (`Variants::Multiple`
schema; per-variant LayoutData but not per-variant
discriminant values directly);
`../../prior-art/charon.md` Theme B lines 397-512 (Charon's
per-variant `VariantLayout.tag: Option<ScalarValue>` and
typeck-level `Variant.discriminant`; direct precedent for
Option 1);
`../../prior-art/mrustc.md` Theme B lines 295-319
(discriminant scheme on layout; typeck-level
`HIR::Enum.m_tag_repr`);
`../../prior-art/rustc-codegen-clr.md` Theme B lines
202-212 (CIL offset-collision approach; no per-variant
discriminant on the layout).

### D16 -- `c_void` recognition: rejected at bridge in v1 [LOCKED]

**Question.** The shared `../../discussions.md` D16 entry
asks: "Recognize `core::ffi::c_void` as a flagged lang-item,
or treat as ordinary Adt?" with lean "ordinary Adt (no flag)
for v1; recognition can be added later if intrinsic
signatures using `*mut c_void` motivate it (via
`tcx.is_lang_item(LangItem::CVoid)` -- not in `AdtFlags`)."

That framing addresses the *flagging* question (no flag for
v1 -- correct, and already locked by D1.5) but does not
address the bridge's translation behaviour when it
encounters `c_void` in driver code today. The substantive
sub-question for SESSION-C: in v1, with no consumer for
flag-driven c_void recognition, does the bridge (a)
translate `c_void` as an ordinary `Ty::Enum` (with the
pinned-toolchain shape: 2-variant `repr(u8)` per
`library/core/src/ffi/mod.rs:49-76`, where the two hidden
variants `__variant1` / `__variant2` make the type
inhabited rather than zero-variant uninhabited) and let
downstream codegen handle the structurally-faithful but
FFI-wrong lowering, or (b) reject `c_void` at the bridge
(D9 / D10 / D1.4 rejection-pattern symmetry) until a
consumer materialises and the flag is added?

**Key options.**

1. *α -- Translate as ordinary `Ty::Enum`* (the pinned-
   toolchain 2-variant `repr(u8)` shape; no bridge rejection;
   codegen produces a structurally-faithful 1-byte struct
   that does not match the FFI-canonical `void *` shape
   c_void's lang-item identity implies).
2. *β -- Reject `c_void` at the bridge in v1*
   (D1.4-style; diagnostic recommends opaque-struct pattern;
   no `is_c_void` flag added until D16 reopens).
3. *γ -- Translate as ordinary, plus a stage-13 forward-ref*
   noting that the structurally-faithful 2-variant `repr(u8)`
   c_void lowering does not match the FFI-canonical `void *`
   shape and is incomplete in v1 (α with extra paperwork).

**Decision.** Option β. The bridge rejects
`core::ffi::c_void` at type translation in v1, with a
diagnostic recommending the opaque-struct pattern as the
canonical alternative. D1.5's "no flag" lock carries forward
unchanged; D16 hardens the deferral into active rejection
pending a future consumer.

The recognition mechanism is `tcx.is_lang_item(did,
LangItem::CVoid)` per D1.5's locked recognition-path note.
Rejection scope is the c_void type itself; pointers to
c_void (`*mut c_void`, `*const c_void`) reject transitively
when the inner type is encountered during pointer
translation. Trigger conditions for re-opening: a v1 driver
fixture or stage-13 intrinsic surfaces a stable need for
FFI-canonical pointer-to-c_void lowering -- specifically
`*mut c_void` -> C `void *` and/or `*const c_void` -> C
`const void *` (the two pointer forms have distinct C
spellings per locked D7/D8's `Ty::Ptr(TyId, Mutability)`
and S0's raw-pointer `const`-derivation rule; the future
D16 reopens with both spellings on the table). When that
materialises, D16 reopens to add `is_c_void` flag on `Item`
and lift the rejection.

**Reasoning.**

Three architectural strands carry the decision:

1. *No v1 consumer.* The evidence sweep confirms zero v1
   driver fixtures use `core::ffi::c_void`, and zero planned
   stage-13 intrinsic signatures mention it. Per the
   methodology rule "ask who consumes it; what shape best
   serves them," there is no v1 consumer to serve.

2. *Symmetric posture with D9 / D10 / D1.4.* All three
   already-locked rejections share the same structure: a
   stable `core::*` API or feature is rejected at the
   bridge because it lies outside the regseq DSL's
   operational surface, with a diagnostic pointing to the
   canonical alternative. D16 fits the same pattern; the
   diagnostic recommends the opaque-struct pattern (the
   same alternative D1.4 names for opaque C-side handles --
   there is no D9/D10-style DSL-defined replacement because
   c_void's role is structural, not operational).

3. *The non-rejection alternative does not earn its weight.*
   Translating `c_void` as an ordinary `Ty::Enum` -- per the
   pinned-toolchain shape: 2-variant `repr(u8)` enum
   (`__variant1`, `__variant2`,
   `library/core/src/ffi/mod.rs:49-76`) with
   `EnumShape::Multiple { tag: u8, encoding: Direct,
   variants: [...] }` -- produces a structurally-faithful
   1-byte tagged struct in C that is *not* the FFI-canonical
   `void *` shape c_void's lang-item identity entails.
   Stage-13 codegen would have to commit to either the
   structurally-faithful but FFI-wrong 1-byte struct
   lowering or perform lang-item recognition the bridge is
   not currently doing. A future consumer surfacing FFI
   compatibility would then either receive the wrong shape
   or require the bridge to retroactively recognise c_void.
   β avoids both v1 lowering commitments.

The decision hardens D1.5's "recognition deferred until
intrinsic signatures using `*mut c_void` motivate it" into
"actively rejected pending recognition" -- the only honest
interpretation given that an unrecognised `c_void` cannot
be lowered correctly without a recognition-driven C-lowering
decision.

**Cross-refs.** `vocabulary-SESSION-C.md` S4 (full audit
shape; the bridge rejection mechanism; trigger conditions
for reopening), S4.1 (worked example: rejected
`*mut c_void` parameter, suggested rewrite using opaque-
struct pattern); merge note item 2 (D16 rejection added to
the outbound-forward-ref list to stage-13/expectations.md);
S5 D16 row;
`../../vocabulary.md` Section 1.5 (D1.4 -- the
`Foreign(_)` rejection precedent that D16 mirrors), Section
1.6 (D1.5 -- the "no flag for v1; recognition deferred"
locked phrasing that D16 builds on; the
`tcx.is_lang_item(LangItem::CVoid)` recognition path is
locked there at lines 402-406), Section 1.7 (D1.6.c -- the
`Never` vs c_void distinction that forwarded the
substantive lock to D16);
`../../findings-rustc-draft.md` Theme D.2 lines 546-564
(c_void declared as Target::Enum at
`rustc_hir/src/lang_items.rs:279`; recognition via
`tcx.is_lang_item(did, LangItem::CVoid)`; not in
`AdtFlags`);
external Rust source: `library/core/src/ffi/mod.rs:49-76`
(the c_void declaration -- `#[lang = "c_void"]` attribute,
with two hidden `__variant1` / `__variant2` variants that
make the type inhabited rather than zero-variant);
`../../prior-art/charon.md` Theme D lines 609 and 664-678
(Charon catalogues which marker types -- `UnsafeCell`,
`MaybeUninit`, `PhantomData`, atomic types, `NonNull`,
`Cell`, `RefCell`, `Pin`, `ManuallyDrop` -- are treated as
ordinary ADTs without type-level markers; `c_void` is not
specifically named, so the claim that Charon admits c_void
the same way is *inference* from Charon's general
`TyKind::Adt(TypeDeclRef)` model with limited builtins,
not direct evidence -- the alpha-style admittance posture,
which D16 declines for the v1 codegen reasons above).

---

## 2. Cross-references

- Per-decision spec section (this session's): `vocabulary-SESSION-C.md`.
- Per-round Codex review (this session's): `review-log-SESSION-C.md`.
- Shared decision discussions (read-only during SESSION-C): `../../discussions.md`.
- Shared spec (read-only during SESSION-C): `../../vocabulary.md`.
- Shared review log (read-only during SESSION-C): `../../review-log.md`.
- Inbound forward refs from stage 1: `../../expectations.md`.
- Primary-source rustc evidence: `../../findings-rustc-draft.md`.
- Per-project deep prior-art research: `../../prior-art/<project>.md`.
