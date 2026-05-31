# Generic-C backend — workstream HQ

This folder is the **design + strategy home** for the generic-C backend
workstream: the IR→C lowering effort that maps a validated `RegseqModel`
to a working, host-compilable C translation unit, with stubs for Regseq
domain intrinsics. It is the codegen successor to the bridge-coverage
workstream (which closed the *front-end* feature-complete gate — see
`.harness/archive/2026-05-26-bridge-coverage-runner/HANDOFF.md`).

## What lives here

- **Roadmap (Level 1)** — the phase sequence + per-phase exit gates +
  cross-track (front-end ↔ backend) dependencies.
- **`running.md`** — operational how-to-run guide: emitting C from a real
  crate via `cargo regseq --emit generic-c`, plus the codegen-coverage
  runner. The practical companion to the design docs here.
- **Per-phase plan-first design docs (Level 2)** — one pre-spec design
  doc per phase (the shape of the bridge workstream's
  `.harness/audits/2026-05-25-feature-complete-gate-plan.md`):
  resolve the open architecture decisions, then derive implementation
  tasks.
- **Codegen-coverage matrix** — the design-level enumeration of every
  `RegseqModel` construct and its pinned C lowering + fixture (the
  codegen analog of `../regseq-model/bridge-coverage-matrix.md`).
- **Cross-backend strategy** — design notes meant to serve the future
  per-OS backends (Linux, Zephyr) as well as generic-C.
- **`decisions-and-state.md`** — the durable cold-start digest of locked
  decisions, the phase arc, project orientation, and key file anchors
  (the project-side half of the former single-file handoff; cross-refs
  `roadmap.md` / `phase-1/02` as authoritative). Its process-side sibling
  is `.harness/generic-c/methodology.md`; volatile session state lives in
  `.handoff/` (session-handoff plugin).

## What does NOT live here (the governance split)

- **The canonical C-emission SPEC is `../regseq-model/generic-c-backend.md`**
  — a *governed co-equal merger artifact* (per the regseq-model
  `README.md` §5.9 rule 7 / D-14-10), cross-referenced from `spec.md`
  and `pretty-printer.md`, and wired into 18+ places. It STAYS there and
  is refined in place; this folder does not duplicate or relocate it.
  Future per-OS backend specs (`linux-backend.md`, `zephyr-backend.md`)
  are likewise top-level merger artifacts in `../regseq-model/` under the
  same rule. (A future deliberate consolidation of all backend specs into
  one folder — "option B" — would require updating §5.9 + every
  cross-reference; it is explicitly not done here.)
- **Live pass/fail ledgers, status trackers, and task-spec scratchpads**
  → `.harness/generic-c/` (working state, not design).

## Scope reminder

The generic-C target is **hosted userspace** (libc, `printf`, `main()`),
and the verification boundary is **IR-structural → C that compiles and
runs on a host toolchain** — NOT real-OS integration (that is the
per-target Linux/Zephyr backends). "Codegen-complete" for a domain means
every construct lowers, compiles, runs, and auto-validates on the host.
