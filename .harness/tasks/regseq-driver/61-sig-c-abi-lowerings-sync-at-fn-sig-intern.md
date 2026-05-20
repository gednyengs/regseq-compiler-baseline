# regseq-driver-61: `sig_c_abi_lowerings` parallel-table sync at every driver-side `FnSig` intern

## Description

Fix the bridge invariant-violation bug surfaced by manual playground
stress testing under `panic=abort`: any source body containing a
non-cleanup-block `Drop` terminator (e.g., a `self`-consuming method
on a `Drop`-implementing type) panics the bridge with

```
thread 'rustc' panicked at crates/regseq-model/src/model.rs:3814:5:
assertion `left == right` failed
  left: 4
 right: 5
```

at the `permute_index_vec_with_keep::<FnSigId, Option<CAbiLowering>>`
length check inside `RegseqModel::remove_fn_sigs_and_remap`
(`crates/regseq-model/src/model.rs:1620`), invoked from
`bridge::diag_log::dead_item_elim::eliminate_diag_log_peel_residue`
(`crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs:437`).

At the panic point `sigs.len() == 5, sig_c_abi_lowerings.len() == 4`.
The fifth (unpaired) `FnSig` is the Drop-as-Call collapse drop-glue
signature -- `abi=Rust, safety=Unsafe, inputs=[*mut T], output=()` --
synthesised by `translate_drop_as_call` at
`crates/regseq-driver/src/bridge/body/terminator.rs:638` via the
closure passed to `lookup_or_create_direct_instance`.

Root cause: `lookup_or_create_direct_instance`
(`crates/regseq-driver/src/bridge/body/terminator.rs:504-558`) calls
its `make_fn_sig` argument (which interns directly via
`cx.model.sigs.intern(FnSig { ... })`) and pushes a fresh `Instance`,
but never extends `cx.model.sig_c_abi_lowerings`. That violates the
spec.md §3 invariant `sig_c_abi_lowerings.len() == sigs.len()`
(`docs/discussions/regseq-model/spec.md:540-542`) and the spec.md §12
step 5 per-entry presence rule ("`Some` iff `FnAbi::C`",
`docs/discussions/regseq-model/spec.md:746-747`).

Two `lookup_or_create_direct_instance` call sites in
`bridge/body/terminator.rs` route through the buggy path:

- `terminator.rs:316-331` (closure-call FnSig synthesis): bare
  `cx.model.sigs.intern(FnSig { ... })` inside the `make_fn_sig`
  closure -- latent, not currently exercised by any committed fixture
  (no closure-call fixture trips the parallel-table desync today).
- `terminator.rs:631-646` (Drop-as-Call collapse): same shape -- the
  firing offender for the playground fixture and for any fixture with
  a self-consuming method on a `Drop`-implementing type.

The third caller at `terminator.rs:423-430` (the regular `Call`
callee path) routes its `make_fn_sig` through `callsite_fn_sig` ->
`crate::bridge::fnsig::translate_fn_sig` -> `ensure_cabi_lowering_slots`
(`crates/regseq-driver/src/bridge/fnsig.rs:76-80`), so it is NOT
affected.

A third latent site lives outside `lookup_or_create_direct_instance`:
the `TyKind::FnPtr` translation branch at
`crates/regseq-driver/src/bridge/ty.rs:172-179` interns a `FnSig`
whose ABI comes from `translate_abi(abi)?` (so it CAN be `FnAbi::C`
for an `extern "C" fn(...)` pointer in source). The current code
neither calls `ensure_cabi_lowering_slots` nor populates the C-ABI
lowering, so an `extern "C" fn` pointer in a source body would both
desync the table length AND violate the "Some iff `FnAbi::C`"
presence rule -- not currently exercised because no committed fixture
uses an `extern "C" fn` value, but it is the same class of bug.

The fix centralises the parallel-table maintenance at the two intern
choke points the driver still routes around (`lookup_or_create_direct_instance`
and the `TyKind::FnPtr` branch), so the spec.md §3 + §12 step 5
invariant becomes structurally maintained for every driver-side
`FnSig` intern. Production-code only; no spec amendment, no model-API
change, no four-bucket validator change.

## Input Context

### Spec anchors (binding)

- `docs/discussions/regseq-model/spec.md` §3 (`RegseqModel` schema,
  `sig_c_abi_lowerings` field at `spec.md:313`; length invariant
  "`sig_c_abi_lowerings.len() == sigs.len()`" + per-entry presence
  rule at `spec.md:540-542`). Binding invariant violated by the bug.
- `docs/discussions/regseq-model/spec.md` §12 step 5 (validator
  rule at `spec.md:742-758`): `sig_c_abi_lowerings.len() ==
  sigs.len()` AND "`Some` iff `FnAbi::C`" with the per-entry
  arity / shape-target / tag-scalar checks for the present
  `CAbiLowering` entries.
