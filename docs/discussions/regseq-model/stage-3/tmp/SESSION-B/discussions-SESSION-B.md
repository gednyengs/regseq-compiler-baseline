# SESSION-B Addendum to `discussions.md` (D7, D8, D15)

This file holds the SESSION-B contributions to stage 3's `discussions.md`,
in the distilled-decision format used by the existing Section 1 entries
(Question / Key options / Decision / Reasoning / Cross-refs). Entries
below are written for direct lift-in by the merge agent.

Scope:
- D7 / D8 -- replace the existing `[OPEN]` entry (lines 472-494 of the
  current `discussions.md`) with the `[LOCKED]` entry below.
- D15 -- replace the existing `[OPEN]` entry (lines 650-660 of the
  current `discussions.md`) with the `[LOCKED]` entry below.

The pedagogical primer in Section 2 of `discussions.md` does not need
new sub-sections for D7/D8/D15 -- the relevant background (axes of
Rust mutability, ABI / type-identity coupling) is captured inside the
`Reasoning` blocks of the locked entries. The merge agent may, at its
discretion, lift the "three axes of Rust mutability" sub-block from
the D7/D8 Reasoning into a Section 2.6 primer if cross-referencing
across the spec wants a single citation point.

---

## D7 / D8 -- Provenance enum on `Ref`; `Ty::Ptr` shape [LOCKED]

**Question.** What does the provenance information on pointer types
contain, and where does it live? Sub-questions: (a) which variants
appear in the alphabet (`SharedBorrow` / `UniqueBorrow` / `Raw` +
advisory bits?); (b) is the alphabet carried on `Ref` only, or also
on raw pointers; (c) does `Ty::Ref` redundantly carry both
`Mutability` and the new `Provenance` field?

**Key options.**

For the alphabet:
1. Three-variant baseline `{ SharedBorrow, UniqueBorrow, Raw }`,
   used uniformly on both `Ref` and raw pointers (Charon-style).
2. Three-variant baseline plus rustc's advisory bits (`frozen`,
   `unpin`, `global`).
3. Two-variant `{ SharedBorrow, UniqueBorrow }` carried on `Ref`
   only; `Raw` is encoded by the variant choice (raw pointer is a
   distinct `Ty` variant).

For the IR shape of `Ty::Ref`:
A. `Ref(TyId, Mutability, Provenance)` -- both fields, with an
   invariant linking them.
B. `Ref(TyId, Provenance)` -- drop `Mutability`; it is 1:1 redundant
   with `Provenance` for borrows.
C. `Ref(TyId, Mutability)` -- drop `Provenance`; rely on the
   `Mutability` enum.

For the `Ty::RawPtr` variant naming:
i.  Keep `Ty::RawPtr(TyId, Mutability)` -- mirrors rustc's
    `TyKind::RawPtr` 1:1.
ii. Rename to `Ty::Ptr(TyId, Mutability)` -- pairs with `Ty::Ref`
    symmetrically.

**Decision.** Three coupled changes:

- Alphabet: option **3**. `Provenance := { SharedBorrow,
  UniqueBorrow }`. Carried on `Ty::Ref` only. Raw provenance is
  implicit in the `Ty::Ptr` variant choice; advisory bits dropped.
- `Ty::Ref` shape: option **B**. `Ref(TyId, Provenance)`. The
  `Mutability` field is 1:1 redundant with `Provenance` for
  borrows and is dropped.
- Naming: option **ii**. `Ty::RawPtr` is renamed to `Ty::Ptr`.
  The rename is mechanical (~25 sites in our-IR text across
  `vocabulary.md`, `discussions.md`, `review-log.md`); foreign-
  quoted references to `TyKind::RawPtr` (rustc) and `RawPtr`
  (Charon) are unchanged.

The bridge translation rule is forced once the IR shape is locked:

```
TyKind::Ref(_region, ty, Mutability::Not) -> Ty::Ref(translate(ty), SharedBorrow)
TyKind::Ref(_region, ty, Mutability::Mut) -> Ty::Ref(translate(ty), UniqueBorrow)
TyKind::RawPtr(ty, mutbl)                 -> Ty::Ptr(translate(ty), mutbl)
```

The `_region` slot is discarded by the bridge regardless of variant.
Post-erasure it is `ReErased` for free regions (typical case) and
`ReBound` for late-bound regions inside `Binder<FnSig>` /
`Binder<Predicate>` and `Dynamic` existential-predicate scopes
(see `findings-rustc-draft.md:675, 695-697`). No region survives
in `Ty::Ref`.

**Reasoning.**

Rust's mutability information is layered across three independent
axes:
1. *Binding mutability* (`let x` vs `let mut x`) -- property of the
   binding site; lives in MIR `LocalDecl`, not `Ty`.
