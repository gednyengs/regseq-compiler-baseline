# Shadow Registers: Codex Adversarial Debate

Two-round Codex adversarial review transcripts supporting the
design and implementation plan at `../shadow-registers.md`.
Preserved in-repo so that future sessions can audit the design
provenance without depending on ephemeral `/tmp` files.

## Round summaries

### Round 1 -- `round-1-design-review.md`

Design-level review of the original shadow-register proposal (per-
descriptor attribute form). Verdict: **adopt-with-amendments**.

Six amendments were folded into the design in `../shadow-registers.md`:

1. Attach shadow metadata to register-block fields, not reusable
   descriptors (shadow is per-instance, not per-layout).
2. Define shadow semantics for every register-access intrinsic
   (originally six: `read`, `read_field`, `write`, `write_field`,
   `modify`, `force_read`).
3. Add explicit concurrency/ISR ordering rules or require caller-
   held synchronization (adopted the latter via `#[rseq::isr]`
   exclusion).
4. Add side-effect eligibility metadata before claiming W1C/read-
   clear rejection (deferred; Stage 1 validates only `WriteOnly`
   rejection).
5. Represent compiler-managed shadow state separately from user
   data (introduced the `ShadowState` category on `DriverStruct`).
6. Lower shadow semantics at the semantic IR level, not as a Linux-
   only rewrite (standalone pass before `call_lowering`; all
   backends see post-rewrite IR).

### Round 2 -- `round-2-plan-review.md`

Implementation-plan review. Verdict: **needs-plan-amendment**. All
amendments folded into `../shadow-registers.md` Implementation
plan section:

- Adopt `#[rseq::register_entry(shadow = true, reset_value = <lit>)]`
  attribute syntax (vs trailing-brackets or inline-fields
  alternatives). Codex Q3.1.
- Metadata on both `RegisterBlockField` (source of truth) AND
  `DriverStruct.shadow_state` (backend storage planning). Codex Q3.2.
- Standalone rewrite pass before `call_lowering`, not a branch
  inside `call_lowering`. Codex Q3.3.
- Explicit IR variants (not a parameterized `ShadowOp`). Codex Q3.4.
- Keep PL011's explicit IMSC hardware writes in `init()` /
  `port_close()` -- compiler-generated shadow init initializes
  DRIVER STORAGE, not hardware. Codex Q3.5.
- One harness task per phase per the repo's task-spec workflow
  (`.harness/task-spec-workflow.md:120`), not combined super-tasks.
  Codex Q3.6.
- Added per-phase touch-points not in the original plan:
  normalization (raw_artifact, surface), Phase-1 and Phase-2
  verifier entries, cross-item declared-ID bookkeeping, RegisterIndex
  consumers (register_metadata.rs:338, 452, 569), intrinsic enum
  match sites (intrinsic.rs:20, 65, 96), legacy emit_behavior.rs
  path, Zephyr NotImplemented stub.
- Revised effort estimate from 9.5-15.5 to 14-22 days.

Additional user clarifications (2026-04-22) folded in after the
round:
- `Mmio::modify` composes `read` + `write`; no `ShadowModify` IR
  node required; effort revised down to 13-20 days.
- Path-X sequencing: user-data Path-A lands first; shadow-register
  feature deferred until post-pipe-clean.

## How to add new transcripts

If a future round opens (e.g., Round 3 re-review after
implementation kickoff):

1. Save each Codex round output as a new file in this directory,
   named `round-N-<description>.md`.
2. Update this README's "Round summaries" section.
3. Reference the new round from `../shadow-registers.md` so the
   design doc's provenance stays auditable.

Do NOT reference ephemeral `/tmp/` paths from committed docs --
future sessions will not have those files.