- `docs/discussions/regseq-model/spec.md` §14 (`FnAbi` and `FnSig`
  minimum canonical schema, including the `sig_c_abi_lowerings` side
  table specification at `spec.md:5437` and `spec.md:5499-5527`).
- `docs/discussions/regseq-model/spec.md` §23.4 (Drop-as-Call collapse
  at the bridge, `spec.md:9388-9528`): the normative contract for
  the drop-glue Function Instance synthesis whose FnSig intern is the
  firing offender. The collapse synthesises an Instance with
  `flavor = FunctionInstanceFlavor::DropGlue` and FnSig
  `unsafe fn(*mut self_ty) -> ()` (`FnAbi::Rust`).

### Bridge code anchors (this task's edit surface)

- `crates/regseq-driver/src/bridge/fnsig.rs:76-80` --
  `ensure_cabi_lowering_slots(cx: &mut BridgeCtxt<'_>)`, currently
  module-private. Change A promotes it to `pub(crate)` so the two
  out-of-module callers (Changes B and C) can invoke it.
- `crates/regseq-driver/src/bridge/fnsig.rs:65-74` --
  `compute_cabi_lowering(cx, sig_id) -> CAbiLowering`, already `pub`.
  Used in Change C to populate the lowering for `FnAbi::C` FnPtrs
  per spec.md §12 step 5 "Some iff `FnAbi::C`".
- `crates/regseq-driver/src/bridge/body/terminator.rs:504-558` --
  `lookup_or_create_direct_instance`. The `make_fn_sig` closure (line
  541) interns a fresh `FnSigId`; Change B inserts the parallel-slot
  call between line 541 (intern) and line 544
  (`cx.model.instances.push(...)`).
- `crates/regseq-driver/src/bridge/body/terminator.rs:316-331` --
  closure-call `make_fn_sig` closure (currently latent; covered
  automatically by Change B).
- `crates/regseq-driver/src/bridge/body/terminator.rs:631-646` --
  Drop-as-Call `make_fn_sig` closure (the firing offender; covered
  automatically by Change B).
- `crates/regseq-driver/src/bridge/ty.rs:135-179` --
  `TyKind::FnPtr` branch. The bare
  `cx.model.sigs.intern(sig)` at line 179 produces the `FnSigId`
  that Change C must pair with a parallel-table maintenance step.
- `crates/regseq-model/src/model.rs:1613-1629` --
  `RegseqModel::remove_fn_sigs_and_remap`. Change D inserts a
  `debug_assert_eq!(self.sig_c_abi_lowerings.len(), self.sigs.len())`
  at the top of the function body, before the
  `permute_index_vec_with_keep` call at line 1620, so future
  desyncs surface with a clear message rather than the bare-numeric
  `permute_index_vec_with_keep` panic at
  `crates/regseq-model/src/model.rs:3814`.

### Audit: every `\.sigs\.intern\|\.sigs\.push` site in
`crates/regseq-driver/src/`

| Site | Sync? | Mechanism |
|------|-------|-----------|
| `bridge/fnsig.rs:56` (`translate_fn_sig`) | OK | calls `ensure_cabi_lowering_slots(cx)` at `fnsig.rs:57` and writes lowering at `fnsig.rs:58` |
| `bridge/synth/vtable.rs:190` (`compute_method_shim_fn_sig`) | OK | calls `compute_cabi_lowering(cx, erased_sig_id)` at `synth/vtable.rs:191` |
| `bridge/synth/vtable.rs:684` (`canonical_vacant_fn_ptr_ty`) | OK | calls `compute_cabi_lowering(cx, sig_id)` at `synth/vtable.rs:685` |
| `bridge/diag_log/dead_item_elim.rs:529` (`rewrite_log_intrinsic_signatures`) | OK | inline `while < sigs.len() push None` at `dead_item_elim.rs:530-532` |
| `bridge/diag_log/dead_item_elim.rs:1156, 1305, 1326` | OK | inline `push(None)` immediately after intern at `dead_item_elim.rs:1163, 1312, 1333` |
| `bridge/mod.rs:2296` (closure-call FnSig synth, eager) | OK | inline `while < sigs.len() push None` at `bridge/mod.rs:2303-2305` |
| `bridge/diag_log/peel.rs:961, 1514` | n/a | both inside `#[cfg(test)]` test-helper `mod tests` blocks (test-only) |
| `bridge/body/materialise.rs:755`, `bridge/body/vtable_indirect.rs:509` | n/a | both inside `#[cfg(test)]` test-helper `mod tests` blocks (test-only) |
| **`bridge/body/terminator.rs:316-331`** | **BUG (latent)** | bare `cx.model.sigs.intern(FnSig{...})` inside `make_fn_sig` closure; closure-call path not exercised by any committed fixture |
| **`bridge/body/terminator.rs:638`** | **BUG (firing)** | bare `cx.model.sigs.intern(FnSig{...})` inside `make_fn_sig` closure; Drop-as-Call path |
| **`bridge/ty.rs:179`** | **BUG (latent)** | bare `cx.model.sigs.intern(sig)`; `extern "C" fn(...)` source FnPtr not exercised by any committed fixture |