2. *Reference mutability* (`&T` vs `&mut T`, or `*const T` vs
   `*mut T`) -- carried in the *type*. This is what
   `TyKind::Ref`'s third field and `TyKind::RawPtr`'s second field
   encode.
3. *Data mutability of the original owner* -- partially observable
   through `is_unsafe_cell` on `Item` (interior mutability).

The IR's `Provenance` and `Mutability` fields target axis 2
exclusively. For `Ref`, `Mutability::Not` and
`Provenance::SharedBorrow` are 1:1 synonyms on this axis; storing
both is redundant. We keep `Provenance` because its spelling
encodes the C-codegen-relevant connotation ("is this borrow
unique?") -- exactly the predicate that controls `restrict`
emission. This follows the methodology rule "store the result, not
the input."

For `Ty::Ptr`, axis-2 is the only signal that varies, and raw
pointers always have implicitly-`Raw` provenance with no
uniqueness contract. A `Provenance` field on `Ty::Ptr` would be
single-valued; including it is the kind of redundant flag the
methodology forbids.

The asymmetry between `Ref(_, Provenance)` and `Ptr(_, Mutability)`
is deliberate: `Ref`'s axis-2 is a uniqueness/aliasing contract;
`Ptr`'s axis-2 is a source-level `*const`/`*mut` marker with no
aliasing implication. Charon (option 1) collapses this distinction
to a uniform `RefKind` reused on both kinds; we diverge because
byte-exact C codegen wants the variant-level distinction (the
`restrict` rule fires for borrows only, and the rule is local on
the variant tag).

Advisory bits (option 2) are dropped because no v1 codegen consumer
materializes them: `frozen` would matter only if we added
`restrict` on shared `&T` (we don't); `unpin` matters only when
async/`Pin<P>` is in scope (rejected); `global` matters only for
Box (rejected). Per `rustc_abi/src/lib.rs:2134-2137`, "if the
information is absent, that must always be okay."

The rename `Ty::RawPtr -> Ty::Ptr` is purely cosmetic but pairs
the two variants by name (`Ref` / `Ptr`), which reads more
naturally given the size of the alphabet asymmetry between them.

**Cross-refs.** `vocabulary.md` Section 1.D7-D8 (full audit shape
once merged); Section 1.1 variant table (revised rows for `Ref`,
`Ptr`); Section 4.1 (worked example, replacement); Section 4.1.1
(bridge-translation edge-cases table); Section 5 D7/D8 rows;
`findings-rustc-draft.md` Theme B.10 (PointerKind / PointeeInfo),
Theme A.8 (`TyKind::Ref` / `TyKind::RawPtr`, lines 191-196), Theme
E.6 (post-erasure `Region` for `Ref`, lines 695-697);
`prior-art/charon.md`
Theme A (RefKind reused on both kinds); `prior-art/mrustc.md` (no
per-`TypeRef` flag for interior mutability); `prior-art/llvm-ir.md`
(typeless `ptr`); `prior-art/compcert.md` Theme A (`Tpointer`
attributes). For SESSION-B-local audit trail, `tmp/SESSION-B/
review-log-SESSION-B.md`.

---

## D15 -- fn-pointer ABI subset (`FnAbi` enum) [LOCKED]

**Question.** What ABI vocabulary does our IR carry on function
types? Sub-questions: (a) which variants of rustc's 28-element
`ExternAbi` are accepted; (b) is the `unwind: bool` parameter on
some rustc variants stored or rejected; (c) what is the shape of
the rejection diagnostic for unsupported ABIs; (d) where does the
enum live structurally.

**Key options.**

For the variant set:
1. Tight: `{ Rust, C }`. Reject everything else.
2. Wider: `{ Rust, C, System, Unadjusted }`. Reject the remaining
   24 ExternAbi variants.
3. Driver-portable: `{ Rust, C, System }`. Drop `Unadjusted` (rare
   in driver code).
4. Full mirror: all 28 ExternAbi variants.

For `unwind: bool` handling:
A. Store the flag: `FnAbi::C { unwind: bool }` etc.; codegen
   rejects `unwind: true`.
B. Flatten: `FnAbi::C` (no unwind component); bridge translates
   `C { unwind: false }` -> `FnAbi::C`, rejects `C { unwind: true }`.
C. Add separate variants: `FnAbi::C`, `FnAbi::CUnwind`.

For the rejection diagnostic:
α. Soft fallback to `FnAbi::C` with a warning.
β. Hard reject: `BridgeError::UnsupportedAbi { abi, span }`; refuse
   to translate the function.
γ. Reachability-aware reject: hard for body functions, soft for
   never-called extern declarations.

For the storage location:
I. Embedded in `FnSig`.
II. Separate `FnAbi` table indexed alongside `FnSig`.

**Decision.** Variant set option **1** (tight `{ Rust, C }`);
`unwind` handling option **B** (flatten, reject `unwind: true`);
diagnostic option **β** (hard reject); storage option **I**
(embedded in `FnSig`). The enum lives in the stage-3 type-system
vocabulary; stage 11 embeds `FnSig::abi: FnAbi` (forward note).

The bridge mapping table:

```
ExternAbi::Rust                       ->  FnAbi::Rust  (accept)
ExternAbi::C { unwind: false }        ->  FnAbi::C     (accept)
ExternAbi::C { unwind: true }         ->  reject (BridgeError::UnsupportedAbi)
[all non-Rust / non-C ExternAbi variants] ->  reject (BridgeError::UnsupportedAbi)
```

**Reasoning.**

The `FnAbi` field exists for two distinct reasons with different
stakes:

*Type-identity preservation (soundness).* Rustc treats
`fn() -> u32` and `extern "C" fn() -> u32` as **distinct types**
with no implicit coercion. If our `FnSig` omits the ABI, the
bridge maps two rustc-distinct types to one IR type, the interner
conflates them, and downstream MIR casts between the two become
vacuous. *This forces an `FnAbi` field on `FnSig`.* The variant
set must be at least as wide as the ABIs that appear in code we
accept.

*Codegen and downstream decisions.* Active and near-future
consumers: the pretty-printer (D14, full enum); the future
FFI-validity linter (predicate "is this `FnAbi::C`?"). The
calling-convention attribute emitter (`__attribute__((stdcall))`
etc.) is a no-op for Linux x86-64 because all accepted ABIs
collapse to the platform default at the C level. **Symbol mangling is not a `FnAbi` consumer**: it
is governed by `Item.linkage` (the `Linkage` enum, see
`stage-1/vocabulary.md:77`) and `Instance.symbol_name` (default
`tcx.symbol_name`, see `stage-1/vocabulary.md:670-673`).
`extern "C" fn foo()` without `#[no_mangle]` still receives a
mangled symbol; the ABI does not control the mangled-vs-unmangled
choice.

The minimal variant set is therefore driven by the type-identity
argument: which ABIs appear in code we accept? For Linux v1
driver code, `Rust` (default) and `C` (FFI to kernel APIs) cover
the realistic surface. `System` resolves to `C` on Linux,
`Unadjusted` is compiler-internal, `RustCall` is closure-trait
territory (D6). Picking the tight set over the wider lean
({Rust, C, System, Unadjusted}) is the methodology-cleanest move:
"expand vocabulary lazily," "reject what shouldn't appear; never
speculate-handle." Adding `System`/`Unadjusted` later is a one-
line enum extension plus a matching bridge entry.

`unwind: true` (rustc's `extern "C-unwind"` variant) is rejected
because it represents a runtime capability drivers don't have:
Linux/Windows kernels run abort-on-panic semantics; cross-FFI
unwinding is meaningless. Mapping `C { unwind: true } -> FnAbi::C`
would silently strip type-system information -- a defensive
fallback. Hard rejection (option β) surfaces the problem at the
bridge with the source span; if a future use case for cross-FFI
unwinding materializes, lift the enum to include a `CUnwind`
variant or parameterize `C { unwind: bool }`.

Reachability-aware rejection (option γ) is appealing pragmatically
(don't fail the whole crate over an unused extern decl) but adds
reachability analysis the bridge doesn't otherwise need at this
stage. Defer to a stage-11 refinement if the v1 ergonomics
demand it.

Charon's choice to drop the ABI entirely (`prior-art/charon.md`
line 1143-1148: "Calling conventions, including extern \"C\",
must be recovered (or are lost)") is incompatible with our type-
identity posture. Charon's verifier-IR can abstract over calling
conventions; we cannot, because rustc-distinct types must remain
distinct in our model. mrustc's choice to keep the ABI as a
string (`m_abi: RcString`, included in type-equality at
`prior-art/mrustc.md:430`) confirms the soundness-driven design
is the conventional choice for direct C-targeting Rust compilers.

**Cross-refs.** `vocabulary.md` Section 1.D15 (full audit shape
once merged); Section 5 D15 row; `findings-rustc-draft.md` Theme
A.9 (full ExternAbi enumeration, lines 198-230);
`prior-art/charon.md` Theme F (FunSig drops ABI);
`prior-art/mrustc.md` (m_abi as string, included in type
equality); `prior-art/llvm-ir.md` (calling convention attribute,
not part of FunctionType identity); `stage-1/vocabulary.md` line
76 (`Abi` field reservation on `FnSig`, deferred to stage 11);
`tmp/SESSION-B/review-log-SESSION-B.md` (SESSION-B audit trail).
