# Stage 3 SESSION-C -- Type System Vocabulary (addendum)

This file is a session-local addendum to `../../vocabulary.md`. It
holds the canonical-spec entries for the four decisions owned by
SESSION-C, written in the **full audit shape** used by the locked
entries in the shared spec so they can be lifted directly into
`../../vocabulary.md` at merge time:

1. The decision statement.
2. Reasoning -- why this and not the alternatives.
3. Methodology rules invoked (cross-references to `../../../context.md`
   "Methodology rules").
4. Prior-art evidence (cross-references to `../../prior-art/<project>.md`
   files and `../../findings-rustc-draft.md`).
5. Worked example(s) showing the C lowering each decision drives
   (where applicable).

Decisions in scope:

- **D9** -- Volatile semantics (per-op vs per-type) -- LOCKED
- **D10** -- Atomics (operation-level vs type-level) -- LOCKED
- **D13** -- Discriminant level (typeck vs layout; per-variant values) -- LOCKED
- **D16** -- `c_void` recognition (rejected at bridge in v1) -- LOCKED

Section numbering uses placeholder `S<x>.<y>` headings so the merge
session can renumber into the shared spec without ambiguity.

**Merge notes.**

1. *S0 placement.* Section S0 below articulates a framing
   principle ("no QualType layer") that emerged during D9's
   discussion. It applies across stage 3 (and beyond), not just
   to volatile. The merge session should decide whether to land
   it as a standalone framing subsection at the top of
   `../../vocabulary.md` Section 1 or to fold it into D9's
   Reasoning. Either placement is correct; the principle stands
   on its own.

2. *Outbound forward refs to stage 13.* SESSION-C's D9, D10,
   and D16 commit to bridge-side rejections and to stage-13-
   owned intrinsic vocabulary entries:
   - D9 rejection (Call-translation only): raw
     `core::ptr::read_volatile` and `core::ptr::write_volatile`
     calls.
   - D10 rejections, decomposed by translation phase:
     *type-translation* rejection of the
     `core::sync::atomic::Atomic*` type family (`AtomicU32`,
     `AtomicBool`, `AtomicI64`, `AtomicPtr<T>`, ...);
     *Call-translation* rejection of methods on those types
     (`.load`, `.store`, `.fetch_add`, `.compare_exchange`,
     ...) and of the rustc-internal `core::intrinsics::atomic_*`
     family. `core::sync::atomic::Ordering` (the value-level
     enum) is explicitly *not* in the rejection scope.
   - D16 rejection (type-translation only): the
     `core::ffi::c_void` lang-item ADT itself (recognised via
     `tcx.is_lang_item(did, LangItem::CVoid)` per D1.5).
     Rejection applies to the type, which transitively
     rejects pointers to it (`*mut c_void` /
     `*const c_void`); diagnostic recommends the opaque-
     struct pattern (`#[repr(C)] struct Opaque { _private:
     [u8; 0] }`) per D1.4's existing precedent for opaque
     C-side handles.
   - Stage-13 vocabulary entries owed to SESSION-C: the MMIO
     `mmio_*` intrinsic family (D9); the atomic-operation
     intrinsic family plus the ordering encoding choice (D10).
   `../../../stage-13/expectations.md` currently records the
   general intrinsic-vocabulary mechanism (per Q7) but does
   not yet enumerate these specific commitments. When merging
   SESSION-C, the merge session should append outbound
   forward-ref entries to `stage-13/expectations.md` recording:
   (a) the MMIO intrinsic vocabulary entries owed to D9; (b)
   the atomic intrinsic vocabulary entries owed to D10;
   (c) the ordering encoding choice for atomics; and (d) the
   bridge-side rejection enforcement -- specifically D9's
   Call-translation rejection of `core::ptr::read_volatile`
   and `core::ptr::write_volatile`, plus D10's two-phase
   rejection (type-translation rejection of the
   `core::sync::atomic::Atomic*` type family;
   Call-translation rejection of methods on those types and
   of the rustc-internal `core::intrinsics::atomic_*` family;
   with `core::sync::atomic::Ordering` explicitly *not* in
   the rejection scope; and `core::sync::atomic::{fence,
   compiler_fence}` and related barrier/fence free functions
   explicitly *out of D10's scope* -- they are not atomic
   intrinsic vocabulary entries, and their positive
   classification is deferred to a future construct-level
   decision per
   `docs/compiler-collateral/constructs/atomic-memory-access-and-ordering.md`,
   plus D16's type-translation rejection of
   `core::ffi::c_void` -- recognised via
   `tcx.is_lang_item(did, LangItem::CVoid)` per D1.5;
   diagnostic recommends the opaque-struct pattern per D1.4
   precedent).

3. *Note on S0 alignment with D7/D8.* Section S0's
   qualifier-to-IR-site table cites locked D7/D8 (Provenance
   enum on `Ty::Ref`; `Ty::Ptr` shape per the shared
   `../../discussions.md` Section 1 D7/D8 entry, lines 472+,
   and `../../vocabulary.md` Section 1.8) as the derivation
   path for `const` and `restrict`. D7/D8 was locked in the
   shared spec during SESSION-C's iteration on D9 + D10; the
   S0 table now reflects the locked names directly
   (`Ty::Ref(TyId, Provenance)` 2-arg form, `Ty::Ptr(TyId,
   Mutability)` -- renamed from `Ty::RawPtr`). No re-wording
   pending; the table is current.

4. *D13 extends D11's locked Layout schema.* D13 adds two
   fields to `VariantLayout` (Section 2.2 of the shared
   `../../vocabulary.md`) to carry per-variant discriminant
   information that D11 inadvertently omitted:
   `discriminant: u128` (the Rust-level semantic discriminant
   value, i.e. what `Discriminant(place)` returns; from
   `adt_def.discriminants(tcx)`) and
   `tag_value: Option<u128>` (the layout-level tag bytes
   `SetDiscriminant` writes; `None` when no tag write
   occurs -- niche-untagged variants and `EnumShape::Single`
   variants). The merge session should land both fields
   into the locked D11 schema in `../../vocabulary.md` as
   part of integrating SESSION-C. See Section S3 for the
   full audit shape, the schema change, the rationale, and
   worked
   examples.

---

## S0. Framing principle -- no QualType layer

Stage 3's locked decisions, taken together, commit to a meta-
pattern that has not yet been named explicitly: **regseq-model's
`Ty` represents the unqualified Rust type; C-side qualifiers
(`const`, `volatile`, `restrict`, and similar) are emitted at
codegen time from per-decl / per-op information accumulated
elsewhere in the IR**. There is no Clang-style `QualType`
wrapper that pairs an unqualified type with a `QualifierFlags`
bitfield, and no `Ty::ConstQualified(TyId)` / `Ty::Volatile<T>`
variant.

Each C qualifier already maps to a distinct, semantically-natural
IR site:

| C qualifier | Where it's derived from in our IR | Source / status |
| --- | --- | --- |
| `const` on a reference (`Ty::Ref`) | `Provenance` on `Ty::Ref(TyId, Provenance)` (locked D7/D8: 2-arg form, Provenance subsumes Mutability for refs) plus the transitive `is_unsafe_cell` flag on `Item` (the flag drops `const` when the referent transitively contains `UnsafeCell`) | D1.1 LOCKED + D1.5 LOCKED + D7/D8 LOCKED (`../../discussions.md` Section 1 D7/D8 entry; `../../vocabulary.md` Section 1.8) |
| `const` on a raw pointer (`Ty::Ptr`) | `Mutability` on `Ty::Ptr(TyId, Mutability)` only (`*const T` vs `*mut T`; locked D7/D8 renames `RawPtr` -> `Ptr`); the `is_unsafe_cell` flag does *not* apply to raw pointers per locked D1.5 | D1.1 LOCKED + D7/D8 LOCKED |
| `static` (storage class / linkage) | `Item` decl attributes (stage-12 territory: name mangling and linkage) | stage-1 Item vocabulary LOCKED |
| `restrict` | `Provenance::UniqueBorrow` on `Ty::Ref` in declaration contexts where uniqueness is preserved | D1.1 LOCKED + D7/D8 LOCKED; Section 4.1 worked example (already in shared `../../vocabulary.md`) shows the projection |
| `volatile` | intrinsic `InstanceId` identity (closed MMIO-intrinsic vocabulary; the `Call` terminator's callee identity is the carrier) | D9 LOCKED (this addendum) |

The S0 framing principle is independent of the specific
Provenance encoding D7/D8 picked: the principle ("`Ty` does
not carry C qualifiers as its own bits; qualifiers are
emitted at codegen time from per-decl / per-op information")
would hold under any Provenance shape D7/D8 might have
landed on. The table now reflects the actual locked D7/D8
shape (`Ty::Ref(TyId, Provenance)` 2-arg form;
`Ty::Ptr(TyId, Mutability)` renamed from `Ty::RawPtr`).
The principle is robust to the specific Provenance encoding
chosen.

**Why no QualType layer.**

- *Clang has `QualType` because C's type system genuinely treats
  `const int` as a distinct type from `int`* -- C expression-
  result types depend on the qualifier (`*p` for `p: const int*`
  has type `const int`). A C frontend has to model that.
- *Rust's MIR does not have type-level qualifiers in that sense.*
  `const`-on-pointee is encoded by the `*const T` vs `*mut T`
  and `&T` vs `&mut T` distinction (already distinct `Ty`
  variants in our IR via the `Mutability` and `Provenance`
  fields); `volatile` is intrinsic-level, never a type
  qualifier; `static` is an item attribute. Nothing in Rust's
  source-language type system forces a QualType wrapper on our
  IR.
- *Methodology -- "C semantics with information preservation":*
  preserve what rustc actually computed; do not import a
  C-frontend-specific layer that rustc deliberately does not
  use.
- *Methodology -- "compositional behavior beats special-case
  flags":* each qualifier already falls out of existing fields.
  Adding a `qualifiers: QualifierFlags` field on `Ty` would
  duplicate information and create divergence questions ("what
  if QualType says one thing and the per-decl data says
  another?").
- *Methodology -- "compiler-theory-first":* C qualifiers are an
  artifact of C's surface syntax; in the resolved post-mono Rust
  view, the same information is carried at the operation /
  declaration / item level. The IR should mirror the resolved
  structure, not the surface syntax of the target language.

**How future qualifier questions resolve under this frame.**
A new C qualifier (or a new question about an existing one)
should be answered by identifying the IR site that already
carries the necessary information; only if no such site exists
does the question expand to "should we add a new field?", and
even then the field should land at the operation / declaration
level, not on `Ty`. D9 below applies the principle to
`volatile`.

---

## S1. D9 -- Volatile semantics: per-intrinsic-InstanceId, no Ty bit [LOCKED]

**Decision.** `Ty` does not carry any volatile annotation. The
regseq-model IR has no `volatile: bool` field at any level --
not on `Ty`, not on a generic load/store `Statement` (stage 9
territory), not on the `Call` terminator. Volatility is encoded
**on the intrinsic `InstanceId`**: MMIO-access intrinsics
recognised by the closed intrinsic vocabulary
(`#[rseq::intrinsic("mmio_read")]`,
`#[rseq::intrinsic("mmio_write")]`, and the `*_field` variants
exemplified in `tests/fixtures/pl011-full-spec/src/regseq/mmio.rs:35-71`)
are the carriers. Stage-13 codegen for those `InstanceId`s emits
volatile-marked C accesses (`*(volatile uint32_t*)addr` and the
write counterpart); the IR carries no IR-level flag.

The bridge **rejects** raw `core::ptr::read_volatile` and
`core::ptr::write_volatile` calls at `Call`-terminator
translation. All MMIO access in the regseq DSL must flow through
the `#[rseq::intrinsic("mmio_*")]` API. This rejection is the
analog of D1.4 Category A applied to *function calls* rather
than *types*: rigid Rust constructs that exist in `core` but
lie outside the regseq DSL surface are rejected at the bridge
with a clear diagnostic. Enforcement lives at stage 9 / stage
13 (the Call-translation and intrinsic-recognition layers
respectively); D9 records the policy here for audit
completeness.

This decision is the application of the S0 framing principle
("no QualType layer") to the volatile qualifier. Each C
qualifier maps to a distinct IR site (S0 table); for `volatile`
that site is the intrinsic `InstanceId` identity.

**Reasoning.**

Stage 3 owns the question "does `Ty` carry a volatile bit?"
The answer is no, by direct application of the S0 framing
principle: regseq-model's `Ty` represents the unqualified Rust
type; C qualifiers are emitted at codegen time from per-decl /
per-op information accumulated elsewhere in the IR. Adopting
type-level volatile would be a one-off departure from the
pattern that already governs `const`, `static`, and `restrict`.

Among the three options stage 3 had to disambiguate -- per-type
(CompCert-style `attr_volatile: bool`), per-op-on-Statement
(CLR-style `LdInd { volatile: bool }`), or
per-intrinsic-InstanceId (no IR flag at any level) -- the third
follows from already-locked commitments rather than from
prior-art arbitration:

1. *Q7 + the closed intrinsic vocabulary* already commit to
   `#[rseq::intrinsic("NAME")]`-attribute-driven recognition
   with unknown names rejected. The infrastructure to encode
   "this `Call` is to an MMIO operation" already exists at the
   `InstanceId` level.

2. *The DSL three-mechanism convention* (`regseq-background.md`
   lines 14-27) places MMIO unambiguously in intrinsic territory
   (Intrinsics for operational verbs; Properties for declarative
   adjectives; Traits for structural integration). The pl011
   fixture's `mmio.rs` (lines 35-71) confirms intrinsic-only
   usage as the actual driver pattern, not aspirational design.

3. *DSL surface rule* ("Regseq DSL uses a limited subset of
   `core`. No other crates.") plus the methodology rule "no
   defensive fallbacks" jointly imply rejection of raw
   `core::ptr::{read,write}_volatile`: drivers do not need them
   when the intrinsic API exists; if they appear, that is a
   DSL-scope violation, not a pattern to support. This is the
   same posture D1.4 Category A takes for `Box`, `Coroutine`,
   etc. -- restated for function calls instead of types.

4. *Compositional behavior beats special-case flags.* If
   volatile lives on the `InstanceId`, no `Ty` flag is needed
   and no Statement flag is needed. Adding either is exactly
   the kind of speculative carrier the methodology rejects --
   and under per-intrinsic-InstanceId placement, no consumer
   for an IR-level flag materialises in v1.

5. *Stage-3 footprint becomes minimal and precise.* D9's
   stage-3 lock answers "is there a `Ty` bit?" with a justified
   no, and forwards the enforcement question to the layers
   that already own intrinsic recognition (stage 9 + stage 13).

The CompCert outlier (per-type `attr_volatile`) is justified by
trace-semantics observability: CompCert's verified semantics
must record volatile operations as observable behaviour, and
the type system therefore has to know which accesses are
volatile (`prior-art/compcert.md` Theme D, citing the CompCert
manual lines 729-746). This rationale does not transfer: we
emit C and have no trace-semantics requirement.

The CLR per-op-on-Statement choice (`LdInd { volatile: bool }`)
is sound for projects that admit raw volatile reads/writes in
their input language and have to round-trip the flag through
the IR. We do not -- the DSL surface rejects raw `*_volatile`
upstream of the IR.

**Methodology rules invoked.**

- *"C semantics with information preservation"* -- preserve
  rustc's view (no type-level volatile); emit C qualifiers at
  codegen from per-decl / per-op information accumulated
  elsewhere in the IR.
- *"Compositional behavior beats special-case flags"* -- each
  C qualifier maps to an existing IR site (S0 table); no
  QualType wrapper or flag-on-Ty is needed.
- *"No defensive fallbacks"* -- bridge rejects raw
  `core::ptr::{read,write}_volatile` rather than carrying a
  speculative IR flag for consumers that do not exist in v1.
- *"Don't redo work the rustc compiler has already done"* --
  rustc routes volatile through intrinsics; we mirror that
  posture rather than inventing a type-level qualifier rustc
  itself does not use.
- *"Compiler-theory-first"* -- volatile is a memory-effect
  property of an operation, not a type-level shape. Type-level
  volatile is justified only when the IR's semantics must
  observe volatile at the type-system level (CompCert's trace
  semantics); we have no such constraint.

**Prior-art evidence.**

Across surveyed projects, all explicit volatile-handling
evidence in the prior-art Theme D entries places volatile
per-op or off the type entirely; CompCert is the lone per-type
outlier (`prior-art/<project>.md` Theme D entries):

- *LLVM* -- volatile is a per-op flag on load/store
  instructions; the type system does not carry it. Explicit
  design statement: "Volatile is a flag on memory operations,
  not on the type." (`prior-art/llvm-ir.md` Theme D, lines
  311-336.)
- *mrustc* -- volatile is not used in the C output for Rust
  types; appears only as `__asm__ __volatile__` inside
  intrinsic implementations (`xgetbv`, `pause`); atomics are
  realised through intrinsic helpers, not type markers
  (`prior-art/mrustc.md` Theme D, lines 516-518 and 520-536).
- *Charon* -- Theme D shows that atomic types and marker types
  (`UnsafeCell`, `MaybeUninit`, `PhantomData`) are ordinary
  `Adt(TypeDeclRef)` instances with no type-level markers;
  consumers query type path on-demand
  (`prior-art/charon.md` Theme D, lines 609 and 664). Charon's
  Theme D does not specifically address volatile -- the
  *absence* of a volatile marker in Charon's TyKind is
  suggestive of the no-type-level-volatile pattern but is not
  a load-bearing claim for D9. (D9's substantive lock rests on
  the explicit per-op evidence from LLVM/mrustc/CLR/rustc-codegen-c
  and the general "special semantics off type" pattern from
  MLIR, plus rustc's intrinsic-level volatile.)
- *rustc_codegen_clr* -- per-op boolean flag on
  `LdInd { addr, tpe, volatile: bool }` and
  `StInd(addr, value, type, is_volatile)`; atomicity is
  lowered to method calls, not embedded in type; UnsafeCell
  is invisible at the type level. Design summary: "does not
  bake any 'special semantics' into `Type`."
  (`prior-art/rustc-codegen-clr.md` Theme D, lines 328-382;
  summary at line 381.)
- *rustc-codegen-c* -- no `const` / `volatile` / `restrict`
  qualifiers anywhere on `CTy`; design pattern is "preserve
  Rust semantics in emitted expressions, not in the type
  encoding" (`prior-art/rustc-codegen-c.md` Theme D, lines
  224-242, with full-file verification at line 120).
- *MLIR* -- type traits / parameters / type interfaces are
  the three orthogonal mechanisms for special semantics; none
  embed volatility into the type itself; layout queried via
  `DataLayoutTypeInterface`, not stored on type
  (`prior-art/mlir.md` Theme D, lines 365-405; layout
  reference at lines 221-255).
- *CompCert (sole outlier)* -- `attr_volatile: bool` is on
  every `attr` record in `Ctypes.type` (lines 100-107 of
  Ctypes.v). Rationale per the CompCert manual: "a type
  denotes a volatile access if it carries the volatile
  attribute and it is accessed by value" (line 429); required
  because CompCert's *trace semantics* must record volatile
  operations as observable behaviour
  (`prior-art/compcert.md` Theme D, lines 415-434, citing
  the CompCert manual).

Rustc's own posture mirrors the convergent group: volatile is
intrinsic-level (`ptr::read_volatile`, `ptr::write_volatile`);
no type variant carries volatile semantics
(`findings-rustc-draft.md` lines 570-572). regseq-model
follows rustc's posture, refined for our DSL surface (raw
volatile rejected; MMIO via intrinsic only).

**DSL evidence.**

- *Three-mechanism convention.* `../../../regseq-background.md`
  lines 14-27 establish the three mechanisms (Intrinsics
  operational / Properties declarative / Traits integration)
  with `busy_wait`, `sleep`, `poll_until` listed as
  illustrative operational-verb examples. The convention
  places memory-effect operations (including MMIO) in
  intrinsic territory by category, even though the specific
  MMIO intrinsic names are not enumerated in background.md.
- *pl011 fixture (specific intrinsic names).*
  `tests/fixtures/pl011-full-spec/src/regseq/mmio.rs:35-71`
  uses `#[rseq::intrinsic("mmio_read")]`,
  `#[rseq::intrinsic("mmio_write")]`,
  `#[rseq::intrinsic("mmio_read_field")]`,
  `#[rseq::intrinsic("mmio_write_field")]` exclusively. The
  intrinsic methods are DSL stubs (`todo!("facade stub:
  lowered by the compiler backend")`); the compiler backend
  handles volatile semantics at lowering time. This is the
  load-bearing in-repo evidence for the `mmio_*` intrinsic
  names; background.md establishes the *category* (operational
  verbs as intrinsics), and the fixture establishes the
  *specific names*.
- *Stage 1 Q7.* The bridge recognises intrinsics by
  `#[rseq::intrinsic("NAME")]` attribute on the callee, with
  a closed intrinsic vocabulary; unknown names are rejected
  (`../../../stage-1/vocabulary.md` lines 869-899;
  `../../../architecture.md` Q7). MMIO intrinsics will be
  added to the closed vocabulary in stage 13.
- *Stage 13 expectations (current state and merge-session
  task).* `../../../stage-13/expectations.md` (lines 12-17 and
  50-58) currently records that stage 13 ships the closed
  intrinsic-vocabulary list and the bridge-side recognition
  surface (the "Pin the intrinsic vocabulary (Q7)" bullet and
  the "Recognition surface" bullet under "Bridge concerns").
  The MMIO-specific vocabulary entries and the
  Call-translation enforcement of D9's rejection of raw
  `core::ptr::{read,write}_volatile` are not yet recorded
  there; they are outbound forward refs from stage 3 (D9) to
  stage 13 that the merge session should land into
  `stage-13/expectations.md` when SESSION-C is integrated.

**Worked example.** See section S1.1 below.

### S1.1 Worked example -- MMIO read via intrinsic

**Rust (driver-side):**
```rust
#[repr(C)]
struct UartFlagsReg(u32);

// In a real driver, `read::<UartFlagsReg, _>(...)` resolves
// to the intrinsic-flagged method on the MMIO façade
// (per pl011 fixture mmio.rs:35-71). Reduced here for
// readability.
#[rseq::intrinsic("mmio_read")]
fn mmio_read_u32(base: *mut u8, offset: usize) -> u32 {
    todo!("facade stub: lowered by the compiler backend")
}

fn poll_busy(base: *mut u8) -> bool {
    let flags = mmio_read_u32(base, 0x18);
    (flags & (1 << 5)) != 0   // status-bit check (illustrative)
}
```

**IR (post-bridge, schematic).** Inside `poll_busy`'s
body the bridge produces a `Call` terminator targeting the
`InstanceId` for `mmio_read_u32` (the intrinsic). The call's
operands and result hold ordinary `TyId`s with no volatile
annotation:

```
poll_busy:
  bb0:
    _1 = base                               // Ty: Ptr(U8, Mutability::Mut)
    _2 = const 0x18_usize                   // Ty: Uint(Usize)
    Call(_3 = mmio_read_u32(_1, _2),
         target: bb1)                       // _3 Ty: Uint(U32)
                                            //   ^ NO volatile bit anywhere
                                            //   ^ Call terminator carries
                                            //     no `volatile: bool` field
                                            //   ^ The InstanceId for
                                            //     mmio_read_u32 is the
                                            //     carrier (recognised by
                                            //     stage-13's closed
                                            //     intrinsic vocabulary)
  bb1:
    _4 = BitAnd(_3, const 0x20_u32)         // Ty: Uint(U32)
    _0 = Ne(_4, const 0_u32)                // Ty: Bool
    Return
```

**C lowering (stage-13 emission, preview).**

```c
static inline bool poll_busy(uint8_t *base) {
    /* Stage-13 codegen for the mmio_read_u32 InstanceId
       emits a volatile-cast access. The volatility is a fact
       about THIS InstanceId's emission, not about any Ty in
       the IR. */
    uint32_t flags = *(volatile uint32_t *)(base + 0x18);
    return (flags & (1u << 5)) != 0;
}
```

**Rejected counter-example (raw `core::ptr::read_volatile`).**
A driver that sidesteps the intrinsic API:

```rust
fn poll_busy_bad(base: *mut u8) -> bool {
    // REJECTED at the bridge (Call-terminator translation,
    // stage 9 / stage 13 enforcement).
    let flags = unsafe {
        core::ptr::read_volatile(base.add(0x18) as *const u32)
    };
    (flags & (1 << 5)) != 0
}
```

The bridge raises a diagnostic naming `core::ptr::read_volatile`
as out-of-DSL-scope and pointing to the
`#[rseq::intrinsic("mmio_*")]` API as the canonical
alternative. Same posture as D1.4 Category A rejections,
applied to function calls instead of types.

**Observation.** No `Ty` in the model carries a volatile bit.
The `*mut u8` argument, the `usize` offset, the `u32` result
-- every type along the path is the unqualified Rust type
exactly as rustc resolves it. The volatility is recovered at
codegen time from the `InstanceId`'s identity, consistent
with the S0 framing principle.

---

## S2. D10 -- Atomics: regseq-DSL `Atomic<T>` with intrinsic operations; no Ty bit [LOCKED]

**Decision.** `Ty` does not carry any atomic annotation. The
regseq-model IR has no `atomic: bool` field at any level -- not
on `Ty`, not on a generic load/store `Statement` (stage 9
territory), not on the `Call` terminator. There is no `Ty::Atomic<T>`
variant and no `is_atomic` flag on `Item`. This is direct
application of the S0 framing principle ("no QualType layer") to
the atomic-operation domain, paralleling D9's resolution for
`volatile`.

Atomic *types* in the regseq DSL are ordinary `Ty::Struct`
instances. The DSL provides its own `Atomic<T>` abstraction
(stage-13 territory for the exact shape) -- a `repr(C)` struct
containing an `UnsafeCell<T>` field -- and the bridge sees it as
a plain `Ty::Struct(<regseq::Atomic ItemId>, <args=[T]>)`. The
existing `is_unsafe_cell` flag (D1.5) on the inner field handles
`const`-dropping on enclosing references, exactly as for any
other UnsafeCell-bearing struct; no atomic-specific machinery is
needed at the type level.

Atomic *operations* are stage-13 intrinsic Calls, recognised by
Q7's existing `#[rseq::intrinsic("NAME")]` attribute mechanism.
Memory ordering (`Relaxed` / `Acquire` / `Release` / `AcqRel` /
`SeqCst`) is operation-level metadata -- it survives in the IR
either as an operand on the `Call` to a single
`atomic_<op>(_, order)` intrinsic, or via per-(op × ordering)
`InstanceId` identity (e.g., `atomic_load_acquire`,
`atomic_load_relaxed`, ...). Stage 13 owns the encoding choice;
either form keeps ordering at the operation level and out of
`Ty`.

The bridge rejection has two phases:

- *Type-translation rejection.* The bridge rejects the
  `core::sync::atomic::Atomic*` type family at the point it
  translates an atomic-typed Rust binding (e.g., `&AtomicU32`,
  `*const AtomicBool`, an `AtomicI64` field). Members
  (per `library/core/src/sync/atomic.rs` in the pinned
  toolchain): `AtomicU8`, `AtomicI8`, `AtomicU16`, `AtomicI16`,
  `AtomicU32`, `AtomicI32`, `AtomicU64`, `AtomicI64`,
  `AtomicU128`, `AtomicI128` (the 128-bit variants are gated
  on `target_has_atomic_load_store = "128"` per
  `library/core/src/sync/atomic.rs:3884-3920` in the toolchain
  source; rejection scope is the family, not gated by the
  cfg), `AtomicUsize`, `AtomicIsize`, `AtomicBool`,
  `AtomicPtr<T>`, plus `Atomic<T>` -- the unstable generic
  atomic type alias declared with the
  `#[unstable(feature = "generic_atomic", issue = "130539")]`
  attribute (tracking issue rust-lang/rust#130539) at
  `library/core/src/sync/atomic.rs:340-345` in the toolchain
  source. Rejection scope is the family, not conditional on
  the unstable feature gate (the alias is in scope even
  though source-side use of it requires
  `#![feature(generic_atomic)]`).

  *Naming clarification.* `core::sync::atomic::Atomic<T>`
  (the unstable generic alias just listed) and the
  regseq-DSL `Atomic<T>` facade (introduced in this
  decision, e.g., `regseq::Atomic<T>`) are distinct types
  living in distinct namespaces. The rejection target is
  *the core alias* (`core::sync::atomic::Atomic<T>` and the
  rest of the family above); driver code that refers to
  `regseq::Atomic<T>` via the regseq DSL imports is the
  accept path.
- *Call-translation rejection.* The bridge rejects calls to
  the methods on the above types (`.load()`, `.store()`,
  `.fetch_add()`, `.fetch_sub()`, `.fetch_and()`, `.fetch_or()`,
  `.fetch_xor()`, `.swap()`, `.compare_exchange()`,
  `.compare_exchange_weak()`, ...) and to the rustc-internal
  `core::intrinsics::atomic_*` family.

Explicitly *not* in the rejection scope:
`core::sync::atomic::Ordering` (the value-level enum -- a
plain Rust enum that `regseq::Atomic<T>` may re-export or
shadow with its own ordering enum, per stage-13's choice).

*Out of D10's scope (free functions for memory barriers /
fences).* The `core::sync::atomic` module also contains
free functions for memory barriers and compiler fences --
notably `core::sync::atomic::fence(Ordering)` and
`core::sync::atomic::compiler_fence(Ordering)`, plus
related items. These are *not* part of the regseq Atomic
construct surface (which is value-keyed atomic accesses;
see
`docs/compiler-collateral/constructs/atomic-memory-access-and-ordering.md`
for the construct-level scoping that explicitly excludes
barrier/fence elements from the Atomic abstraction). D10
does not classify them: barrier/fence treatment is a
separate construct-level decision, deferred to a future
discussion. Concretely, this means D10 adds no accepted
intrinsic-vocabulary entry for these free functions; any
source-level use of them in driver code remains outside
D10's accept path. The bridge-side handling of such calls
is *not* governed by Q7's
`#[rseq::intrinsic("NAME")]`-attribute mechanism (Q7
applies only to attribute-tagged callees, not to raw
`core::*` free-function calls per
`../../../architecture.md` Q7 and
`../../../stage-13/expectations.md:12-17, 50-58`); whether
the bridge rejects, errors-on-unsupported, or eventually
accepts these calls is owed to the future construct-level
decision (or to a separate stage-13 unsupported-call
policy), not to D10's stage-3 lock.

All driver atomic access flows through `regseq::Atomic<T>` and
its `#[rseq::intrinsic("atomic_*")]`-flagged methods. The
rejection is the same posture D9 takes for raw
`core::ptr::{read,write}_volatile` -- rigid Rust constructs
that exist in `core` but lie outside the regseq DSL surface
are rejected with a clear diagnostic, not handled
defensively. The Ordering exclusion does not affect D10's
stage-3 lock.

This decision is the application of the S0 framing principle
("no QualType layer") to the atomic operational domain. Each C
qualifier or memory-effect property maps to a distinct IR site
(S0 table); for atomics that site is the intrinsic `InstanceId`
identity plus an ordering operand or per-ordering `InstanceId`
selection.

**Reasoning.**

Stage 3 owns the question "does `Ty` carry an atomic bit?"
The answer is no, by direct application of the S0 framing
principle and by parallel construction with D9. The four
sub-questions stage 3 had to disambiguate resolve as follows:

1. *Substantive lock -- atomic types are ordinary `Ty::Struct`.*
   Atomic types in the Rust stdlib (and, by analogy, in the
   regseq DSL) are structurally `repr(C)` structs containing a
   single `UnsafeCell<T>` field. The C lowering needed (drop
   `const` on enclosing shared references, since interior
   mutation is permitted) is already produced by the
   `is_unsafe_cell` flag (D1.5). Adding an `is_atomic` flag or a
   `Ty::Atomic<T>` variant would duplicate information and
   inflate the variant set with no semantic gain.

2. *Ingest policy -- regseq DSL provides `Atomic<T>`; the
   `core::sync::atomic::Atomic*` type family + their methods +
   `core::intrinsics::atomic_*` are rejected at the bridge
   (`Ordering` excluded; see decomposition above).* The
   argument has five architectural strands:

   - *Architectural symmetry with D9.* D9 rejected raw
     `core::ptr::{read,write}_volatile` and required
     `#[rseq::intrinsic("mmio_*")]`. Admitting the
     `core::sync::atomic` operational API would have given the
     DSL two different ingest patterns for what are structurally
     the same thing -- operational verbs in the three-mechanism
     convention. Rejecting it makes both volatile and atomic
     follow the same rule: operational primitives are DSL
     intrinsics; nothing operational comes from `core::*`
     directly.

   - *DSL surface coherence.* Regseq drivers are DSL-encoded
     device specs, not generic Rust programs with kernel
     attributes; the author has already opted into the regseq
     surface. Using `regseq::Atomic<T>` instead of
     `core::sync::atomic::AtomicU32` is a one-import-line
     change, not an ergonomic loss.

   - *DSL controls vocabulary instead of inheriting it.* With
     `regseq::Atomic<T>`, the DSL pins exactly which atomic
     operations exist; rustc evolution is decoupled from the
     DSL surface. Admitting `core::sync::atomic` would have
     left the DSL at the mercy of stdlib changes -- new
     methods auto-leaking in or requiring explicit gating.

   - *Single bridge recognition mechanism.* Q7's
     `#[rseq::intrinsic("NAME")]` attribute remains the only
     admission path. Admitting `core::sync::atomic` would have
     required a parallel path-based recognizer for
     `core::intrinsics::atomic_*` (rustc-internal, no
     `#[rseq::intrinsic]` attribute). One mechanism is cleaner.

   - *Per-target codegen latitude is unaffected.* The
     Linux/Zephyr divergence -- Linux's `atomic_*` family
     covers Relaxed/Acquire/Release directly via the
     `_relaxed`/`_acquire`/`_release` suffixes plus an
     unsuffixed (fully ordered) form for SeqCst, with AcqRel
     conservatively strengthened to unsuffixed (Linux has no
     dedicated AcqRel suffix); Zephyr's public API is
     SeqCst-only and codegen must use GCC `__atomic_*`
     builtins to preserve non-SeqCst orderings -- is handled
     the same way regardless: stage-13 codegen for the regseq
     atomic-intrinsic InstanceIds picks per target. The IR
     carries ordering at the operation/InstanceId level
     either way.

3. *Memory ordering -- no stage-3 representation.* Memory
   ordering is operation-level metadata in the IR, surviving
   either as an operand on the `Call` to a single
   `atomic_<op>(_, order)` intrinsic, or via per-(op × ordering)
   `InstanceId` identity (e.g., `atomic_load_acquire`,
   `atomic_load_relaxed`, ...). Both encodings keep ordering
   at the operation/InstanceId level; neither requires a
   stage-3 `Ty`-level representation. Stage 13 picks the
   encoding shape (and, in particular, decides whether ordering
   on the regseq `Atomic<T>` facade is a const-generic
   parameter, a value parameter that must be const-evaluable
   pre-bridge, or fully baked into the intrinsic name --
   stage-13 territory, not stage-3).

4. *Compositional behavior beats special-case flags.* If
   atomicity lives on the `InstanceId`, no `Ty` flag is needed
   and no Statement flag is needed. Adding either is exactly the
   kind of speculative carrier the methodology rejects -- and
   under per-intrinsic-`InstanceId` placement, no consumer for
   an IR-level flag materialises in v1.

**Methodology rules invoked.**

- *"C semantics with information preservation"* -- preserve
  rustc's view (no type-level atomic); emit C atomic-operation
  forms at codegen from per-op information accumulated at the
  `Call` site.
- *"Compositional behavior beats special-case flags"* -- the
  existing `is_unsafe_cell` flag (D1.5) handles the type-level
  part; no atomic-specific flag or variant is needed.
- *"No defensive fallbacks"* -- bridge rejects the
  `core::sync::atomic::Atomic*` type family and methods plus
  the rustc-internal `core::intrinsics::atomic_*` family
  (Ordering excluded), rather than carrying speculative IR
  flags for consumers that do not exist in v1.
- *"Don't redo work the rustc compiler has already done"* --
  rustc routes atomicity through intrinsics; we mirror that
  posture rather than inventing a type-level qualifier rustc
  itself does not use. Memory ordering is operation-level
  metadata that survives at the operation/InstanceId level;
  stage 13 chooses the encoding shape.
- *"Re-derive from evidence; do not import baggage"* -- the
  `core::sync::atomic` API is a stdlib design choice oriented
  to general Rust programs; the DSL surface is operational-
  verb-driven (three-mechanism convention), and atomic
  operations belong in the intrinsic vocabulary alongside
  MMIO. Importing the stdlib API into the DSL surface would
  duplicate layers without gain.
- *"Compiler-theory-first"* -- atomicity is a memory-effect
  property of an operation, not a type-level shape. Type-level
  atomic representations exist only in languages whose semantics
  must observe atomicity at the type-system level (which is
  rare; CompCert does not even cover atomics).

**Prior-art evidence.**

Across surveyed projects with explicit atomic-handling
evidence in the prior-art Theme D entries: atomic *types* are
treated as ordinary structs (with `UnsafeCell<T>` or
equivalent) and atomic *operations* are kept op-level
(intrinsic, method, or per-op metadata) -- no surveyed project
puts atomicity on the type. CompCert's source language has no
atomics, so it contributes no for/against atomic evidence.
Citations from `prior-art/<project>.md` Theme D entries:

- *LLVM* -- atomic operations carry an `atomic` modifier and a
  separate memory-ordering metadata field on load/store
  instructions (`monotonic` / `acquire` / `release` /
  `acq_rel` / `seq_cst`). The type system carries no
  atomicity (`prior-art/llvm-ir.md` Theme D, the same
  "Volatile is a flag on memory operations, not on the type"
  rule applies to atomicity, lines 311-336).
- *mrustc* -- atomic operations are realised through intrinsic
  helpers; `m_atomics` lives on the target architecture struct,
  not on type. Atomic types lower to ordinary structs in C
  output (`prior-art/mrustc.md` Theme D, lines 520-536).
- *Charon* -- no type-level markers for atomic types or
  marker types (`UnsafeCell`, `MaybeUninit`, `PhantomData`);
  atomic types are ordinary `Adt(TypeDeclRef)` instances
  (`prior-art/charon.md` Theme D, lines 609 and 664).
- *rustc_codegen_clr* -- atomicity is lowered to method calls;
  not embedded in type. Same posture as volatile in this
  project (`prior-art/rustc-codegen-clr.md` Theme D, lines
  328-382, 361-378).
- *rustc-codegen-c* -- atomic types are ordinary structs;
  atomicity is preserved in emitted expressions, not in the
  type encoding (`prior-art/rustc-codegen-c.md` Theme D, lines
  224-242).
- *MLIR* -- the prior-art Theme D entry does not record
  atomic-specific evidence; the file's general pattern
  (special semantics expressed via type parameters / type
  interfaces / type traits, not via closed-enum type variants
  -- `prior-art/mlir.md` Theme D, lines 365-405) is consistent
  with operation-level atomicity. The MLIR codebase generally
  uses dedicated dialect ops for atomics (`arith.atomic_*`,
  `memref.atomic_rmw`), but our prior-art file does not
  catalogue this; the inference is plausible but not
  load-bearing for D10.
- *CompCert* -- C source language has no atomics, so the
  Theme D atomic-specific evidence (lines 436-451 cover
  `_Atomic` and ABI/alignment context; the volatile-specific
  lines 415-434 are separate) contributes no for/against
  signal for D10's lock (`prior-art/compcert.md` Theme D,
  lines 436-451).

Rustc's own posture mirrors the surveyed group: atomic
operations are intrinsics (`core::intrinsics::atomic_*`); atomic
types (`AtomicU32`, etc.) are ordinary structs containing
`UnsafeCell<T>`; no type variant carries atomic semantics
(`findings-rustc-draft.md` Theme D regarding atomics; library
source `library/core/src/sync/atomic.rs` for the struct
shapes).

**DSL evidence.**

- *Three-mechanism convention.* `regseq-background.md` lines
  14-27 canonicalise operational verbs (Intrinsics) as the
  appropriate surface for memory-effect operations including
  MMIO, scheduling, and -- by direct analogy -- atomics.
- *Q7 attribute-based intrinsic recognition.* The bridge
  recognises intrinsics by `#[rseq::intrinsic("NAME")]`
  attribute on the callee, with a closed intrinsic vocabulary
  managed at stage 13 (`../../../stage-1/vocabulary.md` lines
  869-899; `../../../architecture.md` Q7). The planned regseq
  `Atomic<T>` facade (stage-13 territory; the precise method
  set, generic bounds, and ordering encoding are stage-13's
  to define) will carry the `#[rseq::intrinsic("atomic_*")]`
  attribute pattern on its operations, identical to the pl011
  fixture's `mmio.rs` facade convention
  (`tests/fixtures/pl011-full-spec/src/regseq/mmio.rs:35-71`).
  D10's stage-3 lock commits to the *posture* (DSL-defined
  facade with intrinsic-attribute methods) but not to the
  facade's exact API shape, which is stage-13's to lock.
- *Stage 13 expectations (current state and merge-session
  task).* `../../../stage-13/expectations.md` (lines 12-17 and
  50-58) currently records that stage 13 ships the closed
  intrinsic-vocabulary list and the bridge-side recognition
  surface (the "Pin the intrinsic vocabulary (Q7)" bullet and
  the "Recognition surface" bullet under "Bridge concerns").
  The atomic-specific vocabulary entries, the ordering
  encoding choice (per-(op × ordering) InstanceIds vs
  const-generic ordering on a single InstanceId per op), and
  D10's bridge-side rejection enforcement -- which decomposes
  into (a) *type-translation* rejection of the
  `core::sync::atomic::Atomic*` type family (`AtomicU32`,
  `AtomicBool`, `AtomicI64`, `AtomicPtr<T>`, ...), (b)
  *Call-translation* rejection of methods on those types
  (`.load`, `.store`, `.fetch_add`, `.compare_exchange`, ...)
  and of the rustc-internal `core::intrinsics::atomic_*`
  family, with (c) `core::sync::atomic::Ordering` (the
  value-level enum) explicitly *not* in the rejection scope,
  and (d) `core::sync::atomic::{fence, compiler_fence}` and
  related barrier/fence free functions explicitly *out of
  D10's scope* (not atomic intrinsic vocabulary entries;
  positive classification deferred to a future
  construct-level decision per
  `docs/compiler-collateral/constructs/atomic-memory-access-and-ordering.md`)
  -- are not yet recorded there; they are outbound forward
  refs from stage 3 (D10) to stage 13 that the merge session
  should land into `stage-13/expectations.md` when SESSION-C
  is integrated.