Out of scope: the 4 test-only sites in `#[cfg(test)]` modules. They
construct ad-hoc `RegseqModel`s for unit testing and are deliberately
isolated from the production sync invariants.

### Test surface

The Codex-review-confirmed assertion shape for the regression-guard
fixture: assert on the bridge-synthesised drop-glue
**Instance flavor + synthetic-item kind**, NOT on a literal
`<Reg as Drop>::drop` source-side def_path. Drop-glue synthesis
creates a synthetic `SyntheticItemKind::DropGlue { self_ty }`
Function Item (per `crates/regseq-driver/src/bridge/synth/drop_glue.rs`),
and `translate_drop_as_call` emits an `Instance` with
`FunctionInstanceFlavor::DropGlue` and `virtual_idx = None`
(`crates/regseq-driver/src/bridge/body/terminator.rs:611-646`).
The `<Reg as Drop>::drop` source method has its own (separate)
Function Instance with `FunctionInstanceFlavor::Item`; the test
asserts on the synthetic drop-glue Instance, not the source-side
Drop impl.

- `crates/test-ir-fixtures/src/lib.rs:188-198` -- the
  `bridge_fixture` helper already injects `-C panic=abort` so any
  new test fixture automatically gets the cleanup-block-pruning
  behavior that exposes the on-success Drop terminator. No test
  helper changes required.

### Prior tasks

- regseq-driver-60 (closed at `a1ac768`): the body materializer's
  args-substitution fix. Closed Phase 1a's last test gap before this
  bug surfaced. Trait default method bodies now materialize; the
  next class of fixtures (concrete impls of `Drop` + self-consuming
  methods) trips this separate parallel-table bug.

## Implementation Notes

### Change A: promote `ensure_cabi_lowering_slots` to `pub(crate)`

**Anchor**: `crates/regseq-driver/src/bridge/fnsig.rs:76-80`.

```rust
// Before
fn ensure_cabi_lowering_slots(cx: &mut BridgeCtxt<'_>) {
    while cx.model.sig_c_abi_lowerings.len() < cx.model.sigs.len() {
        cx.model.sig_c_abi_lowerings.push(None);
    }
}

// After
pub(crate) fn ensure_cabi_lowering_slots(cx: &mut BridgeCtxt<'_>) {
    while cx.model.sig_c_abi_lowerings.len() < cx.model.sigs.len() {
        cx.model.sig_c_abi_lowerings.push(None);
    }
}
```

Internal helper, function body untouched. The two callers added by
Changes B and C live in sibling modules
(`crates/regseq-driver/src/bridge/body/terminator.rs` and
`crates/regseq-driver/src/bridge/ty.rs`), so `pub(crate)` is the
minimum visibility that suffices. No re-export, no module surgery.

### Change B: sync slots inside `lookup_or_create_direct_instance`

**Anchor**: `crates/regseq-driver/src/bridge/body/terminator.rs:541`
(inside `lookup_or_create_direct_instance`).

Insert the slot-ensure call between the `make_fn_sig` call (line
541) and the `Instance::push` (line 544):

```rust
let fn_sig = make_fn_sig(cx)?;
crate::bridge::fnsig::ensure_cabi_lowering_slots(cx);
let emission_name = cx.model.items[item].def_path.rendered;
```

Rationale (covering both `make_fn_sig` shapes the helper supports):

- **Closure-call site** (`terminator.rs:316-331`): the closure
  interns directly via `cx.model.sigs.intern(FnSig{...})`. The
  intern produces a new `FnSigId` that needs a parallel slot.
  Change B's call materialises it.
- **Drop-as-Call site** (`terminator.rs:631-646`): same shape.
- **Regular-Call site** (`terminator.rs:423-430`, routes through
  `callsite_fn_sig` -> `translate_fn_sig`): already syncs at
  `fnsig.rs:57`. Change B's call is then idempotent (the
  `while < sigs.len()` body executes zero times). No double-push,
  no behavior change.

The placement is AFTER `make_fn_sig` resolves but BEFORE the new
`Instance` is pushed. That keeps the parallel-table invariant
holding at every observable point in the helper's surface.

### Change C: sync slots after `TyKind::FnPtr` intern, respecting
"Some iff `FnAbi::C`"

**Anchor**: `crates/regseq-driver/src/bridge/ty.rs:172-179`.

The current code interns a `FnSig` whose `abi` is whatever
`translate_abi(abi)?` returned -- can be `FnAbi::Rust` (RustCall
re-shaped, plain Rust) OR `FnAbi::C` (`extern "C" fn(...)`). Simply
appending `None` would fix the length but violate the spec.md §12
step 5 "Some iff `FnAbi::C`" presence rule for the `extern "C" fn`
case.

Mirror the `translate_fn_sig` pattern (`fnsig.rs:51-58`):

