# regseq-backends-06: Generic-C type system + field-offset emission (Phase-1 task c1)

## Description

Fill banks 2–3 of the generic-C 5-bank driver (`regseq-backends-05`): the
**type system foundation** — the `Ty`→C type-name/spelling function (§2 +
the D1 naming rules), the **§14/D10 field-offset aggregate emitter** (the
shared primitive: sort-by-offset + explicit padding + `_Static_assert`
guards), the bank-2/3 **forward-decls + definitions** with the §15 by-value-
containment **topo-sort**, and the remaining §3.2 fat-pointer struct shapes
(`rust_slice_<T>` + the `trait_obj_<Trait>` shape). This is **task c1** of
the Phase-1 a→b→**c1**→c2→d→e sequence (the original "(c) types + layout"
was split — c2 is enum representation §13/D9, the flagged top-risk item,
reusing this task's D10 primitive).

After this task, `emit_generic_c` emits compilable C type definitions for
**sized, non-enum, non-`dyn`** aggregates (struct / union / tuple / closure /
array / slice / primitives), each with `_Static_assert(sizeof/_Alignof/
offsetof)` layout guards — so compiling the emitted TU *is itself* the proof
that the lowering reproduces rustc's layout. **No green codegen-coverage
fixture is produced** (a green fixture needs a function body + diag-log →
tasks d/e); c1's verification is backend-internal (bridge an aggregate
snippet → emit → the TU compiles, i.e. the layout guards hold), the same
pattern task b used.

### Scope boundaries (flagged for review — see § Review)

1. **Enum *bodies* are deferred to c2.** c1's `Ty`→C **spelling** handles
   `Ty::Enum` (→ `struct <name>`, so a non-enum aggregate may *name* an enum
   field type), and the discovery/topo-sort treats enums as graph nodes, but
   c1 emits **no enum definition** — `InstanceKind::Enum` is the c2 hook. A
   model containing an enum will not fully compile until c2; **c1 is verified
   on enum-free models**.
2. **`dyn` / vtable construction + virtual dispatch are deferred** (a later
   task). c1 emits the §3.2 `trait_obj_<Trait>` **two-slot struct shape** +
   a **forward-decl** of `<Trait>_VTable`, but **not** the VTable definition,
   the vtable static instances, or virtual dispatch (those are
   instance/body/§9.6 work). c1 is verified on non-`dyn` models.
3. **Field *access* (`.fname` / `memcpy`, §14 item 7) is task d.** c1 emits
   the type *definitions* (offsets, padding, guards); the place/operand
   expressions that *read/write* fields are use-site lowering (task d/§4).
4. **Type names are *derived*, instances keep `emission_name`** (D1). c1
   implements the D1 type-name derivation (`sanitize(DefPath.rendered)` +
   `__<hex>` `ArgsKey` suffix; tuple/closure/field names; the `_clash_<n>`
   safety net). Instance symbols (fn/static/const `emission_name`) are
   unchanged — those are bank 4/5 (task d).

## Input Context

### Design source — `docs/discussions/regseq-model/generic-c-backend.md`

- **§2 Per-`Ty` C lowering table** (lines 63–95): the C spelling of each of
  the 19 `Ty` variants. c1 implements the spelling for **all** variants
  (used by d/e for locals/signatures/fields), emitting **bodies** only for
  the in-scope aggregates (struct/union/tuple/closure); `Enum`→`struct
  <name>` spelling but no body (c2); `Dynamic`-behind-ref→`trait_obj_<Trait>`
  shape (no vtable, deferred).
- **§3.2 Fat-pointer C struct shapes** (lines 116–145): `rust_slice_<T>
  { const T *data; size_t len; }` (per element `T`, mutability per the
  table) and `trait_obj_<Trait> { void *data; const <Trait>_VTable
  *vtable; }` (the §3.3 `void *` data slot). `rust_str` already shipped in
  bank 1 (task b). The per-`T`/per-`Trait` structs are bank-2/3 here.