**Linux / Zephyr C-side evidence.**

Both v1 codegen targets reject C11 `_Atomic` by convention; both
keep atomicity at the operation/function level. This is hard
external corroboration that an IR targeting Linux + Zephyr need
not (and should not) carry type-level atomic information.

- *Linux kernel.* `Documentation/atomic_t.txt` documents the
  canonical atomic API; types are `atomic_t`, `atomic64_t`,
  `atomic_long_t`, `refcount_t` (none of them C11 `_Atomic`).
  Operations expose four ordering surfaces via suffix --
  `_relaxed` (Relaxed), `_acquire` (Acquire), `_release`
  (Release), and unsuffixed (fully ordered, used for SeqCst
  and -- conservatively, since no Linux suffix maps to it
  directly -- for AcqRel). Examples: `atomic_read_acquire`,
  `atomic_set_release`, `atomic_xchg_relaxed`,
  `atomic_cmpxchg{,_relaxed,_acquire,_release}`. Headers:
  `include/linux/atomic.h`,
  `include/linux/atomic/atomic-arch-fallback.h`,
  `arch/$ARCH/include/asm/atomic.h`. Linus's well-known
  position (LKML 2017) prefers `volatile` + inline asm over
  `_Atomic`; `atomic_t.txt` mentions C11 only obliquely as a
  semantic match. Non-RMW canonical implementation uses
  `READ_ONCE` / `WRITE_ONCE` / `smp_load_acquire` /
  `smp_store_release` (`include/asm-generic/rwonce.h`).
