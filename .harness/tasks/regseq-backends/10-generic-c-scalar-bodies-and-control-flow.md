# regseq-backends-10: Generic-C scalar bodies + control flow (Phase-1 task d1)

## Description

Fill banks 4–5 of the generic-C 5-bank driver (`mod.rs:517-529`, the two
`TODO(task d)` hooks) for the **scalar / pointer / control-flow** subset of
function bodies, and **widen the §15 discovery roots** to signature / local /
static / const types. This is **task d1** of the Phase-1
a→b→c1→c2→**d1**→d2→e sequence — the split of the original "(d) bodies" task
(user-approved 2026-05-27, mirroring the c→c1/c2 split): d1 lowers everything
needed to emit a function whose values are all **scalars and pointers**
(arithmetic, casts, calls, branches, asserts, returns); **d2** adds structured
data (field / downcast / index access, `Aggregate` rvalues, the §13 enum tag
operations, the aggregate / `Bytes` / `EnumValue` `Const` forms, `memcpy`-class
moves). The seam is the natural "scalar SSA + CFG, no aggregate construction or
projection" boundary.

After d1, `emit_generic_c` emits a **complete, host-compilable-and-runnable**
translation unit for any model whose function bodies use only the d1 construct
subset: bank 4 declares every function prototype (Rust- and C-ABI) + every
`extern` static; bank 5 emits the function bodies (§7 envelope, §7.1
terminators, §10/§11/§12/§4/§5/§6 value lowering, §16 panic) + scalar/pointer
static initializers. **This is the first task whose verification *runs* the
emitted program** (not just compiles it): a behavioral test bridges a scalar
Rust function, emits the TU, compiles it with the harness's `void
regseq_main(void)` entry whose lowered body computes a value and `abort()`s on a
wrong result — so a clean exit (code 0) *is* the proof the body lowering
computed correctly. (The first *coverage-ledger* green fixture is still **task
e**: a ledger-green fixture needs the diag-log stdout oracle, which is §8 + the
§7.1 call-site interception — both task e.)

### Scope boundaries (flagged for review — see § Review)

1. **Scalars + pointers + control flow only.** d1 lowers: bare-local & `Deref`
   places (§4); all `Operand` variants (§5); the §10 rvalues `Use`, `Ref` /
   `RawPtr` *to a sized pointee*, `Cast`, `BinaryOp` (the full §10.1 table),
   `UnaryOp::{Not, Neg}`; the §11 casts that don't build a fat pointer or a
   closure shim; the §12 `Const` forms `Bool` / `Char` / `Int` / `Uint` /
   `Float` / `ZST` / `NullPtr` / `Reloc` (Function / Static targets only —
   boundary 6); all 6 §7.1 terminators (ordinary + divergent `Call`, **not**
   diag-log interception); the §16 panic wiring; the §6 intrinsics (`Assume`,
   `CopyNonOverlapping`).
2. **Deferred to d2 (must fail loud, not mis-emit).** `Field` / `Index` /
   `ConstantIndex` / `Subslice` / `Downcast` place projections (§4);
   `Rvalue::{Repeat, Discriminant, Aggregate}` and `UnaryOp::PtrMetadata` (§10);
   `PointerCoercion::{Unsize, ClosureFnPointer}` and any `Ref` / `RawPtr` to an
   **unsized** pointee (§11 / §3.2 fat-pointer construction);
   `StatementKind::SetDiscriminant` (§13); the §12 `Const::{Bytes, Aggregate,
   EnumValue}` forms. Each unsupported construct returns a new
   **`EmitError::Unsupported(&'static str)`** naming the deferred construct
   ([[feedback_no_defensive_fallbacks]] — fail loud; d2 replaces each error arm
   with the real lowering).
3. **Diag-log interception is task e.** d1's `Call` terminator emits the
   **ordinary** call (`Direct` → `name(args…)`, `Indirect` → `(op)(args…)`) and
   the **divergent** call (`target: None` → `<call>; __builtin_unreachable();`).
   A `Direct` callee whose source `item` carries `IntrinsicKind::Log(..)` →
   `EmitError::Unsupported("diag-log call (task e)")` (e replaces this arm with
   the §8 `printf`). d1 **does** implement the §15 **diag-log exclusion** (skip
   a log-marked instance in banks 4/5 — no prototype, no body), so a model
   carrying a log-marked recognition tag still emits a well-formed TU.
4. **No `dyn` / vtable / virtual dispatch.** Only the `trait_obj` shape + VTable
   forward-decl ship (already, from c1/c2). Bank-4/5 handling of `virtual_idx`
   shims, vtable statics, and `Indirect` dispatch through a vtable slot is
   deferred (later task, post-d2).
5. **The bank-1 prelude is already complete** — `regseq_panic`,
   `regseq_unreachable`, the §16 `AssertKind` message `#define`s, the §10.1
   `*WithOverflow` wrappers, and the §11 saturating `FloatToInt` helpers all
   ship from task b (`prelude.rs`). d1 **uses** these at the body sites; it
   emits **no new** prelude/helper definitions.
6. **`Const::Reloc` is Function / Static targets only.** The bridge-validated
   §10.1.7 invariant (the sibling pretty-printer encodes it at
   `pretty.rs:1147-1228` — Function/Static admission at `1158-1218`,
   inadmissible-target rejection at `1220-1226`) admits `Reloc` only to
   `InstanceKind::Function` or
   `Static`; a `Const` / `Struct` / `Enum` / `Union` target is a model-corruption
   bug. d1 lowers Function (`&<emission_name>`) and Static (`&<emission_name>`,
   cast to the consumer pointer `Ty`) targets; an inadmissible target kind is a
   hard error. **Address-taken `const` items** are surfaced by the bridge as
   **synthetic `ConstMemoryStatic`** statics (not `Const` instances) whose `init`
   is a `Bytes` const — so a `Reloc` to one is structurally a `Reloc`-to-Static
   (handled), but emitting *that static's own definition* hits a `Bytes`
   initializer → `EmitError::Unsupported` (d2). No separate d1/d2 `Reloc` split
   is needed at the `Reloc` site.

## Input Context

### Design source — `docs/discussions/regseq-model/generic-c-backend.md`

- **§7 Body-to-C function lowering** (lines 257–314): the function envelope —
  signature from `InstanceKind::Function.fn_sig`; parameter locals
  (`local_decls[1..=arg_count]`) become C formal arguments (not re-declared);
  internal locals (`local_decls[arg_count+1..]`) declared as `local_ty _N;` (or
  the `debug_name` when `Some`); basic blocks emitted as `bb_N:` labels in RPO
  storage order. Statements: `Assign(place, rvalue)` → `<place> = <rvalue>;`;
  `SetDiscriminant` → §13 (**d2**); `Intrinsic` → §6. **Note for d1:** the
  return place `_0` (`LocalId::RETURN_PLACE`, index 0) is **neither** an arg
  **nor** in `[arg_count+1..]`, so it needs its **own** declaration when the
  output `Ty` is non-ZST (a `void` / unit output declares no `_0`).
- **§7.1 Per-terminator C-emission** (lines 316–360): the 6-variant lowering —
  `Goto` → `goto bb_<t>;`; `Return` → `return _0;` / `return;` (unit/void; a
  `Never`-output body is `_Noreturn` and carries no `Return`); `SwitchInt` →
  C `switch` for a ≤64-bit discr, an if-else chain for a 128-bit discr (the
  `SwitchTargets.cases` values are `u128`); `Call` (diag-log interception
  **first** — **task e**; then ordinary `Direct`/`Indirect`; destination = `<dst>
  = <call>;` unless unit/ZST return → `<call>;`; `target: Some(t)` → trailing
  `goto bb_<t>;`; `target: None` → `<call>; __builtin_unreachable();`, no dst;
  Drop-as-`Call` is just a `Direct` call); `Assert` →
  `if ((cond) == expected) goto bb_<target>; else <§16 panic>;`; `Unreachable`
  → the §16 trap.
- **§4 `PlaceElem` C lvalue forms** (lines 190–208): d1 = the bare local +
  `Deref` (`*p`) rows; **`Field` / `Index` / `ConstantIndex` / `Subslice` /
  `Downcast` are d2** (structured access).
- **§5 `Operand` C lowering** (lines 212–228): `Copy(p)` / `Move(p)` → the C
  value-load of `p`'s lvalue (§4) — identical bare-load semantics (`Move` is
  informational at the C level); `Constant(id)` → §12 from
  `model.consts.resolve(id)`.
