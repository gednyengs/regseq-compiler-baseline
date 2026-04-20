# RegseqModel Refactor: Enriched Execution Plan

## Context

The Regseq compiler generates OS-specific C device drivers from OS-agnostic Rust specs. The current internal representations -- three separate models (`DeclarationModel`, `BindingModel`, `JoinedArtifact`), an expression-tree semantic IR (`Stmt`/`Expr`), string-based identity (`CanonicalPath`), and domain-specific types (`RegisterBlock`, `ResourceHandle`) -- make it difficult to produce correct driver code. The new spec (spec.md, ir-reference.md, compiler-knowledge.md) defines a unified `RegseqModel` with CFG-based IR, typed flat IDs, and a clean type system. This plan sequences the refactor to reach that target.

**Register access design (settled):** Register access intrinsics are methods on `Mmio<T>` directly (`Mmio::read`, `Mmio::read_field`, `Mmio::write`, `Mmio::write_field`), each marked with `#[rseq::intrinsic("mmio_...")]`. Similarly, `Clock`, `ResetLine`, `PinCtrl` carry their own intrinsic methods. `ReadOnly`/`WriteOnly`/`ReadWrite` remain as register layout types in the block struct for access-kind enforcement, but do NOT carry intrinsic methods. The spec documents (spec.md, compiler-knowledge.md, ir-reference.md) are now consistent on this design.

---

## Phase 0: Foundation Types

**Goal:** Build the leaf types all other new code depends on. No changes to existing modules.

**New module: `crates/regseq-model/src/identity/`**

| Step | File | Types | Notes |
|------|------|-------|-------|
| 0.1 | `identity/symbol.rs` | `Symbol(u32)`, `FileId(u32)`, `SymbolInterner` | Custom serde: serialize as resolved string. Simplest approach: carry both `u32` and `String` internally, serialize the string. |
| 0.2 | `identity/decl_id.rs` | `DeclId(u32)` + 9 wrappers: `ModuleId`, `ConstantId`, `FunctionId`, `TypeDefId`, `RegisterDescriptorId`, `RegisterBlockId`, `ResourceId`, `PropertyId`, `UserDataId`. Also `LocalId(u32)`, `BasicBlockId(u32)` (body-level). | All `Copy + Clone + Eq + Hash + Serialize + Deserialize`. |
| 0.3 | `identity/source.rs` | `SourceLocation { file, start_line, start_col, end_line, end_col }`, `SourceOrigin { Source(SourceLocation) \| CompilerGenerated }` | |
| 0.4 | `identity/mod.rs` | Re-exports | |

**New module: `crates/regseq-model/src/ir/`** (leaf types only)

| Step | File | Types | Notes |
|------|------|-------|-------|
| 0.5 | `ir/types.rs` | `RegseqType` (10 variants), `ScalarType` (10 variants), `Mutability` | Recursive variants boxed. |
| 0.6 | `ir/ops.rs` | `BinOp` (26 variants), `UnOp` (2 variants: `Not`, `Neg`) | |

**Verification:** Unit tests constructing instances, serde round-trip for each type.

---

## Phase 1: IR Types

**Goal:** Build the CFG-based IR type hierarchy. Depends on Phase 0.

| Step | File | Types | Notes |
|------|------|-------|-------|
| 1.1 | `ir/place.rs` | `Place { local, projection }`, `Projection` (6 variants) | `Field` carries `name: Option<Symbol>` and `ty: RegseqType`. |
| 1.2 | `ir/operand.rs` | `Operand` (4 variants), `LiteralValue` (11 variants) | |
| 1.3 | `ir/rvalue.rs` | `Rvalue` (8 variants), `BorrowKind`, `CastKind` (6), `AggregateKind` (3) | |
| 1.4 | `ir/intrinsic.rs` | `RegseqIntrinsic` (17 variants), `RegseqAbstraction` (9 variants), `IntrinsicMetadata` (`Log`, `Register`, `PinCtrl`), `FormatPlaceholder`, `FormatKind`, `RegisterFieldMeta` | |
| 1.5 | `ir/statement.rs` | `Statement { kind }`, `StatementKind` (5 variants), `MirIntrinsic` (2 variants) | |
| 1.6 | `ir/terminator.rs` | `Terminator { kind }`, `TerminatorKind` (7 variants), `SwitchTargets` | `IntrinsicCall` carries `metadata: Option<IntrinsicMetadata>`, `error_target: Option<BasicBlockId>`. |
| 1.7 | `ir/body.rs` | `Body`, `BasicBlock`, `LocalDecl`, `OrderedRegion`, `BodySourceMap`, `BasicBlockSourceMap`, `Body::predecessors()` | |
| 1.8 | `ir/mod.rs` | Re-exports | |