- *Zephyr RTOS.* `include/zephyr/sys/atomic_types.h` defines
  `atomic_t` (long, word-sized) and `atomic_ptr_t`. No C11
  `_Atomic`. Public API in `include/zephyr/sys/atomic.h` is
  SeqCst-only -- the default backend
  (`include/zephyr/sys/atomic_builtin.h`) implements every op
  via GCC `__atomic_*` builtins with `__ATOMIC_SEQ_CST` baked
  in. Other orderings are not exposed by the public API. To
  preserve a non-SeqCst ordering on Zephyr, codegen must emit
  `__atomic_*` builtins directly (e.g.,
  `__atomic_load_n(&v, __ATOMIC_ACQUIRE)`), bypassing the
  public Zephyr API.

The implication for stage 13 (recorded here for the audit
trail): the IR carries memory ordering as operation-level
metadata; stage-13 codegen for each target must define a per-op
mapping from the regseq `Ordering` set
(`Relaxed`/`Acquire`/`Release`/`AcqRel`/`SeqCst`) into the
target's C-side surface, preserving Rust ordering where the
target supports it directly and conservatively strengthening
where it does not. On Linux, the `_relaxed`/`_acquire`/`_release`
suffix family covers `Relaxed`/`Acquire`/`Release` directly;
the unsuffixed (fully ordered) form covers `SeqCst` and must
also stand in for `AcqRel` (since Linux's `atomic_*` family
does not provide a dedicated AcqRel suffix -- substituting
fully-ordered is sound but conservatively stronger than
AcqRel). On Zephyr, codegen either falls through to GCC
`__atomic_*` builtins (preserves the ordering exactly) or
uses the public Zephyr API (which pessimises every ordering
to SeqCst). Either Zephyr choice fits the IR shape; the
choice is per-target, per stage-13's discretion.

**Worked example.** See section S2.1 below.

### S2.1 Worked example -- `Atomic<u32>` with acquire-load

**Regseq DSL `Atomic<T>` facade (illustrative; exact API is
stage-13 territory).** The shape mirrors the pl011 fixture's
`mmio.rs` convention -- intrinsic-flagged methods with `todo!()`
bodies that the compiler backend lowers per target:

```rust
// Conceptual sketch -- the actual definition lives in a
// regseq DSL crate (regseq-core or similar; stage-13 owns
// the precise field set, generic bounds on T, and method
// list).
#[repr(C)]
pub struct Atomic<T> {
    inner: UnsafeCell<T>,
}

pub enum Ordering { Relaxed, Acquire, Release, AcqRel, SeqCst }

impl<T> Atomic<T> {
    #[rseq::intrinsic("atomic_load")]
    pub fn load(&self, _order: Ordering) -> T {
        todo!("facade stub: lowered by the compiler backend")
    }

    #[rseq::intrinsic("atomic_store")]
    pub fn store(&self, _value: T, _order: Ordering) {
        todo!("facade stub: lowered by the compiler backend")
    }

    #[rseq::intrinsic("atomic_fetch_add")]
    pub fn fetch_add(&self, _value: T, _order: Ordering) -> T {
        todo!("facade stub: lowered by the compiler backend")
    }
    /* ... */
}
```

**Driver source.**

```rust
use regseq::{Atomic, Ordering};

fn read_count(counter: &Atomic<u32>) -> u32 {
    counter.load(Ordering::Acquire)
}
```

**IR (post-bridge, schematic).** `read_count`'s body produces
a `Call` terminator targeting the `InstanceId` for the
atomic-load intrinsic (recognised by Q7 via the
`#[rseq::intrinsic("atomic_load")]` attribute on
`Atomic<T>::load`). The call's operands and result carry
ordinary `TyId`s with no atomic annotation; ordering is an
operand value (or, equivalently in stage 13, baked into the
InstanceId identity for an `atomic_load_acquire` variant):

```
read_count:
  bb0:
    _1 = counter                           // Ty: Ref(Atomic_u32, Provenance::SharedBorrow)
    _2 = const Ordering::Acquire           // Ty: Enum (regseq::Ordering)
    Call(_3 = atomic_load(_1, _2),
         target: bb1)                      // _3 Ty: Uint(U32)
                                            //   ^ NO atomic bit anywhere
                                            //   ^ Call carries no `atomic: bool`
                                            //   ^ Ty::Struct(regseq::Atomic, [u32])
                                            //     is an ordinary struct with an
                                            //     `is_unsafe_cell` flag on its
                                            //     inner UnsafeCell<u32> field
                                            //     (D1.5 -- which drops `const`
                                            //     on the enclosing &Atomic<u32>)
                                            //   ^ The InstanceId for atomic_load
                                            //     is the carrier; ordering is
                                            //     an operand
  bb1:
    _0 = _3
    Return
```

**C lowering, target-aware (stage-13 emission, preview).**

The C-side `Atomic<u32>` lowers to a plain struct -- no C11
`_Atomic`, neither target uses it:

```c
typedef struct Atomic_u32 {
    uint32_t value;   /* the inner field, after UnsafeCell unwrapping per D1.5 */
} Atomic_u32_t;
```

The acquire-load is target-specific:

*Linux (preserves Acquire ordering using the canonical Linux
non-RMW idiom for plain memory locations):*

```c
/* Note: NO `const` qualifier on the parameter, because
   `Atomic_u32_t` transitively contains `UnsafeCell<u32>` and
   the locked D1.5 `is_unsafe_cell` flag drops `const` on
   enclosing references (see `../../vocabulary.md` Section 1.6
   D1.5 and Section 4.5 worked example). The Rust signature
   `&Atomic<u32>` (shared borrow) maps to `Atomic_u32_t *`
   (no `const`), reflecting Rust's interior-mutability
   contract for `UnsafeCell`-bearing types. */
static inline uint32_t read_count(Atomic_u32_t *counter) {
    /* Stage-13 codegen for the atomic_load + Acquire InstanceId
       emits the Linux non-RMW Acquire idiom. The atomicity /
       ordering is a fact about THIS InstanceId's emission, not
       about any Ty in the IR. */
    return smp_load_acquire(&counter->value);
}
```

