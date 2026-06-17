# serial-uart-management — T04 (frontend enforcement) — COMPLETE

**Status:** done 2026-06-17. PLAN T04 (W-A diagnostics). Codex review loop **converged →
DEFENSIBLE** (round 1 UNSOUND → round 2 UNSOUND → round 3 DEFENSIBLE). Snapshot/regression green;
the in-scope serial goldens (generic/linux/zephyr) stay **byte-frozen**.

## What landed

The serial bridge (`regseq-driver/src/bridge/serial.rs`) now fails-loud at the **acquire** stage —
BEFORE the serial IR node is built and BEFORE `resolve_isrs` — on the three T04 reject gates (R07 +
R10). The backend never sees a rejected or partially-built serial surface.

- **The reject matrix (3 `SER-ERR-*` diagnostics, `bridge/mod.rs`):**
  - `SER-ERR-DYNAMIC-IRQ-SET` (R07/D03/I01) — any author body that **references** the backend-only
    `irq_source_set` (call, fn-pointer reification, or closure).
  - `SER-ERR-DEFERRED-MEMBER` (R10/C03/C05) — an author OVERRIDE of a v1-deferred member inside an
    in-scope trait: `SerialRuntimeLineConfig::{line_format_readback, break_set, flow_auto_set}` and
    `SerialQueueControl::fifo_purge_hw`.
  - `SER-ERR-DEFERRED-TRAIT` (R10/C06–C08) — an impl of a deferred companion trait
    `SerialModemControl` / `SerialConsole` / `SerialBulkTransfer` on a serial-class driver.
- **The checks (all keyed off a single `driver_impl_blocks` scan = every trait+inherent impl whose
  Self is the driver):**
  - `reject_deferred_traits` — reject if any impl's trait NAME ∈ the deferred-trait set.
  - `reject_deferred_members` — for the in-scope `SerialRuntimeLineConfig`/`SerialQueueControl`
    impls, reject if the impl block DEFINES (overrides) a deferred member. `tcx.associated_items`
    on an impl returns only the items the block defines, not inherited trait defaults — so an
    in-scope impl that overrides only `line_format_apply`/`tx_drain_query` is clean.
  - `reject_dynamic_irq_source_set` — a bounded BFS (`body_tree_references_irq_source_set`) over each
    non-`irq_source_set` driver method + every closure it constructs (discovered via
    `Aggregate(Closure)` AND `TyKind::Closure` locals, transitively). A read-only MIR `Visitor`
    (`IrqSourceSetRefFinder`) flags any `Operand::Constant` of `FnDef` type resolving (via
    `opt_associated_item().trait_item_def_id()`) to the `irq_source_set` trait method. mir-opt-level
    is 0 (no inlining), so the reference survives.

## Surface + fixtures (W-B)

- **In-scope surface untouched.** The deferred members/traits are NOT added to the in-scope
  `_serial_surface.rs` — doing so emitted dead deferred-type forward-decls + struct defs +
  `_Static_assert`s + derived `Clone`/`Eq` decls into the **generic-c oracle golden**. Instead a
  reject-corpus-only `_serial_deferred_surface.rs` declares the FULL framework-pure vocabulary (the
  in-scope traits with their deferred members carrying default bodies + the deferred companion
  traits + support types). Reject fixtures fail at *acquire* (before emit), so they never produce a
  golden — the deferred surface costs nothing in the oracle.
- **Negative corpus (10 fixtures, `fixtures/_serial/serial-*-reject/`)** gated by
  `tests/serial_reject_e2e.rs` (mirrors `irq_reject_e2e.rs`): each is GREEN-as-a-reject (fails at the
  declared `stage = "acquire"` with its `diagnostic` substring):
  - `serial-dynamic-irq-set-reject` — direct `self.irq_source_set(..)` from a base op.
  - `serial-irq-set-fnptr-reject` — `<Self as SerialInterruptDriven>::irq_source_set` reified to a
    fn pointer + called (the Codex round-1 hole).
  - `serial-irq-set-closure-reject` — the call hidden inside a closure called directly (the Codex
    round-1 hole; caught by the closure BFS).
  - `serial-{line-format-readback,break-set,flow-auto-set,fifo-purge}-reject` — the 4 deferred
    members.
  - `serial-{modem-control,console,bulk-transfer}-reject` — the 3 deferred companion traits.