- **§14 Aggregate field-offset reproduction** (lines 959–986) + **Doc 2 D10**
  (`02-architecture-decisions.md:491-528`): the 7-step technique — (1)
  sort emittable fields by `FieldLayout.offset`; (2) **omit ZST** fields
  (`is_phantom_data` / unit / ZST) but account for them; (3) explicit
  `unsigned char _padN[k];` fillers between fields + trailing pad to
  `layout.size`; (4) alignment — force the aggregate's C **type** alignment
  to `layout.align` via the GNU type attribute
  `__attribute__((aligned(<layout.align>)))` (**`_Alignas` is NOT valid on a
  `struct`/`union` tag definition** — it specifies objects/members, not a
  type; use the GNU attribute), adding `packed` when `pack.is_some()` →
  `__attribute__((packed, aligned(<layout.align>)))`. **The alignment is
  always `layout.align`, never `pack`'s `n`** — `pack` only sets `packed`;
  the §14 guard asserts `_Alignof(T)==layout.align`; (5) **unions** =
  all-offset-0 → C `union` + trailing pad; (6)
  **(Doc-override note.)** This **intentionally refines** the illustrative
  §14 / Doc-2 D10 wording, which lists `_Alignas(layout.align)` and `pack:
  Some(n) → packed + aligned(n)`. `_Alignas` is invalid on a `struct`/`union`
  tag, and `aligned(n)` is a **latent bug**: under `repr(packed(n))` the
  type's true alignment is `min(max_field_align, n) = layout.align`, which
  can be `< n` — so `aligned(n)` would over-align and the
  `_Static_assert(_Alignof==layout.align)` guard would fail the build. The
  task uses `__attribute__((aligned(layout.align)))` (always `layout.align`).
  **Flag a follow-up doc fix** to §14 + Doc-2 D10 to match; the guard makes
  the implementation self-correcting regardless.
  `_Static_assert(sizeof==size)`, `_Static_assert(_Alignof==align)`,
  per-field `_Static_assert(offsetof(T,f)==offset)`; (7) field access is
  **task d** (not here).
- **§15 Declaration order** (lines 988–1044): **bank 2** = tag forward-decls
  (`struct <T>;` / `union <T>;`) for every aggregate in F4 order; **bank 3** =
  full definitions, **topologically sorted by by-value containment** — the
  edge set is pinned at §15: `T→U` iff `U` is the resolved `Ty` of a
  `FieldLayout.ty` (struct/union/tuple/enum-payload/closure-capture field)
  **or** the element of a `Ty::Array` reached that way; **no** edge through
  `Ref`/`Ptr`/`FnPtr` (the *pointee* is forward-declarable) or `FnDef` (ZST).
  **Fat-pointer refinement (Change E):** a `Ref`/`Ptr` to an *unsized*
  pointee (`Str`/`Slice`/`Dynamic`) does **not** lower to a pointer — it
  lowers to a *by-value* fat struct (§3.2), so a field/element of that type
  adds an edge to the **fat struct** (`rust_slice_<T>` / `trait_obj_<Trait>`;
  `rust_str` lives in bank 1, always complete) — **not** to the pointee. A
  cycle = model corruption (hard error). F4/kind-rank order is the
  deterministic tie-breaker. c1 fills banks 2–3 for the in-scope aggregates;
  the enum-body branch of bank 3 is the c2 hook.
- **D1 Naming** (`02-architecture-decisions.md:39-77`): instances use
  `Instance.emission_name`; **aggregate type names** are derived —
  `sanitize(DefPath.rendered)` (`::`→`_`, non-`[A-Za-z0-9_]`→`_`) +
  `generic_suffix(ArgsKey)` (`__<hex>` for generic items, empty otherwise);
  tuples `tuple_<hash-of-field-TyIds>` with fields `_0`,`_1`,…; closures use
  their capture-struct name; fields use `sanitize(FieldLayout.name)`; a final
  `_clash_<n>` pass is the safety net.

### Code anchors (IR surface — confirm line numbers at HEAD)

- `crates/regseq-model/src/layout.rs` — `Layout { size, align, pack:
  Option<u32>, uninhabited, kind }`; `LayoutKind::{Struct{fields},
  Union{fields}, Enum{shape}}`; `FieldLayout { name: StringId, ty: TyId,
  offset: u64 }` (**source/MIR order; `offset` absolute to the aggregate**).
  The §14 emitter consumes these directly.