**Key design decision:** Existing `declaration/mir_body.rs` types remain as the extraction serialization format. New `ir/` types are the processed output. Both coexist.

**Verification:** Construct a hand-built `Body` (3 blocks, straight-line), verify serde round-trip.

---

## Phase 2: Model Structure

**Goal:** Build the unified `RegseqModel`. Depends on Phase 0 + 1.

**New module: `crates/regseq-model/src/model/`**

| Step | File | Types |
|------|------|-------|
| 2.1 | `model/framework.rs` | `FrameworkTypeKind` (10 variants), `TypeDefClassification` |
| 2.2 | `model/type_def.rs` | Type definition with `id`, `name`, `kind`, `classification`, fields/variants, `attrs` |
| 2.3 | `model/register.rs` | `RegisterDescriptor`, `RegisterBlock` |
| 2.4 | `model/constant.rs` | Constant with `id`, `name`, `ty`, `value` |
| 2.5 | `model/function.rs` | `Function { id, params, return_type, body, attrs }`, `Param`, `ParamKind` |
| 2.6 | `model/resources.rs` | Per-kind resource decl/binding types, `Shape` |
| 2.7 | `model/driver.rs` | Driver struct with metadata, resource/property/user-data fields |
| 2.8 | `model/impl_block.rs` | `ImplBlock` |
| 2.9 | `model/module.rs` | `Module { name, kind, items, submodules, abstractions_used, intrinsics_used }` |
| 2.10 | `model/regseq_model.rs` | `RegseqModel { symbols, source_locations, root_module, format_version }` |
| 2.11 | `model/mod.rs` | Re-exports |

**Update `crates/regseq-model/src/lib.rs`:** Add `pub mod identity;`, `pub mod ir;`, `pub mod model;`.

**Verification:** Construct a minimal `RegseqModel` by hand, verify serde round-trip.

---

## Phase 3: Facade Crate, Macros, and Fixture Updates

**Goal:** Restructure register access surface, add `#[rseq::]` attributes, implement `register_block!` macro, update `ordered!` macro, update PL011 fixture. Independent of Phases 0-2 (can run in parallel). Required by Phase 4+.

### 3.1: Framework type annotations

Add `#[rseq::framework_type("...")]` to types in `crates/regseq/src/`:

| File | Type | Attribute |
|------|------|-----------|
| `error.rs` | `RegseqError` | `#[rseq::framework_type("regseq-error")]` |
| `serial.rs` | `SerialErrors` | `#[rseq::framework_type("serial-errors")]` |
| `serial.rs` | `RxChar` | `#[rseq::framework_type("rx-char")]` |
| `serial.rs` | `SerialConfig` | `#[rseq::framework_type("serial-config")]` |
| `serial.rs` | `DataBits` | `#[rseq::framework_type("data-bits")]` |
| `serial.rs` | `Parity` | `#[rseq::framework_type("parity")]` |
| `serial.rs` | `StopBits` | `#[rseq::framework_type("stop-bits")]` |
| `serial.rs` | `FlowControl` | `#[rseq::framework_type("flow-control")]` |
| `time.rs` | `Duration` | `#[rseq::framework_type("duration")]` |
| `irq.rs` | `IrqResult` | `#[rseq::framework_type("irq-result")]` |

