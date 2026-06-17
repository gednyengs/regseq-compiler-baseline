# M2 Codex review — outcome (SOUND after 6 rounds)

The external adversarial reviewer (Codex GPT-5.5, high effort) reviewed the M2
implementation (T06–T09). The loop ran 6 rounds; final verdict **SOUND**. All
findings folded; full `cargo test` green (155 ok suites, 0 failed) throughout.

## Round 1 — UNSOUND-MUST-REVISE (4 blocking + 3 should-fix)
- **B1** `Completion::wait` result discarded without proving the dest is dead →
  wrong-code if the author branches on it. **Fix:** reject a *consumed* wait
  result at recognition (`SyncDiagnostic::CompletionWaitResultConsumed`,
  `acquire.rs::wait_result_consumed`); negative fixture `_sync_os/sync-wait-consumed`.
- **B2** generic-c completion hooks did not assert init (the T08 claim was unmet).
  **Fix:** shared `regseq_lock_require_inited` called by acquire + all completion hooks.
- **B3** oracle ids ≥ `REGSEQ_LOCK_SLOTS` silently bypassed the init/held checks.
  **Fix:** `regseq_lock_bounds(id)` aborts out-of-range, called before all indexing.
- **B4** Zephyr `data` undeclared when a lock-bearing driver has no `on_init`.
  **Fix:** `uses_data` now ORs `!sync_inits.is_empty()`.
- **SF** unknown cardinality silently defaulted (→ `UnknownCardinality` rejection);
  ISR test asserted only "some release" (→ exactly-2 both-exits); no init-before-enable
  assertion (→ `spin_lock_init` before `request_irq` in `sync_irq_e2e`).

## Rounds 2–6 — progressive completeness of `wait_result_consumed`
The reviewer then drove the consumed-result taint analysis to completeness over the
intraprocedural model surface (each round a narrower escape):
- **R2** by-REFERENCE consumption (`r.is_ok()` borrows `r`) → 2-pass taint closure
  over `Ref`/`RawPtr`/`Use`/`Cast` whole-local aliases; drop-glue exempt via
  `SyntheticItemKind::DropGlue`. Negative fixture `_sync_os/sync-wait-byref`.
- **R3** `StatementKind::Intrinsic` (`copy_nonoverlapping(&r, …)`) → scan `Assume` +
  `CopyNonOverlapping` operands.
- **R4** `Call` indirect callee operand + `Assert` `AssertKind` operands → scanned.
- **R5** move/borrow into a PROJECTED destination (`pair.0 = move tmp`) → the
  taint-building exception now applies only to WHOLE-local destinations; a projected
  destination is a consume.
- **R6 — SOUND.** Invariant confirmed: a tainted local in ANY source position is a
  consume EXCEPT a whole-local taint-build or the drop-glue arg. No false positive on
  `let _ = wait()`.

Negative fixtures + tests added (all in `sync_os_e2e`): `completion_wait_consumed_result_is_rejected`
(`?`), `completion_wait_consumed_by_borrow_is_rejected` (`.is_ok()`),
`unknown_cardinality_is_rejected` (ident), `non_identifier_cardinality_is_rejected` (literal).

## Scope note
The analysis is **intraprocedural** by design (GRILL I04); a result escaping into
another body is the deferred Open-4 case. Codex judged soundness within that scope.