- `crates/regseq-model/src/instance.rs:71` — `InstanceKind::{Struct{layout:
  TyId}, Enum{layout}, Union{layout}, Function, Static, Const}`; `Instance {
  item: ItemId, generic_args: GenericArgs, emission_name: StringId, kind }`
  (HEAD stores the args list `generic_args`, **not** a precomputed key —
  derive the `ArgsKey` for naming via
  `args_key_for_generic_args(model, &inst.generic_args)`,
  `crates/regseq-model/src/args_key.rs`).
  **Nominal aggregates are `InstanceKind::{Struct,Union,Enum}` instances**,
  each `layout` deref'ing to a `Ty::{Struct,Union,Enum}{layout}` in the
  interner. c1 emits a definition per in-scope (non-enum) such instance.
- `crates/regseq-model/src/ty.rs` — the 19-variant `Ty` (`Struct{item,args,
  layout}`, `Union{..}`, `Enum{..}`, `Tuple{fields,layout}`, `Closure(item,
  args)`, `Array(TyId,u64)`, `Slice(TyId)`, `Ref/Ptr/Str/Dynamic`, scalars,
  `FnPtr/FnDef/Never/Bool/Char`). Tuples/closures are **structural**
  (interner, no instance) — discovered via by-value containment from the
  nominal instances.
- `crates/regseq-model/src/item.rs:18` — `Item { def_path: DefPath, kind }`;
  `DefPath.rendered: StringId` + `.segments` (the type-name derivation
  input). Reach via `model.items[instance.item].def_path`.
- `crates/regseq-model/src/model.rs` — `model.types: TyInterner` (l.72),
  `model.instances: IndexVec<InstanceId, Instance>` (l.100),
  `model.size_align_of(TyId) -> SizeAlign` (l.1831). The string interner for
  `StringId`→`&str` (confirm the accessor).
- `crates/regseq-backends/src/generic_c/mod.rs` — the bank-2/3 hooks
  (`bank2_type_forward_decls`, `bank3_type_definitions`) c1 fills, and
  `prelude.rs` (the bank-1 `rust_str` + helpers from task b).

### Prior tasks

- `regseq-backends-05` (`5bfdcb8`, done): the 5-bank driver + bank-1 prelude.
  c1 fills banks 2–3; the driver assembly + determinism contract are set.
- Doc-1 status reconciliation (`111da9b`): the matrix rows c1 will *enable*
  (TY-17 struct ref, IN-9 struct layout, IN-11 union, LY-2/3/6 field offsets,
  the tuple/closure/array/slice rows) go **green only when a behavioral
  fixture exercises them** (d/e). c1 flips **no** matrix status (the status
  column is the spec axis, stable) and adds **no** green fixture.

## Implementation Notes

### Change A: the naming module (`generic_c/naming.rs`, D1)

A pure naming layer (used by the spelling + the emitters):

- `type_name(model, ty_id) -> String` for aggregates: `sanitize(DefPath.
  rendered) + generic_suffix(args_key)` where `args_key =
  args_key_for_generic_args(model, &inst.generic_args)`. Resolve the `DefPath` via
  the owning `InstanceKind::{Struct,Union,Enum}` instance (or, for a
  structural type reached by containment, via its `Ty::{Struct,..}.item`).
- tuples → `tuple_<hash>` (hash of the field `TyId`s, stable); closures →
  the parallel capture-struct name; fields → `sanitize(FieldLayout.name)`.
- `sanitize`: `::`→`_`, any non-`[A-Za-z0-9_]`→`_`; total + deterministic.
- A final `_clash_<n>` disambiguation pass over the assembled name table
  (expected no-op given `(DefPath, ArgsKey)` identity; ship it as the safety
  net + a test that it fires on a synthetic clash).

Names are a **pure function of the model** (no counters seeded by iteration
order) → byte-stable ([[codegen-ledger-determinism]]).

### Change B: `Ty`→C spelling (`generic_c/types.rs`, §2/§3)