### 3.2: Abstraction type annotations

Add `#[rseq::abstraction("...")]` to types in `crates/regseq/src/`:

| File | Type | Attribute |
|------|------|-----------|
| `clock.rs` | `Clock` | `#[rseq::abstraction("clock")]` |
| `reset.rs` | `ResetLine` | `#[rseq::abstraction("reset")]` |
| `pinctrl.rs` | `PinCtrl` | `#[rseq::abstraction("pinctrl")]` |
| `irq.rs` | `Interrupt` | `#[rseq::abstraction("interrupt")]` |
| `mmio.rs` | `Mmio<T>` | `#[rseq::abstraction("mmio")]` |
| `sync.rs` | `Mutex<T>`, `SpinLock<T>`, `IrqSpinLock<T>`, `Completion` | `#[rseq::abstraction("mutex")]`, etc. |

### 3.3: Register access surface restructure + intrinsic annotations

**This is a breaking change to the authored surface.** Register access intrinsics move from `ReadOnly`/`WriteOnly`/`ReadWrite` wrapper methods to `Mmio<T>` methods.

**Changes to `crates/regseq/src/`:**

1. **`mmio.rs`**: Add register access methods to `Mmio<T>`:
   - `read<W, R>(&self, reg: RegisterId<W, R>) -> W` with `#[rseq::intrinsic("mmio_read")]`
   - `read_field<W, R>(&self, field: Field<W, R>) -> W` with `#[rseq::intrinsic("mmio_read_field")]`
   - `write<W, R>(&self, reg: RegisterId<W, R>, value: W)` with `#[rseq::intrinsic("mmio_write")]`
   - `write_field<W, R>(&self, values: FieldValue<W, R>)` with `#[rseq::intrinsic("mmio_write_field")]`
   - `modify<W, R>(&self, values: FieldValue<W, R>)` -- NOT an intrinsic; body calls `self.read()` then `self.write()`
   - All constrained by `R: RegisterOf<Block>` + `R::Access: ReadAccess`/`WriteAccess`

2. **`registers.rs`**: Add supporting types:
   - `RegisterOf<Block>` trait with `type Access`
   - `ReadAccess`, `WriteAccess` traits
   - `ReadOnlyAccess`, `WriteOnlyAccess`, `ReadWriteAccess` marker types with appropriate impls
   - `RegisterId<W, R>` type (per-register constant identifying the full register)
   - `ReadOnly<T, R>`, `WriteOnly<T, R>`, `ReadWrite<T, R>` become **layout-only types** -- remove their `read`/`write`/`modify` methods. Keep `#[repr(transparent)]` for block layout.

3. **Intrinsic annotations on abstraction methods:**

| File | Method | Canonical name |
|------|--------|----------------|
| `clock.rs` | `Clock::enable` | `clock_enable` |
| `clock.rs` | `Clock::disable` | `clock_disable` |
| `clock.rs` | `Clock::get_rate` | `clock_get_rate` |
| `reset.rs` | `ResetLine::assert` | `reset_assert` |
| `reset.rs` | `ResetLine::deassert` | `reset_deassert` |
| `reset.rs` | `ResetLine::toggle` | `reset_toggle` |
| `pinctrl.rs` | `PinCtrl::apply` | `pinctrl_apply` |
| `intrinsics/timed_delay.rs` | `busy_wait` | `busy_wait` |
| `intrinsics/timed_delay.rs` | `sleep` | `sleep` |
| `intrinsics/diagnostic_logging.rs` | `error` | `log_error` |
| `intrinsics/diagnostic_logging.rs` | `warn` | `log_warn` |
| `intrinsics/diagnostic_logging.rs` | `info` | `log_info` |
| `intrinsics/diagnostic_logging.rs` | `debug` | `log_debug` |
| `intrinsics/register_access.rs` | `poll_until` | `poll_until` |

### 3.4: ErrorKind removal

Remove `pub type ErrorKind = SerialErrors;` from `serial.rs` and its re-export in `lib.rs`.