```rust
// Before
let sig = FnSig {
    inputs,
    output,
    abi: translate_abi(abi)?,
    c_variadic: sig.c_variadic,
    safety: translate_safety(sig.safety),
};
Ty::FnPtr(cx.model.sigs.intern(sig))

// After
let sig = FnSig {
    inputs,
    output,
    abi: translate_abi(abi)?,
    c_variadic: sig.c_variadic,
    safety: translate_safety(sig.safety),
};
let sig_id = cx.model.sigs.intern(sig);
if cx.model.sigs.resolve(sig_id).abi == FnAbi::C {
    let _ = crate::bridge::fnsig::compute_cabi_lowering(cx, sig_id);
} else {
    crate::bridge::fnsig::ensure_cabi_lowering_slots(cx);
}
Ty::FnPtr(sig_id)
```

Notes:

- `compute_cabi_lowering` internally calls `ensure_cabi_lowering_slots`
  and writes `Some(CAbiLowering)` only when `abi == FnAbi::C`
  (`fnsig.rs:69-72`), so it produces the spec-correct shape.
- The explicit `if abi == FnAbi::C` branch avoids calling
  `compute_cabi_lowering` on a Rust-ABI sig, which panics via
  `.expect("translated FnSig must be C ABI admissible")` at
  `fnsig.rs:68` when `CAbiChecker::sig_lowering` rejects the input.
  The branch matches `translate_fn_sig`'s own dispatch shape at
  `fnsig.rs:51-58`.

### Change D: `debug_assert` on the invariant inside
`RegseqModel::remove_fn_sigs_and_remap`

**Anchor**: `crates/regseq-model/src/model.rs:1613-1629`.

Insert a `debug_assert_eq!` for the side-table-length invariant
between the existing `assert_eq!(fn_sig_keep.len(), self.sigs.len(), ...)`
at line 1614 and the `compact_by_keep` call at line 1619:

```rust
pub fn remove_fn_sigs_and_remap(&mut self, fn_sig_keep: &[bool]) -> Vec<Option<FnSigId>> {
    assert_eq!(
        fn_sig_keep.len(),
        self.sigs.len(),
        "remove_fn_sigs_and_remap keep slice length must match FnSigInterner storage length",
    );
    debug_assert_eq!(
        self.sig_c_abi_lowerings.len(),
        self.sigs.len(),
        "remove_fn_sigs_and_remap invariant: sig_c_abi_lowerings.len() must equal sigs.len() per spec.md section 3 (every interned FnSig has a parallel side-table slot)",
    );
    let remap = self.sigs.compact_by_keep(fn_sig_keep);
    permute_index_vec_with_keep(&mut self.sig_c_abi_lowerings, fn_sig_keep);
    // ...
}
```

Future desyncs panic with the named-invariant message at the
`remove_fn_sigs_and_remap` boundary, not the bare-numeric
`permute_index_vec_with_keep` panic at
`crates/regseq-model/src/model.rs:3814`. The bug-discovery
trajectory ("4 left vs 5 right" -> trace -> diagnose) for this
specific bug shows the latter shape is unhelpful for future
debuggers.

This is a `debug_assert_eq!`, not `assert_eq!`: production runs do
not pay the check; the dev-test gate catches every desync. Mirrors
the existing `debug_assert_eq!` shape in
`permute_index_vec_with_keep` itself at `model.rs:3814`.

### Change E: harness expectations entry

**Anchor**: `.harness/expectations/regseq-driver-61.md` (new file).

Record:

- Before-fix evidence: the panic shape (
  `assertion left == right failed; left=4, right=5`) at
  `crates/regseq-model/src/model.rs:3814`; the firing offender
  intern at `crates/regseq-driver/src/bridge/body/terminator.rs:638`;
  the spec.md invariant pin at `spec.md:540-542` and `spec.md:746-747`.
- After-fix evidence: the new B1 Drop-glue fixture compiles and
  bridges cleanly; the new B2 closure-call latent-site regression
  guard test passes; the new B3 FnPtr-translation latent-site
  regression guard test passes (both Rust-ABI and C-ABI variants).
- Out-of-scope: the `cargo regseq` wrapper does NOT inject
  `-C panic=abort` today; the playground fixture compensates with
  `[profile.dev/release] panic = "abort"` in
  `tests/fixtures/playground/Cargo.toml`. The
  `bridge_fixture` test helper already injects `-C panic=abort`
  (`crates/test-ir-fixtures/src/lib.rs:188-198`). The wrapper-side
  panic-strategy gap is a separate concern; this task does NOT
  address it.

## Non-Goals

- Do NOT change the model-side `FnSigInterner::intern` surface to
  push the parallel slot atomically. That is a structurally cleaner
  fix (the parallel-table invariant becomes structurally enforced),
  but it touches `crates/regseq-model/src/model.rs` and changes the
  model crate's public API. File as a separate model-side task in a
  follow-up; this task is the driver-side bug-fix scope.