`c_type(model, ty_id) -> String` — the C spelling for a value of `ty`, the
function d/e use for locals/params/returns/fields. Per §2:
scalars→`intN_t`/`uintN_t`/`_Bool`/`uint32_t`/float types; `Ref/Ptr` to a
**sized** pointee → `const T*`/`T*` (§3.1, `contains_unsafe_cell` /
mutability per §3.1); `Str`/`Slice`/`Dynamic` behind `Ref/Ptr` → the §3.2 fat
struct (`struct rust_str` / `struct rust_slice_<T>` / `struct
trait_obj_<Trait>`); `Array(T,n)`→`T[n]` (field/local context); `FnPtr`→the
function-pointer declarator; `FnDef`→ZST (no value emission); aggregates →
`struct`/`union <name>` (via Change A). `Ty::Enum` → `struct <name>`
(spelling only; **body is c2**). Declarator placement (arrays,
function-pointers) follows C declarator syntax — pin a small
declarator helper rather than naive `"<type> <name>"` concatenation.

### Change C: the §14/D10 field-offset aggregate emitter (`generic_c/layout_emit.rs`)

The **shared primitive** (c2 reuses it for enum payloads). Given an
aggregate's `Vec<FieldLayout>` + `Layout`, emit a C `struct`/`union`
definition by the §14 7-step technique: sort by `offset`; omit ZST fields
(`is_phantom_data` via the item flag / unit / size-0) accounting for them;
insert `unsigned char _padN[k];` fillers + trailing pad to `size`; force the
C type alignment to `layout.align` via `__attribute__((aligned(<layout.align>)))`
(**not** `_Alignas`, invalid on a tag definition), adding `packed` when
`pack.is_some()` → `__attribute__((packed, aligned(<layout.align>)))` (the
alignment is always `layout.align`, never `pack`'s `n`); unions = all-zero
offsets → C `union`; emit `_Static_assert(sizeof(T)==size)`,
`_Static_assert(_Alignof(T)==align)`, per-field
`_Static_assert(offsetof(T,f)==offset)`. The static-asserts make the emitted
TU **self-checking**: a wrong offset fails the compile (stage 4). Field
*types* come from Change B; field *names* from Change A. (ZST-field
detection needs the field type's size — use `model.size_align_of`.)

### Change D: fat-pointer struct shapes (`generic_c/types.rs`, §3.2/§3.3)

Emit, when reached: `struct rust_slice_<T> { const T *data; size_t len; };`
(per element `T`; mutability variation per the §3.2 table) and `struct
trait_obj_<Trait> { void *data; const <Trait>_VTable *vtable; };` (§3.3
`void *` data slot) **plus a forward-decl** `struct <Trait>_VTable;`. The
VTable *definition* + vtable statics + virtual dispatch are **deferred**
(Scope decision 2) — `trait_obj` references the forward-declared VTable, so
a `dyn`-using model does not fully compile until that later task.

### Change E: discovery + topo-sort + bank-2/3 wiring (`generic_c/mod.rs`)

- **Discovery roots (c1 = nominal-containment only)**: c1's roots are the
  `InstanceKind::{Struct,Union,Enum}` instances; the set to emit = those
  **plus** the structural aggregates (tuples, closure capture structs,
  `rust_slice_<T>` / `trait_obj_<Trait>` fat structs) reachable by **by-value
  containment** from them. This is **complete for c1**, which emits no
  function signatures, locals, or static types. **Structural aggregates used
  *only* in a signature / local / static type (not contained in any nominal
  aggregate) are NOT roots in c1 — task d MUST widen the discovery roots** to
  include signature/local/static types when it adds banks 4/5 (those types
  do not exist in the emitted TU until then). State this hand-off explicitly
  so task d does not assume c1 already rooted them.
- **Topo-sort** (§15): order definitions so a type is emitted after every
  type it contains by value; edges per the §15 set — through `FieldLayout.ty`
  + `Array` element; **not** through `Ref`/`Ptr`/`FnPtr`/`FnDef` (the pointee
  is forward-declarable). **Fat-pointer edge:** a field/element of type
  `Ref`/`Ptr` to an **unsized** pointee (`Str`/`Slice`/`Dynamic`) is a
  by-value fat struct (§3.2), so it adds an edge to that **fat struct**
  (`rust_slice_<T>` / `trait_obj_<Trait>`; `rust_str` is bank-1, always
  complete) — and discovers it. The fat struct itself holds *pointers*
  (`T*`), so it has **no** onward by-value edge to `T`. F4 / kind-rank order
  is the deterministic tie-breaker. A by-value **cycle** is a hard
  `EmitError` (model corruption — add the variant; `#[non_exhaustive]`
  allows it).
- **Bank 2** (`bank2_type_forward_decls`): `struct <T>;` for struct / tuple /
  closure / enum, `union <T>;` for unions — for **every** discovered
  aggregate (incl. enums, so c2's bodies have their forward-decls).
- **Bank 3** (`bank3_type_definitions`): the §14 emitter (Change C) for each
  in-scope **non-enum** aggregate in topo order; **`InstanceKind::Enum` →
  the c2 hook** (c1: emit nothing / a `// TODO(task c2)` marker; the forward-
  decl from bank 2 stands). Determinism: discovery + sort are total + stable.

### Change F: tests

- **Structural** (`regseq-backends`): emit for a hand-built or bridged model
  with a `repr(Rust)` multi-field struct, a tuple, a union, an array field,
  and a `&[T]` slice; assert the TU contains the expected `struct`/`union`
  defs with sorted fields, `_padN` fillers, `__attribute__((aligned(...)))`,
  and the three `_Static_assert` forms; assert byte-stability across two
  emits; assert the
  `_clash_<n>` net fires on a synthetic name clash.
- **Behavioral** (`codegen-coverage-runner` tests, reusing
  `toolchain::compile_and_run`): bridge small snippets exercising
  `repr(Rust)` reordering, a `#[repr(packed)]` struct, a union, a tuple, and
  an array; emit; **compile the TU + a trivial `regseq_main`**; assert it
  compiles **clean** (the `_Static_assert` layout guards passing *is* the
  proof that offsets reproduce rustc's layout). A deliberately-wrong-offset
  unit test (inject a bad offset into a hand-built layout) asserts the
  emitted `_Static_assert` **fails** the compile — proving the guards bite.

## Non-Goals

- **No enum definitions** (`InstanceKind::Enum` body) — task c2 (§13/D9),
  reusing Change C. c1 only forward-decls enums + spells `struct <name>`.
- **No `dyn`/vtable construction, vtable statics, or virtual dispatch** — a
  later task. c1 emits only the `trait_obj` shape + VTable forward-decl.
- **No field-access lowering** (`.fname` / `memcpy`, §14 item 7) — task d.
- **No bank 4/5** (prototypes, `extern` statics, function bodies, static
  initializers) — tasks d. c1 leaves those hooks empty.
- **No green codegen-coverage fixture / no Doc-1 status flip** — a green
  fixture needs a body + diag-log (d/e). c1's verification is
  backend-internal (compile + `_Static_assert`).
- **No layout recomputation** — the backend trusts `Layout`/`FieldLayout` as
  bridge certificates ([[regseq-model test surfaces]] / HANDOFF: walk the IR,
  don't recompute). `size_align_of` is read-only.
- **No deps added**; **no harness changes** (reuse `toolchain` from tests).
- Do not modify the spec, Docs 1–3, the bridge, the model, or
  `test-ir-fixtures`.

## Test Plan

### B1: build + workspace green
- `cargo build -p regseq-backends -p codegen-coverage-runner` clean.
- `cargo test -p regseq-backends -p codegen-coverage-runner -- --test-threads=1`
  green (existing 35 tests unchanged + the new c1 tests).

### B2: layout fidelity (Change F)
- Structural: `repr(Rust)` struct → fields sorted by offset, `_padN`
  fillers, `__attribute__((aligned(...)))`, three `_Static_assert` forms;
  union → C `union`;
  tuple → `tuple_<hash>` with `_0`/`_1`; array field → `T[n]`; `&[T]` →
  `struct rust_slice_<T>`.
- Behavioral (via `toolchain`): bridged `repr(Rust)` / `repr(packed)` /
  union / tuple / array snippets → emit → **TU compiles clean** (layout
  guards hold). The wrong-offset injection test → compile **fails** on the
  `_Static_assert` (guards bite).

### B3: naming + determinism
- D1 type names = `sanitize(DefPath)+__<hex>`; tuple/closure/field names per
  D1; `_clash_<n>` net fires on a synthetic clash; `emit_generic_c`
  byte-identical across two calls.

### B4: scope + ledger
- Ledger verdict unchanged (`not-pass`); no green fixture, no Doc-1 flip.
- `InstanceKind::Enum` / `dyn` models: c1 forward-decls + spells but emits no
  body (a test asserts the enum hook is empty + a `dyn` model emits the
  `trait_obj` shape + VTable forward-decl).

### B5: fmt + clippy
- `cargo fmt --all -- --check` clean; `cargo clippy -p regseq-backends -p
  codegen-coverage-runner --all-targets -- -D warnings` clean (scoped per
  [[clippy-all-targets-test-drift]]).

### B6: diff scope
Confined to `crates/regseq-backends/src/generic_c/**` (new `naming.rs` /
`types.rs` / `layout_emit.rs` + `mod.rs` bank-2/3 wiring), the runner's new
behavioral test, and the registration. No spec/doc/bridge/model/matrix/ledger
changes.

## Review

### Files to send to Codex
- This spec; `generic-c-backend.md` §2/§3/§14/§15; Doc 2 D1/D10 (+ D2 decl
  order); `regseq-model/src/{layout.rs, instance.rs, ty.rs, item.rs}`;
  `regseq-backends/src/generic_c/{mod.rs, prelude.rs}`.

### Reviewer focus
- **Scope discipline (c1 vs c2 vs d)**: enum bodies → c2; `dyn`/vtable/
  dispatch → later; field access → d. Is the split clean — does Change C's
  D10 primitive generalize to enum payloads (so c2 truly reuses it)?
- **The 4 scope decisions** (Description). AGREE / propose alternative.
- **§14/D10 fidelity**: sort-by-offset, ZST omission + accounting, padding
  math (inter-field + trailing), `pack` → `packed`/`aligned`, union all-zero,
  the three `_Static_assert` forms. Any way offsets fail to reproduce?
- **Discovery + topo-sort (§15 edge set)**: does the containment walk
  discover all by-value-reachable structural aggregates (tuples, closure
  capture structs, slices)? Is the cycle = hard-error correct? Is the
  tie-break deterministic?
- **Naming (D1)**: is deriving type names from `DefPath`+`ArgsKey` (not the
  instance `emission_name`) correct, and collision-free given the identity
  key? Is the `_clash_<n>` net well-placed?
- **Determinism**: pure function of the model (no iteration-order leakage in
  discovery/naming/sort)?
- **Verification honesty**: is "compile + `_Static_assert` holds" a sound
  proof of layout fidelity absent a behavioral fixture? Does the
  wrong-offset test genuinely prove the guards bite?
- Citation drift vs HEAD.

### Adversarial prompt template
```
<task>
Review .harness/tasks/regseq-backends/06-generic-c-types-and-layout.md —
Phase-1 task c1 of the generic-C backend: the type-system foundation (Ty->C
spelling + D1 naming + the §14/D10 field-offset aggregate emitter + the §15
bank-2/3 forward-decls/definitions topo-sort + the §3.2 rust_slice/trait_obj
shapes). Enum BODIES (§13/D9) are split out to task c2 (reusing this task's
D10 primitive); dyn/vtable/dispatch and field-access are later tasks. No
green fixture (needs a body+diag-log, d/e); verification is backend-internal
(emit aggregate defs -> compile -> the _Static_assert layout guards hold).
Identify:
- Scope discipline (c1 vs c2-enum vs d-bodies/access vs deferred-dyn).
- Soundness of the 4 scope decisions.
- §14/D10 fidelity (sort-by-offset, ZST omission, padding math, pack,
  union all-zero, the 3 _Static_assert forms) — any way offsets mis-reproduce?
- Discovery + §15 topo-sort correctness (by-value edge set; structural-type
  discovery; cycle=hard-error; deterministic tie-break).
- D1 naming (DefPath+ArgsKey type names, collision-freedom, _clash net).
- Determinism (pure function of the model).
- Whether "compile + _Static_assert holds" soundly proves layout fidelity,
  and the wrong-offset test proves the guards bite.
- Scope creep vs Non-Goals; citation drift vs HEAD.
Read: generic-c-backend.md §2/§3/§14/§15; Doc 2 D1/D10/D2;
regseq-model/src/{layout,instance,ty,item}.rs;
regseq-backends/src/generic_c/{mod,prelude}.rs.
</task>

<grounding_rules>
- Cite file:line for every claim. Mark hypotheses as hypotheses.
- Don't speculate about code/spec you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
SCOPE-DISCIPLINE (c1 vs c2/d/dyn): PASS / FAIL with citation
SCOPE-DECISIONS: (1)/(2)/(3)/(4) AGREE / DISAGREE_<rationale>
D10-FIDELITY (§14 field-offset): PASS / FAIL with citation
DISCOVERY+TOPOSORT (§15): PASS / FAIL with citation
NAMING (D1): PASS / FAIL with citation
DETERMINISM: PASS / FAIL
VERIFICATION-HONESTY: PASS / FAIL with citation
SCOPE-CREEP: PASS / FAIL
CITATION DRIFT: list or "none"
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate
- `cargo build -p regseq-backends -p codegen-coverage-runner` clean.
- `cargo fmt --all -- --check` + `cargo clippy -p regseq-backends -p
  codegen-coverage-runner --all-targets -- -D warnings` clean.
- `cargo test -p regseq-backends -p codegen-coverage-runner -- --test-threads=1`
  green (existing + new c1 tests).
- Bridged aggregate snippets emit a TU that compiles clean (layout guards
  hold); the wrong-offset injection test fails the compile on the
  `_Static_assert`.
- `emit_generic_c` byte-stable; ledger verdict unchanged (`not-pass`).
- `git diff --stat` confined to the Change-A–F file set + registration.
- Codex Task Review: APPROVED or APPROVED_WITH_MINOR_FIXES.

## Acceptance Criteria

1. **`Ty`→C spelling** (`types.rs`): all 19 variants spelled per §2/§3
   (aggregates → `struct`/`union <name>`; `Str`/`Slice`/`Dynamic`-behind-ref
   → the §3.2 fat structs; `Array`→`T[n]`; `FnPtr` declarator; `Enum`→`struct
   <name>` spelling only).
2. **D1 naming** (`naming.rs`): aggregate type names = `sanitize(DefPath)` +
   `__<hex>` `ArgsKey` suffix; tuple/closure/field names per D1; `_clash_<n>`
   safety net present + tested.
3. **§14/D10 emitter** (`layout_emit.rs`): struct/union/tuple/closure
   definitions with sorted fields, ZST omission, explicit `_padN` padding +
   trailing pad, `__attribute__((aligned(layout.align)))` (+ `packed` when
   `pack.is_some()`; alignment always `layout.align`, never `pack`'s `n`),
   and the three
   `_Static_assert` guards.
4. **§3.2 fat structs**: `rust_slice_<T>` emitted per element type;
   `trait_obj_<Trait>` shape + `<Trait>_VTable` forward-decl emitted (no
   VTable body / vtable static / dispatch).
5. **Banks 2–3**: forward-decls for every discovered aggregate; non-enum
   definitions emitted in §15 by-value-containment topo order; a by-value
   cycle is a hard `EmitError`; `InstanceKind::Enum` → empty c2 hook.
6. **Layout fidelity proven**: bridged `repr(Rust)`/`repr(packed)`/union/
   tuple/array snippets emit a TU that compiles clean (guards hold); the
   wrong-offset injection test fails the compile on the `_Static_assert`.
7. **Determinism**: `emit_generic_c` byte-identical across two calls.
8. **No scope creep**: no enum body, no `dyn`/vtable/dispatch, no field
   access, no bank 4/5, no green fixture, no Doc-1 flip; ledger verdict
   unchanged; diff confined to the Quality-Gate file set.
