**Q1 — has-gaps**

The whitelist is sound, but the rejection rationale should distinguish “invalid to zero” from “valid but intentionally unsupported.”

- Enums: reject. `unsafe { MaybeUninit::<#[repr(u8)] enum E { A = 1 }>::zeroed().assume_init() }` produces discriminant `0`, which is invalid. Rust requires enum discriminants and active variant fields to be valid. Source: Rust Reference invalid values, enum rule lines 140-168: https://doc.rust-lang.org/stable/reference/behavior-considered-undefined.html#invalid-values
- Unions: reject. `union U { nz: NonZeroU32, raw: u32 }`; zeroing then reading `u.nz` produces invalid `NonZeroU32`. The Reference says union validity is still not fully decided, which is enough reason not to encode this contract around unions. Same source lines 174-175.
- `NonZero*` / `NonNull<T>`: reject. `zeroed::<NonZeroU32>()` violates the standard-library valid-range invariant; `zeroed::<NonNull<u8>>()` is null. Source: valid-range rule lines 184-190.
- Function pointers: reject. `zeroed::<fn()>()` is a null function pointer; null `fn` pointers are invalid. Source lines 148-150.
- `Box`, `Rc`, owning pointers: reject. `zeroed::<Box<u32>>()` is null/dangling and violates Box/reference validity; `Rc` also needs an allocation/header. Source lines 177-178.
- `Cell` / `UnsafeCell`: reject, but partly conservative. `UnsafeCell<NonZeroU32>` zeroes to invalid inner state; `UnsafeCell<u32>` is probably value-valid, but it is not `Copy` and interior-mutability semantics are not a plain C field contract. Outer `Copy` check is therefore important.
- `AtomicU32`: conservative, not inherently “zero is invalid.” A zero-valued atomic counter is meaningful, but `AtomicU32` is not `Copy`, has operation semantics the C backend must preserve, and should not sneak through a field-recursive struct rule.
- Raw pointers: conservative. `zeroed::<*mut u8>()` is a null raw pointer and validity-wise initialized, but dereferencing would break; rejecting raw pointers is a semantic policy choice, not a zero-validity necessity. Source: scalar/raw pointer validity line 161.
- Extra categories to name explicitly: references, `fn` pointers, `NonNull<T>`, `!`, unresolved aliases/projections/params, and `char`. `char` zero is valid, but the design list excludes it while saying “rustc primitive scalar kind”; implement the exact list, not “all primitive kinds.”

**Q2 — has-gaps**

`field.ty(tcx, args)` is the right API for generic structs: rustc documents that it returns the field type and that `args` is typically the second field of `TyKind::Adt`. Source: https://doc.rust-lang.org/beta/nightly-rustc/rustc_middle/ty/struct.FieldDef.html lines 99-101. For `Pair<u32>`, the outer type is `TyKind::Adt(Pair, [u32])`; walking `Pair`’s fields and calling `field.ty(tcx, args)` turns each `T` field into `u32`.

The cycle guard as stated is the main flaw. A global visited-set on `DefId` is not sufficient with generics:

```rust
#[derive(Copy, Clone)]
struct Boxed<T>(T);

#[derive(Copy, Clone)]
struct Outer {
    good: Boxed<u32>,
    bad: Boxed<core::num::NonZeroU32>,
}
```

If `Boxed` is memoized/visited only by `DefId`, visiting `Boxed<u32>` first can cause `Boxed<NonZeroU32>` to be skipped. Use an active recursion stack keyed by monomorphized `Ty<'tcx>` or `(DefId, GenericArgsRef)`, not positive memoization by `DefId`.

No field category is inherently missed: struct/tuple fields come from the ADT variant fields; existing repo code already walks ADT fields with `field.ty(tcx, args)` in MIR extraction at [mir_extract.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/extract/mir_extract.rs:500) and [mir_extract.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/extract/mir_extract.rs:536). Associated types are the edge: `field.ty` is not normalized, so `<T as Trait>::Assoc` must be normalized or rejected.

**Q3 — sound**

Foreign field inspection works. The repo’s cross-crate type registration calls `tcx.adt_def(adt_did)` at [cross_crate_types.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/cross_crate_types.rs:218), then walks `non_enum_variant().fields` at [cross_crate_types.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/cross_crate_types.rs:340). rustc documents `non_enum_variant` as returning the unique variant for a struct/union: https://doc.rust-lang.org/beta/nightly-rustc/rustc_middle/ty/struct.AdtDef.html lines 264-267.

No namespace restriction is needed for soundness. The rule should be structural and version-local: if the dependency’s private fields change, acceptance changes at compile time. The real limiter should be backend realizability, not crate identity.