### 3.5: `ordered!` macro migration

**Current:** `ordered!` expands to `memory_barrier()` intrinsic calls.
**Target:** `ordered!` expands to `regseq::internal::ordered_region_begin()` / `ordered_region_end()` marker calls.

Update `crates/regseq-macros/src/ordered.rs`. Add marker functions in `crates/regseq/src/` (`internal` module -- compiler markers, not real intrinsics).

MUST happen before Phase 5 Rule 5 (OrderedRegion detection).

### 3.6: `register_block!` macro

**New proc macro** in `crates/regseq-macros/src/register_block.rs`.

Replaces both `register_bitfields!` (descriptors) and manual `#[repr(C)]` struct (layout) with a single unified definition. The macro syntax matches spec.md Section 10.2:

```rust
regseq::register_block! {
    #[endian = "little", access_width = 32]
    pub pl011 {
        dr: ReadWrite { DATA OFFSET(0) NUMBITS(8) [], OE OFFSET(11) NUMBITS(1) [], ... },
        fr: ReadOnly { RXFE OFFSET(4) NUMBITS(1) [], TXFF OFFSET(5) NUMBITS(1) [], ... },
        cr: ReadWrite { UARTEN OFFSET(0) NUMBITS(1) [DISABLED = 0, ENABLED = 1], ... },
        // ...
    }
}
```

Generates a scoped module:
- `pl011::Regs` -- `#[repr(C)]` layout struct, used as `Mmio<pl011::Regs>`
- `pl011::dr`, `pl011::fr`, `pl011::cr` -- per-register modules with scoped `Field<W, R>`, `FieldValue<W, R>`, `RegisterId<W, R>` constants, unique `Register` marker enum
- `pl011::dr::OFFSET`, etc. -- byte offset constants
- `pl011::dr::REG` -- `RegisterId` constant for full-register read/write
- `RegisterOf<pl011::Regs>` trait impls carrying access kind

### 3.7: PL011 fixture updates

Comprehensive update to adopt the new register architecture:

1. **`registers.rs`**: Replace `register_bitfields!` + `#[repr(C)] struct Pl011Registers` with single `register_block!` call producing scoped `pl011` module.

2. **`driver.rs` imports**: Change from:
   ```rust
   use crate::registers::{Pl011Registers, CR_REG, DR_REG, FR_REG, ...};
   ```
   to:
   ```rust
   use crate::registers::pl011::{self, dr, fr, cr, lcr_h, ibrd, fbrd, ifls, imsc, icr, mis};
   ```

3. **`driver.rs` struct**: Change `regs: Mmio<Pl011Registers>` to `regs: Mmio<pl011::Regs>`.

4. **`driver.rs` register access**: Change all calls from wrapper methods to Mmio methods:
   - `self.regs.cr.write(CR_REG::UARTEN::ENABLED + ...)` -> `self.regs.write_field(cr::UARTEN::ENABLED + ...)`
   - `self.regs.fr.read(FR_REG::RXFE::FIELD)` -> `self.regs.read_field(fr::RXFE)`
   - `self.regs.dr.read_raw()` -> `self.regs.read(dr::REG)`
   - `self.regs.lcr_h.modify(LCRH_REG::FEN::DISABLED)` -> `self.regs.modify(lcr_h::FEN::DISABLED)`
   - `self.regs.imsc.modify(raw_reg_val::<IMSC_REG::Register>(...))` -> `self.regs.modify(raw_reg_val::<imsc::Register>(...))`

5. **`driver.rs` helpers**: Update `field_val`, `raw_reg_val`, `field_from_raw` generic parameters from `<DESCRIPTOR_REG::Register>` to `<register_module::Register>`.

6. **`driver.rs` pin states**: Add `#[rseq::pin_state(id = "default")]` to `DefaultState`, `#[rseq::pin_state(id = "sleep")]` to `SleepState`.

### 3.8: Compiler config for release-profile MIR