- Do NOT modify the `cargo regseq` wrapper to inject `-C panic=abort`
  globally. The wrapper-side panic-strategy gap is real but is a
  separate concern (it surfaces the cleanup-block survival
  rejection, not this parallel-table desync). File as a separate
  task.
- Do NOT modify spec.md §3, §12, §14, or §23. The spec contract is
  correct; the bug is in the driver's adherence.
- Do NOT modify pretty-printer.md. Rendering rules are unchanged.
- Do NOT modify the four-bucket validator at
  `crates/regseq-model/src/model.rs:3528-3633`. The desync surfaces
  before validation runs (during the peel-residue DCE in
  `eliminate_diag_log_peel_residue` at
  `crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs:437`).
- Do NOT modify `tests/fixtures/playground/`. Playground is the
  user's experimentation space and must not carry committed test
  assertions.
- Do NOT add a coverage check or invariant assertion at every
  intern site. The centralization at `lookup_or_create_direct_instance`
  + the `TyKind::FnPtr` branch handles every known production site.
  Adding a probe per intern site would be defensive coding contrary
  to `feedback_no_defensive_fallbacks`.
- Do NOT widen the `ensure_cabi_lowering_slots` visibility beyond
  `pub(crate)`. The model crate does not need it; only the
  sibling-module driver callers do.
- Do NOT retrofit the four test-only `sigs.intern` sites in
  `#[cfg(test)]` modules. They are local ad-hoc constructions
  with their own paired `push(None)`; they are correct as written.

## Test Plan

### B1: end-to-end Drop-glue fixture bridges cleanly