**Q4 — sound**

Use (a): one `Copy` check on the outer Rule-1 field type, plus the recursive whitelist. Current code checks `Option<T>`’s inner type with `type_is_copy_modulo_regions` at [driver.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/driver.rs:227) and defines that helper at [driver.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/driver.rs:562).

Do not drop the outer `Copy` check. Field recursion alone would accidentally accept `UnsafeCell<u32>` as a struct containing `u32`. Recursive `Copy` checks are redundant for acceptance, though useful for diagnostics.

**Q5 — has-gaps**

`TypingEnv::fully_monomorphized()` is correct only after proving the type is actually concrete. rustc documents it as “only suitable for monomorphized, post-typeck environments”: https://doc.rust-lang.org/beta/nightly-rustc/src/rustc_middle/ty/mod.rs.html lines 3285-3291. `type_is_copy_modulo_regions` checks `ty: Copy` under a `TypingEnv`: https://doc.rust-lang.org/beta/nightly-rustc/rustc_middle/ty/struct.TyCtxt.html lines 4293-4298.

Amendment: reject generic driver user-data types containing `TyKind::Param` or unresolved aliases, or normalize associated types with an appropriate post-analysis typing environment before applying the whitelist. The current construction code uses identity instantiation for field types at [driver.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/driver.rs:224), so generic driver structs are not safely covered unless explicitly banned.

**Q6 — has-gaps**

Beyond the listed 5 touch-points, grep found these update sites:

- [construction.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction.rs:146): error docs and display text still say user-data “must be `Option<T>`”; display text is at [construction.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction.rs:467).
- [verify/mod.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/verify/mod.rs:205): verifier error kind is named/documented as `UserDataFieldNotOptionCopy`.
- [driver_struct.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/verify/rules/driver_struct.rs:212): verifier logic still only checks `inner_type` copy and reports “spec requires Option<T>” at [driver_struct.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/verify/rules/driver_struct.rs:239).
- [cross_item.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/verify/rules/cross_item.rs:348): cross-item type resolution visits `u.inner_type`; this must follow the new classified stored type.
- [surface.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/normalize/surface.rs:1044): raw/legacy normalization still enforces `Option<T: Copy>`, and [surface.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/normalize/surface.rs:1081) still stores every user-data field as `Scalar(U32)`.
- [surface.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/normalize/surface.rs:1102): string Copy table currently accepts `u128`, `i128`, and `char`, which diverges from the final whitelist.
- [from_model.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/from_model.rs:190) and [planner.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/planner.rs:311): both lowering pipelines build `ResourceObligations`; both need user-data population.
- [linux/plan.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/plan.rs:400): existing scalar C mapper is property-only and supports only `u8/u16/u32/u64/bool`, not signed ints, `usize`, floats, arrays, tuples, or structs.
- [emit_semantic.rs](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/emit_semantic.rs:216): C type emission panics on unknown structs, so accepted user-defined user-data structs need a backend type strategy.

**Q7 — sound**

Paper framing is concise: “Regseq initializes driver-private state implicitly only for a compiler-proven structural zero-safe subset; all other unseeded state must be explicit as `Option`.” Deferring author opt-in is defensible because it strengthens the paper’s safety story: no user assertion is trusted for initialization validity.

Do clarify that “Option” means the compiler’s lowered representation is initialized to `None`; it must not rely on Rust niche layout unless that Tier is implemented.

**Q8 — sound**

Concrete future gap: `AtomicU32`.

```rust
pub struct D {
    pub irq_count: core::sync::atomic::AtomicU32,
}
```

This is realistic for driver state shared between interrupt and process contexts, but the whitelist rejects it because it is not `Copy` and not a primitive/array/tuple/struct-of-accepted-Copy in the intended sense. This is an acceptable deferral, not a blocker: atomics are a synchronization/codegen contract, not just an initialization contract.

FINAL-DESIGN-VERDICT: adopt-with-amendments

The core design is sound: compiler-controlled structural zero acceptance plus explicit `Option` is defensible and implementable. The remaining issues are implementation-precision issues, not reasons to reopen the trait/attribute debate.

Amendments:
- Key recursion guards by active monomorphized type, not global `DefId`.
- Require an outer `Copy` check for Rule 1 and reject unresolved params/projections unless normalized.
- Make the primitive whitelist exact; do not accidentally accept `char`, `u128`, or `i128` unless the backend/model contract is extended.
- Add a backend-realizability gate or implement C lowering for every accepted Rule-1 shape.
- Update raw normalization, verifier errors, cross-item checks, tests, and both lowering pipelines.

CONVERGENCE: converged