Update `callbacks.rs` to set `overflow_checks = Some(false)`, `debug_assertions = Some(false)` alongside existing `mir_opt_level = Some(0)`.

**Verification:** `cargo build` on all modified crates. PL011 fixture compiles with new register surface. MIR dump shows no `Assert` terminators or `*WithOverflow` ops.

---

## Phase 4: Extraction Pipeline Updates

**Goal:** Update `regseq-driver` to recognize new attributes and build registries. Depends on Phase 3.

### 4.0: Cross-crate attribute validation (FIRST)

Validate that `#[rseq::intrinsic("...")]` attributes on facade crate types are visible via `tcx.get_attrs(def_id)` when extracting a dependent crate. **This gates the rest of Phase 4.**

### 4.1: New extraction modules

| File | Purpose |
|------|---------|
| `extract/intrinsic_registry.rs` | Collect `#[rseq::intrinsic("...")]` -> `HashMap<DefId, String>` |
| `extract/framework_types.rs` | Collect `#[rseq::framework_type("...")]` -> `HashMap<DefId, String>` |
| `extract/abstraction_types.rs` | Collect `#[rseq::abstraction("...")]` -> `HashMap<DefId, String>` |

Replace path-based heuristics. Keep heuristic fallback if 4.0 reveals limitations.

### 4.2: Pin-state attribute extraction

Extend `extract/pinctrl.rs` for `#[rseq::pin_state(id = "...")]`.

### 4.3: Declaration-level source locations

Add span extraction for all items with a `DefId`. Store in `RawFrontendArtifact`. Populates `RegseqModel.source_locations` during normalization.

### 4.4: `RawFrontendArtifact` expansion

Add new surface fields: `framework_types`, `abstraction_types`, `intrinsic_registry`, `source_locations`. Wire into `callbacks.rs`.

### 4.5: Unit test specimen updates

Update 33+ test specimens to carry `#[rseq::]` attributes.

### 4.6: Golden file regeneration

Regenerate golden JSON files. Update E2E assertions.

**Verification:** `cargo test` on `crates/regseq-driver/` and E2E tests pass.

---

## Phase 5: IR Construction Pass

**Goal:** Build the MIR-to-Regseq-IR translation. Hardest phase. Depends on Phases 1, 2, 4.

### 5.1: Direct IR construction (no intermediate AnnotatedMirBody)

Do IR construction directly from existing `MirBody` JSON + attribute registries from Phase 4.

**New file:** `crates/regseq-model/src/ir/construction.rs`

### 5.2: Pass 1 -- Declaration skeleton

1. Create `SymbolInterner`, intern all names
2. Walk modules, assign `ModuleId`s, determine Driver vs Normal
3. For each item: assign typed `DeclId`s, record types with classification, register descriptors/blocks, constants
4. Map `identity_enrichment` facts into Module/ImplBlock/Function placement
5. For driver module: record resource fields, property fields, user data fields
6. Build `Module` tree with empty function bodies
7. Populate `source_locations` from Phase 4.3 data

### 5.3: Pass 2 -- Body construction

For each function with a `MirBody`:
1. Map `MirLocalDecl` -> `LocalDecl` with `RegseqType` and interned `Symbol` name
2. Walk basic blocks, translate statements and terminators with recognition rules
3. Drop cleanup blocks (`is_cleanup` ending in `resume`)
4. Build `BodySourceMap`
5. Attach `Body` to owning `Function`

### 5.4: Recognition rules

**Rule 1: Intrinsic recognition.** Callee path in intrinsic registry -> `IntrinsicCall`. For method intrinsics on `Mmio<T>`, `Clock`, etc., the `&self` receiver is preserved as the first arg. The receiver's Place expression identifies the resource.

**Rule 2: `?`-pattern folding.** Match MIR pattern: `Call -> Try::branch -> SwitchInt(ControlFlow) -> Continue/Break`. Fold into `error_target`.

**Rule 3: Core library function extraction.** `Option::map`, `Option::unwrap_or`, `<FieldValue as Add>::add`, etc. extracted as regular `Function`s with bodies.