## Codex review loop (UNSOUND → UNSOUND → DEFENSIBLE)

- **Round 1 → UNSOUND, 2 blocking false-negatives:** the original scan checked only the `Call`
  terminator's `func` operand and only `FnDef`, so (1) a fn-pointer reification of `irq_source_set`
  and (2) a closure/generic trampoline hiding the call slipped through. **Fix:** widened to an
  operand-level MIR `Visitor` (catches the `FnDef` ZST as ANY operand — call, reify, or pass-as-value
  / generic trampoline) + a bounded BFS recursing into constructed closures. Proven by the
  fnptr + closure reject fixtures. (Findings 3–6 were documented scope: the anti-recursion rule "only
  `irq_source_set`'s body may reference it"; name-based recognition is construct-wide;
  `is_mir_available` is always true for author impl fns.)
- **Round 2 → UNSOUND, 2 more:** (A) a closure COERCED to a fn pointer might have no
  `TyKind::Closure` local; (B) a const-promoted fn-pointer table hides the `FnDef` behind a `FnPtr`
  const. **Fix for (A):** discover closures by their `Aggregate(Closure(..))` construction too (not
  only local-decl types). **(A) turned out to be unreachable** — a closure-to-fn-pointer coercion is
  independently rejected EARLIER by the bridge's substantive-HRTB restriction (`extern "rust-call"
  fn(..)` is unsupported DSL), verified live (the fixture rejects via the HRTB gate, so it was
  removed — wrong diagnostic for an unreachable path). The `Aggregate(Closure)` discovery is kept as
  robustness (it catches a closure CALLED directly regardless of local shape).
- **Round 3 → DEFENSIBLE.** Framing: **R07 is a guard rail, not a sandbox.** It cannot be airtight by
  construction — an author owns the mask bits and could replicate the IMSC write directly in a base
  op (an MMIO write the compiler cannot distinguish from any other), so any `irq_source_set`-reference
  check is bypassable by simply not naming the op. The right v1 bar is "block the obvious/idiomatic
  misuse — call, local fn-pointer, capturing closure — and DOCUMENT the boundary." Codex verdict:
  "Documenting that boundary is a defensible guard-rail design."

### Documented guard-rail boundary (NOT check holes)
- (A) closure→fn-pointer coercion — rejected earlier by the HRTB restriction (can't reach T04).
- (B) `const F: fn(..) = <Self>::irq_source_set; F(self,..)` — the `FnDef` is hidden behind a `FnPtr`
  const initializer the scan does not evaluate. Non-idiomatic/deliberate indirection.
- (C) the author replicating the IMSC mask write directly in a base op (never naming the op) —
  inherently undetectable; outside what any `irq_source_set`-reference scan can prove.

## Tests

```
cargo test -p codegen-coverage-runner --test serial_reject_e2e          # the 10-fixture T04 matrix
cargo test -p regseq-driver --test bridge_serial_recognition            # 5 — positive recognition unaffected
cargo test -p regseq-driver --lib                                       # 109
cargo test -p regseq-backends --lib                                     # 411 (shared-emitter regression)
cargo test -p codegen-coverage-runner --test serial_pl011_e2e \
  --test linux_serial_pl011_e2e --test zephyr_serial_pl011_e2e          # in-scope goldens BYTE-FROZEN
```
All PASS. The in-scope serial goldens are byte-identical to pre-T04 (verified: 0 deferred-type
leakage in `golden.c`); the deferred vocabulary lives only in the reject-only surface.
