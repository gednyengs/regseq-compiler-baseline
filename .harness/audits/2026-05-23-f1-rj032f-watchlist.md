# F1 (IX-014 + RJ-032-F watch-list) — RESOLVED: IX-014 already covered, RJ-032-F deferred-pending-phase

**Status**: RESOLVED (2026-05-23). F1 was the F68 audit's LOW
"deferred-pending-external-work watch list" proposal for two
escalations. At HEAD one is already closed and the other is
transparently blocked on an unimplemented phase, so F1 needs no
watch-list bookkeeping beyond a one-row reclassification. Verified at
HEAD on the pinned `nightly-2026-02-01` toolchain. This closes the
**entire original F68 proposal slate** (A1, B1, B2, C1, C2, D1, F1).

---

## 1. IX-014 — already covered (F1 pointer obsolete)

F1 proposed an IX-014 watch-list pointer "toward IX-004 / IX-010
resolution" (path (b)), explicitly deferring the alternative path (a) —
a `SyntheticItemKind` sub-variant predicate — per the F68 task-spec
policy choice. **IX-014 is now already `covered`** (matrix row in the
`## 5` section, `crates/bridge-coverage-runner/snippets/IX-014.rs`):
regseq-driver-71 brought it to PASS via the closure-call rewrite to
`ClosureCall { call_kind: CallOnce }`, matched by the **flat**
`SyntheticItemKind = "ClosureCall"` predicate (`rows.toml:1467`) — i.e.
F71 covered IX-014 without needing path (a)'s sub-variant predicate;
that predicate (`SyntheticItemKindSub`) landed separately in F72 for
SK-004/SK-005, where the matrix note records IX-014's own CallOnce
sub-axis as still masked by the flat predicate. Either way F1's IX-014
watch-list pointer (which targeted path (b)) is **obsolete** — no
action; the row is covered.

## 2. RJ-032-F (D8) — `uncovered` → `blocked-unimplemented-phase`

Per `.harness/expectations/regseq-driver-67.md:393-417`, the D8
`DiagLogValidation` diagnostic short-circuits to `Ok(())`. Confirmed at
HEAD: `crates/regseq-driver/src/bridge/diag_log/peel.rs:179-180` is

```rust
fn target_str_arg_bound(_cx: &BridgeCtxt<'_>) -> Option<usize> {
    None
}
```

with the surrounding comments (`:141-148`, `:174-179`) stating the
target-binding wiring that would populate `max_str_args_per_call` is not
landed, so the helper passes `None` and the D8 check short-circuits. D8
is therefore **unreachable from source** until the backend
target-binding selection lands (spec §26.8/26.9 + the per-target
`<target>-backend.md` artifacts).

This is neither a bug (no panic/reject — it short-circuits to `Ok`), nor
a by-design rejection, nor rustc-infeasible (rustc can produce the
shape), nor a spec gap (the spec describes a realisable witness, just
gated on a future phase). It is the first instance of a distinct
category, so a new matrix status class was introduced:

- **`blocked-unimplemented-phase`** — realisable in principle, but the
  producing bridge/backend path is deliberately stubbed pending a future
  implementation phase; the construct short-circuits rather than
  reaching that path. A fixture becomes authorable when the phase lands.

RJ-032-F is reclassified `uncovered` → `blocked-unimplemented-phase`
with the trigger condition recorded (target-binding wiring per §26.8/26.9).

## 3. Disposition

- **F1 RESOLVED** and removed from the active escalation queue.
- **IX-014**: no action — already `covered` (F71).
- **RJ-032-F**: `uncovered` → `blocked-unimplemented-phase`; revisit when
  the backend target-binding wiring lands.
- New status class `blocked-unimplemented-phase` added to the matrix
  legend.
- This closes the original F68 slate. Remaining work is the
  session-surfaced real-fix backlog (R1 const-fn-ptr-provenance, R2
  tuple-clone ItemMissingInstance, R3 closure-clone ICE) plus the F70
  residuals (FF-005, RV-009-G) and Category 4 (Section 7 validator
  audit).