**Rule 4: Deref collapse.** `<Mmio<T> as Deref>::deref` Call -> transparent access. Note: with intrinsics on `Mmio<T>`, the primary register access path does NOT go through Deref. Deref collapse is for cases where the user accesses the inner block directly (rare) or where `modify()`'s body accesses block internals.

**Rule 5: OrderedRegion detection.** Paired `ordered_region_begin()`/`ordered_region_end()` markers -> `OrderedRegion { entry, exit }`.

**Rule 6: Logging extraction.** Template string + LogArg carriers -> `IntrinsicMetadata::Log`.

**Rule 7: Register metadata.** For register intrinsics on `Mmio<T>`, resolve monomorphized type parameters: `Block` from receiver type, `R` (register marker) from type parameter, `R::Access` for access kind, field argument for mask/shift -> `IntrinsicMetadata::Register`.

### 5.5: Incremental implementation order

1. Straight-line code (Assign, Call, IntrinsicCall, Return) -- test: `disable_uart()`
2. SwitchInt for if/else -- test: `tx_ready()`, `send_break()`
3. `?`-pattern folding -- test: `enable_clock_if_present()`
4. Loop detection (back-edge Goto) -- test: `flush_rx_fifo()`
5. OrderedRegion detection -- test: `program_polling_port()`
6. Logging metadata extraction -- test: (add logging call to fixture if none exists)
7. Register metadata resolution -- test: all register access functions
8. `modify()` body extraction -- test: `disable_fifo()`, `configure_line_control()` (verify `Mmio::modify` body contains two IntrinsicCalls)
9. Volatile read-to-dead-temp -- test: `flush_rx_fifo()` (`let _ = self.regs.read(dr::REG)`)

### 5.6: Cross-crate function extraction

Validate monomorphized MIR for `Option::map`, `Option::unwrap_or`, `<FieldValue as Add>::add` is available.

**Verification:** IR for each PL011 function, serialized to JSON, matches expectations.

---

## Phase 6: Normalization + Join Update

**Goal:** Wire IR construction into the pipeline. Produce `RegseqModel`.

### 6.1: Phase 1 normalization

`fn normalize(input: &RawArtifactInput) -> Result<RegseqModel, NormalizeError>`

### 6.2: Phase 2 join

`fn populate_bindings(model: &mut RegseqModel, platform: &PlatformFile) -> Result<(), JoinError>`

### 6.3: Phase legality verification

Post-Phase-1 and Post-Phase-2 checks. Module tree invariants, ID resolution, type consistency.

**Verification:** Full pipeline for PL011: extraction -> normalize -> join -> verify.

---

## Phase 7: Consumer Migration + CFG Emission