- **§6 Intrinsic C-emission** (lines 232–253): `Assume(op)` → emit the
  **portable** form `if (!(<op>)) __builtin_unreachable();` (the spec lists
  `__builtin_assume(_N)` for clang **or** the `if (!x) __builtin_unreachable()`
  gcc fallback at lines 239–240; the latter compiles on **both** gcc and clang
  under the Doc-3 host assumption, and the bank-1 prelude adds no `assume`
  macro, so d1 emits the portable form unconditionally);
  `CopyNonOverlapping{src,dst,count}` → `memcpy(dst, src, count * sizeof T)` —
  read the **named** `src` / `dst` fields so the C arg order is correct without
  positional accident (rustc's `(src,dst,count)` vs C's `(dst,src,count)`).
- **§10 `Rvalue` C-emission** + **§10.1 `BinOp`** (lines 820–862): d1 = `Use`,
  `Ref` / `RawPtr` (→ `&<place>`, **sized pointee only**), `Cast` (§11),
  `BinaryOp` (the full table — wrapping arithmetic incl. the `*WithOverflow`
  builtin wrappers returning `(T, bool)`, the masked / arithmetic-vs-logical
  shifts, `Cmp` three-way, `Offset` pointer arith), `UnaryOp::{Not, Neg}`.
  **`Repeat` / `Discriminant` / `Aggregate` / `UnaryOp::PtrMetadata` are d2.**
  The host `-fwrapv` assumption (and the explicit shift masking, which `-fwrapv`
  does not cover) is pinned here.
- **§11 Cast C-emission** (lines 866–888): d1 = `IntToInt` / `FloatToFloat` /
  `IntToFloat` (`(T)x`), `FloatToInt` (the prelude **saturating helper**, never
  a bare cast), `PtrToPtr` / `FnPtrToPtr` (`(T*)p`), `PointerExposeProvenance`
  (`(uintptr_t)p`), `PointerWithExposedProvenance` (`(T*)n`), `Transmute`
  (`memcpy` between equal-`sizeof` objects), `Subtype` (no-op);
  `PointerCoercion::{ReifyFnPointer (→ &fn), UnsafeFnPointer (no-op)}`.
  **`PointerCoercion::{Unsize, ClosureFnPointer}` are d2.**
