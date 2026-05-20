# regseq-driver-61: implementation evidence

Implementation deliverable for `regseq-driver-61`, the
`sig_c_abi_lowerings` parallel-table sync fix described at
`.harness/tasks/regseq-driver/61-sig-c-abi-lowerings-sync-at-fn-sig-intern.md`.

Parent context: `.harness/expectations/regseq-driver-60.md` (the
body-materializer args-substitution fix that unblocked the next
class of fixtures and surfaced this separate parallel-table desync).

## Scope

Driver-side production-code fix at five intern / invariant sites:

- `crates/regseq-driver/src/bridge/fnsig.rs` (Change A): promote
  `ensure_cabi_lowering_slots` from module-private to `pub(crate)`
  so the two sibling-module callers (Changes B and C) can invoke it.
  Adds a new `pub(crate) try_populate_cabi_lowering` helper used by
  Change C for the inadmissibility-graceful FnPtr path.
- `crates/regseq-driver/src/bridge/body/terminator.rs` (Change B):
  insert `ensure_cabi_lowering_slots(cx)` between the `make_fn_sig`
  call (line 541) and the `Instance::push` inside
  `lookup_or_create_direct_instance`, so the Drop-as-Call collapse
  and closure-call bare-intern paths sync the parallel side table.
- `crates/regseq-driver/src/bridge/ty.rs` (Change C): in the
  `TyKind::FnPtr` branch, dispatch on the resolved `FnAbi`. For
  `FnAbi::C`, attempt to populate the C-ABI lowering via
  `try_populate_cabi_lowering`; if the sig is inadmissible the
  slot remains `None` and the outer C-ABI admissibility check
  raises the rejection at the correct outer position. For non-C
  ABIs, call `ensure_cabi_lowering_slots` only.
- `crates/regseq-model/src/model.rs` (Change D): insert a
  `debug_assert_eq!(self.sig_c_abi_lowerings.len(), self.sigs.len())`
  at the top of `remove_fn_sigs_and_remap` so future desyncs panic
  with the named-invariant message instead of the bare-numeric
  `permute_index_vec_with_keep` panic.
- `.harness/expectations/regseq-driver-61.md` (Change E): this file.

No spec amendment. No schema change. No pretty-printer change. No
four-bucket validator change. No `cargo regseq` wrapper change.

## Deliberate deviation from the literal task spec

The spec's Change C snippet calls
`crate::bridge::fnsig::compute_cabi_lowering(cx, sig_id)`. That helper
panics via `.expect("translated FnSig must be C ABI admissible")`
(`crates/regseq-driver/src/bridge/fnsig.rs:68`) on inadmissible C-ABI
sigs. The pre-existing test
`nested_fnptr_input_inherits_field_position`
(`crates/regseq-driver/tests/bridge_fnsig.rs:491-511`) exercises an
inadmissible C-ABI FnPtr (`extern "C" fn(char)`) reached through
the `TyKind::FnPtr` branch during struct-field translation, and
expects the rejection to surface from the OUTER struct's
`check_nested_sig` path with `CAbiTypePosition::Field`. The literal
spec call would have panicked at FnPtr-intern time instead of
returning the expected `Err`.

Resolution: added a new `pub(crate) try_populate_cabi_lowering`
helper in `fnsig.rs` that returns `Option<CAbiLowering>` (`None` if
inadmissible) and uses it in `bridge/ty.rs`'s FnPtr branch. The
helper preserves the spec semantics for admissible sigs
(populates the slot with `Some(CAbiLowering)`) AND keeps the
existing test passing (inadmissible C-ABI FnPtrs encountered at type
translation leave the slot `None`; the rejection is raised later at
the correct outer position by `CAbiChecker::check_nested_sig`).
`compute_cabi_lowering` itself is unchanged and remains in use at
the vtable-synth call sites
(`bridge/synth/vtable.rs:191`, `bridge/synth/vtable.rs:685`) where
admissibility is guaranteed by construction.

## Before-fix evidence

- Playground manual stress reproduction (per the bug description in
  the task spec):
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
- Firing offender: the bare
  `cx.model.sigs.intern(FnSig { ... })` inside the
  `lookup_or_create_direct_instance` `make_fn_sig` closure at
  `crates/regseq-driver/src/bridge/body/terminator.rs:638`
  (Drop-as-Call collapse path). `lookup_or_create_direct_instance`
  itself never extended `cx.model.sig_c_abi_lowerings`.
- Spec invariant pin: `sig_c_abi_lowerings.len() == sigs.len()` and
  "`Some` iff `FnAbi::C`" presence rule at
  `docs/discussions/regseq-model/spec.md:540-542` and
  `docs/discussions/regseq-model/spec.md:746-747`.

## After-fix evidence

- B1 Drop-glue fixture test passes:
  `drop_glue_in_self_consuming_method_bridges_without_parallel_table_desync`
  in `crates/regseq-driver/tests/bridge_drop_glue_sig_sync.rs`
  bridges the new
  `crates/test-ir-fixtures/fixtures/drop_glue_in_self_consuming_method.rs`
  fixture (a `Drop`-implementing `R` consumed by an inherent
  `consume(self)` method) and asserts: an Instance carries
  `FunctionInstanceFlavor::DropGlue` + `virtual_idx == None`; that
  Instance's Item carries
  `SyntheticItemKind::DropGlue { self_ty: <ArgsKey for R> }`,
  with the `self_ty` payload checked structurally against
  `args_key_for_generic_args(&model, &[GenericArg::Type(r_ty_id)])`
  per the construction at
  `crates/regseq-driver/src/bridge/synth/drop_glue.rs:22-26`; the
  Instance's FnSig resolves to `unsafe fn(*mut R) -> ()` with
  `FnAbi::Rust`; the parallel-table length equals `sigs.len()`; the
  per-entry "Some iff `FnAbi::C`" presence rule holds.