Why `smp_load_acquire` rather than `atomic_read_acquire`:
`atomic_read_acquire` takes `const atomic_t *`, where `atomic_t`
is an opaque struct wrapping `int`; type-punning a
`const uint32_t *` to `const atomic_t *` is not valid in
general. The Linux convention for non-RMW Acquire loads on a
plain memory location is `smp_load_acquire`, defined in
`include/asm-generic/barrier.h` and documented in
`Documentation/atomic_t.txt` ("Non-RMW ops are canonically
implemented using `READ_ONCE()`, `WRITE_ONCE()`,
`smp_load_acquire()` and `smp_store_release()`"), which works
on plain primitive types. If a future SESSION-C-derived
codegen choice prefers `atomic_t`-typed storage (to enable
RMW operations like `atomic_add_acquire`), the storage shape
would change to `atomic_t value;` -- this is stage-13's
per-target storage-shape decision, not stage-3's lock.

*Zephyr (Zephyr's public API is SeqCst-only; codegen falls
through to GCC `__atomic_*` builtins to preserve Acquire):*

```c
/* Same `const` drop as the Linux signature above:
   `Atomic_u32_t` transitively contains `UnsafeCell<u32>`, so
   the D1.5 flag drops `const` on the enclosing reference. */
static inline uint32_t read_count(Atomic_u32_t *counter) {
    /* For the plain uint32_t storage shown above, only the GCC
       __atomic_* builtin works directly. Zephyr's atomic_get is
       not type-compatible with this storage -- it takes
       atomic_t* (long, word-sized, per
       include/zephyr/sys/atomic_types.h), so a different storage
       shape (e.g., `atomic_t value;` declared per stage-13's
       per-target choice) would be required to use it. Even
       given compatible storage, Zephyr's public atomic_get is
       SeqCst-only and would pessimise Acquire to SeqCst. The
       __atomic_load_n bypass shown here preserves Acquire on
       the existing storage; stage-13 chooses. */
    return __atomic_load_n(&counter->value, __ATOMIC_ACQUIRE);
}
```

**Rejected counter-example (raw `core::sync::atomic::AtomicU32`).**
A driver that sidesteps the regseq DSL's atomic facade:

```rust
use core::sync::atomic::{AtomicU32, Ordering};

fn read_count_bad(counter: &AtomicU32) -> u32 {
    // REJECTED at the bridge by the type-translation phase:
    // AtomicU32 is in the core::sync::atomic::Atomic* type-
    // family rejection list. The bridge raises a diagnostic
    // naming `regseq::Atomic` as the canonical
    // alternative.
    counter.load(Ordering::Acquire)
}
```

The bridge rejection here is structurally:
*type-translation* rejection of the
`core::sync::atomic::Atomic*` type family (the analog of D1.4
Category A applied to types -- `AtomicU32` is rejected at the
point the bridge translates the `&AtomicU32` parameter type),
plus *Call-translation* rejection of methods on those types
(`.load`, `.store`, ...) and of the rustc-internal
`core::intrinsics::atomic_*` family (the analog of D9's
Call-translation rejection of `core::ptr::read_volatile`).
`core::sync::atomic::Ordering` (the value-level enum) is
*not* in the rejection scope -- the diagnostic on the
counter-example arises from the `&AtomicU32` parameter type,
not from the `Ordering::Acquire` argument.

**Observation.** No `Ty` in the model carries an atomic bit.
`Ty::Struct(regseq::Atomic, [u32])` is structurally identical
to any other UnsafeCell-bearing struct; `Ty::Ref(...,
SharedBorrow)` correctly drops `const` via the existing D1.5
machinery. Memory ordering survives in the IR at the
operation/InstanceId level -- exactly where it needs to be for
target-aware codegen. The S0 framing principle holds.

---

## S3. D13 -- Discriminant level: per-variant resolved tag value on `VariantLayout` [LOCKED]

**Decision.** D13 disambiguates two distinct senses of
"discriminant" that the shared `../../discussions.md` D13
entry conflated, and locks each:

- *Discriminant **type*** -- the `repr(<int>)` choice (or
  rustc's default tag-size selection). **Already settled by
  D11**: visible via `TagScalar.scalar_ty: TyId` (resolved
  to `Ty::Int(_)` / `Ty::Uint(_)` / `Ty::Ptr(_, _)` /
  `Ty::Ref(_, _)` per locked D7/D8 plus D11's mapping table at
  `../../vocabulary.md` Section 2.2). D11's table explicitly
  drops `ReprOptions.int` because the resolved scalar type is
  authoritative. D13 carries forward this lock without
  modification.
- *Per-variant discriminant **values*** -- two separate
  layout-level facts, distinguished cleanly here:
  - The Rust-level *semantic discriminant* -- what
    `Discriminant(place)` returns. For `repr(<int>)` enums
    with user-assigned values like `Idle = 0`, `Active = 1`,
    `Error = 0xFF`, `Reset = 0xDEAD`, this is the
    user-assigned value. For non-`repr(<int>)` enums, this
    is the rustc-resolved discriminant value (typically
    aligned with the variant index for default cases).
    Sourced from `adt_def.discriminants(tcx)` (which returns
    a `Discr { val: u128, ty: Ty }` per
    `rustc_middle/src/ty/util.rs:36-40` -- one entry per
    variant, never absent).
  - The layout-level *tag bytes* -- the bit pattern that
    `SetDiscriminant` writes to the tag location. For
    direct-encoded variants this equals the semantic
    discriminant; for niche-encoded niched variants it is
    `(variant_idx - niche_variants.start).wrapping_add(niche_start)`
    (resolved per the D12 formula); for niche-encoded
    untagged variants there is no tag write (the inner
    data's bit pattern *is* the tag); for `EnumShape::Single`
    variants there is no tag scalar at all.
  **Settled here by D13**: `VariantLayout` carries two
  fields, distinguishing the two senses cleanly:

The refined `VariantLayout` schema (extending D11's Section
2.2 declaration by two fields):

```rust
pub struct VariantLayout {
    pub name:         StringId,
    pub fields:       Vec<FieldLayout>,
    pub uninhabited:  bool,
    pub discriminant: u128,           // D13: Rust-level semantic discriminant
                                      //   (what `Discriminant(place)` returns;
                                      //   from adt_def.discriminants(tcx)). Always
                                      //   present for any VariantLayout in any
                                      //   EnumShape (Single or Multiple); the
                                      //   shared D11 schema's EnumShape::Empty
                                      //   carries no VariantLayout, so the field
                                      //   does not apply there.
    pub tag_value:    Option<u128>,   // D13: resolved layout-level tag bytes
                                      //   (what `SetDiscriminant` writes to the
                                      //   tag location). Some(v) when a tag
                                      //   write occurs; None when no tag write
                                      //   is needed.
                                      //   - Some(v) for EnumShape::Multiple
                                      //     direct-encoded variants
                                      //     (v == discriminant)
                                      //   - Some(v) for EnumShape::Multiple
                                      //     niche-encoded *niched* variants
                                      //     (v = niche_start +
                                      //     (variant_idx - niche_variants.start),
                                      //     resolved per D12's formula and
                                      //     baked in)
                                      //   - None for EnumShape::Multiple
                                      //     niche-encoded *untagged* variants
                                      //     (the inner data's bit pattern IS
                                      //     the tag; SetDiscriminant writes
                                      //     the data, not a separate tag)
                                      //   - None for EnumShape::Single
                                      //     variants (no tag scalar exists;
                                      //     the variant is selected by type,
                                      //     not by a tag write).
}
```

`u128` width matches rustc's resolved `Discr.val: u128`
(`rustc_middle/src/ty/util.rs:36-40`), and is sufficient
for any tag scalar up to 128 bits wide. The bit pattern is
unsigned at storage; signed interpretation (for `repr(i*)`
enums) comes from `TagScalar.scalar_ty: TyId` (D11), which
identifies the resolved scalar type and dictates how the
`u128` bit pattern is interpreted at codegen
(sign-extended on read, two's-complement at compare). This
mirrors rustc's own `Discr` shape, where `val: u128` is the
storage and `ty: Ty` carries the signedness.

The cited rustc sources: `rustc_middle/src/ty/mod.rs:1116-1128`
declares `VariantDef.discr: VariantDiscr`;
`rustc_middle/src/ty/mod.rs:1313-1323` defines the
`VariantDiscr` enum (`Explicit(DefId)` /
`Relative(u32)` -- the source-level form, before
evaluation); `rustc_middle/src/ty/util.rs:36-40` declares
the resolved `Discr { val: u128, ty: Ty }` type that
`adt_def.discriminants(tcx)` returns. The bridge resolves
the source-level `VariantDiscr` to the post-eval `Discr`
via the `AdtDef::discriminants(self, tcx)` iterator
(`compiler/rustc_middle/src/ty/adt.rs:583-600`) at
type-construction
time and copies the `val: u128` into the `discriminant`
field.

**Reasoning.**

The shared `../../discussions.md` D13 lean ("layout-level tag
only; the typeck-level discriminant is recoverable from the
layout's tag scalar type") is half-correct: it correctly
disposes of the discriminant *type* question (already settled
by D11), but it misses the per-variant discriminant *values*
question entirely.

Per `../../findings-rustc-draft.md` Theme W.8 (line 775):
*"Discriminant type defaults to `isize`... Typeck discr type;
layout may shrink it."* This text describes the discriminant
*type* relationship (typeck → layout-shrinkable), not the
per-variant discriminant *values*. The values are a separate
concern.

For `Direct` tag encoding with user-assigned discriminants,
variant index and discriminant value are *independent*:

```rust
#[repr(u32)]
enum Status {
    Idle   = 0,        // variant_idx = 0, discriminant = 0
    Active = 1,        // variant_idx = 1, discriminant = 1
    Error  = 0xFF,     // variant_idx = 2, discriminant = 0xFF
    Reset  = 0xDEAD,   // variant_idx = 3, discriminant = 0xDEAD
}
```

Codegen for `SetDiscriminant(Status::Error)` must write
`0xFF` (not `2` -- the variant index) to the tag location.
With D11's locked `VariantLayout` (no `discriminant` field),
codegen has no source of `0xFF` -- the value lives only in
rustc's typeck-level `adt_def.discriminants(tcx)`, which is
post-bridge unavailable. **Without D13, driver code that
uses `repr(<int>)` enums with arbitrary user-assigned tag
values cannot lower correctly.**

This is not a hypothetical pattern. Driver code routinely
uses `repr(<int>)` enums for status codes, error codes, and
register values where the byte value carries domain semantics
(e.g., a UART status register's "ready" bit pattern, an
interrupt controller's vector number, a power-management
state-machine encoding). The pattern is foundational to MMIO
register interface design.

The substantive options for *where* to put per-variant
discriminants:

1. *On `VariantLayout`* (this lock; two fields). Cohesive:
   discriminant data is intrinsic to a variant's layout
   alongside its fields' offsets and the `uninhabited` flag.
2. *On `EnumShape::Multiple` as parallel `Vec<u128>` and
   `Vec<Option<u128>>`*. Same information, less cohesion --
   requires keeping the parallel collections synchronised
   with `variants`.
3. *On the enum's `Item` decl* (stage-12 territory). Crosses
   the stage-3 / stage-12 boundary that D11 deliberately
   respects. Stage-12 owns *decl-level* enum representation;
   stage-3 owns *layout-level* tag bytes, and per-variant
   discriminant data is layout-level (it is the resolved tag
   bytes plus the resolved Rust-level semantic discriminant,
   not the AST-level user-source values).
4. *Recover from variant index + tag encoding.* Fails for
   user-assigned `repr(<int>)` (the original lean).

Option 1 is the cleanest fit:

- *Methodology -- "store the resolved result, not the input."*
  Per-variant discriminants are a resolved-by-rustc decision;
  rustc has already evaluated user-source `= EXPR` clauses,
  applied auto-incrementing where omitted, and produced the
  final values. Storing them mirrors D11's posture for
  field offsets and tag-encoding parameters.
- *Methodology -- "C semantics with information preservation".*
  C codegen for `SetDiscriminant(...)` needs the tag bytes;
  C codegen for `Discriminant(...)` needs the Rust-level
  semantic discriminant. Both must be available post-bridge.
  Storing both honestly preserves what codegen consumes.
- *Methodology -- "don't redo work the rustc compiler has
  already done."* Discriminant evaluation (auto-incrementing,
  signed/unsigned conversion, constant folding, niche-formula
  application) is rustc's responsibility; we copy the answers.

*Why the two-field split.* Combining the two senses into a
single `Option<u128>` (an earlier draft did this) would
conflate `Discriminant(place)` semantics with `SetDiscriminant`
codegen needs:

- For a `repr(u32)` direct-encoded enum, `Discriminant(place)`
  returns the user-assigned value (e.g., `0xFF` for
  `Status::Error`); `SetDiscriminant(Status::Error)` writes
  `0xFF` to the tag location. The two coincide here, so a
  single field suffices for direct-encoded.
- For a niche-encoded enum like `Option<&u32>`,
  `Discriminant(place)` returns rustc's resolved per-variant
  semantic discriminant (typically the variant index for
  default-discriminant enums, e.g., `0` for `None`, `1` for
  `Some`); `SetDiscriminant(None)` writes the niche bit
  pattern (`0` -- the null pointer) to the inner pointer
  field, while `SetDiscriminant(Some(p))` writes `p` itself
  (no separate tag write). The two senses *diverge* here, and
  a single-field design cannot honestly represent both.

The split into `discriminant: u128` (always present;
`Discriminant(place)`) and `tag_value: Option<u128>` (present
when a tag write occurs; `SetDiscriminant`) tracks rustc's
own data model: rustc keeps `adt_def.discriminants(tcx)`
returning a `Discr` for *every* variant
(`rustc_middle/src/ty/adt.rs:583-600`) while the layout-level
tag write is computed separately per `tag_encoding`
(`rustc_abi/src/lib.rs:1890-1897`). Charon's prior-art
similarly splits the two: typeck-level `Variant.discriminant`
plus layout-level `VariantLayout.tag: Option<ScalarValue>`
(`prior-art/charon.md` Theme B lines 397-512, with
`Variant.discriminant` referenced at line 504).

*Coverage across `EnumShape` variants.* The two D13 fields
are scoped uniformly across the locked D11 `EnumShape`
options:

- *`EnumShape::Multiple` direct-encoded*: every variant has
  `discriminant = v` (the resolved `Discr.val: u128`) and
  `tag_value = Some(v)` (equal to the semantic discriminant
  -- direct encoding writes the discriminant verbatim).
- *`EnumShape::Multiple` niche-encoded niched variant*:
  `discriminant = rustc_resolved_value` (the bare `u128` from
  `adt_def.discriminants(tcx)`) and
  `tag_value = Some(niche_bit_pattern)` (per the D12
  formula, baked in).
- *`EnumShape::Multiple` niche-encoded untagged variant*:
  `discriminant = rustc_resolved_value` (rustc gives every
  variant a discriminant, including the untagged one) and
  `tag_value = None` (no separate tag write; the inner
  data's bit pattern serves as the discriminator).
- *`EnumShape::Single`*: `discriminant = v` (the sole
  variant's semantic discriminant, e.g., `42` for
  `repr(u32) enum One { A = 42 }`) and `tag_value = None`
  (no tag scalar exists in the layout; the variant is
  selected by type identity, not by a tag write).
- *`EnumShape::Empty`*: no `VariantLayout` exists at all
  (zero-variant enums have no variants to lay out), so the
  D13 fields do not apply. This was already implicit in
  D11's locked `EnumShape::Empty` shape; D13 confirms.

For niche-encoded enums, the `tag_value` field's `Some(v)`
on niched variants and `None` on the untagged variant
captures the layout-level reality. The redundancy with
`niche_variants` and `niche_start` (D12) is intentional and
load-bearing for different codegen aspects: D12 tells
codegen *where* the tag write goes (into the niched field,
not a separate tag location) and the formula's structure;
D13's `tag_value` carries the resolved bit pattern,
already-computed for direct lookup.

**Methodology rules invoked.**

- *"C semantics with information preservation"* -- C codegen
  for `SetDiscriminant` and `Discriminant` consumes resolved
  tag bytes; we store the bytes.
- *"Don't redo work the rustc compiler has already done"* --
  rustc's `adt_def.discriminants(tcx)` is the authoritative
  source; we copy the answer at bridge time.
- *"Compositional behavior beats special-case flags"* -- two
  fields on `VariantLayout` (`discriminant` and `tag_value`)
  cover all `EnumShape` cases (Multiple direct, Multiple
  niche-niched, Multiple niche-untagged, Single, with Empty
  carrying no `VariantLayout` at all) without
  encoding-mode-specific branches.
- *"No defensive fallbacks"* -- `discriminant: u128`
  admits exactly the resolved value (always present for
  any variant rustc produces); `tag_value: Option<u128>`
  admits exactly the two valid states (tag-write occurs
  vs no tag-write). No sentinel values, no guessing.
- *"Re-derive from evidence; do not import baggage"* -- the
  shared D13 lean ("recoverable from layout's tag scalar
  type") was an over-generalisation that didn't hold up
  against the user-assigned `repr(<int>)` case. D13's lock
  follows the evidence (`adt_def.discriminants(tcx)` exists
  in rustc precisely because variant index ≠ discriminant in
  general).

**Prior-art evidence.**

Across surveyed projects, per-variant discriminant value
storage divides as:

- *Charon* -- carries per-variant resolved tag bytes
  explicitly: `VariantLayout.tag: Option<ScalarValue>` (the
  in-memory discriminant value per variant, with `Option`
  used for the same niche-untagged case D13 names) at
  `prior-art/charon.md` Theme B lines 397-512 (specifically
  the `tag` field at lines 413-415); also a typeck-level
  `Variant.discriminant` at line 504 ("enum discriminant
  representations are encoded in `Variant::discriminant`
  and `DiscriminantLayout`"). Direct precedent for D13's
  Option 1.
- *mrustc* -- carries the discriminant *scheme*
  (`VariantMode::{Linear, Values, NonZero}`) on the layout
  at `prior-art/mrustc.md` Theme B lines 295-319, plus a
  typeck-level `HIR::Enum` carrying `m_tag_repr`. Per-variant
  values are computed from the scheme + variant index plus
  rustc-equivalent typeck data. mrustc's approach implicitly
  splits discriminant *type* / discriminant *values* the
  same way D13 does.
- *rustc_codegen_clr* -- no per-variant discriminant carried
  on the layout; uses CIL's offset-collision-based variant
  encoding (`prior-art/rustc-codegen-clr.md` Theme B lines
  202-212). The CIL emission strategy avoids needing
  per-variant tag values explicitly.
- *rustc-codegen-c, llvm-ir, compcert* -- no per-variant
  discriminant storage observed (LLVM has no first-class
  enum; CompCert's source language has no Rust-style sum
  types; rustc-codegen-c's prior-art coverage is incomplete
  for this layout-level detail).

Rustc itself stores per-variant discriminant values at the
typeck level (`VariantDef.discr: VariantDiscr` at
`rustc_middle/src/ty/adt.rs:159`; accessed via
`adt_def.discriminants(tcx)`). Layout-level
`Variants::Multiple` (`rustc_abi/src/lib.rs:1858-1880`)
does *not* duplicate them on each variant's `LayoutData`;
the per-variant tag value is queried separately from
typeck. Our IR is post-bridge and has no live typeck query
available, so we must materialise the per-variant value at
bridge time -- either onto `VariantLayout` (Charon-style;
D13's lock) or onto a separate stage-12 `Item`-side
collection (option 3 above; rejected).

**Worked examples.** See sections S3.1 and S3.2 below.

### S3.1 Worked example -- `repr(u32)` enum with user-assigned tags (Direct encoding)

**Rust:**

```rust
#[repr(u32)]
pub enum Status {
    Idle   = 0,
    Active = 1,
    Error  = 0xFF,
    Reset  = 0xDEAD,
}

fn classify(s: Status) -> &'static str {
    match s {
        Status::Idle   => "idle",
        Status::Active => "active",
        Status::Error  => "error",
        Status::Reset  => "reset",
    }
}
```

**Layout** (from `tcx.layout_of(Status)` plus
`status_adt_def.discriminants(tcx)`):

```text
size = 4, align = 4, pack = None, uninhabited = false
kind = Enum {
    shape: Multiple {
        tag: TagScalar {
            offset:      0,
            scalar_ty:   Uint(U32),
            valid_range: WrappingRange { start: 0, end: 0xDEAD },
        },
        encoding: Direct,
        variants: [
            VariantLayout { name: "Idle",   fields: [], uninhabited: false, discriminant: 0,      tag_value: Some(0)      },
            VariantLayout { name: "Active", fields: [], uninhabited: false, discriminant: 1,      tag_value: Some(1)      },
            VariantLayout { name: "Error",  fields: [], uninhabited: false, discriminant: 0xFF,   tag_value: Some(0xFF)   },
            VariantLayout { name: "Reset",  fields: [], uninhabited: false, discriminant: 0xDEAD, tag_value: Some(0xDEAD) },
        ],
    },
}
```

For Direct-encoded enums, `discriminant` and `tag_value`
coincide on every variant: the bridge copies `discriminant`
from `status_adt_def.discriminants(tcx)` (the resolved
`Discr.val: u128`), and sets `tag_value = Some(discriminant)`
(direct encoding writes the discriminant verbatim). Variant
index (`0..=3`) and discriminant value (`0`, `1`, `0xFF`,
`0xDEAD`) are independent.

**C lowering:**

```c
typedef struct Status {
    uint32_t tag;
} Status_t;

#define STATUS_IDLE   0u
#define STATUS_ACTIVE 1u
#define STATUS_ERROR  0xFFu
#define STATUS_RESET  0xDEADu

const char *classify(Status_t s) {
    switch (s.tag) {
        case STATUS_IDLE:   return "idle";
        case STATUS_ACTIVE: return "active";
        case STATUS_ERROR:  return "error";
        case STATUS_RESET:  return "reset";
    }
}

// SetDiscriminant(Status::Error) -> place->tag = STATUS_ERROR;
//   codegen reads VariantLayout.tag_value = Some(0xFF)
//   and emits 0xFF as the tag value, NOT 2 (variant index).
// Discriminant(place: &Status) -> reads place->tag and matches
//   against per-variant VariantLayout.discriminant to produce
//   the Rust-level semantic value (which here equals the tag
//   value, since direct encoding).
```

Without `VariantLayout.tag_value`, codegen would have no
basis for emitting `0xFF` -- it would either need to maintain
a separate per-build discriminant table (keyed by enum and
variant name) or re-query rustc (impossible post-bridge).

### S3.2 Worked example -- niche-encoded `Option<&u32>` (untagged variant has `None`)

**Rust:**
```rust
fn f(x: Option<&u32>) -> u32 {
    match x {
        Some(p) => *p,
        None    => 0,
    }
}
```

**Layout** (from `tcx.layout_of(Option<&u32>)`; restated from
the existing Section 4.4 worked example in
`../../vocabulary.md` plus the D13 discriminant fields):

```text
size = 8, align = 8, pack = None, uninhabited = false
kind = Enum {
    shape: Multiple {
        tag: TagScalar {
            offset:      0,
            scalar_ty:   Ptr(U32, Mutability::Not),   // pointer-sized, shape mirrors the inner Ref
            valid_range: WrappingRange { start: 1, end: u64::MAX as u128 },  // null excluded
        },
        encoding: Niche {
            untagged_variant: Some,
            niche_variants:   None..=None,
            niche_start:      0,
        },
        variants: [
            // Variant index 0: None. Niched -- inner data's bit pattern
            //   is forced to the niche (null pointer).
            VariantLayout {
                name:         "None",
                fields:       [],
                uninhabited:  false,
                discriminant: 0,             // semantic discriminant from
                                             //   adt_def.discriminants(tcx): None
                                             //   has discriminant 0 (variant
                                             //   index, since no repr(<int>))
                tag_value:    Some(0),       // resolved tag bit pattern: the
                                             //   niche (null pointer)
            },
            // Variant index 1: Some(p: &u32). Untagged -- the inner pointer
            //   IS the tag (any non-null value identifies this variant).
            VariantLayout {
                name:         "Some",
                fields:       [FieldLayout { name: "0", ty: Ref(U32, Provenance::SharedBorrow), offset: 0 }],
                uninhabited:  false,
                discriminant: 1,             // semantic discriminant from
                                             //   adt_def.discriminants(tcx): Some
                                             //   has discriminant 1
                tag_value:    None,          // no separate tag write occurs;
                                             //   the inner pointer IS the
                                             //   discriminator
            },
        ],
    },
}
```

The split is load-bearing for niche-encoded enums.
`Discriminant(place: &Option<&u32>)` returns the Rust-level
semantic discriminant: `0` for `None`, `1` for `Some` --
codegen reads the inner pointer, matches against the niche
bit pattern (`0`) to identify `None`, and produces
`discriminant` accordingly. `SetDiscriminant(Option::None)`
writes the niche bit pattern (`tag_value = Some(0)`) into
the inner pointer field; `SetDiscriminant(Option::Some(p))`
writes `p` into the inner pointer field (no tag write
occurs; `tag_value = None` records this honestly). The two
fields cleanly distinguish the two senses of "discriminant"
for niche-encoded enums, where they diverge.

**C lowering** (consistent with shared Section 4.4):

```c
typedef struct Option_ref_u32 {
    const uint32_t *p;   // 0 = None; nonzero = Some(p)
} Option_ref_u32_t;

uint32_t f(Option_ref_u32_t x) {
    if (x.p == NULL) return 0;        // None: tag value 0 (the niche)
    return *x.p;                      // Some: any non-null pointer
}

// SetDiscriminant(Option::None)    -> place->p = (const uint32_t *)0;   /* tag_value = Some(0) */
// SetDiscriminant(Option::Some(p)) -> place->p = p;                     /* tag_value = None: data IS the tag */
// Discriminant(place: &Option<&u32>) -> reads place->p; if NULL produce
//   semantic discriminant 0 (None.discriminant); else 1 (Some.discriminant).
```

The two-field shape correctly distinguishes the two senses
of "discriminant" for niche encoding:

- `None` variant: `discriminant = 0` (semantic, from
  `adt_def.discriminants(tcx)`) and `tag_value = Some(0)` (the bit
  pattern to write into the niched field).
- `Some` variant: `discriminant = 1` (semantic) and
  `tag_value = None` (no tag value to write; the data field
  is the tag).

The redundancy with D12's `niche_variants`/`niche_start` is
intentional and load-bearing for different codegen aspects
(see Reasoning above).

---

## S4. D16 -- `c_void` recognition: rejected at bridge in v1 [LOCKED]

**Decision.** The bridge **rejects** `core::ffi::c_void` at
type translation in v1. No `is_c_void` flag on `Item`; no
special `Ty` variant; no acceptance as an ordinary
`Ty::Enum`. Recognition is via
`tcx.is_lang_item(did, LangItem::CVoid)` (per D1.5's locked
recognition-path note); the bridge runs this query at
`Ty::Adt` translation and rejects with a clear diagnostic if
true.

The pinned-toolchain `core::ffi::c_void` is declared with
two hidden `__variant1` / `__variant2` variants
(`library/core/src/ffi/mod.rs:49-76`, `#[unstable(feature =
"c_void_variant", reason = "temporary implementation
detail", issue = "none")]` and `#[doc(hidden)]`)
specifically so that c_void is *inhabited* (not
zero-variant uninhabited). Without recognition, the bridge
would translate c_void as an ordinary 2-variant `Ty::Enum`
with `repr(u8)` -- structurally indistinguishable from
any user-defined `repr(u8) enum Foo { X, Y }`. The C
lowering of such a translation would emit a 1-byte struct
holding the tag, *not* the FFI-canonical `void *` shape
that c_void's lang-item identity entails. The rejection is
motivated by this missing FFI-canonical-lowering decision
combined with D9 / D10 / D1.4 rejection-pattern symmetry
(see Reasoning).

The rejection scope is the `c_void` type itself, which
transitively covers pointers to it (`*mut c_void`,
`*const c_void`) -- the bridge rejects when it encounters
the inner pointee type during pointer translation, not via
a separate raw-pointer rule. The diagnostic recommends the
opaque-struct pattern (`#[repr(C)] struct Opaque { _private:
[u8; 0] }` and `*mut Opaque`) as the canonical alternative
for opaque C-side handles, mirroring D1.4's existing
guidance for `Foreign(_)` rejection.

This is the formal close of the deferral D1.5 / D1.6.c
opened: D1.5 locked "no flag for v1; recognition deferred";
D1.6.c described `c_void` as the `CVoid` lang-item ADT and
forwarded the substantive lock to D16. (D1.6.c's
description of c_void as a "zero-variant enum" reflects an
older toolchain shape; in the pinned toolchain
`core::ffi::c_void` carries two hidden variants per
`library/core/src/ffi/mod.rs:49-76` -- the lang-item
identity is unchanged, but the structural shape is
2-variant inhabited rather than zero-variant uninhabited.
D16 does not need to revisit D1.6.c on this point because
the rejection rests on lang-item identity plus FFI-
lowering policy, not on the structural variant count.)
D16 closes by adding *active rejection* on top of D1.5's
non-recognition, mirroring the D9 / D10 rejection-pattern
posture established for raw `core::ptr::*_volatile` and
`core::sync::atomic::Atomic*`.

**Trigger conditions for re-opening D16 (deferred to
post-v1).**

D16 reopens when *either* of the following materialises:

- *A driver fixture surfaces a stable need for FFI-canonical
  `void *` lowering* -- e.g., a Linux/Zephyr driver passing
  an opaque user-data pointer through a kernel callback that
  insists on `void *` typing in its C signature, where the
  opaque-struct pattern is not interoperable.
- *A stage-13 intrinsic vocabulary entry's signature uses
  `*mut c_void`* -- e.g., a hypothetical
  `regseq_runtime_alloc(size: usize) -> *mut c_void` style
  intrinsic that needs to round-trip through the regseq DSL.

When either materialises, D16 reopens to:

- Lift the bridge rejection.
- Add an `is_c_void` flag on `Item` (analogous to D1.5's
  `is_phantom_data` / `is_unsafe_cell` / `is_manually_drop`
  flags), driven by `tcx.is_lang_item(LangItem::CVoid)`.
- Define the C lowering, distinguishing the two pointer
  mutabilities (per locked D7/D8's `Ty::Ptr(TyId,
  Mutability)` shape; the `Mutability` field drives C
  `const`-derivation per S0):
  `*mut c_void` -> C `void *` (FFI-canonical mutable opaque
  pointer); `*const c_void` -> C `const void *` (FFI-canonical
  read-only opaque pointer). The exact C-spelling for the
  const-qualified case is part of the reopened D16's
  decision; recorded here for completeness so the future
  D16 reopens with both spellings on the table rather than
  defaulting to `void *` for both.

Until that future reopen, the v1 rejection holds.

**Reasoning.**

Three architectural strands carry the decision:

1. *No v1 consumer.* The evidence sweep confirms zero v1
   driver fixtures use `core::ffi::c_void` (checked across
   `tests/fixtures/{pl011-full-spec, minimal-driver,
   log-intrinsic-smoke, property-driver}/` and all
   `crates/*/src/`); zero planned stage-13 intrinsic
   signatures mention it (checked across all
   `stage-N/expectations.md` files,
   `docs/compiler-collateral/`, and
   `docs/discussions/regseq-model/regseq-background.md`).
   Per the methodology rule "ask who consumes it; what shape
   best serves them," there is no v1 consumer to serve.

2. *Symmetric posture with D9 / D10 / D1.4.* All three
   already-locked rejections share the same structure: a
   stable `core::*` API or feature is rejected at the bridge
   because it lies outside the regseq DSL's operational
   surface, with a diagnostic pointing to the canonical
   alternative (D1.4 -> opaque-struct pattern; D9 ->
   `#[rseq::intrinsic("mmio_*")]`; D10 -> `regseq::Atomic<T>`
   facade). D16 fits this pattern exactly: c_void is rejected
   at the bridge; the diagnostic points to the opaque-struct
   pattern (the same alternative D1.4 names for opaque
   C-side handles -- there is no D9/D10-style DSL-defined
   replacement because c_void's role is purely structural,
   not operational).

3. *The non-rejection alternative does not earn its weight.*
   Translating `c_void` as an ordinary `Ty::Enum` -- per
   the pinned toolchain's actual structure: a 2-variant
   `repr(u8)` enum (`__variant1`, `__variant2`,
   `library/core/src/ffi/mod.rs:49-76`) with
   `EnumShape::Multiple { tag: u8, encoding: Direct,
   variants: [...] }` -- admits a type whose stage-13 C
   lowering is structurally indistinguishable from any
   user-defined `repr(u8) enum Foo { X, Y }`: a 1-byte
   tagged struct. That is *not* the FFI-canonical `void *`
   shape that c_void's lang-item identity implies and that
   any real consumer (Linux `void *priv`, Zephyr
   `void *user_data`, FFI signatures interoperating with
   stable C APIs) would require. Stage-13 codegen would
   either need to commit to the structurally-faithful but
   FFI-wrong 1-byte struct lowering (silently broken for
   FFI), or perform lang-item recognition that the bridge
   is not currently doing (an asymmetry the rejection
   posture avoids). Rejecting in v1 avoids committing to
   either path before D16 can reopen with proper recognition
   plus an FFI-canonical pointer-to-c_void lowering decision
   (covering both `*mut c_void` -> C `void *` and
   `*const c_void` -> C `const void *`, per locked D7/D8's
   `Ty::Ptr(TyId, Mutability)` shape and S0's raw-pointer
   `const`-derivation rule).

The decision is consistent with D1.5's locked phrasing
("recognition deferred until intrinsic signatures using
`*mut c_void` motivate it"): D16 hardens "deferred
recognition" into "actively rejected pending recognition,"
which is the only honest interpretation given that an
unrecognised `c_void` cannot be lowered correctly without a
recognition-driven C-lowering decision.

**Methodology rules invoked.**

- *"No defensive fallbacks."* Don't speculate-handle a type
  that has no v1 consumer and that we cannot lower
  correctly without recognition.
- *"Ask who consumes it; what shape best serves them."* No
  consumer in v1; no shape decision to make.
- *"Re-derive from evidence; do not import baggage."* The
  shared `../../discussions.md` D16 lean ("ordinary Adt; no
  flag for v1") was correct on the no-flag question but did
  not address the bridge-translation question. The full lock
  follows from the rejection-pattern symmetry with D9 / D10
  / D1.4 plus the no-consumer evidence.
- *"Compositional behavior beats special-case flags."* The
  rejection mechanism is the same `tcx.is_lang_item(did,
  LangItem::*)`-driven path D1.5 already establishes; no
  c_void-specific machinery is added.
- *"DSL surface coherence"* (the principle articulated in
  the D9 / D10 reasoning): operational and FFI-typed
  `core::*` constructs are rejected at the bridge; the DSL
  surface is its own coherent thing.

**Prior-art evidence.**

The c_void recognition question is essentially a Rust-stdlib
specific concern; surveyed prior-art projects do not directly
address it because their target languages handle FFI
differently:

- *rustc itself* declares `c_void` as a lang-item enum at
  `rustc_hir/src/lang_items.rs:279` (Target::Enum,
  GenericRequirement::None; the Target::Enum tag means "is
  an enum," not "is zero-variant" -- the actual c_void
  declaration in `library/core/src/ffi/mod.rs:49-76` carries
  two hidden `__variant1` / `__variant2` variants),
  accessed via `tcx.is_lang_item(did, LangItem::CVoid)`. Not
  in `AdtFlags` -- per `findings-rustc-draft.md` Theme D.2
  (lines 546-564) and D1.5's locked recognition-path note
  (`../../vocabulary.md` Section 1.6 lines 402-406). Rustc
  itself does not "lower" c_void to anything special at the
  type level; lowering is a downstream codegen concern.
- *Charon* (inference, not direct evidence). The
  `prior-art/charon.md` Theme D entry catalogues which Rust
  marker types Charon treats as ordinary ADTs without
  type-level markers: `UnsafeCell`, `MaybeUninit`,
  `PhantomData`, atomic types, `NonNull`, `Cell`, `RefCell`,
  `Pin`, `ManuallyDrop` (lines 609 and 664-678). The
  evidence file does not specifically name `c_void`. By
  Charon's general posture (`TyKind::Adt(TypeDeclRef)` for
  any nominal ADT, with limited builtins), c_void would
  *inferentially* fall into the same ordinary-ADT bucket;
  this is not directly cited evidence but an extrapolation.
  Charon's verifier-IR posture admits ordinary ADTs because
  Charon operates on abstract type structure rather than
  emitting C; our codegen-target posture has stricter
  emission requirements (the FFI-canonical `void *` shape
  c_void's lang-item identity entails) that an ordinary-ADT
  translation does not satisfy without recognition.
- *mrustc, rustc_codegen_clr, rustc-codegen-c, llvm-ir,
  compcert* do not specifically address c_void recognition
  in their Theme D entries; their lowering paths handle
  opaque types at the C-emission stage rather than via
  type-level lang-item recognition.

The closest in-spec precedent is D1.4's rejection of
`Foreign(_)` (the `extern type T` feature):

- D1.4 rejects `Foreign` at the bridge because it represents
  an unsized opaque FFI type with no v1 lowering shape.
- D1.4's diagnostic recommends the opaque-struct pattern
  (`#[repr(C)] struct T { _private: [u8; 0] }`) as the
  canonical alternative.
- D16 mirrors the rejection mechanism and recommends the
  same alternative; the difference is that c_void is a
  stable `core` API rather than an unstable Rust feature
  (`extern type T`), but the v1 absence of a consumer makes
  the distinction immaterial.

**Worked example.** See section S4.1 below.

### S4.1 Worked example -- rejected `*mut c_void` usage

**Driver source (rejected counter-example):**

```rust
use core::ffi::c_void;

#[rseq::intrinsic("opaque_callback_register")]
fn register_callback(cb: extern "C" fn(*mut c_void), ctx: *mut c_void) {
    todo!("hypothetical -- not in v1 stage-13 intrinsic vocabulary")
}
```

The bridge rejects this driver code at type translation:

- When translating the `*mut c_void` parameter type, the
  bridge encounters the `c_void` ADT.
- The bridge runs `tcx.is_lang_item(c_void_def_id,
  LangItem::CVoid)`; returns `true`.
- D16's rejection fires: the bridge raises a diagnostic
  naming `core::ffi::c_void` as out-of-DSL-scope in v1, and
  recommends the opaque-struct pattern as the canonical
  alternative.

**Suggested rewrite (per D1.4 / D16 diagnostic):**

```rust
#[repr(C)]
struct OpaqueCtx {
    _private: [u8; 0],
}

#[rseq::intrinsic("opaque_callback_register")]
fn register_callback(cb: extern "C" fn(*mut OpaqueCtx), ctx: *mut OpaqueCtx) {
    todo!("hypothetical -- not in v1 stage-13 intrinsic vocabulary")
}
```

The opaque-struct pattern is structurally an `Adt` (an
ordinary `Ty::Struct`) and is supported by every project
that handles ADTs. C lowering of `*mut OpaqueCtx` is a
forward-declared struct pointer (`struct OpaqueCtx *`),
which is a valid opaque-handle shape on both Linux and
Zephyr targets. (Note: the `register_callback` intrinsic
itself is hypothetical -- v1 stage-13 has no such intrinsic
in its closed vocabulary; the example is illustrative of
the type-translation rejection only.)

**Observation.** The rejection is at *type translation*, not
at Call translation: any use of `c_void` -- as a parameter
type, return type, struct field type, generic argument, or
the pointee of a raw pointer -- triggers the rejection. This
is structurally identical to D1.4's `Foreign(_)` rejection
(also type-translation-phase) and contrasts with D9's
`*_volatile` rejection (Call-translation-phase, on the
function call) and D10's mixed rejection (type-translation
for `Atomic*`, Call-translation for methods).

---

## S5. Open-decisions table updates

Replacement entries for `../../vocabulary.md` Section 5 (Open
decisions). Filled per SESSION-C decision as it locks.

| Tag | Subject | Status |
| --- | --- | --- |
| D9 | Volatile per-op vs per-type | **LOCKED** -- no `Ty` bit; volatile is carried on the intrinsic `InstanceId` (no IR-level flag at any level); raw `core::ptr::{read,write}_volatile` rejected at the bridge (Section S1) |
| D10 | Atomics as intrinsics | **LOCKED** -- no `Ty` bit; atomic types are ordinary `Ty::Struct` with the existing `is_unsafe_cell` flag on inner `UnsafeCell<T>`; atomic operations are stage-13 intrinsics on the regseq-DSL `Atomic<T>` facade; bridge rejects `core::sync::atomic::Atomic*` types (type-translation) and their methods + `core::intrinsics::atomic_*` (Call-translation); `core::sync::atomic::Ordering` (value-level enum) explicitly *not* rejected; barrier/fence free functions in `core::sync::atomic` (`fence`, `compiler_fence`) are out of D10's scope and deferred to a future construct decision (Section S2) |
| D13 | Discriminant level (typeck vs layout; per-variant values) | **LOCKED** -- discriminant *type* settled by D11 (`TagScalar.scalar_ty`); per-variant discriminant *values* added via two new fields on `VariantLayout`: `discriminant: u128` (Rust-level semantic discriminant, always present per `adt_def.discriminants(tcx)`) and `tag_value: Option<u128>` (layout-level tag bytes for `SetDiscriminant`; `None` for niche-encoded untagged variants and for `EnumShape::Single` variants). Refines D11's Section 2.2 schema by two fields (Section S3). |
| D16 | `c_void` recognition (rejected at bridge in v1) | **LOCKED** -- bridge rejects `core::ffi::c_void` at type translation in v1 (recognition via `tcx.is_lang_item(did, LangItem::CVoid)` per D1.5); diagnostic recommends opaque-struct pattern (per D1.4 precedent for opaque C-side handles); D1.5's "no flag" lock carries forward unchanged. Reopens when a v1 driver fixture or stage-13 intrinsic surfaces a stable need for FFI-canonical pointer-to-c_void lowering (`*mut c_void` -> `void *`; `*const c_void` -> `const void *`) (Section S4) |

---

## S6. References (session-local)

- Decision discussions (this session's): `discussions-SESSION-C.md`.
- Codex review log (this session's): `review-log-SESSION-C.md`.
- Shared spec (read-only during SESSION-C): `../../vocabulary.md`.
- Shared decision discussions: `../../discussions.md`.
- Methodology rules: `../../../context.md`.
- Stage-3 invariants: `../../memory.md`.
- Stage-3 current state: `../../stage-3-status.md`.
- Inbound forward refs: `../../expectations.md`.
- Primary-source rustc evidence: `../../findings-rustc-draft.md`.
- Per-project prior art: `../../prior-art/<project>.md`.
- DSL three-mechanism convention: `../../../regseq-background.md`
  lines 14-27.
- pl011 fixture (intrinsic-only MMIO usage):
  `tests/fixtures/pl011-full-spec/src/regseq/mmio.rs:35-71`.
- Stage 1 intrinsic recognition (Q7):
  `../../../stage-1/vocabulary.md` lines 869-899; `../../../architecture.md` Q7.
- Stage 13 expectations: `../../../stage-13/expectations.md`.
