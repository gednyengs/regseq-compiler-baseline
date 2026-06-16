---
feature_id:   F-backend-zephyr-c-e1000
feature_slug: backend-zephyr-c-e1000
artifact_id:  GATE-REPORT-2026-06-11-D08-log-levels
type:         gate-report
status:       closed
source:       [GRILL-2026-06-11-100437, PRD-2026-06-11-100716]
created:      2026-06-11
---

# Gate-report — the D08(1) "four LOG levels" criterion vs the frozen fixture (C5 review blocking finding)

**Boundary:** the Codex C5 chunk review (round 1, blocking finding 1)
correctly observed that GRILL D08(1) / PRD R11 word the happy-path L2
criterion as "authored LOG lines at all four levels visible", while the
composed leg's gate (40-smoke-gate.sh) enforces dbg/inf/wrn only and the
live console carries no authored err line on the happy path.

**Why the wording cannot be satisfied as written:** the frozen
`e1000-composed` fixture's three authored `log::error!` sites are ALL
inside failure/absent paths — the TCTL readback-fail arm of `on_init`
(input.rs:498, followed by `return Err(HardwareError)`), the
`on_surprise_remove` body (input.rs:522 — the AbsentDispatch slot, never
dispatched on this target), and the `on_init_failure` hook
(input.rs:529). On a PASSING bring-up none executes, BY DESIGN: an
authored err line on the happy path would mean the bring-up failed. The D08(1) wording inherited the pl011 shape
(whose orderly path happens to fire all four levels); for THIS fixture,
"all four levels on the happy path" and "the happy path passes" are
mutually exclusive. Editing the fixture to add a reachable err line is
the born-converged wall (I03: the ONE input.rs serves three backends and
two frozen legs) and would also dilute err-level semantics.

**Disposition (the criteria amendment, recorded — never a silent relax):**
D08(1)'s log clause is amended for this unit to its achievable intent —
*"every authored level the orderly path FIRES is visible through the live
log core (dbg/inf/wrn for this fixture), with the err-level live-log
witness delivered by the initfail leg (same prj.conf level pin, same
deferred log core, same image family)"*. The initfail leg's gate REQUIRES
both authored LOG_ERR lines (40-smoke-gate.sh initfail mode) and passed
live (results/l2-initfail-2026-06-11). The err LEVEL is therefore
runtime-witnessed in this unit's L2 evidence set; only its co-residence
with a passing bring-up is dropped, which the fixture's own semantics
forbid. This mirrors the linux-leg precedent (the e1000-composed linux
oracle suite records "error is the readback-fail path, dynamically
absent" — e1000_composed_e2e.rs).

**Scope:** evidence-criteria interpretation only; no code, fixture,
golden, or capability-row change. The T14 gate carries this report in its
evidence set.