- **§12 `Const` → C initializers** (lines 892–912): d1 = `Bool` / `Char` /
  `Int` / `Uint` / `Float` (hex-float / `__builtin_inf*` / `__builtin_nan*`
  constant expressions) / `ZST` (no-op / omitted) / `NullPtr(ty)` (`(T*)0`) /
  `Reloc(InstanceRef)` (boundary 6: resolve to the target `InstanceId`; Function
  → `&<emission_name>`, Static → `&<emission_name>` cast to the consumer
  pointer `Ty`). The const lowering is **consumer-`Ty`-aware** where the form
  needs it (`NullPtr` carries its own `ty`; `Reloc` takes the consumer `Ty` from
  the static's declared type or the assignment destination, mirroring the
  pretty-printer's `render_const(..., consumer_ty)`). **`Bytes` / `Aggregate` /
  `EnumValue` are d2.** Every d1 form must materialise both at a value site (§5
  operand) **and** as a static / `const` initializer (`InstanceKind::Static.init`).
- **§15 Declaration order, signatures & C-ABI prototypes** (lines 1020–1088):
  the **discovery-root widening** lives here — bank 4 needs every signature
  type named + (bank-3) complete; bank 5 needs every local / static-init type
  named + complete. **C-ABI** (lines 1075–1080): a position with
  `CAbiShape::Substituted{emit_as}` emits `emit_as`'s C type, `Direct` the
  position's own type — and **"Nested `FnPtr` declarators recurse through the
  same table"** (line 1080), so the declarator spelling is C-ABI-aware at *every*
  fn-pointer level, not just the top-level prototype. `c_variadic` → trailing
  `, ...`. Bank 4 also = `extern` declarations for every static. Bank 5 =
  function bodies + static initializers, F4 order. **Diag-log exclusion**: a
  log-marked instance is skipped in banks 4/5.
- **§16 Host panic & `AssertKind` messages** (lines 1092–1122): `Assert`
  failure + `Unreachable` route to the prelude `regseq_panic` /
  `regseq_unreachable`. Each `AssertKind` → its fixed Rust-style message (the
  prelude `REGSEQ_MSG_*` `#define`s); **operand-bearing** messages
  (`BoundsCheck{len,index}`, `Overflow(op,l,r)`,
  `MisalignedPointerDereference{required,found}`, **and
  `InvalidEnumConstruction(v)`** — it carries an operand,
  `terminator.rs:174`, and the §16 message has a `<v>`) interpolate the
  operand(s) rendered by their `Ty` into the template's `%s` slots, then call
  `regseq_panic`.
- **§17 Symbol & linkage policy** (lines 1126–1155): `Linkage` (`Default` →
  file-scope, no `static`; `NoMangle`/`export_name` → verbatim `emission_name`;
  `Foreign` → `extern` decl only, never defined; `Weak` →
  `__attribute__((weak))`); `link_section` → `__attribute__((section(...)))`;
  `inline_hint` (`Hint`→`inline`, `Always`→`inline __attribute__((always_inline))`,
  `Never`→`__attribute__((noinline))`); `Static{mutability}` (`Not`→`const`,
  `Mut`→non-const; `init: None` → `extern`). `is_unsafe` / `FnSig.safety` are
  **not** lowered.

### Code anchors (confirm line numbers at HEAD)

- `crates/regseq-backends/src/generic_c/mod.rs`:
  - `bank4_declarations` (lines 515–521) + `bank5_definitions` (lines 523–529)
    — the two `TODO(task d)` hooks d1 fills.
  - `discover` (lines 274–287) — **roots only on `InstanceKind::{Struct, Enum,
    Union}` today** (the comment at 270–273 explicitly flags "task d must widen
    the roots"). d1 extends this to also root on function-signature / local /
    static / const types.
  - **`discovery_edges` (150–156) vs `ordering_edges` (168–174)** — the
    existing discovery-vs-ordering split. `discovery_edges` already follows a
    `Node::Slice`'s element for **naming + forward-decl** while `ordering_edges`
    does not. `agg_edges` (131–137) / `containment_target` (236–247) are the
    **by-value (ordering)** edge set and return `None` for a sized `Ref`/`Ptr`
    — the seam Change A broadens for discovery.
  - `dfs_discover` (290–298), `topo_sort` / `topo_visit` (312–340),
    `build_name_table` (344–359) — **reused unchanged**; the widened root set +
    broadened discovery edges flow through the same machinery.
  - `EmitError` (535–550) — `#[non_exhaustive]`; d1 adds the `Unsupported`
    variant (+ its `Display` arm).
- `crates/regseq-backends/src/generic_c/types.rs`: `declare(model, names, ty,
  decl)` / `c_type(model, names, ty)` (lines 64–78) — the C declarator-aware
  spelling d1 reuses for parameters, return types, and local decls.
  `fn_ptr_decl` (261–288) currently spells a `Ty::FnPtr` from the **raw**
  `model.sigs.resolve(sig).inputs/output` — d1 makes it **C-ABI-aware** (read
  `sig_c_abi_lowerings[sig]` per nested position, §15 line 1080), so a nested
  C-ABI fn-pointer parameter/return is spelled with its `emit_as` types. For
  `abi == Rust` (or an all-`Direct` lowering) this is byte-identical to today's
  output (the common case; c1's existing fn-pointer tests stay green).
- `crates/regseq-backends/src/generic_c/prelude.rs`: `regseq_panic` /
  `regseq_unreachable` (lines 48–57), the `REGSEQ_MSG_*` `AssertKind` `#define`s
  (65–72, incl. the `%s` in `REGSEQ_MSG_INVALID_ENUM_CONSTRUCTION` at 72), the
  `regseq_{add,sub,mul}_<T>` overflow wrappers + `struct regseq_overflow_<T>`
  (142–156), the `regseq_<fprefix>_to_<T>` saturating-cast helpers (160–179) —
  d1's **use-site** targets; naming scheme documented at 109–123.
- IR surface (read-only — trust as bridge certificates, no recomputation):
  - `crates/regseq-model/src/body.rs` — `Body { basic_blocks: IndexVec<BlockId,
    BasicBlock>, local_decls: IndexVec<LocalId, LocalDecl>, arg_count, span }`
    (lines 24–42); `LocalDecl { ty, mutability, span, debug_name }` (112–127);
    `return_local` / `arg_locals` / `inner_locals` helpers (49–67).
  - `crates/regseq-model/src/basic_block.rs:36` — `BasicBlock { statements:
    Box<[Statement]>, terminator: Terminator }`.
  - `crates/regseq-model/src/statement.rs` — `StatementKind::{Assign(Place,
    Rvalue), SetDiscriminant(Place, VariantIdx), Intrinsic(NonDivergingIntrinsic)}`
    (34–54); `NonDivergingIntrinsic::{Assume(Operand),
    CopyNonOverlapping(CopyNonOverlapping)}` (64–76); `CopyNonOverlapping`
    fields (85+).
  - `crates/regseq-model/src/terminator.rs` — `TerminatorKind::{Goto, SwitchInt
    {discr, targets}, Return, Unreachable, Call {callee, args, destination,
    target}, Assert {cond, expected, msg, target}}` (64–107); `CallCallee::{
    Direct(InstanceId), Indirect(Operand)}` (113–119); `SwitchTargets {cases:
    Box<[(u128, BlockId)]>, otherwise}` (128–133); `AssertKind` (141–175 — the 8
    variants the §16 table maps; `InvalidEnumConstruction(Operand)` at 173).
  - `crates/regseq-model/src/rvalue.rs` — `Rvalue` (19–75), `CastKind` (77–101),
    `PointerCoercion` (104–112), `BinOp` (117–171), `UnOp::{Not, Neg,
    PtrMetadata}` (174–181), `AggregateKind` (185–222, **d2**).
  - `crates/regseq-model/src/operand.rs:30` — `Operand::{Copy(Place),
    Move(Place), Constant(ConstId)}`.
  - `crates/regseq-model/src/place.rs` — `Place {local: LocalId, projection:
    Box<[PlaceElem]>}` (30–38); `PlaceElem::{Deref, Field, Index,
    ConstantIndex, Subslice, Downcast}` (70–131) + the I-Deref-first /
    I-Downcast-followed-by-Field invariants (56–64).
  - `crates/regseq-model/src/const_value.rs` — `Const::{Bool, Char, Int, Uint,
    Float, ZST, Bytes, Aggregate, EnumValue, NullPtr, Reloc(InstanceRef)}`
    (20–50); `Byte` (69–75, **d2** — only via `Bytes`).
  - `crates/regseq-model/src/instance.rs` — `InstanceKind::{Function(
    FunctionInstanceData), Static{ty, init}, Const{ty, value}, Struct, Enum,
    Union}` (71–108); `FunctionInstanceData {fn_sig: FnSigId, body:
    Option<Body>}` (28–32); `Instance {item: ItemId, emission_name: StringId,
    kind, …}` (134–145); `instance_ref_of(id) -> InstanceRef`
    (`model.rs:1093`).
  - `crates/regseq-model/src/fn_sig.rs:14` — `FnSig {inputs: Vec<TyId>, output:
    TyId, abi: FnAbi, c_variadic: bool, safety}`; `model.sigs.resolve(id)`.
  - `crates/regseq-model/src/cabi.rs` — `CAbiLowering {inputs: Vec<CAbiShape>,
    output: CAbiShape}` (37–41); `CAbiShape::{Direct, Substituted{emit_as:
    TyId, ..}}` (9–22); read via `model.sig_c_abi_lowerings[sig]:
    Option<CAbiLowering>` (`model.rs:97`).
  - `crates/regseq-model/src/item.rs` — `Item {def_path, linkage, link_section,
    inline_hint, …}` (18–45); `Linkage::{Default, NoMangle, Foreign, Weak}`
    (138–147); `InlineHint::{None, Hint, Always, Never}` (232–241).
  - `crates/regseq-model/src/domain_attr.rs` — diag-log recognition tag:
    `DomainAttrTables.item_attrs: IndexVec<ItemId, Vec<AttrData>>` (19);
    `AttrData::Intrinsic(IntrinsicKind)` (64); `IntrinsicKind::Log(LogLevel)`
    (75). Recognition = `model.domain_attrs.item_attrs[instance.item]` contains
    an `AttrData::Intrinsic(IntrinsicKind::Log(_))`.
  - `model.strings.resolve(StringId)` → the `emission_name` string
    (`string_interner.rs:62`); `model.consts.resolve(ConstId) -> &Const`
    (`const_interner.rs:61`).
  - **`InstanceRef` → `InstanceId` resolution + the admissible target kinds**:
    there is no direct table; mirror the sibling pretty-printer's scan —
    `pretty.rs:1292` `find_instance_id_for_ref` iterates instances comparing
    `model.instance_ref_of(id) == target`. `pretty.rs:1147-1228`
    `render_const_reloc` is the reference: `Reloc` resolves **only** to
    `InstanceKind::Function` (→ the fn designator / fn pointer) or `Static` (→
    address, typed cast); `Const` / `Struct` / `Enum` / `Union` targets panic as
    bridge bugs (boundary 6).

### Prior tasks

- `regseq-backends-06`/`08`/`09` (c1/c2/09, done) — banks 2–3 (every sized
  aggregate incl. enums) + the `types::declare` spelling + the §15 discovery /
  topo-sort machinery d1 widens. c2 **explicitly deferred** the §13 tag
  operations to "task d" — they land in **d2**, not d1.
- `regseq-backends-05` (task b, done) — the bank-1 prelude d1 uses unchanged
  (panic / overflow / saturating-cast helpers + AssertKind `#define`s).
- Doc-1 status reconciliation (`111da9b`): the body / terminator / rvalue /
  cast / const / linkage rows d1 **enables** go **green only when a behavioral
  fixture exercises them** — the first ledger-green fixture is **task e**. d1
  flips **no** matrix status and adds **no** coverage-ledger green fixture (its
  behavioral test is a compile-and-**run** check, backend-internal, the same
  home as c1/c2's tests).

### Test surface

- `crates/codegen-coverage-runner/tests/generic_c_layout.rs` — c1/c2's
  behavioral harness (`#![feature(rustc_private)]` + `extern crate
  rustc_driver`; `try_bridge_fixture_path` from `test-ir-fixtures`;
  `toolchain::compile_and_run(tu_c, entry, cc)`; the `TempSnippet` pattern at
  lines 39–61 + the hand-built-model / wrong-offset-injection pattern at
  ~lines 316–390). d1's behavioral test is a sibling file (e.g.
  `generic_c_bodies.rs`) reusing these.
- `crates/codegen-coverage-runner/src/toolchain.rs:60-77` — `generate_main_c`
  emits `extern void <entry>(void); int main(void){ <entry>(); return 0; }`
  and **ignores any return value**; `{i,u}32` exit-code propagation is an
  explicitly **deferred harness feature** (lines 62–66). d1 therefore verifies
  via a `void regseq_main(void)` entry that `abort()`s on a wrong result (no
  harness change). `compile_and_run` returns `RunOutput { stdout, stderr,
  exit_code }` (115–120).
- `crates/regseq-backends/src/generic_c/mod_tests.rs` — the in-crate
  hand-built-model structural tests (the `Instance` / `emission_name`
  construction pattern at 163/261/464/717/878). d1 adds structural + negative
  (d2-construct → `Unsupported`) cases here.
- [[regseq-model-test-surfaces]]: the model's own validator tests are not in
  scope; d1 touches only `regseq-backends` + the runner test.

## Implementation Notes

### Change A: widen the §15 discovery roots + the discovery/ordering pointer split (`generic_c/mod.rs`)

Two coupled edits, both **discovery-only** (naming + bank-2 forward-decl); the
**ordering** edge set (`containment_target` / `ordering_edges` / `topo_sort`)
is **unchanged**, so no false `ContainmentCycle` can be introduced.

**A1 — widen the roots.** Extend `discover` (274–287) so that, **in addition
to** the existing nominal-aggregate roots, it `dfs_discover`s from the discovery
node of the **effective emitted types** of every function / static / const
instance, in `InstanceId` order (the deterministic tie-break), within an
instance in position order:

- **`InstanceKind::Function`**: each **emitted signature position** — for
  `abi == C` with `sig_c_abi_lowerings[sig] = Some(l)`, the per-position
  effective type (`CAbiShape::Substituted{emit_as}` → `emit_as`; `Direct` → the
  `FnSig` position's own type), inputs in order then output; for `abi == Rust`
  (or `None` lowering), the raw `FnSig.inputs` then `output`. **And** every
  `local_decls[i].ty` of the body (`body: Some`), in `LocalId` order (a foreign
  `body: None` roots only on its signature).
- **`InstanceKind::Static{ty, ..}`** / **`InstanceKind::Const{ty, ..}`**: `ty`.

**A2 — follow sized pointer/ref edges in *discovery* (not ordering).** A type
used in a signature / local / static may be (or transitively reach, through a
sized `Ref`/`Ptr`) an aggregate that must be **named + forward-declared** so
`types::declare` can spell it — but `containment_target` returns `None` for a
sized pointer (`mod.rs:236-247`), so the by-value edge set drops it. The
nominal-root safety net (`bridge/mod.rs:2375`) names every *nominal* aggregate
behind a pointer, but **not structural tuples / closures** (they have no nominal
root) — e.g. a `&(u8, u32)` parameter or local would leave its tuple
undiscovered and panic the `NameTable`. So broaden **discovery** (the root
resolver in A1 **and** `discovery_edges` for `Node::Agg`) to also follow a sized
`Ref`/`Ptr` edge to its pointee's discovery `Node` (recursing), exactly as
`discovery_edges` already does for a `Node::Slice` element. This is **naming /
forward-decl only**: a C pointer to an incomplete type is legal, so **no
ordering edge** is added (`ordering_edges` still stops at pointers — the legal
self-reference `struct Inner { xs: &Inner }` stays acyclic). This also closes
the latent c1 gap (a by-value field that is a `&structural-aggregate`); it
preserves c1 output byte-for-byte for models without such through-pointer
structural aggregates (the new edges yield already-discovered or `None` nodes).

Update the `discover` doc-comment (the "task c1's roots are nominal-containment
only…" note at 270–273) to record that d1 widened the roots and broadened
discovery. `topo_sort`, `ordering_edges`, `build_name_table`, and banks 2–3 are
otherwise unchanged; the §15 "complete-before by-value param/return" requirement
(line 1038) is satisfied **structurally by the bank order** (bank 3 emits all
type definitions before banks 4/5).

### Change B: bank 4 — prototypes + extern statics (`generic_c/mod.rs`)

Replace the `bank4_declarations` hook. Emit, in F4 (instance) order:

- **Function prototypes** for every `InstanceKind::Function` instance
  (including foreign `body: None`), **except** a **diag-log-marked** instance
  (§15 exclusion — skip entirely). The prototype is the §17 linkage-qualified
  signature: `<linkage/inline quals> <ret> <emission_name>(<params>);`
  - **Signature spelling.** `emission_name` = `model.strings.resolve(
    instance.emission_name)`. Return / params spelled via `types::declare`,
    reading the **C-ABI effective types** when `abi == C` (`Substituted{emit_as}`
    → `emit_as`; `Direct` → the `FnSig` type) and recursing the C-ABI table
    through **nested `Ty::FnPtr`** positions (Change B's `fn_ptr_decl` edit, §15
    line 1080); raw `FnSig` types for `abi == Rust`. No inputs → `(void)`.
    `c_variadic == true` → trailing `, ...` after the fixed params.
  - **§17 qualifiers.** `Linkage::Foreign` → `extern` (decl only — also no bank
    5 body); `Default` → no `static`; `NoMangle` / `Weak` → verbatim name,
    `Weak` adds `__attribute__((weak))`; `link_section` →
    `__attribute__((section("…")))`; `inline_hint` → `inline` / `inline
    __attribute__((always_inline))` / `__attribute__((noinline))`. A
    `FnSig.output == Ty::Never` function is `_Noreturn`.
- **`extern` static declarations** for every `InstanceKind::Static`: `extern
  [const] <T> <emission_name>;` (`const` iff `Mutability::Not`). Both foreign
  (`init: None`) and locally-defined statics are declared here so a reference
  before the bank-5 definition resolves. `InstanceKind::Const` items emit **no**
  bank-4 declaration (materialised at use sites per §17; an address-taken const
  is a synthetic `ConstMemoryStatic` *Static*, handled by the static path).

### Change C: bank 5 — bodies + static initializers (`generic_c/mod.rs` + the new body module)

Replace the `bank5_definitions` hook. Emit, in F4 order:

- **Function definitions** for every non-foreign (`body: Some`),
  non-diag-log-marked `InstanceKind::Function`: the §17-qualified signature
  (same spelling as bank 4, minus `extern`) + the §7 body (Change D). A foreign
  or log-marked instance emits nothing here.
- **Static definitions** for every `InstanceKind::Static{ty, init: Some(c)}`:
  `[const] <T> <emission_name> = <§12 initializer of c, consumer Ty = ty>;` (the
  d1 `Const` subset; an aggregate / `Bytes` / `EnumValue` initializer →
  `EmitError::Unsupported` — this is where a `Reloc`-to-`ConstMemoryStatic`
  target's own `Bytes` init fails loud, boundary 6). `init: None` (foreign) → no
  definition.

### Change D: the §7 body envelope (`generic_c/body_emit.rs`, new module)

`emit_body(model, names, instance, body) -> Result<String, EmitError>` builds
the function body block:

1. **Locals.** Declare the **return place** `_0` when `FnSig.output` is non-ZST
   (`types::declare(.., output, "_0")` — skip for unit / `Never` / ZST output).
   Declare each **inner local** `local_decls[arg_count+1..]` as
   `types::declare(.., decl.ty, &local_name(decl, id))`, where `local_name`
   uses the sanitised `debug_name` when `Some`, else the synthetic `_N`
   (`N == LocalId`). Argument locals (`1..=arg_count`) are **not** declared (they
   are the C formal parameters). A ZST local declares nothing.
2. **Basic blocks.** For each `(BlockId, BasicBlock)` in `basic_blocks` storage
   order (RPO): emit `bb_<N>: { <statements> <terminator> }`. Statements per
   the dispatch below; terminator per Change E.
3. **Statement dispatch.** `Assign(place, rvalue)` → `<place lvalue §4> =
   <rvalue §10>;` (a ZST destination → the rvalue is emitted for effects /
   elided); `Intrinsic(Assume(op))` → `if (!(<op>)) __builtin_unreachable();`
   (§6, portable form); `Intrinsic(CopyNonOverlapping{src,dst,count})` →
   `memcpy(<dst>, <src>, <count> * sizeof(*<dst>));` (named fields, §6);
   `SetDiscriminant(..)` → `EmitError::Unsupported("SetDiscriminant (task d2)")`.

The place / operand / rvalue / cast / const lowering (§4 / §5 / §10 / §11 /
§12) is the value layer — factor it into the same module or a sibling
`expr_emit.rs` (a senior reviewer should recognise the shape: a `place_lvalue`,
an `operand_expr`, an `rvalue_expr`, a `cast_expr`, a consumer-`Ty`-aware
`const_expr`). Each returns `Result<String, EmitError>` and routes deferred
constructs to `EmitError::Unsupported`.

### Change E: the §7.1 terminators (`generic_c/body_emit.rs`)

- `Goto(t)` → `goto bb_<t>;`.
- `Return` → `return _0;` (non-ZST output) / `return;` (unit / void).
- `SwitchInt{discr, targets}` → C `switch (<discr §5>) { case <v>: goto bb_<t>;
  … default: goto bb_<otherwise>; }` when the discr type is ≤64-bit; an if-else
  chain when the discr type is 128-bit. Case values are the `u128` literals,
  spelled with the discr type's width suffix.
- `Call{callee, args, destination, target}`:
  - **Diag-log guard (first).** `Direct(inst)` whose
    `model.domain_attrs.item_attrs[model.instances[inst].item]` carries
    `AttrData::Intrinsic(IntrinsicKind::Log(_))` →
    `EmitError::Unsupported("diag-log call (task e)")`.
  - **Ordinary call.** `Direct(inst)` → `<name>(<args §5>)` with `name =
    model.strings.resolve(model.instances[inst].emission_name)`; `Indirect(op)`
    → `(<op §5>)(<args §5>)`.
  - **Destination + successor.** unit/ZST return → `<call>;` (no assignment),
    else `<destination §4> = <call>;`. `target: Some(t)` → trailing `goto
    bb_<t>;`; `target: None` (divergent) → `<call>; __builtin_unreachable();`,
    no destination assignment.
- `Assert{cond, expected, msg, target}` → `if ((<cond §5>) == <expected ? 1 :
  0>) goto bb_<target>; else <§16 panic for msg>;` (Change F).
- `Unreachable` → `regseq_unreachable();` (the §16 trap).

### Change F: §16 panic wiring (`generic_c/body_emit.rs`)

The `Assert` failure path calls the **prelude** routines (no new prelude). For a
**fixed-message** `AssertKind` (`OverflowNeg`, `DivisionByZero`,
`RemainderByZero`, `NullPointerDereference`) → `regseq_panic(REGSEQ_MSG_<KIND>);`.
For an **operand-bearing** kind (`BoundsCheck{len,index}`, `Overflow(op,l,r)`,
`MisalignedPointerDereference{required,found}`, **`InvalidEnumConstruction(v)`**)
the site formats the operand(s) (rendered by their `Ty`) into the template's
`%s` slots, then calls `regseq_panic`. The concrete buffering (a fixed stack
`char` buffer + `snprintf` of each numeric operand to a small buffer, then
`snprintf` of the `REGSEQ_MSG_*` template — operands are scalars, widths
bounded) is an implementation detail; the **binding contract** is: stderr
carries the §16 Rust-style message with the operand value(s), and the process
`abort()`s (non-zero exit). `Overflow`'s `%s` slot is the operator **verb**
(`add`/`subtract`/`multiply`/…); the `l`/`r` operands are **not** in the §16
message (`"attempt to <op> with overflow"`), so only the verb is interpolated.

### Change G: `EmitError::Unsupported` (`generic_c/mod.rs`)

Add `Unsupported(&'static str)` to `EmitError` (the enum is `#[non_exhaustive]`)
+ its `Display` arm (`"generic-C emission: unsupported construct: {0} (task
d2)"`). Every deferred-construct site (boundaries 2/3/6) returns it with a
static label. d2 removes each arm as it implements the construct.

## Non-Goals

- **No structured-data lowering (all d2).** No `Field` / `Index` /
  `ConstantIndex` / `Subslice` / `Downcast` place access; no `Rvalue::{Repeat,
  Discriminant, Aggregate}`; no `UnaryOp::PtrMetadata`; no
  `StatementKind::SetDiscriminant`; no §13 tag operations; no `Const::{Bytes,
  Aggregate, EnumValue}`; no fat-pointer **construction**
  (`PointerCoercion::{Unsize, ClosureFnPointer}` / `Ref`-to-unsized).
- **No diag-log interception / §8 printf / no green coverage-ledger fixture**
  (task e). d1 emits the ordinary/divergent `Call` and the §15 diag-log
  *exclusion*; it errors on a diag-log *call site*.
- **No `dyn` / vtable / virtual dispatch** (post-d2): no `virtual_idx` shim
  emission, no vtable static construction, no `Indirect` dispatch through a
  vtable slot.
- **No prelude / helper changes** — bank 1 is complete; d1 only wires use-sites.
- **No discovery *ordering* / `topo_sort` / `build_name_table` / bank-2 / bank-3
  changes** — Change A only **adds roots** and **broadens discovery edges**
  (naming/forward-decl only); the ordering machinery is untouched.
- **No harness change** — d1 verifies through the existing `void
  regseq_main(void)` contract (`toolchain.rs:60-77`); the `{i,u}32` exit-code
  harness feature stays deferred.
- **No layout / C-ABI recomputation** — trust `FnSig`, `sig_c_abi_lowerings`,
  `Body`, `Const`, `Layout` as bridge certificates.
- **No deps; no spec / Doc / bridge / model / matrix / ledger /
  `test-ir-fixtures` changes.** d1 is confined to `crates/regseq-backends/src/
  generic_c/**` + a new runner behavioral test + the `tasks.json` registration.

## Test Plan

### B1: build + workspace green
- `cargo build -p regseq-backends -p codegen-coverage-runner` clean.
- `cargo test -p regseq-backends -p codegen-coverage-runner -- --test-threads=1`
  green — **all c1/c2 tests still pass** (banks 2–3 untouched; the discovery
  broadening preserves their byte output), plus the new d1 tests.

### B2: end-to-end body behavior (the first compile-**and-run** check)
- Bridge a `#![no_std]` snippet whose **`#[no_mangle] pub extern "C" fn
  regseq_main()`** (the harness entry, a `void(void)`) computes a scalar value
  via a helper and **panics on a wrong result** — e.g. `let r = add(2, 3); if r
  != 5 { panic!() }` with `#[no_mangle] pub extern "C" fn add(a: u32, b: u32) ->
  u32 { a.wrapping_add(b) }`. `emit_generic_c` → `compile_and_run(tu, "regseq_
  main", cc)` → assert `exit_code == Some(0)` (the computation was correct; a
  wrong lowering takes the `Assert`/panic path → `abort()` → non-zero). This
  exercises §7 envelope + §7.1 Return/Goto/SwitchInt/Call/Assert + §10
  BinaryOp/Cast + §5 + §4 bare-local + §16 end-to-end **at runtime** (stronger
  than c1/c2's compile-only).
- A fixture whose `regseq_main` deliberately hits a panic path (a failing
  `assert!`) → assert `exit_code != Some(0)` and the §16 message on stderr —
  proving the panic wiring fires.
- Where the bridge cannot surface a shape (a specific `CastKind`, a 128-bit
  `SwitchInt`, a `Foreign` / `Weak` / `link_section` / `inline` linkage, a
  `Reloc` static initializer, a `&(u8,u32)` structural-pointee param), **hand-
  build the model** (the c1 `repr(packed)` precedent) and assert the emitted TU
  compiles + the structural shape.

### B3: bank 4/5 structure + §15 root widening / discovery broadening
- An aggregate used **only** in a signature / local / static type → discovered,
  forward-declared (bank 2), defined (bank 3); the prototype / local decl /
  static decl spell it correctly. A **`&(u8,u32)`** (or `&closure`) signature /
  local → the **structural tuple/closure is named** (A2; would panic without
  the broadening). A C-ABI fn with a `Substituted{emit_as}` position → the
  prototype emits the `emit_as` C type; a **nested C-ABI `FnPtr`** parameter →
  spelled through the nested sig's lowering. A `c_variadic` fn → trailing
  `, ...`.
- §17: `Foreign` → `extern` decl, no definition; `Weak` →
  `__attribute__((weak))`; `link_section` → `section(...)`; `inline` hints; a
  `Never`-output fn → `_Noreturn`; `const`/non-`const` static per mutability.
- Diag-log exclusion: a model carrying a log-marked instance emits **no**
  bank-4 prototype / bank-5 body for it, and the TU is well-formed.

### B4: the d1/d2 boundary fails loud
- A body with a `Field` projection (or an `Aggregate` rvalue, a
  `SetDiscriminant`, a `Const::Aggregate`, a `Ref`-to-unsized, a `Reloc` to a
  `Bytes`-init static) → `emit_generic_c` returns `Err(EmitError::Unsupported(_))`
  (not a panic, not a mis-emit). One negative test per deferred-construct family.

### B5: determinism
- `emit_generic_c` byte-identical across two calls on a body-bearing model
  ([[codegen-ledger-determinism]] — local order, bb order, instance order, the
  widened root order all deterministic).

### B6: fmt + clippy + diff scope
- `cargo fmt --all -- --check` clean; `cargo clippy -p regseq-backends -p
  codegen-coverage-runner --all-targets -- -D warnings` clean (scoped per
  [[clippy-all-targets-test-drift]]).
- Diff confined to `crates/regseq-backends/src/generic_c/**` (the new
  `body_emit.rs` [+ optional `expr_emit.rs`], the `mod.rs` Change-A/B/C/G
  edits + `mod` declarations, the `types.rs` `fn_ptr_decl` C-ABI edit), the
  runner's new behavioral test, and the `tasks.json` registration. **No** spec
  / Doc / bridge / model / matrix / ledger / `test-ir-fixtures` / harness
  change; **no** ledger green fixture / Doc-1 flip.

## Review

### Files to send to Codex
This spec; `docs/discussions/regseq-model/generic-c-backend.md`
§4/§5/§6/§7/§7.1/§10/§11/§12/§15/§16/§17;
`crates/regseq-backends/src/generic_c/{mod.rs, types.rs, prelude.rs}`; the IR
surface `crates/regseq-model/src/{body.rs, basic_block.rs, statement.rs,
terminator.rs, rvalue.rs, operand.rs, place.rs, const_value.rs, fn_sig.rs,
cabi.rs, instance.rs, item.rs, domain_attr.rs}`;
`crates/regseq-model/src/pretty.rs` (`render_const_reloc` /
`find_instance_id_for_ref`, the sibling consumer d1 mirrors for `Reloc`);
`crates/codegen-coverage-runner/src/toolchain.rs` (the `generate_main_c` /
`compile_and_run` contract) +
`crates/codegen-coverage-runner/tests/generic_c_layout.rs` (the harness d1's
behavioral test mirrors).

### Reviewer focus
- **§15 root-widening + discovery/ordering split.** Does A1 (sig C-ABI
  effective / local / static / const roots) + A2 (follow sized pointer edges in
  discovery, recursively; ordering unchanged) discover + name **every** type
  banks 4/5 spell, including a structural tuple/closure behind a pointer that
  the nominal-root net misses? Is "no new ordering edge — bank 3 < bank 4/5
  covers by-value param/return/local completeness" correct, and can A2 introduce
  no false `ContainmentCycle`? Determinism of the widened root order?
- **§7 / §7.1 fidelity.** `_0` declared exactly when output non-ZST?
  Args-as-params vs inner-locals (the `_0`-not-in-`[arg_count+1..]` subtlety)?
  All 6 terminators (divergent `Call` no-dst + `__builtin_unreachable`, unit/ZST
  destination elision, `_Noreturn`, ≤64-bit `switch` vs 128-bit if-else)?
- **§6/§10/§11/§12 value lowering.** Portable `Assume` (`if (!x)
  __builtin_unreachable()`, both gcc+clang)? `CopyNonOverlapping` arg order via
  named fields? BinOp table (shifts, `*WithOverflow` struct, `Cmp`, `Offset`)?
  Cast table (saturating `FloatToInt` helper, `Transmute` memcpy, the d1
  `PointerCoercion` subset)? Const table (Float constant-exprs; `NullPtr`;
  consumer-`Ty`-aware `Reloc` resolved to **Function/Static only**, the
  `ConstMemoryStatic` Bytes case failing loud at the static def)? Is the d2
  deferral line clean and does d2 inherit a clean error seam?
- **§16 panic.** Operand-bearing set correct — incl. **`InvalidEnumConstruction`
  (operand-bearing)**? `Overflow` verb-only interpolation? All via the existing
  prelude `regseq_panic`?
- **§17 linkage.** `Foreign`=decl-only, `Default`=no-`static`, `Weak`,
  `link_section`, `inline_hint`, `Static` mutability — consistent bank 4 ↔ 5?
- **Nested `FnPtr` C-ABI (§15 line 1080).** Does the `fn_ptr_decl` edit recurse
  the C-ABI table per nested position, and stay byte-identical for `abi==Rust` /
  all-`Direct` (c1 fn-pointer tests green)?
- **Diag-log seam (d1 vs e).** Ordinary/divergent `Call` correct, interception
  **deferred** (error), §15 bank-4/5 **exclusion included**?
- **Scope discipline (d1 vs d2 vs e); verification honesty.** Is "bridge a
  scalar fn → emit → compile + **run** a `void regseq_main` that aborts on a
  wrong result → assert exit 0" a sound end-to-end body-lowering proof under the
  existing harness contract (`toolchain.rs:60-77`)? Does the d1/d2 negative test
  prove fail-loud? Is "no green ledger fixture until e" correct?
- Determinism; citation drift vs HEAD; scope creep vs Non-Goals.

### Adversarial prompt template
```
<task>
Review .harness/tasks/regseq-backends/10-generic-c-scalar-bodies-and-control-flow.md
— Phase-1 task d1 of the generic-C backend: §15 discovery-root widening + the
discovery/ordering pointer split + banks 4-5 (prototypes/extern-statics +
function bodies/static-inits) for the SCALAR / POINTER / CONTROL-FLOW subset of
bodies (§7 envelope, §7.1 all 6 terminators, §6/§10/§11/§12/§4/§5 value
lowering, §16 panic, §17 linkage). Structured data (Field/Index/Downcast
access, Aggregate/Repeat/Discriminant rvalues, §13 enum tag-ops,
SetDiscriminant, Bytes/Aggregate/EnumValue consts, fat-pointer construction) is
DEFERRED to d2 and must return a new EmitError::Unsupported (fail loud).
Diag-log call-site interception + §8 printf + the first green coverage-ledger
fixture are task e; d1 emits the ordinary/divergent Call, errors on a diag-log
call, and INCLUDES the §15 bank-4/5 diag-log exclusion. The bank-1 prelude
(panic/overflow/saturating-cast helpers + AssertKind #defines) is already
complete — d1 only wires use-sites. Verification = bridge a scalar fn → emit →
compile AND RUN the harness's void regseq_main(void) which aborts on a wrong
result → assert exit 0 (the {i,u}32 exit-code harness feature is deferred;
toolchain.rs:60-77).
Identify:
- §15 root-widening + discovery/ordering split: A1 roots on signature (C-ABI
  effective via sig_c_abi_lowerings: Substituted{emit_as} vs Direct) / local /
  static / const types; A2 follows sized Ref/Ptr edges in DISCOVERY (naming/
  forward-decl, recursive) but NOT ordering, closing the structural-pointee
  (tuple/closure behind a pointer) gap the nominal-root net misses. Is the "no
  new ordering edge; bank 3 < bank 4/5 covers by-value completeness" claim
  right; can A2 introduce a false ContainmentCycle; is the widened root order
  deterministic?
- §7/§7.1 fidelity: _0 declared iff non-ZST output; args-as-params vs
  inner-locals (_0-not-in-[arg_count+1..]); all 6 terminators incl. divergent
  Call (no dst + __builtin_unreachable), unit/ZST dst elision, _Noreturn,
  ≤64-bit switch vs 128-bit if-else.
- §6/§10/§11/§12 value lowering: portable Assume (if(!x)__builtin_unreachable,
  both gcc+clang); CopyNonOverlapping named-field arg order; BinOp/Cast tables;
  consumer-Ty-aware Const incl. Reloc resolved to Function/Static ONLY (pretty.rs
  §10.1.7) with the ConstMemoryStatic Bytes case failing loud at the static def.
  Clean d2 deferral seam?
- §16 Assert: operand-bearing set incl. InvalidEnumConstruction(v); Overflow
  verb-only; via the existing prelude regseq_panic.
- §17 linkage consistent bank 4 ↔ 5; nested FnPtr C-ABI recursion (§15 line
  1080) in fn_ptr_decl, byte-identical for abi==Rust.
- Diag-log seam; scope discipline d1 vs d2 vs e; verification honesty (compile-
  and-run a void regseq_main that aborts on mismatch is sound; negative test
  proves fail-loud; no ledger green until e).
- Determinism; scope creep vs Non-Goals; citation drift vs HEAD.
Read: docs/discussions/regseq-model/generic-c-backend.md
§4/§5/§6/§7/§7.1/§10/§11/§12/§15/§16/§17;
crates/regseq-backends/src/generic_c/{mod,types,prelude}.rs;
crates/regseq-model/src/{body,basic_block,statement,terminator,rvalue,operand,
place,const_value,fn_sig,cabi,instance,item,domain_attr}.rs + pretty.rs
(render_const_reloc/find_instance_id_for_ref);
crates/codegen-coverage-runner/src/toolchain.rs +
crates/codegen-coverage-runner/tests/generic_c_layout.rs.
</task>

<grounding_rules>
- Cite file:line for every claim. Mark hypotheses as hypotheses.
- Don't speculate about code/spec you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
ROOT-WIDENING (A1 roots + A2 discovery/ordering split names all bank-4/5 types incl. structural pointees; no false cycle; deterministic): PASS / FAIL with citation
ENVELOPE-AND-TERMINATORS (§7/§7.1, _0 decl, divergent Call, switch split, _Noreturn): PASS / FAIL with citation
VALUE-LOWERING (§6/§10/§11/§12: portable Assume; Reloc Function/Static-only consumer-Ty-aware; clean d2 seam): PASS / FAIL with citation
PANIC-AND-LINKAGE (§16 incl. InvalidEnumConstruction operand-bearing; §17 bank4↔bank5; nested FnPtr C-ABI): PASS / FAIL with citation
DIAG-LOG-SEAM (ordinary/divergent Call; interception deferred; bank-4/5 exclusion included): PASS / FAIL with citation
SCOPE-DISCIPLINE (d1 vs d2 vs e): PASS / FAIL with citation
VERIFICATION-HONESTY (void-regseq_main-aborts proof; fail-loud negative; no ledger green until e): PASS / FAIL with citation
DETERMINISM: PASS / FAIL
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
  green (all c1/c2 tests unchanged + the new d1 tests).
- A bridged scalar function emits a TU whose `void regseq_main` exits 0 (correct
  computation) / aborts with the §16 message on a wrong result; the d1/d2-
  boundary negative test returns `EmitError::Unsupported`.
- `emit_generic_c` byte-stable; ledger verdict unchanged (`not-pass`); no Doc-1
  flip.
- `git diff --stat` confined to the Change-A–G file set + the `types.rs`
  `fn_ptr_decl` edit + the runner test + registration.
- Codex Task Review: APPROVED or APPROVED_WITH_MINOR_FIXES.

## Acceptance Criteria

1. **§15 root widening + discovery/ordering split** (`mod.rs`): A1 roots
   additionally on every function-signature (C-ABI effective) / body-local /
   static / const type, in deterministic InstanceId × position order; A2 follows
   sized `Ref`/`Ptr` edges in **discovery only** (recursively — names structural
   tuples/closures behind a pointer), with `ordering_edges` / `topo_sort` /
   `build_name_table` / banks 2–3 unchanged and no new ordering edge / false
   cycle; c1/c2 output preserved.
2. **Bank 4** (`mod.rs`): a §17-qualified prototype for every non-log-marked
   function instance (C-ABI via `sig_c_abi_lowerings`, incl. nested `FnPtr`
   recursion; `c_variadic` → `, ...`; `Foreign` → `extern`/decl-only; `Weak` /
   `link_section` / `inline_hint` / `_Noreturn`) + an `extern [const] T X;` for
   every static.
3. **Bank 5** (`mod.rs` + `body_emit.rs`): a §17-qualified definition + §7 body
   for every non-foreign, non-log-marked function; a `[const] T X = <init>;`
   definition for every `init: Some` static (d1 `Const` subset).
4. **§7 envelope**: return place `_0` declared iff output non-ZST; inner locals
   declared (debug_name or `_N`); args are C params; `bb_N:` labels in storage
   order; the statement dispatch (`Assign` / portable `Assume` /
   `CopyNonOverlapping`).
5. **§7.1 terminators**: all six correct — `Goto`; `Return` (`_0` / void);
   `SwitchInt` (≤64-bit `switch` vs 128-bit if-else); ordinary + divergent
   `Call` (dst elision, `__builtin_unreachable`); `Assert` → §16; `Unreachable`
   → §16 trap. Diag-log call → `Unsupported` (deferred to e).
6. **§6/§10/§11/§12 value lowering** for the d1 subset (places: bare-local +
   `Deref`; operands; `Use`/`Ref`/`RawPtr`-sized/`Cast`/`BinaryOp`/
   `UnaryOp::{Not,Neg}`; the d1 cast + `PointerCoercion` subset; the d1 `Const`
   subset incl. consumer-`Ty`-aware `Reloc` to Function/Static; portable
   `Assume`), using the prelude helpers at the `*WithOverflow` / `FloatToInt`
   sites.
7. **§16 panic** via the existing prelude (`regseq_panic` /
   `regseq_unreachable`; operand-bearing `AssertKind` messages — incl.
   `InvalidEnumConstruction(v)` — interpolated by `Ty`; `Overflow` verb-only).
   **§17 linkage** mapped consistently across banks 4 and 5.
8. **Fail-loud d2 seam**: every deferred construct (boundaries 2/3/6) returns
   `EmitError::Unsupported(&'static str)`; a negative test per family proves it.
9. **Verification**: a bridged scalar function compiles **and runs** —
   `regseq_main` exits 0 iff the lowering computed correctly; determinism holds;
   **no scope creep** — no structured-data lowering, no diag-log interception /
   §8 / green ledger fixture, no `dyn`/vtable, no prelude / discovery-ordering /
   harness / spec / Doc / bridge / model change; diff confined to the
   Quality-Gate file set.
