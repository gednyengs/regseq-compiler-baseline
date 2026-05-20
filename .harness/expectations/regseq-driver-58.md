# regseq-driver-58: implementation evidence

Implementation deliverable for `regseq-driver-58` (Changes A-J of
the task spec at
`.harness/tasks/regseq-driver/58-eager-hir-enumeration-of-source-top-level-items.md`).
Records before/after evidence for the Item-enumeration asymmetry
that surfaced during manual playground stress-testing.

## Scope

Eager `tcx.hir_crate_items` walks plus a four-bucket
Instance-coverage policy on the bridge-completeness validator.
Together they realize the spec contract that "every source-defined
`struct` / `enum` / `union` / `static` / `const` / `fn` at the
source crate's top-level surfaces as a corresponding `Instance`",
plus the spec.md section 9.4 carve-outs for foreign declarations
and non-rigid signatures.

## Playground fixture (manual reproduction case)

`tests/fixtures/playground/` is the user's experimentation space.
This expectations file references but does NOT modify any files
under that directory. Per the task spec the playground is exercised
manually by recompiling
(`cargo build -p regseq-driver -p cargo-regseq` then
`cargo regseq --target-os linux --platform-file platform.toml --emit
model`) and inspecting `regseq-out/model.txt`.

### Before-fix evidence

Per the task spec Description (cited from
`.harness/tasks/regseq-driver/58-eager-hir-enumeration-of-source-top-level-items.md`):

- `tests/fixtures/playground/regseq-out/model.txt` after
  regseq-driver-55 + -57 carried `entry`, `Reg::REG_0`, and
  `Reg::REG_1` -- but NOT the source `Reg` Struct itself and NOT the
  `Reg::new` inherent method. The asymmetry traced to
  `crates/regseq-driver/src/bridge/mod.rs:717-738` enumerating
  `Const | AssocConst` eagerly while ADTs (Struct / Enum / Union)
  and Functions were interned only reactively (via `Ty::Adt(_)` /
  `MonoItem::Fn` reachability respectively). Reviewers (manual +
  Codex) flagged the asymmetry SIGNIFICANT.

### After-fix expected enumeration

Expected slot enumeration in the rebuilt `regseq-out/model.txt`
once regseq-driver-58 lands, for the `struct Reg(u32) + impl Reg {
const REG_0; const REG_1; fn new(off) -> Reg } + pub fn entry()`
fixture:

- `struct playground::Reg(u32);` (or the canonical pretty-printer
  shape `struct playground::Reg { 0: u32 }` -- the existing
  pretty-printer rule for tuple structs is unchanged by this task).
- `const playground::{impl#0}::REG_0: u32 = 0_u32;`
- `const playground::{impl#0}::REG_1: u32 = 1_u32;`
- `fn playground::{impl#0}::new(_1: u32) -> ::Reg { ... }` with
  full body.
- `fn playground::entry() -> () { ... }` with full body.
- Total source Item count = 5; total source Instance count = 5.

## Test-fixture regression guard (normative)

The normative regression guards land alongside this fix:

- `crates/regseq-driver/tests/bridge_eager_hir_enumeration.rs` --
  bridges fixtures shaped like the playground stress fixture and
  asserts:
  - B1: every source-derived top-level item lands in `model.items`
    with the right `ItemKind`, `Linkage::Default`, and
    `BridgeItemFlags { is_generic: false, signature_non_rigid:
    false }`. Covers all six ItemKind variants in one bridge run.
  - B2: each bucket-1 Item has EXACTLY one matching Instance;
    layouts / fn_sigs are non-empty.
  - B3: foreign Function and Static Items surface with
    `Linkage::Foreign` and produce exactly one Instance with
    `body: None` / `init: None` per spec.md section 9.4
    (`spec.md:2642-2648` + `:2668-2677`).
  - B4: generic Items (`Slot<T>`, `Either<L, R>`, `Bag<T: Copy>`,
    `id<T>`) carry `is_generic == true` and have ZERO Instances
    when no monomorphisation is reached (bucket 3).
  - B5: RPIT functions (`pub fn make_counter() -> impl Fn(u32) ->
    u32`) carry `signature_non_rigid == true` and have ZERO
    Instances (bucket 4).
- `crates/regseq-model/tests/validate_bridge_completeness_buckets.rs` --
  hand-constructed-model coverage of the four-bucket validator:
  bucket-1 zero-Instance reject, bucket-1 two-Instance reject,
  bucket-2 zero-Instance reject, bucket-2 body/init-not-None
  reject, bucket-3 zero / two-Instance accept, bucket-4 zero-
  Instance accept, bucket-4 one-Instance reject.

## Out of scope (per task Non-Goals)

- Pretty-printer rendering shapes are unchanged. All four bucket
  outputs reuse the existing shapes per `pretty-printer.md`
  Specimen 120 (foreign static) and the established Function /
  Struct / Enum / Union / Static / Const renderers.
- No new `ItemKind` or `InstanceKind` variants. The six-variant
  closed alphabet at `docs/discussions/regseq-model/spec.md:2205-2230`
  stays intact; bucket 4 surfaces as an Item-without-Instance.
- Rule 3 emission policy unchanged: pretty-printer iterates
  `model.instances` only, so Item-only entries (buckets 3, 4)
  remain invisible in rendered output. This preserves the
  pretty-printer.md §4249 guarantee.
- `tests/fixtures/playground/` is not modified. Playground is the
  user's experimentation space; manual verification recompiles +
  inspects without modification.
- regseq-driver-56 call-site canonicalization
  (`canonicalize_call_site_callee`) and regseq-driver-57 diag_log
  recognizer cleanup are untouched.
- `Linkage::Foreign` is preserved as the canonical signal for
  bucket 2; the `#[foreign]` attribute rendering rule at
  `pretty-printer.md:3640-3698` (and Specimen 120 at
  `pretty-printer.md:2236-2244`) carries the foreign decls.
- Layout-of-generic-ADT-with-zero-monos is out of scope per Rule 2
  bucket 3: generic ADTs without concrete monomorphisations have
  zero Instances and are not laid out.
