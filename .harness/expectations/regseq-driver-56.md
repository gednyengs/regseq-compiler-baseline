# regseq-driver-56: implementation evidence

Implementation deliverable for `regseq-driver-56` (Change C of the
task spec at
`.harness/tasks/regseq-driver/56-direct-call-callee-static-dispatch-resolution.md`).
Records before/after evidence for the static-dispatch trait-method
callee canonicalization fix.

## Scope

Direct call sites whose MIR operand is
`Operand::Constant(FnDef(def_id, generic_args))` where rustc's
`ty::Instance::try_resolve(.., fully_monomorphized, def_id,
generic_args)` returns `InstanceKind::Item(resolved_def_id)`. The
fix canonicalizes the callee Item to the resolved `(resolved_def_id,
instance.args)` so trait-declaration DefIds no longer leak into
`model.items` / `model.instances` as phantom body-less Function
Instances. `Virtual` (dyn-Trait) and shim flavors retain their
pre-fix `(def_id, generic_args)` keying per spec section 8.1 (the
amended Implementation Note added by Change B at
`docs/discussions/regseq-model/spec.md`).

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
`.harness/tasks/regseq-driver/56-direct-call-callee-static-dispatch-resolution.md`):

- `tests/fixtures/playground/regseq-out/model.txt:38` exhibited a
  phantom body-less Function Instance
  `fn playground::GAdd::add(_1, _2) -> Point;`. The DefPath traces
  to the trait DECLARATION `GAdd::add`, not the impl method
  `{impl#1}::add`. This violated spec.md section 8.1
  (`spec.md:2341-2350` -- trait declarations are erased) and
  section 9.4 (`spec.md:2595-2606` -- `body: None` reserved for
  foreign declarations).
- `tests/fixtures/playground/regseq-out/model.txt:51` showed the
  call site inside `pub fn add` rendering
  `playground::GAdd::add<playground::Point>(...)` (trait DefPath
  + Self substitution) instead of the resolved impl DefPath.

### After-fix evidence

Expected slot enumeration in the rebuilt `regseq-out/model.txt`:

- Exactly THREE Function Instances + ONE Struct Instance:
  - `{impl#0}::new` -- the inherent impl constructor.
  - `{impl#1}::add` -- the trait impl method (full body).
  - `playground::add` -- the top-level caller (full body).
  - `playground::Point` -- the struct Instance.
- The phantom `fn playground::GAdd::add(...);` Function Instance
  is GONE.
- The call site inside `pub fn add` renders
  `playground::{impl#1}::add(move _3, move _4)` (or the equivalent
  canonical impl DefPath produced by `tcx.def_path_str`).
- The total Function Instance count drops by 1 (3 instead of 4).

## Test-fixture regression guard (normative)

The new `test-ir-fixtures` fixture at
`crates/test-ir-fixtures/fixtures/trait_method_resolution.rs`
mirrors the playground's trait + impl + caller shape:

```rust
pub trait T { fn m(&self) -> u32; }
pub struct S;
impl T for S { fn m(&self) -> u32 { 42 } }
pub fn caller(s: &S) -> u32 { s.m() }
```

The normative regression guard is the driver-side integration test
at `crates/regseq-model/tests/e2e_trait_method_resolution.rs` plus
its checked-in golden snapshot at
`crates/regseq-model/tests/snapshots/e2e_trait_method_resolution__render.snap`.
The snapshot pins the rendered form (`{impl#0}::m(...)` at the call
site, no `T::m(...);` body-less declaration).

Additional structural assertions in
`crates/regseq-driver/tests/bridge_trait_method_resolution.rs`
(Test Plan B1 + B2) cover:

- Exactly two Function Instances (impl method + caller); zero
  Function Instances with `body == None` for source-defined fns.
- No source-derived Function Item is keyed on a trait DECLARATION
  DefId (`tcx.trait_of_assoc(_) == Some(_) &&
  tcx.impl_of_assoc(_) == None`).
- The caller body's `Terminator::Call` resolves to the impl-method
  Instance (terminal `m` segment under a `{impl#N}` ancestor).
- Pretty-printer rendering contains `{impl#...}::m(` at the call
  site and contains no `T::m(...);` body-less declaration.

## Out of scope (per task Non-Goals)

- The bare-Ty `TyKind::FnDef` arm at
  `crates/regseq-driver/src/bridge/ty.rs:128-133` is unchanged;
  non-call sites that reach `translate_ty` with a `FnDef` value
  continue to intern the original DefId. Those sites do not produce
  Function Instances and are invisible in rendered output per
  `pretty-printer.md` §10.3.
- Foreign declarations (`extern "C"`) remain SPEC-AMBIGUOUS per the
  regseq-driver-55 Description.
- The `validate_bridge_completeness` Function carve-out at
  `crates/regseq-model/src/model.rs:3442-3486` is untouched
  (preserves the regseq-driver-55-v2 RPIT-uncalled-fn carve-out).
- The dyn-Trait Virtual call-site path is preserved unchanged: the
  pre-rewrite Direct Virtual Function Instance keyed on the trait
  method's DefPathHash still carries `flavor = Virtual` +
  `virtual_idx = Some(_)` so the stage-11 vtable-indirect rewrite
  (`spec.md` §23.6.2 B-8) can locate the vtable slot.
- Closure-call shim direct callees
  (`crates/regseq-driver/src/bridge/body/terminator.rs:320`) are
  untouched.