**New file**:
`crates/test-ir-fixtures/fixtures/drop_glue_in_self_consuming_method.rs`
(new fixture; the playground's stress case in normalized form).

Source (rough shape; implementer reconciles against the
`compile_with_bridge`/`bridge_fixture` calling convention):

```rust
#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

pub struct R(u32);

impl Drop for R {
    fn drop(&mut self) {}
}

impl R {
    pub fn new(off: u32) -> R { R(off) }
}

impl Into<u32> for R {
    fn into(self) -> u32 { self.0 }
}

pub fn entry() -> u32 {
    R::new(0).into() + R::new(1).into()
}
```

The fixture exercises:

- `Into::into(self)` -- a self-consuming trait method whose MIR
  carries a `drop(_1) -> [return: bb_succ, unwind unreachable]`
  terminator on the success path (NOT in a cleanup block), so the
  `-C panic=abort` cleanup-block pruning does NOT eliminate it.
- The Drop terminator triggers Drop-as-Call collapse at
  `crates/regseq-driver/src/bridge/body/terminator.rs:560+`.
- The collapse interns a new drop-glue FnSig
  (`unsafe fn(*mut R) -> ()`, `FnAbi::Rust`) via
  `lookup_or_create_direct_instance` at
  `crates/regseq-driver/src/bridge/body/terminator.rs:631-646`.

**New test**:
`crates/regseq-driver/tests/bridge_drop_glue_sig_sync.rs` (or extend
an existing bridge integration test file -- implementer chooses
based on workspace test taxonomy).

Assertions:

- `bridge_fixture("drop_glue_in_self_consuming_method")` returns
  `Ok(_)` (no panic, no `BridgeError`).
- The resulting `RegseqModel` has at least one Instance whose
  `kind.as_function().flavor == FunctionInstanceFlavor::DropGlue`
  and `virtual_idx == None`.
- That Instance's Item carries
  `SyntheticItemKind::DropGlue { self_ty: <TyId for R> }`.
- That Instance's `fn_sig` resolves to a `FnSig` with
  `inputs.len() == 1`, `output == unit_ty`, `abi == FnAbi::Rust`,
  `safety == Safety::Unsafe`. The single input resolves to
  `Ty::Ptr(<TyId for R>, Mutability::Mut)`.
- `model.sig_c_abi_lowerings.len() == model.sigs.len()` (the
  spec.md §3 invariant; positive assertion after fix).
- For every `sig_id` with `sigs.resolve(sig_id).abi == FnAbi::Rust`,
  `sig_c_abi_lowerings[sig_id] == None` (the spec.md §12 step 5
  presence rule). The drop-glue Rust-ABI sig's slot must be `None`.

Pre-fix: this test panics at the same site as the playground stress
case (`crates/regseq-model/src/model.rs:3814`).

Post-fix: this test passes.

### B2: closure-call latent-site regression guard

**New file**:
`crates/test-ir-fixtures/fixtures/closure_call_sig_sync.rs`
(focused fixture; the closure-call FnSig-intern path at
`terminator.rs:316-331` -- latent today, no committed fixture trips
it).

Source (rough shape):

```rust
#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

pub fn entry() -> u32 {
    let f = |x: u32| x + 1;
    f(7)
}
```

**New test** (same crate as B1):

- `bridge_fixture("closure_call_sig_sync")` returns `Ok(_)`.
- `model.sig_c_abi_lowerings.len() == model.sigs.len()`.
- For every Function Instance with
  `kind.as_function().flavor` matching the closure-call shape
  (`SyntheticItemKind::ClosureCall { .. }` carrier on the Item,
  per `bridge/synth/closure.rs`), `sig_c_abi_lowerings[fn_sig]
  == None` (Rust ABI).

Pre-fix: implementer verifies the fixture actually trips the
closure-call `make_fn_sig` path (i.e., that the closure call site
flows through `terminator.rs:316-331` rather than through
`bridge/mod.rs:2296`'s eager-synth pre-paired path). If the eager
synth at `bridge/mod.rs:2278-2305` always paints the slot before
`terminator.rs` ever reaches the `make_fn_sig` closure, this site
is structurally unreachable and B2 collapses to a no-op coverage
test. Implementer marks the case in the harness expectations file
either way.

Post-fix: passes regardless.

### B3: `TyKind::FnPtr` latent-site regression guard (Rust + C ABI)

**New file**:
`crates/test-ir-fixtures/fixtures/fn_ptr_translation_sig_sync.rs`
(focused fixture; the `bridge/ty.rs:179` FnPtr-intern path).

Source (rough shape; both Rust-ABI and `extern "C"` variants):

```rust
#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

pub fn helper_rust(x: u32) -> u32 { x }
pub extern "C" fn helper_c(x: u32) -> u32 { x }

pub fn entry() -> u32 {
    let p_rust: fn(u32) -> u32 = helper_rust;
    let p_c: extern "C" fn(u32) -> u32 = helper_c;
    p_rust(0) + p_c(0)
}
```

**New test** (same crate as B1):

- `bridge_fixture("fn_ptr_translation_sig_sync")` returns `Ok(_)`.
- `model.sig_c_abi_lowerings.len() == model.sigs.len()`.
- There exists at least one `sig_id` with
  `sigs.resolve(sig_id).abi == FnAbi::Rust` and
  `sig_c_abi_lowerings[sig_id] == None` (the Rust-ABI FnPtr).
- There exists at least one `sig_id` with
  `sigs.resolve(sig_id).abi == FnAbi::C` and
  `sig_c_abi_lowerings[sig_id] == Some(_)` whose `inputs.len() ==
  1` matches the source `extern "C" fn(u32) -> u32` shape (the
  C-ABI FnPtr -- this directly exercises Codex finding #3, the
  "Some iff `FnAbi::C`" presence rule).

Pre-fix: this test panics at
`crates/regseq-model/src/model.rs:3814` (length desync) before
validation runs.

Post-fix: this test passes.

### B4: `debug_assert` fires when the invariant is violated

**Unit test** (in
`crates/regseq-model/tests/id_referential_integrity_validator.rs`
or a new sibling test file; implementer chooses):

Construct a small `RegseqModel` by hand with `sigs.len() == 2` and
`sig_c_abi_lowerings.len() == 1`, then call
`remove_fn_sigs_and_remap(&[true, true])`. Assert the call panics
in debug builds with a message matching
`r"sig_c_abi_lowerings\.len\(\) must equal sigs\.len\(\)"`.

This locks Change D's failure mode into the test suite -- future
desyncs surface with the named-invariant message at the
`remove_fn_sigs_and_remap` boundary, not at the bare-numeric
`permute_index_vec_with_keep` panic.

### B5: existing test suites stay green

- `cargo test --workspace -- --test-threads=1` green.
- Specifically: regseq-driver-58 B1-B5 (eager HIR enumeration),
  regseq-driver-59 B8-B9 (foreign rendering + trait default
  method), regseq-driver-60 B1-B4 (body materializer
  args-substitution + generic-fn body materialization). All
  unchanged behaviorally; Change B is idempotent on the regular-Call
  path, Change C is gated on `abi == FnAbi::C`, Change D is a
  debug-only assertion, Change A is a visibility-only change.

### B6: manual verification on the playground stress fixture

After the fix lands, the implementer manually verifies:

```bash
cd tests/fixtures/playground && \
  PATH="../../../target/debug:$PATH" cargo regseq \
    --target-os linux --platform-file platform.toml --emit model
```

succeeds (no panic, no `BridgeError`) against the same playground
source that surfaced the bug. The implementer does NOT add a
committed assertion against the playground fixture per
`playground-is-experimentation-space` rule -- this is a one-off
manual smoke check, recorded in the harness expectations file as
"manually verified at <commit>".

## Review

### Files to send to Codex (`scripts/python/codex_runner.py`)

- This file
  (`.harness/tasks/regseq-driver/61-sig-c-abi-lowerings-sync-at-fn-sig-intern.md`).

(No spec amendment in this task.)

### Reviewer focus

- **Root-cause framing**: does the Description correctly identify
  the firing offender (`terminator.rs:638` Drop-as-Call FnSig
  intern, called via `lookup_or_create_direct_instance` which
  doesn't sync the parallel table) and the two latent sites
  (`terminator.rs:316-331` closure-call, `ty.rs:172-179` FnPtr)?
- **Helper-placement correctness**: is the `ensure_cabi_lowering_slots`
  call placement inside `lookup_or_create_direct_instance` (between
  `make_fn_sig` and the `Instance::push`) idempotent for the regular-Call
  path that already syncs via `translate_fn_sig`? Confirm by tracing
  `terminator.rs:423-430` -> `callsite_fn_sig` -> `translate_fn_sig`
  -> `ensure_cabi_lowering_slots`.
- **`FnPtr` branch correctness**: does the proposed Change C handle
  the `abi == FnAbi::C` case correctly via `compute_cabi_lowering`
  and the `abi == FnAbi::Rust` case correctly via
  `ensure_cabi_lowering_slots`? Cross-check against spec.md §12
  step 5 "Some iff `FnAbi::C`" presence rule.
- **Latent-site enumeration**: confirm no production
  `\.sigs\.intern\|\.sigs\.push` site in
  `crates/regseq-driver/src/` is left out of the audit table.
- **Test-assertion shape**: B1's assertion targets
  `FunctionInstanceFlavor::DropGlue` + `SyntheticItemKind::DropGlue
  { self_ty }` (not literal `<Reg as Drop>::drop` def_path). B2 /
  B3 target the parallel-table invariant directly. Confirm the
  shape matches the actual bridge synthesis path.
- **Defensive-fallback audit** (per
  `feedback_no_defensive_fallbacks`): the proposed fix centralises
  the sync at intern choke points -- it does NOT add per-call-site
  defensive `if needs_sync { ... }` branches. Confirm.
- **No-spec-amendment scope check**: does the fix stay
  driver-side? spec.md §3, §12, §14, §23 are unchanged; the
  model.rs surface change is only the `debug_assert_eq!` in
  `remove_fn_sigs_and_remap` (Change D), not a new API or
  semantics change.
- **Citation drift**: every file:line must anchor against HEAD.

### Adversarial prompt template

```
<task>
Review .harness/tasks/regseq-driver/61-sig-c-abi-lowerings-sync-at-fn-sig-intern.md.
Identify:
- Citation drift.
- Whether Change B's helper-placement is idempotent for the
  regular-Call path that already syncs via translate_fn_sig.
- Whether Change C's `abi == FnAbi::C` branch correctly populates
  via compute_cabi_lowering (spec.md section 12 step 5 "Some iff
  FnAbi::C" presence rule).
- Whether the audit table missed any production `sigs.intern` /
  `sigs.push` site in `crates/regseq-driver/src/`.
- Whether B1's drop-glue assertion shape matches the bridge's
  synthesis path (FunctionInstanceFlavor::DropGlue,
  SyntheticItemKind::DropGlue { self_ty }).
- Defensive-fallback smells.
- Scope creep.
</task>

<grounding_rules>
- Cite file:line for every claim.
- Mark hypotheses as hypotheses.
- Don't speculate about code or spec sections you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED WITH MINOR FIXES | NOT READY
PER-CHANGE VERIFICATION: A, B, C, D, E bullets with file:line
HELPER-PLACEMENT IDEMPOTENCY CHECK: PASS / FAIL with citation
FN-PTR ABI BRANCH CHECK: PASS / FAIL with citation
LATENT-SITE ENUMERATION CHECK: PASS / FAIL (additional sites?)
TEST ASSERTION SHAPE CHECK: PASS / FAIL with citation
CITATION DRIFT: list or "none"
NO-DEFENSIVE-FALLBACK AUDIT: pass/fail
SCOPE-CREEP CHECK: pass/fail with citation
NEW ISSUES: numbered list or "none"
REQUIRED FIXES: numbered list (only if VERDICT != APPROVED)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate

- `cargo fmt --all` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test --workspace -- --test-threads=1` green. Specifically:
  - The new B1 Drop-glue fixture test passes.
  - The new B2 closure-call latent-site test passes (or collapses
    to a no-op coverage test if the eager-synth at
    `bridge/mod.rs:2278-2305` makes the bare-intern path
    structurally unreachable; implementer documents either way).
  - The new B3 FnPtr-translation latent-site test passes for both
    Rust-ABI and C-ABI variants.
  - The new B4 `debug_assert` unit test passes (panic with
    named-invariant message).
  - No regressions in regseq-driver-58 / -59 / -60 tests or any
    existing workspace test.
- `git diff --stat HEAD..` shows changes confined to:
  - `crates/regseq-driver/src/bridge/fnsig.rs` (Change A)
  - `crates/regseq-driver/src/bridge/body/terminator.rs` (Change B)
  - `crates/regseq-driver/src/bridge/ty.rs` (Change C)
  - `crates/regseq-model/src/model.rs` (Change D)
  - new fixture files under
    `crates/test-ir-fixtures/fixtures/` and new test file(s)
    under `crates/regseq-driver/tests/` and/or
    `crates/regseq-model/tests/`
  - new `.harness/expectations/regseq-driver-61.md` (Change E)
- No spec.md, pretty-printer.md, generic-c-backend.md, or
  validator changes.
- Manual verification on `tests/fixtures/playground/` per B6.
- Codex Task Review verdict: APPROVED or APPROVED WITH MINOR FIXES.

## Acceptance Criteria

1. **`ensure_cabi_lowering_slots` is `pub(crate)`**:
   `crates/regseq-driver/src/bridge/fnsig.rs:76` declares the
   function `pub(crate) fn ensure_cabi_lowering_slots(...)`.

2. **`lookup_or_create_direct_instance` syncs the parallel table
   immediately after `make_fn_sig`**: in
   `crates/regseq-driver/src/bridge/body/terminator.rs` (the body of
   `lookup_or_create_direct_instance` at lines 504-558), between
   the `let fn_sig = make_fn_sig(cx)?;` (line 541) and the
   `cx.model.instances.push(...)` (line 544), there is a call to
   `crate::bridge::fnsig::ensure_cabi_lowering_slots(cx)`.

3. **`TyKind::FnPtr` branch handles both ABIs correctly**: in
   `crates/regseq-driver/src/bridge/ty.rs` (the `TyKind::FnPtr` branch
   at lines 135-179), after `let sig_id = cx.model.sigs.intern(sig)`,
   the code branches on `cx.model.sigs.resolve(sig_id).abi`: for
   `FnAbi::C` it calls `crate::bridge::fnsig::compute_cabi_lowering(cx,
   sig_id)`; for non-C it calls
   `crate::bridge::fnsig::ensure_cabi_lowering_slots(cx)`. The
   resulting model satisfies the spec.md §12 step 5 "Some iff
   `FnAbi::C`" presence rule for every FnPtr-derived FnSig.

4. **`remove_fn_sigs_and_remap` carries the side-table-length
   `debug_assert_eq!`**:
   `crates/regseq-model/src/model.rs` lines 1613-1629 contain a
   `debug_assert_eq!(self.sig_c_abi_lowerings.len(),
   self.sigs.len(), ...)` between the existing `assert_eq!` at line
   1614 and the `compact_by_keep` call at line 1619.

5. **B1 Drop-glue fixture test passes**: the new test asserts on
   `FunctionInstanceFlavor::DropGlue` + `SyntheticItemKind::DropGlue
   { self_ty }` + the `unsafe fn(*mut R) -> ()` FnSig shape +
   `sig_c_abi_lowerings.len() == sigs.len()` + the "Some iff
   `FnAbi::C`" presence rule.

6. **B2 closure-call regression guard test lands**: a test
   exercises (or coverage-checks) the
   `terminator.rs:316-331` closure-call `make_fn_sig` path and
   asserts the parallel-table invariant. If the eager synth at
   `bridge/mod.rs:2278-2305` makes the bare-intern path
   structurally unreachable, the test documents that AND verifies
   the eager-synth path's own slot-pairing.

7. **B3 FnPtr-translation regression guard test passes for both
   Rust-ABI and C-ABI variants**: at least one FnSig with
   `abi == FnAbi::C` has a `Some(CAbiLowering)` slot; at least one
   FnSig with `abi == FnAbi::Rust` has a `None` slot.

8. **B4 `debug_assert` unit test passes**: constructing an invalid
   `RegseqModel` (intentionally desynced `sig_c_abi_lowerings`)
   and calling `remove_fn_sigs_and_remap` panics in debug builds
   with the named-invariant message.

9. **B5 no regressions**: the existing workspace test suite
   (regseq-driver-58 / -59 / -60 plus all crate-level integration
   tests) stays green.

10. **B6 manual playground verification**: the playground stress
    fixture at `tests/fixtures/playground/src/lib.rs` (with the
    `[profile.dev/release] panic = "abort"` patches in
    `tests/fixtures/playground/Cargo.toml`) compiles cleanly via
    `cargo regseq --target-os linux --platform-file platform.toml
    --emit model`. Recorded in `.harness/expectations/regseq-driver-61.md`
    as "manually verified at <commit-sha>".

11. **No defensive fallback**: Change B uses the centralised
    helper call; Change C's branch is dispatch on the spec-defined
    `FnAbi::C` discriminant (not a defensive "maybe needs sync"
    guard). Per `feedback_no_defensive_fallbacks`.

12. **No scope creep**: `git diff main -- docs/` returns zero.
    `git diff main -- crates/regseq-model/src/model.rs` shows
    only the Change D `debug_assert_eq!` insertion. No
    pretty-printer.md change, no four-bucket validator change, no
    cargo-regseq wrapper change.

13. **Expectations file lands**:
    `.harness/expectations/regseq-driver-61.md` records before /
    after evidence with file:line citations.