> **Redesigned.** The original Phase-7 framing ("migrate consumers via a
> `compat.rs` adapter that projects `RegseqModel` back to the legacy
> shape") was retired after an implementer blocker. The authoritative
> design now lives in `docs/discussions/models/regseq-model/migration.md`
> (sections Q0-Q8). In particular: (a) no `compat.rs` adapter; consumers
> migrate forward against `RegseqModel` directly or are deleted
> outright; (b) Phase-7 absorbs what was formerly Phase-8.1 (goto-based
> CFG C emission) because `linux/emit_semantic.rs` was pre-written in
> the tree and would otherwise straddle the boundary; (c) validation is
> a layered D' stack, not old-vs-new byte-parity. See migration.md Q5
> for the validation stack and Q6 for the ordered migration plan.

**Goal:** Migrate consumers forward to `RegseqModel`, rewrite the
Linux backend's body emission over CFG IR, delete the superseded
lowering/emission submodules and old model modules, and prove the
new pipeline end-to-end (contract + construct tests + blessed
reference + host-build + determinism + QEMU functional smoke +
CFG structural invariants).

### Sub-phases (per migration.md Q6)

- **7.0 Oracle + infra foundation** (parallel): emission contract +
  construct exemplars; new `backends::c` AST; QEMU boot/probe harness
  productized from the frozen PL011 eval-env baseline
  (`experiments/pl011-eval-env/`).
- **7A Analysis foundation**: `regseq-model::query` helpers;
  `LintFinding` API on `verify_phase_1`; rewrite `regseq-analysis`;
  delete `AnalysisInput` and `body_readiness`.
- **7B Lowering**: new `RegseqModel`-based lowering entry alongside
  old; trim `DriverLoweringPlan` (delete `behavior`,
  `legacy_behavior`, `callback_actions`; relocate
  reporting-only sequencing/metadata); fix `LinuxBackendPlan`'s leaky
  emission boundary.
- **7C CFG emission (absorbs former 8.1)**: retarget
  `linux::emit_semantic` and `linux::emit_module` to walk CFG `Body`
  via the new `backends::c` AST with goto-based control flow; delete
  `linux::emit_behavior`.
- **7C-validation**: bless the PL011 normalized reference -- pass
  required through QEMU functional smoke that exercises probe +
  backend-generated tty open + one-byte write.
- **7D Pipeline cutover**: `emit_driver.rs` swaps to
  `normalize() + populate_bindings() + new lowering entry + new
  emission`.
- **7E E2E retarget**: rewrite legacy-bootstrap E2Es against
  `RegseqModel`; add Test B (CFG structural invariants).
- **7F Cleanup**: delete dead lowering submodules
  (`behavior_planner`, `body_interpreter`, `mir_interpreter`,
  `hierarchy`, `projection`, `semantic_lowering`) and old model
  modules (`declaration/`, `binding/`, `joined/`, `semantic/`,
  `item/`). No `compat.rs` to delete -- it was never created.

**Verification:** The D' validation stack (migration.md Q5) all holds:
emission contract + exemplars + construct tests + blessed reference
+ host-build + determinism + QEMU functional smoke + CFG
invariants.

---

## Phase 8: Backend C Emission (shrunk; 8.1 absorbed by 7C)

**Goal:** Dominator-based resource cleanup on lifecycle callbacks,
plus deprecated-code removal not already handled in Phase 7F.

### 8.1: [Absorbed by Phase 7C] Goto-based C emitter

*Moved into Phase 7C.* The CFG-based goto emitter landed as part of
the consumer-migration cut because `linux/emit_semantic.rs` was
already in the tree as a partial replacement for `emit_behavior.rs`;
separating the two would have straddled the Phase-7/8 boundary.
Phase 8 entry point is 8.2.

### 8.2: Dominator-based resource cleanup

For lifecycle callbacks, find reversible intrinsics dominating error
exits. Generate reverse operations.

**Handled cases:** Sequential `?` chain, match on result, early
return in branch, infallible ops in chain, inside ordered region,
non-reversible failure.

**PL011 limitations:** `init()` uses helper-contained optional
acquisitions (Case 7 + Case 2). Compiler emits diagnostic warning.
Each helper handles its own failure internally, so the direct `?`
chain in `init()` is safe for dominator analysis.

### 8.3: Residual deprecated-code removal

Any deprecated backend code not already removed in Phase 7F. Phase
7F removes `lowering::{behavior_planner, body_interpreter,
mir_interpreter, hierarchy, projection, semantic_lowering}` plus
the legacy lowering entry; Phase 8.3 handles anything that surfaces
after 8.2's cleanup work lands.

---

## Phase 9: Cleanup (shrunk; 9.1 absorbed by 7F)

### 9.1: [Absorbed by Phase 7F] Remove old model modules

*Moved into Phase 7F.* Old model modules
(`declaration/`, `binding/`, `joined/`, `semantic/`, `item/`)
delete as part of the Phase-7 cleanup sub-phase, because no
`compat.rs` adapter was built; nothing reads these modules after
Phase 7D cutover. Phase 9 entry point is 9.2.

### 9.2: Final golden file regeneration
### 9.3: Final test cleanup

---

## Gaps and Risks

### Risks

| Risk | Mitigation |
|------|------------|
| `?`-pattern folding correctness | Prototype against PL011 MIR dump. Incremental implementation. Fallback: treat as regular Call |
| Cross-crate attribute visibility | Validate FIRST in Phase 4.0. Fall back to path heuristics if needed |
| `ordered!` macro migration breaks tests | Update macro + tests together in Phase 3.5 |
| `register_block!` macro complexity | Test against PL011's full register set. Ensure generated HIR matches what extractor expects |
| PL011 fixture register surface migration | Large but mechanical change. Every `self.regs.X.method()` -> `self.regs.method(X::...)` |
| PL011 `init()` rollback limitations | Emit diagnostic for interprocedural gaps. Direct `?` chain handled correctly |
| Goto-based C readability | Structured recovery is a follow-up |
| Consumer migration cascade | Adapter layer allows incremental migration |

### What is NOT in scope

- Zephyr backend updates (follows Linux pattern later)
- Conditional cleanup flags for optional resources (Cases 2, 3, 5 -- emit diagnostic)
- Loop induction variable tracking for array cleanup (Case 6)
- Interprocedural resource summaries (Case 7 -- emit diagnostic)
- Closure support in IR (deferred until needed)
- Synchronization intrinsics (deferred until sync-using driver is ported)
- Structured control flow recovery in C emission (follow-up)

---

## Verification Strategy

1. **Per-phase unit tests:** Construction + serde round-trip for each new type
2. **Phase 4.0 gate test:** Cross-crate attribute visibility validation
3. **Incremental IR construction tests:** 9 test targets in Phase 5.5
4. **`modify()` body test:** Verify extracted body contains two IntrinsicCall terminators
5. **Volatile dead-temp test:** `let _ = self.regs.read(dr::REG)` produces RegisterRead that backend does not elide
6. **Cross-crate function extraction test:** `Option::map`, `FieldValue::add` MIR bodies available
7. **OrderedRegion test:** `program_polling_port()` produces correct SESE region
8. **Golden file comparison:** Full extraction pipeline produces expected JSON
9. **End-to-end:** PL011 -> extraction -> model -> analysis -> lowering -> C emission -> compare
10. **Regression:** Existing E2E tests pass throughout

---

## Critical Files Reference

| Purpose | Path |
|---------|------|
| Facade types to annotate | `crates/regseq/src/{clock,reset,pinctrl,mmio,irq,sync,error,serial,time}.rs` |
| Register access restructure | `crates/regseq/src/{mmio,registers}.rs` |
| Intrinsics to annotate | `crates/regseq/src/intrinsics/{timed_delay,diagnostic_logging,register_access,sync}.rs` |
| `ordered!` macro to update | `crates/regseq-macros/src/ordered.rs` |
| `register_block!` macro (new) | `crates/regseq-macros/src/register_block.rs` |
| Extraction entry point | `crates/regseq-driver/src/callbacks.rs` |
| Compiler config (MIR settings) | `crates/regseq-driver/src/callbacks.rs` (lines 38-52) |
| Existing MIR types (keep) | `crates/regseq-model/src/declaration/mir_body.rs` |
| Current normalization (replace) | `crates/regseq-model/src/declaration/normalize.rs` |
| Current join logic (replace) | `crates/regseq-model/src/joined/join.rs` |
| Platform parser (update) | `crates/regseq-model/src/platform_parser.rs` |
| Backend semantic emission (rewrite) | `crates/regseq-backends/src/linux/emit_semantic.rs` |
| Backend MIR interpreter (remove) | `crates/regseq-backends/src/lowering/mir_interpreter/` |
| Backend semantic lowering (remove) | `crates/regseq-backends/src/lowering/semantic_lowering/` |
| PL011 fixture | `tests/fixtures/pl011-full-spec/src/{driver,registers}.rs` |
| Golden files | `tests/fixtures/pl011-full-spec/expected/raw-frontend.json` |
| Unit tests | `crates/regseq-driver/tests/` (33+ files) |
| E2E tests | `tests/e2e/tests/` (9 files) |