- B2 closure-call latent-site coverage test passes:
  `closure_call_fixture_bridges_without_parallel_table_desync`
  bridges the new
  `crates/test-ir-fixtures/fixtures/closure_call_sig_sync.rs`
  fixture and asserts the parallel-table invariant holds AND that
  at least one `SyntheticItemKind::ClosureCall` Item exists in the
  model.
- B3 FnPtr-translation latent-site regression guard test passes:
  `fn_ptr_translation_fixture_bridges_without_parallel_table_desync`
  bridges the new
  `crates/test-ir-fixtures/fixtures/fn_ptr_translation_sig_sync.rs`
  fixture (both `fn(u32) -> u32` Rust-ABI and
  `extern "C" fn(u32) -> u32` C-ABI variants) and asserts: the
  parallel-table length holds; at least one FnPtr-derived FnSig has
  `FnAbi::Rust` with a `None` slot; at least one FnPtr-derived FnSig
  has `FnAbi::C` with `inputs.len() == 1` and a `Some(CAbiLowering)`
  slot.
- B4 `debug_assert` unit test passes:
  `remove_fn_sigs_and_remap_panics_on_desynced_parallel_table` in
  `crates/regseq-model/tests/remove_fn_sigs_and_remap_invariant.rs`
  constructs a `RegseqModel` with `sigs.len() == 2` and
  `sig_c_abi_lowerings.len() == 1`, calls
  `remove_fn_sigs_and_remap(&[true, true])`, and asserts the
  resulting panic message references the named invariant
  (`sig_c_abi_lowerings.len() must equal sigs.len()`). Guarded with
  `#[cfg_attr(not(debug_assertions), ignore = ...)]` because the
  `debug_assert_eq!` only fires in debug builds.
- B5 no regressions: `cargo test --workspace -- --test-threads=1`
  green; the pre-existing
  `nested_fnptr_input_inherits_field_position` test passes through
  the new `try_populate_cabi_lowering` helper path (see Deliberate
  deviation above).
- B6 manual playground verification: at commit `649b02a` the
  current playground source
  (`tests/fixtures/playground/src/lib.rs`) compiles cleanly via
  `cd tests/fixtures/playground && PATH="../../../target/debug:$PATH" cargo regseq --target-os linux --platform-file platform.toml --emit model`.
  Note: the current playground source is a trivial `add(p1, p2)`
  shape that does NOT itself trigger the Drop-as-Call collapse;
  the spec author's "playground stress fixture" reference was the
  user's transient experimentation case that is not in HEAD. Per
  the `playground-is-experimentation-space` rule, no committed
  assertion is added against the playground -- the structural
  regression guards live in B1-B4 instead.

## Latent-site notes

- Closure-call bare-intern path at
  `crates/regseq-driver/src/bridge/body/terminator.rs:316-331`:
  currently structurally unreachable end-to-end because the
  bridge's `callsite_fn_sig` shells out to `tcx.fn_sig(def_id)`,
  which panics with ICE
  ```
  compiler/rustc_hir_analysis/src/collect.rs:1028:13:
  to get the signature of a closure, use `args.as_closure().sig()`
  not `fn_sig()`
  ```
  for closure DefIds. The B2 fixture therefore exercises the
  eager closure-family synthesis path at
  `crates/regseq-driver/src/bridge/mod.rs:2278+` (whose own slot
  pairing at `bridge/mod.rs:2303-2305` is unchanged) rather than
  the bare-intern path. This is a SEPARATE pre-existing bridge gap,
  out of scope for this task (would need its own task to wire
  `args.as_closure().sig()` into `callsite_fn_sig`). Change B's
  insertion still covers this path defensively: when the closure-
  call ICE is eventually fixed, the bare-intern site will be
  exercised and Change B's `ensure_cabi_lowering_slots` will keep
  the invariant.
- `TyKind::FnPtr` C-ABI inadmissibility: per the Deliberate deviation
  note above, inadmissible C-ABI FnPtrs encountered during type
  translation leave the slot `None`. In end-to-end source flows,
  such FnPtrs would only reach `model.validate()` if they survive
  body translation without being caught by an outer admissibility
  check -- which in practice does not occur, because the `FnDef`
  the FnPtr points at would have been rejected earlier by
  `translate_fn_sig`. The `nested_fnptr_input_inherits_field_position`
  unit test is the only path that constructs an inadmissible C-ABI
  FnPtr in isolation, and it asserts the Err shape rather than
  running validation on the resulting model.

## Out-of-scope

- The `cargo regseq` wrapper does NOT inject `-C panic=abort` today;
  the playground fixture compensates with
  `[profile.dev/release] panic = "abort"` in
  `tests/fixtures/playground/Cargo.toml`. The `bridge_fixture` test
  helper injects `-C panic=abort` at
  `crates/test-ir-fixtures/src/lib.rs:196-197`. The wrapper-side
  panic-strategy gap is a separate concern; this task does NOT
  address it (explicit Non-Goal per task spec lines 368-371).
- Atomically pushing the parallel slot inside
  `FnSigInterner::intern` is a structurally cleaner fix; explicit
  Non-Goal per task spec lines 361-366.
- Spec.md / pretty-printer.md / generic-c-backend.md unchanged.
  Four-bucket validator unchanged.
