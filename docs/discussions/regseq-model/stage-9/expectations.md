# Expectations on stage 9 (incoming)

Stage 9 covers MIR terminators (Call, Return, Goto, Drop, etc.).

The convention is described in `../memory.md` under "Forward
expectations live in `stage-N/expectations.md`."

## From stage 1

- **`Drop` lowered to `Call`.** Per the C-semantics methodology
  rule and the spec's "Where InstanceId references appear"
  section, stage 9 lowers `Drop` terminators to regular `Call`
  terminators targeting the drop-glue `Instance` for the type.
  Stage 9 terminators do NOT have a separate `Drop` variant;
  there is no exception path for drops. The bridge synthesizes
  drop-glue `Instance`s as part of the mono-item closure (per
  invariant 9 in the spec).

- **Vtable synthesis for `dyn Trait`.** When MIR contains a
  `dyn Trait` value, stage 9 (with stage 12) synthesizes the
  vtable as a Static `Instance` whose value is a mixed-aggregate
  matching rustc's `tcx.vtable_entries(trait_ref)` sequence:
  drop-glue fn pointer, size, alignment, then per-trait-method
  fn pointers, supertrait vtable pointers, and `Vacant` slots
  for non-dispatchable methods. Each method fn pointer is the
  lifted `Instance` for the corresponding method. Dynamic
  dispatch becomes an indirect call through the loaded fn
  pointer. The precise vtable struct `Ty` lands here.

- **Possibly extend `DefPathData` for synthetic vtable scopes.**
  If stage 9 needs a *new* `DefPathData` variant for synthetic
  vtable scopes (rather than overloading an existing variant),
  appending it is a stage-1 revision: append to the `DefPathData`
  enum + the `def_path_data_sort_rank` rank table at the next
  free rank (currently 19). Trigger Codex review of the addition
  per the toolchain-bump methodology event. The
  `anon_namespace_name` table at
  `crates/regseq-model/src/def_path.rs` would also need a new
  arm for the new variant.

- **Closes construction invariant 9.** Once stage 9 introduces
  call-site references to `InstanceId`, validate's invariant 9
  (instance closure-completeness) becomes enforceable
  non-vacuously. Stage 9 walks call sites in `validate` and
  checks each `InstanceId` resolves; positive + negative test in
  `tests/model.rs`.

## Bridge concerns

- **Drop-glue `Instance` interning.** Drop-glue is one of rustc's
  shim variants tracked by `RustcInstanceKindTag::DropGlue`.
  The bridge's instance-interning HashMap (transient) must
  separate drop-glue from `Item`-kind instances of the same
  `DefId` via the `fn_shim_disambig` discriminator (per the F4-2
  RegseqInstanceKey rule).

- **`tcx.vtable_entries(trait_ref)` consumption.** Bridge code
  reads this rustc API to materialize vtable contents.
